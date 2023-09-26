use tokio::net::TcpStream;

mod reader;
mod writer;
mod writer_progress;
pub use reader::EncryptedReader;

pub use writer_progress::EncryptedWriter;

const MAX_CHUNK_SIZE: usize = 100_000;


const BUF_CAPACITY: usize = 10_000;

pub async fn new(
    tcp_stream: TcpStream,
    shared_secret: [u8; 32],
) -> std::io::Result<(EncryptedReader, EncryptedWriter)> {
    let (read, write) = tcp_stream.into_split();

    let writer = EncryptedWriter::new(write, shared_secret).await?;
    let reader = EncryptedReader::new(read, shared_secret).await?;

    Ok((reader, writer))
}

