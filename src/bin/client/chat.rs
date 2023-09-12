use std::io::BufRead;

use crate::peer_connection;
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};

pub struct Chat {
    read: OwnedReadHalf,
    write: OwnedWriteHalf,
    key: [u8; 32],
}

impl Chat {
    pub async fn begin(stream: TcpStream, key: [u8; 32]) -> Result<(), peer_connection::Error> {
        let (read, write) = stream.into_split();


        let talk = tokio::spawn(talk(write, key));
        let listen = tokio::spawn(listen(read, key));
        
        let _ = talk.await.unwrap();
        listen.await.unwrap()
    }


}

async fn listen(mut read: OwnedReadHalf, key: [u8; 32]) -> Result<(), peer_connection::Error> {
    loop {
        let msg = peer_connection::read(&mut read, key).await?;
        let msg = String::from_utf8(msg)?;

        println!("peer: {msg}");
    }
}

async fn talk(mut write: OwnedWriteHalf, key: [u8; 32]) -> Result<(), peer_connection::Error> {
    let mut buf = String::new();
    loop {
        std::io::stdin().lock().read_line(&mut buf)?;
        peer_connection::write(&mut write, buf.as_bytes(), key).await?;
        buf.clear();
    }
}