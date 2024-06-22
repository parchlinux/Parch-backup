use crate::cli::BackupArgs;
use crate::utils::security::{self, CryptoError};
use chrono::Local;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tar::Builder;

/// Consolidates individual backups into a single archive file.
pub fn consolidate_backups(backup_files: &[(&str, PathBuf)], args: &BackupArgs) -> io::Result<()> {
    // Prepare timestamp and components for archive name
    let timestamp = Local::now().format("%Y-%m-%d-%H:%M:%S");
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
    let archive_name = format!("backup-{}-{}.{}", timestamp, components, crate::utils::compression::TAR_GZ);

    // Create the archive file
    let archive_file = File::create(&archive_name)?;
    let enc = flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
    let mut tar = Builder::new(enc);

    // Add each backup file to the archive and remove original files
    for (subdir, file) in backup_files {
        let path_in_archive = Path::new(subdir).join(file.file_name().unwrap());
        tar.append_path_with_name(file, path_in_archive)?;
        fs::remove_file(file)?;
    }

    // Finalize the archive
    tar.finish()?;

    // Encrypt the consolidated backup file if encryption is enabled
    if let Some(key) = &args.encrypt_key {
        if let Err(e) = security::encrypt_file(Path::new(&archive_name), key.as_bytes()) {
            match e {
                CryptoError::FileRead(_) => eprintln!("Failed to read the archive file: {:?}", e),
                CryptoError::FileWrite(_) => {
                    eprintln!("Failed to write the encrypted file: {:?}", e)
                }
                _ => {}
            }
        }
    }

    Ok(())
}
