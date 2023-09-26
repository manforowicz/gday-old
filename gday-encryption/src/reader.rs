use chacha20poly1305::{aead::stream::DecryptorLE31, ChaCha20Poly1305};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    io::ErrorKind,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    net::tcp::OwnedReadHalf,
};

use crate::MAX_CHUNK_SIZE;

#[pin_project]
pub struct EncryptedReader {
    #[pin]
    reader: OwnedReadHalf,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    cipher_buf: VecDeque<u8>,
    decryption_space: Vec<u8>,
    plaintext: VecDeque<u8>,
}

impl EncryptedReader {
    pub(super) async fn new(
        mut reader: OwnedReadHalf,
        shared_key: [u8; 32],
    ) -> std::io::Result<Self> {
        let mut nonce = [0; 8];
        reader.read_exact(&mut nonce).await?;

        let decryptor = DecryptorLE31::new(&shared_key.into(), &nonce.into());
        Ok(Self {
            reader,
            decryptor,
            cipher_buf: VecDeque::new(),
            decryption_space: Vec::new(),
            plaintext: VecDeque::new(),
        })
    }

    /// Return bool is true when EOF is reached.
    fn read_to_local_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::io::Result<bool> {
        let this = self.project();
        let old_len = this.cipher_buf.len();

        this.cipher_buf.resize(old_len + MAX_CHUNK_SIZE + 4, 0);
        
        let mut read_buf = ReadBuf::new(&mut this.cipher_buf.make_contiguous()[old_len..]);

        let poll = this.reader.poll_read(cx, &mut read_buf)?;

        let bytes_read = read_buf.filled().len();

        this.cipher_buf.resize(old_len + bytes_read, 0);

        if bytes_read == 0 {
            match poll {
                Poll::Pending => return Ok(false),
                Poll::Ready(()) => return Ok(true),
            }
        }
        let cipher_buf = this.cipher_buf.make_contiguous();

        if let Some(header) = cipher_buf.get(0..4) {
            let length = u32::from_be_bytes(header.try_into().unwrap()) as usize;
            if let Some(ciphertext) = cipher_buf.get(4..length) {
                this.decryption_space.clear();
                this.decryption_space.extend_from_slice(ciphertext);
                this.decryptor
                    .decrypt_next_in_place(&[], this.decryption_space)
                    .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;

                this.plaintext.extend(this.decryption_space.iter());

                this.cipher_buf.drain(0..length);
            }
        }

        Ok(false)
    }
}

impl AsyncRead for EncryptedReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.plaintext.len() < buf.remaining() {
            let is_eof = self.as_mut().read_to_local_buf(cx)?;
            if self.plaintext.is_empty() {
                if is_eof {
                    return Poll::Ready(Ok(()));
                } else {
                    return Poll::Pending;
                }
            }
        }

        let this = self.project();

        let len = std::cmp::min(buf.remaining(), this.plaintext.len());

        let (a, b) = this.plaintext.as_slices();

        if a.len() < len {
            buf.put_slice(a);
            let b_slice = &b[0..(len - a.len())];
            buf.put_slice(b_slice);
        } else {
            buf.put_slice(&a[0..len]);
        }

        this.plaintext.drain(0..len);
        Poll::Ready(Ok(()))
    }
}
