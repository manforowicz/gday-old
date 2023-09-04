use std::net::{SocketAddrV4, SocketAddrV6};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_client(mut stream: TcpStream) {
    // read 20 bytes at a time from stream echoing back to stream
    loop {
        let mut read = [0; 1028];
        match stream.read(&mut read).await {
            Ok(n) => {
                if n == 0 {
                    // connection was closed
                    break;
                }
                stream.write(&read[0..n]).await.unwrap();
            }
            Err(err) => {
                panic!("{err}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            handle_client(stream).await;
        });
    }
}

struct ClientInfo {
    id: u32,
    private_addr_v4: Option<SocketAddrV4>,
    public_addr_v4: Option<SocketAddrV4>,
    private_addr_v6: Option<SocketAddrV6>,
    public_addr_v6: Option<SocketAddrV6>,
}
