#![warn(clippy::all, clippy::pedantic)]
#![allow(dead_code)]

use clap::{Parser, Subcommand};
use holepunch::client::contact_share::{ContactSharer, PeerConnection};
use holepunch::client::file_dialog::confirm_send;
use holepunch::client::holepuncher;
use holepunch::client::server_connection::ServerAddr;
use std::iter::Iterator;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};
use std::path::PathBuf;
use std::process::exit;

const SERVER: ServerAddr = ServerAddr {
    v6: SocketAddrV6::new(
        Ipv6Addr::new(
            0x2603, 0xc024, 0xc00c, 0xb17e, 0xfce5, 0xf16d, 0x4207, 0xb22d,
        ),
        49870,
        0,
        0,
    ),
    v4: SocketAddrV4::new(Ipv4Addr::new(146, 235, 206, 20), 49870),
    name: "psend",
};

/// TODO description here
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    operation: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Create a room to send files
    Send { path: PathBuf },

    /// Create a room to chat
    Chat,

    /// Join a room
    Join { password: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.operation {
        Commands::Send { path } => {
            let files = confirm_send(&path).unwrap_or_else(|err| {
                eprintln!("{err}");
                exit(1)
            });
            let connection = start_connection();
        }

        Commands::Chat => {
            let connection = start_connection();
        }

        Commands::Join { password } => {
            let connection = join_connection(password);
        }
    }

    println!("Successfully established encrypted connection with peer.");
}

async fn start_connection() -> PeerConnection {
    let (sharer, room_id) = ContactSharer::create_room(SERVER)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Error connecting to server: {err}");
            exit(1)
        });

    let peer_secret = holepuncher::random_peer_secret();
    let mut password = room_id.into_iter().chain(peer_secret).collect::<Vec<u8>>();

    password.insert(4, b'-');
    password.insert(8, b'-');

    let password = String::from_utf8(password).unwrap();
    println!("Have your peer run: gday join {password}");
    let (peer, me) = sharer.get_peer_contact().await.unwrap_or_else(|err| {
        eprintln!("Error getting peer contact: {err}");
        exit(1)
    });

    holepuncher::get_peer_conection(peer, peer_secret, true, me)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Couldn't connect to peer: {err}");
            exit(1);
        })
}

async fn join_connection(mut password: String) -> PeerConnection {
    password.retain(|c| !c.is_whitespace() && c != '-');
    let password = password.to_uppercase();
    let password: [u8; 9] = password.as_bytes().try_into().unwrap_or_else(|_| {
        eprintln!("Password must be exactly 9 characters!");
        exit(1)
    });

    if !password
        .iter()
        .all(|c| b"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890".contains(c))
    {
        eprintln!("Password must be alphanumeric!");
        exit(1)
    }

    let room_id = password[0..6].try_into().unwrap();
    let peer_secret = password[6..9].try_into().unwrap();

    let sharer = ContactSharer::join_room(SERVER, room_id)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Error joining room: {err}");
            exit(1)
        });

    let (peer, me) = sharer.get_peer_contact().await.unwrap_or_else(|err| {
        eprintln!("Error getting peer contact: {err}");
        exit(1)
    });

    holepuncher::get_peer_conection(peer, peer_secret, false, me)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Couldn't connect to peer: {err}");
            exit(1);
        })
}
