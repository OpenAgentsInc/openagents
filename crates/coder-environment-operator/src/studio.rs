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
pub mod leftovers;
pub mod push;
pub mod sign_in;

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
    /// The setup agent's model through an Open Responses endpoint with an
    /// API key (such as the OpenAgents gateway on the same host) instead
    /// of a Codex login. A server with no person to keep a Codex login
    /// fresh (Cloud Run) uses this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_api: Option<ModelApi>,
    /// Days a stopped computer that is not the person's own signed-in copy
    /// is kept before it is deleted (#11296); default
    /// [`sign_in::DEFAULT_RETENTION_DAYS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home_retention_days: Option<u64>,
}

/// An Open Responses endpoint for the setup agent ([`Config::model_api`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelApi {
    /// The full responses address, such as
    /// `http://127.0.0.1:8791/v1/responses`.
    pub url: String,
    /// A private file holding the API key; read when the studio opens.
    pub key_file: PathBuf,
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
        if self.home_retention_days.is_some_and(|d| d == 0 || d > 365) {
            return Err("home_retention_days must be 1 to 365.".into());
        }
        if let Some(api) = &self.model_api
            && (!(api.url.starts_with("https://") || api.url.starts_with("http://"))
                || !api.key_file.is_absolute())
        {
            return Err("model_api needs an http(s) url and an absolute key_file.".into());
        }
        Ok(())
    }
    fn model(&self) -> String {
        self.model.clone().unwrap_or_else(|| DEFAULT_MODEL.into())
    }
    fn size(&self) -> String {
        self.size.clone().unwrap_or_else(|| DEFAULT_SIZE.into())
    }
    fn retention_ms(&self) -> u64 {
        self.home_retention_days
            .unwrap_or(sign_in::DEFAULT_RETENTION_DAYS)
            .saturating_mul(86_400_000)
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
    /// The account the environment was made for; `None` for one made on a
    /// server without sign-in.
    pub account: Option<String>,
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
    /// The signed-in account it was made for (absent on records made
    /// before environments had owners, and on servers without sign-in).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    account: Option<String>,
}

enum Work {
    Run(String),
    Steer(String, String),
    Retry(String),
    Save(String, String, oneshot::Sender<Result<u64, String>>),
}

/// How often the studio looks for computers to delete (#11296).
const SWEEP_EVERY: Duration = Duration::from_secs(60 * 60);

/// Sweeps at start and every [`SWEEP_EVERY`] while the studio lives.
fn spawn_sweeper(studio: std::sync::Weak<Studio>) {
    let _ = std::thread::Builder::new()
        .name("environment-computer-sweep".into())
        .spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            loop {
                let Some(live) = studio.upgrade() else { return };
                runtime.block_on(async {
                    if let Ok(client) = boat::Client::from_env().await {
                        live.sweep_homes(&sign_in::Boat { client }).await;
                    }
                });
                drop(live);
                std::thread::sleep(SWEEP_EVERY);
            }
        });
}

/// Makes a model transport for one environment's setup.
pub type Transports<T> = Arc<dyn Fn(&str) -> Result<T, String> + Send + Sync>;

/// The Environments service the web holds.
/// A Claude Code turn on a chat's own computer ([`Studio::chat_turn`]).
/// `env` holds secret values; they never print and are zeroed on drop.
pub struct ChatTurn {
    pub environment: String,
    pub version: u64,
    pub version_id: String,
    /// The saved version's image (a Boat named snapshot).
    pub image: String,
    pub size: String,
    /// The turn's shell command; it holds no secret.
    pub command: String,
    pub env: Vec<(String, String)>,
}

impl Drop for ChatTurn {
    fn drop(&mut self) {
        for (_, value) in &mut self.env {
            let mut bytes = std::mem::take(value).into_bytes();
            bytes.fill(0);
        }
    }
}

impl std::fmt::Debug for ChatTurn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChatTurn")
            .field("environment", &self.environment)
            .field("version", &self.version)
            .field("image", &self.image)
            .finish_non_exhaustive()
    }
}

