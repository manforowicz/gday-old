#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use postcard::{from_bytes, to_stdvec};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::num::TryFromIntError;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error with encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("Message cannot be longer than max of {} bytes", u8::MAX)]
    MessageTooLong(#[from] TryFromIntError),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ClientMessage {
    /// Request the server to create a room
    CreateRoom,
    /// (room_id, user is creator of room?, private contact)
    SendContact([u8; 6], bool, Option<SocketAddr>),

    /// (room_id, user is creator of room?)
    DoneSending([u8; 6], bool)
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ServerMessage {
    /// Room successfully created
    /// (room_password, user_id)
    RoomCreated([u8; 6]),
    /// (full contact info of peer)
    SharePeerContacts(FullContact),
    SyntaxError,
    ErrorNoSuchRoomID,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Default)]
pub struct Contact {
    pub v6: Option<SocketAddrV6>,
    pub v4: Option<SocketAddrV4>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Default)]
pub struct FullContact {
    pub private_v6: Option<SocketAddrV6>,
    pub public_v6: Option<SocketAddrV6>,
    pub private_v4: Option<SocketAddrV4>,
    pub public_v4: Option<SocketAddrV4>,
}

pub async fn deserialize_from<T: AsyncReadExt + Unpin, U: DeserializeOwned>(
    stream: &mut T,
) -> Result<U, Error> {
    let length = stream.read_u8().await? as usize;
    let mut buf = vec![0; length];
    stream.read_exact(&mut buf).await?;
    Ok(from_bytes(&buf)?)
}

pub async fn serialize_into<T: AsyncWriteExt + Unpin, U: Serialize>(
    stream: &mut T,
    msg: &U,
) -> Result<(), Error> {
    let msg = to_stdvec(msg)?;
    let length = u8::try_from(msg.len())?.to_be_bytes();
    Ok(stream.write_all(&[&length[..], &msg[..]].concat()).await?)
}
