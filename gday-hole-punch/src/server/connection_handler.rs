use crate::server::global_state::State;
use crate::{SerializationError, Messenger};
use crate::{ClientMessage, ServerMessage};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use super::ServerError;

pub struct ConnectionHandler {
    state: State,
    messenger: Messenger<TlsStream<TcpStream>>,
}

impl ConnectionHandler {
    pub async fn start(state: State, stream: TlsStream<TcpStream>) -> Result<(), ServerError> {
        let mut this = ConnectionHandler {
            state,
            messenger: Messenger::with_capacity(stream, 68),
        };
        loop {
            Self::handle_message(&mut this).await?;
        }
    }

    async fn handle_message(&mut self) -> Result<(), ServerError> {
        //let msg = deserialize_from(&mut self.stream, &mut self.tmp_buf).await;

        let msg: Result<_, _> = self.messenger.next_msg().await;

        match msg {
            Ok(ClientMessage::CreateRoom) => {
                let room_id = self.state.create_room();
                self.send(ServerMessage::RoomCreated(room_id)).await?;
            }
            Ok(ClientMessage::SendContact {
                room_id,
                is_creator,
                private_addr,
            }) => {
                if self
                    .state
                    .update_client(room_id, is_creator, self.messenger.inner_stream().get_ref().0.peer_addr()?, true)
                    .is_err()
                {
                    self.send(ServerMessage::ErrorNoSuchRoomID).await?;
                }

                if let Some(addr) = private_addr {
                    if self
                        .state
                        .update_client(room_id, is_creator, addr, false)
                        .is_err()
                    {
                        self.send(ServerMessage::ErrorNoSuchRoomID).await?;
                    };
                }
            }
            Ok(ClientMessage::DoneSending {
                room_id,
                is_creator,
            }) => {
                if let Ok(rx) = self.state.set_client_done(room_id, is_creator) {
                    let (local_public, peer) = rx.await.unwrap();
                    self.send(ServerMessage::SharePeerContacts {
                        client_public: local_public,
                        peer,
                    })
                    .await?;
                } else {
                    self.send(ServerMessage::ErrorNoSuchRoomID).await?;
                };
            }
            Err(err) => {
                self.send(ServerMessage::SyntaxError).await?;
                Err(err)?;
            }
        };

        Ok(())
    }

    async fn send(&mut self, msg: ServerMessage) -> Result<(), SerializationError> {
        self.messenger.write_msg(msg).await
    }
}
