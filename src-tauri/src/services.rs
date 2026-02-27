//! Windows Services Manager — list, control, and categorize services

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: String,     // Running, Stopped, Paused
    pub start_type: String, // Automatic, Manual, Disabled
    pub memory_mb: f64,
    pub pid: u32,
    pub description: String,
    pub category: String, // "essential", "optional", "telemetry", "gaming", "media", "unknown"
    pub safe_to_disable: bool,
    pub recommendation: String,
}

/// Known service classifications
const SERVICE_CLASSIFICATIONS: &[(&str, &str, &str, bool, &str)] = &[
    // (name_pattern, category, display, safe_to_disable, recommendation)
    // Telemetry
    (
        "DiagTrack",
        "telemetry",
        "Connected User Experiences and Telemetry",
        true,
        "Sends usage data to Microsoft",
    ),
    (
        "dmwappushservice",
        "telemetry",
        "WAP Push Message Routing",
        true,
        "Telemetry routing service",
    ),
    (
        "diagnosticshub",
        "telemetry",
        "Diagnostics Hub",
        true,
        "Data collection for diagnostics",
    ),
    // Gaming
    (
        "XblAuth",
        "gaming",
        "Xbox Live Auth Manager",
        true,
        "Only needed for Xbox Live features",
    ),
    (
        "XblGameSave",
        "gaming",
        "Xbox Live Game Save",
        true,
        "Cloud saves for Xbox games",
    ),
    (
        "XboxNetApiSvc",
        "gaming",
        "Xbox Live Networking",
        true,
        "Xbox online networking",
    ),
    (
        "XboxGipSvc",
        "gaming",
        "Xbox Accessory Management",
        true,
        "Xbox controller service",
    ),
    // Search & Indexing
    (
        "WSearch",
        "optional",
        "Windows Search",
        true,
        "Disk-intensive indexing service",
    ),
    (
        "SysMain",
        "optional",
        "SysMain (Superfetch)",
        true,
        "Memory prefetching — minimal benefit on SSDs",
    ),
    // Print
    (
        "Spooler",
        "optional",
        "Print Spooler",
        true,
        "Only needed if you use printers",
    ),
    // Remote
    (
        "RemoteRegistry",
        "optional",
        "Remote Registry",
        true,
        "Security risk if enabled",
    ),
    (
        "RemoteAccess",
        "optional",
        "Routing and Remote Access",
        true,
        "VPN/routing service",
    ),
    (
        "TermService",
        "optional",
        "Remote Desktop Services",
        true,
        "Only needed for remote desktop",
    ),
    // Media
    (
        "WMPNetworkSvc",
        "media",
        "Windows Media Player Network",
        true,
        "WMP streaming service",
    ),
    // Fax
    ("Fax", "optional", "Fax", true, "Legacy fax support"),
    // Essential — NEVER disable
    (
        "Winmgmt",
        "essential",
        "WMI",
        false,
        "Critical system service",
    ),
    (
        "RpcSs",
        "essential",
        "RPC",
        false,
        "Critical system service",
    ),
    (
        "DcomLaunch",
        "essential",
        "DCOM Launch",
        false,
        "Critical system service",
    ),
    (
        "LSM",
        "essential",
        "Local Session Manager",
        false,
        "Critical system service",
    ),
    (
        "EventLog",
        "essential",
        "Windows Event Log",
        false,
        "System logging",
    ),
    (
        "Dhcp",
        "essential",
        "DHCP Client",
        false,
        "Network configuration",
    ),
    (
        "Dnscache",
        "essential",
        "DNS Client",
        false,
        "DNS resolution",
    ),
    (
        "BFE",
        "essential",
        "Base Filtering Engine",
        false,
        "Firewall foundation",
    ),
    (
        "mpssvc",
        "essential",
        "Windows Defender Firewall",
        false,
        "System firewall",
    ),
    (
        "WinDefend",
        "essential",
        "Windows Defender",
        false,
        "Antivirus protection",
    ),
    (
        "Schedule",
        "essential",
        "Task Scheduler",
        false,
        "System task management",
    ),
    ("Themes", "essential", "Themes", false, "Desktop appearance"),
    (
        "AudioSrv",
        "essential",
        "Windows Audio",
        false,
        "Sound system",
    ),
    (
        "AudioEndpointBuilder",
        "essential",
        "Audio Endpoint Builder",
        false,
        "Audio device management",
    ),
    (
        "wuauserv",
        "essential",
        "Windows Update",
        false,
        "System updates",
    ),
    (
        "BITS",
        "essential",
        "Background Intelligent Transfer",
        false,
        "Download management",
    ),
    (
        "CryptSvc",
        "essential",
        "Cryptographic Services",
        false,
        "Security certificates",
    ),
];

