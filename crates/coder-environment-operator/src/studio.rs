//! Environments from repositories, for the web's Environments pages.
//!
//! A [`Studio`] owns the packaged environment owners ([`Owners`]) on a
//! thread of its own and runs the setup [`agent`](crate::agent) for each
//! environment: pick a repository and branch, and the agent sets it up on
//! a dedicated setup computer, a clean build seals an image, and a fresh
//! machine checks it. The person reads the work as a conversation
//! ([`crate::activity`]), answers questions, steers, and saves the verified
//! candidate as a version. A saved version can then run Claude Code tasks
//! ([`claude`]).
//!
//! Reads (lists, views, activity) come straight from the retained records
//! on disk; everything that drives a provider or the model goes to the
//! owner thread, so no provider future needs to be `Send`.
//!
//! State layout under `state`:
//!
//! | Path | Contents |
//! | --- | --- |
//! | `environments/`, `environment-setup/`, … | the owners' records ([`crate::Layout`]) |
//! | `environment-studio/<env>/meta.json` | repository, branch, and when it was added |
//! | `environment-studio/<env>/activity.jsonl` | the conversation ([`crate::activity`]) |
//! | `environment-studio/<env>/agent.json` | the setup agent's retained state |
//! | `environment-claude/<env>/` | Claude Code runs (Cloud job records) |

pub mod claude;
pub mod github;

use crate::activity::{CheckLine, Entry, Logs, Record, plain};
use crate::agent::{Agent, Brief, Phase, State};
use crate::{Custody, Owners, Providers, now_ms};
use coder_environment::promotion::Review;
use coder_environment::transition::Command;
use coder_environment::{
    ArtifactPin, Environment, ImagePin, Limits, Platform, ProjectLink, Provider, Qualification,
    Recipe, Script, SourcePin, digest,
};
use coder_working_computer::Principal;
use coder_working_computer::provider::{Commands, Images};
use codex_transport::Transport;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub const SCHEMA: &str = "openagents.environment.studio.v1";
pub const DEFAULT_MODEL: &str = "gpt-6.1-sol";
pub const DEFAULT_SIZE: &str = "small";
pub const DEFAULT_DEADLINE_SECONDS: u64 = 2 * 3600;
/// The operator profile alias setup sessions run under.
pub const PROFILE_ALIAS: &str = "environment-setup";
pub const MAX_MESSAGE: usize = 4000;

/// The studio's configuration document (`--environments`). It names
/// credentials by environment variable; it holds no secret.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    /// A private directory for every record.
    pub state: PathBuf,
    /// The machines: the same document `--environment-owners` takes.
    pub machines: crate::Config,
    /// Who owns the environments made here.
    pub owner: Principal,
    /// Codex's home, holding the login the setup agent uses; default
    /// `$CODEX_HOME` or `~/.codex`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex_home: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_seconds: Option<u64>,
    /// The environment variable holding a GitHub token, for listing your
    /// repositories and fetching private ones. It must also be one of
    /// `machines.credential_names` for private fetches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    /// The environment variable holding your Anthropic API key, for
    /// Claude Code runs on saved environments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_key: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|_| "The environments config is unreadable.")?;
        let config: Self = serde_json::from_slice(&bytes)
            .map_err(|e| format!("The environments config is invalid: {e}"))?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("The environments config schema must be {SCHEMA}."));
        }
        if !self.state.is_absolute() {
            return Err("The environments state directory must be absolute.".into());
        }
        self.machines.validate()?;
        if self.machines.provider != crate::ProviderKind::Boat {
            return Err("Environments set up here run on Boat.".into());
        }
        if !coder_environment::valid_id(&self.owner.workspace)
            || !coder_environment::valid_id(&self.owner.principal)
        {
            return Err("The owner needs a workspace and principal.".into());
        }
        if self
            .deadline_seconds
            .is_some_and(|d| d == 0 || d > coder_environment_setup::MAX_DEADLINE_SECONDS)
        {
            return Err("deadline_seconds must be 1 to 86400.".into());
        }
        Ok(())
    }
    fn model(&self) -> String {
        self.model.clone().unwrap_or_else(|| DEFAULT_MODEL.into())
    }
    fn size(&self) -> String {
        self.size.clone().unwrap_or_else(|| DEFAULT_SIZE.into())
    }
    fn deadline(&self) -> u64 {
        self.deadline_seconds.unwrap_or(DEFAULT_DEADLINE_SECONDS)
    }
    /// The GitHub credential setup machines may use for fetching.
    fn git_credential(&self) -> Option<String> {
        self.machines
            .credential_names
            .iter()
            .find(|n| coder_environment_setup::GIT_CREDENTIALS.contains(&n.as_str()))
            .cloned()
    }
    fn secret(name: &Option<String>) -> Option<String> {
        name.as_deref()
            .and_then(|n| std::env::var(n).ok())
            .filter(|v| !v.trim().is_empty())
    }
}

