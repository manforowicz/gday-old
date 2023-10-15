#![feature(doc_cfg)]
#![allow(dead_code)]

//! # Welcome to gday-hole-punch
//! - Bullet
//! - Bullet
//! - Test
//! # Examples
//! A simple test:
//! ```
//! use gday_hole_punch::{client, RoomId, ContactSharer};
//!
//! let (sharer, room_id) = ContactSharer::create_room(server_addr)?;
//! println!("Here's the id of my room: {room_id}");
//! let connector = sharer.get_peer_connector()?;
//! let weak_shared_secret = b"A5P";
//! connector.connect_to_peer(weak_shared_secret);
//! let (peer_tcp_stream, strong_shared_secret) = connector.connect_to_peer()?;
//! ```
//! It works!

use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::TlsStream;

#[doc(cfg(feature = "server"))]
#[cfg(feature = "server")]
pub mod server;

#[doc(cfg(feature = "client"))]
#[cfg(feature = "client")]
pub mod client;

/// Both peers send the server the same [`RoomId`] to get each other's contacts.
///
/// 6 random ascii characters. Each character will be an uppercase letter A through Z or a digit 0 through 9.


/// A message from [`client`] -> [`server`]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
enum ClientMessage {
    /// Request the server to create a room
    CreateRoom,
    /// (room_id, user is creator of room?, private contact)
    SendContact {
        room_id: u32,
        is_creator: bool,
        private_addr: Option<SocketAddr>,
    },

    /// (room_id, user is creator of room?)
    DoneSending { room_id: u32, is_creator: bool },
}

/// A message from [`server`] -> [`client`]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
enum ServerMessage {
    /// Room successfully created
    /// (room_password, user_id)
    RoomCreated(u32),
    /// (full contact info of peer)
    SharePeerContacts {
        client_public: Contact,
        peer: FullContact,
    },
    SyntaxError,
    ErrorNoSuchRoomID,
}

/// The addresses of a single network endpoint.
///
/// An endpoint may have IPv6, IPv4, none, or both.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct Contact {
    /// Endpoint's IPv6 socket address
    pub v6: Option<SocketAddrV6>,
    /// Endpiont's IPv4 socket address
    pub v4: Option<SocketAddrV4>,
}

/// The public and private contacts of an entity.
///
/// `public` is different from `private` when the entity is behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct FullContact {
    /// The peer's private contact in it's local network.
    pub private: Contact,
    /// The entity's public contact visible to the public internet.
    pub public: Contact,
}

#[derive(Debug)]
struct Messenger {
    stream: TlsStream<TcpStream>,
    buf: Vec<u8>,
}

impl Messenger {
    pub fn with_capacity(stream: impl Into<TlsStream<TcpStream>>, capacity: usize) -> Self {
        Self {
            stream: stream.into(),
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

    pub fn inner_stream(&self) -> &TcpStream {
        self.stream.get_ref().0
    }

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
