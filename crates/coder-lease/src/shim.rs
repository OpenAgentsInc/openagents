//! Lease shims: a `cargo` that takes a build lease for heavy subcommands,
//! put first on the `PATH` of every agent Coder delegates to.
//!
//! The shim is a POSIX `sh` script written from here into
//! `~/.openagents/bin/lease-shims/` ([`SHIMS_VAR`] moves it). It runs
//! `openagents lease build --keep-target-dir -- <real cargo> ARGS` for
//! `build`, `test`, `check`, `clippy`, `run`, `bench`, and `nextest`, and
//! runs the real `cargo` directly for every other subcommand. It finds the
//! real `cargo` by walking `PATH` and skipping its own directory. Under an
//! existing `build` lease (`OPENAGENTS_LEASES` names `build`) it passes
//! through, so a leased build that runs `cargo` again can't wait on itself.
//! It sets [`SHIM_VAR`], under which `openagents lease build` runs the
//! command without a lease when the lease table can't be written, such as
//! inside a sandbox that keeps the home unwritten.
//!
//! A process turns the shims on with [`enable`] before it starts
//! delegates; until then [`delegate_vars`] adds nothing, so a library's
//! tests never write into the real home. The directory is never put on
//! the owner's own interactive `PATH`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Names another shim directory than `~/.openagents/bin/lease-shims`.
pub const SHIMS_VAR: &str = "OPENAGENTS_LEASE_SHIMS";
/// Names the `openagents` binary the shim runs; else the first one on
/// `PATH`, else `~/.openagents/bin/openagents`.
pub const BIN_VAR: &str = "OPENAGENTS_LEASE_BIN";
/// Set to `1` by the shim for the `openagents lease build` it runs.
pub const SHIM_VAR: &str = "OPENAGENTS_LEASE_SHIM";

/// The `cargo` subcommands the shim runs under a build lease.
pub const LEASED: [&str; 11] = [
    "build", "b", "test", "t", "check", "c", "clippy", "run", "r", "bench", "nextest",
];

/// The `cargo` shim's text.
pub const CARGO_SHIM: &str = r#"#!/bin/sh
# OpenAgents lease shim for cargo, written by the coder-lease crate.
# Heavy subcommands run under `openagents lease build`; others pass through.
# docs/coder/runtime/leases.md explains it. Don't edit: it's rewritten.
set -f
shims=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd -P)
real=
saved_ifs=$IFS
IFS=:
for dir in $PATH; do
  [ -n "$dir" ] || dir=.
  here=$(CDPATH= cd -- "$dir" 2>/dev/null && pwd -P) || continue
  [ "$here" = "$shims" ] && continue
  if [ -f "$dir/cargo" ] && [ -x "$dir/cargo" ]; then
    real=$dir/cargo
    break
  fi
done
IFS=$saved_ifs
if [ -z "$real" ]; then
  echo "cargo: no cargo on PATH besides the lease shim in $shims" >&2
  exit 127
fi
case ",${OPENAGENTS_LEASES:-}," in
  *,build,*) exec "$real" "$@" ;;
esac
sub=
skip=
for arg in "$@"; do
  if [ -n "$skip" ]; then
    skip=
    continue
  fi
  case $arg in
    -Z|-C|--config|--color) skip=1 ;;
    +*|-*) ;;
    *) sub=$arg; break ;;
  esac
done
case $sub in
  build|b|test|t|check|c|clippy|run|r|bench|nextest) ;;
  *) exec "$real" "$@" ;;
esac
lease=${OPENAGENTS_LEASE_BIN:-}
[ -n "$lease" ] || lease=$(command -v openagents 2>/dev/null)
[ -n "$lease" ] || lease=${HOME:-/nonexistent}/.openagents/bin/openagents
if [ ! -x "$lease" ]; then
  echo "cargo: openagents was not found, so cargo $sub runs without a build lease" >&2
  exec "$real" "$@"
fi
OPENAGENTS_LEASE_SHIM=1
export OPENAGENTS_LEASE_SHIM
exec "$lease" lease build --keep-target-dir -- "$real" "$@"
"#;

struct Enabled {
    dir: PathBuf,
    bin: Option<PathBuf>,
}

static ENABLED: RwLock<Option<Enabled>> = RwLock::new(None);

/// The shim directory: `$OPENAGENTS_LEASE_SHIMS`, else
/// `~/.openagents/bin/lease-shims`.
///
/// # Errors
/// A sentence when neither the variable nor `HOME` is set.
pub fn dir_from(env: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    if let Some(dir) = set(SHIMS_VAR) {
        return Ok(PathBuf::from(dir));
    }
    let home = set("HOME").ok_or_else(|| format!("set HOME or {SHIMS_VAR}"))?;
    Ok(PathBuf::from(home).join(".openagents/bin/lease-shims"))
}

/// Writes the shims into `dir`, replacing a stale copy; an up-to-date one
/// is left alone.
///
/// # Errors
/// The directory or the shim can't be written.
pub fn install(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join("cargo");
    if std::fs::read(&path).is_ok_and(|bytes| bytes == CARGO_SHIM.as_bytes()) {
        return Ok(());
    }
    let staging = dir.join(format!(".cargo.{}.new", std::process::id()));
    std::fs::write(&staging, CARGO_SHIM)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&staging, &path)
}

