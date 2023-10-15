mod connection_handler;
mod global_state;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

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

    #[error("Room timed out.")]
    RoomTimedOut,

    #[error("No such room id exists")]
    NoSuchRoomId,
}

pub async fn run(listener: TcpListener, tls_acceptor: TlsAcceptor) -> Result<(), ServerError> {
    let state = State::default();

    let blocked: Arc<Mutex<HashMap<IpAddr, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        let blocked = blocked.clone();
        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await?;

            let is_blocked = blocked.lock().unwrap().get(&addr.ip()).copied();

            if let Some(time) = is_blocked {
                tokio::time::sleep(time - Instant::now()).await;
            }

            blocked
                .lock()
                .unwrap()
                .insert(addr.ip(), Instant::now() + Duration::from_secs(5));

            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let mut blocked = blocked.lock().unwrap();
                if let Some(&deadline) = blocked.get(&addr.ip()) {
                    if deadline <= Instant::now() {
                        blocked.remove(&addr.ip());
                    }
                }

            });

            ConnectionHandler::start(state, tls_stream).await
        });
    }
}
