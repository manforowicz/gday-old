use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::process::exit;

use holepunch::protocol;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use tokio_native_tls::{native_tls, TlsConnector};

const SERVER_ADDR_V4: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(138, 2, 238, 120), 49870);
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
    let cert = native_tls::Certificate::from_pem(include_bytes!("MyCertificate.crt")).unwrap();
    let tls_connector = TlsConnector::from(
        native_tls::TlsConnector::builder()
            .add_root_certificate(cert)
            .build()
            .unwrap(),
    );

    get_peer_contact(SocketAddr::from(SERVER_ADDR_V4), tls_connector).await;
}

async fn get_peer_contact(server_addr: SocketAddr, tls_connector: TlsConnector) {
    let tcp_stream = TcpStream::connect(server_addr).await.unwrap();
    let tls_stream = tls_connector.connect("marcin", tcp_stream).await.unwrap();
}