/// Writes the shims into `dir` and puts them on every delegate's `PATH`
/// this process builds from now on. `bin` is the `openagents` binary the
/// shim runs, when this process knows it.
///
/// # Errors
/// The shims can't be written; delegates then get no shim.
pub fn enable(dir: &Path, bin: Option<PathBuf>) -> std::io::Result<()> {
    install(dir)?;
    if let Ok(mut enabled) = ENABLED.write() {
        *enabled = Some(Enabled {
            dir: dir.to_path_buf(),
            bin: bin.filter(|bin| bin.is_file()),
        });
    }
    Ok(())
}

/// [`enable`] at the directory this environment names, with `bin`. Unix
/// only: elsewhere it does nothing.
///
/// # Errors
/// A sentence when the directory can't be found or written.
pub fn enable_from_env(bin: Option<PathBuf>) -> Result<(), String> {
    if !cfg!(unix) {
        return Ok(());
    }
    let dir = dir_from(&|name| std::env::var_os(name))?;
    enable(&dir, bin).map_err(|error| {
        format!(
            "the lease shims could not be written to {}: {error}",
            dir.display()
        )
    })
}

/// Turns the shims off for delegates this process builds from now on.
pub fn disable() {
    if let Ok(mut enabled) = ENABLED.write() {
        *enabled = None;
    }
}

/// The shim directory delegates get, when [`enable`] turned them on.
#[must_use]
pub fn enabled() -> Option<PathBuf> {
    ENABLED
        .read()
        .ok()
        .and_then(|enabled| enabled.as_ref().map(|enabled| enabled.dir.clone()))
}

/// `path` with `dir` first, and no other copy of `dir`.
#[must_use]
pub fn path_with(dir: &Path, path: Option<&OsStr>) -> OsString {
    let rest = path
        .map(|path| {
            std::env::split_paths(path)
                .filter(|entry| entry != dir)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(rest))
        .unwrap_or_else(|_| path.map(OsStr::to_owned).unwrap_or_default())
}

/// The variables a delegate's environment sets so its `cargo` takes build
/// leases: `PATH` as `path` with the shims first, `OPENAGENTS_LEASE_BIN`
/// when this process knows the binary, and this process's
/// `OPENAGENTS_LEASE_PRIORITY` when set, so the delegate's builds wait at
/// the delegation's priority. Empty until [`enable`].
#[must_use]
pub fn delegate_vars(path: Option<&OsStr>) -> Vec<(OsString, OsString)> {
    let Ok(enabled) = ENABLED.read() else {
        return Vec::new();
    };
    let Some(enabled) = enabled.as_ref() else {
        return Vec::new();
    };
    let mut vars = vec![(OsString::from("PATH"), path_with(&enabled.dir, path))];
    if let Some(bin) = &enabled.bin {
        vars.push((OsString::from(BIN_VAR), bin.clone().into_os_string()));
    }
    if let Ok(Some(priority)) = crate::Priority::from_env() {
        vars.push((
            OsString::from(crate::PRIORITY_VAR),
            OsString::from(priority.as_str()),
        ));
    }
    vars
}

/// The `openagents` binary the shims run, when [`enable`] was given one
/// that exists.
#[must_use]
pub fn enabled_bin() -> Option<PathBuf> {
    ENABLED
        .read()
        .ok()
        .and_then(|enabled| enabled.as_ref().and_then(|enabled| enabled.bin.clone()))
}

/// [`delegate_vars`] over this process's own `PATH`.
#[must_use]
pub fn delegate_vars_here() -> Vec<(OsString, OsString)> {
    delegate_vars(std::env::var_os("PATH").as_deref())
}

/// [`delegate_vars`] as strings, for environments kept as text.
#[must_use]
pub fn delegate_vars_lossy(path: Option<&str>) -> Vec<(String, String)> {
    delegate_vars(path.map(OsStr::new))
        .into_iter()
        .map(|(name, value)| {
            (
                name.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_directory_is_the_variable_else_under_home() {
        let env = |name: &str| (name == SHIMS_VAR).then(|| OsString::from("/x/shims"));
        assert_eq!(dir_from(&env).unwrap(), PathBuf::from("/x/shims"));
        let env = |name: &str| (name == "HOME").then(|| OsString::from("/h"));
        assert_eq!(
            dir_from(&env).unwrap(),
            PathBuf::from("/h/.openagents/bin/lease-shims")
        );
        assert!(dir_from(&|_| None).is_err());
    }

    #[test]
    fn the_shims_go_first_once() {
        let dir = Path::new("/s");
        assert_eq!(
            path_with(dir, Some(OsStr::new("/usr/bin:/s:/bin"))),
            OsString::from("/s:/usr/bin:/bin")
        );
        assert_eq!(path_with(dir, None), OsString::from("/s"));
    }

    #[test]
    fn delegates_get_the_shims_only_once_enabled() {
        let scratch = tempfile::tempdir().unwrap();
        disable();
        assert!(delegate_vars(Some(OsStr::new("/bin"))).is_empty());
        let dir = scratch.path().join("shims");
        enable(&dir, None).unwrap();
        let vars = delegate_vars(Some(OsStr::new("/bin")));
        let mut expected = dir.clone().into_os_string();
        expected.push(":/bin");
        assert_eq!(vars, vec![(OsString::from("PATH"), expected)]);
        assert_eq!(
            std::fs::read_to_string(dir.join("cargo")).unwrap(),
            CARGO_SHIM
        );
        disable();
    }
}
