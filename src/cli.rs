use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    author = "DanielcoderX",
    version = "1.0",
    about = "ParchBackup",
    long_about = "Koonam goshade"
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
    /// Schedule backup
    Schedule(ScheduleArgs),
}

#[derive(Args)]
pub struct BackupArgs {
    /// Backup installed apps names
    #[arg(long, help = "Backup installed apps names")]
    pub apps: bool,
    /// Backup home directory
    #[arg(long, help = "Backup home directory")]
    pub home: bool,
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


#[derive(Args)]
pub struct ScheduleArgs {
    /// Cron expression for scheduling
    #[arg(short, long, help = "Cron expression for scheduling")]
    pub cron: String,
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
