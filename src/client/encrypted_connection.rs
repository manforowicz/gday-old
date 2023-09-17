use chacha20poly1305::{
    aead::{stream::{DecryptorLE31, EncryptorLE31}, generic_array::GenericArray},
    ChaCha20Poly1305,
};
use pin_project::pin_project;
use std::{
    collections::VecDeque,
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

#[pin_project]
pub struct Reader<R: AsyncRead + Unpin> {
    #[pin]
    reader: R,
    decryptor: DecryptorLE31<ChaCha20Poly1305>,
    cipher_buf: Vec<u8>,
    decryption_space: Vec<u8>,
    plaintext: VecDeque<u8>,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    pub async fn new(mut reader: R, shared_secret: [u8; 32]) -> Result<Self, std::io::Error> {

        let mut nonce = [0; 12];
        reader.read_exact(&mut nonce).await?;

        let decryptor = DecryptorLE31::new(&shared_secret.into(), GenericArray::from_slice(&nonce));
        Ok(Self {
            reader,
            decryptor,
            cipher_buf: Vec::new(),
            decryption_space: Vec::new(),
            plaintext: VecDeque::new(),
        })
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Reader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();

        let old_len = this.cipher_buf.len();

        this.cipher_buf.resize(old_len + 8000, 0);

        let mut read_buf = ReadBuf::new(&mut this.cipher_buf[old_len..]);

        ready!(this.reader.poll_read(cx, &mut read_buf))?;
        let bytes_read = read_buf.filled().len();
        this.cipher_buf.resize(old_len + bytes_read, 0);

        if let Some(header) = this.cipher_buf.get(0..4) {
            let length = u32::from_be_bytes(header.try_into().unwrap()) as usize;

            if let Some(ciphertext) = this.cipher_buf.get(4..length) {
                this.decryption_space.clear();
                this.decryption_space.extend_from_slice(ciphertext);
                this.decryptor
                    .decrypt_next_in_place(&[], this.decryption_space)
                    .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;

                this.plaintext.extend(this.decryption_space.iter());

                this.cipher_buf.rotate_left(length);
                this.cipher_buf.truncate(length);
            }
        }

        let len = std::cmp::min(buf.remaining(), this.plaintext.len());

        let (a, b) = this.plaintext.as_slices();

        if a.len() < len {
            buf.put_slice(a);
            let b_slice = &b[0..(len - a.len())];
            buf.put_slice(b_slice);
        } else {
            buf.put_slice(&a[0..len]);
        }

        this.plaintext.truncate(len);

        Poll::Ready(Ok(()))
    }
}

#[pin_project]
pub struct Writer<W: AsyncWrite + Unpin> {
    #[pin]
    writer: W,
    encryptor: EncryptorLE31<ChaCha20Poly1305>,
    encryption_space: Vec<u8>,
    ciphertext: VecDeque<u8>,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    pub async fn new(mut writer: W, shared_secret: [u8; 32]) -> Result<Self, std::io::Error> {
        let nonce: [u8; 12] = rand::random();
        
        writer.write_all(&nonce).await?;
        let encryptor = EncryptorLE31::new(&shared_secret.into(), GenericArray::from_slice(&nonce));
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

impl<W: AsyncWrite + Unpin> AsyncWrite for Writer<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
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

        todo!()
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        ready!(self.as_mut().flush_local_buffer(cx))?;
        let this = self.project();
        this.writer.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        ready!(self.as_mut().poll_flush(cx))?;
        let this = self.project();
        this.writer.poll_shutdown(cx)
    }
}