pub struct Studio {
    config: Config,
    layout: crate::Layout,
    logs: Arc<Logs>,
    runs: claude::Runs,
    sign_ins: sign_in::Sessions,
    homes: sign_in::Homes,
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
        let template = machines
            .template
            .clone()
            .unwrap_or_else(|| "boat-default".into());
        if let Some(api) = config.model_api.clone() {
            let key = fs::read_to_string(&api.key_file)
                .map_err(|_| "The model API key file is unreadable.".to_owned())?;
            let key = key.trim().to_owned();
            codex_transport::codex::CodexTransport::with_key(&api.url, key.clone(), "check")
                .map_err(|e| format!("The model API can't be used: {e}"))?;
            let transports: Transports<codex_transport::codex::CodexTransport> =
                Arc::new(move |env: &str| {
                    codex_transport::codex::CodexTransport::with_key(
                        &api.url,
                        key.clone(),
                        &format!("environment-{env}"),
                    )
                    .map_err(|e| format!("The model API can't be used: {e}"))
                });
            let mut config = config;
            config.machines = machines;
            return Self::start(
                config,
                &template,
                providers,
                crate::environment_custody(),
                transports,
                Duration::from_secs(1),
            );
        }
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
            sign_ins: sign_in::Sessions::default(),
            homes: sign_in::Homes::under(sign_in::root(&config.state)),
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
        // And every Claude Code run that was mid-turn (#11331).
        studio.runs.pick_up_after_restart();
        spawn_sweeper(Arc::downgrade(&studio));
        Ok(studio)
    }

    /// Delete the environment computers no longer anyone's own copy
    /// (#11296, [`sign_in::sweep`]); how many were deleted.
    pub async fn sweep_homes<M: sign_in::Machines>(&self, machines: &M) -> usize {
        let envs = self.envs();
        let current = |id: &str| {
            envs.read(id)
                .ok()
                .and_then(|env| env.pin())
                .map(|pin| pin.number)
        };
        let busy = |id: &str| {
            self.sign_ins
                .view(id)
                .is_some_and(|view| !view.phase.finished())
        };
        let mut deleted = sign_in::sweep(
            machines,
            &self.homes,
            &current,
            &busy,
            crate::now_ms(),
            self.config.retention_ms(),
        )
        .await;
        // The computers setups, image builds, fresh-machine checks, and
        // Claude Code runs left stopped (#11317); never a signed-in one.
        let keep: BTreeSet<String> = self
            .homes
            .all()
            .into_iter()
            .flat_map(|(_, home)| std::iter::once(home.sandbox).chain(home.retired))
            .collect();
        for id in leftovers::finished(&self.config.state, crate::now_ms(), &keep) {
            if let Ok(Some(state)) = machines.state(&id).await
                && state == "stopped"
                && machines.delete(&id).await.is_ok()
            {
                eprintln!(
                    "environments: deleted the stopped computer {id}, which nothing uses again"
                );
                deleted += 1;
            }
        }
        deleted
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

    /// The Open Responses endpoint the setup agent uses
    /// ([`Config::model_api`]), as `(url, key, model)`, for other model
    /// calls this server makes on its own key, such as answering about the
    /// images and PDFs sent to a chat. `None` without one, or when its key
    /// file can't be read now.
    pub fn model_api(&self) -> Option<(String, String, String)> {
        let api = self.config.model_api.as_ref()?;
        let key = fs::read_to_string(&api.key_file).ok()?;
        let key = key.trim();
        (!key.is_empty()).then(|| (api.url.clone(), key.to_owned(), self.config.model()))
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
            account: meta.account,
        })
    }

    /// One environment's page.
    /// Resolve a saved version for allocation without the page's history limit.
    pub fn version_image(&self, id: &str, number: u64) -> Option<coder_environment::ImageIdentity> {
        let env = self.envs().read(id).ok()?;
        if env.project.workspace != self.config.owner.workspace || env.retired_ms.is_some() {
            return None;
        }
        env.versions
            .iter()
            .find(|v| v.number == number)
            .map(|v| v.image.clone())
    }

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

    /// Read the account's selected, saved version for a computer job.
    pub fn computer_pin(
        &self,
        account: &str,
        id: &str,
        number: u64,
    ) -> Result<
        (
            coder_environment::ProjectLink,
            coder_environment::VersionPin,
        ),
        String,
    > {
        if self.account(id).as_deref() != Some(account) {
            return Err("No such environment version.".into());
        }
        let env = self.envs().read(id).map_err(|e| e.to_string())?;
        if env.project.workspace != self.config.owner.workspace {
            return Err("No such environment version.".into());
        }
        let pin = env
            .pin()
            .ok_or("Save the environment before starting a job.")?;
        if pin.number != number {
            return Err("Choose the environment's selected version.".into());
        }
        Ok((env.project.clone(), pin))
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
        self.create_for(resolved, None)
    }

    /// [`Studio::create`] for the signed-in `account`, which then owns it
    /// ([`Studio::account`]).
    pub fn create_for(
        &self,
        resolved: &github::Resolved,
        account: Option<&str>,
    ) -> Result<String, String> {
        self.add(resolved, account)
            .map_err(|e| plain(&e, "The environment couldn't be added. Try again."))
    }

    /// The account an environment was made for, when it has one.
    pub fn account(&self, id: &str) -> Option<String> {
        self.meta(id)?.account
    }

    fn add(&self, resolved: &github::Resolved, account: Option<&str>) -> Result<String, String> {
        let now = now_ms();
        let full = resolved.repository.full();
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| "The environment couldn't be added. Try again.")?;
        let id = format!(
            "env-{}",
            nonce
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
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
        fs::create_dir_all(
            dir.parent()
                .ok_or("The environment folder couldn't be opened.")?,
        )
        .map_err(|e| format!("Cannot keep the environment: {e}"))?;
        // Reserve the new directory before writing ownership metadata. A
        // collision must never replace another environment's account.
        fs::create_dir(&dir).map_err(|e| format!("Cannot keep the environment: {e}"))?;
        let meta = Meta {
            repository: full.clone(),
            branch: resolved.branch.clone(),
            created_ms: now,
            account: account.map(str::to_owned),
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

    /// Start a Claude Code run on the environment's saved version. `own`
    /// is the signed-in person's own Claude credential, released for this
    /// run; without one, the run uses the key this server's environment
    /// names ([`Config::claude_key`]).
    pub fn run_claude(
        &self,
        id: &str,
        prompt: &str,
        own: Option<claude::Key>,
    ) -> Result<String, String> {
        self.run_claude_with_files(id, prompt, own, Vec::new())
    }

    /// [`Self::run_claude`] with `files` put in the computer's working
    /// directory before Claude Code starts ([`claude::Attachment`]).
    pub fn run_claude_with_files(
        &self,
        id: &str,
        prompt: &str,
        own: Option<claude::Key>,
        files: Vec<claude::Attachment>,
    ) -> Result<String, String> {
        self.run_claude_pushing(id, prompt, own, files, None)
    }

    /// [`Self::run_claude_with_files`], with GitHub access for the
    /// environment's repository when `push` is given ([`push::Push`]).
    pub fn run_claude_pushing(
        &self,
        id: &str,
        prompt: &str,
        own: Option<claude::Key>,
        files: Vec<claude::Attachment>,
        push: Option<push::Push>,
    ) -> Result<String, String> {
        let env = self.envs().read(id).map_err(|e| e.to_string())?;
        if env.project.workspace != self.config.owner.workspace {
            return Err("That environment isn't yours.".into());
        }
        // Access for another repository is never handed to this run.
        let push = push.filter(|push| {
            env.source
                .repository
                .as_deref()
                .is_some_and(|repo| repo.eq_ignore_ascii_case(push.repository()))
        });
        // The person's own key first; then the environment's computer they
        // signed in to Claude on (#11294); then this server's key.
        if own.is_none()
            && let Some(home) = self.claude_home(&env)
        {
            return self
                .runs
                .start_on(
                    &env,
                    prompt,
                    &self.config.machines.workdir,
                    &self.config.size(),
                    None,
                    files,
                    push,
                    Some(home.sandbox),
                )
                .map_err(|e| plain(&e, "Claude Code didn't start. Try again."));
        }
        let key = match own {
            Some(key) => key,
            // The server's own key may be an API key or a subscription
            // token; it runs under the variable its prefix names.
            None => Config::secret(&self.config.claude_key)
                .and_then(|value| {
                    let name = coder_cloud::claude::detect(&value)
                        .map_or(coder_cloud::claude::API_KEY, |class| class.name());
                    claude::Key::new(name, value).ok()
                })
                .ok_or("Add your Claude key in Settings to run Claude Code here.")?,
        };
        self.runs
            .start_pushing(
                &env,
                prompt,
                &self.config.machines.workdir,
                &self.config.size(),
                Some(key),
                files,
                push,
            )
            .map_err(|e| plain(&e, "Claude Code didn't start. Try again."))
    }

    /// A Claude Code turn on a chat's own computer (#11624): the saved
    /// version's image the computer starts from, the command the turn runs
    /// there, and its per-turn environment. The environment holds the
    /// task, the Claude credential (the person's own released for this
    /// turn, else this server's key) and the GitHub broker ticket; it is
    /// handed to the computer for this one command, in memory, never
    /// written anywhere. Files sent with a message are not carried yet.
    pub fn chat_turn(
        &self,
        id: &str,
        prompt: &str,
        own: Option<claude::Key>,
        push: Option<push::Push>,
        session: &str,
    ) -> Result<ChatTurn, String> {
        let env = self.envs().read(id).map_err(|e| e.to_string())?;
        if env.project.workspace != self.config.owner.workspace {
            return Err("That environment isn't yours.".into());
        }
        let pin = env
            .pin()
            .ok_or("Save the environment before running Claude Code on it.")?;
        let prompt = prompt.trim();
        if prompt.is_empty() || prompt.len() > claude::MAX_PROMPT {
            return Err("Write what Claude Code should do, up to 16 KB.".into());
        }
        let push = push.filter(|push| {
            env.source
                .repository
                .as_deref()
                .is_some_and(|repo| repo.eq_ignore_ascii_case(push.repository()))
        });
        let key = match own {
            Some(key) => key,
            None => Config::secret(&self.config.claude_key)
                .and_then(|value| {
                    let name = coder_cloud::claude::detect(&value)
                        .map_or(coder_cloud::claude::API_KEY, |class| class.name());
                    claude::Key::new(name, value).ok()
                })
                .ok_or("Add your Claude key in Settings to run Claude Code here.")?,
        };
        if key.name() == coder_cloud::claude::VERTEX {
            return Err("A Vertex credential can't run on a chat's computer yet; add an Anthropic key or Claude token in Settings.".into());
        }
        let workdir = self.config.machines.workdir.clone();
        let mut task = claude::task(prompt, &workdir);
        task.push_str(&push::task_note(push.as_ref()));
        let mut names = vec![key.name().to_owned()];
        let mut values = vec![
            ("OA_TURN_TASK".to_owned(), task),
            (key.name().to_owned(), key.expose().to_owned()),
        ];
        let setup = match &push {
            Some(push) => {
                names.push(push.name().to_owned());
                values.push((push.name().to_owned(), push.value()));
                format!(
                    "( {} ) >/dev/null || {{ echo 'GitHub access could not be set up.' >&2; exit 1; }}\n",
                    push::setup_script(push, &workdir, push::GITHUB_API)
                )
            }
            None => String::new(),
        };
        let q = boat::shell_quote;
        let session: String = session
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .take(96)
            .collect();
        let command = format!(
            r#"set -u
umask 077
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:/usr/local/bin:$PATH"
p=$(command -v coder-cloud-runtime || command -v openagents || true)
[ -n "$p" ] || {{ echo 'This computer has no Coder runtime.' >&2; exit 1; }}
s="$HOME/.oa-chat/state"
mkdir -p "$s"
{setup}export OA_CODER_CLOUD_CREDENTIAL_NAMES={names}
export OA_ENGINE_NOTICE_DIR="$HOME/.openagents/engine"
"$p" --json coder --in {workdir} --state "$s" delegate claude --task "$OA_TURN_TASK" --session {session}
"#,
            names = q(&names.join(",")),
            workdir = q(&workdir),
            session = q(&format!("chat-{session}")),
        );
        Ok(ChatTurn {
            environment: env.id.clone(),
            version: pin.number,
            version_id: pin.version_id.clone(),
            image: pin.image.image_id.clone(),
            size: self.config.size(),
            command,
            env: values,
        })
    }

    /// The environment's computer signed in to Claude for its selected
    /// version, if the person signed in there (#11294).
    fn claude_home(&self, env: &Environment) -> Option<sign_in::Home> {
        let pin = env.pin()?;
        self.homes.signed_in(&env.id, pin.number)
    }

    /// Whether Claude Code can run in environment `id` on the login the
    /// person made inside it.
    pub fn claude_signed_in(&self, id: &str) -> bool {
        self.envs()
            .read(id)
            .ok()
            .filter(|env| env.project.workspace == self.config.owner.workspace)
            .and_then(|env| self.claude_home(&env))
            .is_some()
    }

    /// Open Claude Code's own sign-in on the environment's computer: made
    /// from its saved version the first time, started again after. The
    /// web follows it with [`Self::sign_in_view`] and types into it with
    /// [`Self::sign_in_send`].
    pub fn start_sign_in(&self, id: &str) -> Result<(), String> {
        let env = self.envs().read(id).map_err(|e| e.to_string())?;
        if env.project.workspace != self.config.owner.workspace {
            return Err("That environment isn't yours.".into());
        }
        let pin = env
            .pin()
            .ok_or("Save the environment before signing in to Claude there.")?;
        let job = sign_in::Job {
            environment: env.id.clone(),
            image: pin.image.image_id.clone(),
            version: pin.number,
            size: self.config.size(),
        };
        let input = self.sign_ins.begin(&env.id)?;
        let (sessions, homes) = (self.sign_ins.clone(), self.homes.clone());
        std::thread::Builder::new()
            .name(format!("claude-sign-in-{}", env.id))
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async {
                    match boat::Client::from_env().await {
                        Ok(client) => {
                            sign_in::drive(
                                &sign_in::Boat { client },
                                &homes,
                                &sessions,
                                &job,
                                input,
                                Duration::from_secs(1),
                            )
                            .await;
                        }
                        Err(_) => sessions.set(&job.environment, |v| {
                            v.phase = sign_in::Phase::Failed(
                                "Computers aren't available right now. Try again in a minute."
                                    .into(),
                            );
                        }),
                    }
                });
            })
            .map_err(|_| "Sign-in couldn't start. Try again.")?;
        Ok(())
    }

    /// The environment's sign-in as it stands, if one was started.
    pub fn sign_in_view(&self, id: &str) -> Option<sign_in::View> {
        self.sign_ins.view(id)
    }

    /// Type a line into the environment's open sign-in.
    pub fn sign_in_send(&self, id: &str, text: &str) -> Result<(), String> {
        self.sign_ins.send(id, text)
    }

    /// The environment's Claude Code runs, newest first.
    pub fn claude_runs(&self, id: &str) -> Vec<claude::Run> {
        self.runs.list(id)
    }

    pub fn claude_run(&self, id: &str, run: &str) -> Option<claude::Run> {
        self.runs.read(id, run)
    }

    pub fn stop_claude(&self, id: &str, run: &str) -> Result<(), String> {
        self.runs.stop(id, run)
    }

    /// Send a run a message for its next turn ([`claude::Runs::message`]).
    pub fn message_claude(&self, id: &str, run: &str, text: &str) -> Result<claude::Sent, String> {
        self.runs.message(id, run, text)
    }

    /// The messages waiting for a run's next turn.
    pub fn claude_waiting(&self, id: &str, run: &str) -> Vec<String> {
        self.runs.waiting(id, run)
    }

    /// What a paused run offers the person whose saved Claude credential
    /// is `saved` (#11293).
    pub fn claude_choices(
        &self,
        id: &str,
        run: &str,
        saved: Option<coder_cloud::claude::OwnCredential>,
    ) -> Vec<coder_cloud::claude_task::PauseChoice> {
        self.runs.choices(id, run, saved)
    }

    /// Apply the person's choice to a paused run ([`claude::Runs::choose`]).
    /// `own` is their own saved credential, used only to continue on it.
    pub fn choose_claude(
        &self,
        id: &str,
        run: &str,
        choice: coder_cloud::claude_task::PauseChoice,
        own: Option<claude::Key>,
    ) -> Result<claude::Chosen, String> {
        self.runs.choose(id, run, choice, own)
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
                let saved = result.is_ok();
                if let Ok(number) = &result {
                    let _ = logs.push(&id, Entry::Saved { number: *number }, now_ms());
                }
                let _ = reply.send(result);
                if saved {
                    // Images older checks left unsaved go now.
                    let owners = owners.clone();
                    let root = root.clone();
                    tokio::task::spawn_local(async move {
                        crate::images::sweep(&owners, &root, &id).await;
                    });
                }
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
