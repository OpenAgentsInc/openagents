//! `accept.define` on a mini-task, then a plain loop to green.
//!
//! The mini-task is set up in a scratch directory, the requirement map is
//! extracted with Jev, and `accept.define` writes and freezes the suite.
//! Then the loop is the simplest one that uses it: run the suite; while
//! it's red, give a Microluna edit session the task, the frozen tests, and
//! the red tests' output; stop when it's green or the sessions run out.
//! The mini-task's own grader, which the sessions never see, says whether
//! green meant solved.

use std::path::{Path, PathBuf};
use std::time::Duration;

use microluna::{Brief, Config, Ending, Isolation, Transport};
use serde_json::{Value, json};

use super::{AcceptanceSuite, Inputs, Local, MicrolunaWriter, Options, Task, define, run};
use crate::component::jev::JevMode;
use crate::record::Recorder;

/// The schema of a mini-task loop's result.
pub const SCHEMA: &str = "openagents.coder-one.acceptance-minitask.v1";

/// What an edit session is told about the frozen suite.
pub const EDIT_GUIDANCE: &str = "A frozen acceptance suite defines done for this task. Change \
the workspace until every test in it passes; you can't change the tests. The current state shows \
the red tests' output from the host's run just before this session. Rerun the suite with the \
command the evidence gives after every edit. When every test passes, call finish with status \
done: the host reruns the suite after the session, and that run decides done. If a test seems to \
contradict the task, follow the task and say so in your summary.";

/// How the loop runs.
#[derive(Clone, Debug)]
pub struct LoopOptions {
    pub model: String,
    /// The most edit sessions.
    pub sessions: u32,
    pub session_turns: usize,
    pub session_sec: u64,
    /// The writing session's bounds.
    pub writer_turns: usize,
    pub writer_sec: u64,
    pub define: Options,
    pub echo: bool,
}

impl Default for LoopOptions {
    fn default() -> Self {
        LoopOptions {
            model: "gpt-6-luna".to_string(),
            sessions: 4,
            session_turns: 30,
            session_sec: 600,
            writer_turns: 40,
            writer_sec: 600,
            define: Options::default(),
            echo: false,
        }
    }
}

/// One edit session of the loop.
#[allow(clippy::too_many_arguments)]
async fn edit<T: Transport>(
    transport: &T,
    workdir: &Path,
    artifacts: &Path,
    suite: &AcceptanceSuite,
    task: &Task,
    red: Vec<String>,
    number: u32,
    options: &LoopOptions,
) -> Value {
    let brief = Brief {
        task: task.instruction.clone(),
        guidance: EDIT_GUIDANCE.to_string(),
        evidence: vec![suite.evidence()],
        state: red,
    };
    let config = Config {
        max_turns: options.session_turns,
        deadline: Some(Duration::from_secs(options.session_sec)),
        model: options.model.clone(),
        ..Config::luna(&format!("accept-{}", &suite.digest[..16]))
    };
    let mut recorder = microluna::Recorder::new();
    if options.echo {
        recorder = recorder.echoing();
    }
    let session = atif::Session::opening(
        &format!("accept-edit-{number}"),
        &options.model,
        "codex-login",
        &workdir.display().to_string(),
        &crate::episode::version(),
    );
    if let Ok(log) = atif::Log::create_at(
        &artifacts.join(format!("edit-{number}.atif.jsonl")),
        &session,
    ) {
        recorder = recorder.logging(log);
    }
    let report = match microluna::Workspace::new(workdir) {
        Ok(workspace) => {
            microluna::run(
                transport,
                &workspace.isolated_by(Isolation::Boundary),
                &brief,
                &config,
                &mut recorder,
            )
            .await
        }
        Err(error) => {
            return json!({ "number": number, "ending": "transport", "error": error.to_string(), "usd": 0.0 });
        }
    };
    recorder.close(match report.ending {
        Ending::Finished => atif::log::ENDED,
        _ => atif::log::INTERRUPTED,
    });
    json!({
        "number": number,
        "ending": format!("{:?}", report.ending),
        "finish": report.finish,
        "turns": report.turns,
        "calls": report.calls,
        "usd": report.cost_usd.unwrap_or_default(),
        "milliseconds": report.milliseconds,
    })
}

