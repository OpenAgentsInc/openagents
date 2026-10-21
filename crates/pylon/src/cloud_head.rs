//! Where a cloud group's head runs (#12399, W12), and the head that runs
//! inside the group's zone.
//!
//! W15's head ran on the budget host, so each token paid a round trip from
//! there to each stage. `pylon cloud up --head stage0|vm` moves the head
//! next to the stages: onto the machine that runs stage 0, or onto a small
//! machine of its own in the same zone. The budget host keeps the spend
//! guard: it creates and deletes every machine, and talks only to the
//! head, over one ssh session (`scripts/pylon/v41-head-ssh.sh`):
//!
//! - **In.** The first line is the launch: the head's version 6 settings
//!   and the group's own ssh key, which the head uses to start each stage
//!   on the other machines (`scripts/pylon/v41-stage-ssh.sh --ssh`). Later
//!   lines are commands; `down`, or the end of input, stops the head.
//! - **Out.** One JSON line per change: `{"event": "status", ...}` with the
//!   head's beacon address, readiness, and generation, and a last
//!   `{"event": "stopped", ...}` once the head drained and published its
//!   beacons offline.
//!
//! The ssh session is the head's lifetime: a lost head machine or budget
//! host ends it, the head stops, and the budget host deletes the machines.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use crate::identity::Identity;
use crate::lease::Dedicated;
use crate::now;
use crate::online::{Options, load_live};
use crate::share::{self, Settings};

/// Where the group head runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeadPlacement {
    /// On the budget host (W15).
    #[default]
    Local,
    /// On the machine that runs stage 0.
    Stage0,
    /// On a small machine of its own in the group's zone.
    Vm,
}

impl HeadPlacement {
    /// Read `local`, `stage0`, or `vm`.
    ///
    /// # Errors
    ///
    /// For any other word.
    pub fn parse(word: &str) -> Result<Self, String> {
        match word {
            "local" => Ok(Self::Local),
            "stage0" => Ok(Self::Stage0),
            "vm" => Ok(Self::Vm),
            _ => Err("--head takes local, stage0, or vm".into()),
        }
    }

    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Stage0 => "stage0",
            Self::Vm => "vm",
        }
    }

    /// The head runs on a machine in the zone, not on the budget host.
    #[must_use]
    pub fn remote(self) -> bool {
        self != Self::Local
    }
}

/// Stage programs and arguments name the head's home with this; the head
/// replaces it with its own home when it starts.
pub const HOME_TOKEN: &str = "{head}";
/// The group's ssh key in the head's home.
pub const SSH_KEY: &str = "group-ssh";
/// The user the head's ssh key logs in as on the stage machines.
pub const SSH_USER: &str = "oa-stage";

/// The group's ssh key pair for a remote head, made at launch: the private
/// key travels to the head over its session, and the stage machines carry
/// the public key in their `ssh-keys` metadata (`keys`).
pub struct GroupKey {
    pub private: String,
    /// `oa-stage:ssh-ed25519 ...`, the file the stage machines' metadata
    /// reads.
    pub keys: PathBuf,
}

/// Make the group's key pair in `dir` with `ssh-keygen`.
///
/// # Errors
///
/// When `ssh-keygen` is missing or fails.
pub fn make_key(dir: &Path, group: &str) -> Result<GroupKey, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let key = dir.join(SSH_KEY);
    let _ = std::fs::remove_file(&key);
    let _ = std::fs::remove_file(dir.join(format!("{SSH_KEY}.pub")));
    let made = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C"])
        .arg(format!("oa-group-{group}"))
        .arg("-f")
        .arg(&key)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run ssh-keygen for the group's key: {e}"))?;
    if !made.status.success() {
        return Err(format!(
            "ssh-keygen could not make the group's key: {}",
            String::from_utf8_lossy(&made.stderr).trim()
        ));
    }
    let private = std::fs::read_to_string(&key).map_err(|e| e.to_string())?;
    let public =
        std::fs::read_to_string(dir.join(format!("{SSH_KEY}.pub"))).map_err(|e| e.to_string())?;
    let keys = dir.join("ssh-keys");
    std::fs::write(&keys, format!("{SSH_USER}:{}", public.trim())).map_err(|e| e.to_string())?;
    Ok(GroupKey { private, keys })
}

/// What the budget host knows about a remote head, from its status lines.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HeadState {
    /// The head beacon's address, `30200:<pubkey>:<name>`.
    pub head: Option<String>,
    pub ready: bool,
    pub generation: u64,
    pub failed: Option<String>,
    pub reason: String,
    /// The head drained and published its beacons offline.
    pub stopped: bool,
    /// The session ended, and why.
    pub ended: Option<String>,
}

/// The budget host's end of a remote head's session.
pub struct RemoteHead {
    child: tokio::process::Child,
    input: Option<tokio::process::ChildStdin>,
    state: Arc<Mutex<HeadState>>,
}

