use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::old_global_state::State;

use crate::old_protocol::{receive, send};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct ClientSyntaxError;

impl std::error::Error for ClientSyntaxError {}

impl fmt::Display for ClientSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received message of invalid syntax from client")
    }
}

pub struct ConnectionHandler<T: AsyncReadExt + AsyncWriteExt + Unpin> {
    state: State,
    stream: T,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin> ConnectionHandler<T> {
    pub async fn start(state: State, stream: T) {
        let mut this = ConnectionHandler { state, stream };
        Self::handle_connection(&mut this).await;
    }

    async fn handle_connection(&mut self) {
        while self.handle_message().await.is_ok() {}
    }

    async fn handle_message(&mut self) -> Result<()> {

        let (msg_type, msg) = receive(&mut self.stream).await?;

        match msg_type {
            1 => self.handle1(&msg).await?,
            2 => self.handle2(&msg).await?,
            4 => self.receive4(&msg).await?,
            _ => return self.invalid_syntax().await,
        }

        Ok(())
    }

    /// Handle "create room" request
    async fn handle1(&mut self, msg: &[u8]) -> Result<()> {
        let password = msg;
        if self.state.room_exists(password) {
            // Error: room with this password already exists
            send(&mut self.stream, 7, &[]).await?;
        } else {
            let id = self.state.add_client(password);
            // Response: sending user id
            send(&mut self.stream, 3, &id.to_be_bytes()[..]).await?;
        }
        Ok(())
    }

    /// Handle "join room" request
    async fn handle2(&mut self, msg: &[u8]) -> Result<()> {
        let password = msg;
        if self.state.room_exists(password) {
            // Response: sending user id
            let id = self.state.add_client(password);
            send(&mut self.stream, 3, &id.to_be_bytes()[..]).await?;
        } else {
            // Error: no room with this password exists
            send(&mut self.stream, 8, &[]).await?;
        }
        Ok(())
    }

    /// Handle "send contact info" request
    async fn receive4(&mut self, msg: &[u8]) -> Result<()> {
        let id = u64::from_be_bytes(msg[0..8].try_into().unwrap());

        if !self.state.id_exists(id) {
            send(&mut self.stream, 9, &[]).await?;
            return Ok(());
        }

        loop {
            let flag = msg[0];
            let msg = &msg[1..];
            match flag {
                1 => self
                    .state
                    .update_client(id, msg_to_addr_v6(&msg[0..18]), false),
                2 => self
                    .state
                    .update_client(id, msg_to_addr_v4(&msg[0..6]), false),
                5 => self.client_done(id).await?,
                _ => return self.invalid_syntax().await,
            };
        }
    }

    async fn client_done(&mut self, client_id: u64) -> Result<()> {
        let contacts = self.state.set_client_done(client_id).await.unwrap();

        let mut msg = Vec::new();

        for contact in contacts {
            msg.push(6);
            if let Some(addr) = contact.private_v6 {
                msg.push(1);
                msg.extend(addr_v6_to_msg(addr));
            }
            if let Some(addr) = contact.private_v4 {
                msg.push(2);
                msg.extend(addr_v4_to_msg(addr));
            }
            if let Some(addr) = contact.public_v6 {
                msg.push(3);
                msg.extend(addr_v6_to_msg(addr));
            }
            if let Some(addr) = contact.public_v4 {
                msg.push(4);
                msg.extend(addr_v4_to_msg(addr));
            }
        }

        send(&mut self.stream, 5, &msg).await?;

        Ok(())
    }

    async fn invalid_syntax(&mut self) -> Result<()> {
        send(&mut self.stream, 6, &[]).await?;
        Err(ClientSyntaxError)?
    }
}

macro_rules! bytes_to_int {
    ($bytes:expr, $type:ty) => {{
        <$type>::from_be_bytes($bytes.try_into().unwrap())
    }};
}

fn msg_to_addr_v6(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..16], u128);
    let port = bytes_to_int!(msg[16..18], u16);
    SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ip)), port)
}

fn msg_to_addr_v4(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..4], u32);
    let port = bytes_to_int!(msg[4..6], u16);
    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port)
}

fn addr_v6_to_msg(addr: SocketAddrV6) -> [u8; 18] {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: [u8; 18] = [0; 18];
    let (left, right) = msg.split_at_mut(16);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}

fn addr_v4_to_msg(addr: SocketAddrV4) -> RoomId {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: RoomId = [0; 6];
    let (left, right) = msg.split_at_mut(4);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}
