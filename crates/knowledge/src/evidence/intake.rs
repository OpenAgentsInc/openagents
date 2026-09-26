//! Lossless intake and explicit comparison identities for knowledge evidence.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::digest;

const MAX_SUMMARY_BYTES: u64 = 16 * 1024 * 1024;

/// Whether an intake produced a usable run summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntakeStatus {
    Complete,
    Incomplete,
    Malformed,
    Unreadable,
}

/// What a run claims about how its comparison was organized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StudyKind {
    Observational,
    /// A declaration alone does not prove pre-run freezing or assignment.
    ProspectiveUnverified,
}

/// The exact entry version shown in a run, to the extent it was recorded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EntryPin {
    pub id: String,
    pub version: Option<u32>,
    pub digest: Option<String>,
}

/// Required cost components and the sum that is actually known.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cost {
    pub components: BTreeMap<String, Option<f64>>,
    pub known_lower_bound_usd: f64,
    /// Present only when all required components are valid and present.
    pub total_usd: Option<f64>,
    pub missing: Vec<String>,
}

impl Cost {
    fn read(value: &Value) -> Self {
        let outcome = &value["outcome"];
        let mut components = BTreeMap::new();
        for key in ["model_usd", "jev_usd", "embedding_usd"] {
            components.insert(key.to_string(), dollars(&outcome[key]));
        }
        // A producer can declare additional costs, but cannot remove the core
        // components from the denominator by supplying a shorter list.
        let mut invalid_declaration = false;
        if let Some(required) = value.get("required_cost_components") {
            if let Some(keys) = required.as_array().filter(|keys| !keys.is_empty()) {
                for key in keys {
                    if let Some(key) = key.as_str().filter(|key| !key.is_empty()) {
                        components.insert(key.to_string(), dollars(&outcome[key]));
                    } else {
                        invalid_declaration = true;
                    }
                }
            } else {
                invalid_declaration = true;
            }
        }
        if invalid_declaration {
            components.insert("invalid_required_cost_components".to_string(), None);
        }
        let mut known_lower_bound_usd: f64 = components.values().flatten().sum();
        let core = ["model_usd", "jev_usd", "embedding_usd"];
        let core_known: f64 = core.iter().filter_map(|key| components[*key]).sum();
        let core_complete = core.iter().all(|key| components[*key].is_some());
        if let Some(recorded) = outcome.get("known_usd") {
            match dollars(recorded) {
                Some(value)
                    if value + 1e-9 >= core_known
                        && (!core_complete || (value - core_known).abs() <= 1e-9) =>
                {
                    let extra: f64 = components
                        .iter()
                        .filter(|(key, _)| !core.contains(&key.as_str()))
                        .filter_map(|(_, value)| *value)
                        .sum();
                    known_lower_bound_usd = value + extra;
                }
                _ => {
                    components.insert("invalid_known_cost_lower_bound".into(), None);
                }
            }
        }
        if outcome
            .get("cost_unknown")
            .is_some_and(|value| !value.as_array().is_some_and(Vec::is_empty))
        {
            components.insert("producer_cost_unknown".into(), None);
        }
        let mut missing: Vec<String> = components
            .iter()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.clone())
            .collect();
        if !known_lower_bound_usd.is_finite() {
            missing.push("cost_sum_overflow".to_string());
        }
        Self {
            total_usd: missing.is_empty().then_some(known_lower_bound_usd),
            known_lower_bound_usd: if known_lower_bound_usd.is_finite() {
                known_lower_bound_usd
            } else {
                f64::MAX
            },
            components,
            missing,
        }
    }
}

fn dollars(value: &Value) -> Option<f64> {
    value.as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}

/// A summary's retained identities. Missing fields remain missing.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Identity {
    pub study_kind: StudyKind,
    /// Exact recorded object, never a synthesized claim of equivalence.
    pub recorded: Option<Value>,
    /// Digest only when all comparison identity fields are present.
    pub comparison_digest: Option<String>,
    pub missing: Vec<String>,
}

