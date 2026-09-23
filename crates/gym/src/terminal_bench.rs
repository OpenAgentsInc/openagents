//! Read Terminal-Bench episodes without changing the decision-row store.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const ATTEMPT_SCHEMA: &str = "openagents.tbench.attempt.v1";
const MANIFEST_SCHEMA: &str = "openagents.tbench.episode-manifest.v1";
const RETENTION_SCHEMA: &str = "openagents.tbench.retention.v1";

/// The status of an attempt a provider's usage or rate limit throttled.
pub const USAGE_LIMITED: &str = "usage_limited";

/// Exceptions that mean a provider throttled the agent: the harness's own
/// and Harbor's two classifications of a CLI's limit output.
const USAGE_LIMIT_EXCEPTIONS: [&str; 3] =
    ["UsageLimitError", "ApiRateLimitError", "ApiUsageLimitError"];

/// The note a usage limit's record reads as.
fn usage_limit_note(limit: &Value) -> Option<String> {
    let message = limit.get("message").and_then(Value::as_str);
    let resets = limit.get("resets_at_iso").and_then(Value::as_str);
    if message.is_none() && resets.is_none() {
        return None;
    }
    Some(format!(
        "Usage limit ({}): {} Resets {}.",
        limit
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("unknown provider"),
        message.unwrap_or("no message recorded."),
        resets.unwrap_or("at an unknown time")
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvidenceState {
    Verified,
    Missing,
    Edited,
    Sanitized,
    NotPresent,
    Unresolved,
    Unchecked,
}

impl EvidenceState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Missing => "missing",
            Self::Edited => "digest mismatch",
            Self::Sanitized => "sanitized copy",
            Self::NotPresent => "none recorded",
            Self::Unresolved => "unresolved",
            Self::Unchecked => "no digest",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Evidence {
    pub kind: String,
    pub path: Option<PathBuf>,
    pub state: EvidenceState,
    pub note: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub source: String,
    pub job: String,
    pub trial: String,
    pub arm: String,
    pub profile: String,
    pub kind: String,
    pub task: String,
    pub commit: Option<String>,
    pub checksum: Option<String>,
    pub architecture: Option<String>,
    pub host: Option<String>,
    pub image_state: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub artifact: Option<String>,
    pub reward: Option<f64>,
    pub status: String,
    pub started_at: Option<String>,
    pub phases_ms: [Option<u64>; 5],
    /// How the agent was installed: `prebuilt`, `network`, or unknown.
    pub setup_mode: Option<String>,
    /// Whether the toolchain came from a warm cache, a cold build, or none.
    pub setup_cache: Option<String>,
    /// The toolchain install alone, inside Harbor's agent_setup phase.
    pub setup_install_ms: Option<u64>,
    pub input_tokens: Option<u64>,
    pub cache_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub usage_coverage: String,
    pub cost_usd: Option<f64>,
    pub cost_provenance: String,
    pub costs: Vec<(String, Option<f64>, String)>,
    pub counts: Vec<(String, Option<u64>)>,
    pub evidence: Vec<Evidence>,
    pub notes: Vec<String>,
    /// The Coder One policy manifest the episode resolved, when its
    /// manifest records one.
    pub policy: Option<crate::coder_policy::PolicyRecord>,
    /// Coder One's per-call ledger from `evaluation/usage.json`: each
    /// call's charge (`priced`, `zero`, or `unknown`), cost, and provenance.
    pub ledger: Vec<Value>,
    /// The episode deadline's record from the episode manifest.
    pub deadline: Option<Value>,
    /// The delegate's agent (`claude-code` or `codex`) from the episode
    /// manifest.
    pub delegate_agent: Option<String>,
    /// The delegate session's native turns and model calls.
    pub delegate_turns: Option<u64>,
    pub delegate_calls: Option<u64>,
    /// The system prompt the delegate was sent (`exec.system`), when the
    /// episode recorded it.
    pub system: Option<Value>,
    /// The tunable composition's record, `artifacts/composition.json`,
    /// when the episode ran one.
    pub composition: Option<Value>,
}

impl Attempt {
    pub fn is_control(&self) -> bool {
        matches!(self.arm.as_str(), "oracle" | "nop")
    }

    pub fn has_complete_comparison_identity(&self) -> bool {
        self.commit.is_some()
            && self.checksum.is_some()
            && self.architecture.is_some()
            && self.host.is_some()
            && self.image_state.is_some()
            && self.model.is_some()
            && self.artifact.is_some()
    }

    pub fn pin(&self) -> String {
        format!(
            "{} / {} / {} / {} / {}",
            self.commit.as_deref().unwrap_or("unknown commit"),
            self.checksum.as_deref().unwrap_or("unknown checksum"),
            self.architecture
                .as_deref()
                .unwrap_or("unknown architecture"),
            self.host.as_deref().unwrap_or("unknown host"),
            self.image_state.as_deref().unwrap_or("unknown image state")
        )
    }

    pub fn evidence_health(&self) -> &'static str {
        if self
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Edited)
        {
            "digest mismatch"
        } else if self
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Missing)
        {
            "missing evidence"
        } else if self
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Unresolved)
        {
            "unresolved evidence"
        } else if self
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Sanitized)
        {
            "sanitized copy"
        } else if self
            .evidence
            .iter()
            .any(|e| e.state == EvidenceState::Verified)
        {
            "verified files"
        } else {
            "not checked"
        }
    }

    /// Evidence the attempt references but nobody retained.
    pub fn missing_evidence(&self) -> Vec<&Evidence> {
        self.evidence
            .iter()
            .filter(|e| e.state == EvidenceState::Missing)
            .collect()
    }

    /// Whether the attempt ended in setup rather than in the task.
    pub fn is_setup_failure(&self) -> bool {
        matches!(self.status.as_str(), "install_failure" | "setup_failure")
    }

    /// The setup cache state, for grouping setup times.
    pub fn setup_cache_label(&self) -> &str {
        self.setup_cache.as_deref().unwrap_or("unknown")
    }

    /// Whether a provider's usage or rate limit throttled the attempt's
    /// session: an infrastructure failure, never a graded result.
    pub fn is_usage_limited(&self) -> bool {
        self.status == USAGE_LIMITED
    }

    /// Marks the attempt usage-limited: its reward says nothing about the
    /// agent, so it is withheld, and the limit is noted.
    pub fn mark_usage_limited(&mut self, limit: &Value) {
        if !self.is_usage_limited() {
            if let Some(reward) = self.reward {
                self.notes.push(format!(
                    "Verifier reward {reward} withheld: the session was usage-limited."
                ));
            }
            self.status = USAGE_LIMITED.to_owned();
            self.reward = None;
        }
        if let Some(note) = usage_limit_note(limit)
            && !self.notes.contains(&note)
        {
            self.notes.push(note);
        }
    }

    pub fn display_status(&self) -> &str {
        if self.status == "completed" && self.reward == Some(0.0) {
            "task failure"
        } else if self.status == "completed" && self.reward.is_none() {
            "unverifiable"
        } else {
            &self.status
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Records {
    pub attempts: Vec<Attempt>,
    pub errors: Vec<String>,
    pub sources: Vec<String>,
    pub report_label: Option<String>,
    pub report_warnings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ComparisonGroup {
    pub task: String,
    pub pin: String,
    /// The arm, or `policy <digest prefix>` when the attempts recorded a
    /// policy manifest: attempts group by what ran, not by arm name.
    pub arm: String,
    /// The policy manifest digest the group shares, when there is one.
    pub policy: Option<String>,
    /// The arm names of the group's attempts.
    pub arms: Vec<String>,
    /// The job profile the group's attempts ran under; the pin includes
    /// it, so a group never mixes profiles.
    pub profile: String,
    pub attempts: Vec<usize>,
}

impl ComparisonGroup {
    /// Whether `query` names this group: its label, one of its arms, or a
    /// prefix of its policy digest.
    pub fn named(&self, query: &str) -> bool {
        self.arm == query
            || self.arms.iter().any(|arm| arm == query)
            || self
                .policy
                .as_deref()
                .is_some_and(|digest| query.len() >= 6 && digest.starts_with(query))
    }
}

impl ComparisonGroup {
    pub fn from_records(records: &Records) -> Vec<Self> {
        let mut grouped: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
        for (index, attempt) in records.attempts.iter().enumerate() {
            let identity = format!(
                "{} / profile {} / model {} / artifact {}",
                attempt.pin(),
                attempt.profile,
                attempt.model.as_deref().unwrap_or("unknown"),
                attempt.artifact.as_deref().unwrap_or("unknown")
            );
            let pin = if attempt.has_complete_comparison_identity() {
                identity
            } else {
                format!("{identity} / job {}", attempt.job)
            };
            // Attempts that recorded a policy manifest group by its digest,
            // so two arm names running one configuration pool, and one arm
            // name whose configuration changed splits.
            let side = attempt.policy.as_ref().map_or_else(
                || attempt.arm.clone(),
                |policy| format!("policy {}", policy.digest),
            );
            grouped
                .entry((attempt.task.clone(), pin, side))
                .or_default()
                .push(index);
        }
        grouped
            .into_iter()
            .map(|((task, pin, side), attempts)| {
                let mut arms: Vec<String> = attempts
                    .iter()
                    .map(|&index| records.attempts[index].arm.clone())
                    .collect();
                arms.sort();
                arms.dedup();
                let policy = side.strip_prefix("policy ").map(str::to_owned);
                let profile = attempts
                    .first()
                    .map(|&index| records.attempts[index].profile.clone())
                    .unwrap_or_default();
                let arm = match &policy {
                    Some(digest) => format!("policy {}", crate::coder_policy::short(digest)),
                    None => side,
                };
                Self {
                    task,
                    pin,
                    arm,
                    policy,
                    arms,
                    profile,
                    attempts,
                }
            })
            .collect()
    }
}

impl Records {
    /// Read local Harbor jobs and the sanitized evidence kept in the checkout.
    pub fn load(jobs: Option<&Path>, traces: Option<&Path>, samples: Option<&Path>) -> Self {
        let mut records = Self::default();
        let mut seen = BTreeSet::new();
        for (root, name, reader) in [
            (
                jobs,
                "local jobs",
                read_jobs as fn(&Path, &mut Records, &mut BTreeSet<String>),
            ),
            (traces, "retained traces", read_traces),
            (samples, "checked samples", read_samples),
        ] {
            if let Some(root) = root {
                if root.is_dir() {
                    records.sources.push(format!("{name}: {}", root.display()));
                    reader(root, &mut records, &mut seen);
                } else {
                    records
                        .errors
                        .push(format!("{name} unavailable: {}", root.display()));
                }
            }
        }
        if let Some(jobs) = jobs {
            let report = jobs.join("tbench-report.json");
            if report.is_file() {
                match read_json(&report) {
                    Ok(value)
                        if string(&value, "/schema").as_deref()
                            == Some("openagents.tbench.report.v1") =>
                    {
                        records.report_label = string(&value, "/label");
                        records.report_warnings.extend(
                            value
                                .get("pin_warnings")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .filter_map(Value::as_str)
                                .map(str::to_owned),
                        );
                        if value
                            .get("attempts_total")
                            .and_then(Value::as_u64)
                            .is_some_and(|n| {
                                n != records
                                    .attempts
                                    .iter()
                                    .filter(|a| a.source == "local job")
                                    .count() as u64
                            })
                        {
                            records.report_warnings.push("The saved report has a different attempt count; its aggregates may be stale.".to_owned());
                        }
                    }
                    Ok(_) => records
                        .errors
                        .push(format!("{}: unsupported report schema", report.display())),
                    Err(error) => records.errors.push(error),
                }
            }
        }
        records.attempts.sort_by(|a, b| {
            a.task
                .cmp(&b.task)
                .then(a.arm.cmp(&b.arm))
                .then(a.job.cmp(&b.job))
                .then(a.trial.cmp(&b.trial))
        });
        records
    }

    /// Attempts a provider's usage or rate limit throttled: never graded.
    pub fn usage_limited(&self) -> usize {
        self.attempts
            .iter()
            .filter(|attempt| attempt.is_usage_limited())
            .count()
    }

    pub fn status_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for attempt in &self.attempts {
            *counts
                .entry(attempt.display_status().to_owned())
                .or_default() += 1;
        }
        counts
    }
}

fn read_jobs(root: &Path, records: &mut Records, seen: &mut BTreeSet<String>) {
    for job in children(root) {
        let attempts = job.join("tbench/attempts");
        for path in children(&attempts)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
        {
            match read_json(&path).and_then(|value| parse_attempt(&value, &path, "local job", &job))
            {
                Ok(mut attempt) => {
                    let manifest = job
                        .join("tbench/manifests")
                        .join(format!("{}.json", attempt.trial));
                    attach_manifest(&mut attempt, &manifest, None, records);
                    let episode = job.join(&attempt.trial).join("agent/episode");
                    attach_episode(
                        &mut attempt,
                        &episode.join("manifest.json"),
                        &episode,
                        records,
                    );
                    attach_usage(
                        &mut attempt,
                        &episode.join("evaluation/usage.json"),
                        records,
                    );
                    apply_manual_price(&mut attempt);
                    let verifier = job.join(&attempt.trial).join("verifier/ctrf.json");
                    attach_verifier(&mut attempt, &verifier);
                    if seen.insert(format!("{}: {}", attempt.job, attempt.trial)) {
                        records.attempts.push(attempt);
                    }
                }
                Err(error) => records.errors.push(error),
            }
        }
    }
}

fn read_samples(root: &Path, records: &mut Records, seen: &mut BTreeSet<String>) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for child in children(&directory) {
            if child.is_dir() && child.file_name().is_none_or(|name| name != "evidence") {
                pending.push(child);
            }
        }
        let sample = directory;
        let path = sample.join("attempt.json");
        if !path.is_file() {
            continue;
        }
        match read_json(&path)
            .and_then(|value| parse_attempt(&value, &path, "checked sample", &sample))
        {
            Ok(mut attempt) => {
                attach_manifest(
                    &mut attempt,
                    &sample.join("manifest.json"),
                    Some(&sample.join("evidence")),
                    records,
                );
                attach_verifier(&mut attempt, &sample.join("evidence/verifier/ctrf.json"));
                if seen.insert(format!("{}: {}", attempt.job, attempt.trial)) {
                    records.attempts.push(attempt);
                }
            }
            Err(error) => records.errors.push(error),
        }
    }
}

