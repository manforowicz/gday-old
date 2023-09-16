use crate::client::{
    peer_connection::PeerConnection,
    server_connection::{ServerAddr, ServerConnection},
};
use futures::stream::{FuturesUnordered, StreamExt};
use crate::protocol::{deserialize_from, serialize_into, ClientMessage, FullContact, ServerMessage};
use rand::seq::SliceRandom;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::net::SocketAddr;
use crate::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
};
use tokio_rustls::{self, client::TlsStream};



pub struct Establisher {
    room_id: [u8; 6],
    peer_id: [u8; 3],
    creator: bool,
    connection: ServerConnection,
    tmp_buf: [u8; 68],
}

impl Establisher {
    pub async fn create_room(server_addr: ServerAddr) -> Result<Self, Error> {
        let mut connection = ServerConnection::new(server_addr).await?;

        let mut tmp_buf = [0; 68];

        Ok(Self {
            room_id: Self::request_room(connection.get_any_stream(), &mut tmp_buf).await?,
            peer_id: Self::generate_peer_id(),
            creator: true,
            connection,
            tmp_buf,
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

    async fn request_room(
        stream: &mut TlsStream<TcpStream>,
        tmp_buf: &mut [u8],
    ) -> Result<[u8; 6], Error> {
        serialize_into(stream, &ClientMessage::CreateRoom, tmp_buf).await?;
        let response: ServerMessage = deserialize_from(stream, tmp_buf).await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok(room_id)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    pub async fn join_room(server_addr: ServerAddr, password: [u8; 9]) -> Result<Self, Error> {
        let connection = ServerConnection::new(server_addr).await?;

        Ok(Self {
            room_id: password[0..6].try_into().expect("Unreachable. Slice is correct size."),
            peer_id: password[6..9].try_into().expect("Unreachable. Slice is correct size."),
            creator: false,
            connection,
            tmp_buf: [0; 68],
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
                futs.push(tokio::spawn(Self::try_connect(addr, socket, p)));
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

        for conn in &mut conns {
            let msg = ClientMessage::SendContact(self.room_id, self.creator, Some(conn.1));
            serialize_into(conn.0, &msg, &mut self.tmp_buf).await?;
        }

        let msg = ClientMessage::DoneSending(self.room_id, self.creator);
        serialize_into(conns[0].0, &msg, &mut self.tmp_buf).await?;

        println!("Waiting for peer...");

        let response: ServerMessage =
            deserialize_from(conns.last_mut().unwrap().0, &mut self.tmp_buf).await?;

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
        let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
            &Password::new(peer_id),
            &Identity::new(b"psend peer"),
        );

        stream.write_all(&outbound_msg).await?;

        let mut inbound_message = [0; 33];
        stream.read_exact(&mut inbound_message).await?;

        let shared_key = spake.finish(&inbound_message)?.try_into().unwrap();

        Ok(PeerConnection { stream, shared_secret: shared_key })
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
