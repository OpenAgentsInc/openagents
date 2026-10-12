//! A managed-agent workflow run (`wrun_`).
//!
//! The session agent writes a plain JavaScript script. This crate runs it.
//! `agent` starts a thread through the host's runner. `pipeline` streams
//! items through stages. `parallel` waits for every thunk. Threads share the
//! session budget. A step whose prompt and options are unchanged is reused.
//! A run does not start another run.

mod engine;
mod script;

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub use script::DeclaredPhase;

/// Threads working at once in one run. Further `agent` calls wait.
pub const MAX_IN_FLIGHT: u32 = 64;
/// Threads one run may start over its life, unless the session set a lower cap.
pub const MAX_THREADS: u32 = 1_000;
/// Open runs in one session, paused runs included.
pub const MAX_OPEN_RUNS: u32 = 10;
/// A run's lifetime, unless the caller set a shorter one. A pause does not
/// extend it.
pub const DEFAULT_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);

const MAX_CACHED_SCRIPTS: usize = 32;

tokio::task_local! {
    static RUN_DEPTH: u32;
}

/// One thread the script asked for.
#[derive(Clone, Debug)]
pub struct ThreadCall {
    pub prompt: String,
    pub label: Option<String>,
    pub phase: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub agent_type: Option<String>,
    pub agent: Option<InlineAgent>,
    pub schema: Option<Value>,
    /// Set when the run is stopped or its lifetime has elapsed.
    pub cancel: Arc<AtomicBool>,
}

/// An agent the script defines for one thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineAgent {
    pub name: String,
    pub description: String,
    pub system: String,
}

/// What the thread finished with. `value: None` is a skip or a thread that
/// died. Either way the script sees `null`.
#[derive(Clone, Debug, PartialEq)]
pub struct ThreadResult {
    pub value: Option<Value>,
    pub cost_sats: u64,
}

impl ThreadResult {
    #[must_use]
    pub fn dead() -> Self {
        Self {
            value: None,
            cost_sats: 0,
        }
    }
}

/// Host function that runs one thread. Tests pass a stub. The harness passes
/// a model call. The future is polled on the run's own thread.
pub type ThreadRunner =
    Arc<dyn Fn(ThreadCall) -> Pin<Box<dyn Future<Output = ThreadResult> + Send>> + Send + Sync>;

/// What to run.
#[derive(Clone, Debug)]
pub struct RunRequest {
    pub script: String,
    pub args: Value,
    /// Lower than [`MAX_THREADS`] when the session set one. `None` uses the
    /// server cap.
    pub max_agents: Option<u32>,
    pub lifetime: Duration,
    pub inline_agents: bool,
    pub predefined_agents: Vec<String>,
    /// Set when the host asks the run to stop. The run's own lifetime and
    /// budget still apply.
    pub cancel: Option<Arc<AtomicBool>>,
}

impl RunRequest {
    #[must_use]
    pub fn new(script: impl Into<String>) -> Self {
        Self {
            script: script.into(),
            args: Value::Null,
            max_agents: None,
            lifetime: DEFAULT_LIFETIME,
            inline_agents: true,
            predefined_agents: Vec::new(),
            cancel: None,
        }
    }

    #[must_use]
    pub fn args(mut self, args: Value) -> Self {
        self.args = args;
        self
    }

    #[must_use]
    pub fn max_agents(mut self, max_agents: u32) -> Self {
        self.max_agents = Some(max_agents);
        self
    }

    #[must_use]
    pub fn lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }
}

/// How a finished run ended. `completed` means the script finished, not that
/// the work passed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndResult {
    pub kind: String,
    pub error_type: Option<String>,
    pub message: Option<String>,
}

impl EndResult {
    pub(crate) fn completed() -> Self {
        Self {
            kind: "completed".into(),
            error_type: None,
            message: None,
        }
    }

    pub(crate) fn stopped() -> Self {
        Self {
            kind: "stopped".into(),
            error_type: None,
            message: Some("The run was stopped.".into()),
        }
    }

    pub(crate) fn error(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: "error".into(),
            error_type: Some(kind.into()),
            message: Some(message.into()),
        }
    }
}

/// One start's result. A paused run stays open and keeps its id.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub id: Option<String>,
    /// `idle` when the run paused, `ended` when it finished or never started.
    pub status: String,
    pub result: Option<EndResult>,
    pub value: Value,
    pub events: Vec<Value>,
    pub notice: String,
    pub logs: Vec<String>,
}

