use crate::FullContact;
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;

#[derive(Error, Debug)]
#[error("No room with this id exists.")]
pub struct NoSuchRoomId;

/// Information about a client in a [`Room`].
#[derive(Default)]
struct Client {
    /// The known private and public IP addresses of this client
    contact: FullContact,
    /// - `None` if the client is still sending their contact info
    /// - `Some` if the client is done sending their contact info.
    /// Once the peer is also done, this channel sends
    /// (this client's contact, peer's contact) to the connection thread.
    sender: Option<oneshot::Sender<(FullContact, FullContact)>>,
}

/// A room holds 2 [Client]s that want to exchange their contact info
#[derive(Default)]
struct Room {
    /// The client that created this room
    creator: Client,
    /// The client that joined this room
    joiner: Client,
}

impl Room {
    fn get_client(&mut self, is_creator: bool) -> &Client {
        if is_creator {
            &self.creator
        } else {
            &self.joiner
        }
    }

    fn get_client_mut(&mut self, is_creator: bool) -> &mut Client {
        if is_creator {
            &mut self.creator
        } else {
            &mut self.joiner
        }
    }
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room_id to clients
    rooms: Arc<Mutex<HashMap<u32, Room>>>,

    blocked: Arc<Mutex<HashSet<IpAddr>>>,
}

impl State {
    fn block(&mut self, addr: IpAddr) {
        let mut blocked = self.blocked.lock().unwrap();
        blocked.insert(addr);
    }

    pub fn create_room(&mut self) -> u32 {
        let mut rooms = self.rooms.lock().unwrap();

        let mut rng = rand::thread_rng();
        let mut room_id = rng.gen_range(0..1_048_576);
        while rooms.contains_key(&room_id) {
            room_id = rng.gen_range(0..1_048_576);
        }

        rooms.insert(room_id, Room::default());
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
        let contact = &mut room.get_client_mut(is_creator).contact;

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

    /// Returns a [`oneshot::Receiver`] that will send the other peer's contact info
    /// once that peer is also ready.
    pub fn set_client_done(
        &mut self,
        room_id: u32,
        is_creator: bool,
    ) -> Result<oneshot::Receiver<(FullContact, FullContact)>, NoSuchRoomId> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&room_id).ok_or(NoSuchRoomId)?;

        let (tx, rx) = oneshot::channel();

        room.get_client_mut(is_creator).sender = Some(tx);

        let client_info = room.get_client(is_creator).contact;
        let peer_info = room.get_client(!is_creator).contact;

        // if both peers are waiting for each others' contact info
        if let Some(client_sender) = room.get_client_mut(is_creator).sender.take() {
            if let Some(peer_sender) = room.get_client_mut(!is_creator).sender.take() {
                // exchange their info
                // don't care about error, since nothing critical happens
                // if the receiver has been dropped.
                let _ = client_sender.send((client_info, peer_info));
                let _ = peer_sender.send((peer_info, client_info));

                // remove their room
                rooms.remove(&room_id);
            }
        }

        Ok(rx)
    }

    /// Removes the room with `room_id` from `self.rooms` after 10 minutes.
    fn room_timeout(&self, room_id: u32) {
        let state_rooms = self.rooms.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(60 * 10)).await;
            let mut rooms = state_rooms.lock().unwrap();
            rooms.remove(&room_id);
        });
    }

    /// Removes the given IP address from the `self.blocked` after 1 second.
    fn blocked_timeout(&self, addr: IpAddr) {
        let state_blocked = self.blocked.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let mut blocked = state_blocked.lock().unwrap();
            blocked.remove(&addr);
        });
    }
}
