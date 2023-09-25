use std::{
    fs::File,
    path::{Path, PathBuf},
    process::exit,
};

use humansize::{format_size, DECIMAL};

use super::chat::FileMeta;

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
        format_size(new_size, DECIMAL)
    );
    println!(
        "Total size, including already saved files: {}",
        format_size(total_size, DECIMAL)
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
    let path = PathBuf::from("gday_received").join(&meta.path);
    if let Ok(file) = File::open(path) {
        if let Ok(local_meta) = file.metadata() {
            if local_meta.len() == meta.size {
                return true;
            }
        }
    }
    false
}

pub fn confirm_send(path: &Path) -> Result<Vec<FileMeta>, std::io::Error> {
    let files = get_file_metadatas(path)?;

    let size: u64 = files.iter().map(|file| file.size).sum();
    println!("{} files:", files.len());
    for file in &files {
        println!("{:?}", file.path);
    }
    println!("Total size: {}\n", format_size(size, DECIMAL));
    print!("Do you want to send these files? (y/n): ");
    let mut response = String::new();
    std::io::stdin().read_line(&mut response)?;

    if !"yes".starts_with(&response.trim().to_lowercase()) {
        println!("Cancelled send");
        exit(1)
    }

    Ok(files)
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
    } else if meta.is_file() && File::open(path).is_ok() {
        let local_path = path.strip_prefix(top_path).unwrap().to_path_buf();
        let file_meta = FileMeta {
            path: local_path,
            size: meta.len(),
        };
        files.push(file_meta);
    }
    Ok(())
}
