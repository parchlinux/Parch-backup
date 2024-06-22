use crate::cli::RestoreArgs;
use crate::flatpak::flatpak;
use crate::pm::paru;
use crate::utils::security;
use crate::utils::security::CryptoError;
use flate2::read::GzDecoder;
use std::fs;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

/// Restores files from a backup archive.
pub fn handle_restore(args: &RestoreArgs) -> io::Result<()> {
    let archive_path_for_restore = PathBuf::from(&args.archive_path); // Start with original path
    if args.archive_path.contains("e") {
        // Decrypt the archive if decryption is enabled
        if args.decrypt {
            let key = args
                .decrypt_key
                .as_ref()
                .expect("Decryption key not provided");
            if let Err(e) = security::decrypt_file(&archive_path_for_restore, key.as_bytes()) {
                match e {
                    CryptoError::FileRead(_) => {
                        eprintln!("Failed to read the archive file: {:?}", e)
                    }
                    CryptoError::FileWrite(_) => {
                        eprintln!("Failed to write the decrypted file: {:?}", e)
                    }
                    _ => {},
                }
            }
        } else {
            eprintln!("Decryption is not enabled. Please enable decryption by passing the --decrypt flag.");
            return Ok(());
        }
    }
    // Open the archive file
    let archive_file = fs::File::open(&archive_path_for_restore)?;
    let archive_decoder = GzDecoder::new(archive_file);
    let mut tar = Archive::new(archive_decoder);

    // In-memory collections for apps and flatpak apps
    let mut apps_to_install = Vec::new();
    let mut flatpak_apps_to_install = Vec::new();

    // Iterate over each entry in the archive
    for entry in tar.entries()? {
        let mut entry = entry?;

        // Get the path of the entry in the archive
        let entry_path = entry.path()?;
        println!("Restoring {:?}", entry_path);

        // Determine the destination path where the entry will be restored
        let dest_path = determine_restore_path(entry_path.to_path_buf(), &args)?;

        // Create parent directories if they don't exist
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Collect applications to install without writing to disk
        if let Some(subdir) = entry_path.iter().next().and_then(|s| s.to_str()) {
            match subdir {
                "appsb" => collect_apps_list_from_entry(&mut entry, &mut apps_to_install)?,
                "flatpakb" => {
                    collect_apps_list_from_entry(&mut entry, &mut flatpak_apps_to_install)?
                }
                _ => {
                    // Extract other entries to the destination path
                    if entry_path.extension() == Some(std::ffi::OsStr::new("gz")) {
                        // Handle nested tarball
                        extract_nested_tarball(&dest_path, &mut entry)?;
                    } else {
                        // Otherwise, unpack as usual
                        entry.unpack(&dest_path)?;
                    }
                }
            }
        }
    }

    // Restore installed applications
    if !apps_to_install.is_empty() {
        paru::restore_installed_apps(&apps_to_install)?;
    } else {
        eprintln!("No Paru apps found in backup.");
    }

    // Restore installed Flatpak applications
    if !flatpak_apps_to_install.is_empty() {
        flatpak::restore_installed_flatpak_apps(&flatpak_apps_to_install)?;
    } else {
        eprintln!("No Flatpak apps found in backup.");
    }

    println!("Restore completed successfully.");

    Ok(())
}

/// Extract a nested tarball within the archive.
fn extract_nested_tarball(
    dest_path: &Path,
    entry: &mut tar::Entry<impl io::Read>,
) -> io::Result<()> {
    let mut tar_gz = Vec::new();
    entry.read_to_end(&mut tar_gz)?;

    // Create a reader for the nested tarball
    let cursor = std::io::Cursor::new(tar_gz);
    let tar_gz_decoder = flate2::read::GzDecoder::new(cursor);
    let mut nested_tar = tar::Archive::new(tar_gz_decoder);

    // Iterate over each entry in the nested tarball
    for nested_entry in nested_tar.entries()? {
        let mut nested_entry = nested_entry?;
        let nested_entry_path = nested_entry.path()?;
        let nested_dest_path = dest_path.parent().unwrap().join(nested_entry_path);

        // Create parent directories if they don't exist
        if let Some(parent) = nested_dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Extract the nested entry to the destination path
        nested_entry.unpack(nested_dest_path)?;
    }

    Ok(())
}

/// Determine the restore path for an entry based on backup arguments.
fn determine_restore_path(entry_path: PathBuf, _args: &RestoreArgs) -> io::Result<PathBuf> {
    // Extract subdirectory and actual file path from the entry path
    let subdir = entry_path
        .iter()
        .next()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let actual_path = entry_path.iter().skip(1).collect::<PathBuf>();

    // Determine the base path for restoration
    let base_path = match subdir {
        "appsb" => PathBuf::from("."), // Restore apps list in current directory
        "homeb" => std::env::var("HOME").map(PathBuf::from).unwrap_or_default(),
        "flatpakb" => PathBuf::from("."), // Restore flatpak list in current directory
        "gnupgb" => std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".gnupg"))
            .unwrap_or_default(),
        "sshb" => std::env::var("HOME")
            .map(|home| PathBuf::from(home).join(".ssh"))
            .unwrap_or_default(),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unknown subdirectory in archive",
            ))
        }
    };

    // Combine base path with actual path
    Ok(base_path.join(actual_path))
}

/// Collect applications list from an entry without writing to disk.
fn collect_apps_list_from_entry<R: Read>(
    entry: &mut tar::Entry<R>,
    apps_list: &mut Vec<String>,
) -> io::Result<()> {
    let mut content = String::new();
    entry.read_to_string(&mut content)?;
    apps_list.extend(content.lines().map(|line| line.to_string()));
    Ok(())
}
