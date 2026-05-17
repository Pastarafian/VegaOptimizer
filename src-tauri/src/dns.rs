//! DNS Quick-Switch — one-click DNS provider switching
//! Uses `netsh` for maximum compatibility across Windows 10/11.

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsProvider {
    pub id: String,
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub description: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsStatus {
    pub adapter_name: String,
    pub current_primary: String,
    pub current_secondary: String,
    pub is_dhcp: bool,
    pub active_provider: String, // matched provider ID or "custom"
}

/// Curated list of privacy-respecting, high-performance DNS providers
pub fn get_dns_providers() -> Vec<DnsProvider> {
    vec![
        DnsProvider {
            id: "cloudflare".into(),
            name: "Cloudflare".into(),
            primary: "1.1.1.1".into(),
            secondary: "1.0.0.1".into(),
            description: "Fastest public DNS — privacy-first, no logging".into(),
            icon: "CF".into(),
        },
        DnsProvider {
            id: "cloudflare_malware".into(),
            name: "Cloudflare (Malware Block)".into(),
            primary: "1.1.1.2".into(),
            secondary: "1.0.0.2".into(),
            description: "Cloudflare with built-in malware blocking".into(),
            icon: "CF+".into(),
        },
        DnsProvider {
            id: "google".into(),
            name: "Google".into(),
            primary: "8.8.8.8".into(),
            secondary: "8.8.4.4".into(),
            description: "Google Public DNS — reliable worldwide coverage".into(),
            icon: "G".into(),
        },
        DnsProvider {
            id: "quad9".into(),
            name: "Quad9".into(),
            primary: "9.9.9.9".into(),
            secondary: "149.112.112.112".into(),
            description: "Threat-blocking DNS — blocks known malicious domains".into(),
            icon: "Q9".into(),
        },
        DnsProvider {
            id: "adguard".into(),
            name: "AdGuard".into(),
            primary: "94.140.14.14".into(),
            secondary: "94.140.15.15".into(),
            description: "Ad-blocking DNS — blocks ads and trackers at DNS level".into(),
            icon: "AG".into(),
        },
        DnsProvider {
            id: "opendns".into(),
            name: "OpenDNS".into(),
            primary: "208.67.222.222".into(),
            secondary: "208.67.220.220".into(),
            description: "Cisco's public DNS with phishing protection".into(),
            icon: "OD".into(),
        },
        DnsProvider {
            id: "auto".into(),
            name: "Automatic (DHCP)".into(),
            primary: String::new(),
            secondary: String::new(),
            description: "Use your ISP's default DNS servers".into(),
            icon: "AUTO".into(),
        },
    ]
}

/// Get the primary active network adapter name for DNS operations
fn get_active_adapter() -> Result<String, String> {
    let output = Command::new("powershell")
        .args([
            "-Command",
            r#"(Get-NetAdapter | Where-Object { $_.Status -eq 'Up' -and $_.InterfaceDescription -notmatch 'Virtual|Loopback|Hyper-V' } | Select-Object -First 1).Name"#,
        ])
        .output()
        .map_err(|e| format!("Failed to detect network adapter: {}", e))?;

    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        Err("No active network adapter found".into())
    } else {
        Ok(name)
    }
}

/// Get the current DNS configuration
pub fn get_dns_status() -> Result<DnsStatus, String> {
    let adapter = get_active_adapter()?;

    let output = Command::new("powershell")
        .args([
            "-Command",
            &format!(
                r#"$dns = Get-DnsClientServerAddress -InterfaceAlias '{}' -AddressFamily IPv4 -ErrorAction SilentlyContinue; $dhcp = (Get-NetIPInterface -InterfaceAlias '{}' -AddressFamily IPv4 -ErrorAction SilentlyContinue).Dhcp; "$($dns.ServerAddresses -join ',')|$dhcp""#,
                adapter, adapter
            ),
        ])
        .output()
        .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split('|').collect();

    let servers: Vec<&str> = parts
        .first()
        .unwrap_or(&"")
        .split(',')
        .filter(|s| !s.is_empty())
        .collect();
    let is_dhcp = parts.get(1).map(|s| s.trim()) == Some("Enabled");

    let primary = servers.first().unwrap_or(&"").to_string();
    let secondary = servers.get(1).unwrap_or(&"").to_string();

    // Match against known providers
    let providers = get_dns_providers();
    let active_provider = providers
        .iter()
        .find(|p| p.primary == primary && (p.secondary == secondary || p.secondary.is_empty()))
        .map(|p| p.id.clone())
        .unwrap_or_else(|| {
            if is_dhcp || primary.is_empty() {
                "auto".to_string()
            } else {
                "custom".to_string()
            }
        });

    Ok(DnsStatus {
        adapter_name: adapter,
        current_primary: primary,
        current_secondary: secondary,
        is_dhcp,
        active_provider,
    })
}

/// Set DNS to a specific provider by ID
pub fn set_dns_provider(provider_id: &str) -> Result<String, String> {
    let providers = get_dns_providers();
    let provider = providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| format!("Unknown DNS provider: {}", provider_id))?;

    let adapter = get_active_adapter()?;

    if provider_id == "auto" {
        // Reset to DHCP
        let result = Command::new("netsh")
            .args([
                "interface", "ip", "set", "dns",
                &format!("name={}", adapter),
                "source=dhcp",
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if result.status.success() {
            // Flush DNS cache after change
            let _ = Command::new("ipconfig").args(["/flushdns"]).output();
            Ok("DNS reset to automatic (DHCP)".into())
        } else {
            Err(format!(
                "Failed to reset DNS: {}",
                String::from_utf8_lossy(&result.stderr)
            ))
        }
    } else {
        // Set primary DNS
        let result = Command::new("netsh")
            .args([
                "interface", "ip", "set", "dns",
                &format!("name={}", adapter),
                "static",
                &provider.primary,
            ])
            .output()
            .map_err(|e| e.to_string())?;

        if !result.status.success() {
            return Err(format!(
                "Failed to set primary DNS: {}",
                String::from_utf8_lossy(&result.stderr)
            ));
        }

        // Set secondary DNS
        if !provider.secondary.is_empty() {
            let _ = Command::new("netsh")
                .args([
                    "interface", "ip", "add", "dns",
                    &format!("name={}", adapter),
                    &provider.secondary,
                    "index=2",
                ])
                .output();
        }

        // Flush DNS cache after change
        let _ = Command::new("ipconfig").args(["/flushdns"]).output();

        Ok(format!(
            "DNS switched to {} ({} / {})",
            provider.name, provider.primary, provider.secondary
        ))
    }
}
