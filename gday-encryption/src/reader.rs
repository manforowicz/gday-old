#![allow(dead_code)]

use bytes::{Buf, BufMut, BytesMut, Bytes};
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


#[pin_project]
pub struct EncryptedReader<T: AsyncReadable> {
    #[pin]
    reader: T,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    cleartext: BytesMut,
    ciphertext: BytesMut,
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
            ciphertext: BytesMut::with_capacity(MAX_CHUNK_SIZE),
        })
    }

    fn get_next_cipher_chunk(ciphertext: &mut BytesMut) -> Option<Bytes> {
        if ciphertext.remaining() >= 4 {
            let mut len = [0; 4];
            ciphertext.copy_to_slice(&mut len);
            let len = u32::from_be_bytes(len) as usize;

            if ciphertext.remaining() >= len {
                return Some(ciphertext.copy_to_bytes(len));
            }
        } 

        None
    }

    fn inner_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.as_mut().project();

        let tmp = this.ciphertext.chunk_mut();
        let tmp = unsafe { tmp.as_uninit_slice_mut() };
        let mut read_buf = ReadBuf::uninit(tmp);
        ready!(this.reader.poll_read(cx, &mut read_buf))?;
        let bytes_read = read_buf.filled().len();
        unsafe { this.ciphertext.advance_mut(bytes_read) };

        while let Some(chunk) = Self::get_next_cipher_chunk(this.ciphertext) {
            let mut decryption_space = this.cleartext.split_off(this.cleartext.len());
            decryption_space.extend_from_slice(&chunk);
            this.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
            this.cleartext.unsplit(decryption_space);
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
        
        if buf.remaining() > self.cleartext.chunk().len() {

            if self.cleartext.is_empty() {
                while self.cleartext.is_empty() {
                    ready!(self.as_mut().inner_read(cx))?;
                    if self.cleartext.is_empty() && self.ciphertext.is_empty() {
                        return Poll::Ready(Ok(()));
                    }
                }
            } else {
                let _ = self.as_mut().inner_read(cx)?;
            }
        }

        let chunk = self.cleartext.chunk();
        let num_bytes = std::cmp::min(buf.remaining(), chunk.len());

        buf.put_slice(&chunk[0..num_bytes]);
        self.cleartext.advance(num_bytes);

        Poll::Ready(Ok(()))
    }
}
