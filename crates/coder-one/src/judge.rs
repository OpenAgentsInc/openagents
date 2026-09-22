//! The Jev step: code finds candidates, Jev judges them, and the answers
//! become hints in the prompt.
//!
//! One `POST /v1/systemone` request per step. Every question reads the
//! same state and none depends on another, so they share the request:
//!
//! - `file_<i>` (Noul): would reading or editing candidate file `i` help?
//! - `outcome` (Choice): what did the last command show?
//! - `chunk_<k>` (Noul): does chunk `k` of a long output decide the next
//!   step?
//! - `criterion_<j>` (Noul): does the evidence show requirement `j` met?
//!
//! Without a client, the same candidates become hints ordered by search
//! hits. That is the `--no-jev` baseline and the fallback when Jev fails.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use indexmap::IndexMap;
use jev::{Choice, Entry, Noul, Questions, SystemOneRequest};
use serde_json::json;

use atif::document::{Decision, Step};

use crate::agent::{Judge, Judgments};
use crate::credentials::{JEV_BASE_URL, JEV_MODEL};
use crate::record::Recorder;
use crate::state::{Issue, State, Turn};

/// The most candidate files one request judges.
const MAX_CANDIDATES: usize = 20;
/// Output longer than this is split into chunks for Jev to pick from.
const CHUNK_OVER: usize = 4_000;
const CHUNK_LINES: usize = 40;
const MAX_CHUNKS: usize = 12;
const MAX_CHUNK_CHARS: usize = 1_500;
/// A Noul at or above this reads as yes. An unmeasured development value.
const YES: f64 = 0.5;

/// A file code found that might matter to the issue.
#[derive(Debug, Clone)]
struct Candidate {
    path: String,
    hits: usize,
    excerpt: String,
}

/// The judge. `client` is `None` for the deterministic baseline.
pub struct JevJudge {
    client: Option<jev::Client>,
    workdir: PathBuf,
    keywords: Vec<String>,
    criteria: Vec<String>,
    /// Whether the workdir is a Git work tree. Terminal-Bench task
    /// directories often are not, and search falls back to a file walk.
    is_git: bool,
    recorder: Recorder,
    /// Requests answered and input tokens reported, for the run summary.
    pub calls: u32,
    pub input_tokens: u64,
    /// Requests that failed.
    pub failed: u32,
}

impl JevJudge {
    pub fn new(
        client: Option<jev::Client>,
        workdir: PathBuf,
        issue: &Issue,
        recorder: Recorder,
    ) -> Self {
        let is_git = git(&workdir, &["rev-parse", "--is-inside-work-tree"]).trim() == "true";
        Self {
            client,
            workdir,
            keywords: keywords(issue),
            criteria: criteria(&issue.body),
            is_git,
            recorder,
            calls: 0,
            input_tokens: 0,
            failed: 0,
        }
    }

    /// Every searchable file and how many of its lines hold a keyword,
    /// through Git when the workdir is a work tree and a bounded walk when
    /// it is not.
    fn search(&self) -> (BTreeSet<String>, BTreeMap<String, usize>) {
        let mut hits: BTreeMap<String, usize> = BTreeMap::new();
        if !self.is_git {
            let files = walk(&self.workdir);
            for path in &files {
                let Ok(text) = std::fs::read_to_string(self.workdir.join(path)) else {
                    continue;
                };
                let count = text
                    .lines()
                    .filter(|line| {
                        let lower = line.to_lowercase();
                        self.keywords.iter().any(|k| lower.contains(k.as_str()))
                    })
                    .count();
                if count > 0 {
                    hits.insert(path.clone(), count);
                }
            }
            return (files.into_iter().collect(), hits);
        }
        let tracked: BTreeSet<String> = git(&self.workdir, &["ls-files"])
            .lines()
            .map(str::to_string)
            .collect();
        if !self.keywords.is_empty() {
            let mut args = vec!["grep", "-I", "-i", "-c", "-F"];
            for keyword in &self.keywords {
                args.extend(["-e", keyword.as_str()]);
            }
            for line in git(&self.workdir, &args).lines() {
                if let Some((path, count)) = line.rsplit_once(':') {
                    hits.insert(path.to_string(), count.parse().unwrap_or(0));
                }
            }
        }
        (tracked, hits)
    }

