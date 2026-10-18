//! The build gate for the desktop app's agent chats (#11971).
//!
//! Foreman's cap on heavy builds was only an instruction, so many chats
//! compiled at once, each into a private copy of a warm target folder. The
//! gate makes the cap a rule. The app writes [`AGENT_CARGO_SHIM`] as `cargo`
//! into its own folder ([`Gate::install`]) and puts that folder first on the
//! `PATH` of every agent it starts ([`Gate::vars`]). For `build`, `test`,
//! `check`, `clippy`, `run`, `bench`, and `nextest`, the shim:
//!
//! - Builds in one shared target folder per Cargo workspace under
//!   [`TARGETS_VAR`] (`~/work/agent-targets` by default). Every worktree of a
//!   repository shares the folder for the same workspace, and Cargo's own
//!   lock keeps two builds in it from colliding. `OPENAGENTS_KEEP_TARGET_DIR=1`
//!   keeps the caller's `CARGO_TARGET_DIR` instead.
//! - Keys intermediate compiler state by Cargo's workspace manifest path hash
//!   under the shared target, and disables incremental compilation, including
//!   builds already inside a lease (#12071).
//! - Refuses to start below the disk floor, [`FLOOR_GIB`] GiB free, with a
//!   plain message.
//! - Waits for one of the `build` leases under `openagents lease build
//!   --keep-target-dir`, with the count the coordinator chose
//!   ([`LIMIT_FILE`] in the lease root, [`DEFAULT_LIMIT`] when unset).
//!
//! Every other subcommand, and a build already inside a `build` lease, runs
//! the real `cargo` directly. [`status`] is what the coordinator reads: the
//! builds running and waiting, the limit, and the free disk.
//!
//! [`sweep`] keeps the shared folders from filling the disk: launchd runs it
//! every five minutes, with no agent session, to hold the 100 GiB floor
//! (#12023).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

pub mod sweep;

/// Names another folder for the agent `cargo` shim than
/// `~/.openagents/bin/agent-build-shims`.
pub const SHIMS_VAR: &str = "OPENAGENTS_AGENT_SHIMS";
/// The folder that holds the shared target folders, one per workspace.
pub const TARGETS_VAR: &str = "OPENAGENTS_AGENT_TARGETS";
/// The build limit when the coordinator has set none.
pub const LIMIT_VAR: &str = "OPENAGENTS_AGENT_BUILD_LIMIT";
/// Overrides the disk floor, in GiB.
pub const FLOOR_VAR: &str = "OPENAGENTS_AGENT_FLOOR_GIB";
/// Set to `0` in the app's environment to start agents without the gate.
pub const GATE_VAR: &str = "OPENAGENTS_AGENT_BUILD_GATE";
/// The file in the lease root that holds the coordinator's build limit.
pub const LIMIT_FILE: &str = "agent-build-limit";
/// How many agent builds run at once when the coordinator has set no limit.
pub const DEFAULT_LIMIT: u64 = 5;
/// The free space, in GiB, below which an agent build doesn't start.
pub const FLOOR_GIB: u64 = 30;
/// One GiB.
pub const GIB: u64 = 1 << 30;

/// Shared settings for direct builds and both Cargo shims.
pub const BUILD_ENV_SCRIPT: &str = include_str!("../../../scripts/dev/cargo-build-env.sh");

/// The agent `cargo` shim's text.
pub const AGENT_CARGO_SHIM: &str = r#"#!/bin/sh
# OpenAgents build gate for agent chats, written by the coder-lease crate.
# Heavy subcommands build in the workspace's shared target folder and wait
# for a build slot; others pass through. docs/coder/runtime/leases.md
# explains it. Don't edit: it's rewritten.
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
  echo "cargo: no cargo on PATH besides the build gate in $shims" >&2
  exit 127
fi
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
# Cargo's workspace-relative fingerprints can call another worktree's source
# Fresh without invoking rustc or kache. Keep compiler state under a manifest-
# path hash inside the one shared target; kache still keys outputs by content.
CARGO_INCREMENTAL=0
export CARGO_INCREMENTAL
manifest=
previous=
for arg in "$@"; do
  if [ "$previous" = manifest ]; then manifest=$arg; previous=; continue; fi
  case $arg in
    --manifest-path) previous=manifest ;;
    --manifest-path=*) manifest=${arg#--manifest-path=} ;;
    --) break ;;
  esac
