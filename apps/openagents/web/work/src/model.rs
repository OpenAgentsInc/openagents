//! What the work panel reads, from the site's own JSON: work runs
//! (`/v1/work`), the landing queue (`/v1/land`), Mac jobs (`/v1/mac-jobs`),
//! and environments (`/environments/*.json`), with the words each row and
//! line shows. Fields the site may leave out read as their defaults.

use serde::Deserialize;
use serde_json::Value;

/// A work run's progress line.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Progress {
    pub phase: String,
    pub text: String,
}

/// A work run (`/v1/work`'s `view`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct WorkRun {
    pub id: String,
    pub repo: String,
    pub issue: Option<u64>,
    pub title: Option<String>,
    pub state: String,
    pub why: Option<String>,
    pub lines: Vec<Progress>,
    /// The `after` that reads only lines after these.
    pub next: usize,
    pub result: Option<Value>,
    pub updated_unix: u64,
}

impl WorkRun {
    pub fn finished(&self) -> bool {
        matches!(self.state.as_str(), "done" | "failed" | "cancelled")
    }

    pub fn name(&self) -> String {
        let title = self.title.clone().filter(|t| !t.is_empty());
        match (self.issue, title) {
            (Some(n), Some(t)) => format!("#{n} {t}"),
            (Some(n), None) => format!("{} #{n}", self.repo),
            (None, Some(t)) => t,
            (None, None) => self.repo.clone(),
        }
    }

    /// The pull request the run opened, when it did.
    pub fn pull_request(&self) -> Option<String> {
        let pr = self.result.as_ref()?.get("pr")?;
        pr.as_str()
            .map(str::to_owned)
            .or_else(|| pr.get("url").and_then(Value::as_str).map(str::to_owned))
    }

    /// Adds a long poll's answer: newer lines, and the run's state.
    pub fn merge(&mut self, newer: WorkRun) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.extend(newer.lines.iter().cloned());
        *self = WorkRun { lines, ..newer };
    }
}

/// Plain words for a run's or job's state.
pub fn state_words(state: &str) -> &'static str {
    match state {
        "waiting" => "Waiting for a computer",
        "running" => "Working",
        "asking" => "Waiting for your answer",
        "done" => "Done",
        "failed" => "Failed",
        "cancelled" => "Stopped",
        "uncertain" => "Not sure it finished",
        _ => "",
    }
}

/// The landing queue at a glance.
pub fn land_line(status: &Value) -> String {
    let count = |key: &str| status[key].as_array().map_or(0, Vec::len);
    let running = status["integrator"]["running"].as_bool().unwrap_or(false);
    format!(
        "Landing queue: {} waiting, {} landing, {} bounced. The integrator is {}.",
        count("waiting"),
        count("landing"),
        count("failed"),
        if running { "running" } else { "stopped" }
    )
}

/// `owner/repo#123`, `owner/repo 123`, or an issue's address, as `/v1/work`
/// takes it.
pub fn issue_of(text: &str) -> Option<(String, u64)> {
    let text = text.trim();
    if let Some(rest) = text
        .strip_prefix("https://github.com/")
        .or_else(|| text.strip_prefix("github.com/"))
    {
        let mut parts = rest.split('/');
        let (owner, repo, kind, number) =
            (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
        return (kind == "issues")
            .then(|| number.parse().ok().map(|n| (format!("{owner}/{repo}"), n)))
            .flatten();
    }
    let (repo, number) = text.split_once('#').or_else(|| text.split_once(' '))?;
    let repo = repo.trim();
    let valid = repo.split('/').count() == 2 && !repo.starts_with('/') && !repo.ends_with('/');
    let number = number.trim().parse().ok()?;
    valid.then(|| (repo.to_owned(), number))
}

/// A Mac job's question.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Question {
    pub id: String,
    pub text: String,
}

/// A Mac job (`/v1/mac-jobs`'s `view`).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct MacJob {
    pub id: String,
    pub computer: String,
    pub title: String,
    pub state: String,
    pub question: Option<Question>,
    pub summary: Option<String>,
    pub why: Option<String>,
    pub lines: Vec<String>,
    pub next: usize,
    pub last_line: Option<String>,
}

