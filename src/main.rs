/*

1 bytes  - total inclusive length of the entire message
1 byte - type of the message (see below)



--- 1 - client: create room (transmits new password) ---

password: array of u8 (up to 253 bytes)

--- 2 - client: join room (transmits password) ---

password: array of u8 (up to 253 bytes)

--- 3 - server: transmits user id (room created) ---

8 bytes

--- 4 - client: sending info ---

personal id (8 bytes)

series of optional: flag byte followed by content

1 - private ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)
2 - private ipv4 (4 byte ip, 2 byte port)

5 - nothing else to share / request peer info when available

--- 5 - server: other peer finished, here's their contact info ---

series of optional: flag byte followed by content

1 - private ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)
2 - private ipv4 (4 byte ip, 2 byte port)
3 - public ipv4 (4 byte ip, 2 byte port)
4 - public ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)



ERROR MESSAGE TYPES (no content in them)


6 - invalid message syntax
7 - room with this password already exists
8 - no room with this password exists
9 - unknown personal id
255 - other

*/

#![warn(clippy::all, clippy::pedantic)]

mod state;
use state::State;

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct ClientHandler {
    state: State,
    stream: TcpStream,
}

impl ClientHandler {
    async fn handle_client(&mut self) {
        loop {
            self.handle_message().await;
        }
    }

    async fn handle_message(&mut self) {
        let msg_length = self.stream.read_u8().await.unwrap();
        let msg_type = self.stream.read_u8().await.unwrap();

        if msg_length <= 2 {
            // error: invalid message syntax
            self.send(6, &[]).await;
            return;
        }

        let mut msg = vec![0; msg_length as usize - 2];
        self.stream.read_exact(&mut msg).await.unwrap();

        match msg_type {
            1 => self.handle1(&msg).await,
            2 => self.handle2(&msg).await,
            4 => self.receive4(&msg).await,
            // error: invalid message syntax
            _ => self.send(6, &[]).await,
        }
    }

    /// Handle "create room" request
    async fn handle1(&mut self, msg: &[u8]) {
        let password = msg;

        if self.state.room_exists(password) {
            // Error: room with this password already exists
            self.send(7, &[]).await;
        } else {
            let id = self.state.add_client(password);
            // Response: sending user id
            self.send(3, &id.to_be_bytes()[..]).await;
        }
    }

    /// Handle "join room" request
    async fn handle2(&mut self, msg: &[u8]) {
        let password = msg;
        if self.state.room_exists(password) {
            // Error: no room with this password exists
            self.send(8, &[]).await;
        } else {
            // Response: sending user id
            let id = self.state.add_client(password);
            self.send(3, &id.to_be_bytes()[..]).await;
        }
    }

    /// Handle "send contact info" request
    async fn receive4(&mut self, msg: &[u8]) {
        let id = u64::from_be_bytes(msg[0..8].try_into().unwrap());

        loop {
            let flag = msg[0];
            let msg = &msg[1..];
            match flag {
                1 => self
                    .state
                    .update_client(id, parse_addr_v6(&msg[0..18]), false),
                2 => self
                    .state
                    .update_client(id, parse_addr_v4(&msg[0..6]), false),
                5 => self.state.set_client_done(id, |x| ()),
                _ => self.send(5, &[]).await,
            };
        }
    }

    async fn send5(&mut self) {}

    async fn send(&mut self, code: u8, msg: &[u8]) {
        self.stream
            .write_all(&[&[msg.len().try_into().unwrap(), code], msg].concat())
            .await
            .unwrap();
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

#[tokio::main]
async fn main() {
    let state = State::default();

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let mut handler = ClientHandler {
            state: state.clone(),
            stream,
        };
        tokio::spawn(async move { handler.handle_client().await });
    }
}
