use bytes::{BytesMut, Buf};
use chacha20poly1305::{aead::stream::EncryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    net::tcp::OwnedWriteHalf,
};

use crate::BUF_CAPACITY;

#[pin_project]
pub struct EncryptedWriter {
    #[pin]
    writer: OwnedWriteHalf,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    bytes: BytesMut,
}

impl EncryptedWriter {
    pub(super) async fn new(
        mut writer: OwnedWriteHalf,
        shared_key: [u8; 32],
    ) -> std::io::Result<Self> {
        let nonce: [u8; 8] = rand::random();

        writer.write_all(&nonce).await?;
        let encryptor = EncryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            writer,
            encryptor,
            bytes: BytesMut::with_capacity(BUF_CAPACITY),
        })
    }

    fn flush_local_buffer(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        let this = self.project();

        if !this.bytes.is_empty() {
            let chunk = this.bytes.chunk();
            let bytes_wrote = ready!(this.writer.poll_write(cx, chunk))?;
            this.bytes.advance(bytes_wrote);
        }

        if this.bytes.remaining() == 0 {
            this.bytes.clear();
            Poll::Ready(Ok(()))
        } else {
            cx.waker().wake_by_ref();
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
        let this = self.as_mut().project();

        let bytes_taken = std::cmp::min(buf.len(), this.bytes.spare_capacity_mut().len());
        
        this.bytes.extend_from_slice(&[0, 0, 0, 0]);
        this.bytes.extend_from_slice(&buf[0..bytes_taken]);
        let mut encryption_space = this.bytes.split_off(4);

        this.encryptor
            .encrypt_next_in_place(&[], &mut encryption_space)
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
        
        let len = u32::try_from(this.bytes.len()).unwrap().to_be_bytes();
        this.bytes[0..4].copy_from_slice(&len);

        ready!(self.as_mut().flush_local_buffer(cx))?;
        Poll::Ready(Ok(bytes_taken))
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