/// List all Windows services with classifications
pub fn list_services() -> Vec<ServiceInfo> {
    let mut services = Vec::new();

    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"Get-Service | ForEach-Object { $s = $_; $wmi = try{Get-CimInstance Win32_Service -Filter "Name='$($s.Name)'" -ErrorAction SilentlyContinue}catch{$null}; $pid = if($wmi){$wmi.ProcessId}else{0}; $desc = if($wmi){$wmi.Description}else{''}; $start = if($wmi){$wmi.StartMode}else{$s.StartType}; "$($s.Name)|$($s.DisplayName)|$($s.Status)|$start|$pid|$desc" }"#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Get process memory map
        let mut sys = sysinfo::System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let mut pid_mem: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
        for (pid, proc_) in sys.processes() {
            pid_mem.insert(pid.as_u32(), proc_.memory() as f64 / 1_048_576.0);
        }

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 5 { continue; }

            let name = parts[0].trim().to_string();
            let display = parts[1].trim().to_string();
            let status = parts[2].trim().to_string();
            let start_type = parts[3].trim().to_string();
            let pid: u32 = parts[4].trim().parse().unwrap_or(0);
            let desc = parts.get(5).unwrap_or(&"").trim().to_string();

            let mem = pid_mem.get(&pid).copied().unwrap_or(0.0);

            // Classify
            let mut category = "unknown".to_string();
            let mut safe = false;
            let mut rec = String::new();

            for (pattern, cat, _disp, can_disable, recommendation) in SERVICE_CLASSIFICATIONS {
                if name.to_lowercase().contains(&pattern.to_lowercase()) {
                    category = cat.to_string();
                    safe = *can_disable;
                    rec = recommendation.to_string();
                    break;
                }
            }

            services.push(ServiceInfo {
                name,
                display_name: display,
                status,
                start_type,
                memory_mb: mem,
                pid,
                description: desc,
                category,
                safe_to_disable: safe,
                recommendation: rec,
            });
        }
    }

    // Sort: running first, then by memory
    services.sort_by(|a, b| {
        let a_running = if a.status == "Running" { 0 } else { 1 };
        let b_running = if b.status == "Running" { 0 } else { 1 };
        a_running.cmp(&b_running).then(
            b.memory_mb
                .partial_cmp(&a.memory_mb)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    services
}

/// Start a service
pub fn start_service(name: &str) -> Result<String, String> {
    match Command::new("sc").args(["start", name]).output() {
        Ok(o) if o.status.success() => Ok(format!("Started {}", name)),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Stop a service
pub fn stop_service(name: &str) -> Result<String, String> {
    // Safety check
    for (pattern, cat, _, safe, _) in SERVICE_CLASSIFICATIONS {
        if name.to_lowercase().contains(&pattern.to_lowercase()) && !safe {
            return Err(format!("{} is an essential system service", name));
        }
    }
    match Command::new("sc").args(["stop", name]).output() {
        Ok(o) if o.status.success() => Ok(format!("Stopped {}", name)),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Set service startup type
pub fn set_service_startup(name: &str, startup: &str) -> Result<String, String> {
    let sc_type = match startup {
        "Automatic" | "Auto" => "auto",
        "Manual" => "demand",
        "Disabled" => "disabled",
        _ => return Err("Invalid startup type".into()),
    };
    match Command::new("sc")
        .args(["config", name, "start=", sc_type])
        .output()
    {
        Ok(o) if o.status.success() => Ok(format!("Set {} to {}", name, startup)),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    }
}
