mod server_connection;

use super::{peer_connector::PeerConnector, ClientError};
use crate::{ClientMessage, FullContact, ServerMessage};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use server_connection::ServerConnection;

pub struct ContactSharer {
    is_creator: bool,
    connection: ServerConnection,
}

type Stream = TlsStream<TcpStream>;

impl ContactSharer {
    pub async fn create_room(
        server_stream_v6: Option<Stream>,
        server_stream_v4: Option<Stream>,
    ) -> Result<(Self, u32), ClientError> {
        let mut connection = ServerConnection::new(server_stream_v6, server_stream_v4).await?;

        let messenger = connection.get_any_messenger();
        messenger.write_msg(ClientMessage::CreateRoom).await?;
        let response = messenger.next_msg().await?;

        if let ServerMessage::RoomCreated(room_id) = response {
            Ok((
                Self {
                    is_creator: true,
                    connection,
                },
                room_id,
            ))
        } else {
            Err(ClientError::InvalidServerReply)
        }
    }

    pub async fn join_room(
        server_stream_v6: Option<Stream>,
        server_stream_v4: Option<Stream>,
        room_id: u32,
    ) -> Result<Self, ClientError> {
        let mut connection = ServerConnection::new(server_stream_v6, server_stream_v4).await?;

        let messenger = connection.get_any_messenger();
        messenger
            .write_msg(ClientMessage::JoinRoom(room_id))
            .await?;
        let response = messenger.next_msg().await?;

        if ServerMessage::RoomJoined == response {
            Ok(Self {
                is_creator: false,
                connection,
            })
        } else {
            Err(ClientError::InvalidServerReply)
        }
    }

    pub async fn get_peer_connector(mut self) -> Result<PeerConnector, ClientError> {
        let mut conns = self.connection.get_all_messengers()?;

        for conn in &mut conns {
            let msg = ClientMessage::SendPrivateAddr(Some(conn.local_addr()?));
            conn.write_msg(msg).await?;
        }

        conns[0].write_msg(ClientMessage::DoneSending).await?;

        let response: ServerMessage = conns[0].next_msg().await?;

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
            Err(ClientError::InvalidServerReply)
        }
    }
}
