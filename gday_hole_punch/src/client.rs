mod contact_sharer;
mod peer_connector;

use crate::{SerializationError, ServerMessage};
pub use contact_sharer::ContactSharer;
pub use peer_connector::{PeerConnector, PeerSecret, random_peer_secret};
use std::net::{SocketAddrV4, SocketAddrV6};
use thiserror::Error;

pub struct ServerAddr {
    pub v6: SocketAddrV6,
    pub v4: SocketAddrV4,
    pub name: &'static str,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Serialization error {0}")]
    SerializationError(#[from] SerializationError),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Peer cryptographical error")]
    Cyrptographical,

    #[error("Double check the first 6 characters of your password! {0:?}")]
    InvalidServerReply(ServerMessage),

    #[error("Couldn't connect to peer")]
    PeerConnectFailed,

    #[error("Key exchange failed: {0}")]
    SpakeFailed(#[from] spake2::Error),

    #[error("Couldn't authenticate peer. Check last 3 characters of password!")]
    PeerAuthenticationFailed,

    #[error("Rustls error")]
    Rustls(#[from] tokio_rustls::rustls::Error),

    #[error("Invalid utf-8")]
    InvalidUtf8(#[from] std::str::Utf8Error),
}
