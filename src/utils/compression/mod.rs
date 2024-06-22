use std::fs::File;
use std::io;
use std::path::Path;
use flate2::write::GzEncoder;
use flate2::Compression;

pub const TAR_GZ: &str = "tar.gz";
/// Compresses the contents of a directory into a tar.gz file.
pub fn compress_directory<P: AsRef<Path>>(source_dir: P, target_file: P) -> io::Result<()> {
    let tar_gz = File::create(target_file)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_dir_all(".", source_dir)?;
    tar.finish()?;
    Ok(())
}
