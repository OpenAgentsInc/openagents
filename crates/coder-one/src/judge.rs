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
use crate::delegate::{Evidence, Span};
use crate::record::Recorder;
use crate::state::{Issue, State, Surveyed, Turn};

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
pub(crate) struct Candidate {
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
    /// What the judgments found so far, kept structured for a delegate's
    /// briefing and the escalation policy.
    pub evidence: Evidence,
    /// Deep mode: a parallel survey before the first step, a readiness
    /// question each step, and hints about repeated commands.
    deep: bool,
    /// Probe mode: before the survey, run a fixed battery of read-only
    /// commands and let Jev pick the outputs worth handing on.
    probes: bool,
    /// Probe v2: a Jev-gated setup pack, git probes in the repositories the
    /// task names, and whole edit targets.
    v2: bool,
    /// The most files the deep survey judges.
    survey_files: usize,
}

/// The most characters of one probe's output Jev reads and a briefing keeps.
const PROBE_OUTPUT_CHARS: usize = 6_000;
/// The most probe outputs, and characters, a briefing carries.
const PROBE_KEEP: usize = 6;
const PROBE_TOTAL_CHARS: usize = 14_000;

/// The most files the survey judges, in batches of [`SURVEY_BATCH`].
pub(crate) const SURVEY_FILES: usize = 100;
/// Probe v2's smaller survey pool: Jev's file survey was most of its cost.
pub(crate) const SURVEY_FILES_V2: usize = 40;
/// The most characters of one likely edit target probe v2 hands on.
const EDIT_TARGET_CHARS: usize = 16_000;
const SURVEY_BATCH: usize = 20;
/// The most files, and characters, the survey puts in the prompt.
const SURVEY_KEEP: usize = 6;
const SURVEY_FILE_CHARS: usize = 8_000;
const SURVEY_TOTAL_CHARS: usize = 24_000;
/// Files the survey always offers when present: where a repository says
/// how it builds and tests.
const MANIFESTS: &[&str] = &[
    "README.md",
    "README",
    "setup.py",
    "setup.cfg",
    "pyproject.toml",
    "Cargo.toml",
    "package.json",
    "Makefile",
    "requirements.txt",
    "go.mod",
];

impl JevJudge {
    pub fn new(
        client: Option<jev::Client>,
        workdir: PathBuf,
        issue: &Issue,
        recorder: Recorder,
    ) -> Self {
        let is_git = git(&workdir, &["rev-parse", "--is-inside-work-tree"]).trim() == "true";
        let criteria = criteria(&issue.body);
        let evidence = Evidence {
            criteria: criteria.iter().map(|c| (c.clone(), None)).collect(),
            ..Evidence::default()
        };
        Self {
            client,
            workdir,
            keywords: keywords(issue),
            criteria,
            is_git,
            recorder,
            calls: 0,
            input_tokens: 0,
            failed: 0,
            evidence,
            deep: false,
            probes: false,
            v2: false,
            survey_files: SURVEY_FILES,
        }
    }

    /// Sets how many files the deep survey judges, at most
    /// [`SURVEY_FILES`].
    #[must_use]
    pub fn survey_files(mut self, files: usize) -> Self {
        self.survey_files = files.clamp(1, SURVEY_FILES);
        self
    }

    /// The judge's switches, for the record and the policy canaries:
    /// whether Jev runs, deep mode, probes, probe v2, and the survey size.
    #[must_use]
    pub fn switches(&self) -> (bool, bool, bool, bool, usize) {
        (
            self.client.is_some(),
            self.deep,
            self.probes,
            self.v2,
            self.survey_files,
        )
    }

    /// Turns on probe v2.
    #[must_use]
    pub fn probe_v2(mut self, v2: bool) -> Self {
        self.v2 = v2;
        self
    }