    /// Files mentioned in the issue or already changed come first, then
    /// files by how many issue keywords they contain.
    fn candidates(&self, issue: &Issue) -> Vec<Candidate> {
        let (tracked, hits) = self.search();
        let mut first: Vec<String> = Vec::new();
        let text = format!("{}\n{}", issue.title, issue.body);
        for token in text.split(|c: char| c.is_whitespace() || "`'\"()[],".contains(c)) {
            let token = token.trim_matches(|c: char| c == '.' || c == ':');
            if token.contains('/') || token.contains('.') {
                // An absolute path inside the workdir names a file in it.
                let root = format!("{}/", self.workdir.to_string_lossy());
                let token = token.strip_prefix(root.as_str()).unwrap_or(token);
                let token = token.trim_start_matches("./");
                if tracked.contains(token) && !first.contains(&token.to_string()) {
                    first.push(token.to_string());
                }
            }
        }
        let status = if self.is_git {
            git(&self.workdir, &["status", "--porcelain"])
        } else {
            String::new()
        };
        for line in status.lines() {
            let path = line.get(3..).unwrap_or_default().to_string();
            if !path.is_empty() && !first.contains(&path) {
                first.push(path);
            }
        }
        let mut ranked: Vec<(String, usize)> = hits.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        let mut out = Vec::new();
        for path in first
            .into_iter()
            .chain(ranked.iter().map(|(path, _)| path.clone()))
        {
            if out.len() == MAX_CANDIDATES {
                break;
            }
            if out.iter().any(|c: &Candidate| c.path == path) {
                continue;
            }
            let hits = ranked
                .iter()
                .find(|(p, _)| *p == path)
                .map_or(0, |(_, n)| *n);
            let excerpt = self.excerpt(&path);
            out.push(Candidate {
                path,
                hits,
                excerpt,
            });
        }
        out
    }

