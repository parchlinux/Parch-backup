use crate::backup::consolidate;
use crate::cli::BackupArgs;
use crate::flatpak::flatpak;
use crate::pm::paru;
use crate::system::{home, keys};
use crate::utils::compression::ARCHIVE_EXT;
use dialoguer::{Confirm, Password};
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Clean up backup files matching the pattern *_backup.EXTENSION in the current directory.
fn cleanup_backup_files() {
    let pattern = format!(r".*_backup\.{}", ARCHIVE_EXT);
    let re = Regex::new(&pattern).unwrap();

    for entry in fs::read_dir(".").unwrap() {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if re.is_match(file_name) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

pub fn handle_backup(args: &BackupArgs) {
    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let home_path = Path::new(&home_dir);

    let mut backup_files = Vec::new();

    // Atomic flag to indicate if the process was interrupted.
    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupt_clone = Arc::clone(&interrupted);

    ctrlc::set_handler(move || {
        interrupt_clone.store(true, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    if args.apps {
        backup_apps(&mut backup_files);
    }

    if args.home {
        backup_home(
            &mut backup_files,
            home_path,
            &args.exclude_dir,
            &interrupted,
        );
    }

    if args.flatpak {
        backup_flatpak(&mut backup_files);
    }

    if args.keys {
        backup_keys(&mut backup_files, home_path, &interrupted);
    }

    // Check for interruption after initial backups.
    if interrupted.load(Ordering::SeqCst) {
        exit_gracefully();
        return;
    }

    if !backup_files.is_empty() {
        match consolidate::consolidate_backups(&backup_files, args) {
            Ok(_) => println!("All backups consolidated successfully."),
            Err(e) => eprintln!("Failed to consolidate backups: {}", e),
        }
        return;
    } else {
        // Prompt the user for additional actions
        if Confirm::new()
            .with_prompt(
                "Do you want to backup with all functionality (apps, home, keys, flatpak)?",
            )
            .interact()
            .unwrap_or(false)
        {
            let backup_key = if Confirm::new()
                .with_prompt("Do you want to encrypt the backup?")
                .interact()
                .unwrap_or(false)
            {
                Some(
                    Password::new()
                        .with_prompt("Enter the encryption key")
                        .with_confirmation("Confirm the encryption key", "Keys mismatch!")
                        .interact()
                        .unwrap(),
                )
            } else {
                None
            };
            // Backup with all functionality
            backup_apps(&mut backup_files);
            backup_home(
                &mut backup_files,
                home_path,
                &args.exclude_dir,
                &interrupted,
            );
            backup_flatpak(&mut backup_files);
            backup_keys(&mut backup_files, home_path, &interrupted);

            if let Some(key) = backup_key {
                match consolidate::consolidate_backups(
                    &backup_files,
                    &BackupArgs {
                        archive_path: args.archive_path.clone(),
                        apps: true,
                        home: true,
                        exclude_dir: args.exclude_dir.clone(),
                        flatpak: true,
                        keys: true,
                        encrypt: true,
                        encrypt_key: Some(key),
                        ..*args
                    },
                ) {
                    Ok(_) => println!("All backups consolidated successfully."),
                    Err(e) => eprintln!("Failed to consolidate backups: {}", e),
                }
            } else {
                match consolidate::consolidate_backups(
                    &backup_files,
                    &BackupArgs {
                        archive_path: args.archive_path.clone(),
                        apps: true,
                        home: true,
                        exclude_dir: args.exclude_dir.clone(),
                        flatpak: true,
                        keys: true,
                        encrypt: false,
                        encrypt_key: None,
                        ..*args
                    },
                ) {
                    Ok(_) => println!("All backups consolidated successfully."),
                    Err(e) => eprintln!("Failed to consolidate backups: {}", e),
                }
            }
        } else {
            exit_gracefully();
        }
    }
}
fn backup_apps(backup_files: &mut Vec<(&str, PathBuf)>) {
    println!("Backing up installed apps...");
    match paru::list_installed_apps() {
        Ok(file) => {
            println!("Installed apps backed up successfully.");
            backup_files.push(("appsb", file));
        }
        Err(e) => eprintln!("Failed to backup installed apps: {}", e),
    }
}
fn backup_home(
    backup_files: &mut Vec<(&str, PathBuf)>,
    home_path: &Path,
    exclude_dirs: &[String],
    interrupted: &Arc<AtomicBool>,
) {
    println!("Backing up home directory...");
    match home::backup_home(home_path, &exclude_dirs, &interrupted) {
        Ok(file) => {
            println!("Home directory backed up successfully.");
            backup_files.push(("homeb", file));
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                cleanup_backup_files();
                return;
            } else {
                eprintln!("Failed to backup home directory: {}", e);
                return;
            }
        }
    }
}
fn backup_flatpak(backup_files: &mut Vec<(&str, PathBuf)>) {
    println!("Backing up Flatpak applications...");
    match flatpak::list_installed_flatpak_apps() {
        Ok(file) => {
            println!("Installed Flatpak apps backed up successfully.");
            backup_files.push(("flatpakb", file));
        }
        Err(e) => eprintln!("Failed to backup Flatpak apps: {}", e),
    }
}
fn backup_keys(
    backup_files: &mut Vec<(&str, PathBuf)>,
    home_path: &Path,
    interrupted: &Arc<AtomicBool>,
) {
    println!("Backing up GPG keys...");
    match keys::backup_gpg_keys(home_path, &interrupted) {
        Ok(file) => backup_files.push(("gnupgb", file)),
        Err(e) => eprintln!("Failed to backup GPG keys: {}", e),
    }

    println!("Backing up SSH keys...");
    match keys::backup_ssh_keys(home_path, &interrupted) {
        Ok(file) => backup_files.push(("sshb", file)),
        Err(e) => eprintln!("Failed to backup SSH keys: {}", e),
    }
}
fn exit_gracefully() {
    cleanup_backup_files();
    eprintln!("Operation canceled.");
}
