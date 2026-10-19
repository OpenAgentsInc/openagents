//! The user's own pause choices: `openagents pylon settings
//! [--pause-on-battery on|off] [--pause-when-hot on|off]`.
//!
//! Both are off by default (owner, 2026-10-19: no rule is imposed on
//! laptops). When the user turns one on, the pylon drains while it holds:
//! its beacon says `draining`, admitted jobs finish, and new ones are
//! refused, through the same path the owner's own work uses
//! ([`crate::lease::Paused`]).
//!
//! On battery: `pmset -g batt` on macOS, `/sys/class/power_supply` on
//! Linux. Hot: macOS thermal pressure above nominal
//! (`com.apple.system.thermalpressurelevel`, with `pmset -g therm` as a
//! fallback); an NVIDIA GPU within 5 °C of its slowdown temperature
//! (`nvidia-smi -q -d TEMPERATURE`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The user's pause choices, stored as `settings.json` in the pylon home.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Pause while this computer runs on battery.
    pub pause_on_battery: bool,
    /// Pause while this computer (or its NVIDIA GPU) is hot.
    pub pause_when_hot: bool,
}

impl Settings {
    /// The file name in the pylon home.
    pub const FILE: &'static str = "settings.json";

    /// Where the settings live under `home`.
    #[must_use]
    pub fn path(home: &Path) -> PathBuf {
        home.join(Self::FILE)
    }

    /// The settings in `home`; the defaults (both off) when there is no
    /// file.
    ///
    /// # Errors
    ///
    /// When the file exists but cannot be read or parsed.
    pub fn load(home: &Path) -> Result<Self, String> {
        let path = Self::path(home);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| format!("{} is not valid settings: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        }
    }

    /// Write the settings to `home` atomically. Refuses a symlink in the
    /// file's place.
    ///
    /// # Errors
    ///
    /// When the file cannot be written.
    pub fn save(&self, home: &Path) -> Result<(), String> {
        std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
        let path = Self::path(home);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(format!(
                "{} is a symlink; not writing through it",
                path.display()
            ));
        }
        let tmp = home.join(format!(".{}.{}.tmp", Self::FILE, std::process::id()));
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, format!("{text}\n")).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            e.to_string()
        })
    }
}

/// Why a pylon paused for the user's own settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Pause {
    OnBattery,
    Hot,
}

impl Pause {
    /// Plain words for the user.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::OnBattery => "paused: on battery (your setting)",
            Self::Hot => "paused: this computer is hot (your setting)",
        }
    }
}

/// What the machine's power and heat look like now. `None` means it could
/// not be read, which never pauses a pylon.
pub trait Sensors: Send + Sync {
    /// Whether this computer runs on battery.
    fn on_battery(&self) -> Option<bool>;
    /// Whether this computer or its GPU is hot.
    fn hot(&self) -> Option<bool>;
}

/// Whether `settings` pause the pylon given what `sensors` read. A sensor
/// is only read when its setting is on.
#[must_use]
pub fn pause(settings: Settings, sensors: &dyn Sensors) -> Option<Pause> {
    if settings.pause_on_battery && sensors.on_battery() == Some(true) {
        return Some(Pause::OnBattery);
    }
    if settings.pause_when_hot && sensors.hot() == Some(true) {
        return Some(Pause::Hot);
    }
    None
}

/// This computer's sensors.
pub struct System;

fn out(cmd: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(cmd).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

impl Sensors for System {
    fn on_battery(&self) -> Option<bool> {
        if cfg!(target_os = "macos") {
            out("pmset", &["-g", "batt"]).and_then(|t| parse_pmset_batt(&t))
        } else {
            linux_on_battery(Path::new("/sys/class/power_supply"))
        }
    }

    fn hot(&self) -> Option<bool> {
        let gpu = out("nvidia-smi", &["-q", "-d", "TEMPERATURE"])
            .and_then(|t| parse_nvidia_temperature(&t));
        let mac = if cfg!(target_os = "macos") {
            out(
                "notifyutil",
                &["-g", "com.apple.system.thermalpressurelevel"],
            )
            .and_then(|t| parse_thermal_pressure(&t))
            .or_else(|| out("pmset", &["-g", "therm"]).and_then(|t| parse_pmset_therm(&t)))
        } else {
            None
        };
        match (gpu, mac) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (None, None) => None,
            _ => Some(false),
        }
    }
}

/// `pmset -g batt`: whether it draws from the battery.
#[must_use]
pub fn parse_pmset_batt(text: &str) -> Option<bool> {
    let line = text.lines().find(|l| l.contains("drawing from"))?;
    if line.contains("'Battery Power'") {
        Some(true)
    } else if line.contains("'AC Power'") || line.contains("'UPS Power'") {
        Some(false)
    } else {
        None
    }
}

