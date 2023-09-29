#![allow(dead_code)]
use chacha20poly1305::{aead::stream::DecryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};

use crate::{MAX_CHUNK_SIZE, HelperBuf};

pub trait AsyncReadable: AsyncRead + Send + Unpin {}
impl<T: AsyncRead + Send + Unpin> AsyncReadable for T {}

/*
struct HelperBuf {
    buf: Vec<u8>,
    l: usize,
    r: usize,
}

impl HelperBuf {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity],
            l: 0,
            r: 0,
        }
    }

    fn spare_capacity_mut(&mut self) -> ReadBuf<'_> {
        ReadBuf::new(&mut self.buf[self.r..])
    }

    fn advance_l_cursor(&mut self, num_bytes: usize) {
        self.l += num_bytes;
        assert!(self.l <= self.r);

        if self.l == self.r {
            self.l = 0;
            self.r = 0;
        }
    }

    fn advance_r_cursor(&mut self, num_bytes: usize) {
        self.r += num_bytes;
        assert!(self.r <= self.buf.len());

        if self.r == self.buf.len() && self.peek_cipher_chunk().is_none() {
            let (blank, data) = self.buf.split_at_mut(self.l);
            assert!(blank.len() >= data.len());
            blank[0..data.len()].copy_from_slice(data);
            self.l = 0;
            self.r = data.len();
        }
    }

    fn data(&self) -> &[u8] {
        &self.buf[self.l..self.r]
    }

    fn peek_cipher_chunk(&mut self) -> Option<&[u8]> {
        if let Some(len) = self.data().get(0..4) {
            let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
            if let Some(chunk) = self.data().get(4..4 + len) {
                return Some(chunk);
            }
        }
        None
    }
}

*/

fn peek_cipher_chunk(buf: &mut HelperBuf) -> Option<&[u8]> {
    if let Some(len) = buf.data().get(0..4) {
        let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
        if let Some(chunk) = buf.data().get(4..4 + len) {
            return Some(chunk);
        }
    }
    None
}

#[pin_project]
pub struct EncryptedReader<T: AsyncReadable> {
    #[pin]
    reader: T,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    cleartext: HelperBuf,
    ciphertext: HelperBuf,
}

impl<T: AsyncReadable> EncryptedReader<T> {
    pub async fn new(mut reader: T, shared_key: [u8; 32]) -> std::io::Result<Self> {
        let mut nonce = [0; 8];
        reader.read_exact(&mut nonce).await?;

        let decryptor = DecryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            reader,
            decryptor,
            cleartext: HelperBuf::with_capacity(MAX_CHUNK_SIZE),
            ciphertext: HelperBuf::with_capacity(MAX_CHUNK_SIZE * 2),
        })
    }

    fn inner_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.as_mut().project();


        let old_cipherbuf_len = this.ciphertext.buf.len();

        let spare = this.ciphertext.buf.spare_capacity_mut();
        let mut read_buf = ReadBuf::uninit(spare);
        ready!(this.reader.poll_read(cx, &mut read_buf))?;
        let new_len = old_cipherbuf_len + read_buf.filled().len();
        unsafe { this.ciphertext.buf.set_len(new_len) }

        while let Some(msg) = peek_cipher_chunk(this.ciphertext) {
            let msg_len = msg.len();
            if this.cleartext.spare_capacity_len() < msg_len {
                //println!("capacity: {} len: {}", self.cleartext.capacity(), self.cleartext.len());
                break;
            }

            let cleartext_len = this.cleartext.buf.len();
            let mut decryption_space = this.cleartext.buf.split_off(cleartext_len);
            assert!(decryption_space.spare_capacity_mut().len() >= msg.len());
            
            decryption_space.extend_from_slice(msg);

            this.ciphertext.advance_cursor(msg_len + 4);

            if peek_cipher_chunk(this.ciphertext).is_none() && this.cleartext.spare_capacity_len() == 0 {
                this.ciphertext.wrap();
            }

            this.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;


            this.cleartext.buf.unsplit(decryption_space);
        }

        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncReadable> AsyncRead for EncryptedReader<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        assert!(self.cleartext.buf.capacity() == MAX_CHUNK_SIZE);
        assert!(self.ciphertext.buf.capacity() == 2 * MAX_CHUNK_SIZE);

        if buf.remaining() > self.cleartext.data().len() {
            if self.cleartext.data().is_empty() {
                while self.cleartext.data().is_empty() {
                    ready!(self.as_mut().inner_read(cx))?;
                    if self.cleartext.data().is_empty() && self.ciphertext.data().is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                }
            } else {
                let _ = self.as_mut().inner_read(cx)?;
            }
        }
        
        let chunk = self.cleartext.data();
        let num_bytes = std::cmp::min(buf.remaining(), chunk.len());


        buf.put_slice(&chunk[0..num_bytes]);

        self.cleartext.advance_cursor(num_bytes);

        Poll::Ready(Ok(()))
    }
}
