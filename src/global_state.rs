use rand::Rng;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::oneshot;

use crate::enum_protocol::{Contact, FullContact};

struct Client {
    password: [u8; 9],
    waiting: Option<oneshot::Sender<Vec<FullContact>>>,
    contact_info: FullContact,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room password to client IDs in that room
    rooms: Arc<Mutex<HashMap<[u8; 9], Vec<u64>>>>,
    /// Maps client ID to client
    clients: Arc<Mutex<HashMap<u64, Client>>>,
}

fn generate_password() -> [u8; 9] {
    let mut rng = rand::thread_rng();
    let mut password = [0; 9];
    for letter in &mut password {
        *letter = rng.gen_range(b'a'..=b'z');
    }
    password
}

impl State {
    pub fn create_room(&mut self) -> [u8; 9] {
        let mut rooms = self.rooms.lock().unwrap();

        let mut password = generate_password();
        while rooms.contains_key(&password) {
            password = generate_password();
        }

        rooms.insert(password, Vec::new());

        password
    }

    /// Adds a client and returns their new id
    pub fn join_room(&mut self, password: &[u8; 9]) -> Result<u64, NoSuchPasswordOrId> {
        let mut rooms = self.rooms.lock().unwrap();
        let mut clients = self.clients.lock().unwrap();

        let room = rooms.get_mut(password).ok_or(NoSuchPasswordOrId)?;
        let mut id = rand::random();
        while clients.contains_key(&id) {
            id = rand::random();
        }

        let client = Client {
            password: *password,
            waiting: None,
            contact_info: FullContact::default(),
        };

        clients.insert(id, client);
        room.push(id);

        Ok(id)
    }

    pub fn update_client(
        &mut self,
        client_id: u64,
        contact: &Contact,
    ) -> Result<(), NoSuchPasswordOrId> {
        let mut clients = self.clients.lock().unwrap();
        let client = clients.get_mut(&client_id).ok_or(NoSuchPasswordOrId)?;

        if let Some(v6) = contact.v6 {
            client.contact_info.private.v6 = Some(v6);
        }
        if let Some(v4) = contact.v4 {
            client.contact_info.private.v4 = Some(v4);
        }

        Ok(())
    }

    /// Assumes that client id exists
    pub fn set_client_done(&mut self, client_id: u64) -> oneshot::Receiver<Vec<FullContact>> {
        let mut clients = self.clients.lock().unwrap();
        let rooms = self.rooms.lock().unwrap();

        let client = clients.get_mut(&client_id).unwrap();

        let (tx, rx) = oneshot::channel();

        client.waiting = Some(tx);

        let client_ids = &rooms[&client.password];

        if client_ids.iter().all(|id| clients[id].waiting.is_some()) {
            for current_id in client_ids {
                let contacts: Vec<FullContact> = client_ids
                    .iter()
                    .filter(|id| *id != current_id)
                    .map(|id| clients[id].contact_info.clone())
                    .collect();
                clients
                    .get_mut(current_id)
                    .unwrap()
                    .waiting
                    .take()
                    .unwrap()
                    .send(contacts)
                    .unwrap();
            }
        }

        rx
    }
}

#[derive(Debug)]
pub struct NoSuchPasswordOrId;

impl Error for NoSuchPasswordOrId {}

impl fmt::Display for NoSuchPasswordOrId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No such password or id")
    }
}
