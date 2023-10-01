use crate::{AsyncReadable, AsyncWritable, Error};
use rustyline_async::{Readline, ReadlineEvent, SharedWriter};
use std::io::Write;
use tokio::io::{AsyncWriteExt, AsyncBufReadExt};
use crossterm::style::Stylize;

pub async fn start_chat(
    reader: &mut impl AsyncReadable,
    writer: &mut impl AsyncWritable,
) -> Result<(), Error> {
    let (user_input, terminal) = Readline::new("you: ".to_string()).unwrap();

    let future_a = chat_listen(reader, terminal.clone());
    let future_b = chat_talk(writer, user_input, terminal);

    tokio::select!(
        val = future_a => val,
        val = future_b => val
    )
}

async fn chat_listen(
    reader: &mut impl AsyncReadable,
    mut terminal: SharedWriter,
) -> Result<(), Error> {

    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        writeln!(terminal, "{} {}", "peer:".magenta(), line.magenta())?;
    }
    Ok(())
}

async fn chat_talk(
    writer: &mut impl AsyncWritable,
    mut user_input: Readline,
    mut terminal: SharedWriter
) -> Result<(), Error> {

    while let ReadlineEvent::Line(text) = user_input.readline().await? {
        if !text.trim().is_empty() {
            user_input.add_history_entry(text.to_string());
            writer.write_all(text.as_bytes()).await?;
            writer.flush().await?;
            terminal.flush()?;
        }
    }

    Ok(())
}
