use crate::{Contact, Messenger};
use socket2::SockRef;
use std::net::{
    SocketAddr::{V4, V6},
    SocketAddrV4, SocketAddrV6,
};

use super::{ClientError, Stream};

pub struct ServerConnection {
    v6: Option<Messenger>,
    v4: Option<Messenger>,
}

impl ServerConnection {
    pub async fn new(
        server_addr_v6: Option<Stream>,
        server_addr_v4: Option<Stream>,
    ) -> Result<Self, ClientError> {
        if server_addr_v6.is_none() && server_addr_v4.is_none() {
            return Err(ClientError::NoAddressProvided);
        }

        let mut this = Self { v6: None, v4: None };

        if let Some(stream) = server_addr_v6 {
            let tcp = stream.get_ref().0;
            if !matches!(tcp.local_addr()?, V6(_)) {
                return Err(ClientError::ExpectedIPv6);
            }; 
            this.v6 = Some(configure_stream(stream));
        }

        if let Some(stream) = server_addr_v4 {
            let tcp = stream.get_ref().0;
            if !matches!(tcp.local_addr()?, V4(_)) {
                return Err(ClientError::ExpectedIPv4);
            };
            this.v4 = Some(configure_stream(stream));
        }
        Ok(this)
    }

    pub(super) fn get_any_messenger(&mut self) -> &mut Messenger {
        if let Some(stream) = &mut self.v6 {
            stream
        } else if let Some(stream) = &mut self.v4 {
            stream
        } else {
            unreachable!()
        }
    }

    pub(super) fn get_all_messengers(&mut self) -> std::io::Result<Vec<&mut Messenger>> {
        let mut messengers = Vec::new();

        if let Some(messenger) = &mut self.v6 {
            messengers.push(messenger);
        }
        if let Some(messenger) = &mut self.v4 {
            messengers.push(messenger);
        }

        Ok(messengers)
    }

    pub fn get_local_contact(&self) -> std::io::Result<Contact> {
        Ok(Contact {
            v6: self.local_addr_v6()?,
            v4: self.local_addr_v4()?,
        })
    }

    fn local_addr_v6(&self) -> std::io::Result<Option<SocketAddrV6>> {
        let Some(stream) = &self.v6 else {
            return Ok(None);
        };
        let V6(v6) = stream.local_addr()? else {
            unreachable!()
        };
        Ok(Some(v6))
    }

    fn local_addr_v4(&self) -> std::io::Result<Option<SocketAddrV4>> {
        let Some(stream) = &self.v4 else {
            return Ok(None);
        };
        let V4(v4) = stream.local_addr()? else {
            unreachable!()
        };
        Ok(Some(v4))
    }
}

fn configure_stream(stream: Stream) -> Messenger {
    let sock = SockRef::from(stream.get_ref().0);
    let _ = sock.set_reuse_address(true);
    let _ = sock.set_reuse_port(true);
    Messenger::with_capacity(stream, 68)
}
