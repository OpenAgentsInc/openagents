//! Microcoder runs, read into the same [`Run`] the Harbor and Coder One
//! runs are read into.
//!
//! Microcoder (`crates/microcoder`) writes one directory per run under
//! `~/.openagents/microcoder/runs/<task>-<stamp>/`: `summary.json` when the
//! run ends and `events.jsonl` as it goes. The stamp is Unix seconds for
//! older runs and milliseconds since `4c749622f2`. Retained copies live in
//! the checkout under `bench/terminal-bench/microcoder-runs/<host>/`, one
//! directory per host with a `MANIFEST.json` that names every file's
//! SHA-256 and where each run's commit attribution comes from. A run in
//! more than one directory is read once, from the copy a manifest lists,
//! and a manifest's marks hold for every copy ([`Plan`]).
//!
//! Besides the task, model, reward, time, and cost every run has, a
//! Microcoder run carries the labels a claim about it must print
//! ([`Microcoder`]):
//!
//! - **Provider and cost basis.** A run through the Codex login reports GPT-6
//!   Luna's list price for the tokens it reported (`list_price`); a run
//!   through OpenRouter reports what OpenRouter billed (`billed`). A record
//!   that names neither and can't be told apart by its model name stays
//!   `unknown`. An unmeasured cost is `None`, never zero.
//! - **Knowledge.** Whether the run was knowledge-assisted, the entries its
//!   prompts listed or showed (ID, digest, and version when the record has
//!   them), and the retrieval mode.
//! - **In-sample.** A run is in-sample when an entry it used was written
//!   from its own task: the entry's `provenance.written_from`, in the
//!   checkout's `knowledge/*.md`, names a run of the task, with the
//!   task-name rule of [`knowledge::evidence::task_of`].
//! - **Commit.** The record's own `commit` when it has one; otherwise the
//!   retained manifest's attribution, labelled as such.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::runs::{Agent, Files, Outcome, Run, Tests};
use crate::runs_transcript::{Block, Kind, Transcript};

/// The job every Microcoder run is listed under: its ID is
/// `microcoder/<run directory>`.
pub const JOB: &str = "microcoder";

/// The retained Microcoder records in the checkout, one directory per host.
pub const RETAINED: &str = "bench/terminal-bench/microcoder-runs";

/// The manifest a retained host directory carries.
pub const MANIFEST: &str = "MANIFEST.json";

/// The manifest's schema.
pub const MANIFEST_SCHEMA: &str = "openagents.gym.microcoder-retained.v1";

/// Where a run's cost came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CostBasis {
    /// The provider billed it: OpenRouter's reported charge.
    Billed,
    /// List price applied to the tokens the provider reported: the Codex
    /// login, which bills a subscription rather than per token.
    ListPrice,
    /// The record doesn't say, and the provider can't be told.
    Unknown,
}

impl CostBasis {
    /// `billed`, `list_price`, or `unknown`, as records and JSON write it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            CostBasis::Billed => "billed",
            CostBasis::ListPrice => "list_price",
            CostBasis::Unknown => "unknown",
        }
    }

    /// The basis in a claim's words.
    #[must_use]
    pub fn phrase(self) -> &'static str {
        match self {
            CostBasis::Billed => "billed cost",
            CostBasis::ListPrice => "list-price cost",
            CostBasis::Unknown => "cost basis unknown",
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "billed" => CostBasis::Billed,
            "list_price" => CostBasis::ListPrice,
            _ => CostBasis::Unknown,
        }
    }
}

/// Whether the entries a run used were written from its own task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Sample {
    /// An entry it used was written from this task.
    InSample,
    /// It used entries, and none was written from this task.
    OutOfSample,
    /// No entry it used names this task, but some aren't in the checkout's
    /// `knowledge/`, so their provenance is unknown.
    Unknown,
    /// It used no entries.
    NoKnowledge,
}

impl Sample {
    /// `in_sample`, `out_of_sample`, `unknown`, or `no_knowledge`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Sample::InSample => "in_sample",
            Sample::OutOfSample => "out_of_sample",
            Sample::Unknown => "unknown",
            Sample::NoKnowledge => "no_knowledge",
        }
    }

    /// The label a claim prints.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Sample::InSample => "in-sample",
            Sample::OutOfSample => "out-of-sample",
            Sample::Unknown => "sample unknown",
            Sample::NoKnowledge => "no knowledge used",
        }
    }
}

/// One entry a run's prompts listed or showed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UsedEntry {
    pub id: String,
    /// `sha256:…` of the entry as the run saw it, when recorded.
    pub digest: Option<String>,
    /// The entry's version as the run saw it, from the run's `started`
    /// event, when recorded.
    pub version: Option<u64>,
    /// Steps whose prompt listed it.
    pub kept_steps: Option<u64>,
    /// Steps whose prompt showed its whole body.
    pub expanded_steps: Option<u64>,
    /// Its `provenance.written_from` in the checkout's `knowledge/`, or
    /// `None` when the checkout has no entry with this ID.
    pub written_from: Option<Vec<String>>,
    /// Whether `written_from` names this run's task.
    pub names_task: bool,
}

/// What a Microcoder record says beyond the common run fields.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Microcoder {
    /// The run directory's name, such as `gsea-proteomics-1790430415859`.
    pub name: String,
    /// The model as the record names it, such as `openai/gpt-6-luna`.
    pub model: Option<String>,
    /// `codex` or `openrouter`, or `None` when it can't be told.
    pub provider: Option<String>,
    /// Whether the provider is the record's own word or read from the model
    /// name (OpenRouter's names carry a `vendor/` prefix; the Codex login's
    /// don't).
    pub provider_recorded: bool,
    pub cost_basis: Option<CostBasis>,
    pub cost_basis_recorded: bool,
    pub model_usd: Option<f64>,
    pub jev_usd: Option<f64>,
    pub embedding_usd: Option<f64>,
    /// Tokens the model calls reported, summed over `generated` events.
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub effort: Option<String>,
    /// `on`, `off`, or `candidates`, when recorded.
    pub kb: Option<String>,
    pub kb_trust: Option<String>,
    pub knowledge_assisted: bool,
    pub entries: Vec<UsedEntry>,
    pub sample: Option<Sample>,
    /// `embeddings` (words and embeddings), `lexical` (words alone),
    /// `mixed`, or `off`.
    pub retrieval: Option<String>,
    pub commit: Option<String>,
    /// Where `commit` comes from: `record`, or the manifest's attribution
    /// rule in words.
    pub commit_source: Option<String>,
    pub steps: Option<u64>,
    /// Why it ended: `finished`, `time_limit`, `tests_held`, and so on.
    pub ending: Option<String>,
    /// The acceptance tests the model froze, and how many passed at the end.
    pub acceptance: Option<Tests>,
    /// More than one run wrote this directory, so its records are mixed.
    pub mixed: bool,
    /// Retained files whose SHA-256 doesn't match the manifest.
    pub digest_mismatches: Vec<String>,
    /// The retained manifest's SHA-256 of `summary.json` and `events.jsonl`.
    pub digests: BTreeMap<String, String>,
}