impl Identity {
    fn read(value: &Value) -> Self {
        let recorded = value.get("evidence_identity").cloned();
        let Some(identity) = recorded.as_ref().and_then(Value::as_object) else {
            return Self {
                study_kind: StudyKind::Observational,
                recorded,
                comparison_digest: None,
                missing: vec!["evidence_identity".to_string()],
            };
        };
        let mut missing = Vec::new();
        if identity.get("schema").and_then(Value::as_str)
            != Some("openagents.kb-evidence-identity.v1")
        {
            missing.push("schema".to_string());
        }
        for field in [
            "harness_digest",
            "configuration_digest",
            "environment_digest",
            "workload_digest",
            "context_digest",
        ] {
            if !identity
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(valid_digest)
            {
                missing.push(field.to_string());
            }
        }
        for field in ["model", "effort", "partition", "group"] {
            if !identity
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
            {
                missing.push(field.to_string());
            }
        }
        if identity.get("model") != value.get("model") {
            missing.push("model_mismatch".to_string());
        }
        if let Some(effort) = value.get("effort")
            && identity.get("effort") != Some(effort)
        {
            missing.push("effort_mismatch".to_string());
        }
        let budget = identity.get("budget");
        if !budget.and_then(Value::as_object).is_some_and(|budget| {
            ["steps", "tokens", "seconds", "usd"].iter().all(|key| {
                budget
                    .get(*key)
                    .and_then(Value::as_f64)
                    .is_some_and(|n| n.is_finite() && n >= 0.0)
            })
        }) {
            missing.push("budget".to_string());
        }
        let prospective = identity.contains_key("assignment");
        let mut comparison = identity.clone();
        comparison.remove("assignment");
        let comparison_digest = if missing.is_empty() {
            nostr::contracts::digest_value(&Value::Object(comparison)).ok()
        } else {
            None
        };
        Self {
            study_kind: if prospective {
                StudyKind::ProspectiveUnverified
            } else {
                StudyKind::Observational
            },
            recorded,
            comparison_digest,
            missing,
        }
    }
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// One attempted intake. Failed reads remain in the report denominator.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Run {
    pub name: String,
    pub source: String,
    pub task: String,
    pub model: String,
    pub reward: Option<f64>,
    pub cost: Cost,
    pub used: Vec<String>,
    pub knowledge: Vec<EntryPin>,
    /// False means that membership cannot safely be assigned to either arm.
    pub knowledge_complete: bool,
    pub started: u64,
    pub intake: IntakeStatus,
    pub problems: Vec<String>,
    pub identity: Identity,
    pub summary_digest: Option<String>,
    /// Retained as a separate content-addressed report artifact.
    #[serde(skip)]
    pub summary_bytes: Option<Vec<u8>>,
}

impl Run {
    /// Whether the recorded verifier gave the full reward.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.reward == Some(1.0)
    }

    fn missing(dir: &Path, status: IntakeStatus, problem: String) -> Self {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        Self {
            task: super::task_of(&name),
            started: name
                .rsplit_once('-')
                .and_then(|(_, stamp)| stamp.parse::<u64>().ok())
                .map(|stamp| {
                    if stamp >= 1_000_000_000_000 {
                        stamp / 1000
                    } else {
                        stamp
                    }
                })
                .unwrap_or(0),
            name,
            source: dir.display().to_string(),
            model: "unknown".to_string(),
            reward: None,
            cost: Cost::read(&Value::Null),
            used: Vec::new(),
            knowledge: Vec::new(),
            knowledge_complete: false,
            intake: status,
            problems: vec![problem],
            identity: Identity::read(&Value::Null),
            summary_digest: None,
            summary_bytes: None,
        }
    }
}

