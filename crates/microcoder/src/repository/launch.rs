//! Detach the common task owner while retaining the exact operator grant.
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use coder::task::{self, Store};
use serde::Serialize;

#[derive(Serialize)]
pub struct Launched {
    pub task_id: String,
    pub owner_process: u32,
    pub admission: &'static str,
    pub grant_digest: String,
    pub diagnostic_path: PathBuf,
}

/// A platform with neither the Unix nor the Windows write boundary starts
/// no task owner.
#[cfg(not(any(unix, windows)))]
pub fn start(_directory: &Path, _bytes: &[u8]) -> Result<Launched, String> {
    Err("repository tasks need a write boundary, which this computer does not have".into())
}

/// What the launcher checks before it starts an owner, on every platform:
/// the grant, its task, and the executable it pins. Returns the task ID.
#[cfg(any(unix, windows))]
fn admit(directory: &Path, bytes: &[u8]) -> Result<(String, PathBuf), Box<dyn std::error::Error>> {
    let grant = task::owner::Grant::parse(bytes)?;
    let configuration = grant
        .adapter_configuration
        .as_ref()
        .ok_or("repository launch requires an adapter configuration")?;
    // Every provider the repository engine runs a turn on: Codex and
    // Claude Code through Microcoder, Grok Build, OpenCode, and Devin as
    // whole agents over ACP. Grok Build is a local run's default (#10091).
    if !matches!(
        configuration.provider.as_str(),
        "codex" | "claude" | "grok" | "opencode" | "devin"
    ) {
        return Err(
            "the detached repository CLI requires the codex, claude, grok, opencode, or devin provider"
                .into(),
        );
    }
    // The launcher, like the owner it starts, waits out a busy store.
    let store = Store::open_for_owner(directory)?;
    let task = store.show(&grant.task_id)?;
    if task.status != task::Status::Queued || task.run.is_some() {
        return Err(task::Error::InvalidTransition.into());
    }
    if task.intent_digest != grant.intent_digest || task.revision != grant.expected_revision {
        return Err(task::Error::RevisionMismatch.into());
    }
    if task.intent.configuration.adapter != task::adapter::NAME
        || !task
            .intent
            .configuration
            .model
            .as_deref()
            .is_some_and(|model| configuration.admits_model(model))
    {
        return Err("task and repository launch configuration differ".into());
    }
    // This very program, even when a rebuild replaced its file while it
    // ran (#10237): the owner it starts is the engine that admitted it.
    let (_, executable) = task::autostart::running_program()?;
    // Only a pinned executable is read and digested: reading the whole
    // engine on every start costs a start time for nothing (#10115).
    if let Some(pin) = &configuration.expected_controller_digest
        && pin != &nostr::contracts::digest_bytes(&std::fs::read(&executable)?)
    {
        return Err("repository controller differs from the pinned executable".into());
    }
    Ok((task.task_id, executable))
}

/// The Windows launcher: the same checks and retained grant as on Unix,
/// and an owner started in a process group of its own with no console
/// window, which outlives this process as `setsid` makes it do on Unix.
/// Its environment is cleared but for the account and profile variables a
/// Windows program needs, and the model host's own.
#[cfg(windows)]
pub fn start(directory: &Path, bytes: &[u8]) -> Result<Launched, String> {
    /// `CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`.
    const DETACHED: u32 = 0x0000_0200 | 0x0800_0000;
    let attempt = || -> Result<Launched, Box<dyn std::error::Error>> {
        let (task_id, executable) = admit(directory, bytes)?;
        let directory = directory.canonicalize()?;
        let identity = format!(
            "repository-launch-{task_id}-{}-{}",
            std::process::id(),
            atif::now_ms()
        );
        // The store directory admits only this user, and what is created
        // in it inherits that.
        let saved_grant = directory.join(format!("{identity}.grant.json"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&saved_grant)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        let diagnostic_path = directory.join(format!("{identity}.jsonl"));
        let diagnostic = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&diagnostic_path)?;
        let mut process = Command::new(coder_boundary::plain_path(&executable));
        process
            .env_clear()
            .envs(task::owner::base_environment())
            .env("PATH", task::owner::SYSTEM_PATH);
        for key in [
            "HOME",
            "USERPROFILE",
            "USERNAME",
            "USERDOMAIN",
            "APPDATA",
            "LOCALAPPDATA",
            "ProgramData",
            crate::claude::BIN_VAR,
            // Where the whole-agent engines are, when the launcher was told:
            // never a credential.
            acp_client::grok::BIN_VAR,
            acp_client::grok::HOME_VAR,
            acp_client::opencode::BIN_VAR,
            acp_client::devin::BIN_VAR,
            "TYPESAFE_API_KEY",
            "TYPESAFE_BASE_URL",
            "TYPESAFE_DEFAULT_MODEL",
            jev_hosted::RELAY_VAR,
            jev_hosted::WORKER_VAR,
            jev_hosted::HOSTED_VAR,
            // The recipe's off switch, for a with/without measurement (#10209).
            crate::repository::recipe::OFF_VAR,
        ] {
            if let Some(value) = std::env::var_os(key) {
                process.env(key, value);
            }
        }
        // The model host finds the Codex login where the readiness check
        // did: `$CODEX_HOME` when the person set it, else `~/.codex`
        // (#10083).
        if let Some(codex) = codex_transport::codex::Login::home_override() {
            process.env(codex_transport::codex::HOME_VAR, codex);
        }
        // The person's toolchain variables, for a run with this
        // computer's tools, reach the model host that admits it.
        process.envs(std::env::vars_os().filter(|(key, _)| {
            key.to_string_lossy()
                .starts_with(coder_boundary::toolchains::CARRIED_PREFIX)
        }));
        process
            .args(["repository", "--store"])
            .arg(&directory)
            .arg("--grant")
            .arg(&saved_grant)
            .stdin(Stdio::null())
            .stdout(Stdio::from(diagnostic.try_clone()?))
            .stderr(Stdio::from(diagnostic))
            .creation_flags(DETACHED);
        let child = process.spawn()?;
        Ok(Launched {
            task_id,
            owner_process: child.id(),
            admission: "pending",
            grant_digest: nostr::contracts::digest_bytes(bytes),
            diagnostic_path,
        })
    };
    attempt().map_err(|error| error.to_string())
}

