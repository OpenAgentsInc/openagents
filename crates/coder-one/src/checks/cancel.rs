//! Cancellation lifecycle: a child process runs the candidate's runner on
//! tasks that record their start and cleanup, the host interrupts it once
//! the tasks are running, and the event sequence says whether every
//! started task finished its cleanup before the call returned.
//!
//! The runner is interrupted below, at, and above its concurrency limit,
//! by a real SIGINT and, once, by cancelling the call from inside the
//! event loop. The limit and the sizes are host choices; the relation
//! comes from the instruction's request that cleanup still runs.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};

use super::{Bounds, Context, Ineligible, Relation, Scenario, Verdict, base_name};
use crate::minitask::process;

/// The concurrency limit the scenarios pass: a host choice.
pub const LIMIT: usize = 3;

/// The task counts: below, at, and above the limit.
pub const SIZES: [(&str, usize); 3] = [("below", LIMIT - 1), ("at", LIMIT), ("above", LIMIT + 2)];

/// The driver: runs the candidate's function on recording tasks and
/// writes one event per line.
const DRIVER: &str = r#"import asyncio, json, os, sys, time
work, module, func, events, n, limit, mode = sys.argv[1:8]
n, limit = int(n), int(limit)
sys.path.insert(0, work)
os.chdir(work)
def emit(kind, task=None):
    with open(events, "a") as f:
        f.write(json.dumps({"kind": kind, "task": task}) + "\n")
try:
    run_tasks = getattr(__import__(module), func)
except BaseException as error:
    emit("import_error", type(error).__name__ + ": " + str(error)[:300])
    sys.exit(3)
def make(i):
    async def task():
        emit("start", i)
        try:
            await asyncio.sleep(3600)
        finally:
            emit("cleanup_begin", i)
            await asyncio.sleep(0.05)
            emit("cleanup_end", i)
    return task
def started():
    try:
        return sum(1 for line in open(events) if '"start"' in line)
    except OSError:
        return 0
async def main():
    try:
        if mode == "internal":
            runner = asyncio.ensure_future(run_tasks([make(i) for i in range(n)], limit))
            while started() < min(n, limit):
                await asyncio.sleep(0.01)
            await asyncio.sleep(0.05)
            emit("interrupt")
            runner.cancel()
            await runner
        else:
            await run_tasks([make(i) for i in range(n)], limit)
    finally:
        emit("returned")
emit("begin")
try:
    asyncio.run(main())
except KeyboardInterrupt:
    emit("keyboard_interrupt")
except asyncio.CancelledError:
    emit("cancelled")
except BaseException as error:
    emit("error", type(error).__name__)
emit("exited")
"#;

struct Target {
    module: String,
    function: String,
    file: String,
}

fn target(context: &Context<'_>) -> Result<Target, String> {
    let (module, function) = context
        .task
        .instruction
        .split('`')
        .skip(1)
        .step_by(2)
        .find_map(|span| {
            let words: Vec<&str> = span.split_whitespace().collect();
            match words.as_slice() {
                ["from", module, "import", function] => {
                    Some(((*module).to_string(), (*function).to_string()))
                }
                _ => None,
            }
        })
        .ok_or("the instruction names no `from MODULE import FUNCTION` interface")?;
    let file = format!("{module}.py");
    let (path, source) = context
        .candidate
        .file_named(&file)
        .ok_or_else(|| format!("the candidate has no {file}"))?;
    if !source.contains(&format!("def {function}")) {
        return Err(format!("{file} defines no {function}"));
    }
    Ok(Target {
        module,
        function,
        file: path.clone(),
    })
}

