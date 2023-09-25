#![warn(clippy::all, clippy::pedantic)]
#![allow(dead_code)]

use clap::{Parser, Subcommand};
use gday_chat::file_dialog;
use gday_encryption::{EncryptedReader, EncryptedWriter};
use gday_hole_punch::client::{random_peer_secret, ContactSharer, PeerSecret, ServerAddr};
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
    Send { paths: Vec<PathBuf> },

    /// Create a room to chat
    Chat,

    /// Join a room
    Join { password: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.operation {
        Commands::Send { paths } => {
            let files = file_dialog::confirm_send(&paths).unwrap_or_else(|err| {
                eprintln!("{err}");
                exit(1)
            });
            let (mut reader, mut writer) = start_connection().await;
            println!("Encryption established baby!");
            gday_chat::creator_run(&mut reader, &mut writer, Some(files))
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    exit(1)
                });
        }

        Commands::Chat => {
            let (mut reader, mut writer) = start_connection().await;
            println!("Encryption established baby!");
            gday_chat::creator_run(&mut reader, &mut writer, None)
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    exit(1)
                });
        }

        Commands::Join { password } => {
            let (mut reader, mut writer) = join_connection(password).await;
            println!("Encryption established baby!");
            gday_chat::not_creator_run(&mut reader, &mut writer)
                .await
                .unwrap_or_else(|err| {
                    eprintln!("{err}");
                    exit(1)
                });
        }
    }
}

async fn start_connection() -> (EncryptedReader, EncryptedWriter) {
    let (sharer, room_id) = ContactSharer::create_room(SERVER)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Error connecting to server: {err}");
            exit(1)
        });

    let peer_secret = random_peer_secret();
    let mut password = room_id.into_iter().chain(peer_secret).collect::<Vec<u8>>();

    password.insert(3, b'-');
    password.insert(6, b'-');

    let password = String::from_utf8(password).unwrap();
    println!("Have your peer run: \"gday join {password}\". Password is case-insensitive.");

    establish_peer_connection(sharer, peer_secret).await
}

async fn join_connection(mut password: String) -> (EncryptedReader, EncryptedWriter) {
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

    establish_peer_connection(sharer, peer_secret).await
}

async fn establish_peer_connection(
    contact_sharer: ContactSharer,
    peer_secret: PeerSecret,
) -> (EncryptedReader, EncryptedWriter) {
    let connector = contact_sharer
        .get_peer_connector()
        .await
        .unwrap_or_else(|err| {
            eprintln!("Error getting peer contact: {err}");
            exit(1)
        });

    let (tcp_stream, shared_secret) =
        connector
            .connect_to_peer(peer_secret)
            .await
            .unwrap_or_else(|err| {
                eprintln!("Couldn't connect to peer: {err}");
                exit(1)
            });
    gday_encryption::new(tcp_stream, shared_secret)
        .await
        .unwrap_or_else(|err| {
            eprintln!("Couldn't set up encrypted channel with peer: {err}");
            exit(1)
        })
}
