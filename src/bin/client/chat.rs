use crate::peer_connection::{self, PeerConnection, PeerReader, PeerWriter};
use owo_colors::OwoColorize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::io;
use tokio::sync::Mutex;

enum Message {
    Text(String),
}

struct FileMeta {
    path: PathBuf,
    size: u64,
}

pub async fn start(peer_connection: PeerConnection) -> Result<(), peer_connection::Error> {
    let (reader, writer) = peer_connection.split();

    let handle1 = tokio::spawn(Receiver::start(reader));
    let handle2 = tokio::spawn(Sender::start(writer));

    handle1.await.unwrap()?;
    handle2.await.unwrap()?;

    Ok(())
}

struct Receiver {
    reader: PeerReader,
}

impl Receiver {
    async fn start(reader: PeerReader) -> Result<(), peer_connection::Error> {
        let mut this = Self { reader };
        this.run().await
    }

    async fn run(&mut self) -> Result<(), peer_connection::Error> {
        loop {
            let msg = self.reader.receive().await?;
            let msg = String::from_utf8(msg)?;

            println!("{}{}{}", "peer: ".purple(), msg.purple(), "you".green());
            std::io::stdout().flush()?;
        }
    }
}

struct Sender {
    writer: Arc<Mutex<PeerWriter>>,
}

impl Sender {
    async fn start(writer: PeerWriter) -> Result<(), peer_connection::Error> {
        let mut this = Self {
            writer: Arc::new(Mutex::new(writer)),
        };
        this.run().await
    }

    async fn run(&mut self) -> Result<(), peer_connection::Error> {
        let mut line = String::new();

        let mut lines = io::BufReader::new(io::stdin());

        loop {
            lines.read_line(&mut line).await?;
            self.process_line(&line).await?;
            line.clear();
        }
    }

    async fn process_line(&mut self, line: &str) -> Result<(), peer_connection::Error> {
        if line.trim().is_empty() {
            return Ok(());
        }

        if line.as_bytes()[0] == b'/' {
            self.process_command(&line[1..]);
        } else {
            let mut writer = self.writer.lock().await;
            writer.send(line.as_bytes()).await?;
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
