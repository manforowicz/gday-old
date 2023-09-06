use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::oneshot;

struct Client {
    password: Vec<u8>,
    ready_channel: Option<oneshot::Sender<Vec<ContactInfo>>>,
    contact_info: ContactInfo,
}

#[derive(Default, Clone, Debug)]
pub struct ContactInfo {
    pub private_v6: Option<SocketAddrV6>,
    pub private_v4: Option<SocketAddrV4>,
    pub public_v6: Option<SocketAddrV6>,
    pub public_v4: Option<SocketAddrV4>,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room password to client IDs in that room
    rooms: Arc<Mutex<HashMap<Vec<u8>, Vec<u64>>>>,
    /// Maps client ID to client
    clients: Arc<Mutex<HashMap<u64, Client>>>,

    /// Total number of clients served
    clients_served: Arc<Mutex<u64>>,
}

impl State {
    pub fn room_exists(&self, password: &[u8]) -> bool {
        let rooms = self.rooms.lock().unwrap();
        rooms.contains_key(password)
    }

    pub fn id_exists(&self, id: u64) -> bool {
        let clients = self.clients.lock().unwrap();
        clients.contains_key(&id)
    }

    /// Adds a client and returns their new id
    pub fn add_client(&mut self, password: &[u8]) -> u64 {
        let mut rooms = self.rooms.lock().unwrap();
        let mut clients = self.clients.lock().unwrap();
        let mut clients_served = self.clients_served.lock().unwrap();

        let client = Client {
            password: password.to_vec(),
            ready_channel: None,
            contact_info: ContactInfo::default(),
        };

        *clients_served = clients_served.wrapping_add(1);

        let id = *clients_served;
        if rooms.contains_key(password) {
            rooms.get_mut(password).unwrap().push(id);
        } else {
            rooms.insert(password.to_vec(), vec![id]);
        }

        clients.insert(id, client);

        id
    }

    pub fn update_client(&mut self, client_id: u64, addr: SocketAddr, public: bool) {
        let mut clients = self.clients.lock().unwrap();
        let client = clients.get_mut(&client_id).unwrap();

        match addr {
            SocketAddr::V6(addr) => {
                if public {
                    client.contact_info.public_v6 = Some(addr);
                } else {
                    client.contact_info.private_v6 = Some(addr);
                }
            }
            SocketAddr::V4(addr) => {
                if public {
                    client.contact_info.public_v4 = Some(addr);
                } else {
                    client.contact_info.private_v4 = Some(addr);
                }
            }
        }
    }

    pub fn set_client_done(&mut self, client_id: u64) -> oneshot::Receiver<Vec<ContactInfo>> {
        let mut clients = self.clients.lock().unwrap();

        let rooms = self.rooms.lock().unwrap();

        let client = clients.get_mut(&client_id).unwrap();

        let (tx, rx) = oneshot::channel();

        client.ready_channel = Some(tx);

        let client_ids = &rooms[&client.password];

        //let clients: Vec<&mut Client> = client_ids.iter().map(|id| (&mut clients).get_mut(id).unwrap()).collect();

        if client_ids
            .iter()
            .all(|id| clients[id].ready_channel.is_some())
        {
            for current_id in client_ids {
                let contacts: Vec<ContactInfo> = client_ids
                    .iter()
                    .filter(|id| *id != current_id)
                    .map(|id| clients[id].contact_info.clone())
                    .collect();
                clients
                    .get_mut(current_id)
                    .unwrap()
                    .ready_channel
                    .take()
                    .unwrap()
                    .send(contacts)
                    .unwrap();
            }
        }

        rx
    }
}