    /// The setup pack: commands the instruction itself names in code spans
    /// (a `git clone`, a `pip install`) that a delegate would otherwise
    /// spend its first turns on. One Jev request decides, per command,
    /// whether the task needs it run before the work; the host runs the
    /// approved ones in order, bounded and without credentials, and the
    /// outcomes join the survey so the briefing reports them.
    async fn setup(&mut self, client: &jev::Client, state: &mut State) {
        let commands = setup_commands(&state.issue.body);
        if commands.is_empty() {
            return;
        }
        let mut questions = Questions::new();
        for i in 0..commands.len() {
            questions = questions.with(
                format!("setup_{i}"),
                Noul::new(format!(
                    "Does the task in `issue` require running the command `setup[{i}]` as written before the rest of the work can start?"
                )),
            );
        }
        let jev_state = json!({
            "issue": { "title": state.issue.title, "body": clip(&state.issue.body, 8_000) },
            "setup": commands,
        });
        let Some(answers) = self
            .ask_nouls(client, "jev_setup", jev_state, questions)
            .await
        else {
            return;
        };
        for (i, command) in commands.iter().enumerate() {
            let p = answers.get(&format!("setup_{i}")).copied().unwrap_or(0.0);
            if p < YES {
                println!("  setup ▸ skipped `{command}` p={p:.2}");
                continue;
            }
            if let Some(destination) = clone_destination(command)
                && std::path::Path::new(&destination).exists()
            {
                println!("  setup ▸ `{command}`: {destination} already exists");
                continue;
            }
            let mut prepared = std::process::Command::new("bash");
            prepared
                .arg("-c")
                .arg(command)
                .current_dir(&self.workdir)
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("PYTHONDONTWRITEBYTECODE", "1");
            for (name, _) in std::env::vars_os() {
                if name.to_str().is_some_and(crate::shell::is_credential) {
                    prepared.env_remove(&name);
                }
            }
            let started = Instant::now();
            let ended = supervise::Job::from_command(prepared)
                .bounded(
                    supervise::Limits::within(std::time::Duration::from_secs(240))
                        .keeping(16 * 1024),
                )
                .run()
                .await;
            let mut output = ended.stdout.marked();
            if !ended.stderr.is_empty() {
                output.push('\n');
                output.push_str(&ended.stderr.marked());
            }
            println!(
                "  setup ▸ ran `{command}` p={p:.2}: {} in {:.1}s",
                ended.ending,
                started.elapsed().as_secs_f64()
            );
            state.survey.push(Surveyed {
                path: format!(
                    "$ {command}   (setup the host already ran: {})",
                    ended.ending
                ),
                relevance: p,
                edit: 0.0,
                content: clip_tail(output.trim(), 2_000),
            });
        }
    }

    /// One Jev request of Nouls, recorded like the others; the answers by
    /// question id, or `None` when Jev could not answer.
    async fn ask_nouls(
        &mut self,
        client: &jev::Client,
        name: &str,
        jev_state: serde_json::Value,
        questions: Questions,
    ) -> Option<BTreeMap<String, f64>> {
        let request = SystemOneRequest::new(Entry::from(jev_state), questions);
        let body = request
            .body(JEV_MODEL)
            .map(serde_json::Value::Object)
            .unwrap_or_else(|_| json!({}));
        let asked = Instant::now();
        let result = client.system_one(request).await;
        let milliseconds = u64::try_from(asked.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut decision = Decision {
            id: format!("{name}-{}", self.calls + self.failed + 1),
            name: name.to_string(),
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
                println!("  {name} ▸ Jev unavailable: {error}");
                return None;
            }
        };
        self.calls += 1;
        self.input_tokens += response.usage.input_tokens.unwrap_or(0);
        decision.model = response.model.clone();
        decision.answers = serde_json::from_str::<serde_json::Value>(&response.raw().text())
            .ok()
            .and_then(|body| body.get("answers").cloned())
            .unwrap_or(serde_json::Value::Null);
        self.recorder
            .push(Step::called(decision.call()).taking(milliseconds).noting(
                "jev_usage",
                json!({
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                }),
            ));
        Some(
            response
                .nouls()
                .map(|(id, answer)| (id.to_string(), answer.noul))
                .collect(),
        )
    }

    /// Turns on probe mode.
    #[must_use]
    pub fn probing(mut self, probes: bool) -> Self {
        self.probes = probes;
        self
    }

