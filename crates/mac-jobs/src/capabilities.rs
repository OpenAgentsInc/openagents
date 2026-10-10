//! What a Mac can do, as it reports it: versions, signing identities by
//! name, simulators, and whether an App Store Connect key is present.
//! Never a key, a certificate, or a password.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::spec::{Recipe, Spec};

/// The most simulators reported.
const MAX_SIMULATORS: usize = 24;
/// The most signing identities reported.
const MAX_IDENTITIES: usize = 16;

/// One simulator on the Mac.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Simulator {
    pub name: String,
    /// The runtime as people read it ("iOS 26.5").
    pub runtime: String,
    pub udid: String,
    pub booted: bool,
}

/// What the Mac reports it can do.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Capabilities {
    /// "26.4 (25E246)".
    pub macos: Option<String>,
    /// "26.6 (17F113)".
    pub xcode: Option<String>,
    /// Code-signing identities by name only, such as
    /// "Apple Distribution: OpenAgents, Inc. (TEAMID)".
    pub signing_identities: Vec<String>,
    pub simulators: Vec<Simulator>,
    /// An App Store Connect API key is on the Mac. The key never leaves it.
    pub asc_key: bool,
    /// Free space for jobs, in GB.
    pub free_disk_gb: Option<u64>,
    /// The recipes this Mac runs.
    pub recipes: Vec<Recipe>,
    /// It is running a job now.
    pub busy: bool,
}

impl Capabilities {
    /// Whether this Mac can run `spec`; the error says what it lacks.
    ///
    /// # Errors
    /// The Mac doesn't offer the recipe, or lacks Xcode, a simulator, a
    /// distribution identity, or an App Store Connect key.
    pub fn supports(&self, spec: &Spec) -> Result<(), String> {
        if !self.recipes.contains(&spec.recipe) {
            return Err(format!("This Mac doesn't run {}.", spec.recipe.name()));
        }
        let needs_xcode = spec.recipe != Recipe::DesktopCapture;
        if needs_xcode && self.xcode.is_none() {
            return Err("This Mac has no Xcode.".into());
        }
        match spec.recipe {
            Recipe::IosReleaseGate if self.simulators.is_empty() => {
                Err("This Mac has no iOS simulator.".into())
            }
            Recipe::IosTestflight if !self.asc_key => {
                Err("This Mac has no App Store Connect key.".into())
            }
            Recipe::IosTestflight
                if !self
                    .signing_identities
                    .iter()
                    .any(|name| name.starts_with("Apple Distribution")) =>
            {
                Err("This Mac has no Apple Distribution signing identity.".into())
            }
            _ => Ok(()),
        }
    }

    /// The capabilities with every list bounded and every text one short
    /// line: what a website keeps of a report.
    #[must_use]
    pub fn bounded(mut self) -> Self {
        let short = |text: &str| -> String {
            text.chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect::<String>()
                .trim()
                .to_owned()
        };
        self.macos = self.macos.as_deref().map(short).filter(|t| !t.is_empty());
        self.xcode = self.xcode.as_deref().map(short).filter(|t| !t.is_empty());
        self.signing_identities.truncate(MAX_IDENTITIES);
        for name in &mut self.signing_identities {
            *name = short(name);
        }
        self.signing_identities.retain(|name| !name.is_empty());
        self.simulators.truncate(MAX_SIMULATORS);
        for simulator in &mut self.simulators {
            simulator.name = short(&simulator.name);
            simulator.runtime = short(&simulator.runtime);
            simulator.udid = short(&simulator.udid);
        }
        self.recipes.dedup();
        self
    }
}

/// `sw_vers` output as "26.4 (25E246)".
#[must_use]
pub fn parse_sw_vers(text: &str) -> Option<String> {
    let field = |name: &str| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == name).then(|| value.trim().to_owned())
        })
    };
    let version = field("ProductVersion")?;
    Some(match field("BuildVersion") {
        Some(build) if !build.is_empty() => format!("{version} ({build})"),
        _ => version,
    })
}

/// `xcodebuild -version` output as "26.6 (17F113)".
#[must_use]
pub fn parse_xcode_version(text: &str) -> Option<String> {
    let mut lines = text.lines();
    let version = lines
        .next()?
        .trim()
        .strip_prefix("Xcode ")?
        .trim()
        .to_owned();
    if version.is_empty() {
        return None;
    }
    let build = lines.find_map(|line| line.trim().strip_prefix("Build version ").map(str::trim));
    Some(match build {
        Some(build) if !build.is_empty() => format!("{version} ({build})"),
        _ => version,
    })
}

