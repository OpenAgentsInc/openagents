//! Coder One's executor system prompts (`exec.system`), per attempt and
//! per variant.
//!
//! An episode that sent a variant records it in its manifest, under
//! `delegate.delegation.system`: the sections sent, their sizes, and the
//! variant digest. An attempt with no such record ran the executor's own
//! default prompt; the Gym shows that default's sections from the library
//! Coder One checks in (`crates/coder-one/prompts/library.json`) and marks
//! the view as inferred. The capture measurements in
//! `docs/terminal-bench/delegate-prompts/measurements.json` give each
//! variant's first-request size without inference.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::terminal_bench::{Attempt, Records};

/// The schema of `gym coder prompt --json`.
pub const SCHEMA: &str = "openagents.gym.coder-prompt.v1";

/// The checked-in library and capture measurements.
#[must_use]
pub fn default_paths() -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    (
        root.join("crates/coder-one/prompts/library.json"),
        root.join("docs/terminal-bench/delegate-prompts/measurements.json"),
    )
}

/// One section as a view shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub id: String,
    pub status: String,
    pub chars: u64,
    /// `cli`, `manifest`, or `jev`.
    pub by: String,
}

/// One attempt's system prompt.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptView {
    pub agent: String,
    /// `default` or `manifest`.
    pub variant: String,
    /// The variant digest; `None` for a CLI default.
    pub digest: Option<String>,
    pub mode: String,
    pub chars: u64,
    pub protected: Option<bool>,
    pub sections: Vec<Section>,
    /// Jev's answers for the optional sections it was asked about.
    pub asked: Vec<(String, Option<f64>)>,
    /// The view comes from the library, not from the attempt's record.
    pub inferred: bool,
}

impl PromptView {
    /// The key variants group by.
    #[must_use]
    pub fn key(&self) -> String {
        self.digest
            .clone()
            .unwrap_or_else(|| format!("default {}", self.agent))
    }

    /// A short label.
    #[must_use]
    pub fn label(&self) -> String {
        match &self.digest {
            Some(digest) => format!(
                "{} {} {}",
                self.agent,
                self.mode,
                digest.get(..12).unwrap_or(digest)
            ),
            None => format!("{} default", self.agent),
        }
    }

    fn from_record(record: &Value, inferred: bool) -> Option<Self> {
        let text = |key: &str| record.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            agent: text("agent")?,
            variant: text("variant").unwrap_or_else(|| "manifest".to_owned()),
            digest: text("digest"),
            mode: text("mode").unwrap_or_else(|| "default".to_owned()),
            chars: record.get("chars").and_then(Value::as_u64).unwrap_or(0),
            protected: record.get("protected").and_then(Value::as_bool),
            sections: record
                .get("sections")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|section| Section {
                    id: section["id"].as_str().unwrap_or_default().to_owned(),
                    status: section["status"].as_str().unwrap_or_default().to_owned(),
                    chars: section["chars"].as_u64().unwrap_or(0),
                    by: section["by"].as_str().unwrap_or("cli").to_owned(),
                })
                .collect(),
            asked: record
                .get("asked")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|asked| {
                    (
                        asked["id"].as_str().unwrap_or_default().to_owned(),
                        asked["p"].as_f64(),
                    )
                })
                .collect(),
            inferred,
        })
    }

    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "agent": self.agent,
            "variant": self.variant,
            "digest": self.digest,
            "mode": self.mode,
            "chars": self.chars,
            "protected": self.protected,
            "inferred": self.inferred,
            "sections": self.sections.iter().map(|s| json!({
                "id": s.id, "status": s.status, "chars": s.chars, "by": s.by,
            })).collect::<Vec<_>>(),
            "asked": self.asked.iter().map(|(id, p)| json!({ "id": id, "p": p })).collect::<Vec<_>>(),
        })
    }

    /// The view as text lines.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "{} · {} characters · protected section {}{}",
            self.label(),
            self.chars,
            match self.protected {
                Some(true) => "present",
                Some(false) => "absent",
                None => "unknown",
            },
            if self.inferred {
                " · inferred: no variant recorded, so the CLI's default"
            } else {
                ""
            }
        )];
        if let Some(digest) = &self.digest {
            lines.push(format!("variant digest {digest}"));
        }
        for section in &self.sections {
            lines.push(format!(
                "  {:<28} {:<9} {:>6}  {}",
                section.id, section.status, section.chars, section.by
            ));
        }
        for (id, p) in &self.asked {
            lines.push(format!(
                "  Jev on {id}: {}",
                p.map_or("unknown".to_owned(), |p| format!("p={p:.2}"))
            ));
        }
        lines
    }
}

