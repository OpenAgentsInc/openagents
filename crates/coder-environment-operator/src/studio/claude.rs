//! Claude Code tasks on a saved environment version (BYO-03).
//!
//! A run is an ordinary Cloud job ([`coder_cloud::Record`]) whose engine is
//! Claude Code ([`coder_cloud::claude::ENGINE`]) and whose environment pin
//! is the selected saved version, resolved once when the run starts
//! ([`coder_environment::Environment::pin`]): Boat boots exactly that
//! version's image, never the daily template, and a missing image is a
//! failure, not a fallback. The job runs through [`coder_cloud::drive`]
//! with the Boat backend on a thread of its own, so the job store, cleanup,
//! and the engine evidence (engine, pinned version, credential type) are
//! the operator's.
//!
//! The run uses a Claude credential ([`Key`]): the one the signed-in
//! person saved in Settings, released for this run only, else the key this
//! server's environment names (BYO-04/05: applied fresh at the turn, never
//! stored in the record or the image). A fresh computer from a saved image
//! has no Claude login inside it, so the web offers runs only when a
//! credential is available.

use coder_cloud::runtime::Credentials;
use coder_cloud::{Mode, Placement, Record, Spec, State, Store};
use coder_environment::Environment;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// The longest one run may take.
pub const RUN_SECONDS: u64 = 3600;
pub const MAX_PROMPT: usize = 16 * 1024;

/// A Claude credential for one run: its name (`ANTHROPIC_API_KEY`,
/// `CLAUDE_CODE_OAUTH_TOKEN` for a subscription token, or the Bedrock,
/// Vertex, or Foundry name) and value. It never prints, and its
/// bytes are zeroed when it drops.
pub struct Key {
    name: String,
    value: String,
}

impl Key {
    /// A credential named `name` (one of [`coder_cloud::claude`]'s own
    /// credential names) holding `value`.
    pub fn new(name: &str, value: String) -> Result<Self, String> {
        let key = Self {
            name: name.to_owned(),
            value,
        };
        if coder_cloud::claude::OwnCredential::from_name(name).is_none()
            || key.value.trim().is_empty()
        {
            return Err("That Claude credential can't be used.".into());
        }
        Ok(key)
    }

    /// The credential's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Runtime credentials holding a copy of the value, which redact it
    /// from traces.
    fn credentials(&self) -> Result<Credentials, String> {
        Credentials::from_names(std::slice::from_ref(&self.name), |_| {
            Some(self.value.clone())
        })
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.value).into_bytes();
        bytes.fill(0);
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Key({}, redacted)", self.name)
    }
}

/// One run as the web shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub id: String,
    pub environment: String,
    pub prompt: String,
    pub version: Option<u64>,
    pub state: RunState,
    pub events: Vec<Value>,
    pub reply: Option<String>,
    pub error: Option<String>,
    pub created_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Starting,
    Running,
    Paused,
    Done,
    Failed,
    Stopped,
}

impl RunState {
    pub fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Stopped)
    }
}

/// The run store: one Cloud job store per environment.
#[derive(Clone, Debug)]
pub struct Runs {
    root: PathBuf,
}

impl Runs {
    pub fn under(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn store(&self, environment: &str) -> Store {
        Store::under(self.root.join(environment))
    }

    /// Runs of `environment`, newest first.
    pub fn list(&self, environment: &str) -> Vec<Run> {
        self.store(environment)
            .list()
            .unwrap_or_default()
            .into_iter()
            .map(|r| view(environment, r))
            .collect()
    }

    pub fn read(&self, environment: &str, id: &str) -> Option<Run> {
        self.store(environment)
            .read(id)
            .ok()
            .map(|r| view(environment, r))
    }

    /// Ask a run to stop; the driver cancels it and deletes the machine.
    pub fn stop(&self, environment: &str, id: &str) -> Result<(), String> {
        self.store(environment).cancel(id)
    }

    /// Record a new run of `prompt` on the environment's selected version
    /// and start it on its own thread. `key` is the Claude credential, if
    /// any; it lives in memory for the run only.
    pub fn start(
        &self,
        env: &Environment,
        prompt: &str,
        workdir: &str,
        size: &str,
        key: Option<Key>,
    ) -> Result<String, String> {
        let pin = env
            .pin()
            .ok_or("Save the environment before running Claude Code on it.")?;
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.len() > MAX_PROMPT {
            return Err("Write what Claude Code should do, up to 16 KB.".into());
        }
        let store = self.store(&env.id);
        let n = store.list().map(|l| l.len()).unwrap_or(0) + 1;
        let id = format!("claude-{}-{n}", env.id);
        let names: Vec<String> = key.iter().map(|k| k.name.clone()).collect();
        let spec = Spec {
            placement: Placement::Boat,
            mode: Mode::Coder,
            agent: coder_cloud::claude::ENGINE.into(),
            task: task(prompt, workdir),
            model: None,
            reasoning: None,
            cwd: PathBuf::from(workdir),
            timeout_seconds: RUN_SECONDS,
            size: size.into(),
            template: None,
            credential_names: names,
        };
        let lease = store.lease(&id)?;
        if lease.exists() {
            return Err("That run already exists.".into());
        }
        let mut record = Record::new(&id, spec)?;
        record.environment = Some(pin);
        coder_cloud::claude_task::admit(&mut record, &env.id);
        lease.save(&record)?;
        std::thread::Builder::new()
            .name(format!("claude-{n}"))
            .spawn(move || drive(lease, record, key))
            .map_err(|_| "The run couldn't start.")?;
        Ok(id)
    }
}

/// What Claude Code is asked: the person's words, then where the
/// repository is.
pub fn task(prompt: &str, workdir: &str) -> String {
    format!("{prompt}\n\nThe repository is checked out at {workdir}; work there.")
}

fn drive(lease: coder_cloud::Lease, mut record: Record, key: Option<Key>) {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        return;
    };
    // The runtime credentials hold the only copy from here on; the key's
    // own bytes are zeroed now.
    let credentials = key.as_ref().map(Key::credentials).transpose();
    drop(key);
    let result = runtime.block_on(async {
        let credentials = credentials?.unwrap_or_default();
        let client = boat::Client::from_env()
            .await
            .map_err(|e| format!("Boat is unavailable: {e}"))?;
        let backend = coder_cloud::boat_backend::Boat {
            client,
            credentials,
        };
        coder_cloud::drive(
            &backend,
            &lease,
            &mut record,
            &AtomicBool::new(false),
            Duration::from_secs(2),
            &mut |_| {},
        )
        .await
    });
    if let Err(error) = result
        && !record.state.terminal()
    {
        record.state = State::Failed;
        record.error = Some(error);
        let _ = lease.save(&record);
    }
}

