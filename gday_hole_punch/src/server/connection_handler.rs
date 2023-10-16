use std::net::SocketAddr;

use crate::server::global_state::State;
use crate::{ClientMessage, ServerMessage};
use crate::{Messenger, SerializationError};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

use super::ServerError;

pub struct ConnectionHandler {
    state: State,
    messenger: Messenger,
    room_id: u32,
    is_creator: bool,
}

impl ConnectionHandler {
    pub async fn start(mut state: State, stream: TlsStream<TcpStream>) -> Result<(), ServerError> {
        let mut messenger = Messenger::with_capacity(stream, 68);

        let (room_id, is_creator) = match messenger.next_msg().await {
            Ok(ClientMessage::CreateRoom) => {
                let room_id = state.create_room();
                messenger
                    .write_msg(ServerMessage::RoomCreated(room_id))
                    .await?;
                (room_id, true)
            }
            Ok(ClientMessage::JoinRoom(room_id)) => {
                if state.room_exists(room_id) {
                    messenger.write_msg(ServerMessage::RoomJoined).await?;
                } else {
                    messenger
                        .write_msg(ServerMessage::ErrorNoSuchRoomID)
                        .await?;
                    return Err(ServerError::NoSuchRoomId);
                }
                (room_id, false)
            }
            Ok(_msg) => {
                messenger.write_msg(ServerMessage::SyntaxError).await?;
                return Err(ServerError::ReceivedIncorrectMessage);
            }
            Err(err) => {
                messenger.write_msg(ServerMessage::SyntaxError).await?;
                return Err(err.into());
            }
        };

        let mut this = Self {
            state,
            messenger,
            room_id,
            is_creator,
        };

        loop {
            this.handle_message().await?;
        }
    }

    async fn handle_message(&mut self) -> Result<(), ServerError> {
        let msg: Result<_, _> = self.messenger.next_msg().await;

        match msg {
            Ok(ClientMessage::SendPrivateAddr(private_addr)) => {
                self.update_client(self.messenger.peer_addr()?, true)
                    .await?;

                if let Some(addr) = private_addr {
                    self.update_client(addr, false).await?;
                }
            }
            Ok(ClientMessage::DoneSending) => {
                if let Ok(rx) = self.state.set_client_done(self.room_id, self.is_creator) {
                    let Ok((local_public, peer)) = rx.await else {
                        return Err(ServerError::RoomTimedOut);
                    };
                    self.send(ServerMessage::SharePeerContacts {
                        client_public: local_public,
                        peer,
                    })
                    .await?;
                    return Ok(());
                } else {
                    self.send_no_such_room().await?;
                };
            }
            Ok(_msg) => {
                self.send(ServerMessage::SyntaxError).await?;
                return Err(ServerError::ReceivedIncorrectMessage);
            }
            Err(err) => {
                self.send(ServerMessage::SyntaxError).await?;
                return Err(err.into());
            }
        };

        Ok(())
    }

    async fn send(&mut self, msg: ServerMessage) -> Result<(), SerializationError> {
        self.messenger.write_msg(msg).await
    }

    async fn update_client(&mut self, addr: SocketAddr, public: bool) -> Result<(), ServerError> {
        if self
            .state
            .update_client(self.room_id, self.is_creator, addr, public)
            .is_err()
        {
            self.send_no_such_room().await
        } else {
            Ok(())
        }
    }

    async fn send_no_such_room(&mut self) -> Result<(), ServerError> {
        self.send(ServerMessage::ErrorNoSuchRoomID).await?;
        Err(ServerError::NoSuchRoomId)
    }
}
