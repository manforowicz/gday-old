/*

2 bytes  - total inclusive length of the entire message
1 byte - type of the message (see below)

--- 1 - client: create room (transmits new password) ---

password: array of u8 (up to 253 bytes)

--- 2 - client: join room (transmits password) ---

password: array of u8 (up to 253 bytes)

--- 3 - server: transmits user id (room created) ---

8 bytes

--- 4 - client: sending info ---

personal id (8 bytes)

series of optional: flag byte followed by content

1 - private ipv6 (16 byte ip, 2 byte port, 4 byte flowinfo, 4 byte scope_id)
2 - private ipv4 (4 byte ip, 2 byte port)

5 - nothing else to share / request peer info when available

--- 5 - server: other peer finished, here's their contact info ---

series of optional: flag byte followed by content


6 - spacer: each peer is precedeed by this flag
1 - private ipv6 (16 byte ip, 2 byte port)
2 - private ipv4 (4 byte ip, 2 byte port)
3 - public ipv4 (4 byte ip, 2 byte port)
4 - public ipv6 (16 byte ip, 2 byte port)




ERROR MESSAGE TYPES (no content in them)


6 - invalid message syntax
7 - room with this password already exists
8 - no room with this password exists
9 - unknown personal id
255 - other

*/

#![warn(clippy::all, clippy::pedantic)]

mod global_state;

use global_state::State;

mod connection_handler;
use connection_handler::ConnectionHandler;

mod protocol;

use tokio::net::TcpListener;
use tokio_native_tls::native_tls;

#[tokio::main]
async fn main() {
    let state = State::default();

    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    let der = include_bytes!("identity.pfx");

    let identity = native_tls::Identity::from_pkcs12(der, "###").unwrap();
    let tls_acceptor = tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());

    loop {
        let (stream, _addr) = listener.accept().await.unwrap();
        let tls_acceptor = tls_acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {  
            let tls_stream = tls_acceptor.accept(stream).await?;
            ConnectionHandler::start(state, tls_stream).await;
            Result::<(), native_tls::Error>::Ok(())
        } );
    }
}