/// The checked-in library: each agent's default record, in the shape an
/// episode records a variant.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub defaults: BTreeMap<String, Value>,
}

impl Library {
    /// Reads `library.json`.
    ///
    /// # Errors
    ///
    /// Returns a message when the file doesn't read.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let mut defaults = BTreeMap::new();
        for default in value
            .get("defaults")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(agent) = default.get("agent").and_then(Value::as_str) else {
                continue;
            };
            defaults.insert(
                agent.to_owned(),
                json!({
                    "agent": agent,
                    "variant": "default",
                    "mode": "default",
                    "digest": Value::Null,
                    "chars": default.get("chars"),
                    "protected": default.get("protected"),
                    "sections": default.get("sections").and_then(Value::as_array).map(|sections| {
                        sections.iter().map(|s| json!({
                            "id": s.get("id"), "status": s.get("status"), "chars": s.get("chars"), "by": "cli",
                        })).collect::<Vec<_>>()
                    }),
                }),
            );
        }
        Ok(Self { defaults })
    }
}

/// The executor an attempt ran, from its manifest, its policy, or its
/// arm name; `None` when it ran none that the library covers.
#[must_use]
pub fn agent_of(attempt: &Attempt) -> Option<String> {
    if let Some(agent) = &attempt.delegate_agent {
        return Some(agent.clone());
    }
    if let Some(agent) = attempt
        .policy
        .as_ref()
        .and_then(|policy| policy.manifest.pointer("/policy/executor/agent"))
        .and_then(Value::as_str)
    {
        return Some(agent.to_owned());
    }
    let arm = attempt.arm.as_str();
    if arm.starts_with("claude-code") {
        Some("claude-code".to_owned())
    } else if arm.starts_with("codex") || (arm.starts_with("coder-one") && arm.contains("luna")) {
        Some("codex".to_owned())
    } else if arm.starts_with("coder-one")
        && ["opus", "sonnet", "haiku"]
            .iter()
            .any(|model| arm.contains(model))
    {
        Some("claude-code".to_owned())
    } else {
        None
    }
}

/// One attempt's prompt view.
#[must_use]
pub fn view(attempt: &Attempt, library: &Library) -> Option<PromptView> {
    if let Some(record) = &attempt.system {
        return PromptView::from_record(record, false);
    }
    let agent = agent_of(attempt)?;
    PromptView::from_record(library.defaults.get(&agent)?, true)
}

/// One variant across attempts, beside its capture.
#[derive(Clone, Debug)]
pub struct VariantRow {
    pub key: String,
    pub label: String,
    pub agent: String,
    pub chars: u64,
    pub inferred: bool,
    pub attempts: usize,
    pub graded: usize,
    pub passes: usize,
    pub mean_cost: Option<f64>,
    pub mean_turns: Option<f64>,
    /// The captured first request's size: characters and the estimate of
    /// tokens (characters / 4).
    pub first_request_chars: Option<u64>,
    pub first_request_tokens: Option<u64>,
}

