use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
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
    v6: Option<(TlsStream<TcpStream>, SocketAddr)>, // TODO: Maybe change to SocketAddrV6 ???
    v4: Option<(TlsStream<TcpStream>, SocketAddr)>,
}

impl ServerConnection {
    pub async fn new(server_addr: ServerAddr) -> Result<Self, std::io::Error> {
        let tls_connector =
            Self::get_tls_connector(include_bytes!("cert_authority.der").to_vec()).unwrap();

        let v6 = Self::connect_to_addr(
            TcpSocket::new_v6()?,
            server_addr.v6,
            server_addr.name,
            &tls_connector,
        )
        .await;
        let v4 = Self::connect_to_addr(
            TcpSocket::new_v4()?,
            server_addr.v4,
            server_addr.name,
            &tls_connector,
        )
        .await;

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
            panic!("Server connection not valid.")
        }
    }

    pub fn get_all_streams_with_sockets(&mut self) -> Vec<(&mut TlsStream<TcpStream>, SocketAddr)> {
        let mut streams = Vec::new();

        if let Some((stream, addr)) = &mut self.v6 {
            streams.push((stream, *addr));
        }
        if let Some((stream, addr)) = &mut self.v4 {
            streams.push((stream, *addr));
        }

        streams
    }

    pub fn get_all_addr(&self) -> (Option<SocketAddr>, Option<SocketAddr>) {
        (
            self.v6.as_ref().map(|conn| conn.1),
            self.v4.as_ref().map(|conn| conn.1),
        )
    }

    pub fn get_v4_addr(&self) -> Option<SocketAddr> {
        self.v4.as_ref().map(|conn| conn.1)
    }

    pub fn get_v6_addr(&self) -> Option<SocketAddr> {
        self.v6.as_ref().map(|conn| conn.1)
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

    async fn connect_to_addr(
        tcp_socket: TcpSocket,
        server_addr: impl Into<SocketAddr>,
        server_name: &str,
        tls_connector: &TlsConnector,
    ) -> Result<(TlsStream<TcpStream>, SocketAddr), std::io::Error> {
        tcp_socket.set_reuseaddr(true)?;
        let tcp_stream = tcp_socket.connect(server_addr.into()).await?;
        let local_addr = tcp_stream.local_addr()?;
        let tls_stream = tls_connector
            .connect(server_name.try_into().unwrap(), tcp_stream)
            .await?;
        Ok((tls_stream, local_addr))
    }
}
