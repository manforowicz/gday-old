use std::path::Path;

use crate::{
    protocol::{FileMeta, LocalFileMeta},
    RECEIVED_FILE_FOLDER,
};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use tokio::{
    fs::File,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

pub async fn send_files(
    writer: &mut (impl AsyncWrite + Unpin),
    files: Vec<LocalFileMeta>,
) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();

    let progress = create_progress_bar(size);

    let mut buf = vec![0; 10_000];
    for meta in files {
        let mut file = File::open(meta.local_path).await?;
        loop {
            let bytes_read = file.read(&mut buf).await?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buf[..bytes_read]).await?;

            progress.inc(bytes_read as u64);
        }
    }
    writer.flush().await?;
    Ok(())
}

pub async fn receive_files(
    reader: &mut (impl AsyncRead + Unpin),
    files: Vec<FileMeta>,
) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();
    let progress = create_progress_bar(size);

    let mut buf = vec![0; 10_000];
    for meta in files {
        let path = Path::new(RECEIVED_FILE_FOLDER).join(meta.path);
        let prefix = path.parent().unwrap_or(Path::new(""));
        std::fs::create_dir_all(prefix)?;
        let mut file = File::create(path).await?;
        let mut bytes_left = meta.size;
        while bytes_left != 0 {
            let chunk_size = std::cmp::min(buf.len(), bytes_left as usize);
            let bytes_read = reader.read(&mut buf[..chunk_size]).await?;
            bytes_left -= bytes_read as u64;
            file.write_all(&buf[0..bytes_read]).await?;

            progress.inc(bytes_read as u64);
        }
    }
    Ok(())
}

fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "[{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | time left: {eta}",
    )
    .unwrap();
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(bytes), draw).with_style(style)
}