fn read_traces(root: &Path, records: &mut Records, seen: &mut BTreeSet<String>) {
    for job in children(root) {
        if !job.is_dir() {
            continue;
        }
        let job_name = name(&job);
        for path in children(&job)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
        {
            let trial = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            if seen.contains(&format!("{job_name}: {trial}")) {
                continue;
            }
            let episode = job.join(format!("{trial}.episode"));
            let harbor = episode.join("harbor-result.json");
            if !harbor.is_file() {
                let mut attempt = empty_attempt("retained trace", &job_name, &trial);
                attempt.task = trial.split("__").next().unwrap_or("unknown").to_owned();
                attempt.status = "unverifiable".to_owned();
                attempt
                    .notes
                    .push("No retained Harbor result; reward and timing are unknown.".to_owned());
                attempt.evidence.push(Evidence {
                    kind: "trajectory".to_owned(),
                    path: Some(path.clone()),
                    state: EvidenceState::Unchecked,
                    note: None,
                });
                attach_trajectory(&mut attempt, &path);
                records.attempts.push(attempt);
                seen.insert(format!("{job_name}: {trial}"));
                continue;
            }
            match read_json(&harbor) {
                Ok(value) => {
                    let mut attempt = empty_attempt("retained trace", &job_name, &trial);
                    attempt.task = string(&value, "/task_name").unwrap_or_else(|| {
                        trial.split("__").next().unwrap_or("unknown").to_owned()
                    });
                    attempt.reward = value
                        .pointer("/verifier_result/rewards/reward")
                        .and_then(Value::as_f64);
                    attempt.status = retained_status(&value, attempt.reward).to_owned();
                    if attempt.status == USAGE_LIMITED {
                        attempt.status = "completed".to_owned();
                        attempt.mark_usage_limited(&Value::Null);
                    }
                    attempt.started_at = string(&value, "/started_at");
                    attempt.phases_ms = [
                        "/environment_setup",
                        "/agent_setup",
                        "/agent_execution",
                        "/verifier",
                        "",
                    ]
                    .map(|p| elapsed(&value, p));
                    attempt.input_tokens = value
                        .pointer("/agent_result/n_input_tokens")
                        .and_then(Value::as_u64);
                    attempt.cache_tokens = value
                        .pointer("/agent_result/n_cache_tokens")
                        .and_then(Value::as_u64);
                    attempt.output_tokens = value
                        .pointer("/agent_result/n_output_tokens")
                        .and_then(Value::as_u64);
                    attempt.cost_usd = value
                        .pointer("/agent_result/cost_usd")
                        .and_then(Value::as_f64);
                    attempt.usage_coverage =
                        coverage(attempt.input_tokens, attempt.output_tokens).to_owned();
                    attempt.cost_provenance =
                        if attempt.cost_usd.is_some() && attempt.arm.starts_with("claude-code") {
                            "CLI list price; subscription reference"
                        } else {
                            "unknown"
                        }
                        .to_owned();
                    attempt.evidence.push(Evidence {
                        kind: "trajectory".to_owned(),
                        path: Some(path.clone()),
                        state: EvidenceState::Unchecked,
                        note: None,
                    });
                    attach_trajectory(&mut attempt, &path);
                    attempt.evidence.push(Evidence {
                        kind: "Harbor result".to_owned(),
                        path: Some(harbor),
                        state: EvidenceState::Unchecked,
                        note: None,
                    });
                    let manifest = episode.join("manifest.json");
                    attach_episode(&mut attempt, &manifest, &episode, records);
                    // Older retention kept usage beside the manifest;
                    // `tbench retain` mirrors the episode's own layout.
                    let usage = episode.join("usage.json");
                    let usage = if usage.is_file() {
                        usage
                    } else {
                        episode.join("evaluation/usage.json")
                    };
                    attach_usage(&mut attempt, &usage, records);
                    attach_retention(&mut attempt, &job, &episode, records);
                    attach_setup(&mut attempt, &episode.join("setup/toolchain-setup.json"));
                    attach_verifier(&mut attempt, &episode.join("verifier/ctrf.json"));
                    apply_manual_price(&mut attempt);
                    records.attempts.push(attempt);
                    seen.insert(format!("{job_name}: {trial}"));
                }
                Err(error) => records.errors.push(error),
            }
        }
    }
}

