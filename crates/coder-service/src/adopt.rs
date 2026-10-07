//! Adopting a computer set up the old way.
//!
//! Before the desktop app, a Coder host was set up by a setup command that
//! has since been removed (#9978): the host key is a file in the access store
//! (`~/.openagents/coder-access/host.key`), the owner key a file in
//! `~/.openagents/coder-owner/owner.key`, and the host runs under the
//! launchd agent or systemd unit that [`crate::service::install`] wrote.
//!
//! The desktop app keeps those secrets in the OS keychain and runs the host
//! under its own login agent. [`detect`] reads an old-style setup without
//! changing anything, and [`adopt`] moves it over:
//!
//! 1. Copy the host key, and the owner key when it is this host's owner,
//!    into the keychain, then read each back. A read-back that differs
//!    stops here with every file in place.
//! 2. Uninstall the old agent through [`crate::service::uninstall`], and
//!    stop unless the service manager reports it stopped; then stop and
//!    remove every other user agent or unit that runs `coder host serve`
//!    ([`Stray`]), and retire the old agent's record (`service.json`
//!    becomes `service.adopted.json`) so no installer or updater brings the
//!    old agent back.
//! 3. Under the access store's lock, so no host still serves from it,
//!    check each file against the keychain once more and delete it.
//! 4. Hand over to the caller to register its own agent on the same state.
//!
//! The desktop app runs this silently on launch (#9965): there is no
//! question to answer. Any check that fails leaves the old setup running
//! exactly as it was.
//!
//! On Windows there was never a launchd agent or systemd unit, only key
//! files a `coder host init` wrote, so adoption there moves the keys and
//! has no agent to stop.
//!
//! The access store's grants, epochs, invitations, and replies, the host
//! root's settings and auto-start policy, and the tasks are never opened
//! for writing: the adopted host is the same identity over the same state,
//! so a phone paired before still passes the handshake. A keychain item
//! that already holds a *different* key is never overwritten. Every step
//! can be run again: a rerun after a crash finds the keys already in the
//! keychain and continues. Nothing here prints, logs, or returns a secret.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

use secp256k1::{Keypair, Secp256k1, SecretKey};
use serde::Serialize;

#[cfg(unix)]
use crate::launcher::{Config, Layout};
#[cfg(unix)]
use crate::service::{self, Runner, UninstallReport};
use crate::{Error, Result, fsx};

/// Runs service manager commands. Windows has none to run.
#[cfg(not(unix))]
pub trait Runner {}
#[cfg(not(unix))]
impl<T> Runner for T {}
/// What an uninstall did. Windows never uninstalls anything here.
#[cfg(not(unix))]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UninstallReport {}

/// The desktop app's own login agent label; never a stray.
pub const APP_AGENT: &str = "com.openagents.desktop.host";
/// The development host's login agent (`scripts/desktop/dev-host.sh`),
/// which serves the same keychain identity in the app agent's place on
/// purpose; never a stray.
pub const DEV_AGENT: &str = "com.openagents.dev.host";
/// Where a retired agent's record goes, beside the host root's
/// `service.json`.
pub const RETIRED_RECORD: &str = "service.adopted.json";

/// The keychain service the desktop app keeps its secrets under.
pub const KEYCHAIN_SERVICE: &str = "com.openagents.desktop";
/// The keychain account of the host's Nostr secret key.
pub const HOST_KEY_ACCOUNT: &str = "host-key";
/// The keychain account of the owner's Nostr secret key.
pub const OWNER_KEY_ACCOUNT: &str = "owner-key";

/// The most bytes a key file may hold: 64 hex characters and a newline,
/// with room for surrounding whitespace.
const KEY_FILE_MAX: u64 = 256;
/// The most bytes of the access store this module reads.
const ACCESS_MAX: u64 = 64 * 1024 * 1024;

/// The OS keychain, as the desktop app's key source reaches it. Each item
/// holds one secp256k1 secret key as 64 lowercase hexadecimal characters
/// under [`KEYCHAIN_SERVICE`] and the given account.
pub trait Keychain {
    /// The item's value, or `None` when there is no item.
    fn read(&mut self, account: &str) -> Result<Option<String>>;
    /// Creates or replaces the item.
    fn write(&mut self, account: &str, value: &str) -> Result<()>;
}

