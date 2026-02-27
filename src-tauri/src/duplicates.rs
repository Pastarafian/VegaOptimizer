//! Duplicate File Finder — hash-based duplicate detection

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub file_size_mb: f64,
    pub count: usize,
    pub total_wasted_mb: f64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateFile {
    pub path: String,
    pub size_mb: f64,
    pub modified: String,
    pub extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanResult {
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicates: usize,
    pub total_wasted_mb: f64,
    pub files_scanned: usize,
    pub duration_ms: u64,
}

/// Scan for duplicate files in common user directories
pub fn scan_duplicates(min_size_mb: f64) -> DuplicateScanResult {
    let start = std::time::Instant::now();
    let min_bytes = (min_size_mb * 1_048_576.0) as u64;

    let user_profile = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into());
    let scan_dirs = vec![
        format!("{}\\Desktop", user_profile),
        format!("{}\\Documents", user_profile),
        format!("{}\\Downloads", user_profile),
        format!("{}\\Pictures", user_profile),
        format!("{}\\Videos", user_profile),
        format!("{}\\Music", user_profile),
    ];

    // Phase 1: Group files by size (fast pre-filter)
    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut files_scanned = 0usize;

    for dir in &scan_dirs {
        scan_directory(dir, &mut size_groups, min_bytes, &mut files_scanned, 0, 4);
    }

    // Phase 2: Only hash files that share the same size (potential duplicates)
    let mut hash_groups: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

    for (size, paths) in &size_groups {
        if paths.len() < 2 {
            continue;
        } // Need at least 2 files of same size

        for path in paths {
            if let Some(hash) = quick_hash(path) {
                hash_groups
                    .entry(hash)
                    .or_default()
                    .push((path.clone(), *size));
            }
        }
    }

    // Phase 3: Build duplicate groups
    let mut groups: Vec<DuplicateGroup> = Vec::new();

    for (hash, files) in &hash_groups {
        if files.len() < 2 {
            continue;
        }

        let file_size_mb = files[0].1 as f64 / 1_048_576.0;
        let dup_files: Vec<DuplicateFile> = files
            .iter()
            .map(|(path, size)| {
                let modified = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .map(|t| {
                        let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                        let secs = dur.as_secs();
                        let days = secs / 86400;
                        if days > 365 {
                            format!("{:.0}y ago", days as f64 / 365.0)
                        } else if days > 30 {
                            format!("{:.0}mo ago", days as f64 / 30.0)
                        } else if days > 0 {
                            format!("{}d ago", days)
                        } else {
                            "Today".into()
                        }
                    })
                    .unwrap_or_else(|_| "Unknown".into());

                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default();

                DuplicateFile {
                    path: path.to_string_lossy().to_string(),
                    size_mb: *size as f64 / 1_048_576.0,
                    modified,
                    extension: ext,
                }
            })
            .collect();

        groups.push(DuplicateGroup {
            hash: hash[..16].to_string(),
            file_size_mb,
            count: dup_files.len(),
            total_wasted_mb: file_size_mb * (dup_files.len() - 1) as f64,
            files: dup_files,
        });
    }

    groups.sort_by(|a, b| {
        b.total_wasted_mb
            .partial_cmp(&a.total_wasted_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let total_duplicates = groups.iter().map(|g| g.count - 1).sum();
    let total_wasted = groups.iter().map(|g| g.total_wasted_mb).sum();

    DuplicateScanResult {
        groups: groups.into_iter().take(100).collect(),
        total_duplicates,
        total_wasted_mb: total_wasted,
        files_scanned,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn scan_directory(
    dir: &str,
    size_groups: &mut HashMap<u64, Vec<PathBuf>>,
    min_bytes: u64,
    count: &mut usize,
    depth: u32,
    max_depth: u32,
) {
    if depth > max_depth {
        return;
    }
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() && meta.len() >= min_bytes {
                    *count += 1;
                    size_groups
                        .entry(meta.len())
                        .or_default()
                        .push(entry.path());
                } else if meta.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    // Skip hidden/system dirs
                    if !name.starts_with('.')
                        && name != "node_modules"
                        && name != ".git"
                        && name != "AppData"
                    {
                        scan_directory(
                            &entry.path().to_string_lossy(),
                            size_groups,
                            min_bytes,
                            count,
                            depth + 1,
                            max_depth,
                        );
                    }
                }
            }
        }
    }
}

/// Quick hash using first+last 8KB + size for speed
fn quick_hash(path: &PathBuf) -> Option<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    let size = meta.len();

    let mut hasher = DefaultHasher::new();
    size.hash(&mut hasher);

    // Read first 8KB
    let mut buf = vec![0u8; 8192.min(size as usize)];
    file.read_exact(&mut buf).ok()?;
    buf.hash(&mut hasher);

    // Read last 8KB if file is large enough
    if size > 16384 {
        use std::io::Seek;
        file.seek(std::io::SeekFrom::End(-8192)).ok()?;
        let mut end_buf = vec![0u8; 8192];
        file.read_exact(&mut end_buf).ok()?;
        end_buf.hash(&mut hasher);
    }

    Some(format!("{:016x}", hasher.finish()))
}

/// Delete a specific duplicate file
pub fn delete_duplicate(path: &str) -> Result<String, String> {
    // Safety: don't delete from system dirs
    let lower = path.to_lowercase();
    if lower.contains("\\windows\\")
        || lower.contains("\\program files")
        || lower.contains("\\system32")
    {
        return Err("Cannot delete files from system directories".into());
    }

    match std::fs::remove_file(path) {
        Ok(_) => Ok(format!("Deleted: {}", path)),
        Err(e) => Err(format!("Failed to delete: {}", e)),
    }
}
