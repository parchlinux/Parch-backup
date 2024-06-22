use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const APPS_LIST_FILE: &str = "apps.txt";

/// List installed apps using the `paru` package manager and save to `apps.txt`.
pub fn list_installed_apps() -> io::Result<PathBuf> {
    let output = Command::new("paru")
        .arg("-Qe")
        .stdout(Stdio::piped())
        .output()?;

    if output.status.success() {
        let installed_apps = String::from_utf8_lossy(&output.stdout).to_string();
        let apps_list_path = PathBuf::from(APPS_LIST_FILE);
        let mut file = File::create(&apps_list_path)?;
        file.write_all(installed_apps.as_bytes())?;
        Ok(apps_list_path)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, error_message))
    }
}
pub fn restore_installed_apps(apps_to_install: &[String]) -> io::Result<()> {
    if apps_to_install.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No applications to restore."));
    }

    let mut command = Command::new("paru");
    command.arg("-S");
    command.args(apps_to_install);
    command.arg("--noconfirm");

    let output = command
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(io::Error::new(io::ErrorKind::Other, error_message));
    }

    Ok(())
}
