mod connection_handler;
mod global_state;

use connection_handler::ConnectionHandler;
use global_state::State;

use tokio::net::TcpListener;
use tokio_native_tls::{native_tls, TlsAcceptor};

#[tokio::main]
async fn main() {
    let state = State::default();

    let listener_v6 = TcpListener::bind("::8080").await.unwrap();
    let listener_v4 = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let der = include_bytes!("identity.pfx");
    let identity =
        native_tls::Identity::from_pkcs12(der, "trolejbus").expect("Couldn't decrypt certificate");
    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());

    tokio::spawn(listen(listener_v6, tls_acceptor.clone(), state.clone()));
    tokio::spawn(listen(listener_v4, tls_acceptor.clone(), state.clone()));
}

async fn listen(listener: TcpListener, tls_acceptor: TlsAcceptor, state: State) {
    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await?;
            ConnectionHandler::start(state, tls_stream, addr).await;
            Result::<(), native_tls::Error>::Ok(())
        });
    }
}