fn parse_attempt(
    value: &Value,
    path: &Path,
    source: &str,
    job_dir: &Path,
) -> Result<Attempt, String> {
    if string(value, "/schema").as_deref() != Some(ATTEMPT_SCHEMA) {
        return Err(format!("{}: unsupported attempt schema", path.display()));
    }
    let job = required(value, "/attempt/job", path)?;
    let trial = required(value, "/attempt/trial", path)?;
    let arm = required(value, "/attempt/arm", path)?;
    let task = required(value, "/task/name", path)?;
    let mut attempt = empty_attempt(source, &job, &trial);
    attempt.arm = arm;
    attempt.task = task;
    attempt.profile = string(value, "/attempt/profile").unwrap_or_default();
    attempt.kind = string(value, "/attempt/kind").unwrap_or_else(|| "unknown".to_owned());
    attempt.commit =
        string(value, "/task/git_commit_id").or_else(|| string(value, "/task/pin/git_commit_id"));
    attempt.checksum = string(value, "/task/checksum");
    attempt.architecture = string(value, "/task/pin/architecture")
        .or_else(|| string(value, "/environment/architecture"));
    attempt.host = string(value, "/environment/host").or_else(|| string(value, "/task/pin/host"));
    attempt.image_state = string(value, "/environment/image_state");
    attempt.agent =
        string(value, "/agent/observed_name").or_else(|| string(value, "/agent/selector"));
    attempt.model =
        string(value, "/agent/observed_model").or_else(|| string(value, "/agent/requested_model"));
    for (label, pointer) in [
        ("Agent selector", "/agent/selector"),
        ("Agent version", "/agent/observed_version"),
        ("Requested model", "/agent/requested_model"),
        ("Observed provider", "/agent/observed_provider"),
        ("Authentication mode", "/attempt/auth_mode"),
        ("Task path", "/task/path"),
        ("Image", "/environment/image"),
        ("Image source", "/environment/image_source"),
        ("Image action", "/environment/image_action"),
        ("Image state method", "/environment/image_state_method"),
    ] {
        if let Some(detail) = string(value, pointer) {
            attempt.notes.push(format!("{label}: {detail}"));
        }
    }
    attempt.reward = value.pointer("/outcome/reward").and_then(Value::as_f64);
    attempt.status =
        string(value, "/outcome/terminal_status").unwrap_or_else(|| "unknown".to_owned());
    if let Some(exception) = string(value, "/outcome/exception/exception_type") {
        attempt.notes.push(format!("Exception: {exception}"));
        if USAGE_LIMIT_EXCEPTIONS.contains(&exception.as_str()) {
            attempt.mark_usage_limited(&Value::Null);
        }
    }
    if let Some(limit) = value
        .pointer("/outcome/usage_limit")
        .filter(|v| v.is_object())
    {
        attempt.mark_usage_limited(limit);
    }
    attempt.started_at = string(value, "/timing/started_at");
    attempt.phases_ms = [
        "environment_setup_ms",
        "agent_setup_ms",
        "agent_execution_ms",
        "verifier_ms",
        "total_ms",
    ]
    .map(|field| {
        value
            .pointer(&format!("/timing/{field}"))
            .and_then(Value::as_u64)
    });
    attempt.setup_mode = string(value, "/setup/mode");
    attempt.setup_cache = string(value, "/setup/cache");
    attempt.setup_install_ms = value.pointer("/setup/install_ms").and_then(Value::as_u64);
    attempt.input_tokens = value.pointer("/usage/input_tokens").and_then(Value::as_u64);
    attempt.cache_tokens = value.pointer("/usage/cache_tokens").and_then(Value::as_u64);
    attempt.output_tokens = value
        .pointer("/usage/output_tokens")
        .and_then(Value::as_u64);
    attempt.usage_coverage = string(value, "/usage/coverage")
        .unwrap_or_else(|| coverage(attempt.input_tokens, attempt.output_tokens).to_owned());
    attempt.cost_usd = value.pointer("/cost/amount_usd").and_then(Value::as_f64);
    attempt.cost_provenance =
        string(value, "/cost/provenance").unwrap_or_else(|| "unknown".to_owned());
    for field in [
        "atif_steps",
        "model_invocations",
        "typed_decisions",
        "tool_calls",
        "shell_commands",
        "retries",
        "subagent_steps",
    ] {
        attempt.counts.push((
            field.to_owned(),
            value
                .pointer(&format!("/counts/{field}"))
                .and_then(Value::as_u64),
        ));
    }
    if let Some(semantics) = string(value, "/counts/semantics") {
        attempt.notes.push(format!("Counts: {semantics}"));
    }
    if let Some(note) = string(value, "/cost/note") {
        attempt.notes.push(note);
    }
    attempt.evidence.push(Evidence {
        kind: "attempt record".to_owned(),
        path: Some(path.to_path_buf()),
        state: EvidenceState::Unchecked,
        note: None,
    });
    let trial_dir = job_dir.join(&trial);
    for (kind, relative) in [
        ("trial result", "result.json"),
        ("trajectory", "agent/trajectory.json"),
        ("verifier report", "verifier/ctrf.json"),
        ("verifier reward", "verifier/reward.txt"),
    ] {
        let path = trial_dir.join(relative);
        if path.is_file() {
            attempt.evidence.push(Evidence {
                kind: kind.to_owned(),
                state: EvidenceState::Unchecked,
                path: Some(path),
                note: None,
            });
        }
    }
    Ok(attempt)
}

