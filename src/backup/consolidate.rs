use chrono::Utc;

use crate::cli::BackupArgs;
use crate::utils::compression::create_tar_gz_archive;
use crate::utils::security::{self, CryptoError};
use std::{fs, io};
use std::path::{Path, PathBuf};

/// Consolidates individual backups into a single archive file.
pub fn consolidate_backups(backup_files: &[(&str, PathBuf)], args: &BackupArgs) -> io::Result<()> {
    // Prepare timestamp and components for archive name
    let timestamp = Utc::now().format("%Y-%m-%d-%H:%M:%S");
    let components = {
        let mut components = String::new();
        if args.apps {
            components.push('a');
        }
        if args.home {
            components.push('h');
        }
        if args.keys {
            components.push('k');
        }
        if args.flatpak {
            components.push('f');
        }
        if args.encrypt {
            components.push('e');
        }
        components
    };

    // Construct archive name
    let archive_name = format!(
        "backup-{}-{}.{}",
        timestamp,
        components,
        crate::utils::compression::ARCHIVE_EXT
    );

    // Determine the full archive path
    let archive_path = if let Some(ref path) = args.archive_path {
        Path::new(path).join(&archive_name)
    } else {
        Path::new(&archive_name).to_path_buf()
    };

    // Create directory if it doesn't exist
    if let Some(parent_dir) = archive_path.parent() {
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir)?;
            println!("Created directory: {}", parent_dir.display());
        }
    }

    // Convert &str elements in backup_files to String
    let backup_files: Vec<(String, PathBuf)> = backup_files
        .iter()
        .map(|(subdir, path)| (subdir.to_string(), path.clone()))
        .collect();

    // Create the archive
    create_tar_gz_archive(archive_path.to_str().unwrap(), &backup_files)?;

    // Encrypt the consolidated backup file if encryption is enabled
    if let Some(key) = &args.encrypt_key {
        if let Err(e) = security::encrypt_file(&archive_path, key.as_bytes()) {
            match e {
                CryptoError::FileRead(_) => eprintln!("Failed to read the archive file: {:?}", e),
                CryptoError::FileWrite(_) => eprintln!("Failed to write the encrypted file: {:?}", e),
                _ => {},
            }
        }
    }

    println!("Archive located at: {}", archive_path.display());

    Ok(())
}
