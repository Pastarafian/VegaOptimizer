mod battery;
mod benchmark;
mod debloater;
mod disk_cleanup;
mod disk_health;
mod duplicates;
mod monitor;
mod network;
mod optimizer;
mod registry;
mod scanner;
mod services;
mod startup;

use monitor::{get_hardware_info, get_health_score, get_live_metrics};
use optimizer::{get_optimization_catalog, get_processes, get_system_info, run_optimization};
use scanner::{
    clean_browser_cache, clean_privacy_item, clean_windows_update, detect_browsers,
    get_privacy_items, list_drivers, scan_large_files,
};
use startup::{list_startup_programs, toggle_startup};

// ═══════════════════════════════════════════════════════════════════════════════
// Helper — run blocking code on a background thread (prevents UI freezing)
// ═══════════════════════════════════════════════════════════════════════════════

/// Wraps a blocking closure in tokio's spawn_blocking, used by every command.
async fn bg<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(f).await.unwrap()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Original Optimizer (all async now)
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_get_system_info() -> optimizer::SystemInfo {
    bg(get_system_info).await
}

#[tauri::command]
async fn cmd_get_processes() -> Vec<optimizer::ProcessInfo> {
    bg(get_processes).await
}

#[tauri::command]
async fn cmd_get_catalog() -> Vec<optimizer::OptimizationItem> {
    bg(get_optimization_catalog).await
}

