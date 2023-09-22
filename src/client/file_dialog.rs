use std::{path::Path, process::exit};

use humansize::{format_size, DECIMAL};

use super::chat::FileMeta;

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
