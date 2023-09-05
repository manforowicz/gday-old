/*

1 bytes  - total inclusive length of the entire message
1 byte - type of the message (see below)



--- 1 - client: transmits password ---

password: array of u8 (up to 253 bytes)

--- 2 - server: transmits user id (room created) ---

8 bytes

--- 3 - client: sending info ---

personal id (8 bytes)

series of optional: flag byte followed by content

1 - private ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)
2 - private ipv4 (4 byte ip, 2 byte port)

5 - nothing else to share / request peer info when available

--- 4 - server: other peer finished, here's their contact info ---

series of optional: flag byte followed by content

1 - private ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)
2 - private ipv4 (4 byte ip, 2 byte port)
3 - public ipv4 (4 byte ip, 2 byte port)
4 - public ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)



ERROR MESSAGE TYPES (no content in them)


5 - invalid message syntax
6 - password taken (pick a new one, start over)
7 - unknown personal id
255 - other

*/

#![warn(clippy::all, clippy::pedantic)]

mod state;
use state::State;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_client(mut state: State, mut stream: TcpStream) {
    loop {
        handle_message(&mut state, &mut stream).await;
    }
}

async fn handle_message(state: &mut State, stream: &mut TcpStream) {
    let msg_length = stream.read_u8().await.unwrap();
    let msg_type = stream.read_u8().await.unwrap();

    if msg_length <= 2 {
        send(stream, 6, &[]).await;
        return;
    }

    let mut msg = vec![0; msg_length as usize - 2];
    stream.read_exact(&mut msg).await.unwrap();

    match msg_type {
        1 => receive1(state, stream, &msg).await,
        3 => receive3(state, stream, &msg).await,
        // invalid message type (only types 1 and 3 are for client)
        _ => send(stream, 5, &[]).await,
    }
}

async fn receive1(state: &mut State, stream: &mut TcpStream, msg: &[u8]) {
    let password = msg;

    match state.add_client(password) {
        Ok(id) => send(stream, 2, &id.to_be_bytes()[..]).await,
        Err(_) => send(stream, 8, &[]).await,
    }
}

async fn receive3(state: &mut State, stream: &mut TcpStream, msg: &[u8]) {
    let id = u64::from_be_bytes(msg[0..8].try_into().unwrap());

    loop {
        let flag = msg[0];
        let msg = &msg[1..];
        match flag {
            1 => state.update_client(id, parse_addr_v6(&msg[0..18]), false),
            2 => state.update_client(id, parse_addr_v4(&msg[0..6]), false),
            5 => state.set_client_done(id),
            _ => send(stream, 5, &[]).await,
        };
    }
}

macro_rules! bytes_to_int {
    ($bytes:expr, $type:ty) => {{
        <$type>::from_be_bytes($bytes.try_into().unwrap())
    }};
}

fn parse_addr_v6(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..16], u128);
    let port = bytes_to_int!(msg[16..18], u16);
    SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ip)), port)
}

fn parse_addr_v4(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..4], u32);
    let port = bytes_to_int!(msg[4..6], u16);
    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port)
}

async fn send(stream: &mut TcpStream, code: u8, data: &[u8]) {
    stream
        .write_all(&[&[data.len().try_into().unwrap(), code], data].concat())
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    let state = State::default();

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        tokio::spawn(handle_client(state.clone(), stream));
    }
}
