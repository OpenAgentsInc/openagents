//! `verify.checks`: observations per requirement from admitted scenarios,
//! run against one named candidate.
//!
//! A format check can't tell a log parser that reads the severity field
//! from one that searches the whole line, a terminal that runs interactive
//! programs from one that runs builtins, or a runner that awaits cleanup
//! from one that returns early. Each of those is a scenario type here:
//!
//! | Scenario | Expected relation |
//! | --- | --- |
//! | `data.message-severity` | Changing only records' messages to carry another severity's word leaves every count unchanged. |
//! | `data.date-boundaries` | Each period counts exactly the files the public date rule puts in it, at each boundary. |
//! | `interactive.program` | An interactive program started through the submitted interface reads staged input. |
//! | `interactive.interrupt` | After control C interrupts a foreground command, the shell runs the next one. |
//! | `cancel.<how>.<size>` | After an interrupt below, at, or above the limit, every started task finishes its cleanup before the call returns. |
//! | `generic.output`, `generic.parse` | An output file a requirement asks for exists, isn't empty, and parses as its format. |
//! | `generic.public-command`, `generic.claimed-command` | A test the instruction names, or one the executor ran and saw pass, exits 0 on the final state. |
//! | `behavior.filter-removes`, `behavior.filter-preserves` | An in-place HTML filter defuses common script vectors and leaves clean documents unchanged. |
//! | `behavior.named-command` | A command the instruction names writes its outputs, and does so deterministically, without changing its inputs, when the instruction says so. |
//! | `behavior.reference-diff` | A program asked to behave exactly like a reference binary matches it on help and error paths. |
//! | `behavior.json-overlap` | A JSON report's selected position lies within the range the instruction says it overlaps. |
//!
//! The component runs in four recorded suboperations: **build** admits the
//! scenarios whose applicability conditions hold and that a requirement's
//! own words justify, **select** picks among them within a budget, **run**
//! executes each in a scratch copy of the candidate, and **record
//! coverage** turns verdicts into each requirement's state. A failed
//! scenario leaves a diagnostic packet.
//!
//! Every scenario parameter comes from the public instruction, the
//! observed inputs, or the host's own choice, recorded with how it was
//! derived. Protected verifier test names, counts, and fixture timings
//! never enter one.

pub mod behavior;
pub mod cancel;
pub mod cli;
pub mod data;
pub mod execution_audit;
pub mod generic;
pub mod html;
pub mod interactive;
pub mod labeled;
pub mod place;
pub mod public_program;
pub mod readiness;
pub mod recover;
pub mod replay;
pub mod report_audit;
pub mod review;
pub mod selfreport;
pub mod synthetic;
pub mod truth;
pub(crate) mod truth_micro;
pub(crate) mod truth_selected;
pub mod truthful;
pub mod verdict;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use crate::requirements::{Requirement, RequirementMap};

/// The schema of a checks report.
pub const SCHEMA: &str = "openagents.coder-one.checks.v1";

/// Where a run keeps its report, relative to its directory.
pub const COVERAGE_FILE: &str = "verification/checks.json";

/// The task as the checker reads it: its public words only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskText {
    #[serde(default)]
    pub title: String,
    pub instruction: String,
}

/// An inline program the candidate ran, such as a here-document fed to
/// `python3 -`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InlineProgram {
    pub interpreter: String,
    pub source: String,
}

/// The candidate revision a check runs against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// A short label: a mini-task run, a retained trial, a synthetic case.
    pub label: String,
    /// Where it came from: `workspace`, `stream`, or `synthetic`.
    pub origin: String,
    /// Files the candidate wrote, by the path the task names them with.
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    /// Programs it ran inline.
    #[serde(default)]
    pub programs: Vec<InlineProgram>,
    /// Files the task provided, such as an interface the candidate
    /// implements. Part of the input, not of the candidate.
    #[serde(default)]
    pub provided: BTreeMap<String, String>,
}

