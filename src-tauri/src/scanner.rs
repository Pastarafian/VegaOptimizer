//! Scanner module — large files, browser cleanup, privacy, drivers

use serde::{Deserialize, Serialize};
use std::process::Command;

// ═══════════════════════════════════════════════════════════════════════════════
// Large File Scanner
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeFile {
    pub path: String,
    pub size_mb: f64,
    pub extension: String,
    pub category: String,
    pub modified: String,
}

pub fn scan_large_files(min_size_mb: u64, max_results: usize) -> Vec<LargeFile> {
    let mut files: Vec<LargeFile> = Vec::new();
    let min_bytes = min_size_mb * 1_048_576;

    // Scan common locations
    let dirs_to_scan = [
        std::env::var("USERPROFILE").unwrap_or_default(),
        "C:\\".to_string(),
    ];

    for base_dir in &dirs_to_scan {
        if base_dir.is_empty() {
            continue;
        }
        scan_dir_recursive(base_dir, min_bytes, &mut files, 3, max_results);
        if files.len() >= max_results {
            break;
        }
    }

    files.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    files.truncate(max_results);
    files
}

fn scan_dir_recursive(
    dir: &str,
    min_bytes: u64,
    files: &mut Vec<LargeFile>,
    depth: u32,
    max: usize,
) {
    if depth == 0 || files.len() >= max {
        return;
    }

    let skip_dirs = [
        "Windows",
        "Program Files",
        "Program Files (x86)",
        "$Recycle.Bin",
        "System Volume Information",
        ".git",
        "node_modules",
        "target",
        "AppData",
    ];

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if files.len() >= max {
                return;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    if !skip_dirs.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                        scan_dir_recursive(
                            &path.to_string_lossy(),
                            min_bytes,
                            files,
                            depth - 1,
                            max,
                        );
                    }
                } else if meta.is_file() && meta.len() >= min_bytes {
                    let ext = path
                        .extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| {
                            let secs = d.as_secs();
                            let days_ago = (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                                - secs)
                                / 86400;
                            if days_ago == 0 {
                                "Today".into()
                            } else if days_ago == 1 {
                                "Yesterday".into()
                            } else {
                                format!("{} days ago", days_ago)
                            }
                        })
                        .unwrap_or_else(|| "Unknown".into());

                    files.push(LargeFile {
                        path: path.to_string_lossy().to_string(),
                        size_mb: meta.len() as f64 / 1_048_576.0,
                        extension: ext.clone(),
                        category: categorize_extension(&ext),
                        modified,
                    });
                }
            }
        }
    }
}

fn categorize_extension(ext: &str) -> String {
    match ext {
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" => "Video",
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" => "Audio",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "webp" | "svg" => "Image",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" => "Archive",
        "iso" | "img" | "vhd" | "vhdx" => "Disk Image",
        "exe" | "msi" | "dll" => "Application",
        "log" | "txt" | "csv" => "Log/Text",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => "Document",
        "bak" | "tmp" | "dmp" | "old" => "Backup/Temp",
        _ => "Other",
    }
    .to_string()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Browser Cleanup
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub name: String,
    pub cache_size_mb: f64,
    pub cache_path: String,
    pub installed: bool,
}

pub fn detect_browsers() -> Vec<BrowserInfo> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    let browsers = vec![
        (
            "Google Chrome",
            format!("{}\\Google\\Chrome\\User Data\\Default\\Cache", local),
        ),
        (
            "Microsoft Edge",
            format!("{}\\Microsoft\\Edge\\User Data\\Default\\Cache", local),
        ),
        (
            "Mozilla Firefox",
            format!("{}\\Mozilla\\Firefox\\Profiles", appdata),
        ),
        (
            "Brave",
            format!(
                "{}\\BraveSoftware\\Brave-Browser\\User Data\\Default\\Cache",
                local
            ),
        ),
        (
            "Opera",
            format!("{}\\Opera Software\\Opera Stable\\Cache", appdata),
        ),
        (
            "Vivaldi",
            format!("{}\\Vivaldi\\User Data\\Default\\Cache", local),
        ),
    ];

    browsers
        .into_iter()
        .map(|(name, cache_path)| {
            let exists = std::path::Path::new(&cache_path).exists();
            let size = if exists { dir_size(&cache_path) } else { 0 };
            BrowserInfo {
                name: name.to_string(),
                cache_size_mb: size as f64 / 1_048_576.0,
                cache_path,
                installed: exists,
            }
        })
        .collect()
}