    /// Up to three numbered lines of `path` that contain a keyword.
    fn excerpt(&self, path: &str) -> String {
        let Ok(text) = std::fs::read_to_string(self.workdir.join(path)) else {
            return String::new();
        };
        let mut lines = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            if self.keywords.iter().any(|k| lower.contains(k.as_str())) {
                lines.push(format!("{}: {}", number + 1, clip(line.trim(), 160)));
                if lines.len() == 3 {
                    break;
                }
            }
        }
        if lines.is_empty() {
            // No keyword inside: show the file's opening instead.
            for (number, line) in text.lines().take(3).enumerate() {
                lines.push(format!("{}: {}", number + 1, clip(line.trim(), 160)));
            }
        }
        lines.join("\n")
    }

    fn deterministic(&self, candidates: &[Candidate], note: Option<String>) -> Judgments {
        let mut hints = Vec::new();
        if let Some(note) = note {
            hints.push(note);
        }
        for candidate in candidates.iter().take(8) {
            hints.push(format!(
                "Candidate file {} ({} keyword hits):\n{}",
                candidate.path, candidate.hits, candidate.excerpt
            ));
        }
        for criterion in &self.criteria {
            hints.push(format!("Requirement from the issue: {criterion}"));
        }
        Judgments::Answered(hints)
    }

    async fn ask(
        &mut self,
        client: &jev::Client,
        state: &State,
        candidates: &[Candidate],
    ) -> Result<Vec<String>, String> {
        let last = state.history.iter().rev().find_map(|turn| match turn {
            Turn::Shell {
                command,
                observation,
                ..
            } => Some((command, observation)),
            Turn::Malformed { .. } => None,
        });
        let chunks: Vec<String> = match last {
            Some((_, observation)) if observation.output.len() > CHUNK_OVER => {
                chunk(&observation.output)
            }
            _ => Vec::new(),
        };
        let history: Vec<_> = state
            .history
            .iter()
            .rev()
            .take(15)
            .rev()
            .filter_map(|turn| match turn {
                Turn::Shell {
                    command,
                    observation,
                    ..
                } => Some(json!({ "command": command, "exit": observation.exit })),
                Turn::Malformed { .. } => None,
            })
            .collect();
        let jev_state = json!({
            "issue": {
                "title": state.issue.title,
                "body": clip(&state.issue.body, 8_000),
            },
            "candidates": candidates.iter().map(|c| json!({
                "path": c.path,
                "excerpt": c.excerpt,
            })).collect::<Vec<_>>(),
            "criteria": self.criteria,
            "history": history,
            "last": last.map(|(command, observation)| json!({
                "command": command,
                "exit": observation.exit,
                "output": if chunks.is_empty() { clip_tail(&observation.output, CHUNK_OVER) } else { String::new() },
                "chunks": chunks,
            })),
        });

        let mut questions = Questions::new();
        for i in 0..candidates.len() {
            questions = questions.with(
                format!("file_{i}"),
                Noul::new(format!(
                    "Would reading or editing the file `candidates[{i}]` help resolve the GitHub issue described in `issue`?"
                )),
            );
        }
        if last.is_some() {
            questions = questions.with(
                "outcome",
                Choice::new(
                    "What did the most recent command in `last` show about progress on the issue in `issue`?",
                    IndexMap::from([
                        ("progress".to_string(), Some(Entry::from("It produced useful information or a change that moves the issue toward resolution."))),
                        ("error".to_string(), Some(Entry::from("It failed with an error that needs diagnosis before continuing."))),
                        ("checks_passed".to_string(), Some(Entry::from("Tests, builds, or checks relevant to the issue ran and passed."))),
                        ("no_effect".to_string(), Some(Entry::from("It did nothing useful: no relevant output, a repeated command, or the wrong place."))),
                    ]),
                ),
            );
            for j in 0..self.criteria.len() {
                questions = questions.with(
                    format!("criterion_{j}"),
                    Noul::new(format!(
                        "Do the commands and outputs in `history` and `last` show that the requirement `criteria[{j}]` from the issue is already satisfied?"
                    )),
                );
            }
        }
        for k in 0..chunks.len() {
            questions = questions.with(
                format!("chunk_{k}"),
                Noul::new(format!(
                    "Does `last.chunks[{k}]` contain an error message or result that decides what to do next on the issue in `issue`?"
                )),
            );
        }

        if questions.is_empty() {
            // Nothing to judge yet: no candidate files and no command run.
            println!("  jev ▸ nothing to judge yet");
            return Ok(self
                .criteria
                .iter()
                .map(|criterion| format!("Requirement from the issue: {criterion}"))
                .collect());
        }
        let request = SystemOneRequest::new(Entry::from(jev_state), questions);
        let body = request
            .body(JEV_MODEL)
            .map(serde_json::Value::Object)
            .unwrap_or_else(|_| json!({}));
        let started = Instant::now();
        let result = client.system_one(request).await;
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut decision = Decision {
            id: format!("jev-{}", self.calls + self.failed + 1),
            name: "jev_step".to_string(),
            door: JEV_BASE_URL.to_string(),
            model: JEV_MODEL.to_string(),
            request: body,
            answers: serde_json::Value::Null,
            route: None,
            error: None,
            attempts: Vec::new(),
            review: None,
            milliseconds,
        };
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                self.failed += 1;
                decision.error = Some(error.to_string());
                self.recorder
                    .push(Step::called(decision.call()).taking(milliseconds));
                return Err(error.to_string());
            }
        };
        self.calls += 1;
        let tokens = response.usage.input_tokens.unwrap_or(0);
        self.input_tokens += tokens;
        decision.model = response.model.clone();
        decision.answers = serde_json::from_str::<serde_json::Value>(&response.raw().text())
            .ok()
            .and_then(|body| body.get("answers").cloned())
            .unwrap_or(serde_json::Value::Null);
        println!(
            "  jev ▸ {} answers in {} ms, {} input tokens",
            response.answers.len(),
            started.elapsed().as_millis(),
            tokens
        );

        let mut hints = Vec::new();
        let mut files: Vec<(f64, &Candidate)> = candidates
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Some((response.noul(&format!("file_{i}")).ok()?.noul, c)))
            .collect();
        files.sort_by(|a, b| b.0.total_cmp(&a.0));
        let confident: Vec<_> = files.iter().filter(|(p, _)| *p >= YES).take(6).collect();
        let shown: Vec<_> = if confident.is_empty() {
            files.iter().take(3).collect()
        } else {
            confident
        };
        for (p, candidate) in shown {
            let label = if *p >= YES {
                "Relevant file"
            } else {
                "Possibly relevant file (low)"
            };
            println!("  jev ▸ {label} {} p={p:.2}", candidate.path);
            hints.push(format!(
                "{label} (Jev p={p:.2}): {}\n{}",
                candidate.path, candidate.excerpt
            ));
        }
        if let Ok(outcome) = response.choice("outcome") {
            println!(
                "  jev ▸ last command: {} ({:.2})",
                outcome.choice, outcome.confidence
            );
            hints.push(format!(
                "Last command outcome (Jev): {} (confidence {:.2})",
                outcome.choice, outcome.confidence
            ));
        }
        let mut picked: Vec<(f64, usize)> = (0..chunks.len())
            .filter_map(|k| Some((response.noul(&format!("chunk_{k}")).ok()?.noul, k)))
            .filter(|(p, _)| *p >= YES)
            .collect();
        picked.sort_by(|a, b| b.0.total_cmp(&a.0));
        picked.truncate(3);
        picked.sort_by_key(|(_, k)| *k);
        for (p, k) in picked {
            println!("  jev ▸ key output chunk {k} p={p:.2}");
            hints.push(format!(
                "Key span of the last command's output (Jev p={p:.2}, chunk {k}):\n{}",
                chunks[k]
            ));
        }
        for (j, criterion) in self.criteria.iter().enumerate() {
            match response.noul(&format!("criterion_{j}")) {
                Ok(answer) => hints.push(format!(
                    "Requirement from the issue: {criterion} — evidence shows it satisfied: p={:.2} (Jev)",
                    answer.noul
                )),
                Err(_) => hints.push(format!("Requirement from the issue: {criterion}")),
            }
        }
        self.recorder.push(
            Step::called(decision.call())
                .taking(milliseconds)
                .noting("hints", json!(hints))
                .noting(
                    "jev_usage",
                    json!({
                        "input_tokens": response.usage.input_tokens,
                        "output_tokens": response.usage.output_tokens,
                    }),
                ),
        );
        Ok(hints)
    }
}

