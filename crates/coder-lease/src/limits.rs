//! The capacities counted leases share, and where each default comes from.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

/// Overrides the number of concurrent build leases.
pub const BUILD_LEASES_VAR: &str = "OPENAGENTS_BUILD_LEASES";
/// Overrides the memory budget, in GiB.
pub const MEMORY_GIB_VAR: &str = "OPENAGENTS_MEMORY_LEASE_GIB";
/// Overrides the free-space floor, in GB, that disk leases keep. It is the
/// same variable Coder's build slots read.
pub const SLOT_FREE_VAR: &str = "OPENAGENTS_SLOT_FREE_GB";
/// The free-space floor when nothing sets it, in GB.
pub const DEFAULT_FLOOR_GB: u64 = 10;
/// Overrides the disk budget, in GB, each `build` lease reserves above the
/// floor.
pub const BUILD_DISK_VAR: &str = "OPENAGENTS_BUILD_DISK_GB";
/// The disk budget of one `build` lease when nothing sets it, in GB.
pub const DEFAULT_BUILD_DISK_GB: u64 = 10;
/// Overrides the aging step, in minutes: a waiter grows one priority level
/// more urgent for each step it waits. `0` turns aging off.
pub const AGING_VAR: &str = "OPENAGENTS_LEASE_AGING_MINUTES";
/// The aging step when nothing sets it.
pub const DEFAULT_AGING: Duration = Duration::from_secs(20 * 60);
/// Names another settings file than `~/.openagents/settings.json`.
const SETTINGS_VAR: &str = "OPENAGENTS_SETTINGS";

/// The most pool jobs a pylon runs at once; its own slot count bounds it
/// further.
pub const PYLON_JOBS: u64 = 64;

/// The capacities counted leases share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Limits {
    /// Concurrent build slots.
    pub build: u64,
    /// The memory budget, in GiB.
    pub memory_gib: u64,
    /// The free space, in GB, that disk leases never go below.
    pub disk_floor_gb: u64,
    /// The disk budget, in GB, each `build` lease reserves above the floor.
    pub build_disk_gb: u64,
}

impl Limits {
    /// The limits this machine's environment and settings choose: a
    /// variable first, then `coder.build_leases` and `coder.slot_free_gb`
    /// in the settings file, then the defaults.
    ///
    /// # Errors
    /// A sentence naming the variable or setting whose value is invalid.
    pub fn from_env() -> Result<Limits, String> {
        let settings = std::env::var_os(SETTINGS_VAR)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".openagents/settings.json"))
            });
        Limits::read(
            &|name| std::env::var(name).ok(),
            settings.as_deref(),
            Machine::detect(),
        )
    }

    /// The limits `env`, the settings file, and the machine choose.
    ///
    /// # Errors
    /// A sentence naming the variable or setting whose value is invalid.
    pub fn read(
        env: &dyn Fn(&str) -> Option<String>,
        settings: Option<&Path>,
        machine: Machine,
    ) -> Result<Limits, String> {
        let coder = settings.and_then(read_coder_settings);
        let setting = |key: &str| coder.as_ref().and_then(|coder| coder.get(key).cloned());
        let number = |var: &str, key: Option<&str>| -> Result<Option<u64>, String> {
            if let Some(value) = env(var).filter(|value| !value.trim().is_empty()) {
                return value
                    .trim()
                    .parse::<u64>()
                    .map(Some)
                    .map_err(|_| format!("{var} is `{value}`, not a whole number"));
            }
            let Some(key) = key else { return Ok(None) };
            match setting(key) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(value) => value
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| format!("coder.{key} is {value}, not a whole number")),
            }
        };
        let build = match number(BUILD_LEASES_VAR, Some("build_leases"))? {
            Some(0) => return Err("the build lease count must be at least 1".to_owned()),
            Some(count) => count,
            None => (machine.cores / 4).clamp(1, 4),
        };
        let memory_gib = match number(MEMORY_GIB_VAR, None)? {
            Some(0) => return Err("the memory budget must be at least 1 GiB".to_owned()),
            Some(gib) => gib,
            None => (machine.memory_bytes / (1 << 30) * 3 / 4).max(1),
        };
        let disk_floor_gb =
            number(SLOT_FREE_VAR, Some("slot_free_gb"))?.unwrap_or(DEFAULT_FLOOR_GB);
        let build_disk_gb = number(BUILD_DISK_VAR, None)?.unwrap_or(DEFAULT_BUILD_DISK_GB);
        Ok(Limits {
            build,
            memory_gib,
            disk_floor_gb,
            build_disk_gb,
        })
    }

    /// The capacity of a counted resource, or `None` for an exclusive one
    /// and for disk, whose capacity is the free space.
    #[must_use]
    pub fn capacity(&self, resource: &crate::Resource) -> Option<u64> {
        match resource {
            crate::Resource::Build => Some(self.build),
            crate::Resource::Memory => Some(self.memory_gib),
            crate::Resource::Pylon => Some(PYLON_JOBS),
            _ => None,
        }
    }
}

