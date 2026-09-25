//! `checks.metric_target` and `control.optimize` in the lean loop, with
//! scripted Luna replies (`microluna::fake`) and recorded Jev answers.

use std::collections::BTreeMap;

use microluna::fake::call;

use super::optimize::{MetricTargetRule, OptimizeRule, metric_dir};
use super::*;
use crate::checks::metric_target::{self as metric, Protocol};
use crate::component::jev::{JevMode, RECORDED_SCHEMA, Recorded, RecordedAnswer};

fn usage() -> TokenUsage {
    TokenUsage {
        input: 1_000,
        cached: 0,
        output: 30,
        reasoning: 0,
    }
}

fn run(id: &str, command: &str) -> microluna::Reply {
    call(
        id,
        "run_command",
        &json!({ "command": command, "timeout_seconds": null }),
        usage(),
    )
}

fn write(id: &str, path: &str, contents: &str) -> microluna::Reply {
    call(
        id,
        "write_file",
        &json!({ "path": path, "contents": contents }),
        usage(),
    )
}

fn finish(id: &str, status: &str) -> microluna::Reply {
    call(
        id,
        "finish",
        &json!({ "status": status, "summary": format!("{id} ended {status}."), "answer": "" }),
        usage(),
    )
}

/// Jev's recorded answer for `instruction`: its first number is the goal,
/// with `direction`, `quantity`, and `relative`.
fn recorded(instruction: &str, direction: &str, quantity: &str, relative: &str) -> JevMode {
    let numbers = metric::candidates(instruction);
    let references = metric::references(instruction);
    let request = metric::request(instruction, &numbers, &references, &[]);
    let mut answers = serde_json::Map::new();
    for j in 0..numbers.len() {
        let first = j == 0;
        answers.insert(
            format!("goal_{j}"),
            json!({"noul": if first { 0.9 } else { 0.1 }}),
        );
        answers.insert(
            format!("direction_{j}"),
            json!({"choice": if first { direction } else { "neither" }}),
        );
        answers.insert(format!("quantity_{j}"), json!({"choice": quantity}));
        answers.insert(format!("relative_{j}"), json!({"choice": relative}));
    }
    let mut entries = BTreeMap::new();
    entries.insert(
        metric::request_key(&request),
        RecordedAnswer {
            name: "jev_metric_target".to_string(),
            model: "test".to_string(),
            answers: Value::Object(answers),
            input_tokens: Some(1_000),
            output_tokens: None,
            milliseconds: None,
            source: "test".to_string(),
        },
    );
    JevMode::Recorded(Recorded {
        schema: RECORDED_SCHEMA.to_string(),
        entries,
    })
}

fn micro(dir: &Path, replies: Vec<microluna::Reply>, lean: lean::Lean) -> Micro {
    let work = dir.join("work");
    let artifacts = dir.join("artifacts");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&artifacts).unwrap();
    let mut micro = Micro::new(
        "gpt-6-luna",
        None,
        Duration::from_secs(300),
        &work,
        &artifacts,
        Recorder::default(),
        0,
        Policy {
            lean: Some(lean),
            spend_usd: 5.0,
            ..Policy::default()
        },
        Isolation::TaskContainer,
    );
    micro.wire = Ok(Wire::Fake(FakeTransport::new(replies)));
    micro
}

fn prepared(instruction: &str, jev: JevMode) -> Prepared {
    Prepared {
        instruction: instruction.to_string(),
        title: "cost".to_string(),
        directions: String::new(),
        requirements: crate::requirements::mechanical(instruction),
        items: Vec::new(),
        informs: BTreeMap::new(),
        jev,
        deadline: None,
    }
}

fn briefing(text: &str) -> Briefing {
    Briefing {
        text: text.to_string(),
        cap: 12_000,
        included: Vec::new(),
        omitted: Vec::new(),
    }
}

