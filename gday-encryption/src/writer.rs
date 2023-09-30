use chacha20poly1305::{aead::stream::EncryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{HelperBuf, CIPHERTEXT_OVERHEAD, MAX_CHUNK_SIZE};

pub trait AsyncWritable: AsyncWrite + Send + Unpin {}
impl<T: AsyncWrite + Send + Unpin> AsyncWritable for T {}

#[pin_project]
pub struct EncryptedWriter<T: AsyncWritable> {
    #[pin]
    writer: T,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    bytes: HelperBuf,
    is_flushing: bool,
}

impl<T: AsyncWritable> EncryptedWriter<T> {
    pub async fn new(mut writer: T, shared_key: [u8; 32]) -> std::io::Result<Self> {
        let nonce: [u8; 8] = rand::random();

        writer.write_all(&nonce).await?;
        writer.flush().await?;
        let encryptor = EncryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            writer,
            encryptor,
            bytes: HelperBuf::with_capacity(MAX_CHUNK_SIZE),
            is_flushing: true,
        })
    }

    fn poll_flush_local(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        debug_assert!(self.is_flushing);

        let mut this = self.project();

        while !this.bytes.data().is_empty() {
            let bytes_wrote = ready!(this.writer.as_mut().poll_write(cx, this.bytes.data()))?;
            this.bytes.advance_cursor(bytes_wrote);
        }

        *this.is_flushing = false;
        this.bytes.buf.extend_from_slice(&[0, 0, 0, 0]);
        Poll::Ready(Ok(()))
    }

    fn start_flushing(&mut self) -> std::io::Result<()> {
        let mut msg = self.bytes.buf.split_off(4);

        self.encryptor
            .encrypt_next_in_place(&[], &mut msg)
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;

        let len = u32::try_from(msg.len()).unwrap().to_be_bytes();

        self.bytes.buf.copy_from_slice(&len);
        self.bytes.buf.unsplit(msg);

        self.is_flushing = true;
        Ok(())
    }
}

impl<T: AsyncWritable> AsyncWrite for EncryptedWriter<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        debug_assert!(self.bytes.buf.capacity() == MAX_CHUNK_SIZE);
        if self.is_flushing {
            ready!(self.as_mut().poll_flush_local(cx))?;
        }

        let bytes_taken = std::cmp::min(
            buf.len(),
            self.bytes.spare_capacity_len() - CIPHERTEXT_OVERHEAD,
        );

        self.bytes.buf.extend_from_slice(&buf[0..bytes_taken]);

        if self.bytes.spare_capacity_len() == CIPHERTEXT_OVERHEAD {
            self.start_flushing()?;
        }

        Poll::Ready(Ok(bytes_taken))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if !self.is_flushing && !self.bytes.data().is_empty() {
            self.start_flushing()?;
        }
        if self.is_flushing {
            ready!(self.as_mut().poll_flush_local(cx))?;
        }
        self.project().writer.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.project().writer.poll_shutdown(cx)
    }
}
