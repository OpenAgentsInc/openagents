//! SSH setup: install or reuse the release, start or adopt the host, redeem
//! its invitation, and open a tunnel, on a thread of its own. Prompts from
//! `ssh` wait for an answer through the snapshot.
//!
//! The tunnel forwards a loopback port on this machine to the host's
//! loopback listener. The connector tries that port first, as a local route
//! only this process can use: it is same-machine evidence for that one
//! address under NIP-REACH, so it is never saved, published, or offered to
//! another device. When the tunnel's `ssh` process ends, the route is cleared
//! and the host is reached through its relay; nothing stops the host.
//!
//! Remove runs `coder-ssh`'s explicit remove, which stops only a host a setup
//! started and detaches from one that was already running, then forgets the
//! computer on this device.
use super::{Code, Error, Handle, Result, Shared, SshAttempt, State, lock};
use crate::model::{SshRemoval, SshStage};
use coder_host::client::Route;
use coder_link::{Phase, Signal};
use coder_ssh::{Launcher, Prompter, Removal, Secret, Tunnel};
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Weak, mpsc};
use std::time::{Duration, Instant};

/// How long a prompt waits for the person before `ssh` is refused.
const PROMPT_WAIT: Duration = Duration::from_secs(300);
/// The longest prompt text shown.
const PROMPT_MAX: usize = 200;
/// The longest label an SSH destination gives a new host.
const LABEL_MAX: usize = 64;
/// How long a new tunnel may take to open, prompts included.
const TUNNEL_WAIT: Duration = Duration::from_secs(120);
/// How often a relay connection is asked to move onto an open tunnel.
const TUNNEL_NUDGE: Duration = Duration::from_secs(5);

/// One host's tunnel. `tunnel` is `None` once the tunnel closed; the entry
/// stays so the screens can say the host is reached through its relay.
pub(super) struct TunnelLive {
    tunnel: Option<Tunnel>,
    address: SocketAddr,
    nudged: Option<Instant>,
}

/// Whether each tunnel is open, and its local address, by host key.
pub(super) fn tunnels(state: &State) -> BTreeMap<String, (bool, SocketAddr)> {
    state
        .tunnels
        .iter()
        .map(|(host, live)| (host.clone(), (live.tunnel.is_some(), live.address)))
        .collect()
}

/// Clear the routes of tunnels that closed, and move a relay connection onto
/// a tunnel that is open. The pump calls this on every turn.
pub(super) fn watch(shared: &Shared) {
    let mut registry = lock(&shared.registry);
    let mut state = lock(&shared.state);
    let State { hosts, tunnels, .. } = &mut *state;
    for (host, live) in tunnels.iter_mut() {
        let Some(key) = hosts.get(host).and_then(|host| host.key.clone()) else {
            continue;
        };
        let Some(tunnel) = live.tunnel.as_mut() else {
            continue;
        };
        if !tunnel.alive() {
            // Dropping the closed tunnel reaps its `ssh` process. The host
            // keeps running; the next attempt uses the relay.
            live.tunnel = None;
            let _ = registry.connector_mut().set_local_route(&key, None);
            let _ = registry.signal(&key, Signal::RetryNow);
            continue;
        }
        let on_relay = registry
            .status(&key)
            .filter(|status| status.phase == Phase::Connected)
            .and_then(|status| status.connection)
            .and_then(|connection| registry.connector().link(&key, connection))
            .is_some_and(|link| matches!(link.route(), Route::Relay(_)));
        if on_relay && live.nudged.is_none_or(|at| at.elapsed() >= TUNNEL_NUDGE) {
            // A probe of a relay connection fails while a direct route
            // answers, and the supervisor replaces it with that route.
            live.nudged = Some(Instant::now());
            let _ = registry.signal(&key, Signal::RetryNow);
        }
    }
}

/// End a host's tunnel and clear its route. The host keeps running.
pub(super) fn close_tunnel(shared: &Shared, host: &str) {
    let mut registry = lock(&shared.registry);
    let mut state = lock(&shared.state);
    let key = state.hosts.get(host).and_then(|live| live.key.clone());
    if let Some(key) = key {
        let _ = registry.connector_mut().set_local_route(&key, None);
    }
    state.tunnels.remove(host);
}

/// Open a tunnel to the host `up` reported and give its port to the
/// connector. A tunnel that doesn't open leaves the host on its relay.
fn open_tunnel(shared: &Shared, launcher: &Launcher, remote: &coder_ssh::Host, host: &str) {
    let opened = launcher.connect(remote).and_then(|mut tunnel| {
        tunnel.ready(TUNNEL_WAIT)?;
        Ok(tunnel)
    });
    let Ok(tunnel) = opened else {
        return;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, tunnel.local_port()));
    let mut registry = lock(&shared.registry);
    let mut state = lock(&shared.state);
    let Some(key) = state.hosts.get(host).and_then(|live| live.key.clone()) else {
        return;
    };
    if registry
        .connector_mut()
        .set_local_route(&key, Some(address))
        .is_err()
    {
        return;
    }
    let _ = registry.signal(&key, Signal::RetryNow);
    state.tunnels.insert(
        host.to_owned(),
        TunnelLive {
            tunnel: Some(tunnel),
            address,
            nudged: Some(Instant::now()),
        },
    );
}