    /// The probe battery: cheap, read-only commands a delegate would
    /// otherwise spend its first turns on, run in parallel. Jev judges
    /// which outputs carry information the task needs, and the chosen
    /// outputs join `state.survey`, so the briefing carries them.
    async fn probe(&mut self, client: &jev::Client, state: &mut State) {
        let started = Instant::now();
        let mut battery: Vec<String> = vec![
            "pwd && ls -la".to_string(),
            "find . -maxdepth 3 -not -path '*/.git/*' -not -path '*/node_modules/*' -not -path '*/__pycache__/*' | head -150".to_string(),
            "for f in README* readme*; do [ -f \"$f\" ] && head -120 \"$f\"; done".to_string(),
            "find . -maxdepth 4 \\( -name 'test_*.py' -o -name '*_test.py' -o -name tests -o -name '*.test.*' \\) -not -path '*/.git/*' | head -40".to_string(),
            "python3 --version 2>&1; pip list 2>/dev/null | head -60".to_string(),
        ];
        if self.is_git {
            for command in [
                "git status",
                "git branch -a -vv",
                "git log --oneline --graph --all -n 40",
                "git reflog -n 40",
                "git stash list",
            ] {
                battery.push(command.to_string());
            }
        }
        // Absolute paths the instruction names: list a directory, read the
        // head of a file.
        let text = format!("{}\n{}", state.issue.title, state.issue.body);
        let mut named = BTreeSet::new();
        for token in text.split(|c: char| c.is_whitespace() || "`'\"(),".contains(c)) {
            let token = token.trim_end_matches(['.', ':', ';']);
            if token.starts_with('/') && token.len() > 1 && named.len() < 6 {
                let path = std::path::Path::new(token);
                if path.is_dir() {
                    named.insert(format!("ls -la {token}"));
                    // A repository the task names, outside the workdir.
                    if self.v2
                        && git(path, &["rev-parse", "--is-inside-work-tree"]).trim() == "true"
                        && !self.is_git
                    {
                        for sub in [
                            "status",
                            "log --oneline --graph --all -n 40",
                            "reflog -n 40",
                            "branch -a -vv",
                        ] {
                            named.insert(format!("git -C {token} {sub}"));
                        }
                    }
                } else if path.is_file() {
                    named.insert(format!("head -200 {token}"));
                }
            }
        }
        battery.extend(named);

        let runs = battery.iter().map(|command| {
            let mut prepared = std::process::Command::new("bash");
            prepared
                .arg("-c")
                .arg(command)
                .current_dir(&self.workdir)
                .env("GIT_PAGER", "cat")
                .env("PAGER", "cat");
            for (name, _) in std::env::vars_os() {
                if name.to_str().is_some_and(crate::shell::is_credential) {
                    prepared.env_remove(&name);
                }
            }
            supervise::Job::from_command(prepared)
                .bounded(
                    supervise::Limits::within(std::time::Duration::from_secs(10))
                        .keeping(16 * 1024),
                )
                .run()
        });
        let ended = futures_util::future::join_all(runs).await;
        let outputs: Vec<(String, String)> = battery
            .iter()
            .zip(ended)
            .filter_map(|(command, ended)| {
                let mut output = ended.stdout.marked();
                if !ended.stderr.is_empty() {
                    output.push('\n');
                    output.push_str(&ended.stderr.marked());
                }
                let output = output.trim().to_string();
                (!output.is_empty()).then(|| (command.clone(), clip(&output, PROBE_OUTPUT_CHARS)))
            })
            .collect();
        if outputs.is_empty() {
            return;
        }

        let mut questions = Questions::new();
        for i in 0..outputs.len() {
            questions = questions.with(
                format!("probe_{i}"),
                Noul::new(format!(
                    "Does the output in `probes[{i}].output` contain information someone needs to complete the task in `issue`, such as where the relevant code or data is, what state it is in, or what went wrong?"
                )),
            );
        }
        let jev_state = json!({
            "issue": {
                "title": state.issue.title,
                "body": clip(&state.issue.body, 8_000),
            },
            "probes": outputs.iter().map(|(command, output)| json!({
                "command": command,
                "output": clip(output, 3_000),
            })).collect::<Vec<_>>(),
        });
        let request = SystemOneRequest::new(Entry::from(jev_state), questions);
        let body = request
            .body(JEV_MODEL)
            .map(serde_json::Value::Object)
            .unwrap_or_else(|_| json!({}));
        let asked = Instant::now();
        let result = client.system_one(request).await;
        let milliseconds = u64::try_from(asked.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut decision = Decision {
            id: format!("jev-probe-{}", self.calls + self.failed + 1),
            name: "jev_probe".to_string(),
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
                println!("  probe ▸ Jev unavailable: {error}");
                return;
            }
        };
        self.calls += 1;
        self.input_tokens += response.usage.input_tokens.unwrap_or(0);
        decision.model = response.model.clone();
        decision.answers = serde_json::from_str::<serde_json::Value>(&response.raw().text())
            .ok()
            .and_then(|body| body.get("answers").cloned())
            .unwrap_or(serde_json::Value::Null);
        self.recorder
            .push(Step::called(decision.call()).taking(milliseconds).noting(
                "jev_usage",
                json!({
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                }),
            ));
        let mut picked: Vec<(f64, &(String, String))> = outputs
            .iter()
            .enumerate()
            .filter_map(|(i, probe)| {
                let p = response.noul(&format!("probe_{i}")).ok()?.noul;
                (p >= YES).then_some((p, probe))
            })
            .collect();
        picked.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut total = 0;
        for (p, (command, output)) in picked.into_iter().take(PROBE_KEEP) {
            if total + output.chars().count() > PROBE_TOTAL_CHARS {
                continue;
            }
            total += output.chars().count();
            println!("  probe ▸ kept `{command}` p={p:.2}");
            state.survey.push(Surveyed {
                path: format!("$ {command}"),
                relevance: p,
                edit: 0.0,
                content: output.clone(),
            });
        }
        println!(
            "  probe ▸ {} probes run and judged in {} ms; {} chars kept",
            outputs.len(),
            started.elapsed().as_millis(),
            total
        );
    }