/// A launch receipt means a host process started, not that it admitted the task.
/// The child acquires the existing owner lease before any model or shell effect.
#[cfg(unix)]
pub fn start(directory: &Path, bytes: &[u8]) -> Result<Launched, String> {
    let attempt = || -> Result<Launched, Box<dyn std::error::Error>> {
        let (task_id, executable) = admit(directory, bytes)?;
        let directory = directory.canonicalize()?;
        let identity = format!(
            "repository-launch-{}-{}-{}",
            task_id,
            std::process::id(),
            atif::now_ms()
        );
        let saved_grant = directory.join(format!("{identity}.grant.json"));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&saved_grant)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        let diagnostic_path = directory.join(format!("{identity}.jsonl"));
        let diagnostic = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&diagnostic_path)?;
        std::fs::File::open(&directory)?.sync_all()?;
        let mut process = Command::new(executable);
        process.env_clear().env("PATH", "/usr/bin:/bin");
        // These stay in the model host. Its supervised shell children clear
        // their environment again and never receive provider credentials.
        // CLAUDE_BIN names the claude binary when it is off the fixed PATH.
        // USER and LOGNAME name the account: Claude Code on macOS finds its
        // sign-in in the Keychain under the user's name, and reports "Not
        // logged in" without it.
        // A service manager such as launchd may start the host without USER,
        // so the name comes from the account database when it is missing.
        if let Some(name) = std::env::var_os("USER").or_else(account_name) {
            process.env("USER", &name).env("LOGNAME", name);
        }
        // SHELL names the owner's login shell, which a full-access run asks
        // for the owner's environment; its commands get that environment,
        // never this process's.
        for key in [
            "HOME",
            "SHELL",
            crate::claude::BIN_VAR,
            // Where the whole-agent engines are, when the launcher was told:
            // never a credential.
            acp_client::grok::BIN_VAR,
            acp_client::grok::HOME_VAR,
            acp_client::opencode::BIN_VAR,
            acp_client::devin::BIN_VAR,
            "TYPESAFE_API_KEY",
            "TYPESAFE_BASE_URL",
            "TYPESAFE_DEFAULT_MODEL",
            jev_hosted::RELAY_VAR,
            jev_hosted::WORKER_VAR,
            jev_hosted::HOSTED_VAR,
            // The recipe's off switch, for a with/without measurement (#10209).
            crate::repository::recipe::OFF_VAR,
        ] {
            if let Some(value) = std::env::var_os(key) {
                process.env(key, value);
            }
        }
        // The model host finds the Codex login where the readiness check
        // did: `$CODEX_HOME` when the person set it, else `~/.codex`
        // (#10083).
        if let Some(codex) = codex_transport::codex::Login::home_override() {
            process.env(codex_transport::codex::HOME_VAR, codex);
        }
        // The person's toolchain variables, for a run with this
        // computer's tools, reach the model host that admits it.
        process.envs(std::env::vars_os().filter(|(key, _)| {
            key.to_string_lossy()
                .starts_with(coder_boundary::toolchains::CARRIED_PREFIX)
        }));
        process
            .args(["repository", "--store"])
            .arg(&directory)
            .arg("--grant")
            .arg(&saved_grant)
            .stdin(Stdio::null())
            .stdout(Stdio::from(diagnostic.try_clone()?))
            .stderr(Stdio::from(diagnostic));
        // SAFETY: setsid is async-signal-safe and touches no parent-memory state.
        unsafe {
            process.pre_exec(|| {
                if libc::setsid() == -1 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let child = process.spawn()?;
        Ok(Launched {
            task_id,
            owner_process: child.id(),
            admission: "pending",
            grant_digest: nostr::contracts::digest_bytes(bytes),
            diagnostic_path,
        })
    };
    attempt().map_err(|error| error.to_string())
}

/// This process's account name from the account database.
#[cfg(unix)]
fn account_name() -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: getpwuid returns a pointer into static storage or null; the
    // name is copied before any other call that could reuse it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_name.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*entry).pw_name);
        Some(std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string())
    }
}