fn shape(sessions: u32, optimize: Option<OptimizeRule>) -> lean::Lean {
    let mut lean: lean::Lean = serde_json::from_value(json!({
        "sessions": sessions,
        "source_chars": 2_000,
        "keep_best": true,
        "score_sec": 20,
    }))
    .unwrap();
    lean.metric_target = Some(Box::new(MetricTargetRule {
        protocol: Protocol {
            warmup: 1,
            repeats: 3,
            run_sec: 10,
            budget_sec: 60,
        },
        write_harness: true,
        harness_usd: 0.25,
        harness_sec: 120,
        finish: true,
    }));
    lean.optimize = optimize.map(Box::new);
    lean
}

const COST_TASK: &str = "Write plan.txt holding a plan, and value.txt holding its cost. \
The plan's cost must be at most 5.";

/// The harness session: writes a harness that prints the cost in
/// `value.txt`, after a phase line.
fn cost_harness(work: &Path) -> Vec<microluna::Reply> {
    let dir = metric_dir(work, Isolation::TaskContainer);
    vec![
        run(
            "h1",
            &format!(
                "mkdir -p {d} && printf '%s\\n' '[ -f value.txt ] || exit 1' \
                 'echo \"phase plan: 0.1s\"' 'echo \"METRIC $(cat value.txt)\"' > {d}/harness.sh",
                d = dir.display()
            ),
        ),
        finish("h2", "done"),
    ]
}

/// The first work session: writes the score script, the plan, and a cost
/// of `cost`.
fn first_session(work: &Path, cost: &str) -> Vec<microluna::Reply> {
    let eval = lean::eval_dir(work, Isolation::TaskContainer);
    vec![
        run(
            "a1",
            &format!(
                "mkdir -p {e} && printf '%s\\n' 'if grep -q plan plan.txt; then echo SCORE 1 1; \
                 else echo SCORE 0 1; fi' > {e}/score.sh",
                e = eval.display()
            ),
        ),
        write("a2", "plan.txt", "plan\n"),
        write("a3", "value.txt", &format!("{cost}\n")),
        finish("a4", "done"),
    ]
}

fn cleanup(work: &Path) {
    let _ = std::fs::remove_dir_all(lean::eval_dir(work, Isolation::TaskContainer));
    let _ = std::fs::remove_dir_all(metric_dir(work, Isolation::TaskContainer));
}

async fn run_loop(
    dir: &Path,
    instruction: &str,
    jev: JevMode,
    replies: Vec<microluna::Reply>,
    lean: lean::Lean,
) -> (Value, Vec<microluna::transport::Request>) {
    let mut executor = micro(dir, replies, lean);
    executor.prepared = Some(prepared(instruction, jev));
    executor.execute(&briefing(instruction)).await;
    let requests = match &executor.wire {
        Ok(Wire::Fake(fake)) => fake.requests(),
        _ => Vec::new(),
    };
    (executor.last.clone().unwrap(), requests)
}

fn moves_of<'a>(record: &'a Value, kind: &str) -> Vec<&'a Value> {
    record["moves"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["kind"] == kind)
        .collect()
}

fn read(work: &Path, file: &str) -> String {
    std::fs::read_to_string(work.join(file))
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn cost_jev() -> JevMode {
    recorded(COST_TASK, "at_most", "cost", "absolute")
}

#[tokio::test]
async fn a_done_finish_is_refused_while_the_stated_target_is_unmet() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    cleanup(&work);
    let mut replies = cost_harness(&work);
    replies.extend(first_session(&work, "8"));
    replies.extend([write("b1", "value.txt", "4\n"), finish("b2", "done")]);
    let (record, requests) =
        run_loop(dir.path(), COST_TASK, cost_jev(), replies, shape(2, None)).await;
    let setup = &moves_of(&record, "lean.metric_target")[0];
    assert_eq!(setup["targets"][0]["quantity"], "cost");
    assert_eq!(setup["targets"][0]["threshold"], 5.0);
    assert_eq!(setup["harness"]["source"], "written");
    assert_eq!(setup["harness"]["frozen"], true);
    assert_eq!(setup["harness"]["workspace_changed"], false);
    // No value.txt yet: the untouched workspace is unmeasured.
    assert_eq!(setup["untouched"]["verdict"], "unmeasured");
    let refused = moves_of(&record, "lean.finish_refused");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["session"], 2);
    let sessions = moves_of(&record, "lean");
    assert_eq!(sessions[0]["metric"]["value"], 8.0);
    assert_eq!(sessions[0]["metric"]["verdict"], "unmet");
    assert_eq!(sessions[1]["metric"]["verdict"], "met");
    let stopped = record["stopped"].as_str().unwrap();
    assert!(stopped.contains("session 3 ended done"), "{stopped}");
    assert!(stopped.contains("the stated target is met"), "{stopped}");
    // Session 3 was told why session 2's done didn't stand, and every
    // work session saw the target.
    let text = serde_json::to_string(&requests.last().unwrap().input).unwrap();
    assert!(text.contains("didn't accept session 2's done"), "{text}");
    assert!(text.contains("Stated target"));
    // The harness session saw the goal, not the task.
    let harness = serde_json::to_string(&requests[0].input).unwrap();
    assert!(harness.contains("The goal to measure: cost at most 5"));
    assert!(!harness.contains("Write plan.txt holding a plan"));
    cleanup(&work);
}