/// What the person sees about one environment in a list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub repository: String,
    pub branch: String,
    pub commit: String,
    pub status: Status,
    /// The newest saved version number.
    pub saved: Option<u64>,
    pub updated_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Working,
    NeedsInput,
    Building,
    Checking,
    ReadyToSave,
    Saved,
    Failed,
}

/// The candidate a person reviews before saving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// The exact candidate's digest; Save names it.
    pub digest: String,
    pub recipe_revision: u64,
    pub commit: String,
    pub image: String,
    pub checks: Vec<CheckLine>,
    pub summary: String,
}

/// One saved version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub number: u64,
    pub selected: bool,
    pub image: String,
    pub created_ms: u64,
}

/// Everything the environment page shows.
#[derive(Clone, Debug)]
pub struct View {
    pub summary: Summary,
    pub records: Vec<Record>,
    pub phase: Phase,
    pub question: Option<String>,
    pub candidate: Option<Candidate>,
    pub versions: Vec<Version>,
    /// The current recipe script.
    pub recipe: Option<String>,
    pub runs: Vec<claude::Run>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Meta {
    repository: String,
    branch: String,
    created_ms: u64,
}

enum Work {
    Run(String),
    Steer(String, String),
    Retry(String),
    Save(String, String, oneshot::Sender<Result<u64, String>>),
}

/// Makes a model transport for one environment's setup.
pub type Transports<T> = Arc<dyn Fn(&str) -> Result<T, String> + Send + Sync>;

/// The Environments service the web holds.
pub struct Studio {
    config: Config,
    layout: crate::Layout,
    logs: Arc<Logs>,
    runs: claude::Runs,
    github: github::GitHub,
    base: ImagePin,
    runtime: ArtifactPin,
    work: mpsc::UnboundedSender<Work>,
}

impl std::fmt::Debug for Studio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Studio")
            .field("state", &self.config.state)
            .finish()
    }
}

impl Studio {
    /// Open the studio on Boat with the Codex login, as `--environments`
    /// does. Reads `BOAT_API_KEY`/`BOAT_API_BASE` and the named
    /// credentials from this process's environment.
    pub async fn open(config: Config) -> Result<Arc<Self>, String> {
        config.validate()?;
        let mut machines = config.machines.clone();
        if machines.template.is_none() {
            machines.template = crate::boat::runtime_template().await?;
        }
        let providers = crate::boat::providers(&machines).await?;
        let login = config
            .codex_home
            .clone()
            .map(|h| h.join("auth.json"))
            .or_else(codex_transport::codex::Login::default_path)
            .ok_or("No Codex login was found. Sign in with `codex login`.")?;
        codex_transport::codex::Login::load(&login)
            .map_err(|e| format!("The Codex login at {} can't be used: {e}", login.display()))?;
        let transports: Transports<codex_transport::codex::CodexTransport> =
            Arc::new(move |env: &str| {
                codex_transport::codex::CodexTransport::new(
                    login.clone(),
                    &format!("environment-{env}"),
                )
                .map_err(|e| format!("The Codex login can't be used: {e}"))
            });
        let template = machines
            .template
            .clone()
            .unwrap_or_else(|| "boat-default".into());
        let mut config = config;
        config.machines = machines;
        Self::start(
            config,
            &template,
            providers,
            crate::environment_custody(),
            transports,
            Duration::from_secs(1),
        )
    }

