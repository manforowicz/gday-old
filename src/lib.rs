#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::must_use_candidate)]

use std::{
    net::SocketAddr,
    num::TryFromIntError,
};

use postcard::{from_bytes, to_stdvec};
use protocol::Endpoint;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod protocol;

pub async fn deserialize_from<T: AsyncReadExt + Unpin, U: DeserializeOwned>(
    stream: &mut T,
) -> Result<U, Error> {
    let length = stream.read_u16().await? as usize;
    let mut buf = vec![0; length];
    stream.read_exact(&mut buf).await?;
    Ok(from_bytes(&buf)?)
}

pub async fn serialize_into<T: AsyncWriteExt + Unpin, U: Serialize>(
    stream: &mut T,
    msg: &U,
) -> Result<(), Error> {
    let msg = to_stdvec(msg)?;
    let length = u16::try_from(msg.len())?.to_be_bytes();
    Ok(stream.write_all(&[&length[..], &msg[..]].concat()).await?)
}

pub fn endpoint_from_addr(addr: SocketAddr) -> Endpoint {
    match addr {
        SocketAddr::V6(addr) => Endpoint::V6(u128::from(*addr.ip()), addr.port()),
        SocketAddr::V4(addr) => Endpoint::V4(u32::from(*addr.ip()), addr.port()),
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error with encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("Message is longer than max of {} bytes", u16::MAX)]
    MessageTooLong(#[from] TryFromIntError),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}
