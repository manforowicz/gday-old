use crate::protocol::FullContact;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Default)]
struct Client {
    contact: FullContact,
    waiting: Option<oneshot::Sender<FullContact>>,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room_id to clients
    rooms: Arc<Mutex<HashMap<[u8; 6], [Client; 2]>>>,
}

#[derive(Error, Debug)]
#[error("There is no room with this room_id")]
pub struct NoSuchRoomExists;

fn generate_room_id() -> [u8; 6] {
    let mut rng = rand::thread_rng();
    let characters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut id = [0; 6];
    for letter in &mut id {
        *letter = *characters.choose(&mut rng).unwrap();
    }

    id
}

impl State {
    pub fn create_room(&mut self) -> [u8; 6] {
        let mut rooms = self.rooms.lock().unwrap();

        let mut room_id = generate_room_id();
        while rooms.contains_key(&room_id) {
            room_id = generate_room_id();
        }

        rooms.insert(room_id, [Client::default(), Client::default()]);
        self.room_timeout(room_id);

        room_id
    }

    pub fn update_client(
        &mut self,
        room_id: [u8; 6],
        is_creator: bool,
        endpoint: SocketAddr,
        public: bool,
    ) -> Result<(), NoSuchRoomExists> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_id).ok_or(NoSuchRoomExists)?;
        let contact = &mut room[usize::from(is_creator)].contact;

        if public {
            match endpoint {
                SocketAddr::V6(addr) => {
                    contact.public_v6 = Some(addr);
                }
                SocketAddr::V4(addr) => {
                    contact.public_v4 = Some(addr);
                }
            }
        } else {
            match endpoint {
                SocketAddr::V6(addr) => {
                    contact.private_v6 = Some(addr);
                }
                SocketAddr::V4(addr) => {
                    contact.private_v4 = Some(addr);
                }
            }
        };

        Ok(())
    }

    /// Assumes that client id exists
    pub fn set_client_done(
        &mut self,
        room_id: [u8; 6],
        is_creator: bool,
    ) -> Result<oneshot::Receiver<FullContact>, NoSuchRoomExists> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_id).ok_or(NoSuchRoomExists)?;

        let client_i = usize::from(is_creator);
        let peer_i = usize::from(!is_creator);

        let client = &mut room[client_i];

        let (tx, rx) = oneshot::channel();
        client.waiting = Some(tx);

        let peer = &room[peer_i];

        if peer.waiting.is_some() {
            let client_info = room[client_i].contact.clone();
            let peer_info = peer.contact.clone();

            let client = &mut room[client_i];
            client.waiting.take().unwrap().send(peer_info).unwrap();

            let peer = &mut room[peer_i];
            peer.waiting.take().unwrap().send(client_info).unwrap();
        }

        Ok(rx)
    }

    fn room_timeout(&self, room: [u8; 6]) {
        let state = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
            let mut rooms = state.rooms.lock().unwrap();
            rooms.remove(&room);
        });
    }
}
