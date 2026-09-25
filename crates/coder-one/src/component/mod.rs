//! Components run alone, on fixtures, in seconds.
//!
//! A component is one step of an episode with a fixed meaning: the setup
//! gate, the probe keep question, the survey's relevance and edit
//! questions, the briefing packer, and the closing check today. Each one
//! here is a function from a serializable input to an output, and every run
//! writes the same invocation records an episode writes, so isolated runs
//! and episodes land in the same Gym views.
//!
//! A fixture directory holds one `<component>.json` per component input
//! and a `jev-recorded.json` of recorded answers; [`extract`] makes one
//! from a retained attempt. A later component plugs in by implementing
//! [`Component`] and adding itself to [`registry`].
//!
//! ```text
//! coder-one component run evidence.pack --fixture DIR [--jev recorded] [--json]
//! coder-one component suite evidence.probes [--fixtures DIR] [--jev recorded]
//! ```

pub mod checks;
pub mod cli;
pub mod departures;
pub mod evidence;
pub mod extract;
pub mod finish;
pub mod jev;
pub mod monitor;
pub mod pack;
pub mod repair;
pub mod replay;
pub mod scripted;
pub mod stall;
pub mod support;

use std::path::{Path, PathBuf};
use std::time::Instant;

use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::delegate::{Briefing, BriefingInputs};
use crate::record::{Finish, Implementation, Outcome, Recorder, Start};
use evidence::{Candidate, Probe, Task};
use jev::{Ask, Asked, JevMode, Recorded};

/// The schema of a component fixture file.
pub const FIXTURE_SCHEMA: &str = "openagents.coder-one.component-fixture.v1";

/// The schema of a component run's result.
pub const RESULT_SCHEMA: &str = "openagents.coder-one.component-run.v1";

/// The file of recorded Jev answers in a fixture directory.
pub const RECORDED_FILE: &str = "jev-recorded.json";

/// The session a component run's log opens with.
const SUITE_DOOR: &str = "component-runner";

/// One component input, with where it came from and what the retained run
/// produced from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fixture {
    pub schema: String,
    pub component: String,
    /// Where the input came from: a retained trace, a synthetic case.
    pub source: Value,
    pub input: Value,
    /// What the retained run produced, for comparison; null when unknown.
    #[serde(default)]
    pub retained: Value,
}

impl Fixture {
    /// Reads `<dir>/<component>.json`.
    ///
    /// # Errors
    ///
    /// Returns a message when the file is missing or doesn't read.
    pub fn load(dir: &Path, component: &str) -> Result<Self, String> {
        let path = dir.join(format!("{component}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let fixture: Fixture = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a fixture: {error}", path.display()))?;
        if fixture.schema != FIXTURE_SCHEMA || fixture.component != component {
            return Err(format!(
                "{} is not a {FIXTURE_SCHEMA} fixture for {component}",
                path.display()
            ));
        }
        Ok(fixture)
    }

    /// Writes the fixture as `<dir>/<component>.json`.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        crate::record::write_atomic(
            &dir.join(format!("{}.json", self.component)),
            format!("{text}\n").as_bytes(),
        )
    }
}

/// What one run produced.
#[derive(Clone, Debug, PartialEq)]
pub struct Ran {
    pub output: Value,
    /// Named numbers and flags, comparable across runs.
    pub metrics: Map<String, Value>,
}

/// A component the runner can call.
pub trait Component {
    /// The component ID, such as `evidence.pack`.
    fn id(&self) -> &'static str;
    /// The implementation this build runs, with its parameter digest.
    fn implementation(&self) -> Implementation;
    /// One short sentence on what the component decides.
    fn about(&self) -> &'static str;
    /// Runs the component on a fixture's input.
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>>;
}

/// Every component the runner knows, in episode order.
#[must_use]
pub fn registry() -> Vec<Box<dyn Component>> {
    vec![
        Box::new(TaskProfile),
        Box::new(Requirements),
        Box::new(SetupGate),
        Box::new(Planner),
        Box::new(ProbeKeep),
        Box::new(Select),
        Box::new(Pack),
        Box::new(departures::Departures),
        Box::new(scripted::ScriptedAdapter),
        Box::new(monitor::MonitorComponent),
        Box::new(stall::StallComponent),
        Box::new(stall::NextComponent),
        Box::new(finish::FinishComponent),
        Box::new(HandoffComponent),
        Box::new(SystemSelect),
        Box::new(checks::Checks),
        Box::new(crate::checks::truthful::Truthful),
        Box::new(support::Support),
        Box::new(repair::Repair),
        Box::new(Close),
        Box::new(scripted::MiniTaskRun),
    ]
}

