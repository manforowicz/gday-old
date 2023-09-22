use crate::client::encrypted_connection::{Reader, Writer};
use crate::client::establisher::PeerConnection;
use crate::protocol::{deserialize_from, serialize_into};
use crate::Error;
use humansize::{format_size, DECIMAL};
use indicatif::ProgressBar;
use owo_colors::OwoColorize;
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::io::{AsyncBufReadExt, AsyncWrite};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum PeerMessage<'a> {
    Text(&'a str),
    Offer(Vec<FileMeta>),
    Accept(Vec<bool>),
    FileChunk(&'a [u8]),
    DoneSending,
}

pub async fn start(peer_connection: PeerConnection) -> Result<(), Error> {
    let (reader, writer) = peer_connection.stream.into_split();

    let writer = Writer::new(writer, peer_connection.shared_secret).await?;
    let reader = Reader::new(reader, peer_connection.shared_secret).await?;

    println!("foboodfsa");

    let handle1 = tokio::spawn(Receiver::start(reader));
    let handle2 = tokio::spawn(Sender::start(writer));

    handle1.await.unwrap()?;
    handle2.await.unwrap()?;

    Ok(())
}

struct Receiver<R: AsyncRead + Unpin> {
    reader: R,
    tmp_buf: Vec<u8>,
}

impl<R: AsyncRead + Unpin> Receiver<R> {
    async fn start(reader: R) -> Result<(), Error> {
        let mut this = Self {
            reader,
            tmp_buf: vec![0; 10000],
        };
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
                PeerMessage::Offer(offer) => {

                },
            }
        }
    }

    async fn ask_about_offer(files: &[FileMeta]) -> Vec<bool> {
        
        let mut chosen = vec![true; files.len()];
        let mut chosen_size = 0;
        let mut total_size = 0;

        println!("{} files:", files.len());

        for (i, meta) in files.iter().enumerate() {

            total_size += meta.size;

            println!("{:?}", meta.path);
            if let Ok(file) = File::open(&meta.path).await {
                if let Ok(local_meta) = file.metadata().await {
                    if local_meta.len() == meta.size {
                        println!(" ALREADY SAVED")
                    }   
                }
            }
            chosen[i] = true;
            chosen_size += meta.size;
            println!();
        }

        println!();
        println!("Size of modified/new files only: {}", format_size(chosen_size, DECIMAL));
        println!("Total size, including already saved files: {}", format_size(total_size, DECIMAL));
        println!("Choose an option:");
        println!("1. Reject all files. Don't download anything.");
        println!("2. Accept only files with new path or changed size. (New files will overwrite old files with the same path.)");
        println!("3. Accept all files, overwritting any old files with the same path.");




        Vec::new()
    }
}

struct Sender<W: AsyncWrite + Unpin + Send> {
    writer: W,
    tmp_buf: Vec<u8>,
    rl: rustyline::Editor<(), DefaultHistory>,
}

impl<W: AsyncWrite + Unpin + Send> Sender<W> {
    async fn start(writer: W) -> Result<(), Error> {
        let mut this = Self {
            writer,
            tmp_buf: vec![0; 10000],
            rl: rustyline::DefaultEditor::new()?,
        };
        this.run().await
    }

    async fn run(&mut self) -> Result<(), Error> {
        loop {
            let line = self.rl.readline(">> ")?;
            let line = line.trim();

            if line.as_bytes()[0] == b'/' {
                self.process_command(&line[1..]).await?;
            } else {
                let msg = PeerMessage::Text(line);
                serialize_into(&mut self.writer, &msg, &mut self.tmp_buf).await?;
            }
        }
    }

    async fn process_command(&mut self, mut command: &str) -> Result<(), Error> {
        if command.starts_with("send") {
            command = command.strip_prefix("send").unwrap();
            let path = Path::new(command);
            self.send_files(path).await?;
        } else if command.starts_with("help") {
            println!("HELP MESASAGE");
        } else {
            println!("Type \"/help\" for help");
        }
        Ok(())
    }

    async fn send_files(&mut self, path: &Path) -> Result<(), Error> {
        let files = get_file_metadatas(path)?;
        if !confirm_send(&files)? {
            println!("Cancelled");
            return Ok(());
        }

        let mut buf = vec![0; 10000];

        let total_size: u64 = files.iter().map(|file| file.size).sum();
        let progress_bar = ProgressBar::new(total_size);

        for meta in files {
            let mut file = File::open(&meta.path).await?;
            let status = format!(
                "Sending {:?} ({})",
                meta.path,
                format_size(meta.size, DECIMAL)
            );
            progress_bar.set_message(status);

            loop {
                let num_read = file.read(&mut buf).await?;
                if num_read == 0 {
                    break;
                }
                progress_bar.inc(num_read as u64);
                let filled = &buf[0..num_read];
                let msg = PeerMessage::FileChunk(filled);
                serialize_into(&mut self.writer, &msg, &mut self.tmp_buf).await?;
            }
        }

        progress_bar.finish_with_message("Files successfully sent");

        Ok(())
    }
}

fn confirm_send(files: &[FileMeta]) -> Result<bool, std::io::Error> {
    let size: u64 = files.iter().map(|file| file.size).sum();
    println!("{} files:", files.len());
    for file in files {
        println!("{:?}", file.path);
    }
    println!("Total size: {}", format_size(size, DECIMAL));
    println!();
    print!("Are you sure you want to send these files? (y/n)");
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;
    Ok("yes".starts_with(&response.to_lowercase()))
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