/// `notifyutil -g com.apple.system.thermalpressurelevel`: above nominal
/// (0) is hot. Levels: 0 nominal, 1 moderate, 2 heavy, 3 trapping,
/// 4 sleeping.
#[must_use]
pub fn parse_thermal_pressure(text: &str) -> Option<bool> {
    let level: u32 = text.split_whitespace().last()?.parse().ok()?;
    Some(level > 0)
}

/// `pmset -g therm`: hot when the CPU speed is limited below 100 % or a
/// thermal warning level above zero is recorded.
#[must_use]
pub fn parse_pmset_therm(text: &str) -> Option<bool> {
    let mut read = false;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let Ok(value) = value.trim().parse::<u32>() else {
            continue;
        };
        if key.contains("cpu_speed_limit") {
            read = true;
            if value < 100 {
                return Some(true);
            }
        } else if key.contains("thermal") && key.contains("level") {
            read = true;
            if value > 0 {
                return Some(true);
            }
        }
    }
    if read || text.contains("No thermal warning level") {
        Some(false)
    } else {
        None
    }
}

fn celsius(value: &str) -> Option<f64> {
    value.trim().strip_suffix('C')?.trim().parse().ok()
}

/// `nvidia-smi -q -d TEMPERATURE`: any GPU within 5 °C of its slowdown
/// temperature. Reads absolute temperatures (`GPU Current Temp`, `GPU
/// Slowdown Temp`) or, on drivers that report margins instead, `GPU
/// T.Limit Temp` (degrees below the limit) against `GPU Slowdown T.Limit
/// Temp`.
#[must_use]
pub fn parse_nvidia_temperature(text: &str) -> Option<bool> {
    // One block per GPU, each starting at a `GPU 0000…` line.
    let mut gpus: Vec<[Option<f64>; 4]> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // `GPU 00000000:01:00.0` opens a block; fields read `Key : value`.
        let Some((key, value)) = trimmed.split_once(" : ") else {
            if trimmed.starts_with("GPU ") {
                gpus.push([None; 4]);
            }
            continue;
        };
        let slot = match key.trim() {
            "GPU Current Temp" => 0,
            "GPU Slowdown Temp" => 1,
            "GPU T.Limit Temp" => 2,
            "GPU Slowdown T.Limit Temp" => 3,
            _ => continue,
        };
        if gpus.is_empty() {
            gpus.push([None; 4]);
        }
        if let Some(last) = gpus.last_mut() {
            last[slot] = celsius(value);
        }
    }
    let mut read = false;
    for [current, slowdown, margin, slowdown_margin] in gpus {
        let hot = match (current, slowdown, margin, slowdown_margin) {
            (Some(now), Some(limit), _, _) => Some(now > limit - 5.0),
            (_, _, Some(margin), Some(at)) => Some(margin - at < 5.0),
            _ => None,
        };
        match hot {
            Some(true) => return Some(true),
            Some(false) => read = true,
            None => {}
        }
    }
    read.then_some(false)
}