impl Outcome {
    #[must_use]
    pub fn error_type(&self) -> Option<&str> {
        self.result
            .as_ref()
            .and_then(|result| result.error_type.as_deref())
    }

    fn ended(
        id: Option<String>,
        result: EndResult,
        value: Value,
        events: Vec<Value>,
        notice: String,
        logs: Vec<String>,
    ) -> Self {
        Self {
            id,
            status: "ended".into(),
            result: Some(result),
            value,
            events,
            notice,
            logs,
        }
    }
}

/// The session a run belongs to. Open runs share its budget and its step
/// store. Clone shares the same book.
#[derive(Clone)]
pub struct Session {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    book: Mutex<Book>,
    notify: tokio::sync::Notify,
}

struct Book {
    budget_total: Option<u64>,
    spent: u64,
    open: Vec<OpenRun>,
    caches: HashMap<String, HashMap<String, CacheEntry>>,
    cache_order: VecDeque<String>,
    next_id: u64,
}

struct OpenRun {
    id: String,
    script_key: String,
    args_key: String,
    name: String,
    #[allow(dead_code)]
    description: String,
    status: OpenStatus,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    halt: Halt,
    events: Vec<Value>,
    logs: Vec<String>,
    /// Threads this run has started. A reused step does not add to it, and a
    /// call that is not started does not either. The count does not pass the
    /// life cap; threads already started finish, and a later call that would
    /// pass the cap ends the run.
    started: u32,
    limit_hit: bool,
    drops_limit: u32,
    drops_budget: u32,
    #[allow(dead_code)]
    declared: Vec<DeclaredPhase>,
    phase_ids: HashMap<String, String>,
    open_phases: Vec<OpenPhase>,
    next_phase: u32,
    next_event: u32,
}

struct OpenPhase {
    id: String,
    started_event_id: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpenStatus {
    Running,
    Idle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Halt {
    None,
    Stopped,
    Timeout,
}

#[derive(Clone)]
enum CacheEntry {
    Inflight,
    Ready(ThreadResult),
}

pub(crate) struct Prepared {
    pub(crate) id: String,
    #[allow(dead_code)]
    pub(crate) name: String,
    pub(crate) script_key: String,
    pub(crate) body: String,
    pub(crate) args: Value,
    pub(crate) cap: u32,
    pub(crate) deadline: Instant,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) inline_agents: bool,
    pub(crate) predefined: HashSet<String>,
    #[allow(dead_code)]
    pub(crate) resumed: bool,
}

impl Session {
    #[must_use]
    pub fn new(budget_total_sats: Option<u64>) -> Self {
        Self {
            inner: Arc::new(SessionInner {
                book: Mutex::new(Book {
                    budget_total: budget_total_sats,
                    spent: 0,
                    open: Vec::new(),
                    caches: HashMap::new(),
                    cache_order: VecDeque::new(),
                    next_id: 1,
                }),
                notify: tokio::sync::Notify::new(),
            }),
        }
    }

    /// Raise, lower, or remove the session cap. `None` removes it.
    pub fn set_budget(&self, total_sats: Option<u64>) {
        lock(&self.inner.book).budget_total = total_sats;
    }

    #[must_use]
    pub fn spent(&self) -> u64 {
        lock(&self.inner.book).spent
    }

    #[must_use]
    pub fn budget_total(&self) -> Option<u64> {
        lock(&self.inner.book).budget_total
    }

    #[must_use]
    pub fn open_count(&self) -> usize {
        lock(&self.inner.book).open.len()
    }

    /// Stop one open run. A paused run ends immediately. A running run ends
    /// when its current thread returns.
    pub fn stop(&self, id: &str) -> bool {
        let mut book = lock(&self.inner.book);
        let Some(run) = book.open.iter_mut().find(|run| run.id == id) else {
            return false;
        };
        run.cancel.store(true, Ordering::Relaxed);
        if run.halt == Halt::None {
            run.halt = Halt::Stopped;
        }
        if run.status == OpenStatus::Idle {
            let run = book
                .open
                .iter()
                .position(|run| run.id == id)
                .map(|index| book.open.remove(index));
            if let Some(mut run) = run {
                let outcome = finish_run(&mut run, EndResult::stopped(), Value::Null);
                drop(outcome);
            }
        }
        drop(book);
        self.inner.notify.notify_waiters();
        true
    }