/// The component with `id`.
///
/// # Errors
///
/// Returns a message naming the known components when none matches.
pub fn find(id: &str) -> Result<Box<dyn Component>, String> {
    registry()
        .into_iter()
        .find(|component| component.id() == id)
        .ok_or_else(|| {
            format!(
                "no component {id}; known: {}",
                registry()
                    .iter()
                    .map(|c| c.id())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

fn input<T: for<'de> Deserialize<'de>>(fixture: &Fixture) -> Result<T, String> {
    serde_json::from_value(fixture.input.clone())
        .map_err(|error| format!("the {} input doesn't read: {error}", fixture.component))
}

/// Asks through `jev` under the component's invocation.
async fn ask_one(
    jev: &JevMode,
    recorder: &Recorder,
    component: &str,
    name: &str,
    (state, questions): (Value, ::jev::Questions),
) -> Asked {
    jev::ask(
        jev,
        recorder,
        Ask {
            component,
            name,
            id: format!("{name}-1"),
            state,
            questions,
            parent: None,
            // An isolated run has no episode deadline.
            deadline: None,
        },
    )
    .await
}

fn same_set(a: &[String], b: Option<&Value>) -> Value {
    let Some(b) = b.and_then(Value::as_array) else {
        return Value::Null;
    };
    let mut a: Vec<&str> = a.iter().map(String::as_str).collect();
    let mut b: Vec<&str> = b.iter().filter_map(Value::as_str).collect();
    a.sort_unstable();
    b.sort_unstable();
    json!(a == b)
}

/// `task.requirements`: the requirement map from the task's own words,
/// scored against hand labels when the fixture holds them.
struct Requirements;

#[derive(Deserialize)]
struct RequirementsInput {
    task: Task,
    #[serde(default)]
    params: Option<crate::requirements::Params>,
}

impl Component for Requirements {
    fn id(&self) -> &'static str {
        "task.requirements"
    }
    fn implementation(&self) -> Implementation {
        crate::requirements::implementation(crate::requirements::Params::default(), true)
    }
    fn about(&self) -> &'static str {
        "Jev reads each span of the task as a deliverable, behavior, constraint, check, or context."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: RequirementsInput = input(fixture)?;
            let params = input.params.unwrap_or_default();
            let (map, _) = crate::requirements::extract_with(
                &input.task.title,
                &input.task.body,
                params,
                jev,
                recorder,
                None,
            )
            .await;
            let mut metrics = Map::new();
            metrics.insert("spans".to_string(), json!(map.coverage.spans));
            metrics.insert("requirements".to_string(), json!(map.requirements.len()));
            metrics.insert(
                "uncertain".to_string(),
                json!(
                    map.requirements
                        .iter()
                        .filter(|r| r.binding == crate::requirements::Binding::Uncertain)
                        .count()
                ),
            );
            metrics.insert("coverage".to_string(), json!(map.coverage.fraction));
            metrics.insert(
                "unanswered_spans".to_string(),
                json!(map.coverage.unanswered_spans),
            );
            let mut score = Value::Null;
            if let Some(labels) = fixture.retained.get("labels") {
                let labels: Vec<crate::requirements::Label> =
                    serde_json::from_value(labels.clone())
                        .map_err(|error| format!("the labels don't read: {error}"))?;
                let scored = crate::requirements::score(&map, &labels);
                metrics.insert("labels".to_string(), json!(labels.len()));
                metrics.insert("recall".to_string(), json!(scored.recall));
                metrics.insert("precision".to_string(), json!(scored.precision));
                metrics.insert("recall_binding".to_string(), json!(scored.recall_binding));
                metrics.insert(
                    "precision_binding".to_string(),
                    json!(scored.precision_binding),
                );
                metrics.insert("kind_agreement".to_string(), json!(scored.kind_agreement));
                score = json!(scored);
            }
            Ok(Ran {
                output: json!({ "map": map.record(), "score": score }),
                metrics,
            })
        })
    }
}

/// `evidence.setup`: which setup commands the task needs run first. The
/// isolated run judges only; it never runs a command.
struct SetupGate;

#[derive(Deserialize)]
struct SetupInput {
    task: Task,
    commands: Vec<String>,
}

impl Component for SetupGate {
    fn id(&self) -> &'static str {
        "evidence.setup"
    }
    fn implementation(&self) -> Implementation {
        evidence::setup_implementation()
    }
    fn about(&self) -> &'static str {
        "Jev gates each setup command the task names."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: SetupInput = input(fixture)?;
            let request = evidence::setup_request(&input.task.state(), &input.commands);
            let asked = ask_one(jev, recorder, self.id(), "jev_setup", request).await;
            let gated = evidence::setup_decide(&input.commands, |id| asked.noul(id));
            let approved: Vec<String> = gated
                .iter()
                .filter(|g| g.approved)
                .map(|g| g.command.clone())
                .collect();
            let mut metrics = Map::new();
            metrics.insert("commands".to_string(), json!(gated.len()));
            metrics.insert("approved".to_string(), json!(approved.len()));
            metrics.insert(
                "unknown".to_string(),
                json!(gated.iter().filter(|g| g.p.is_none()).count()),
            );
            metrics.insert(
                "matches_retained".to_string(),
                same_set(&approved, fixture.retained.get("approved")),
            );
            Ok(Ran {
                output: json!({ "gated": gated }),
                metrics,
            })
        })
    }
}

/// `evidence.probes.planner`: which typed, read-only operations the host
/// runs. The isolated run plans only; it never runs an operation.
struct Planner;

impl Component for Planner {
    fn id(&self) -> &'static str {
        "evidence.probes.planner"
    }
    fn implementation(&self) -> Implementation {
        crate::probes::implementation(crate::probes::PlanParams::default())
    }
    fn about(&self) -> &'static str {
        "Code plans the typed, read-only operations the host runs before the work."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: crate::probes::PlanInput = input(fixture)?;
            let planned = crate::probes::plan(&input.facts, input.params);
            let labels: Vec<String> = planned.iter().map(|p| p.operation.label()).collect();
            let mut metrics = Map::new();
            metrics.insert("operations".to_string(), json!(planned.len()));
            metrics.insert(
                "not_observe".to_string(),
                json!(
                    planned
                        .iter()
                        .filter(|p| p.operation.effects().class != crate::ops::EffectClass::Observe)
                        .count()
                ),
            );
            // The retained battery's commands, each covered by an
            // equivalent operation or dropped on purpose.
            let mut uncovered = Vec::new();
            if let Some(commands) = fixture.retained.get("commands").and_then(Value::as_array) {
                let mut covered = 0;
                let mut subsumed = 0;
                let mut missing = Vec::new();
                for command in commands.iter().filter_map(Value::as_str) {
                    match crate::probes::equivalent(command, &input.facts.workdir) {
                        None => subsumed += 1,
                        Some(wanted) => {
                            let found = wanted.iter().all(|want| {
                                labels.iter().any(|label| label.starts_with(want.as_str()))
                            });
                            if found {
                                covered += 1;
                            } else {
                                missing.push(command.to_string());
                            }
                        }
                    }
                }
                metrics.insert("retained_covered".to_string(), json!(covered));
                metrics.insert("retained_subsumed".to_string(), json!(subsumed));
                metrics.insert("matches_retained".to_string(), json!(missing.is_empty()));
                metrics.insert("retained_missing".to_string(), json!(missing.len()));
                uncovered = missing;
            }
            Ok(Ran {
                output: json!({
                    "planned": planned.iter().map(|p| json!({
                        "id": p.id,
                        "operation": p.operation,
                        "label": p.operation.label(),
                        "effect": p.operation.effects().class.word(),
                        "reason": p.reason,
                    })).collect::<Vec<_>>(),
                    "retained_missing": uncovered,
                }),
                metrics,
            })
        })
    }
}