/// Builds the cancellation scenarios that apply.
///
/// # Errors
///
/// Returns why none applies.
pub fn build(context: &Context<'_>) -> Result<Vec<Scenario>, Vec<Ineligible>> {
    let not = |why: String| {
        Err(vec![Ineligible {
            kind: "cancellation".to_string(),
            why,
        }])
    };
    let text = context.task.instruction.to_lowercase();
    if !(text.contains("cancel") || text.contains("interrupt")) || !text.contains("cleanup") {
        return not("the instruction asks for no cleanup on cancellation".to_string());
    }
    if !text.contains("concurren") {
        return not("the instruction states no concurrency limit".to_string());
    }
    let found = match target(context) {
        Ok(found) => found,
        Err(why) => return not(why),
    };
    let cleanup =
        context.requirements_saying(&[&["cleanup"], &["keyboard interrupt"], &["cancel"]]);
    let limit = context.requirements_saying(&[&["concurren"]]);
    if cleanup.is_empty() {
        return not("no requirement asks for cleanup".to_string());
    }
    let mut observed = cleanup.clone();
    let mut bounded = Vec::new();
    for r in limit {
        if !observed.iter().any(|o| o.id == r.id) {
            bounded.push(r.id.clone());
            observed.push(r);
        }
    }
    let cleanup_ids: Vec<String> = cleanup.iter().map(|r| r.id.clone()).collect();
    let candidate = context.candidate.digest();
    let interface = format!(
        "from {} import {}; asyncio.run({}(tasks, max_concurrent)) in a child process",
        found.module, found.function, found.function
    );
    let mut scenarios = Vec::new();
    let runs = SIZES.iter().map(|(size, n)| ("signal", *size, *n)).chain([(
        "internal",
        "above",
        LIMIT + 2,
    )]);
    for (how, size, n) in runs {
        scenarios.push(Scenario {
            id: format!("cancel.{how}.{size}"),
            kind: "cancellation".to_string(),
            requirements: observed.iter().map(|r| r.id.clone()).collect(),
            spans: context.spans_of(&observed),
            applies: vec![
                "the instruction asks that cleanup still runs when a run is cancelled".to_string(),
                format!("the candidate's {} defines {}", found.file, found.function),
            ],
            interface: interface.clone(),
            bounds: Bounds { seconds: 15, processes: 1 },
            effects: vec![
                "copies the candidate into a scratch directory and runs it in a child process there".to_string(),
                if how == "signal" {
                    "sends the child SIGINT".to_string()
                } else {
                    "cancels the call inside the child's event loop".to_string()
                },
            ],
            candidate: candidate.clone(),
            input: atif::digest(&json!({ "tasks": n, "limit": LIMIT, "how": how, "driver": DRIVER })),
            seed: None,
            expected: Relation {
                statement: format!(
                    "With {n} tasks and a limit of {LIMIT}, once {} tasks have started, an interrupt leaves every started task's cleanup finished before the call returns, never more than {LIMIT} run at once, and the process exits.",
                    n.min(LIMIT)
                ),
                derivation: "The instruction asks for a limit on concurrent tasks and for the tasks' cleanup to still run when a run is cancelled by a keyboard interrupt.".to_string(),
            },
            params: json!({
                "tasks": n, "limit": LIMIT, "size": size, "how": how,
                "cleanup_requirements": cleanup_ids, "limit_requirements": bounded,
                "derived_from": "host choices around the instruction's concurrency limit",
            }),
        });
    }
    Ok(scenarios)
}

fn event(events: &[Value], i: usize) -> (&str, Option<i64>) {
    (
        events[i]["kind"].as_str().unwrap_or_default(),
        events[i]["task"].as_i64(),
    )
}

