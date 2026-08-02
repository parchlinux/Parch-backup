use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const APPS_LIST_FILE: &str = "apps.txt";

/// Auto-detect available package manager: paru -> yay -> pacman
pub fn detect_package_manager() -> &'static str {
    for pm in &["paru", "yay", "pacman"] {
        if Command::new(pm)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return pm;
        }
    }
    "pacman"
}

/// List installed explicit packages using detected package manager and save to `apps.txt`.
pub fn list_installed_apps() -> io::Result<(PathBuf, usize, String)> {
    let pm = detect_package_manager();
    let output = Command::new(pm)
        .arg("-Qe")
        .stdout(Stdio::piped())
        .output()?;

    if output.status.success() {
        let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let pkg = line.split_whitespace().next().unwrap_or("").to_string();
                if pkg.is_empty() {
                    None
                } else {
                    Some(pkg)
                }
            })
            .collect();

        let count = lines.len();
        let installed_apps = lines.join("\n");

        let apps_list_path = PathBuf::from(APPS_LIST_FILE);
        let mut file = File::create(&apps_list_path)?;
        file.write_all(installed_apps.as_bytes())?;
        Ok((apps_list_path, count, pm.to_string()))
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr).to_string();
        Err(io::Error::new(io::ErrorKind::Other, error_message))
    }
}

pub fn restore_installed_apps(apps_to_install: &[String]) -> io::Result<()> {
    if apps_to_install.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No applications to restore."));
    }

    let pm = detect_package_manager();
    let mut command = Command::new(pm);
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
