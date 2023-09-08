/*

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::process::exit;

use holepunch::protocol;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const SERVER_ADDR_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(138, 2, 238, 120), 49870);
const SERVER_ADDR_V6: SocketAddrV6 = SocketAddrV6::new(
    Ipv6Addr::new(
        0x2603, 0xc024, 0xc00c, 0xb17e, 0xfce5, 0xf16d, 0x4207, 0xb22d,
    ),
    49870,
    0,
    0,
);

#[tokio::main]
async fn main() {
    if let Ok(mut stream) = TcpStream::connect(SERVER_ADDR_V6).await {
        stream
            .write_all(&Vec::from(protocol::ClientMessage::CreateRoom))
            .await
            .unwrap();
        let response = receive(&mut stream);
    }
}

async fn receive<T: AsyncReadExt + AsyncWriteExt + Unpin>(
    stream: &mut T,
) -> Result<protocol::ServerMessage, Box<dyn std::error::Error>> {
    let length = stream.read_u16().await?;
    let mut msg = vec![0; length as usize - 1];
    stream.read_exact(&mut msg).await?;
    Ok(protocol::ServerMessage::try_from(&msg[..])?)
}

*/

fn main() {}