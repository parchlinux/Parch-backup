use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    author = "DanielcoderX",
    version = "1.0",
    about = "ParchBackup",
    long_about = "The ParchLinux Backup Application is a utility designed to simplify the backup
process for ParchLinux users. It provides a comprehensive solution for backing
up installed applications, home directory, Flatpak packages (optional), and GPG
and SSH keys."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Backup functionality
    Backup(BackupArgs),
    /// Restore functionality
    Restore(RestoreArgs),
    // Schedule(ScheduleArgs),
}

#[derive(Args)]
pub struct BackupArgs {
    /// Backup archive location
    #[arg(long, help = "Backup archive location", default_value = "/home/backup")]
    pub archive_path: Option<String>,
    /// Backup installed apps names
    #[arg(long, help = "Backup installed apps names")]
    pub apps: bool,
    /// Backup home directory
    #[arg(long, help = "Backup home directory")]
    pub home: bool,
    /// Execluded directories from backup
    #[arg(long, help = "Excluded directories from backup", num_args(1..))]
    pub exclude_dir: Vec<String>,
    /// Backup flatpak applications
    #[arg(long, help = "Backup flatpak applications")]
    pub flatpak: bool,
    /// Backup keys
    #[arg(long, help = "Backup keys")]
    pub keys: bool,
    /// Use Encryption
    #[arg(
        long,
        help = "Use Encryption, pass the password",
        requires = "encrypt_key"
    )]
    pub encrypt: bool,
    /// Encryption key
    #[arg(long, help = "Encryption key", requires = "encrypt")]
    pub encrypt_key: Option<String>,
}

#[derive(Args)]
pub struct RestoreArgs {
    /// Archive path
    #[arg(help = "Archive path")]
    pub archive_path: String,
    /// Use Decryption
    #[arg(
        long,
        help = "Use Decryption, pass the password",
        requires = "decrypt_key"
    )]
    pub decrypt: bool,
    /// Decryption key
    #[arg(long, help = "Decryption key", requires = "decrypt")]
    pub decrypt_key: Option<String>,
}

// #[derive(Args)]
// pub struct ScheduleArgs {
//     /// Cron expression for scheduling
//     /// sec  min   hour   day of month   month   day of week   year
//     #[arg(
//         short,
//         long,
//         help = "Cron expression for scheduling backup functionality: \nExample: 'sec  min   hour   day of month   month   day of week   year'\n"
//     )]
//     pub cron: String,
//     /// Backup installed apps names
//     #[arg(long, help = "Backup installed apps names")]
//     pub apps: bool,
//     /// Backup home directory
//     #[arg(long, help = "Backup home directory")]
//     pub home: bool,
//     /// Execluded directories from backup
//     #[arg(long, help = "Excluded directories from backup", num_args(1..))]
//     pub exclude_dir: Vec<String>,
//     /// Backup flatpak applications
//     #[arg(long, help = "Backup flatpak applications")]
//     pub flatpak: bool,
//     /// Backup keys
//     #[arg(long, help = "Backup keys")]
//     pub keys: bool,
//     /// Use Encryption
//     #[arg(
//         long,
//         help = "Use Encryption, pass the password",
//         requires = "encrypt_key"
//     )]
//     pub encrypt: bool,
//     /// Encryption key
//     #[arg(long, help = "Encryption key", requires = "encrypt")]
//     pub encrypt_key: Option<String>,
// }

pub fn parse_cli() -> Cli {
    Cli::parse()
}