/// Where an old-style setup keeps its files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Paths {
    /// The access store: `host.key`, `access.json`, and `access.lock`.
    pub access: PathBuf,
    /// The owner key file.
    pub owner_key: PathBuf,
    /// The host root: `service.json`, `serve.json`, and the auto-start
    /// policy.
    pub host_root: PathBuf,
    /// The task store.
    pub tasks: PathBuf,
    /// The user's agent or unit directory (`~/Library/LaunchAgents` or
    /// `~/.config/systemd/user`), searched for [`Stray`] agents. `None`
    /// where there is none (Windows).
    pub registrations: Option<PathBuf>,
}

impl Paths {
    /// The default locations under `home`.
    #[must_use]
    pub fn under(home: &Path) -> Self {
        test_home::refuse_real_home(home);
        let base = home.join(".openagents");
        let registrations = if cfg!(target_os = "macos") {
            Some(home.join("Library/LaunchAgents"))
        } else if cfg!(target_os = "linux") {
            Some(home.join(".config/systemd/user"))
        } else {
            None
        };
        Self {
            access: base.join("coder-access"),
            owner_key: base.join("coder-owner/owner.key"),
            host_root: base.join("host"),
            tasks: base.join("tasks"),
            registrations,
        }
    }

    fn host_key(&self) -> PathBuf {
        self.access.join("host.key")
    }

    fn access_state(&self) -> PathBuf {
        self.access.join("access.json")
    }

    fn access_lock(&self) -> PathBuf {
        self.access.join("access.lock")
    }
}

/// Where the host key is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HostKey {
    /// In `host.key`, and it signs as the access store's host.
    File,
    /// `host.key` is gone: an earlier adoption moved it. [`adopt`] checks
    /// the keychain for it.
    Moved,
    /// `host.key` is not the access store's host key, or cannot be read.
    Invalid,
}

/// What the owner key file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OwnerKey {
    /// There is no owner key file.
    Absent,
    /// The file holds this host's owner key; adoption moves it.
    ThisHost,
    /// The file holds another owner's key; adoption leaves it.
    OtherOwner,
    /// The file is not a private 64-character hexadecimal key (an `nsec`
    /// file, for example); adoption leaves it.
    Unrecognized,
}

/// The old agent, as `service.json` describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OldAgent {
    /// The label, for example `org.openagents.coder-host`.
    pub label: String,
    /// The registration link the service manager reads.
    pub registration: PathBuf,
    /// Whether that link points at this host root's definition.
    pub registered: bool,
    /// Whether the canonical definition exists.
    pub definition: bool,
    /// Whether the configuration names the access store's host key.
    pub same_host: bool,
}

/// Another user agent or unit that runs `coder host serve` (or the old
/// `coder-service ... run` launcher) on this account: a hand-written unit, a
/// second label, anything but the desktop app's own agent and the agent
/// `service.json` names. Adoption stops and removes it, because two hosts
/// on one access store never both serve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Stray {
    /// The unit name or launchd label (the file name without extension).
    pub name: String,
    /// The definition or registration link.
    pub path: PathBuf,
}

/// What stays where it is. Adoption never writes any of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Kept {
    /// Grants in the access store, including revoked and expired ones.
    pub grants: usize,
    /// Grants neither revoked nor expired now.
    pub active_grants: usize,
    /// Devices with an advanced epoch.
    pub epochs: usize,
    /// Whether `serve.json` exists.
    pub serve_settings: bool,
    /// Whether `serve.json` sets tailnet admission.
    pub tailnet_admission: bool,
    /// Whether the auto-start policy exists.
    pub autostart: bool,
    /// Whether the task store exists.
    pub tasks: bool,
}

/// An old-style setup, read without changing anything. It holds no secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Detection {
    /// Where it was found.
    pub paths: Paths,
    /// The host's public key, from the access store.
    pub host: String,
    /// The owner's public key, from the access store.
    pub owner: String,
    /// Where the host key is.
    pub host_key: HostKey,
    /// What the owner key file is.
    pub owner_key: OwnerKey,
    /// The old agent, when `service.json` exists.
    pub agent: Option<OldAgent>,
    /// Other agents that run a Coder host on this account.
    pub strays: Vec<Stray>,
    /// What adoption keeps in place.
    pub kept: Kept,
    /// Why [`adopt`] would refuse. Empty when it can proceed.
    pub problems: Vec<String>,
}

