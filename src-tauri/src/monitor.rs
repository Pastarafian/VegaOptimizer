//! Real-time monitoring, health score, and hardware info

use serde::{Deserialize, Serialize};
use sysinfo::{Components, Disks, Networks, ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveMetrics {
    pub cpu_usage: f32,
    pub cpu_per_core: Vec<f32>,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub memory_percent: f64,
    pub swap_used_mb: u64,
    pub swap_total_mb: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub process_count: usize,
    pub thread_count: usize,
    pub uptime_seconds: u64,
    pub temperatures: Vec<TempReading>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempReading {
    pub label: String,
    pub temp_c: f32,
    pub critical: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthScore {
    pub overall: u32,
    pub memory_score: u32,
    pub cpu_score: u32,
    pub disk_score: u32,
    pub startup_score: u32,
    pub uptime_score: u32,
    pub details: Vec<HealthDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDetail {
    pub category: String,
    pub score: u32,
    pub label: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu_name: String,
    pub cpu_arch: String,
    pub cpu_cores_physical: usize,
    pub cpu_cores_logical: usize,
    pub cpu_frequency_mhz: u64,
    pub ram_total_gb: f64,
    pub ram_type: String,
    pub os_name: String,
    pub os_version: String,
    pub os_build: String,
    pub hostname: String,
    pub disks: Vec<DiskInfo>,
    pub gpus: Vec<String>,
    pub network_adapters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_percent: f64,
    pub is_removable: bool,
}

pub fn get_live_metrics() -> LiveMetrics {
    let mut sys = System::new_all();
    sys.refresh_all();
    std::thread::sleep(std::time::Duration::from_millis(100));
    sys.refresh_cpu_all();

    let cpu_per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
    let cpu_usage = if cpu_per_core.is_empty() {
        0.0
    } else {
        cpu_per_core.iter().sum::<f32>() / cpu_per_core.len() as f32
    };

    let total_mem = sys.total_memory() / 1_048_576;
    let used_mem = sys.used_memory() / 1_048_576;

    // Disk I/O - aggregate across processes
    sys.refresh_processes(ProcessesToUpdate::All, true);
    let (mut total_read, mut total_write) = (0u64, 0u64);
    for (_pid, proc_) in sys.processes() {
        let dio = proc_.disk_usage();
        total_read += dio.read_bytes;
        total_write += dio.written_bytes;
    }

    // Network
    let networks = Networks::new_with_refreshed_list();
    let (mut rx, mut tx) = (0u64, 0u64);
    for (_name, net) in &networks {
        rx += net.received();
        tx += net.transmitted();
    }

    // Temperatures
    let components = Components::new_with_refreshed_list();
    let temperatures: Vec<TempReading> = components
        .iter()
        .map(|c| TempReading {
            label: c.label().to_string(),
            temp_c: c.temperature().unwrap_or(0.0),
            critical: c.critical(),
        })
        .collect();

    let process_count = sys.processes().len();

    LiveMetrics {
        cpu_usage,
        cpu_per_core,
        memory_used_mb: used_mem,
        memory_total_mb: total_mem,
        memory_percent: if total_mem > 0 {
            (used_mem as f64 / total_mem as f64) * 100.0
        } else {
            0.0
        },
        swap_used_mb: sys.used_swap() / 1_048_576,
        swap_total_mb: sys.total_swap() / 1_048_576,
        disk_read_bytes: total_read,
        disk_write_bytes: total_write,
        net_rx_bytes: rx,
        net_tx_bytes: tx,
        process_count,
        thread_count: 0,
        uptime_seconds: System::uptime(),
        temperatures,
    }
}

pub fn get_health_score() -> HealthScore {
    let mut sys = System::new_all();
    sys.refresh_all();

    let mut details: Vec<HealthDetail> = Vec::new();

    // Memory score (100 = low usage, 0 = full)
    let mem_pct = sys.used_memory() as f64 / sys.total_memory().max(1) as f64 * 100.0;
    let memory_score = if mem_pct < 50.0 {
        100
    } else if mem_pct < 70.0 {
        80
    } else if mem_pct < 85.0 {
        60
    } else if mem_pct < 95.0 {
        30
    } else {
        10
    };
    details.push(HealthDetail {
        category: "Memory".into(),
        score: memory_score,
        label: format!(
            "{:.0}% used ({} MB / {} MB)",
            mem_pct,
            sys.used_memory() / 1_048_576,
            sys.total_memory() / 1_048_576
        ),
        suggestion: if mem_pct > 80.0 {
            "Run memory optimization to free RAM".into()
        } else {
            "Memory usage is healthy".into()
        },
    });

    // CPU score
    std::thread::sleep(std::time::Duration::from_millis(200));
    sys.refresh_cpu_all();
    let cpu_avg: f32 =
        sys.cpus().iter().map(|c| c.cpu_usage()).sum::<f32>() / sys.cpus().len().max(1) as f32;
    let cpu_score = if cpu_avg < 30.0 {
        100
    } else if cpu_avg < 50.0 {
        80
    } else if cpu_avg < 70.0 {
        60
    } else if cpu_avg < 90.0 {
        30
    } else {
        10
    };
    details.push(HealthDetail {
        category: "CPU".into(),
        score: cpu_score,
        label: format!("{:.0}% average utilization", cpu_avg),
        suggestion: if cpu_avg > 70.0 {
            "Consider lowering idle process priorities".into()
        } else {
            "CPU load is normal".into()
        },
    });

    // Disk score
    let disks = Disks::new_with_refreshed_list();
    let mut worst_disk_pct = 0.0f64;
    for disk in disks.iter() {
        let total = disk.total_space() as f64;
        let avail = disk.available_space() as f64;
        if total > 0.0 {
            let used_pct = ((total - avail) / total) * 100.0;
            if used_pct > worst_disk_pct {
                worst_disk_pct = used_pct;
            }
        }
    }
    let disk_score = if worst_disk_pct < 60.0 {
        100
    } else if worst_disk_pct < 75.0 {
        80
    } else if worst_disk_pct < 85.0 {
        60
    } else if worst_disk_pct < 95.0 {
        30
    } else {
        10
    };
    details.push(HealthDetail {
        category: "Disk".into(),
        score: disk_score,
        label: format!("Most-used disk at {:.0}%", worst_disk_pct),
        suggestion: if worst_disk_pct > 85.0 {
            "Run disk cleanup to free space".into()
        } else {
            "Disk space is adequate".into()
        },
    });

    // Startup score - estimate based on process count
    let proc_count = sys.processes().len();
    let startup_score = if proc_count < 100 {
        100
    } else if proc_count < 200 {
        80
    } else if proc_count < 300 {
        60
    } else {
        40
    };
    details.push(HealthDetail {
        category: "Startup".into(),
        score: startup_score,
        label: format!("{} running processes", proc_count),
        suggestion: if proc_count > 200 {
            "Review startup programs to reduce bloat".into()
        } else {
            "Process count is normal".into()
        },
    });

    // Uptime score
    let uptime = System::uptime();
    let uptime_days = uptime / 86400;
    let uptime_score = if uptime_days < 3 {
        100
    } else if uptime_days < 7 {
        80
    } else if uptime_days < 14 {
        60
    } else {
        30
    };
    details.push(HealthDetail {
        category: "Uptime".into(),
        score: uptime_score,
        label: format!("{} days since last reboot", uptime_days),
        suggestion: if uptime_days > 7 {
            "Consider rebooting to clear stale state".into()
        } else {
            "System uptime is fine".into()
        },
    });

    let overall = (memory_score + cpu_score + disk_score + startup_score + uptime_score) / 5;

    HealthScore {
        overall,
        memory_score,
        cpu_score,
        disk_score,
        startup_score,
        uptime_score,
        details,
    }
}

pub fn get_hardware_info() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_name = sys
        .cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or("Unknown".into());
    let cpu_freq = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);

    let disks = Disks::new_with_refreshed_list();
    let disk_list: Vec<DiskInfo> = disks
        .iter()
        .map(|d| {
            let total = d.total_space() as f64 / 1_073_741_824.0;
            let avail = d.available_space() as f64 / 1_073_741_824.0;
            let used = total - avail;
            DiskInfo {
                name: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                fs_type: d.file_system().to_string_lossy().to_string(),
                total_gb: total,
                used_gb: used,
                free_gb: avail,
                usage_percent: if total > 0.0 {
                    (used / total) * 100.0
                } else {
                    0.0
                },
                is_removable: d.is_removable(),
            }
        })
        .collect();

    // GPU detection via WMI command
    let gpus = match std::process::Command::new("wmic")
        .args(["path", "win32_videocontroller", "get", "name"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout)
            .lines()
            .skip(1)
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => vec!["Unknown GPU".into()],
    };

    // Network adapters
    let nets = Networks::new_with_refreshed_list();
    let adapters: Vec<String> = nets.iter().map(|(name, _)| name.clone()).collect();

    HardwareInfo {
        cpu_name,
        cpu_arch: std::env::consts::ARCH.to_string(),
        cpu_cores_physical: System::physical_core_count().unwrap_or(0),
        cpu_cores_logical: sys.cpus().len(),
        cpu_frequency_mhz: cpu_freq,
        ram_total_gb: sys.total_memory() as f64 / 1_073_741_824.0,
        ram_type: "DDR4/DDR5".into(), // Can't detect via sysinfo
        os_name: System::name().unwrap_or("Windows".into()),
        os_version: System::os_version().unwrap_or("Unknown".into()),
        os_build: System::long_os_version().unwrap_or("Unknown".into()),
        hostname: System::host_name().unwrap_or("Unknown".into()),
        disks: disk_list,
        gpus,
        network_adapters: adapters,
    }
}
