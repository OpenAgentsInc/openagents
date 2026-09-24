//! The run's trajectory and its component invocations, appended as ATIF
//! steps when they happen.
//!
//! The judge, the generator, the shell, and the component runner each hold
//! a clone of one [`Recorder`], so the loop itself stays unaware of
//! recording. A recorder made with [`Recorder::durable`] also appends every
//! step to an `atif::log::Log`, which syncs each line before the call
//! returns: a killed episode leaves a readable prefix, and the bundle and
//! the ATIF trajectory are derived from that one log.
//!
//! # Invocations
//!
//! Every component invocation writes two events: a start, written before
//! the component does anything with effects, and an end. Each event is an
//! ATIF step whose `invocation` extension holds the record, in the
//! [`INVOCATION_SCHEMA`] shape:
//!
//! ```text
//! {"schema":"openagents.coder-one.invocation.v1","event":"start","id":"inv-3",
//!  "parent":"inv-1","component":"evidence.probes","name":"probe battery",
//!  "implementation":{"name":"…","digest":"…"},"input_digest":"…",
//!  "evidence_revision":0,"effects":true,"at":…}
//! {"schema":"openagents.coder-one.invocation.v1","event":"end","id":"inv-3",
//!  "component":"evidence.probes","outcome":"completed","output":{…},
//!  "cost":{"usd":0.0001,"provenance":"price_estimate"},"milliseconds":597,"at":…}
//! ```
//!
//! A start with no end is an invocation whose result is unknown. When the
//! start says `effects: true`, a restarted controller can't assume the
//! effect didn't happen. Two invocations of one component, such as two
//! concurrent Jev requests, differ by ID. Ordinary steps written while an
//! invocation is open carry its ID in the `invocation_id` extension, so a
//! Jev request, a generation, or a command is credited to the component
//! that made it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use atif::document::{Source, Step};
use atif::log::Log;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The schema every invocation event carries.
pub const INVOCATION_SCHEMA: &str = "openagents.coder-one.invocation.v1";

/// The step extension that holds an invocation event.
pub const EVENT_KEY: &str = "invocation";

/// The step extension that credits an ordinary step to an invocation.
pub const ATTRIBUTION_KEY: &str = "invocation_id";

/// One concrete way to perform a component: a name, and the digest of the
/// component ID, that name, and every parameter. Changing a threshold, a
/// cap, or a question's wording makes a new digest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub digest: String,
}

impl Implementation {
    /// The implementation `name` of `component` with `parameters`.
    #[must_use]
    pub fn new(component: &str, name: &str, parameters: &Value) -> Self {
        Implementation {
            name: name.to_string(),
            digest: atif::digest(&json!({
                "component": component,
                "name": name,
                "parameters": parameters,
            })),
        }
    }
}

/// What an invocation cost, and where the number came from. An unknown
/// cost is `usd: None`, never zero.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub usd: Option<f64>,
    /// `provider_reported`, `price_estimate`, `cli_list_price`,
    /// `cli_reported`, `recorded_replay`, `none`, or `unknown`.
    pub provenance: String,
}

impl Cost {
    /// A cost the host knows is zero: no inference ran.
    #[must_use]
    pub fn none() -> Self {
        Cost {
            usd: Some(0.0),
            provenance: "none".to_string(),
        }
    }

    /// A cost nobody reported.
    #[must_use]
    pub fn unknown() -> Self {
        Cost {
            usd: None,
            provenance: "unknown".to_string(),
        }
    }
}

/// How an invocation ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It did its job.
    Completed,
    /// It ran and failed.
    Failed,
    /// It chose not to run, or had nothing to do.
    Skipped,
}

impl Outcome {
    /// The outcome as the record spells it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Failed => "failed",
            Outcome::Skipped => "skipped",
        }
    }
}

/// What starts an invocation.
#[derive(Clone, Debug)]
pub struct Start {
    /// The component ID, such as `evidence.pack`.
    pub component: String,
    /// A short label for this invocation, such as `jev_probe`.
    pub name: Option<String>,
    pub implementation: Implementation,
    /// The digest of the input the invocation reads, when it has one.
    pub input_digest: Option<String>,
    /// Whether the invocation runs a process, an executor, or anything
    /// else that changes the world.
    pub effects: bool,
    /// The declared effect class of a typed host operation: `observe`,
    /// `write`, or `install`.
    pub effect_class: Option<String>,
    /// The parent invocation; the innermost open one when `None`.
    pub parent: Option<String>,
}

