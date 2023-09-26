use chacha20poly1305::{
    aead::stream::EncryptorLE31,
    ChaCha20Poly1305,
};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::{io::{ AsyncWrite, AsyncWriteExt}, net::tcp::OwnedWriteHalf};

use crate::MAX_CHUNK_SIZE;



#[pin_project]
pub struct EncryptedWriter {
    #[pin]
    writer: OwnedWriteHalf,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    encryption_space: Vec<u8>,
    ciphertext: VecDeque<u8>,
}

impl EncryptedWriter {
    pub(super) async fn new(mut writer: OwnedWriteHalf, shared_key: [u8; 32]) -> std::io::Result<Self> {
        let nonce: [u8; 8] = rand::random();

        writer.write_all(&nonce).await?;
        let encryptor = EncryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            writer,
            encryptor,
            encryption_space: Vec::new(),
            ciphertext: VecDeque::new(),
        })
    }

    fn flush_local_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.project();
        if !this.ciphertext.is_empty() {
            let bytes_written = ready!(this
                .writer
                .poll_write(cx, this.ciphertext.make_contiguous()))?;
            this.ciphertext.drain(0..bytes_written);
        }

        if this.ciphertext.is_empty() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl AsyncWrite for EncryptedWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        ready!(self.as_mut().flush_local_buffer(cx))?;

        let mut this = self.project();

        for chunk in buf.chunks(MAX_CHUNK_SIZE) {
            this.encryption_space.clear();
            this.encryption_space.extend_from_slice(chunk);
            this.encryptor
                .encrypt_next_in_place(&[], this.encryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;
    
            let len = u32::try_from(this.encryption_space.len())
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Message too long"))?;
    
            let len = (len + 4).to_be_bytes();

            this.ciphertext.extend(len);
            this.ciphertext.extend(&this.encryption_space[..]);
        }

        let bytes_written = ready!(this.writer.as_mut().poll_write(cx, this.ciphertext.make_contiguous()))?;
        this.ciphertext.drain(0..bytes_written);

        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().flush_local_buffer(cx))?;
        let this = self.project();
        this.writer.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let this = self.project();
        this.writer.poll_shutdown(cx)
    }
}
