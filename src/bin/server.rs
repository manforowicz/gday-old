use holepunch::connection_handler::ConnectionHandler;
use holepunch::global_state::State;

use tokio::net::TcpListener;
use tokio_native_tls::native_tls;

#[tokio::main]
async fn main() {
    let state = State::default();

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let der = std::fs::read("identity.pfx").expect("Couldn't find server certificate file");

    let identity =
        native_tls::Identity::from_pkcs12(&der, "trolejbus").expect("Couldn't decrypt certificate");
    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await?;
            ConnectionHandler::start(state, tls_stream).await;
            Result::<(), native_tls::Error>::Ok(())
        });
    }
}