/// After this many steps with a clean checkout, the prompt says so.
const STALL_STEPS: usize = 5;

impl Judge for JevJudge {
    async fn judge(&mut self, state: &State) -> Judgments {
        let judgments = self.judge_step(state).await;
        let steps = state.history.len();
        let clean = git(&self.workdir, &["status", "--porcelain"])
            .trim()
            .is_empty();
        match judgments {
            Judgments::Answered(mut hints) if self.is_git && clean && steps >= STALL_STEPS => {
                println!("  host ▸ no file changed after {steps} steps");
                hints.insert(
                    0,
                    format!(
                        "No file in the checkout has changed after {steps} steps. If you \
                         understand the fix, write it into the files now instead of checking again."
                    ),
                );
                Judgments::Answered(hints)
            }
            judgments => judgments,
        }
    }
}

impl JevJudge {
    async fn judge_step(&mut self, state: &State) -> Judgments {
        println!("\n── step {} ──", state.history.len() + 1);
        let candidates = self.candidates(&state.issue);
        let Some(client) = self.client.take() else {
            println!(
                "  jev ▸ off; {} candidate files by search hits",
                candidates.len()
            );
            return self.deterministic(&candidates, None);
        };
        let result = self.ask(&client, state, &candidates).await;
        self.client = Some(client);
        match result {
            Ok(hints) => Judgments::Answered(hints),
            Err(error) => {
                println!("  jev ▸ unavailable: {error}");
                self.deterministic(
                    &candidates,
                    Some(format!(
                        "Jev was unavailable this step ({error}); files are ordered by search hits."
                    )),
                )
            }
        }
    }
}

/// Words from the issue worth searching for: code spans first, then
/// identifier-like words, then plain title words.
fn keywords(issue: &Issue) -> Vec<String> {
    const STOP: &[&str] = &[
        "this", "that", "with", "from", "have", "should", "would", "when", "then", "there",
        "their", "which", "into", "only", "also", "about", "what", "does", "each", "make", "more",
        "some", "than", "they", "will", "work", "just", "like", "need", "needs", "issue", "using",
        "used", "same", "other", "after", "before", "where", "these", "those", "being", "been",
        "were", "them", "such", "while", "because", "https", "github", "com",
    ];
    let mut out: Vec<String> = Vec::new();
    let add = |word: &str, out: &mut Vec<String>| {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        let lower = word.to_lowercase();
        if lower.len() >= 4 && !STOP.contains(&lower.as_str()) && !out.contains(&lower) {
            out.push(lower);
        }
    };
    let text = format!("{}\n{}", issue.title, issue.body);
    for (i, span) in text.split('`').enumerate() {
        if i % 2 == 1 && span.len() <= 60 {
            for word in span.split(|c: char| !c.is_alphanumeric() && c != '_') {
                add(word, &mut out);
            }
        }
    }
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let identifier = word.contains('_')
            || (word.chars().skip(1).any(char::is_uppercase)
                && word.chars().any(char::is_lowercase));
        if identifier {
            add(word, &mut out);
        }
    }
    for word in issue
        .title
        .split(|c: char| !c.is_alphanumeric() && c != '_')
    {
        add(word, &mut out);
    }
    out.truncate(12);
    out
}