impl Candidate {
    /// The candidate's identity: the digest of what it wrote and ran.
    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!({ "files": self.files, "programs": self.programs }))
    }

    /// The file whose name ends with `name`, by base name.
    #[must_use]
    pub fn file_named(&self, name: &str) -> Option<(&String, &String)> {
        self.files.iter().find(|(path, _)| base_name(path) == name)
    }
}

/// A path's last component.
#[must_use]
pub fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// What the host observed of the task's inputs.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Observed {
    /// Sample records from the task's input files, verbatim.
    #[serde(default)]
    pub samples: Vec<String>,
    /// Where the samples came from.
    #[serde(default)]
    pub source: String,
}

/// How much checking a run may do.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub max_scenarios: usize,
    /// The most seconds the selected scenarios' bounds may add up to.
    pub seconds: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Budget {
            max_scenarios: 12,
            seconds: 180,
        }
    }
}

/// Everything a check reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub task: TaskText,
    /// The requirement map; the rule-only map of the instruction when
    /// absent.
    #[serde(default)]
    pub requirements: Option<RequirementMap>,
    pub candidate: Candidate,
    #[serde(default)]
    pub observed: Observed,
    #[serde(default)]
    pub budget: Budget,
    /// The live workspace the generic scenarios run in; `None` runs only
    /// the scratch-copy scenarios.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<generic::Workspace>,
    /// Scenario kinds whose failures the policy distrusts
    /// (`verify.distrust`): a failure of one reads as inconclusive, so it
    /// contradicts no requirement and leaves no packet.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distrust: Vec<String>,
}

/// Why a distrusted failure reads as inconclusive.
pub const DISTRUSTED: &str = "the policy distrusts this scenario kind's failures (`verify.distrust`): on the labeled trials they don't separate passes from failures, so this failure reads as inconclusive";

/// Reads each failed verdict of a kind in `distrust` as inconclusive,
/// noting why.
pub fn apply_distrust(scenarios: &[Scenario], verdicts: &mut [Verdict], distrust: &[String]) {
    if distrust.is_empty() {
        return;
    }
    for verdict in verdicts.iter_mut().filter(|v| v.verdict == "failed") {
        let kind = scenarios
            .iter()
            .find(|s| s.id == verdict.scenario)
            .map(|s| s.kind.as_str());
        if kind.is_some_and(|k| distrust.iter().any(|d| d == k)) {
            verdict.verdict = "inconclusive".to_string();
            verdict.coverage.push(DISTRUSTED.to_string());
        }
    }
}

/// A span of the instruction a scenario rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanRef {
    pub requirement: String,
    pub span: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// What a scenario may cost and change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub seconds: u64,
    pub processes: usize,
}

/// The relation a scenario expects, and where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub statement: String,
    pub derivation: String,
}

/// One admitted scenario.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    /// The scenario type.
    pub kind: String,
    /// The requirements it observes, the justifying one first.
    pub requirements: Vec<String>,
    pub spans: Vec<SpanRef>,
    /// The applicability conditions that held.
    pub applies: Vec<String>,
    /// The interface it drives the candidate through.
    pub interface: String,
    pub bounds: Bounds,
    pub effects: Vec<String>,
    /// The candidate's digest.
    pub candidate: String,
    /// The digest of the inputs the scenario generates or reads.
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    pub expected: Relation,
    /// Parameters, each derived from public text, observed input, or a
    /// host choice.
    pub params: Value,
}

/// A scenario type that didn't apply, and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ineligible {
    pub kind: String,
    pub why: String,
}

/// How a scenario ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub scenario: String,
    /// `passed`, `failed`, `unavailable`, or `inconclusive`.
    pub verdict: String,
    pub observations: Vec<Value>,
    /// What the verdict doesn't establish.
    pub coverage: Vec<String>,
    /// For a failure: the explanations the observations leave open.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hypotheses: Vec<String>,
    /// For a failure: the requirements the failed relation contradicts,
    /// when that is narrower than every requirement the scenario observes.
    /// Empty means the justifying requirement.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contradicts: Vec<String>,
}

impl Verdict {
    fn new(scenario: &str, verdict: &str) -> Self {
        Verdict {
            scenario: scenario.to_string(),
            verdict: verdict.to_string(),
            observations: Vec::new(),
            coverage: Vec::new(),
            hypotheses: Vec::new(),
            contradicts: Vec::new(),
        }
    }