impl Start {
    /// A start for `component` with `implementation`.
    #[must_use]
    pub fn new(component: &str, implementation: Implementation) -> Self {
        Start {
            component: component.to_string(),
            name: None,
            implementation,
            input_digest: None,
            effects: false,
            effect_class: None,
            parent: None,
        }
    }

    /// Names this invocation.
    #[must_use]
    pub fn named(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Records the digest of the input.
    #[must_use]
    pub fn reading(mut self, input: &Value) -> Self {
        self.input_digest = Some(atif::digest(input));
        self
    }

    /// Records an input digest computed elsewhere.
    #[must_use]
    pub fn reading_digest(mut self, digest: String) -> Self {
        self.input_digest = Some(digest);
        self
    }

    /// Marks the invocation as one with effects.
    #[must_use]
    pub fn with_effects(mut self) -> Self {
        self.effects = true;
        self
    }

    /// Declares a typed host operation's effect class. Any class but
    /// `observe` also marks the invocation as one with effects.
    #[must_use]
    pub fn effect(mut self, class: &str) -> Self {
        self.effects = class != "observe";
        self.effect_class = Some(class.to_string());
        self
    }

    /// Sets the parent explicitly.
    #[must_use]
    pub fn under(mut self, parent: Option<&str>) -> Self {
        self.parent = parent.map(str::to_string);
        self
    }
}

/// What ends an invocation.
#[derive(Clone, Debug)]
pub struct Finish {
    pub outcome: Outcome,
    /// The digest of the output, when it has one.
    pub output_digest: Option<String>,
    /// A short account of the output, or where it lives.
    pub output: Value,
    /// Leaf invocations carry their own cost; a parent's is the sum of its
    /// children's, so it records none.
    pub cost: Option<Cost>,
}

impl Finish {
    /// An ending with `outcome` and nothing else.
    #[must_use]
    pub fn new(outcome: Outcome) -> Self {
        Finish {
            outcome,
            output_digest: None,
            output: Value::Null,
            cost: None,
        }
    }

    /// Records the output and its digest.
    #[must_use]
    pub fn output(mut self, output: Value) -> Self {
        self.output_digest = Some(atif::digest(&output));
        self.output = output;
        self
    }

    /// Records a short account of the output without digesting it.
    #[must_use]
    pub fn summary(mut self, summary: Value) -> Self {
        self.output = summary;
        self
    }

    /// Records the cost.
    #[must_use]
    pub fn cost(mut self, cost: Cost) -> Self {
        self.cost = Some(cost);
        self
    }
}

#[derive(Default)]
struct Inner {
    steps: Vec<Step>,
    log: Option<Log>,
    log_failed: bool,
    next: u64,
    /// Invocations entered with [`Recorder::enter`], innermost last. Steps
    /// pushed while one is open are credited to the innermost.
    stack: Vec<String>,
    /// Every open invocation: when it started and its component.
    open: BTreeMap<String, (u64, String)>,
    revision: u64,
    /// Who hears each step as it is recorded; see [`Recorder::watch`].
    watchers: Vec<Watcher>,
}

/// Hears one step as it is recorded.
type Watcher = Rc<dyn Fn(&Step)>;

impl Inner {
    /// Records `step` and hands back who should hear it, so they hear it
    /// after the recorder's borrow ends.
    fn keep(&mut self, step: Step) -> Option<(Vec<Watcher>, Step)> {
        self.append(&step);
        let heard = (!self.watchers.is_empty()).then(|| (self.watchers.clone(), step.clone()));
        self.steps.push(step);
        heard
    }
}

/// Tells each watcher about the step [`Inner::keep`] kept.
fn tell(heard: Option<(Vec<Watcher>, Step)>) {
    if let Some((watchers, step)) = heard {
        for watcher in &watchers {
            watcher(&step);
        }
    }
}

/// A shared, append-only list of trajectory steps, optionally durable.
#[derive(Clone, Default)]
pub struct Recorder(Rc<RefCell<Inner>>);

impl Recorder {
    /// A recorder that also appends every step to `log`.
    #[must_use]
    pub fn durable(log: Log) -> Self {
        let recorder = Recorder::default();
        recorder.0.borrow_mut().log = Some(log);
        recorder
    }

