#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::must_use_candidate, dead_code)]

use std::num::TryFromIntError;

use thiserror::Error;


pub mod server;
pub mod client;
mod protocol;


#[derive(Error, Debug)]
pub enum Error {
    #[error("Error with encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("Message cannot be longer than max of {} bytes", u8::MAX)]
    MessageTooLong(#[from] TryFromIntError),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Temporary buffer too small")]
    TmpBufTooSmall,

    #[error("Peer cryptographical error")]
    Cyrptographical,

    #[error("Double check the first 6 characters of your password!")]
    InvalidServerReply(protocol::ServerMessage),
    #[error("Couldn't connect to peer")]
    PeerConnectFailed,

    #[error(
        "Key exchange failed: {0}"
    )]
    SpakeFailed(#[from] spake2::Error),

    #[error(
        "Couldn't authenticate peer. Check last 3 characters of password!"
    )]
    PeerAuthenticationFailed,

    #[error("Rustls error")]
    Rustls(#[from] tokio_rustls::rustls::Error),
}