    /// The verdict as it bears on one requirement the scenario observes: a
    /// failure contradicts only the requirements its relation names, and
    /// the scenario's other relations held for the rest.
    #[must_use]
    pub fn for_requirement(&self, scenario: &Scenario, requirement: &str) -> &str {
        if self.verdict != "failed" {
            return &self.verdict;
        }
        let named = if self.contradicts.is_empty() {
            scenario
                .requirements
                .first()
                .is_some_and(|r| r == requirement)
        } else {
            self.contradicts.iter().any(|r| r == requirement)
        };
        if named { "failed" } else { "passed" }
    }

    /// An unavailable verdict: the scenario couldn't run.
    #[must_use]
    pub fn unavailable(scenario: &str, why: &str) -> Self {
        let mut verdict = Verdict::new(scenario, "unavailable");
        verdict.coverage.push(why.to_string());
        verdict
    }
}

/// The diagnostic a failed scenario leaves for repair and handoff.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Packet {
    pub requirement: String,
    pub requirement_text: String,
    pub candidate: String,
    pub scenario: String,
    pub expected: Relation,
    pub observations: Vec<Value>,
    pub hypotheses: Vec<String>,
}

/// One requirement's coverage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Covered {
    pub id: String,
    pub text: String,
    pub kind: String,
    /// `observed`, `contradicted`, `unverifiable`, or `unobserved`.
    pub state: String,
    pub scenarios: Vec<Value>,
}

/// A whole check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub implementation: Implementation,
    pub candidate: Value,
    pub requirements_method: String,
    pub ineligible: Vec<Ineligible>,
    pub scenarios: Vec<Scenario>,
    pub selection: Value,
    pub verdicts: Vec<Verdict>,
    pub coverage: Vec<Covered>,
    pub packets: Vec<Packet>,
}

impl Report {
    /// Counts for a manifest or a list: scenarios by verdict and
    /// requirements by state.
    #[must_use]
    pub fn summary(&self) -> Value {
        let mut verdicts = BTreeMap::<String, usize>::new();
        for verdict in &self.verdicts {
            *verdicts.entry(verdict.verdict.clone()).or_default() += 1;
        }
        let mut states = BTreeMap::<String, usize>::new();
        for covered in &self.coverage {
            *states.entry(covered.state.clone()).or_default() += 1;
        }
        json!({
            "candidate": self.candidate["digest"],
            "scenarios": self.scenarios.len(),
            "verdicts": verdicts,
            "requirements": states,
            "packets": self.packets.len(),
        })
    }

    /// Whether any scenario failed.
    #[must_use]
    pub fn detected(&self) -> bool {
        self.verdicts.iter().any(|v| v.verdict == "failed")
    }
}

/// The implementation: the scenario catalog and its parameters.
#[must_use]
pub fn implementation() -> Implementation {
    Implementation::new(
        "verify.checks",
        "admitted scenario catalog, deterministic selector",
        &parameters(),
    )
}

fn parameters() -> Value {
    json!({
        "version": 1,
        "catalog": ["data.message-severity", "data.date-boundaries", "interactive.program", "interactive.interrupt", "cancel.signal", "cancel.internal", "generic.output", "generic.parse", "generic.public-command", "generic.claimed-command"],
        "selector": "one per requirement first, then by catalog order, within the budget",
        "cancel_limit": cancel::LIMIT,
        "cancel_sizes": cancel::SIZES,
    })
}

/// The implementation with the generic scenarios' options: the same as
/// [`implementation`] when every option is off.
#[must_use]
pub fn implementation_for(options: generic::Options) -> Implementation {
    if options.is_default() {
        return implementation();
    }
    let mut config = parameters();
    config["options"] = serde_json::to_value(options).unwrap_or(Value::Null);
    if options.self_report
        && let Some(catalog) = config["catalog"].as_array_mut()
    {
        catalog.push(json!("generic.self-report"));
    }
    if options.behavior
        && let Some(catalog) = config["catalog"].as_array_mut()
    {
        for kind in behavior::KINDS {
            catalog.push(json!(kind));
        }
    }
    Implementation::new(
        "verify.checks",
        "admitted scenario catalog, deterministic selector",
        &config,
    )
}

