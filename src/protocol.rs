use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ClientMessage {
    /// Request the server to create a room
    CreateRoom,
    /// (password, user is creator of room?, private contact, done sending all info)
    SendContact([u8; 6], bool, Endpoint, bool),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum ServerMessage {
    /// Room successfully created
    /// (room_password, user_id)
    RoomCreated([u8; 6]),
    /// (full contact info of peer)
    SharePeerContacts(FullContact),
    SyntaxError,
    NoSuchRoomPasswordError,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum Endpoint {
    V6(u128, u16),
    V4(u32, u16)
}


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Default)]
pub struct Contact {
    pub v6: Option<(u128, u16)>,
    pub v4: Option<(u32, u16)>,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Default)]
pub struct FullContact {
    pub private: Contact,
    pub public: Contact,
}
