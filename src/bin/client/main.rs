use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, IpAddr};
use std::sync::Arc;


use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use tokio_rustls::rustls::{self, ServerName};
use tokio_rustls::{self, TlsConnector};

const SERVER_ADDR_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 49870);
const SERVER_ADDR_V6: SocketAddrV6 = SocketAddrV6::new(
    Ipv6Addr::new(
        0x2603, 0xc024, 0xc00c, 0xb17e, 0xfce5, 0xf16d, 0x4207, 0xb22d,
    ),
    49870,
    0,
    0,
);

#[tokio::main]
async fn main() {
    let cert = rustls::Certificate(include_bytes!("cert_authority.der").to_vec());
    let mut cert_store = rustls::RootCertStore::empty();
    cert_store.add(&cert).unwrap();
    let config = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(cert_store)
        .with_no_client_auth();

    let tls_connector = TlsConnector::from(Arc::new(config));

    get_peer_contact(SocketAddr::from(SERVER_ADDR_V4), tls_connector).await;
}

async fn get_peer_contact(server_addr: SocketAddr, tls_connector: TlsConnector) {
    println!("first stop");
    let tcp_stream = TcpStream::connect(server_addr).await.unwrap();
    println!("got here");

    let server_name = ServerName::IpAddress(server_addr.ip());
    let tls_stream = tls_connector.connect("psend".try_into().unwrap(), tcp_stream).await.unwrap();
    println!("done!!!");
}

async fn thing<T: AsyncReadExt, AsyncWriteExt, Unpin>(stream: T) {

}