done
build_dir=$(pwd -P)
if [ -n "$manifest" ]; then
  build_dir=$(CDPATH= cd -- "$(dirname -- "$manifest")" && pwd -P) || exit 1
  project=$("$real" locate-project --workspace --message-format plain --manifest-path "$manifest" 2>/dev/null) || project=
else
  project=$("$real" locate-project --workspace --message-format plain 2>/dev/null) || project=
fi
ws=
if [ -f "$project" ]; then
  ws=$(CDPATH= cd -- "$(dirname -- "$project")" && pwd -P) || exit 1
else
  project=
  # Older Cargo shims may not implement locate-project. Keep their lock-based
  # discovery rather than assigning a fresh target for every member directory.
  dir=$build_dir
  while [ -n "$dir" ]; do
    if [ -f "$dir/Cargo.lock" ]; then ws=$dir; break; fi
    [ "$dir" = / ] && break
    dir=$(dirname -- "$dir")
  done
fi
[ -z "$ws" ] || build_dir=$ws
targets=${OPENAGENTS_AGENT_TARGETS:-}
if [ -n "$targets" ] && [ "${OPENAGENTS_KEEP_TARGET_DIR:-}" != 1 ]; then
  if [ -n "$ws" ]; then
    name=$(basename -- "$ws")
    key=$ws
    common=$(cd "$ws" && git rev-parse --git-common-dir 2>/dev/null) || common=
    top=$(cd "$ws" && git rev-parse --show-toplevel 2>/dev/null) || top=
    [ -n "$common" ] && common=$(cd "$ws" && cd "$common" 2>/dev/null && pwd -P)
    [ -n "$top" ] && top=$(cd "$top" 2>/dev/null && pwd -P)
    if [ -n "$common" ] && [ -n "$top" ]; then
      case $common in
        */.git) repo=$(basename -- "$(dirname -- "$common")") ;;
        *) repo=$(basename -- "$common" .git) ;;
      esac
      rel=${ws#"$top"}
      key=$common$rel
      name=$repo$(printf '%s' "$rel" | tr '/' '-')
    fi
    sum=$(printf '%s' "$key" | cksum | awk '{ print $1 }')
    shared=$targets/$name-$sum
    if [ -n "${CARGO_TARGET_DIR:-}" ] && [ "$CARGO_TARGET_DIR" != "$shared" ]; then
      echo "cargo: building in the shared folder $shared, not $CARGO_TARGET_DIR" >&2
    fi
    mkdir -p "$shared" 2>/dev/null
    CARGO_TARGET_DIR=$shared
    export CARGO_TARGET_DIR
  fi
fi
# KEEP_TARGET_DIR and an existing lease only bypass target assignment or
# admission, not source identity. Override stale inherited build-dir settings.
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
  CARGO_TARGET_DIR=${ws:-$build_dir}/target
  export CARGO_TARGET_DIR
fi
oa_build_manifest=${project:-${ws:-$build_dir}/Cargo.toml}
. "$shims/build-env.sh" || exit 1
case ",${OPENAGENTS_LEASES:-}," in
  *,build,*) exec "$real" "$@" ;;
esac
floor=${OPENAGENTS_AGENT_FLOOR_GIB:-30}
case $floor in
  ''|*[!0-9]*) floor=30 ;;
esac
probe=${CARGO_TARGET_DIR:-.}
while [ ! -d "$probe" ]; do
  probe=$(dirname -- "$probe")
done
free=$(df -Pk "$probe" 2>/dev/null | awk 'NR == 2 { print $4 }')
case $free in
  ''|*[!0-9]*) free= ;;
esac
if [ -n "$free" ] && [ "$free" -lt $((floor * 1048576)) ]; then
  echo "cargo: only $((free / 1048576)) GiB is free on this disk, and agent builds need at least $floor GiB. Free up space, then try again." >&2
  exit 1
fi
root=${OPENAGENTS_LEASE_ROOT:-${HOME:-/nonexistent}/.openagents/leases}
limit=
[ -f "$root/agent-build-limit" ] && limit=$(sed -n 1p "$root/agent-build-limit" 2>/dev/null)
case $limit in
  ''|*[!0-9]*|0) limit=${OPENAGENTS_AGENT_BUILD_LIMIT:-5} ;;