/// `evidence.probes.selector`: which finished probe outputs reach the
/// briefing.
struct ProbeKeep;

#[derive(Deserialize)]
struct ProbeInput {
    task: Task,
    probes: Vec<Probe>,
}

impl Component for ProbeKeep {
    fn id(&self) -> &'static str {
        "evidence.probes.selector"
    }
    fn implementation(&self) -> Implementation {
        evidence::probe_implementation()
    }
    fn about(&self) -> &'static str {
        "Jev's keep question picks the probe outputs the briefing carries."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: ProbeInput = input(fixture)?;
            let request = evidence::probe_request(&input.task.state(), &input.probes);
            let asked = ask_one(jev, recorder, self.id(), "jev_probe", request).await;
            let selected = evidence::probe_keep(&input.probes, |id| asked.noul(id));
            let kept: Vec<String> = selected
                .iter()
                .filter(|s| s.decision == "kept")
                .map(|s| s.command.clone())
                .collect();
            let mut metrics = Map::new();
            metrics.insert("probes".to_string(), json!(selected.len()));
            metrics.insert("kept".to_string(), json!(kept.len()));
            metrics.insert(
                "kept_chars".to_string(),
                json!(
                    selected
                        .iter()
                        .filter(|s| s.decision == "kept")
                        .map(|s| s.chars)
                        .sum::<usize>()
                ),
            );
            metrics.insert(
                "unknown".to_string(),
                json!(selected.iter().filter(|s| s.p.is_none()).count()),
            );
            metrics.insert(
                "matches_retained".to_string(),
                same_set(&kept, fixture.retained.get("kept")),
            );
            Ok(Ran {
                output: json!({ "selected": selected }),
                metrics,
            })
        })
    }
}

/// `evidence.select`: which candidate files the survey reads into the
/// briefing, by Jev's relevance and edit answers.
struct Select;

#[derive(Deserialize)]
struct SelectInput {
    task: Task,
    candidates: Vec<Candidate>,
}

impl Component for Select {
    fn id(&self) -> &'static str {
        "evidence.select"
    }
    fn implementation(&self) -> Implementation {
        evidence::select_implementation()
    }
    fn about(&self) -> &'static str {
        "Jev judges each candidate file's relevance and whether it needs an edit."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: SelectInput = input(fixture)?;
            let issue = input.task.state();
            let mut scored = Vec::new();
            let mut answered = 0;
            for batch in input.candidates.chunks(evidence::SURVEY_BATCH) {
                let request = evidence::survey_request(&issue, batch);
                let asked = ask_one(jev, recorder, self.id(), "jev_survey", request).await;
                answered += usize::from(asked.answered());
                for (i, candidate) in batch.iter().enumerate() {
                    scored.push((
                        candidate.path.clone(),
                        asked.noul(&format!("rel_{i}")),
                        asked.noul(&format!("edit_{i}")),
                    ));
                }
            }
            let ranked = evidence::survey_rank(scored);
            let selected: Vec<String> = ranked
                .iter()
                .filter(|s| s.selected)
                .map(|s| s.path.clone())
                .collect();
            let mut metrics = Map::new();
            metrics.insert("candidates".to_string(), json!(ranked.len()));
            metrics.insert("requests_answered".to_string(), json!(answered));
            metrics.insert("selected".to_string(), json!(selected.len()));
            metrics.insert(
                "edit_targets".to_string(),
                json!(
                    ranked
                        .iter()
                        .filter(|s| s.edit.is_some_and(|p| p >= evidence::EDIT_TARGET))
                        .count()
                ),
            );
            metrics.insert(
                "unknown".to_string(),
                json!(ranked.iter().filter(|s| s.relevance.is_none()).count()),
            );
            // The retained run read only what fit, so the comparison is
            // recall: how many files it read the selection still holds.
            if let Some(read) = fixture.retained.get("read").and_then(Value::as_array) {
                let held = read
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|path| selected.iter().any(|s| s == path))
                    .count();
                metrics.insert(
                    "retained_recall".to_string(),
                    if read.is_empty() {
                        Value::Null
                    } else {
                        json!(pack::round(held as f64 / read.len() as f64))
                    },
                );
            }
            Ok(Ran {
                output: json!({ "ranked": ranked, "selected": selected }),
                metrics,
            })
        })
    }
}

/// `evidence.pack`: the briefing within its budget, every omission named.
struct Pack;

#[derive(Deserialize)]
struct PackInput {
    inputs: BriefingInputs,
    cap: usize,
    /// The same evidence with each surveyed item whole, for the coverage
    /// packer; `inputs` when absent.
    #[serde(default)]
    whole: Option<BriefingInputs>,
}

