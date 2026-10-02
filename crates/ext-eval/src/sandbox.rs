//! The throwaway directory one run lives in, and the boundary around it.
//!
//! Each run gets a new `<tmp>/oa-eval-XXXXXX`:
//!
//! ```text
//! home/            HOME: the child's whole personal space
//!   cwd/           the working directory: the case's fixtures, nothing else;
//!                  for a files test, a Git repository holding its template
//!                  and fixtures, committed once
//!   .openagents/   OPENAGENTS_HOME: fresh, empty stores; the agent's
//!                  question sets under questions/ in both arms, and the
//!                  subject's programs under programs/
//! out/             CODER_TRACE_DIR and the run's own records
//! tmp/             TMPDIR and TMP, mode 0700
//! guidance.md      the appended guidance, when the arm has any
//! ```
//!
//! The child runs inside `coder-boundary` with reads confined to the
//! system's program directories, this directory, and the agent binary, and
//! writes allowed only under `home/.openagents/`, `out/`, and `tmp/`, plus
//! `home/cwd/` when the operator granted `write`. Where the host has no
//! confinement backend, [`Sandbox::confine`] refuses with
//! `unconfined_host` and no child is ever spawned.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use coder_boundary::{Boundary, Error as BoundaryError};

use crate::case::{Case, Grant};

/// The prefix of every run directory.
pub const PREFIX: &str = "oa-eval-";

/// What stops a sandbox from existing.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    /// This host has no confinement backend: the run refuses.
    #[error("this host can't confine the run ({0}); nothing ran")]
    Unconfined(String),
    /// The directory could not be made.
    #[error("the run directory could not be prepared: {0}")]
    Io(String),
}

/// One run's directory.
#[derive(Debug)]
pub struct Sandbox {
    root: Option<tempfile::TempDir>,
    /// The canonical root, the path every other path hangs from.
    path: PathBuf,
    keep: bool,
    /// A files test: the workspace is a Git repository, and its `.git`
    /// is never one of the run's files.
    git: bool,
}

