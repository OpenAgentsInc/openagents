//! Whether an engine's own program needs an update, and updating it
//! (#11463). Only Cursor's agent (`cursor-agent`) has an update command
//! Coder runs today.
//!
//! Cursor's service refuses old `cursor-agent` builds mid-turn with
//! `Update Required`. Coder keeps the build Cursor refused in
//! `~/.openagents/coder/engine-versions.json`, with the newest build Cursor's
//! install script names, so the next delegation to that same build stops
//! before it starts and `/plugins` shows that Cursor needs an update.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::bundled_runtime::AcpAgent;

/// The agent whose program Coder can update.
pub const CURSOR: &str = "cursor";
/// The file under Coder's data folder.
pub const FILE: &str = "engine-versions.json";
/// How long a refusal is believed without a new one.
const REFUSAL_KEPT: u64 = 7 * 24 * 60 * 60;
/// How long the newest build Cursor's installer named is reused.
const LATEST_KEPT: u64 = 12 * 60 * 60;
/// How long a check of Cursor's installer may take.
const LATEST_LIMIT: Duration = Duration::from_secs(5);

/// Whether Coder can update agent `id`'s program.
#[must_use]
pub fn updatable(id: &str) -> bool {
    id == CURSOR
}

/// What Coder knows about the builds of one agent's program.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The build the agent's service refused, and when (Unix seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refused: Option<(String, u64)>,
    /// The newest build out, and when Coder last read it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<(String, u64)>,
}

/// Where the records are kept.
#[must_use]
pub fn path() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    crate::home::data_path(FILE)
}

/// The records in `path`, by agent; empty when there are none.
#[must_use]
pub fn load(path: Option<&Path>) -> BTreeMap<String, Record> {
    path.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn change(path: Option<&Path>, id: &str, edit: impl FnOnce(&mut Record)) {
    let Some(path) = path else {
        return;
    };
    let mut records = load(Some(path));
    edit(records.entry(id.to_owned()).or_default());
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(&records) {
        let temporary = path.with_extension("json.tmp");
        if std::fs::write(&temporary, bytes).is_ok() {
            let _ = std::fs::rename(&temporary, path);
        }
    }
}

/// Keeps that agent `id`'s service refused `build`.
pub fn record_refused(path: Option<&Path>, id: &str, build: &str) {
    change(path, id, |record| {
        record.refused = Some((build.to_owned(), now()));
    });
}

fn record_latest(path: Option<&Path>, id: &str, build: &str) {
    change(path, id, |record| {
        record.latest = Some((build.to_owned(), now()));
    });
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Where an agent's installed build stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// The agent's service refused this very build.
    Refused { installed: String },
    /// A newer build is out; this one still works as far as Coder knows.
    Behind { installed: String, latest: String },
    /// The newest build Coder knows of.
    Current { installed: String },
    /// The program did not say its build.
    Unknown,
}

impl State {
    /// The state of `installed` given `record`, at `now`.
    #[must_use]
    pub fn of(installed: Option<&str>, record: &Record, now: u64) -> State {
        let Some(installed) = installed else {
            return State::Unknown;
        };
        let installed = installed.to_owned();
        if record.refused.as_ref().is_some_and(|(build, at)| {
            *build == installed && now.saturating_sub(*at) < REFUSAL_KEPT
        }) {
            return State::Refused { installed };
        }
        match &record.latest {
            Some((latest, _)) if acp_client::cursor::older(&installed, latest) => State::Behind {
                installed,
                latest: latest.clone(),
            },
            _ => State::Current { installed },
        }
    }

    /// The short status a list shows: `Update required`, `Update
    /// available`, or nothing.
    #[must_use]
    pub fn label(&self) -> Option<&'static str> {
        match self {
            State::Refused { .. } => Some(UPDATE_REQUIRED),
            State::Behind { .. } => Some("Update available"),
            _ => None,
        }
    }

    /// One sentence for the person, with the command that fixes it.
    #[must_use]
    pub fn sentence(&self) -> Option<String> {
        let command = acp_client::cursor::UPDATE_COMMAND;
        match self {
            State::Refused { installed } => Some(format!(
                "Cursor no longer accepts cursor-agent {installed}. Press U to update it, or run `{command}`."
            )),
            State::Behind { installed, latest } => Some(format!(
                "cursor-agent {latest} is out; this computer has {installed}. Press U to update it, or run `{command}`."
            )),
            _ => None,
        }
    }
}

/// The status the plugin list shows when an agent needs an update.
pub const UPDATE_REQUIRED: &str = "Update required";

