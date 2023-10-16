use crate::{Contact, FullContact};
use rand::Rng;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Error, Debug)]
#[error("No room with this id exists.")]
pub struct NoSuchRoomId;

#[derive(Default)]
struct Client {
    contact: FullContact,
    waiting: Option<oneshot::Sender<(Contact, FullContact)>>,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room_id to clients
    rooms: Arc<Mutex<HashMap<u32, [Client; 2]>>>,
}

impl State {
    pub fn create_room(&mut self) -> u32 {
        let mut rooms = self.rooms.lock().unwrap();

        let mut rng = rand::thread_rng();
        let mut room_id = rng.gen_range(0..1_048_576);
        while rooms.contains_key(&room_id) {
            room_id = rng.gen_range(0..1_048_576);
        }

        rooms.insert(room_id, [Client::default(), Client::default()]);
        self.room_timeout(room_id);

        room_id
    }

    pub fn room_exists(&self, room_id: u32) -> bool {
        self.rooms.lock().unwrap().contains_key(&room_id)
    }

    pub fn update_client(
        &mut self,
        room_id: u32,
        is_creator: bool,
        endpoint: SocketAddr,
        public: bool,
    ) -> Result<(), NoSuchRoomId> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_id).ok_or(NoSuchRoomId)?;
        let contact = &mut room[usize::from(is_creator)].contact;

        let contact = if public {
            &mut contact.public
        } else {
            &mut contact.private
        };

        match endpoint {
            SocketAddr::V6(addr) => {
                contact.v6 = Some(addr);
            }
            SocketAddr::V4(addr) => {
                contact.v4 = Some(addr);
            }
        };

        Ok(())
    }

    pub fn set_client_done(
        &mut self,
        room_id: u32,
        is_creator: bool,
    ) -> Result<oneshot::Receiver<(Contact, FullContact)>, NoSuchRoomId> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_id).ok_or(NoSuchRoomId)?;

        let client_i = usize::from(is_creator);
        let peer_i = usize::from(!is_creator);

        let client = &mut room[client_i];

        let (tx, rx) = oneshot::channel();
        client.waiting = Some(tx);

        let peer = &room[peer_i];

        if peer.waiting.is_some() {
            let client_info = room[client_i].contact;
            let peer_info = peer.contact;

            let client = &mut room[client_i];
            client
                .waiting
                .take()
                .unwrap()
                .send((client_info.public, peer_info))
                .unwrap();

            let peer = &mut room[peer_i];
            peer.waiting
                .take()
                .unwrap()
                .send((peer_info.public, client_info))
                .unwrap();
            rooms.remove(&room_id);
        }

        Ok(rx)
    }

    fn room_timeout(&self, room_id: u32) {
        let state = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
            let mut rooms = state.rooms.lock().unwrap();
            rooms.remove(&room_id);
        });
    }
}
