use crate::Error;
use std::path::PathBuf;

use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct LocalFileMeta {
    pub local_path: PathBuf,
    pub public_path: PathBuf,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum Message {
    FileOffer(Option<Vec<FileMeta>>),
    FileAccept(Vec<bool>),
}

pub async fn serialize_into<T: AsyncWriteExt + Unpin, U: Serialize>(
    stream: &mut T,
    msg: &U,
) -> Result<(), Error> {
    let mut msg = to_stdvec(&msg)?;
    let len = u32::try_from(msg.len())?.to_be_bytes();
    msg.splice(0..0, len);
    stream.write_all(&msg).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn deserialize_from<'a, T: AsyncReadExt + Unpin, U: Deserialize<'a>>(
    stream: &mut T,
    tmp_buf: &'a mut Vec<u8>,
) -> Result<U, Error> {
    let length = stream.read_u32().await? as usize;

    if tmp_buf.len() < length {
        tmp_buf.resize(length, 0);
    }
    stream.read_exact(&mut tmp_buf[0..length]).await?;
    Ok(from_bytes(tmp_buf)?)
}
