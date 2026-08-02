use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PbarManifest {
    pub format_version: String,
    pub created_at: String,
    pub creator: String,
    pub security: SecurityInfo,
    pub system_info: SystemInfo,
    pub archive_contents: ArchiveContents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub encrypted: bool,
    pub signed: bool,
    pub signature_type: String,
    pub kdf: String,
    pub cipher: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub distro: String,
    pub release: String,
    pub kernel: String,
    pub arch: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveContents {
    pub apps: ComponentInfo,
    pub flatpak: ComponentInfo,
    pub home_dotfiles: HomeInfo,
    pub keys: KeysInfo,
    pub systemd_services: ComponentInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub included: bool,
    #[serde(default)]
    pub count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeInfo {
    pub included: bool,
    pub uncompressed_size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysInfo {
    pub included: bool,
    pub gpg_keys: bool,
    pub ssh_keys: bool,
}

impl PbarManifest {
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    pub fn from_json_slice(slice: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(slice)
    }
}
