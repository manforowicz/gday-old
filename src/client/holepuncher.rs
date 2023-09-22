use std::net::SocketAddr;
use futures::{stream::FuturesUnordered, StreamExt};
use rand::seq::SliceRandom;
use sha2::{Sha256, Digest};
use spake2::{Spake2, Ed25519Group, Password, Identity};
use tokio::{net::{TcpSocket, TcpStream}, io::{AsyncWriteExt, AsyncReadExt}};

use crate::{Error, protocol::{FullContact, Contact}};

use super::contact_share::PeerConnection;

pub type PeerSecret = [u8; 3];



pub fn random_peer_secret() -> PeerSecret {
    let mut rng = rand::thread_rng();
    let characters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut id = [0; 3];
    for letter in &mut id {
        *letter = *characters.choose(&mut rng).unwrap();
    }

    id
}

pub async fn get_peer_conection(peer: FullContact, peer_id: PeerSecret, is_creator: bool, my_contact: Contact) -> Result<PeerConnection, Error> {
    let c = is_creator;
    let p = peer_id;
    let mut futs = FuturesUnordered::new();


    //TODO: REMOVE TOKIO SPAwNS HERE. THEY'RE UNNECESSARY

    if let Some(local) = my_contact.v6 {
        futs.push(tokio::spawn(try_accept(local, p, c)));

        if let Some(peer) = peer.private.v6 {
            futs.push(tokio::spawn(try_connect(local, peer, p, c)));
        }
        if let Some(peer) = peer.public.v6 {
            futs.push(tokio::spawn(try_connect(local, peer, p, c)));
        }
    }

    if let Some(local) = my_contact.v4 {
        futs.push(tokio::spawn(try_accept(local, p, c)));

        if let Some(peer) = peer.private.v4 {
            futs.push(tokio::spawn(try_connect(local, peer, p, c)));
        }

        if let Some(peer) = peer.public.v4 {
            futs.push(tokio::spawn(try_connect(local, peer, p, c)));
        }
    }

    while let Some(result) = futs.next().await {
        if let Ok(Ok(connection)) = result {
            return Ok(connection);
        }
    }

    Err(Error::PeerConnectFailed)
}





async fn try_connect(
    local: impl Into<SocketAddr>,
    peer: impl Into<SocketAddr>,
    peer_id: PeerSecret,
    is_creator: bool,
) -> Result<PeerConnection, std::io::Error> {
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
) -> Result<PeerConnection, std::io::Error> {
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
) -> Result<PeerConnection, Error> {
    let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(peer_id),
        &Identity::new(b"psend peer"),
    );
    println!("trying to verify");

    stream.write_all(&outbound_msg).await?;

    let mut inbound_message = [0; 33];
    stream.read_exact(&mut inbound_message).await?;

    let shared_key: [u8; 32] = spake.finish(&inbound_message)?.try_into().unwrap();

    let my_code = get_verification_code(shared_key, is_creator);
    let peer_code = get_verification_code(shared_key, !is_creator);

    stream.write_all(&my_code).await?;

    let mut received = [0; 32];

    stream.read_exact(&mut received).await?;

    if received == peer_code {
        Ok(PeerConnection {
            stream,
            shared_secret: shared_key,
        })
    } else {
        Err(Error::PeerAuthenticationFailed)
    }
}

fn get_verification_code(key: [u8; 32], is_creator: bool) -> [u8; 32] {
    Sha256::new()
        .chain_update(key)
        .chain_update([u8::from(is_creator)])
        .finalize()
        .into()
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