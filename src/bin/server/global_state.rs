use holepunch::FullContact;
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

#[derive(Default)]
struct Client {
    contact: FullContact,
    waiting: Option<oneshot::Sender<FullContact>>,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room password to clients
    rooms: Arc<Mutex<HashMap<[u8; 9], [Client; 2]>>>,
}

fn generate_password() -> [u8; 9] {
    let mut rng = rand::thread_rng();
    let characters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut password = [0; 9];
    for letter in &mut password {
        *letter = *characters.choose(&mut rng).unwrap();
    }

    password
}

impl State {
    pub fn room_exists(&self, password: [u8; 9]) -> bool {
        let rooms = self.rooms.lock().unwrap();
        rooms.contains_key(&password)
    }

    pub fn create_room(&mut self) -> [u8; 9] {
        let mut rooms = self.rooms.lock().unwrap();

        let mut password = generate_password();
        while rooms.contains_key(&password) {
            password = generate_password();
        }

        rooms.insert(password, [Client::default(), Client::default()]);

        password
    }

    pub fn update_client(
        &mut self,
        password: [u8; 9],
        is_creator: bool,
        endpoint: SocketAddr,
        public: bool,
    ) {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&password).unwrap();
        let client = &mut room[usize::from(is_creator)];

        let client_info = if public {
            &mut client.contact.public
        } else {
            &mut client.contact.private
        };

        match endpoint {
            SocketAddr::V6(addr) => {
                client_info.v6 = Some(addr);
            }
            SocketAddr::V4(addr) => {
                client_info.v4 = Some(addr);
            }
        }
    }

    /// Assumes that client id exists
    pub fn set_client_done(
        &mut self,
        password: [u8; 9],
        is_creator: bool,
    ) -> oneshot::Receiver<FullContact> {
        let mut rooms = self.rooms.lock().unwrap();
        let room = rooms.get_mut(&password).unwrap();

        let client_i = usize::from(is_creator);
        let peer_i = !client_i;

        let client = &mut room[client_i];

        let (tx, rx) = oneshot::channel();
        client.waiting = Some(tx);

        let peer = &room[!usize::from(is_creator)];

        if peer.waiting.is_some() {
            let client_info = room[client_i].contact.clone();
            let peer_info = peer.contact.clone();

            let client = &mut room[client_i];
            client.waiting.take().unwrap().send(peer_info).unwrap();

            let peer = &mut room[peer_i];
            peer.waiting.take().unwrap().send(client_info).unwrap();
        }

        rx
    }
}
