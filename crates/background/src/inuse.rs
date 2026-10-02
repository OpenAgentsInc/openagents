//! In-use detection: advisory locks a build holds, and processes with a
//! working directory or an open file inside a folder.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// Which processes use which paths.
pub trait Processes: Send + Sync {
    /// Every path some process other than this one has as its working
    /// directory or holds open, read now.
    ///
    /// # Errors
    /// The process table cannot be read; callers then treat every candidate
    /// as in use.
    fn snapshot(&self) -> Result<Snapshot, String>;
}

/// The paths processes use at one moment.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    paths: BTreeSet<PathBuf>,
}

impl Snapshot {
    #[must_use]
    pub fn new(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }

    /// How many open paths lie inside `dir`.
    #[must_use]
    pub fn inside(&self, dir: &Path) -> usize {
        self.paths
            .range(dir.to_owned()..)
            .take_while(|path| path.starts_with(dir))
            .count()
    }
}

/// The real process table: `lsof` on macOS, `/proc` on Linux.
#[derive(Clone, Copy, Debug, Default)]
pub struct System;

impl Processes for System {
    fn snapshot(&self) -> Result<Snapshot, String> {
        if cfg!(target_os = "linux") {
            proc_snapshot()
        } else {
            lsof_snapshot()
        }
    }
}

fn lsof_snapshot() -> Result<Snapshot, String> {
    let output = std::process::Command::new("lsof")
        .args(["-n", "-P", "-w", "-F", "pn"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|error| format!("lsof: {error}"))?;
    // lsof exits 1 when some process could not be read; the rest is
    // still a full listing of this user's processes.
    if output.stdout.is_empty() {
        return Err("lsof listed nothing".into());
    }
    let own = std::process::id();
    let mut pid = 0u32;
    let mut paths = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(rest) = line.strip_prefix('p') {
            pid = rest.parse().unwrap_or(0);
        } else if let Some(name) = line.strip_prefix('n')
            && pid != own
            && name.starts_with('/')
        {
            // lsof appends notes such as " (deleted)"; keep the path.
            let name = name.split(" (").next().unwrap_or(name);
            paths.push(PathBuf::from(name));
        }
    }
    Ok(Snapshot::new(paths))
}

fn proc_snapshot() -> Result<Snapshot, String> {
    let own = std::process::id().to_string();
    let mut paths = Vec::new();
    let entries = std::fs::read_dir("/proc").map_err(|error| format!("/proc: {error}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.bytes().all(|byte| byte.is_ascii_digit()) || name == own {
            continue;
        }
        let dir = entry.path();
        if let Ok(target) = std::fs::read_link(dir.join("cwd")) {
            paths.push(target);
        }
        if let Ok(fds) = std::fs::read_dir(dir.join("fd")) {
            for fd in fds.flatten() {
                if let Ok(target) = std::fs::read_link(fd.path())
                    && target.is_absolute()
                {
                    paths.push(target);
                }
            }
        }
    }
    Ok(Snapshot::new(paths))
}

/// The advisory locks that mark a Cargo target directory busy: a slot's
/// `<dir>.lock` and each profile's `.cargo-lock`.
#[must_use]
pub fn locks_of(dir: &Path) -> Vec<PathBuf> {
    let mut slot = dir.as_os_str().to_owned();
    slot.push(".lock");
    vec![
        PathBuf::from(slot),
        dir.join("debug/.cargo-lock"),
        dir.join("release/.cargo-lock"),
    ]
}

/// Locks taken for the length of a deletion, so a build cannot start
/// mid-delete. Dropping releases them.
#[derive(Debug, Default)]
pub struct Held {
    files: Vec<File>,
}

impl Held {
    /// Take every existing lock in `paths`. A missing lock file is free.
    ///
    /// # Errors
    /// The lock that is held, or could not be opened safely.
    pub fn take(paths: &[PathBuf]) -> Result<Self, String> {
        let mut held = Self::default();
        for path in paths {
            match std::fs::symlink_metadata(path) {
                Err(_) => continue,
                Ok(meta) if !meta.is_file() => {
                    return Err(format!("{} is not a lock file", path.display()));
                }
                Ok(_) => {}
            }
            let mut options = OpenOptions::new();
            options.read(true).write(true);
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let file = options
                .open(path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            match file.try_lock() {
                Ok(()) => held.files.push(file),
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(format!("{} is held", lock_name(path)));
                }
                Err(std::fs::TryLockError::Error(error)) => {
                    return Err(format!("{}: {error}", path.display()));
                }
            }
        }
        Ok(held)
    }
}

fn lock_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_lock_is_reported_and_a_free_one_is_taken() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("t");
        std::fs::create_dir_all(target.join("debug")).unwrap();
        let cargo = target.join("debug/.cargo-lock");
        std::fs::write(&cargo, "").unwrap();
        let held = Held::take(&locks_of(&target)).unwrap();
        drop(held);
        let build = File::options().write(true).open(&cargo).unwrap();
        build.lock().unwrap();
        assert!(Held::take(&locks_of(&target)).unwrap_err().contains("held"));
    }

    #[test]
    fn a_child_with_an_open_file_shows_in_the_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path().canonicalize().unwrap();
        let file = dir.join("open");
        std::fs::write(&file, "x").unwrap();
        let mut child = std::process::Command::new("tail")
            .arg("-f")
            .arg(&file)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut seen = 0;
        for _ in 0..50 {
            seen = System.snapshot().unwrap().inside(&dir);
            if seen > 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(seen > 0);
    }
}