#[tauri::command]
async fn cmd_optimize(ids: Vec<String>) -> optimizer::OptimizationReport {
    bg(move || run_optimization(ids)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Live Monitoring
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_get_live_metrics() -> monitor::LiveMetrics {
    bg(get_live_metrics).await
}

#[tauri::command]
async fn cmd_get_health_score() -> monitor::HealthScore {
    bg(get_health_score).await
}

#[tauri::command]
async fn cmd_get_hardware_info() -> monitor::HardwareInfo {
    bg(get_hardware_info).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Startup Manager
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_list_startup() -> Vec<startup::StartupEntry> {
    bg(list_startup_programs).await
}

#[tauri::command]
async fn cmd_toggle_startup(
    name: String,
    registry_path: String,
    enable: bool,
) -> Result<String, String> {
    bg(move || toggle_startup(&name, &registry_path, enable)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Scanner / Cleanup
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_scan_large_files(min_size_mb: u64) -> Vec<scanner::LargeFile> {
    bg(move || scan_large_files(min_size_mb, 100)).await
}

#[tauri::command]
async fn cmd_delete_file(path: String) -> Result<String, String> {
    bg(move || {
        let p = std::path::Path::new(&path);
        if !p.exists() {
            return Err("File not found".to_string());
        }
        if !p.is_file() {
            return Err("Not a file".to_string());
        }
        // Safety: refuse to delete from system dirs
        let lower = path.to_lowercase();
        if lower.starts_with("c:\\windows") || lower.starts_with("c:\\program files") {
            return Err("Cannot delete system files".to_string());
        }
        let size = p.metadata().map(|m| m.len()).unwrap_or(0);
        match std::fs::remove_file(p) {
            Ok(_) => Ok(format!(
                "Deleted {} ({:.1} MB)",
                path,
                size as f64 / 1_048_576.0
            )),
            Err(e) => Err(format!("Failed to delete: {}", e)),
        }
    })
    .await
}

#[tauri::command]
async fn cmd_reveal_file(path: String) -> Result<(), String> {
    bg(move || {
        let p = std::path::Path::new(&path);
        if !p.exists() {
            return Err("File not found".to_string());
        }
        std::process::Command::new("explorer")
            .args(&["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
}

#[tauri::command]
async fn cmd_detect_browsers() -> Vec<scanner::BrowserInfo> {
    bg(detect_browsers).await
}

#[tauri::command]
async fn cmd_clean_browser(name: String) -> Result<String, String> {
    bg(move || clean_browser_cache(&name)).await
}

#[tauri::command]
async fn cmd_get_privacy_items() -> Vec<scanner::PrivacyItem> {
    bg(get_privacy_items).await
}

#[tauri::command]
async fn cmd_clean_privacy(id: String) -> Result<String, String> {
    bg(move || clean_privacy_item(&id)).await
}

#[tauri::command]
async fn cmd_list_drivers() -> Vec<scanner::DriverInfo> {
    bg(list_drivers).await
}

#[tauri::command]
async fn cmd_clean_windows_update() -> Result<String, String> {
    bg(clean_windows_update).await
}

#[tauri::command]
async fn cmd_kill_process(pid: u32) -> Result<String, String> {
    bg(move || {
        match std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()
        {
            Ok(o) if o.status.success() => Ok(format!("Killed process {}", pid)),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

#[derive(serde::Serialize)]
struct ProcessSuggestion {
    pid: u32,
    name: String,
    memory_mb: f64,
    cpu_percent: f32,
    estimated_savings_mb: f64,
    reason: String,
    severity: String, // "high", "medium", "low"
    category: String, // "bloated", "idle_hog", "background", "duplicate"
    safe_to_optimize: bool,
}

#[derive(serde::Serialize)]
struct ProcessOptReport {
    total_freed_mb: f64,
    processes_trimmed: usize,
    results: Vec<ProcessOptResult>,
}

#[derive(serde::Serialize)]
struct ProcessOptResult {
    pid: u32,
    name: String,
    memory_before_mb: f64,
    memory_after_mb: f64,
    freed_mb: f64,
    success: bool,
    message: String,
}

/// Protected system processes that should never be optimized
const PROTECTED_PROCESSES: &[&str] = &[
    "system",
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "svchost.exe",
    "winlogon.exe",
    "dwm.exe",
    "explorer.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "ntoskrnl.exe",
    "registry",
    "memory compression",
    "secure system",
    "system idle process",
];

#[tauri::command]
async fn cmd_get_process_suggestions() -> Vec<ProcessSuggestion> {
    bg(|| {
        use sysinfo::{ProcessesToUpdate, System};

        let mut sys = System::new_all();
        sys.refresh_all();
        std::thread::sleep(std::time::Duration::from_millis(200));
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut suggestions: Vec<ProcessSuggestion> = Vec::new();

        // Count process instances for duplicate detection
        let mut name_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut name_memory: std::collections::HashMap<String, f64> =
            std::collections::HashMap::new();
        for (_pid, proc_) in sys.processes() {
            let name = proc_.name().to_string_lossy().to_lowercase();
            *name_counts.entry(name.clone()).or_insert(0) += 1;
            *name_memory.entry(name).or_insert(0.0) += proc_.memory() as f64 / 1_048_576.0;
        }

        for (pid, proc_) in sys.processes() {
            let name_lower = proc_.name().to_string_lossy().to_lowercase();
            let name = proc_.name().to_string_lossy().to_string();
            let mem = proc_.memory() as f64 / 1_048_576.0;
            let cpu = proc_.cpu_usage();

            if PROTECTED_PROCESSES.contains(&name_lower.as_str()) {
                continue;
            }
            if mem < 2.0 {
                continue;
            }

            // High memory (>200 MB) & low CPU (<2%) => bloated/idle
            if mem > 200.0 && cpu < 2.0 {
                suggestions.push(ProcessSuggestion {
                    pid: pid.as_u32(),
                    name: name.clone(),
                    memory_mb: mem,
                    cpu_percent: cpu,
                    estimated_savings_mb: mem * 0.3,
                    reason: format!(
                        "{:.0} MB used with {:.1}% CPU — likely idle bloat",
                        mem, cpu
                    ),
                    severity: "high".into(),
                    category: "bloated".into(),
                    safe_to_optimize: true,
                });
            }
            // Medium memory (50–200 MB) & idle
            else if mem > 50.0 && cpu < 1.0 {
                suggestions.push(ProcessSuggestion {
                    pid: pid.as_u32(),
                    name: name.clone(),
                    memory_mb: mem,
                    cpu_percent: cpu,
                    estimated_savings_mb: mem * 0.2,
                    reason: format!(
                        "{:.0} MB used, completely idle — memory can be trimmed",
                        mem
                    ),
                    severity: "medium".into(),
                    category: "idle_hog".into(),
                    safe_to_optimize: true,
                });
            }

            // Duplicate processes using >100 MB total
            let count = name_counts.get(&name_lower).copied().unwrap_or(0);
            let total_mem = name_memory.get(&name_lower).copied().unwrap_or(0.0);
            if count > 3 && total_mem > 100.0 && mem > 20.0 {
                let already = suggestions
                    .iter()
                    .any(|s| s.name.to_lowercase() == name_lower && s.category == "duplicate");
                if !already {
                    suggestions.push(ProcessSuggestion {
                        pid: pid.as_u32(),
                        name: name.clone(),
                        memory_mb: mem,
                        cpu_percent: cpu,
                        estimated_savings_mb: total_mem * 0.15,
                        reason: format!("{} instances using {:.0} MB total", count, total_mem),
                        severity: "medium".into(),
                        category: "duplicate".into(),
                        safe_to_optimize: true,
                    });
                }
            }

            // Background processes (>30 MB, zero CPU, not in previous categories)
            if mem > 30.0 && cpu < 0.5 && !suggestions.iter().any(|s| s.pid == pid.as_u32()) {
                suggestions.push(ProcessSuggestion {
                    pid: pid.as_u32(),
                    name: name.clone(),
                    memory_mb: mem,
                    cpu_percent: cpu,
                    estimated_savings_mb: mem * 0.15,
                    reason: format!(
                        "Background process using {:.0} MB with no CPU activity",
                        mem
                    ),
                    severity: "low".into(),
                    category: "background".into(),
                    safe_to_optimize: true,
                });
            }
        }

        // Sort: high severity first, then by memory
        suggestions.sort_by(|a, b| {
            let sev = |s: &str| match s {
                "high" => 0,
                "medium" => 1,
                _ => 2,
            };
            sev(&a.severity).cmp(&sev(&b.severity)).then(
                b.memory_mb
                    .partial_cmp(&a.memory_mb)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        suggestions.truncate(50);
        suggestions
    })
    .await
}

#[tauri::command]
async fn cmd_optimize_processes(pids: Vec<u32>) -> ProcessOptReport {
    bg(move || {
        use sysinfo::{Pid, ProcessesToUpdate, System};

        // ── Enable SeDebugPrivilege (required to trim other processes' working sets) ──
        #[cfg(windows)]
        {
            enable_debug_privilege();
        }

        let mut sys = System::new_all();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let mut results: Vec<ProcessOptResult> = Vec::new();

        for &pid in &pids {
            let before_mb = sys
                .process(Pid::from_u32(pid))
                .map(|p| p.memory() as f64 / 1_048_576.0)
                .unwrap_or(0.0);

            let name = sys
                .process(Pid::from_u32(pid))
                .map(|p| p.name().to_string_lossy().to_string())
                .unwrap_or_else(|| format!("PID {}", pid));

            let success;
            let message;

            #[cfg(windows)]
            {
                use winapi::um::errhandlingapi::GetLastError;
                use winapi::um::handleapi::CloseHandle;
                use winapi::um::processthreadsapi::OpenProcess;
                use winapi::um::psapi::EmptyWorkingSet;
                use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA};

                unsafe {
                    let handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, 0, pid);
                    if handle.is_null() {
                        let err = GetLastError();
                        success = false;
                        message = format!(
                            "Cannot open process (error {}{})",
                            err,
                            if err == 5 {
                                " — run as Administrator"
                            } else {
                                ""
                            }
                        );
                    } else {
                        let r = EmptyWorkingSet(handle);
                        if r != 0 {
                            success = true;
                            message = "Working set trimmed".to_string();
                        } else {
                            let err = GetLastError();
                            success = false;
                            message = format!("EmptyWorkingSet failed (error {})", err);
                        }
                        CloseHandle(handle);
                    }
                }
            }

            #[cfg(not(windows))]
            {
                success = false;
                message = "Not supported on this platform".to_string();
            }

            results.push(ProcessOptResult {
                pid,
                name,
                memory_before_mb: before_mb,
                memory_after_mb: 0.0,
                freed_mb: 0.0,
                success,
                message,
            });
        }

        // Re-scan to measure actual memory freed
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut sys2 = System::new_all();
        sys2.refresh_processes(ProcessesToUpdate::All, true);

        let mut total_freed = 0.0;
        for result in &mut results {
            if result.success {
                let after = sys2
                    .process(Pid::from_u32(result.pid))
                    .map(|p| p.memory() as f64 / 1_048_576.0)
                    .unwrap_or(0.0);
                result.memory_after_mb = after;
                result.freed_mb = (result.memory_before_mb - after).max(0.0);
                result.message = format!("Freed {:.1} MB", result.freed_mb);
                total_freed += result.freed_mb;
            }
        }

        ProcessOptReport {
            total_freed_mb: total_freed,
            processes_trimmed: results.iter().filter(|r| r.success).count(),
            results,
        }
    })
    .await
}

/// Enable SeDebugPrivilege so we can call EmptyWorkingSet on any process
#[cfg(windows)]
fn enable_debug_privilege() {
    use std::ptr::null_mut;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::AdjustTokenPrivileges;
    use winapi::um::winbase::LookupPrivilegeValueA;
    use winapi::um::winnt::{
        LUID, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
    };

    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        ) == 0
        {
            return;
        }

        let mut luid = LUID {
            LowPart: 0,
            HighPart: 0,
        };
        let priv_name = b"SeDebugPrivilege\0";
        if LookupPrivilegeValueA(null_mut(), priv_name.as_ptr() as *const i8, &mut luid) == 0 {
            CloseHandle(token);
            return;
        }

        let mut tp: TOKEN_PRIVILEGES = std::mem::zeroed();
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = luid;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        AdjustTokenPrivileges(token, 0, &mut tp, 0, null_mut(), null_mut());

        CloseHandle(token);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Network Monitor
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_get_network_overview() -> network::NetworkOverview {
    bg(|| network::get_network_connections()).await
}

#[tauri::command]
async fn cmd_ping_test(host: String) -> f64 {
    bg(move || network::ping_test(&host)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Windows Debloater
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_list_appx() -> Vec<debloater::AppxPackage> {
    bg(|| debloater::list_appx_packages()).await
}

#[tauri::command]
async fn cmd_remove_appx(name: String) -> Result<String, String> {
    bg(move || debloater::remove_appx_package(&name)).await
}

#[tauri::command]
async fn cmd_remove_all_bloatware() -> Vec<(String, bool, String)> {
    bg(|| debloater::remove_all_bloatware()).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — System Benchmark
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_run_benchmark() -> benchmark::BenchmarkResult {
    bg(|| benchmark::run_benchmark()).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Disk Health
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_get_disk_health() -> Vec<disk_health::DiskHealthInfo> {
    bg(|| disk_health::get_disk_health()).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Duplicate Finder
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_scan_duplicates(min_size_mb: f64) -> duplicates::DuplicateScanResult {
    bg(move || duplicates::scan_duplicates(min_size_mb)).await
}

#[tauri::command]
async fn cmd_delete_duplicate(path: String) -> Result<String, String> {
    bg(move || duplicates::delete_duplicate(&path)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Services Manager
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_list_services() -> Vec<services::ServiceInfo> {
    bg(|| services::list_services()).await
}

#[tauri::command]
async fn cmd_start_service(name: String) -> Result<String, String> {
    bg(move || services::start_service(&name)).await
}

#[tauri::command]
async fn cmd_stop_service(name: String) -> Result<String, String> {
    bg(move || services::stop_service(&name)).await
}

#[tauri::command]
async fn cmd_set_service_startup(name: String, startup: String) -> Result<String, String> {
    bg(move || services::set_service_startup(&name, &startup)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Registry Cleaner
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_scan_registry() -> registry::RegistryScanResult {
    bg(|| registry::scan_registry()).await
}

#[tauri::command]
async fn cmd_fix_registry_issue(
    key_path: String,
    value_name: String,
    issue_type: String,
) -> Result<String, String> {
    bg(move || registry::fix_registry_issue(&key_path, &value_name, &issue_type)).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Battery Health
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_get_battery_health() -> battery::BatteryHealth {
    bg(|| battery::get_battery_health()).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Driver Management
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_scan_driver_updates() -> Result<String, String> {
    bg(|| {
        match std::process::Command::new("pnputil")
            .args(["/scan-devices"])
            .output()
        {
            Ok(o) => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

#[tauri::command]
async fn cmd_open_device_manager() -> Result<String, String> {
    bg(|| {
        match std::process::Command::new("cmd")
            .args(["/C", "start devmgmt.msc"])
            .output()
        {
            Ok(_) => Ok("Device Manager opened".into()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

#[tauri::command]
async fn cmd_open_windows_update() -> Result<String, String> {
    bg(|| {
        match std::process::Command::new("cmd")
            .args(["/C", "start ms-settings:windowsupdate"])
            .output()
        {
            Ok(_) => Ok("Windows Update opened".into()),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tauri Commands — Disk Cleanup
// ═══════════════════════════════════════════════════════════════════════════════

#[tauri::command]
async fn cmd_scan_junk() -> Vec<disk_cleanup::JunkCategory> {
    bg(|| disk_cleanup::scan_junk_categories()).await
}

#[tauri::command]
async fn cmd_clean_junk_category(id: String) -> Result<disk_cleanup::CleanResult, String> {
    bg(move || disk_cleanup::clean_junk_category(&id)).await
}

#[tauri::command]
async fn cmd_scan_app_caches() -> Vec<disk_cleanup::AppCache> {
    bg(|| disk_cleanup::scan_app_caches()).await
}

#[tauri::command]
async fn cmd_clean_app_cache(app_name: String) -> Result<disk_cleanup::CleanResult, String> {
    bg(move || disk_cleanup::clean_app_cache(&app_name)).await
}

#[tauri::command]
async fn cmd_scan_stale_files(days: u64) -> Vec<disk_cleanup::StaleFile> {
    bg(move || disk_cleanup::scan_stale_files(days, 100)).await
}

#[tauri::command]
async fn cmd_list_installed_programs() -> Vec<disk_cleanup::InstalledProgram> {
    bg(|| disk_cleanup::list_installed_programs()).await
}

#[tauri::command]
async fn cmd_uninstall_program(command: String) -> Result<String, String> {
    bg(move || disk_cleanup::uninstall_program(&command)).await
}

#[tauri::command]
async fn cmd_list_restore_points() -> Vec<disk_cleanup::RestorePoint> {
    bg(|| disk_cleanup::list_restore_points()).await
}

#[tauri::command]
async fn cmd_delete_restore_point(seq: u32) -> Result<String, String> {
    bg(move || disk_cleanup::delete_restore_point(seq)).await
}

#[tauri::command]
async fn cmd_shred_file(path: String, passes: u32) -> Result<disk_cleanup::ShredResult, String> {
    bg(move || disk_cleanup::shred_file(&path, passes)).await
}

#[tauri::command]
async fn cmd_wipe_free_space(
    drive: String,
    passes: u32,
) -> Result<disk_cleanup::WipeProgress, String> {
    bg(move || disk_cleanup::wipe_free_space(&drive, passes)).await
}

#[tauri::command]
async fn cmd_get_ai_suggestions() -> Vec<disk_cleanup::AiSuggestion> {
    bg(|| disk_cleanup::get_ai_suggestions()).await
}

#[tauri::command]
async fn cmd_get_folder_sizes(root: String) -> Vec<disk_cleanup::FolderSize> {
    bg(move || disk_cleanup::get_folder_sizes(&root, 3)).await
}

#[tauri::command]
async fn cmd_deep_clean() -> disk_cleanup::DeepCleanResult {
    bg(|| disk_cleanup::deep_clean()).await
}

#[derive(serde::Serialize)]
pub struct ScheduledTask {
    pub name: String,
    pub status: String,
}

#[tauri::command]
async fn cmd_list_scheduled_tasks() -> Result<Vec<ScheduledTask>, String> {
    bg(|| {
        let out = std::process::Command::new("schtasks")
            .args(&["/query", "/fo", "csv", "/nh"])
            .output()
            .map_err(|e| e.to_string())?;
        let s = String::from_utf8_lossy(&out.stdout);
        let mut tasks = Vec::new();
        for line in s.lines() {
            let mut parts = line.split("\",\"");
            if let (Some(n), Some(_), Some(stat)) = (parts.next(), parts.next(), parts.next()) {
                tasks.push(ScheduledTask {
                    name: n.trim_start_matches('"').to_string(),
                    status: stat
                        .trim_end_matches('"')
                        .trim_end_matches('\r')
                        .to_string(),
                });
            }
        }
        Ok(tasks)
    })
    .await
}

#[tauri::command]
async fn cmd_toggle_scheduled_task(name: String, enable: bool) -> Result<String, String> {
    bg(move || {
        let action = if enable { "/Enable" } else { "/Disable" };
        let out = std::process::Command::new("schtasks")
            .args(&["/Change", "/TN", &name, action])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok("Success".into())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into())
        }
    })
    .await
}

#[tauri::command]
async fn cmd_enable_game_booster() -> Result<String, String> {
    bg(|| {
        // High performance scheme
        let _ = std::process::Command::new("powercfg")
            .args(&["/s", "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"])
            .output();
        
        // Stop non-essential heavy background services gracefully
        // We use powershell to allow it to silently fail if the service doesn't exist
        let _ = std::process::Command::new("powershell")
            .args(&[
                "-WindowStyle", "Hidden", 
                "-Command", 
                "Stop-Service -Name SysMain -Force -ErrorAction SilentlyContinue; Stop-Service -Name Spooler -Force -ErrorAction SilentlyContinue"
            ])
            .output();
            
        Ok("Game Booster Enabled".into())
    })
    .await
}

#[tauri::command]
async fn cmd_restore_normal_mode() -> Result<String, String> {
    bg(|| {
        // Balanced scheme
        let _ = std::process::Command::new("powercfg")
            .args(&["/s", "381b4222-f694-41f0-9685-ff5bb260df2e"])
            .output();
            
        // Restart standard services
        let _ = std::process::Command::new("powershell")
            .args(&[
                "-WindowStyle", "Hidden", 
                "-Command", 
                "Start-Service -Name SysMain -ErrorAction SilentlyContinue; Start-Service -Name Spooler -ErrorAction SilentlyContinue"
            ])
            .output();
            
        Ok("Restored Normal Mode".into())
    })
    .await
}

#[tauri::command]
async fn cmd_toggle_telemetry(setting: String, disable: bool) -> Result<String, String> {
    bg(move || {
        let val = if disable { "0" } else { "1" };
        match setting.as_str() {
            "telemetry" => {
                let _ = std::process::Command::new("cmd").args(&["/C", "reg add HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\DataCollection /v AllowTelemetry /t REG_DWORD /d", val, "/f"]).output();
            },
            "cortana" => {
                let _ = std::process::Command::new("cmd").args(&["/C", "reg add HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\Windows Search /v AllowCortana /t REG_DWORD /d", val, "/f"]).output();
            },
            "activity_history" => {
                let _ = std::process::Command::new("cmd").args(&["/C", "reg add HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\System /v EnableActivityFeed /t REG_DWORD /d", val, "/f"]).output();
                let _ = std::process::Command::new("cmd").args(&["/C", "reg add HKLM\\SOFTWARE\\Policies\\Microsoft\\Windows\\System /v PublishUserActivities /t REG_DWORD /d", val, "/f"]).output();
            },
            "ad_id" => {
                let _ = std::process::Command::new("cmd").args(&["/C", "reg add HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\AdvertisingInfo /v Enabled /t REG_DWORD /d", val, "/f"]).output();
            },
            _ => return Err("Unknown setting".into()),
        }
        Ok("Success".into())
    }).await
}

// ═══════════════════════════════════════════════════════════════════════════════
// App Entry
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Original
            cmd_get_system_info,
            cmd_get_processes,
            cmd_get_catalog,
            cmd_optimize,
            // Monitoring
            cmd_get_live_metrics,
            cmd_get_health_score,
            cmd_get_hardware_info,
            // Startup
            cmd_list_startup,
            cmd_toggle_startup,
            // Scanner / Cleanup
            cmd_scan_large_files,
            cmd_detect_browsers,
            cmd_clean_browser,
            cmd_get_privacy_items,
            cmd_clean_privacy,
            cmd_list_drivers,
            cmd_clean_windows_update,
            cmd_kill_process,
            cmd_get_process_suggestions,
            cmd_optimize_processes,
            // Network
            cmd_get_network_overview,
            cmd_ping_test,
            // Debloater
            cmd_list_appx,
            cmd_remove_appx,
            cmd_remove_all_bloatware,
            // Benchmark
            cmd_run_benchmark,
            // Disk Health
            cmd_get_disk_health,
            // Duplicates
            cmd_scan_duplicates,
            cmd_delete_duplicate,
            // Services
            cmd_list_services,
            cmd_start_service,
            cmd_stop_service,
            cmd_set_service_startup,
            // Registry
            cmd_scan_registry,
            cmd_fix_registry_issue,
            // Battery
            cmd_get_battery_health,
            // Driver Management
            cmd_scan_driver_updates,
            cmd_open_device_manager,
            cmd_open_windows_update,
            // Disk Cleanup
            cmd_scan_junk,
            cmd_clean_junk_category,
            cmd_scan_app_caches,
            cmd_clean_app_cache,
            cmd_scan_stale_files,
            cmd_list_installed_programs,
            cmd_uninstall_program,
            cmd_list_restore_points,
            cmd_delete_restore_point,
            cmd_shred_file,
            cmd_wipe_free_space,
            cmd_get_ai_suggestions,
            cmd_get_folder_sizes,
            cmd_deep_clean,
            // New Features
            cmd_list_scheduled_tasks,
            cmd_toggle_scheduled_task,
            cmd_enable_game_booster,
            cmd_restore_normal_mode,
            cmd_toggle_telemetry,
            // File delete
            cmd_delete_file,
            cmd_reveal_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