pub fn clean_browser_cache(browser_name: &str) -> Result<String, String> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    let cache_paths: Vec<String> = match browser_name {
        "Google Chrome" => vec![
            format!("{}\\Google\\Chrome\\User Data\\Default\\Cache", local),
            format!("{}\\Google\\Chrome\\User Data\\Default\\Code Cache", local),
            format!("{}\\Google\\Chrome\\User Data\\Default\\GPUCache", local),
        ],
        "Microsoft Edge" => vec![
            format!("{}\\Microsoft\\Edge\\User Data\\Default\\Cache", local),
            format!("{}\\Microsoft\\Edge\\User Data\\Default\\Code Cache", local),
        ],
        "Brave" => vec![format!(
            "{}\\BraveSoftware\\Brave-Browser\\User Data\\Default\\Cache",
            local
        )],
        "Mozilla Firefox" => {
            // Firefox profiles have random names
            let profiles_dir = format!("{}\\Mozilla\\Firefox\\Profiles", appdata);
            let mut paths = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    if entry.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                        paths.push(format!("{}\\cache2", entry.path().to_string_lossy()));
                    }
                }
            }
            paths
        }
        _ => return Err(format!("Unknown browser: {}", browser_name)),
    };

    let mut total_freed: u64 = 0;
    let mut files_deleted: u32 = 0;

    for path in &cache_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        let size = meta.len();
                        if std::fs::remove_file(entry.path()).is_ok() {
                            total_freed += size;
                            files_deleted += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(format!(
        "Cleaned {} — deleted {} files, freed {:.1} MB",
        browser_name,
        files_deleted,
        total_freed as f64 / 1_048_576.0
    ))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Cleanup
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub data_size_mb: f64,
}

pub fn get_privacy_items() -> Vec<PrivacyItem> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    vec![
        PrivacyItem {
            id: "recent_files".into(),
            name: "Recent File History".into(),
            description: "Clear recently accessed files list".into(),
            category: "Windows".into(),
            data_size_mb: dir_size_mb(&format!("{}\\Microsoft\\Windows\\Recent", appdata)),
        },
        PrivacyItem {
            id: "clipboard".into(),
            name: "Clipboard History".into(),
            description: "Clear clipboard contents and history".into(),
            category: "Windows".into(),
            data_size_mb: 0.0,
        },
        PrivacyItem {
            id: "explorer_history".into(),
            name: "Explorer Address Bar History".into(),
            description: "Clear typed paths in File Explorer".into(),
            category: "Windows".into(),
            data_size_mb: 0.0,
        },
        PrivacyItem {
            id: "notification_cache".into(),
            name: "Notification Cache".into(),
            description: "Clear Windows notification history".into(),
            category: "Windows".into(),
            data_size_mb: dir_size_mb(&format!("{}\\Microsoft\\Windows\\Notifications", local)),
        },
        PrivacyItem {
            id: "activity_history".into(),
            name: "Activity Timeline".into(),
            description: "Clear Windows Timeline/Activity History".into(),
            category: "Windows".into(),
            data_size_mb: dir_size_mb(&format!("{}\\ConnectedDevicesPlatform", local)),
        },
        PrivacyItem {
            id: "prefetch".into(),
            name: "Prefetch Data".into(),
            description: "Clear application prefetch traces".into(),
            category: "System".into(),
            data_size_mb: dir_size_mb("C:\\Windows\\Prefetch"),
        },
    ]
}