    /// Run the script. A nested call, made while this session is already
    /// inside a run on this task, is refused.
    pub async fn start(&self, request: RunRequest, runner: ThreadRunner) -> Outcome {
        if RUN_DEPTH.try_with(|depth| *depth).unwrap_or(0) > 0 {
            return refuse("program_error", "A run does not start another run.");
        }
        let prepared = match self.prepare(&request) {
            Ok(prepared) => prepared,
            Err(outcome) => return outcome,
        };
        let session = self.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let spawned = std::thread::Builder::new()
            .name("workflow-run".into())
            .spawn(move || {
                let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_time()
                        .build()
                        .expect("workflow runtime");
                    runtime.block_on(RUN_DEPTH.scope(1, engine::execute(session, prepared, runner)))
                }));
                let outcome =
                    outcome.unwrap_or_else(|_| refuse("unknown_error", "The run failed."));
                let _ = tx.send(outcome);
            });
        if spawned.is_err() {
            return refuse("unknown_error", "The run failed.");
        }
        rx.await
            .unwrap_or_else(|_| refuse("unknown_error", "The run failed."))
    }

    fn prepare(&self, request: &RunRequest) -> Result<Prepared, Outcome> {
        if request.script.len() > 256 * 1024 {
            return Err(refuse("program_error", "The script is too long."));
        }
        let parsed = match script::parse(&request.script) {
            Ok(parsed) => parsed,
            Err(message) => return Err(refuse("program_error", message)),
        };
        let cap = match request.max_agents {
            None => MAX_THREADS,
            Some(max) if (1..=MAX_THREADS).contains(&max) => max,
            Some(_) => {
                return Err(refuse(
                    "program_error",
                    "max_agents has to be from 1 to 1000.",
                ));
            }
        };
        let lifetime = request.lifetime.min(DEFAULT_LIFETIME);
        let script_key = script_key(&request.script);
        let args_key = serde_json::to_string(&request.args).unwrap_or_else(|_| "null".into());
        let mut book = lock(&self.inner.book);
        if let Some(existing) = book.open.iter_mut().find(|run| {
            run.status == OpenStatus::Idle
                && run.script_key == script_key
                && run.args_key == args_key
        }) {
            existing.status = OpenStatus::Running;
            existing.drops_budget = 0;
            existing.drops_limit = 0;
            existing.limit_hit = false;
            if let Some(flag) = &request.cancel {
                existing.cancel = Arc::clone(flag);
            }
            let id = existing.id.clone();
            let deadline = existing.deadline;
            let cancel = Arc::clone(&existing.cancel);
            let name = existing.name.clone();
            push_event(
                existing,
                "workflow_run.status_running",
                serde_json::Map::new(),
            );
            return Ok(Prepared {
                id,
                name,
                script_key,
                body: parsed.body,
                args: request.args.clone(),
                cap,
                deadline,
                cancel,
                inline_agents: request.inline_agents,
                predefined: request.predefined_agents.iter().cloned().collect(),
                resumed: true,
            });
        }
        if book.open.len() >= MAX_OPEN_RUNS as usize {
            return Err(refuse(
                "max_workflow_runs_error",
                "This session already has 10 workflow runs open.",
            ));
        }
        let id = format!("wrun_{:016x}", book.next_id);
        book.next_id += 1;
        let now = Instant::now();
        let deadline = now.checked_add(lifetime).unwrap_or(now);
        let mut run = OpenRun {
            id: id.clone(),
            script_key: script_key.clone(),
            args_key,
            name: parsed.name.clone(),
            description: parsed.description.clone(),
            status: OpenStatus::Running,
            deadline,
            cancel: request
                .cancel
                .clone()
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
            halt: Halt::None,
            events: Vec::new(),
            logs: Vec::new(),
            started: 0,
            limit_hit: false,
            drops_limit: 0,
            drops_budget: 0,
            declared: parsed.phases.clone(),
            phase_ids: HashMap::new(),
            open_phases: Vec::new(),
            next_phase: 0,
            next_event: 0,
        };
        let phases: Vec<Value> = parsed
            .phases
            .iter()
            .enumerate()
            .map(|(index, phase)| {
                let phase_id = format!("wph_{index}");
                run.phase_ids
                    .entry(phase.title.clone())
                    .or_insert_with(|| phase_id.clone());
                json!({
                    "id": phase_id,
                    "name": phase.title,
                    "description": phase.detail,
                })
            })
            .collect();
        let mut created = serde_json::Map::new();
        created.insert("name".into(), Value::String(parsed.name.clone()));
        created.insert("description".into(), Value::String(parsed.description));
        created.insert("phases".into(), Value::Array(phases));
        push_event(&mut run, "workflow_run.created", created);
        push_event(
            &mut run,
            "workflow_run.status_running",
            serde_json::Map::new(),
        );
        let cancel = Arc::clone(&run.cancel);
        book.open.push(run);
        remember_script(&mut book, &script_key);
        Ok(Prepared {
            id,
            name: parsed.name,
            script_key,
            body: parsed.body,
            args: request.args.clone(),
            cap,
            deadline,
            cancel,
            inline_agents: request.inline_agents,
            predefined: request.predefined_agents.iter().cloned().collect(),
            resumed: false,
        })
    }

    pub(crate) fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.inner.notify.notified()
    }

    pub(crate) fn is_stopped(&self, id: &str) -> bool {
        let book = lock(&self.inner.book);
        match book.open.iter().find(|run| run.id == id) {
            None => true,
            Some(run) => stopped(run),
        }
    }

    pub(crate) async fn until_stopped(&self, id: &str) {
        loop {
            if self.is_stopped(id) {
                return;
            }
            tokio::select! {
                _ = self.notified() => {}
                _ = tokio::time::sleep(Duration::from_millis(25)) => {}
            }
        }
    }

    pub(crate) fn mark_timeout(&self, id: &str) {
        let mut book = lock(&self.inner.book);
        if let Some(run) = book.open.iter_mut().find(|run| run.id == id) {
            if run.halt == Halt::None {
                run.halt = Halt::Timeout;
            }
            run.cancel.store(true, Ordering::Relaxed);
        }
        drop(book);
        self.inner.notify.notify_waiters();
    }

    /// Look up a step, or reserve the right to start it.
    pub(crate) fn admit(&self, script_key: &str, call_key: &str, run_id: &str, cap: u32) -> Admit {
        let mut book = lock(&self.inner.book);
        match book
            .caches
            .get(script_key)
            .and_then(|cache| cache.get(call_key))
        {
            Some(CacheEntry::Ready(result)) => return Admit::Ready(result.clone()),
            Some(CacheEntry::Inflight) => return Admit::Wait,
            None => {}
        }
        let over_budget = book.budget_total.is_some_and(|total| book.spent >= total);
        let start = {
            let Some(run) = book.open.iter_mut().find(|run| run.id == run_id) else {
                return Admit::Skip;
            };
            if stopped(run) {
                return Admit::Skip;
            }
            if over_budget {
                run.drops_budget += 1;
                return Admit::Skip;
            }
            if run.started >= cap {
                run.drops_limit += 1;
                run.limit_hit = true;
                return Admit::Skip;
            }
            run.started += 1;
            true
        };
        if start {
            book.caches
                .entry(script_key.to_string())
                .or_default()
                .insert(call_key.to_string(), CacheEntry::Inflight);
        }
        Admit::Start
    }

    pub(crate) fn store(&self, script_key: &str, call_key: &str, result: ThreadResult) {
        {
            let mut book = lock(&self.inner.book);
            book.caches
                .entry(script_key.to_string())
                .or_default()
                .insert(call_key.to_string(), CacheEntry::Ready(result));
        }
        self.inner.notify.notify_waiters();
    }

    pub(crate) fn add_spent(&self, cost: u64) {
        if cost == 0 {
            return;
        }
        let mut book = lock(&self.inner.book);
        book.spent = book.spent.saturating_add(cost);
    }

    pub(crate) fn finish_kind(&self, id: &str) -> FinishKind {
        let book = lock(&self.inner.book);
        let Some(run) = book.open.iter().find(|run| run.id == id) else {
            return FinishKind::Stopped;
        };
        if run.halt == Halt::Timeout || Instant::now() >= run.deadline {
            FinishKind::Timeout
        } else if run.limit_hit {
            FinishKind::ThreadLimit
        } else if run.halt == Halt::Stopped || run.cancel.load(Ordering::Relaxed) {
            FinishKind::Stopped
        } else if run.drops_budget > 0 {
            FinishKind::Budget
        } else {
            FinishKind::Completed
        }
    }

    pub(crate) fn log_line(&self, id: &str, message: String) {
        let mut book = lock(&self.inner.book);
        if let Some(run) = book.open.iter_mut().find(|run| run.id == id) {
            run.logs.push(message);
        }
    }

    pub(crate) fn begin_phase(&self, id: &str, title: &str) {
        let title = cut(title.trim(), 64);
        if title.is_empty() {
            return;
        }
        let mut book = lock(&self.inner.book);
        let Some(run) = book.open.iter_mut().find(|run| run.id == id) else {
            return;
        };
        let phase_id = if let Some(existing) = run.phase_ids.get(&title) {
            existing.clone()
        } else {
            let phase_id = format!("wph_extra_{}", run.next_phase);
            run.next_phase += 1;
            run.phase_ids.insert(title, phase_id.clone());
            phase_id
        };
        if run.open_phases.iter().any(|phase| phase.id == phase_id) {
            return;
        }
        let mut body = serde_json::Map::new();
        body.insert(
            "workflow_run_phase_id".into(),
            Value::String(phase_id.clone()),
        );
        let started = push_event(run, "workflow_run.phase_started", body);
        run.open_phases.push(OpenPhase {
            id: phase_id,
            started_event_id: started,
        });
    }

    pub(crate) fn complete(&self, id: &str, result: EndResult, value: Value) -> Outcome {
        let outcome = {
            let mut book = lock(&self.inner.book);
            let Some(index) = book.open.iter().position(|run| run.id == id) else {
                return refuse("unknown_error", "The run failed.");
            };
            let mut run = book.open.remove(index);
            finish_run(&mut run, result, value)
        };
        self.inner.notify.notify_waiters();
        outcome
    }

    pub(crate) fn pause(&self, id: &str, value: Value) -> Outcome {
        let outcome = {
            let mut book = lock(&self.inner.book);
            let Some(run) = book.open.iter_mut().find(|run| run.id == id) else {
                return refuse("unknown_error", "The run failed.");
            };
            run.status = OpenStatus::Idle;
            push_event(run, "workflow_run.status_idle", serde_json::Map::new());
            Outcome {
                id: Some(run.id.clone()),
                status: "idle".into(),
                result: None,
                value,
                events: run.events.clone(),
                notice: idle_notice(run),
                logs: run.logs.clone(),
            }
        };
        self.inner.notify.notify_waiters();
        outcome
    }

    #[cfg(test)]
    pub(crate) fn open_ids(&self) -> Vec<String> {
        lock(&self.inner.book)
            .open
            .iter()
            .map(|run| run.id.clone())
            .collect()
    }
}

