use flate2::read::GzDecoder;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tar::Archive;

use crate::backup::consolidate::expand_user_path;
use crate::cli::RestoreArgs;
use crate::events::{BackupPhase, ProgressEvent};
use crate::flatpak::flatpak;
use crate::pbar::{derive_argon2_key, PbarChunkReader, PbarHeader, PbarManifest};
use crate::pm::paru;
use crossbeam_channel::Sender;

pub fn handle_restore_with_tx(
    args: &RestoreArgs,
    tx: Option<&Sender<ProgressEvent>>,
) -> io::Result<()> {
    let archive_path = expand_user_path(&args.archive_path);
    if !archive_path.exists() {
        let err = format!("Archive file not found: {}", archive_path.display());
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::Error(err.clone()));
        }
        return Err(io::Error::new(io::ErrorKind::NotFound, err));
    }

    if let Some(sender) = tx {
        let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Scanning));
    }

    let mut file = fs::File::open(&archive_path)?;

    // 1. Read PBAR Header
    let header = PbarHeader::read_from(&mut file)?;

    // 2. Check Encryption Key
    let derived_key = if header.is_encrypted() {
        if !args.decrypt {
            let err = "Archive is encrypted. Please provide --decrypt and --decrypt-key.".to_string();
            if let Some(sender) = tx {
                let _ = sender.send(ProgressEvent::Error(err.clone()));
            }
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, err));
        }

        let key_str = args
            .decrypt_key
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Decryption key not provided"))?;

        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Encrypting));
        }

        Some(derive_argon2_key(key_str.as_bytes(), &header.salt)?)
    } else {
        None
    };

    // 3. Read Manifest bytes
    let manifest_size = header.manifest_size as usize;
    let mut manifest_bytes = vec![0u8; manifest_size];
    file.read_exact(&mut manifest_bytes)?;

    // Parse manifest if valid JSON
    if let Ok(manifest) = PbarManifest::from_json_slice(&manifest_bytes) {
        println!("Restoring PBAR Backup created by: {}", manifest.creator);
        println!("Distro: {}, Kernel: {}", manifest.system_info.distro, manifest.system_info.kernel);
    }

    if let Some(sender) = tx {
        let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Restoring));
    }

    // 4. Create PbarChunkReader to stream inner tarball
    let chunk_reader = PbarChunkReader::new(file, derived_key, header.base_nonce);
    let gz_decoder = GzDecoder::new(chunk_reader);
    let mut tar = Archive::new(gz_decoder);

    let mut apps_to_install = Vec::new();
    let mut flatpak_apps_to_install = Vec::new();

    for entry in tar.entries()? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        println!("Extracting {:?}", entry_path);

        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::FileProgress {
                file_name: entry_path.display().to_string(),
                bytes_processed: entry.header().size().unwrap_or(0),
            });
        }

        if entry_path == Path::new("manifest.json") {
            continue; // Already processed
        }

        let dest_path = determine_restore_path(&entry_path)?;

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if let Some(subdir) = entry_path.iter().next().and_then(|s| s.to_str()) {
            match subdir {
                "appsb" => collect_apps_list_from_entry(&mut entry, &mut apps_to_install)?,
                "flatpakb" => collect_apps_list_from_entry(&mut entry, &mut flatpak_apps_to_install)?,
                _ => {
                    if entry_path.extension() == Some(std::ffi::OsStr::new("gz"))
                        || entry_path.extension() == Some(std::ffi::OsStr::new("zst"))
                    {
                        extract_nested_tarball(&dest_path, &mut entry)?;
                    } else {
                        entry.unpack(&dest_path)?;
                    }
                }
            }
        }
    }

    if !apps_to_install.is_empty() {
        println!("Restoring {} package manager applications...", apps_to_install.len());
        if let Err(e) = paru::restore_installed_apps(&apps_to_install) {
            eprintln!("Package manager restore warning: {}", e);
        }
    }

    if !flatpak_apps_to_install.is_empty() {
        println!("Restoring {} Flatpak applications...", flatpak_apps_to_install.len());
        if let Err(e) = flatpak::restore_installed_flatpak_apps(&flatpak_apps_to_install) {
            eprintln!("Flatpak restore warning: {}", e);
        }
    }

    println!("Restore completed successfully.");
    if let Some(sender) = tx {
        let _ = sender.send(ProgressEvent::Completed);
    }

    Ok(())
}

pub fn handle_restore(args: &RestoreArgs) -> io::Result<()> {
    handle_restore_with_tx(args, None)
}

fn extract_nested_tarball(
    dest_path: &Path,
    entry: &mut tar::Entry<impl io::Read>,
) -> io::Result<()> {
    let mut tar_gz = Vec::new();
    entry.read_to_end(&mut tar_gz)?;

    let cursor = std::io::Cursor::new(tar_gz);
    let tar_gz_decoder = GzDecoder::new(cursor);
    let mut nested_tar = Archive::new(tar_gz_decoder);

    for nested_entry in nested_tar.entries()? {
        let mut nested_entry = nested_entry?;
        let nested_entry_path = nested_entry.path()?;
        let nested_dest_path = dest_path.parent().unwrap().join(nested_entry_path);

        if let Some(parent) = nested_dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        nested_entry.unpack(nested_dest_path)?;
    }

    Ok(())
}

fn determine_restore_path(entry_path: &Path) -> io::Result<PathBuf> {
    let subdir = entry_path
        .iter()
        .next()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let actual_path = entry_path.iter().skip(1).collect::<PathBuf>();

    let home_dir = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();

    let base_path = match subdir {
        "appsb" => PathBuf::from("."),
        "homeb" => home_dir,
        "flatpakb" => PathBuf::from("."),
        "gnupgb" => home_dir.join(".gnupg"),
        "sshb" => home_dir.join(".ssh"),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown subdirectory in archive: {}", subdir),
            ))
        }
    };

    Ok(base_path.join(actual_path))
}

fn collect_apps_list_from_entry<R: Read>(
    entry: &mut tar::Entry<R>,
    apps_list: &mut Vec<String>,
) -> io::Result<()> {
    let mut content = String::new();
    entry.read_to_string(&mut content)?;
    apps_list.extend(content.lines().map(|line| line.trim().to_string()).filter(|l| !l.is_empty()));
    Ok(())
}
