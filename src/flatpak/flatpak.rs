use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const FLATPAK_APPS_LIST_FILE: &str = "flatpak_apps.txt";

/// Check if Flatpak is installed on the system.
pub fn is_flatpak_installed() -> bool {
    Command::new("flatpak")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// List installed Flatpak application IDs and save to a file.
pub fn list_installed_flatpak_apps() -> Result<(PathBuf, usize), io::Error> {
    if !is_flatpak_installed() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Flatpak is not installed."));
    }

    let output = Command::new("flatpak")
        .arg("list")
        .arg("--app")
        .arg("--columns=app")
        .stdout(Stdio::piped())
        .output()?;

    if output.status.success() {
        let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            return Err(io::Error::new(io::ErrorKind::Other, "No Flatpak applications found."));
        }

        let count = lines.len();
        let installed_apps = lines.join("\n");

        let flatpak_list_path = PathBuf::from(FLATPAK_APPS_LIST_FILE);
        let mut file = File::create(&flatpak_list_path)?;
        file.write_all(installed_apps.as_bytes())?;
        Ok((flatpak_list_path, count))
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, error_message))
    }
}

/// Restore installed Flatpak applications.
pub fn restore_installed_flatpak_apps(apps_to_install: &[String]) -> io::Result<()> {
    if apps_to_install.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No applications to restore."));
    }

    let mut command = Command::new("flatpak");
    command.arg("install");
    command.arg("-y");
    command.args(apps_to_install);

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
