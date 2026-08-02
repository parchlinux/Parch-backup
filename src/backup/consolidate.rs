use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::cli::BackupArgs;
use crate::pbar::manifest::{
    ArchiveContents, ComponentInfo, HomeInfo, KeysInfo, PbarManifest, SecurityInfo,
};
use crate::pbar::{derive_argon2_key, PbarChunkWriter, PbarHeader};
use crate::system::info::collect_system_info;

pub const PBAR_EXT: &str = "pbar";

#[derive(Debug, Clone)]
pub struct BackupComponentMeta {
    pub category: &'static str,
    pub path: PathBuf,
    pub count: usize,
    pub size_bytes: u64,
    pub extra_info: Option<String>,
}

pub fn expand_user_path(p: &str) -> PathBuf {
    if p.starts_with("~/") || p == "~" {
        if let Ok(home) = std::env::var("HOME") {
            if p == "~" {
                return PathBuf::from(home);
            } else {
                return PathBuf::from(home).join(&p[2..]);
            }
        }
    }
    PathBuf::from(p)
}

/// Consolidates individual backup files into a single `.pbar` container archive.
pub fn consolidate_backups(
    components: &[BackupComponentMeta],
    args: &BackupArgs,
) -> io::Result<PathBuf> {
    let timestamp = Utc::now().format("%Y-%m-%d-%H-%M-%S").to_string();
    let mut flags_str = String::new();
    if args.apps {
        flags_str.push('a');
    }
    if args.home {
        flags_str.push('h');
    }
    if args.keys {
        flags_str.push('k');
    }
    if args.flatpak {
        flags_str.push('f');
    }
    if args.encrypt {
        flags_str.push('e');
    }

    let archive_name = format!("backup-{}-{}.{}", timestamp, flags_str, PBAR_EXT);

    let raw_archive_dir = args.archive_path.as_deref().unwrap_or("~/Backups");
    let archive_dir = expand_user_path(raw_archive_dir);

    if !archive_dir.exists() {
        fs::create_dir_all(&archive_dir)?;
        println!("Created directory: {}", archive_dir.display());
    }

    let archive_path = archive_dir.join(&archive_name);

    // Build PbarManifest
    let sys_info = collect_system_info();

    let mut apps_info = ComponentInfo {
        included: false,
        count: 0,
        package_manager: None,
        file_path: None,
    };
    let mut flatpak_info = ComponentInfo {
        included: false,
        count: 0,
        package_manager: None,
        file_path: None,
    };
    let mut home_info = HomeInfo {
        included: false,
        uncompressed_size_bytes: 0,
        file_path: None,
    };
    let mut gpg_included = false;
    let mut ssh_included = false;

    for meta in components {
        match meta.category {
            "appsb" => {
                apps_info.included = true;
                apps_info.count = meta.count;
                apps_info.package_manager = meta.extra_info.clone();
                apps_info.file_path = Some("appsb/apps.txt".to_string());
            }
            "flatpakb" => {
                flatpak_info.included = true;
                flatpak_info.count = meta.count;
                flatpak_info.file_path = Some("flatpakb/flatpak_apps.txt".to_string());
            }
            "homeb" => {
                home_info.included = true;
                home_info.uncompressed_size_bytes = meta.size_bytes;
                home_info.file_path = Some("homeb/home_backup.tar.gz".to_string());
            }
            "gnupgb" => {
                gpg_included = true;
            }
            "sshb" => {
                ssh_included = true;
            }
            _ => {}
        }
    }

    let manifest = PbarManifest {
        format_version: "1.2".to_string(),
        created_at: Utc::now().to_rfc3339(),
        creator: "Parch Backup v0.1.0".to_string(),
        security: SecurityInfo {
            encrypted: args.encrypt,
            signed: false,
            signature_type: "None".to_string(),
            kdf: "Argon2id".to_string(),
            cipher: if args.encrypt { "AES-256-GCM" } else { "None" }.to_string(),
        },
        system_info: sys_info,
        archive_contents: ArchiveContents {
            apps: apps_info,
            flatpak: flatpak_info,
            home_dotfiles: home_info,
            keys: KeysInfo {
                included: gpg_included || ssh_included,
                gpg_keys: gpg_included,
                ssh_keys: ssh_included,
            },
            systemd_services: ComponentInfo {
                included: false,
                count: 0,
                package_manager: None,
                file_path: None,
            },
        },
    };

    let manifest_bytes = manifest.to_json_bytes().map_err(|e| {
        io::Error::new(io::ErrorKind::InvalidData, format!("Manifest error: {}", e))
    })?;

    // Prepare PbarHeader
    let header = PbarHeader::new(args.encrypt, true, manifest_bytes.len() as u32);

    let derived_key = if args.encrypt {
        if let Some(ref pass) = args.encrypt_key {
            Some(derive_argon2_key(pass.as_bytes(), &header.salt)?)
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Encryption enabled but no key provided.",
            ));
        }
    } else {
        None
    };

    let archive_file = File::create(&archive_path)?;
    let mut pbar_writer = PbarChunkWriter::new(archive_file, derived_key, header.base_nonce);

    // 1. Write PBAR Header into chunk writer stream
    header.write_to(&mut pbar_writer)?;

    // 2. Write Manifest bytes
    pbar_writer.write_all(&manifest_bytes)?;

    // 3. Write POSIX inner tarball compressed stream into payload
    {
        let enc = GzEncoder::new(&mut pbar_writer, Compression::default());
        let mut tar_builder = tar::Builder::new(enc);

        // Add manifest.json as first file in tarball
        let mut manifest_header = tar::Header::new_gnu();
        manifest_header.set_size(manifest_bytes.len() as u64);
        manifest_header.set_mode(0o644);
        manifest_header.set_cksum();
        tar_builder.append_data(&mut manifest_header, "manifest.json", &manifest_bytes[..])?;

        // Add each backup component to tarball
        for meta in components {
            let file_name = meta.path.file_name().unwrap();
            let subdir = meta.category;
            let path_in_archive = Path::new(subdir).join(file_name);

            let mut f = File::open(&meta.path)?;
            tar_builder.append_file(&path_in_archive, &mut f)?;

            // Clean up temporary component file after packing
            let _ = fs::remove_file(&meta.path);
        }

        tar_builder.finish()?;
    }

    let _final_file = pbar_writer.finish()?;

    println!("PBAR archive created successfully: {}", archive_path.display());
    Ok(archive_path)
}
