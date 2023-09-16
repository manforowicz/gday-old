use crate::client::peer_connection::{self, FileMeta, PeerConnection};
use crate::protocol::{deserialize_from, PeerMessage, serialize_into};
use crate::Error;
use owo_colors::OwoColorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{self, AsyncRead};
use tokio::io::{AsyncBufReadExt, AsyncWrite};
use tokio::sync::Mutex;


pub async fn start(peer_connection: PeerConnection) -> Result<(), Error> {
    let (reader, writer) = peer_connection.split();

    let handle1 = tokio::spawn(Receiver::start(reader));
    let handle2 = tokio::spawn(Sender::start(writer));

    handle1.await.unwrap()?;
    handle2.await.unwrap()?;

    Ok(())
}

struct Receiver<R: AsyncRead + Unpin> {
    reader: R,
    tmp_buf: [u8; 8000]
}

impl<R: AsyncRead + Unpin> Receiver<R> {
    async fn start(reader: R) -> Result<(), Error> {
        let mut this = Self { reader , tmp_buf: [0; 8000]};
        this.run().await
    }

    async fn run(&mut self) -> Result<(), Error> {
        loop {
            let msg = deserialize_from(&mut self.reader, &mut self.tmp_buf).await?;
            match msg {
                PeerMessage::Text(text) => {
                    println!("{}{}{}", "peer: ".purple(), text.purple(), "you".green());
                    std::io::stdout().flush()?;
                }
                _ => ()
            }

        }
    }
}

struct Sender<W: AsyncWrite + Unpin> {
    writer: W,
    tmp_buf: [u8; 8000]
}

impl<W: AsyncWrite + Unpin> Sender<W> {
    async fn start(writer: W) -> Result<(), Error> {
        let mut this = Self {
            writer,
            tmp_buf: [0; 8000]
        };
        this.run().await
    }

    async fn run(&mut self) -> Result<(), Error> {
        let mut line = String::new();

        let mut lines = io::BufReader::new(io::stdin());

        loop {
            lines.read_line(&mut line).await?;
            self.process_line(&line).await?;
            line.clear();
        }
    }

    async fn process_line(&mut self, line: &str) -> Result<(), Error> {
        if line.trim().is_empty() {
            return Ok(());
        }

        if line.as_bytes()[0] == b'/' {
            self.process_command(&line[1..]);
        } else {
            let msg = PeerMessage::Text(line);
            serialize_into(&mut self.writer, &msg, &mut self.tmp_buf).await?;
        }

        Ok(())
    }

    fn process_command(&mut self, mut command: &str) {
        if command.starts_with("send") {
            command = command.strip_prefix("send").unwrap();
            let path = Path::new(command);
        } else if command.starts_with("help") {
        } else {
            println!("HELPFUL LIST OF COMMANDS GOES HERE");
        }
    }

    async fn send_files(&mut self, path: &Path) -> Result<(), std::io::Error> {
        let metadatas = get_file_metadatas(path);

        Ok(())
    }
}

fn get_file_metadatas(path: &Path) -> std::io::Result<Vec<FileMeta>> {
    let mut files = Vec::new();
    get_file_metadatas_helper(path, path, &mut files)?;
    Ok(files)
}

fn get_file_metadatas_helper(
    top_path: &Path,
    path: &Path,
    files: &mut Vec<FileMeta>,
) -> std::io::Result<()> {
    let meta = std::fs::metadata(path)?;

    if meta.is_dir() {
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            get_file_metadatas_helper(top_path, &entry?.path(), files)?;
        }
    } else if meta.is_file() {
        let local_path = path.strip_prefix(top_path).unwrap().to_path_buf();
        let file_meta = FileMeta {
            path: local_path,
            size: meta.len(),
        };
        files.push(file_meta);
    }
    Ok(())
}