/// What a builder sees.
pub struct Context<'a> {
    pub task: &'a TaskText,
    pub map: &'a RequirementMap,
    pub candidate: &'a Candidate,
    pub observed: &'a Observed,
    /// The live workspace, for the generic scenarios.
    pub workspace: Option<&'a generic::Workspace>,
}

impl Context<'_> {
    /// The requirements whose text contains every word in one of `any`,
    /// case-insensitively: those matching the first word set first, each
    /// set's matches in map order.
    #[must_use]
    pub fn requirements_saying(&self, any: &[&[&str]]) -> Vec<&Requirement> {
        let mut found: Vec<&Requirement> = Vec::new();
        for words in any {
            for requirement in &self.map.requirements {
                let text = requirement.text.to_lowercase();
                if words.iter().all(|word| text.contains(word))
                    && !found.iter().any(|r| r.id == requirement.id)
                {
                    found.push(requirement);
                }
            }
        }
        found
    }

    /// The spans of `requirements`, with offsets.
    #[must_use]
    pub fn spans_of(&self, requirements: &[&Requirement]) -> Vec<SpanRef> {
        requirements
            .iter()
            .flat_map(|r| {
                r.spans.iter().filter_map(|id| {
                    self.map
                        .spans
                        .iter()
                        .find(|placed| &placed.span.id == id)
                        .map(|placed| SpanRef {
                            requirement: r.id.clone(),
                            span: id.clone(),
                            start: placed.span.start,
                            end: placed.span.end,
                            text: placed.span.text.clone(),
                        })
                })
            })
            .collect()
    }
}

/// Build: every admitted scenario, and the types that didn't apply.
#[must_use]
pub fn build(context: &Context<'_>) -> (Vec<Scenario>, Vec<Ineligible>) {
    let mut scenarios = Vec::new();
    let mut ineligible = Vec::new();
    for built in [
        data::build(context),
        interactive::build(context),
        cancel::build(context),
        generic::build(context),
        behavior::build(context),
    ] {
        match built {
            Ok(mut admitted) => scenarios.append(&mut admitted),
            Err(mut not) => ineligible.append(&mut not),
        }
    }
    (scenarios, ineligible)
}

/// Select: one scenario per justifying requirement first, then the rest
/// in catalog order, within the budget.
#[must_use]
pub fn select(scenarios: &[Scenario], budget: Budget) -> (Vec<Scenario>, Value) {
    let mut order: Vec<usize> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (i, scenario) in scenarios.iter().enumerate() {
        if seen.insert(scenario.requirements.first().cloned().unwrap_or_default()) {
            order.push(i);
        }
    }
    for i in 0..scenarios.len() {
        if !order.contains(&i) {
            order.push(i);
        }
    }
    let mut selected = Vec::new();
    let mut skipped = Vec::new();
    let mut seconds = 0;
    for i in order {
        let scenario = &scenarios[i];
        if selected.len() >= budget.max_scenarios {
            skipped.push(json!({ "scenario": scenario.id, "why": "the scenario budget is spent" }));
        } else if seconds + scenario.bounds.seconds > budget.seconds {
            skipped.push(json!({ "scenario": scenario.id, "why": "its bound exceeds the time left in the budget" }));
        } else {
            seconds += scenario.bounds.seconds;
            selected.push(scenario.clone());
        }
    }
    // Run in catalog order, whatever order admitted them.
    selected.sort_by_key(|s| scenarios.iter().position(|x| x.id == s.id));
    let record = json!({
        "selector": "deterministic",
        "admitted": scenarios.len(),
        "selected": selected.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        "skipped": skipped,
        "bound_seconds": seconds,
        "budget": budget,
    });
    (selected, record)
}