/// Reads a run while retaining malformed, incomplete, and unreadable intake.
#[must_use]
pub fn read_run(dir: &Path) -> Run {
    let path = dir.join("summary.json");
    let mut run = Run::missing(dir, IntakeStatus::Incomplete, "summary_missing".to_string());
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                run.intake = IntakeStatus::Unreadable;
                run.problems = vec![format!("summary_metadata: {error}")];
            }
            return run;
        }
    };
    if !metadata.is_file() || metadata.len() > MAX_SUMMARY_BYTES {
        run.intake = IntakeStatus::Unreadable;
        run.problems = vec!["summary must be a regular file of at most 16 MiB".to_string()];
        return run;
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            run.intake = IntakeStatus::Unreadable;
            run.problems = vec![format!("summary_read: {error}")];
            return run;
        }
    };
    run.summary_digest = Some(digest(&bytes));
    run.summary_bytes = Some(bytes.clone());
    let value: Value =
        match nostr::contracts::parse_strict_bounded(&bytes, MAX_SUMMARY_BYTES as usize) {
            Ok(value) => value,
            Err(error) => {
                run.intake = IntakeStatus::Malformed;
                run.problems = vec![format!("summary_json: {error}")];
                return run;
            }
        };
    run.problems.clear();
    run.intake = IntakeStatus::Complete;
    if let Some(task) = value["task"].as_str().filter(|s| !s.is_empty()) {
        run.task = task.to_string();
    } else {
        run.problems.push("task_missing".to_string());
    }
    if let Some(model) = value["model"].as_str().filter(|s| !s.is_empty()) {
        run.model = model.to_string();
    } else {
        run.problems.push("model_missing".to_string());
    }
    run.reward = value["reward"]
        .as_f64()
        .filter(|r| r.is_finite() && (0.0..=1.0).contains(r));
    if run.reward.is_none() {
        run.problems.push("reward_unknown".to_string());
    }
    run.cost = Cost::read(&value);
    run.identity = Identity::read(&value);
    if let Some(knowledge) = value["outcome"]["knowledge"].as_array() {
        run.knowledge_complete = true;
        for item in knowledge {
            if let Some(id) = item["id"].as_str().filter(|id| !id.is_empty()) {
                run.used.push(id.to_string());
                run.knowledge.push(EntryPin {
                    id: id.to_string(),
                    version: item["version"]
                        .as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .filter(|v| *v > 0),
                    digest: item["digest"]
                        .as_str()
                        .filter(|s| valid_digest(s))
                        .map(str::to_string),
                });
            } else {
                run.knowledge_complete = false;
                run.problems.push("knowledge_id_missing".to_string());
            }
        }
    } else {
        run.problems
            .push("knowledge_membership_unknown".to_string());
    }
    if !run.problems.is_empty() {
        run.intake = IntakeStatus::Incomplete;
    }
    run
}

/// Every child directory is an attempt, even before its summary exists.
/// Listing errors are retained as intake faults, never an empty success.
#[must_use]
pub fn scan(dir: &Path) -> Vec<Run> {
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(error) => {
            return vec![Run::missing(
                dir,
                IntakeStatus::Unreadable,
                format!("runs_directory: {error}"),
            )];
        }
    };
    let mut runs = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in listing {
        match entry {
            Ok(entry) => match entry.file_type() {
                Ok(kind) if kind.is_dir() => dirs.push(entry.path()),
                Ok(kind) if kind.is_symlink() => runs.push(Run::missing(
                    &entry.path(),
                    IntakeStatus::Unreadable,
                    "run_directory_symlink".to_string(),
                )),
                Ok(_) => {}
                Err(error) => runs.push(Run::missing(
                    &entry.path(),
                    IntakeStatus::Unreadable,
                    format!("run_directory_metadata: {error}"),
                )),
            },
            Err(error) => runs.push(Run::missing(
                dir,
                IntakeStatus::Unreadable,
                format!("runs_directory_entry: {error}"),
            )),
        }
    }
    dirs.sort();
    runs.extend(dirs.iter().map(|dir| read_run(dir)));
    runs.sort_by(|a, b| a.source.cmp(&b.source));
    runs
}