/// Before a delegated turn to `agent_id`: its installed build, or the
/// reason the turn does not start because the agent's service already
/// refused that build. A newer build out is a note, not a refusal.
pub async fn before_turn(
    agent_id: &str,
    program: &Path,
    environment: &[(String, String)],
    cwd: &Path,
    path: Option<&Path>,
) -> Result<Checked, String> {
    if agent_id != CURSOR {
        return Ok(Checked::default());
    }
    let installed = acp_client::cursor::version(program, environment, cwd).await;
    let mut record = load(path).remove(agent_id).unwrap_or_default();
    let stale = record
        .latest
        .as_ref()
        .is_none_or(|(_, at)| now().saturating_sub(*at) >= LATEST_KEPT);
    if stale
        && path.is_some()
        && let Some(latest) = latest_cursor().await
    {
        record_latest(path, agent_id, &latest);
        record.latest = Some((latest, now()));
    }
    let state = State::of(installed.as_deref(), &record, now());
    if let State::Refused { installed } = &state {
        return Err(format!(
            "Cursor no longer accepts cursor-agent {installed}, so this task was not started. Run `{}`, then try again.",
            acp_client::cursor::UPDATE_COMMAND
        ));
    }
    Ok(Checked {
        note: match &state {
            State::Behind { installed, latest } => Some(format!(
                "cursor-agent {latest} is out; this computer has {installed}. To update, run `{}`.",
                acp_client::cursor::UPDATE_COMMAND
            )),
            _ => None,
        },
        installed,
    })
}

/// What [`before_turn`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checked {
    /// The installed build, when the program said it.
    pub installed: Option<String>,
    /// A note for the result when a newer build is out.
    pub note: Option<String>,
}

/// The newest `cursor-agent` build Cursor's install script names.
async fn latest_cursor() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(LATEST_LIMIT)
        .build()
        .ok()?;
    let script = client
        .get(acp_client::cursor::INSTALLER_URL)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()?;
    acp_client::cursor::latest_in_installer(&script)
}

/// The environment Coder's own checks run the agent's program with: this
/// process's, as the person's shell gave it.
fn environment() -> Vec<(String, String)> {
    std::env::vars_os()
        .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

fn working_folder() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
}

/// What a background check or update sends back.
enum Done {
    Checked(String, State),
    Updated(String, State, String),
}

/// The update state `/plugins` shows for each agent Coder can update, and
/// the work running in the background for it.
#[derive(Default)]
pub struct Panel {
    states: BTreeMap<String, State>,
    /// The agent being updated, if any.
    pub updating: Option<String>,
    /// What the last update said.
    pub message: Option<String>,
    checking: bool,
    checked_at: Option<std::time::Instant>,
    receiver: Option<mpsc::Receiver<Done>>,
}

impl Panel {
    /// The state Coder last found for agent `id`.
    #[must_use]
    pub fn state(&self, id: &str) -> Option<&State> {
        self.states.get(id)
    }

    /// Whether any of `agents` needs an update before it can run.
    #[must_use]
    pub fn any_refused<'a>(&self, mut agents: impl Iterator<Item = &'a AcpAgent>) -> bool {
        agents.any(|agent| matches!(self.states.get(&agent.id), Some(State::Refused { .. })))
    }

    /// Checks the agents Coder can update among `agents`, in the
    /// background, at most once a minute unless `force`.
    pub fn check(&mut self, agents: &[&AcpAgent], force: bool) {
        if self.checking || self.updating.is_some() {
            return;
        }
        if !force
            && self
                .checked_at
                .is_some_and(|at| at.elapsed() < Duration::from_secs(60))
        {
            return;
        }
        let targets: Vec<(String, PathBuf)> = agents
            .iter()
            .filter(|agent| updatable(&agent.id))
            .map(|agent| (agent.id.clone(), agent.program.clone()))
            .collect();
        if targets.is_empty() {
            return;
        }
        self.checking = true;
        self.checked_at = Some(std::time::Instant::now());
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(async {
                for (id, program) in targets {
                    let state = current_state(&id, &program).await;
                    let _ = sender.send(Done::Checked(id, state));
                }
            });
        });
    }

    /// Updates agent `agent`'s program in the background. Returns whether
    /// an update started.
    pub fn update(&mut self, agent: &AcpAgent) -> bool {
        if !updatable(&agent.id) || self.updating.is_some() {
            return false;
        }
        self.updating = Some(agent.id.clone());
        self.message = None;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.checking = false;
        let id = agent.id.clone();
        let program = agent.program.clone();
        std::thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                let _ = sender.send(Done::Updated(
                    id,
                    State::Unknown,
                    "Coder could not start the update.".into(),
                ));
                return;
            };
            runtime.block_on(async {
                let updated =
                    acp_client::cursor::update(&program, &environment(), &working_folder()).await;
                let state = current_state(&id, &program).await;
                let _ = sender.send(Done::Updated(id, state, updated.message()));
            });
        });
        true
    }

    /// Takes in finished background work. Returns whether anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };
        let mut changed = false;
        loop {
            match receiver.try_recv() {
                Ok(Done::Checked(id, state)) => {
                    self.states.insert(id, state);
                    changed = true;
                }
                Ok(Done::Updated(id, state, message)) => {
                    self.states.insert(id, state);
                    self.message = Some(message);
                    self.updating = None;
                    changed = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.checking = false;
                    if self.updating.take().is_some() {
                        self.message = Some("The update stopped without saying why.".into());
                    }
                    changed = true;
                    break;
                }
            }
        }
        changed
    }

    #[cfg(test)]
    pub(crate) fn set(&mut self, id: &str, state: State) {
        self.states.insert(id.to_owned(), state);
    }
}

