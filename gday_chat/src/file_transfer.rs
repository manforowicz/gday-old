use std::{path::Path, task::Poll};

use crate::{
    protocol::{FileMeta, LocalFileMeta},
    AsyncReadable, AsyncWritable, RECEIVED_FILE_FOLDER,
};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use pin_project::pin_project;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
};

pub async fn send_files(
    writer: &mut impl AsyncWritable,
    files: Vec<LocalFileMeta>,
) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();

    let progress = create_progress_bar(size);

    for meta in files {
        let msg = meta.public_path.to_string_lossy().to_string();
        progress.set_message(msg);
        let mut file = File::open(meta.local_path).await?;

        let mut writer = ProgressWrite {
            writer,
            progress: &progress,
        };

        tokio::io::copy(&mut file, &mut writer).await?;
    }

    writer.flush().await?;
    Ok(())
}

pub async fn receive_files(
    reader: &mut impl AsyncReadable,
    files: Vec<FileMeta>,
) -> std::io::Result<()> {
    let size: u64 = files.iter().map(|meta| meta.size).sum();
    let progress = create_progress_bar(size);

    for meta in files {
        let msg = meta.path.to_string_lossy().to_string();
        progress.set_message(msg);
        let path = Path::new(RECEIVED_FILE_FOLDER).join(meta.path);
        let prefix = path.parent().unwrap_or(Path::new(""));
        std::fs::create_dir_all(prefix)?;
        let mut file = File::create(path).await?;

        let mut reader = reader.take(meta.size);
        let mut writer = ProgressWrite {
            writer: &mut file,
            progress: &progress,
        };

        tokio::io::copy_buf(&mut reader, &mut writer).await?;
    }
    Ok(())
}

fn create_progress_bar(bytes: u64) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{msg} [{wide_bar}] {bytes}/{total_bytes} | {bytes_per_sec} | {eta} left",
    )
    .unwrap();
    let draw = ProgressDrawTarget::stderr_with_hz(2);
    ProgressBar::with_draw_target(Some(bytes), draw).with_style(style).with_message("Starting")
}

#[pin_project]
struct ProgressWrite<'a, T: AsyncWritable> {
    #[pin]
    writer: &'a mut T,
    progress: &'a ProgressBar,
}

impl<'a, T: AsyncWritable> AsyncWrite for ProgressWrite<'a, T> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.project();
        let poll = this.writer.poll_write(cx, buf);

        if let Poll::Ready(Ok(num)) = poll {
            this.progress.inc(num as u64);
        } else {
            this.progress.tick();
        }
        poll
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().writer.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        self.project().writer.poll_shutdown(cx)
    }
}
