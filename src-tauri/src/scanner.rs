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
    pub ai_tooltip: Option<String>,
}

pub fn scan_large_files(min_size_mb: u64, max_results: usize) -> Vec<LargeFile> {
    let mut files: Vec<LargeFile> = Vec::new();
    let min_bytes = min_size_mb * 1_048_576;

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

    let sys_drive = format!("{}\\" , std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()));

    let mut stack = vec![
        (std::env::var("USERPROFILE").unwrap_or_default(), 0),
        (sys_drive, 0),
    ];

    while let Some((dir, depth)) = stack.pop() {
        if dir.is_empty() || depth > 8 {
            // Max depth 8
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        if !skip_dirs.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                            stack.push((path.to_string_lossy().to_string(), depth + 1));
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
                                    .unwrap_or_default()
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
                            ai_tooltip: None,
                        });
                    }
                }
            }
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

fn categorize_extension(ext: &str) -> String {
    match ext {
        // ── Video ────────────────────────────────────────────────────
        | "mp4" | "m4v" | "avi" | "mkv" | "mov" | "wmv" | "flv" | "webm"
        | "mpg" | "mpeg" | "m2v" | "m2ts" | "mts" | "ts" | "vob" | "ogv"
        | "rm" | "rmvb" | "3gp" | "3g2" | "divx" | "xvid" | "asf" | "f4v"
        | "hevc" | "h264" | "h265" => "Video",

        // ── Audio ────────────────────────────────────────────────────
        | "mp3" | "flac" | "wav" | "aac" | "ogg" | "oga" | "wma" | "m4a"
        | "alac" | "aiff" | "aif" | "ape" | "opus" | "wv" | "mka" | "mid"
        | "midi" | "amr" | "au" | "ra" | "dts" | "ac3" | "eac3" => "Audio",

        // ── Image (raster + vector) ──────────────────────────────────
        | "jpg" | "jpeg" | "jfif" | "png" | "gif" | "bmp" | "tif" | "tiff"
        | "webp" | "svg" | "svgz" | "ico" | "cur" | "pcx" | "ppm" | "pgm"
        | "pbm" | "tga" | "xbm" | "xpm" | "hdr" | "exr" | "avif" => "Image",

        // ── RAW Photo ─────────────────────────────────────────────────
        | "raw" | "cr2" | "cr3" | "nef" | "nrw" | "arw" | "srf" | "sr2"
        | "dng" | "orf" | "rw2" | "pef" | "kdc" | "dcr" | "mrw" | "raf"
        | "x3f" | "erf" | "mef" | "3fr" | "fff" => "RAW Photo",

        // ── Archive / Compression ─────────────────────────────────────
        | "zip" | "zipx" | "rar" | "r00" | "7z" | "tar" | "gz" | "tgz"
        | "bz2" | "tbz" | "tbz2" | "xz" | "txz" | "lzma" | "lz" | "lz4"
        | "zst" | "cab" | "z" | "arj" | "ace" | "lha" | "lzh" | "wim"
        | "swm" | "esd" => "Archive",

        // ── Disk / Virtual Machine Image ──────────────────────────────
        | "iso" | "img" | "mdf" | "mds" | "nrg" | "cue" | "bin" | "ccd"
        | "sub" | "toast" | "dmg" | "vhd" | "vhdx" | "vmdk" | "vdi"
        | "qcow" | "qcow2" | "hdd" | "wbfs" | "gdi" | "cdi" | "chd"
        | "xci" | "nsp" => "Disk Image",

        // ── Application / Executable ──────────────────────────────────
        | "exe" | "msi" | "msix" | "appx" | "dll" | "sys" | "drv" | "ocx"
        | "com" | "scr" | "cpl" | "app" | "pkg" | "ipa" | "apk" | "aab"
        | "xapk" | "deb" | "rpm" | "flatpak" | "snap" | "appimage"
        | "jar" | "war" | "ear" => "Application",

        // ── Source Code ───────────────────────────────────────────────
        | "c" | "h" | "cpp" | "cxx" | "cc" | "hpp" | "hxx" | "cs" | "vb"
        | "java" | "class" | "kt" | "kts" | "swift" | "m" | "mm" | "rs"
        | "go" | "py" | "pyx" | "pxd" | "rb" | "rake" | "pl" | "pm"
        | "lua" | "r" | "jl" | "f" | "f90" | "f95" | "f03" | "asm" | "s"
        | "dart" | "scala" | "groovy" | "clj" | "cljs" | "ex" | "exs"
        | "erl" | "hrl" | "hs" | "lhs" | "ml" | "mli" | "nim" | "v"
        | "sv" | "vhdl" | "zig" => "Source Code",

        // ── Script / Automation ───────────────────────────────────────
        | "js" | "mjs" | "cjs" | "tsx" | "jsx" | "coffee" | "sh"
        | "bash" | "zsh" | "fish" | "bat" | "cmd" | "ps1" | "psm1" | "psd1"
        | "vbs" | "vbe" | "wsf" | "ahk" | "au3" | "tcl" | "awk" | "sed"
        | "php" | "php3" | "php4" | "php5" | "phtml" => "Script",

        // ── Web ───────────────────────────────────────────────────────
        | "html" | "htm" | "xhtml" | "css" | "scss" | "sass" | "less"
        | "json" | "json5" | "jsonc" | "xml" | "xsl" | "xslt" | "yaml"
        | "yml" | "toml" | "ini" | "cfg" | "conf" | "env" | "graphql"
        | "wasm" => "Web / Config",

        // ── Document ──────────────────────────────────────────────────
        | "pdf" | "doc" | "docx" | "docm" | "dot" | "dotx" | "odt" | "ott"
        | "rtf" | "wps" | "wpd" | "pages" | "tex" | "bib" | "eps" | "ps"
        | "epub" | "mobi" | "azw" | "azw3" | "fb2" | "lit" | "djvu"
        | "xps" | "oxps" => "Document",

        // ── Spreadsheet / Data ────────────────────────────────────────
        | "xls" | "xlsx" | "xlsm" | "xlsb" | "xlt" | "xltx" | "ods" | "ots"
        | "csv" | "tsv" | "numbers" | "ppt" | "pptx" | "pptm" | "pot"
        | "potx" | "odp" | "otp" | "key" => "Presentation / Sheet",

        // ── Text / Log ────────────────────────────────────────────────
        | "txt" | "log" | "md" | "rst" | "nfo" | "diz" | "asc"
        | "readme" | "changelog" | "license" => "Text / Log",

        // ── Database ──────────────────────────────────────────────────
        | "db" | "sqlite" | "sqlite3" | "db3" | "ldf" | "ndf"
        | "frm" | "ibd" | "ibdata" | "myd" | "myi"
        | "accdb" | "accde" | "mdb" | "dbf" | "fdb" | "gdb" | "nsf"
        | "realm" => "Database",

        // ── AI / ML Model Weights ─────────────────────────────────────
        | "safetensors" | "ckpt" | "pt" | "pth" | "gguf" | "ggml" | "onnx"
        | "h5" | "hdf5" | "pb" | "tflite" | "mlmodel" | "mlpackage"
        | "pdparams" | "npy" | "npz" | "pkl" | "pickle" | "joblib" => "AI Model",

        // ── 3D Model / CAD ────────────────────────────────────────────
        | "obj" | "fbx" | "stl" | "blend" | "3ds" | "dae" | "gltf" | "glb"
        | "ply" | "abc" | "usd" | "usda" | "usdc" | "usdz" | "step" | "stp"
        | "iges" | "igs" | "sldprt" | "sldasm" | "f3d" | "skp" | "ifc"
        | "dwg" | "dxf" | "3mf" | "x3d" | "wrl" | "vrml" => "3D / CAD",

        // ── Game / ROM / Emulator ─────────────────────────────────────
        | "nes" | "smc" | "sfc" | "gb" | "gbc" | "gba" | "n64" | "z64"
        | "v64" | "nds" | "cia" | "gen" | "sms" | "gg"
        | "psx" | "ps2" | "pbp" | "nca" | "wad"
        | "gcm" | "ciso" | "rvz" | "bsp" | "pk3" | "pk4"
        | "vpk" | "assets" | "unity" | "unitypackage" | "uasset" | "upk"
        | "pck" | "big" | "forge" => "Game / ROM",

        // ── Font ──────────────────────────────────────────────────────
        | "ttf" | "otf" | "woff" | "woff2" | "eot" | "fnt" | "fon" | "pfm"
        | "pfb" | "afm" | "dfont" | "suit" | "pcf" | "bdf" => "Font",

        // ── Certificate / Security ────────────────────────────────────
        | "pem" | "crt" | "cer" | "der" | "pfx" | "p12" | "p7b" | "p7c"
        | "p8" | "csr" | "jks" | "keystore" | "pvk" | "spc" => "Certificate",

        // ── Design / DAW / Project ────────────────────────────────────
        | "psd" | "psb" | "xcf" | "kra" | "afdesign" | "afphoto" | "afpub"
        | "ai" | "indd" | "idml" | "qxd" | "qxp" | "sketch" | "fig"
        | "aep" | "aepx" | "prproj" | "ppj" | "drp" | "kdenlive"
        | "flp" | "als" | "logic" | "band" | "ptx" | "pts" | "ptf"
        | "reason" | "rns" | "nki" | "nkx" | "c4d" | "max" | "ma" | "mb"
        | "hip" | "hiplc" | "zpr" => "Design / DAW",

        // ── Backup / Temp ─────────────────────────────────────────────
        | "bak" | "old" | "orig" | "tmp" | "temp" | "dmp" | "mdmp" | "swp"
        | "swo" | "lock" | "pid" => "Backup / Temp",

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

    // Chrome and Edge now bury cache in Cache/Cache_Data
    let mut actual_paths = Vec::new();
    for p in &cache_paths {
        actual_paths.push(p.clone());
        actual_paths.push(format!("{}\\Cache_Data", p));
        actual_paths.push(format!("{}\\js", p));
    }

    let mut total_freed: u64 = 0;
    let mut files_deleted: u32 = 0;

    for path in &actual_paths {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    let size = meta.len();
                    let entry_path = entry.path();
                    if meta.is_dir() {
                        if std::fs::remove_dir_all(&entry_path).is_ok() {
                            total_freed += size; // Size of dir itself might be small, but it's something.
                        }
                    } else if meta.is_file() {
                        if std::fs::remove_file(&entry_path).is_ok() {
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
            data_size_mb: dir_size_mb(&format!("{}\\Prefetch", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))),
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
            let prefetch = format!("{}\\Prefetch", std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()));
            let count = clean_dir_files(&prefetch);
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

    let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());

    // Software Distribution Download
    let dl_path = format!("{}\\SoftwareDistribution\\Download", sys_root);
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
