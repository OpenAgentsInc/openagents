//! Durable agent scratch: one private directory a session, under
//! `~/.openagents/scratch/<session>/`, that survives a reboot the way
//! `/private/tmp` doesn't.
//!
//! The scratch root is `$OPENAGENTS_SCRATCH_ROOT`, else `scratch` beside
//! the lease root, which is `~/.openagents/scratch` for the machine's
//! table. A session's directory is its identity ([`crate::Session`]) made
//! into a safe file name ([`dir_name`]), created with mode `0700`, and
//! holds a `.session` file naming the session, which the disk monitor reads
//! to age out the scratch of sessions that ended.
//!
//! A command run under a lease gets the directory in [`SCRATCH_VAR`], and
//! so does every agent Coder delegates to once a program turned the lease
//! shims on ([`crate::shim::enable`]). Evidence a check needs still belongs
//! in the task store or the repository.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The variable that names a session's scratch directory.
pub const SCRATCH_VAR: &str = "OPENAGENTS_SCRATCH";
/// Names another scratch root than `~/.openagents/scratch`.
pub const SCRATCH_ROOT_VAR: &str = "OPENAGENTS_SCRATCH_ROOT";
/// The file in a session's scratch directory that names the session.
pub const SESSION_FILE: &str = ".session";

/// The longest directory name a session gets.
const MAX_NAME: usize = 96;

/// The scratch root: `$OPENAGENTS_SCRATCH_ROOT`, else `scratch` beside the
/// lease root ([`crate::root_from`]).
///
/// # Errors
/// A sentence when neither variable nor `HOME` is set.
pub fn root_from(env: &dyn Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let set = |name: &str| env(name).filter(|value| !value.is_empty());
    let root = if let Some(root) = set(SCRATCH_ROOT_VAR) {
        PathBuf::from(root)
    } else {
        let leases = crate::root_from(&set).map_err(|_| {
            format!(
                "set HOME, {SCRATCH_ROOT_VAR}, or {} to choose the scratch root",
                crate::ROOT_VAR
            )
        })?;
        beside(&leases)
    };
    crate::refuse_real_home(&root);
    Ok(root)
}

/// [`root_from`] over this process's environment.
///
/// # Errors
/// A sentence when neither variable nor `HOME` is set.
pub fn root_from_env() -> Result<PathBuf, String> {
    root_from(&|name| std::env::var_os(name))
}

/// The scratch root beside a lease root: its sibling `scratch`.
#[must_use]
pub fn beside(lease_root: &Path) -> PathBuf {
    lease_root.parent().map_or_else(
        || lease_root.join("scratch"),
        |parent| parent.join("scratch"),
    )
}

/// A session's identity as a directory name: letters, digits, `.`, `_`,
/// and `-` are kept, `:` becomes `-`, anything else becomes `_`, a leading
/// `.` is dropped, and the name is at most 96 characters. A session with
/// nothing left is `session`.
#[must_use]
pub fn dir_name(session: &str) -> String {
    let mapped: String = session
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => c,
            ':' => '-',
            _ => '_',
        })
        .collect();
    let name: String = mapped
        .trim_start_matches('.')
        .chars()
        .take(MAX_NAME)
        .collect();
    if name.is_empty() {
        "session".to_owned()
    } else {
        name
    }
}

/// The scratch directory of `session` under `root`, without creating it.
#[must_use]
pub fn path(root: &Path, session: &str) -> PathBuf {
    root.join(dir_name(session))
}

/// Creates the scratch directory of `session` under `root`, private to
/// this user (mode `0700`), writes its `.session` file when missing, and
/// returns its path. An existing directory keeps its contents.
///
/// # Errors
/// The directory can't be created, or something other than a directory,
/// such as a symbolic link, is already at its path.
pub fn ensure(root: &Path, session: &str) -> std::io::Result<PathBuf> {
    crate::refuse_real_home(root);
    let dir = path(root, session);
    make_private(root)?;
    make_private(&dir)?;
    let marker = dir.join(SESSION_FILE);
    if std::fs::symlink_metadata(&marker).is_err() {
        crate::table::write_atomic(&marker, session.as_bytes())?;
    }
    Ok(dir)
}