fn empty_attempt(source: &str, job: &str, trial: &str) -> Attempt {
    Attempt {
        source: source.to_owned(),
        job: job.to_owned(),
        trial: trial.to_owned(),
        arm: job.split("--").nth(1).unwrap_or("unknown").to_owned(),
        profile: job.split("--").next().unwrap_or("unknown").to_owned(),
        kind: "unknown".to_owned(),
        task: "unknown".to_owned(),
        commit: None,
        checksum: None,
        architecture: None,
        host: None,
        image_state: None,
        agent: None,
        model: None,
        artifact: None,
        reward: None,
        status: "unknown".to_owned(),
        started_at: None,
        phases_ms: [None; 5],
        setup_mode: None,
        setup_cache: None,
        setup_install_ms: None,
        input_tokens: None,
        cache_tokens: None,
        output_tokens: None,
        usage_coverage: "unknown".to_owned(),
        cost_usd: None,
        cost_provenance: "unknown".to_owned(),
        costs: Vec::new(),
        counts: Vec::new(),
        evidence: Vec::new(),
        notes: Vec::new(),
        policy: None,
        ledger: Vec::new(),
        deadline: None,
        delegate_agent: None,
        delegate_turns: None,
        delegate_calls: None,
        system: None,
        composition: None,
    }
}

#[cfg(test)]
pub(crate) fn test_attempt() -> Attempt {
    let mut attempt = empty_attempt("test", "smoke--agent", "trial");
    attempt.task = "terminal-bench/example".to_owned();
    attempt
}

fn attach_manifest(
    attempt: &mut Attempt,
    path: &Path,
    sample_evidence: Option<&Path>,
    records: &mut Records,
) {
    if !path.is_file() {
        attempt.notes.push("Episode manifest missing.".to_owned());
        return;
    }
    match read_json(path) {
        Ok(value) if string(&value, "/schema").as_deref() == Some(MANIFEST_SCHEMA) => {
            attempt.evidence.push(Evidence {
                kind: "episode manifest".to_owned(),
                path: Some(path.to_path_buf()),
                state: EvidenceState::Unchecked,
                note: None,
            });
            if let Some(evidence) = value.get("evidence") {
                collect_evidence(evidence, attempt, sample_evidence);
            }
            if let Some(notes) = value.get("task_notes").and_then(Value::as_array) {
                attempt
                    .notes
                    .extend(notes.iter().filter_map(Value::as_str).map(str::to_owned));
            }
        }
        Ok(_) => records.errors.push(format!(
            "{}: unsupported episode manifest schema",
            path.display()
        )),
        Err(error) => records.errors.push(error),
    }
}

fn collect_evidence(value: &Value, attempt: &mut Attempt, sample_evidence: Option<&Path>) {
    match value {
        Value::Object(map) if map.contains_key("kind") => {
            let kind = string(value, "/kind").unwrap_or_else(|| "evidence".to_owned());
            let original = string(value, "/path");
            let path = original.as_deref().and_then(|p| {
                if p.starts_with("<jobs-dir>/") {
                    sample_evidence.map(|base| {
                        let marker = format!("/{}/", attempt.trial);
                        if let Some((_, relative)) = p.split_once(&marker) {
                            base.join(relative)
                        } else if p.contains("/tbench/attempts/") {
                            base.parent().unwrap_or(base).join("attempt.json")
                        } else {
                            base.join(p.rsplit('/').next().unwrap_or(p))
                        }
                    })
                } else {
                    Some(PathBuf::from(p))
                }
            });
            let digest = string(value, "/sha256");
            let state = match (
                value.pointer("/resolved").and_then(Value::as_bool),
                path.as_deref(),
                digest.as_deref(),
            ) {
                (Some(false), None, None) if kind == "collection-failure" => {
                    EvidenceState::NotPresent
                }
                (Some(false), _, _) | (_, None, _) => EvidenceState::Unresolved,
                (_, Some(path), _) if !path.is_file() => EvidenceState::Missing,
                (_, Some(path), Some(digest)) => {
                    if sha256(path).as_deref() == Some(digest) {
                        EvidenceState::Verified
                    } else if sample_evidence.is_some() {
                        EvidenceState::Sanitized
                    } else {
                        EvidenceState::Edited
                    }
                }
                _ => EvidenceState::Unchecked,
            };
            attempt.evidence.push(Evidence {
                kind,
                path,
                state,
                note: original.filter(|p| p.starts_with("<jobs-dir>/")),
            });
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_evidence(item, attempt, sample_evidence);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_evidence(item, attempt, sample_evidence);
            }
        }
        _ => {}
    }
}

