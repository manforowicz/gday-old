mod chat;
pub mod file_dialog;
mod file_transfer;
mod protocol;

use std::str::Utf8Error;

use protocol::{deserialize_from, serialize_into, FileMeta, Message, LocalFileMeta};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};


const RECEIVED_FILE_FOLDER: &str = "gday_received/";

pub trait AsyncReadable: AsyncRead + Send + Unpin {}
impl<T: AsyncRead + Send + Unpin> AsyncReadable for T {}

pub trait AsyncWritable: AsyncWrite + Send + Unpin {}
impl<T: AsyncWrite + Send + Unpin> AsyncWritable for T {}



#[derive(Error, Debug)]
pub enum Error {
    #[error("Error with encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Message too long: {0}")]
    MessageTooLong(#[from] std::num::TryFromIntError),

    #[error("Readline async error: {0}")]
    RustylineAsync(#[from] rustyline_async::ReadlineError),

    #[error("Non utf-8 chat message received: {0}")]
    NonUtf8MessageReceived(#[from] Utf8Error),

    #[error("Unexpected message: {0:?}")]
    UnexpectedMessge(Message),
}

pub async fn creator_run(
    reader: &mut impl AsyncReadable,
    writer: &mut impl AsyncWritable,
    files: Option<Vec<LocalFileMeta>>,
) -> Result<(), Error> {

    if let Some(files) = files {
        let metas = files.iter().map(|file| FileMeta{path: file.public_path.clone(), size: file.size}).collect();

        let msg = Message::FileOffer(Some(metas));
        serialize_into(writer, &msg).await?;

        let mut tmp_buf = Vec::new();
        let reply = deserialize_from(reader, &mut tmp_buf).await?;

        if let Message::FileAccept(chosen) = reply {
            // ADD ASSERT TO ENSURE REPLY IS PROPER LENGTH

            let files_to_send: Vec<LocalFileMeta> = files
                .into_iter()
                .zip(chosen.into_iter())
                .filter(|(_file, accepted)| *accepted)
                .map(|(file, _accepted)| file)
                .collect();

            file_transfer::send_files(writer, files_to_send).await?;
        } else {
            return Err(Error::UnexpectedMessge(reply));
        }
    } else {
        let msg = Message::FileOffer(None);
        serialize_into(writer, &msg).await?;
    }


    chat::start_chat(reader, writer).await
}

pub async fn not_creator_run(
    mut reader: &mut impl AsyncReadable,
    mut writer: &mut impl AsyncWritable,
) -> Result<(), Error> {
    let mut tmp_buf = Vec::new();
    println!("Waiting on message!");
    let msg: Message = deserialize_from(&mut reader, &mut tmp_buf).await?;
    println!("Received messsge at least :?");

    if let Message::FileOffer(files) = msg {
        if let Some(files) = files {
            let chosen = file_dialog::confirm_receive(&files)?;
            let msg = Message::FileAccept(chosen.clone());
            serialize_into(&mut writer, &msg).await?;

            let files_to_receive: Vec<FileMeta> = files
                .into_iter()
                .zip(chosen.into_iter())
                .filter(|(_file, accepted)| *accepted)
                .map(|(file, _accepted)| file)
                .collect();

            
            file_transfer::receive_files(&mut reader, files_to_receive).await?;
        }
    } else {
        return Err(Error::UnexpectedMessge(msg));
    }

    chat::start_chat(reader, writer).await
}
