//! The runner's configuration, read from its environment file on the host
//! (`deploy/eval-runner/eval-runner.env.example`). Keys stay in that file
//! and in memory; nothing here prints one.

use std::path::{Path, PathBuf};

use ext_eval::proxy::Secret;
use ext_eval::run::{DecisionPin, Door};

/// The runner's bounds beyond the hosted request's own.
///
/// The owner decided on 2026-10-01 (#10120, #10121) that nothing in the app
/// has a usage limit, so neither daily count is set by default: every
/// trainer may run as often as they like, and every job is recorded in the
/// usage log ([`crate::usage`]) instead. The two counts stay only as an
/// emergency brake an operator may set; the shipped configuration sets
/// neither.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Emergency brake: suite runs one trainer may ask for per UTC day
    /// (`EVAL_RUNNER_RUNS_PER_DAY`). `None`, the default, is unlimited.
    /// Checks never count.
    pub runs_per_trainer: Option<u32>,
    /// Emergency brake: agent turns (cases × runs × arms) every trainer
    /// together may use per UTC day, checks included
    /// (`EVAL_RUNNER_TURNS_PER_DAY`). `None`, the default, is unlimited.
    pub turns_per_day: Option<u64>,
    /// Suites run at once.
    pub jobs: usize,
    /// Runs at once inside one suite, 1 to 8.
    pub concurrency: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            runs_per_trainer: None,
            turns_per_day: None,
            jobs: 2,
            concurrency: 4,
        }
    }
}