impl MacJob {
    pub fn finished(&self) -> bool {
        matches!(self.state.as_str(), "done" | "failed" | "cancelled")
    }

    pub fn merge(&mut self, newer: MacJob) {
        let mut lines = std::mem::take(&mut self.lines);
        lines.extend(newer.lines.iter().cloned());
        *self = MacJob { lines, ..newer };
    }
}

/// One environment in the list.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Environment {
    pub id: String,
    pub repository: String,
    pub branch: String,
    pub status: String,
}

/// A transcript line: `user`, `agent`, `step`, or `note`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Item {
    pub kind: String,
    pub text: String,
}

/// An environment's Claude Code run, in its list.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RunRow {
    pub id: String,
    pub prompt: String,
    pub state: String,
    pub finished: bool,
}

/// `/environments/{id}/state.json`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct EnvironmentState {
    pub environment: Environment,
    pub question: Option<String>,
    pub items: Vec<Item>,
    pub runs: Vec<RunRow>,
}

/// A pause choice a run offers.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Choice {
    pub key: String,
    pub label: String,
}

/// `/environments/{id}/runs/{run}/state.json`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct RunState {
    pub id: String,
    pub environment: String,
    pub state: String,
    pub finished: bool,
    pub paused: Option<String>,
    pub choices: Vec<Choice>,
    pub items: Vec<Item>,
}

/// A form body, encoded for a page's own post.
pub fn form(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_work_run_reads_and_grows_by_long_poll() {
        let mut run: WorkRun = serde_json::from_value(json!({
            "id": "w1", "repo": "OpenAgentsInc/openagents", "issue": 12225, "title": null,
            "state": "running", "lines": [{"secs": 1.0, "phase": "start", "text": "Starting"}],
            "next": 1, "extra": true,
        }))
        .unwrap();
        assert_eq!(run.name(), "OpenAgentsInc/openagents #12225");
        assert!(!run.finished());
        let newer: WorkRun = serde_json::from_value(json!({
            "id": "w1", "repo": "OpenAgentsInc/openagents", "issue": 12225, "state": "done",
            "lines": [{"phase": "pr", "text": "Opened a pull request"}], "next": 2,
            "result": {"pr": "https://github.com/OpenAgentsInc/openagents/pull/1"},
        }))
        .unwrap();
        run.merge(newer);
        assert_eq!(run.lines.len(), 2);
        assert!(run.finished());
        assert_eq!(run.next, 2);
        assert!(run.pull_request().unwrap().ends_with("/pull/1"));
        assert_eq!(state_words("cancelled"), "Stopped");
    }

    #[test]
    fn issues_read_in_the_ways_people_write_them() {
        let want = Some(("OpenAgentsInc/openagents".to_owned(), 12225));
        assert_eq!(issue_of("OpenAgentsInc/openagents#12225"), want);
        assert_eq!(issue_of(" OpenAgentsInc/openagents 12225 "), want);
        assert_eq!(
            issue_of("https://github.com/OpenAgentsInc/openagents/issues/12225"),
            want
        );
        assert_eq!(issue_of("openagents#12"), None);
        assert_eq!(issue_of("https://github.com/a/b/pull/3"), None);
    }

    #[test]
    fn the_landing_queue_and_jobs_read_plainly() {
        let line = land_line(&json!({"waiting": [{}, {}], "landing": [{}], "failed": [],
            "integrator": {"running": true}}));
        assert_eq!(
            line,
            "Landing queue: 2 waiting, 1 landing, 0 bounced. The integrator is running."
        );
        let mut job: MacJob = serde_json::from_value(json!({"id": "j1", "state": "asking",
            "question": {"id": "1", "text": "Upload?", "subject": "x"}, "lines": ["a"], "next": 1}))
        .unwrap();
        job.merge(
            serde_json::from_value(json!({"id": "j1", "state": "done", "lines": ["b"], "next": 2}))
                .unwrap(),
        );
        assert_eq!(job.lines, ["a", "b"]);
        assert!(job.finished());
        assert_eq!(
            form(&[("q", "a b&c"), ("choice", "key")]),
            "q=a+b%26c&choice=key"
        );
    }
}