/// Every variant, and the capture measurements.
#[derive(Clone, Debug, Default)]
pub struct Comparison {
    pub rows: Vec<VariantRow>,
    pub captures: Vec<Value>,
    pub errors: Vec<String>,
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// Groups `records` by the system prompt each attempt ran, and matches
/// each variant to its capture.
#[must_use]
pub fn compare(records: &Records, library: &Library, captures: &[Value]) -> Comparison {
    let mut groups: BTreeMap<String, (PromptView, Vec<&Attempt>)> = BTreeMap::new();
    for attempt in records.attempts.iter().filter(|a| !a.is_control()) {
        if let Some(view) = view(attempt, library) {
            groups
                .entry(view.key())
                .or_insert_with(|| (view, Vec::new()))
                .1
                .push(attempt);
        }
    }
    let capture_for = |view: &PromptView| {
        captures.iter().find(|capture| {
            capture.get("error").is_none()
                && capture["agent"].as_str() == Some(view.agent.as_str())
                && capture["variant"]["digest"].as_str() == view.digest.as_deref()
        })
    };
    let mut rows: Vec<VariantRow> = groups
        .into_values()
        .map(|(view, attempts)| {
            let graded: Vec<&&Attempt> = attempts.iter().filter(|a| a.reward.is_some()).collect();
            let costs: Vec<f64> = attempts.iter().filter_map(|a| a.cost_usd).collect();
            let turns: Vec<f64> = attempts
                .iter()
                .filter_map(|a| a.delegate_turns)
                .map(|n| n as f64)
                .collect();
            let capture = capture_for(&view);
            VariantRow {
                key: view.key(),
                label: view.label(),
                agent: view.agent.clone(),
                chars: view.chars,
                inferred: view.inferred,
                attempts: attempts.len(),
                graded: graded.len(),
                passes: graded
                    .iter()
                    .filter(|a| a.reward.is_some_and(|r| r >= 1.0))
                    .count(),
                mean_cost: mean(&costs),
                mean_turns: mean(&turns),
                first_request_chars: capture.and_then(|c| c["totals"]["text_chars"].as_u64()),
                first_request_tokens: capture
                    .and_then(|c| c["totals"]["estimated_tokens"].as_u64()),
            }
        })
        .collect();
    // Captured variants no attempt has run yet still appear, so a variant
    // is comparable before its first trial.
    for capture in captures.iter().filter(|c| c.get("error").is_none()) {
        let agent = capture["agent"].as_str().unwrap_or_default().to_owned();
        let digest = capture["variant"]["digest"].as_str();
        let key = digest.map_or_else(|| format!("default {agent}"), str::to_owned);
        if rows.iter().any(|row| row.key == key) {
            continue;
        }
        rows.push(VariantRow {
            label: match digest {
                Some(digest) => format!(
                    "{agent} {} {} ({})",
                    capture["variant"]["mode"].as_str().unwrap_or_default(),
                    digest.get(..12).unwrap_or(digest),
                    capture["label"].as_str().unwrap_or_default()
                ),
                None => format!("{agent} default"),
            },
            key,
            agent,
            chars: capture["variant"]["chars"].as_u64().unwrap_or(0),
            inferred: false,
            attempts: 0,
            graded: 0,
            passes: 0,
            mean_cost: None,
            mean_turns: None,
            first_request_chars: capture["totals"]["text_chars"].as_u64(),
            first_request_tokens: capture["totals"]["estimated_tokens"].as_u64(),
        });
    }
    rows.sort_by(|a, b| a.agent.cmp(&b.agent).then(b.chars.cmp(&a.chars)));
    Comparison {
        rows,
        captures: captures.to_vec(),
        errors: Vec::new(),
    }
}

/// Reads the capture measurements file's `captures`.
///
/// # Errors
///
/// Returns a message when the file doesn't read.
pub fn load_captures(path: &Path) -> Result<Vec<Value>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(value
        .get("captures")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// The comparison from the checked-in library and captures.
#[must_use]
pub fn comparison(records: &Records) -> Comparison {
    let (library_path, captures_path) = default_paths();
    let mut errors = Vec::new();
    let library = Library::load(&library_path).unwrap_or_else(|error| {
        errors.push(error);
        Library::default()
    });
    let captures = load_captures(&captures_path).unwrap_or_else(|error| {
        errors.push(error);
        Vec::new()
    });
    let mut comparison = compare(records, &library, &captures);
    comparison.errors = errors;
    comparison
}

fn money(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |usd| format!("${usd:.4}"))
}

impl Comparison {
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "variants": self.rows.iter().map(|row| json!({
                "key": row.key,
                "label": row.label,
                "agent": row.agent,
                "chars": row.chars,
                "inferred": row.inferred,
                "attempts": row.attempts,
                "graded": row.graded,
                "passes": row.passes,
                "mean_cost_usd": row.mean_cost,
                "mean_turns": row.mean_turns,
                "first_request_chars": row.first_request_chars,
                "first_request_estimated_tokens": row.first_request_tokens,
            })).collect::<Vec<_>>(),
            "errors": self.errors,
        })
    }

    /// The comparison as text lines.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            "exec.system variants: system prompt, first request (captured, no inference), and attempts"
                .to_owned(),
            format!(
                "  {:<52} {:>7} {:>9} {:>8} {:>7} {:>9} {:>6}",
                "variant", "system", "1st req", "~tokens", "passed", "mean $", "turns"
            ),
        ];
        for row in &self.rows {
            lines.push(format!(
                "  {:<52} {:>7} {:>9} {:>8} {:>7} {:>9} {:>6}",
                format!("{}{}", row.label, if row.inferred { " *" } else { "" }),
                row.chars,
                row.first_request_chars
                    .map_or("—".to_owned(), |n| n.to_string()),
                row.first_request_tokens
                    .map_or("—".to_owned(), |n| n.to_string()),
                if row.graded == 0 {
                    "—".to_owned()
                } else {
                    format!("{}/{}", row.passes, row.graded)
                },
                money(row.mean_cost),
                row.mean_turns.map_or("—".to_owned(), |t| format!("{t:.1}"))
            ));
        }
        lines.push(
            "  * inferred: the attempts recorded no variant, so they ran the CLI's default."
                .to_owned(),
        );
        for error in &self.errors {
            lines.push(format!("  unavailable: {error}"));
        }
        lines
    }
}