#[tokio::test]
async fn an_optimization_round_that_improves_beyond_the_spread_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    cleanup(&work);
    let mut replies = cost_harness(&work);
    replies.extend(first_session(&work, "4"));
    replies.extend([write("o1", "value.txt", "3\n"), finish("o2", "done")]);
    let (record, requests) = run_loop(
        dir.path(),
        COST_TASK,
        cost_jev(),
        replies,
        shape(
            1,
            Some(OptimizeRule {
                rounds: 1,
                wall_sec: 300,
                spend_usd: 1.0,
            }),
        ),
    )
    .await;
    let rounds = moves_of(&record, "lean.optimize");
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0]["kept"], true);
    assert_eq!(rounds[0]["acceptance"]["passed"], true);
    assert_eq!(rounds[0]["before"]["value"], 4.0);
    assert_eq!(rounds[0]["after"]["value"], 3.0);
    assert_eq!(read(&work, "value.txt"), "3");
    // The round's session saw the measurement and the harness's output.
    let text = serde_json::to_string(&requests.last().unwrap().input).unwrap();
    assert!(text.contains("Optimization round 1"), "{text}");
    assert!(text.contains("phase plan: 0.1s"), "{text}");
    assert!(record["stopped"].as_str().unwrap().contains("1 kept"));
    cleanup(&work);
}

#[tokio::test]
async fn a_regression_is_rejected_and_the_last_passing_workspace_restored() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    cleanup(&work);
    let mut replies = cost_harness(&work);
    replies.extend(first_session(&work, "4"));
    replies.extend([write("o1", "value.txt", "4.5\n"), finish("o2", "done")]);
    let (record, _) = run_loop(
        dir.path(),
        COST_TASK,
        cost_jev(),
        replies,
        shape(
            1,
            Some(OptimizeRule {
                rounds: 1,
                wall_sec: 300,
                spend_usd: 1.0,
            }),
        ),
    )
    .await;
    let rounds = moves_of(&record, "lean.optimize");
    assert_eq!(rounds[0]["kept"], false);
    assert_eq!(rounds[0]["restored"], true);
    assert_eq!(rounds[0]["after"]["value"], 4.5);
    assert_eq!(read(&work, "value.txt"), "4");
    cleanup(&work);
}

#[tokio::test]
async fn a_change_that_breaks_acceptance_is_restored_whatever_it_measures() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    cleanup(&work);
    let mut replies = cost_harness(&work);
    replies.extend(first_session(&work, "4"));
    replies.extend([
        write("o1", "value.txt", "1\n"),
        write("o2", "plan.txt", "nothing\n"),
        finish("o3", "done"),
    ]);
    let (record, _) = run_loop(
        dir.path(),
        COST_TASK,
        cost_jev(),
        replies,
        shape(
            1,
            Some(OptimizeRule {
                rounds: 1,
                wall_sec: 300,
                spend_usd: 1.0,
            }),
        ),
    )
    .await;
    let rounds = moves_of(&record, "lean.optimize");
    assert_eq!(rounds[0]["kept"], false);
    assert_eq!(rounds[0]["acceptance"]["passed"], false);
    assert!(rounds[0]["after"].is_null(), "not measured once rejected");
    assert_eq!(read(&work, "value.txt"), "4");
    assert_eq!(read(&work, "plan.txt"), "plan");
    cleanup(&work);
}

