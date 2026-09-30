//! The local control socket: the owner's actions on this computer.
//!
//! The desktop app and `openagents connect` speak the local control
//! protocol (`openagents_connect::control`) to a running host over a Unix
//! socket. The socket is `0600` in a `0700` directory, and the host serves a
//! peer only when the kernel reports the peer's user ID equal to its own
//! (see `socket`). On Windows the same protocol runs over this user's named
//! pipe, whose DACL admits only the user and whose server checks every
//! client's token user (see [`windows`]). A caller that passes is the local operator, which
//! NIP-HOST treats as the owner acting with a command on the host: it can
//! create and cancel invitations (connect codes), list and revoke devices,
//! get and set the auto-start policy and the projects, and read status. No
//! device, grant, relay message, or direct channel reaches this socket.
//!
//! Every action goes through the same grant store as a device's request,
//! under this process's store lock. A revocation wakes every open channel,
//! which rechecks and closes before it serves the device's next message.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use coder_access::{Right, Rights};
use openagents_connect::control::{
    self, Autostart, Device, Op, Project, Reply, Request, Response, Status,
};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Semaphore;

use crate::config::Control;
use crate::serve::Shared;
use crate::settings::ServeSettings;
use crate::{Error, Result};

#[cfg(unix)]
pub mod socket;
pub mod windows;

#[cfg(unix)]
pub use socket::{Bound, own_uid};

/// Connections served at once; more wait for a slot.
const CONNECTIONS: usize = 16;
/// The lifetime of a connect code's grant: the most NIP-HOST allows.
pub const CONNECT_GRANT_SECS: u64 = coder_access::protocol::MAX_GRANT_LIFETIME;

/// The default socket path for this platform, or `None` where the
/// variable it needs is unset.
#[cfg(unix)]
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    control::socket_path()
}

/// This user's control pipe name, or `None` when the process token cannot
/// be read.
#[cfg(windows)]
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    windows::current_user_sid()
        .ok()
        .and_then(|sid| windows::pipe_name(&sid))
        .map(PathBuf::from)
}

/// Bind the control socket the configuration names.
///
/// # Errors
/// As `socket::bind`.
#[cfg(unix)]
pub async fn bind(config: &Control) -> Result<Bound> {
    socket::bind(&config.path, config.uid).await
}

/// Serve the bound socket until the host stops.
#[cfg(unix)]
pub(crate) async fn serve(shared: Arc<Shared>, bound: Bound) {
    let slots = Arc::new(Semaphore::new(CONNECTIONS));
    loop {
        let Ok((stream, _)) = bound.listener.accept().await else {
            continue;
        };
        // The kernel's word for who is calling, before a byte is read. Any
        // other user, or a peer whose ID cannot be read, is closed at once.
        if !socket::admits(bound.uid, socket::peer_uid(&stream)) {
            drop(stream);
            continue;
        }
        let Ok(slot) = slots.clone().acquire_owned().await else {
            return;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            connection(shared, stream).await;
            drop(slot);
        });
    }
}

/// The bound control pipe on Windows.
#[cfg(windows)]
pub struct Bound {
    pipe: windows::ControlPipe,
    path: PathBuf,
}

#[cfg(windows)]
impl std::fmt::Debug for Bound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bound").field("path", &self.path).finish()
    }
}

