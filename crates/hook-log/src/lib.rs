//! One decision-log format for every System One hook (#11492).
//!
//! Each hook appends one JSON line per decision to
//! `~/.openagents/decisions/<hook>.jsonl` (or the path named by
//! `CODER_<HOOK>_LOG`). [`report`] joins those lines with run outcomes and
//! prints a calibration table per hook and question.

pub mod report;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The run id the host sets so a decision can be joined to its outcome.
pub const RUN_ID_VAR: &str = "CODER_RUN_ID";

/// How a decision was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// A rule decided; nothing was asked.
    Rule,
    /// The Decisions API answered.
    Decision,
    /// The call failed and a rule decided instead.
    Fallback,
}

/// One logged decision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    /// The hook, e.g. `squeeze`.
    pub hook: String,
    /// The question asked, e.g. `keep_chunk`.
    #[serde(default)]
    pub question: String,
    /// The raw probability, when a model gave one.
    #[serde(default)]
    pub p: Option<f64>,
    /// The threshold the probability was compared with.
    #[serde(default)]
    pub threshold: Option<f64>,
    /// What the hook did.
    #[serde(default)]
    pub action: String,
    /// How the action was chosen.
    pub how: How,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub door: Option<String>,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// The run this decision belongs to (`CODER_RUN_ID`).
    #[serde(default)]
    pub run_id: Option<String>,
    /// Milliseconds since the Unix epoch; also the join key for labels.
    #[serde(default)]
    pub ts: u64,
    /// Anything hook-specific.
    #[serde(default)]
    pub detail: Value,
}

impl Decision {
    /// A decision for `hook` and `question`, stamped now, with the run id
    /// from the environment.
    #[must_use]
    pub fn new(hook: &str, question: &str, action: &str, how: How) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        Self {
            hook: hook.to_string(),
            question: question.to_string(),
            p: None,
            threshold: None,
            action: action.to_string(),
            how,
            model: None,
            door: None,
            latency_ms: None,
            run_id: std::env::var(RUN_ID_VAR)
                .ok()
                .filter(|v| !v.trim().is_empty()),
            ts,
            detail: Value::Null,
        }
    }
}

/// The environment variable that overrides `hook`'s log path:
/// `CODER_<HOOK>_LOG`.
#[must_use]
pub fn log_var(hook: &str) -> String {
    let name: String = hook
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("CODER_{name}_LOG")
}

/// Where `hook` logs: the override variable, else
/// `~/.openagents/decisions/<hook>.jsonl`.
#[must_use]
pub fn default_path(hook: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var(log_var(hook))
        && !path.trim().is_empty()
    {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(format!(".openagents/decisions/{hook}.jsonl")))
}

/// Appends one line to `path`, creating directories. Never panics and never
/// fails the caller.
pub fn append(path: &Path, decision: &Decision) {
    let Ok(line) = serde_json::to_string(decision) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}

/// A hook's own JSON line as a [`Decision`]: the shared fields are read
/// from it (`hook`, `question`, `p`, `threshold`, `action`, `how`, `model`,
/// `door`, `latency_ms`), and the whole line is kept in `detail`. `how`
/// comes from the line when it names one, else from what it holds: an
/// `error` is a fallback, a `p` a decision, anything else a rule.
#[must_use]
pub fn from_line(line: &Value) -> Decision {
    let text = |key: &str| line[key].as_str().unwrap_or_default().to_owned();
    let how = match line["how"].as_str() {
        Some("decision") => How::Decision,
        Some("rule") => How::Rule,
        Some(word) if word.contains("unavailable") || word == "fallback" => How::Fallback,
        _ if line.get("error").is_some_and(|e| !e.is_null()) => How::Fallback,
        _ if line["p"].is_number() => How::Decision,
        _ => How::Rule,
    };
    let mut decision = Decision::new(&text("hook"), &text("question"), &text("action"), how);
    decision.p = line["p"].as_f64();
    decision.threshold = line["threshold"].as_f64();
    decision.model = line["model"].as_str().map(str::to_owned);
    decision.door = line["door"].as_str().map(str::to_owned);
    decision.latency_ms = line["latency_ms"].as_u64();
    decision.detail = line.clone();
    decision
}

/// Appends to the hook's default path.
pub fn append_default(decision: &Decision) {
    if let Some(path) = default_path(&decision.hook) {
        append(&path, decision);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_line_keeps_its_shared_fields_and_the_rest_in_detail() {
        let line = serde_json::json!({"hook": "gate", "question": "unread_script", "p": 0.97,
            "threshold": 0.95, "action": "deny", "model": "clef-flash", "door": "clef",
            "latency_ms": 180, "command": "bash x.sh"});
        let d = from_line(&line);
        assert_eq!(
            (d.hook.as_str(), d.how, d.p),
            ("gate", How::Decision, Some(0.97))
        );
        assert_eq!(d.detail["command"], "bash x.sh");
        let failed = from_line(
            &serde_json::json!({"hook": "context", "how": "rule (decision unavailable)"}),
        );
        assert_eq!(failed.how, How::Fallback);
    }

    #[test]
    fn append_creates_dirs_and_one_line_per_call() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/x.jsonl");
        let mut d = Decision::new("x", "q", "keep", How::Decision);
        d.p = Some(0.7);
        append(&path, &d);
        append(&path, &d);
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        let back: Decision = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn append_to_unwritable_path_is_silent() {
        let d = Decision::new("x", "q", "a", How::Rule);
        append(Path::new("/dev/null/nope/x.jsonl"), &d);
    }

    #[test]
    fn log_var_follows_the_squeeze_convention() {
        assert_eq!(log_var("squeeze"), "CODER_SQUEEZE_LOG");
        assert_eq!(log_var("token-saver"), "CODER_TOKEN_SAVER_LOG");
    }
}