/// Checkbox lines from the issue body, as requirements.
fn criteria(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let line = line.trim_start();
            line.strip_prefix("- [ ]")
                .or_else(|| line.strip_prefix("- [x]"))
                .or_else(|| line.strip_prefix("* [ ]"))
        })
        .map(|text| clip(text.trim(), 300))
        .filter(|text| !text.is_empty())
        .take(10)
        .collect()
}

fn chunk(output: &str) -> Vec<String> {
    let lines: Vec<&str> = output.lines().collect();
    let chunks: Vec<String> = lines
        .chunks(CHUNK_LINES)
        .map(|part| clip(&part.join("\n"), MAX_CHUNK_CHARS))
        .collect();
    if chunks.len() <= MAX_CHUNKS {
        return chunks;
    }
    // Keep the first chunk and the last ones: the command's framing and
    // where it ended. The indexes stay the original ones' order.
    let mut kept = vec![chunks[0].clone()];
    kept.extend(chunks[chunks.len() - (MAX_CHUNKS - 1)..].iter().cloned());
    kept
}

/// Files under `root` a search should read: relative paths, skipping
/// hidden directories, dependency and build trees, and files over 256
/// KiB, at most 5,000 files.
fn walk(root: &std::path::Path) -> Vec<String> {
    const SKIP: &[&str] = &[
        "node_modules",
        "target",
        "__pycache__",
        "venv",
        "site-packages",
        "dist",
        "build",
    ];
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if depth < 8 && !SKIP.contains(&name.as_str()) {
                    pending.push((path, depth + 1));
                }
            } else if kind.is_file()
                && entry.metadata().is_ok_and(|meta| meta.len() <= 256 * 1024)
                && let Ok(relative) = path.strip_prefix(root)
            {
                files.push(relative.to_string_lossy().into_owned());
                if files.len() == 5_000 {
                    return files;
                }
            }
        }
    }
    files
}

fn git(workdir: &std::path::Path, args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .current_dir(workdir)
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// At most `max` characters of `text`, marked when cut.
pub fn clip(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

/// The last `max` characters of `text`, marked when cut.
fn clip_tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let start = text
        .char_indices()
        .nth(count - max)
        .map_or(0, |(index, _)| index);
    format!("…{}", &text[start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(title: &str, body: &str) -> Issue {
        Issue {
            url: String::new(),
            title: title.to_string(),
            body: body.to_string(),
            labels: vec![],
        }
    }

    #[test]
    fn keywords_prefer_code_spans_and_identifiers() {
        let words = keywords(&issue(
            "Parser panics on empty input",
            "Calling `parse_line` with `\"\"` panics in lexer. See tokenStream too.",
        ));
        assert_eq!(words[0], "parse_line");
        assert!(words.contains(&"tokenstream".to_string()));
        assert!(words.contains(&"parser".to_string()));
        assert!(!words.contains(&"with".to_string()));
    }

    #[test]
    fn criteria_are_checkbox_lines() {
        let found = criteria("Intro\n- [ ] Add a flag\n  - [x] Test it\n- plain item\n");
        assert_eq!(found, ["Add a flag", "Test it"]);
    }

    #[test]
    fn long_output_keeps_its_first_and_last_chunks() {
        let output: String = (0..1_000).map(|n| format!("line {n}\n")).collect();
        let chunks = chunk(&output);
        assert_eq!(chunks.len(), MAX_CHUNKS);
        assert!(chunks[0].starts_with("line 0"));
        assert!(chunks[MAX_CHUNKS - 1].contains("line 999"));
    }
}