impl Microcoder {
    /// The labels every public number about this run carries, in words:
    /// `in-sample · knowledge-assisted · OpenRouter · billed cost`.
    #[must_use]
    pub fn labels(&self) -> String {
        [
            self.sample
                .unwrap_or(Sample::NoKnowledge)
                .label()
                .to_owned(),
            if self.knowledge_assisted {
                "knowledge-assisted".to_owned()
            } else {
                "not knowledge-assisted".to_owned()
            },
            provider_name(self.provider.as_deref()).to_owned(),
            self.cost_basis
                .unwrap_or(CostBasis::Unknown)
                .phrase()
                .to_owned(),
        ]
        .join(" · ")
    }
}

/// `OpenRouter`, `the Codex login`, or `provider unknown`.
#[must_use]
pub fn provider_name(provider: Option<&str>) -> &str {
    match provider {
        Some("openrouter") => "OpenRouter",
        Some("codex") => "the Codex login",
        Some(other) => other,
        None => "provider unknown",
    }
}

/// The checkout's knowledge entries' provenance: each ID's
/// `written_from`.
#[derive(Clone, Debug, Default)]
pub struct Knowledge {
    pub dir: Option<PathBuf>,
    pub written_from: HashMap<String, Vec<String>>,
}

impl Knowledge {
    /// Reads every entry in `dir`, whatever its status.
    #[must_use]
    pub fn read(dir: &Path) -> Self {
        let (entries, _) = knowledge::Base::read(dir);
        Knowledge {
            dir: Some(dir.to_path_buf()),
            written_from: entries
                .into_iter()
                .map(|entry| (entry.id, entry.written_from))
                .collect(),
        }
    }
}

/// A retained host directory's manifest, read.
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    /// Per run directory: file name to `sha256:` digest.
    pub files: HashMap<String, BTreeMap<String, String>>,
    /// Per run directory: the attributed commit and its source.
    pub commits: HashMap<String, (String, String)>,
    /// Per run directory: why its records are known to be mixed, when a
    /// note outside the records says so.
    pub mixed: HashMap<String, String>,
}

impl Manifest {
    /// Reads `dir/MANIFEST.json`, when there is one.
    #[must_use]
    pub fn read(dir: &Path) -> Option<Self> {
        let value = crate::runs::read_json(&dir.join(MANIFEST))?;
        if value["schema"] != MANIFEST_SCHEMA {
            return None;
        }
        let rule = value["commit_rule"].as_str().unwrap_or_default().to_owned();
        let mut manifest = Manifest::default();
        for run in value["runs"].as_array().into_iter().flatten() {
            let Some(name) = run["name"].as_str() else {
                continue;
            };
            let files = run["files"]
                .as_object()
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|(file, digest)| {
                            Some((file.clone(), digest.as_str()?.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            manifest.files.insert(name.to_owned(), files);
            if let Some(commit) = run["commit"].as_str() {
                let source = run["commit_source"]
                    .as_str()
                    .map_or_else(|| rule.clone(), str::to_owned);
                manifest
                    .commits
                    .insert(name.to_owned(), (commit.to_owned(), source));
            }
            if let Some(why) = run["mixed"].as_str() {
                manifest.mixed.insert(name.to_owned(), why.to_owned());
            }
        }
        Some(manifest)
    }
}

/// `sha256:` and the hex digest of a file.
#[must_use]
pub fn file_digest(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    Some(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

/// The run directories under `dir`: those with a summary or an event log.
#[must_use]
pub fn run_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("summary.json").is_file() || path.join("events.jsonl").is_file())
        .collect();
    dirs.sort();
    dirs
}

/// The standard places Microcoder runs are read from: this computer's run
/// directory and each retained host directory in the checkout. Their order
/// doesn't matter: [`Plan`] picks a run's copy and marks.
#[must_use]
pub fn standard_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents/microcoder/runs"));
    }
    let retained = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(RETAINED);
    let mut hosts: Vec<PathBuf> = std::fs::read_dir(&retained)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    hosts.sort();
    dirs.extend(hosts);
    dirs
}

/// When a run started, from its directory's stamp: seconds for ten
/// digits, milliseconds for thirteen.
#[must_use]
pub fn started_ms(name: &str) -> Option<i64> {
    let (_, stamp) = name.rsplit_once('-')?;
    if stamp.is_empty() || !stamp.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let value: i64 = stamp.parse().ok()?;
    Some(if stamp.len() >= 13 {
        value
    } else {
        value * 1000
    })
}

/// The events of a run's `events.jsonl`, skipping lines that aren't JSON,
/// and how many were skipped.
#[must_use]
pub fn events(path: &Path) -> (Vec<Value>, usize) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (Vec::new(), 0);
    };
    let mut skipped = 0;
    let events = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| match serde_json::from_str::<Value>(line) {
            Ok(value) => Some(value),
            Err(_) => {
                skipped += 1;
                None
            }
        })
        .collect();
    (events, skipped)
}