fn attach_episode(attempt: &mut Attempt, path: &Path, episode: &Path, records: &mut Records) {
    if !path.is_file() {
        return;
    }
    match read_json(path) {
        Ok(value)
            if string(&value, "/contract").as_deref() == Some("openagents.coder.episode.v1") =>
        {
            attempt.artifact = string(&value, "/artifact/version");
            if let Some(limit) = value.get("usage_limit").filter(|v| v.is_object()) {
                attempt.mark_usage_limited(limit);
            } else if string(&value, "/outcome").as_deref() == Some(USAGE_LIMITED) {
                attempt.mark_usage_limited(&Value::Null);
            }
            attempt.policy = value
                .get("policy")
                .and_then(crate::coder_policy::PolicyRecord::from_episode);
            attempt.deadline = value.get("deadline").cloned();
            if let Some(policy) = &attempt.policy {
                attempt.notes.push(format!(
                    "Policy {} {} ({}, {} overrides)",
                    policy.name.as_deref().unwrap_or("unnamed"),
                    policy.short(),
                    policy.source.as_deref().unwrap_or("unknown source"),
                    policy.overrides.len()
                ));
            }
            attempt.evidence.push(Evidence {
                kind: "agent episode manifest".to_owned(),
                path: Some(path.to_path_buf()),
                state: EvidenceState::Unchecked,
                note: None,
            });
            for (kind, file) in value
                .get("files")
                .and_then(Value::as_object)
                .into_iter()
                .flat_map(|m| m.iter())
            {
                if let Some(relative) = file.get("path").and_then(Value::as_str) {
                    let mut full = episode.join(relative);
                    // Retention before `tbench retain` flattened the
                    // episode, keeping `evaluation/usage.json` as `usage.json`.
                    if let Some(flat) = Path::new(relative).file_name().map(|n| episode.join(n))
                        && !full.is_file()
                        && flat.is_file()
                    {
                        full = flat;
                    }
                    let state = if !full.is_file() {
                        EvidenceState::Missing
                    } else if file
                        .get("sha256")
                        .and_then(Value::as_str)
                        .is_some_and(|digest| sha256(&full).as_deref() != Some(digest))
                    {
                        EvidenceState::Edited
                    } else {
                        EvidenceState::Verified
                    };
                    attempt.evidence.push(Evidence {
                        kind: kind.clone(),
                        path: Some(full),
                        state,
                        note: None,
                    });
                }
            }
            attempt.composition = value
                .pointer("/files/composition/path")
                .and_then(Value::as_str)
                .filter(|relative| !relative.contains(".."))
                .and_then(|relative| read_json(&episode.join(relative)).ok());
            attempt.delegate_agent = string(&value, "/delegate/agent");
            if let Some(delegate) = value.pointer("/delegate/delegation") {
                attempt.delegate_turns = delegate.get("num_turns").and_then(Value::as_u64);
                attempt.delegate_calls = delegate.get("api_calls").and_then(Value::as_u64);
                attempt.system = delegate.get("system").filter(|v| !v.is_null()).cloned();
                attempt.notes.push(format!(
                    "Delegate: {} {} · {} turns · briefing {} characters",
                    string(&value, "/delegate/agent").unwrap_or_default(),
                    string(&value, "/delegate/model").unwrap_or_default(),
                    delegate
                        .get("num_turns")
                        .and_then(Value::as_u64)
                        .map_or("—".to_owned(), |v| v.to_string()),
                    delegate
                        .pointer("/briefing/chars")
                        .and_then(Value::as_u64)
                        .map_or("—".to_owned(), |v| v.to_string())
                ));
                if let Some(omitted) = delegate
                    .pointer("/briefing/omitted")
                    .and_then(Value::as_array)
                {
                    attempt
                        .notes
                        .push(format!("Briefing omitted {} items", omitted.len()));
                }
                if delegate
                    .pointer("/stream/truncated")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    attempt.notes.push("Delegate stream truncated".to_owned());
                }
            }
        }
        Ok(_) => records.errors.push(format!(
            "{}: unsupported agent episode contract",
            path.display()
        )),
        Err(error) => records.errors.push(error),
    }
}

/// Fold a `tbench retain` record into the attempt: each retained file is
/// checked against the digest taken at retention, and each reference the
/// retention could not copy stays visible as missing.
fn attach_retention(attempt: &mut Attempt, job: &Path, episode: &Path, records: &mut Records) {
    let path = episode.join("retention.json");
    if !path.is_file() {
        return;
    }
    let value = match read_json(&path) {
        Ok(value) if string(&value, "/schema").as_deref() == Some(RETENTION_SCHEMA) => value,
        Ok(_) => {
            records
                .errors
                .push(format!("{}: unsupported retention schema", path.display()));
            return;
        }
        Err(error) => {
            records.errors.push(error);
            return;
        }
    };
    attempt.evidence.push(Evidence {
        kind: "retention record".to_owned(),
        path: Some(path.clone()),
        state: EvidenceState::Unchecked,
        note: None,
    });
    let known: BTreeSet<PathBuf> = attempt
        .evidence
        .iter()
        .filter_map(|e| e.path.clone())
        .collect();
    let resolve = |relative: &str| {
        if relative == format!("{}.json", attempt.trial) {
            job.join(relative)
        } else {
            episode.join(relative)
        }
    };
    for file in value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(relative) = file.get("path").and_then(Value::as_str) else {
            continue;
        };
        let full = resolve(relative);
        if known.contains(&full) {
            continue;
        }
        let state = if !full.is_file() {
            EvidenceState::Missing
        } else {
            match file.get("sha256").and_then(Value::as_str) {
                Some(digest) if sha256(&full).as_deref() == Some(digest) => EvidenceState::Verified,
                Some(_) => EvidenceState::Edited,
                None => EvidenceState::Unchecked,
            }
        };
        attempt.evidence.push(Evidence {
            kind: string(file, "/kind").unwrap_or_else(|| "retained file".to_owned()),
            path: Some(full),
            state,
            note: None,
        });
    }
    for missing in value
        .get("missing")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let reference = string(missing, "/reference").unwrap_or_default();
        let full = resolve(&reference);
        if known.contains(&full) {
            continue;
        }
        attempt.evidence.push(Evidence {
            kind: string(missing, "/kind").unwrap_or_else(|| "evidence".to_owned()),
            path: Some(full),
            state: EvidenceState::Missing,
            note: string(missing, "/reason"),
        });
    }
    for note in value
        .get("notes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        attempt.notes.push(format!("Retention: {note}"));
    }
    if let Some(conversion) = string(&value, "/atif/conversion") {
        attempt.notes.push(format!("ATIF: {conversion}"));
    }
}

/// Read the adapter's toolchain setup record kept with a retained trace.
fn attach_setup(attempt: &mut Attempt, path: &Path) {
    let Ok(value) = read_json(path) else {
        return;
    };
    attempt.setup_mode = string(&value, "/mode");
    attempt.setup_cache = string(&value, "/cache");
    attempt.setup_install_ms = value.pointer("/install_ms").and_then(Value::as_u64);
}

fn attach_usage(attempt: &mut Attempt, path: &Path, records: &mut Records) {
    if !path.is_file() {
        return;
    }
    match read_json(path) {
        Ok(value) => {
            attempt.evidence.push(Evidence {
                kind: "component usage".to_owned(),
                path: Some(path.to_path_buf()),
                state: EvidenceState::Unchecked,
                note: None,
            });
            if let Some(components) = value.get("components").and_then(Value::as_object) {
                for (name, component) in components {
                    attempt.costs.push((
                        name.clone(),
                        component.get("cost_usd").and_then(Value::as_f64),
                        string(component, "/cost_provenance")
                            .unwrap_or_else(|| "unknown".to_owned()),
                    ));
                }
            }
            if let Some(amount) = value.pointer("/cost/amount_usd").and_then(Value::as_f64) {
                attempt.cost_usd = Some(amount);
            }
            if let Some(provenance) = string(&value, "/cost/provenance") {
                attempt.cost_provenance = provenance;
            }
            if let Some(calls) = value.get("calls").and_then(Value::as_object) {
                for (name, count) in calls {
                    attempt.counts.push((name.clone(), count.as_u64()));
                }
            }
            if let Some(ledger) = value.get("ledger").and_then(Value::as_array) {
                attempt.ledger.clone_from(ledger);
            }
        }
        Err(error) => records.errors.push(error),
    }
}

fn attach_verifier(attempt: &mut Attempt, path: &Path) {
    let Ok(value) = read_json(path) else {
        return;
    };
    let Some(summary) = value.pointer("/results/summary") else {
        return;
    };
    let count = |field: &str| {
        summary
            .get(field)
            .and_then(Value::as_u64)
            .map_or("—".to_owned(), |n| n.to_string())
    };
    attempt.notes.push(format!(
        "Independent verifier: {} tests, {} passed, {} failed, {} skipped",
        count("tests"),
        count("passed"),
        count("failed"),
        count("skipped")
    ));
    if let Some(tests) = value.pointer("/results/tests").and_then(Value::as_array) {
        let failed: Vec<_> = tests
            .iter()
            .filter(|test| string(test, "/status").as_deref() == Some("failed"))
            .filter_map(|test| string(test, "/name"))
            .take(8)
            .collect();
        if !failed.is_empty() {
            attempt
                .notes
                .push(format!("Failed verifier tests: {}", failed.join(", ")));
        }
    }
}

