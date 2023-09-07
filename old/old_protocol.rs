use std::{
    fmt,
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6},
};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
pub struct ClientSyntaxError;

impl std::error::Error for ClientSyntaxError {}

impl fmt::Display for ClientSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Received message of invalid syntax from client")
    }
}

#[derive(Default)]
pub struct HalfContact {
    v6: Option<SocketAddrV6>,
    v4: Option<SocketAddrV4>,
}

#[derive(Default)]
pub struct Contact {
    private: HalfContact,
    public: HalfContact,
}

pub fn encode_contact(contact: &Contact) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(6);
    if let Some(addr) = contact.private.v6 {
        msg.push(1);
        msg.extend(addr_v6_to_msg(addr));
    }
    if let Some(addr) = contact.private.v4 {
        msg.push(2);
        msg.extend(addr_v4_to_msg(addr));
    }
    if let Some(addr) = contact.public.v6 {
        msg.push(3);
        msg.extend(addr_v6_to_msg(addr));
    }
    if let Some(addr) = contact.public.v4 {
        msg.push(4);
        msg.extend(addr_v4_to_msg(addr));
    }

    msg
}

pub fn encode_private_contact(contact: &HalfContact) -> Vec<u8> {
    let mut msg = Vec::new();
    if let Some(addr) = contact.v6 {
        msg.push(1);
        msg.extend(addr_v6_to_msg(addr));
    }
    if let Some(addr) = contact.v4 {
        msg.push(2);
        msg.extend(addr_v4_to_msg(addr));
    }
    msg
}

pub fn decode_contact(mut msg: &[u8]) -> Result<Contact> {
    let mut contact = Contact::default();
    while let Some(flag) = msg.first() {
        match flag {
            1 => {
                contact.private.v6 = Some(msg_to_addr_v6(msg.get(0..18).ok_or(ClientSyntaxError)?));
                msg = &msg[18..];
            }
            2 => {
                contact.private.v4 = Some(msg_to_addr_v4(msg.get(0..6).ok_or(ClientSyntaxError)?));
                msg = &msg[6..];
            }
            3 => {
                contact.public.v6 = Some(msg_to_addr_v6(msg.get(0..18).ok_or(ClientSyntaxError)?));
                msg = &msg[18..];
            }
            4 => {
                contact.public.v4 = Some(msg_to_addr_v4(msg.get(0..6).ok_or(ClientSyntaxError)?));
                msg = &msg[6..];
            }
            _ => Err(ClientSyntaxError)?,
        }
    }

    Ok(contact)
}

pub fn decode_private_contact(mut msg: &[u8]) -> Result<HalfContact> {
    let mut half = HalfContact::default();

    while let Some(flag) = msg.first() {
        match flag {
            1 => {
                half.v6 = Some(msg_to_addr_v6(msg.get(0..18).ok_or(ClientSyntaxError)?));
                msg = &msg[18..];
            }
            2 => {
                half.v4 = Some(msg_to_addr_v4(msg.get(0..6).ok_or(ClientSyntaxError)?));
                msg = &msg[6..];
            }
            _ => Err(ClientSyntaxError)?,
        }
    }

    Ok(half)
}

pub async fn receive<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<(u8, Vec<u8>)> {
    let msg_length = stream.read_u16().await?;
    let msg_type = stream.read_u8().await?;

    if msg_length < 4 {
        Err(ClientSyntaxError)?;
    }

    let mut msg = vec![0; msg_length as usize - 2];
    stream.read_exact(&mut msg).await?;
    Ok((msg_type, msg))
}

pub async fn send<T: AsyncWriteExt + Unpin>(stream: &mut T, code: u8, msg: &[u8]) -> Result<()> {
    let len = &msg.len().to_be_bytes()[0..2];
    stream.write_all(&[len, &[code], msg].concat()).await?;
    Ok(())
}

macro_rules! bytes_to_int {
    ($bytes:expr, $type:ty) => {{
        <$type>::from_be_bytes($bytes.try_into().unwrap())
    }};
}

fn msg_to_addr_v6(msg: &[u8]) -> SocketAddrV6 {
    let ip = bytes_to_int!(msg[0..16], u128);
    let port = bytes_to_int!(msg[16..18], u16);
    SocketAddrV6::new(Ipv6Addr::from(ip), port, 0, 0)
}

fn msg_to_addr_v4(msg: &[u8]) -> SocketAddrV4 {
    let ip = bytes_to_int!(msg[0..4], u32);
    let port = bytes_to_int!(msg[4..6], u16);
    SocketAddrV4::new(Ipv4Addr::from(ip), port)
}

fn addr_v6_to_msg(addr: SocketAddrV6) -> [u8; 18] {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: [u8; 18] = [0; 18];
    let (left, right) = msg.split_at_mut(16);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}

fn addr_v4_to_msg(addr: SocketAddrV4) -> [u8; 6] {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: [u8; 6] = [0; 6];
    let (left, right) = msg.split_at_mut(4);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}