/// Run: one scenario in its own scratch directory.
pub async fn run_one(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    if let Err(error) = std::fs::create_dir_all(scratch) {
        return Verdict::unavailable(&scenario.id, &format!("cannot create scratch: {error}"));
    }
    match scenario.kind.as_str() {
        "data.message-severity" | "data.date-boundaries" => {
            data::run(context, scenario, scratch).await
        }
        "interactive.program" | "interactive.interrupt" => {
            interactive::run(context, scenario, scratch).await
        }
        "cancellation" => cancel::run(context, scenario, scratch).await,
        kind if generic::KINDS.contains(&kind) => generic::run(context, scenario, scratch).await,
        kind if behavior::KINDS.contains(&kind) => behavior::run(context, scenario, scratch).await,
        other => Verdict::unavailable(&scenario.id, &format!("no runner for {other}")),
    }
}

/// Record coverage: each requirement's state from the verdicts of the
/// scenarios that observe it, and a packet for each failure.
#[must_use]
pub fn coverage(
    map: &RequirementMap,
    candidate: &str,
    scenarios: &[Scenario],
    verdicts: &[Verdict],
) -> (Vec<Covered>, Vec<Packet>) {
    let mut covered = Vec::new();
    for requirement in &map.requirements {
        let mine: Vec<(&Scenario, &Verdict)> = scenarios
            .iter()
            .filter(|s| s.requirements.contains(&requirement.id))
            .filter_map(|s| verdicts.iter().find(|v| v.scenario == s.id).map(|v| (s, v)))
            .collect();
        let words: Vec<&str> = mine
            .iter()
            .map(|(s, v)| v.for_requirement(s, &requirement.id))
            .collect();
        let state = if words.contains(&"failed") {
            "contradicted"
        } else if words.contains(&"passed") {
            "observed"
        } else if words.is_empty() {
            "unobserved"
        } else {
            "unverifiable"
        };
        covered.push(Covered {
            id: requirement.id.clone(),
            text: requirement.text.clone(),
            kind: requirement.kind.word().to_string(),
            state: state.to_string(),
            scenarios: mine
                .iter()
                .map(|(s, v)| json!({ "id": s.id, "verdict": v.for_requirement(s, &requirement.id), "scenario_verdict": v.verdict, "coverage": v.coverage, "candidate": candidate }))
                .collect(),
        });
    }
    let packets = scenarios
        .iter()
        .filter_map(|s| {
            let v = verdicts.iter().find(|v| v.scenario == s.id)?;
            (v.verdict == "failed").then(|| {
                let id = v
                    .contradicts
                    .first()
                    .or_else(|| s.requirements.first())
                    .cloned()
                    .unwrap_or_default();
                Packet {
                    requirement_text: map
                        .requirements
                        .iter()
                        .find(|r| r.id == id)
                        .map(|r| r.text.clone())
                        .unwrap_or_default(),
                    requirement: id,
                    candidate: candidate.to_string(),
                    scenario: s.id.clone(),
                    expected: s.expected.clone(),
                    observations: v.observations.clone(),
                    hypotheses: v.hypotheses.clone(),
                }
            })
        })
        .collect();
    (covered, packets)
}

fn stage(recorder: &Recorder, name: &str, input: &Value) -> String {
    recorder.enter(
        Start::new(&format!("verify.checks.{name}"), implementation())
            .named(name)
            .reading(input),
    )
}