/// Reads one run directory. `manifest` is the retained host directory's
/// manifest, when the run is retained.
#[must_use]
pub fn read(dir: &Path, knowledge: &Knowledge, manifest: Option<&Manifest>, now: i64) -> Run {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let summary_path = dir.join("summary.json");
    let events_path = dir.join("events.jsonl");
    let summary = crate::runs::read_json(&summary_path);
    let (events, skipped) = events(&events_path);
    let started_event = events.iter().find(|e| e["event"] == "started");
    let task = summary
        .as_ref()
        .and_then(|s| s["task"].as_str())
        .or_else(|| started_event.and_then(|e| e["task"].as_str()))
        .map_or_else(|| knowledge::evidence::task_of(&name), str::to_owned);
    let raw_model = summary
        .as_ref()
        .and_then(|s| s["model"].as_str())
        .or_else(|| started_event.and_then(|e| e["model"].as_str()))
        .map(str::to_owned);
    let mut notes = Vec::new();
    if skipped > 0 {
        notes.push(format!("{skipped} lines of events.jsonl aren't JSON"));
    }

    // Provider and cost basis: the record's word, else the model's name.
    let recorded_provider = summary
        .as_ref()
        .and_then(|s| s["provider"].as_str())
        .or_else(|| started_event.and_then(|e| e["provider"].as_str()))
        .map(str::to_owned);
    let provider_recorded = recorded_provider.is_some();
    let provider = recorded_provider.or_else(|| {
        raw_model.as_deref().map(|model| {
            if model.contains('/') {
                "openrouter".to_owned()
            } else {
                "codex".to_owned()
            }
        })
    });
    let recorded_basis = summary
        .as_ref()
        .and_then(|s| {
            s["cost_basis"]
                .as_str()
                .or_else(|| s["outcome"]["cost_basis"].as_str())
        })
        .map(CostBasis::parse);
    let cost_basis_recorded = recorded_basis.is_some();
    let cost_basis = recorded_basis.unwrap_or(match provider.as_deref() {
        Some("openrouter") => CostBasis::Billed,
        Some("codex") => CostBasis::ListPrice,
        _ => CostBasis::Unknown,
    });

    let outcome_value = summary.as_ref().map(|s| &s["outcome"]);
    let number = |key: &str| outcome_value.and_then(|o| o[key].as_f64());
    let (model_usd, jev_usd, embedding_usd) = (
        number("model_usd"),
        number("jev_usd"),
        number("embedding_usd"),
    );
    let seconds = number("seconds").or_else(|| {
        events
            .iter()
            .find(|e| e["event"] == "ended")
            .and_then(|e| e["seconds"].as_f64())
    });

    // Tokens, summed over the model calls.
    let mut prompt_tokens = None::<u64>;
    let mut completion_tokens = None::<u64>;
    for event in events.iter().filter(|e| e["event"] == "generated") {
        if let Some(n) = event["generated"]["prompt_tokens"].as_u64() {
            *prompt_tokens.get_or_insert(0) += n;
        }
        if let Some(n) = event["generated"]["completion_tokens"].as_u64() {
            *completion_tokens.get_or_insert(0) += n;
        }
    }

    // The entries it used, with the versions the run started with.
    let versions: HashMap<String, (Option<u64>, Option<String>)> = started_event
        .and_then(|e| e["knowledge_entries"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some((
                entry["id"].as_str()?.to_owned(),
                (
                    entry["version"].as_u64(),
                    entry["digest"].as_str().map(str::to_owned),
                ),
            ))
        })
        .collect();
    let entries: Vec<UsedEntry> = outcome_value
        .and_then(|o| o["knowledge"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|used| {
            let id = used["id"].as_str()?.to_owned();
            let digest = used["digest"].as_str().map(str::to_owned);
            // The version is the base's at start, when the digest matches.
            let version = versions.get(&id).and_then(|(version, seen)| {
                (digest.is_none() || seen.is_none() || *seen == digest)
                    .then_some(*version)
                    .flatten()
            });
            let written_from = knowledge.written_from.get(&id).cloned();
            let names_task = written_from.as_ref().is_some_and(|from| {
                from.iter()
                    .filter(|w| w.as_str() != "reference")
                    .any(|w| knowledge::evidence::task_of(w) == task)
            });
            Some(UsedEntry {
                id,
                digest,
                version,
                kept_steps: used["kept_steps"].as_u64(),
                expanded_steps: used["expanded_steps"].as_u64(),
                written_from,
                names_task,
            })
        })
        .collect();
    let knowledge_assisted = summary
        .as_ref()
        .and_then(|s| {
            s["knowledge_assisted"]
                .as_bool()
                .or_else(|| s["outcome"]["knowledge_assisted"].as_bool())
        })
        .unwrap_or(!entries.is_empty());
    let sample = summary.is_some().then(|| {
        if entries.is_empty() {
            Sample::NoKnowledge
        } else if entries.iter().any(|e| e.names_task) {
            Sample::InSample
        } else if entries.iter().any(|e| e.written_from.is_none()) {
            Sample::Unknown
        } else {
            Sample::OutOfSample
        }
    });

    // Retrieval: the record's word (`retrieval.mode` since `dabfc62ca3`:
    // `off`, `lexical`, `embeddings`, or `mixed`), else the same words from
    // what each step's retrieval shows.
    let retrieval = summary
        .as_ref()
        .and_then(|s| {
            s["retrieval"]["mode"]
                .as_str()
                .or_else(|| s["retrieval_mode"].as_str())
        })
        .map(str::to_owned)
        .or_else(|| {
            let steps: Vec<&Value> = events
                .iter()
                .filter(|e| e["event"] == "retrieved")
                .collect();
            let kb = summary
                .as_ref()
                .and_then(|s| s["kb"].as_str())
                .or_else(|| started_event.and_then(|e| e["kb"].as_str()));
            if steps.is_empty() {
                return (kb == Some("off")).then(|| "off".to_owned());
            }
            let lexical = steps
                .iter()
                .filter(|e| !e["retrieval"]["lexical_only"].is_null())
                .count();
            Some(match lexical {
                0 => "embeddings".to_owned(),
                n if n == steps.len() => "lexical".to_owned(),
                n => format!(
                    "mixed: embeddings on {} steps, lexical on {n}",
                    steps.len() - n
                ),
            })
        });

    // The commit: the record's, else the retained manifest's attribution.
    let (commit, commit_source) = match summary.as_ref().and_then(|s| s["commit"].as_str()) {
        Some(commit) => (Some(commit.to_owned()), Some("record".to_owned())),
        None => manifest
            .and_then(|m| m.commits.get(&name))
            .map_or((None, None), |(commit, source)| {
                (Some(commit.clone()), Some(source.clone()))
            }),
    };

    // Two runs that started in the same second shared one directory before
    // run directories carried milliseconds (`4c749622f2`). Their records
    // show it as a second start, or as event times that go backwards; a
    // retained manifest can also say so.
    let starts = events.iter().filter(|e| e["event"] == "started").count();
    let times: Vec<f64> = events
        .iter()
        .filter(|e| e["event"] != "ended")
        .filter_map(|e| e["seconds"].as_f64())
        .collect();
    let backwards = times.windows(2).any(|pair| pair[1] < pair[0]);
    let noted = manifest.and_then(|m| m.mixed.get(&name));
    let mixed = starts > 1 || backwards || noted.is_some();
    if starts > 1 {
        notes.push(format!(
            "{starts} runs wrote this directory, so its records are mixed"
        ));
    } else if backwards {
        notes.push(
            "its event times go backwards: two runs wrote this directory, so its records are mixed"
                .to_owned(),
        );
    } else if let Some(why) = noted {
        notes.push(format!("its records are mixed: {why}"));
    }

    // Retained files must match the manifest.
    let mut digests = BTreeMap::new();
    let mut digest_mismatches = Vec::new();
    if let Some(files) = manifest.and_then(|m| m.files.get(&name)) {
        for (file, expected) in files {
            digests.insert(file.clone(), expected.clone());
            if file_digest(&dir.join(file)).as_deref() != Some(expected.as_str()) {
                digest_mismatches.push(file.clone());
            }
        }
        if !digest_mismatches.is_empty() {
            notes.push(format!(
                "{} doesn't match the retained manifest's digest",
                digest_mismatches.join(" and ")
            ));
        }
    }

    let reward = summary.as_ref().and_then(|s| s["reward"].as_f64());
    let started = started_ms(&name);
    let outcome = match &summary {
        Some(_) => match reward {
            Some(r) if r >= 1.0 => Outcome::Passed,
            Some(_) => Outcome::Failed,
            None => Outcome::NotGraded("grading failed: the summary has no reward".to_owned()),
        },
        None => {
            let last = crate::runs::modified_ms(&events_path)
                .or(started)
                .unwrap_or(now);
            if now - last > crate::runs::ABANDONED_AFTER_MS {
                Outcome::NotGraded(
                    "it stopped before it wrote summary.json; only its events remain".to_owned(),
                )
            } else {
                Outcome::Running
            }
        }
    };
    let ending = outcome_value
        .and_then(|o| o["ending"]["reason"].as_str())
        .map(str::to_owned);
    if ending.as_deref() == Some("time_limit") {
        notes.push("It reached its time limit.".to_owned());
    }
    let acceptance = summary.as_ref().and_then(|s| {
        let results = s["test_results"].as_array()?;
        let passed = results.iter().filter(|r| r["exit"] == 0).count() as u64;
        Some(Tests {
            passed,
            failed: results.len() as u64 - passed,
            total: results.len() as u64,
        })
    });
    let tests = summary
        .as_ref()
        .and_then(|s| s["verifier_output"].as_str())
        .and_then(crate::runs::pytest_summary);
    // A component the record names with no number is unknown, and so is
    // the total (Microcoder writes `null` for it since `dabfc62ca3`). A
    // component an older record doesn't name at all didn't exist then.
    let named_unknown = ["jev_usd", "embedding_usd"].iter().any(|key| {
        outcome_value
            .and_then(|o| o.get(*key))
            .is_some_and(|value| !value.is_number())
    });
    let cost_usd = model_usd
        .filter(|_| !named_unknown)
        .map(|model| model + jev_usd.unwrap_or(0.0) + embedding_usd.unwrap_or(0.0));
    let agent_ms = seconds.map(|s| (s * 1000.0).round() as u64);
    let model = raw_model
        .as_deref()
        .map(|model| model.rsplit('/').next().unwrap_or(model).to_owned());

    Run {
        job: JOB.to_owned(),
        trial: name.clone(),
        batch: JOB.to_owned(),
        retained: manifest.is_some(),
        files: Files {
            dir: dir.to_path_buf(),
            result: summary.is_some().then(|| summary_path.clone()),
            live: events_path.is_file().then(|| events_path.clone()),
            ..Files::default()
        },
        task,
        task_path: None,
        ask: None,
        category: None,
        expert_hours: None,
        time_limit_sec: started_event.and_then(|e| e["limits"]["max_seconds"].as_f64()),
        agent: Agent::Microcoder,
        variant: None,
        model,
        started_ms: started,
        ended_ms: started.zip(agent_ms).map(|(s, ms)| s + ms as i64),
        agent_ms,
        active_ms: None,
        outcome,
        reward,
        tests,
        cost_usd,
        cost_estimated: cost_basis == CostBasis::ListPrice,
        notes,
        microcoder: Some(Box::new(Microcoder {
            name,
            model: raw_model,
            provider,
            provider_recorded,
            cost_basis: Some(cost_basis),
            cost_basis_recorded,
            model_usd,
            jev_usd,
            embedding_usd,
            prompt_tokens,
            completion_tokens,
            effort: summary
                .as_ref()
                .and_then(|s| s["effort"].as_str())
                .map(str::to_owned),
            kb: summary
                .as_ref()
                .and_then(|s| s["kb"].as_str())
                .or_else(|| started_event.and_then(|e| e["kb"].as_str()))
                .map(str::to_owned),
            kb_trust: summary
                .as_ref()
                .and_then(|s| s["kb_trust"].as_str())
                .or_else(|| started_event.and_then(|e| e["kb_trust"].as_str()))
                .map(str::to_owned),
            knowledge_assisted,
            entries,
            sample,
            retrieval,
            commit,
            commit_source,
            steps: outcome_value.and_then(|o| o["steps"].as_u64()),
            ending,
            acceptance,
            mixed,
            digest_mismatches,
            digests,
        })),
    }
}

/// Which copy of each run to read from a set of directories, and the marks
/// their retained manifests hold, whatever order the directories came in.
///
/// A run's ID is its directory name, so directories holding a run of the
/// same name hold copies of one run, and one copy is read. The copy a
/// manifest lists wins; then a copy in a directory with a manifest; then
/// the first by path. A manifest's marks, the mixed mark and the commit
/// attribution, hold for every copy of a run of that name, wherever the
/// copy read comes from. When two manifests disagree, the first by path
/// gives the reason and the commit.
#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// Each directory's manifest, in the order of `sources`.
    manifests: Vec<Option<Manifest>>,
    /// Every manifest's commit attributions and mixed marks, merged.
    marks: Manifest,
    /// The copy of each run to read, by name.
    pub copies: Vec<Copy>,
}

