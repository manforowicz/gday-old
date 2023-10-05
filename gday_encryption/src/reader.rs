#![allow(dead_code)]
use chacha20poly1305::{aead::stream::DecryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, ReadBuf};

use crate::{HelperBuf, MAX_CHUNK_SIZE};

pub trait AsyncReadable: AsyncRead + Send + Unpin {}
impl<T: AsyncRead + Send + Unpin> AsyncReadable for T {}

fn peek_cipher_chunk(buf: &HelperBuf) -> Option<&[u8]> {
    if let Some(len) = buf.data().get(0..4) {
        let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
        buf.data().get(4..4 + len)
    } else {
        None
    }
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

    /// Reads data from the inner reader into self.ciphertext,
    /// and returns the poll returned by the inner reader.
    fn inner_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.as_mut().project();

        let old_cipherbuf_len = this.ciphertext.buf.len();
        let spare = this.ciphertext.buf.spare_capacity_mut();
        let mut read_buf = ReadBuf::uninit(spare);
        ready!(this.reader.poll_read(cx, &mut read_buf))?;

        let new_len = old_cipherbuf_len + read_buf.filled().len();
        unsafe { this.ciphertext.buf.set_len(new_len) }

        Poll::Ready(Ok(()))
    }

    /// true if decrypted all full chunks, false otherwise
    fn decrypt_all_full_chunks(self: Pin<&mut Self>) -> std::io::Result<()> {
        let this = self.project();
        while let Some(msg) = peek_cipher_chunk(this.ciphertext) {
            let msg_len = msg.len();
            if this.cleartext.spare_capacity_len() < msg_len {
                return Ok(());
            }
   
            let mut decryption_space = this.cleartext.buf.split_off(this.cleartext.buf.len());

            decryption_space.extend_from_slice(msg);

            this.ciphertext.advance_cursor(msg_len + 4);


            this.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;

            this.cleartext.buf.unsplit(decryption_space);
        }

        if this.ciphertext.spare_capacity_len() == 0 {
            this.ciphertext.wrap();
        }

        Ok(())
    }

    /// True if eof, false if not. Stops reading when cleartext has length at least wanted_bytes
    fn read_if_necessary(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        wanted_bytes: Option<usize>,
    ) -> Poll<std::io::Result<bool>> {
        debug_assert!(self.cleartext.buf.capacity() == MAX_CHUNK_SIZE);
        debug_assert!(self.ciphertext.buf.capacity() == 2 * MAX_CHUNK_SIZE);

        let mut bytes_amount =
            self.cleartext.buf.capacity();

        if let Some(wanted_bytes) = wanted_bytes {
            bytes_amount = std::cmp::min(bytes_amount, wanted_bytes);
        }

        self.as_mut().decrypt_all_full_chunks()?;

        while self.cleartext.data().len() < bytes_amount && self.ciphertext.spare_capacity_len() != 0 {
            let poll = self.as_mut().inner_read(cx)?;

            if poll == Poll::Pending {
                if self.cleartext.data().is_empty() {
                    return Poll::Pending;
                } else {
                    break;
                }
            } else if self.ciphertext.data().is_empty() && self.cleartext.data().is_empty() {
                return Poll::Ready(Ok(true));
            } else {
                self.as_mut().decrypt_all_full_chunks()?;
            }
        }

        debug_assert!(!self.cleartext.data().is_empty());

        Poll::Ready(Ok(false))
    }
}

impl<T: AsyncReadable> AsyncRead for EncryptedReader<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let is_eof = ready!(self.as_mut().read_if_necessary(cx, Some(buf.remaining())))?;
        if is_eof {
            return Poll::Ready(Ok(()));
        }

        let chunk = self.cleartext.data();
        let num_bytes = std::cmp::min(buf.remaining(), chunk.len());

        buf.put_slice(&chunk[0..num_bytes]);

        self.cleartext.advance_cursor(num_bytes);

        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncReadable> AsyncBufRead for EncryptedReader<T> {
    fn poll_fill_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&[u8]>> {
        let is_eof = ready!(self.as_mut().read_if_necessary(cx, None))?;
        if is_eof {
            Poll::Ready(Ok(&[]))
        } else {
            Poll::Ready(Ok(self.project().cleartext.data()))
        }
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.cleartext.advance_cursor(amt);
    }
}
