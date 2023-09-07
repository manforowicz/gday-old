use std::error::Error;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::global_state::State;
use crate::enum_protocol::{ClientMessage, ServerMessage};

pub struct ConnectionHandler<T: AsyncReadExt + AsyncWriteExt + Unpin> {
    state: State,
    stream: T,
}

impl<T: AsyncReadExt + AsyncWriteExt + Unpin> ConnectionHandler<T> {
    pub async fn start(state: State, stream: T) {
        let mut this = ConnectionHandler { state, stream };
        while Self::handle_message(&mut this).await.is_ok() {}
    }


    async fn handle_message(&mut self) -> Result<(), Box<dyn Error>> {
        let msg_length = self.stream.read_u16().await? as usize;
        let mut msg = vec![0; msg_length - 1];
        self.stream.read_exact(&mut msg).await?;
        let msg = ClientMessage::try_from(&msg[..]);

        let response = match msg {
            Ok(ClientMessage::CreateRoom) => Some(ServerMessage::RoomCreated(self.state.create_room())),
            Ok(ClientMessage::JoinRoom(password)) => {
                if let Ok(id) = self.state.join_room(&password) {
                    Some(ServerMessage::RoomJoined(id))
                } else {
                    Some(ServerMessage::NoSuchRoomError)
                }
            },
            Ok(ClientMessage::SendContact(id, contact, done)) => {
                if self.state.update_client(id, &contact).is_err() {
                    Some(ServerMessage::NoSuchClientIDError)
                } else if done {
                    let contacts = self.state.set_client_done(id).await?;
                    Some(ServerMessage::SharePeerContacts(contacts))
                } else {
                    None
                }
            },
            Err(_) => {
                Some(ServerMessage::SyntaxError)
            }
        };

        if let Some(response) = response {
            self.stream.write_all(&Vec::from(response)).await?;
        }

        Ok(())
    }
}