/// One run's copy to read, and the copies of it that aren't read.
#[derive(Clone, Debug)]
pub struct Copy {
    /// The run directory's name: the run's ID after `microcoder/`.
    pub name: String,
    /// The run directory read.
    pub dir: PathBuf,
    /// Which source directory it's in: an index into the plan's manifests.
    source: usize,
    /// The other directories holding a run of this name.
    pub others: Vec<PathBuf>,
}

impl Plan {
    /// Plans the reading of every run under `dirs`.
    #[must_use]
    pub fn new(dirs: &[PathBuf]) -> Self {
        let mut sources: Vec<&PathBuf> = dirs.iter().collect();
        sources.sort();
        sources.dedup();
        let manifests: Vec<Option<Manifest>> =
            sources.iter().map(|dir| Manifest::read(dir)).collect();
        let mut marks = Manifest::default();
        for manifest in manifests.iter().flatten() {
            for (name, commit) in &manifest.commits {
                marks
                    .commits
                    .entry(name.clone())
                    .or_insert_with(|| commit.clone());
            }
            for (name, why) in &manifest.mixed {
                marks
                    .mixed
                    .entry(name.clone())
                    .or_insert_with(|| why.clone());
            }
        }
        // Per name: (preference, run directory, source), lowest first.
        let mut found: BTreeMap<String, Vec<(u8, PathBuf, usize)>> = BTreeMap::new();
        for (source, dir) in sources.iter().enumerate() {
            let manifest = manifests[source].as_ref();
            for run_dir in run_dirs(dir) {
                let name = run_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let preference = match manifest {
                    Some(m) if m.files.contains_key(&name) => 0,
                    Some(_) => 1,
                    None => 2,
                };
                found
                    .entry(name)
                    .or_default()
                    .push((preference, run_dir, source));
            }
        }
        let copies = found
            .into_iter()
            .map(|(name, mut found)| {
                found.sort();
                let (_, dir, source) = found.remove(0);
                Copy {
                    name,
                    dir,
                    source,
                    others: found.into_iter().map(|(_, dir, _)| dir).collect(),
                }
            })
            .collect();
        Plan {
            manifests,
            marks,
            copies,
        }
    }

