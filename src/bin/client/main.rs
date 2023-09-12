#![warn(clippy::all, clippy::pedantic)]
#![allow(dead_code)]

mod establisher;
mod server_connection;
mod peer_connection;
mod chat;

use clap::{Parser, Subcommand};
use establisher::Establisher;

use std::net::{SocketAddrV6, Ipv6Addr, SocketAddrV4, Ipv4Addr};

use server_connection::ServerAddr;
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
    /// Create a room
    Start,

    /// Join a room
    Join { password: String },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let mut establisher = match cli.operation {
        Commands::Start => {
            let establisher = Establisher::create_room(SERVER).await.unwrap_or_else(|err| {
                eprintln!("Error connecting to server: {err}");
                exit(1)
            });
            let password = String::from_utf8(establisher.get_password().to_vec()).unwrap();
            println!("Send this password to your peer: {password}");
            establisher
        }
        Commands::Join {mut password} => {
            password.retain(|c| !c.is_whitespace() && c != '-');
            let password = password.to_uppercase();
            let password: [u8; 9] = password.as_bytes().try_into().unwrap_or_else(|_| {
                eprintln!("Provided password must be exactly 9 alphanumeric characters!");
                exit(1)
            });
            Establisher::join_room(SERVER, password).await.unwrap_or_else(|err| {
                eprintln!("Error joining room: {err}");
                exit(1)
            })
        }
    };

    let connection = establisher.get_peer_conection().await.unwrap_or_else(|err| {
        eprintln!("Error getting peer info from server: {err}");
        exit(1)
    });

    println!("Successfully established encrypted connection with peer.");

    chat::Chat::begin(connection.stream, connection.shared_key).await.unwrap_or_else(|err| {
        eprintln!("Peer disconnected: {err}");
        exit(1)
    });
}