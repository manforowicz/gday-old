use crate::{protocol::{FileMeta, LocalFileMeta}, RECEIVED_FILE_FOLDER};
use indicatif::HumanBytes;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub fn confirm_receive(files: &[FileMeta]) -> Result<Vec<bool>, std::io::Error> {
    let mut new_files = Vec::with_capacity(files.len());
    let mut new_size = 0;
    let mut total_size = 0;

    println!("Peer wants to send you {} files:", files.len());

    for meta in files {
        print!("{:?}", meta.path);
        total_size += meta.size;

        if file_exists(meta) {
            print!(" ALREADY EXISTS");
        } else {
            new_files.push(!file_exists(meta));
            new_size += meta.size;
        }
        println!();
    }

    println!();
    println!(
        "Size of modified/new files (files with a different path or size) only: {}",
        HumanBytes(new_size)
    );
    println!(
        "Total size, including already saved files: {}",
        HumanBytes(total_size)
    );
    println!("Options: ");
    println!("1. Reject all files. Don't download anything. (Default)");
    println!("2. Accept only files with new path or changed size. (New files will overwrite old files with the same path.)");
    println!("3. Accept all files, overwritting any old files with the same path.");
    print!("Choose an option (1, 2, or 3): ");

    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if response.trim() == "2" {
        Ok(new_files)
    } else if response.trim() == "3" {
        Ok(vec![true; files.len()])
    } else {
        Ok(vec![false; files.len()])
    }
}

fn file_exists(meta: &FileMeta) -> bool {
    let path = PathBuf::from(RECEIVED_FILE_FOLDER).join(&meta.path);
    if let Ok(file) = File::open(path) {
        if let Ok(local_meta) = file.metadata() {
            if local_meta.len() == meta.size {
                return true;
            }
        }
    }
    false
}

#[derive(Error, Debug)]
pub enum ConfirmSendError {
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Only non-overlapping paths allowed. The following paths overlap with each other:\n{0}\n{1}")]
    OverlappingPaths(PathBuf, PathBuf),

    #[error("Send cancelled")]
    UserCancelledSend,
}

pub fn confirm_send(paths: &[PathBuf]) -> Result<Vec<LocalFileMeta>, ConfirmSendError> {
    for (i, path1) in paths.iter().enumerate() {
        let canonical1 = path1.canonicalize()?;
        for path2 in &paths[i + 1..] {
            let canonical2 = path2.canonicalize()?;
            if canonical1.starts_with(&canonical2) || canonical2.starts_with(&canonical1) {
                return Err(ConfirmSendError::OverlappingPaths(
                    path1.to_path_buf(),
                    path2.to_path_buf(),
                ));
            }
        }
    }

    let files = get_file_metadatas(paths)?;

    let size: u64 = files.iter().map(|file| file.size).sum();
    println!("{} files:", files.len());
    for file in &files {
        println!("{} ({})", file.public_path.display(), HumanBytes(file.size));
    }
    println!("\nTotal size: {}", HumanBytes(size));
    print!("Do you want to send these files? (y/n): ");
    std::io::stdout().flush()?;
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if !"yes".starts_with(&response.trim().to_lowercase()) {
        return Err(ConfirmSendError::UserCancelledSend);
    }

    Ok(files)
}

fn get_file_metadatas(paths: &[PathBuf]) -> std::io::Result<Vec<LocalFileMeta>> {
    let mut files = Vec::new();

    for path in paths {
        let path = path.canonicalize()?;
        get_file_metadatas_helper(&path, &path, &mut files)?;
    }
    Ok(files)
}

fn get_file_metadatas_helper(
    top_path: &Path,
    path: &Path,
    files: &mut Vec<LocalFileMeta>,
) -> std::io::Result<()> {
    let path = path.canonicalize()?;
    let meta = std::fs::metadata(&path)?;

    if meta.is_dir() {
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            get_file_metadatas_helper(top_path, &entry?.path(), files)?;
        }
    } else if meta.is_file() && File::open(&path).is_ok() {
        let public_path = path.strip_prefix(top_path).unwrap().to_path_buf();
        let file_meta = LocalFileMeta {
            local_path: path,
            public_path,
            size: meta.len(),
        };
        files.push(file_meta);
    }
    Ok(())
}
