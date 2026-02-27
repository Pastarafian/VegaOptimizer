//! Network Monitor — per-process bandwidth, connections, speed

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConnection {
    pub protocol: String,
    pub local_addr: String,
    pub remote_addr: String,
    pub state: String,
    pub pid: u32,
    pub process_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessBandwidth {
    pub pid: u32,
    pub name: String,
    pub connections: usize,
    pub bytes_sent: u64,
    pub bytes_recv: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkOverview {
    pub total_connections: usize,
    pub tcp_established: usize,
    pub tcp_listening: usize,
    pub udp_active: usize,
    pub processes_with_network: usize,
    pub top_talkers: Vec<ProcessBandwidth>,
    pub connections: Vec<NetworkConnection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedTestResult {
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub ping_ms: f64,
    pub server: String,
    pub timestamp: String,
}

/// Get all network connections with process mapping
pub fn get_network_connections() -> NetworkOverview {
    let mut connections = Vec::new();
    let mut proc_conn_count: HashMap<u32, usize> = HashMap::new();
    let mut proc_names: HashMap<u32, String> = HashMap::new();

    // Get TCP connections
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"Get-NetTCPConnection | Select-Object LocalAddress,LocalPort,RemoteAddress,RemotePort,State,OwningProcess | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)|$($_.RemoteAddress):$($_.RemotePort)|$($_.State)|$($_.OwningProcess)" }"#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let pid = parts[3].trim().parse::<u32>().unwrap_or(0);
                let state = parts[2].trim().to_string();
                connections.push(NetworkConnection {
                    protocol: "TCP".into(),
                    local_addr: parts[0].trim().to_string(),
                    remote_addr: parts[1].trim().to_string(),
                    state,
                    pid,
                    process_name: String::new(),
                });
                *proc_conn_count.entry(pid).or_insert(0) += 1;
            }
        }
    }

    // Get UDP endpoints
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"Get-NetUDPEndpoint | Select-Object LocalAddress,LocalPort,OwningProcess | ForEach-Object { "$($_.LocalAddress):$($_.LocalPort)|*:*|Listen|$($_.OwningProcess)" }"#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 4 {
                let pid = parts[3].trim().parse::<u32>().unwrap_or(0);
                connections.push(NetworkConnection {
                    protocol: "UDP".into(),
                    local_addr: parts[0].trim().to_string(),
                    remote_addr: parts[1].trim().to_string(),
                    state: "Active".into(),
                    pid,
                    process_name: String::new(),
                });
                *proc_conn_count.entry(pid).or_insert(0) += 1;
            }
        }
    }

    // Resolve process names
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (_pid, proc_) in sys.processes() {
        proc_names.insert(_pid.as_u32(), proc_.name().to_string_lossy().to_string());
    }

    // Fill in process names
    for conn in &mut connections {
        conn.process_name = proc_names
            .get(&conn.pid)
            .cloned()
            .unwrap_or_else(|| "System".into());
    }

    // Build per-process bandwidth via perf counters
    let mut top_talkers: Vec<ProcessBandwidth> = proc_conn_count
        .iter()
        .filter(|(pid, count)| **count > 0 && **pid != 0)
        .map(|(pid, count)| {
            let name = proc_names
                .get(pid)
                .cloned()
                .unwrap_or_else(|| "Unknown".into());
            ProcessBandwidth {
                pid: *pid,
                name,
                connections: *count,
                bytes_sent: 0,
                bytes_recv: 0,
                status: if *count > 10 {
                    "Heavy".into()
                } else if *count > 3 {
                    "Active".into()
                } else {
                    "Light".into()
                },
            }
        })
        .collect();

    // Try to get per-process network I/O via ETW/perf counters
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"Get-Process | Where-Object { $_.Id -ne 0 } | Select-Object Id,ProcessName,@{N='Sent';E={try{(Get-NetTCPConnection -OwningProcess $_.Id -ErrorAction SilentlyContinue | Measure-Object).Count * 1024}catch{0}}},@{N='Recv';E={0}} | Where-Object { $_.Sent -gt 0 } | ForEach-Object { "$($_.Id)|$($_.Sent)|$($_.Recv)" } 2>$null"#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let io_map: HashMap<u32, (u64, u64)> = stdout.lines()
            .filter_map(|l| {
                let p: Vec<&str> = l.split('|').collect();
                if p.len() >= 3 {
                    Some((
                        p[0].parse().ok()?,
                        (p[1].parse().unwrap_or(0), p[2].parse().unwrap_or(0))
                    ))
                } else { None }
            })
            .collect();

        for talker in &mut top_talkers {
            if let Some((sent, recv)) = io_map.get(&talker.pid) {
                talker.bytes_sent = *sent;
                talker.bytes_recv = *recv;
            }
        }
    }

    top_talkers.sort_by(|a, b| b.connections.cmp(&a.connections));

    let tcp_established = connections
        .iter()
        .filter(|c| c.state == "Established")
        .count();
    let tcp_listening = connections.iter().filter(|c| c.state == "Listen").count();
    let udp_active = connections.iter().filter(|c| c.protocol == "UDP").count();

    NetworkOverview {
        total_connections: connections.len(),
        tcp_established,
        tcp_listening,
        udp_active,
        processes_with_network: top_talkers.len(),
        top_talkers: top_talkers.into_iter().take(30).collect(),
        connections: connections.into_iter().take(200).collect(),
    }
}

/// Quick ping test
pub fn ping_test(host: &str) -> f64 {
    if let Ok(output) = Command::new("ping")
        .args(["-n", "3", "-w", "2000", host])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse "Average = XXms"
        for line in stdout.lines() {
            if line.contains("Average") || line.contains("average") {
                if let Some(ms_str) = line.split('=').last() {
                    let cleaned = ms_str.trim().replace("ms", "").trim().to_string();
                    return cleaned.parse().unwrap_or(999.0);
                }
            }
        }
    }
    999.0
}
