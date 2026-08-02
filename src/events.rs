use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackupPhase {
    Scanning,
    Packages,
    Flatpaks,
    Home,
    Keys,
    Compressing,
    Encrypting,
    Restoring,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProgressEvent {
    Started { total_files: usize, total_bytes: u64 },
    FileProgress { file_name: String, bytes_processed: u64 },
    PhaseChanged(BackupPhase),
    StatusMessage(String),
    Completed,
    Error(String),
}