/// The launcher for `destination` with this client's release, runner,
/// program, and prompts.
fn launcher(shared: &Arc<Shared>, destination: &str) -> Result<Launcher> {
    let setup = shared
        .settings
        .ssh
        .clone()
        .ok_or_else(|| Error::new(Code::Unavailable, "no host release to install"))?;
    if shared.settings.platform == crate::model::Platform::Phone {
        return Err(Error::new(Code::Unsupported, "phones never start SSH"));
    }
    let mut launcher = Launcher::new(destination, setup.release, setup.runner)
        .map_err(|_| Error::new(Code::Malformed, "not an SSH destination"))?;
    if let Some(program) = setup.program {
        launcher = launcher.program(program);
    }
    Ok(launcher.prompter(Arc::new(Prompts(Arc::downgrade(shared)))))
}

/// Begin an SSH attempt unless one runs.
fn begin(shared: &Shared, destination: &str, stage: SshStage) -> Result<()> {
    let mut state = lock(&shared.state);
    if state
        .ssh
        .as_ref()
        .is_some_and(|attempt| attempt.stage.running())
    {
        return Err(Error::new(Code::Conflict, "an SSH setup is running"));
    }
    state.ssh = Some(SshAttempt {
        destination: destination.to_owned(),
        stage,
    });
    Ok(())
}

/// Start `coder-ssh`'s explicit remove for a host this device set up over
/// SSH. When it finishes, forget the computer and report what happened.
pub(super) fn remove(shared: &Arc<Shared>, host: &str) -> Result<()> {
    let (destination, label) = {
        let state = lock(&shared.state);
        let saved = state
            .saved
            .hosts
            .iter()
            .find(|saved| saved.access.grant.host == host)
            .ok_or_else(|| Error::new(Code::Stale, "unknown computer"))?;
        let destination = saved
            .ssh
            .clone()
            .ok_or_else(|| Error::new(Code::Unsupported, "this computer wasn't set up over SSH"))?;
        let label = state
            .saved
            .owner
            .as_ref()
            .and_then(|owner| owner.directory.as_ref())
            .and_then(|directory| directory.entry(host))
            .map_or_else(|| saved.label.clone(), |entry| entry.label.clone());
        (destination, label)
    };
    let launcher = launcher(shared, &destination)?;
    begin(
        shared,
        &destination,
        SshStage::Removing {
            label: label.clone(),
        },
    )?;
    let worker = shared.clone();
    let target = host.to_owned();
    let spawned = std::thread::Builder::new()
        .name("coder-computers-ssh-remove".into())
        .spawn(move || {
            let stage = match launcher.remove() {
                Ok(removal) => {
                    let removal = match removal {
                        Removal::Stopped { .. } => SshRemoval::Stopped,
                        Removal::Detached { .. } => SshRemoval::Detached,
                        Removal::Absent => SshRemoval::Absent,
                    };
                    match worker.forget(&target) {
                        Ok(()) => SshStage::Removed { label, removal },
                        Err(_) => SshStage::RemoveFailed {
                            label,
                            reason: "the remote host was removed, and this device couldn't update its list.".into(),
                        },
                    }
                }
                Err(error) => SshStage::RemoveFailed {
                    label,
                    reason: reason(&error),
                },
            };
            stage_to(&worker, stage);
        });
    if spawned.is_err() {
        stage_to(
            shared,
            SshStage::Failed {
                reason: "this device couldn't start the remove.".into(),
            },
        );
        return Err(Error::new(Code::Unavailable, "cannot start the SSH remove"));
    }
    Ok(())
}

pub(super) fn start(shared: &Arc<Shared>, runtime: &Handle, destination: &str) -> Result<()> {
    let launcher = launcher(shared, destination)?;
    begin(shared, destination, SshStage::Starting)?;
    let worker = shared.clone();
    let runtime = runtime.clone();
    let target = destination.to_owned();
    let spawned = std::thread::Builder::new()
        .name("coder-computers-ssh".into())
        .spawn(move || {
            let stage = match run(&worker, &runtime, &launcher, &target) {
                Ok(host) => SshStage::Added { host },
                Err(reason) => SshStage::Failed { reason },
            };
            stage_to(&worker, stage);
        });
    if spawned.is_err() {
        stage_to(
            shared,
            SshStage::Failed {
                reason: "this device couldn't start the setup.".into(),
            },
        );
        return Err(Error::new(Code::Unavailable, "cannot start the SSH setup"));
    }
    Ok(())
}

