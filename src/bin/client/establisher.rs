use std::net::SocketAddr::{self, V4, V6};

use futures::stream::{FuturesUnordered, StreamExt};
use holepunch::{deserialize_from, serialize_into, ClientMessage, FullContact, ServerMessage};
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
    #[error("Server sent different message than expected")]
    InvalidServerReply(holepunch::ServerMessage),
    #[error("Couldn't connect to peer")]
    PeerConnectFailed,
    #[error("Password authenticated key exchange failed: {0}")]
    SpakeFailed(#[from] spake2::Error),
}

pub struct Establisher {
    password: [u8; 9],
    creator: bool,
    connection: ServerConnection,
}

impl Establisher {
    pub async fn create_room(server_addr: ServerAddr) -> Result<Self, Error> {
        let mut connection = ServerConnection::new(server_addr).await?;

        let password = Self::request_room(connection.get_any_stream()).await?;

        Ok(Self {
            password,
            creator: true,
            connection,
        })
    }

    async fn request_room(stream: &mut TlsStream<TcpStream>) -> Result<[u8; 9], Error> {
        serialize_into(stream, &ClientMessage::CreateRoom).await?;
        let response: ServerMessage = deserialize_from(stream).await?;

        if let ServerMessage::RoomCreated(password) = response {
            Ok(password)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    pub async fn join_room(server_addr: ServerAddr, password: [u8; 9]) -> Result<Self, Error> {
        let connection = ServerConnection::new(server_addr).await?;

        Ok(Self {
            password,
            creator: false,
            connection,
        })
    }

    pub fn get_password(&self) -> [u8; 9] {
        self.password
    }

    pub async fn get_peer_conection(&mut self) -> Result<PeerConnection, Error> {
        let peer = self.get_peer_contact().await?;

        let (local_v6, local_v4) = self.connection.get_all_addr();

        let p = self.password;

        let mut futs = FuturesUnordered::new();

        if let Some(local_addr) = local_v6 {
            futs.push(tokio::spawn(Self::try_accept(local_addr, p)));

            if let Some(socket) = peer.private.v6 {
                futs.push(tokio::spawn(Self::try_connect(local_addr, V6(socket), p)));
            }
            if let Some(socket) = peer.public.v6 {
                futs.push(tokio::spawn(Self::try_connect(local_addr, V6(socket), p)));
            }
        }

        if let Some(local_addr) = local_v4 {
            futs.push(tokio::spawn(Self::try_accept(local_addr, p)));

            if let Some(socket) = peer.private.v4 {
                futs.push(tokio::spawn(Self::try_connect(local_addr, V4(socket), p)));
            }

            if let Some(socket) = peer.public.v4 {
                futs.push(tokio::spawn(Self::try_connect(local_addr, V4(socket), p)));
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
                ClientMessage::SendContact(self.password, self.creator, Some(conns[i].1), is_done);
            serialize_into(conns[i].0, &msg).await?;
        }

        let response: ServerMessage = deserialize_from(conns.last_mut().unwrap().0).await?;

        if let ServerMessage::SharePeerContacts(full_contact) = response {
            Ok(full_contact)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    async fn try_connect(
        local: SocketAddr,
        peer: SocketAddr,
        password: [u8; 9],
    ) -> Result<PeerConnection, std::io::Error> {
        loop {
            let local_socket = Self::get_local_socket(local)?;
            let stream = local_socket.connect(peer).await?;
            if let Ok(connection) = Self::verify_peer(password, stream).await {
                return Ok(connection);
            }
        }
    }

    async fn try_accept(
        local: SocketAddr,
        password: [u8; 9],
    ) -> Result<PeerConnection, std::io::Error> {
        let local_socket = Self::get_local_socket(local)?;
        let listener = local_socket.listen(1024)?;
        loop {
            let (stream, _addr) = listener.accept().await?;
            if let Ok(connection) = Self::verify_peer(password, stream).await {
                return Ok(connection);
            }
        }
    }

    async fn verify_peer(
        password: [u8; 9],
        mut stream: TcpStream,
    ) -> Result<PeerConnection, Error> {
        let (s, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(password),
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
