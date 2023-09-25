use chacha20poly1305::{
    aead::stream::{DecryptorLE31, EncryptorLE31},
    ChaCha20Poly1305,
};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

pub async fn new(
    tcp_stream: TcpStream,
    shared_secret: [u8; 32],
) -> std::io::Result<(EncryptedReader, EncryptedWriter)> {
    let (read, write) = tcp_stream.into_split();

    let writer = EncryptedWriter::new(write, shared_secret).await?;
    let reader = EncryptedReader::new(read, shared_secret).await?;

    Ok((reader, writer))
}

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
    async fn new(mut reader: OwnedReadHalf, shared_key: [u8; 32]) -> std::io::Result<Self> {
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

        this.cipher_buf.resize(old_len + 8000, 0);

        let mut read_buf = ReadBuf::new(&mut this.cipher_buf.make_contiguous()[old_len..]);

        let poll = this.reader.poll_read(cx, &mut read_buf)?;

        let bytes_read = read_buf.filled().len();

        if bytes_read == 0 {
            match poll {
                Poll::Pending => return Ok(false),
                Poll::Ready(_) => return Ok(true),
            }
        }

        this.cipher_buf.resize(old_len + bytes_read, 0);

        let cipher_buf_contiguous = this.cipher_buf.make_contiguous();

        if let Some(header) = cipher_buf_contiguous.get(0..4) {
            let length = u32::from_be_bytes(header.try_into().unwrap()) as usize;

            if let Some(ciphertext) = cipher_buf_contiguous.get(4..length) {
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
                }
                return Poll::Pending;
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

#[pin_project]
pub struct EncryptedWriter {
    #[pin]
    writer: OwnedWriteHalf,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    encryption_space: Vec<u8>,
    ciphertext: VecDeque<u8>,
}

impl EncryptedWriter {
    async fn new(mut writer: OwnedWriteHalf, shared_key: [u8; 32]) -> std::io::Result<Self> {
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

        this.encryption_space.clear();
        this.encryption_space.extend_from_slice(buf);
        this.encryptor
            .encrypt_next_in_place(&[], this.encryption_space)
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;

        let len = u32::try_from(this.encryption_space.len())
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Message too long"))?;

        let len = (len + 4).to_be_bytes();

        this.encryption_space.splice(0..0, len);

        let bytes_written = ready!(this.writer.as_mut().poll_write(cx, this.encryption_space))?;

        this.ciphertext
            .extend(&this.encryption_space[bytes_written..]);

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