fn run(
    shared: &Shared,
    runtime: &Handle,
    launcher: &Launcher,
    destination: &str,
) -> std::result::Result<String, String> {
    let remote = launcher.up().map_err(|error| reason(&error))?;
    let invitation = launcher.invite(&remote).map_err(|error| reason(&error))?;
    stage_to(shared, SshStage::Enrolling);
    let label: String = destination.chars().take(LABEL_MAX).collect();
    let host = shared
        .redeem(
            runtime,
            invitation.expose(),
            Some(label),
            Some(destination.to_owned()),
        )
        .map_err(|error| crate::describe(&error))?;
    // A new setup replaces an old tunnel to the same host.
    close_tunnel(shared, &host);
    open_tunnel(shared, launcher, &remote, &host);
    Ok(host)
}

fn stage_to(shared: &Shared, stage: SshStage) {
    if let Some(attempt) = lock(&shared.state).ssh.as_mut() {
        attempt.stage = stage;
    }
}

pub(super) fn answer(shared: &Shared, id: u64, answer: Option<&str>) -> Result<()> {
    let mut state = lock(&shared.state);
    match state.prompt.take() {
        Some((waiting, sender, resume)) if waiting == id => {
            let _ = sender.send(answer.map(|text| Secret::new(text.as_bytes().to_vec())));
            if let Some(attempt) = state.ssh.as_mut()
                && matches!(attempt.stage, SshStage::Prompt { id: shown, .. } if shown == id)
            {
                attempt.stage = resume;
            }
            Ok(())
        }
        other => {
            state.prompt = other;
            Err(Error::new(Code::Stale, "that prompt is no longer waiting"))
        }
    }
}

/// Asks the person through the snapshot, and waits for the answer.
struct Prompts(Weak<Shared>);

impl Prompter for Prompts {
    fn answer(&self, prompt: &str) -> Option<Secret> {
        let (sender, receiver) = mpsc::channel();
        let id = {
            let shared = self.0.upgrade()?;
            let mut state = lock(&shared.state);
            state.prompts += 1;
            let id = state.prompts;
            let text: String = prompt
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(PROMPT_MAX)
                .collect();
            // Resume the stage the prompt interrupted: a setup or a remove.
            let mut resume = SshStage::Starting;
            if let Some(attempt) = state.ssh.as_mut() {
                resume = match &attempt.stage {
                    SshStage::Prompt { .. } => SshStage::Starting,
                    stage => stage.clone(),
                };
                attempt.stage = SshStage::Prompt {
                    id,
                    text: text.trim().to_owned(),
                };
            }
            state.prompt = Some((id, sender, resume));
            id
        };
        let answer = receiver.recv_timeout(PROMPT_WAIT).ok().flatten();
        if let Some(shared) = self.0.upgrade() {
            let mut state = lock(&shared.state);
            let resume = match state.prompt.take() {
                Some((waiting, _, resume)) if waiting == id => Some(resume),
                other => {
                    state.prompt = other;
                    None
                }
            };
            if let (Some(resume), Some(attempt)) = (resume, state.ssh.as_mut())
                && matches!(attempt.stage, SshStage::Prompt { id: shown, .. } if shown == id)
            {
                attempt.stage = resume;
            }
        }
        answer
    }
}

/// User-facing copy for a failed setup step.
fn reason(error: &coder_ssh::Error) -> String {
    use coder_ssh::Error as E;
    match error {
        E::InvalidDestination(_) => "that isn't a destination ssh accepts.".into(),
        E::Ssh { .. } | E::Spawn(_) => {
            "ssh couldn't connect or sign in. Check the destination and your credentials.".into()
        }
        E::TimedOut(_) => "it took too long.".into(),
        E::Unsupported { os, arch } => format!("Coder has no build for {os} {arch}."),
        E::NoArtifact { os, arch } => {
            format!("this app has no Coder release for {os} {arch}.")
        }
        E::LocalChecksumMismatch(_) | E::ChecksumMismatch => {
            "the Coder release didn't match its checksum, so nothing was installed.".into()
        }
        E::BadArchive | E::BinaryRejected => {
            "the Coder release didn't run there, so nothing was installed.".into()
        }
        E::MissingTool(tool) => format!("the machine lacks {tool}."),
        E::Busy => "another setup is installing Coder there. Try again shortly.".into(),
        E::HostDidNotStart => "the host didn't start.".into(),
        E::NotInstalled | E::Invitation(_) => "the host didn't create an invitation.".into(),
        E::InvalidRelease(_) | E::InvalidRunner(_) => "this app's SSH settings are invalid.".into(),
        E::Remote(_) | E::Protocol(_) | E::Io(_) => "the setup failed.".into(),
    }
}
