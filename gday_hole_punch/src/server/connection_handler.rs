use crate::server::global_state::State;
use crate::{ClientMessage, ServerMessage};
use crate::{Messenger, SerializationError};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use super::ServerError;

pub struct ConnectionHandler {
    state: State,
    messenger: Messenger,
}

impl ConnectionHandler {
    pub async fn start(state: State, stream: TlsStream<TcpStream>) -> Result<(), ServerError> {
        let mut this = ConnectionHandler {
            state,
            messenger: Messenger::with_capacity(stream, 68),
        };

        loop {
            if let Err(err) = Self::handle_message(&mut this).await {
                println!("stargin sleep;");
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                println!("ended sleep");
                return Err(err);
            }
        }
    }

    async fn handle_message(&mut self) -> Result<(), ServerError> {
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
                    .update_client(room_id, is_creator, self.messenger.peer_addr()?, true)
                    .is_err()
                {
                    self.send_no_such_room().await?;
                }

                if let Some(addr) = private_addr {
                    if self
                        .state
                        .update_client(room_id, is_creator, addr, false)
                        .is_err()
                    {
                        self.send_no_such_room().await?;
                    };
                }
            }
            Ok(ClientMessage::DoneSending {
                room_id,
                is_creator,
            }) => {
                if let Ok(rx) = self.state.set_client_done(room_id, is_creator) {
                    let Ok((local_public, peer)) = rx.await else {
                        return Err(ServerError::RoomTimedOut);
                    };
                    self.send(ServerMessage::SharePeerContacts {
                        client_public: local_public,
                        peer,
                    })
                    .await?;
                } else {
                    self.send_no_such_room().await?;
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

    async fn send_no_such_room(&mut self) -> Result<(), ServerError> {
        self.send(ServerMessage::ErrorNoSuchRoomID).await?;
        Err(ServerError::NoSuchRoomId)
    }
}
