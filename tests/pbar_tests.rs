use parch_backup::pbar::header::{PbarHeader, PBAR_MAGIC};
use parch_backup::pbar::manifest::{
    ArchiveContents, ComponentInfo, HomeInfo, KeysInfo, PbarManifest, SecurityInfo, SystemInfo,
};
use parch_backup::pbar::stream::{derive_argon2_key, PbarChunkReader, PbarChunkWriter};
use std::io::{Read, Write};

#[test]
fn test_header_serialization_roundtrip() {
    let original = PbarHeader::new(true, true, 512);
    let mut buffer = Vec::new();
    original.write_to(&mut buffer).expect("Write header");

    assert_eq!(&buffer[0..4], PBAR_MAGIC);

    let parsed = PbarHeader::read_from(&buffer[..]).expect("Read header");
    assert_eq!(parsed.version, original.version);
    assert_eq!(parsed.feature_flags, original.feature_flags);
    assert_eq!(parsed.salt, original.salt);
    assert_eq!(parsed.base_nonce, original.base_nonce);
    assert_eq!(parsed.manifest_size, 512);
    assert!(parsed.is_encrypted());
    assert!(parsed.is_compressed());
}

#[test]
fn test_manifest_json_serialization() {
    let manifest = PbarManifest {
        format_version: "1.2".to_string(),
        created_at: "2026-08-02T12:00:00Z".to_string(),
        creator: "Parch Backup v0.1.0".to_string(),
        security: SecurityInfo {
            encrypted: true,
            signed: false,
            signature_type: "None".to_string(),
            kdf: "Argon2id".to_string(),
            cipher: "AES-256-GCM".to_string(),
        },
        system_info: SystemInfo {
            distro: "Parch Linux".to_string(),
            release: "2026.07".to_string(),
            kernel: "6.10.2".to_string(),
            arch: "x86_64".to_string(),
            hostname: "parch-desktop".to_string(),
        },
        archive_contents: ArchiveContents {
            apps: ComponentInfo {
                included: true,
                count: 100,
                package_manager: Some("paru".to_string()),
                file_path: Some("appsb/apps.txt".to_string()),
            },
            flatpak: ComponentInfo {
                included: true,
                count: 5,
                package_manager: None,
                file_path: Some("flatpakb/flatpak_apps.txt".to_string()),
            },
            home_dotfiles: HomeInfo {
                included: true,
                uncompressed_size_bytes: 1048576,
                file_path: Some("homeb/home_backup.tar.gz".to_string()),
            },
            keys: KeysInfo {
                included: true,
                gpg_keys: true,
                ssh_keys: true,
            },
            systemd_services: ComponentInfo {
                included: false,
                count: 0,
                package_manager: None,
                file_path: None,
            },
        },
    };

    let json_bytes = manifest.to_json_bytes().expect("To JSON");
    let parsed_manifest = PbarManifest::from_json_slice(&json_bytes).expect("From JSON");

    assert_eq!(parsed_manifest.format_version, "1.2");
    assert_eq!(parsed_manifest.system_info.distro, "Parch Linux");
    assert_eq!(parsed_manifest.archive_contents.apps.count, 100);
}

#[test]
fn test_streaming_encryption_roundtrip() {
    let passphrase = b"secret-passphrase";
    let salt = [42u8; 16];
    let base_nonce = [7u8; 12];

    let key = derive_argon2_key(passphrase, &salt).expect("Argon2 KDF");

    let sample_data = b"Hello, Parch Linux PBAR container format streaming test data! ".repeat(1000);

    let mut output_buf = Vec::new();
    {
        let mut writer = PbarChunkWriter::new(&mut output_buf, Some(key), base_nonce);
        writer.write_all(&sample_data).expect("Write data");
        writer.finish().expect("Finish writer");
    }

    let mut reader = PbarChunkReader::new(&output_buf[..], Some(key), base_nonce);
    let mut decrypted_data = Vec::new();
    reader.read_to_end(&mut decrypted_data).expect("Read decrypted");

    assert_eq!(sample_data, decrypted_data);
}
