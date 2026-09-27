//! SSH setup: install or reuse the release, start or adopt the host, and
//! redeem its invitation, on a thread of its own. Prompts from `ssh` wait
//! for an answer through the snapshot.
use super::{Code, Error, Handle, Result, Shared, SshAttempt, lock};
use crate::model::SshStage;
use coder_ssh::{Launcher, Prompter, Secret};
use std::sync::{Arc, Weak, mpsc};
use std::time::Duration;

/// How long a prompt waits for the person before `ssh` is refused.
const PROMPT_WAIT: Duration = Duration::from_secs(300);
/// The longest prompt text shown.
const PROMPT_MAX: usize = 200;
/// The longest label an SSH destination gives a new host.
const LABEL_MAX: usize = 64;

pub(super) fn start(shared: &Arc<Shared>, runtime: &Handle, destination: &str) -> Result<()> {
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
    let launcher = launcher.prompter(Arc::new(Prompts(Arc::downgrade(shared))));
    {
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
            stage: SshStage::Starting,
        });
    }
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
    let host = launcher.up().map_err(|error| reason(&error))?;
    let invitation = launcher.invite(&host).map_err(|error| reason(&error))?;
    stage_to(shared, SshStage::Enrolling);
    let label: String = destination.chars().take(LABEL_MAX).collect();
    shared
        .redeem(
            runtime,
            invitation.expose(),
            Some(label),
            Some(destination.to_owned()),
        )
        .map_err(|error| crate::describe(&error))
}

fn stage_to(shared: &Shared, stage: SshStage) {
    if let Some(attempt) = lock(&shared.state).ssh.as_mut() {
        attempt.stage = stage;
    }
}

pub(super) fn answer(shared: &Shared, id: u64, answer: Option<&str>) -> Result<()> {
    let mut state = lock(&shared.state);
    match state.prompt.take() {
        Some((waiting, sender)) if waiting == id => {
            let _ = sender.send(answer.map(|text| Secret::new(text.as_bytes().to_vec())));
            if let Some(attempt) = state.ssh.as_mut()
                && matches!(attempt.stage, SshStage::Prompt { id: shown, .. } if shown == id)
            {
                attempt.stage = SshStage::Starting;
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
            state.prompt = Some((id, sender));
            let text: String = prompt
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .take(PROMPT_MAX)
                .collect();
            if let Some(attempt) = state.ssh.as_mut() {
                attempt.stage = SshStage::Prompt {
                    id,
                    text: text.trim().to_owned(),
                };
            }
            id
        };
        let answer = receiver.recv_timeout(PROMPT_WAIT).ok().flatten();
        if let Some(shared) = self.0.upgrade() {
            let mut state = lock(&shared.state);
            if state
                .prompt
                .as_ref()
                .is_some_and(|(waiting, _)| *waiting == id)
            {
                state.prompt = None;
            }
            if let Some(attempt) = state.ssh.as_mut()
                && matches!(attempt.stage, SshStage::Prompt { id: shown, .. } if shown == id)
            {
                attempt.stage = SshStage::Starting;
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
