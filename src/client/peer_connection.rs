use std::path::PathBuf;
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};

use crate::client::encrypted_connection::{Reader, Writer};

pub struct PeerConnection {
    pub stream: TcpStream,
    pub shared_key: [u8; 32],
}

impl PeerConnection {
    pub fn split(self) -> (Reader<OwnedReadHalf>, Writer<OwnedWriteHalf>) {
        let (read, write) = self.stream.into_split();
        (
            Reader::new(read, self.shared_key),
            Writer::new(write, self.shared_key),
        )
    }
}

pub enum Message<'a> {
    Text(&'a str),
    Offer(&'a [FileMeta]),
    Accept(&'a [bool]),
    FileChunk(&'a [u8]),
    DoneSending
}


pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}