/// `security find-identity -v -p codesigning` output: each identity's
/// quoted name, once each, without its hash.
#[must_use]
pub fn parse_identities(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for line in text.lines() {
        let Some(start) = line.find('"') else {
            continue;
        };
        let Some(end) = line.rfind('"').filter(|end| *end > start) else {
            continue;
        };
        let name = line[start + 1..end].trim().to_owned();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// "com.apple.CoreSimulator.SimRuntime.iOS-26-5" as "iOS 26.5".
fn runtime_label(identifier: &str) -> String {
    let tail = identifier.rsplit('.').next().unwrap_or(identifier);
    match tail.split_once('-') {
        Some((platform, version)) => format!("{platform} {}", version.replace('-', ".")),
        None => tail.to_owned(),
    }
}

/// `xcrun simctl list devices available -j` output: the iOS simulators,
/// booted ones first.
#[must_use]
pub fn parse_simctl(json: &str) -> Vec<Simulator> {
    let Ok(value) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let Some(devices) = value["devices"].as_object() else {
        return found;
    };
    for (runtime, list) in devices {
        if !runtime.contains(".iOS-") {
            continue;
        }
        for device in list.as_array().into_iter().flatten() {
            if device["isAvailable"] == Value::Bool(false) {
                continue;
            }
            let (Some(name), Some(udid)) = (device["name"].as_str(), device["udid"].as_str())
            else {
                continue;
            };
            found.push(Simulator {
                name: name.to_owned(),
                runtime: runtime_label(runtime),
                udid: udid.to_owned(),
                booted: device["state"] == "Booted",
            });
        }
    }
    found.sort_by(|a, b| b.booted.cmp(&a.booted).then_with(|| a.name.cmp(&b.name)));
    found
}

/// `df -Pk PATH` output: the available space, in whole GB.
#[must_use]
pub fn parse_df_available_gb(text: &str) -> Option<u64> {
    let line = text.lines().nth(1)?;
    let available_kb: u64 = line.split_whitespace().nth(3)?.parse().ok()?;
    Some(available_kb / (1024 * 1024))
}

/// Whether an App Store Connect key is on this Mac, found the way
/// `scripts/release/testflight.sh` finds it: `ASC_API_PRIVATE_KEY_PATH`,
/// or the env file `OPENAGENTS_ASC_ENV` names (default
/// `~/work/.secrets/appstoreconnect.env`) naming a key file that exists.
/// Only the answer leaves this function.
#[must_use]
pub fn asc_key_present(env: &dyn Fn(&str) -> Option<String>, home: Option<&Path>) -> bool {
    if let Some(path) = env("ASC_API_PRIVATE_KEY_PATH").filter(|p| !p.is_empty()) {
        return Path::new(&path).is_file();
    }
    let file = env("OPENAGENTS_ASC_ENV")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join("work/.secrets/appstoreconnect.env")));
    let Some(text) = file.and_then(|file| std::fs::read_to_string(file).ok()) else {
        return false;
    };
    text.lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("export ").unwrap_or(line.trim());
            line.strip_prefix("ASC_API_PRIVATE_KEY_PATH=")
        })
        .map(|value| value.trim().trim_matches(['"', '\'']).to_owned())
        .find(|value| !value.is_empty())
        .map(|value| {
            let expanded = match (value.strip_prefix("~/"), home) {
                (Some(rest), Some(home)) => home.join(rest),
                _ => match (value.strip_prefix("$HOME/"), home) {
                    (Some(rest), Some(home)) => home.join(rest),
                    _ => PathBuf::from(value),
                },
            };
            expanded.is_file()
        })
        .unwrap_or(false)
}

/// Run `program` with `args`, its standard output when it exits 0 within
/// `limit`.
fn output(program: &str, args: &[&str], limit: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let until = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let text = reader.join().ok()?;
                return status.success().then_some(text);
            }
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// Ask this Mac what it can do. `jobs_root` is where jobs run, for the
/// free space. Off macOS, only the desktop capture is offered.
#[must_use]
pub fn detect(jobs_root: &Path) -> Capabilities {
    let limit = Duration::from_secs(20);
    let macos = output("sw_vers", &[], limit)
        .as_deref()
        .and_then(parse_sw_vers);
    let xcode = output("xcodebuild", &["-version"], limit)
        .as_deref()
        .and_then(parse_xcode_version);
    let signing_identities = output(
        "security",
        &["find-identity", "-v", "-p", "codesigning"],
        limit,
    )
    .as_deref()
    .map(parse_identities)
    .unwrap_or_default();
    let simulators = if xcode.is_some() {
        output(
            "xcrun",
            &["simctl", "list", "devices", "available", "-j"],
            limit,
        )
        .as_deref()
        .map(parse_simctl)
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    let probe = if jobs_root.exists() {
        jobs_root.to_path_buf()
    } else {
        jobs_root
            .ancestors()
            .find(|dir| dir.exists())
            .map_or_else(|| PathBuf::from("/"), Path::to_path_buf)
    };
    let free_disk_gb = probe
        .to_str()
        .and_then(|probe| output("df", &["-Pk", probe], limit))
        .as_deref()
        .and_then(parse_df_available_gb);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let asc_key = asc_key_present(&|name| std::env::var(name).ok(), home.as_deref());
    let recipes = if macos.is_some() && xcode.is_some() {
        Recipe::ALL.to_vec()
    } else if macos.is_some() {
        vec![Recipe::DesktopCapture]
    } else {
        Vec::new()
    };
    Capabilities {
        macos,
        xcode,
        signing_identities,
        simulators,
        asc_key,
        free_disk_gb,
        recipes,
        busy: false,
    }
    .bounded()
}
