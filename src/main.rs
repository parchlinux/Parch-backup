pub mod backup;
pub mod cli;
pub mod flatpak;
pub mod pm;
pub mod restore;
pub mod system;
pub mod utils;
use crate::backup::backup::handle_backup;
use crate::restore::restore::handle_restore;
use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Backup(args) => {
            handle_backup(&args);
        }
        Commands::Restore(args) => {
            let result = handle_restore(&args);
            if let Err(e) = result {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        // Commands::Schedule(args) => {
        //     let result = system::schedule::schedule_backup(&args);
        //     if let Err(e) = result {
        //         eprintln!("Error: {}", e);
        //         std::process::exit(1);
        //     }
        // }
    }
}