impl Sandbox {
    /// Makes a new run directory under `parent` and fills the workspace
    /// with the case's fixtures.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Io`] when a directory or a fixture can't be
    /// written.
    pub fn create(parent: &Path, case: &Case, keep: bool) -> Result<Self, SandboxError> {
        let io = |error: std::io::Error| SandboxError::Io(error.to_string());
        std::fs::create_dir_all(parent).map_err(io)?;
        let root = tempfile::Builder::new()
            .prefix(PREFIX)
            .tempdir_in(parent)
            .map_err(io)?;
        let path = root.path().canonicalize().map_err(io)?;
        let sandbox = Self {
            root: Some(root),
            path,
            keep,
            git: case.workspace.is_some(),
        };
        for dir in [
            sandbox.cwd(),
            sandbox.openagents(),
            sandbox.out(),
            sandbox.trace_dir(),
            sandbox.tmp(),
        ] {
            std::fs::create_dir_all(&dir).map_err(io)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(sandbox.tmp(), std::fs::Permissions::from_mode(0o700))
                .map_err(io)?;
        }
        let template = case
            .workspace
            .as_ref()
            .map(crate::workspace::Workspace::files)
            .unwrap_or_default();
        for (relative, bytes) in template.iter().chain(&case.files.fixtures) {
            let target = sandbox.cwd().join(relative);
            if !target.starts_with(sandbox.cwd()) {
                return Err(SandboxError::Io(format!(
                    "fixture {relative} lands outside the workspace"
                )));
            }
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir).map_err(io)?;
            }
            std::fs::write(&target, bytes).map_err(io)?;
        }
        if sandbox.git {
            sandbox.commit_fixture();
        }
        Ok(sandbox)
    }

    /// Makes a files test's workspace a Git repository with its starting
    /// files committed, so Coder can read history and `git diff` like in
    /// any checkout. Runs before the child starts, in an environment
    /// built from nothing; a host without Git leaves a plain folder.
    fn commit_fixture(&self) {
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(self.cwd())
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
                .env("HOME", self.tmp())
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        };
        let _ = git(&["init", "-q", "-b", "main"])
            && git(&["config", "user.name", "OpenAgents test"])
            && git(&["config", "user.email", "test@openagents.invalid"])
            && git(&["config", "commit.gpgsign", "false"])
            && git(&["add", "-A"])
            && git(&[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "The test's starting files",
            ]);
    }

    /// Whether the run is a files test.
    #[must_use]
    pub const fn files_test(&self) -> bool {
        self.git
    }

    /// The run directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.path
    }

    /// `home/`: the child's `HOME`.
    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.path.join("home")
    }

    /// `home/cwd/`: the workspace.
    #[must_use]
    pub fn cwd(&self) -> PathBuf {
        self.home().join("cwd")
    }

    /// `home/.openagents/`: the child's `OPENAGENTS_HOME`.
    #[must_use]
    pub fn openagents(&self) -> PathBuf {
        self.home().join(".openagents")
    }

    /// `home/.openagents/programs/`: where the subject's programs are
    /// admitted from, the directory Coder searches under its `HOME`.
    #[must_use]
    pub fn programs(&self) -> PathBuf {
        self.openagents().join("programs")
    }

    /// `home/.openagents/questions/`: the agent's question sets.
    #[must_use]
    pub fn questions(&self) -> PathBuf {
        self.openagents().join("questions")
    }

    /// `out/`: what the run leaves for the harness.
    #[must_use]
    pub fn out(&self) -> PathBuf {
        self.path.join("out")
    }

    /// `out/trace/`: the child's `CODER_TRACE_DIR`.
    #[must_use]
    pub fn trace_dir(&self) -> PathBuf {
        self.out().join("trace")
    }

    /// `out/trajectory.atif.jsonl`: the log the child is told to write.
    #[must_use]
    pub fn trajectory(&self) -> PathBuf {
        self.out().join("trajectory.atif.jsonl")
    }

    /// `tmp/`: the child's `TMPDIR`.
    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.path.join("tmp")
    }

    /// `guidance.md`: the text appended to the child's instructions.
    #[must_use]
    pub fn guidance(&self) -> PathBuf {
        self.path.join("guidance.md")
    }

    /// Whether the directory outlives the run.
    #[must_use]
    pub const fn kept(&self) -> bool {
        self.keep
    }

    /// Keeps the directory after the run and returns its path.
    #[must_use]
    pub fn keep(mut self) -> PathBuf {
        self.keep = true;
        self.path.clone()
    }

    /// The boundary the child runs inside: reads confined to the system,
    /// this directory, and `readable`; writes under `home/.openagents/`,
    /// `out/`, and `tmp/`, and `home/cwd/` with `write`; no network but
    /// loopback unless `network` is granted.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Unconfined`] where `coder-boundary` has no
    /// backend or its backend can't run, and [`SandboxError::Io`] for a
    /// path it can't resolve.
    pub fn confine(
        &self,
        grants: &BTreeSet<Grant>,
        readable: &[PathBuf],
    ) -> Result<Boundary, SandboxError> {
        confinement_available()?;
        let mut spec = Boundary::readonly()
            .readable(self.root())
            .writable(self.openagents())
            .writable(self.out())
            .writable(self.tmp());
        if grants.contains(&Grant::Write) {
            spec = spec.writable(self.cwd());
        }
        for path in readable {
            spec = spec.readable(path);
        }
        if !grants.contains(&Grant::Network) && offline_reaches_loopback() {
            spec = spec.offline();
        }
        spec.build().map_err(|error| match error {
            BoundaryError::Unsupported(_)
            | BoundaryError::Unavailable(_)
            | BoundaryError::Inoperable { .. } => SandboxError::Unconfined(error.to_string()),
            other => SandboxError::Io(other.to_string()),
        })
    }

    /// Every regular file under the workspace, by relative path, with its
    /// bytes' digest. Symlinks are listed by their target and never
    /// followed.
    #[must_use]
    pub fn workspace_files(&self) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        walk(
            &self.cwd(),
            &self.cwd(),
            self.git,
            &mut |relative, path, meta| {
                let value = if meta.file_type().is_symlink() {
                    let target = std::fs::read_link(path)
                        .map(|t| t.display().to_string())
                        .unwrap_or_default();
                    format!("symlink:{target}")
                } else {
                    std::fs::read(path)
                        .map(|bytes| nostr::contracts::digest_bytes(&bytes))
                        .unwrap_or_default()
                };
                files.insert(relative, value);
            },
        );
        files
    }

    /// Every regular file under the workspace with its bytes, for a files
    /// test's changes and diff. A symlink is its target's path, never
    /// followed; a file over [`MAX_SNAPSHOT_FILE`] is its digest.
    #[must_use]
    pub fn workspace_contents(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        walk(
            &self.cwd(),
            &self.cwd(),
            self.git,
            &mut |relative, path, meta| {
                let value = if meta.file_type().is_symlink() {
                    let target = std::fs::read_link(path)
                        .map(|t| t.display().to_string())
                        .unwrap_or_default();
                    format!("symlink -> {target}\n").into_bytes()
                } else if meta.len() > MAX_SNAPSHOT_FILE {
                    std::fs::read(path)
                        .map(|bytes| nostr::contracts::digest_bytes(&bytes))
                        .unwrap_or_default()
                        .into_bytes()
                        .into_iter()
                        .chain([0u8])
                        .collect()
                } else {
                    std::fs::read(path).unwrap_or_default()
                };
                files.insert(relative, value);
            },
        );
        files
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        if self.keep
            && let Some(root) = self.root.take()
        {
            // `TempDir` removes itself on drop; a kept run stays.
            let _ = root.keep();
        }
    }
}

