use tokio::net::TcpStream;

mod reader;
mod writer;
pub use reader::EncryptedReader;

pub use writer::EncryptedWriter;

pub async fn new(
    tcp_stream: TcpStream,
    shared_secret: [u8; 32],
) -> std::io::Result<(EncryptedReader, EncryptedWriter)> {
    let (read, write) = tcp_stream.into_split();

    let writer = EncryptedWriter::new(write, shared_secret).await?;
    let reader = EncryptedReader::new(read, shared_secret).await?;

    Ok((reader, writer))
}