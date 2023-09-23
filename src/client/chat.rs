use indicatif::{ProgressBar, ProgressStyle};
use rustyline_async::{Readline, ReadlineEvent, SharedWriter};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::PathBuf};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

use crate::{
    protocol::{dynamic_deserialize_from, dynamic_serialize_into},
    Error,
};

use super::{
    contact_share::PeerConnection,
    encrypted_connection::{self, EncryptedReader, EncryptedWriter},
    file_dialog,
};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct FileMeta {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub enum PeerMessage {
    FileOffer(Option<Vec<FileMeta>>),
    FileAccept(Vec<bool>),
}

pub async fn creator_run(
    connection: PeerConnection,
    files: Option<Vec<FileMeta>>,
) -> Result<(), Error> {
    let (mut reader, mut writer) = encrypted_connection::new(connection).await?;

    let msg = &PeerMessage::FileOffer(files.clone());
    dynamic_serialize_into(&mut writer, msg).await?;

    if let Some(files) = files {
        let mut tmp_buf = Vec::new();
        let reply: PeerMessage = dynamic_deserialize_from(&mut reader, &mut tmp_buf).await?;

        if let PeerMessage::FileAccept(chosen) = reply {
            // ADD ASSERT TO ENSURE REPLY IS PROPER LENGTH

            let files_to_send: Vec<FileMeta> = files
                .into_iter()
                .zip(chosen.into_iter())
                .filter(|(_file, accepted)| *accepted)
                .map(|(file, _accepted)| file)
                .collect();

            send_files(&mut writer, files_to_send).await?;
        } else {
            // THROW AN ERROR HERE
        }
    }

    // START CHAT HERE
    Ok(())
}

pub async fn send_files(writer: &mut EncryptedWriter, files: Vec<FileMeta>) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();

    let progress = create_progress_bar(size);

    let mut buf = vec![0; 1_000_000];
    for meta in files {
        let mut file = File::open(meta.path).await?;
        loop {
            let bytes_read = file.read(&mut buf).await?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buf[..bytes_read]).await?;

            progress.inc(bytes_read as u64);
        }
    }
    Ok(())
}

pub async fn receive_files(
    reader: &mut EncryptedReader,
    files: Vec<FileMeta>,
) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();
    let progress = create_progress_bar(size);

    let mut buf = vec![0; 1_000_000];
    for meta in files {
        let mut file = File::create(meta.path).await?;
        let mut bytes_left = meta.size;
        while bytes_left != 0 {
            #[allow(clippy::cast_possible_truncation)]
            let chunk_size = std::cmp::min(buf.len(), bytes_left as usize);

            let bytes_read = reader.read(&mut buf[..chunk_size]).await?;
            bytes_left -= bytes_read as u64;
            file.write_all(&buf[0..bytes_read]).await?;

            progress.inc(bytes_read as u64);
        }
    }
    Ok(())
}

pub async fn not_creator_run(connection: PeerConnection) -> Result<(), Error> {
    let (mut reader, mut writer) = encrypted_connection::new(connection).await?;

    let mut tmp_buf = Vec::new();
    let msg: PeerMessage = dynamic_deserialize_from(&mut reader, &mut tmp_buf).await?;

    if let PeerMessage::FileOffer(files) = msg {
        if let Some(files) = files {
            let chosen = file_dialog::confirm_receive(&files)?;
            let msg = PeerMessage::FileAccept(chosen.clone());

            let files_to_receive: Vec<FileMeta> = files
                .into_iter()
                .zip(chosen.into_iter())
                .filter(|(_file, accepted)| *accepted)
                .map(|(file, _accepted)| file)
                .collect();

            dynamic_serialize_into(&mut writer, &msg).await?;
            receive_files(&mut reader, files_to_receive).await?;
        }
    } else {
        // THROW ERROR HERE
        todo!()
    }

    // SPAWN

    Ok(())
}

async fn start_chat(reader: EncryptedReader, writer: EncryptedWriter) -> Result<(), Error> {
    let (user_input, terminal) = Readline::new("> ".to_string()).unwrap();

    let terminal_clone = terminal.clone();

    let future_a = tokio::spawn(async move { chat_listen(reader, terminal_clone).await });
    let future_b = tokio::spawn(async move { chat_talk(writer, user_input, terminal).await });

    future_a.await.unwrap()?;
    future_b.await.unwrap()?;

    Ok(())
}

async fn chat_listen(
    mut reader: impl AsyncRead + Unpin,
    mut terminal: SharedWriter,
) -> Result<(), Error> {
    let mut tmp_buf = [0; 1_000];

    loop {
        let bytes_read = reader.read(&mut tmp_buf).await?;
        let text = std::str::from_utf8(&tmp_buf[..bytes_read])?;
        for line in text.lines() {
            write!(terminal, "peer: {line}")?;
        }
        
    }
}

async fn chat_talk(
    mut writer: impl AsyncWrite + Unpin,
    mut user_input: Readline,
    mut terminal: SharedWriter,
) -> Result<(), Error> {
    loop {
        let event = user_input.readline().await?;

        match event {
            ReadlineEvent::Line(text) => {
                writer.write_all(text.as_bytes()).await?;
                write!(terminal, "you: {text}")?;
            }
            _ => {
                return Ok(());
            }
        }
    }
}

fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})").unwrap();
    ProgressBar::new(bytes).with_style(style)
}
