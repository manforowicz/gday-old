use std::{
    error::Error,
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6}, fmt,
};

#[derive(PartialEq, Debug, Clone)]
pub enum ClientMessage {
    /// Code 1
    CreateRoom,
    /// Code 3
    JoinRoom([u8; 9]),
    /// Code 5
    SendContact(u64, Contact, bool),
}

impl From<ClientMessage> for Vec<u8> {
    fn from(msg: ClientMessage) -> Self {
        let mut bytes = Vec::new();

        match msg {
            ClientMessage::CreateRoom => {
                let length = 3u16.to_be_bytes();
                let msg_type = 1;
                bytes.extend(length);
                bytes.push(msg_type);
            }
            ClientMessage::JoinRoom(password) => {
                let length = 12u16.to_be_bytes();
                let msg_type = 3;
                bytes.extend(length);
                bytes.push(msg_type);
                bytes.extend(password);
            }
            ClientMessage::SendContact(id, contact, done) => {
                let mut length: u16 = 11;
                if contact.v6.is_some() {
                    length += 19;
                }
                if contact.v4.is_some() {
                    length += 7;
                }
                if done {
                    length += 1;
                }
                let length = length.to_be_bytes();
                let msg_type = 5;
                bytes.extend(id.to_be_bytes());
                bytes.extend(length);
                bytes.push(msg_type);

                if let Some(addr) = contact.v6 {
                    bytes.extend(addr_v6_to_msg(addr));
                }
                if let Some(addr) = contact.v4 {
                    bytes.extend(addr_v4_to_msg(addr));
                }
                if done {
                    bytes.push(5);
                }
            }
        }
        bytes
    }
}

impl TryFrom<&[u8]> for ClientMessage {
    type Error = SyntaxError;

