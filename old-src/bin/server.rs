#![warn(clippy::all, clippy::pedantic)]



use std::sync::Arc;

use holepunch::server::connection_handler::ConnectionHandler;
use holepunch::server::global_state::State;

use tokio::net::TcpListener;

use tokio_rustls::rustls;

use clap::Parser;

use std::fs;
use std::path::PathBuf;
use std::process::exit;


/// Run a TODO server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Private TLS Key
    #[arg(short, long)]
    key: PathBuf,

    /// Signed server TLS certificate
    #[arg(short, long)]
    certificate: PathBuf,
}

#[tokio::main]
async fn main() {
    let state = State::default();
    let listener = TcpListener::bind("0.0.0.0:49870").await.unwrap();

    let tls_acceptor = get_tls_acceptor();

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let tls_stream = tls_acceptor.accept(stream).await?;
            ConnectionHandler::start(state, tls_stream, addr).await;
            Result::<(), std::io::Error>::Ok(())
        });
    }
}

fn get_tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let cli = Cli::parse();

    let key_file = fs::read(&cli.key).unwrap_or_else(|err| {
        eprintln!("Couldn't open key '{}': {}", cli.key.display(), err);
        exit(1)
    });

    let cert_file = fs::read(&cli.certificate).unwrap_or_else(|err| {
        eprintln!("Couldn't open certificate '{}': {}", cli.certificate.display(), err);
        exit(1)
    });

    let key = rustls::PrivateKey(key_file);
    let cert = rustls::Certificate(cert_file);

    let tls_config = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .unwrap();

    tokio_rustls::TlsAcceptor::try_from(Arc::new(tls_config)).unwrap_or_else(|err| {
        eprintln!("Error making TLS Acceptor: {err}");
        exit(1);
    })
}