    /// Turns on deep mode.
    #[must_use]
    pub fn deep(mut self, deep: bool) -> Self {
        self.deep = deep;
        self
    }

    /// The survey: before the first step, Jev judges up to
    /// [`SURVEY_FILES`] candidate files in parallel requests, and the most
    /// relevant files' contents go into `state.survey`, where every prompt
    /// carries them in its stable prefix. The generator can then start
    /// from the code instead of spending steps finding it.
    pub async fn survey(&mut self, state: &mut State) {
        let Some(client) = self.client.clone().filter(|_| self.deep) else {
            return;
        };
        if self.probes && self.v2 {
            self.setup(&client, state).await;
        }
        if self.probes {
            self.probe(&client, state).await;
        }
        let started = Instant::now();
        let pool = self.survey_pool(&state.issue);
        if pool.is_empty() {
            println!("  survey ▸ no candidate files");
            return;
        }
        let batches: Vec<Vec<Candidate>> = pool
            .chunks(SURVEY_BATCH)
            .map(<[Candidate]>::to_vec)
            .collect();
        let issue = json!({
            "title": state.issue.title,
            "body": clip(&state.issue.body, 8_000),
        });
        let requests = batches.iter().map(|batch| {
            let mut questions = Questions::new();
            for i in 0..batch.len() {
                questions = questions
                    .with(
                        format!("rel_{i}"),
                        Noul::new(format!(
                            "Would reading or editing the file `files[{i}]` help resolve the task described in `issue`?"
                        )),
                    )
                    .with(
                        format!("edit_{i}"),
                        Noul::new(format!(
                            "Will resolving the task described in `issue` most likely require changing the file `files[{i}]`?"
                        )),
                    );
            }
            let jev_state = json!({
                "issue": issue,
                "files": batch.iter().map(|c| json!({"path": c.path, "excerpt": c.excerpt})).collect::<Vec<_>>(),
            });
            let request = SystemOneRequest::new(Entry::from(jev_state), questions);
            let body = request
                .body(JEV_MODEL)
                .map(serde_json::Value::Object)
                .unwrap_or_else(|_| json!({}));
            let client = client.clone();
            async move {
                let started = Instant::now();
                let result = client.system_one(request).await;
                (body, result, started.elapsed())
            }
        });
        let results = futures_util::future::join_all(requests).await;

        let mut scored: Vec<(f64, f64, &Candidate)> = Vec::new();
        for ((body, result, elapsed), batch) in results.into_iter().zip(&batches) {
            let milliseconds = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
            let mut decision = Decision {
                id: format!("jev-survey-{}", self.calls + self.failed + 1),
                name: "jev_survey".to_string(),
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
            match result {
                Ok(response) => {
                    self.calls += 1;
                    self.input_tokens += response.usage.input_tokens.unwrap_or(0);
                    decision.model = response.model.clone();
                    decision.answers =
                        serde_json::from_str::<serde_json::Value>(&response.raw().text())
                            .ok()
                            .and_then(|body| body.get("answers").cloned())
                            .unwrap_or(serde_json::Value::Null);
                    for (i, candidate) in batch.iter().enumerate() {
                        let rel = response.noul(&format!("rel_{i}")).map_or(0.0, |a| a.noul);
                        let edit = response.noul(&format!("edit_{i}")).map_or(0.0, |a| a.noul);
                        scored.push((rel, edit, candidate));
                    }
                    self.recorder
                        .push(Step::called(decision.call()).taking(milliseconds).noting(
                            "jev_usage",
                            json!({
                                "input_tokens": response.usage.input_tokens,
                                "output_tokens": response.usage.output_tokens,
                            }),
                        ));
                }
                Err(error) => {
                    self.failed += 1;
                    decision.error = Some(error.to_string());
                    self.recorder
                        .push(Step::called(decision.call()).taking(milliseconds));
                }
            }
        }

        // Rank by relevance, and let a likely edit break ties.
        scored.sort_by(|a, b| (b.0 + 0.1 * b.1).total_cmp(&(a.0 + 0.1 * a.1)));
        let mut total = 0;
        for (rel, edit, candidate) in scored
            .iter()
            .filter(|(rel, ..)| *rel >= YES)
            .take(SURVEY_KEEP)
        {
            let Ok(text) = std::fs::read_to_string(self.workdir.join(&candidate.path)) else {
                continue;
            };
            // Probe v2 hands likely edit targets on whole, so the delegate
            // edits instead of reading first.
            let cap = if self.v2 && *edit >= 0.8 {
                EDIT_TARGET_CHARS
            } else {
                SURVEY_FILE_CHARS
            };
            let room = cap.min(SURVEY_TOTAL_CHARS.saturating_sub(total));
            if room < 500 {
                break;
            }
            let content = if text.chars().count() > room {
                format!(
                    "{}\n…[cut at {room} of {} characters; read the rest if needed]",
                    clip(&text, room),
                    text.chars().count()
                )
            } else {
                text
            };
            total += content.chars().count();
            state.survey.push(Surveyed {
                path: candidate.path.clone(),
                relevance: *rel,
                edit: *edit,
                content,
            });
        }
        println!(
            "  survey ▸ {} files judged in {} parallel Jev requests, {} ms; {} put in the prompt ({} chars)",
            scored.len(),
            batches.len(),
            started.elapsed().as_millis(),
            state.survey.len(),
            total
        );
        for file in &state.survey {
            println!(
                "  survey ▸ {} relevance {:.2} edit {:.2}",
                file.path, file.relevance, file.edit
            );
        }
    }

    /// The files the survey judges: those the issue names, then by
    /// keyword hits, then build manifests, then short paths, at most
    /// [`SURVEY_FILES`].
    pub(crate) fn survey_pool(&self, issue: &Issue) -> Vec<Candidate> {
        let (tracked, hits) = self.search();
        let mut order: Vec<String> = self.candidates(issue).into_iter().map(|c| c.path).collect();
        let mut ranked: Vec<(&String, &usize)> = hits.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let mut rest: Vec<&String> = tracked.iter().collect();
        rest.sort_by_key(|path| (path.matches('/').count(), path.len()));
        let manifests = tracked.iter().filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            path.matches('/').count() <= 1 && MANIFESTS.contains(&name)
        });
        for path in ranked
            .into_iter()
            .map(|(path, _)| path)
            .chain(manifests)
            .chain(rest)
        {
            if order.len() >= self.survey_files {
                break;
            }
            if !order.contains(path) {
                order.push(path.clone());
            }
        }
        order.truncate(self.survey_files);
        order
            .into_iter()
            .map(|path| {
                let excerpt = self.excerpt(&path);
                Candidate {
                    path,
                    hits: 0,
                    excerpt,
                }
            })
            .collect()
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
            if self.deep {
                questions = questions.with(
                    "ready",
                    Noul::new(
                        "Do the commands and outputs in `history` and `last` show that the task in `issue` is complete and its result has been checked?",
                    ),
                );
            }
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
        let outcome_label = response
            .choice("outcome")
            .ok()
            .map(|outcome| outcome.choice.clone());
        if last.is_some() {
            self.evidence.outcomes.push(outcome_label);
        }
        for (i, candidate) in candidates.iter().enumerate() {
            if let Ok(answer) = response.noul(&format!("file_{i}")) {
                self.evidence.relevance(&candidate.path, answer.noul);
            }
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
            if let Some((command, _)) = last {
                self.evidence.spans.push(Span {
                    step: state.history.len(),
                    command: clip(command, 200),
                    p,
                    text: chunks[k].clone(),
                });
            }
            hints.push(format!(
                "Key span of the last command's output (Jev p={p:.2}, chunk {k}):\n{}",
                chunks[k]
            ));
        }
        if let Ok(ready) = response.noul("ready") {
            println!("  jev ▸ done and checked: p={:.2}", ready.noul);
            if ready.noul >= 0.8 {
                hints.push(format!(
                    "The evidence suggests the task is complete and checked (Jev p={:.2}). If nothing is left to verify, call finished now.",
                    ready.noul
                ));
            }
        }
        for (j, criterion) in self.criteria.iter().enumerate() {
            match response.noul(&format!("criterion_{j}")) {
                Ok(answer) => {
                    if let Some(slot) = self.evidence.criteria.get_mut(j) {
                        slot.1 = Some(answer.noul);
                    }
                    hints.push(format!(
                    "Requirement from the issue: {criterion} — evidence shows it satisfied: p={:.2} (Jev)",
                    answer.noul
                ))
                }
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
        let mut judgments = self.judge_step(state).await;
        if self.deep
            && let Judgments::Answered(hints) = &mut judgments
        {
            let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
            for turn in &state.history {
                if let Turn::Shell { command, .. } = turn {
                    *counts.entry(command.as_str()).or_default() += 1;
                }
            }
            for (command, count) in counts.into_iter().filter(|(_, count)| *count >= 2) {
                hints.push(format!(
                    "You have run `{}` {count} times; its latest output is in the history. Do not run it again unless something changed.",
                    clip(command, 120)
                ));
            }
        }
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
        for candidate in &candidates {
            self.evidence
                .candidate(&candidate.path, candidate.hits, &candidate.excerpt);
        }
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

/// The closing check's answers: whether the task looks done, and each
/// requirement's probability of being met, after a delegate ran.
#[derive(Debug, Clone, PartialEq)]
pub struct Close {
    /// Jev's probability that the evidence shows the whole task done.
    pub done: Option<f64>,
    /// Each requirement with its probability of being met.
    pub criteria: Vec<(String, Option<f64>)>,
    /// Why the check has no answers, when it has none.
    pub unavailable: Option<String>,
}

impl JevJudge {
    /// Asks Jev once more, after a delegate ran, whether the task and each
    /// requirement now look satisfied. `delegate` is the delegate's final
    /// report and `changes` is what changed in the working directory.
    pub async fn close(&mut self, state: &State, delegate: &str, changes: &str) -> Close {
        let unanswered = |criteria: &[String], why: String| Close {
            done: None,
            criteria: criteria.iter().map(|c| (c.clone(), None)).collect(),
            unavailable: Some(why),
        };
        let Some(client) = self.client.take() else {
            return unanswered(&self.criteria, "Jev is off".to_string());
        };
        let jev_state = json!({
            "issue": {
                "title": state.issue.title,
                "body": clip(&state.issue.body, 8_000),
            },
            "criteria": self.criteria,
            "delegate": {
                "report": clip_tail(delegate, 3_000),
                "changes": clip(changes, 5_000),
            },
        });
        let mut questions = Questions::new().with(
            "done",
            Noul::new(
                "Do the delegate's report in `delegate.report` and the changes in `delegate.changes` show that the task described in `issue` is complete?",
            ),
        );
        for j in 0..self.criteria.len() {
            questions = questions.with(
                format!("criterion_{j}"),
                Noul::new(format!(
                    "Do `delegate.report` and `delegate.changes` show that the requirement `criteria[{j}]` from the issue is satisfied?"
                )),
            );
        }
        let request = SystemOneRequest::new(Entry::from(jev_state), questions);
        let body = request
            .body(JEV_MODEL)
            .map(serde_json::Value::Object)
            .unwrap_or_else(|_| json!({}));
        let started = Instant::now();
        let result = client.system_one(request).await;
        self.client = Some(client);
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut decision = Decision {
            id: format!("jev-{}", self.calls + self.failed + 1),
            name: "jev_close".to_string(),
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
                return unanswered(&self.criteria, error.to_string());
            }
        };
        self.calls += 1;
        self.input_tokens += response.usage.input_tokens.unwrap_or(0);
        decision.model = response.model.clone();
        decision.answers = serde_json::from_str::<serde_json::Value>(&response.raw().text())
            .ok()
            .and_then(|body| body.get("answers").cloned())
            .unwrap_or(serde_json::Value::Null);
        let close = Close {
            done: response.noul("done").ok().map(|answer| answer.noul),
            criteria: self
                .criteria
                .iter()
                .enumerate()
                .map(|(j, c)| {
                    let p = response
                        .noul(&format!("criterion_{j}"))
                        .ok()
                        .map(|answer| answer.noul);
                    (c.clone(), p)
                })
                .collect(),
            unavailable: None,
        };
        self.evidence.criteria.clone_from(&close.criteria);
        self.recorder
            .push(Step::called(decision.call()).taking(milliseconds).noting(
                "jev_usage",
                json!({
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                }),
            ));
        close
    }
}

/// Setup commands the instruction names in inline code spans: a
/// `git clone`, or a `pip install`, at most three. A clone with no
/// destination gets the absolute path named after it in the same sentence
/// ("to `/app/pyknotid`").
pub fn setup_commands(text: &str) -> Vec<String> {
    let spans: Vec<&str> = text.split('`').collect();
    let mut out = Vec::new();
    for i in (1..spans.len()).step_by(2) {
        let command = spans[i].trim();
        let setup = command.starts_with("git clone ")
            || command.starts_with("pip install ")
            || command.starts_with("pip3 install ")
            || command.starts_with("python3 -m pip install ")
            || command.starts_with("python -m pip install ");
        if !setup || command.contains('\n') || out.len() == 3 {
            continue;
        }
        let mut command = command.to_string();
        if command.starts_with("git clone ") && clone_destination(&command).is_none() {
            // The next code span in the same sentence, when it is a path.
            let between = spans.get(i + 1).copied().unwrap_or_default();
            if let Some(next) = spans.get(i + 2)
                && next.starts_with('/')
                && !between.contains('.')
                && between
                    .split_whitespace()
                    .any(|word| word == "to" || word == "into")
            {
                command.push(' ');
                command.push_str(next.trim());
            }
        }
        out.push(command);
    }
    out
}

/// A `git clone` command's explicit destination: its last argument when
/// that is an absolute path.
fn clone_destination(command: &str) -> Option<String> {
    if !command.starts_with("git clone ") {
        return None;
    }
    let last = command.split_whitespace().last()?;
    last.starts_with('/').then(|| last.to_string())
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
pub(crate) fn walk(root: &std::path::Path) -> Vec<String> {
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

pub(crate) fn git(workdir: &std::path::Path, args: &[&str]) -> String {
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
pub(crate) fn clip_tail(text: &str, max: usize) -> String {
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
    fn setup_commands_come_from_code_spans_with_their_destination() {
        let text = "You should clone the source code with `git clone --depth 1 --branch 0.5.3 https://github.com/SPOCKnots/pyknotid.git` to `/app/pyknotid`.\nThen `import pyknotid` works. Run `pip install -e .` too.";
        assert_eq!(
            setup_commands(text),
            [
                "git clone --depth 1 --branch 0.5.3 https://github.com/SPOCKnots/pyknotid.git /app/pyknotid",
                "pip install -e .",
            ]
        );
        assert_eq!(
            clone_destination("git clone url /app/x"),
            Some("/app/x".to_string())
        );
        assert!(setup_commands("Use `ls` and `python3 run.py`.").is_empty());
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
