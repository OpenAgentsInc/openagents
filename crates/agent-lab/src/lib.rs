//! Agent Lab documents and experiment evidence. Checking and rendering perform no I/O.
use agent_contract::{AgentDefinition, AgentSpec, valid_id};
use lab_kit::{LabKind, Refusal};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;
pub mod api;
pub mod components;
pub mod kind;
#[cfg(feature = "views")]
pub mod view;
pub use kind::AgentLab;

pub const MAX_TRIALS: usize = 32;
fn short(s: &str, n: usize) -> bool {
    !s.trim().is_empty() && s.len() <= n && !s.chars().any(char::is_control)
}
fn text(s: &str, n: usize) -> bool {
    !s.trim().is_empty() && s.len() <= n && !s.contains('\0')
}
fn invalid(message: &'static str) -> Refusal {
    Refusal::new("invalid_experiment", message)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub id: String,
    pub revision: u64,
}
impl Pin {
    pub fn valid(&self) -> bool {
        valid_id(&self.id) && self.revision > 0
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub spec: AgentSpec,
    pub source: Option<Pin>,
    #[serde(default)]
    pub components: Vec<Component>,
}
impl Draft {
    pub fn agent_spec(&self) -> Result<AgentSpec, Refusal> {
        let mut spec = self.spec.clone();
        for c in &self.components {
            c.validate()?;
            match c.context() {
                Some(context) => {
                    spec.instructions.push_str("\n\n");
                    spec.instructions.push_str(&context);
                }
                None => {
                    return Err(invalid(
                        "Remove live Lab references before saving this as an agent.",
                    ));
                }
            }
        }
        spec.validate().map_err(invalid)?;
        Ok(spec)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Component {
    Skill {
        name: String,
        text: String,
        sha256: String,
    },
    Memory {
        name: String,
        text: String,
        sha256: String,
    },
    ToolContext {
        binding: String,
        sha256: String,
    },
}
impl Component {
    pub fn validate(&self) -> Result<(), Refusal> {
        match self {
            Self::Skill {
                name,
                text: body,
                sha256,
            }
            | Self::Memory {
                name,
                text: body,
                sha256,
            } => {
                if !short(name, 80)
                    || !text(body, 8192)
                    || lab_kit::digest(&json!({"name":name,"text":body})) != *sha256
                {
                    return Err(invalid("Check the saved reference text."));
                }
            }
            Self::ToolContext { binding, sha256 } => {
                if !short(binding, 80)
                    || !binding
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                    || !valid_id(sha256)
                {
                    return Err(invalid("Choose an installed tool reference."));
                }
            }
        }
        Ok(())
    }
    pub fn context(&self) -> Option<String> {
        match self {
            Self::Skill { name, text, .. } | Self::Memory { name, text, .. } => {
                Some(format!("{name}:\n{text}"))
            }
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    Development,
    Confirmation,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suite {
    pub title: String,
    pub partition: Partition,
    pub cases: Vec<Case>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub title: String,
    pub task: String,
    pub check: Check,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    Contains { expected: String },
    Exact { expected: String },
    Json { pointer: String, expected: Value },
    Review { criteria: String },
}
impl Check {
    pub fn validate(&self) -> bool {
        match self {
            Self::Contains { expected } | Self::Exact { expected } => text(expected, 4096),
            Self::Json { pointer, expected } => {
                pointer.len() <= 256
                    && (pointer.is_empty() || pointer.starts_with('/'))
                    && serde_json::to_vec(expected).is_ok_and(|v| v.len() <= 4096)
            }
            Self::Review { criteria } => text(criteria, 2048),
        }
    }
    /// None means that explicit human review is still needed, never a passing check.
    pub fn evaluate(&self, result: &str) -> Option<bool> {
        match self {
            Self::Contains { expected } => Some(result.contains(expected)),
            Self::Exact { expected } => Some(result.trim() == expected.trim()),
            Self::Json { pointer, expected } => Some(
                serde_json::from_str::<Value>(result)
                    .ok()
                    .and_then(|v| v.pointer(pointer).cloned())
                    .as_ref()
                    == Some(expected),
            ),
            Self::Review { .. } => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Experiment {
    pub title: String,
    pub draft: Draft,
    pub suite: Suite,
    pub compare: Option<Pin>,
    pub host: String,
    pub project: String,
    pub repetitions: u32,
    pub max_seconds: u64,
}
impl Experiment {
    pub fn validate(&self) -> Result<(), Refusal> {
        self.draft.spec.validate().map_err(invalid)?;
        if !short(&self.title, 100)
            || !short(&self.suite.title, 100)
            || !short(&self.host, 128)
            || !short(&self.project, 128)
        {
            return Err(invalid("Choose an agent, computer, and project."));
        }
        if self.draft.source.as_ref().is_some_and(|p| !p.valid())
            || self.compare.as_ref().is_some_and(|p| !p.valid())
        {
            return Err(invalid("Choose a saved agent version."));
        }
        if !(1..=8).contains(&self.suite.cases.len())
            || !(1..=4).contains(&self.repetitions)
            || !(10..=1800).contains(&self.max_seconds)
            || self.trial_count() > MAX_TRIALS
        {
            return Err(invalid(
                "Use up to eight cases and 32 total runs, within 30 minutes.",
            ));
        }
        let mut ids = BTreeSet::new();
        for c in &self.suite.cases {
            if !short(&c.id, 64)
                || !short(&c.title, 100)
                || !text(&c.task, 16384)
                || !c.check.validate()
                || !ids.insert(&c.id)
            {
                return Err(invalid(
                    "Give each test a unique name, a task, and a check.",
                ));
            }
        }
        if self.draft.components.len() > 8 {
            return Err(invalid("Use up to eight references."));
        }
        for c in &self.draft.components {
            c.validate()?;
        }
        if (0..self.suite.cases.len()).any(|i| self.task(i).len() > 48 * 1024) {
            return Err(invalid("Shorten the test tasks or reference text."));
        }
        if serde_json::to_vec(self).map_or(true, |v| v.len() > 128 * 1024) {
            return Err(invalid("Shorten this experiment."));
        }
        Ok(())
    }
    pub fn trial_count(&self) -> usize {
        self.suite.cases.len()
            * self.repetitions as usize
            * if self.compare.is_some() { 2 } else { 1 }
    }
    pub fn task(&self, case: usize) -> String {
        let mut task = String::new();
        for c in &self.draft.components {
            if let Some(context) = c.context() {
                task.push_str(&context);
                task.push_str("\n\n");
            }
        }
        task.push_str(&self.suite.cases[case].task);
        task
    }
    pub fn workload(&self) -> String {
        lab_kit::digest(
            &json!({"suite":self.suite,"host":self.host,"project":self.project,"repetitions":self.repetitions,"max_seconds":self.max_seconds,"components":self.draft.components}),
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "schema", deny_unknown_fields)]
pub enum Content {
    #[serde(rename = "agent-draft.v1")]
    Draft { draft: Draft },
    #[serde(rename = "agent-suite.v1")]
    Suite { suite: Suite },
    #[serde(rename = "agent-experiment.v1")]
    Experiment { experiment: Experiment },
    #[serde(rename = "agent-results.v1")]
    Results { run: Run },
}
impl Content {
    pub fn validate(&self) -> Result<(), Refusal> {
        match self {
            Self::Draft { draft } => {
                draft.spec.validate().map_err(invalid)?;
                if draft.components.len() > 8 || draft.source.as_ref().is_some_and(|p| !p.valid()) {
                    return Err(invalid("Check the agent references."));
                }
                for c in &draft.components {
                    c.validate()?;
                }
                Ok(())
            }
            Self::Suite { suite } => {
                let spec = starter();
                let mut e = spec;
                e.suite = suite.clone();
                e.validate()
            }
            Self::Experiment { experiment } => experiment.validate(),
            Self::Results { run } => run.validate(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub id: String,
    pub versions: Vec<lab_kit::Version>,
    pub deleted: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Pending,
    Running,
    Passed,
    Failed,
    NeedsReview,
    Stopped,
    TimedOut,
    Unavailable,
    Unknown,
}
impl Outcome {
    pub fn finished(self) -> bool {
        !matches!(self, Self::Pending | Self::Running)
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub project: String,
    pub source_sha256: String,
    pub context_sha256: String,
    pub runtime_version: String,
    pub context: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trial {
    pub index: usize,
    pub candidate: usize,
    pub case: usize,
    pub repetition: u32,
    pub assignment: String,
    pub chat: Option<String>,
    pub outcome: Outcome,
    pub result: Option<String>,
    pub evidence: Option<Evidence>,
    pub started_ms: Option<u64>,
    pub finished_ms: Option<u64>,
    pub cost_usd: Option<f64>,
    pub reviewer: Option<String>,
    pub review_note: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub id: String,
    pub revision: u64,
    pub version: lab_kit::Reference,
    pub experiment: Experiment,
    pub candidates: Vec<AgentDefinition>,
    pub trials: Vec<Trial>,
    pub created_ms: u64,
    pub deadline_ms: u64,
    pub stopping: bool,
    pub archived: bool,
    #[serde(default)]
    pub host_settled: bool,
}
impl Run {
    pub fn validate(&self) -> Result<(), Refusal> {
        self.experiment.validate()?;
        if !valid_id(&self.id)
            || self.revision == 0
            || self.candidates.is_empty()
            || self.candidates.len()
                != if self.experiment.compare.is_some() {
                    2
                } else {
                    1
                }
            || self.trials.len() != self.experiment.trial_count()
        {
            return Err(invalid("This experiment record is incomplete."));
        }
        if !valid_id(&self.version.document)
            || self.version.version == 0
            || self
                .candidates
                .iter()
                .any(|d| !valid_id(&d.id) || d.revision == 0 || d.spec.validate().is_err())
        {
            return Err(invalid("Check the saved agent and experiment versions."));
        }
        let cases = self.experiment.suite.cases.len();
        let repeats = self.experiment.repetitions as usize;
        for (i, t) in self.trials.iter().enumerate() {
            if t.index != i
                || t.candidate >= self.candidates.len()
                || t.case >= self.experiment.suite.cases.len()
                || t.assignment != trial_id(&self.id, i)
                || t.candidate != i / (cases * repeats)
                || t.case != i % cases
                || t.repetition as usize != (i / cases) % repeats
                || t.result.as_ref().is_some_and(|s| s.len() > 4096)
                || t.review_note.as_ref().is_some_and(|s| s.len() > 2048)
                || t.started_ms.zip(t.finished_ms).is_some_and(|(a, b)| a > b)
                || t.cost_usd.is_some_and(|x| !x.is_finite() || x < 0.0)
            {
                return Err(invalid("This test record is incomplete."));
            }
        }
        Ok(())
    }
    pub fn complete(&self) -> bool {
        self.trials.iter().all(|t| t.outcome.finished())
    }
    pub fn counts(&self, candidate: usize) -> Counts {
        let mut c = Counts::default();
        for t in self.trials.iter().filter(|t| t.candidate == candidate) {
            c.planned += 1;
            match t.outcome {
                Outcome::Passed => c.passed += 1,
                Outcome::Failed => c.failed += 1,
                Outcome::Pending | Outcome::Running => c.pending += 1,
                Outcome::NeedsReview => c.review += 1,
                _ => c.incomplete += 1,
            }
        }
        c
    }
    pub fn comparison_eligible(&self) -> bool {
        self.candidates.len() == 2
            && self.complete()
            && self.trials.iter().all(|t| {
                matches!(t.outcome, Outcome::Passed | Outcome::Failed)
                    && t.evidence.as_ref().is_some_and(|e| {
                        valid_id(&e.source_sha256)
                            && valid_id(&e.context_sha256)
                            && valid_id(&e.runtime_version)
                    })
            })
            && self.trials.windows(2).all(|p| {
                p[0].evidence
                    .as_ref()
                    .map(|e| (&e.source_sha256, &e.context_sha256))
                    == p[1]
                        .evidence
                        .as_ref()
                        .map(|e| (&e.source_sha256, &e.context_sha256))
            })
            && (0..2).all(|c| {
                let versions: BTreeSet<_> = self
                    .trials
                    .iter()
                    .filter(|t| t.candidate == c)
                    .filter_map(|t| t.evidence.as_ref().map(|e| &e.runtime_version))
                    .collect();
                versions.len() == 1
            })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub planned: usize,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    pub review: usize,
    pub incomplete: usize,
}
pub fn trial_id(run: &str, index: usize) -> String {
    lab_kit::digest(&json!({"run":run,"trial":index}))
}
pub fn starter() -> Experiment {
    Experiment {
        title: "New experiment".into(),
        draft: Draft {
            spec: AgentSpec {
                name: "My agent".into(),
                description: String::new(),
                instructions: "Answer the assigned task.".into(),
                runtime: "codex".into(),
                model: None,
                access: agent_contract::Access::ReadOnly,
            },
            source: None,
            components: vec![],
        },
        suite: Suite {
            title: "My tests".into(),
            partition: Partition::Development,
            cases: vec![Case {
                id: "case-1".into(),
                title: "First test".into(),
                task: "Reply with HELLO.".into(),
                check: Check::Contains {
                    expected: "HELLO".into(),
                },
            }],
        },
        compare: None,
        host: "choose".into(),
        project: "choose".into(),
        repetitions: 1,
        max_seconds: 300,
    }
}
pub use lab_kit::{Reference, Version};
