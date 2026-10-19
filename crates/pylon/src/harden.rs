//! Hardening every pylon command shares: a scrubbed environment for child
//! processes, and secret files written `0600` by an atomic write that
//! refuses a symbolic-link target.
//!
//! A child (the Psionic server, `nvidia-smi`, `sysctl`) sees only `PATH`,
//! `HOME`, the user and temporary folder, the locale, proxy settings,
//! `OPENAGENTS_*` and `PSIONIC_*` settings, and the GPU selectors
//! (`CUDA_*`, `NVIDIA_*`, and on Linux `LD_LIBRARY_PATH`, where CUDA's
//! libraries often live). Names that read as credentials (`…_TOKEN`,
//! `…_KEY`, `…_SECRET`, `…_PASSWORD`, `…_MNEMONIC`) never pass, even under
//! a kept prefix.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Variables a child keeps by exact name.
pub const KEPT: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "TMP",
    "TEMP",
    "LANG",
    "LANGUAGE",
    "TZ",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    // Windows needs these to start any process.
    "SYSTEMROOT",
    "SystemRoot",
    "WINDIR",
    "USERPROFILE",
];

/// Variables a child keeps by prefix.
pub const KEPT_PREFIXES: &[&str] = &["LC_", "OPENAGENTS_", "PSIONIC_", "CUDA_", "NVIDIA_"];

/// Suffixes that mark a credential; never passed on.
const SECRET_SUFFIXES: &[&str] = &["_TOKEN", "_KEY", "_SECRET", "_PASSWORD", "_MNEMONIC"];

/// Whether a child process keeps the variable `name`.
#[must_use]
pub fn kept(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if SECRET_SUFFIXES.iter().any(|s| upper.ends_with(s)) {
        return false;
    }
    if cfg!(target_os = "linux") && name == "LD_LIBRARY_PATH" {
        return true;
    }
    KEPT.contains(&name) || KEPT_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// The environment a child process gets: this process's, scrubbed.
#[must_use]
pub fn child_env() -> Vec<(OsString, OsString)> {
    std::env::vars_os()
        .filter(|(name, _)| name.to_str().is_some_and(kept))
        .collect()
}

/// Give `command` the scrubbed environment.
pub fn scrub(command: &mut std::process::Command) -> &mut std::process::Command {
    command.env_clear().envs(child_env())
}

/// [`scrub`] for an async child.
pub fn scrub_async(command: &mut tokio::process::Command) -> &mut tokio::process::Command {
    command.env_clear().envs(child_env())
}

/// Write a secret to `path` with mode `0600`: into a fresh file beside it,
/// synced, then moved into place, so a reader never sees half a key.
/// Refuses when `path` is a symbolic link. With `replace` false it also
/// refuses (`AlreadyExists`) when `path` exists, without a window in which
/// two writers could both win.
///
/// # Errors
///
/// When `path` is a link, exists and `replace` is false, or cannot be
/// written.
pub fn write_secret(path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{} is a symbolic link; a secret is never written through one",
                    path.display()
                ),
            ));
        }
        if !replace {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists", path.display()),
            ));
        }
    }
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .map_or_else(|| "secret".into(), |n| n.to_string_lossy().into_owned());
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".{name}.{}.{nonce}.tmp", std::process::id()));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let written = options.open(&tmp).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    let placed = written.and_then(|()| {
        if replace {
            fs::rename(&tmp, path)
        } else {
            // A hard link fails when the name exists, unlike a rename.
            fs::hard_link(&tmp, path)
        }
    });
    let _ = fs::remove_file(&tmp);
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn children_keep_only_the_named_variables() {
        assert!(kept("PATH"));
        assert!(kept("HOME"));
        assert!(kept("LC_ALL"));
        assert!(kept("https_proxy"));
        assert!(kept("OPENAGENTS_PYLON_HOME"));
        assert!(kept("PSIONIC_LOG"));
        assert!(kept("CUDA_VISIBLE_DEVICES"));
        assert!(!kept("AWS_SECRET_ACCESS_KEY"));
        assert!(!kept("GITHUB_TOKEN"));
        assert!(!kept("OPENAGENTS_TOKEN"));
        assert!(!kept("OPENAGENTS_API_KEY"));
        assert!(!kept("PSIONIC_HF_TOKEN"));
        assert!(!kept("MDK_TREASURY_MNEMONIC"));
        assert!(!kept("DYLD_INSERT_LIBRARIES"));
        assert!(!kept("SSH_AUTH_SOCK"));
    }

    #[test]
    fn a_scrubbed_child_sees_no_secret() {
        let mut command = std::process::Command::new("env");
        scrub(&mut command).env("PYLON_TEST_SECRET", "x");
        let names: Vec<_> = command
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| kept(n) || n == "PYLON_TEST_SECRET"));
    }

    #[cfg(unix)]
    #[test]
    fn secrets_are_0600_atomic_and_never_through_a_link() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("provider.key");
        write_secret(&path, b"one\n", false).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(
            write_secret(&path, b"two\n", false).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\n");
        write_secret(&path, b"two\n", true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");

        let target = dir.path().join("elsewhere");
        fs::write(&target, "untouched").unwrap();
        let link = dir.path().join("linked.key");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        for replace in [false, true] {
            assert_eq!(
                write_secret(&link, b"secret", replace).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
        // No temporary file is left behind.
        let left: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(left.is_empty());
    }
}
