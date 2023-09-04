/*

1 bytes  - total inclusive length of the entire message
1 byte - type of the message (see below)



--- 1 - client: transmits password ---

password: array of u8 (up to 253 bytes)

--- 2 - server: transmits user id (room created) ---

8 bytes

--- 3 - client: sending info ---

personal id (8 bytes)

series of optional: flag byte followed by content

1 - private ipv4 (4 byte ip, 2 byte port)
2 - private ipv6 (16 byte ip, 2 byte port)
5 - nothing else to share / request peer info when available

--- 4 - server: other peer finished, here's their contact info ---

series of optional: flag byte followed by content

1 - private ipv4 (4 byte ip, 2 byte port)
2 - private ipv6 (16 byte ip, 2 byte port)
3 - public ipv4 (4 byte ip, 2 byte port)
4 - public ipv6 (16 byte ip, 2 byte port)



ERROR MESSAGE TYPES (no content in them)


5 - invalid message type (only 1 and 3 are for client)
6 - invalid message length (must be at least 3 bytes long)
7 - invalid flag byte (only 1, 2, 5 are for client)
8 - password taken (pick a new one, start over)
9 - unknown personal id
255 - other

*/

use std::collections::HashMap;
use std::net::{SocketAddrV4, SocketAddrV6};
use std::sync::Arc;
use std::sync::Mutex;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

async fn handle_client(state: Arc<Mutex<State>>, mut stream: TcpStream) {
    loop {
        handle_message(&state, &mut stream).await;
    }
}

async fn handle_message(state: &Arc<Mutex<State>>, stream: &mut TcpStream) {
    let msg_length = stream.read_u8().await.unwrap();
    let msg_type = stream.read_u8().await.unwrap();

    if msg_length <= 2 {
        send(stream, 6, &[]).await;
        return;
    }

    let mut msg = vec![0; msg_length as usize - 2];
    stream.read_exact(&mut msg).await.unwrap();

    match msg_type {
        1 => receive1(state, stream, &msg).await,
        3 => receive3(state, stream, &msg).await,
        // invalid message type (only types 1 and 3 are for client)
        _ => send(stream, 5, &[]).await,
    }
}

async fn receive1(state: &Arc<Mutex<State>>, stream: &mut TcpStream, msg: &[u8]) {
    let password = msg;

    if state.lock().unwrap().rooms.contains_key(password) {
        send(stream, 8, &[]).await
    } else {
        let id = state.lock().unwrap().add_client(password);

        send(stream, 2, &id.to_be_bytes()[..]).await;
    }
    
}

async fn receive3(state: &Arc<Mutex<State>>, stream: &mut TcpStream, msg: &[u8]) {}



async fn send(stream: &mut TcpStream, code: u8, data: &[u8]) {
    stream
        .write(&[&[data.len() as u8, code], data].concat())
        .await
        .unwrap();
}

#[tokio::main]
async fn main() {
    let state = Arc::new(Mutex::new(State::default()));

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        tokio::spawn(handle_client(Arc::clone(&state), stream));
    }
}

#[derive(Default)]
struct State {
    /// Maps room password to client IDs in that room
    rooms: HashMap<Vec<u8>, Vec<u64>>,
    /// Maps client ID to client
    clients: HashMap<u64, Client>,
    clients_served: u64,
}

impl State {
    /// Adds a client and returns their new id
    fn add_client(&mut self, password: &[u8]) -> u64 {
        let client = Client {
            password: password.to_vec(),
            ..Default::default()
        };

        self.clients_served = self.clients_served.wrapping_add(1);

        let id = self.clients_served;
        self.rooms.insert(password.to_vec(), vec![id]);
        self.clients.insert(id, client);

        id
    }
}

#[derive(PartialEq, Eq, Hash, Default)]
struct Client {
    password: Vec<u8>,
    private_addr_v4: Option<SocketAddrV4>,
    public_addr_v4: Option<SocketAddrV4>,
    private_addr_v6: Option<SocketAddrV6>,
    public_addr_v6: Option<SocketAddrV6>,
}