    /// Where the durable log is, when there is one.
    #[must_use]
    pub fn log_path(&self) -> Option<PathBuf> {
        self.0
            .borrow()
            .log
            .as_ref()
            .map(|log| log.path().to_path_buf())
    }

    /// Calls `watcher` with every step recorded from now on, as it is
    /// recorded: ordinary steps and invocation events alike. A host that
    /// shows a run live, such as Coder Terminal, watches instead of
    /// polling. The watcher must not record into this recorder.
    pub fn watch(&self, watcher: impl Fn(&Step) + 'static) {
        self.0.borrow_mut().watchers.push(Rc::new(watcher));
    }

    /// Appends one step, credited to the innermost open invocation.
    pub fn push(&self, mut step: Step) {
        let mut inner = self.0.borrow_mut();
        if !step.extensions.contains_key(EVENT_KEY)
            && !step.extensions.contains_key(ATTRIBUTION_KEY)
            && let Some(id) = inner.stack.last()
        {
            step.extensions
                .insert(ATTRIBUTION_KEY.to_string(), json!(id));
        }
        let heard = inner.keep(step);
        drop(inner);
        tell(heard);
    }

    /// Every step so far, in order.
    #[must_use]
    pub fn steps(&self) -> Vec<Step> {
        self.0.borrow().steps.clone()
    }

    /// The evidence revision: how many times a component has changed the
    /// evidence an invocation reads.
    #[must_use]
    pub fn evidence_revision(&self) -> u64 {
        self.0.borrow().revision
    }

    /// Notes that a component changed the evidence.
    pub fn revise(&self) {
        self.0.borrow_mut().revision += 1;
    }

    /// Starts an invocation and writes its start event, without making it
    /// the parent of later steps. Use this for concurrent invocations.
    pub fn begin(&self, start: Start) -> String {
        let at = atif::now_ms();
        let mut inner = self.0.borrow_mut();
        inner.next += 1;
        let id = format!("inv-{}", inner.next);
        let parent = start.parent.clone().or_else(|| inner.stack.last().cloned());
        let record = json!({
            "schema": INVOCATION_SCHEMA,
            "event": "start",
            "id": id,
            "parent": parent,
            "component": start.component,
            "name": start.name,
            "implementation": start.implementation,
            "input_digest": start.input_digest,
            "evidence_revision": inner.revision,
            "effects": start.effects,
            "effect_class": start.effect_class,
            "at": at,
        });
        let mut step = Step::said(
            Source::System,
            &format!(
                "invocation {id} started: {}{}",
                start.component,
                start
                    .name
                    .as_deref()
                    .map_or(String::new(), |name| format!(" ({name})"))
            ),
        )
        .noting(EVENT_KEY, record);
        step.at = at;
        inner.open.insert(id.clone(), (at, start.component));
        let heard = inner.keep(step);
        drop(inner);
        tell(heard);
        id
    }

    /// Starts an invocation and makes it the parent of later steps and
    /// invocations until it ends.
    pub fn enter(&self, start: Start) -> String {
        let id = self.begin(start);
        self.0.borrow_mut().stack.push(id.clone());
        id
    }

    /// Ends invocation `id` and writes its end event.
    pub fn end(&self, id: &str, finish: Finish) {
        let at = atif::now_ms();
        let mut inner = self.0.borrow_mut();
        inner.stack.retain(|open| open != id);
        let (started, component) = inner.open.remove(id).unwrap_or((at, String::new()));
        let milliseconds = at.saturating_sub(started);
        let record = json!({
            "schema": INVOCATION_SCHEMA,
            "event": "end",
            "id": id,
            "component": component,
            "outcome": finish.outcome.word(),
            "output": { "digest": finish.output_digest, "summary": finish.output },
            "cost": finish.cost,
            "evidence_revision": inner.revision,
            "milliseconds": milliseconds,
            "at": at,
        });
        let mut step = Step::said(
            Source::System,
            &format!(
                "invocation {id} ended: {component} {} in {milliseconds} ms",
                finish.outcome.word()
            ),
        )
        .noting(EVENT_KEY, record)
        .taking(milliseconds);
        step.at = at;
        let heard = inner.keep(step);
        drop(inner);
        tell(heard);
    }

