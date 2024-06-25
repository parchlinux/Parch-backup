use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use walkdir::WalkDir;

pub static ARCHIVE_EXT: &str = "tar.gz";

pub fn compress_directory<P: AsRef<Path>>(
    source_dir: P,
    target_file: P,
    exclude_dirs: Option<&[String]>,
    should_stop: &AtomicBool,
) -> io::Result<()> {
    let tar_gz = File::create(target_file)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let source_dir = source_dir.as_ref();
    let exclude_paths: Vec<PathBuf> = exclude_dirs
        .unwrap_or(&[])
        .iter()
        .map(|d| source_dir.join(d))
        .collect();

    for entry in WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        if should_stop.load(Ordering::SeqCst) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Operation canceled"));
        }

        let entry_path = entry.path();

        // Skip directories that need to be excluded
        if exclude_paths.iter().any(|d| entry_path.starts_with(d)) {
            continue;
        }

        let path_in_archive = match entry_path.strip_prefix(source_dir) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if path_in_archive.components().count() == 0 {
            // Skip empty paths
            continue;
        }

        if entry.file_type().is_dir() {
            tar.append_dir(path_in_archive, entry_path)?;
        } else {
            match File::open(entry_path) {
                Ok(mut file) => {
                    tar.append_file(path_in_archive, &mut file)?;
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    eprintln!("Failed to backup file: {} - {}", entry_path.display(), e);
                    continue; // Skip not found files and continue the loop
                }
                Err(e) => return Err(e),
            }
        }
    }

    tar.finish()?;
    Ok(())
}

/// Create a compressed tar archive (tar.gz) from specified files.
pub fn create_tar_gz_archive<P: AsRef<Path>>(
    archive_name: P,
    backup_files: &[(String, PathBuf)],
) -> io::Result<()> {
    // Create the archive file
    let archive_file = File::create(&archive_name)?;
    let enc = GzEncoder::new(archive_file, Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Add each backup file to the archive and remove original files
    for (subdir, file) in backup_files {
        let path_in_archive = Path::new(subdir).join(file.file_name().unwrap());
        tar.append_path_with_name(file, path_in_archive)?;
        std::fs::remove_file(file)?;
    }

    // Finalize the archive
    tar.finish()?;

    Ok(())
}
/// Opens and decodes a tar.gz archive file.
pub fn open_and_decode_archive<P: AsRef<Path>>(
    archive_path: P,
) -> io::Result<tar::Archive<flate2::read::GzDecoder<File>>> {
    // Open the archive file
    let archive_file = File::open(archive_path)?;
    let archive_decoder = flate2::read::GzDecoder::new(archive_file);
    let tar = tar::Archive::new(archive_decoder);
    Ok(tar)
}
