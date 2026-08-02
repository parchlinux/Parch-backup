use crate::pbar::manifest::SystemInfo;
use std::fs;
use std::process::Command;

pub fn collect_system_info() -> SystemInfo {
    let mut distro = "Parch Linux".to_string();
    let mut release = "Unknown".to_string();

    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("NAME=") {
                distro = line.trim_start_matches("NAME=").trim_matches('"').to_string();
            } else if line.starts_with("VERSION_ID=") {
                release = line.trim_start_matches("VERSION_ID=").trim_matches('"').to_string();
            }
        }
    }

    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "Linux".to_string());

    let arch = Command::new("uname")
        .arg("-m")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "x86_64".to_string());

    let hostname = Command::new("hostname")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "parch-desktop".to_string());

    SystemInfo {
        distro,
        release,
        kernel,
        arch,
        hostname,
    }
}