impl Component for Pack {
    fn id(&self) -> &'static str {
        "evidence.pack"
    }
    fn implementation(&self) -> Implementation {
        crate::pack::implementation(crate::pack::Params::default(), false)
    }
    fn about(&self) -> &'static str {
        "Code packs the evidence by requirement coverage into a briefing within a character budget."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: PackInput = input(fixture)?;
            // Before: the first packer, which must rebuild the retained
            // briefing.
            let briefing = Briefing::build(&input.inputs, input.cap);
            let mut metrics = pack::metrics(&input.inputs, &briefing);
            metrics.insert(
                "reproduces_retained".to_string(),
                fixture
                    .retained
                    .get("sha256")
                    .and_then(Value::as_str)
                    .map_or(Value::Null, |sha| json!(sha == briefing.sha256())),
            );
            // After: the coverage packer on the same evidence, whole.
            let whole = input.whole.clone().unwrap_or_else(|| input.inputs.clone());
            let params = crate::pack::Params {
                cap: input.cap,
                ..crate::pack::Params::default()
            };
            let map = crate::requirements::mechanical(&whole.instruction);
            // The first packer's items as it had them, with the relevance
            // of each item its briefing left out restored from `whole`.
            let mut first = input.inputs.clone();
            for (file, restored) in first.files.iter_mut().zip(&whole.files) {
                if file.1.is_none() {
                    file.1 = restored.1;
                }
            }
            let before = crate::pack::measure(
                &briefing,
                &crate::pack::delivered_by_sections(&first, &briefing),
            );
            let after = crate::pack::pack(&whole, &map, None, params);
            let measured = crate::pack::measure(
                &after.briefing,
                &crate::pack::delivered_by_pack(&whole, &after),
            );
            let mut add = |prefix: &str, m: &crate::pack::Measure, uncovered: Option<usize>| {
                for (name, value) in [
                    ("selected_dropped", json!(m.selected_dropped)),
                    ("duplicate_bytes", json!(m.duplicate_bytes)),
                    ("omitted", json!(m.omitted)),
                    ("data_chars", json!(m.data_chars)),
                    ("chars", json!(m.chars)),
                    (
                        "drops_selected_while_duplicates_kept",
                        json!(m.dropped_while_duplicates_kept),
                    ),
                ] {
                    metrics.insert(format!("{prefix}_{name}"), value);
                }
                if let Some(uncovered) = uncovered {
                    metrics.insert(format!("{prefix}_uncovered_requirements"), json!(uncovered));
                }
            };
            add("before", &before, None);
            add("after", &measured, Some(after.record.uncovered.len()));
            // With Jev: the same packer, with Jev's coverage judgments
            // deciding what each item informs.
            let mut jev_record = Value::Null;
            if !matches!(jev, JevMode::Off) {
                let title = whole
                    .instruction
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| crate::judge::clip(line.trim(), 120))
                    .unwrap_or_default();
                let (coverage, asked) = crate::pack::judge_coverage(
                    &title,
                    &whole.instruction,
                    &whole,
                    &map,
                    jev,
                    recorder,
                    None,
                )
                .await;
                if asked.iter().any(|a| a.answered()) {
                    let judged = crate::pack::pack(&whole, &map, Some(&coverage), params);
                    let m = crate::pack::measure(
                        &judged.briefing,
                        &crate::pack::delivered_by_pack(&whole, &judged),
                    );
                    add("after_jev", &m, Some(judged.record.uncovered.len()));
                    jev_record = json!(judged.record);
                }
            }
            Ok(Ran {
                output: json!({
                    "before": briefing.record(),
                    "after": after.record,
                    "after_text": after.briefing.text,
                    "after_jev": jev_record,
                }),
                metrics,
            })
        })
    }
}

/// `task.profile`: Jev's feature battery over the task text, and, when the
/// input carries the executors' measured behavior, the executor it names.
struct TaskProfile;

#[derive(Deserialize)]
struct ProfileInput {
    task: Task,
    #[serde(default)]
    executors: Vec<crate::profile::Executor>,
}

impl Component for TaskProfile {
    fn id(&self) -> &'static str {
        "task.profile"
    }
    fn implementation(&self) -> Implementation {
        crate::profile::implementation()
    }
    fn about(&self) -> &'static str {
        "Jev's task features for the router; a Choice of executor only with their measured behavior."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: ProfileInput = input(fixture)?;
            let request = crate::profile::request(&input.task.state(), &input.executors)?;
            let asked = ask_one(jev, recorder, self.id(), "jev_profile", request).await;
            let profile = crate::profile::read(asked.answers.as_ref());
            let mut metrics = Map::new();
            for (id, p) in &profile.features {
                metrics.insert(id.clone(), json!(p));
            }
            metrics.insert("difficulty".to_string(), json!(profile.difficulty));
            metrics.insert(
                "unknown".to_string(),
                json!(
                    profile.features.values().filter(|p| p.is_none()).count()
                        + usize::from(profile.difficulty.is_none())
                ),
            );
            if !input.executors.is_empty() {
                // Whether the Choice named an executor at all; the name is
                // in the output, since a summary can't average it.
                metrics.insert(
                    "executor_named".to_string(),
                    json!(profile.executor.is_some()),
                );
            }
            Ok(Ran {
                output: serde_json::to_value(&profile).unwrap_or(Value::Null),
                metrics,
            })
        })
    }
}

/// `exec.system`: the executor's system prompt, with the optional sections
/// Jev reads the task as needing.
struct SystemSelect;

#[derive(Deserialize)]
struct SystemInput {
    task: Task,
    /// `claude-code` or `codex`.
    agent: String,
    system: crate::system::Policy,
}

impl Component for SystemSelect {
    fn id(&self) -> &'static str {
        "exec.system"
    }
    fn implementation(&self) -> Implementation {
        crate::system::implementation()
    }
    fn about(&self) -> &'static str {
        "Jev picks the optional system prompt sections the task needs; the protected section stays."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: SystemInput = input(fixture)?;
            let agent = crate::delegate::Agent::parse(&input.agent)?;
            let problems = input.system.validate(agent);
            if !problems.is_empty() {
                return Err(problems.join("; "));
            }
            let mut variant = crate::system::Variant::new(agent, input.system);
            if !variant.options().is_empty() {
                let request =
                    crate::system::selection_request(&input.task.state(), variant.options());
                let asked = ask_one(jev, recorder, self.id(), "jev_system", request).await;
                let answers =
                    crate::system::selection_answers(variant.options(), |id| asked.noul(id));
                variant.select(answers);
            }
            let record = variant.record();
            let selected: Vec<String> = variant
                .sections()
                .iter()
                .filter(|(_, by)| *by == "jev")
                .map(|(section, _)| section.id.to_string())
                .collect();
            let default_chars = crate::system::default_text(agent).chars().count();
            let chars = variant.text().chars().count();
            let mut metrics = Map::new();
            metrics.insert("chars".to_string(), json!(chars));
            metrics.insert("default_chars".to_string(), json!(default_chars));
            metrics.insert(
                "saved_chars".to_string(),
                json!(
                    i64::try_from(default_chars).unwrap_or(0) - i64::try_from(chars).unwrap_or(0)
                ),
            );
            metrics.insert("sections".to_string(), json!(variant.sections().len()));
            metrics.insert("selected".to_string(), json!(selected.len()));
            metrics.insert(
                "unknown".to_string(),
                json!(variant.asked.iter().filter(|(_, p)| p.is_none()).count()),
            );
            metrics.insert(
                "protected_present".to_string(),
                json!(variant.policy.protected(agent)),
            );
            metrics.insert(
                "matches_retained".to_string(),
                same_set(&selected, fixture.retained.get("selected")),
            );
            Ok(Ran {
                output: json!({ "variant": record, "selected": selected }),
                metrics,
            })
        })
    }
}