impl RemoteHead {
    /// Start `program args` (the head's ssh session) and send it `launch`.
    ///
    /// # Errors
    ///
    /// When the session can't start or doesn't take the launch.
    pub async fn start(program: &Path, args: &[String], launch: &Value) -> Result<Self, String> {
        let mut command = tokio::process::Command::new(program);
        command
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|e| format!("the head's session did not start: {e}"))?;
        let mut input = child.stdin.take();
        let output = child.stdout.take();
        let state = Arc::new(Mutex::new(HeadState::default()));
        if let Some(output) = output {
            let state = state.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(output).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let Ok(event) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                    match event["event"].as_str() {
                        Some("status") => {
                            state.head = event["head"].as_str().map(String::from);
                            state.ready = event["ready"].as_bool().unwrap_or(false);
                            state.generation = event["generation"].as_u64().unwrap_or(0);
                            state.failed = event["failed"].as_str().map(String::from);
                            state.reason = event["reason"].as_str().unwrap_or("").into();
                        }
                        Some("stopped") => state.stopped = true,
                        _ => {}
                    }
                }
                let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
                state
                    .ended
                    .get_or_insert_with(|| "the head's session ended".into());
            });
        }
        let mut line = launch.to_string();
        line.push('\n');
        if let Some(input) = input.as_mut() {
            input
                .write_all(line.as_bytes())
                .await
                .map_err(|e| format!("the head did not take its launch: {e}"))?;
        }
        Ok(Self {
            child,
            input,
            state,
        })
    }

    /// The head's latest status.
    pub fn state(&self) -> HeadState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Ask the head to drain and stop; wait up to `wait` for it to say it
    /// stopped, then end the session.
    pub async fn stop(&mut self, wait: Duration) -> HeadState {
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(b"down\n").await;
            let _ = input.flush().await;
            drop(input);
        }
        let deadline = Instant::now() + wait;
        while Instant::now() < deadline {
            let state = self.state();
            if state.stopped || state.ended.is_some() {
                break;
            }
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let exited = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;
        if exited.is_err() {
            let _ = self.child.start_kill();
            let _ = self.child.wait().await;
        }
        self.state()
    }
}

/// Replace [`HOME_TOKEN`] in a lane's stage programs and arguments.
fn place(settings: &mut Settings, home: &Path) {
    let home = home.display().to_string();
    if let Some(group) = settings.group.as_mut() {
        for stage in &mut group.stages {
            stage.program = stage
                .program
                .display()
                .to_string()
                .replace(HOME_TOKEN, &home)
                .into();
            for arg in &mut stage.args {
                *arg = arg.replace(HOME_TOKEN, &home);
            }
        }
    }
}

fn say(value: &Value) {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

/// `pylon cloud head --home DIR`: head a group in its zone for the budget
/// host on the other end of standard input and output.
///
/// # Errors
///
/// When the launch is missing or unreadable, or the home can't be written.
pub async fn serve(home: &Path, every: Duration) -> Result<(), String> {
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let line = input
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("no launch arrived on standard input")?;
    let launch: Value =
        serde_json::from_str(&line).map_err(|e| format!("an unreadable launch: {e}"))?;
    let mut settings: Settings = serde_json::from_value(launch["settings"].clone())
        .map_err(|e| format!("unreadable head settings: {e}"))?;
    let Some(lane) = settings.group.clone() else {
        return Err("the launch names no group".into());
    };
    lane.validate()?;
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    if let Some(key) = launch["ssh_key"].as_str() {
        let path = home.join(SSH_KEY);
        std::fs::write(&path, key).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| e.to_string())?;
        }
    }
    place(&mut settings, home);
    settings.on = true;
    settings.changed_at = now();
    share::save(home, &settings)?;
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let head = format!("30200:{}:{}", identity.pubkey(), settings.pylon);

    let (halt, halted) = oneshot::channel::<()>();
    let mut options = Options::foreground();
    options.every = every;
    let mut supervisor = tokio::spawn(crate::online::supervise(
        home.to_path_buf(),
        Arc::new(Dedicated),
        options,
        Arc::new(|line| eprintln!("pylon head: {line}")),
        async {
            let _ = halted.await;
        },
    ));
    let mut last = Value::Null;
    let mut beat = Instant::now();
    let mut finished = false;
    let reason = loop {
        let group = load_live(home).and_then(|l| l.group);
        let status = json!({
            "event": "status",
            "head": head,
            "ready": group.as_ref().is_some_and(|g| g.ready),
            "generation": group.as_ref().map_or(0, |g| g.generation),
            "failed": group.as_ref().and_then(|g| g.failed.clone()),
            "reason": group.as_ref().map(|g| g.reason.clone()).unwrap_or_default(),
            "stages": group.as_ref().map(|g| g.stages.iter().map(|s| json!({
                "name": s.name, "ready": s.ready,
            })).collect::<Vec<_>>()).unwrap_or_default(),
        });
        if status != last || beat.elapsed() >= Duration::from_secs(5) {
            say(&status);
            last = status;
            beat = Instant::now();
        }
        tokio::select! {
            line = input.next_line() => match line {
                Ok(Some(word)) if word.trim() == "down" => break "the budget host asked".to_string(),
                Ok(Some(_)) => {}
                _ => break "the budget host's session ended".to_string(),
            },
            _ = &mut supervisor => {
                finished = true;
                break "the head stopped".to_string();
            }
            () = tokio::time::sleep(every) => {}
        }
    };
    settings.on = false;
    settings.changed_at = now();
    let _ = share::save(home, &settings);
    let deadline = Instant::now() + crate::provider::DRAIN + Duration::from_secs(30);
    while !finished && !supervisor.is_finished() && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let _ = halt.send(());
    if !finished {
        let _ = supervisor.await;
    }
    say(&json!({"event": "stopped", "reason": reason, "head": head}));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placements_read_and_write_their_words() {
        for placement in [
            HeadPlacement::Local,
            HeadPlacement::Stage0,
            HeadPlacement::Vm,
        ] {
            assert_eq!(HeadPlacement::parse(placement.word()), Ok(placement));
        }
        assert!(HeadPlacement::parse("budget").is_err());
        assert!(!HeadPlacement::Local.remote());
        assert!(HeadPlacement::Vm.remote());
    }
}
