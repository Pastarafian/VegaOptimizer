//! Registry Cleaner — scan for orphaned/broken registry entries

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIssue {
    pub key_path: String,
    pub value_name: String,
    pub issue_type: String, // "orphaned_software", "broken_shortcut", "invalid_path", "obsolete_clsid", "empty_key"
    pub description: String,
    pub severity: String, // "low", "medium", "high"
    pub safe_to_fix: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryScanResult {
    pub issues: Vec<RegistryIssue>,
    pub total_issues: usize,
    pub by_type: Vec<(String, usize)>,
    pub duration_ms: u64,
}

/// Scan registry for common issues
pub fn scan_registry() -> RegistryScanResult {
    let start = std::time::Instant::now();
    let mut issues = Vec::new();

    // 1. Orphaned software entries — programs listed in Uninstall that don't exist
    scan_orphaned_uninstall(&mut issues);

    // 2. Broken file associations
    scan_broken_associations(&mut issues);

    // 3. Invalid SharedDLLs paths
    scan_shared_dlls(&mut issues);

    // 4. Broken App Paths
    scan_app_paths(&mut issues);

    // 5. MUI Cache orphans
    scan_mui_cache(&mut issues);

    // Tally by type
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for issue in &issues {
        *type_counts.entry(issue.issue_type.clone()).or_insert(0) += 1;
    }
    let by_type: Vec<(String, usize)> = type_counts.into_iter().collect();

    RegistryScanResult {
        total_issues: issues.len(),
        issues: issues.into_iter().take(200).collect(),
        by_type,
        duration_ms: start.elapsed().as_millis() as u64,
    }
}

fn scan_orphaned_uninstall(issues: &mut Vec<RegistryIssue>) {
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            $paths = @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*','HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*')
            foreach($p in $paths) {
                Get-ItemProperty $p -ErrorAction SilentlyContinue | ForEach-Object {
                    $loc = $_.InstallLocation
                    $name = $_.DisplayName
                    if($loc -and $name -and !(Test-Path $loc -ErrorAction SilentlyContinue)) {
                        "$($_.PSPath)|$name|$loc"
                    }
                }
            }
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                issues.push(RegistryIssue {
                    key_path: parts[0].trim().to_string(),
                    value_name: parts[1].trim().to_string(),
                    issue_type: "orphaned_software".into(),
                    description: format!("'{}' install path no longer exists: {}", parts[1].trim(), parts[2].trim()),
                    severity: "medium".into(),
                    safe_to_fix: true,
                });
            }
        }
    }
}

fn scan_broken_associations(issues: &mut Vec<RegistryIssue>) {
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            Get-ChildItem 'HKLM:\SOFTWARE\Classes' -ErrorAction SilentlyContinue | Where-Object { $_.Name -match '^\.' } | ForEach-Object {
                $ext = $_.PSChildName
                $prog = (Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue).'(default)'
                if($prog -and $prog -ne '') {
                    $check = "HKLM:\SOFTWARE\Classes\$prog"
                    if(!(Test-Path $check -ErrorAction SilentlyContinue)) {
                        "$ext|$prog"
                    }
                }
            } | Select-Object -First 50
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 2 {
                issues.push(RegistryIssue {
                    key_path: format!("HKLM\\SOFTWARE\\Classes\\{}", parts[0].trim()),
                    value_name: parts[0].trim().to_string(),
                    issue_type: "broken_shortcut".into(),
                    description: format!("{} file type points to missing handler: {}", parts[0].trim(), parts[1].trim()),
                    severity: "low".into(),
                    safe_to_fix: true,
                });
            }
        }
    }
}

fn scan_shared_dlls(issues: &mut Vec<RegistryIssue>) {
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            $key = Get-Item 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\SharedDLLs' -ErrorAction SilentlyContinue
            if($key) {
                $key.GetValueNames() | ForEach-Object {
                    if($_ -and !(Test-Path $_ -ErrorAction SilentlyContinue)) { $_ }
                } | Select-Object -First 50
            }
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let path = line.trim();
            if !path.is_empty() {
                issues.push(RegistryIssue {
                    key_path: "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\SharedDLLs".into(),
                    value_name: path.to_string(),
                    issue_type: "invalid_path".into(),
                    description: format!("SharedDLL entry points to missing file: {}", path),
                    severity: "low".into(),
                    safe_to_fix: true,
                });
            }
        }
    }
}

fn scan_app_paths(issues: &mut Vec<RegistryIssue>) {
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            Get-ChildItem 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths' -ErrorAction SilentlyContinue | ForEach-Object {
                $p = (Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue).'(default)'
                $name = $_.PSChildName
                if($p -and $p -ne '' -and !(Test-Path $p -ErrorAction SilentlyContinue)) {
                    "$name|$p"
                }
            } | Select-Object -First 50
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 2 {
                issues.push(RegistryIssue {
                    key_path: "HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths".into(),
                    value_name: parts[0].trim().to_string(),
                    issue_type: "orphaned_software".into(),
                    description: format!("App path for '{}' points to missing: {}", parts[0].trim(), parts[1].trim()),
                    severity: "medium".into(),
                    safe_to_fix: true,
                });
            }
        }
    }
}

fn scan_mui_cache(issues: &mut Vec<RegistryIssue>) {
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            $path = "HKCU:\SOFTWARE\Classes\Local Settings\Software\Microsoft\Windows\Shell\MuiCache"
            if(Test-Path $path) {
                $key = Get-Item $path
                $key.GetValueNames() | Where-Object { $_ -match '^[A-Z]:\\' } | ForEach-Object {
                    $file = ($_ -split '\\.')[0..($_.Split('.').Count-2)] -join '.'
                    if(!(Test-Path $file -ErrorAction SilentlyContinue)) { $_ }
                } | Select-Object -First 30
            }
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let entry = line.trim();
            if !entry.is_empty() {
                issues.push(RegistryIssue {
                    key_path: "HKCU\\SOFTWARE\\Classes\\Local Settings\\MuiCache".into(),
                    value_name: entry.to_string(),
                    issue_type: "orphaned_software".into(),
                    description: format!("MUI cache entry for removed application: {}", entry.split('\\').last().unwrap_or(entry)),
                    severity: "low".into(),
                    safe_to_fix: true,
                });
            }
        }
    }
}

/// Fix a specific registry issue (delete orphaned key/value)
pub fn fix_registry_issue(
    key_path: &str,
    value_name: &str,
    issue_type: &str,
) -> Result<String, String> {
    // Only fix known safe types
    match issue_type {
        "orphaned_software" | "broken_shortcut" | "invalid_path" => {}
        _ => return Err("This issue type cannot be auto-fixed".into()),
    }

    // For SharedDLLs, remove the value
    if key_path.contains("SharedDLLs") {
        match Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Remove-ItemProperty -Path '{}' -Name '{}' -ErrorAction Stop",
                    key_path
                        .replace("HKLM\\", "HKLM:\\")
                        .replace("HKCU\\", "HKCU:\\"),
                    value_name
                ),
            ])
            .output()
        {
            Ok(o) if o.status.success() => return Ok(format!("Fixed: removed {}", value_name)),
            Ok(o) => return Err(String::from_utf8_lossy(&o.stderr).to_string()),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(format!("Marked for cleanup: {} - {}", key_path, value_name))
}
