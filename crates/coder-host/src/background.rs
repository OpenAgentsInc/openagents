//! The background runner (docs/background/2026-10-02-background-processes.md):
//! `serve` starts it for this user's own host, so the disk cleanup monitor
//! runs wherever the host does, and the NIP-HOST `background.*` methods
//! answer from it.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use background::{Layout, TaskFact};
use coder_access::Code;
use coder_access::protocol::Operation;
use serde_json::{Value, json};

/// Reads the task store at a directory: the `coder` binary sets it to
/// `coder::task::background_facts`.
pub type FactsFn = fn(&Path) -> Result<Vec<TaskFact>, String>;

static FACTS: OnceLock<FactsFn> = OnceLock::new();
static JUDGE: OnceLock<Arc<dyn background::engine::Judge>> = OnceLock::new();
static SERVICES: OnceLock<ServicesFn> = OnceLock::new();

/// Makes the host's services for the task store at a directory: the
/// program that starts the host sets it (Coder runs, issue claims, health,
/// usage, flakes, plugins).
pub type ServicesFn = fn(&Path) -> Arc<dyn background::services::Services>;
static RUNNER: OnceLock<(Layout, background::runner::Handle)> = OnceLock::new();

/// Name the task store reader the runner uses. Without one, the classes
/// that need the task store (ended tasks' builds and worktrees) are
/// skipped.
pub fn set_facts(facts: FactsFn) {
    let _ = FACTS.set(facts);
}

/// Name the judge a rule's `Judgment` condition asks (Jev, from the
/// program that starts the host). Without one, a judgment never holds.
pub fn set_judge(judge: Arc<dyn background::engine::Judge>) {
    let _ = JUDGE.set(judge);
}

/// Name what makes the services phase 3 rules use. Without them, those
/// actions say the host cannot do them here.
pub fn set_services(services: ServicesFn) {
    let _ = SERVICES.set(services);
}

/// Start the runner over the task store `tasks`. `OPENAGENTS_BACKGROUND=off`
/// leaves it off.
pub(crate) fn start(tasks: &Path) {
    if std::env::var("OPENAGENTS_BACKGROUND").is_ok_and(|value| value == "off") {
        eprintln!("openagents host: background rules are off (OPENAGENTS_BACKGROUND=off)");
        return;
    }
    let layout = match std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| std::io::Error::other("HOME is not set"))
        .and_then(|home| Layout::new(&home, Some(tasks.to_owned())))
    {
        Ok(layout) => layout,
        Err(error) => {
            eprintln!("openagents host: background rules are off: {error}");
            return;
        }
    };
    let facts = FACTS.get().map(|read| {
        let read = *read;
        let store = tasks.to_owned();
        Arc::new(move || read(&store)) as Arc<dyn background::Facts>
    });
    let handle = background::runner::start_full(
        layout.clone(),
        facts,
        JUDGE.get().cloned(),
        SERVICES.get().map(|make| make(tasks)),
        Box::new(|line| eprintln!("openagents host: {line}")),
    );
    let _ = RUNNER.set((layout, handle));
    eprintln!("openagents host: background rules on");
}

/// Answer a `background.*` operation.
pub(crate) fn answer(op: &Operation) -> Result<Value, Code> {
    let (layout, handle) = RUNNER.get().ok_or(Code::Unsupported)?;
    let value = |v: Result<Value, serde_json::Error>| v.map_err(|_| Code::Unavailable);
    match op {
        Operation::ListBackground {} => value(serde_json::to_value(background::view::list(layout))),
        Operation::ShowBackground { rule } => {
            let rule = background::store::load(layout, rule).map_err(|_| Code::Unsupported)?;
            Ok(json!({"rule": rule, "digest": rule.digest()}))
        }
        Operation::LogBackground { rule, since } => value(serde_json::to_value(
            background::view::log(layout, rule.as_deref(), *since, 50),
        )),
        Operation::RunBackground { rule } => {
            background::store::load(layout, rule).map_err(|_| Code::Unsupported)?;
            handle.run(rule);
            Ok(json!({"queued": rule}))
        }
        Operation::PauseBackground {
            rule,
            until,
            resume,
        } => {
            let rule = background::view::pause(layout, rule, *until, *resume)
                .map_err(|_| Code::Unsupported)?;
            value(serde_json::to_value(rule))
        }
        _ => Err(Code::Unsupported),
    }
}