pub fn clean_privacy_item(id: &str) -> Result<String, String> {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();

    match id {
        "recent_files" => {
            let path = format!("{}\\Microsoft\\Windows\\Recent", appdata);
            let count = clean_dir_files(&path);
            Ok(format!("Cleared {} recent file entries", count))
        }
        "clipboard" => {
            let _ = Command::new("cmd").args(["/C", "echo off | clip"]).output();
            Ok("Clipboard cleared".into())
        }
        "explorer_history" => {
            let _ = Command::new("reg")
                .args([
                    "delete",
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\TypedPaths",
                    "/f",
                ])
                .output();
            Ok("Explorer history cleared".into())
        }
        "notification_cache" => {
            let path = format!("{}\\Microsoft\\Windows\\Notifications", local);
            let count = clean_dir_files(&path);
            Ok(format!("Cleared {} notification entries", count))
        }
        "activity_history" => {
            let path = format!("{}\\ConnectedDevicesPlatform", local);
            let count = clean_dir_files(&path);
            Ok(format!("Cleared activity history ({} items)", count))
        }
        "prefetch" => {
            let count = clean_dir_files("C:\\Windows\\Prefetch");
            Ok(format!("Cleared {} prefetch files", count))
        }
        _ => Err(format!("Unknown privacy item: {}", id)),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Driver Information
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverInfo {
    pub name: String,
    pub provider: String,
    pub version: String,
    pub date: String,
    pub device_class: String,
    pub signed: bool,
    pub status: String, // "OK", "Outdated", "Problem"
}

pub fn list_drivers() -> Vec<DriverInfo> {
    // Use driverquery for a comprehensive list
    let output = Command::new("driverquery")
        .args(["/v", "/fo", "csv"])
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let mut drivers: Vec<DriverInfo> = Vec::new();

            for (i, line) in stdout.lines().enumerate() {
                if i == 0 {
                    continue;
                } // Skip header

                let fields: Vec<&str> = line.split("\",\"").collect();
                if fields.len() >= 6 {
                    let name = fields[0].trim_matches('"').to_string();
                    let display_name = fields[1].trim_matches('"').to_string();

                    // Skip system-internal drivers for cleaner display
                    if name.is_empty() {
                        continue;
                    }

                    let driver_type = if fields.len() > 3 {
                        fields[3].trim_matches('"').to_string()
                    } else {
                        "Unknown".into()
                    };
                    let state = if fields.len() > 4 {
                        fields[4].trim_matches('"').to_string()
                    } else {
                        "Unknown".into()
                    };
                    let link_date = if fields.len() > 10 {
                        fields[10].trim_matches('"').to_string()
                    } else {
                        "Unknown".into()
                    };

                    drivers.push(DriverInfo {
                        name: display_name,
                        provider: name,
                        version: "".into(),
                        date: link_date,
                        device_class: driver_type,
                        signed: true,
                        status: if state.contains("Running") {
                            "OK".into()
                        } else {
                            "Stopped".into()
                        },
                    });
                }
            }

            // Also get PnP driver info for versions
            if let Ok(pnp) = Command::new("powershell")
                .args(["-Command", "Get-WmiObject Win32_PnPSignedDriver | Select-Object DeviceName,DriverVersion,Manufacturer,DriverDate,IsSigned | ConvertTo-Csv -NoTypeInformation | Select-Object -First 50"])
                .output()
            {
                let pnp_out = String::from_utf8_lossy(&pnp.stdout);
                for (i, line) in pnp_out.lines().enumerate() {
                    if i == 0 { continue; }
                    let fields: Vec<&str> = line.split("\",\"").collect();
                    if fields.len() >= 5 {
                        let dev_name = fields[0].trim_matches('"').to_string();
                        let version = fields[1].trim_matches('"').to_string();
                        let mfr = fields[2].trim_matches('"').to_string();
                        let date = fields[3].trim_matches('"').to_string();
                        let signed = fields[4].trim_matches('"').contains("True");

                        if dev_name.is_empty() { continue; }

                        drivers.push(DriverInfo {
                            name: dev_name,
                            provider: mfr,
                            version,
                            date,
                            device_class: "PnP Device".into(),
                            signed,
                            status: "OK".into(),
                        });
                    }
                }
            }

            drivers
        }
        Err(_) => vec![],
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Windows Update Cleanup
// ═══════════════════════════════════════════════════════════════════════════════

pub fn clean_windows_update() -> Result<String, String> {
    let mut freed = 0u64;
    let mut count = 0u32;

    // Software Distribution Download
    let dl_path = "C:\\Windows\\SoftwareDistribution\\Download";
    if let Ok(entries) = std::fs::read_dir(dl_path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    freed += meta.len();
                    let _ = std::fs::remove_file(entry.path());
                    count += 1;
                } else if meta.is_dir() {
                    if let Ok(size) = dir_size_result(&entry.path().to_string_lossy()) {
                        freed += size;
                    }
                    let _ = std::fs::remove_dir_all(entry.path());
                    count += 1;
                }
            }
        }
    }

    Ok(format!(
        "Cleaned Windows Update cache — {} items, freed {:.1} MB",
        count,
        freed as f64 / 1_048_576.0
    ))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn dir_size(path: &str) -> u64 {
    dir_size_result(path).unwrap_or(0)
}

fn dir_size_result(path: &str) -> Result<u64, std::io::Error> {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += dir_size(&entry.path().to_string_lossy());
                }
            }
        }
    }
    Ok(total)
}

fn dir_size_mb(path: &str) -> f64 {
    dir_size(path) as f64 / 1_048_576.0
}

fn clean_dir_files(path: &str) -> u32 {
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    if std::fs::remove_file(entry.path()).is_ok() {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}