fn attach_trajectory(attempt: &mut Attempt, path: &Path) {
    let Ok(value) = read_json(path) else {
        attempt
            .notes
            .push("Retained trajectory is unreadable; identity and counts are unknown.".to_owned());
        return;
    };
    if string(&value, "/schema_version").as_deref() != Some("ATIF-v1.7") {
        attempt
            .notes
            .push("Retained trajectory has an unsupported ATIF version.".to_owned());
        return;
    }
    attempt.agent = string(&value, "/agent/name");
    attempt.model = string(&value, "/agent/model_name");
    if let Some(version) = string(&value, "/agent/version") {
        attempt.notes.push(format!("Agent version: {version}"));
    }
    let Some(steps) = value.get("steps").and_then(Value::as_array) else {
        return;
    };
    let mut models = 0;
    let mut decisions = 0;
    let mut tools = 0;
    let mut shells = 0;
    let mut retries = 0;
    let mut subagents = 0;
    for step in steps {
        if string(step, "/source").as_deref() != Some("agent") {
            continue;
        }
        if step
            .get("subagent_trajectory_ref")
            .is_some_and(|v| !v.is_null())
        {
            subagents += 1;
        }
        if step.get("model_name").is_some_and(|v| !v.is_null())
            || step.get("message").is_some_and(|v| !v.is_null())
        {
            models += 1;
        }
        if step
            .pointer("/extra/decision_call")
            .and_then(Value::as_bool)
            == Some(true)
            || step
                .pointer("/extra/typed_decision")
                .and_then(Value::as_bool)
                == Some(true)
        {
            decisions += 1;
        }
        for call in step
            .get("tool_calls")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            tools += 1;
            let name = string(call, "/tool_name")
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "bash" | "shell" | "execute_command" | "run_command"
            ) {
                shells += 1;
            }
            if call.pointer("/is_retry").and_then(Value::as_bool) == Some(true)
                || call.pointer("/extra/retry").and_then(Value::as_bool) == Some(true)
            {
                retries += 1;
            }
        }
    }
    attempt.counts.extend([
        ("atif_steps".to_owned(), Some(steps.len() as u64)),
        ("model_invocations".to_owned(), Some(models)),
        ("typed_decisions".to_owned(), Some(decisions)),
        ("tool_calls".to_owned(), Some(tools)),
        ("shell_commands".to_owned(), Some(shells)),
        ("retries".to_owned(), Some(retries)),
        ("subagent_steps".to_owned(), Some(subagents)),
    ]);
    attempt
        .notes
        .push("Counts: atif-v1-steps-and-calls".to_owned());
}

fn apply_manual_price(attempt: &mut Attempt) {
    if attempt.cost_usd.is_some() {
        return;
    }
    let model = attempt
        .arm
        .strip_prefix("codex-")
        .or(attempt.model.as_deref());
    let Some(model) = model else {
        return;
    };
    let Some((input, cache, output)) = attempt
        .input_tokens
        .zip(attempt.cache_tokens)
        .zip(attempt.output_tokens)
        .map(|((a, b), c)| (a, b, c))
    else {
        return;
    };
    if cache > input {
        attempt
            .notes
            .push("Cache tokens exceed total input; manual price unavailable.".to_owned());
        return;
    }
    let catalog: Value = match serde_json::from_str(include_str!(
        "../../../bench/terminal-bench/profiles/manual-prices.json"
    )) {
        Ok(value) => value,
        Err(_) => return,
    };
    let Some(rates) = catalog.pointer(&format!("/models/{model}")) else {
        return;
    };
    let Some((uncached_rate, cache_rate, output_rate)) = rates
        .get("input")
        .and_then(Value::as_f64)
        .zip(rates.get("cached_input").and_then(Value::as_f64))
        .zip(rates.get("output").and_then(Value::as_f64))
        .map(|((a, b), c)| (a, b, c))
    else {
        return;
    };
    attempt.cost_usd = Some(
        ((input - cache) as f64 * uncached_rate
            + cache as f64 * cache_rate
            + output as f64 * output_rate)
            / 1_000_000.0,
    );
    attempt.cost_provenance = "manual list price; subscription reference".to_owned();
    attempt.notes.push("Price source: docs/terminal-bench/runbook.md, operator-supplied standard short-context rates on 2026-09-22. This is not a bill.".to_owned());
}

fn children(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default()
}

fn read_json(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

fn required(value: &Value, pointer: &str, path: &Path) -> Result<String, String> {
    string(value, pointer)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("{}: missing {pointer}", path.display()))
}

fn string(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

fn coverage(input: Option<u64>, output: Option<u64>) -> &'static str {
    match (input, output) {
        (Some(_), Some(_)) => "full",
        (None, None) => "unknown",
        _ => "partial",
    }
}

fn retained_status(value: &Value, reward: Option<f64>) -> &'static str {
    let Some(exception) = value.get("exception_info").filter(|e| !e.is_null()) else {
        return if reward.is_some() {
            "completed"
        } else {
            "unverifiable"
        };
    };
    let kind = string(exception, "/exception_type").unwrap_or_default();
    if USAGE_LIMIT_EXCEPTIONS.contains(&kind.as_str()) {
        return USAGE_LIMITED;
    }
    let detail = format!(
        "{kind} {}",
        string(exception, "/exception_message").unwrap_or_default()
    )
    .to_ascii_lowercase();
    if detail.contains("timeout") {
        "timeout"
    } else if detail.contains("cancel") {
        "cancelled"
    } else if detail.contains("refusal")
        || detail.contains("quota")
        || detail.contains("usage limit")
    {
        "provider_refusal"
    } else if detail.contains("install") || detail.contains("setup") {
        "install_failure"
    } else if detail.contains("verifier") {
        "verifier_failure"
    } else {
        "agent_error"
    }
}

fn elapsed(value: &Value, pointer: &str) -> Option<u64> {
    let start = if pointer.is_empty() {
        value.get("started_at")?
    } else {
        value.pointer(&format!("{pointer}/started_at"))?
    }
    .as_str()?;
    let end = if pointer.is_empty() {
        value.get("finished_at")?
    } else {
        value.pointer(&format!("{pointer}/finished_at"))?
    }
    .as_str()?;
    let start = timestamp_ms(start)?;
    let end = timestamp_ms(end)?;
    end.checked_sub(start)
        .and_then(|value| u64::try_from(value).ok())
}

pub(crate) fn timestamp_ms(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
    {
        return None;
    }
    let number = |start: usize, end: usize| text.get(start..end)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }
    let mut cursor = 19;
    let mut fraction_ms = 0;
    if bytes.get(cursor) == Some(&b'.') {
        cursor += 1;
        let start = cursor;
        while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        let fraction = text.get(start..cursor)?;
        if fraction.is_empty() {
            return None;
        }
        fraction_ms = format!("{fraction:0<3}").get(..3)?.parse::<i64>().ok()?;
    }
    let offset = match bytes.get(cursor) {
        Some(b'Z') if cursor + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-'))
            if cursor + 6 == bytes.len() && bytes.get(cursor + 3) == Some(&b':') =>
        {
            let hours = number(cursor + 1, cursor + 3)?;
            let minutes = number(cursor + 4, cursor + 6)?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            (hours * 60 + minutes) * 60 * if *sign == b'+' { 1 } else { -1 }
        }
        _ => return None,
    };
    // Civil date to days since 1970-01-01. Harbor timestamps are UTC or
    // include an explicit offset, and only their elapsed difference is used.
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 86_400 + hour * 3_600 + minute * 60 + second - offset) * 1_000) + fraction_ms)
}