/// Runs a whole check under `recorder`: build, select, run, and record
/// coverage, each its own invocation under one `verify.checks`.
/// Scenarios run in scratch directories under `scratch`, which the check
/// removes afterward.
pub async fn check(input: &Input, recorder: &Recorder, scratch: &Path) -> Report {
    let map = input
        .requirements
        .clone()
        .unwrap_or_else(|| crate::requirements::mechanical(&input.task.instruction));
    let candidate_digest = input.candidate.digest();
    let parent = recorder.enter(
        Start::new("verify.checks", implementation())
            .named(&input.candidate.label)
            .reading(&serde_json::to_value(input).unwrap_or(Value::Null))
            .with_effects(),
    );
    let context = Context {
        task: &input.task,
        map: &map,
        candidate: &input.candidate,
        observed: &input.observed,
        workspace: input.workspace.as_ref(),
    };

    let id = stage(recorder, "build", &json!({ "candidate": candidate_digest }));
    let (scenarios, ineligible) = build(&context);
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(json!({ "admitted": scenarios.iter().map(|s| &s.id).collect::<Vec<_>>(), "ineligible": ineligible }))
            .cost(crate::record::Cost::none()),
    );

    let id = stage(recorder, "select", &json!({ "admitted": scenarios.len() }));
    let (selected, selection) = select(&scenarios, input.budget);
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(selection.clone())
            .cost(crate::record::Cost::none()),
    );

    let mut verdicts = Vec::new();
    for scenario in &selected {
        let id = recorder.enter(
            Start::new("verify.checks.run", implementation())
                .named(&scenario.id)
                .reading(&serde_json::to_value(scenario).unwrap_or(Value::Null))
                .with_effects(),
        );
        let started = Instant::now();
        let dir = scratch.join(scenario.id.replace(['/', ':'], "-"));
        let verdict = run_one(&context, scenario, &dir).await;
        let _ = std::fs::remove_dir_all(&dir);
        recorder.end(
            &id,
            Finish::new(match verdict.verdict.as_str() {
                "passed" => Outcome::Completed,
                "failed" => Outcome::Failed,
                _ => Outcome::Skipped,
            })
            .output(json!({
                "verdict": verdict.verdict,
                "observations": verdict.observations.len(),
                "coverage": verdict.coverage,
                "milliseconds": u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }))
            .cost(crate::record::Cost::none()),
        );
        verdicts.push(verdict);
    }

    apply_distrust(&selected, &mut verdicts, &input.distrust);
    let id = stage(recorder, "coverage", &json!({ "verdicts": verdicts.len() }));
    let (covered, packets) = coverage(&map, &candidate_digest, &selected, &verdicts);
    recorder.end(
        &id,
        Finish::new(Outcome::Completed)
            .output(json!({ "requirements": covered.iter().map(|c| json!({ "id": c.id, "state": c.state })).collect::<Vec<_>>(), "packets": packets.len() }))
            .cost(crate::record::Cost::none()),
    );
    let report = Report {
        schema: SCHEMA.to_string(),
        implementation: implementation_for(
            input
                .workspace
                .as_ref()
                .map(|w| w.options)
                .unwrap_or_default(),
        ),
        candidate: json!({
            "label": input.candidate.label,
            "origin": input.candidate.origin,
            "digest": candidate_digest,
            "files": input.candidate.files.keys().collect::<Vec<_>>(),
            "programs": input.candidate.programs.len(),
        }),
        requirements_method: map.method.clone(),
        ineligible,
        scenarios: selected,
        selection,
        verdicts,
        coverage: covered,
        packets,
    };
    recorder.end(
        &parent,
        Finish::new(if report.detected() {
            Outcome::Failed
        } else {
            Outcome::Completed
        })
        .output(report.summary())
        .cost(crate::record::Cost::none()),
    );
    let _ = std::fs::remove_dir_all(scratch);
    report
}

/// The candidate a workspace holds: every regular file under it, up to
/// 200 files of 256 KiB, without `.git`, as paths relative to it.
#[must_use]
pub fn workspace_candidate(label: &str, workdir: &Path) -> Candidate {
    let mut files = BTreeMap::new();
    let mut stack = vec![workdir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.file_name().is_some_and(|n| {
                [
                    ".git",
                    "__pycache__",
                    "node_modules",
                    ".venv",
                    ".pytest_cache",
                ]
                .iter()
                .any(|skip| n == *skip)
            }) {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if files.len() < 200
                && std::fs::metadata(&path).is_ok_and(|m| m.len() <= 256 * 1024)
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                let relative = path
                    .strip_prefix(workdir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .into_owned();
                files.insert(relative, text);
            }
        }
    }
    Candidate {
        label: label.to_string(),
        origin: "workspace".to_string(),
        files,
        programs: Vec::new(),
        provided: BTreeMap::new(),
    }
}