    /// The manifest of the directory `copy` is in, when it has one.
    #[must_use]
    pub fn manifest(&self, copy: &Copy) -> Option<&Manifest> {
        self.manifests.get(copy.source).and_then(Option::as_ref)
    }

    /// Reads `copy`, with every manifest's marks for its name.
    #[must_use]
    pub fn read(&self, copy: &Copy, knowledge: &Knowledge, now: i64) -> Run {
        let mut run = read(&copy.dir, knowledge, self.manifest(copy), now);
        self.mark(copy, &mut run);
        run
    }

    /// Applies the manifests' marks for `copy`'s name to a run read from
    /// it, and notes another copy whose summary differs.
    fn mark(&self, copy: &Copy, run: &mut Run) {
        let Some(m) = run.microcoder.as_deref_mut() else {
            return;
        };
        if !m.mixed
            && let Some(why) = self.marks.mixed.get(&copy.name)
        {
            m.mixed = true;
            run.notes.push(format!("its records are mixed: {why}"));
        }
        if m.commit.is_none()
            && let Some((commit, source)) = self.marks.commits.get(&copy.name)
        {
            m.commit = Some(commit.clone());
            m.commit_source = Some(source.clone());
        }
        if copy.others.is_empty() {
            return;
        }
        let Some(own) = file_digest(&copy.dir.join("summary.json")) else {
            return;
        };
        let differing: Vec<String> = copy
            .others
            .iter()
            .filter(|other| {
                file_digest(&other.join("summary.json")).is_some_and(|digest| digest != own)
            })
            .map(|other| other.display().to_string())
            .collect();
        if !differing.is_empty() {
            run.notes.push(format!(
                "another copy of this run holds a different summary.json ({}); this one, at {}, was read",
                differing.join(", "),
                copy.dir.display()
            ));
        }
    }
}

/// Every run in `dirs`, one copy of each: the retained copy when a
/// manifest lists it, with every manifest's marks, in any order of `dirs`.
#[must_use]
pub fn read_all(dirs: &[PathBuf], knowledge: &Knowledge, now: i64) -> Vec<Run> {
    let plan = Plan::new(dirs);
    plan.copies
        .iter()
        .map(|copy| plan.read(copy, knowledge, now))
        .collect()
}

/// The run's details in lines, for `gym runs show`.
#[must_use]
pub fn detail_lines(run: &Run) -> Vec<String> {
    let Some(m) = &run.microcoder else {
        return Vec::new();
    };
    let usd = crate::runs_analysis::usd;
    let mut lines = vec![
        "Microcoder".to_owned(),
        format!("  Labels: {}", m.labels()),
        format!(
            "  Model: {} through {}{}; effort {}",
            m.model.as_deref().unwrap_or("unknown"),
            provider_name(m.provider.as_deref()),
            if m.provider_recorded {
                ""
            } else {
                " (read from the model name; the record doesn't say)"
            },
            m.effort.as_deref().unwrap_or("unknown"),
        ),
        format!(
            "  Cost: {} ({}{}): model {}, Jev {}, embeddings {}",
            run.cost_usd.map_or_else(|| "unknown".to_owned(), usd),
            m.cost_basis.unwrap_or(CostBasis::Unknown).word(),
            if m.cost_basis_recorded {
                ""
            } else {
                ", from the provider; the record doesn't say"
            },
            m.model_usd.map_or_else(|| "unknown".to_owned(), usd),
            m.jev_usd.map_or_else(|| "unknown".to_owned(), usd),
            m.embedding_usd.map_or_else(|| "unknown".to_owned(), usd),
        ),
    ];
    if let (Some(input), Some(output)) = (m.prompt_tokens, m.completion_tokens) {
        lines.push(format!("  Tokens: {input} in, {output} out"));
    }
    lines.push(format!(
        "  Knowledge base: {}{}; retrieval {}",
        m.kb.as_deref().unwrap_or("not recorded"),
        m.kb_trust
            .as_deref()
            .map_or_else(String::new, |trust| format!(" (trust {trust})")),
        m.retrieval.as_deref().unwrap_or("not recorded"),
    ));
    for entry in &m.entries {
        let provenance = match &entry.written_from {
            Some(from) => format!("written from {}", from.join(", ")),
            None => "not in the checkout's knowledge/".to_owned(),
        };
        lines.push(format!(
            "    {}{}{} · {provenance}{}",
            entry.id,
            entry.version.map_or_else(String::new, |v| format!(" v{v}")),
            entry
                .digest
                .as_deref()
                .map_or_else(String::new, |d| format!(" {}", d.get(..19).unwrap_or(d))),
            if entry.names_task {
                " · names this task"
            } else {
                ""
            },
        ));
    }
    lines.push(format!(
        "  Commit: {}",
        match (&m.commit, &m.commit_source) {
            (Some(commit), Some(source)) if source == "record" => commit.clone(),
            (Some(commit), Some(source)) => format!("{commit} (not in the record; {source})"),
            _ => "not recorded".to_owned(),
        }
    ));
    if let Some(acceptance) = m.acceptance {
        lines.push(format!(
            "  Acceptance tests: {} of {} passed at the end",
            acceptance.passed, acceptance.total
        ));
    }
    if let Some(ending) = &m.ending {
        lines.push(format!(
            "  Ended: {ending} after {} steps",
            m.steps.map_or_else(|| "?".to_owned(), |s| s.to_string())
        ));
    }
    for (file, digest) in &m.digests {
        lines.push(format!("  Retained {file}: {digest}"));
    }
    lines
}

/// The run as JSON: the common fields' extras for `gym runs --json`.
#[must_use]
pub fn json(m: &Microcoder) -> Value {
    serde_json::json!({
        "name": m.name,
        "labels": m.labels(),
        "model": m.model,
        "provider": m.provider,
        "provider_recorded": m.provider_recorded,
        "cost_basis": m.cost_basis.map(CostBasis::word),
        "cost_basis_recorded": m.cost_basis_recorded,
        "model_usd": m.model_usd,
        "jev_usd": m.jev_usd,
        "embedding_usd": m.embedding_usd,
        "prompt_tokens": m.prompt_tokens,
        "completion_tokens": m.completion_tokens,
        "kb": m.kb,
        "kb_trust": m.kb_trust,
        "knowledge_assisted": m.knowledge_assisted,
        "sample": m.sample.map(Sample::word),
        "entries": m.entries.iter().map(|e| serde_json::json!({
            "id": e.id,
            "digest": e.digest,
            "version": e.version,
            "kept_steps": e.kept_steps,
            "expanded_steps": e.expanded_steps,
            "written_from": e.written_from,
            "names_task": e.names_task,
        })).collect::<Vec<_>>(),
        "retrieval": m.retrieval,
        "commit": m.commit,
        "commit_source": m.commit_source,
        "steps": m.steps,
        "ending": m.ending,
        "mixed": m.mixed,
        "digests": m.digests,
        "digest_mismatches": m.digest_mismatches,
    })
}

