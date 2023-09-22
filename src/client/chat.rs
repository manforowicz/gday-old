use std::path::PathBuf;

use serde::{Serialize, Deserialize};




#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum PeerMessage<'a> {
    Text(&'a str),
    Offer(Vec<FileMeta>),
    Accept(Vec<bool>),
    FileChunk(&'a [u8]),
    DoneSending,
}