esac
case $limit in
  ''|*[!0-9]*|0) limit=5 ;;
esac
OPENAGENTS_BUILD_LEASES=$limit
OPENAGENTS_SLOT_FREE_GB=$(( (floor * 1073741824 + 999999999) / 1000000000 ))
OPENAGENTS_BUILD_DISK_GB=${OPENAGENTS_BUILD_DISK_GB:-1}
export OPENAGENTS_BUILD_LEASES OPENAGENTS_SLOT_FREE_GB OPENAGENTS_BUILD_DISK_GB
lease=${OPENAGENTS_LEASE_BIN:-}
[ -n "$lease" ] || lease=${HOME:-/nonexistent}/.openagents/bin/openagents
if [ ! -x "$lease" ]; then
  echo "cargo: openagents isn't installed, so cargo $sub runs without waiting for a build slot" >&2
  exec "$real" "$@"
fi
OPENAGENTS_LEASE_SHIM=1
export OPENAGENTS_LEASE_SHIM
exec "$lease" lease build --keep-target-dir -- "$real" "$@"
"#;

/// Where the app's agents get the gate from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gate {
    /// The folder the `cargo` shim is written to, first on agents' `PATH`.
    pub shims: PathBuf,
    /// The folder of shared target folders.
    pub targets: PathBuf,
    /// The `openagents` command that takes the build lease, when found.
    pub bin: Option<PathBuf>,
}

impl Gate {
    /// The gate this environment names: [`SHIMS_VAR`] else
    /// `~/.openagents/bin/agent-build-shims`, [`TARGETS_VAR`] else
    /// `~/work/agent-targets`, and `OPENAGENTS_LEASE_BIN` else
    /// `~/.openagents/bin/openagents` when it exists. `None` when
    /// [`GATE_VAR`] is `0` or no home is known.
    #[must_use]
    pub fn from(env: &dyn Fn(&str) -> Option<OsString>) -> Option<Gate> {
        let set = |name: &str| env(name).filter(|value| !value.is_empty());
        if set(GATE_VAR).is_some_and(|value| value == "0") {
            return None;
        }
        let home = user_home::home_in(set).map(PathBuf::from);
        let shims = set(SHIMS_VAR).map(PathBuf::from).or_else(|| {
            home.as_ref()
                .map(|home| home.join(".openagents/bin/agent-build-shims"))
        })?;
        let targets = set(TARGETS_VAR)
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|home| home.join("work/agent-targets")))?;
        let bin = set(crate::shim::BIN_VAR)
            .map(PathBuf::from)
            .or_else(|| {
                home.as_ref()
                    .map(|home| home.join(".openagents/bin/openagents"))
            })
            .filter(|bin| bin.is_file());
        Some(Gate {
            shims,
            targets,
            bin,
        })
    }

    /// [`Gate::from`] over this process's environment.
    #[must_use]
    pub fn from_env() -> Option<Gate> {
        Gate::from(&|name| std::env::var_os(name))
    }

    /// Writes the shim as `cargo` in [`Gate::shims`], unless an up-to-date
    /// copy is there.
    ///
    /// # Errors
    /// The folder or the shim can't be written.
    pub fn install(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.shims)?;
        for (name, content) in [
            ("build-env.sh", BUILD_ENV_SCRIPT),
            ("cargo", AGENT_CARGO_SHIM),
        ] {
            let path = self.shims.join(name);
            if std::fs::read(&path).is_ok_and(|bytes| bytes == content.as_bytes()) {
                continue;
            }
            let staging = self
                .shims
                .join(format!(".{name}.{}.new", std::process::id()));
            std::fs::write(&staging, content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))?;
            }
            std::fs::rename(&staging, &path)?;
        }
        Ok(())
    }

    /// The variables an agent's environment sets: `PATH` as `path` with the
    /// shim folder first, [`TARGETS_VAR`], and `OPENAGENTS_LEASE_BIN` when
    /// the command is known.
    #[must_use]
    pub fn vars(&self, path: Option<&OsStr>) -> Vec<(OsString, OsString)> {
        let mut vars = vec![
            (
                OsString::from("PATH"),
                crate::shim::path_with(&self.shims, path),
            ),
            (
                OsString::from(TARGETS_VAR),
                self.targets.clone().into_os_string(),
            ),
        ];
        if let Some(bin) = &self.bin {
            vars.push((
                OsString::from(crate::shim::BIN_VAR),
                bin.clone().into_os_string(),
            ));
        }
        vars
    }
}