const HELP: &str = "\
gym coder prompt [ATTEMPT] [--json]

Without ATTEMPT: each system prompt variant (exec.system) beside its captured
first request and the attempts that ran it. With ATTEMPT (a trial name, or a
job and trial as JOB/TRIAL, or a prefix of either): that attempt's sections,
their sizes, the variant digest, and Jev's section answers.

  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --no-jobs | --no-traces  omit one source
  --json                   print versioned JSON instead of text";

/// `gym coder prompt …`.
///
/// # Errors
///
/// Returns a message for an unknown option or an attempt that doesn't
/// match.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut jobs = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".openagents/terminal-bench/jobs"));
    let mut traces = Some(repo.join("traces"));
    let mut json_output = false;
    let mut query = None;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--no-jobs" => jobs = None,
            "--no-traces" => traces = None,
            "--jobs-dir" | "--traces-dir" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                if argument == "--jobs-dir" {
                    jobs = Some(value.into());
                } else {
                    traces = Some(value.into());
                }
                index += 1;
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{HELP}"));
            }
            other => query = Some(other.to_owned()),
        }
        index += 1;
    }
    let records = Records::load(jobs.as_deref(), traces.as_deref(), None);
    let comparison = comparison(&records);
    let write = |out: &mut dyn std::io::Write, value: &Value| {
        serde_json::to_writer_pretty(&mut *out, value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())
    };
    let Some(query) = query else {
        if json_output {
            let mut value = comparison.to_json();
            value["schema"] = json!(SCHEMA);
            write(out, &value)?;
        } else {
            for line in comparison.lines() {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
        return Ok(0);
    };
    let (library_path, _) = default_paths();
    let library = Library::load(&library_path).unwrap_or_default();
    let matches: Vec<&Attempt> = records
        .attempts
        .iter()
        .filter(|a| {
            a.trial.starts_with(&query) || format!("{}/{}", a.job, a.trial).starts_with(&query)
        })
        .collect();
    let attempt = match matches.as_slice() {
        [one] => *one,
        [] => return Err(format!("no attempt matches {query}")),
        many => {
            return Err(format!(
                "{query} matches {} attempts; name JOB/TRIAL",
                many.len()
            ));
        }
    };
    let view = view(attempt, &library);
    if json_output {
        write(
            out,
            &json!({
                "schema": SCHEMA,
                "attempt": { "job": attempt.job, "trial": attempt.trial, "arm": attempt.arm, "task": attempt.task },
                "prompt": view.as_ref().map(PromptView::to_json),
            }),
        )?;
    } else {
        writeln!(out, "{} / {} ({})", attempt.job, attempt.trial, attempt.arm)
            .map_err(|error| error.to_string())?;
        match view {
            Some(view) => {
                for line in view.lines() {
                    writeln!(out, "{line}").map_err(|error| error.to_string())?;
                }
            }
            None => writeln!(
                out,
                "No executor system prompt: the attempt ran no delegate the library covers."
            )
            .map_err(|error| error.to_string())?,
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Library {
        Library::load(&default_paths().0).unwrap()
    }

    #[test]
    fn an_attempt_without_a_record_shows_its_executors_default() {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.arm = "coder-one-jevprobe3-luna".to_owned();
        let view = view(&attempt, &library()).unwrap();
        assert!(view.inferred);
        assert_eq!(view.agent, "codex");
        assert_eq!(view.protected, Some(false));
        assert_eq!(view.sections.len(), 9);
        attempt.arm = "devin".to_owned();
        assert!(super::view(&attempt, &library()).is_none());
    }

    #[test]
    fn a_recorded_variant_shows_its_sections_and_digest() {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.arm = "coder-one-x".to_owned();
        attempt.system = Some(json!({
            "agent": "claude-code", "variant": "manifest", "mode": "replace",
            "digest": "abcdef0123456789", "chars": 1800, "protected": true,
            "sections": [
                { "id": "security", "status": "protected", "chars": 461, "by": "manifest" },
                { "id": "long-builds", "status": "optional", "chars": 234, "by": "jev" }
            ],
            "asked": [{ "id": "long-builds", "p": 0.97 }]
        }));
        attempt.reward = Some(1.0);
        attempt.cost_usd = Some(0.02);
        attempt.delegate_turns = Some(4);
        let view = view(&attempt, &library()).unwrap();
        assert!(!view.inferred);
        let text = view.lines().join("\n");
        assert!(text.contains("variant digest abcdef0123456789"), "{text}");
        assert!(
            text.contains("long-builds") && text.contains("jev"),
            "{text}"
        );
        let records = Records {
            attempts: vec![attempt],
            ..Records::default()
        };
        let captures = load_captures(&default_paths().1).unwrap();
        let comparison = compare(&records, &library(), &captures);
        let row = comparison
            .rows
            .iter()
            .find(|row| row.key == "abcdef0123456789")
            .unwrap();
        assert_eq!((row.passes, row.graded), (1, 1));
        assert_eq!(row.mean_turns, Some(4.0));
        // Every captured variant appears, run or not, with its size.
        let codex_default = comparison
            .rows
            .iter()
            .find(|row| row.key == "default codex")
            .unwrap();
        assert!(codex_default.first_request_chars.unwrap() > 40_000);
        assert!(
            comparison
                .rows
                .iter()
                .filter(|row| row.agent == "codex")
                .count()
                >= 3
        );
    }
}