/// `verify.close`: whether the delegate's report and the changes show the
/// task done.
struct Close;

#[derive(Deserialize)]
struct CloseInput {
    task: Task,
    #[serde(default)]
    criteria: Vec<String>,
    report: String,
    changes: String,
}

impl Component for Close {
    fn id(&self) -> &'static str {
        "verify.close"
    }
    fn implementation(&self) -> Implementation {
        evidence::close_implementation()
    }
    fn about(&self) -> &'static str {
        "Jev reads the delegate's report and the changes as done or not."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: CloseInput = input(fixture)?;
            let request = evidence::close_request(
                &input.task.state(),
                &input.criteria,
                &input.report,
                &input.changes,
            );
            let asked = ask_one(jev, recorder, self.id(), "jev_close", request).await;
            let done = asked.noul("done");
            let criteria: Vec<Value> = input
                .criteria
                .iter()
                .enumerate()
                .map(|(j, c)| json!({ "requirement": c, "p": asked.noul(&format!("criterion_{j}")) }))
                .collect();
            let mut metrics = Map::new();
            metrics.insert("done".to_string(), json!(done));
            metrics.insert(
                "reads_done".to_string(),
                done.map_or(Value::Null, |p| json!(p >= evidence::YES)),
            );
            metrics.insert(
                "matches_retained".to_string(),
                match (done, fixture.retained.get("done").and_then(Value::as_f64)) {
                    (Some(p), Some(q)) => json!((p - q).abs() < 1e-9),
                    _ => Value::Null,
                },
            );
            Ok(Ran {
                output: json!({ "done": done, "criteria": criteria }),
                metrics,
            })
        })
    }
}

/// Where Jev's answers come from for a run: live, the fixture's recorded
/// answers, or nowhere.
#[derive(Clone)]
pub enum JevChoice {
    Live(::jev::Client),
    Recorded,
    Off,
}

impl JevChoice {
    /// Parses `live`, `recorded`, or `off`; `live` needs a client.
    ///
    /// # Errors
    ///
    /// Returns a message for another word, or `live` without a key.
    pub fn parse(
        word: &str,
        client: impl FnOnce() -> Result<::jev::Client, String>,
    ) -> Result<Self, String> {
        match word {
            "recorded" => Ok(JevChoice::Recorded),
            "off" => Ok(JevChoice::Off),
            "live" => Ok(JevChoice::Live(client()?)),
            other => Err(format!("--jev takes live, recorded, or off, not {other}")),
        }
    }

    /// The mode's word.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            JevChoice::Live(_) => "live",
            JevChoice::Recorded => "recorded",
            JevChoice::Off => "off",
        }
    }

    pub(crate) fn mode(&self, dir: &Path) -> Result<JevMode, String> {
        Ok(match self {
            JevChoice::Live(client) => JevMode::Live(client.clone()),
            JevChoice::Recorded => JevMode::Recorded(Recorded::load(&dir.join(RECORDED_FILE))?),
            JevChoice::Off => JevMode::Off,
        })
    }
}

/// One fixture's run.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureRun {
    /// The fixture directory's name.
    pub fixture: String,
    pub input_digest: String,
    pub output: Value,
    pub output_digest: String,
    pub metrics: Map<String, Value>,
    /// Jev requests by how they were answered: `recorded`, `miss`, `live`,
    /// `failed`, `off`.
    pub jev: Map<String, Value>,
    /// The run's error, when it didn't produce an output.
    pub error: Option<String>,
    pub milliseconds: u64,
    /// The summed cost of the Jev requests, when every one is known.
    pub cost_usd: Option<f64>,
}

impl FixtureRun {
    /// The run as the result's `fixtures` array holds it: everything but
    /// the timing, so two recorded runs compare byte for byte.
    #[must_use]
    pub fn result(&self) -> Value {
        json!({
            "fixture": self.fixture,
            "input_digest": self.input_digest,
            "output_digest": self.output_digest,
            "output": self.output,
            "metrics": self.metrics,
            "jev": self.jev,
            "error": self.error,
            "cost_usd": self.cost_usd,
        })
    }
}

/// Runs `component` on the fixture in `dir`, recording one invocation
/// under `parent`.
///
/// # Errors
///
/// Returns a message when the fixture or its recorded answers don't read.
/// A component that fails on its input is a run with an error, not an
/// error of the runner.
pub async fn run_fixture(
    component: &dyn Component,
    dir: &Path,
    choice: &JevChoice,
    recorder: &Recorder,
    parent: Option<&str>,
) -> Result<FixtureRun, String> {
    let fixture = Fixture::load(dir, component.id())?;
    let mode = choice.mode(dir)?;
    let name = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let input_digest = atif::digest(&fixture.input);
    let invocation = recorder.enter(
        Start::new(component.id(), component.implementation())
            .named(&name)
            .reading_digest(input_digest.clone())
            .under(parent),
    );
    let started = Instant::now();
    let result = component.run(&fixture, &mode, recorder).await;
    let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    // Tally the Jev requests this run made, from their own records.
    let mut jev = Map::new();
    let mut costs = Vec::new();
    // Every descendant counts: a component may ask under an invocation of
    // its own, as `verify.support` does.
    let all = crate::record::invocations(&recorder.steps());
    let mut mine = std::collections::BTreeSet::from([invocation.clone()]);
    for child in &all {
        if child.parent.as_ref().is_some_and(|p| mine.contains(p)) {
            mine.insert(child.id.clone());
        }
    }
    for child in all
        .into_iter()
        .filter(|child| child.parent.as_ref().is_some_and(|p| mine.contains(p)))
    {
        let Some(end) = &child.ended else { continue };
        // Only a Jev request records how it was answered; a component's
        // other children, such as session control, aren't Jev's.
        let Some(how) = end.pointer("/output/summary/how").and_then(Value::as_str) else {
            continue;
        };
        let count = jev.get(how).and_then(Value::as_u64).unwrap_or(0) + 1;
        jev.insert(how.to_string(), json!(count));
        costs.push(end.pointer("/cost/usd").and_then(Value::as_f64));
    }
    let cost_usd = costs
        .iter()
        .copied()
        .sum::<Option<f64>>()
        .map(|usd| usd + 0.0);
    let (output, metrics, error) = match result {
        Ok(ran) => (ran.output, ran.metrics, None),
        Err(error) => (Value::Null, Map::new(), Some(error)),
    };
    let output_digest = atif::digest(&output);
    recorder.end(
        &invocation,
        Finish::new(if error.is_none() {
            Outcome::Completed
        } else {
            Outcome::Failed
        })
        .output(json!({
            "fixture": name,
            "jev_mode": choice.word(),
            "output": output,
            "metrics": metrics,
            "jev": jev,
            "error": error,
        })),
    );
    Ok(FixtureRun {
        fixture: name,
        input_digest,
        output,
        output_digest,
        metrics,
        jev,
        error,
        milliseconds,
        cost_usd,
    })
}