// ---------------------------------------------------------------------------
// The transcript
// ---------------------------------------------------------------------------

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let mut clipped: String = text.chars().take(limit).collect();
        clipped.push('…');
        clipped
    }
}

fn answers(judgment: &Value) -> String {
    judgment["answers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|pair| Some(format!("{} {:.2}", pair[0].as_str()?, pair[1].as_f64()?)))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// One event as a transcript block, or `None` for an event the transcript
/// doesn't show.
fn blocks_of(event: &Value, at: Option<i64>) -> Vec<Block> {
    let block = |kind: Kind| Block { at, kind };
    let step = event["step"].as_u64();
    match event["event"].as_str().unwrap_or_default() {
        "started" => vec![block(Kind::Section {
            title: "Microcoder started".to_owned(),
            note: Some(format!(
                "{} · effort {} · knowledge base {}",
                event["model"].as_str().unwrap_or("model unknown"),
                event["effort"].as_str().unwrap_or("unknown"),
                event["kb"].as_str().unwrap_or("not recorded"),
            )),
            milliseconds: None,
            cost_usd: None,
        })],
        "retrieved" => {
            let retrieval = &event["retrieval"];
            let kept: Vec<String> = retrieval["kept"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|k| {
                    Some(format!(
                        "{} ({:.2})",
                        k["id"].as_str()?,
                        k["relevance"].as_f64().unwrap_or(0.0)
                    ))
                })
                .collect();
            let expanded: Vec<String> = retrieval["expanded"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|pair| pair[0].as_str().map(str::to_owned))
                .collect();
            if kept.is_empty() && expanded.is_empty() {
                return Vec::new();
            }
            vec![block(Kind::Look {
                what: format!(
                    "Knowledge for step {}: kept {}",
                    step.unwrap_or(0),
                    kept.len()
                ),
                output: format!(
                    "kept: {}\nshown in full: {}",
                    kept.join(", "),
                    if expanded.is_empty() {
                        "none".to_owned()
                    } else {
                        expanded.join(", ")
                    }
                ),
            })]
        }
        "judged" => vec![block(Kind::Decision {
            question: "Jev: is it done, making progress, repeating?".to_owned(),
            answer: answers(&event["judgment"]),
            detail: Vec::new(),
            probability: None,
            milliseconds: event["judgment"]["milliseconds"].as_u64(),
            cost_usd: event["judgment"]["usd"].as_f64(),
        })],
        "generated" => {
            let generated = &event["generated"];
            let mut out = vec![block(Kind::Section {
                title: format!("Step {}", step.unwrap_or(0)),
                note: generated["model"].as_str().map(str::to_owned),
                milliseconds: generated["milliseconds"].as_u64(),
                cost_usd: generated["usd"].as_f64(),
            })];
            let action = &generated["action"];
            if let Some(error) = action["Err"].as_str() {
                out.push(block(Kind::Note(format!(
                    "The reply didn't parse: {}",
                    clip(error, 400)
                ))));
            } else {
                let ok = &action["Ok"];
                if let Some(rationale) = ok["rationale"].as_str() {
                    out.push(block(Kind::Say(rationale.to_owned())));
                }
                if ok["freeze_tests"] == true {
                    out.push(block(Kind::Note(
                        "It froze its acceptance tests.".to_owned(),
                    )));
                }
                if ok["finished"] == true {
                    out.push(block(Kind::Note(
                        "It said the task is finished.".to_owned(),
                    )));
                }
            }
            out
        }
        "ran" => {
            let result = &event["result"];
            let exit = result["exit"].as_i64();
            vec![block(Kind::Command {
                command: result["command"].as_str().unwrap_or_default().to_owned(),
                output: clip(result["output"].as_str().unwrap_or_default(), 4000),
                exit,
                failed: exit.is_some_and(|code| code != 0) || result["timed_out"] == true,
            })]
        }
        "tested" => {
            let results: Vec<&Value> = event["results"].as_array().into_iter().flatten().collect();
            let passed = results.iter().filter(|r| r["exit"] == 0).count();
            vec![block(Kind::Check {
                title: if event["froze"] == true {
                    "Acceptance tests, frozen".to_owned()
                } else {
                    "Acceptance tests".to_owned()
                },
                verdict: format!("{passed} of {} pass", results.len()),
                lines: results
                    .iter()
                    .filter(|r| r["exit"] != 0)
                    .map(|r| {
                        format!(
                            "{} failed: {}",
                            r["command"].as_str().unwrap_or("?"),
                            // A traceback's last line names the failure.
                            clip(
                                r["output"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .lines()
                                    .rev()
                                    .find(|line| !line.trim().is_empty())
                                    .unwrap_or("no output")
                                    .trim(),
                                300
                            )
                        )
                    })
                    .collect(),
                good: Some(passed == results.len()),
            })]
        }
        kind @ ("disputed" | "covered" | "conformed") => vec![block(Kind::Decision {
            question: match kind {
                "disputed" => "Jev: is a failing frozen test itself wrong?",
                "covered" => "Jev: do the passing tests leave the task uncovered?",
                _ => "Jev: does the finished code contradict a relevant entry?",
            }
            .to_owned(),
            answer: answers(&event["judgment"]),
            detail: ["dropped", "flagged"]
                .iter()
                .filter_map(|key| {
                    let list: Vec<String> = event[*key]
                        .as_array()?
                        .iter()
                        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                        .collect();
                    (!list.is_empty()).then(|| ((*key).to_owned(), list.join(", ")))
                })
                .collect(),
            probability: None,
            milliseconds: event["judgment"]["milliseconds"].as_u64(),
            cost_usd: event["judgment"]["usd"].as_f64(),
        })],
        "ended" => vec![block(Kind::Report(format!(
            "Ended: {} after {} steps.",
            event["outcome"]["ending"]["reason"]
                .as_str()
                .unwrap_or("unknown"),
            event["outcome"]["steps"]
                .as_u64()
                .map_or_else(|| "?".to_owned(), |s| s.to_string())
        )))],
        "verified" => vec![block(Kind::Note(format!(
            "The task's verifier gave reward {}.",
            event["reward"]
                .as_f64()
                .map_or_else(|| "none".to_owned(), |r| format!("{r}"))
        )))],
        _ => Vec::new(),
    }
}

/// The run's `events.jsonl` as a transcript, each block at the run's start
/// plus the event's `seconds`.
#[must_use]
pub fn transcript(run: &Run) -> Transcript {
    let Some(path) = &run.files.live else {
        return Transcript::default();
    };
    crate::index::touch(path);
    let (events, _) = events(path);
    let start = run.started_ms;
    let mut blocks = Vec::new();
    for event in &events {
        let at = start
            .zip(event["seconds"].as_f64())
            .map(|(start, seconds)| start + (seconds * 1000.0).round() as i64);
        blocks.extend(blocks_of(event, at));
    }
    Transcript {
        blocks,
        sources: vec![path.clone()],
        ..Transcript::default()
    }
}

/// One replayable item: when, a title, its text parts (each marked prose
/// or literal), and the complete record it came from.
pub(crate) struct ReplayItem {
    pub at: Option<i64>,
    pub title: String,
    pub parts: Vec<(String, bool)>,
    pub record: String,
}

/// The run's events as head-to-head replay items, timed like the
/// transcript. `None` when the run has no event log.
#[must_use]
pub(crate) fn replay_items(run: &Run) -> Option<Vec<ReplayItem>> {
    let path = run.files.live.as_ref()?;
    let (events, _) = events(path);
    let start = run.started_ms;
    let mut items = Vec::new();
    for event in &events {
        let at = start
            .zip(event["seconds"].as_f64())
            .map(|(start, seconds)| start + (seconds * 1000.0).round() as i64);
        let record = serde_json::to_string_pretty(event).unwrap_or_default();
        for block in blocks_of(event, at) {
            let (title, parts) = match block.kind {
                Kind::Section { title, note, .. } => {
                    (title, note.map(|n| vec![(n, false)]).unwrap_or_default())
                }
                Kind::Say(text) => ("Microcoder".to_owned(), vec![(text, true)]),
                Kind::Command {
                    command,
                    output,
                    exit,
                    ..
                } => (
                    format!(
                        "Command{}",
                        exit.map_or_else(String::new, |code| format!(" (exit {code})"))
                    ),
                    vec![(format!("$ {command}"), false), (output, false)],
                ),
                Kind::Look { what, output } => (what, vec![(output, false)]),
                Kind::Decision {
                    question,
                    answer,
                    detail,
                    ..
                } => {
                    let mut text = answer;
                    for (key, value) in detail {
                        text.push_str(&format!("\n{key}: {value}"));
                    }
                    (question, vec![(text, false)])
                }
                Kind::Check {
                    title,
                    verdict,
                    lines,
                    ..
                } => {
                    let mut text = verdict;
                    for line in lines {
                        text.push('\n');
                        text.push_str(&line);
                    }
                    (title, vec![(text, false)])
                }
                Kind::Report(text) => ("Report".to_owned(), vec![(text, true)]),
                Kind::Note(text) => ("Note".to_owned(), vec![(text, true)]),
                _ => continue,
            };
            items.push(ReplayItem {
                at: block.at,
                title,
                parts: parts.into_iter().filter(|(t, _)| !t.is_empty()).collect(),
                record: record.clone(),
            });
        }
    }
    Some(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/microcoder")
    }

    fn catalog() -> Vec<Run> {
        let knowledge = Knowledge::read(&fixtures().join("knowledge"));
        read_all(&[fixtures().join("runs")], &knowledge, i64::MAX / 4)
    }

    fn find<'a>(runs: &'a [Run], name: &str) -> &'a Run {
        runs.iter().find(|r| r.trial == name).expect(name)
    }

    #[test]
    fn reads_an_openrouter_run_with_its_labels() {
        let runs = catalog();
        let run = find(&runs, "drift-check-1790393791");
        let m = run.microcoder.as_ref().unwrap();
        assert_eq!(run.agent, Agent::Microcoder);
        assert_eq!(run.id(), "microcoder/drift-check-1790393791");
        assert_eq!(run.outcome, Outcome::Passed);
        assert_eq!(run.model.as_deref(), Some("gpt-6-luna"));
        assert_eq!(run.started_ms, Some(1_790_393_791_000));
        assert_eq!(run.agent_ms, Some(141_500));
        assert!((run.cost_usd.unwrap() - 0.0165).abs() < 1e-9);
        assert!(!run.cost_estimated);
        assert_eq!(m.provider.as_deref(), Some("openrouter"));
        assert!(!m.provider_recorded);
        assert_eq!(m.cost_basis, Some(CostBasis::Billed));
        assert!(m.knowledge_assisted);
        assert_eq!(m.sample, Some(Sample::InSample));
        assert_eq!(m.retrieval.as_deref(), Some("embeddings"));
        let mmd = m.entries.iter().find(|e| e.id == "statistics.mmd").unwrap();
        assert!(mmd.names_task);
        assert_eq!(mmd.version, Some(2));
        assert_eq!(mmd.digest.as_deref(), Some("sha256:aa"));
        let general = m.entries.iter().find(|e| e.id == "slip.general").unwrap();
        assert!(!general.names_task);
        assert_eq!(
            run.tests,
            Some(Tests {
                passed: 11,
                failed: 0,
                total: 11
            })
        );
        assert_eq!(
            m.labels(),
            "in-sample · knowledge-assisted · OpenRouter · billed cost"
        );
    }

    #[test]
    fn a_codex_run_is_list_price_and_a_recorded_basis_wins() {
        let runs = catalog();
        let codex = find(&runs, "drift-check-1790430415859");
        let m = codex.microcoder.as_ref().unwrap();
        assert_eq!(codex.started_ms, Some(1_790_430_415_859));
        assert_eq!(m.provider.as_deref(), Some("codex"));
        assert_eq!(m.cost_basis, Some(CostBasis::ListPrice));
        assert!(codex.cost_estimated);
        assert_eq!(m.retrieval.as_deref(), Some("lexical"));
        let recorded = find(&runs, "drift-check-1790500000000");
        let m = recorded.microcoder.as_ref().unwrap();
        assert!(m.provider_recorded && m.cost_basis_recorded);
        assert_eq!(m.cost_basis, Some(CostBasis::Billed));
        assert_eq!(m.commit.as_deref(), Some("0123456789"));
        assert_eq!(m.commit_source.as_deref(), Some("record"));
        // The record's own retrieval mode, and a Jev cost it names as
        // unknown, which leaves the total unknown.
        assert_eq!(m.retrieval.as_deref(), Some("mixed"));
        assert_eq!(recorded.cost_usd, None);
        assert_eq!(m.model_usd, Some(0.02));
    }

    #[test]
    fn unknown_stays_unknown() {
        let runs = catalog();
        // No model cost recorded: the cost is unknown, not zero.
        let run = find(&runs, "drift-check-1790396075");
        assert_eq!(run.cost_usd, None);
        assert_eq!(run.outcome, Outcome::Failed);
        let m = run.microcoder.as_ref().unwrap();
        // An entry the checkout doesn't have leaves the sample unknown.
        assert_eq!(m.sample, Some(Sample::Unknown));
        // No knowledge at all.
        let bare = find(&runs, "drift-check-1790386981");
        let m = bare.microcoder.as_ref().unwrap();
        assert!(!m.knowledge_assisted);
        assert_eq!(m.sample, Some(Sample::NoKnowledge));
        assert_eq!(m.retrieval.as_deref(), Some("off"));
        // A run with no summary that stopped long ago isn't graded.
        let stopped = find(&runs, "drift-check-1790387425");
        assert!(matches!(stopped.outcome, Outcome::NotGraded(_)));
        assert!(stopped.microcoder.as_ref().unwrap().sample.is_none());
    }

    #[test]
    fn out_of_sample_and_mixed_records() {
        let runs = catalog();
        let other = find(&runs, "sound-shift-1790402067");
        assert_eq!(
            other.microcoder.as_ref().unwrap().sample,
            Some(Sample::OutOfSample)
        );
        let mixed = find(&runs, "drift-check-1790406355");
        assert!(mixed.microcoder.as_ref().unwrap().mixed);
    }

    #[test]
    fn a_retained_manifest_attributes_commits_and_checks_digests() {
        let dir = tempfile::tempdir().unwrap();
        let host = dir.path().join("host");
        let run = host.join("drift-check-1790393791");
        std::fs::create_dir_all(&run).unwrap();
        for file in ["summary.json", "events.jsonl"] {
            std::fs::copy(
                fixtures().join("runs/drift-check-1790393791").join(file),
                run.join(file),
            )
            .unwrap();
        }
        let digest = file_digest(&run.join("summary.json")).unwrap();
        let manifest = serde_json::json!({
            "schema": MANIFEST_SCHEMA,
            "commit_rule": "the last crates/microcoder commit at or before the start",
            "runs": [{
                "name": "drift-check-1790393791",
                "files": {"summary.json": digest, "events.jsonl": "sha256:00"},
                "commit": "a784eadef8",
            }],
        });
        std::fs::write(host.join(MANIFEST), manifest.to_string()).unwrap();
        let runs = read_all(&[host], &Knowledge::default(), 0);
        let m = runs[0].microcoder.as_ref().unwrap();
        assert!(runs[0].retained);
        assert_eq!(m.commit.as_deref(), Some("a784eadef8"));
        assert_eq!(
            m.commit_source.as_deref(),
            Some("the last crates/microcoder commit at or before the start")
        );
        assert_eq!(m.digest_mismatches, vec!["events.jsonl".to_owned()]);
    }

    #[test]
    fn a_manifest_marks_every_copy_of_a_run_in_any_order() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live");
        let host = dir.path().join("host");
        let name = "drift-check-1790393791";
        std::fs::create_dir_all(live.join(name)).unwrap();
        std::fs::create_dir_all(&host).unwrap();
        for file in ["summary.json", "events.jsonl"] {
            std::fs::copy(
                fixtures().join("runs").join(name).join(file),
                live.join(name).join(file),
            )
            .unwrap();
        }
        // The manifest marks the run, but its directory only holds the
        // manifest: the live copy is read, and still carries the marks.
        let manifest = serde_json::json!({
            "schema": MANIFEST_SCHEMA,
            "commit_rule": "the rule",
            "runs": [{"name": name, "commit": "a784eadef8", "mixed": "two runs shared it"}],
        });
        std::fs::write(host.join(MANIFEST), manifest.to_string()).unwrap();
        for dirs in [[live.clone(), host.clone()], [host.clone(), live.clone()]] {
            let runs = read_all(&dirs, &Knowledge::default(), 0);
            assert_eq!(runs.len(), 1);
            let m = runs[0].microcoder.as_ref().unwrap();
            assert!(!runs[0].retained);
            assert!(m.mixed);
            assert_eq!(m.commit.as_deref(), Some("a784eadef8"));
            assert_eq!(m.commit_source.as_deref(), Some("the rule"));
            assert!(
                runs[0]
                    .notes
                    .contains(&"its records are mixed: two runs shared it".to_owned())
            );
        }
        // A retained copy that differs from the live one is read, and the
        // difference is noted.
        std::fs::create_dir_all(host.join(name)).unwrap();
        std::fs::copy(
            live.join(name).join("events.jsonl"),
            host.join(name).join("events.jsonl"),
        )
        .unwrap();
        std::fs::write(host.join(name).join("summary.json"), "{\"reward\": 0.0}").unwrap();
        let manifest = serde_json::json!({
            "schema": MANIFEST_SCHEMA,
            "runs": [{"name": name, "files": {}}],
        });
        std::fs::write(host.join(MANIFEST), manifest.to_string()).unwrap();
        for dirs in [[live.clone(), host.clone()], [host.clone(), live.clone()]] {
            let runs = read_all(&dirs, &Knowledge::default(), 0);
            assert!(runs[0].retained);
            assert_eq!(runs[0].files.dir, host.join(name));
            assert!(
                runs[0]
                    .notes
                    .iter()
                    .any(|n| n
                        .starts_with("another copy of this run holds a different summary.json"))
            );
        }
    }

    #[test]
    fn the_transcript_reads_the_events() {
        let runs = catalog();
        let run = find(&runs, "drift-check-1790393791");
        let transcript = transcript(run);
        assert!(
            transcript.blocks.iter().any(
                |b| matches!(&b.kind, Kind::Command { command, .. } if command.contains("ls"))
            )
        );
        assert!(
            transcript.blocks.iter().any(
                |b| matches!(&b.kind, Kind::Check { verdict, .. } if verdict == "1 of 1 pass")
            )
        );
        let first = transcript.blocks.iter().find_map(|b| b.at).unwrap();
        assert!(first >= 1_790_393_791_000);
    }

    #[test]
    fn head_to_head_replays_the_events() {
        let runs = catalog();
        let run = find(&runs, "drift-check-1790393791").clone();
        let replay =
            crate::runs_replay::Replay::load(&crate::runs_replay::Source::Local(Box::new(run)))
                .unwrap();
        assert!(replay.events.iter().any(|e| e.title.starts_with("Command")));
        assert!(replay.events.iter().any(|e| e.text.contains("$ ls")));
        // The loop's own time sets the length.
        assert_eq!(replay.duration_ms, 141_500);
        assert!(replay.origin.contains("events.jsonl"));
    }

    #[test]
    fn started_ms_reads_seconds_and_milliseconds() {
        assert_eq!(started_ms("a-b-1790393791"), Some(1_790_393_791_000));
        assert_eq!(started_ms("a-b-1790430415859"), Some(1_790_430_415_859));
        assert_eq!(started_ms("a-b"), None);
    }
}