    /// Writes the log's closing record. Steps after this stay in memory
    /// only.
    pub fn finish(&self, state: &str) {
        let mut inner = self.0.borrow_mut();
        if let Some(log) = inner.log.as_mut()
            && let Err(error) = log.finish(state)
        {
            eprintln!("coder-one: cannot close the invocation log: {error}");
        }
    }
}

impl Inner {
    fn append(&mut self, step: &Step) {
        let Some(log) = self.log.as_mut() else {
            return;
        };
        if let Err(error) = log.append(step)
            && !self.log_failed
        {
            // A trace is evidence about the episode, not part of it: say
            // so once and carry on.
            self.log_failed = true;
            eprintln!(
                "coder-one: cannot append to {}: {error}",
                log.path().display()
            );
        }
    }
}

/// Writes `bytes` to `path` through a temporary file and a rename, so a
/// reader sees the old file or the new one and never part of either.
///
/// # Errors
///
/// Returns a message naming the path when a write, sync, or rename fails.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let written = std::fs::File::create(&temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    if let Err(error) = written.and_then(|()| std::fs::rename(&temporary, path)) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("cannot write {}: {error}", path.display()));
    }
    Ok(())
}

/// One invocation read back from steps: its start, and its end when one
/// was written.
#[derive(Clone, Debug, PartialEq)]
pub struct Invocation {
    pub id: String,
    pub parent: Option<String>,
    pub component: String,
    pub name: Option<String>,
    pub implementation: Option<Implementation>,
    pub input_digest: Option<String>,
    pub evidence_revision: Option<u64>,
    pub effects: bool,
    pub started: u64,
    /// The end event, or `None` when the invocation never ended.
    pub ended: Option<Value>,
}

