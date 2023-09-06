use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;


struct HalfContact {
    v6: Option<SocketAddrV6>,
    v4: Option<SocketAddrV4>
}

pub struct Contact {
    private: HalfContact,
    public: HalfContact,
}

pub fn encode_contact(contact: Contact) -> Vec<u8> {
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

pub fn decode_contact(msg: &[u8]) {
    
}


pub async fn receive<T: AsyncReadExt + Unpin>(stream: &mut T) -> Result<(u8, Vec<u8>)> {
    let msg_length = stream.read_u16().await?;
    let msg_type = stream.read_u8().await?;

    if msg_length < 4 {
        //
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

fn msg_to_addr_v6(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..16], u128);
    let port = bytes_to_int!(msg[16..18], u16);
    SocketAddr::new(IpAddr::V6(Ipv6Addr::from(ip)), port)
}

fn msg_to_addr_v4(msg: &[u8]) -> SocketAddr {
    let ip = bytes_to_int!(msg[0..4], u32);
    let port = bytes_to_int!(msg[4..6], u16);
    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), port)
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