impl Detection {
    /// Whether anything is left to move: a key file, an old agent, or a
    /// stray. A setup already adopted (its keys moved, its agent gone) has
    /// nothing, and the caller only registers its own agent.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.host_key == HostKey::File
            || self.owner_key == OwnerKey::ThisHost
            || self
                .agent
                .as_ref()
                .is_some_and(|agent| agent.registered || agent.definition)
            || !self.strays.is_empty()
    }
}

/// What [`adopt`] did. It holds no secret.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Adopted {
    /// The host's public key, unchanged.
    pub host: String,
    /// Whether the owner key moved into the keychain.
    pub owner_moved: bool,
    /// The old agent's uninstall, when there was one to remove.
    pub uninstalled: Option<UninstallReport>,
    /// The stray agents stopped and removed.
    pub strays_removed: Vec<String>,
    /// The key files deleted.
    pub removed: Vec<PathBuf>,
    /// What stayed in place.
    pub kept: Kept,
}

/// Reads an old-style setup under `paths`. `None` when there is no access
/// store; otherwise [`Detection::problems`] lists what [`adopt`] would
/// refuse. `now` is Unix seconds, for counting active grants.
pub fn detect(paths: &Paths, now: u64) -> Result<Option<Detection>> {
    let Some(book) = read_access(&paths.access_state())? else {
        return Ok(None);
    };
    let mut problems = Vec::new();
    let text = |key: &str| {
        book.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let (host, owner) = (text("host"), text("owner"));
    if !is_key(&host) || !is_key(&owner) {
        return Err(Error::refused(
            "the access store does not name a host and an owner",
        ));
    }

    let host_key = match read_secret_file(&paths.host_key(), Format::Raw) {
        Ok(Some(secret)) if public(&secret) == host => HostKey::File,
        Ok(None) => HostKey::Moved,
        Ok(Some(_)) | Err(_) => HostKey::Invalid,
    };
    if host_key == HostKey::Invalid {
        problems.push(format!(
            "{} is not the access store's host key; nothing can be adopted",
            paths.host_key().display()
        ));
    }

    let owner_key = match read_secret_file(&paths.owner_key, Format::Hex) {
        Ok(None) => OwnerKey::Absent,
        Ok(Some(secret)) if public(&secret) == owner => OwnerKey::ThisHost,
        Ok(Some(_)) => OwnerKey::OtherOwner,
        Err(_) => OwnerKey::Unrecognized,
    };

    let agent = old_agent(paths, &host, &mut problems)?;
    let strays = strays(paths, agent.as_ref());

    let grants = book.get("grants").and_then(serde_json::Value::as_object);
    let active_grants = grants.map_or(0, |grants| {
        grants
            .values()
            .filter(|record| {
                record
                    .get("revoked_at")
                    .is_none_or(serde_json::Value::is_null)
                    && record
                        .pointer("/grant/expires_at")
                        .and_then(serde_json::Value::as_u64)
                        .is_some_and(|expires| expires > now)
            })
            .count()
    });
    let serve = read_json(&paths.host_root.join("serve.json"))?;
    let kept = Kept {
        grants: grants.map_or(0, serde_json::Map::len),
        active_grants,
        epochs: book
            .get("epochs")
            .and_then(serde_json::Value::as_object)
            .map_or(0, serde_json::Map::len),
        serve_settings: serve.is_some(),
        tailnet_admission: serve
            .as_ref()
            .and_then(|serve| serve.get("tailnet_admission"))
            .is_some_and(|value| !value.is_null()),
        autostart: fs::symlink_metadata(paths.host_root.join("autostart.json")).is_ok(),
        tasks: paths.tasks.is_dir(),
    };
    Ok(Some(Detection {
        paths: paths.clone(),
        host,
        owner,
        host_key,
        owner_key,
        agent,
        strays,
        kept,
        problems,
    }))
}

/// The old agent `service.json` describes, adding what is wrong with it
/// to `problems`.
#[cfg(unix)]
fn old_agent(paths: &Paths, host: &str, problems: &mut Vec<String>) -> Result<Option<OldAgent>> {
    let layout = Layout::new(&paths.host_root);
    Ok(match fsx::read_optional(&layout.config())? {
        None => None,
        Some(_) => match Config::load(&layout) {
            Ok(config) => {
                let canonical = service::canonical_path(&layout, &config);
                let registration = service::registration_path(&config);
                let registered =
                    fs::read_link(&registration).ok().as_deref() == Some(canonical.as_path());
                if config.host_key != host {
                    problems.push(format!(
                        "the service {} serves host {}, not this access store's host {host}",
                        config.label, config.host_key
                    ));
                }
                Some(OldAgent {
                    label: config.label.clone(),
                    registration,
                    registered,
                    definition: fs::symlink_metadata(&canonical).is_ok(),
                    same_host: config.host_key == host,
                })
            }
            Err(error) => {
                problems.push(format!("the service configuration cannot be read: {error}"));
                None
            }
        },
    })
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn old_agent(_: &Paths, _: &str, _: &mut Vec<String>) -> Result<Option<OldAgent>> {
    Ok(None)
}

/// Adopts the old-style setup under `paths`: see the module documentation.
/// `register` registers the caller's own agent on the same state; it runs
/// last, after the old agent is gone and the key files are deleted.
///
/// # Errors
/// Refuses when there is nothing to adopt, when [`detect`] reports a
/// problem, when the keychain holds a different key or returns another
/// value than it was given (the files stay), when the old agent does not
/// stop, and when another process holds the access store.
pub fn adopt(
    paths: &Paths,
    now: u64,
    keychain: &mut dyn Keychain,
    runner: &mut dyn Runner,
    register: &mut dyn FnMut(&Detection) -> Result<()>,
) -> Result<Adopted> {
    let detection = detect(paths, now)?
        .ok_or_else(|| Error::refused("there is no Coder host on this computer to adopt"))?;
    if !detection.problems.is_empty() {
        return Err(Error::refused(detection.problems.join("; ")));
    }

    // 1. Keys into the keychain, each verified by reading it back.
    let host_value = match detection.host_key {
        HostKey::File => {
            let secret = read_secret_file(&paths.host_key(), Format::Raw)?
                .ok_or_else(|| Error::refused("the host key file disappeared"))?;
            let value = hex(&secret);
            import(keychain, HOST_KEY_ACCOUNT, &value, "host")?;
            Some(value)
        }
        HostKey::Moved => {
            let stored = keychain.read(HOST_KEY_ACCOUNT)?.ok_or_else(|| {
                Error::refused(
                    "the host key file is gone and the keychain does not hold it; \
                     this host cannot be adopted",
                )
            })?;
            if parse_hex(stored.trim())
                .map(|secret| public(&secret))
                .as_deref()
                != Some(detection.host.as_str())
            {
                return Err(Error::refused(
                    "the keychain's host key is not this access store's host",
                ));
            }
            None
        }
        HostKey::Invalid => unreachable!("detect reports an invalid host key as a problem"),
    };
    let owner_value = if detection.owner_key == OwnerKey::ThisHost {
        let secret = read_secret_file(&paths.owner_key, Format::Hex)?
            .ok_or_else(|| Error::refused("the owner key file disappeared"))?;
        let value = hex(&secret);
        import(keychain, OWNER_KEY_ACCOUNT, &value, "owner")?;
        Some(value)
    } else {
        None
    };

    // 2. The old agent goes, and must be seen to stop; so does every stray,
    // and the old agent's record is retired.
    let (uninstalled, strays_removed) = stop_agents(paths, &detection, runner)?;

    // 3. The files go, under the store's lock, each checked once more.
    let mut removed = Vec::new();
    {
        let _lock = lock(&paths.access_lock())?;
        if let Some(value) = &host_value {
            remove_verified(
                &paths.host_key(),
                Format::Raw,
                value,
                keychain,
                HOST_KEY_ACCOUNT,
            )?;
            removed.push(paths.host_key());
        }
        if let Some(value) = &owner_value {
            remove_verified(
                &paths.owner_key,
                Format::Hex,
                value,
                keychain,
                OWNER_KEY_ACCOUNT,
            )?;
            removed.push(paths.owner_key.clone());
        }
    }

    // 4. The caller's agent, on the same state.
    register(&detection)?;
    Ok(Adopted {
        host: detection.host.clone(),
        owner_moved: owner_value.is_some(),
        uninstalled,
        strays_removed,
        removed,
        kept: detection.kept,
    })
}

/// Stops the old agent and every stray, and retires the old agent's
/// record. Refuses, leaving the rest in place, when one does not stop.
#[cfg(unix)]
fn stop_agents(
    paths: &Paths,
    detection: &Detection,
    runner: &mut dyn Runner,
) -> Result<(Option<UninstallReport>, Vec<String>)> {
    let layout = Layout::new(&paths.host_root);
    let uninstalled = match &detection.agent {
        Some(agent) if agent.registered || agent.definition => {
            let config = Config::load(&layout)?;
            let report = service::uninstall(&layout, &config, runner)?;
            if !report.stopped {
                return Err(Error::refused(format!(
                    "the old agent {} did not stop; the keys are in the keychain and \
                     the files are kept, so try again",
                    agent.label
                )));
            }
            Some(report)
        }
        _ => None,
    };
    let mut removed = Vec::new();
    for stray in &detection.strays {
        remove_stray(stray, runner)?;
        removed.push(stray.name.clone());
    }
    // The record stays, renamed: an installer or updater that finds
    // `service.json` would bring the old agent back.
    if detection.agent.is_some() {
        let record = layout.config();
        let retired = paths.host_root.join(RETIRED_RECORD);
        if fs::symlink_metadata(&record).is_ok() {
            fs::rename(&record, &retired)?;
            fsx::sync_dir(&paths.host_root)?;
        }
    }
    Ok((uninstalled, removed))
}

#[cfg(not(unix))]
#[allow(clippy::unnecessary_wraps)]
fn stop_agents(
    _: &Paths,
    _: &Detection,
    _: &mut dyn Runner,
) -> Result<(Option<UninstallReport>, Vec<String>)> {
    Ok((None, Vec::new()))
}

#[cfg(unix)]
/// The most bytes of a unit or property list read when looking for strays.
const DEFINITION_MAX: u64 = 256 * 1024;

/// Every other agent in [`Paths::registrations`] that runs a Coder host.
#[cfg(unix)]
fn strays(paths: &Paths, agent: Option<&OldAgent>) -> Vec<Stray> {
    use std::io::Read as _;
    let Some(dir) = &paths.registrations else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let (suffix, words): (&str, fn(&str) -> Vec<String>) = if cfg!(target_os = "macos") {
        (".plist", plist_words)
    } else {
        (".service", unit_words)
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(suffix))
        else {
            continue;
        };
        if name.starts_with('.')
            || name == APP_AGENT
            || name == DEV_AGENT
            || agent.is_some_and(|agent| agent.label == name)
        {
            continue;
        }
        // A unit is often a link to its definition; follow it to read.
        let mut text = String::new();
        let read = fs::File::open(&path)
            .and_then(|file| file.take(DEFINITION_MAX).read_to_string(&mut text));
        if read.is_ok() && runs_a_host(&words(&text)) {
            found.push(Stray {
                name: name.to_owned(),
                path,
            });
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

#[cfg(not(unix))]
fn strays(_: &Paths, _: Option<&OldAgent>) -> Vec<Stray> {
    Vec::new()
}

#[cfg(unix)]
/// Whether a command line runs a Coder host: `coder host serve ...`, or the
/// old launcher `coder-service ... run`. `coder earn serve` and every other
/// program are not a host.
fn runs_a_host(words: &[String]) -> bool {
    words.iter().enumerate().any(|(index, word)| {
        let rest = &words[index + 1..];
        match Path::new(word).file_name().and_then(|name| name.to_str()) {
            Some("coder") => rest.len() >= 2 && rest[0] == "host" && rest[1] == "serve",
            Some("coder-service") => rest.iter().any(|word| word == "run"),
            _ => false,
        }
    })
}

#[cfg(unix)]
/// The words of a systemd unit's `ExecStart=` lines, unquoted.
fn unit_words(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("ExecStart="))
        .flat_map(|command| {
            command
                .trim_start_matches(['@', '-', ':', '+', '!'])
                .split_whitespace()
                .map(|word| word.trim_matches(['"', '\'']).to_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(unix)]
/// The `<string>` values of a launchd property list, in order.
fn plist_words(text: &str) -> Vec<String> {
    text.split("<string>")
        .skip(1)
        .filter_map(|part| {
            part.split_once("</string>")
                .map(|(value, _)| value.trim().to_owned())
        })
        .collect()
}

/// Stops a stray agent, confirms it stopped, and removes its definition.
#[cfg(unix)]
fn remove_stray(stray: &Stray, runner: &mut dyn Runner) -> Result<()> {
    let strings = |values: &[&str]| values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
    let stopped = if cfg!(target_os = "macos") {
        // SAFETY: `getuid` takes no arguments and cannot fail.
        let target = format!("gui/{}/{}", unsafe { libc::getuid() }, stray.name);
        let print = strings(&["print", &target]);
        if runner.run("launchctl", &print)?.success() {
            let _ = runner.run("launchctl", &strings(&["bootout", &target]))?;
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if !runner.run("launchctl", &print)?.success() {
                break true;
            }
            if std::time::Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    } else {
        let unit = format!("{}.service", stray.name);
        let _ = runner.run(
            "systemctl",
            &strings(&["--user", "disable", "--now", &unit]),
        )?;
        let show = runner.run(
            "systemctl",
            &strings(&["--user", "show", &unit, "--property=ActiveState"]),
        )?;
        service::parse_properties(&show.stdout)
            .get("ActiveState")
            .is_none_or(|state| state == "inactive" || state == "failed")
    };
    if !stopped {
        return Err(Error::refused(format!(
            "the agent {} did not stop; it is left in place",
            stray.name
        )));
    }
    fsx::remove_file_if_present(&stray.path)?;
    if !cfg!(target_os = "macos") {
        let _ = runner.run("systemctl", &strings(&["--user", "daemon-reload"]))?;
    }
    Ok(())
}

/// Stores `value` unless the item already holds it, then reads it back.
fn import(keychain: &mut dyn Keychain, account: &str, value: &str, what: &str) -> Result<()> {
    match keychain.read(account)? {
        Some(stored) if stored.trim() == value => {}
        Some(_) => {
            return Err(Error::refused(format!(
                "the keychain already holds a different {what} key; \
                 remove it first or keep the existing setup"
            )));
        }
        None => keychain.write(account, value)?,
    }
    match keychain.read(account)? {
        Some(stored) if stored.trim() == value => Ok(()),
        _ => Err(Error::refused(format!(
            "the keychain did not return the {what} key it was given; \
             the key files are left in place"
        ))),
    }
}

/// Deletes a key file only when it still holds `value` and the keychain
/// still returns it.
fn remove_verified(
    path: &Path,
    format: Format,
    value: &str,
    keychain: &mut dyn Keychain,
    account: &str,
) -> Result<()> {
    let current = read_secret_file(path, format)?.map(|secret| hex(&secret));
    if current.as_deref() != Some(value)
        || keychain.read(account)?.as_deref().map(str::trim) != Some(value)
    {
        return Err(Error::refused(format!(
            "{} changed during adoption; it is left in place",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    if let Some(parent) = path.parent() {
        fsx::sync_dir(parent)?;
    }
    Ok(())
}

/// Takes the access store's exclusive lock, which a serving host holds.
fn lock(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    file.try_lock().map_err(|_| {
        Error::refused("a Coder host still holds the access store; stop it and try again")
    })?;
    Ok(file)
}

#[derive(Clone, Copy)]
enum Format {
    /// 32 raw bytes, as the access store writes `host.key`.
    Raw,
    /// 64 hexadecimal characters and optional whitespace, as the old setup
    /// command wrote `owner.key`.
    Hex,
}

/// Reads a private key file. `None` when it does not exist. Refuses a file
/// that is not a private ordinary file of this user's, without echoing it.
fn read_secret_file(path: &Path, format: Format) -> Result<Option<SecretKey>> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !meta.file_type().is_file() || !private_to_this_user(&meta) {
        return Err(Error::refused(format!(
            "{} is not a private key file of this user's",
            path.display()
        )));
    }
    let bytes = fsx::read_bounded(path, KEY_FILE_MAX)?;
    let secret = match format {
        Format::Raw => <[u8; 32]>::try_from(bytes.as_slice())
            .ok()
            .and_then(|raw| SecretKey::from_byte_array(raw).ok()),
        Format::Hex => std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| parse_hex(text.trim())),
    };
    secret
        .map(Some)
        .ok_or_else(|| Error::refused(format!("{} is not a key", path.display())))
}

/// Whether a file is this user's and nobody else's: owner and mode on
/// Unix. On Windows the per-user profile already admits only the user.
#[cfg(unix)]
fn private_to_this_user(meta: &fs::Metadata) -> bool {
    // SAFETY: `getuid` takes no arguments and cannot fail.
    let uid = unsafe { libc::getuid() };
    meta.uid() == uid && meta.mode() & 0o077 == 0
}

#[cfg(not(unix))]
fn private_to_this_user(_: &fs::Metadata) -> bool {
    true
}

fn parse_hex(text: &str) -> Option<SecretKey> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut raw = [0_u8; 32];
    for (index, byte) in raw.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    SecretKey::from_byte_array(raw).ok()
}

fn hex(secret: &SecretKey) -> String {
    secret.display_secret().to_string()
}

/// The x-only public key, in lowercase hexadecimal.
fn public(secret: &SecretKey) -> String {
    Keypair::from_secret_key(&Secp256k1::signing_only(), secret)
        .x_only_public_key()
        .0
        .to_string()
}

fn is_key(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn read_access(path: &Path) -> Result<Option<serde_json::Value>> {
    match fsx::read_bounded(path, ACCESS_MAX) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_json(path: &Path) -> Result<Option<serde_json::Value>> {
    fsx::read_optional(path)?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(Error::from))
        .transpose()
}

/// A guard for tests: a test that reaches the real home's state, instead of
/// a temporary one, fails at once rather than writing into it. The real
/// home is the password database's, so a test that set `HOME` to a
/// temporary directory passes.
pub mod test_home {
    use std::path::Path;

    /// Panics, in this crate's tests, when `home` is the real user's home.
    /// Does nothing outside tests.
    pub fn refuse_real_home(home: &Path) {
        let _ = home;
        #[cfg(all(test, unix))]
        if let Some(real) = real_home() {
            assert!(
                home.canonicalize().ok().as_deref() != Some(real.as_path()),
                "a test reached the real home {}; give it a temporary one",
                real.display()
            );
        }
    }

    /// The home directory the password database names for this user.
    #[cfg(unix)]
    #[must_use]
    pub fn real_home() -> Option<std::path::PathBuf> {
        // SAFETY: `getpwuid` returns a pointer into static storage or null;
        // the directory is copied out before any other call.
        unsafe {
            let entry = libc::getpwuid(libc::getuid());
            if entry.is_null() || (*entry).pw_dir.is_null() {
                return None;
            }
            let dir = std::ffi::CStr::from_ptr((*entry).pw_dir);
            let path = std::path::PathBuf::from(
                <std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(dir.to_bytes()),
            );
            path.canonicalize().ok()
        }
    }
}

#[cfg(all(test, unix))]
mod tests;

/// Windows had no launchd agent or systemd unit, only the key files a
/// `coder host init` wrote: adoption moves them and keeps the store.
#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[derive(Default)]
    struct Memory(std::collections::BTreeMap<String, String>);

    impl Keychain for Memory {
        fn read(&mut self, account: &str) -> Result<Option<String>> {
            Ok(self.0.get(account).cloned())
        }
        fn write(&mut self, account: &str, value: &str) -> Result<()> {
            self.0.insert(account.into(), value.into());
            Ok(())
        }
    }

    #[test]
    fn a_windows_setup_moves_its_host_key_and_keeps_the_store() {
        let home = tempfile::tempdir().unwrap();
        let paths = Paths::under(home.path());
        let owner = SecretKey::new(&mut secp256k1::rand::rng());
        let store =
            coder_access::host::Host::new(&paths.access, coder_access::RelayPolicy::LoopbackTest);
        let host = store.init(&coder_access::protocol::pubkey(&owner)).unwrap();
        let before = fs::read(paths.access.join("access.json")).unwrap();
        let detection = detect(&paths, 0).unwrap().unwrap();
        assert!(detection.pending() && detection.agent.is_none());
        let mut keychain = Memory::default();
        let adopted = adopt(&paths, 0, &mut keychain, &mut (), &mut |_| Ok(())).unwrap();
        assert_eq!(adopted.host, host);
        assert!(!paths.access.join("host.key").exists());
        assert!(keychain.0.contains_key(HOST_KEY_ACCOUNT));
        assert_eq!(fs::read(paths.access.join("access.json")).unwrap(), before);
        assert!(!detect(&paths, 0).unwrap().unwrap().pending());
    }
}
