

use std::net::SocketAddr;

use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::global_state::State;
use holepunch::protocol::{ClientMessage, ServerMessage};
use holepunch::{deserialize_from, serialize_into, endpoint_from_addr};

pub struct ConnectionHandler<T: AsyncReadExt + AsyncWriteExt + Unpin> {
    state: State,
    stream: T,
    client_addr: SocketAddr
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin> ConnectionHandler<T> {
    pub async fn start(state: State, stream: T, client_addr: SocketAddr) {
        let mut this = ConnectionHandler { state, stream, client_addr };
        while Self::handle_message(&mut this).await.is_ok() {}
    }

    async fn handle_message(&mut self) -> Result<(), Error> {
        let msg = deserialize_from(&mut self.stream).await;

        match msg {
            Ok(ClientMessage::CreateRoom) => {
                let password = self.state.create_room();
                self.send(ServerMessage::RoomCreated(password)).await?;
            }
            Ok(ClientMessage::SendContact(password, is_creator, contact, is_done)) => {
                if !self.state.room_exists(&password) {
                    self.send(ServerMessage::NoSuchRoomPasswordError).await?;
                    return Err(Error::NoSuchPassword);
                }

                self.state.update_client(&password, is_creator, &endpoint_from_addr(self.client_addr), true);
                self.state.update_client(&password, is_creator, &contact, false);
                    
                if is_done {
                    let contact = self.state.set_client_done(&password, is_creator).await.unwrap();
                    self.send(ServerMessage::SharePeerContacts(contact)).await?;
                }
            }
            Err(err) => {
                self.send(ServerMessage::SyntaxError).await?;
                Err(err)?
            }
        };

        Ok(())
    }

    async fn send(&mut self, msg: ServerMessage) -> Result<(), holepunch::Error> {
        serialize_into(&mut self.stream, &msg).await
    }
}


#[derive(Debug, Error)]
enum Error {
    #[error("Client sent password that doesn't correspond to any room")]
    NoSuchPassword,
    #[error("{0}")]
    ProtocolError(#[from] holepunch::Error)
}