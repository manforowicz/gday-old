mod server_connection;

use super::{peer_connector::PeerConnector, ClientError, ServerAddr};
use crate::{
    deserialize_from, serialize_into, ClientMessage, FullContact, RoomId, ServerMessage,
};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use server_connection::ServerConnection;

pub struct ContactSharer {
    room_id: RoomId,
    is_creator: bool,
    connection: ServerConnection,
    tmp_buf: [u8; 68],
}

impl ContactSharer {
    pub async fn create_room(server_addr: ServerAddr) -> Result<(Self, RoomId), ClientError> {
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

    pub async fn join_room(server_addr: ServerAddr, room_id: RoomId) -> Result<Self, ClientError> {
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
    ) -> Result<RoomId, ClientError> {
        serialize_into(stream, &ClientMessage::CreateRoom, tmp_buf).await?;
        let response: ServerMessage = deserialize_from(stream, tmp_buf).await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok(room_id)
        } else {
            Err(ClientError::InvalidServerReply(response))
        }
    }

    /// TODO: ADD ERROR HANDLING FOR NO SERVER CONNECTIONS

    pub async fn get_peer_connector(mut self) -> Result<PeerConnector, ClientError> {
        let mut conns = self.connection.get_all_streams_with_sockets()?;

        for conn in &mut conns {
            let msg = ClientMessage::SendContact {
                room_id: self.room_id,
                is_creator: self.is_creator,
                private_addr: Some(conn.1),
            };
            serialize_into(conn.0, &msg, &mut self.tmp_buf).await?;
        }

        let msg = ClientMessage::DoneSending {
            room_id: self.room_id,
            is_creator: self.is_creator,
        };
        serialize_into(conns[0].0, &msg, &mut self.tmp_buf).await?;

        let response: ServerMessage = deserialize_from(conns[0].0, &mut self.tmp_buf).await?;

        if let ServerMessage::SharePeerContacts {
            client_public: local_public,
            peer,
        } = response
        {
            Ok(PeerConnector {
                local: FullContact {
                    private: self.connection.get_local_contact()?,
                    public: local_public,
                },
                peer,
                is_creator: self.is_creator,
            })
        } else {
            Err(ClientError::InvalidServerReply(response))
        }
    }
}