#[cfg(windows)]
impl Bound {
    /// The pipe's name.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Windows admits a control client by its token's user SID, not a user ID,
/// so the configuration's user ID is not read there; this is zero.
#[cfg(windows)]
#[must_use]
pub fn own_uid() -> u32 {
    0
}

/// How long a bind waits for a pipe that another host still holds.
#[cfg(windows)]
const BIND_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Bind the control pipe the configuration names: this user's
/// (`\\.\pipe\openagents-control-<SID>`), or another local pipe name a
/// test chooses. Either way its DACL admits only this user and it is the
/// name's first instance, so a pipe another process already holds refuses
/// the bind.
///
/// # Errors
/// Refuses a name that is not a local pipe name, or a pipe already held.
#[cfg(windows)]
pub async fn bind(config: &Control) -> Result<Bound> {
    let name = config.path.to_string_lossy().into_owned();
    // A host that restarts itself starts its successor before it exits, so
    // the name can stay held for a moment; a host that keeps running holds
    // it past this wait.
    let deadline = std::time::Instant::now() + BIND_WAIT;
    let pipe = loop {
        match windows::ControlPipe::bind_at(&name) {
            Ok(pipe) => break pipe,
            Err(_) if std::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Err(error) => {
                return Err(Error::Config(format!(
                    "the control pipe {name} cannot be created ({error}); is another host running?"
                )));
            }
        }
    };
    Ok(Bound {
        path: config.path.clone(),
        pipe,
    })
}

/// Serve the bound pipe until the host stops. A client that is not this
/// user is disconnected before a byte is read.
#[cfg(windows)]
pub(crate) async fn serve(shared: Arc<Shared>, bound: Bound) {
    let Bound { mut pipe, .. } = bound;
    let slots = Arc::new(Semaphore::new(CONNECTIONS));
    loop {
        let stream = match pipe
            .accept(|refusal| eprintln!("coder host: refused a control client: {refusal}"))
            .await
        {
            Ok(stream) => stream,
            Err(error) => {
                // A pipe that cannot make its next instance cannot serve
                // anyone; wait rather than spin.
                eprintln!("coder host: the control pipe failed: {error}");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
        };
        let Ok(slot) = slots.clone().acquire_owned().await else {
            return;
        };
        let shared = shared.clone();
        tokio::spawn(async move {
            connection(shared, stream).await;
            drop(slot);
        });
    }
}

/// Answer requests on one connection, in order, until the client closes.
async fn connection<S: AsyncRead + AsyncWrite + Unpin>(shared: Arc<Shared>, mut stream: S) {
    loop {
        let request = match control::next_request(&mut stream).await {
            Ok(Some(request)) => request,
            Ok(None) => return,
            Err(error) => {
                // A malformed message ends the connection after saying why.
                let refused = Response::new(0, refused(error.code.as_str(), "malformed request"));
                let _ = control::respond(&mut stream, &refused).await;
                return;
            }
        };
        let response = handle(&shared, request).await;
        if control::respond(&mut stream, &response).await.is_err() {
            return;
        }
    }
}

async fn handle(shared: &Arc<Shared>, request: Request) -> Response {
    let Request { id, op, .. } = request;
    let worker = shared.clone();
    let revoking = matches!(op, Op::DeviceRevoke { .. });
    let reply = tokio::task::spawn_blocking(move || answer(&worker, op))
        .await
        .unwrap_or_else(|_| refused("unavailable", "the host could not answer"));
    if revoking && matches!(reply, Reply::Revoked { .. }) {
        shared.grants_changed.notify_waiters();
    }
    Response::new(id, reply)
}

fn prompt(pending: crate::serve::nearby::Pending) -> control::NearbyPrompt {
    control::NearbyPrompt {
        id: pending.id,
        label: pending.label,
        code: pending.code.digits(),
    }
}

fn refused(code: &str, message: impl Into<String>) -> Reply {
    Reply::Refused {
        code: code.into(),
        message: message.into(),
    }
}

fn boxed(code: &str, message: impl Into<String>) -> Box<Reply> {
    Box::new(refused(code, message))
}

fn access_refused(error: &coder_access::Error) -> Reply {
    let code = serde_json::to_value(error.code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unavailable".into());
    refused(&code, error.message.clone())
}

fn host_refused(error: &Error) -> Reply {
    refused("unavailable", error.to_string())
}

/// Answer one operation. Runs on a blocking thread: store actions take the
/// store's lock and may wait briefly for another local process.
fn answer(shared: &Shared, op: Op) -> Reply {
    match op {
        Op::Status {} => status(shared),
        Op::InviteCreate { terminal } => invite(shared, terminal),
        Op::InviteCancel { invitation } => {
            match shared
                .authority
                .local(|host, _| host.cancel_invitation(&invitation))
            {
                Ok(()) => Reply::Cancelled { count: 1 },
                Err(error) => access_refused(&error),
            }
        }
        Op::InviteCancelAll {} => match shared
            .authority
            .local(|host, now| host.cancel_outstanding(now))
        {
            Ok(count) => Reply::Cancelled {
                count: u32::try_from(count).unwrap_or(u32::MAX),
            },
            Err(error) => access_refused(&error),
        },
        Op::DeviceList {} => match shared.authority.local(|host, now| host.devices(now)) {
            Ok(entries) => Reply::Devices {
                devices: devices(entries),
            },
            Err(error) => access_refused(&error),
        },
        Op::DeviceRevoke { device } => {
            if coder_reach::parse_pubkey(&device).is_err() {
                return refused("malformed", "a device is 64 lowercase hex characters");
            }
            match shared.authority.revoke(&device) {
                Ok((epoch, _)) => Reply::Revoked { device, epoch },
                Err(error) => access_refused(&error),
            }
        }
        Op::AutostartGet {} => match autostart_get(shared) {
            Ok(policy) => Reply::Autostart { policy },
            Err(reply) => *reply,
        },
        Op::AutostartSet { policy } => match autostart_set(shared, &policy) {
            Ok(policy) => Reply::Autostart { policy },
            Err(reply) => *reply,
        },
        Op::ProjectList {} => match root(shared).and_then(|root| projects(&root)) {
            Ok(projects) => Reply::Projects { projects },
            Err(error) => host_refused(&error),
        },
        Op::ProjectAdd { path } => match root(shared) {
            Ok(host_root) => {
                change_projects(shared, |settings| add_project(settings, &host_root, &path))
            }
            Err(error) => host_refused(&error),
        },
        Op::NearbyPending {} => match shared.iroh.get() {
            Some(iroh) => Reply::Nearby {
                pending: iroh.nearby.pending().map(prompt),
            },
            None => refused("unavailable", "this host does not serve nearby pairing"),
        },
        Op::NearbyDecide {
            id,
            connect,
            terminal,
        } => {
            let Some(iroh) = shared.iroh.get() else {
                return refused("unavailable", "this host does not serve nearby pairing");
            };
            let choice = if connect {
                crate::serve::nearby::Choice::Connect { terminal }
            } else {
                crate::serve::nearby::Choice::Decline
            };
            match iroh.nearby.decide(id, choice) {
                Ok(()) => Reply::Nearby {
                    pending: iroh.nearby.pending().map(prompt),
                },
                Err(_) => refused("not_pending", "no nearby request with that ID is waiting"),
            }
        }
        Op::OwnerImport { secret } => owner_import(shared, &secret),
        Op::ProjectRemove { label } => change_projects(shared, |settings| {
            settings
                .workspaces
                .remove(&label)
                .map(|_| ())
                .ok_or_else(|| Error::Config("no project has that label".into()))
        }),
    }
}

/// Make the imported key this host's owner. The book changes only while
/// no phone holds a current grant; the key goes to the host's key source,
/// when it has one, and the host starts again to publish under it.
fn owner_import(shared: &Shared, secret: &str) -> Reply {
    let Some(bytes) = crate::serve::keys::parse_hex(secret) else {
        return refused("malformed", "an owner key is 64 lowercase hex characters");
    };
    let Ok(key) = secp256k1::SecretKey::from_byte_array(bytes) else {
        return refused("malformed", "that is not an owner key");
    };
    let owner = coder_reach::pubkey(&key);
    if owner == shared.owner {
        return Reply::Owner { owner };
    }
    if let Err(error) = shared.authority.local(|host, now| host.reown(&owner, now)) {
        return access_refused(&error);
    }
    if let Some(keys) = &shared.config.keys {
        let secret = openagents_connect::keys::Secret::from_bytes(bytes);
        if keys
            .0
            .store(openagents_connect::keys::KeyName::Owner, &secret)
            .is_err()
        {
            return refused(
                "unavailable",
                "the owner changed, but its key could not be kept; import it again",
            );
        }
    }
    shared.restart.send_replace(true);
    Reply::Owner { owner }
}

fn status(shared: &Shared) -> Reply {
    let now = crate::unix_time().unwrap_or_default();
    let devices = shared.authority.active_devices(None, now).len();
    let outstanding = shared
        .authority
        .local(|host, now| host.outstanding_invitations(now))
        .map(|ids| ids.len())
        .unwrap_or_default();
    let iroh = shared.iroh.get();
    Reply::Status(Status {
        host: shared.host_key.clone(),
        endpoint: iroh
            .map(|iroh| hex(iroh.id().as_bytes()))
            .unwrap_or_default(),
        label: shared.config.label.clone(),
        online: iroh.is_some_and(crate::serve::iroh::Listener::online),
        relay: iroh.and_then(|iroh| iroh.relay().map(str::to_owned)),
        devices: u32::try_from(devices).unwrap_or(u32::MAX),
        outstanding_invitations: u32::try_from(outstanding).unwrap_or(u32::MAX),
        version: shared.config.ready.as_ref().map_or_else(
            || env!("CARGO_PKG_VERSION").to_owned(),
            |r| r.version.clone(),
        ),
    })
}

/// The rights a connect code carries: `observe` and `operate`, plus
/// `terminal` only when the person asked for it before the code was shown.
/// Never `review` or an access right.
#[must_use]
pub fn connect_rights(terminal: bool) -> Rights {
    let mut rights = vec![Right::Observe, Right::Operate];
    if terminal {
        rights.push(Right::Terminal);
    }
    Rights::new(rights).expect("a fixed, valid rights list")
}

/// Create an invitation and its `openagents-connect:` code. The
/// invitation is stored before the code is returned, so the code the
/// window shows is always one the host will honor.
fn invite(shared: &Shared, terminal: bool) -> Reply {
    let Some(iroh) = shared.iroh.get() else {
        return refused(
            "unavailable",
            "this host serves no iroh endpoint, so it cannot show a connect code",
        );
    };
    let Ok(relay) = shared.config.primary().map(str::to_owned) else {
        return refused("unavailable", "the host serves no relay");
    };
    let rights = connect_rights(terminal);
    let issued = match shared.authority.local(|host, now| {
        host.invite(
            &relay,
            rights.clone(),
            now,
            now.saturating_add(CONNECT_GRANT_SECS),
        )
    }) {
        Ok(issued) => issued,
        Err(error) => return access_refused(&error),
    };
    let parts = openagents_connect::code::CodeParts {
        host: shared.host_key.clone(),
        endpoint: iroh.id(),
        issued_at: issued.issued_at,
        relay: iroh.relay().and_then(|relay| relay.parse().ok()),
        addrs: iroh.direct(),
        label: shared.config.label.clone(),
    };
    let code = match openagents_connect::code::ConnectCode::from_invitation(
        parts,
        &issued.id,
        &issued.capability,
    ) {
        Ok(code) => code,
        Err(error) => {
            // Never leave a redeemable invitation behind a code nobody saw.
            let _ = shared
                .authority
                .local(|host, _| host.cancel_invitation(&issued.id));
            return refused(error.code.as_str(), "the connect code cannot be built");
        }
    };
    Reply::Invite {
        invitation: issued.id,
        code: code.encode(),
        expires_at: issued.expires_at,
        rights: rights
            .iter()
            .map(|right| right.as_str().to_owned())
            .collect(),
    }
}

/// One row per device: its newest grant. A device whose newest grant
/// expired or fell to an old epoch is left out; a revoked one stays, so the
/// window can say it was removed.
fn devices(entries: Vec<coder_access::protocol::DeviceEntry>) -> Vec<Device> {
    use coder_access::protocol::DeviceState;
    let mut newest: std::collections::BTreeMap<String, coder_access::protocol::DeviceEntry> =
        std::collections::BTreeMap::new();
    let mut first: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for entry in entries {
        let enrolled = first.entry(entry.device.clone()).or_insert(entry.issued_at);
        *enrolled = (*enrolled).min(entry.issued_at);
        let replace = newest
            .get(&entry.device)
            .is_none_or(|held| (held.issued_at, &held.grant) < (entry.issued_at, &entry.grant));
        if replace {
            newest.insert(entry.device.clone(), entry);
        }
    }
    newest
        .into_values()
        .filter(|entry| entry.state != DeviceState::Expired)
        .map(|entry| Device {
            enrolled_at: first.get(&entry.device).copied().unwrap_or(entry.issued_at),
            device: entry.device,
            label: String::new(),
            rights: entry
                .rights
                .iter()
                .map(|right| right.as_str().to_owned())
                .collect(),
            grant: entry.grant,
            epoch: entry.epoch,
            last_seen: entry.last_seen,
            revoked: entry.state == DeviceState::Revoked,
        })
        .collect()
}

fn root(shared: &Shared) -> Result<PathBuf> {
    shared
        .config
        .control
        .as_ref()
        .map(|control| control.root.clone())
        .ok_or_else(|| Error::Config("the host serves no control socket".into()))
}

fn projects(root: &Path) -> Result<Vec<Project>> {
    let settings = ServeSettings::load(root)?;
    Ok(settings
        .workspaces
        .into_iter()
        .map(|(label, path)| Project {
            label,
            folder: picked_folder(root, &path).map(|folder| folder.display().to_string()),
            path: path.display().to_string(),
        })
        .collect())
}

/// The folder the person picked, for a project the host admitted as its
/// own worktree of it (`HOST_ROOT/projects/NAME-HASH`): the checkout whose
/// Git directory holds the worktree's record. `None` for any other path.
fn picked_folder(host_root: &Path, path: &Path) -> Option<PathBuf> {
    let projects = host_root.join("projects");
    let under = |root: &Path| path.parent() == Some(root);
    if !under(&projects) && !std::fs::canonicalize(&projects).is_ok_and(|p| under(&p)) {
        return None;
    }
    let link = std::fs::read_to_string(path.join(".git")).ok()?;
    let gitdir = path.join(link.strip_prefix("gitdir:")?.trim());
    let common = match std::fs::read_to_string(gitdir.join("commondir")) {
        Ok(common) => gitdir.join(common.trim()),
        Err(_) => gitdir.parent()?.parent()?.to_path_buf(),
    };
    let common = std::fs::canonicalize(common).ok()?;
    (common.file_name()? == ".git")
        .then(|| common.parent().map(Path::to_path_buf))
        .flatten()
}

/// Change the recorded projects, then ask the host to start again, since
/// the terminal host and the task owner read their workspaces at start.
fn change_projects(
    shared: &Shared,
    change: impl FnOnce(&mut ServeSettings) -> Result<()>,
) -> Reply {
    let result = root(shared).and_then(|root| {
        let mut settings = ServeSettings::load(&root)?;
        if settings.schema.is_empty() {
            settings = ServeSettings::new(shared.config.relays.clone(), settings.workspaces);
        }
        let before = settings.workspaces.clone();
        change(&mut settings)?;
        if settings.workspaces != before {
            settings.save(&root)?;
            shared.restart.send_replace(true);
        }
        projects(&root)
    });
    match result {
        Ok(projects) => Reply::Projects { projects },
        Err(error) => host_refused(&error),
    }
}

/// Where [`git`] looks, in order, before the bare name. A service manager
/// such as launchd may start the host with a short `PATH`.
const GIT_PATHS: [&str; 3] = [
    "/usr/bin/git",
    "/opt/homebrew/bin/git",
    "/run/current-system/sw/bin/git",
];

fn git() -> std::process::Command {
    let program = GIT_PATHS
        .iter()
        .find(|path| Path::new(path).exists())
        .copied()
        .unwrap_or("git");
    let mut command = std::process::Command::new(program);
    command.stdin(std::process::Stdio::null());
    command
}

/// Admit a Git checkout as a project, labelled by its directory name.
/// Adding one already admitted is a no-op.
///
/// Coder writes only in a worktree whose Git directory is outside it (the
/// auto-start policy refuses any other), so a folder that holds its own
/// Git directory, which is what a person picks, gets a detached worktree of
/// its current commit under the host root's `projects/`, and that is what
/// the host admits. The folder itself is never changed beyond Git's record
/// of the worktree. A folder that is already such a worktree is admitted
/// as it is.
fn add_project(settings: &mut ServeSettings, host_root: &Path, path: &str) -> Result<()> {
    let chosen = std::fs::canonicalize(path)
        .map_err(|_| Error::Config("that folder does not exist".into()))?;
    if !chosen.is_dir() || !chosen.join(".git").exists() {
        return Err(Error::Config("that folder is not a Git checkout".into()));
    }
    let base: String = chosen
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
        .chars()
        .filter(|c| !c.is_control() && *c != '/')
        .take(64)
        .collect();
    let base = if base.is_empty() || base.starts_with('.') {
        "project".into()
    } else {
        base
    };
    let root = if chosen.join(".git").is_dir() {
        host_worktree(host_root, &chosen, &base)?
    } else {
        chosen
    };
    if settings.workspaces.values().any(|held| *held == root) {
        return Ok(());
    }
    if settings.workspaces.len() >= 64 {
        return Err(Error::Config(
            "this computer admits at most 64 projects".into(),
        ));
    }
    let mut label = base.clone();
    let mut n = 2;
    while settings.workspaces.contains_key(&label) {
        label = format!("{base}-{n}");
        n += 1;
    }
    settings.workspaces.insert(label, root);
    Ok(())
}

/// The host's worktree of `checkout`: `HOST_ROOT/projects/NAME-HASH`, made
/// detached at the checkout's current commit the first time and reused
/// after, so picking the same folder again admits the same worktree.
fn host_worktree(host_root: &Path, checkout: &Path, base: &str) -> Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(checkout.as_os_str().as_encoded_bytes());
    let name = format!("{base}-{}", hex(&digest[..4]));
    let projects = host_root.join("projects");
    let target = projects.join(name);
    if !target.exists() {
        std::fs::create_dir_all(&projects)
            .map_err(|_| Error::Config("cannot make the host's projects folder".into()))?;
        let output = git()
            .arg("-C")
            .arg(checkout)
            .args(["worktree", "add", "--detach", "--quiet"])
            .arg(&target)
            .arg("HEAD")
            .output()
            .map_err(|_| Error::Config("cannot run git".into()))?;
        if !output.status.success() {
            let _ = std::fs::remove_dir(&target);
            return Err(Error::Config(
                "Git could not copy that folder for Coder; it needs at least one commit".into(),
            ));
        }
    }
    std::fs::canonicalize(&target)
        .map_err(|_| Error::Config("the host's worktree is missing".into()))
}

/// The fields of the host's auto-start policy file that the window edits.
#[derive(serde::Deserialize)]
struct PolicyView {
    enabled: bool,
    workspaces: Vec<String>,
    max_running: u32,
}

fn autostart_get(shared: &Shared) -> std::result::Result<Autostart, Box<Reply>> {
    let root = root(shared).map_err(|error| Box::new(host_refused(&error)))?;
    let path = root.join("autostart.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Autostart {
                enabled: false,
                projects: Vec::new(),
                max_running: 1,
            });
        }
        Err(_) => {
            return Err(boxed("unavailable", "the auto-start policy cannot be read"));
        }
    };
    let view: PolicyView = serde_json::from_slice(&bytes)
        .map_err(|_| boxed("malformed", "the auto-start policy is malformed"))?;
    Ok(Autostart {
        enabled: view.enabled,
        projects: view.workspaces,
        max_running: u8::try_from(view.max_running).unwrap_or(u8::MAX),
    })
}