/// Creates `dir` and its missing parents, and makes `dir` private to this
/// user (mode `0700`).
///
/// # Errors
/// The directory can't be created, or something other than a directory,
/// such as a symbolic link, is at its path.
pub fn make_private(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(dir)?;
    let meta = std::fs::symlink_metadata(dir)?;
    if !meta.is_dir() {
        return Err(std::io::Error::other(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if meta.permissions().mode() & 0o777 != 0o700 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// The session a scratch directory belongs to: its `.session` file, else
/// its name.
#[must_use]
pub fn session_of(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(SESSION_FILE))
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| {
            dir.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// The process a session's identity names, when it names one: the `PID`
/// of `process:PID` and of `AGENT:PID` for an agent process name, such as
/// `codex:4242` or `coder:77`.
#[must_use]
pub fn session_pid(session: &str) -> Option<u32> {
    let (name, pid) = session.rsplit_once(':')?;
    if name != "process" && !crate::AGENT_PROCESSES.contains(&name) {
        return None;
    }
    pid.parse().ok().filter(|pid| *pid > 1)
}

/// The session Coder's delegates share: `$OPENAGENTS_SESSION` when this
/// process runs under one, else `coder:<this process>`.
#[must_use]
pub fn delegate_session() -> String {
    std::env::var(crate::SESSION_VAR)
        .ok()
        .filter(|session| !session.is_empty())
        .unwrap_or_else(|| format!("coder:{}", std::process::id()))
}

/// The sessions that hold or wait for a lease in the table at
/// `lease_root`, after the table drops leases whose holders are gone.
/// Empty, and nothing is created, when no table is there.
///
/// # Errors
/// The table can't be read or written.
pub fn live_sessions(
    lease_root: &Path,
) -> Result<std::collections::BTreeSet<String>, crate::Error> {
    if std::fs::symlink_metadata(lease_root.join("table.json")).is_err() {
        return Ok(std::collections::BTreeSet::new());
    }
    let mut guard = crate::table::Guard::open(lease_root)?;
    if guard.prune() > 0 {
        guard.save()?;
    }
    Ok(guard
        .table
        .leases
        .iter()
        .map(|entry| entry.holder.session.clone())
        .collect())
}

static DELEGATES: std::sync::RwLock<Option<PathBuf>> = std::sync::RwLock::new(None);

/// Creates the scratch directory of [`delegate_session`] under `root` and
/// gives it to every agent this process delegates to from now on, in
/// [`SCRATCH_VAR`] ([`crate::shim::delegate_vars`]).
///
/// # Errors
/// The directory can't be created; delegates then get none.
pub fn enable(root: &Path) -> std::io::Result<PathBuf> {
    let dir = ensure(root, &delegate_session())?;
    if let Ok(mut delegates) = DELEGATES.write() {
        *delegates = Some(dir.clone());
    }
    Ok(dir)
}

/// [`enable`] at the scratch root this environment names. A process that
/// runs under a lease or a delegation already, so that
/// `$OPENAGENTS_SCRATCH` is set, passes that directory on.
///
/// # Errors
/// A sentence when the root can't be found or the directory written.
pub fn enable_from_env() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os(SCRATCH_VAR).filter(|dir| !dir.is_empty()) {
        let dir = PathBuf::from(dir);
        if make_private(&dir).is_ok() {
            if let Ok(mut delegates) = DELEGATES.write() {
                *delegates = Some(dir.clone());
            }
            return Ok(dir);
        }
    }
    let root = root_from_env()?;
    enable(&root).map_err(|error| {
        format!(
            "the scratch directory could not be made under {}: {error}",
            root.display()
        )
    })
}

/// Stops giving delegates a scratch directory.
pub fn disable() {
    if let Ok(mut delegates) = DELEGATES.write() {
        *delegates = None;
    }
}

/// The scratch directory delegates get, once [`enable`] chose one.
#[must_use]
pub fn delegate_dir() -> Option<PathBuf> {
    DELEGATES.read().ok().and_then(|dir| dir.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_names_become_safe_directory_names() {
        assert_eq!(dir_name("claude-code:abc-123"), "claude-code-abc-123");
        assert_eq!(dir_name("codex:4242"), "codex-4242");
        assert_eq!(dir_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(dir_name(".hidden"), "hidden");
        assert_eq!(dir_name("..."), "session");
        assert_eq!(dir_name(""), "session");
        assert_eq!(dir_name("a b/c"), "a_b_c");
        assert_eq!(dir_name(&"x".repeat(300)).len(), MAX_NAME);
    }

    #[test]
    fn the_root_is_the_variable_else_beside_the_leases() {
        let env = |name: &str| (name == SCRATCH_ROOT_VAR).then(|| OsString::from("/s"));
        assert_eq!(root_from(&env).unwrap(), PathBuf::from("/s"));
        let env = |name: &str| (name == crate::ROOT_VAR).then(|| OsString::from("/x/leases"));
        assert_eq!(root_from(&env).unwrap(), PathBuf::from("/x/scratch"));
        let env = |name: &str| (name == "HOME").then(|| OsString::from("/h"));
        assert_eq!(
            root_from(&env).unwrap(),
            PathBuf::from("/h/.openagents/scratch")
        );
        assert!(root_from(&|_| None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ensure_makes_a_private_directory_that_names_its_session() {
        use std::os::unix::fs::PermissionsExt as _;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("scratch");
        let dir = ensure(&root, "codex:4242").unwrap();
        assert_eq!(dir, root.join("codex-4242"));
        for path in [&root, &dir] {
            let mode = std::fs::metadata(path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", path.display());
        }
        assert_eq!(session_of(&dir), "codex:4242");
        std::fs::write(dir.join("keep"), "x").unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(ensure(&root, "codex:4242").unwrap(), dir);
        assert!(dir.join("keep").is_file());
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        // A link in the session's place is refused, not followed.
        let elsewhere = temp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join("linked")).unwrap();
        assert!(ensure(&root, "linked").is_err());
    }

    #[test]
    fn live_sessions_are_the_holders_still_running() {
        let temp = tempfile::tempdir().unwrap();
        let leases = temp.path().join("leases");
        assert!(live_sessions(&leases).unwrap().is_empty());
        assert!(!leases.exists(), "reading created the lease root");
        let broker = crate::Broker::new(
            leases.clone(),
            crate::Limits {
                build: 1,
                memory_gib: 1,
                disk_floor_gb: 1,
                build_disk_gb: 0,
            },
        );
        let mut holder = crate::Holder::detect("cargo");
        holder.session = "codex:4242".to_owned();
        let lease = broker
            .acquire(crate::Request::new(crate::Resource::Gpu, holder))
            .unwrap();
        let env = lease.env();
        let scratch = env
            .iter()
            .find(|(name, _)| name == SCRATCH_VAR)
            .map(|(_, value)| PathBuf::from(value))
            .unwrap();
        assert_eq!(scratch, temp.path().join("scratch/codex-4242"));
        assert!(scratch.is_dir());
        assert!(live_sessions(&leases).unwrap().contains("codex:4242"));
        drop(lease);
        assert!(live_sessions(&leases).unwrap().is_empty());
    }

    #[test]
    fn only_process_and_agent_sessions_name_a_process() {
        assert_eq!(session_pid("codex:4242"), Some(4242));
        assert_eq!(session_pid("process:77"), Some(77));
        assert_eq!(session_pid("coder:9"), Some(9));
        assert_eq!(session_pid("claude-code:abc"), None);
        assert_eq!(session_pid("studio-seat:3"), None);
        assert_eq!(session_pid("process:1"), None);
    }
}
