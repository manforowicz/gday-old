mod contact_sharer;
mod peer_connector;

use crate::{SerializationError, ServerMessage};
pub use contact_sharer::ContactSharer;
pub use peer_connector::{PeerConnector, PeerSecret, random_peer_secret};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {

    #[error("Both provided addresses were None.")]
    NoAddressProvided,
    
    #[error("Received an IPv4, but expected IPv6.")]
    ExpectedIPv6,

    #[error("Received an IPV6, but expected an IPv4.")]
    ExpectedIPv4,


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
