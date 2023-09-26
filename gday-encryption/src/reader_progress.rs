#![allow(dead_code)]

use bytes::{ BytesMut, Buf};
use chacha20poly1305::{aead::stream::DecryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    net::tcp::OwnedReadHalf,
};

use crate::BUF_CAPACITY;

#[pin_project]
pub struct EncryptedReader {
    #[pin]
    reader: OwnedReadHalf,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    decrypted: BytesMut,
    encrypted: BytesMut,
}

impl EncryptedReader {
    pub(super) async fn new(
        mut reader: OwnedReadHalf,
        shared_key: [u8; 32],
    ) -> std::io::Result<Self> {
        let mut nonce = [0; 8];
        reader.read_exact(&mut nonce).await?;

        let decryptor = DecryptorLE31::new(&shared_key.into(), &nonce.into());
        let mut decrypted = BytesMut::with_capacity(BUF_CAPACITY);
        let encrypted = decrypted.split_off(0);
        Ok(Self {
            reader,
            decryptor,
            decrypted,
            encrypted
        })
    }
}

impl AsyncRead for EncryptedReader {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();

        if this.bytes.remaining() == 0 {
            this.bytes.clear();
        }

        if buf.remaining() > this.bytes.remaining() {

            let mut encrypted = this.bytes.split_off(this.bytes.len());
            let mut tmp = ReadBuf::uninit(encrypted.spare_capacity_mut());
            let poll = this.reader.poll_read(cx, &mut tmp)?;
            let bytes_read = tmp.filled().len();
            unsafe { encrypted.set_len(bytes_read) }
            this.decryptor.decrypt_next_in_place(&[], &mut encrypted).unwrap();
            this.bytes.unsplit(encrypted);

            if poll == Poll::Pending && this.bytes.remaining() == 0 {
                return Poll::Pending;
            }
        }

        let chunk = this.bytes.chunk();
        let write_len = std::cmp::min(chunk.len(), buf.remaining());
        let tmp = &chunk[..write_len];
        buf.put_slice(tmp);
        this.bytes.advance(write_len);

        Poll::Ready(Ok(()))
    }
}
