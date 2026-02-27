//! Windows Debloater — list, analyze, and remove preinstalled UWP apps

use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppxPackage {
    pub name: String,
    pub display_name: String,
    pub publisher: String,
    pub version: String,
    pub size_mb: f64,
    pub install_location: String,
    pub is_framework: bool,
    pub is_system: bool,
    pub category: String, // "bloatware", "utility", "system", "game", "media"
    pub safe_to_remove: bool,
    pub description: String,
}

/// Known bloatware / safe-to-remove apps
const BLOATWARE_PATTERNS: &[(&str, &str, &str)] = &[
    ("Microsoft.BingWeather", "Weather", "bloatware"),
    ("Microsoft.BingNews", "News", "bloatware"),
    ("Microsoft.BingFinance", "Finance", "bloatware"),
    ("Microsoft.BingSports", "Sports", "bloatware"),
    ("Microsoft.GetHelp", "Get Help", "bloatware"),
    ("Microsoft.Getstarted", "Tips", "bloatware"),
    ("Microsoft.MicrosoftOfficeHub", "Office Hub", "bloatware"),
    (
        "Microsoft.MicrosoftSolitaireCollection",
        "Solitaire",
        "game",
    ),
    (
        "Microsoft.MixedReality.Portal",
        "Mixed Reality",
        "bloatware",
    ),
    ("Microsoft.People", "People", "bloatware"),
    ("Microsoft.SkypeApp", "Skype", "bloatware"),
    ("Microsoft.Todos", "To Do", "utility"),
    ("Microsoft.WindowsAlarms", "Alarms", "utility"),
    ("Microsoft.WindowsFeedbackHub", "Feedback Hub", "bloatware"),
    ("Microsoft.WindowsMaps", "Maps", "bloatware"),
    ("Microsoft.YourPhone", "Your Phone", "bloatware"),
    ("Microsoft.ZuneMusic", "Groove Music", "media"),
    ("Microsoft.ZuneVideo", "Movies & TV", "media"),
    ("Microsoft.Xbox", "Xbox", "game"),
    ("Microsoft.XboxApp", "Xbox App", "game"),
    ("Microsoft.XboxGameOverlay", "Xbox Game Overlay", "game"),
    ("Microsoft.XboxGamingOverlay", "Xbox Game Bar", "game"),
    ("Microsoft.XboxIdentityProvider", "Xbox Identity", "game"),
    ("Microsoft.XboxSpeechToTextOverlay", "Xbox Speech", "game"),
    ("king.com", "Candy Crush", "game"),
    ("SpotifyAB", "Spotify (Preinstalled)", "media"),
    ("Disney", "Disney+", "media"),
    ("Facebook", "Facebook", "bloatware"),
    ("Instagram", "Instagram", "bloatware"),
    ("Twitter", "Twitter", "bloatware"),
    ("TikTok", "TikTok", "bloatware"),
    ("Clipchamp", "Clipchamp", "media"),
    ("Microsoft.549981C3F5F10", "Cortana", "bloatware"),
    (
        "Microsoft.WindowsCommunicationsApps",
        "Mail & Calendar",
        "utility",
    ),
    (
        "Microsoft.PowerAutomateDesktop",
        "Power Automate",
        "bloatware",
    ),
    ("MicrosoftTeams", "Teams (Consumer)", "bloatware"),
];

/// System-critical packages that should NEVER be removed
const PROTECTED_PACKAGES: &[&str] = &[
    "Microsoft.WindowsStore",
    "Microsoft.WindowsTerminal",
    "Microsoft.WindowsCalculator",
    "Microsoft.WindowsNotepad",
    "Microsoft.Paint",
    "Microsoft.ScreenSketch",
    "Microsoft.Windows.Photos",
    "Microsoft.DesktopAppInstaller",
    "Microsoft.UI",
    "Microsoft.NET",
    "Microsoft.VCLibs",
    "Microsoft.DirectX",
    "Microsoft.StorePurchaseApp",
    "Microsoft.WindowsAppRuntime",
    "Microsoft.WindowsAppSDK",
];