pub(crate) enum Admit {
    Ready(ThreadResult),
    Wait,
    Start,
    Skip,
}

pub(crate) enum FinishKind {
    Timeout,
    ThreadLimit,
    Stopped,
    Budget,
    Completed,
}

fn remember_script(book: &mut Book, script_key: &str) {
    if book.caches.contains_key(script_key) {
        return;
    }
    book.caches.insert(script_key.to_string(), HashMap::new());
    book.cache_order.push_back(script_key.to_string());
    while book.cache_order.len() > MAX_CACHED_SCRIPTS {
        let Some(old) = book.cache_order.pop_front() else {
            break;
        };
        if book.open.iter().any(|run| run.script_key == old) {
            book.cache_order.push_back(old);
            continue;
        }
        book.caches.remove(&old);
    }
}

fn refuse(kind: &str, message: impl Into<String>) -> Outcome {
    let message = message.into();
    let mut error = serde_json::Map::new();
    error.insert("workflow_run_id".into(), Value::Null);
    error.insert("error".into(), json!({"type": kind, "message": message}));
    let event = event_value(None, "workflow_run.error", "ev_refused", error);
    Outcome::ended(
        None,
        EndResult::error(kind, message.clone()),
        Value::Null,
        vec![event],
        message,
        Vec::new(),
    )
}

