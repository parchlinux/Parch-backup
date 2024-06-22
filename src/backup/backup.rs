use crate::backup::consolidate;
use crate::cli::BackupArgs;
use crate::flatpak::flatpak;
use crate::pm::paru;
use crate::system::{home, keys};
use std::path::Path;

pub fn handle_backup(args: &BackupArgs) {
    if !args.apps && !args.home && !args.flatpak && !args.keys {
        eprintln!("No backup options specified. Please provide at least one backup option.");
        return;
    }

    let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
    let home_path = Path::new(&home_dir);

    let mut backup_files = Vec::new();

    if args.apps {
        println!("Backing up installed apps...");
        match paru::list_installed_apps() {
            Ok(file) => {
                println!("Installed apps backed up successfully.");
                backup_files.push(("appsb", file));
            },
            Err(e) => eprintln!("Failed to backup installed apps: {}", e),
        }
    }

    if args.home {
        println!("Backing up home directory...");
        match home::backup_home(home_path) {
            Ok(file) => {
                println!("Home directory backed up successfully.");
                backup_files.push(("homeb", file));
            },
            Err(e) => eprintln!("Failed to backup home directory: {}", e),
        }
    }

    if args.flatpak {
        println!("Backing up Flatpak applications...");
        match flatpak::list_installed_flatpak_apps() {
            Ok(file) => {
                println!("Installed Flatpak apps backed up successfully.");
                backup_files.push(("flatpakb", file));
            },
            Err(e) => eprintln!("Failed to backup Flatpak apps: {}", e),
        }
    }

    if args.keys {
        println!("Backing up GPG keys...");
        match keys::backup_gpg_keys(home_path) {
            Ok(file) => backup_files.push(("gnupgb", file)),
            Err(e) => eprintln!("Failed to backup GPG keys: {}", e),
        }

        println!("Backing up SSH keys...");
        match keys::backup_ssh_keys(home_path) {
            Ok(file) => backup_files.push(("sshb", file)),
            Err(e) => eprintln!("Failed to backup SSH keys: {}", e),
        }
    }

    if !backup_files.is_empty() {
        match consolidate::consolidate_backups(&backup_files, args) {
            Ok(_) => println!("All backups consolidated successfully."),
            Err(e) => eprintln!("Failed to consolidate backups: {}", e),
        }
    } else {
        eprintln!("No backups to consolidate.");
    }
}