    /// Start the studio over `providers` and `transports`. `template`
    /// names the image fresh setup and builder machines start from.
    pub fn start<P, T>(
        config: Config,
        template: &str,
        providers: Providers<P>,
        custody: Custody,
        transports: Transports<T>,
        poll: Duration,
    ) -> Result<Arc<Self>, String>
    where
        P: Commands + Images + Send + 'static,
        T: Transport + 'static,
    {
        let layout = crate::Layout::under(&config.state);
        let logs = Arc::new(Logs::under(config.state.join("environment-studio")));
        let (work, inbox) = mpsc::unbounded_channel();
        let (ready, opened) = std::sync::mpsc::channel();
        let thread_config = config.clone();
        let thread_logs = logs.clone();
        std::thread::Builder::new()
            .name("environments".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = ready.send(Err(format!("The environments runtime failed: {e}")));
                        return;
                    }
                };
                let local = tokio::task::LocalSet::new();
                local.block_on(&runtime, async move {
                    let owners = match Owners::open(&thread_config.state, providers, custody) {
                        Ok(o) => Arc::new(o),
                        Err(e) => {
                            let _ = ready.send(Err(e));
                            return;
                        }
                    };
                    let _ = ready.send(Ok(()));
                    serve(thread_config, owners, thread_logs, transports, poll, inbox).await;
                });
            })
            .map_err(|e| format!("The environments thread didn't start: {e}"))?;
        opened
            .recv()
            .map_err(|_| "The environments thread stopped.".to_owned())??;
        let token = Config::secret(&config.github_token);
        let studio = Arc::new(Self {
            base: ImagePin {
                provider: Provider::Boat,
                image_id: template.into(),
                digest: digest(format!("boat-template:{template}").as_bytes()),
            },
            runtime: ArtifactPin {
                revision: template.into(),
                digest: digest(format!("runtime:{template}").as_bytes()),
            },
            layout,
            logs,
            runs: claude::Runs::under(claude::root(&config.state)),
            github: github::GitHub::new(token),
            config,
            work,
        });
        // Pick up every setup that was moving when the studio stopped.
        for s in studio.list() {
            if matches!(
                s.status,
                Status::Starting | Status::Working | Status::Building | Status::Checking
            ) {
                let _ = studio.work.send(Work::Run(s.id));
            }
        }
        Ok(studio)
    }

    fn studio_dir(&self, id: &str) -> PathBuf {
        self.config.state.join("environment-studio").join(id)
    }

    fn envs(&self) -> coder_environment::store::Store {
        coder_environment::store::Store::under(self.layout.environments())
    }

    fn meta(&self, id: &str) -> Option<Meta> {
        let bytes = fs::read(self.studio_dir(id).join("meta.json")).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    fn state(&self, id: &str) -> Option<State> {
        let bytes = fs::read(self.studio_dir(id).join("agent.json")).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// The GitHub client (your repositories when a token is configured).
    pub fn github(&self) -> &github::GitHub {
        &self.github
    }

    /// Whether Claude Code runs can start (an Anthropic API key is
    /// configured).
    pub fn claude_ready(&self) -> bool {
        Config::secret(&self.config.claude_key).is_some()
    }

    /// A receiver that changes whenever any environment's conversation
    /// grows.
    pub fn watch(&self) -> tokio::sync::watch::Receiver<u64> {
        self.logs.watch()
    }

    /// How many records an environment's conversation holds.
    pub fn revision(&self, id: &str) -> usize {
        self.logs.read(id).len()
    }

    /// Every environment of the owner, most recently changed first.
    pub fn list(&self) -> Vec<Summary> {
        let mut rows: Vec<Summary> = self
            .envs()
            .list()
            .unwrap_or_default()
            .iter()
            .filter(|e| e.project.workspace == self.config.owner.workspace)
            .filter_map(|e| self.summary(e))
            .collect();
        rows.sort_by_key(|s| std::cmp::Reverse(s.updated_ms));
        rows
    }

    fn summary(&self, env: &Environment) -> Option<Summary> {
        let meta = self.meta(&env.id)?;
        let state = self.state(&env.id);
        let saved = env.versions.last().map(|v| v.number);
        let status = match state.as_ref().map(|s| &s.phase) {
            None | Some(Phase::Starting) => Status::Starting,
            Some(Phase::Working) => Status::Working,
            Some(Phase::Waiting { .. }) => Status::NeedsInput,
            Some(Phase::Building) => Status::Building,
            Some(Phase::Verifying) => Status::Checking,
            Some(Phase::Review { .. }) => Status::ReadyToSave,
            Some(Phase::Saved { .. }) => Status::Saved,
            Some(Phase::Failed { .. }) => Status::Failed,
        };
        let updated_ms = self
            .logs
            .read(&env.id)
            .last()
            .map_or(meta.created_ms, |r| r.at_ms);
        Some(Summary {
            id: env.id.clone(),
            repository: meta.repository,
            branch: meta.branch,
            commit: env.source.revision.clone(),
            status,
            saved,
            updated_ms,
        })
    }

    /// One environment's page.
    pub fn view(&self, id: &str) -> Option<View> {
        let env = self.envs().read(id).ok()?;
        if env.project.workspace != self.config.owner.workspace {
            return None;
        }
        let summary = self.summary(&env)?;
        let state = self.state(id).unwrap_or_else(|| State::new(id));
        let question = match &state.phase {
            Phase::Waiting { question } => Some(question.clone()),
            _ => None,
        };
        let candidate = match &state.phase {
            Phase::Review { verification } => env.propose(verification).ok().map(|c| Candidate {
                digest: c.digest(),
                recipe_revision: c.recipe_revision,
                commit: c.source.revision.clone(),
                image: c.image.image_id.clone(),
                checks: state
                    .checks
                    .as_ref()
                    .map(|k| k.lines.clone())
                    .unwrap_or_default(),
                summary: state.summary.clone().unwrap_or_default(),
            }),
            _ => None,
        };
        let versions = env
            .history(None, 20)
            .into_iter()
            .map(|h| Version {
                number: h.number,
                selected: h.selected,
                image: h.image.image_id,
                created_ms: h.created_ms,
            })
            .collect();
        let recipe = fs::read_to_string(
            self.layout
                .setup_blobs()
                .join(&env.draft().recipe.install.digest),
        )
        .ok();
        Some(View {
            summary,
            records: self.logs.read(id),
            phase: state.phase,
            question,
            candidate,
            versions,
            recipe,
            runs: self.runs.list(id),
        })
    }

    /// Resolve what the person picked to an exact commit.
    pub async fn resolve(
        &self,
        repository: &str,
        branch: Option<&str>,
    ) -> Result<github::Resolved, String> {
        let name = github::RepoName::parse(repository)
            .ok_or("Enter a GitHub repository as owner/name or its github.com address.")?;
        self.github.resolve(&name, branch).await
    }

    /// Add an environment for a resolved repository and start its setup.
    pub fn create(&self, resolved: &github::Resolved) -> Result<String, String> {
        self.add(resolved)
            .map_err(|e| plain(&e, "The environment couldn't be added. Try again."))
    }

    fn add(&self, resolved: &github::Resolved) -> Result<String, String> {
        let now = now_ms();
        let full = resolved.repository.full();
        let id = format!(
            "env-{}",
            &digest(format!("{full}\0{}\0{now}", resolved.commit).as_bytes())[..12]
        );
        let project = project_id(&resolved.repository.name);
        let source = SourcePin {
            repository: Some(full.clone()),
            revision: resolved.commit.clone(),
            digest: digest(format!("github:{full}@{}", resolved.commit).as_bytes()),
        };
        let env = Environment::new(
            &id,
            ProjectLink {
                workspace: self.config.owner.workspace.clone(),
                project,
            },
            source,
            self.recipe(),
            now,
        )
        .map_err(|e| e.to_string())?;
        let dir = self.studio_dir(&id);
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot keep the environment: {e}"))?;
        let meta = Meta {
            repository: full.clone(),
            branch: resolved.branch.clone(),
            created_ms: now,
        };
        fs::write(
            dir.join("meta.json"),
            serde_json::to_vec(&meta).map_err(|_| "Cannot encode the environment.")?,
        )
        .map_err(|e| format!("Cannot keep the environment: {e}"))?;
        self.envs().create(&env).map_err(|e| e.to_string())?;
        self.logs.push(
            &id,
            Entry::User {
                text: format!(
                    "Set up {full} on {} so agents can build and test it.",
                    resolved.branch
                ),
            },
            now,
        )?;
        self.logs.push(&id, Entry::Starting, now)?;
        self.send(Work::Run(id.clone()))?;
        Ok(id)
    }

    fn recipe(&self) -> Recipe {
        Recipe {
            schema: coder_environment::RECIPE_SCHEMA.into(),
            base: self.base.clone(),
            runtime: self.runtime.clone(),
            platform: Platform {
                os: "linux".into(),
                architecture: "x86_64".into(),
            },
            install: Script {
                cwd: ".".into(),
                digest: digest(b"no install recipe yet"),
            },
            start: Default::default(),
            inputs: Default::default(),
            credential_names: Default::default(),
            qualification: Qualification {
                profile: "unset".into(),
                plan_digest: digest(b"no checks yet"),
            },
            limits: Limits {
                deadline_seconds: self.config.deadline(),
                concurrent_machines: 2,
                total_machine_allocations: 64,
                output_bytes: 64 << 20,
            },
            capture: Default::default(),
        }
    }

    fn send(&self, work: Work) -> Result<(), String> {
        self.work
            .send(work)
            .map_err(|_| "Environments are not running right now.".to_owned())
    }

    /// A message from the person: an answer, steering, or (after a stop or
    /// a result) a request to keep going.
    pub fn steer(&self, id: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > MAX_MESSAGE {
            return Err("Write a message of up to 4,000 characters.".into());
        }
        match self.state(id).map(|s| s.phase) {
            None | Some(Phase::Starting) => {
                return Err("You can send a message once the computer is ready.".into());
            }
            Some(Phase::Building | Phase::Verifying) => {
                return Err("You can send a message when the check finishes.".into());
            }
            _ => {}
        }
        self.logs
            .push(id, Entry::User { text: text.into() }, now_ms())?;
        self.send(Work::Steer(id.into(), text.into()))
    }

    /// Try again after a stop.
    pub fn retry(&self, id: &str) -> Result<(), String> {
        if !matches!(self.state(id).map(|s| s.phase), Some(Phase::Failed { .. })) {
            return Err("There is nothing to retry.".into());
        }
        self.logs.push(id, Entry::Retried, now_ms())?;
        self.send(Work::Retry(id.into()))
    }

    /// Save the candidate the person reviewed (`candidate` is the digest
    /// the page showed). Returns the new version number.
    pub async fn save(&self, id: &str, candidate: &str) -> Result<u64, String> {
        let (reply, answer) = oneshot::channel();
        self.send(Work::Save(id.into(), candidate.into(), reply))?;
        answer
            .await
            .map_err(|_| "Environments are not running right now.".to_owned())?
            .map_err(|e| plain(&e, "Saving didn't go through. Try again."))
    }

    /// Start a Claude Code run on the environment's saved version.
    pub fn run_claude(&self, id: &str, prompt: &str) -> Result<String, String> {
        let env = self.envs().read(id).map_err(|e| e.to_string())?;
        if env.project.workspace != self.config.owner.workspace {
            return Err("That environment isn't yours.".into());
        }
        let key = Config::secret(&self.config.claude_key)
            .ok_or("Add your Anthropic API key to run Claude Code here.")?;
        self.runs
            .start(
                &env,
                prompt,
                &self.config.machines.workdir,
                &self.config.size(),
                Some(key),
            )
            .map_err(|e| plain(&e, "Claude Code didn't start. Try again."))
    }

    pub fn claude_run(&self, id: &str, run: &str) -> Option<claude::Run> {
        self.runs.read(id, run)
    }

    pub fn stop_claude(&self, id: &str, run: &str) -> Result<(), String> {
        self.runs.stop(id, run)
    }
}

/// A project identity from a repository name.
fn project_id(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches('-');
    if s.is_empty() {
        "repository".into()
    } else {
        s.chars().take(64).collect()
    }
}

/// The admitted Coder/Codex profile a setup session runs under: this
/// environment's project and exact source on a dedicated Boat machine,
/// with the Codex login and only the configured GitHub credential.
fn profile(config: &Config, env: &Environment) -> coder_cloud::operator::Profile {
    let mut credentials = BTreeMap::from([(
        "OA_CODEX_AUTH".to_owned(),
        config
            .codex_home
            .clone()
            .map(|h| h.join("auth.json"))
            .or_else(codex_transport::codex::Login::default_path)
            .unwrap_or_else(|| PathBuf::from("/nonexistent/auth.json")),
    )]);
    if let Some(g) = config.git_credential() {
        credentials.insert(g.clone(), PathBuf::from(format!("/env/{g}")));
    }
    coder_cloud::operator::Profile {
        workspace: env.project.workspace.clone(),
        project: env.project.project.clone(),
        cwd: config.state.clone(),
        source_revision: env.source.revision.clone(),
        source_digest: env.source.digest.clone(),
        repository: env.source.repository.clone(),
        branch: None,
        paths: vec![],
        include: vec![],
        pool: "boat".into(),
        placement: coder_cloud::Placement::Boat,
        mode: coder_cloud::Mode::Coder,
        executor: coder_environment_setup::SETUP_ENGINE.into(),
        model: None,
        reasoning: None,
        max_timeout_seconds: coder_environment_setup::MAX_DEADLINE_SECONDS,
        size: config.size(),
        template: config.machines.template.clone(),
        credentials,
        adapter: coder_cloud::operator::Adapter::Boat {
            origin: std::env::var("BOAT_API_BASE").unwrap_or_else(|_| "https://boat".into()),
            token_file: PathBuf::from("/env/BOAT_API_KEY"),
        },
    }
}

fn brief(config: &Config, env: &Environment, meta: Option<&Meta>) -> Brief {
    let git = config.git_credential();
    let repository = meta
        .map(|m| m.repository.clone())
        .or_else(|| env.source.repository.clone())
        .unwrap_or_default();
    let branch = meta.map(|m| m.branch.clone()).unwrap_or_default();
    Brief {
        environment: env.id.clone(),
        owner: config.owner.clone(),
        profile_alias: PROFILE_ALIAS.into(),
        profile: profile(config, env),
        credential_names: git.iter().cloned().collect::<BTreeSet<_>>(),
        git_credential: git,
        deadline_seconds: config.deadline(),
        size: config.size(),
        objective: format!(
            "Set up {repository} so cloud agents can build and test it. Make an install recipe that works on a clean machine."
        ),
        context: format!(
            "Repository: {repository}\nBranch: {branch}\nCommit: {}",
            env.source.revision
        ),
    }
}

/// The owner thread: runs setups and applies the person's requests.
async fn serve<P, T>(
    config: Config,
    owners: Arc<Owners<P>>,
    logs: Arc<Logs>,
    transports: Transports<T>,
    poll: Duration,
    mut inbox: mpsc::UnboundedReceiver<Work>,
) where
    P: Commands + Images + 'static,
    T: Transport + 'static,
{
    let running: std::rc::Rc<std::cell::RefCell<BTreeSet<String>>> = Default::default();
    let again: std::rc::Rc<std::cell::RefCell<BTreeSet<String>>> = Default::default();
    let root = config.state.join("environment-studio");
    let config = Arc::new(config);
    // An agent with no transport, for loading and saving state here.
    let records = |owners: &Arc<Owners<P>>| Agent::<P, Never> {
        owners: owners.clone(),
        transport: Never,
        model: String::new(),
        logs: logs.clone(),
        root: root.clone(),
        poll,
    };
    let launch = |id: String| {
        if !running.borrow_mut().insert(id.clone()) {
            again.borrow_mut().insert(id);
            return;
        }
        let owners = owners.clone();
        let logs = logs.clone();
        let root = root.clone();
        let config = config.clone();
        let transports = transports.clone();
        let running = running.clone();
        let again = again.clone();
        tokio::task::spawn_local(async move {
            loop {
                let env = owners.setup.environments.read(&id);
                let meta = fs::read(root.join(&id).join("meta.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<Meta>(&b).ok());
                match (env, transports(&id)) {
                    (Ok(env), Ok(transport)) => {
                        let agent = Agent {
                            owners: owners.clone(),
                            transport,
                            model: config.model(),
                            logs: logs.clone(),
                            root: root.clone(),
                            poll,
                        };
                        let state = agent.load(&id).unwrap_or_else(|| State::new(&id));
                        agent.run(&brief(&config, &env, meta.as_ref()), state).await;
                    }
                    (Err(e), _) => {
                        let _ = logs.push(
                            &id,
                            Entry::Failed {
                                reason: e.to_string(),
                            },
                            now_ms(),
                        );
                    }
                    (_, Err(reason)) => {
                        let agent = Agent::<P, Never> {
                            owners: owners.clone(),
                            transport: Never,
                            model: String::new(),
                            logs: logs.clone(),
                            root: root.clone(),
                            poll,
                        };
                        let mut state = agent.load(&id).unwrap_or_else(|| State::new(&id));
                        let _ = logs.push(
                            &id,
                            Entry::Failed {
                                reason: reason.clone(),
                            },
                            now_ms(),
                        );
                        state.phase = Phase::Failed { reason };
                        let _ = agent.save(&state);
                    }
                }
                if !again.borrow_mut().remove(&id) {
                    break;
                }
            }
            running.borrow_mut().remove(&id);
        });
    };
    while let Some(work) = inbox.recv().await {
        match work {
            Work::Run(id) => launch(id),
            Work::Steer(id, text) => {
                let store = records(&owners);
                let Some(mut state) = store.load(&id) else {
                    continue;
                };
                match state.phase {
                    Phase::Working | Phase::Waiting { .. } => {
                        // Retained by the setup owner; the loop hands it
                        // to the model before its next request.
                        if let Err(e) = owners.setup.steer(&state.session(), &text, now_ms()).await
                        {
                            let _ = logs.push(
                                &id,
                                Entry::Failed {
                                    reason: format!("Your message didn't reach the setup: {e}"),
                                },
                                now_ms(),
                            );
                        }
                        launch(id);
                    }
                    Phase::Review { .. } | Phase::Saved { .. } | Phase::Failed { .. } => {
                        if running.borrow().contains(&id) {
                            continue;
                        }
                        state.reopen(&text);
                        let _ = store.save(&state);
                        launch(id);
                    }
                    Phase::Starting | Phase::Building | Phase::Verifying => {}
                }
            }
            Work::Retry(id) => {
                if running.borrow().contains(&id) {
                    continue;
                }
                let store = records(&owners);
                if let Some(mut state) = store.load(&id) {
                    state.retry();
                    let _ = store.save(&state);
                    launch(id);
                }
            }
            Work::Save(id, candidate, reply) => {
                let store = records(&owners);
                let result = save(&config, &owners, &store, &id, &candidate);
                if let Ok(number) = &result {
                    let _ = logs.push(&id, Entry::Saved { number: *number }, now_ms());
                }
                let _ = reply.send(result);
            }
        }
    }
}

fn user_message(text: &str) -> serde_json::Value {
    serde_json::json!({"role":"user","content":[{"type":"input_text","text":text}]})
}

fn save<P: Commands + Images, T: Transport>(
    config: &Config,
    owners: &Owners<P>,
    store: &Agent<P, T>,
    id: &str,
    shown: &str,
) -> Result<u64, String> {
    let mut state = store
        .load(id)
        .ok_or("This environment has nothing to save.")?;
    let Phase::Review { verification } = state.phase.clone() else {
        return Err("There is no checked result to save right now.".into());
    };
    let env = owners
        .setup
        .environments
        .read(id)
        .map_err(|e| e.to_string())?;
    let candidate = env
        .propose(&verification)
        .map_err(|e| format!("This result can't be saved: {e}"))?;
    if candidate.digest() != shown {
        return Err("The result changed since you looked. Review it again.".into());
    }
    let now = now_ms();
    let review = Review {
        id: format!("review-{verification}"),
        actor: config.owner.principal.clone(),
        candidate,
        granted_ms: now,
        expires_ms: now + 3_600_000,
    };
    owners
        .setup
        .environments
        .apply(
            id,
            &Command::Promote {
                request_id: format!("save-{verification}"),
                expected_selection_revision: env.selection.revision,
                review,
            },
            now,
        )
        .map_err(|e| format!("Saving didn't go through: {e}"))?;
    let env = owners
        .setup
        .environments
        .read(id)
        .map_err(|e| e.to_string())?;
    let number = env
        .active()
        .map(|v| v.number)
        .ok_or("Saving didn't go through.")?;
    state.phase = Phase::Saved { number };
    store.save(&state)?;
    Ok(number)
}

/// A transport that never answers: for loading and saving agent state.
pub struct Never;
impl Transport for Never {
    async fn respond(
        &self,
        _request: &codex_transport::Request,
    ) -> Result<codex_transport::Reply, codex_transport::TransportError> {
        Err(codex_transport::TransportError::Exhausted)
    }
}

impl State {
    /// Keep going after a result or a stop: a new setup session that keeps
    /// the conversation and starts from the person's message.
    pub fn reopen(&mut self, text: &str) {
        self.attempt += 1;
        self.phase = Phase::Starting;
        self.seen_steering = 0;
        self.installed = None;
        self.build_job = None;
        self.verify_job = None;
        self.summary = None;
        self.input.push(user_message(&format!(
            "{text}\n\n(You are on a new setup computer with the same commit checked out. Any recipe draft and declared checks are kept.)"
        )));
    }
}

#[cfg(test)]
mod tests;