const SPEED_TASK: &str = "Make the solver at least 3x faster than the original code, \
and write plan.txt.";

#[tokio::test]
async fn a_relative_target_is_measured_against_a_noisy_reference() {
    let dir = tempfile::tempdir().unwrap();
    let work = dir.path().join("work");
    cleanup(&work);
    let harness_dir = metric_dir(&work, Isolation::TaskContainer);
    let counter = dir.path().join("counter");
    // The reference takes 10, 11, or 12 seconds by turns; the candidate
    // what speed.txt says, plus up to 0.4.
    let script = format!(
        "mkdir -p {d} && printf '%s\\n' \
         'n=$(cat {c} 2>/dev/null || echo 0); n=$((n + 1)); echo $n > {c}' \
         'if [ \"$1\" = reference ]; then echo \"METRIC $((10 + n % 3))\"; exit 0; fi' \
         '[ -f speed.txt ] || exit 1' \
         'echo \"METRIC $(cat speed.txt).$((n % 5))\"' > {d}/harness.sh",
        d = harness_dir.display(),
        c = counter.display()
    );
    let mut replies = vec![run("h1", &script), finish("h2", "done")];
    let eval = lean::eval_dir(&work, Isolation::TaskContainer);
    replies.extend([
        run(
            "a1",
            &format!(
                "mkdir -p {e} && printf '%s\\n' 'if grep -q plan plan.txt; then echo SCORE 1 1; \
                 else echo SCORE 0 1; fi' > {e}/score.sh",
                e = eval.display()
            ),
        ),
        write("a2", "plan.txt", "plan\n"),
        write("a3", "speed.txt", "2\n"),
        finish("a4", "done"),
    ]);
    let jev = recorded(SPEED_TASK, "at_least", "speedup", "original");
    let (record, _) = run_loop(dir.path(), SPEED_TASK, jev, replies, shape(1, None)).await;
    let setup = &moves_of(&record, "lean.metric_target")[0];
    assert_eq!(setup["targets"][0]["relative"]["kind"], "original");
    assert_eq!(setup["untouched"]["verdict"], "unmeasured");
    let m = &moves_of(&record, "lean")[0]["metric"];
    assert_eq!(m["verdict"], "met", "{m}");
    // One warmup and three recorded runs of each side.
    assert_eq!(m["runs"], 8);
    let values: Vec<f64> = m["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(values.len(), 3);
    assert!(values.iter().all(|v| (4.0..=6.0).contains(v)), "{values:?}");
    assert!(m["spread"].as_f64().unwrap() > 0.0, "{m}");
    assert!(
        record["stopped"]
            .as_str()
            .unwrap()
            .contains("the stated target is met")
    );
    cleanup(&work);
}

#[test]
fn the_switches_validate_and_stay_off_by_default() {
    let lean: lean::Lean =
        serde_json::from_value(json!({"sessions": 1, "source_chars": 10})).unwrap();
    assert!(lean.metric_target.is_none() && lean.optimize.is_none());
    let text = serde_json::to_string(&lean).unwrap();
    assert!(!text.contains("metric_target") && !text.contains("optimize"));
    let mut bad = shape(
        1,
        Some(OptimizeRule {
            rounds: 1,
            wall_sec: 10,
            spend_usd: 1.0,
        }),
    );
    bad.metric_target = None;
    bad.keep_best = false;
    let problems = bad.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("optimize requires metric_target"))
    );
    assert!(
        problems
            .iter()
            .any(|p| p.contains("optimize requires keep_best"))
    );
    let parsed: MetricTargetRule = serde_json::from_value(json!({})).unwrap();
    assert_eq!(parsed.protocol, Protocol::default());
    assert!(parsed.finish && parsed.write_harness);
}
