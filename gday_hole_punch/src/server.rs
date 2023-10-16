mod connection_handler;
mod global_state;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::SerializationError;

use self::global_state::State;
use connection_handler::ConnectionHandler;
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};
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

    #[error("No such room id exists")]
    ReceivedIncorrectMessage,
}

#[derive(Clone)]
struct GlobalData {
    state: State,
    blocked: Arc<Mutex<HashMap<IpAddr, Option<TcpStream>>>>,
    tls_acceptor: TlsAcceptor,
}

pub async fn run(listener: TcpListener, tls_acceptor: TlsAcceptor) -> Result<(), ServerError> {
    let global_data = GlobalData {
        state: State::default(),
        blocked: Arc::new(Mutex::new(HashMap::new())),
        tls_acceptor,
    };

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(ok) => ok,
            Err(err) => {
                println!("Error accepting connection: {err}");
                continue;
            }
        };
        if let Some(stream_option) = global_data.blocked.lock().unwrap().get_mut(&addr.ip()) {
            *stream_option = Some(stream);
        } else {
            serve_client(stream, global_data.clone());
        }
    }
}

fn serve_client(tcp_stream: TcpStream, global_data: GlobalData) {
    let addr = match tcp_stream.local_addr() {
        Ok(ok) => ok,
        Err(err) => {
            println!("{err}");
            return;
        }
    }
    .ip();
    println!("a");
    global_data.blocked.lock().unwrap().insert(addr, None);
    println("b");

    let global_data2 = global_data.clone();
    tokio::spawn(async move {
        println!("h");
        tokio::time::sleep(Duration::from_secs(5)).await;
        if let Some(Some(tcp_stream)) = global_data.blocked.lock().unwrap().remove(&addr) {
            serve_client(tcp_stream, global_data2);
        }
    });
    println!("e");
    tokio::spawn(async move {
        let tls_stream = match global_data.tls_acceptor.accept(tcp_stream).await {
            Ok(ok) => ok,
            Err(err) => {
                println!("TLS connector error: {err}");
                return;
            }
        };
        if let Err(err) = ConnectionHandler::start(global_data.state, tls_stream).await {
            println!("{err}")
        }
    });
}
