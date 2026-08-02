use crate::backup::consolidate::{self, BackupComponentMeta};
use crate::cli::BackupArgs;
use crate::events::{BackupPhase, ProgressEvent};
use crate::flatpak::flatpak;
use crate::pm::paru;
use crate::system::{home, keys};
use crate::utils::compression::ARCHIVE_EXT;
use crossbeam_channel::Sender;
use dialoguer::{Confirm, Password};
use regex::Regex;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Clean up temporary backup files in current directory
fn cleanup_backup_files() {
    let pattern = format!(r".*_backup\.{}", ARCHIVE_EXT);
    let re = Regex::new(&pattern).unwrap();

    if let Ok(entries) = fs::read_dir(".") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
                if re.is_match(file_name) || file_name == "apps.txt" || file_name == "flatpak_apps.txt" {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }
}

pub fn handle_backup_with_tx(args: &BackupArgs, tx: Option<&Sender<ProgressEvent>>) {
    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let home_path = Path::new(&home_dir);

    let mut backup_components: Vec<BackupComponentMeta> = Vec::new();

    let interrupted = Arc::new(AtomicBool::new(false));
    let interrupt_clone = Arc::clone(&interrupted);

    ctrlc::set_handler(move || {
        interrupt_clone.store(true, Ordering::SeqCst);
    })
    .ok();

    if let Some(sender) = tx {
        let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Scanning));
    }

    if args.apps {
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Packages));
        }
        backup_apps(&mut backup_components);
    }

    if args.home {
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Home));
        }
        backup_home(&mut backup_components, home_path, &args.exclude_dir, &interrupted);
    }

    if args.flatpak {
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Flatpaks));
        }
        backup_flatpak(&mut backup_components);
    }

    if args.keys {
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Keys));
        }
        backup_keys(&mut backup_components, home_path, &interrupted);
    }

    if interrupted.load(Ordering::SeqCst) {
        exit_gracefully();
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::Error("Operation canceled by user".to_string()));
        }
        return;
    }

    if !backup_components.is_empty() {
        if let Some(sender) = tx {
            let _ = sender.send(ProgressEvent::PhaseChanged(BackupPhase::Compressing));
        }
        match consolidate::consolidate_backups(&backup_components, args) {
            Ok(archive_path) => {
                println!("All backups consolidated successfully into PBAR container.");
                if let Some(sender) = tx {
                    let _ = sender.send(ProgressEvent::StatusMessage(format!(
                        "Archive saved to {}",
                        archive_path.display()
                    )));
                    let _ = sender.send(ProgressEvent::Completed);
                }
            }
            Err(e) => {
                eprintln!("Failed to consolidate backups: {}", e);
                if let Some(sender) = tx {
                    let _ = sender.send(ProgressEvent::Error(e.to_string()));
                }
            }
        }
    } else {
        // CLI interactive prompt if no flags specified
        if Confirm::new()
            .with_prompt("Do you want to backup with all functionality (apps, home, keys, flatpak)?")
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

            backup_apps(&mut backup_components);
            backup_home(&mut backup_components, home_path, &args.exclude_dir, &interrupted);
            backup_flatpak(&mut backup_components);
            backup_keys(&mut backup_components, home_path, &interrupted);

            let new_args = BackupArgs {
                archive_path: args.archive_path.clone(),
                apps: true,
                home: true,
                exclude_dir: args.exclude_dir.clone(),
                flatpak: true,
                keys: true,
                encrypt: backup_key.is_some(),
                encrypt_key: backup_key,
            };

            match consolidate::consolidate_backups(&backup_components, &new_args) {
                Ok(archive_path) => println!("Archive saved to {}", archive_path.display()),
                Err(e) => eprintln!("Failed to consolidate backups: {}", e),
            }
        } else {
            exit_gracefully();
        }
    }
}

pub fn handle_backup(args: &BackupArgs) {
    handle_backup_with_tx(args, None);
}

fn backup_apps(components: &mut Vec<BackupComponentMeta>) {
    println!("Backing up installed apps...");
    match paru::list_installed_apps() {
        Ok((file, count, pm_name)) => {
            println!("Installed apps ({}) backed up successfully.", count);
            let size_bytes = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            components.push(BackupComponentMeta {
                category: "appsb",
                path: file,
                count,
                size_bytes,
                extra_info: Some(pm_name),
            });
        }
        Err(e) => eprintln!("Failed to backup installed apps: {}", e),
    }
}

fn backup_home(
    components: &mut Vec<BackupComponentMeta>,
    home_path: &Path,
    exclude_dirs: &[String],
    interrupted: &Arc<AtomicBool>,
) {
    println!("Backing up home directory...");
    match home::backup_home(home_path, exclude_dirs, interrupted) {
        Ok(file) => {
            println!("Home directory backed up successfully.");
            let size_bytes = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            components.push(BackupComponentMeta {
                category: "homeb",
                path: file,
                count: 1,
                size_bytes,
                extra_info: None,
            });
        }
        Err(e) => {
            if e.kind() == io::ErrorKind::Interrupted {
                cleanup_backup_files();
            } else {
                eprintln!("Failed to backup home directory: {}", e);
            }
        }
    }
}

fn backup_flatpak(components: &mut Vec<BackupComponentMeta>) {
    println!("Backing up Flatpak applications...");
    match flatpak::list_installed_flatpak_apps() {
        Ok((file, count)) => {
            println!("Installed Flatpak apps ({}) backed up successfully.", count);
            let size_bytes = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            components.push(BackupComponentMeta {
                category: "flatpakb",
                path: file,
                count,
                size_bytes,
                extra_info: None,
            });
        }
        Err(e) => eprintln!("Failed to backup Flatpak apps: {}", e),
    }
}

fn backup_keys(
    components: &mut Vec<BackupComponentMeta>,
    home_path: &Path,
    interrupted: &Arc<AtomicBool>,
) {
    println!("Backing up GPG keys...");
    match keys::backup_gpg_keys(home_path, interrupted) {
        Ok(file) => {
            let size_bytes = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            components.push(BackupComponentMeta {
                category: "gnupgb",
                path: file,
                count: 1,
                size_bytes,
                extra_info: None,
            });
        }
        Err(e) => eprintln!("Failed to backup GPG keys: {}", e),
    }

    println!("Backing up SSH keys...");
    match keys::backup_ssh_keys(home_path, interrupted) {
        Ok(file) => {
            let size_bytes = fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
            components.push(BackupComponentMeta {
                category: "sshb",
                path: file,
                count: 1,
                size_bytes,
                extra_info: None,
            });
        }
        Err(e) => eprintln!("Failed to backup SSH keys: {}", e),
    }
}

fn exit_gracefully() {
    cleanup_backup_files();
    eprintln!("Operation canceled.");
}
