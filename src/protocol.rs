use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::Error;



#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ClientMessage {
    /// Request the server to create a room
    CreateRoom,
    /// (room_id, user is creator of room?, private contact)
    SendContact([u8; 6], bool, Option<SocketAddr>),

    /// (room_id, user is creator of room?)
    DoneSending([u8; 6], bool),
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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum PeerMessage<'a> {
    Text(&'a str),
    Offer(Vec<FileMeta>),
    Accept(Vec<bool>),
    FileChunk(&'a [u8]),
    DoneSending,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}

pub async fn deserialize_from<'a, T: AsyncReadExt + Unpin, U: Deserialize<'a>>(
    stream: &mut T,
    tmp_buf: &'a mut [u8],
) -> Result<U, Error> {
    let length = stream.read_u32().await? as usize;

    if tmp_buf.len() < length {
        return Err(Error::TmpBufTooSmall);
    }

    stream.read_exact(&mut tmp_buf[0..length]).await?;
    Ok(from_bytes(tmp_buf)?)
}

pub async fn serialize_into<T: AsyncWriteExt + Unpin, U: Serialize>(
    stream: &mut T,
    msg: &U,
    tmp_buf: &mut [u8],
) -> Result<(), Error> {
    let len = to_slice(&msg, &mut tmp_buf[4..])?.len();
    let len_bytes = u32::try_from(len)?.to_be_bytes();
    tmp_buf[0..4].copy_from_slice(&len_bytes);
    Ok(stream.write_all(&tmp_buf[0..4 + len]).await?)
}