/// Agent `id`'s state now: its build, against the records (and Cursor's
/// installer when the record of the newest build is old).
async fn current_state(id: &str, program: &Path) -> State {
    let path = path();
    let installed = acp_client::cursor::version(program, &environment(), &working_folder()).await;
    let mut record = load(path.as_deref()).remove(id).unwrap_or_default();
    if path.is_some()
        && record
            .latest
            .as_ref()
            .is_none_or(|(_, at)| now().saturating_sub(*at) >= LATEST_KEPT)
        && let Some(latest) = latest_cursor().await
    {
        record_latest(path.as_deref(), id, &latest);
        record.latest = Some((latest, now()));
    }
    State::of(installed.as_deref(), &record, now())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "2026.06.24-00-45-58-9f61de7";
    const NEW: &str = "2026.10.01-e373342";

    #[test]
    fn a_refused_build_needs_an_update_until_it_changes() {
        let record = Record {
            refused: Some((OLD.into(), 1_000)),
            latest: Some((NEW.into(), 1_000)),
        };
        assert_eq!(
            State::of(Some(OLD), &record, 2_000),
            State::Refused {
                installed: OLD.into()
            }
        );
        assert_eq!(
            State::of(Some(OLD), &record, 2_000).label(),
            Some(UPDATE_REQUIRED)
        );
        assert_eq!(
            State::of(Some(NEW), &record, 2_000),
            State::Current {
                installed: NEW.into()
            }
        );
        // An old refusal is no longer believed; the build is only behind.
        assert_eq!(
            State::of(Some(OLD), &record, 1_000 + REFUSAL_KEPT),
            State::Behind {
                installed: OLD.into(),
                latest: NEW.into()
            }
        );
        assert_eq!(State::of(None, &record, 2_000), State::Unknown);
    }

    #[test]
    fn every_sentence_names_the_command() {
        for state in [
            State::Refused {
                installed: OLD.into(),
            },
            State::Behind {
                installed: OLD.into(),
                latest: NEW.into(),
            },
        ] {
            assert!(
                state
                    .sentence()
                    .unwrap()
                    .contains(acp_client::cursor::UPDATE_COMMAND)
            );
        }
    }

    #[test]
    fn records_survive_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        record_refused(Some(&path), CURSOR, OLD);
        record_latest(Some(&path), CURSOR, NEW);
        let records = load(Some(&path));
        assert_eq!(records[CURSOR].refused.as_ref().unwrap().0, OLD);
        assert_eq!(records[CURSOR].latest.as_ref().unwrap().0, NEW);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_refused_build_stops_the_turn_before_it_starts() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("cursor-agent");
        std::fs::write(&program, format!("#!/bin/sh\necho {OLD}\n")).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = dir.path().join(FILE);
        // A fresh record of the newest build, so no network is read.
        record_latest(Some(&path), CURSOR, NEW);
        let checked = before_turn(CURSOR, &program, &[], dir.path(), Some(&path))
            .await
            .unwrap();
        assert_eq!(checked.installed.as_deref(), Some(OLD));
        assert!(
            checked
                .note
                .unwrap()
                .contains(acp_client::cursor::UPDATE_COMMAND)
        );
        record_refused(Some(&path), CURSOR, OLD);
        let refused = before_turn(CURSOR, &program, &[], dir.path(), Some(&path))
            .await
            .unwrap_err();
        assert!(refused.contains("was not started"), "{refused}");
        assert!(refused.contains(acp_client::cursor::UPDATE_COMMAND));
        // Another agent is not checked.
        assert_eq!(
            before_turn("opencode", &program, &[], dir.path(), Some(&path)).await,
            Ok(Checked::default())
        );
    }
}