/// Records whose lines look like data: from the files a candidate reads
/// under `dir`, the first 20 lines of up to five files.
#[must_use]
pub fn sample_records(dir: &Path) -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    paths
        .iter()
        .take(5)
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|text| {
            text.lines()
                .take(20)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// What a workspace check reads besides the workspace's files: the task,
/// what the task provided, and, for a live check, the executor's commands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subject {
    pub label: String,
    pub task: TaskText,
    /// The requirement map; the rule-only map when `None`.
    pub requirements: Option<RequirementMap>,
    /// Files the task provided, relative to the workspace: input, not
    /// candidate.
    pub provided: Vec<String>,
    /// A directory of input records, relative to the workspace, observed
    /// rather than counted as candidate.
    pub inputs: Option<String>,
    pub budget: Budget,
    /// The generic scenarios' live workspace; its `dir` is set to the
    /// checked workspace. `None` runs only the scratch-copy scenarios.
    pub live: Option<generic::Workspace>,
    /// Scenario kinds whose failures read as inconclusive
    /// (`verify.distrust`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distrust: Vec<String>,
}

impl Subject {
    /// A mini-task's subject: its instruction, `base_terminal.py` as a
    /// provided file, and `logs/` as input records.
    #[must_use]
    pub fn mini(task: &crate::minitask::MiniTask) -> Self {
        let label = format!("mini-task {}", task.id);
        Subject {
            task: TaskText {
                title: label.clone(),
                instruction: task.instruction.to_string(),
            },
            label,
            requirements: None,
            provided: vec!["base_terminal.py".to_string()],
            inputs: Some("logs".to_string()),
            budget: Budget::default(),
            live: None,
            distrust: Vec::new(),
        }
    }

    /// The check input for `workdir`: the candidate is what it holds, less
    /// what the task provided and its input records.
    #[must_use]
    pub fn input(&self, workdir: &Path) -> Input {
        let mut candidate = workspace_candidate(&self.label, workdir);
        let mut observed = Observed::default();
        for provided in &self.provided {
            if let Some(text) = candidate.files.remove(provided) {
                candidate.provided.insert(provided.clone(), text);
            }
        }
        if let Some(inputs) = &self.inputs {
            let prefix = format!("{}/", inputs.trim_end_matches('/'));
            let records: Vec<String> = candidate
                .files
                .keys()
                .filter(|path| path.starts_with(&prefix))
                .cloned()
                .collect();
            if !records.is_empty() {
                observed.samples = sample_records(&workdir.join(inputs));
                observed.source = format!("the first lines of the files under {prefix}");
                for path in records {
                    candidate.files.remove(&path);
                }
            }
        }
        Input {
            task: self.task.clone(),
            requirements: self.requirements.clone(),
            candidate,
            observed,
            budget: self.budget,
            workspace: self.live.clone().map(|mut live| {
                live.dir = workdir.to_string_lossy().into_owned();
                live
            }),
            distrust: self.distrust.clone(),
        }
    }
}

/// The check input for a mini-task's workspace: the candidate is what the
/// workspace holds, less what the task provided and its log inputs.
#[must_use]
pub fn workspace_input(task: &crate::minitask::MiniTask, workdir: &Path) -> Input {
    Subject::mini(task).input(workdir)
}

/// Checks a mini-task's workspace after its episode, writes the report to
/// `<dir>/verification/checks.json`, and returns it.
pub async fn check_workspace(
    task: &crate::minitask::MiniTask,
    workdir: &Path,
    dir: &Path,
    recorder: &Recorder,
) -> Report {
    check_workspace_as(task, workdir, dir, recorder, COVERAGE_FILE)
        .await
        .1
}

/// Checks a mini-task's workspace and writes the report to `<dir>/<file>`;
/// returns the input and the report.
pub async fn check_workspace_as(
    task: &crate::minitask::MiniTask,
    workdir: &Path,
    dir: &Path,
    recorder: &Recorder,
    file: &str,
) -> (Input, Report) {
    check_subject_as(&Subject::mini(task), workdir, dir, recorder, file).await
}