/// The routes the switch admits, in preference order: Codex, then Claude
/// Code, with the models `coder host autostart` documents.
const ROUTES: [&str; 2] = ["codex:gpt-6-luna", "claude:claude-opus-5-5"];

/// Change the policy through the host's own `coder host autostart`
/// command, which checks every bound and records the change; a request on
/// this socket is a command on the host.
fn autostart_set(
    shared: &Shared,
    policy: &Autostart,
) -> std::result::Result<Autostart, Box<Reply>> {
    let control = shared
        .config
        .control
        .as_ref()
        .ok_or_else(|| boxed("unavailable", "the host serves no control socket"))?;
    let program = control.autostart.as_ref().ok_or_else(|| {
        boxed(
            "unavailable",
            "this host cannot change the auto-start policy",
        )
    })?;
    if !(1..=8).contains(&policy.max_running) {
        return Err(boxed("bounds", "max_running is 1 to 8"));
    }
    let mut command = std::process::Command::new(program);
    command.args(["host", "autostart"]);
    if policy.enabled {
        if policy.projects.is_empty() {
            return Err(boxed("malformed", "turning auto-start on needs a project"));
        }
        command.arg("on");
        for project in &policy.projects {
            command.args(["--workspace", project]);
        }
        command.args(["--max-running", &policy.max_running.to_string()]);
        // Both local coding agents, in this order: each start takes the
        // first one signed in on this computer with capacity, so a Mac with
        // only Claude Code, or a Codex account at its limit, still runs.
        for route in ROUTES {
            command.args(["--route", route]);
        }
    } else {
        command.arg("off");
    }
    command.arg("--root").arg(&control.root);
    let output = command
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| boxed("unavailable", "the auto-start command did not run"))?;
    if !output.status.success() {
        let why = String::from_utf8_lossy(&output.stderr);
        let why = why.trim().trim_start_matches("coder host autostart: ");
        return Err(boxed(
            "forbidden",
            why.lines()
                .next()
                .unwrap_or("the auto-start command refused"),
        ));
    }
    autostart_get(shared)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
