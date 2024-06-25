use crate::utils::compression;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// Backs up the user's home directory.
pub fn backup_home(
    home_dir: &Path,
    exclude_dir: &[String],
    interrupted: &Arc<AtomicBool>,
) -> io::Result<PathBuf> {
    let file_extension = compression::ARCHIVE_EXT;
    let file_name = format!("home_backup.{}", file_extension);
    let backup_file = PathBuf::from(file_name);

    // Ensure the parent directory exists
    if let Some(parent_dir) = backup_file.parent() {
        fs::create_dir_all(parent_dir)?;
    }

    // Compress the home directory
    match compression::compress_directory(home_dir, &backup_file, Some(exclude_dir), interrupted) {
        Ok(_) => Ok(backup_file),
        Err(e) => {
            eprintln!("Failed to backup home directory: {}", e);
            Err(e)
        }
    }
}