/// The aging step [`AGING_VAR`] chooses in `env`: [`DEFAULT_AGING`] when
/// it's unset, and `None`, no aging, when it's `0`.
///
/// # Errors
/// A sentence when the variable isn't a whole number of minutes.
pub fn aging_from(env: &dyn Fn(&str) -> Option<String>) -> Result<Option<Duration>, String> {
    match env(AGING_VAR).filter(|value| !value.trim().is_empty()) {
        None => Ok(Some(DEFAULT_AGING)),
        Some(value) => match value.trim().parse::<u64>() {
            Ok(0) => Ok(None),
            Ok(minutes) => Ok(Some(Duration::from_secs(minutes.saturating_mul(60)))),
            Err(_) => Err(format!(
                "{AGING_VAR} is `{value}`, not a whole number of minutes"
            )),
        },
    }
}

/// The `coder` section of the settings file, when it can be read.
fn read_coder_settings(file: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let bytes = std::fs::read(file).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("coder")?.as_object().cloned()
}

/// What the machine has, which the defaults are fractions of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Machine {
    /// Logical cores.
    pub cores: u64,
    /// Physical memory, in bytes.
    pub memory_bytes: u64,
}

impl Machine {
    /// This machine.
    #[must_use]
    pub fn detect() -> Machine {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get() as u64);
        Machine {
            cores,
            memory_bytes: physical_memory(),
        }
    }
}

fn physical_memory() -> u64 {
    #[cfg(unix)]
    {
        // SAFETY: `sysconf` reads a system constant and has no
        // preconditions.
        let (pages, size) = unsafe {
            (
                libc::sysconf(libc::_SC_PHYS_PAGES),
                libc::sysconf(libc::_SC_PAGESIZE),
            )
        };
        if pages > 0 && size > 0 {
            return (pages as u64).saturating_mul(size as u64);
        }
    }
    // Unknown: a budget of 8 GiB after the 75 percent share.
    32 << 30
}

/// Free space, in bytes, on the volume that holds `path`.
///
/// # Errors
/// The error `statvfs` reports.
pub fn free_disk(path: &Path) -> std::io::Result<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let name = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        // SAFETY: `statvfs` writes into the zeroed structure and reads the
        // NUL-terminated path, which lives for the call.
        let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statvfs(name.as_ptr(), &mut stat) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        #[allow(clippy::unnecessary_cast)]
        Ok((stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: Machine = Machine {
        cores: 18,
        memory_bytes: 128 << 30,
    };

    #[test]
    fn defaults_are_fractions_of_the_machine() {
        let limits = Limits::read(&|_| None, None, MAC).unwrap();
        assert_eq!(limits.build, 4);
        assert_eq!(limits.memory_gib, 96);
        assert_eq!(limits.disk_floor_gb, 10);
        assert_eq!(limits.build_disk_gb, 10);
        let env = |name: &str| (name == BUILD_DISK_VAR).then(|| "40".to_owned());
        assert_eq!(Limits::read(&env, None, MAC).unwrap().build_disk_gb, 40);
        let small = Machine {
            cores: 4,
            memory_bytes: 8 << 30,
        };
        assert_eq!(Limits::read(&|_| None, None, small).unwrap().build, 1);
    }

    #[test]
    fn build_defaults_scale_to_four_slots() {
        for (cores, expected) in [(1, 1), (4, 1), (8, 2), (12, 3), (18, 4), (28, 4)] {
            let machine = Machine { cores, ..MAC };
            assert_eq!(
                Limits::read(&|_| None, None, machine).unwrap().build,
                expected
            );
        }
    }

    #[test]
    fn variables_beat_settings_which_beat_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(
            &file,
            r#"{"schema":"openagents.settings.v1","coder":{"build_leases":3,"slot_free_gb":20}}"#,
        )
        .unwrap();
        let limits = Limits::read(&|_| None, Some(&file), MAC).unwrap();
        assert_eq!((limits.build, limits.disk_floor_gb), (3, 20));
        let env = |name: &str| (name == BUILD_LEASES_VAR).then(|| "5".to_owned());
        assert_eq!(Limits::read(&env, Some(&file), MAC).unwrap().build, 5);
        let env = |name: &str| (name == BUILD_LEASES_VAR).then(|| "0".to_owned());
        assert!(Limits::read(&env, None, MAC).is_err());
        let env = |name: &str| (name == BUILD_LEASES_VAR).then(|| "two".to_owned());
        assert!(Limits::read(&env, None, MAC).is_err());
        assert_eq!(
            Limits::read(&|_| None, Some(&dir.path().join("absent")), MAC)
                .unwrap()
                .build,
            4
        );
    }

    #[test]
    fn aging_defaults_to_twenty_minutes_and_zero_turns_it_off() {
        assert_eq!(aging_from(&|_| None).unwrap(), Some(DEFAULT_AGING));
        let env =
            |value: &'static str| move |name: &str| (name == AGING_VAR).then(|| value.to_owned());
        assert_eq!(
            aging_from(&env("5")).unwrap(),
            Some(Duration::from_secs(300))
        );
        assert_eq!(aging_from(&env("0")).unwrap(), None);
        assert!(aging_from(&env("soon")).is_err());
    }

    #[test]
    fn free_disk_reads_the_volume() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_disk(dir.path()).unwrap() > 0);
    }
}