/// A suite: one component over every fixture that has an input for it.
#[derive(Clone, Debug)]
pub struct Suite {
    pub component: String,
    pub implementation: Implementation,
    pub jev_mode: String,
    pub runs: Vec<FixtureRun>,
    /// The log the suite's invocation records went to, when one was
    /// written.
    pub log: Option<PathBuf>,
    pub milliseconds: u64,
}

impl Suite {
    /// The deterministic part of the suite: no times, no paths. A
    /// recorded-Jev suite rerun on the same fixtures gives the same bytes.
    #[must_use]
    pub fn result(&self) -> Value {
        json!({
            "schema": RESULT_SCHEMA,
            "component": self.component,
            "implementation": self.implementation,
            "jev_mode": self.jev_mode,
            "fixtures": self.runs.iter().map(FixtureRun::result).collect::<Vec<_>>(),
            "summary": self.summary(),
        })
    }

    /// The result, plus timing and the log's path.
    #[must_use]
    pub fn report(&self) -> Value {
        json!({
            "result": self.result(),
            "timing": {
                "milliseconds": self.milliseconds,
                "fixtures": self.runs.iter().map(|run| json!({ "fixture": run.fixture, "milliseconds": run.milliseconds })).collect::<Vec<_>>(),
            },
            "log": self.log.as_ref().map(|path| path.display().to_string()),
        })
    }

    /// Each metric over the fixtures: the mean of numbers and the count of
    /// true flags, with how many fixtures reported it.
    #[must_use]
    pub fn summary(&self) -> Value {
        summarize(&self.runs)
    }
}

/// Aggregates metrics over runs.
#[must_use]
pub fn summarize(runs: &[FixtureRun]) -> Value {
    let mut names: Vec<&String> = runs.iter().flat_map(|run| run.metrics.keys()).collect();
    names.sort();
    names.dedup();
    let mut metrics = Map::new();
    for name in names {
        let values: Vec<&Value> = runs
            .iter()
            .filter_map(|run| run.metrics.get(name))
            .filter(|value| !value.is_null())
            .collect();
        let summary = if values.iter().all(|value| value.is_boolean()) {
            json!({
                "true": values.iter().filter(|value| value.as_bool() == Some(true)).count(),
                "of": values.len(),
            })
        } else {
            let numbers: Vec<f64> = values.iter().filter_map(|value| value.as_f64()).collect();
            json!({
                "mean": if numbers.is_empty() { Value::Null } else { json!(pack::round(numbers.iter().sum::<f64>() / numbers.len() as f64)) },
                "of": numbers.len(),
            })
        };
        metrics.insert(name.clone(), summary);
    }
    let mut jev = Map::new();
    for run in runs {
        for (how, count) in &run.jev {
            let total =
                jev.get(how).and_then(Value::as_u64).unwrap_or(0) + count.as_u64().unwrap_or(0);
            jev.insert(how.clone(), json!(total));
        }
    }
    json!({
        "fixtures": runs.len(),
        "errors": runs.iter().filter(|run| run.error.is_some()).count(),
        "metrics": metrics,
        "jev": jev,
        "cost_usd": runs.iter().map(|run| run.cost_usd).sum::<Option<f64>>().map(|usd| usd + 0.0),
    })
}

/// The fixture directories under `root` that hold an input for
/// `component`, sorted by name.
#[must_use]
pub fn fixtures_for(root: &Path, component: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join(format!("{component}.json")).is_file())
        .collect();
    dirs.sort();
    dirs
}

/// Opens a durable recorder for a component run under `out`, or an
/// in-memory one when `out` is `None`.
///
/// # Errors
///
/// Returns a message when the log can't be created.
pub fn recorder(out: Option<&Path>, label: &str) -> Result<Recorder, String> {
    let Some(out) = out else {
        return Ok(Recorder::default());
    };
    let at = atif::now_ms();
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let id = format!("component-{label}-{at}-{}-{sequence}", std::process::id());
    let session = atif::Session::opening(
        &id,
        crate::credentials::JEV_MODEL,
        SUITE_DOOR,
        &std::env::current_dir()
            .map(|dir| dir.display().to_string())
            .unwrap_or_default(),
        &crate::episode::version(),
    );
    let log = atif::Log::create_at(
        &out.join(format!("{id}.{}", atif::log::EXTENSION)),
        &session,
    )
    .map_err(|error| format!("cannot create a run log under {}: {error}", out.display()))?;
    Ok(Recorder::durable(log))
}

