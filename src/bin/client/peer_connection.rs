use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
use pin_project::pin_project;
use rand::RngCore;
use std::{num::TryFromIntError, string::FromUtf8Error, pin::{Pin, self}, task::{Poll, Context}};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, AsyncRead, ReadBuf},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Message cannot be longer than max of {} bytes", u32::MAX)]
    MessageTooLong(#[from] TryFromIntError),
    #[error("Peer cryptographical error")]
    Cyrptographical,
    #[error("String decoding error")]
    StringError(#[from] FromUtf8Error),
}

/*

struct Message<'a> {
    msg_type: MsgType,
    content: &'a[u8],
}
*/

/* 
enum MsgType {
    Text(Vec<u8>),
    FilePropose(Vec<FileMeta>),
    FileAccept(Vec<bool>),
    Filechunk(Vec<u8>),
}

impl From<MsgType> for u8 {
    fn from(msg_type: MsgType) -> Self {
        match msg_type {
            MsgType::Text => 1,
            MsgType::FileMetadata => 2,
            MsgType::FileAccept => 3,
            MsgType::Filechunk => 4,
        }
    }
}
*/


pub struct PeerConnection {
    pub stream: TcpStream,
    pub shared_key: [u8; 32],
}

impl PeerConnection {
    pub fn split(self) -> (PeerReader, PeerWriter) {
        let (reader, writer) = self.stream.into_split();
        let key = self.shared_key;
        (PeerReader { reader, key }, PeerWriter { writer, key })
    }
}

pub struct PeerReader {
    reader: OwnedReadHalf,
    key: [u8; 32],
}

impl PeerReader {
    pub async fn receive(&mut self) -> Result<Vec<u8>, Error> {
        let mut length = [0; 4];
        self.reader.read_exact(&mut length).await?;
        let length = u32::from_be_bytes(length);

        let mut nonce = [0; 24];
        self.reader.read_exact(&mut nonce).await?;

        let mut ciphertext = vec![0; length as usize];
        self.reader.read_exact(&mut ciphertext).await?;

        let cipher = XChaCha20Poly1305::new(&self.key.into());

        cipher
            .decrypt(&nonce.into(), &*ciphertext)
            .map_err(|_| Error::Cyrptographical)
    }
}

#[pin_project]
pub struct PeerReaderTest {
    #[pin]
    reader: OwnedReadHalf,
    key: [u8; 32],
    buf: ReadBuf<'static>,
}

impl AsyncRead for PeerReaderTest {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {

        let this = self.project();

        let x = this.reader.poll_read(cx, this.buf)?;

        if let Some(header) = this.buf.filled().get(0..4) {
            let length = u32::from_be_bytes(header.try_into().unwrap()) as usize;
            if let Some(ciphertext) = this.buf.filled().get(4..(4+24+length)) {
                
            }
        }
        


        todo!()
    }
}

pub struct PeerWriter {
    writer: OwnedWriteHalf,
    key: [u8; 32],
}

impl PeerWriter {
    pub async fn send(&mut self, msg: &[u8]) -> Result<(), Error> {
        let mut nonce = [0; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new(&self.key.into());

        let ciphertext = cipher
            .encrypt(&nonce.into(), msg)
            .map_err(|_| Error::Cyrptographical)?;

        let length = u32::try_from(ciphertext.len())?.to_be_bytes();

        let data: Vec<u8> = length
            .into_iter()
            .chain(nonce.into_iter())
            .chain(ciphertext.into_iter())
            .collect();

        self.writer.write_all(&data).await?;

        Ok(())
    }
}
