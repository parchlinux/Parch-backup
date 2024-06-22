use crate::utils::compression;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Backs up the user's home directory.
pub fn backup_home(home_dir: &Path) -> io::Result<PathBuf> {
    let file_extension = compression::TAR_GZ;
    let file_name = format!("home_backup.{}", file_extension);
    let backup_file = PathBuf::from(file_name);

    // Ensure the parent directory exists
    if let Some(parent_dir) = backup_file.parent() {
        fs::create_dir_all(parent_dir)?;
    }

    // Compress the home directory
    compression::compress_directory(home_dir, &backup_file)?;

    Ok(backup_file)
}
