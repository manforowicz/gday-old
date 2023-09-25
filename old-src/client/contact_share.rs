use crate::client::server_connection::{ServerAddr, ServerConnection};
use crate::protocol::{
    deserialize_from, serialize_into, ClientMessage, FullContact, RoomId, ServerMessage, Contact,
};
use crate::Error;
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

pub struct PeerConnection {
    pub stream: TcpStream,
    pub shared_secret: [u8; 32],
}
pub struct ContactSharer {
    room_id: RoomId,
    is_creator: bool,
    connection: ServerConnection,
    tmp_buf: [u8; 68],
}

impl ContactSharer {
    pub async fn create_room(server_addr: ServerAddr) -> Result<(Self, RoomId), Error> {
        let mut connection = ServerConnection::new(server_addr).await?;

        let mut tmp_buf = [0; 68];

        let room_id = Self::request_room(connection.get_any_stream(), &mut tmp_buf).await?;

        Ok((
            Self {
                room_id,
                is_creator: true,
                connection,
                tmp_buf,
            },
            room_id,
        ))
    }

    pub async fn join_room(server_addr: ServerAddr, room_id: RoomId) -> Result<Self, Error> {
        let connection = ServerConnection::new(server_addr).await?;

        Ok(Self {
            room_id,
            is_creator: false,
            connection,
            tmp_buf: [0; 68],
        })
    }

    async fn request_room(
        stream: &mut TlsStream<TcpStream>,
        tmp_buf: &mut [u8],
    ) -> Result<RoomId, Error> {
        serialize_into(stream, &ClientMessage::CreateRoom, tmp_buf).await?;
        let response: ServerMessage = deserialize_from(stream, tmp_buf).await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok(room_id)
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }

    /// TODO: ADD ERROR HANDLING FOR NO SERVER CONNECTIONS

    pub async fn get_peer_contact(mut self) -> Result<(FullContact, Contact), Error> {
        let mut conns = self.connection.get_all_streams_with_sockets();

        for conn in &mut conns {
            let msg = ClientMessage::SendContact(self.room_id, self.is_creator, Some(conn.1));
            serialize_into(conn.0, &msg, &mut self.tmp_buf).await?;
        }

        let msg = ClientMessage::DoneSending(self.room_id, self.is_creator);
        serialize_into(conns[0].0, &msg, &mut self.tmp_buf).await?;

        let response: ServerMessage =
            deserialize_from(conns[0].0, &mut self.tmp_buf).await?;

        

        if let ServerMessage::SharePeerContacts(full_contact) = response {
            Ok((full_contact, self.connection.get_my_contact()))
        } else {
            Err(Error::InvalidServerReply(response))
        }
    }
}
