#![allow(dead_code)]

use bytes::{Buf, BytesMut};
use chacha20poly1305::{aead::stream::DecryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

use crate::MAX_CHUNK_SIZE;

pub trait AsyncReadable: AsyncRead + Send + Unpin {}
impl<T: AsyncRead + Send + Unpin> AsyncReadable for T {}

struct HelpBuf {
    buf: Vec<u8>,
    l_cursor: usize,
    r_cursor: usize,
}

impl HelpBuf {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity],
            l_cursor: 0,
            r_cursor: 0,
        }
    }

    fn get_spare_capacity(&mut self) -> ReadBuf<'_> {
        ReadBuf::new(&mut self.buf[self.r_cursor..])
    }

    fn advance_r(&mut self, bytes: usize) {
        self.r_cursor += bytes;

    }

    fn rotate_if_at_end(&mut self) {
        if self.r_cursor == self.buf.len() {
            let data_len = self.r_cursor - self.l_cursor;
            let (new_location, the_rest) = self.buf.split_at_mut(data_len);
            new_location
                .copy_from_slice(&the_rest[self.l_cursor - data_len..self.r_cursor - data_len]);
            self.l_cursor = 0;
            self.r_cursor = data_len;
        }
    }

    fn advance_l(&mut self, bytes: usize) {
        self.l_cursor += bytes;

        if self.l_cursor == self.r_cursor {
            self.l_cursor = 0;
            self.r_cursor = 0;
        }
    }

    fn data(&self) -> &[u8] {
        &self.buf[self.l_cursor..self.r_cursor]
    }
}

#[pin_project]
pub struct EncryptedReader<T: AsyncReadable> {
    #[pin]
    reader: T,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    cleartext: BytesMut,
    ciphertext: HelpBuf,
}

impl<T: AsyncReadable> EncryptedReader<T> {
    pub async fn new(mut reader: T, shared_key: [u8; 32]) -> std::io::Result<Self> {
        let mut nonce = [0; 8];
        reader.read_exact(&mut nonce).await?;

        let decryptor = DecryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            reader,
            decryptor,
            cleartext: BytesMut::with_capacity(MAX_CHUNK_SIZE),
            ciphertext: HelpBuf::with_capacity(MAX_CHUNK_SIZE),
        })
    }

    fn inner_read(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.project();

        let mut read_buf = this.ciphertext.get_spare_capacity();
        ready!(this.reader.poll_read(cx, &mut read_buf))?;
        let bytes_read = read_buf.filled().len();

        this.ciphertext.advance_r(bytes_read);
        while let Some(msg) = {
            if let Some(len) = this.ciphertext.data().get(0..4) {
                let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
                this.ciphertext.data().get(4..len+4)
            } else {
                None
            }
        } {
            let mut decryption_space = this.cleartext.split_off(this.cleartext.len());
            decryption_space.extend_from_slice(msg);
            this.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
            this.cleartext.unsplit(decryption_space);
            this.ciphertext.advance_l(msg.len());
        }
        this.ciphertext.rotate_if_at_end();

        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncReadable> AsyncRead for EncryptedReader<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        
        if buf.remaining() > self.cleartext.chunk().len() {
            let poll = self.as_mut().inner_read(cx)?;
            
            if self.cleartext.is_empty() {
                
                if poll == Poll::Pending {
                    return Poll::Pending;
                } else if self.ciphertext.data().is_empty() {
                    return Poll::Ready(Ok(()));
                } else {
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            }
        }

        let chunk = self.cleartext.chunk();
        let num_bytes = std::cmp::min(buf.remaining(), chunk.len());

        buf.put_slice(&chunk[0..num_bytes]);
        self.cleartext.advance(num_bytes);

        Poll::Ready(Ok(()))
    }
}