/// Whether this host has a confinement backend that runs.
///
/// # Errors
///
/// Returns [`SandboxError::Unconfined`] naming the missing backend.
pub fn confinement_available() -> Result<(), SandboxError> {
    // Windows' boundary is an AppContainer, which reaches no loopback
    // address, so a child there could never reach the door proxy that is
    // its only way to the model: a run refuses rather than start blind.
    if cfg!(windows) {
        return Err(SandboxError::Unconfined(
            "the Windows boundary cannot reach the loopback door proxy".into(),
        ));
    }
    let Some(_) = coder_boundary::BACKEND else {
        return Err(SandboxError::Unconfined(
            "coder-boundary has no backend on this operating system".into(),
        ));
    };
    let backend = coder_boundary::backend_path();
    if !Path::new(backend).is_file() {
        return Err(SandboxError::Unconfined(format!(
            "{backend} is not installed"
        )));
    }
    Ok(())
}

/// Whether an offline boundary still reaches the loopback door proxy.
///
/// On macOS the profile allows `localhost`, so the child reaches the proxy
/// and nothing else. On Linux `bwrap --unshare-net` gives the child a
/// network namespace of its own, whose loopback is not the host's, so the
/// proxy would be unreachable: the Linux run keeps the network open and
/// relies on the proxy token for credential containment.
#[must_use]
pub const fn offline_reaches_loopback() -> bool {
    cfg!(target_os = "macos")
}

/// How the network is bounded for a run with `grants`, as the report
/// records it.
#[must_use]
pub fn network_policy(grants: &BTreeSet<Grant>) -> &'static str {
    if grants.contains(&Grant::Network) {
        "open"
    } else if offline_reaches_loopback() {
        "door-only"
    } else {
        "open-door-proxied"
    }
}

/// The largest file a files test's snapshot holds whole; a larger one is
/// compared by digest.
pub const MAX_SNAPSHOT_FILE: u64 = 8 << 20;

/// The operator's toolchain directories a files test reads: Rust's
/// `~/.cargo` and `~/.rustup` where they exist, so Coder and `command`
/// checks can build and test (read-only; Cargo's own state goes to the
/// run's `tmp/`).
///
/// On macOS it also holds the developer directory (`xcode-select -p`) and
/// its bundle: the run puts the real `git`, `cc`, and `ld` from it on
/// `PATH` ahead of `/usr/bin`'s shims, which need `xcrun` and its caches
/// outside the boundary.
#[must_use]
pub fn toolchains() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| {
            [".cargo", ".rustup"]
                .iter()
                .map(|name| home.join(name))
                .filter(|path| path.is_dir())
                .filter_map(|path| path.canonicalize().ok())
                .collect()
        })
        .unwrap_or_default();
    if let Some(developer) = developer_dir() {
        // Xcode's shims also load frameworks beside `Contents/Developer`:
        // the whole bundle is read.
        let bundle = developer
            .ancestors()
            .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
            .map(Path::to_path_buf);
        dirs.push(developer);
        dirs.extend(bundle);
    }
    dirs
}

