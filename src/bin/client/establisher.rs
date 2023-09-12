use std::net::SocketAddr;

use futures::stream::{FuturesUnordered, StreamExt};
use holepunch::{deserialize_from, serialize_into, ClientMessage, FullContact, ServerMessage};
use rand::seq::SliceRandom;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
};
use tokio_rustls::{self, client::TlsStream};

use crate::{
    peer_connection::PeerConnection,
    server_connection::{ServerAddr, ServerConnection},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Holepunch Error: {0}")]
    Holepunch(#[from] holepunch::Error),
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Double check the first 6 characters of your password!")]
    InvalidServerReply(holepunch::ServerMessage),
    #[error("Couldn't connect to peer")]
    PeerConnectFailed,
    #[error("Peer authentication failed: {0}. Double check the first 3 characters of your password!")]
    SpakeFailed(#[from] spake2::Error),
}

pub struct Establisher {
    room_id: [u8; 6],
    peer_id: [u8; 3],
    creator: bool,
    connection: ServerConnection,
}

impl Establisher {
    pub async fn create_room(server_addr: ServerAddr) -> Result<Self, Error> {
        let mut connection = ServerConnection::new(server_addr).await?;

        Ok(Self {
            room_id: Self::request_room(connection.get_any_stream()).await?,
            peer_id: Self::generate_peer_id(),
            creator: true,
            connection,
        })
    }

    fn generate_peer_id() -> [u8; 3] {
        let mut rng = rand::thread_rng();
        let characters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut id = [0; 3];
        for letter in &mut id {
            *letter = *characters.choose(&mut rng).unwrap();
        }

        id
    }

    async fn request_room(stream: &mut TlsStream<TcpStream>) -> Result<[u8; 6], Error> {
        serialize_into(stream, &ClientMessage::CreateRoom).await?;
        let response: ServerMessage = deserialize_from(stream).await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok(room_id)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    pub async fn join_room(server_addr: ServerAddr, password: [u8; 9]) -> Result<Self, Error> {
        let connection = ServerConnection::new(server_addr).await?;

        Ok(Self {
            room_id: password[0..6].try_into().unwrap(),
            peer_id: password[6..9].try_into().unwrap(),
            creator: false,
            connection,
        })
    }

    pub fn get_password(&self) -> [u8; 9] {
        let mut password = [0; 9];
        password[0..6].copy_from_slice(&self.room_id);
        password[6..9].copy_from_slice(&self.peer_id);
        password
    }

    pub async fn get_peer_conection(&mut self) -> Result<PeerConnection, Error> {
        let peer = self.get_peer_contact().await?;

        let (local_v6, local_v4) = self.connection.get_all_addr();

        let p = self.peer_id;

        let mut futs = FuturesUnordered::new();

        if let Some(addr) = local_v6 {
            futs.push(tokio::spawn(Self::try_accept(addr, p)));

            if let Some(socket) = peer.private_v6 {
                futs.push(tokio::spawn(Self::try_connect(addr, socket, p)));
            }
            if let Some(socket) = peer.public_v6 {
                futs.push(tokio::spawn(Self::try_connect(addr, socket, p)));
            }
        }

        if let Some(addr) = local_v4 {
            futs.push(tokio::spawn(Self::try_accept(addr, p)));

            if let Some(socket) = peer.private_v4 {
                futs.push(tokio::spawn(Self::try_connect(addr, socket, p)));
            }

            if let Some(socket) = peer.public_v4 {
                futs.push(tokio::spawn(Self::try_connect(addr,socket, p)));
            }
        }

        while let Some(result) = futs.next().await {
            if let Ok(Ok(connection)) = result {
                return Ok(connection);
            }
        }

        Err(Error::PeerConnectFailed)
    }

    async fn get_peer_contact(&mut self) -> Result<FullContact, Error> {
        let mut conns = self.connection.get_all_streams_with_sockets();

        for i in 0..conns.len() {
            let is_done = i == conns.len() - 1;
            let msg =
                ClientMessage::SendContact(self.room_id, self.creator, Some(conns[i].1), is_done);
            serialize_into(conns[i].0, &msg).await?;
        }
        println!("Waiting for peer...");

        let response: ServerMessage = deserialize_from(conns.last_mut().unwrap().0).await?;

        if let ServerMessage::SharePeerContacts(full_contact) = response {
            Ok(full_contact)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    async fn try_connect(
        local: impl Into<SocketAddr>,
        peer: impl Into<SocketAddr>,
        peer_id: [u8; 3],
    ) -> Result<PeerConnection, std::io::Error> {
        let local = local.into();
        let peer = peer.into();
        loop {
            let local_socket = Self::get_local_socket(local)?;
            let stream = local_socket.connect(peer).await?;
            if let Ok(connection) = Self::verify_peer(peer_id, stream).await {
                return Ok(connection);
            }
        }
    }

    async fn try_accept(
        local: impl Into<SocketAddr>,
        peer_id: [u8; 3],
    ) -> Result<PeerConnection, std::io::Error> {
        let local = local.into();
        let local_socket = Self::get_local_socket(local)?;
        let listener = local_socket.listen(1024)?;
        loop {
            let (stream, _addr) = listener.accept().await?;
            if let Ok(connection) = Self::verify_peer(peer_id, stream).await {
                return Ok(connection);
            }
        }
    }

    async fn verify_peer(peer_id: [u8; 3], mut stream: TcpStream) -> Result<PeerConnection, Error> {
        let (s, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(peer_id),
            &Identity::new(b"psend peer"),
        );

        stream.write_all(&outbound_msg).await?;

        let mut inbound_message = [0; 33];
        stream.read_exact(&mut inbound_message).await?;

        let shared_key = s.finish(&inbound_message)?;

        Ok(PeerConnection::new(stream, shared_key.try_into().unwrap()))
    }

    fn get_local_socket(local_addr: SocketAddr) -> Result<TcpSocket, std::io::Error> {
        let socket = match local_addr {
            SocketAddr::V6(_) => TcpSocket::new_v6()?,
            SocketAddr::V4(_) => TcpSocket::new_v4()?,
        };

        let _ = socket.set_reuseaddr(true);
        let _ = socket.set_reuseport(true);
        socket.bind(local_addr)?;
        Ok(socket)
    }
}