fn finish_run(run: &mut OpenRun, result: EndResult, value: Value) -> Outcome {
    close_phases(run);
    if result.kind == "error" {
        let mut body = serde_json::Map::new();
        body.insert(
            "error".into(),
            json!({
                "type": result.error_type,
                "message": result.message,
            }),
        );
        push_event(run, "workflow_run.error", body);
    }
    let mut ended = serde_json::Map::new();
    ended.insert("result".into(), result_value(&result));
    push_event(run, "workflow_run.status_ended", ended);
    let notice = notice(run, &result, &value);
    let logs = run.logs.clone();
    Outcome::ended(
        Some(run.id.clone()),
        result,
        value,
        run.events.clone(),
        notice,
        logs,
    )
}

fn close_phases(run: &mut OpenRun) {
    let phases = std::mem::take(&mut run.open_phases);
    for phase in phases {
        let mut body = serde_json::Map::new();
        body.insert("workflow_run_phase_id".into(), Value::String(phase.id));
        body.insert(
            "phase_started_id".into(),
            Value::String(phase.started_event_id),
        );
        push_event(run, "workflow_run.phase_ended", body);
    }
}

fn notice(run: &OpenRun, result: &EndResult, value: &Value) -> String {
    let mut lines = Vec::new();
    match result.kind.as_str() {
        "completed" => lines.push(format!("Workflow run \"{}\" finished.", run.name)),
        "stopped" => lines.push(format!("Workflow run \"{}\" was stopped.", run.name)),
        _ => lines.push(format!(
            "Workflow run \"{}\" ended: {}.",
            run.name,
            result.message.as_deref().unwrap_or("The run failed")
        )),
    }
    if result.kind == "completed" && !value.is_null() {
        let text = value.to_string();
        if text.chars().count() > 2_000 {
            lines.push(format!("{}…", text.chars().take(2_000).collect::<String>()));
        } else {
            lines.push(text);
        }
    }
    for log in &run.logs {
        lines.push(log.clone());
    }
    if run.drops_limit > 0 {
        lines.push(drop_line(
            run.drops_limit,
            "the run reached its thread limit",
        ));
    }
    if run.drops_budget > 0 {
        lines.push(drop_line(run.drops_budget, "the session budget is spent"));
    }
    lines.join("\n")
}