/// The macOS SDK the compilers read, found outside the boundary (`xcrun
/// --show-sdk-path`), so a build inside it needs no `xcrun`.
#[must_use]
pub fn sdk_root() -> Option<PathBuf> {
    static FOUND: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            let developer = developer_dir()?;
            std::process::Command::new("/usr/bin/xcrun")
                .arg("--show-sdk-path")
                .env_clear()
                .env("DEVELOPER_DIR", &developer)
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success())
                .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
                .filter(|dir| dir.is_dir())
        })
        .clone()
}

/// The macOS developer directory the tool shims load, when there is one.
#[must_use]
pub fn developer_dir() -> Option<PathBuf> {
    static FOUND: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    FOUND
        .get_or_init(|| {
            if !cfg!(target_os = "macos") {
                return None;
            }
            let selected = std::process::Command::new("/usr/bin/xcode-select")
                .arg("-p")
                .env_clear()
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success())
                .map(|out| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()));
            selected
                .into_iter()
                .chain(std::iter::once(PathBuf::from(
                    "/Library/Developer/CommandLineTools",
                )))
                .find(|dir| dir.is_dir())
                .and_then(|dir| dir.canonicalize().ok())
        })
        .clone()
}

fn walk(
    root: &Path,
    dir: &Path,
    skip_git: bool,
    found: &mut dyn FnMut(String, &Path, &std::fs::Metadata),
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        if meta.file_type().is_symlink() || meta.is_file() {
            found(relative, &path, &meta);
        } else if meta.is_dir() {
            if skip_git && path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            walk(root, &path, skip_git, found);
        }
    }
}

/// The paths present after a run that were not there before, in order.
#[must_use]
pub fn created(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Vec<String> {
    after
        .keys()
        .filter(|path| !before.contains_key(*path))
        .cloned()
        .collect()
}

/// `bytes` with every occurrence of each secret replaced by `[redacted]`.
/// Empty secrets are skipped.
#[must_use]
pub fn scrub(bytes: &[u8], secrets: &[&str]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for secret in secrets {
        let needle = secret.as_bytes();
        if needle.is_empty() {
            continue;
        }
        let mut result = Vec::with_capacity(out.len());
        let mut index = 0;
        while index < out.len() {
            if out[index..].starts_with(needle) {
                result.extend_from_slice(b"[redacted]");
                index += needle.len();
            } else {
                result.push(out[index]);
                index += 1;
            }
        }
        out = result;
    }
    out
}

/// Whether any file under `dir` holds `secret`. Used as a last check that
/// the door key reached nothing a run writes.
#[must_use]
pub fn holds(dir: &Path, secret: &str) -> Option<PathBuf> {
    if secret.is_empty() {
        return None;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file()
                && let Ok(bytes) = std::fs::read(&path)
                && bytes
                    .windows(secret.len())
                    .any(|window| window == secret.as_bytes())
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_replaces_every_occurrence() {
        let out = scrub(b"a KEY b KEYKEY c", &["KEY", ""]);
        assert_eq!(out, b"a [redacted] b [redacted][redacted] c");
    }

    #[test]
    fn the_network_is_the_door_alone_where_loopback_survives_offline() {
        let read = BTreeSet::from([Grant::Read]);
        let network = BTreeSet::from([Grant::Read, Grant::Network]);
        assert_eq!(network_policy(&network), "open");
        if cfg!(target_os = "macos") {
            assert!(offline_reaches_loopback());
            assert_eq!(network_policy(&read), "door-only");
        } else {
            assert_eq!(network_policy(&read), "open-door-proxied");
        }
    }

    #[test]
    fn created_lists_only_new_paths() {
        let before = BTreeMap::from([("a".to_string(), "1".to_string())]);
        let after = BTreeMap::from([
            ("a".to_string(), "2".to_string()),
            ("b/c".to_string(), "3".to_string()),
        ]);
        assert_eq!(created(&before, &after), vec!["b/c".to_string()]);
    }
}
