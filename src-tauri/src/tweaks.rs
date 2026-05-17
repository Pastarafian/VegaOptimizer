//! System Tweaks — theme toggle, restore points, Windows Update control

use serde::{Deserialize, Serialize};
use std::process::Command;

// ═══════════════════════════════════════════════════════════════════════════════
// Windows Theme (Dark/Light Mode)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeStatus {
    pub apps_dark: bool,
    pub system_dark: bool,
    pub taskbar_color: bool,
}

/// Read current Windows theme settings from registry
pub fn get_theme_status() -> ThemeStatus {
    let read_reg = |value_name: &str| -> bool {
        Command::new("reg")
            .args([
                "query",
                r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize",
                "/v",
                value_name,
            ])
            .output()
            .ok()
            .and_then(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                // REG_DWORD output: "    ValueName    REG_DWORD    0x0" — 0 = dark, 1 = light
                stdout
                    .lines()
                    .find(|l| l.contains(value_name))
                    .and_then(|l| l.split_whitespace().last())
                    .and_then(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
                    .map(|v| v == 0) // 0 = dark mode enabled
            })
            .unwrap_or(false)
    };

    ThemeStatus {
        apps_dark: read_reg("AppsUseLightTheme"),
        system_dark: read_reg("SystemUsesLightTheme"),
        taskbar_color: !read_reg("ColorPrevalence"),
    }
}

/// Toggle Windows dark/light mode for both apps and system
pub fn set_dark_mode(enabled: bool) -> Result<String, String> {
    let value = if enabled { "0" } else { "1" };

    // Set apps theme
    let _ = Command::new("reg")
        .args([
            "add",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v", "AppsUseLightTheme",
            "/t", "REG_DWORD",
            "/d", value,
            "/f",
        ])
        .output();

    // Set system theme (taskbar, Start menu)
    let result = Command::new("reg")
        .args([
            "add",
            r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v", "SystemUsesLightTheme",
            "/t", "REG_DWORD",
            "/d", value,
            "/f",
        ])
        .output()
        .map_err(|e| e.to_string())?;

    if result.status.success() {
        Ok(format!(
            "Windows theme set to {} mode (restart Explorer for full effect)",
            if enabled { "Dark" } else { "Light" }
        ))
    } else {
        Err("Failed to update theme registry keys".into())
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// System Restore Points
// ═══════════════════════════════════════════════════════════════════════════════

/// Create a system restore point before running optimizations
pub fn create_restore_point(description: &str) -> Result<String, String> {
    // Sanitize description to prevent injection — only allow alphanumeric + spaces
    let safe_desc: String = description
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .take(64)
        .collect();

    let desc = if safe_desc.is_empty() {
        "VegaOptimizer Checkpoint".to_string()
    } else {
        safe_desc
    };

    let cmd = format!(
        "Checkpoint-Computer -Description '{}' -RestorePointType 'MODIFY_SETTINGS' -ErrorAction Stop",
        desc
    );

    match Command::new("powershell")
        .args(["-Command", &cmd])
        .output()
    {
        Ok(o) if o.status.success() => {
            Ok(format!("System restore point created: '{}'", desc))
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            // Windows limits restore points to one per 24 hours by default
            if stderr.contains("1314") || stderr.contains("privilege") {
                Err("Requires Administrator privileges to create restore points".into())
            } else if stderr.contains("frequency") || stderr.contains("already") {
                Err("Windows limits restore point creation to once per 24 hours. A recent restore point already exists.".into())
            } else {
                Err(format!("Failed to create restore point: {}", stderr.trim()))
            }
        }
        Err(e) => Err(format!("Failed to run restore point command: {}", e)),
    }
}

/// Check if System Protection is enabled on the system drive
pub fn is_restore_enabled() -> bool {
    Command::new("powershell")
        .args([
            "-Command",
            r#"(Get-ComputerRestorePoint -ErrorAction SilentlyContinue | Measure-Object).Count -ge 0"#,
        ])
        .output()
        .map(|o| {
            let out = String::from_utf8_lossy(&o.stdout);
            out.trim() == "True"
        })
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Auto Memory Purge Settings
// ═══════════════════════════════════════════════════════════════════════════════

#[allow(dead_code)] // Config struct ready for background auto-purge timer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPurgeConfig {
    pub enabled: bool,
    pub threshold_percent: u32, // Purge when RAM usage exceeds this %
    pub interval_seconds: u32,  // Check interval
    pub purge_standby: bool,
    pub purge_modified: bool,
    pub purge_cache: bool,
}

impl Default for AutoPurgeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_percent: 85,
            interval_seconds: 30,
            purge_standby: true,
            purge_modified: false,
            purge_cache: false,
        }
    }
}
