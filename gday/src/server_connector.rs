use std::{net::SocketAddr, sync::Arc};

use tokio::net::{TcpStream, TcpSocket};
use tokio_rustls::{TlsConnector, rustls, client::TlsStream};




pub fn get_tls_connector(cert_authority: Vec<u8>) -> Result<TlsConnector, rustls::Error> {
    let cert = rustls::Certificate(cert_authority);
    let mut cert_store = rustls::RootCertStore::empty();
    cert_store.add(&cert)?;
    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(cert_store)
        .with_no_client_auth();

    Ok(TlsConnector::from(Arc::new(config)))
}

pub async fn connect(
    server_addr: impl Into<SocketAddr>,
    server_name: &str,
    tls_connector: &TlsConnector,
) -> std::io::Result<TlsStream<TcpStream>> {
    let server_addr = server_addr.into();
    let socket = match server_addr {
        SocketAddr::V6(_) => TcpSocket::new_v6(),
        SocketAddr::V4(_) => TcpSocket::new_v4(),
    }?;
    let _ = socket.set_reuseaddr(true);
    let _ = socket.set_reuseport(true);
    
    let tcp_stream = socket.connect(server_addr).await?;
    let tls_stream = tls_connector
        .connect(server_name.try_into().unwrap(), tcp_stream)
        .await?;

    Ok(tls_stream)
}