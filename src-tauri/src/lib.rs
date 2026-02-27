mod battery;
mod benchmark;
mod debloater;
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
async fn cmd_toggle_startup(name: String, registry_path: String, enable: bool) -> Result<String, String> {
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
            Ok(_) => Ok(format!("Deleted {} ({:.1} MB)", path, size as f64 / 1_048_576.0)),
            Err(e) => Err(format!("Failed to delete: {}", e)),
        }
    }).await
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
    }).await
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
        let mut name_memory: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
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
                    reason: format!("{:.0} MB used with {:.1}% CPU — likely idle bloat", mem, cpu),
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
                    reason: format!("{:.0} MB used, completely idle — memory can be trimmed", mem),
                    severity: "medium".into(),
                    category: "idle_hog".into(),
                    safe_to_optimize: true,
                });
            }

            // Duplicate processes using >100 MB total
            let count = name_counts.get(&name_lower).copied().unwrap_or(0);
            let total_mem = name_memory.get(&name_lower).copied().unwrap_or(0.0);
            if count > 3 && total_mem > 100.0 && mem > 20.0 {
                let already = suggestions.iter().any(|s| s.name.to_lowercase() == name_lower && s.category == "duplicate");
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
                    reason: format!("Background process using {:.0} MB with no CPU activity", mem),
                    severity: "low".into(),
                    category: "background".into(),
                    safe_to_optimize: true,
                });
            }
        }

        // Sort: high severity first, then by memory
        suggestions.sort_by(|a, b| {
            let sev = |s: &str| match s { "high" => 0, "medium" => 1, _ => 2 };
            sev(&a.severity).cmp(&sev(&b.severity))
                .then(b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal))
        });

        suggestions.truncate(50);
        suggestions
    }).await
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
                use winapi::um::processthreadsapi::OpenProcess;
                use winapi::um::psapi::EmptyWorkingSet;
                use winapi::um::handleapi::CloseHandle;
                use winapi::um::errhandlingapi::GetLastError;
                use winapi::um::winnt::{PROCESS_SET_QUOTA, PROCESS_QUERY_INFORMATION};

                unsafe {
                    let handle = OpenProcess(
                        PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, 0, pid
                    );
                    if handle.is_null() {
                        let err = GetLastError();
                        success = false;
                        message = format!("Cannot open process (error {}{})", err,
                            if err == 5 { " — run as Administrator" } else { "" });
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
    }).await
}

/// Enable SeDebugPrivilege so we can call EmptyWorkingSet on any process
#[cfg(windows)]
fn enable_debug_privilege() {
    use winapi::um::processthreadsapi::{GetCurrentProcess, OpenProcessToken};
    use winapi::um::securitybaseapi::AdjustTokenPrivileges;
    use winapi::um::winbase::LookupPrivilegeValueA;
    use winapi::um::winnt::{
        TOKEN_ADJUST_PRIVILEGES, TOKEN_QUERY,
        SE_PRIVILEGE_ENABLED, TOKEN_PRIVILEGES, LUID,
    };
    use winapi::um::handleapi::CloseHandle;
    use std::ptr::null_mut;

    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) == 0 {
            return;
        }

        let mut luid = LUID { LowPart: 0, HighPart: 0 };
        let priv_name = b"SeDebugPrivilege\0";
        if LookupPrivilegeValueA(
            null_mut(),
            priv_name.as_ptr() as *const i8,
            &mut luid,
        ) == 0 {
            CloseHandle(token);
            return;
        }

        let mut tp: TOKEN_PRIVILEGES = std::mem::zeroed();
        tp.PrivilegeCount = 1;
        tp.Privileges[0].Luid = luid;
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;

        AdjustTokenPrivileges(
            token,
            0,
            &mut tp,
            0,
            null_mut(),
            null_mut(),
        );

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
// App Entry
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
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
            // File delete
            cmd_delete_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