fn sha256(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes = file.read(&mut buffer).ok()?;
        if bytes == 0 {
            break;
        }
        digest.update(&buffer[..bytes]);
    }
    Some(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench")
    }

    #[test]
    fn checked_samples_keep_reward_status_and_evidence_distinct() {
        let records = Records::load(None, None, Some(&bench_root().join("samples")));
        assert!(records.errors.is_empty(), "{:?}", records.errors);
        assert!(records.attempts.len() >= 11);
        let probe = records
            .attempts
            .iter()
            .find(|a| a.job == "smoke--coder-v05--fix-git")
            .unwrap();
        assert_eq!(probe.reward, Some(0.0));
        assert_eq!(probe.status, "agent_error");
        assert_eq!(probe.phases_ms[4], Some(13352));
        assert_eq!(probe.cost_usd, None);
        assert!(
            probe
                .evidence
                .iter()
                .any(|e| e.kind == "trajectory" && e.state == EvidenceState::Verified)
        );
        let oracle = records.attempts.iter().find(|a| a.arm == "oracle").unwrap();
        assert_eq!(oracle.reward, Some(1.0));
        assert!(oracle.is_control());
        let timeout = records
            .attempts
            .iter()
            .find(|a| a.job == "resilience--timeout--fix-git")
            .unwrap();
        assert_eq!(timeout.status, "timeout");
        assert_eq!(timeout.reward, None);
        assert_eq!(timeout.image_state.as_deref(), Some("warm"));
    }

    #[test]
    fn retained_traces_include_component_cost_and_manual_price() {
        let records = Records::load(None, Some(&bench_root().join("traces")), None);
        assert!(records.errors.is_empty(), "{:?}", records.errors);
        let delegate = records
            .attempts
            .iter()
            .find(|a| a.job == "smoke--coder-one-delegate-opus--fix-git")
            .unwrap();
        assert_eq!(delegate.reward, Some(1.0));
        assert_eq!(delegate.cost_provenance, "mixed");
        assert!(
            delegate
                .costs
                .iter()
                .any(|(name, amount, _)| name == "jev" && amount.is_some())
        );
        assert!(
            delegate
                .notes
                .iter()
                .any(|note| note.contains("Briefing omitted"))
        );
        let luna = records
            .attempts
            .iter()
            .find(|a| a.job == "smoke--codex-gpt-6-luna--fix-git")
            .unwrap();
        assert!(
            luna.cost_usd
                .is_some_and(|amount| (amount - 0.0052).abs() < 0.0001)
        );
        assert!(luna.cost_provenance.contains("manual list price"));
    }

    #[test]
    fn retained_v3_failures_pass_the_completeness_check() {
        let records = Records::load(None, Some(&bench_root().join("traces")), None);
        for (job, trial) in [
            (
                "extended--coder-one-jevprobe3-luna--log-summary-date-ranges",
                "log-summary-date-ranges__XWSKgz5",
            ),
            (
                "panel--coder-one-jevprobe3-luna--headless-terminal",
                "headless-terminal__tr384w7",
            ),
            (
                "extended--coder-one-jevprobe3-luna--cancel-async-tasks-2",
                "cancel-async-tasks__8MSemsU",
            ),
        ] {
            let attempt = records
                .attempts
                .iter()
                .find(|a| a.job == job && a.trial == trial)
                .unwrap();
            assert!(attempt.missing_evidence().is_empty(), "{job}");
            assert_eq!(attempt.evidence_health(), "verified files", "{job}");
            assert!(attempt.evidence.iter().any(|e| {
                e.state == EvidenceState::Verified
                    && e.path
                        .as_ref()
                        .is_some_and(|p| p.ends_with("artifacts/delegate-1.stream.jsonl"))
            }));
            assert!(attempt.evidence.iter().any(|e| {
                e.kind == "verifier output"
                    && e.path
                        .as_ref()
                        .is_some_and(|p| p.ends_with("verifier/test-stdout.txt"))
            }));
            assert!(attempt.costs.iter().any(|(name, _, _)| name == "jev"));
        }
    }

    #[test]
    fn retention_reports_missing_and_edited_files() {
        let temp = tempfile::tempdir().unwrap();
        let job = temp.path().join("extended--coder-one-x--task");
        let episode = job.join("task__abc.episode");
        fs::create_dir_all(episode.join("verifier")).unwrap();
        fs::write(job.join("task__abc.json"), b"{}").unwrap();
        fs::write(
            episode.join("harbor-result.json"),
            br#"{"task_name":"terminal-bench/task","verifier_result":{"rewards":{"reward":0.0}}}"#,
        )
        .unwrap();
        fs::write(episode.join("verifier/test-stdout.txt"), b"edited").unwrap();
        fs::write(
            episode.join("retention.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema": RETENTION_SCHEMA,
                "files": [
                    {"kind": "verifier output", "path": "verifier/test-stdout.txt", "sha256": "0".repeat(64)},
                    {"kind": "normalized trajectory", "path": "task__abc.json", "sha256": null},
                ],
                "missing": [
                    {"kind": "native delegate stream", "reference": "artifacts/delegate-1.stream.jsonl", "reason": "referenced but not present"}
                ],
                "notes": ["Harbor collected nothing from /logs/artifacts."],
                "atif": {"conversion": "No trajectory exists for this trial."}
            }))
            .unwrap(),
        )
        .unwrap();
        let records = Records::load(None, Some(temp.path()), None);
        assert!(records.errors.is_empty(), "{:?}", records.errors);
        let attempt = &records.attempts[0];
        let missing = attempt.missing_evidence();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].kind, "native delegate stream");
        assert_eq!(
            missing[0].note.as_deref(),
            Some("referenced but not present")
        );
        assert!(
            attempt
                .evidence
                .iter()
                .any(|e| e.kind == "verifier output" && e.state == EvidenceState::Edited)
        );
        assert_eq!(attempt.evidence_health(), "digest mismatch");
        assert!(
            attempt
                .notes
                .iter()
                .any(|n| n.starts_with("Retention: Harbor collected"))
        );
    }

    #[test]
    fn edited_and_missing_bytes_do_not_disappear() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("evidence.json");
        fs::write(&path, b"first").unwrap();
        let digest = sha256(&path).unwrap();
        fs::write(&path, b"second").unwrap();
        let mut attempt = test_attempt();
        collect_evidence(
            &serde_json::json!({"kind":"artifact", "resolved":true, "path":path, "sha256":digest}),
            &mut attempt,
            None,
        );
        collect_evidence(
            &serde_json::json!({"kind":"artifact", "resolved":true, "path":temp.path().join("missing"), "sha256":digest}),
            &mut attempt,
            None,
        );
        assert_eq!(attempt.evidence[0].state, EvidenceState::Edited);
        assert_eq!(attempt.evidence[1].state, EvidenceState::Missing);
    }

    #[test]
    fn absent_collection_failure_marker_does_not_make_a_success_unhealthy() {
        let mut attempt = test_attempt();
        collect_evidence(
            &serde_json::json!({"kind":"collection-failure", "resolved":false, "path":null, "sha256":null}),
            &mut attempt,
            None,
        );
        assert_eq!(attempt.evidence[0].state, EvidenceState::NotPresent);
        assert_eq!(attempt.evidence_health(), "not checked");
    }

    #[test]
    fn elapsed_reads_rfc3339_fraction_and_offset() {
        assert_eq!(
            timestamp_ms("2026-09-22T00:00:00.100Z"),
            timestamp_ms("2026-09-21T19:00:00.100-05:00")
        );
        assert_eq!(
            elapsed(
                &serde_json::json!({"started_at":"2026-09-22T00:00:00.100Z", "finished_at":"2026-09-22T00:00:01.250Z"}),
                ""
            ),
            Some(1150)
        );
    }

    #[test]
    fn a_usage_limited_attempt_is_counted_apart_and_never_graded() {
        let temp = tempfile::tempdir().unwrap();
        let attempts = |job: &str| {
            let dir = temp.path().join(job).join("tbench/attempts");
            fs::create_dir_all(&dir).unwrap();
            dir
        };
        let record = |job: &str, trial: &str, outcome: Value| {
            serde_json::to_vec(&serde_json::json!({
                "schema": ATTEMPT_SCHEMA,
                "attempt": {"job": job, "trial": trial, "arm": "claude-code-opus", "kind": "fresh"},
                "task": {"name": "cad-model"},
                "outcome": outcome,
            }))
            .unwrap()
        };
        // A record the harness wrote before it knew about usage limits:
        // Harbor's rate-limit exception, and a verifier reward of 0.
        let old = "tb4--claude-code-opus--cad-model";
        fs::write(
            attempts(old).join("cad-model__a.json"),
            record(
                old,
                "cad-model__a",
                serde_json::json!({
                    "reward": 0.0,
                    "terminal_status": "agent_error",
                    "exception": {"exception_type": "ApiRateLimitError"},
                }),
            ),
        )
        .unwrap();
        // A record the harness writes now.
        let new = "tb4--claude-code-opus--cad-model-2";
        fs::write(
            attempts(new).join("cad-model__b.json"),
            record(
                new,
                "cad-model__b",
                serde_json::json!({
                    "reward": null,
                    "verifier_rewards": {"reward": 0.0},
                    "terminal_status": "usage_limited",
                    "usage_limit": {
                        "provider": "anthropic",
                        "message": "You've hit your session limit · resets 11:50am (UTC)",
                        "resets_at_iso": "2026-09-23T11:50:00+00:00",
                    },
                }),
            ),
        )
        .unwrap();
        // A graded attempt beside them.
        let graded = "tb4--claude-code-opus--cad-model-3";
        fs::write(
            attempts(graded).join("cad-model__c.json"),
            record(
                graded,
                "cad-model__c",
                serde_json::json!({"reward": 1.0, "terminal_status": "completed"}),
            ),
        )
        .unwrap();
        let records = Records::load(Some(temp.path()), None, None);
        assert_eq!(records.attempts.len(), 3, "{:?}", records.errors);
        assert_eq!(records.usage_limited(), 2);
        assert_eq!(records.status_counts().get(USAGE_LIMITED), Some(&2));
        for attempt in records.attempts.iter().filter(|a| a.is_usage_limited()) {
            assert_eq!(attempt.reward, None, "{}", attempt.job);
        }
        let old = records.attempts.iter().find(|a| a.job == old).unwrap();
        assert!(old.notes.iter().any(|n| n.contains("reward 0 withheld")));
        let new = records.attempts.iter().find(|a| a.job == new).unwrap();
        assert!(
            new.notes
                .iter()
                .any(|n| n.contains("Resets 2026-09-23T11:50:00"))
        );
        // A Coder One episode manifest that says so is enough on its own.
        let mut episode = test_attempt();
        episode.reward = Some(0.0);
        episode.status = "completed".to_owned();
        let manifest = temp.path().join("manifest.json");
        fs::write(
            &manifest,
            serde_json::to_vec(&serde_json::json!({
                "contract": "openagents.coder.episode.v1",
                "outcome": "usage_limited",
                "usage_limit": {"provider": "anthropic", "message": "limited"},
            }))
            .unwrap(),
        )
        .unwrap();
        let mut scratch = Records::default();
        attach_episode(&mut episode, &manifest, temp.path(), &mut scratch);
        assert!(episode.is_usage_limited());
        assert_eq!(episode.reward, None);
        // Harbor's own classification of a retained trace.
        let harbor =
            serde_json::json!({"exception_info": {"exception_type": "ApiUsageLimitError"}});
        assert_eq!(retained_status(&harbor, Some(0.0)), USAGE_LIMITED);
    }

    #[test]
    fn unknown_reward_is_not_a_task_failure() {
        let mut attempt = test_attempt();
        attempt.status = "completed".to_owned();
        assert_eq!(attempt.display_status(), "unverifiable");
        attempt.reward = Some(0.0);
        assert_eq!(attempt.display_status(), "task failure");
    }

    #[test]
    fn a_coder_one_episode_brings_its_policy_ledger_and_deadline() {
        let temp = tempfile::tempdir().unwrap();
        let episode = temp.path();
        std::fs::create_dir_all(episode.join("evaluation")).unwrap();
        let manifest = serde_json::json!({
            "contract": "openagents.coder.episode.v1",
            "artifact": { "version": "coder-one 0.1.0 (test)" },
            "policy": {
                "digest": "ab".repeat(32),
                "name": "coder-one-jevprobe3-luna",
                "source": "inline",
                "overrides": [{"source": "CODER_ONE_DELEGATE_EFFORT", "field": "policy.executor.effort", "value": "high"}],
                "manifest": { "schema": "openagents.coder-one.policy.v1", "policy": {}, "protected": {} },
            },
            "deadline": { "kind": "hard", "total_ms": 1_740_000, "reserve_ms": 30_000, "elapsed_ms": 600_000, "cuts": [] },
            "files": {},
        });
        let usage = serde_json::json!({
            "cost": { "amount_usd": null, "provenance": "unknown" },
            "components": {},
            "calls": {},
            "ledger": [
                {"component": "jev", "id": "jev-1", "charge": "unknown", "cost_usd": null, "provenance": "unknown"},
                {"component": "delegate", "id": "delegate-1", "charge": "priced", "cost_usd": 0.1, "provenance": "price_estimate"},
            ],
        });
        let manifest_path = episode.join("manifest.json");
        let usage_path = episode.join("evaluation/usage.json");
        std::fs::write(&manifest_path, manifest.to_string()).unwrap();
        std::fs::write(&usage_path, usage.to_string()).unwrap();
        let mut records = Records::default();
        let mut attempt = test_attempt();
        attach_episode(&mut attempt, &manifest_path, episode, &mut records);
        attach_usage(&mut attempt, &usage_path, &mut records);
        assert!(records.errors.is_empty(), "{:?}", records.errors);
        let policy = attempt.policy.as_ref().unwrap();
        assert_eq!(policy.short(), "abababababab");
        assert_eq!(policy.overrides.len(), 1);
        assert_eq!(attempt.ledger.len(), 2);
        assert_eq!(attempt.deadline.as_ref().unwrap()["kind"], "hard");
        assert_eq!(crate::coder_calls::charges(&attempt.ledger), (1, 0, 1));
    }

    #[test]
    fn saved_report_is_labeled_and_staleness_is_visible() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("tbench-report.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema": "openagents.tbench.report.v1",
                "label": "small development sample",
                "attempts_total": 1,
                "pin_warnings": ["fix-git: mixed commits"]
            }))
            .unwrap(),
        )
        .unwrap();
        let records = Records::load(Some(temp.path()), None, None);
        assert_eq!(
            records.report_label.as_deref(),
            Some("small development sample")
        );
        assert!(
            records
                .report_warnings
                .iter()
                .any(|warning| warning.contains("mixed commits"))
        );
        assert!(
            records
                .report_warnings
                .iter()
                .any(|warning| warning.contains("stale"))
        );
    }
}