impl Invocation {
    /// `completed`, `failed`, `skipped`, or `unknown` when no end was
    /// written.
    #[must_use]
    pub fn outcome(&self) -> &str {
        self.ended
            .as_ref()
            .and_then(|end| end.get("outcome"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    }
}

/// Every invocation in `steps`, in start order. An end with no start is
/// ignored: it can't be attributed.
#[must_use]
pub fn invocations(steps: &[Step]) -> Vec<Invocation> {
    let mut out: Vec<Invocation> = Vec::new();
    for step in steps {
        let Some(record) = step.extensions.get(EVENT_KEY) else {
            continue;
        };
        if record.get("schema").and_then(Value::as_str) != Some(INVOCATION_SCHEMA) {
            continue;
        }
        let text = |key: &str| record.get(key).and_then(Value::as_str).map(str::to_string);
        match record.get("event").and_then(Value::as_str) {
            Some("start") => out.push(Invocation {
                id: text("id").unwrap_or_default(),
                parent: text("parent"),
                component: text("component").unwrap_or_default(),
                name: text("name"),
                implementation: record
                    .get("implementation")
                    .cloned()
                    .and_then(|value| serde_json::from_value(value).ok()),
                input_digest: text("input_digest"),
                evidence_revision: record.get("evidence_revision").and_then(Value::as_u64),
                effects: record.get("effects").and_then(Value::as_bool) == Some(true),
                started: record.get("at").and_then(Value::as_u64).unwrap_or(step.at),
                ended: None,
            }),
            Some("end") => {
                if let Some(id) = text("id")
                    && let Some(invocation) = out.iter_mut().find(|i| i.id == id)
                {
                    invocation.ended = Some(record.clone());
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use atif::document::Session;

    fn session(id: &str) -> Session {
        Session::opening(id, "free", "https://example.test", "/tmp/work", "test")
    }

    fn implementation() -> Implementation {
        Implementation::new("evidence.probes", "battery", &json!({ "threshold": 0.5 }))
    }

    #[test]
    fn a_watcher_hears_every_step_as_it_is_recorded() {
        let recorder = Recorder::default();
        recorder.push(Step::said(Source::System, "before"));
        let heard = Rc::new(RefCell::new(Vec::new()));
        let into = heard.clone();
        let reader = recorder.clone();
        recorder.watch(move |step| {
            // The recorder is readable while a watcher runs.
            let _ = reader.steps().len();
            into.borrow_mut().push(step.message.clone());
        });
        let id = recorder.enter(Start::new("evidence.probes", implementation()));
        recorder.push(Step::said(Source::System, "during"));
        recorder.end(&id, Finish::new(Outcome::Completed));
        let heard = heard.borrow();
        assert_eq!(heard.len(), 3, "{heard:?}");
        assert!(heard[0].starts_with("invocation inv-1 started"));
        assert_eq!(heard[1], "during");
        assert!(heard[2].starts_with("invocation inv-1 ended"));
    }

    #[test]
    fn a_changed_parameter_changes_the_implementation_digest() {
        let a = Implementation::new("evidence.probes", "battery", &json!({ "threshold": 0.5 }));
        let b = Implementation::new("evidence.probes", "battery", &json!({ "threshold": 0.6 }));
        assert_ne!(a.digest, b.digest);
        assert_eq!(a, implementation());
    }

    #[test]
    fn invocations_nest_and_concurrent_ones_differ_by_id() {
        let recorder = Recorder::default();
        let root = recorder.enter(Start::new("episode", implementation()));
        let a = recorder.begin(Start::new("evidence.select", implementation()).named("jev_survey"));
        let b = recorder.begin(Start::new("evidence.select", implementation()).named("jev_survey"));
        recorder.push(Step::said(Source::Agent, "inside the root"));
        recorder.end(&b, Finish::new(Outcome::Completed));
        recorder.end(&a, Finish::new(Outcome::Failed).cost(Cost::unknown()));
        recorder.end(&root, Finish::new(Outcome::Completed));

        let found = invocations(&recorder.steps());
        assert_eq!(found.len(), 3);
        assert_ne!(found[1].id, found[2].id);
        assert_eq!(found[1].parent.as_deref(), Some(root.as_str()));
        assert_eq!(found[2].parent.as_deref(), Some(root.as_str()));
        assert_eq!(found[1].outcome(), "failed");
        assert_eq!(found[2].outcome(), "completed");
        let credited = recorder
            .steps()
            .into_iter()
            .find(|step| step.message == "inside the root")
            .unwrap();
        assert_eq!(credited.extensions[ATTRIBUTION_KEY], json!(root));
    }

    #[test]
    fn a_durable_recorder_leaves_a_readable_prefix_when_killed() {
        let dir = std::env::temp_dir().join(format!("coder-one-record-{}", atif::now_ms()));
        let path = dir.join("episode.atif.jsonl");
        let log = Log::create_at(&path, &session("killed")).unwrap();
        let recorder = Recorder::durable(log);
        let root = recorder.enter(Start::new("episode", implementation()));
        let child = recorder.enter(Start::new("exec.session", implementation()).with_effects());
        recorder.push(Step::said(Source::System, "delegating"));
        // The process dies here: nothing ends and nothing finishes. Append
        // half a record, as a write interrupted by the kill would.
        drop(recorder);
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(br#"{"record":"step","step":{"at":1,"sou"#)
            .unwrap();

        let read = atif::log::read(&path).unwrap();
        assert!(!read.ended());
        assert_eq!(read.faults.len(), 1);
        assert_eq!(read.faults[0].kind, atif::FaultKind::Torn);
        let found = invocations(&read.steps);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, root);
        assert_eq!(found[1].id, child);
        assert!(found[1].effects);
        assert_eq!(found[1].outcome(), "unknown");
        assert!(atif::log::read_whole(&path).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_finished_log_reads_whole_with_every_event() {
        let dir = std::env::temp_dir().join(format!("coder-one-whole-{}", atif::now_ms()));
        let path = dir.join("episode.atif.jsonl");
        let recorder = Recorder::durable(Log::create_at(&path, &session("whole")).unwrap());
        let root = recorder.enter(Start::new("episode", implementation()));
        recorder.end(
            &root,
            Finish::new(Outcome::Completed).output(json!({ "outcome": "delegated" })),
        );
        recorder.finish(atif::log::ENDED);
        let read = atif::log::read_whole(&path).unwrap();
        assert_eq!(read.steps.len(), recorder.steps().len());
        let found = invocations(&read.steps);
        assert_eq!(found[0].outcome(), "completed");
        let _ = std::fs::remove_dir_all(dir);
    }
}
