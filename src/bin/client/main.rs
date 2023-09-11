#![warn(clippy::all, clippy::pedantic)]
#![allow(dead_code)]

mod establisher;
mod server_connection;
mod peer_connection;

use std::net::{SocketAddrV6, Ipv6Addr, SocketAddrV4, Ipv4Addr};

use server_connection::ServerAddr;

const SERVERS: [ServerAddr; 1] = [ServerAddr {
    v6: SocketAddrV6::new(
        Ipv6Addr::new(
            0x2603, 0xc024, 0xc00c, 0xb17e, 0xfce5, 0xf16d, 0x4207, 0xb22d,
        ),
        49870,
        0,
        0,
    ),
    v4: SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 49870),
    name: "psend",
}];

#[tokio::main]
async fn main() {

}