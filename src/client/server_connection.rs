use std::net::{
    SocketAddr,
    SocketAddr::{V4, V6},
    SocketAddrV4, SocketAddrV6,
};
use std::sync::Arc;
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::rustls;
use tokio_rustls::{self, client::TlsStream, TlsConnector};

pub struct ServerAddr {
    pub v6: SocketAddrV6,
    pub v4: SocketAddrV4,
    pub name: &'static str,
}
pub struct ServerConnection {
    v6: Option<(TlsStream<TcpStream>, SocketAddrV6)>,
    v4: Option<(TlsStream<TcpStream>, SocketAddrV4)>,
}

impl ServerConnection {
    pub async fn new(server_addr: ServerAddr) -> Result<Self, std::io::Error> {
        let root_cert = include_bytes!("cert_authority.der").to_vec();
        let tls_conn = Self::get_tls_connector(root_cert).unwrap();

        let v6 = Self::connect_to_v6(server_addr.v6, server_addr.name, &tls_conn).await;
        let v4 = Self::connect_to_v4(server_addr.v4, server_addr.name, &tls_conn).await;

        if v6.is_err() && v4.is_err() {
            Err(v4.unwrap_err())
        } else {
            Ok(Self {
                v6: v6.ok(),
                v4: v4.ok(),
            })
        }
    }

    pub fn get_any_stream(&mut self) -> &mut TlsStream<TcpStream> {
        if let Some((stream, _)) = &mut self.v6 {
            stream
        } else if let Some((stream, _)) = &mut self.v4 {
            stream
        } else {
            unreachable!()
        }
    }

    pub fn get_all_streams_with_sockets(&mut self) -> Vec<(&mut TlsStream<TcpStream>, SocketAddr)> {
        let mut streams = Vec::new();

        if let Some((stream, addr)) = &mut self.v6 {
            streams.push((stream, SocketAddr::from(*addr)));
        }
        if let Some((stream, addr)) = &mut self.v4 {
            streams.push((stream, SocketAddr::from(*addr)));
        }

        streams
    }

    pub fn get_all_addr(&self) -> (Option<SocketAddrV6>, Option<SocketAddrV4>) {
        (
            self.v6.as_ref().map(|conn| conn.1),
            self.v4.as_ref().map(|conn| conn.1),
        )
    }

    fn get_tls_connector(cert_authority: Vec<u8>) -> Result<TlsConnector, rustls::Error> {
        let cert = rustls::Certificate(cert_authority);
        let mut cert_store = rustls::RootCertStore::empty();
        cert_store.add(&cert)?;
        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(cert_store)
            .with_no_client_auth();

        Ok(TlsConnector::from(Arc::new(config)))
    }

    async fn connect_to_v4(
        server_addr: SocketAddrV4,
        server_name: &str,
        tls_connector: &TlsConnector,
    ) -> Result<(TlsStream<TcpStream>, SocketAddrV4), std::io::Error> {
        let (tls_stream, local_addr) =
            Self::connect(false, server_addr, server_name, tls_connector).await?;
        let V4(local_addr) = local_addr else {
            unreachable!()
        };
        Ok((tls_stream, local_addr))
    }

    async fn connect_to_v6(
        server_addr: SocketAddrV6,
        server_name: &str,
        tls_connector: &TlsConnector,
    ) -> Result<(TlsStream<TcpStream>, SocketAddrV6), std::io::Error> {
        let (tls_stream, local_addr) =
            Self::connect(true, server_addr, server_name, tls_connector).await?;
        let V6(local_addr) = local_addr else {
            unreachable!()
        };
        Ok((tls_stream, local_addr))
    }

    async fn connect(
        v6: bool,
        server_addr: impl Into<SocketAddr>,
        server_name: &str,
        tls_connector: &TlsConnector,
    ) -> Result<(TlsStream<TcpStream>, SocketAddr), std::io::Error> {
        let socket = if v6 {
            TcpSocket::new_v6()
        } else {
            TcpSocket::new_v4()
        }?;
        let _ = socket.set_reuseaddr(true);
        let _ = socket.set_reuseport(true);
        let tcp_stream = socket.connect(server_addr.into()).await?;
        let local_addr = tcp_stream.local_addr()?;
        let tls_stream = tls_connector
            .connect(server_name.try_into().unwrap(), tcp_stream)
            .await?;

        Ok((tls_stream, local_addr))
    }
}
