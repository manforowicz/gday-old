use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use std::sync::Mutex;

#[derive(PartialEq)]
enum ClientStatus {
    Sending,
    Waiting(fn(Vec<ContactInfo>)),
}

struct Client {
    password: Vec<u8>,
    status: ClientStatus,
    contact_info: ContactInfo,
}

#[derive(Default)]
pub struct ContactInfo {
    private_v6: Option<SocketAddrV6>,
    private_v4: Option<SocketAddrV4>,
    public_v6: Option<SocketAddrV6>,
    public_v4: Option<SocketAddrV4>,
}

#[derive(Clone, Default)]
pub struct State {
    /// Maps room password to client IDs in that room
    rooms: Arc<Mutex<HashMap<Vec<u8>, Vec<u64>>>>,
    /// Maps client ID to client
    clients: Arc<Mutex<HashMap<u64, Client>>>,
    clients_served: Arc<Mutex<u64>>,
}

impl State {
    pub fn room_exists(&self, password: &[u8]) -> bool {
        let rooms = self.rooms.lock().unwrap();
        rooms.contains_key(password)
    }

    /// Adds a client and returns their new id
    pub fn add_client(&mut self, password: &[u8]) -> u64 {
        let mut rooms = self.rooms.lock().unwrap();
        let mut clients = self.clients.lock().unwrap();
        let mut clients_served = self.clients_served.lock().unwrap();

        let client = Client {
            password: password.to_vec(),
            status: ClientStatus::Sending,
            contact_info: ContactInfo::default(),
        };

        *clients_served = clients_served.wrapping_add(1);

        let id = *clients_served;
        rooms.insert(password.to_vec(), vec![id]);
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

    pub fn set_client_done(&mut self, client_id: u64, callback: fn(Vec<ContactInfo>)) {
        let mut clients = self.clients.lock().unwrap();
        let mut rooms = self.rooms.lock().unwrap();
        let client = clients.get_mut(&client_id).unwrap();
        client.status = ClientStatus::Waiting(callback);

        let client_ids = &rooms[&client.password];

        if client_ids
            .iter()
            .all(|id| matches!(clients[id].status, ClientStatus::Waiting(_)))
        {}
    }
}
