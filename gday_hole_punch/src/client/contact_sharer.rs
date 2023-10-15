mod server_connection;

use super::{peer_connector::PeerConnector, ClientError};
use crate::{ClientMessage, FullContact, Messenger, RoomId, ServerMessage};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use server_connection::ServerConnection;

pub struct ContactSharer {
    room_id: RoomId,
    is_creator: bool,
    connection: ServerConnection,
}

type Stream = TlsStream<TcpStream>;

impl ContactSharer {
    pub async fn create_room(server_stream_v6: Option<Stream>, server_stream_v4: Option<Stream>) -> Result<(Self, RoomId), ClientError> {
        let mut connection = ServerConnection::new(server_stream_v6, server_stream_v4).await?;

        let room_id = Self::request_room(connection.get_any_messenger()).await?;

        Ok((
            Self {
                room_id,
                is_creator: true,
                connection,
            },
            room_id,
        ))
    }

    pub async fn join_room(server_stream_v6: Option<Stream>, server_stream_v4: Option<Stream>, room_id: RoomId) -> Result<Self, ClientError> {
        let connection = ServerConnection::new(server_stream_v6, server_stream_v4).await?;

        Ok(Self {
            room_id,
            is_creator: false,
            connection,
        })
    }

    async fn request_room(
        messenger: &mut Messenger,
    ) -> Result<RoomId, ClientError> {
        messenger.write_msg(ClientMessage::CreateRoom).await?;
        //serialize_into(messenger, &ClientMessage::CreateRoom, tmp_buf).await?;

        let response: ServerMessage = messenger.next_msg().await?;
        //let response: ServerMessage = deserialize_from(messenger, tmp_buf).await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok(room_id)
        } else {
            Err(ClientError::InvalidServerReply(response))
        }
    }

    /// TODO: ADD ERROR HANDLING FOR NO SERVER CONNECTIONS

    pub async fn get_peer_connector(mut self) -> Result<PeerConnector, ClientError> {
        let mut conns = self.connection.get_all_messengers()?;

        for conn in &mut conns {
            let msg = ClientMessage::SendContact {
                room_id: self.room_id,
                is_creator: self.is_creator,
                private_addr: Some(conn.local_addr()?),
            };
            conn.write_msg(msg).await?;
            //serialize_into(conn.0, &msg, &mut self.tmp_buf).await?;
        }

        let msg = ClientMessage::DoneSending {
            room_id: self.room_id,
            is_creator: self.is_creator,
        };
        conns[0].write_msg(msg).await?;
        //serialize_into(conns[0].0, &msg, &mut self.tmp_buf).await?;

        let response: ServerMessage = conns[0].next_msg().await?;
        //let response: ServerMessage = deserialize_from(conns[0].0, &mut self.tmp_buf).await?;

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
