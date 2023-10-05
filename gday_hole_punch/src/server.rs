mod connection_handler;
mod global_state;


use crate::SerializationError;

use self::global_state::State;
use connection_handler::ConnectionHandler;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;


#[derive(Error, Debug)]
pub enum ServerError {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialization error {0}")]
    SerializationError(#[from] SerializationError),
}

pub async fn run(listener: TcpListener, tls_acceptor: TlsAcceptor) -> Result<(), ServerError> {
    let state = State::default();
    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await?;
            ConnectionHandler::start(state, tls_stream).await
        });
    }
}
