use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::utils::compression;

const GPG_DIR: &str = ".gnupg";
const SSH_DIR: &str = ".ssh";

/// Backs up the user's GPG keys.
pub fn backup_gpg_keys(home_dir: &Path, interrupted: &Arc<AtomicBool>) -> io::Result<PathBuf> {
    let gpg_path = home_dir.join(GPG_DIR);
    let file_extension = compression::ARCHIVE_EXT;
    let file_name = format!("gnupg_backup.{}", file_extension);
    let backup_file = PathBuf::from(file_name);
    if !gpg_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "GPG directory not found",
        ));
    }
    // Ensure the parent directory exists
    if let Some(parent_dir) = backup_file.parent() {
        fs::create_dir_all(parent_dir)?;
    }

    // Compress the GPG directory
    compression::compress_directory(&gpg_path, &backup_file, None, interrupted)?;

    Ok(backup_file)
}

/// Backs up the user's SSH keys.
pub fn backup_ssh_keys(home_dir: &Path, interrupted: &Arc<AtomicBool>) -> io::Result<PathBuf> {
    let ssh_path = home_dir.join(SSH_DIR);
    let file_extension = compression::ARCHIVE_EXT;
    let file_name = format!("ssh_backup.{}", file_extension);
    let backup_file = PathBuf::from(file_name);
    if !ssh_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "SSH directory not found",
        ));
    }
    // Ensure the parent directory exists
    if let Some(parent_dir) = backup_file.parent() {
        fs::create_dir_all(parent_dir)?;
    }

    // Compress the SSH directory
    compression::compress_directory(&ssh_path, &backup_file, None, interrupted)?;

    Ok(backup_file)
}