/// Runs `component` over `dirs`, recording a suite invocation with one
/// child per fixture. `save_live` adds each live answer to its fixture's
/// recorded answers.
///
/// # Errors
///
/// Returns a message when a fixture doesn't read.
pub async fn suite(
    component: &dyn Component,
    dirs: &[PathBuf],
    choice: &JevChoice,
    recorder: &Recorder,
    save_live: bool,
) -> Result<Suite, String> {
    let started = Instant::now();
    let root = recorder.enter(
        Start::new("suite", component.implementation())
            .named(component.id())
            .reading(&json!(
                dirs.iter()
                    .map(|dir| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
                    .collect::<Vec<_>>()
            )),
    );
    let mut runs = Vec::new();
    for dir in dirs {
        let before = recorder.steps().len();
        let run = run_fixture(component, dir, choice, recorder, Some(&root)).await?;
        if save_live && matches!(choice, JevChoice::Live(_)) {
            let path = dir.join(RECORDED_FILE);
            let mut recorded = Recorded::load(&path)?;
            if jev::record_answers(
                &recorder.steps()[before..],
                &format!("live component run of {}", component.id()),
                &mut recorded,
            ) > 0
            {
                recorded.save(&path)?;
            }
        }
        runs.push(run);
    }
    let milliseconds = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let suite = Suite {
        component: component.id().to_string(),
        implementation: component.implementation(),
        jev_mode: choice.word().to_string(),
        runs,
        log: recorder.log_path(),
        milliseconds,
    };
    recorder.end(
        &root,
        Finish::new(Outcome::Completed).output(json!({
            "component": suite.component,
            "jev_mode": suite.jev_mode,
            "summary": suite.summary(),
        })),
    );
    recorder.finish(atif::log::ENDED);
    Ok(suite)
}

/// The schema of a suite's export.
pub const EXPORT_SCHEMA: &str = "openagents.coder-one.component-export.v1";

/// A suite's outputs by task, for a reader outside this crate, such as the
/// Gym's router reading `task.profile` features. Deterministic: no times
/// and no paths.
///
/// # Errors
///
/// Returns a message when a fixture no longer reads.
pub fn export(suite: &Suite, dirs: &[PathBuf]) -> Result<Value, String> {
    let mut fixtures = Vec::new();
    for (run, dir) in suite.runs.iter().zip(dirs) {
        let fixture = Fixture::load(dir, &suite.component)?;
        let task = fixture
            .source
            .pointer("/trace/task")
            .or_else(|| fixture.source.get("task"))
            .cloned()
            .unwrap_or(Value::Null);
        // What the recorded answers cost when they were asked live, so a
        // reader can charge the component even when this run replayed them.
        let recorded = Recorded::load(&dir.join(RECORDED_FILE))?;
        let live = format!("live component run of {}", suite.component);
        let tokens: Option<u64> = recorded
            .entries
            .values()
            .filter(|entry| entry.source.starts_with(&live))
            .map(|entry| entry.input_tokens)
            .sum();
        fixtures.push(json!({
            "fixture": run.fixture,
            "task": task,
            "input_digest": run.input_digest,
            "output": run.output,
            "jev": run.jev,
            "cost_usd": run.cost_usd,
            "recorded_live_cost_usd": tokens.map(|tokens| {
                // Whole nanodollars, so the file carries no float noise.
                (tokens as f64 * jev::USD_PER_MILLION_INPUT * 1_000.0).round() / 1e9
            }),
            "error": run.error,
        }));
    }
    Ok(json!({
        "schema": EXPORT_SCHEMA,
        "component": suite.component,
        "implementation": suite.implementation,
        "jev_mode": suite.jev_mode,
        "fixtures": fixtures,
    }))
}

/// The fixture root this checkout ships.
#[must_use]
pub fn default_fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/components")
}