/// Runs a cancellation scenario.
pub async fn run(context: &Context<'_>, scenario: &Scenario, scratch: &Path) -> Verdict {
    let Some(python) = process::python() else {
        return Verdict::unavailable(&scenario.id, "python3 is not on PATH");
    };
    let found = match target(context) {
        Ok(found) => found,
        Err(why) => return Verdict::unavailable(&scenario.id, &why),
    };
    let n = usize::try_from(scenario.params["tasks"].as_u64().unwrap_or(1)).unwrap_or(1);
    let how = scenario.params["how"]
        .as_str()
        .unwrap_or("signal")
        .to_string();
    let work = scratch.join("work");
    if std::fs::create_dir_all(&work).is_err() {
        return Verdict::unavailable(&scenario.id, "cannot create the scratch copy");
    }
    for (path, text) in context
        .candidate
        .files
        .iter()
        .filter(|(p, _)| p.ends_with(".py"))
        .chain(context.candidate.provided.iter())
    {
        let _ = std::fs::write(work.join(base_name(path)), text);
    }
    let events_path = scratch.join("events.jsonl");
    let mut command = Command::new(python);
    command
        .arg("-c")
        .arg(DRIVER)
        .arg(&work)
        .arg(&found.module)
        .arg(&found.function)
        .arg(&events_path)
        .arg(n.to_string())
        .arg(LIMIT.to_string())
        .arg(&how)
        .current_dir(&work)
        .env("PYTHONDONTWRITEBYTECODE", "1");
    let want = n.min(LIMIT);
    let ready_path = events_path.clone();
    let ready = move || {
        std::fs::read_to_string(&ready_path)
            .map(|text| text.matches("\"start\"").count() >= want)
            .unwrap_or(false)
    };
    let (ran, readied, interrupted) = if how == "signal" {
        let done = process::interrupt_when(
            command,
            scratch,
            &ready,
            Duration::from_secs(5),
            Duration::from_secs(5),
        )
        .await;
        (done.ran, done.ready, done.interrupted_at_ms.is_some())
    } else {
        let ran = process::run(command, Duration::from_secs(scenario.bounds.seconds)).await;
        (ran, true, true)
    };
    let events: Vec<Value> = std::fs::read_to_string(&events_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let mut verdict = Verdict::new(&scenario.id, "passed");
    verdict.coverage.push("Tasks still queued when the interrupt came aren't checked: the instruction doesn't say whether they may start.".to_string());
    verdict
        .coverage
        .push("One interrupt; a second one during cleanup isn't sent.".to_string());
    if let Some(error) = events.iter().find(|e| e["kind"] == "import_error") {
        let detail = error["task"].as_str().unwrap_or_default();
        verdict.verdict = if detail.starts_with("ModuleNotFoundError") {
            "unavailable"
        } else {
            "failed"
        }
        .to_string();
        verdict.observations.push(json!({ "import_error": detail }));
        verdict
            .hypotheses
            .push("The submitted module doesn't import.".to_string());
        return verdict;
    }
    // Replay the sequence: what ran at once, what started, what cleaned up.
    let mut running = 0usize;
    let mut peak = 0usize;
    let mut started = Vec::new();
    let mut cleaned = Vec::new();
    let mut returned_at = None;
    let mut cleaned_after_return = Vec::new();
    for i in 0..events.len() {
        match event(&events, i) {
            ("start", Some(task)) => {
                running += 1;
                peak = peak.max(running);
                started.push(task);
            }
            ("cleanup_end", Some(task)) => {
                running = running.saturating_sub(1);
                cleaned.push(task);
                if returned_at.is_some() {
                    cleaned_after_return.push(task);
                }
            }
            ("returned", _) => returned_at = Some(i),
            _ => {}
        }
    }
    let missing: Vec<i64> = started
        .iter()
        .filter(|t| !cleaned.contains(t))
        .copied()
        .collect();
    let exited = events.iter().any(|e| e["kind"] == "exited");
    verdict.observations.push(json!({
        "tasks": n,
        "limit": LIMIT,
        "ready": readied,
        "interrupted": interrupted,
        "started": started.len(),
        "peak_running": peak,
        "cleaned_up": cleaned.len(),
        "cleanup_missing": missing,
        "cleanup_after_return": cleaned_after_return,
        "returned": returned_at.is_some(),
        "exited": exited,
        "killed": ran.killed,
        "sequence": events.iter().map(|e| match e["task"].as_i64() {
            Some(task) => format!("{} {task}", e["kind"].as_str().unwrap_or_default()),
            None => e["kind"].as_str().unwrap_or_default().to_string(),
        }).collect::<Vec<_>>(),
    }));
    let ids = |key: &str| -> Vec<String> {
        scenario.params[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let (cleanup_ids, limit_ids) = (ids("cleanup_requirements"), ids("limit_requirements"));
    let contradict = |verdict: &mut Verdict, which: &[String]| {
        for id in which {
            if !verdict.contradicts.contains(id) {
                verdict.contradicts.push(id.clone());
            }
        }
    };
    if !readied {
        contradict(&mut verdict, &limit_ids);
        verdict.verdict = "failed".to_string();
        verdict.hypotheses.push(format!(
            "Fewer than {want} tasks started: the runner doesn't start tasks up to the limit."
        ));
        return verdict;
    }
    if peak > LIMIT {
        contradict(&mut verdict, &limit_ids);
        verdict.verdict = "failed".to_string();
        verdict
            .hypotheses
            .push("More tasks ran at once than the limit allows.".to_string());
    }
    if !missing.is_empty() {
        contradict(&mut verdict, &cleanup_ids);
        verdict.verdict = "failed".to_string();
        let begun: Vec<i64> = events
            .iter()
            .filter(|e| e["kind"] == "cleanup_begin")
            .filter_map(|e| e["task"].as_i64())
            .collect();
        if missing.iter().any(|t| begun.contains(t)) {
            verdict.hypotheses.push("Cleanup began but was cut short: a second cancellation, such as cancelling tasks already running their cleanup, interrupted it.".to_string());
        }
        if missing.iter().any(|t| !begun.contains(t)) {
            verdict.hypotheses.push("A started task never reached its cleanup: it was cancelled but not awaited, or the event loop closed first.".to_string());
        }
    }
    if !cleaned_after_return.is_empty() {
        contradict(&mut verdict, &cleanup_ids);
        verdict.verdict = "failed".to_string();
        verdict.hypotheses.push("The call returned before awaiting its cancelled tasks, so cleanup finished only as the event loop shut down.".to_string());
    }
    if !exited || ran.killed {
        contradict(&mut verdict, &cleanup_ids);
        verdict.verdict = "failed".to_string();
        verdict.hypotheses.push("The process didn't exit after the interrupt: cancellation is swallowed, or cleanup waits forever.".to_string());
    }
    verdict
}
