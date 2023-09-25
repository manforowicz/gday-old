use std::path::PathBuf;
use crate::Error;

use postcard::{from_bytes, to_stdvec};
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWriteExt, AsyncReadExt};


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
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
    Ok(stream.write_all(&msg).await?)
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