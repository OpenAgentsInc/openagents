//! Where the lease table lives.

use std::path::{Path, PathBuf};

/// Names another lease root than `~/.openagents/leases`.
pub const ROOT_VAR: &str = "OPENAGENTS_LEASE_ROOT";

/// The lease root: `$OPENAGENTS_LEASE_ROOT`, else `~/.openagents/leases`.
///
/// # Errors
/// A sentence when neither the variable nor `HOME` is set.
pub fn root_from_env() -> Result<PathBuf, String> {
    root_from(&|name| std::env::var_os(name).filter(|value| !value.is_empty()))
}

/// [`root_from_env`] over any environment.
///
/// # Errors
/// A sentence when neither the variable nor `HOME` is set.
pub fn root_from(env: &dyn Fn(&str) -> Option<std::ffi::OsString>) -> Result<PathBuf, String> {
    let root = if let Some(root) = env(ROOT_VAR) {
        PathBuf::from(root)
    } else if let Some(home) = env("HOME") {
        PathBuf::from(home).join(".openagents/leases")
    } else {
        return Err(format!("set HOME or {ROOT_VAR} to choose the lease root"));
    };
    refuse_real_home(&root);
    Ok(root)
}

/// Panics, in this crate's tests, when `root` lies in the real user's
/// home, the way `coder_service::adopt::Paths::under` refuses it. A test
/// gives the broker a temporary root. Does nothing outside tests.
pub fn refuse_real_home(root: &Path) {
    let _ = root;
    #[cfg(all(test, unix))]
    if let Some(real) = real_home() {
        let resolved = resolve(root);
        assert!(
            !resolved.starts_with(&real) && !root.starts_with(&real),
            "a test reached the real home {}; give the broker a temporary lease root",
            real.display()
        );
    }
}

/// The path with its longest existing prefix canonicalized.
#[cfg(all(test, unix))]
fn resolve(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut current = path.to_path_buf();
    loop {
        if let Ok(real) = current.canonicalize() {
            return rest.iter().rev().fold(real, |path, part| path.join(part));
        }
        match (current.file_name(), current.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_owned());
                current = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// The home directory the password database names for this user.
#[cfg(all(test, unix))]
fn real_home() -> Option<PathBuf> {
    // SAFETY: `getpwuid` returns a pointer into static storage or null;
    // the directory is copied out before any other call.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_dir.is_null() {
            return None;
        }
        let dir = std::ffi::CStr::from_ptr((*entry).pw_dir);
        let path = PathBuf::from(
            <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(dir.to_bytes()),
        );
        path.canonicalize().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variable_wins_over_home() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("leases");
        let env = |name: &str| (name == ROOT_VAR).then(|| root.clone().into_os_string());
        assert_eq!(root_from(&env).unwrap(), root);
        let home = dir.path().to_path_buf();
        let env = |name: &str| (name == "HOME").then(|| home.clone().into_os_string());
        assert_eq!(root_from(&env).unwrap(), home.join(".openagents/leases"));
        assert!(root_from(&|_| None).is_err());
    }

    #[cfg(unix)]
    #[test]
    #[should_panic(expected = "real home")]
    fn a_test_that_reaches_the_real_home_panics() {
        let real = real_home().expect("this user has a home");
        let env = |name: &str| (name == "HOME").then(|| real.clone().into_os_string());
        let _ = root_from(&env);
    }
}
