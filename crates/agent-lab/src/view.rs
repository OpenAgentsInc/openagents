//! One workbench form and result card for native and web adapters.
use super::*;
use crate::api::Host;
use rust_native::{Axis, Element, Node, TextRole, View};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Action(pub String);
#[derive(Clone, Serialize, Deserialize)]
pub struct Workbench {
    pub instance: String,
    pub revision: u64,
    pub draft: Experiment,
    pub document: Option<String>,
    pub base: u64,
    pub dirty: bool,
    pub documents: Vec<Document>,
    pub runs: Vec<Run>,
    pub selected_run: Option<String>,
    pub agents: Vec<AgentDefinition>,
    pub hosts: Vec<Host>,
    pub notice: Option<String>,
    pub busy: bool,
    pub pending: bool,
    pub inputs: std::collections::BTreeMap<String, String>,
    pub review_notes: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Command {
    Retry,
    Refresh,
    Load(String),
    Save,
    Start,
    Promote,
    Stop(String),
    Archive(String),
    Review {
        run: String,
        trial: usize,
        passed: bool,
    },
    Session(String),
}
fn node(key: impl Into<String>, element: Element<Action>) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element,
    }
}
fn text(key: impl Into<String>, value: impl Into<String>) -> Node<Action> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role: TextRole::Body,
        },
    )
}
fn button(key: impl Into<String>, label: impl Into<String>, enabled: bool) -> Node<Action> {
    let key = key.into();
    node(
        key.clone(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent: Action(key),
        },
    )
}
fn field(
    key: impl Into<String>,
    label: impl Into<String>,
    value: impl Into<String>,
    max: usize,
    multiline: bool,
    enabled: bool,
) -> Node<Action> {
    let key = key.into();
    node(
        key.clone(),
        Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: String::new(),
            secret: false,
            multiline,
            enabled,
            max_bytes: max,
            on_change: Action(key),
        },
    )
}
impl Workbench {
    pub fn new(instance: String) -> Self {
        Self {
            instance,
            revision: 1,
            draft: starter(),
            document: None,
            base: 0,
            dirty: true,
            documents: vec![],
            runs: vec![],
            selected_run: None,
            agents: vec![],
            hosts: vec![],
            notice: None,
            busy: false,
            pending: false,
            inputs: Default::default(),
            review_notes: Default::default(),
        }
    }
    pub fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1)
    }
    pub fn view(&self) -> View<Action> {
        let enabled = !self.busy && !self.pending;
        let mut n = vec![
            text("heading", "Agent Lab"),
            button("refresh", "Refresh", enabled),
            button("new", "New experiment", enabled),
        ];
        if let Some(notice) = &self.notice {
            n.push(text("notice", notice))
        }
        if self.pending {
            n.push(button("retry", "Retry saved change", !self.busy));
        }
        for d in &self.documents {
            if let Some(v) = d.versions.last() {
                n.push(button(
                    format!("open:{}", d.id),
                    v.content
                        .pointer("/experiment/title")
                        .and_then(Value::as_str)
                        .unwrap_or("Experiment"),
                    enabled,
                ));
            }
        }
        n.push(field(
            "title",
            "Experiment name",
            &self.draft.title,
            100,
            false,
            enabled,
        ));
        for a in &self.agents {
            n.push(button(
                format!("agent:{}", a.id),
                format!("Start from {}", a.spec.name),
                enabled,
            ));
        }
        n.extend([
            field(
                "name",
                "Agent name",
                &self.draft.draft.spec.name,
                80,
                false,
                enabled,
            ),
            field(
                "instructions",
                "Instructions",
                &self.draft.draft.spec.instructions,
                16384,
                true,
                enabled,
            ),
        ]);
        for (id, label) in [
            ("codex", "Codex"),
            ("claude-code", "Claude Code"),
            ("sdk", "Installed Rust runner"),
        ] {
            n.push(button(
                format!("runtime:{id}"),
                format!(
                    "{}{}",
                    if self.draft.draft.spec.runtime == id {
                        "✓ "
                    } else {
                        ""
                    },
                    label
                ),
                enabled,
            ));
        }
        n.push(text("computer-label", "Run tests on"));
        for h in &self.hosts {
            n.push(button(
                format!("host:{}", h.id),
                format!(
                    "{}{}",
                    if self.draft.host == h.id { "✓ " } else { "" },
                    h.name
                ),
                enabled,
            ));
        }
        if let Some(h) = self.hosts.iter().find(|h| h.id == self.draft.host) {
            for p in &h.projects {
                n.push(button(
                    format!("project:{}", p.id),
                    format!(
                        "{}{}",
                        if self.draft.project == p.id {
                            "✓ "
                        } else {
                            ""
                        },
                        p.name
                    ),
                    enabled,
                ));
            }
            if self.draft.draft.spec.runtime == "sdk" {
                for runner in &h.runners {
                    n.push(button(
                        format!("runner:{runner}"),
                        format!(
                            "{}{}",
                            if self.draft.draft.spec.model.as_ref() == Some(runner) {
                                "✓ "
                            } else {
                                ""
                            },
                            runner
                        ),
                        enabled,
                    ));
                }
            }
            for t in h.tools.iter().filter(|t| t.project == self.draft.project) {
                n.push(button(
                    format!("tool:{}", t.id),
                    format!("Use {} reference", t.name),
                    enabled,
                ));
            }
        } else {
            n.push(text(
                "no-computer",
                "Open the updated OpenAgents app on your computer to connect it.",
            ));
        }
        if self.draft.draft.spec.runtime != "sdk" {
            n.push(field(
                "model",
                "Model (leave blank for default)",
                self.draft.draft.spec.model.as_deref().unwrap_or_default(),
                256,
                false,
                enabled,
            ));
        }
        if self.draft.draft.spec.runtime == "codex" {
            n.push(button("access-read", "Read only", enabled));
            n.push(button(
                "access-write",
                "Allow edits in the test copy",
                enabled,
            ));
        }
        n.push(field(
            "guidance",
            "Extra guidance",
            self.component(false),
            8192,
            true,
            enabled,
        ));
        n.push(field(
            "memory",
            "Reference notes",
            self.component(true),
            8192,
            true,
            enabled,
        ));
        for (i, c) in self.draft.suite.cases.iter().enumerate() {
            n.push(field(
                format!("case-title:{i}"),
                format!("Test {} name", i + 1),
                &c.title,
                100,
                false,
                enabled,
            ));
            n.push(field(
                format!("task:{i}"),
                "Task",
                &c.task,
                16384,
                true,
                enabled,
            ));
            for (id, label) in [
                ("contains", "Contains text"),
                ("exact", "Exact reply"),
                ("json", "JSON value"),
                ("review", "Human review"),
            ] {
                n.push(button(format!("check:{i}:{id}"), label, enabled));
            }
            let expected = self
                .inputs
                .get(&format!("expected:{i}"))
                .cloned()
                .unwrap_or_else(|| match &c.check {
                    Check::Contains { expected } | Check::Exact { expected } => expected.clone(),
                    Check::Review { criteria } => criteria.clone(),
                    Check::Json { expected, .. } => expected.to_string(),
                });
            n.push(field(
                format!("expected:{i}"),
                if matches!(c.check, Check::Review { .. }) {
                    "Review criteria"
                } else {
                    "Expected result"
                },
                expected,
                4096,
                true,
                enabled,
            ));
            if let Check::Json { pointer, .. } = &c.check {
                n.push(field(
                    format!("pointer:{i}"),
                    "JSON field path",
                    pointer,
                    256,
                    false,
                    enabled,
                ));
            }
            if self.draft.suite.cases.len() > 1 {
                n.push(button(format!("remove:{i}"), "Remove test", enabled));
            }
        }
        if self.draft.suite.cases.len() < 8 {
            n.push(button("add-case", "Add test", enabled));
        }
        n.push(text(
            "repeat-label",
            format!("{} runs per test", self.draft.repetitions),
        ));
        n.push(button(
            "repeat-less",
            "Fewer runs",
            enabled && self.draft.repetitions > 1,
        ));
        n.push(button(
            "repeat-more",
            "More runs",
            enabled && self.draft.repetitions < 4,
        ));
        n.push(field(
            "seconds",
            "Time limit in seconds",
            self.inputs
                .get("seconds")
                .cloned()
                .unwrap_or_else(|| self.draft.max_seconds.to_string()),
            4,
            false,
            enabled,
        ));
        n.push(text(
            "partition",
            match self.draft.suite.partition {
                Partition::Development => "Development tests",
                Partition::Confirmation => "Final confirmation tests",
            },
        ));
        n.push(button("development", "Use for development", enabled));
        n.push(button(
            "confirmation",
            "Use for final confirmation",
            enabled,
        ));
        n.push(button("compare-none", "Test this candidate only", enabled));
        for a in &self.agents {
            n.push(button(
                format!("compare:{}", a.id),
                format!("Compare with {}", a.spec.name),
                enabled,
            ));
        }
        n.push(button(
            "save",
            "Save experiment",
            enabled
                && self.draft.validate().is_ok()
                && self.inputs.iter().all(|(k, v)| {
                    !k.starts_with("expected:")
                        || !matches!(
                            self.draft
                                .suite
                                .cases
                                .get(k[9..].parse::<usize>().unwrap_or(usize::MAX))
                                .map(|c| &c.check),
                            Some(Check::Json { .. })
                        )
                        || serde_json::from_str::<Value>(v).is_ok()
                }),
        ));
        if self.document.is_some() && !self.dirty {
            n.push(button(
                "start",
                "Run tests",
                enabled
                    && self.hosts.iter().any(|h| {
                        h.id == self.draft.host
                            && h.projects.iter().any(|p| p.id == self.draft.project)
                    }),
            ));
            n.push(button(
                "promote",
                "Use agent",
                enabled && self.draft.draft.agent_spec().is_ok(),
            ));
        }
        if let Some(d) = self
            .documents
            .iter()
            .find(|d| Some(&d.id) == self.document.as_ref())
        {
            for v in &d.versions {
                n.push(button(
                    format!("version:{}", v.number),
                    format!("Open version {}: {}", v.number, v.summary),
                    enabled,
                ));
            }
        }
        for r in &self.runs {
            n.push(button(
                format!("run:{}", r.id),
                format!(
                    "{} · {}",
                    r.experiment.title,
                    if r.complete() {
                        "Finished"
                    } else {
                        "In progress"
                    }
                ),
                enabled,
            ));
        }
        if let Some(r) = self
            .runs
            .iter()
            .find(|r| Some(&r.id) == self.selected_run.as_ref())
        {
            for (i, candidate) in r.candidates.iter().enumerate() {
                let c = r.counts(i);
                n.push(text(format!("counts:{i}"),format!("{}: {} of {} passed; {} failed, {} pending, {} need review, {} incomplete.",candidate.spec.name,c.passed,c.planned,c.failed,c.pending,c.review,c.incomplete)));
            }
            n.push(text(
                "cost",
                format!(
                    "Elapsed: {} seconds · Cost: {}",
                    r.trials
                        .iter()
                        .filter_map(|t| t.started_ms.zip(t.finished_ms))
                        .map(|(a, b)| b.saturating_sub(a) / 1000)
                        .sum::<u64>(),
                    if r.trials.iter().all(|t| t.cost_usd.is_some()) {
                        format!(
                            "${:.4}",
                            r.trials.iter().filter_map(|t| t.cost_usd).sum::<f64>()
                        )
                    } else {
                        "Unknown".into()
                    }
                ),
            ));
            if r.candidates.len() == 2 {
                n.push(text(
                    "comparison",
                    if r.comparison_eligible() {
                        "Both candidates used matching test inputs."
                    } else {
                        "Comparison needs complete results from matching test inputs."
                    },
                ));
            }
            if !r.complete() {
                n.push(button(
                    format!("stop:{}", r.id),
                    "Stop tests",
                    enabled && !r.stopping,
                ));
            } else {
                n.push(button(
                    format!("archive:{}", r.id),
                    "Archive experiment run",
                    enabled,
                ));
            }
            for t in &r.trials {
                n.push(text(
                    format!("result:{}", t.index),
                    format!(
                        "{} · {} · {}",
                        r.candidates[t.candidate].spec.name,
                        r.experiment.suite.cases[t.case].title,
                        outcome(t.outcome)
                    ),
                ));
                if let Some(result) = &t.result {
                    n.push(text(
                        format!("answer:{}", t.index),
                        result.chars().take(600).collect::<String>(),
                    ));
                }
                if let Some(chat) = &t.chat {
                    n.push(button(format!("session:{chat}"), "Open task", enabled));
                }
                if t.outcome == Outcome::NeedsReview {
                    let key = format!("review-note:{}:{}", r.id, t.index);
                    n.push(field(
                        &key,
                        "Review note",
                        self.review_notes.get(&key).cloned().unwrap_or_default(),
                        2048,
                        true,
                        enabled,
                    ));
                    n.push(button(
                        format!("review:{}:{}:pass", r.id, t.index),
                        "Pass this test",
                        enabled,
                    ));
                    n.push(button(
                        format!("review:{}:{}:fail", r.id, t.index),
                        "Fail this test",
                        enabled,
                    ));
                }
            }
        }
        View::new_v3(
            &self.instance,
            self.revision,
            node(
                "agent-lab",
                Element::Stack {
                    axis: Axis::Vertical,
                    children: n,
                },
            ),
        )
    }
    fn component(&self, memory: bool) -> String {
        self.draft
            .draft
            .components
            .iter()
            .find_map(|c| match c {
                Component::Skill { text, .. } if !memory => Some(text.clone()),
                Component::Memory { text, .. } if memory => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
    pub fn field(&mut self, key: &str, value: String) -> Result<(), String> {
        match key {
            "title" => self.draft.title = value,
            "name" => self.draft.draft.spec.name = value,
            "instructions" => self.draft.draft.spec.instructions = value,
            "model" => self.draft.draft.spec.model = (!value.trim().is_empty()).then_some(value),
            "seconds" => {
                self.inputs.insert(key.into(), value.clone());
                self.draft.max_seconds = value.parse().unwrap_or(0);
            }
            "guidance" | "memory" => {
                self.draft.draft.components.retain(|c| {
                    !matches!(
                        (key, c),
                        ("guidance", Component::Skill { .. })
                            | ("memory", Component::Memory { .. })
                    )
                });
                if !value.is_empty() {
                    let name = if key == "memory" {
                        "Reference notes"
                    } else {
                        "Extra guidance"
                    }
                    .to_string();
                    let sha256 = lab_kit::digest(&json!({"name":name,"text":value}));
                    self.draft.draft.components.push(if key == "memory" {
                        Component::Memory {
                            name,
                            text: value,
                            sha256,
                        }
                    } else {
                        Component::Skill {
                            name,
                            text: value,
                            sha256,
                        }
                    });
                }
            }
            _ if key.starts_with("review-note:") => {
                self.review_notes.insert(key.into(), value);
                self.bump();
                return Ok(());
            }
            _ => {
                let (kind, index) = key.split_once(':').ok_or("Choose an editable field.")?;
                let i: usize = index.parse().map_err(|_| "Choose a test.")?;
                let c = self.draft.suite.cases.get_mut(i).ok_or("Choose a test.")?;
                match kind {
                    "case-title" => c.title = value,
                    "task" => c.task = value,
                    "pointer" => {
                        if let Check::Json { pointer, .. } = &mut c.check {
                            *pointer = value
                        }
                    }
                    "expected" => match &mut c.check {
                        Check::Contains { expected } | Check::Exact { expected } => {
                            *expected = value
                        }
                        Check::Review { criteria } => *criteria = value,
                        Check::Json { expected, .. } => {
                            self.inputs.insert(key.into(), value.clone());
                            if let Ok(parsed) = serde_json::from_str(&value) {
                                *expected = parsed;
                            }
                        }
                    },
                    _ => return Err("Choose an editable field.".into()),
                }
            }
        }
        self.dirty = true;
        self.bump();
        Ok(())
    }
    pub fn action(&mut self, key: &str) -> Result<Option<Command>, String> {
        let mut command = None;
        match key {
            "retry" => command = Some(Command::Retry),
            "refresh" => command = Some(Command::Refresh),
            "new" => {
                self.draft = starter();
                self.document = None;
                self.base = 0;
                self.dirty = true;
                self.selected_run = None;
                self.inputs.clear();
            }
            "save" => command = Some(Command::Save),
            "start" => command = Some(Command::Start),
            "promote" => command = Some(Command::Promote),
            "access-read" => {
                self.draft.draft.spec.access = agent_contract::Access::ReadOnly;
                self.dirty = true
            }
            "access-write" => {
                self.draft.draft.spec.access = agent_contract::Access::WorkspaceWrite;
                self.dirty = true
            }
            "development" => {
                self.draft.suite.partition = Partition::Development;
                self.dirty = true
            }
            "confirmation" => {
                self.draft.suite.partition = Partition::Confirmation;
                self.dirty = true
            }
            "compare-none" => {
                self.draft.compare = None;
                self.dirty = true
            }
            "repeat-more" => {
                self.draft.repetitions = (self.draft.repetitions + 1).min(4);
                self.dirty = true
            }
            "repeat-less" => {
                self.draft.repetitions = self.draft.repetitions.saturating_sub(1).max(1);
                self.dirty = true
            }
            "add-case" => {
                if self.draft.suite.cases.len() >= 8 {
                    return Err("Use up to eight tests.".into());
                }
                let mut c = starter().suite.cases.remove(0);
                c.id = format!("case-{}", self.revision);
                c.title = format!("Test {}", self.draft.suite.cases.len() + 1);
                self.draft.suite.cases.push(c);
                self.dirty = true;
            }
            _ => {
                let parts: Vec<_> = key.split(':').collect();
                let id = *parts.get(1).ok_or("Choose an action.")?;
                match parts[0] {
                    "open" => command = Some(Command::Load(id.into())),
                    "run" => self.selected_run = Some(id.into()),
                    "session" => command = Some(Command::Session(id.into())),
                    "stop" => command = Some(Command::Stop(id.into())),
                    "archive" => command = Some(Command::Archive(id.into())),
                    "host" => {
                        self.draft.host = id.into();
                        self.draft.project = self
                            .hosts
                            .iter()
                            .find(|h| h.id == id)
                            .and_then(|h| h.projects.first())
                            .map(|p| p.id.clone())
                            .unwrap_or_else(|| "choose".into());
                        self.dirty = true;
                    }
                    "project" => {
                        self.draft.project = id.into();
                        self.dirty = true
                    }
                    "runner" => {
                        self.draft.draft.spec.model = Some(id.into());
                        self.dirty = true;
                    }
                    "runtime" => {
                        self.draft.draft.spec.runtime = id.into();
                        self.draft.draft.spec.model = None;
                        self.draft.draft.spec.access = if id == "codex" {
                            agent_contract::Access::ReadOnly
                        } else {
                            agent_contract::Access::HostDefault
                        };
                        self.dirty = true;
                    }
                    "agent" | "compare" => {
                        let a = self
                            .agents
                            .iter()
                            .find(|a| a.id == id)
                            .ok_or("This agent is no longer available.")?;
                        let pin = Pin {
                            id: a.id.clone(),
                            revision: a.revision,
                        };
                        if parts[0] == "agent" {
                            self.draft.draft.spec = a.spec.clone();
                            self.draft.draft.source = Some(pin)
                        } else {
                            self.draft.compare = Some(pin)
                        }
                        self.dirty = true;
                    }
                    "version" => {
                        let n = id.parse::<u64>().map_err(|_| "Choose a version.")?;
                        let v = self
                            .documents
                            .iter()
                            .find(|d| Some(&d.id) == self.document.as_ref())
                            .and_then(|d| d.versions.iter().find(|v| v.number == n))
                            .ok_or("This version is no longer available.")?;
                        if let Ok(Content::Experiment { experiment }) =
                            serde_json::from_value(v.content.clone())
                        {
                            self.draft = experiment;
                            self.base = n;
                            self.dirty = false;
                            self.inputs.clear();
                        }
                    }
                    "remove" => {
                        let i = id.parse::<usize>().map_err(|_| "Choose a test.")?;
                        if i >= self.draft.suite.cases.len() || self.draft.suite.cases.len() == 1 {
                            return Err("Keep at least one test.".into());
                        }
                        self.draft.suite.cases.remove(i);
                        self.dirty = true;
                    }
                    "check" => {
                        let c = self
                            .draft
                            .suite
                            .cases
                            .get_mut(id.parse::<usize>().map_err(|_| "Choose a test.")?)
                            .ok_or("Choose a test.")?;
                        c.check = match parts.get(2).copied() {
                            Some("contains") => Check::Contains {
                                expected: "HELLO".into(),
                            },
                            Some("exact") => Check::Exact {
                                expected: "HELLO".into(),
                            },
                            Some("json") => Check::Json {
                                pointer: "/success".into(),
                                expected: json!(true),
                            },
                            Some("review") => Check::Review {
                                criteria: "Check whether this completes the task.".into(),
                            },
                            _ => return Err("Choose a check.".into()),
                        };
                        self.inputs.remove(&format!("expected:{id}"));
                        self.dirty = true;
                    }
                    "review" => {
                        command = Some(Command::Review {
                            run: id.into(),
                            trial: parts
                                .get(2)
                                .and_then(|s| s.parse().ok())
                                .ok_or("Choose a test.")?,
                            passed: parts.get(3) == Some(&"pass"),
                        })
                    }
                    "tool" => {
                        let offer = self
                            .hosts
                            .iter()
                            .filter(|h| h.id == self.draft.host)
                            .flat_map(|h| &h.tools)
                            .find(|t| t.id == id && t.project == self.draft.project)
                            .ok_or("This reference is not available.")?;
                        self.draft.draft.components.push(Component::ToolContext {
                            binding: offer.id.clone(),
                            sha256: offer.sha256.clone(),
                        });
                        self.dirty = true;
                    }
                    _ => return Err("Choose an available action.".into()),
                }
            }
        }
        self.bump();
        Ok(command)
    }
}
pub fn outcome(o: Outcome) -> &'static str {
    match o {
        Outcome::Pending => "Waiting",
        Outcome::Running => "Running",
        Outcome::Passed => "Passed",
        Outcome::Failed => "Failed",
        Outcome::NeedsReview => "Needs review",
        Outcome::Stopped => "Stopped",
        Outcome::TimedOut => "Time limit reached",
        Outcome::Unavailable => "Could not start",
        Outcome::Unknown => "Result not confirmed",
    }
}
