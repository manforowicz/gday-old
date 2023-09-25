use rustyline_async::{Readline, SharedWriter, ReadlineEvent};
use tokio::{io::{AsyncWriteExt, AsyncReadExt}, try_join};
use std::io::Write;
use crate::{Error, AsyncReadable, AsyncWritable};

pub async fn start_chat(reader: &mut impl AsyncReadable, writer: &mut impl AsyncWritable) -> Result<(), Error> {
    let (user_input, terminal) = Readline::new("> ".to_string()).unwrap();

    let terminal_clone = terminal.clone();

    let future_a = chat_listen(reader, terminal_clone);
    let future_b = chat_talk(writer, user_input, terminal);

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
            write!(terminal, "peer: {line}")?;
        }
        
    }
}

async fn chat_talk(
    writer: &mut impl AsyncWritable,
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