use std::num::TryFromIntError;

use chacha20poly1305::{aead::Aead, KeyInit, XChaCha20Poly1305};
use rand::RngCore;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Message cannot be longer than max of {} bytes", u32::MAX)]
    MessageTooLong(#[from] TryFromIntError),

    #[error("Peer cryptographical error")]
    Cyrptographical,
}

pub struct PeerConnection {
    stream: TcpStream,
    shared_key: [u8; 32],
}

impl PeerConnection {
    pub fn new(stream: TcpStream, shared_key: [u8; 32]) -> Self {
        Self { stream, shared_key }
    }

    pub async fn write(&mut self, msg: &[u8]) -> Result<(), Error> {
        let mut nonce = [0; 24];
        rand::thread_rng().fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new(&self.shared_key.into());

        let ciphertext = cipher
            .encrypt(&nonce.into(), msg)
            .map_err(|_| Error::Cyrptographical)?;

        let length = u32::try_from(nonce.len() + ciphertext.len())?.to_be_bytes();

        let data: Vec<u8> = length
            .into_iter()
            .chain(nonce.into_iter())
            .chain(ciphertext.into_iter())
            .collect();

        self.stream.write_all(&data).await?;

        Ok(())
    }

    pub async fn read(&mut self) -> Result<Vec<u8>, Error> {
        let mut length = [0; 4];
        self.stream.read_exact(&mut length).await?;
        let length = u32::from_be_bytes(length);

        let mut nonce = [0; 24];
        self.stream.read_exact(&mut nonce).await?;

        let mut ciphertext = vec![0; length as usize];
        self.stream.read_exact(&mut ciphertext).await?;

        let cipher = XChaCha20Poly1305::new(&self.shared_key.into());

        cipher
            .decrypt(&nonce.into(), &*ciphertext)
            .map_err(|_| Error::Cyrptographical)
    }
}
