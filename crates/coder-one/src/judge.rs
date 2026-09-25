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
use jev::{Choice, Entry, Noul, Questions, RetryPolicy, SystemOneRequest};
use serde_json::json;

use atif::document::{Decision, Step};

use crate::agent::{Judge, Judgments};
use crate::component::jev::{
    DEADLINE_SKIP, JEV_CALL_BUDGET, charge_answered, charge_failed, charge_skipped,
};
use crate::component::{
    self,
    evidence::{self, Probe, SURVEY_BATCH, YES, issue_state},
    jev::{Ask, Asked, JevMode},
};
use crate::credentials::{JEV_BASE_URL, JEV_MODEL};
use crate::deadline::Deadline;
use crate::delegate::{Evidence, Span};
use crate::record::{Cost, Finish, Outcome, Recorder, Start};
use crate::state::{Issue, State, Surveyed, Turn};

/// The most candidate files one request judges.
/// The largest file the keyword survey reads, in bytes.
const SEARCH_FILE_CAP: u64 = 512 * 1024;

const MAX_CANDIDATES: usize = 20;
/// The most requirements the step and closing checks ask about.
const MAX_CRITERIA: usize = 12;
/// Output longer than this is split into chunks for Jev to pick from.
const CHUNK_OVER: usize = 4_000;
const CHUNK_LINES: usize = 40;
const MAX_CHUNKS: usize = 12;
const MAX_CHUNK_CHARS: usize = 1_500;
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
    /// The requirement map `criteria` comes from: by rule at first, from
    /// Jev once the survey has asked.
    pub requirements: crate::requirements::RequirementMap,
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
    /// `evidence.environment`: presence probes and the briefing line
    /// that states what they found, under this template. `None` is off.
    environment: Option<crate::environment::Params>,
    /// `evidence.data_profile`: profile every data file in the probe
    /// stage, under these bounds. `None` is off.
    data_profile: Option<crate::data_profile::Params>,
    /// The most files the deep survey judges.
    survey_files: usize,
    /// The evidence guests the probe stage runs, when the manifest turns
    /// them on.
    guests: Option<crate::guests::Policy>,
    /// The episode deadline every request and command is bounded by.
    deadline: Deadline,
    /// Requests the deadline skipped before they were sent.
    pub skipped: u32,
}

/// The most characters of one probe's output a briefing keeps.
const PROBE_OUTPUT_CHARS: usize = 6_000;