fn view(environment: &str, r: Record) -> Run {
    let state = match r.state {
        State::Created | State::Provisioning | State::Resuming | State::Ready => RunState::Starting,
        State::Dispatching | State::Running => RunState::Running,
        State::Paused => RunState::Paused,
        State::Completed => RunState::Done,
        State::Failed => RunState::Failed,
        State::Cancelled => RunState::Stopped,
    };
    let prompt = r
        .spec
        .task
        .rsplit_once("\n\nThe repository is checked out at ")
        .map_or(r.spec.task.as_str(), |(p, _)| p)
        .to_owned();
    let reply = r.result.as_ref().and_then(|v| {
        v["reply"]
            .as_str()
            .or_else(|| v["result"]["reply"].as_str())
            .map(str::to_owned)
    });
    Run {
        id: r.id.clone(),
        environment: environment.into(),
        prompt,
        version: r.environment.as_ref().map(|p| p.number),
        state,
        events: r.events,
        reply,
        error: r
            .error
            .map(|e| crate::activity::plain(&e, "The run failed.")),
        created_ms: r.created_ms,
    }
}

/// The text of a run's events a person reads: Claude Code's streamed
/// words, joined, and the tool steps it reported.
pub fn transcript(events: &[Value]) -> Vec<Step> {
    let mut out: Vec<Step> = vec![];
    for e in events {
        match e["event"].as_str() {
            Some("delta") => {
                let text = e["text"].as_str().unwrap_or_default();
                if let Some(Step::Said(s)) = out.last_mut() {
                    s.push_str(text);
                } else if !text.is_empty() {
                    out.push(Step::Said(text.into()));
                }
            }
            Some("entry") => {
                let entry = &e["entry"];
                let title = ["title", "tool", "name", "kind", "source"]
                    .iter()
                    .find_map(|k| entry[*k].as_str())
                    .unwrap_or("Step")
                    .to_owned();
                let detail = ["command", "path", "summary", "text"]
                    .iter()
                    .find_map(|k| entry[*k].as_str())
                    .unwrap_or_default()
                    .to_owned();
                out.push(Step::Tool { title, detail });
            }
            _ => {}
        }
    }
    out
}

/// One piece of a run's transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Said(String),
    Tool { title: String, detail: String },
}

/// The run store directory under the studio state.
pub fn root(state: &Path) -> PathBuf {
    state.join("environment-claude")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_key_names_its_class_never_prints_and_carries_into_the_run() {
        // Assembled at run time so no key-shaped literal sits here.
        let value = format!("sk-ant-api03-{}", "a1".repeat(24));
        let key = Key::new(coder_cloud::claude::API_KEY, value.clone()).unwrap();
        assert_eq!(key.name(), coder_cloud::claude::API_KEY);
        assert!(!format!("{key:?}").contains(&value));
        let credentials = key.credentials().unwrap();
        assert_eq!(
            credentials.environment()[coder_cloud::claude::API_KEY],
            value
        );
        assert!(Key::new("GITHUB_TOKEN", value.clone()).is_err());
        assert!(Key::new(coder_cloud::claude::API_KEY, " ".into()).is_err());

        // A saved subscription token runs as CLAUDE_CODE_OAUTH_TOKEN alone.
        let token = format!("sk-ant-oat01-{}", "b2".repeat(40));
        let key = Key::new(coder_cloud::claude::OAUTH_TOKEN, token.clone()).unwrap();
        assert!(!format!("{key:?}").contains(&token));
        let env = key.credentials().unwrap().environment();
        assert_eq!(env.len(), 1);
        assert_eq!(env[coder_cloud::claude::OAUTH_TOKEN], token);
        // A token under the API key's name never reaches a run.
        let wrong = Key::new(coder_cloud::claude::API_KEY, token.clone()).unwrap();
        assert!(wrong.credentials().is_err());
    }

    #[test]
    fn transcripts_join_words_and_list_steps() {
        let events = vec![
            json!({"event":"engine","engine":"claude"}),
            json!({"event":"delta","text":"Look"}),
            json!({"event":"delta","text":"ing."}),
            json!({"event":"entry","entry":{"tool":"Bash","command":"cargo test"}}),
            json!({"event":"delta","text":"Done."}),
        ];
        assert_eq!(
            transcript(&events),
            vec![
                Step::Said("Looking.".into()),
                Step::Tool {
                    title: "Bash".into(),
                    detail: "cargo test".into()
                },
                Step::Said("Done.".into()),
            ]
        );
        assert_eq!(
            task("Fix it", "/home/user/repo"),
            "Fix it\n\nThe repository is checked out at /home/user/repo; work there."
        );
    }
}
