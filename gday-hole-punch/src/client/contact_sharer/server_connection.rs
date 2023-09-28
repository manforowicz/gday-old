use crate::{Contact, client::ServerAddr, Messenger};
use std::net::{
    SocketAddr,
    SocketAddr::{V4, V6},
    SocketAddrV4, SocketAddrV6,
};
use std::sync::Arc;
use tokio::net::{TcpSocket, TcpStream};
use tokio_rustls::rustls;
use tokio_rustls::{self, client::TlsStream, TlsConnector};

use super::ClientError;


pub struct ServerConnection {
    v6: Option<MyMessenger>,
    v4: Option<MyMessenger>,
}

type MyMessenger = Messenger<TlsStream<TcpStream>>;

impl ServerConnection {
    pub async fn new(server_addr: ServerAddr) -> Result<Self, ClientError> {
        let root_cert = include_bytes!("cert_authority.der").to_vec();
        let tls_conn = get_tls_connector(root_cert)?;

        let v6 = connect(server_addr.v6, server_addr.name, &tls_conn).await;
        let v4 = connect(server_addr.v4, server_addr.name, &tls_conn).await;

        if v6.is_err() && v4.is_err() {
            Err(v4.unwrap_err())?
        } else {
            Ok(Self {
                v6: v6.ok(),
                v4: v4.ok(),
            })
        }
    }

    pub(super) fn get_any_messenger(&mut self) -> &mut MyMessenger {
        if let Some(stream) = &mut self.v6 {
            stream
        } else if let Some(stream) = &mut self.v4 {
            stream
        } else {
            unreachable!()
        }
    }

    pub(super) fn get_all_streams_with_sockets(
        &mut self,
    ) -> std::io::Result<Vec<(&mut MyMessenger, SocketAddr)>> {
        let mut streams = Vec::new();

        if let Some(stream) = &mut self.v6 {
            let addr = V6(addr_v6_from_stream(stream)?);
            streams.push((stream, addr));
        }
        if let Some(stream) = &mut self.v4 {
            let addr = V4(addr_v4_from_stream(stream)?);
            streams.push((stream, addr));
        }

        Ok(streams)
    }

    pub fn get_local_contact(&self) -> std::io::Result<Contact> {
        Ok(Contact {
            v6: if let Some(stream) = &self.v6 {
                Some(addr_v6_from_stream(stream)?)
            } else {
                None
            },
            v4: if let Some(stream) = &self.v4 {
                Some(addr_v4_from_stream(stream)?)
            } else {
                None
            },
        })
    }
}

fn addr_v6_from_stream(stream: &MyMessenger) -> std::io::Result<SocketAddrV6> {
    let addr = stream.inner_stream().get_ref().0.local_addr()?;
    let V6(v6) = addr else {
        panic!("Called unwrap_v6 on SocketAddrV4")
    };
    Ok(v6)
}

fn addr_v4_from_stream(stream: &MyMessenger) -> std::io::Result<SocketAddrV4> {
    let addr = stream.inner_stream().get_ref().0.local_addr()?;
    let V4(v4) = addr else {
        panic!("Called unwrap_v6 on SocketAddrV4")
    };
    Ok(v4)
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

async fn connect(
    server_addr: impl Into<SocketAddr>,
    server_name: &str,
    tls_connector: &TlsConnector,
) -> std::io::Result<MyMessenger> {
    let server_addr = server_addr.into();
    let socket = match server_addr {
        V6(_) => TcpSocket::new_v6(),
        V4(_) => TcpSocket::new_v4(),
    }?;
    let _ = socket.set_reuseaddr(true);
    let _ = socket.set_reuseport(true);
    let tcp_stream = socket.connect(server_addr).await?;
    let tls_stream = tls_connector
        .connect(server_name.try_into().unwrap(), tcp_stream)
        .await?;

    Ok(Messenger::with_capacity(tls_stream, 68))
}