/// The most files the survey judges, in batches of [`SURVEY_BATCH`].
pub(crate) const SURVEY_FILES: usize = 100;
/// Probe v2's smaller survey pool: Jev's file survey was most of its cost.
pub(crate) const SURVEY_FILES_V2: usize = 40;
/// The most characters of one likely edit target probe v2 hands on.
const EDIT_TARGET_CHARS: usize = 16_000;
/// The most characters of one file, and of all files, the survey puts in
/// the prompt.
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
        // The requirement map by rule; the survey asks Jev for a better one.
        let requirements = crate::requirements::mechanical(&issue.body);
        let criteria = requirements.criteria(MAX_CRITERIA);
        let evidence = Evidence {
            criteria: criteria.iter().map(|c| (c.clone(), None)).collect(),
            ..Evidence::default()
        };
        Self {
            client,
            workdir,
            keywords: keywords(issue),
            criteria,
            requirements,
            is_git,
            recorder,
            calls: 0,
            input_tokens: 0,
            failed: 0,
            evidence,
            deep: false,
            probes: false,
            v2: false,
            environment: None,
            data_profile: None,
            survey_files: SURVEY_FILES,
            guests: None,
            deadline: Deadline::unbounded(),
            skipped: 0,
        }
    }

    /// Bounds every Jev request, setup command, and probe by the episode
    /// `deadline`.
    #[must_use]
    pub fn within(mut self, deadline: Deadline) -> Self {
        self.deadline = deadline;
        self
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

    /// Turns on the evidence guests, which run in the probe stage.
    #[must_use]
    pub fn guests(mut self, policy: Option<crate::guests::Policy>) -> Self {
        self.guests = policy;
        self
    }

    /// The evidence guests the probe stage runs, if any.
    #[must_use]
    pub fn guest_policy(&self) -> Option<&crate::guests::Policy> {
        self.guests.as_ref()
    }

    /// Turns on probe v2.
    #[must_use]
    pub fn probe_v2(mut self, v2: bool) -> Self {
        self.v2 = v2;
        self
    }

    /// Turns on `evidence.environment` with `params`, or off with `None`.
    /// It runs with the probe battery, so it needs probe mode.
    #[must_use]
    pub fn environment(mut self, params: Option<crate::environment::Params>) -> Self {
        self.environment = params;
        self
    }

    /// Turns on `evidence.data_profile` with `params`, or off with
    /// `None`. It runs in the probe stage, so it needs probe mode.
    #[must_use]
    pub fn data_profile(mut self, params: Option<crate::data_profile::Params>) -> Self {
        self.data_profile = params;
        self
    }

    /// The `evidence.data_profile` bounds, when it is on.
    #[must_use]
    pub fn data_profile_params(&self) -> Option<&crate::data_profile::Params> {
        self.data_profile.as_ref()
    }

    /// The `evidence.environment` template, when it is on.
    #[must_use]
    pub fn environment_params(&self) -> Option<&crate::environment::Params> {
        self.environment.as_ref()
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
        let issue = issue_state(&state.issue.title, &state.issue.body);
        let (jev_state, questions) = evidence::setup_request(&issue, &commands);
        let invocation = self.recorder.enter(
            Start::new("evidence.setup", evidence::setup_implementation())
                .named("setup gate")
                .reading(&jev_state)
                .with_effects(),
        );
        let asked = self
            .ask_jev(client, "evidence.setup", "jev_setup", jev_state, questions)
            .await;
        if !asked.answered() {
            crate::say::say!(
                "  jev_setup ▸ Jev didn't answer, so no setup commands run: {}",
                asked.error.as_deref().unwrap_or("no answers")
            );
            self.recorder.end(
                &invocation,
                Finish::new(Outcome::Failed).summary(json!({ "error": asked.error })),
            );
            return;
        }
        let gated = evidence::setup_decide(&commands, |id| asked.noul(id));
        let scope = crate::ops::Scope::new(&self.workdir);
        let mut ran = Vec::new();
        let mut refused = Vec::new();
        for gate in &gated {
            let command = &gate.command;
            let p = gate.p.unwrap_or(0.0);
            if !gate.approved {
                crate::say::say!("  setup ▸ skipped `{command}` (Jev rated it {p:.2})");
                continue;
            }
            // The command becomes a typed operation or a refusal; its text
            // never reaches a shell.
            let proposed = crate::ops::parse_setup(command, &self.workdir);
            let Some(operation) = proposed.operation else {
                let why = proposed.refused.unwrap_or_default();
                crate::say::say!("  setup ▸ won't run `{command}`: {why}");
                refused.push(json!({ "command": command, "reason": why }));
                continue;
            };
            let Some(limit) = self
                .deadline
                .grant("setup", std::time::Duration::from_secs(240))
            else {
                crate::say::say!("  setup ▸ skipped `{command}`: no time left before the deadline");
                continue;
            };
            let capture = self
                .operate(&operation, "evidence.setup", &scope, limit)
                .await;
            if let Some(refusal) = &capture.refused {
                crate::say::say!("  setup ▸ won't run `{command}`: {refusal}");
                refused.push(json!({ "command": command, "reason": refusal.to_string() }));
                continue;
            }
            let ending = capture
                .exit
                .map_or("no exit code".to_string(), |code| format!("exit {code}"));
            crate::say::say!(
                "  setup ▸ ran {} (Jev rated it {p:.2}): {ending} in {:.1} s",
                capture.label,
                capture.milliseconds as f64 / 1000.0
            );
            ran.push(capture.label.clone());
            state.survey.push(Surveyed {
                path: format!(
                    "$ {}   (setup the host already ran: {ending})",
                    capture.label
                ),
                relevance: p,
                edit: 0.0,
                content: clip_tail(capture.output.trim(), 2_000),
            });
            self.recorder.revise();
        }
        self.recorder.end(
            &invocation,
            Finish::new(Outcome::Completed)
                .output(json!({ "gated": gated, "ran": ran, "refused": refused })),
        );
    }

    /// `task.requirements`: Jev reads each span of the instruction, and
    /// the map it makes replaces the one by rule. When no request is
    /// answered the rule's map stays.
    async fn requirements(&mut self, client: &jev::Client, state: &State) {
        let params = crate::requirements::Params::default();
        let invocation = self.recorder.enter(
            Start::new(
                "task.requirements",
                crate::requirements::implementation(params, true),
            )
            .named("requirement map")
            .reading(&json!({ "title": state.issue.title, "body": state.issue.body })),
        );
        let (map, asked) = crate::requirements::extract_with(
            &state.issue.title,
            &state.issue.body,
            params,
            &JevMode::Live(client.clone()),
            &self.recorder,
            Some(self.deadline.clone()),
        )
        .await;
        for one in &asked {
            self.count(one);
        }
        let answered = asked.iter().any(Asked::answered);
        crate::say::say!(
            "  requirements ▸ found {} requirements ({} unclear) in {} parts of the request, covering {:.0}% of it{}",
            map.requirements.len(),
            map.requirements
                .iter()
                .filter(|r| r.binding == crate::requirements::Binding::Uncertain)
                .count(),
            map.coverage.spans,
            map.coverage.fraction * 100.0,
            if answered {
                ""
            } else {
                "; Jev didn't answer, so a fixed rule split the request"
            }
        );
        if answered {
            self.criteria = map.criteria(MAX_CRITERIA);
            self.evidence.criteria = self.criteria.iter().map(|c| (c.clone(), None)).collect();
            self.requirements = map;
        }
        self.recorder.end(
            &invocation,
            Finish::new(if answered {
                Outcome::Completed
            } else {
                Outcome::Failed
            })
            .output(json!({
                "method": self.requirements.method,
                "requirements": self.requirements.requirements.len(),
                "coverage": self.requirements.coverage,
            })),
        );
        self.recorder.revise();
    }

    /// How this judge reaches Jev, for a component that asks on its own.
    #[must_use]
    pub fn jev_mode(&self) -> JevMode {
        self.client.clone().map_or(JevMode::Off, JevMode::Live)
    }

    /// The episode deadline every request is bounded by.
    #[must_use]
    pub fn episode_deadline(&self) -> Deadline {
        self.deadline.clone()
    }

    /// One Jev request through the shared component call, with the judge's
    /// counters kept.
    async fn ask_jev(
        &mut self,
        client: &jev::Client,
        component: &str,
        name: &str,
        jev_state: serde_json::Value,
        questions: Questions,
    ) -> Asked {
        let id = format!("{name}-{}", self.calls + self.failed + self.skipped + 1);
        let asked = component::jev::ask(
            &JevMode::Live(client.clone()),
            &self.recorder,
            Ask {
                component,
                name,
                id,
                state: jev_state,
                questions,
                parent: None,
                deadline: Some(self.deadline.clone()),
            },
        )
        .await;
        self.count(&asked);
        asked
    }

    fn count(&mut self, asked: &Asked) {
        if asked.answered() {
            self.calls += 1;
            self.input_tokens += asked.input_tokens.unwrap_or(0);
        } else if asked.how == "failed" {
            self.failed += 1;
        } else if asked.how == "skipped" {
            self.skipped += 1;
        }
    }
    /// Turns on probe mode.
    #[must_use]
    pub fn probing(mut self, probes: bool) -> Self {
        self.probes = probes;
        self
    }

    /// Runs one typed host operation as its own invocation, declaring its
    /// effect class before it runs, and records its output as a step.
    async fn operate(
        &mut self,
        operation: &crate::ops::Operation,
        component: &str,
        scope: &crate::ops::Scope,
        limit: std::time::Duration,
    ) -> crate::ops::Capture {
        let label = operation.label();
        let invocation = self.recorder.begin(
            Start::new(component, crate::ops::implementation())
                .named(&label)
                .reading(&json!(operation))
                .effect(operation.effects().class.word()),
        );
        let capture = crate::ops::run_within(&invocation, operation, scope, Some(limit)).await;
        self.finish_operation(&invocation, &capture);
        capture
    }

    /// Runs the evidence guests as one recorded invocation and returns the
    /// outputs that carry something, clipped like a probe's.
    async fn run_guests(&mut self, policy: &crate::guests::Policy) -> Vec<Probe> {
        let invocation = self.recorder.begin(
            Start::new("evidence.guests", crate::guests::implementation(policy))
                .named("evidence guests")
                .reading(&json!({ "policy": policy, "keywords": self.keywords }))
                .effect("observe"),
        );
        let Some(limit) = self
            .deadline
            .grant("guests", std::time::Duration::from_secs(policy.seconds))
        else {
            crate::say::say!("  guests ▸ skipped: no time left before the deadline");
            self.recorder.end(
                &invocation,
                Finish::new(Outcome::Skipped).summary(json!({ "reason": "episode deadline" })),
            );
            return Vec::new();
        };
        let runs = crate::guests::run(&self.workdir, &self.keywords, policy, limit).await;
        for run in &runs {
            crate::say::say!(
                "  guests ▸ {} {} in {}{}",
                run.step,
                run.status,
                crate::say::seconds(u128::from(run.elapsed_ms)),
                run.reason
                    .as_deref()
                    .map(|reason| format!(": {reason}"))
                    .unwrap_or_default()
            );
        }
        let probes: Vec<Probe> = runs
            .iter()
            .filter_map(|run| run.probe.clone())
            .map(|probe| Probe {
                output: clip(probe.output.trim(), PROBE_OUTPUT_CHARS),
                ..probe
            })
            .collect();
        let outcome = if runs.iter().any(|run| run.status == "ok") {
            Outcome::Completed
        } else {
            Outcome::Skipped
        };
        self.recorder.end(
            &invocation,
            Finish::new(outcome)
                .output(json!({ "runs": runs }))
                .cost(Cost::none()),
        );
        probes
    }

    /// Ends an operation's invocation with its record.
    fn finish_operation(&self, invocation: &str, capture: &crate::ops::Capture) {
        let outcome = if capture.refused.is_some() {
            Outcome::Skipped
        } else if capture.succeeded() {
            Outcome::Completed
        } else {
            Outcome::Failed
        };
        self.recorder.end(
            invocation,
            Finish::new(outcome)
                .output(capture.record())
                .cost(Cost::none()),
        );
    }

    /// The probe battery, as two components. The probe planner chooses
    /// typed, read-only operations from what the workspace and the task
    /// name, and they run in parallel under the plan's scope; no
    /// task-derived text reaches a shell. The capture selector then asks
    /// Jev which outputs carry information the task needs, and the chosen
    /// outputs join `state.survey`, so the briefing carries them.
    async fn probe(&mut self, client: &jev::Client, state: &mut State) {
        let started = Instant::now();
        let text = format!("{}\n{}", state.issue.title, state.issue.body);
        let mut facts = crate::probes::facts(&self.workdir, &text);
        if self.environment.is_some() {
            facts.implied = crate::environment::implied(&self.workdir, &text);
        }
        let params = crate::probes::PlanParams {
            v2: self.v2,
            shallow_listing: false,
            environment: self.environment.is_some(),
        };
        let planned = crate::probes::plan(&facts, params);
        let scope = crate::probes::scope(&facts, &self.workdir);

        // The plan is on disk before any operation runs.
        let planner = self.recorder.enter(
            Start::new(
                "evidence.probes.planner",
                crate::probes::implementation(params),
            )
            .named("probe plan")
            .reading(&json!(facts))
            .effect("observe"),
        );
        let Some(limit) = self
            .deadline
            .grant("probes", std::time::Duration::from_secs(10))
        else {
            crate::say::say!("  probe ▸ skipped: no time left before the deadline");
            self.recorder.end(
                &planner,
                Finish::new(Outcome::Skipped).summary(json!({ "reason": "episode deadline" })),
            );
            return;
        };
        let started_ops: Vec<(String, crate::ops::Operation)> = planned
            .iter()
            .map(|p| {
                let id = self.recorder.begin(
                    Start::new("host.operation", crate::ops::implementation())
                        .named(&p.operation.label())
                        .reading(&json!(p.operation))
                        .effect(p.operation.effects().class.word()),
                );
                (id, p.operation.clone())
            })
            .collect();
        let captures = crate::ops::run_all(&started_ops, &scope, Some(limit)).await;
        for capture in &captures {
            self.finish_operation(&capture.id, capture);
        }
        self.recorder.end(
            &planner,
            Finish::new(Outcome::Completed).output(json!({
                "planned": planned.iter().map(|p| json!({
                    "operation": p.operation.label(),
                    "effect": p.operation.effects().class.word(),
                    "reason": p.reason,
                })).collect::<Vec<_>>(),
                "refused": captures.iter().filter(|c| c.refused.is_some()).count(),
            })),
        );
        if let Some(environment) = self.environment.clone() {
            self.environment_line(&environment, &captures, state);
        }
        if let Some(params) = self.data_profile.clone() {
            self.data_profile_items(&params, state).await;
        }
        let mut outputs: Vec<Probe> = captures
            .iter()
            .filter(|capture| !matches!(capture.operation, crate::ops::Operation::Presence { .. }))
            .filter(|capture| capture.refused.is_none() && !capture.output.trim().is_empty())
            .map(|capture| Probe {
                command: capture.label.clone(),
                output: clip(capture.output.trim(), PROBE_OUTPUT_CHARS),
            })
            .collect();
        // Code runs the evidence guests when the manifest turns them on;
        // their outputs face the same keep question as a probe's.
        if let Some(policy) = self.guests.clone() {
            outputs.extend(self.run_guests(&policy).await);
        }
        let invocation = self.recorder.enter(
            Start::new("evidence.probes.selector", evidence::probe_implementation())
                .named("probe keep question")
                .reading(&json!(outputs)),
        );
        if outputs.is_empty() {
            self.recorder.end(
                &invocation,
                Finish::new(Outcome::Skipped).summary(json!({ "probes": 0 })),
            );
            return;
        }

        let issue = issue_state(&state.issue.title, &state.issue.body);
        let (jev_state, questions) = evidence::probe_request(&issue, &outputs);
        let asked = self
            .ask_jev(
                client,
                "evidence.probes.selector",
                "jev_probe",
                jev_state,
                questions,
            )
            .await;
        if !asked.answered() {
            crate::say::say!(
                "  probe ▸ Jev didn't answer, so no command output is kept: {}",
                asked.error.as_deref().unwrap_or("no answers")
            );
            self.recorder.end(
                &invocation,
                Finish::new(Outcome::Failed).summary(json!({ "error": asked.error })),
            );
            return;
        }
        let selected = evidence::probe_keep(&outputs, |id| asked.noul(id));
        let mut total = 0;
        for choice in selected.iter().filter(|s| s.decision == "kept") {
            let Some(probe) = outputs.iter().find(|p| p.command == choice.command) else {
                continue;
            };
            let p = choice.p.unwrap_or(0.0);
            total += choice.chars;
            crate::say::say!(
                "  probe ▸ kept the output of {} (Jev rated it {p:.2})",
                probe.command
            );
            state.survey.push(Surveyed {
                path: format!("$ {}", probe.command),
                relevance: p,
                edit: 0.0,
                content: probe.output.clone(),
            });
        }
        self.recorder.revise();
        crate::say::say!(
            "  probe ▸ ran {} commands and Jev rated {} outputs in {}; kept {} characters",
            captures.len(),
            outputs.len(),
            crate::say::seconds(started.elapsed().as_millis()),
            crate::say::count(total as u64)
        );
        self.recorder.end(
            &invocation,
            Finish::new(Outcome::Completed).output(json!({ "selected": selected })),
        );
    }
    /// `evidence.environment`: the presence captures become one line in
    /// `state.survey` under [`crate::environment::LABEL`], first, so every
    /// packer finds it. Jev isn't asked; presence is a fact.
    fn environment_line(
        &mut self,
        params: &crate::environment::Params,
        captures: &[crate::ops::Capture],
        state: &mut State,
    ) {
        let presence = crate::environment::presence(captures);
        let invocation = self.recorder.enter(
            Start::new(
                "evidence.environment",
                crate::environment::implementation(params),
            )
            .named("presence line")
            .reading(&json!(presence)),
        );
        if presence.is_empty() {
            self.recorder.end(
                &invocation,
                Finish::new(Outcome::Skipped).summary(json!({ "probes": 0 })),
            );
            return;
        }
        let line = crate::environment::line(&presence, params);
        crate::say::say!("  environment ▸ {line}");
        state.survey.retain(|s| s.path != crate::environment::LABEL);
        state.survey.insert(
            0,
            Surveyed {
                path: crate::environment::LABEL.to_string(),
                relevance: 1.0,
                edit: 0.0,
                content: line.clone(),
            },
        );
        self.recorder.revise();
        self.recorder.end(
            &invocation,
            Finish::new(Outcome::Completed)
                .output(json!({ "line": line, "presence": presence }))
                .cost(Cost::none()),
        );
    }

    /// `evidence.data_profile`: code profiles every data file in the
    /// workspace and puts each profile in `state.survey`, after the
    /// environment line, where the coverage packer ranks and trims it with
    /// the rest. Jev isn't asked; a profile is a fact.
    async fn data_profile_items(
        &mut self,
        params: &crate::data_profile::Params,
        state: &mut State,
    ) {
        let invocation = self.recorder.enter(
            Start::new(
                crate::data_profile::COMPONENT,
                crate::data_profile::implementation(params),
            )
            .named("data profile")
            .reading(&json!({ "workdir": self.workdir }))
            .effect("observe"),
        );
        let root = self.workdir.clone();
        let bounds = params.clone();
        let profile =
            tokio::task::spawn_blocking(move || crate::data_profile::profile(&root, &bounds))
                .await
                .unwrap_or_default();
        let items = profile.items(params);
        let at = usize::from(
            state
                .survey
                .first()
                .is_some_and(|s| s.path == crate::environment::LABEL),
        );
        state
            .survey
            .retain(|s| !s.path.starts_with(crate::data_profile::LABEL));
        for (offset, (label, text)) in items.into_iter().enumerate() {
            state.survey.insert(
                (at + offset).min(state.survey.len()),
                Surveyed {
                    path: label,
                    relevance: params.relevance,
                    edit: 0.0,
                    content: text,
                },
            );
        }
        crate::say::say!(
            "  data profile ▸ profiled {} data files in {}{}",
            profile.files.len(),
            crate::say::seconds(u128::from(profile.ms)),
            if profile.skipped.is_empty() {
                String::new()
            } else {
                format!("; {} not read", profile.skipped.len())
            }
        );
        self.recorder.revise();
        self.recorder.end(
            &invocation,
            Finish::new(if profile.files.is_empty() {
                Outcome::Skipped
            } else {
                Outcome::Completed
            })
            .output(profile.summary())
            .cost(Cost::none()),
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
        self.requirements(&client, state).await;
        if self.probes && self.v2 {
            self.setup(&client, state).await;
        }
        if self.probes {
            self.probe(&client, state).await;
        }
        let started = Instant::now();
        let pool = self.survey_pool(&state.issue);
        if pool.is_empty() {
            crate::say::say!("  survey ▸ found no files to rate");
            return;
        }
        let candidates: Vec<evidence::Candidate> = pool
            .iter()
            .map(|c| evidence::Candidate {
                path: c.path.clone(),
                excerpt: c.excerpt.clone(),
            })
            .collect();
        let issue = issue_state(&state.issue.title, &state.issue.body);
        let invocation = self.recorder.enter(
            Start::new("evidence.select", evidence::select_implementation())
                .named("survey")
                .reading(&json!({ "issue": issue, "candidates": candidates })),
        );
        let batches: Vec<&[evidence::Candidate]> = candidates.chunks(SURVEY_BATCH).collect();
        let mode = JevMode::Live(client.clone());
        let first = self.calls + self.failed + self.skipped + 1;
        let recorder = self.recorder.clone();
        let deadline = self.deadline.clone();
        let requests = batches.iter().zip(first..).map(|(batch, number)| {
            let (jev_state, questions) = evidence::survey_request(&issue, batch);
            component::jev::ask(
                &mode,
                &recorder,
                Ask {
                    component: "evidence.select",
                    name: "jev_survey",
                    id: format!("jev-survey-{number}"),
                    state: jev_state,
                    questions,
                    parent: Some(invocation.clone()),
                    deadline: Some(deadline.clone()),
                },
            )
        });
        let results = futures_util::future::join_all(requests).await;

        let mut scored: Vec<(String, Option<f64>, Option<f64>)> = Vec::new();
        for (asked, batch) in results.iter().zip(&batches) {
            self.count(asked);
            if !asked.answered() {
                continue;
            }
            for (i, candidate) in batch.iter().enumerate() {
                scored.push((
                    candidate.path.clone(),
                    Some(asked.noul(&format!("rel_{i}")).unwrap_or(0.0)),
                    Some(asked.noul(&format!("edit_{i}")).unwrap_or(0.0)),
                ));
            }
        }
        let ranked = evidence::survey_rank(scored);
        let judged = ranked.len();
        let mut total = 0;
        let mut read = Vec::new();
        for file in ranked.iter().filter(|file| file.selected) {
            let (rel, edit) = (file.relevance.unwrap_or(0.0), file.edit.unwrap_or(0.0));
            let Ok(text) = std::fs::read_to_string(self.workdir.join(&file.path)) else {
                continue;
            };
            // Probe v2 hands likely edit targets on whole, so the delegate
            // edits instead of reading first.
            let cap = if self.v2 && edit >= evidence::EDIT_TARGET {
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
            read.push(file.path.clone());
            state.survey.push(Surveyed {
                path: file.path.clone(),
                relevance: rel,
                edit,
                content,
            });
        }
        self.recorder.revise();
        crate::say::say!(
            "  survey ▸ Jev rated {} files in {} and put {} in the briefing ({} characters)",
            judged,
            crate::say::seconds(started.elapsed().as_millis()),
            state.survey.len(),
            crate::say::count(total as u64)
        );
        for file in &state.survey {
            crate::say::say!(
                "  survey ▸ {}: relevant {:.2}, needs an edit {:.2}",
                file.path,
                file.relevance,
                file.edit
            );
        }
        let outcome = if results.iter().any(Asked::answered) {
            Outcome::Completed
        } else {
            Outcome::Failed
        };
        self.recorder.end(
            &invocation,
            Finish::new(outcome).output(json!({ "ranked": ranked, "read": read })),
        );
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
            // Search source-sized files only. A repository can track hundreds
            // of megabytes of data (retained traces, fixtures), and a
            // keyword count over it took 91 s against 3 s without it, with
            // nothing on screen. A file past the cap is still a candidate
            // by name; its lines just aren't counted.
            crate::say::line(&format!(
                "  survey ▸ searching up to {} tracked files under {} KB for the request's keywords",
                tracked.len(),
                SEARCH_FILE_CAP / 1024
            ));
            for path in &tracked {
                let full = self.workdir.join(path);
                let small = std::fs::metadata(&full)
                    .is_ok_and(|meta| meta.is_file() && meta.len() <= SEARCH_FILE_CAP);
                if !small {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&full) else {
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
            crate::say::say!("  jev ▸ nothing to rate yet");
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
        let id = format!("jev-{}", self.calls + self.failed + self.skipped + 1);
        let Some(budget) = self.deadline.grant("jev_step", JEV_CALL_BUDGET) else {
            self.skipped += 1;
            let decision = Decision {
                id,
                name: "jev_step".to_string(),
                door: JEV_BASE_URL.to_string(),
                model: JEV_MODEL.to_string(),
                request: body,
                answers: serde_json::Value::Null,
                route: None,
                error: Some(DEADLINE_SKIP.to_string()),
                attempts: Vec::new(),
                review: None,
                milliseconds: 0,
            };
            self.recorder
                .push(Step::called(decision.call()).noting("jev_usage", charge_skipped()));
            return Err(DEADLINE_SKIP.to_string());
        };
        let request = request.retry(RetryPolicy {
            budget: Some(budget),
            ..RetryPolicy::default()
        });
        let started = Instant::now();
        let result = client.system_one(request).await;
        let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let mut decision = Decision {
            id,
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
                self.recorder.push(
                    Step::called(decision.call())
                        .taking(milliseconds)
                        .noting("jev_usage", charge_failed(&error)),
                );
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
        crate::say::say!(
            "  jev ▸ {} answers in {}, {} input tokens",
            response.answers.len(),
            crate::say::seconds(started.elapsed().as_millis()),
            crate::say::count(tokens)
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
            crate::say::say!("  jev ▸ {label}: {} ({p:.2})", candidate.path);
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
            crate::say::say!(
                "  jev ▸ the last command's outcome: {} ({:.2})",
                outcome.choice,
                outcome.confidence
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
            crate::say::say!("  jev ▸ part {k} of the last output matters ({p:.2})");
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
            crate::say::say!(
                "  jev ▸ chance the task is done and checked: {:.2}",
                ready.noul
            );
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
                .noting("jev_usage", charge_answered(&response)),
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
                crate::say::say!("  host ▸ no file has changed after {steps} steps");
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
        crate::say::say!("\n── step {} ──", state.history.len() + 1);
        let candidates = self.candidates(&state.issue);
        for candidate in &candidates {
            self.evidence
                .candidate(&candidate.path, candidate.hits, &candidate.excerpt);
        }
        let Some(client) = self.client.take() else {
            crate::say::say!(
                "  jev ▸ off, so {} files are ranked by keyword matches",
                candidates.len()
            );
            return self.deterministic(&candidates, None);
        };
        let result = self.ask(&client, state, &candidates).await;
        self.client = Some(client);
        match result {
            Ok(hints) => Judgments::Answered(hints),
            Err(error) => {
                crate::say::say!(
                    "  jev ▸ didn't answer, so files are ranked by keyword matches: {error}"
                );
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
    /// `exec.system`: Jev's answer for each optional system prompt section
    /// in `ids`, whether the task needs its guidance. Without Jev, every
    /// answer is unknown and nothing is selected.
    pub async fn select_sections(
        &mut self,
        state: &State,
        ids: &[String],
    ) -> Vec<(String, Option<f64>)> {
        let Some(client) = self.client.clone() else {
            return ids.iter().map(|id| (id.clone(), None)).collect();
        };
        let issue = issue_state(&state.issue.title, &state.issue.body);
        let (jev_state, questions) = crate::system::selection_request(&issue, ids);
        let asked = self
            .ask_jev(&client, "exec.system", "jev_system", jev_state, questions)
            .await;
        crate::system::selection_answers(ids, |id| asked.noul(id))
    }

    pub async fn close(&mut self, state: &State, delegate: &str, changes: &str) -> Close {
        let unanswered = |criteria: &[String], why: String| Close {
            done: None,
            criteria: criteria.iter().map(|c| (c.clone(), None)).collect(),
            unavailable: Some(why),
        };
        let Some(client) = self.client.clone() else {
            return unanswered(&self.criteria, "Jev is off".to_string());
        };
        let issue = issue_state(&state.issue.title, &state.issue.body);
        let (jev_state, questions) =
            evidence::close_request(&issue, &self.criteria, delegate, changes);
        let asked = self
            .ask_jev(&client, "verify.close", "jev_close", jev_state, questions)
            .await;
        if !asked.answered() {
            return unanswered(
                &self.criteria,
                asked.error.unwrap_or_else(|| "no answers".to_string()),
            );
        }
        let close = Close {
            done: asked.noul("done"),
            criteria: self
                .criteria
                .iter()
                .enumerate()
                .map(|(j, c)| (c.clone(), asked.noul(&format!("criterion_{j}"))))
                .collect(),
            unavailable: None,
        };
        self.evidence.criteria.clone_from(&close.criteria);
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

/// Runs Git read-only: no optional locks and no index refresh, so even
/// `git status` leaves the index alone, and with no prompt or pager.
pub(crate) fn git(workdir: &std::path::Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    command
        .args(crate::ops::READ_ONLY_GIT)
        .args(args)
        .current_dir(workdir);
    crate::ops::quiet_environment(&mut command);
    command
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
    fn long_output_keeps_its_first_and_last_chunks() {
        let output: String = (0..1_000).map(|n| format!("line {n}\n")).collect();
        let chunks = chunk(&output);
        assert_eq!(chunks.len(), MAX_CHUNKS);
        assert!(chunks[0].starts_with("line 0"));
        assert!(chunks[MAX_CHUNKS - 1].contains("line 999"));
    }
}
