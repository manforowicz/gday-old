use rand::seq::SliceRandom;
use sha2::{Digest, Sha256};
use socket2::{SockRef, TcpKeepalive};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::{net::SocketAddr, pin::Pin, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
};

use crate::FullContact;
use std::future::Future;

use super::ClientError;

pub type PeerSecret = [u8; 3];

type PeerConnection = (TcpStream, [u8; 32]);

pub struct PeerConnector {
    pub(super) local: FullContact,
    pub(super) peer: FullContact,
    pub(super) is_creator: bool,
}

impl PeerConnector {
    pub fn get_local_contact(&self) -> FullContact {
        self.local
    }

    pub fn get_peer_contact(&self) -> FullContact {
        self.peer
    }

    pub async fn connect_to_peer(
        self,
        shared_secret: PeerSecret,
    ) -> std::io::Result<PeerConnection> {
        let c = self.is_creator;
        let p = shared_secret;
        let mut futs: Vec<Pin<Box<dyn Future<Output = std::io::Result<PeerConnection>>>>> =
            Vec::with_capacity(6);

        if let Some(local) = self.local.private.v6 {
            futs.push(Box::pin(try_accept(local, p, c)));

            if let Some(peer) = self.peer.private.v6 {
                futs.push(Box::pin(try_connect(local, peer, p, c)));
            }
            if let Some(peer) = self.peer.public.v6 {
                futs.push(Box::pin(try_connect(local, peer, p, c)));
            }
        }

        if let Some(local) = self.local.private.v4 {
            futs.push(Box::pin(try_accept(local, p, c)));

            if let Some(peer) = self.peer.private.v4 {
                futs.push(Box::pin(try_connect(local, peer, p, c)));
            }

            if let Some(peer) = self.peer.public.v4 {
                futs.push(Box::pin(try_connect(local, peer, p, c)));
            }
        }

        Ok(futures::future::select_ok(futs).await?.0)
    }
}

pub fn random_peer_secret() -> PeerSecret {
    let mut rng = rand::thread_rng();
    let characters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut id = [0; 3];
    for letter in &mut id {
        *letter = *characters.choose(&mut rng).unwrap();
    }
    id
}

async fn try_connect<T: Into<SocketAddr>>(
    local: T,
    peer: T,
    peer_id: PeerSecret,
    is_creator: bool,
) -> std::io::Result<PeerConnection> {
    let local = local.into();
    let peer = peer.into();
    loop {
        let local_socket = get_local_socket(local)?;
        let stream = local_socket.connect(peer).await?;
        if let Ok(connection) = verify_peer(peer_id, stream, is_creator).await {
            return Ok(connection);
        }
    }
}

async fn try_accept(
    local: impl Into<SocketAddr>,
    peer_id: PeerSecret,
    is_creator: bool,
) -> std::io::Result<PeerConnection> {
    let local = local.into();
    let local_socket = get_local_socket(local)?;
    let listener = local_socket.listen(1024)?;
    loop {
        let (stream, _addr) = listener.accept().await?;
        if let Ok(connection) = verify_peer(peer_id, stream, is_creator).await {
            return Ok(connection);
        }
    }
}

async fn verify_peer(
    peer_id: PeerSecret,
    mut stream: TcpStream,
    is_creator: bool,
) -> Result<PeerConnection, ClientError> {
    let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(peer_id),
        &Identity::new(b"psend peer"),
    );

    stream.write_all(&outbound_msg).await?;

    let mut inbound_message = [0; 33];
    stream.read_exact(&mut inbound_message).await?;

    let shared_secret: [u8; 32] = spake.finish(&inbound_message)?.try_into().unwrap();

    let my_code = get_verification_code(shared_secret, is_creator);
    let peer_code = get_verification_code(shared_secret, !is_creator);

    stream.write_all(&my_code).await?;

    let mut received = [0; 32];

    stream.read_exact(&mut received).await?;

    if received == peer_code {
        Ok((stream, shared_secret))
    } else {
        Err(ClientError::PeerAuthenticationFailed)
    }
}

fn get_verification_code(key: [u8; 32], is_creator: bool) -> [u8; 32] {
    Sha256::new()
        .chain_update(key)
        .chain_update([u8::from(is_creator)])
        .finalize()
        .into()
}

fn get_local_socket(local_addr: SocketAddr) -> std::io::Result<TcpSocket> {
    let socket = match local_addr {
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
    };

    let sock2 = SockRef::from(&socket);

    let _ = sock2.set_reuse_address(true);
    let _ = sock2.set_reuse_port(true);

    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(1))
        .with_retries(10);
    let _ = sock2.set_tcp_keepalive(&keepalive);

    socket.bind(local_addr)?;
    Ok(socket)
}
