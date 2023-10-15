#![warn(clippy::all, clippy::pedantic)]

use clap::Parser;
use gday_hole_punch::server;
use socket2::{SockRef, TcpKeepalive};
use std::fs;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_rustls::rustls;

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
    let listener = TcpListener::bind("0.0.0.0:49870").await.unwrap_or_else(|err| {
        println!("Error binding listener socket: {err}");
        exit(1)
    });
    let sock2 = SockRef::from(&listener);
    let tcp_keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(60))
        .with_interval(Duration::from_secs(1))
        .with_retries(10);

    sock2.set_tcp_keepalive(&tcp_keepalive).unwrap_or_else(|err| {
        println!("Error setting TCP KeepAlive: {err}");
        exit(1)
    });

    let tls_acceptor = get_tls_acceptor();

    if let Err(err) = server::run(listener, tls_acceptor).await {
        println!("Server stopped due to error: {err}");
    }
}

fn get_tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let cli = Cli::parse();

    let key_file = fs::read(&cli.key).unwrap_or_else(|err| {
        println!("Couldn't open key '{}': {}", cli.key.display(), err);
        exit(1)
    });

    let cert_file = fs::read(&cli.certificate).unwrap_or_else(|err| {
        println!(
            "Couldn't open certificate '{}': {}",
            cli.certificate.display(),
            err
        );
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
        println!("Error making TLS Acceptor: {err}");
        exit(1);
    })
}