    fn try_from(mut bytes: &[u8]) -> Result<Self, Self::Error> {
        let msg_type = bytes.first().ok_or(SyntaxError)?;
        bytes = bytes.get(1..).ok_or(SyntaxError)?;
        match msg_type {
            1 => {
                if !bytes.is_empty() {
                    return Err(SyntaxError);
                }
                Ok(ClientMessage::CreateRoom)
            }
            3 => Ok(ClientMessage::JoinRoom(bytes.try_into()?)),
            5 => {
                let mut done = false;
                let id = u64::from_be_bytes(bytes.get(0..8).ok_or(SyntaxError)?.try_into()?);
                bytes = &bytes[4..];

                let contact = contact_from_bytes(&mut bytes)?;

                if let Some(flag) = bytes.first() {
                    if *flag == 5 {
                        done = true;
                    } else {
                        return Err(SyntaxError);
                    }
                }

                Ok(ClientMessage::SendContact(id, contact, done))
            }
            _ => Err(SyntaxError),
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum ServerMessage {
    /// Code 2
    RoomCreated([u8; 9]),
    /// Code 4
    RoomJoined(u64),
    /// Code 6
    SharePeerContacts(Vec<FullContact>),
    /// Code 7
    SyntaxError,
    /// Code 8
    NoSuchRoomError,
    /// Code 9
    NoSuchClientIDError,
}

impl From<ServerMessage> for Vec<u8> {
    fn from(msg: ServerMessage) -> Self {
        let mut bytes = Vec::new();

        match msg {
            ServerMessage::RoomCreated(password) => {
                let length = 12u16.to_be_bytes();
                let msg_type = 2;
                bytes.extend(length);
                bytes.push(msg_type);
                bytes.extend(password);
            }
            ServerMessage::RoomJoined(id) => {
                let length = 11u16.to_be_bytes();
                let msg_type = 4;
                bytes.extend(length);
                bytes.push(msg_type);
                bytes.extend(id.to_be_bytes());
            }
            ServerMessage::SharePeerContacts(contacts) => {
                let msg_type = 6;
                bytes.push(msg_type);
                for contact in contacts {
                    bytes.push(1);
                    bytes.push(2);
                    if let Some(addr) = contact.private.v6 {
                        bytes.extend(addr_v6_to_msg(addr));
                    }
                    if let Some(addr) = contact.private.v4 {
                        bytes.extend(addr_v4_to_msg(addr));
                    }
                    bytes.push(3);
                    if let Some(addr) = contact.public.v6 {
                        bytes.extend(addr_v6_to_msg(addr));
                    }
                    if let Some(addr) = contact.public.v4 {
                        bytes.extend(addr_v4_to_msg(addr));
                    }
                }
                let length = u16::try_from(bytes.len()).unwrap().to_be_bytes();
                bytes.splice(0..0, length);
            }
            ServerMessage::SyntaxError => {
                let length = 3u16.to_be_bytes();
                let msg_type = 7;
                bytes.extend(length);
                bytes.push(msg_type);
            }
            ServerMessage::NoSuchRoomError => {
                let length = 3u16.to_be_bytes();
                let msg_type = 8;
                bytes.extend(length);
                bytes.push(msg_type);
            }
            ServerMessage::NoSuchClientIDError => {
                let length = 3u16.to_be_bytes();
                let msg_type = 9;
                bytes.extend(length);
                bytes.push(msg_type);
            }
        }
        bytes
    }
}

impl TryFrom<&[u8]> for ServerMessage {
    type Error = SyntaxError;

    fn try_from(mut bytes: &[u8]) -> Result<Self, Self::Error> {
        let msg_type = bytes.first().ok_or(SyntaxError)?;
        bytes = bytes.get(1..).ok_or(SyntaxError)?;
        match msg_type {
            2 => Ok(ServerMessage::RoomCreated(bytes.try_into()?)),
            4 => Ok(ServerMessage::RoomJoined(u64::from_be_bytes(
                bytes.try_into()?,
            ))),
            6 => {
                let mut contacts = Vec::new();
                while let Some(flag) = bytes.first() {
                    if *flag != 1 {
                        return Err(SyntaxError);
                    }
                    bytes = &bytes[1..];

                    let mut full_contact = FullContact::default();

                    if let Some(2) = bytes.first() {
                        bytes = &bytes[1..];
                        full_contact.private = contact_from_bytes(&mut bytes)?;
                    }
                    if let Some(3) = bytes.first() {
                        bytes = &bytes[1..];
                        full_contact.private = contact_from_bytes(&mut bytes)?;
                    }
                    contacts.push(full_contact);
                }

                Ok(ServerMessage::SharePeerContacts(contacts))
            }

            _ => Err(SyntaxError),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Contact {
    pub v6: Option<SocketAddrV6>,
    pub v4: Option<SocketAddrV4>,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct FullContact {
    pub private: Contact,
    pub public: Contact,
}

#[derive(Debug)]
pub struct SyntaxError;

impl Error for SyntaxError {}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Couldn't parse message from bytes")
    }
}


impl From<std::array::TryFromSliceError> for SyntaxError {
    fn from(_: std::array::TryFromSliceError) -> Self {
        Self
    }
}


fn contact_from_bytes(bytes: &mut &[u8]) -> Result<Contact, SyntaxError> {
    let mut contact = Contact::default();

    while let Some(flag) = bytes.first() {
        match flag {
            6 => {
                contact.v6 = Some(msg_to_addr_v6(
                    bytes.get(1..19).ok_or(SyntaxError)?.try_into().unwrap(),
                ));
                *bytes = &bytes[19..];
            }
            4 => {
                contact.v4 = Some(msg_to_addr_v4(
                    bytes.get(1..7).ok_or(SyntaxError)?.try_into().unwrap(),
                ));
                *bytes = &bytes[7..];
            }
            _ => (),
        }
    }
    Ok(contact)
}

fn addr_v6_to_msg(addr: SocketAddrV6) -> [u8; 19] {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: [u8; 19] = [0; 19];
    msg[0] = 6;
    let (left, right) = msg[1..].split_at_mut(16);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}

fn addr_v4_to_msg(addr: SocketAddrV4) -> [u8; 7] {
    let ip = addr.ip().octets();
    let port = addr.port().to_be_bytes();

    let mut msg: [u8; 7] = [0; 7];
    msg[0] = 4;
    let (left, right) = msg[1..].split_at_mut(4);
    left.copy_from_slice(&ip);
    right.copy_from_slice(&port);
    msg
}

macro_rules! bytes_to_int {
    ($bytes:expr, $type:ty) => {{
        <$type>::from_be_bytes($bytes.try_into().unwrap())
    }};
}

fn msg_to_addr_v6(msg: &[u8; 18]) -> SocketAddrV6 {
    let ip = bytes_to_int!(msg[0..16], u128);
    let port = bytes_to_int!(msg[16..18], u16);
    SocketAddrV6::new(Ipv6Addr::from(ip), port, 0, 0)
}

fn msg_to_addr_v4(msg: &[u8; 16]) -> SocketAddrV4 {
    let ip = bytes_to_int!(msg[0..4], u32);
    let port = bytes_to_int!(msg[4..6], u16);
    SocketAddrV4::new(Ipv4Addr::from(ip), port)
}


#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::net::SocketAddrV4;

    use crate::protocol::FullContact;
    use crate::protocol::ServerMessage;

    use super::Contact;

    use super::ClientMessage;

    #[test]
    fn client_message_create_room() {
        let msg1 = ClientMessage::CreateRoom;
        let msg2 = ClientMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn client_message_join_room() {
        let msg1 = ClientMessage::JoinRoom([1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let msg2 = ClientMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn client_message_send_contact() {
        let contact = Contact {
            v4: Some(SocketAddrV4::new(Ipv4Addr::new(4, 4, 3, 23), 4353)),
            v6: None,
        };
        let msg1 = ClientMessage::SendContact(3546, contact, true);
        let msg2 = ClientMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn server_message_room_created() {
        let msg1 = ServerMessage::RoomCreated([1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let msg2 = ServerMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn server_message_room_joined() {
        let msg1 = ServerMessage::RoomJoined(3546);
        let msg2 = ServerMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn server_message_share_peer_contacts() {
        let c1 = FullContact{
            private: Contact {
                v4: Some("3.24.24.25:324".parse().unwrap()),
                v6: None,
            },
            public: Contact {
                v4: None,
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
        };

        let c2 = FullContact {
            private: Contact {
                v6: None,
                v4: None,
            },
            public: Contact {
                v6: None,
                v4: None,
            }
        };

        let msg1 = ServerMessage::SharePeerContacts(vec![c1, c2]);
        let msg2 = ServerMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
        assert_eq!(msg1, msg2);
    }


    #[test]
    fn server_message_errors() {

        let errors = [ServerMessage::NoSuchClientIDError, ServerMessage::NoSuchRoomError, ServerMessage::SyntaxError];
        for msg1 in errors {
            let msg2 = ServerMessage::try_from(&Vec::from(msg1.clone())[2..]).unwrap();
            assert_eq!(msg1, msg2);
        }

    }

}