/// Everything the runner reads at start.
#[derive(Clone, Debug)]
pub struct Config {
    /// The relay it listens and publishes on.
    pub relay: String,
    /// The Blossom server suite files go to; `None` is the relay's own.
    /// With [`Config::bucket`], the bucket's public read base.
    pub blossom: Option<String>,
    /// A `gs://` bucket that holds suite files instead of a Blossom
    /// server, and the gcloud configuration directory that may write it.
    pub bucket: Option<(String, PathBuf)>,
    /// The file holding the runner's secret key, 64 hex.
    pub key_file: PathBuf,
    /// Where the runner keeps its ledger, counts, jobs, results, and
    /// usage log (`usage/`).
    pub state: PathBuf,
    /// The catalog: extension directories with a package record.
    pub catalog: Vec<PathBuf>,
    /// The agent binary both arms run (`coder`).
    pub coder: PathBuf,
    /// The question sets the agent asks its decision door.
    pub questions: PathBuf,
    /// The chat door both arms answer through.
    pub door: Door,
    /// The decision door for `decision` graders and the child's classifier.
    pub decision: Option<DecisionPin>,
    /// Bounds.
    pub limits: Limits,
    /// Where run directories are made.
    pub temp_root: PathBuf,
    /// The root key of the `coder-defaults` package whose releases both
    /// arms admit: the package record's, unless `EVAL_RUNNER_DEFAULTS_ROOT`
    /// names another (a test's operator).
    pub defaults_root: String,
    /// Where the documents a defaults release pins are read first
    /// (`EVAL_RUNNER_DEFAULTS_DOCS`, default
    /// `~/.openagents/coder-defaults/documents`); the relay's locators
    /// fill in the rest.
    pub defaults_documents: Option<PathBuf>,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn number<T: std::str::FromStr>(name: &str, default: T) -> Result<T, String> {
    match var(name) {
        None => Ok(default),
        Some(text) => text
            .parse()
            .map_err(|_| format!("{name} must be a number, not {text:?}")),
    }
}

/// An emergency brake: unset is `None` (unlimited); set, a positive whole
/// number. A zero or a typo stops the runner rather than closing it.
fn brake<T: std::str::FromStr + PartialEq + Default>(name: &str) -> Result<Option<T>, String> {
    match var(name) {
        None => Ok(None),
        Some(text) => match text.parse::<T>() {
            Ok(value) if value != T::default() => Ok(Some(value)),
            _ => Err(format!(
                "{name} is an emergency brake: leave it unset for no limit, or set a positive number, not {text:?}"
            )),
        },
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map_or_else(|| PathBuf::from("."), PathBuf::from)
}

impl Config {
    /// The configuration in this process's environment.
    ///
    /// # Errors
    ///
    /// Names the first variable that is missing or wrong.
    pub fn from_env() -> Result<Self, String> {
        let relay =
            var("EVAL_RUNNER_RELAY").unwrap_or_else(|| nostr::eval_ext::hosted::RELAY.to_string());
        let catalog: Vec<PathBuf> = var("EVAL_RUNNER_CATALOG")
            .ok_or("EVAL_RUNNER_CATALOG names no extension directory")?
            .split(':')
            .filter(|part| !part.is_empty())
            .map(PathBuf::from)
            .collect();
        let coder = PathBuf::from(var("EVAL_RUNNER_CODER").ok_or("EVAL_RUNNER_CODER is not set")?);
        let questions =
            PathBuf::from(var("EVAL_RUNNER_QUESTIONS").ok_or("EVAL_RUNNER_QUESTIONS is not set")?);
        let key = var("CODER_DOOR_KEY")
            .or_else(|| var("CODER_AI_GATEWAY_KEY"))
            .ok_or("no door key: set CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY")?;
        let door = Door {
            name: "default".into(),
            url: var("CODER_DOOR_URL").unwrap_or_else(|| "https://ai-gateway.vercel.sh".into()),
            key: Secret::new(key),
            model: var("CODER_MODEL").unwrap_or_else(|| "google/gemini-3.8-flash".into()),
        };
        let decision = var("TYPESAFE_API_KEY").map(|key| DecisionPin {
            url: var("TYPESAFE_BASE_URL").unwrap_or_else(|| "https://api.typesafe.ai".into()),
            key: Secret::new(key),
        });
        let limits = Limits {
            runs_per_trainer: brake("EVAL_RUNNER_RUNS_PER_DAY")?,
            turns_per_day: brake("EVAL_RUNNER_TURNS_PER_DAY")?,
            jobs: number("EVAL_RUNNER_JOBS", 2)?,
            concurrency: number("EVAL_RUNNER_CONCURRENCY", 4)?,
        };
        if !ext_eval::run::CONCURRENCY.contains(&limits.concurrency) {
            return Err("EVAL_RUNNER_CONCURRENCY must be from 1 to 8".into());
        }
        if limits.jobs == 0 {
            return Err("EVAL_RUNNER_JOBS must be at least 1".into());
        }
        Ok(Self {
            relay,
            blossom: var("EVAL_RUNNER_BLOSSOM"),
            bucket: match (var("EVAL_RUNNER_BUCKET"), var("EVAL_RUNNER_GCLOUD_CONFIG")) {
                (Some(bucket), Some(config)) => Some((bucket, PathBuf::from(config))),
                (None, None) => None,
                _ => {
                    return Err(
                        "EVAL_RUNNER_BUCKET and EVAL_RUNNER_GCLOUD_CONFIG go together".into(),
                    );
                }
            },
            key_file: var("EVAL_RUNNER_KEY_FILE").map_or_else(
                || home().join(".openagents/nostr/eval-runner-key"),
                PathBuf::from,
            ),
            state: var("EVAL_RUNNER_STATE")
                .map_or_else(|| home().join(".openagents/eval-runner"), PathBuf::from),
            catalog,
            coder,
            questions,
            door,
            decision,
            limits,
            temp_root: var("EVAL_RUNNER_TMP").map_or_else(std::env::temp_dir, PathBuf::from),
            defaults_root: match var("EVAL_RUNNER_DEFAULTS_ROOT") {
                Some(root) if root.len() == 64 && root.bytes().all(|b| b.is_ascii_hexdigit()) => {
                    root
                }
                Some(_) => {
                    return Err("EVAL_RUNNER_DEFAULTS_ROOT must be a 64-hex public key".into());
                }
                None => xp_ledger::adopt::root(),
            },
            defaults_documents: Some(var("EVAL_RUNNER_DEFAULTS_DOCS").map_or_else(
                || home().join(".openagents/coder-defaults/documents"),
                PathBuf::from,
            )),
        })
    }

    /// The file whose presence closes admission: every new run is refused
    /// `not_admitted` until it's removed. Runs already admitted finish.
    #[must_use]
    pub fn closed_flag(&self) -> PathBuf {
        self.state.join("closed")
    }

    /// The usage log's directory: `usage/` in the state directory.
    #[must_use]
    pub fn usage_dir(&self) -> PathBuf {
        self.state.join("usage")
    }

    /// Whether admission is closed.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed_flag().exists()
    }
}

/// Reads the runner's secret key from `path`: a private file holding 64
/// hex digits.
///
/// # Errors
///
/// When the file is missing, readable by others, or not a key.
pub fn load_identity(path: &Path) -> Result<coder::relay::Identity, String> {
    use std::os::unix::fs::PermissionsExt as _;
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("the runner key {}: {error}", path.display()))?;
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(format!(
            "the runner key {} is readable by others; chmod 600 it",
            path.display()
        ));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("the runner key {}: {error}", path.display()))?;
    coder::relay::Identity::from_text(&text, "the runner key")
}
