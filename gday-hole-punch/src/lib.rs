#![allow(dead_code)]

use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;

pub type RoomId = [u8; 6];

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]

pub enum ClientMessage {
    /// Request the server to create a room
    CreateRoom,
    /// (room_id, user is creator of room?, private contact)
    SendContact {
        room_id: RoomId,
        is_creator: bool,
        private_addr: Option<SocketAddr>,
    },

    /// (room_id, user is creator of room?)
    DoneSending { room_id: RoomId, is_creator: bool },
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ServerMessage {
    /// Room successfully created
    /// (room_password, user_id)
    RoomCreated(RoomId),
    /// (full contact info of peer)
    SharePeerContacts {
        client_public: Contact,
        peer: FullContact,
    },
    SyntaxError,
    ErrorNoSuchRoomID,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct Contact {
    pub v6: Option<SocketAddrV6>,
    pub v4: Option<SocketAddrV4>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct FullContact {
    pub private: Contact,
    pub public: Contact,
}

#[derive(Debug)]
struct Messenger<T: AsyncRead + AsyncWrite + Unpin> {
    stream: T,
    buf: Vec<u8>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> Messenger<T> {
    pub fn with_capacity(stream: T, capacity: usize) -> Self {
        Self {
            stream,
            buf: vec![0; capacity],
        }
    }

    pub async fn next_msg<'a, U: Deserialize<'a>>(&'a mut self) -> Result<U, SerializationError> {
        let length = self.stream.read_u32().await? as usize;

        if self.buf.len() < length {
            return Err(SerializationError::TmpBufTooSmall);
        }

        self.stream.read_exact(&mut self.buf[0..length]).await?;
        Ok(from_bytes(&self.buf[0..length])?)
    }

    pub async fn write_msg(&mut self, msg: impl Serialize) -> Result<(), SerializationError> {
        let len = to_slice(&msg, &mut self.buf[4..])?.len();
        let len_bytes = u32::try_from(len)?.to_be_bytes();
        self.buf[0..4].copy_from_slice(&len_bytes);
        self.stream.write_all(&self.buf[0..4 + len]).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub fn inner_stream(&self) -> &T {
        &self.stream
    }
}

impl Messenger<tokio_rustls::server::TlsStream<TcpStream>> {
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.get_ref().0.local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.get_ref().0.peer_addr()
    }
}

impl Messenger<tokio_rustls::client::TlsStream<TcpStream>> {
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.get_ref().0.local_addr()
    }

    pub fn peer_addr(&self) -> std::io::Result<SocketAddr> {
        self.stream.get_ref().0.peer_addr()
    }
}

#[derive(Error, Debug)]
pub enum SerializationError {
    #[error("Error with encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Temporary buffer too small")]
    TmpBufTooSmall,

    #[error("Message too long: {0}")]
    MessageTooLong(#[from] std::num::TryFromIntError),
}

#[cfg(test)]
mod tests {
    //use super::*;
}
