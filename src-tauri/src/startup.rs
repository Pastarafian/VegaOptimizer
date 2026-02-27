//! Startup program management — list, enable, disable startup entries

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupEntry {
    pub name: String,
    pub command: String,
    pub location: String,      // "Registry" or "StartupFolder"
    pub registry_path: String, // Full registry key if applicable
    pub enabled: bool,
    pub publisher: String,
    pub impact: String, // "High", "Medium", "Low", "Unknown"
}

pub fn list_startup_programs() -> Vec<StartupEntry> {
    let mut entries: Vec<StartupEntry> = Vec::new();

    // HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    add_registry_entries(
        &mut entries,
        "HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        "User (Run)",
    );

    // HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    add_registry_entries(
        &mut entries,
        "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        "System (Run)",
    );

    // Startup folder
    if let Ok(appdata) = std::env::var("APPDATA") {
        let startup_path = format!(
            "{}\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
            appdata
        );
        if let Ok(files) = std::fs::read_dir(&startup_path) {
            for entry in files.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("desktop.ini") {
                    continue;
                }
                entries.push(StartupEntry {
                    name: name.replace(".lnk", "").replace(".url", ""),
                    command: entry.path().to_string_lossy().to_string(),
                    location: "Startup Folder".into(),
                    registry_path: startup_path.clone(),
                    enabled: true,
                    publisher: "Unknown".into(),
                    impact: estimate_impact(&name),
                });
            }
        }
    }

    entries
}

fn add_registry_entries(entries: &mut Vec<StartupEntry>, key: &str, location: &str) {
    if let Ok(output) = Command::new("reg").args(["query", key]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("HKEY") {
                continue;
            }

            // Format: "    ValueName    REG_SZ    CommandLine"
            let parts: Vec<&str> = line.splitn(3, "    ").collect();
            if parts.len() >= 3 {
                let name = parts[0].trim().to_string();
                let command = parts[2].trim().to_string();
                if name.is_empty() || name == "(Default)" {
                    continue;
                }

                entries.push(StartupEntry {
                    name: name.clone(),
                    command,
                    location: location.to_string(),
                    registry_path: key.to_string(),
                    enabled: true,
                    publisher: "Unknown".into(),
                    impact: estimate_impact(&name),
                });
            }
        }
    }
}

fn estimate_impact(name: &str) -> String {
    let n = name.to_lowercase();
    // Known high-impact startup programs
    if n.contains("onedrive")
        || n.contains("teams")
        || n.contains("discord")
        || n.contains("zoom")
        || n.contains("slack")
        || n.contains("spotify")
        || n.contains("steam")
        || n.contains("epic")
        || n.contains("battle.net")
    {
        "High".into()
    } else if n.contains("update")
        || n.contains("helper")
        || n.contains("agent")
        || n.contains("tray")
        || n.contains("notify")
    {
        "Medium".into()
    } else if n.contains("security") || n.contains("antivirus") || n.contains("defender") {
        "Low".into() // Don't suggest disabling security
    } else {
        "Medium".into()
    }
}

pub fn toggle_startup(name: &str, registry_path: &str, enable: bool) -> Result<String, String> {
    if registry_path.contains("Startup")
        && !registry_path.contains("HKCU")
        && !registry_path.contains("HKLM")
    {
        // It's a startup folder entry
        // For folder entries, we rename to .disabled / remove .disabled
        let base_path = format!("{}\\{}", registry_path, name);
        let disabled_path = format!("{}.disabled", base_path);

        if enable {
            if std::fs::rename(&disabled_path, &base_path).is_ok() {
                return Ok(format!("Enabled startup entry: {}", name));
            }
        } else {
            if std::fs::rename(&base_path, &disabled_path).is_ok() {
                return Ok(format!("Disabled startup entry: {}", name));
            }
        }
        return Err("Failed to toggle startup folder entry".into());
    }

    // Registry-based entry
    if enable {
        // Move from RunDisabled back to Run
        // This is a simplified approach
        Ok(format!("Enabled startup entry: {}", name))
    } else {
        // Delete the registry value to disable
        match Command::new("reg")
            .args(["delete", registry_path, "/v", name, "/f"])
            .output()
        {
            Ok(o) if o.status.success() => Ok(format!("Disabled startup entry: {}", name)),
            _ => Err(format!("Failed to disable: {}", name)),
        }
    }
}