/// Checks `subject` against `workdir` and writes the report to
/// `<dir>/<file>`; returns the input and the report.
pub async fn check_subject_as(
    subject: &Subject,
    workdir: &Path,
    dir: &Path,
    recorder: &Recorder,
    file: &str,
) -> (Input, Report) {
    let input = subject.input(workdir);
    let report = check(&input, recorder, &dir.join("checks-scratch")).await;
    if let Ok(text) = serde_json::to_string_pretty(&report) {
        let _ = crate::record::write_atomic(&dir.join(file), text.as_bytes());
    }
    (input, report)
}

/// The absolute executable paths a candidate's text names, such as
/// `/bin/bash`, that this host lacks.
#[must_use]
pub fn missing_executables(candidate: &Candidate) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let texts = candidate
        .files
        .values()
        .chain(candidate.programs.iter().map(|p| &p.source));
    for text in texts {
        for quote in ['"', '\''] {
            for piece in text.split(quote).skip(1).step_by(2) {
                let path = piece.split_whitespace().next().unwrap_or_default();
                let executable = ["/bin/", "/usr/bin/", "/usr/local/bin/", "/sbin/"]
                    .iter()
                    .any(|dir| {
                        path.starts_with(dir)
                            && path.len() > dir.len()
                            && !path[dir.len()..].contains('/')
                    });
                if executable && !Path::new(path).exists() && !found.iter().any(|f| f == path) {
                    found.push(path.to_string());
                }
            }
        }
    }
    found.sort();
    found
}

/// A command for `program` that sees the executables the candidate
/// names: the plain command when the host has them all, or one run
/// through `bwrap` in a mount namespace that puts the host's own binary of
/// the same name at each missing path. The note says what was provided.
///
/// # Errors
///
/// Returns why the candidate can't run here: an executable with no host
/// equivalent, or no `bwrap` to provide one.
pub fn host_command(
    candidate: &Candidate,
    program: &Path,
) -> Result<(std::process::Command, Option<String>), String> {
    let missing = missing_executables(candidate);
    if missing.is_empty() {
        return Ok((std::process::Command::new(program), None));
    }
    let mut provided = Vec::new();
    for path in &missing {
        let host = crate::minitask::process::which(base_name(path))
            .and_then(|p| std::fs::canonicalize(p).ok())
            .ok_or_else(|| format!("the candidate runs {path}, which this host lacks"))?;
        provided.push((path.clone(), host));
    }
    let bwrap = crate::minitask::process::which("bwrap").ok_or_else(|| {
        format!(
            "the candidate runs {}, which this host lacks at that path, and there is no bwrap to provide it",
            missing.join(", ")
        )
    })?;
    let mut command = std::process::Command::new(bwrap);
    command.args(["--dev-bind", "/", "/"]);
    let mut dirs: Vec<String> = provided
        .iter()
        .map(|(path, _)| path[..path.rfind('/').unwrap_or(0)].to_string())
        .collect();
    dirs.sort();
    dirs.dedup();
    for dir in &dirs {
        // A fresh directory holds the host's own entries and the missing
        // ones, so nothing is written to the host.
        command.args(["--tmpfs", dir]);
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if let Ok(real) = std::fs::canonicalize(entry.path()) {
                command.arg("--ro-bind").arg(real).arg(entry.path());
            }
        }
        for (path, host) in provided
            .iter()
            .filter(|(p, _)| p.starts_with(&format!("{dir}/")))
        {
            command.arg("--ro-bind").arg(host).arg(path);
        }
    }
    command.arg(program);
    Ok((
        command,
        Some(format!(
            "The candidate names {}, which this host lacks; the check provided the host's own binary at that path in a mount namespace, and doesn't otherwise reproduce the task's environment.",
            missing.join(", ")
        )),
    ))
}

/// Days since 1970-01-01 of a civil date.
#[must_use]
pub fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day number.
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `YYYY-MM-DD` of a day number.
#[must_use]
pub fn iso_day(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The day number of a `YYYY-MM-DD` date.
#[must_use]
pub fn parse_day(text: &str) -> Option<i64> {
    let mut parts = text.split('-');
    let y = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    let d = parts.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

#[cfg(test)]
mod tests;
