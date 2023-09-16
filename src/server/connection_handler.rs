use crate::server::global_state::State;
use crate::protocol::{deserialize_from, serialize_into, ClientMessage, ServerMessage};
use crate::Error;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;

pub struct ConnectionHandler {
    state: State,
    stream: TlsStream<TcpStream>,
    client_addr: SocketAddr,
    tmp_buf: [u8; 68]
}

impl ConnectionHandler {
    pub async fn start(state: State, stream: TlsStream<TcpStream>, client_addr: SocketAddr) {
        let mut this = ConnectionHandler {
            state,
            stream,
            client_addr,
            tmp_buf: [0; 68]
        };
        while Self::handle_message(&mut this).await.is_ok() {}
    }

    async fn handle_message(&mut self) -> Result<(), Error> {
        let msg = deserialize_from(&mut self.stream, &mut self.tmp_buf).await;

        match msg {
            Ok(ClientMessage::CreateRoom) => {
                let room_id = self.state.create_room();
                self.send(ServerMessage::RoomCreated(room_id)).await?;
            }
            Ok(ClientMessage::SendContact(room_id, is_creator, contact)) => {
                if self
                    .state
                    .update_client(room_id, is_creator, self.client_addr, true)
                    .is_err()
                {
                    self.send(ServerMessage::ErrorNoSuchRoomID).await?;
                }

                if let Some(contact) = contact {
                    if self
                        .state
                        .update_client(room_id, is_creator, contact, false)
                        .is_err()
                    {
                        self.send(ServerMessage::ErrorNoSuchRoomID).await?;
                    };
                }
            }
            Ok(ClientMessage::DoneSending(room_id, is_creator)) => {
                if let Ok(rx) = self.state.set_client_done(room_id, is_creator) {
                    let contact = rx.await.unwrap();
                    self.send(ServerMessage::SharePeerContacts(contact)).await?;
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

    async fn send(&mut self, msg: ServerMessage) -> Result<(), Error> {
        serialize_into(&mut self.stream, &msg, &mut self.tmp_buf).await
    }
}