/// List all installed UWP packages with bloatware classification
pub fn list_appx_packages() -> Vec<AppxPackage> {
    let mut packages = Vec::new();

    if let Ok(output) = Command::new("powershell")
        .args(["-Command", r#"Get-AppxPackage | Select-Object Name,PackageFullName,Publisher,Version,InstallLocation,IsFramework,SignatureKind | ForEach-Object { "$($_.Name)|$($_.Publisher)|$($_.Version)|$($_.InstallLocation)|$($_.IsFramework)|$($_.SignatureKind)" }"#])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() < 6 { continue; }

            let name = parts[0].trim().to_string();
            let publisher = parts[1].trim().to_string();
            let version = parts[2].trim().to_string();
            let install_loc = parts[3].trim().to_string();
            let is_framework = parts[4].trim() == "True";
            let is_system = parts[5].trim() == "System";

            // Skip frameworks and empty
            if is_framework || name.is_empty() { continue; }

            // Classify
            let mut display_name = name.split('.').last().unwrap_or(&name).to_string();
            let mut category = if is_system { "system".to_string() } else { "utility".to_string() };
            let mut safe = false;
            let mut description = String::new();

            for (pattern, disp, cat) in BLOATWARE_PATTERNS {
                if name.to_lowercase().contains(&pattern.to_lowercase()) {
                    display_name = disp.to_string();
                    category = cat.to_string();
                    safe = true;
                    description = format!("Preinstalled {} — safe to remove if unused", cat);
                    break;
                }
            }

            // Check protected list
            for prot in PROTECTED_PACKAGES {
                if name.contains(prot) {
                    safe = false;
                    category = "system".to_string();
                    description = "System component — do not remove".to_string();
                    break;
                }
            }

            // Estimate size from install location
            let size = if !install_loc.is_empty() {
                estimate_dir_size(&install_loc) as f64 / 1_048_576.0
            } else {
                0.0
            };

            packages.push(AppxPackage {
                name: name.clone(),
                display_name,
                publisher: publisher.split(',').next().unwrap_or("Unknown").replace("CN=", "").to_string(),
                version,
                size_mb: size,
                install_location: install_loc,
                is_framework,
                is_system,
                category,
                safe_to_remove: safe,
                description,
            });
        }
    }

    // Sort: bloatware first, then by size
    packages.sort_by(|a, b| {
        let a_score = if a.safe_to_remove { 0 } else { 1 };
        let b_score = if b.safe_to_remove { 0 } else { 1 };
        a_score.cmp(&b_score).then(
            b.size_mb
                .partial_cmp(&a.size_mb)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    packages
}

fn estimate_dir_size(path: &str) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(m) = entry.metadata() {
                if m.is_file() {
                    total += m.len();
                }
            }
        }
    }
    total
}

/// Remove an AppX package
pub fn remove_appx_package(name: &str) -> Result<String, String> {
    // Safety check
    for prot in PROTECTED_PACKAGES {
        if name.contains(prot) {
            return Err(format!("{} is a protected system component", name));
        }
    }

    match Command::new("powershell")
        .args([
            "-Command",
            &format!(
                "Get-AppxPackage '{}' | Remove-AppxPackage -ErrorAction Stop",
                name
            ),
        ])
        .output()
    {
        Ok(o) if o.status.success() => Ok(format!("Removed {}", name)),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Bulk remove multiple packages
pub fn remove_all_bloatware() -> Vec<(String, bool, String)> {
    let packages = list_appx_packages();
    let mut results = Vec::new();

    for pkg in packages.iter().filter(|p| p.safe_to_remove) {
        match remove_appx_package(&pkg.name) {
            Ok(msg) => results.push((pkg.display_name.clone(), true, msg)),
            Err(msg) => results.push((pkg.display_name.clone(), false, msg)),
        }
    }

    results
}