fn drop_line(count: u32, why: &str) -> String {
    if count == 1 {
        format!("1 thread did not start because {why}.")
    } else {
        format!("{count} threads did not start because {why}.")
    }
}

fn result_value(result: &EndResult) -> Value {
    if result.kind == "error" {
        json!({
            "type": "error",
            "error": {"type": result.error_type, "message": result.message},
        })
    } else {
        json!({"type": result.kind})
    }
}

fn push_event(run: &mut OpenRun, kind: &str, body: serde_json::Map<String, Value>) -> String {
    let id = format!("ev_{}_{}", run.next_event, &run.id);
    run.next_event += 1;
    run.events.push(event_value(Some(&run.id), kind, &id, body));
    id
}

fn event_value(
    run_id: Option<&str>,
    kind: &str,
    id: &str,
    mut body: serde_json::Map<String, Value>,
) -> Value {
    body.insert("id".into(), Value::String(id.to_string()));
    body.insert("type".into(), Value::String(kind.to_string()));
    if !body.contains_key("workflow_run_id") {
        body.insert(
            "workflow_run_id".into(),
            run_id
                .map(|id| Value::String(id.to_string()))
                .unwrap_or(Value::Null),
        );
    }
    Value::Object(body)
}

fn script_key(script: &str) -> String {
    digest(script)
}

pub(crate) fn digest(text: &str) -> String {
    let digested = Sha256::digest(text.as_bytes());
    digested.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn stopped(run: &OpenRun) -> bool {
    run.cancel.load(Ordering::Relaxed) || run.halt != Halt::None || Instant::now() >= run.deadline
}

fn idle_notice(run: &OpenRun) -> String {
    let mut lines = vec![format!(
        "Workflow run \"{}\" is paused because the session budget is spent.",
        run.name
    )];
    lines.extend(run.logs.iter().cloned());
    if run.drops_limit > 0 {
        lines.push(drop_line(
            run.drops_limit,
            "the run reached its thread limit",
        ));
    }
    if run.drops_budget > 0 {
        lines.push(drop_line(run.drops_budget, "the session budget is spent"));
    }
    lines.join("\n")
}

pub(crate) fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

static NEXT_SLOT: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_slot() -> u64 {
    NEXT_SLOT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests;
