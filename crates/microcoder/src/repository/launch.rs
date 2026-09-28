//! Detach the common task owner while retaining the exact operator grant.
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
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

/// A launch receipt means a host process started, not that it admitted the task.
/// The child acquires the existing owner lease before any model or shell effect.
pub fn start(directory: &Path, bytes: &[u8]) -> Result<Launched, String> {
    let attempt = || -> Result<Launched, Box<dyn std::error::Error>> {
        let grant = task::owner::Grant::parse(bytes)?;
        let configuration = grant
            .adapter_configuration
            .as_ref()
            .ok_or("repository launch requires an adapter configuration")?;
        if !matches!(configuration.provider.as_str(), "codex" | "claude") {
            return Err("the detached repository CLI requires the codex or claude provider".into());
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
        let directory = directory.canonicalize()?;
        let executable = std::env::current_exe()?.canonicalize()?;
        let controller_digest = nostr::contracts::digest_bytes(&std::fs::read(&executable)?);
        if configuration
            .expected_controller_digest
            .as_ref()
            .is_some_and(|pin| pin != &controller_digest)
        {
            return Err("repository controller differs from the pinned executable".into());
        }
        let identity = format!(
            "repository-launch-{}-{}-{}",
            task.task_id,
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
            "TYPESAFE_API_KEY",
            "TYPESAFE_BASE_URL",
            "TYPESAFE_DEFAULT_MODEL",
        ] {
            if let Some(value) = std::env::var_os(key) {
                process.env(key, value);
            }
        }
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
            task_id: task.task_id,
            owner_process: child.id(),
            admission: "pending",
            grant_digest: nostr::contracts::digest_bytes(bytes),
            diagnostic_path,
        })
    };
    attempt().map_err(|error| error.to_string())
}

/// This process's account name from the account database.
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
