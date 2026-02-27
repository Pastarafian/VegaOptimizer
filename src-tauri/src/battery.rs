//! Battery Health — charge cycles, wear level, capacity

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryHealth {
    pub present: bool,
    pub status: String, // "Charging", "Discharging", "Full", "Not Present"
    pub charge_percent: u32,
    pub design_capacity_mwh: u64,
    pub full_charge_capacity_mwh: u64,
    pub current_capacity_mwh: u64,
    pub health_pct: u32, // full_charge / design * 100
    pub wear_pct: f64,
    pub voltage_mv: u32,
    pub charge_rate_mw: i32, // positive = charging, negative = discharging
    pub estimated_runtime_min: Option<u32>,
    pub cycle_count: Option<u32>,
    pub chemistry: String, // "LiIon", "LiPo", "NiMH"
    pub manufacturer: String,
    pub serial: String,
}

/// Get battery health information
pub fn get_battery_health() -> BatteryHealth {
    // Try WMI battery info first
    let mut battery = BatteryHealth {
        present: false,
        status: "Not Present".into(),
        charge_percent: 0,
        design_capacity_mwh: 0,
        full_charge_capacity_mwh: 0,
        current_capacity_mwh: 0,
        health_pct: 0,
        wear_pct: 0.0,
        voltage_mv: 0,
        charge_rate_mw: 0,
        estimated_runtime_min: None,
        cycle_count: None,
        chemistry: "Unknown".into(),
        manufacturer: "Unknown".into(),
        serial: String::new(),
    };

    // Get battery static info
    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"
            $b = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
            $bs = Get-CimInstance BatteryStaticData -Namespace root\WMI -ErrorAction SilentlyContinue
            $bf = Get-CimInstance BatteryFullChargedCapacity -Namespace root\WMI -ErrorAction SilentlyContinue
            $bc = Get-CimInstance BatteryCycleCount -Namespace root\WMI -ErrorAction SilentlyContinue
            $bstat = Get-CimInstance BatteryStatus -Namespace root\WMI -ErrorAction SilentlyContinue

            if($b) {
                $design = if($bs) { $bs.DesignedCapacity } else { 0 }
                $full = if($bf) { $bf.FullChargedCapacity } else { 0 }
                $cycles = if($bc) { $bc.CycleCount } else { 0 }
                $voltage = if($bstat) { $bstat.Voltage } else { 0 }
                $rate = if($bstat) { $bstat.ChargeRate } else { 0 }
                $charging = if($bstat) { $bstat.Charging } else { $false }
                $runtime = $b.EstimatedRunTime
                $chem = if($bs) { $bs.Chemistry } else { 0 }
                $mfr = if($bs) { [System.Text.Encoding]::Unicode.GetString($bs.ManufacturerName).Trim([char]0) } else { '' }
                $serial = if($bs) { [System.Text.Encoding]::Unicode.GetString($bs.SerialNumber).Trim([char]0) } else { '' }
                $pct = $b.EstimatedChargeRemaining
                $status = $b.BatteryStatus

                "FOUND|$pct|$design|$full|$voltage|$rate|$runtime|$cycles|$chem|$mfr|$serial|$charging|$status"
            } else {
                "NONE"
            }
        "#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.trim();

        if line.starts_with("FOUND|") {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 13 {
                battery.present = true;
                battery.charge_percent = parts[1].parse().unwrap_or(0);
                battery.design_capacity_mwh = parts[2].parse().unwrap_or(0);
                battery.full_charge_capacity_mwh = parts[3].parse().unwrap_or(0);
                battery.voltage_mv = parts[4].parse().unwrap_or(0);
                battery.charge_rate_mw = parts[5].parse().unwrap_or(0);
                battery.estimated_runtime_min = parts[6].parse().ok().filter(|&v: &u32| v < 71582);
                battery.cycle_count = parts[7].parse().ok().filter(|&v: &u32| v > 0 && v < 65535);
                battery.manufacturer = parts[9].trim().to_string();
                battery.serial = parts[10].trim().to_string();

                let is_charging = parts[11].trim() == "True";
                let status_code: u32 = parts[12].parse().unwrap_or(0);
                battery.status = match status_code {
                    1 => "Discharging".into(),
                    2 => if is_charging { "Charging".into() } else { "On AC".into() },
                    3 => "Full".into(),
                    4 => "Low".into(),
                    5 => "Critical".into(),
                    _ => if is_charging { "Charging".into() } else { "Unknown".into() },
                };

                // Chemistry mapping
                let chem_code: u32 = parts[8].parse().unwrap_or(0);
                battery.chemistry = match chem_code {
                    1 => "Other".into(),
                    2 => "Unknown".into(),
                    3 => "Lead Acid".into(),
                    4 => "NiCd".into(),
                    5 => "NiMH".into(),
                    6 => "Li-ion".into(),
                    7 => "Zinc Air".into(),
                    8 => "LiPo".into(),
                    _ => "Li-ion".into(),
                };

                // Calculate health
                if battery.design_capacity_mwh > 0 {
                    battery.health_pct = ((battery.full_charge_capacity_mwh as f64 / battery.design_capacity_mwh as f64) * 100.0).min(100.0) as u32;
                    battery.wear_pct = 100.0 - battery.health_pct as f64;
                }

                battery.current_capacity_mwh = (battery.full_charge_capacity_mwh as f64 * battery.charge_percent as f64 / 100.0) as u64;
            }
        }
    }

    battery
}