/// Runs `accept.define` and the loop on the mini-task `id` under `out`,
/// and returns the result, which is also written to `out/result.json`.
///
/// # Errors
///
/// A message when the task is unknown, can't be set up, or has no
/// Microluna transport.
#[allow(clippy::too_many_lines)]
pub async fn run_minitask(
    id: &str,
    out: &Path,
    jev: &JevMode,
    options: &LoopOptions,
) -> Result<Value, String> {
    let mini = crate::minitask::find(id)?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let out = &out.canonicalize().map_err(|e| e.to_string())?;
    let workdir = out.join("work");
    let suite_dir = out.join("suite");
    let artifacts = out.join("artifacts");
    std::fs::create_dir_all(&artifacts).map_err(|e| e.to_string())?;
    crate::minitask::setup(&mini, &workdir)?;
    let workdir = workdir.canonicalize().map_err(|e| e.to_string())?;
    let session = atif::Session::opening(
        &format!("accept-minitask-{id}"),
        &options.model,
        "codex-login",
        &workdir.display().to_string(),
        &crate::episode::version(),
    );
    let recorder = match atif::Log::create_at(&artifacts.join("episode.atif.jsonl"), &session) {
        Ok(log) => Recorder::durable(log),
        Err(_) => Recorder::default(),
    };
    let task = Task {
        title: id.to_string(),
        instruction: mini.instruction.to_string(),
    };
    let (map, _) = crate::requirements::extract_with(
        &task.title,
        &task.instruction,
        crate::requirements::Params::default(),
        jev,
        &recorder,
        None,
    )
    .await;
    let wire = crate::micro::codex_wire(&format!("accept-{id}-{}", atif::now_ms()))?;
    let writer = MicrolunaWriter {
        transport: &wire,
        config: Config {
            max_turns: options.writer_turns,
            deadline: Some(Duration::from_secs(options.writer_sec)),
            model: options.model.clone(),
            ..Config::luna(&format!(
                "accept-writer-{}",
                &super::sha256(task.instruction.as_bytes())[..16]
            ))
        },
        isolation: Isolation::Boundary,
        traces: Some(artifacts.clone()),
        echo: options.echo,
    };
    let runner = Local::writing(options.define.test_sec);
    let inputs = Inputs {
        task: &task,
        requirements: &map,
        evidence: &[],
        workspace: &workdir,
        suite_dir: &suite_dir,
        workspace_note: String::new(),
        target: None,
    };
    crate::say::line(&format!("accept ▸ {id}: writing the suite"));
    let suite = define(&inputs, &writer, &runner, jev, &recorder, &options.define).await;
    crate::say::line(&format!("accept ▸ {}", suite.headline()));

    let mut runs = Vec::new();
    let mut sessions = Vec::new();
    let mut sessions_usd = 0.0;
    let mut stopped = format!("the bound of {} sessions", options.sessions);
    let mut number = 0;
    loop {
        let label = if number == 0 {
            "start".to_string()
        } else {
            format!("after session {number}")
        };
        let result = match run(&suite, &workdir, &runner, Some(&recorder), &label).await {
            Ok(result) => result,
            Err(tampered) => {
                stopped = tampered.to_string();
                break;
            }
        };
        crate::say::line(&format!(
            "accept ▸ {label}: {} of {} tests pass",
            result.passed, result.total
        ));
        let green = result.green;
        let red = result.red_lines(&suite, 800);
        runs.push(result);
        if green {
            stopped = "the suite is green".to_string();
            break;
        }
        if suite.tests.is_empty() {
            stopped = "the suite has no tests".to_string();
            break;
        }
        if number >= options.sessions {
            break;
        }
        number += 1;
        let ran = edit(
            &wire, &workdir, &artifacts, &suite, &task, red, number, options,
        )
        .await;
        sessions_usd += ran["usd"].as_f64().unwrap_or_default();
        crate::say::line(&format!(
            "accept ▸ session {number}: {} (${:.4})",
            ran["ending"].as_str().unwrap_or_default(),
            ran["usd"].as_f64().unwrap_or_default()
        ));
        sessions.push(ran);
    }
    let grade = crate::minitask::grade(&mini, &workdir, &out.join("grader")).await;
    let green = runs.last().is_some_and(|r| r.green);
    let complete = runs.last().is_some_and(|r| r.complete);
    let result = json!({
        "schema": SCHEMA,
        "task": id,
        "model": options.model,
        "requirements": map.requirements.iter().map(|r| json!({"id": r.id, "kind": r.kind.word(), "text": r.text})).collect::<Vec<_>>(),
        "suite": {
            "status": suite.status,
            "headline": suite.headline(),
            "digest": suite.digest,
            "tests": suite.tests,
            "rejected": suite.rejected,
            "gaps": suite.gaps,
            "coverage": suite.coverage,
            "rounds": suite.rounds,
            "writer_usd": suite.writer_usd,
            "jev_usd": suite.jev_usd,
            "milliseconds": suite.milliseconds,
        },
        "runs": runs.iter().map(|r| json!({"label": r.label, "passed": r.passed, "total": r.total, "green": r.green, "red": r.red_requirements()})).collect::<Vec<_>>(),
        "sessions": sessions,
        "stopped": stopped,
        "green": green,
        "complete": complete,
        "complete_agrees_with_grader": grade.reward().map(|r| (r >= 1.0) == complete),
        "grade": grade,
        "green_agrees_with_grader": grade.reward().map(|r| (r >= 1.0) == green),
        "spend_usd": {
            "writer": suite.writer_usd,
            "jev": suite.jev_usd,
            "sessions": sessions_usd,
            "total": suite.writer_usd + suite.jev_usd + sessions_usd,
        },
        "suite_record": AcceptanceSuite::record_path(&suite_dir),
    });
    let text = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("result.json"), format!("{text}\n").as_bytes())?;
    recorder.finish(atif::log::ENDED);
    Ok(result)
}

/// The default directory mini-task loops record under.
#[must_use]
pub fn default_out(id: &str) -> Option<PathBuf> {
    crate::credentials::openagents_dir().map(|dir| {
        dir.join("coder-one")
            .join("accept")
            .join(format!("{id}-{}", atif::now_ms()))
    })
}