/// Where component runs record themselves: `~/.openagents/coder-one/components`.
#[must_use]
pub fn default_runs_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/components"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        default_fixtures()
    }

    #[test]
    fn concurrent_component_recorders_keep_distinct_logs() {
        let dir = super::handoff_scratch(atif::now_ms()).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(16));
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let dir = dir.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    recorder(Some(&dir), "parallel").unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 16);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn concurrent_handoff_scratch_directories_do_not_share_a_millisecond() {
        let handles: Vec<_> = (0..16)
            .map(|_| std::thread::spawn(|| super::handoff_scratch(123).unwrap()))
            .collect();
        let dirs: std::collections::BTreeSet<_> =
            handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(dirs.len(), 16);
        for dir in dirs {
            std::fs::remove_dir(dir).unwrap();
        }
    }

    #[tokio::test]
    async fn every_component_runs_on_the_checked_in_fixtures_with_recorded_jev() {
        for component in registry() {
            let dirs = fixtures_for(&fixtures(), component.id());
            assert!(!dirs.is_empty(), "no fixture for {}", component.id());
            let suite = suite(
                component.as_ref(),
                &dirs,
                &JevChoice::Recorded,
                &Recorder::default(),
                false,
            )
            .await
            .unwrap();
            for run in &suite.runs {
                assert!(run.error.is_none(), "{}: {:?}", run.fixture, run.error);
                assert!(
                    run.jev.get("miss").is_none(),
                    "{} {} missed the recorded answers",
                    component.id(),
                    run.fixture
                );
                for flag in ["matches_retained", "reproduces_retained"] {
                    if let Some(value) = run.metrics.get(flag) {
                        assert_ne!(
                            value,
                            &json!(false),
                            "{} {} does not reproduce the retained run",
                            component.id(),
                            run.fixture
                        );
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn a_recorded_suite_reruns_byte_identically() {
        let component = find("evidence.probes.selector").unwrap();
        let dirs = fixtures_for(&fixtures(), component.id());
        let first = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Recorded,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        let second = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Recorded,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            serde_json::to_string(&first.result()).unwrap(),
            serde_json::to_string(&second.result()).unwrap()
        );
    }

    /// The Gym's router reads this export; it must be what the recorded
    /// answers give today.
    #[tokio::test]
    async fn the_checked_in_task_features_are_current() {
        let component = find("task.profile").unwrap();
        let dirs = fixtures_for(&fixtures(), component.id());
        assert_eq!(dirs.len(), 8);
        let suite = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Recorded,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        assert!(suite.runs.iter().all(|run| run.jev.get("miss").is_none()));
        let fresh = format!(
            "{}\n",
            serde_json::to_string_pretty(&export(&suite, &dirs).unwrap()).unwrap()
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../bench/terminal-bench/profiles/task-features.json");
        let kept = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            fresh == kept,
            "{} is stale: run `coder-one component suite task.profile --no-record --export {}`",
            path.display(),
            path.display()
        );
    }

    #[tokio::test]
    async fn off_leaves_every_judgment_unknown() {
        let component = find("evidence.probes.selector").unwrap();
        let dirs = fixtures_for(&fixtures(), component.id());
        let suite = suite(
            component.as_ref(),
            &dirs[..1],
            &JevChoice::Off,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        let run = &suite.runs[0];
        assert_eq!(run.metrics["kept"], json!(0));
        assert_eq!(run.metrics["unknown"], run.metrics["probes"]);
        assert_eq!(run.jev["off"], json!(1));
    }

    #[tokio::test]
    async fn a_suite_writes_invocation_records_to_its_log() {
        let out = std::env::temp_dir().join(format!("coder-one-suite-{}", atif::now_ms()));
        let recorder = recorder(Some(&out), "evidence.pack").unwrap();
        let component = find("evidence.pack").unwrap();
        let dirs = fixtures_for(&fixtures(), component.id());
        let suite = suite(
            component.as_ref(),
            &dirs,
            &JevChoice::Recorded,
            &recorder,
            false,
        )
        .await
        .unwrap();
        let log = suite.log.unwrap();
        let read = atif::log::read_whole(&log).unwrap();
        let invocations = crate::record::invocations(&read.steps);
        assert_eq!(invocations[0].component, "suite");
        assert_eq!(
            invocations
                .iter()
                .filter(|i| i.component == "evidence.pack"
                    && i.parent.as_deref() == Some(invocations[0].id.as_str()))
                .count(),
            dirs.len()
        );
        assert!(invocations.iter().all(|i| i.outcome() == "completed"));
        let _ = std::fs::remove_dir_all(out);
    }
}

#[derive(Deserialize)]
struct HandoffInput {
    /// A policy manifest under `crates/coder-one/policies`.
    manifest: String,
    task: String,
    #[serde(default)]
    expect: HandoffExpect,
}

#[derive(Deserialize, Default)]
struct HandoffExpect {
    passed: Option<bool>,
    /// Handoff actions, in order.
    #[serde(default)]
    actions: Vec<String>,
    /// Branch roles, in order.
    #[serde(default)]
    branches: Vec<String>,
}

/// `control.handoff`: one pattern on one mini-task with scripted tiers.
fn handoff_scratch(at: u64) -> Result<std::path::PathBuf, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let out = std::env::temp_dir().join(format!(
            "coder-one-component-handoff-{}-{at}-{n}",
            std::process::id()
        ));
        match std::fs::create_dir(&out) {
            Ok(()) => return Ok(out),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("Cannot create handoff fixture directory: {error}")),
        }
    }
}

struct HandoffComponent;

impl Component for HandoffComponent {
    fn id(&self) -> &'static str {
        crate::handoff::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        crate::handoff::Policy::single().implementation()
    }
    fn about(&self) -> &'static str {
        "Escalate, plan and implement, steer, or race within one budget, on a mini-task with scripted tiers."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: HandoffInput = input(fixture)?;
            let path = crate::policy::reference_dir().join(&input.manifest);
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let manifest = crate::policy::Manifest::parse(&text)?;
            let candidate = crate::handoff::manifest_candidate(&manifest);
            let mut policy = candidate.policy;
            if let Some(to) = &policy.to {
                policy.to = Some(crate::handoff::scripted(to));
            }
            let out = handoff_scratch(atif::now_ms())?;
            let ran = crate::handoff::run(crate::handoff::Options {
                task: crate::minitask::find(&input.task)?,
                policy,
                first: crate::handoff::scripted(&candidate.first),
                out: out.clone(),
                deadline: std::time::Duration::from_secs(600),
                jev: None,
                checks: false,
            })
            .await;
            let _ = std::fs::remove_dir_all(&out);
            let ran = ran?;
            let passed = match ran.grade.verdict.as_str() {
                "passed" => Some(true),
                "failed" => Some(false),
                _ => None,
            };
            let actions: Vec<String> = ran
                .handoffs
                .iter()
                .filter_map(|h| h["action"].as_str().map(str::to_string))
                .collect();
            let branches: Vec<String> =
                ran.ledger.branches.iter().map(|b| b.role.clone()).collect();
            let mut matches = true;
            if let Some(want) = input.expect.passed {
                matches &= passed == Some(want);
            }
            if !input.expect.actions.is_empty() {
                matches &= input.expect.actions == actions;
            }
            if !input.expect.branches.is_empty() {
                matches &= input.expect.branches == branches;
            }
            let mut metrics = Map::new();
            metrics.insert(
                "passed".to_string(),
                passed.map_or(Value::Null, |p| json!(p)),
            );
            metrics.insert("matches_expected".to_string(), json!(matches));
            metrics.insert(
                "episode_seconds".to_string(),
                json!(ran.ledger.clock_ms as f64 / 1_000.0),
            );
            metrics.insert(
                "usd".to_string(),
                ran.ledger
                    .usd()
                    .map_or(Value::Null, |usd| json!((usd * 1e6).round() / 1e6)),
            );
            metrics.insert("handoffs".to_string(), json!(ran.handoffs.len()));
            metrics.insert(
                "reaped".to_string(),
                json!(
                    ran.ledger
                        .branches
                        .iter()
                        .filter(|b| b.stopped_by.is_some())
                        .all(|b| b.reaped)
                ),
            );
            Ok(Ran {
                output: json!({
                    "pattern": ran.manifest["pattern"],
                    "grade": ran.grade.verdict,
                    "grade_detail": ran.grade.detail,
                    "actions": actions,
                    "ledger": ran.ledger.record(),
                    "briefs": ran.handoffs.iter().map(|h| h["brief"]["sha256"].clone()).collect::<Vec<_>>(),
                }),
                metrics,
            })
        })
    }
}
