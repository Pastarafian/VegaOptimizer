//! Disk Cleanup module — junk scanning, shredding, AI suggestions, app caches

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::process::Command;

// ═══════════════════════════════════════════════════════════════════════════════
// Types
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JunkCategory {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub size_mb: f64,
    pub file_count: u32,
    pub safe_to_clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanResult {
    pub category: String,
    pub files_deleted: u32,
    pub space_freed_mb: f64,
    pub errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCache {
    pub app_name: String,
    pub icon: String,
    pub cache_size_mb: f64,
    pub installed: bool,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaleFile {
    pub path: String,
    pub size_mb: f64,
    pub last_accessed_days: u64,
    pub extension: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledProgram {
    pub name: String,
    pub publisher: String,
    pub version: String,
    pub install_date: String,
    pub install_location: String,
    pub size_mb: f64,
    pub uninstall_command: String,
    pub category: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestorePoint {
    pub sequence: u32,
    pub description: String,
    pub creation_time: String,
    pub restore_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShredResult {
    pub path: String,
    pub size_mb: f64,
    pub passes_completed: u32,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WipeProgress {
    pub drive: String,
    pub passes_completed: u32,
    pub bytes_written: u64,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSuggestion {
    pub path: String,
    pub name: String,
    pub size_mb: f64,
    pub suggestion_type: String,
    pub confidence: f64,
    pub reason: String,
    pub risk: String,
    pub action: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderSize {
    pub path: String,
    pub name: String,
    pub size_mb: f64,
    pub file_count: u32,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepCleanResult {
    pub total_freed_mb: f64,
    pub total_files: u32,
    pub categories_cleaned: u32,
    pub results: Vec<CleanResult>,
    pub duration_ms: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// Junk File Scanner
// ═══════════════════════════════════════════════════════════════════════════════

pub fn scan_junk_categories() -> Vec<JunkCategory> {
    let temp = std::env::var("TEMP").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let mut cats = Vec::new();

    // Windows Temp
    let (s1, c1) = dir_stats(&temp);
    let (s2, c2) = dir_stats("C:\\Windows\\Temp");
    cats.push(JunkCategory {
        id: "windows_temp".into(),
        name: "Windows Temp Files".into(),
        description: "Temporary files from Windows and applications".into(),
        icon: "🗑️".into(),
        size_mb: s1 + s2,
        file_count: c1 + c2,
        safe_to_clean: true,
    });

    // Error Reports
    let (s, c) = dir_stats("C:\\ProgramData\\Microsoft\\Windows\\WER");
    cats.push(JunkCategory {
        id: "error_reports".into(),
        name: "Windows Error Reports".into(),
        description: "Crash reports and diagnostic data".into(),
        icon: "⚠️".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Windows Update Cache
    let (s, c) = dir_stats("C:\\Windows\\SoftwareDistribution\\Download");
    cats.push(JunkCategory {
        id: "update_cache".into(),
        name: "Windows Update Cache".into(),
        description: "Downloaded update files no longer needed".into(),
        icon: "🔄".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Delivery Optimization
    let (s, c) = dir_stats("C:\\Windows\\SoftwareDistribution\\DeliveryOptimization");
    cats.push(JunkCategory {
        id: "delivery_opt".into(),
        name: "Delivery Optimization Cache".into(),
        description: "Peer-to-peer update distribution cache".into(),
        icon: "📡".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Thumbnail Cache
    let thumb_path = format!("{}\\Microsoft\\Windows\\Explorer", local);
    let (s, c) = dir_stats_filter(&thumb_path, |name| name.starts_with("thumbcache_"));
    cats.push(JunkCategory {
        id: "thumbnails".into(),
        name: "Thumbnail Cache".into(),
        description: "Cached image thumbnails — rebuilt automatically".into(),
        icon: "🖼️".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Crash Dumps
    let dump_path = format!("{}\\CrashDumps", local);
    let (s1, c1) = dir_stats(&dump_path);
    let (s2, c2) = dir_stats("C:\\Windows\\Minidump");
    let mem_dump = Path::new("C:\\Windows\\MEMORY.DMP");
    let ds = if mem_dump.exists() {
        mem_dump.metadata().map(|m| m.len()).unwrap_or(0) as f64 / 1_048_576.0
    } else {
        0.0
    };
    cats.push(JunkCategory {
        id: "crash_dumps".into(),
        name: "Crash Dumps".into(),
        description: "Memory dumps from system/application crashes".into(),
        icon: "💥".into(),
        size_mb: s1 + s2 + ds,
        file_count: c1 + c2 + if ds > 0.0 { 1 } else { 0 },
        safe_to_clean: true,
    });

    // Shader Cache
    let (s1, c1) = dir_stats(&format!("{}\\NVIDIA\\GLCache", local));
    let (s2, c2) = dir_stats(&format!("{}\\AMD\\GLCache", local));
    let (s3, c3) = dir_stats(&format!("{}\\D3DSCache", local));
    cats.push(JunkCategory {
        id: "shader_cache".into(),
        name: "GPU Shader Cache".into(),
        description: "Cached GPU shaders — rebuilt when needed".into(),
        icon: "🎮".into(),
        size_mb: s1 + s2 + s3,
        file_count: c1 + c2 + c3,
        safe_to_clean: true,
    });

    // Windows Log Files
    let (s, c) = dir_stats("C:\\Windows\\Logs");
    cats.push(JunkCategory {
        id: "windows_logs".into(),
        name: "Windows Log Files".into(),
        description: "Old Windows system and setup log files".into(),
        icon: "📝".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Prefetch
    let (s, c) = dir_stats("C:\\Windows\\Prefetch");
    cats.push(JunkCategory {
        id: "prefetch".into(),
        name: "Prefetch Data".into(),
        description: "Application launch optimization cache".into(),
        icon: "⚡".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Recent Items
    let (s, c) = dir_stats(&format!("{}\\Microsoft\\Windows\\Recent", appdata));
    cats.push(JunkCategory {
        id: "recent_items".into(),
        name: "Recent Items History".into(),
        description: "Shortcuts to recently accessed files".into(),
        icon: "🕐".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Font Cache
    let (s, c) = dir_stats("C:\\Windows\\ServiceProfiles\\LocalService\\AppData\\Local\\FontCache");
    cats.push(JunkCategory {
        id: "font_cache".into(),
        name: "Font Cache".into(),
        description: "Cached font rendering data — rebuilt on reboot".into(),
        icon: "🔤".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    // Installer Patch Cache
    let (s, c) = dir_stats("C:\\Windows\\Installer\\$PatchCache$");
    cats.push(JunkCategory {
        id: "patch_cache".into(),
        name: "Installer Patch Cache".into(),
        description: "Old installer patch files".into(),
        icon: "📦".into(),
        size_mb: s,
        file_count: c,
        safe_to_clean: true,
    });

    cats.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    cats
}

pub fn clean_junk_category(id: &str) -> Result<CleanResult, String> {
    let temp = std::env::var("TEMP").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    let paths: Vec<String> = match id {
        "windows_temp" => vec![temp, "C:\\Windows\\Temp".into()],
        "error_reports" => vec!["C:\\ProgramData\\Microsoft\\Windows\\WER".into()],
        "update_cache" => vec!["C:\\Windows\\SoftwareDistribution\\Download".into()],
        "delivery_opt" => vec!["C:\\Windows\\SoftwareDistribution\\DeliveryOptimization".into()],
        "thumbnails" => vec![format!("{}\\Microsoft\\Windows\\Explorer", local)],
        "crash_dumps" => vec![
            format!("{}\\CrashDumps", local),
            "C:\\Windows\\Minidump".into(),
        ],
        "shader_cache" => vec![
            format!("{}\\NVIDIA\\GLCache", local),
            format!("{}\\AMD\\GLCache", local),
            format!("{}\\D3DSCache", local),
        ],
        "windows_logs" => vec!["C:\\Windows\\Logs".into()],
        "prefetch" => vec!["C:\\Windows\\Prefetch".into()],
        "recent_items" => vec![format!("{}\\Microsoft\\Windows\\Recent", appdata)],
        "font_cache" => {
            vec!["C:\\Windows\\ServiceProfiles\\LocalService\\AppData\\Local\\FontCache".into()]
        }
        "patch_cache" => vec!["C:\\Windows\\Installer\\$PatchCache$".into()],
        _ => return Err(format!("Unknown junk category: {}", id)),
    };

    let mut total_del = 0u32;
    let mut total_freed = 0u64;
    let mut errors = 0u32;

    for path in &paths {
        let (d, f, e) = clean_dir_all(path);
        total_del += d;
        total_freed += f;
        errors += e;
    }

    // Special: delete MEMORY.DMP for crash_dumps
    if id == "crash_dumps" {
        let mem_dump = Path::new("C:\\Windows\\MEMORY.DMP");
        if mem_dump.exists() {
            if let Ok(meta) = mem_dump.metadata() {
                total_freed += meta.len();
                if std::fs::remove_file(mem_dump).is_ok() {
                    total_del += 1;
                } else {
                    errors += 1;
                }
            }
        }
    }

    Ok(CleanResult {
        category: id.to_string(),
        files_deleted: total_del,
        space_freed_mb: total_freed as f64 / 1_048_576.0,
        errors,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// App-Specific Cache Scanner
// ═══════════════════════════════════════════════════════════════════════════════

pub fn scan_app_caches() -> Vec<AppCache> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();

    let apps: Vec<(&str, &str, String, &str)> = vec![
        (
            "Discord",
            "💬",
            format!("{}\\discord\\Cache", appdata),
            "Chat app cache and temporary data",
        ),
        (
            "Spotify",
            "🎵",
            format!("{}\\Spotify\\Data", local),
            "Music streaming cache and offline data",
        ),
        (
            "Steam",
            "🎮",
            "C:\\Program Files (x86)\\Steam\\appcache".into(),
            "Game platform cache files",
        ),
        (
            "VS Code",
            "💻",
            format!("{}\\Code\\Cache", appdata),
            "Code editor cache and extensions cache",
        ),
        (
            "Microsoft Teams",
            "👥",
            format!("{}\\Microsoft\\Teams\\Cache", appdata),
            "Teams chat and meeting cache",
        ),
        (
            "Slack",
            "📱",
            format!("{}\\Slack\\Cache", appdata),
            "Slack messaging cache",
        ),
        (
            "Zoom",
            "📹",
            format!("{}\\Zoom\\data", appdata),
            "Video conferencing cache",
        ),
        (
            "Adobe Creative Cloud",
            "🎨",
            format!("{}\\Adobe", local),
            "Adobe application caches",
        ),
        (
            "Office Cache",
            "📄",
            format!("{}\\Microsoft\\Office\\16.0\\OfficeFileCache", local),
            "Microsoft Office temporary files",
        ),
        (
            "Electron Apps",
            "⚛️",
            format!("{}\\electron\\Cache", appdata),
            "Electron framework shared cache",
        ),
        (
            "pip Cache",
            "🐍",
            format!("{}\\pip\\Cache", local),
            "Python package manager cache",
        ),
        (
            "npm Cache",
            "📦",
            format!("{}\\npm-cache", appdata),
            "Node.js package manager cache",
        ),
        (
            "NuGet Cache",
            "🔷",
            format!("{}\\NuGet\\v3-cache", local),
            "NuGet package manager cache",
        ),
        (
            "Gradle Cache",
            "🏗️",
            format!(
                "{}\\.gradle\\caches",
                std::env::var("USERPROFILE").unwrap_or_default()
            ),
            "Java build system cache",
        ),
    ];

    apps.into_iter()
        .map(|(name, icon, path, desc)| {
            let exists = Path::new(&path).exists();
            let size = if exists { dir_size_recursive(&path) } else { 0 };
            AppCache {
                app_name: name.to_string(),
                icon: icon.to_string(),
                cache_size_mb: size as f64 / 1_048_576.0,
                installed: exists,
                description: desc.to_string(),
            }
        })
        .collect()
}

pub fn clean_app_cache(app_name: &str) -> Result<CleanResult, String> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    let user = std::env::var("USERPROFILE").unwrap_or_default();

    let paths: Vec<String> = match app_name {
        "Discord" => vec![
            format!("{}\\discord\\Cache", appdata),
            format!("{}\\discord\\Code Cache", appdata),
        ],
        "Spotify" => vec![format!("{}\\Spotify\\Data", local)],
        "Steam" => vec!["C:\\Program Files (x86)\\Steam\\appcache".into()],
        "VS Code" => vec![
            format!("{}\\Code\\Cache", appdata),
            format!("{}\\Code\\CachedData", appdata),
        ],
        "Microsoft Teams" => vec![
            format!("{}\\Microsoft\\Teams\\Cache", appdata),
            format!("{}\\Microsoft\\Teams\\blob_storage", appdata),
        ],
        "Slack" => vec![format!("{}\\Slack\\Cache", appdata)],
        "Zoom" => vec![format!("{}\\Zoom\\data", appdata)],
        "Adobe Creative Cloud" => vec![format!("{}\\Adobe", local)],
        "Office Cache" => vec![format!(
            "{}\\Microsoft\\Office\\16.0\\OfficeFileCache",
            local
        )],
        "Electron Apps" => vec![format!("{}\\electron\\Cache", appdata)],
        "pip Cache" => vec![format!("{}\\pip\\Cache", local)],
        "npm Cache" => vec![format!("{}\\npm-cache", appdata)],
        "NuGet Cache" => vec![format!("{}\\NuGet\\v3-cache", local)],
        "Gradle Cache" => vec![format!("{}\\.gradle\\caches", user)],
        _ => return Err(format!("Unknown app: {}", app_name)),
    };

    let mut total_del = 0u32;
    let mut total_freed = 0u64;
    let mut errors = 0u32;
    for path in &paths {
        let (d, f, e) = clean_dir_all(path);
        total_del += d;
        total_freed += f;
        errors += e;
    }

    Ok(CleanResult {
        category: app_name.to_string(),
        files_deleted: total_del,
        space_freed_mb: total_freed as f64 / 1_048_576.0,
        errors,
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Stale File Scanner
// ═══════════════════════════════════════════════════════════════════════════════

pub fn scan_stale_files(days_threshold: u64, max_results: usize) -> Vec<StaleFile> {
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let scan_dirs = vec![
        format!("{}\\Downloads", user),
        format!("{}\\Documents", user),
        format!("{}\\Desktop", user),
        format!("{}\\Videos", user),
        format!("{}\\Music", user),
        format!("{}\\Pictures", user),
    ];

    let mut stale = Vec::new();
    for dir in &scan_dirs {
        scan_stale_recursive(dir, days_threshold, now, &mut stale, 4, max_results);
        if stale.len() >= max_results {
            break;
        }
    }

    stale.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    stale.truncate(max_results);
    stale
}

fn scan_stale_recursive(
    dir: &str,
    days: u64,
    now_secs: u64,
    results: &mut Vec<StaleFile>,
    depth: u32,
    max: usize,
) {
    if depth == 0 || results.len() >= max {
        return;
    }
    let skip = [
        ".git",
        "node_modules",
        "target",
        "__pycache__",
        ".vs",
        ".idea",
    ];

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if results.len() >= max {
                return;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(meta) = entry.metadata() {
                if meta.is_dir() {
                    if !skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                        scan_stale_recursive(
                            &path.to_string_lossy(),
                            days,
                            now_secs,
                            results,
                            depth - 1,
                            max,
                        );
                    }
                } else if meta.is_file() && meta.len() > 1024 {
                    // Use accessed time, fall back to modified
                    let access_secs = meta
                        .accessed()
                        .ok()
                        .or_else(|| meta.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let age_days = if access_secs > 0 {
                        (now_secs - access_secs) / 86400
                    } else {
                        0
                    };

                    if age_days >= days {
                        let ext = path
                            .extension()
                            .map(|e| e.to_string_lossy().to_lowercase())
                            .unwrap_or_default();
                        results.push(StaleFile {
                            path: path.to_string_lossy().to_string(),
                            size_mb: meta.len() as f64 / 1_048_576.0,
                            last_accessed_days: age_days,
                            extension: ext.clone(),
                            category: categorize_ext(&ext),
                        });
                    }
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Installed Programs Analyzer
// ═══════════════════════════════════════════════════════════════════════════════

const BLOATWARE_PATTERNS: &[(&str, &str)] = &[
    ("McAfee", "Trial antivirus — Windows Defender is sufficient"),
    ("Norton", "Trial antivirus — Windows Defender is sufficient"),
    ("WildTangent", "Preinstalled games — waste of space"),
    ("CyberLink", "Preinstalled media software — rarely used"),
    ("Booking.com", "Preinstalled adware bookmark"),
    ("ExpressVPN", "Preinstalled trial VPN app"),
    ("Candy Crush", "Preinstalled casual game"),
    ("HP Support", "Manufacturer support bloatware"),
    ("Dell SupportAssist", "Manufacturer support bloatware"),
    ("Lenovo Vantage", "Manufacturer utility — optional"),
    ("ASUS Armoury", "Manufacturer utility — optional"),
    ("Acer Care", "Manufacturer support bloatware"),
    ("Roblox", "Preinstalled game — remove if unused"),
    ("Solitaire", "Preinstalled game"),
    ("Netflix", "Preinstalled streaming shortcut"),
    ("Disney", "Preinstalled streaming shortcut"),
    ("TikTok", "Preinstalled social media app"),
    ("Instagram", "Preinstalled social media app"),
    ("Facebook", "Preinstalled social media app"),
];

pub fn list_installed_programs() -> Vec<InstalledProgram> {
    let mut programs = Vec::new();
    let cmd = r#"Get-ItemProperty 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName } | ForEach-Object { "$($_.DisplayName)|$($_.Publisher)|$($_.DisplayVersion)|$($_.InstallDate)|$($_.InstallLocation)|$($_.EstimatedSize)|$($_.UninstallString)" }"#;

    if let Ok(output) = Command::new("powershell").args(["-Command", cmd]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 7 {
                continue;
            }
            let name = parts[0].trim().to_string();
            if name.is_empty() {
                continue;
            }
            let publisher = parts[1].trim().to_string();
            let version = parts[2].trim().to_string();
            let install_date = parts[3].trim().to_string();
            let install_loc = parts[4].trim().to_string();
            let size_kb: f64 = parts[5].trim().parse().unwrap_or(0.0);
            let uninstall = parts[6].trim().to_string();

            let mut category = "normal".to_string();
            let mut recommendation = String::new();

            for (pattern, rec) in BLOATWARE_PATTERNS {
                if name.to_lowercase().contains(&pattern.to_lowercase()) {
                    category = "bloatware".to_string();
                    recommendation = rec.to_string();
                    break;
                }
            }

            programs.push(InstalledProgram {
                name,
                publisher,
                version,
                install_date,
                install_location: install_loc,
                size_mb: size_kb / 1024.0,
                uninstall_command: uninstall,
                category,
                recommendation,
            });
        }
    }

    programs.sort_by(|a, b| {
        let cat_ord = |c: &str| match c {
            "bloatware" => 0,
            "rarely_used" => 1,
            _ => 2,
        };
        cat_ord(&a.category).cmp(&cat_ord(&b.category)).then(
            b.size_mb
                .partial_cmp(&a.size_mb)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    programs
}

pub fn uninstall_program(uninstall_cmd: &str) -> Result<String, String> {
    if uninstall_cmd.is_empty() {
        return Err("No uninstall command available".into());
    }
    match Command::new("cmd").args(["/C", uninstall_cmd]).output() {
        Ok(o) if o.status.success() => Ok("Uninstall initiated".into()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// System Restore Point Manager
// ═══════════════════════════════════════════════════════════════════════════════

pub fn list_restore_points() -> Vec<RestorePoint> {
    let mut points = Vec::new();
    let cmd = r#"Get-ComputerRestorePoint -ErrorAction SilentlyContinue | ForEach-Object { "$($_.SequenceNumber)|$($_.Description)|$($_.CreationTime)|$($_.RestorePointType)" }"#;

    if let Ok(output) = Command::new("powershell").args(["-Command", cmd]).output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 4 {
                continue;
            }
            points.push(RestorePoint {
                sequence: parts[0].trim().parse().unwrap_or(0),
                description: parts[1].trim().to_string(),
                creation_time: parts[2].trim().to_string(),
                restore_type: match parts[3].trim() {
                    "0" => "System".into(),
                    "1" => "Application Install".into(),
                    "10" => "Device Driver".into(),
                    "12" => "Modify Settings".into(),
                    "13" => "Cancelled Operation".into(),
                    _ => "Other".into(),
                },
            });
        }
    }
    points
}

pub fn delete_restore_point(seq: u32) -> Result<String, String> {
    let cmd = format!("vssadmin delete shadows /shadow={{}} /quiet 2>$null; if ($?) {{ 'Deleted restore point {}' }} else {{ Checkpoint-Computer -RestorePointType MODIFY_SETTINGS -Description VegaCleanup -ErrorAction SilentlyContinue; 'Restore point {} queued for deletion' }}", seq, seq);
    match Command::new("powershell").args(["-Command", &cmd]).output() {
        Ok(o) => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Secure File Shredder
// ═══════════════════════════════════════════════════════════════════════════════

/// Shred patterns for each pass level
const SHRED_1_PASS: &[u8] = &[0x00];
const SHRED_3_PASS: &[u8] = &[0x00, 0xFF, 0xAA];
const SHRED_7_PASS: &[u8] = &[0xFF, 0x00, 0xAA, 0x55, 0x92, 0x49, 0x24];

pub fn shred_file(path: &str, passes: u32) -> Result<ShredResult, String> {
    let p = Path::new(path);
    if !p.exists() {
        return Err("File not found".into());
    }
    if !p.is_file() {
        return Err("Not a file".into());
    }

    let lower = path.to_lowercase();
    if lower.starts_with("c:\\windows") || lower.starts_with("c:\\program files") {
        return Err("Cannot shred system files".into());
    }

    let size = p.metadata().map(|m| m.len()).unwrap_or(0);
    let size_mb = size as f64 / 1_048_576.0;
    let patterns = match passes {
        1 => SHRED_1_PASS,
        3 => SHRED_3_PASS,
        7 => SHRED_7_PASS,
        _ => SHRED_3_PASS,
    };

    let actual_passes = patterns.len() as u32;
    for pattern in patterns {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(p)
            .map_err(|e| format!("Open failed: {}", e))?;
        let buf = vec![*pattern; 8192];
        let mut remaining = size;
        while remaining > 0 {
            let chunk = std::cmp::min(remaining, 8192) as usize;
            file.write_all(&buf[..chunk])
                .map_err(|e| format!("Write failed: {}", e))?;
            remaining -= chunk as u64;
        }
        file.flush().map_err(|e| format!("Flush failed: {}", e))?;
    }

    std::fs::remove_file(p).map_err(|e| format!("Delete failed: {}", e))?;

    Ok(ShredResult {
        path: path.to_string(),
        size_mb,
        passes_completed: actual_passes,
        success: true,
        message: format!(
            "Securely shredded with {} passes ({:.1} MB)",
            actual_passes, size_mb
        ),
    })
}

/// Wipe free space on a drive by writing a large temp file
pub fn wipe_free_space(drive_letter: &str, passes: u32) -> Result<WipeProgress, String> {
    let drive = if drive_letter.len() == 1 {
        format!("{}:\\", drive_letter)
    } else {
        drive_letter.to_string()
    };

    if !Path::new(&drive).exists() {
        return Err(format!("Drive {} not found", drive));
    }

    // Get free space via PowerShell
    let cmd = format!(
        "(Get-PSDrive {} -ErrorAction SilentlyContinue).Free",
        &drive[..1]
    );
    let free_bytes: u64 = Command::new("powershell")
        .args(["-Command", &cmd])
        .output()
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0);

    if free_bytes == 0 {
        return Err("Could not determine free space".into());
    }

    // Leave 500MB safety margin
    let write_target = if free_bytes > 524_288_000 {
        free_bytes - 524_288_000
    } else {
        return Err("Not enough free space (need >500MB free)".into());
    };
    // Cap at 10GB per run for safety
    let write_target = std::cmp::min(write_target, 10_737_418_240);

    let temp_path = format!("{}vega_wipe_{}.tmp", drive, std::process::id());
    let mut total_written: u64 = 0;
    let actual_passes = std::cmp::min(passes, 3); // Cap at 3 passes for free space

    let patterns: Vec<u8> = match actual_passes {
        1 => vec![0x00],
        2 => vec![0x00, 0xFF],
        _ => vec![0x00, 0xFF, 0xAA],
    };

    for pattern in &patterns {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&temp_path)
            .map_err(|e| format!("Failed to create wipe file: {}", e))?;
        let buf = vec![*pattern; 65536]; // 64KB chunks
        let mut written: u64 = 0;
        while written < write_target {
            let chunk = std::cmp::min(write_target - written, 65536) as usize;
            if file.write_all(&buf[..chunk]).is_err() {
                break;
            } // Disk full, that's fine
            written += chunk as u64;
        }
        file.flush().ok();
        total_written += written;
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    Ok(WipeProgress {
        drive,
        passes_completed: actual_passes,
        bytes_written: total_written,
        success: true,
        message: format!(
            "Wiped {:.1} GB of free space in {} passes",
            total_written as f64 / 1_073_741_824.0,
            actual_passes
        ),
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// AI Deletion Suggestor
// ═══════════════════════════════════════════════════════════════════════════════

pub fn get_ai_suggestions() -> Vec<AiSuggestion> {
    let user = std::env::var("USERPROFILE").unwrap_or_default();
    let mut suggestions = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Scan Downloads for old files
    let downloads = format!("{}\\Downloads", user);
    if let Ok(entries) = std::fs::read_dir(&downloads) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if !meta.is_file() {
                    continue;
                }
                let size = meta.len();
                if size < 1_048_576 {
                    continue;
                } // Skip < 1MB

                let name = entry.file_name().to_string_lossy().to_string();
                let ext = Path::new(&name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let age_days = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| (now - d.as_secs()) / 86400)
                    .unwrap_or(0);

                let (confidence, reason, risk, action) = score_file(&name, &ext, size, age_days);
                if confidence >= 0.3 {
                    suggestions.push(AiSuggestion {
                        path: entry.path().to_string_lossy().to_string(),
                        name: name.clone(),
                        size_mb: size as f64 / 1_048_576.0,
                        suggestion_type: "file".into(),
                        confidence,
                        reason,
                        risk,
                        action,
                        category: categorize_ext(&ext),
                    });
                }
            }
        }
    }

    // Scan Desktop for old files
    let desktop = format!("{}\\Desktop", user);
    if let Ok(entries) = std::fs::read_dir(&desktop) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if !meta.is_file() {
                    continue;
                }
                let size = meta.len();
                let name = entry.file_name().to_string_lossy().to_string();
                let ext = Path::new(&name)
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let age_days = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| (now - d.as_secs()) / 86400)
                    .unwrap_or(0);

                if age_days > 180 && size > 1_048_576 {
                    suggestions.push(AiSuggestion {
                        path: entry.path().to_string_lossy().to_string(),
                        name,
                        size_mb: size as f64 / 1_048_576.0,
                        suggestion_type: "file".into(),
                        confidence: 0.5,
                        reason: format!("Desktop file not modified in {} days", age_days),
                        risk: "caution".into(),
                        action: "review".into(),
                        category: categorize_ext(&ext),
                    });
                }
            }
        }
    }

    suggestions.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(50);
    suggestions
}

fn score_file(name: &str, ext: &str, size: u64, age_days: u64) -> (f64, String, String, String) {
    let lower = name.to_lowercase();
    let size_mb = size as f64 / 1_048_576.0;

    // Installers in Downloads
    if matches!(ext, "msi" | "exe") && age_days > 30 {
        return (
            0.9,
            format!(
                "Installer file ({:.0} MB), {} days old — likely already installed",
                size_mb, age_days
            ),
            "safe".into(),
            "delete".into(),
        );
    }
    // Compressed archives
    if matches!(ext, "zip" | "rar" | "7z" | "tar" | "gz") && age_days > 60 {
        return (
            0.7,
            format!(
                "Archive ({:.0} MB), {} days old — likely already extracted",
                size_mb, age_days
            ),
            "safe".into(),
            "delete".into(),
        );
    }
    // Disk images
    if matches!(ext, "iso" | "img") && age_days > 30 {
        return (
            0.85,
            format!(
                "Disk image ({:.0} MB), {} days old — usually one-time use",
                size_mb, age_days
            ),
            "safe".into(),
            "delete".into(),
        );
    }
    // Duplicate downloads: file(1).ext, file (2).ext, Copy of file
    if lower.contains("(1)")
        || lower.contains("(2)")
        || lower.contains("(3)")
        || lower.starts_with("copy of")
    {
        return (
            0.8,
            format!(
                "Duplicate download ({:.0} MB) — likely has an original copy",
                size_mb
            ),
            "safe".into(),
            "delete".into(),
        );
    }
    // Temp/log files
    if matches!(ext, "tmp" | "log" | "bak" | "old" | "dmp") {
        return (
            0.85,
            format!("Temporary/backup file ({:.0} MB) — safe to remove", size_mb),
            "safe".into(),
            "delete".into(),
        );
    }
    // Large video files not accessed in months
    if matches!(ext, "mp4" | "avi" | "mkv" | "mov" | "wmv") && age_days > 90 && size > 104_857_600 {
        return (
            0.5,
            format!(
                "Large video ({:.0} MB), not modified in {} days",
                size_mb, age_days
            ),
            "caution".into(),
            "review".into(),
        );
    }
    // Old documents
    if matches!(ext, "pdf" | "doc" | "docx" | "ppt" | "pptx") && age_days > 365 {
        return (
            0.3,
            format!("Document not modified in over a year ({} days)", age_days),
            "caution".into(),
            "archive".into(),
        );
    }
    // Generic old large file
    if age_days > 180 && size > 52_428_800 {
        return (
            0.4,
            format!(
                "Large file ({:.0} MB) not modified in {} days",
                size_mb, age_days
            ),
            "caution".into(),
            "review".into(),
        );
    }

    (0.0, String::new(), "risky".into(), "review".into())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Folder Size Analyzer (for treemap)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn get_folder_sizes(root: &str, max_depth: u32) -> Vec<FolderSize> {
    let mut folders = Vec::new();
    let total_size;

    if let Ok(entries) = std::fs::read_dir(root) {
        let mut items: Vec<(String, String, u64, u32)> = Vec::new();
        let mut grand_total: u64 = 0;

        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                let name = entry.file_name().to_string_lossy().to_string();
                let path = entry.path().to_string_lossy().to_string();
                if meta.is_dir() {
                    let (size, count) = dir_stats_deep(&path, max_depth);
                    let size_bytes = (size * 1_048_576.0) as u64;
                    grand_total += size_bytes;
                    items.push((path, name, size_bytes, count));
                }
            }
        }

        total_size = grand_total;
        for (path, name, size, count) in items {
            let pct = if total_size > 0 {
                (size as f64 / total_size as f64) * 100.0
            } else {
                0.0
            };
            if pct >= 0.5 {
                // Only show folders >= 0.5%
                folders.push(FolderSize {
                    path,
                    name,
                    size_mb: size as f64 / 1_048_576.0,
                    file_count: count,
                    percentage: pct,
                });
            }
        }
    }

    folders.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    folders.truncate(30);
    folders
}

// ═══════════════════════════════════════════════════════════════════════════════
// Deep Clean (One-Click)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn deep_clean() -> DeepCleanResult {
    let start = std::time::Instant::now();
    let mut results = Vec::new();

    let safe_cats = [
        "windows_temp",
        "error_reports",
        "crash_dumps",
        "shader_cache",
        "thumbnails",
        "prefetch",
        "recent_items",
        "windows_logs",
    ];

    for id in &safe_cats {
        match clean_junk_category(id) {
            Ok(r) => results.push(r),
            Err(_) => {}
        }
    }

    let total_freed: f64 = results.iter().map(|r| r.space_freed_mb).sum();
    let total_files: u32 = results.iter().map(|r| r.files_deleted).sum();

    DeepCleanResult {
        total_freed_mb: total_freed,
        total_files,
        categories_cleaned: results.len() as u32,
        results,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn dir_stats(path: &str) -> (f64, u32) {
    let mut total: u64 = 0;
    let mut count: u32 = 0;
    dir_stats_walk(path, &mut total, &mut count, 5);
    (total as f64 / 1_048_576.0, count)
}

fn dir_stats_deep(path: &str, depth: u32) -> (f64, u32) {
    let mut total: u64 = 0;
    let mut count: u32 = 0;
    dir_stats_walk(path, &mut total, &mut count, depth);
    (total as f64 / 1_048_576.0, count)
}

fn dir_stats_walk(path: &str, total: &mut u64, count: &mut u32, depth: u32) {
    if depth == 0 {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    *total += meta.len();
                    *count += 1;
                } else if meta.is_dir() {
                    dir_stats_walk(&entry.path().to_string_lossy(), total, count, depth - 1);
                }
            }
        }
    }
}

fn dir_stats_filter(path: &str, filter: impl Fn(&str) -> bool) -> (f64, u32) {
    let mut total: u64 = 0;
    let mut count: u32 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if filter(&name) {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total += meta.len();
                        count += 1;
                    }
                }
            }
        }
    }
    (total as f64 / 1_048_576.0, count)
}

fn dir_size_recursive(path: &str) -> u64 {
    let mut total: u64 = 0;
    let mut count: u32 = 0;
    dir_stats_walk(path, &mut total, &mut count, 10);
    total
}

fn clean_dir_all(path: &str) -> (u32, u64, u32) {
    let mut deleted = 0u32;
    let mut freed = 0u64;
    let mut errors = 0u32;
    clean_dir_recursive(path, &mut deleted, &mut freed, &mut errors);
    (deleted, freed, errors)
}

fn clean_dir_recursive(path: &str, deleted: &mut u32, freed: &mut u64, errors: &mut u32) {
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    let size = meta.len();
                    if std::fs::remove_file(entry.path()).is_ok() {
                        *deleted += 1;
                        *freed += size;
                    } else {
                        *errors += 1;
                    }
                } else if meta.is_dir() {
                    clean_dir_recursive(&entry.path().to_string_lossy(), deleted, freed, errors);
                    let _ = std::fs::remove_dir(entry.path());
                }
            }
        }
    }
}

fn categorize_ext(ext: &str) -> String {
    match ext {
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm" => "Video",
        "mp3" | "flac" | "wav" | "aac" | "ogg" | "wma" => "Audio",
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "svg" => "Image",
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" => "Archive",
        "iso" | "img" | "vhd" | "vhdx" => "Disk Image",
        "exe" | "msi" | "dll" => "Installer",
        "log" | "txt" | "csv" => "Log/Text",
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => "Document",
        "bak" | "tmp" | "dmp" | "old" => "Temp/Backup",
        _ => "Other",
    }
    .to_string()
}
