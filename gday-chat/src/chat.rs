use crate::{AsyncReadable, AsyncWritable, Error};
use rustyline_async::{Readline, ReadlineEvent, SharedWriter};
use std::io::Write;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    try_join,
};

pub async fn start_chat(
    reader: &mut impl AsyncReadable,
    writer: &mut impl AsyncWritable,
) -> Result<(), Error> {
    let (user_input, terminal) = Readline::new("you: ".to_string()).unwrap();

    let future_a = chat_listen(reader, terminal);
    let future_b = chat_talk(writer, user_input);

    try_join!(future_a, future_b)?;
    Ok(())
}

async fn chat_listen(
    reader: &mut impl AsyncReadable,
    mut terminal: SharedWriter,
) -> Result<(), Error> {
    let mut tmp_buf = [0; 1_000];

    loop {
        let bytes_read = reader.read(&mut tmp_buf).await?;
        let text = std::str::from_utf8(&tmp_buf[..bytes_read])?;
        for line in text.lines() {
            writeln!(terminal, "peer: {line}")?;
        }
    }
}

async fn chat_talk(
    writer: &mut impl AsyncWritable,
    mut user_input: Readline
) -> Result<(), Error> {
    loop {
        let event = user_input.readline().await?;

        match event {
            ReadlineEvent::Line(text) => {
                if !text.trim().is_empty() {
                    writer.write_all(text.as_bytes()).await?;
                    user_input.add_history_entry(text);
                }
            }
            _ => {
                return Ok(());
            }
        }
    }
}
