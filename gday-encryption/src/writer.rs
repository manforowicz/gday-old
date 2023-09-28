use bytes::{Buf, BytesMut};
use chacha20poly1305::{aead::stream::EncryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::MAX_CHUNK_SIZE;

pub trait AsyncWritable: AsyncWrite + Send + Unpin {}
impl<T: AsyncWrite + Send + Unpin> AsyncWritable for T {}

#[derive(PartialEq, Debug)]
enum Mode {
    Collecting,
    Flushing,
}

#[pin_project]
pub struct EncryptedWriter<T: AsyncWritable> {
    #[pin]
    writer: T,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    bytes: BytesMut,
    mode: Mode,
}

impl<T: AsyncWritable> EncryptedWriter<T> {
    pub async fn new(mut writer: T, shared_key: [u8; 32]) -> std::io::Result<Self> {
        let nonce: [u8; 8] = rand::random();

        writer.write_all(&nonce).await?;
        let encryptor = EncryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            writer,
            encryptor,
            bytes: BytesMut::with_capacity(MAX_CHUNK_SIZE),
            mode: Mode::Flushing,
        })
    }

    fn poll_flush_local(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        assert_eq!(self.mode, Mode::Flushing);

        let mut this = self.project();

        while this.bytes.remaining() != 0 {
            let chunk = this.bytes.chunk();
            let bytes_wrote = ready!(this.writer.as_mut().poll_write(cx, chunk))?;
            this.bytes.advance(bytes_wrote);
        }

        *this.mode = Mode::Collecting;
        this.bytes.clear();
        this.bytes.extend_from_slice(&[0, 0, 0, 0]);
        Poll::Ready(Ok(()))
    }

    fn start_flushing(&mut self) -> std::io::Result<()> {


        let mut msg = self.bytes.split_off(4);

        self.encryptor
            .encrypt_next_in_place(&[], &mut msg)
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;

        self.bytes.copy_from_slice(&u32::try_from(msg.len()).unwrap().to_be_bytes());
        self.bytes.unsplit(msg);

        self.mode = Mode::Flushing;
        Ok(())
    }
}

impl<T: AsyncWritable> AsyncWrite for EncryptedWriter<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.mode == Mode::Flushing {
            ready!(self.as_mut().poll_flush_local(cx))?;
        }

        let bytes_taken = std::cmp::min(buf.len(), self.bytes.spare_capacity_mut().len() - 16);

        self.bytes.extend_from_slice(&buf[0..bytes_taken]);

        if self.bytes.spare_capacity_mut().len() <= 16 {
            self.start_flushing()?;
        }

        Poll::Ready(Ok(bytes_taken))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        if self.mode != Mode::Flushing && !self.bytes.is_empty() {
            self.start_flushing()?;
        }
        
        if self.mode == Mode::Flushing {
            ready!(self.as_mut().poll_flush_local(cx))?;
        }
        let this = self.project();
        this.writer.poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let this = self.project();
        this.writer.poll_shutdown(cx)
    }
}