/// Linux: on battery when a battery is present and no mains or USB supply
/// is online (`/sys/class/power_supply/*/{type,online,status}`). A
/// desktop with no battery is never on battery.
#[must_use]
pub fn linux_on_battery(root: &Path) -> Option<bool> {
    let entries = std::fs::read_dir(root).ok()?;
    let read = |dir: &Path, name: &str| {
        std::fs::read_to_string(dir.join(name))
            .ok()
            .map(|s| s.trim().to_string())
    };
    let mut battery = false;
    let mut discharging = false;
    let mut mains_seen = false;
    let mut mains_online = false;
    for entry in entries.flatten() {
        let dir = entry.path();
        match read(&dir, "type").as_deref() {
            Some("Battery") => {
                battery = true;
                discharging |= read(&dir, "status").as_deref() == Some("Discharging");
            }
            Some("Mains" | "USB" | "USB_C" | "USB_PD") => {
                if let Some(online) = read(&dir, "online") {
                    mains_seen = true;
                    mains_online |= online == "1";
                }
            }
            _ => {}
        }
    }
    if !battery {
        return Some(false);
    }
    if mains_seen {
        Some(!mains_online)
    } else {
        Some(discharging)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn settings_round_trip_and_default_off() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), Settings::default());
        assert!(!Settings::default().pause_on_battery && !Settings::default().pause_when_hot);
        let on = Settings {
            pause_on_battery: true,
            pause_when_hot: false,
        };
        on.save(dir.path()).unwrap();
        assert_eq!(Settings::load(dir.path()).unwrap(), on);
        // A partial file keeps the other default.
        std::fs::write(Settings::path(dir.path()), r#"{"pause_when_hot": true}"#).unwrap();
        assert_eq!(
            Settings::load(dir.path()).unwrap(),
            Settings {
                pause_on_battery: false,
                pause_when_hot: true
            }
        );
        std::fs::write(Settings::path(dir.path()), "not json").unwrap();
        assert!(Settings::load(dir.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn settings_refuse_to_write_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        std::os::unix::fs::symlink(&target, Settings::path(dir.path())).unwrap();
        assert!(Settings::default().save(dir.path()).is_err());
        assert!(!target.exists());
    }

    #[test]
    fn pmset_and_thermal_parsers_read_real_output() {
        assert_eq!(
            parse_pmset_batt(
                "Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t92%; charging; present: true\n"
            ),
            Some(false)
        );
        assert_eq!(
            parse_pmset_batt(
                "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t80%; discharging;\n"
            ),
            Some(true)
        );
        assert_eq!(parse_pmset_batt(""), None);
        assert_eq!(
            parse_thermal_pressure("com.apple.system.thermalpressurelevel 0\n"),
            Some(false)
        );
        assert_eq!(
            parse_thermal_pressure("com.apple.system.thermalpressurelevel 2\n"),
            Some(true)
        );
        assert_eq!(
            parse_pmset_therm(
                "Note: No thermal warning level has been recorded\nNote: No performance warning level has been recorded\n"
            ),
            Some(false)
        );
        assert_eq!(
            parse_pmset_therm(
                "CPU Power notify\n\tCPU_Scheduler_Limit \t= 100\n\tCPU_Available_CPUs \t= 8\n\tCPU_Speed_Limit \t= 70\n"
            ),
            Some(true)
        );
    }

    #[test]
    fn nvidia_heat_is_within_five_degrees_of_slowdown() {
        let absolute = |now: u32| {
            format!(
                "==============NVSMI LOG==============\nAttached GPUs : 1\nGPU 00000000:01:00.0\n    Temperature\n        GPU Current Temp            : {now} C\n        GPU Shutdown Temp           : 98 C\n        GPU Slowdown Temp           : 95 C\n"
            )
        };
        assert_eq!(parse_nvidia_temperature(&absolute(45)), Some(false));
        assert_eq!(parse_nvidia_temperature(&absolute(90)), Some(false));
        assert_eq!(parse_nvidia_temperature(&absolute(91)), Some(true));
        let margins = |left: i32| {
            format!(
                "GPU 00000000:01:00.0\n    Temperature\n        GPU Current Temp : 60 C\n        GPU T.Limit Temp : {left} C\n        GPU Slowdown T.Limit Temp : -2 C\n"
            )
        };
        assert_eq!(parse_nvidia_temperature(&margins(40)), Some(false));
        assert_eq!(parse_nvidia_temperature(&margins(2)), Some(true));
        // Two GPUs: the second one is hot.
        let two = format!("{}{}", absolute(40), absolute(93));
        assert_eq!(parse_nvidia_temperature(&two), Some(true));
        assert_eq!(parse_nvidia_temperature("No devices were found"), None);
    }

    #[test]
    fn linux_power_supplies() {
        let dir = tempfile::tempdir().unwrap();
        let supply = |name: &str, files: &[(&str, &str)]| {
            let d = dir.path().join(name);
            std::fs::create_dir_all(&d).unwrap();
            for (file, text) in files {
                std::fs::write(d.join(file), format!("{text}\n")).unwrap();
            }
        };
        // A desktop: no battery.
        supply("AC", &[("type", "Mains"), ("online", "1")]);
        assert_eq!(linux_on_battery(dir.path()), Some(false));
        supply("BAT0", &[("type", "Battery"), ("status", "Discharging")]);
        assert_eq!(linux_on_battery(dir.path()), Some(false));
        supply("AC", &[("type", "Mains"), ("online", "0")]);
        assert_eq!(linux_on_battery(dir.path()), Some(true));
        assert_eq!(linux_on_battery(&dir.path().join("missing")), None);
    }

    struct Fake(Option<bool>, Option<bool>);

    impl Sensors for Fake {
        fn on_battery(&self) -> Option<bool> {
            self.0
        }
        fn hot(&self) -> Option<bool> {
            self.1
        }
    }

    #[test]
    fn only_a_setting_the_user_turned_on_pauses() {
        let both = Fake(Some(true), Some(true));
        assert_eq!(pause(Settings::default(), &both), None);
        let battery = Settings {
            pause_on_battery: true,
            ..Settings::default()
        };
        assert_eq!(pause(battery, &both), Some(Pause::OnBattery));
        assert_eq!(pause(battery, &Fake(Some(false), Some(true))), None);
        // A sensor that can't be read never pauses.
        assert_eq!(pause(battery, &Fake(None, None)), None);
        let hot = Settings {
            pause_when_hot: true,
            ..Settings::default()
        };
        assert_eq!(pause(hot, &Fake(Some(true), Some(true))), Some(Pause::Hot));
    }
}
