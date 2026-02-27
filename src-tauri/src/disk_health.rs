//! Disk Health — S.M.A.R.T. data, SSD/HDD health

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskHealthInfo {
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub interface_type: String, // SATA, NVMe, USB
    pub media_type: String,     // SSD, HDD, Removable
    pub size_gb: f64,
    pub health_status: String, // Healthy, Warning, Critical, Unknown
    pub health_pct: u32,       // 0-100
    pub temperature_c: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub total_reads_gb: Option<f64>,
    pub total_writes_gb: Option<f64>,
    pub smart_attributes: Vec<SmartAttribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartAttribute {
    pub id: u32,
    pub name: String,
    pub value: String,
    pub threshold: String,
    pub status: String, // "ok", "warning", "critical"
}

/// Get disk health info for all drives
pub fn get_disk_health() -> Vec<DiskHealthInfo> {
    let mut disks = Vec::new();

    // Get physical disk info via PowerShell
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            Get-PhysicalDisk | ForEach-Object {
                $d = $_
                $health = $d.HealthStatus
                $media = $d.MediaType
                $size = [math]::Round($d.Size / 1GB, 1)
                $model = $d.FriendlyName
                $serial = $d.SerialNumber
                $fw = $d.FirmwareVersion
                $bus = $d.BusType
                $wear = $d.Wear
                $temp = try{ (Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction SilentlyContinue).Temperature }catch{ $null }
                $hours = try{ (Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction SilentlyContinue).PowerOnHours }catch{ $null }
                $reads = try{ [math]::Round((Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction SilentlyContinue).ReadErrorsTotal / 1GB, 2) }catch{ $null }
                $writes = try{ [math]::Round((Get-StorageReliabilityCounter -PhysicalDisk $d -ErrorAction SilentlyContinue).WriteErrorsTotal / 1GB, 2) }catch{ $null }
                "$model|$serial|$fw|$bus|$media|$size|$health|$wear|$temp|$hours|$reads|$writes"
            }
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 8 { continue; }

            let model = parts[0].trim().to_string();
            if model.is_empty() { continue; }

            let health_status = parts[6].trim().to_string();
            let wear_str = parts.get(7).unwrap_or(&"").trim();
            let health_pct = if wear_str.is_empty() || wear_str == "" {
                match health_status.as_str() {
                    "Healthy" => 95,
                    "Warning" => 60,
                    "Degraded" => 40,
                    _ => 80,
                }
            } else {
                let wear_val: f64 = wear_str.parse().unwrap_or(0.0);
                (100.0 - wear_val * 100.0).max(0.0) as u32
            };

            let temp = parts.get(8).and_then(|s| s.trim().parse::<f64>().ok());
            let hours = parts.get(9).and_then(|s| s.trim().parse::<u64>().ok());

            disks.push(DiskHealthInfo {
                model,
                serial: parts[1].trim().to_string(),
                firmware: parts[2].trim().to_string(),
                interface_type: parts[3].trim().to_string(),
                media_type: if parts[4].trim().is_empty() { "Unknown".into() } else { parts[4].trim().to_string() },
                size_gb: parts[5].trim().parse().unwrap_or(0.0),
                health_status,
                health_pct,
                temperature_c: temp,
                power_on_hours: hours,
                total_reads_gb: parts.get(10).and_then(|s| s.trim().parse().ok()),
                total_writes_gb: parts.get(11).and_then(|s| s.trim().parse().ok()),
                smart_attributes: Vec::new(),
            });
        }
    }

    // If PowerShell method didn't work, try wmic
    if disks.is_empty() {
        if let Ok(output) = Command::new("wmic")
            .args([
                "diskdrive",
                "get",
                "Model,SerialNumber,FirmwareRevision,InterfaceType,MediaType,Size,Status",
                "/format:csv",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() < 8 {
                    continue;
                }
                let model = parts[3].trim();
                if model.is_empty() {
                    continue;
                }

                let size_bytes: f64 = parts[6].trim().parse().unwrap_or(0.0);
                let status = parts[7].trim();

                disks.push(DiskHealthInfo {
                    model: model.to_string(),
                    serial: parts[5].trim().to_string(),
                    firmware: parts[1].trim().to_string(),
                    interface_type: parts[2].trim().to_string(),
                    media_type: parts[4].trim().to_string(),
                    size_gb: size_bytes / 1_073_741_824.0,
                    health_status: if status == "OK" {
                        "Healthy".into()
                    } else {
                        status.to_string()
                    },
                    health_pct: if status == "OK" { 90 } else { 50 },
                    temperature_c: None,
                    power_on_hours: None,
                    total_reads_gb: None,
                    total_writes_gb: None,
                    smart_attributes: Vec::new(),
                });
            }
        }
    }

    disks
}