/// The coordinator's build limit in the lease root `root`, else
/// [`LIMIT_VAR`], else [`DEFAULT_LIMIT`].
#[must_use]
pub fn limit(root: &Path) -> u64 {
    let parse = |text: &str| text.trim().parse::<u64>().ok().filter(|&n| n > 0);
    std::fs::read_to_string(root.join(LIMIT_FILE))
        .ok()
        .and_then(|text| text.lines().next().and_then(parse))
        .or_else(|| std::env::var(LIMIT_VAR).ok().as_deref().and_then(parse))
        .unwrap_or(DEFAULT_LIMIT)
}

/// Sets the build limit in the lease root `root`. Builds that start from
/// now on wait under it; running builds keep going.
///
/// # Errors
/// `limit` is 0, or the file can't be written.
pub fn set_limit(root: &Path, limit: u64) -> std::io::Result<()> {
    if limit == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the build limit must be at least 1",
        ));
    }
    std::fs::create_dir_all(root)?;
    let staging = root.join(format!(".{LIMIT_FILE}.{}.new", std::process::id()));
    std::fs::write(&staging, format!("{limit}\n"))?;
    std::fs::rename(&staging, root.join(LIMIT_FILE))
}

/// What the coordinator reads about agent builds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    /// Builds that hold a build slot now.
    pub running: u64,
    /// Builds waiting for a slot.
    pub waiting: usize,
    /// How many builds may run at once.
    pub limit: u64,
    /// Free bytes on the disk the builds write to, when it could be read.
    pub free_bytes: Option<u64>,
    /// The floor, in bytes, below which a build doesn't start.
    pub floor_bytes: u64,
    /// Whether the free space is below the floor.
    pub below_floor: bool,
    /// The sessions of the running builds, such as `claude-code:ID`.
    pub holders: Vec<String>,
}

/// The floor in bytes: [`FLOOR_VAR`] GiB, else [`FLOOR_GIB`].
#[must_use]
pub fn floor_bytes(env: &dyn Fn(&str) -> Option<String>) -> u64 {
    env(FLOOR_VAR)
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(FLOOR_GIB)
        .saturating_mul(GIB)
}

/// The builds in the lease root `root` and the free space at `disk` (its
/// nearest existing folder), with `free_disk` reading the volume. A root
/// that doesn't exist yet holds no builds.
///
/// # Errors
/// The lease table can't be read.
pub fn status_with(
    root: &Path,
    disk: &Path,
    floor_bytes: u64,
    free_disk: &dyn Fn(&Path) -> std::io::Result<u64>,
) -> Result<Status, crate::Error> {
    let seen = if root.is_dir() {
        crate::observe(root, Duration::from_secs(60))?
    } else {
        crate::Observed::default()
    };
    let probe = disk
        .ancestors()
        .find(|dir| dir.is_dir())
        .unwrap_or(Path::new("/"));
    let free_bytes = free_disk(probe).ok();
    Ok(Status {
        running: seen.held_amount("build"),
        waiting: seen.waiting_for("build"),
        limit: limit(root),
        free_bytes,
        floor_bytes,
        below_floor: free_bytes.is_some_and(|free| free < floor_bytes),
        holders: seen
            .held
            .iter()
            .filter(|entry| entry.resource == "build")
            .map(|entry| entry.holder.session.clone())
            .collect(),
    })
}

/// [`status_with`] over this machine's lease root, the gate's target
/// folder, and the real volume.
///
/// # Errors
/// A sentence when the lease root can't be found or its table read.
pub fn status() -> Result<Status, String> {
    let root = crate::root_from_env()?;
    let disk = Gate::from_env().map_or_else(|| root.clone(), |gate| gate.targets);
    let floor = floor_bytes(&|name| std::env::var(name).ok());
    status_with(&root, &disk, floor, &crate::free_disk).map_err(|error| error.to_string())
}

#[cfg(all(test, unix))]
mod tests;
