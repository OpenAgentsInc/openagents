//! Tests for [`crate::runs_analysis`], over a small synthetic trial: a
//! suite writer, two edit sessions where the second reverts the first
//! under a guard's pressure, and a repair that restores the first.

use std::path::Path;

use serde_json::{Value, json};

use super::*;
use crate::runs_analysis_suite as suite;
use crate::runs_learning::Recorded;

const T0: i64 = 1_790_000_000_000;
const JOB: &str = "tb4--coder-one-microluna-demo--demo-task--t1";
const TRIAL: &str = "demo-task__abc";

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    std::fs::write(path, text).expect("a file");
}

fn lines(records: &[Value]) -> String {
    records.iter().map(|r| format!("{r}\n")).collect::<String>()
}

fn start(id: &str, parent: Option<&str>, component: &str, name: &str, at: i64) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "System", "message": "",
        "extensions": {"invocation": {"event": "start", "id": id, "parent": parent,
            "component": component, "name": name, "at": T0 + at,
            "implementation": {"name": "policy coder-one-demo", "digest": "abc123"}}}}})
}

fn end(id: &str, at: i64, summary: Value, cost: Option<f64>) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "System", "message": "",
        "extensions": {"invocation": {"event": "end", "id": id, "at": T0 + at,
            "outcome": "completed", "output": {"summary": summary},
            "cost": cost.map(|usd| json!({"usd": usd}))}}}})
}

fn suite_run(owner: &str, at: i64, label: &str, tests: &[(&str, bool)]) -> Value {
    let passed = tests.iter().filter(|(_, green)| *green).count();
    json!({"record": "step", "step": {"at": T0 + at, "source": "System",
        "message": format!("accept.run {label}"),
        "extensions": {"invocation_id": owner, "accept.run.v1": {
            "label": label, "passed": passed, "total": tests.len(),
            "green": passed == tests.len(),
            "tests": tests.iter().map(|(id, green)| json!({"id": id, "green": green,
                "killed": false, "milliseconds": 900})).collect::<Vec<_>>()}}}})
}

fn session(id: &str, at: i64, brief: &str, steps: &[Value], end_at: i64) -> String {
    let mut records = vec![
        json!({"record": "session", "at": T0 + at, "session": {"id": id,
            "directive": format!("{id} works"), "repository": "/app",
            "version": "coder-one 0.1.0 (demo)"}}),
        json!({"record": "step", "step": {"at": T0 + at, "source": "User", "message": brief}}),
    ];
    records.extend(steps.iter().cloned());
    records.push(json!({"record": "end", "at": T0 + end_at, "state": "ended"}));
    lines(&records)
}

fn turn(at: i64, ms: u64, cost: f64) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "Agent", "message": "",
        "milliseconds": ms, "tokens": [1000, 50], "extensions": {"microluna.usage.v1": {
            "input": 1000, "cached": 500, "output": 50, "reasoning": 10, "cost_usd": cost}}}})
}

fn patch(at: i64, text: &str) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "Agent", "message": "",
        "call": {"id": "c", "name": "apply_patch", "arguments": {"patch": text},
            "output": "Applied", "outcome": "Completed", "milliseconds": 0}}})
}

fn shell(at: i64, text: &str, output: &str, ms: u64) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "Agent", "message": "",
        "call": {"id": "c", "name": "run_command",
            "arguments": {"command": text, "timeout_seconds": 120_000},
            "output": output, "outcome": "Completed", "milliseconds": ms}}})
}

fn finish(at: i64) -> Value {
    json!({"record": "step", "step": {"at": T0 + at, "source": "Agent", "message": "",
        "call": {"id": "c", "name": "finish",
            "arguments": {"status": "done", "summary": "Fixed the mean."},
            "output": "Session finished.", "outcome": "Completed", "milliseconds": 0}}})
}

/// A timestamp `offset` milliseconds after 14:13:20 UTC on some day; only
/// the differences matter.
fn iso(offset: i64) -> String {
    let total = 20_000 + offset;
    let seconds = total.div_euclid(1000);
    format!(
        "2026-09-21T14:{:02}:{:02}.{:03}Z",
        13 + seconds / 60,
        seconds % 60,
        total.rem_euclid(1000)
    )
}

/// The synthetic trial under a fresh directory, and the catalog's sources.
fn fixture() -> (tempfile::TempDir, crate::runs::Sources) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let root = dir.path();
    let task = root.join("tasks/demo-task");
    put(
        &task.join("tests/test_outputs.py"),
        "import os\n\n\ndef test_mean_is_unbiased():\n    \"\"\"The mean must be unbiased. More text.\n\n    Details.\"\"\"\n    from pkg.stats import mean_of\n    assert mean_of([1.0, 2.0]) == 1.5, (\n        \"biased\"\n    )\n\n\ndef test_cli_runs():\n    assert run_cli() == 0\n",
    );
    let trial = root.join("jobs").join(JOB).join(TRIAL);
    put(
        &trial.join("config.json"),
        &json!({"task": {"path": task}, "agent": {"name": "coder-one-tunable",
            "import_path": "coder_one:Agent"}})
        .to_string(),
    );
    put(
        &trial.join("result.json"),
        &json!({
            "task_name": "terminal-bench/demo-task",
            "started_at": iso(-5_000), "finished_at": iso(60_000),
            "agent_execution": {"started_at": iso(-100), "finished_at": iso(40_200)},
            "verifier": {"started_at": iso(41_000), "finished_at": iso(50_000)},
            "agent_result": {"cost_usd": 0.004_042},
            "verifier_result": {"rewards": {"reward": 1.0}},
            "exception_info": null,
        })
        .to_string(),
    );
    put(
        &trial.join("verifier/ctrf.json"),
        &json!({"results": {"tests": [
            {"name": "test_outputs.py::test_mean_is_unbiased", "status": "passed"},
            {"name": "test_outputs.py::test_cli_runs", "status": "passed"},
        ]}})
        .to_string(),
    );
    put(
        &trial.join("verifier/test-stdout.txt"),
        "===== 2 passed in 1.50s =====\n",
    );
    put(
        &trial.join("artifacts/app/pkg/stats.py"),
        "def mean_of(values):\n    # unbiased estimator excludes the diagonal terms\n    return unbiased_mean(values, correction=1)\n",
    );
    let episode = trial.join("agent/episode");
    put(&episode.join("manifest.json"), "{}");
    put(
        &episode.join("evaluation/usage.json"),
        &json!({"cost": {"amount_usd": 0.004_042}, "components": {
            "generation": {"cost_usd": 0.0},
            "jev": {"requests": 1, "cost_usd": 0.000_042},
            "delegate": {"agent": "microluna", "agents": ["microluna"], "cost_usd": 0.004}}})
        .to_string(),
    );
    put(
        &episode.join("accept-suite-1.accept.json"),
        &json!({"dir": "/opt/openagents/episode/accept-suite-1", "status": "partial",
            "digest": "feedfacecafe00", "tests": [
                {"id": "T1", "requirements": ["R1"], "what": "The stats mean is unbiased.", "path": "tests/T1.sh"},
                {"id": "T2", "requirements": ["R1"], "what": "The mean of one value is that value.", "path": "tests/T2.sh"},
            ],
            "start": {"tests": [{"id": "T1", "green": true}, {"id": "T2", "green": false}]}})
        .to_string(),
    );
    put(
        &episode.join("accept-suite-1/tests/T1.sh"),
        "python3 - <<'PY'\nfrom pkg.stats import mean_of\nassert mean_of([2.0, 2.0]) == 2.0\nPY\n",
    );
    put(
        &episode.join("accept-suite-1/tests/T2.sh"),
        "python3 - <<'PY'\nfrom pkg.stats import mean_of\nassert mean_of([3.0]) == 3.0  # unbiased\nPY\n",
    );
    let log = vec![
        json!({"record": "session", "at": T0, "session": {"id": "coder-one",
            "version": "coder-one 0.1.0 (demo)"}}),
        start("inv-1", None, "episode", "openagents.coder.episode.v1", 0),
        start(
            "inv-2",
            Some("inv-1"),
            "exec.session",
            "microluna (gpt-6-luna)",
            100,
        ),
        start(
            "inv-3",
            Some("inv-2"),
            "accept.define",
            "accept.define",
            100,
        ),
        start(
            "inv-4",
            Some("inv-3"),
            "accept.define",
            "jev_accept_test",
            10_100,
        ),
        json!({"record": "step", "step": {"at": T0 + 10_300, "source": "Agent",
            "message": "", "extensions": {"invocation_id": "inv-4",
            "jev_usage": {"input_tokens": 1000}}}}),
        end("inv-4", 10_300, json!({}), Some(0.000_042)),
        end(
            "inv-3",
            10_400,
            json!({"status": "partial", "tests": 2, "gaps": []}),
            None,
        ),
        start("inv-5", Some("inv-2"), "accept.run", "start", 10_500),
        suite_run("inv-5", 12_500, "start", &[("T1", true), ("T2", false)]),
        end("inv-5", 12_500, json!({"passed": 1, "total": 2}), None),
        start(
            "inv-6",
            Some("inv-2"),
            "microluna.session",
            "session 1: R1",
            12_600,
        ),
        end(
            "inv-6",
            20_000,
            json!({"status": "done", "trace": "artifacts/microluna-1-1.atif.jsonl"}),
            None,
        ),
        start(
            "inv-7",
            Some("inv-2"),
            "accept.run",
            "after session 1",
            20_100,
        ),
        suite_run(
            "inv-2",
            22_000,
            "after session 1",
            &[("T1", false), ("T2", true)],
        ),
        end("inv-7", 22_000, json!({"passed": 1, "total": 2}), None),
        start(
            "inv-8",
            Some("inv-2"),
            "microluna.session",
            "session 2: R1",
            22_100,
        ),
        end(
            "inv-8",
            30_000,
            json!({"status": "done", "trace": "artifacts/microluna-1-2.atif.jsonl"}),
            None,
        ),
        start(
            "inv-9",
            Some("inv-2"),
            "accept.run",
            "after session 2",
            30_100,
        ),
        suite_run(
            "inv-9",
            32_000,
            "after session 2",
            &[("T1", true), ("T2", true)],
        ),
        end(
            "inv-9",
            32_000,
            json!({"passed": 2, "total": 2, "green": true}),
            None,
        ),
        end("inv-2", 32_100, json!({"status": "answered"}), Some(0.003)),
        start(
            "inv-10",
            Some("inv-1"),
            "verify.repair",
            "packet brief",
            32_200,
        ),
        start(
            "inv-11",
            Some("inv-10"),
            "exec.session",
            "repair · fresh session",
            32_250,
        ),
        start(
            "inv-12",
            Some("inv-11"),
            "microluna.session",
            "session 1: the briefing",
            32_300,
        ),
        end(
            "inv-12",
            40_000,
            json!({"status": "done", "trace": "artifacts/microluna-2-1.atif.jsonl"}),
            None,
        ),
        end("inv-11", 40_000, json!({}), None),
        end("inv-10", 40_050, json!({"changed": true}), None),
        end("inv-1", 40_100, json!({"outcome": "delegated"}), None),
    ];
    put(&episode.join("episode.atif.jsonl"), &lines(&log));
    let artifacts = episode.join("artifacts");
    put(
        &artifacts.join("accept-writer-1.atif.jsonl"),
        &session(
            "accept-writer-1",
            200,
            "Write the suite.",
            &[turn(9_000, 8_000, 0.01), finish(9_900)],
            10_000,
        ),
    );
    put(
        &artifacts.join("microluna-1-1.atif.jsonl"),
        &session(
            "microluna-1-1",
            12_600,
            "# Current state\n- T2 (R1) is red, exit 1: the mean of one value.\n",
            &[
                turn(13_000, 300, 0.002),
                patch(
                    14_000,
                    "*** Begin Patch\n*** Update File: /app/pkg/stats.py\n@@ def mean_of(values):\n-    return biased_mean(values)\n+    # unbiased estimator excludes the diagonal terms\n+    return unbiased_mean(values, correction=1)\n*** End Patch",
                ),
                shell(
                    17_000,
                    "sh /opt/openagents/episode/accept-suite-1/run.sh",
                    "[exit 0]\nGREEN T2",
                    2_000,
                ),
                shell(
                    19_000,
                    "time python x.py",
                    "[exit 127]\nsh: time: not found",
                    10,
                ),
                finish(19_900),
            ],
            20_000,
        ),
    );
    put(
        &artifacts.join("microluna-1-2.atif.jsonl"),
        &session(
            "microluna-1-2",
            22_100,
            "# Current state\n- T1 (R1) is red, exit 1: A guard: it passes on the untouched workspace.\n",
            &[
                turn(23_000, 700, 0.001),
                patch(
                    24_000,
                    "*** Begin Patch\n*** Update File: pkg/stats.py\n@@\n-    # unbiased estimator excludes the diagonal terms\n-    return unbiased_mean(values, correction=1)\n+    return biased_mean(values, keep=True)\n*** End Patch",
                ),
                finish(29_900),
            ],
            30_000,
        ),
    );
    put(
        &artifacts.join("microluna-2-1.atif.jsonl"),
        &session(
            "microluna-2-1",
            32_300,
            "Repair R1: the code contradicts its docstring.",
            &[
                turn(33_000, 600, 0.001),
                patch(
                    35_000,
                    "*** Begin Patch\n*** Update File: /app/pkg/stats.py\n@@\n-    return biased_mean(values, keep=True)\n+    # unbiased estimator excludes the diagonal terms\n+    return unbiased_mean(values, correction=1)\n*** End Patch",
                ),
            ],
            40_000,
        ),
    );
    let sources = crate::runs::Sources {
        jobs: Some(root.join("jobs")),
        traces: None,
        tasks: Vec::new(),
        index: None,
    };
    (dir, sources)
}

fn fable_manifest() -> Value {
    json!({"trials": [
        {"id": "a", "task": "demo-task", "effort": "low", "reward": 1.0, "cost_usd": 0.5,
         "started_at": "2026-09-01T00:00:00Z", "finished_at": "2026-09-01T00:02:00Z", "steps": 7},
        {"id": "b", "task": "demo-task", "effort": "low", "reward": 1.0, "cost_usd": 0.7,
         "started_at": "2026-09-01T00:00:00Z", "finished_at": "2026-09-01T00:04:00Z", "steps": 9},
        {"id": "c", "task": "demo-task", "effort": "max", "reward": 0.0, "cost_usd": 3.0,
         "started_at": "2026-09-01T00:00:00Z", "finished_at": "2026-09-01T00:20:00Z", "steps": 30},
        {"id": "d", "task": "other-task", "effort": "low", "reward": 1.0, "cost_usd": 0.1,
         "started_at": "2026-09-01T00:00:00Z", "finished_at": "2026-09-01T00:01:00Z", "steps": 3},
    ]})
}

fn load() -> (tempfile::TempDir, Run) {
    let (dir, sources) = fixture();
    let catalog = Catalog::load(sources);
    let run = catalog.find(JOB).expect("the fixture run").clone();
    (dir, run)
}

#[test]
fn the_outcome_cost_and_time_come_from_the_records() {
    let (_dir, run) = load();
    let analysis = compute(&Records::load(&run), Some(&fable_manifest()));
    assert_eq!(analysis.verdict.outcome, "passed");
    assert_eq!((analysis.verdict.passed, analysis.verdict.total), (2, 2));
    assert_eq!(analysis.verdict.seconds, Some(1.5));
    let cost = &analysis.cost;
    assert_eq!(cost.sessions.len(), 4);
    assert!((cost.luna_usd - 0.014).abs() < 1e-9, "{}", cost.luna_usd);
    assert_eq!(cost.jev_requests, 1);
    assert!((cost.total_usd - 0.014_042).abs() < 1e-9);
    assert_eq!(cost.harbor_usd, Some(0.004_042));
    assert_eq!(
        cost.harbor_missing.as_deref(),
        Some("the suite writer ($0.0100)")
    );
    let spans: Vec<&str> = analysis.spans.iter().map(|s| s.span.as_str()).collect();
    assert!(spans.contains(&"Agent execution") && spans.contains(&"Verifier"));
    let verifier = analysis.spans.iter().find(|s| s.span == "Verifier");
    assert_eq!(verifier.map(|s| s.duration_ms), Some(9_000));
    assert_eq!(analysis.run.policy.as_deref(), Some("coder-one-demo"));
    let fable = analysis.fable.expect("Fable's attempts on the task");
    assert_eq!((fable.attempts, fable.passes), (3, 2));
    let cheapest = fable.cheapest_pass.expect("a cheapest pass");
    assert_eq!((cheapest.id.as_str(), cheapest.seconds), ("a", 120.0));
    let tier = fable.tier.expect("the low tier");
    assert_eq!(tier.effort, "low");
    assert!((tier.mean_usd - 0.6).abs() < 1e-9 && (tier.mean_sec - 180.0).abs() < 1e-9);
}

#[test]
fn the_timeline_places_sessions_and_walks_the_critical_path() {
    let (_dir, run) = load();
    let records = Records::load(&run);
    let roles: Vec<(String, Role)> = records
        .sessions
        .iter()
        .map(|s| (s.id.clone(), s.role))
        .collect();
    assert_eq!(
        roles,
        vec![
            ("accept-writer-1".to_owned(), Role::Writer),
            ("microluna-1-1".to_owned(), Role::Edit),
            ("microluna-1-2".to_owned(), Role::Edit),
            ("microluna-2-1".to_owned(), Role::Repair),
        ]
    );
    let timeline = compute(&records, None).timeline.expect("a timeline");
    assert_eq!(timeline.episode_ms, 40_100);
    assert_eq!(timeline.first_edit_ms, Some(14_000));
    let total: u64 = timeline.critical_path.iter().map(|s| s.duration_ms).sum();
    assert_eq!(total, 40_100, "{:#?}", timeline.critical_path);
    assert!(
        timeline
            .critical_path
            .iter()
            .any(|s| s.label.starts_with("microluna-1-2"))
    );
    assert!(
        timeline
            .categories
            .iter()
            .any(|(name, _)| name == "Luna model latency")
    );
    // A suite run the dispatch recorded still starts at its own
    // invocation.
    let after_one = &records.episode.as_ref().expect("an episode").suite_runs[1];
    assert_eq!(after_one.start - T0, 20_100);
    let phases: Vec<&str> = timeline
        .cost_by_phase
        .iter()
        .map(|p| p.phase.as_str())
        .collect();
    assert!(phases.contains(&"suite writing") && phases.contains(&"repair"));
    assert_eq!(timeline.peak_sessions, 1);
}

#[test]
fn a_guard_driven_revert_and_its_restore_are_found() {
    let (_dir, run) = load();
    let analysis = compute(&Records::load(&run), None);
    let [reversal] = analysis.reversals.as_slice() else {
        panic!("one reversal: {:#?}", analysis.reversals);
    };
    assert_eq!(reversal.kind, "revert");
    assert_eq!(
        (reversal.earlier.as_str(), reversal.later.as_str()),
        ("microluna-1-1", "microluna-1-2")
    );
    assert_eq!(reversal.file, "pkg/stats.py");
    assert_eq!(reversal.undone, 2);
    assert_eq!(reversal.symbols, vec!["mean_of".to_owned()]);
    assert_eq!(reversal.restored_by.as_deref(), Some("microluna-2-1"));
    assert_eq!(reversal.guards, vec!["T1".to_owned()]);
    assert!(
        reversal
            .better_then_reverted
            .as_deref()
            .is_some_and(|why| why.contains("passed the verifier"))
    );
    let [edit] = analysis.guard_edits.as_slice() else {
        panic!("one guard edit: {:#?}", analysis.guard_edits);
    };
    assert_eq!(edit.test, "T1");
    assert_eq!(edit.red_in, "the run `after session 1`");
    let kinds: Vec<&str> = analysis.anomalies.iter().map(|a| a.kind.as_str()).collect();
    for kind in [
        "no finish",
        "tool friction",
        "unchecked edits",
        "cost mismatch",
        "reversal",
    ] {
        assert!(kinds.contains(&kind), "{kind} in {kinds:?}");
    }
}

#[test]
fn the_suite_mapping_asks_jev_only_about_open_pairs_and_caches_the_answer() {
    let (dir, run) = load();
    let records = Records::load(&run);
    let tests = suite::tests(&records);
    assert_eq!(tests.len(), 2);
    assert!(tests[0].guard && !tests[1].guard);
    assert_eq!(tests[0].last, Some(true));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime");
    let cache_dir = dir.path().join("cache");
    let mut cache = suite::Cache::open(Some(cache_dir.clone()));
    let off = Judge::Off("test".to_owned());
    let (section, jev) = runtime.block_on(suite::section(&records, &off, &mut cache, None));
    let section = section.expect("a suite section");
    // Both acceptance tests call `mean_of`: the pair is open, and without
    // Jev the rules' guess is labeled.
    let unbiased = &section.verifier[0];
    assert_eq!(unbiased.verifier_test, "test_mean_is_unbiased");
    assert_eq!(unbiased.decided_by, "rules only");
    assert_eq!(jev.errors.len(), 1);
    // The CLI test has no candidate: uncovered by the rules alone.
    assert_eq!(section.uncovered, vec!["test_cli_runs".to_owned()]);

    let chosen: Vec<&suite::AcceptTest> = unbiased
        .candidates
        .iter()
        .map(|c| tests.iter().find(|t| t.id == c.test).expect("a test"))
        .collect();
    let ids: Vec<&str> = chosen.iter().map(|t| t.id.as_str()).collect();
    let key = suite::key(
        &suite::state(&records.verifier[0], &chosen),
        &suite::questions(&ids),
    );
    let mut recorded = Recorded::empty();
    recorded.entries.insert(
        key,
        json!({
            "checks_T1": {"type": "noul", "noul": 0.9},
            "contradicts_T1": {"type": "noul", "noul": 0.1},
            "checks_T2": {"type": "noul", "noul": 0.2},
            "contradicts_T2": {"type": "noul", "noul": 0.8},
        }),
    );
    let judge = Judge::Recorded(recorded);
    let (section, jev) = runtime.block_on(suite::section(&records, &judge, &mut cache, None));
    let section = section.expect("a suite section");
    assert_eq!(jev.asked, 1);
    let unbiased = &section.verifier[0];
    assert_eq!(unbiased.decided_by, "jev");
    assert_eq!(unbiased.covered, "contradicted");
    assert_eq!(unbiased.by, vec!["T2".to_owned()]);
    assert_eq!(
        section.contradicted,
        vec!["test_mean_is_unbiased".to_owned()]
    );
    let anomalies = suite::anomalies(&section);
    assert!(anomalies.iter().any(|a| a.kind == "contradicting test"));

    // A later pass reads the answer from disk and asks nothing.
    let mut fresh = suite::Cache::open(Some(cache_dir));
    let (_, jev) = runtime.block_on(suite::section(&records, &off, &mut fresh, None));
    assert_eq!((jev.asked, jev.cached), (0, 1));
}

#[test]
fn write_keeps_markdown_and_json_beside_the_run() {
    let (dir, run) = load();
    let analysis = analyze(
        &run,
        &Judge::Off("test".to_owned()),
        Some(dir.path().join("cache")),
        Some(&fable_manifest()),
        None,
    )
    .expect("an analysis");
    let (markdown, json_path) = write(&run, &analysis).expect("written");
    assert_eq!(markdown, run.files.dir.join(MARKDOWN_FILE));
    let text = std::fs::read_to_string(&markdown).expect("the Markdown");
    for heading in [
        "# Run analysis: `demo-task`",
        "## Summary",
        "### The verifier",
        "### Per phase",
        "### The critical path and the concurrency",
        "## The suite against the verifier",
        "## Reversals",
        "## Anomalies",
        "## Against Fable 5.1",
    ] {
        assert!(text.contains(heading), "{heading} in\n{text}");
    }
    assert!(text.contains("reverted `microluna-1-1`'s change to `pkg/stats.py`"));
    assert_eq!(stored_markdown(&run).as_deref(), Some(text.as_str()));
    let value: Value = serde_json::from_str(&std::fs::read_to_string(json_path).expect("JSON"))
        .expect("valid JSON");
    assert_eq!(value["schema"], SCHEMA);
    assert_eq!(value["reversals"][0]["restored_by"], "microluna-2-1");
    // The pane wraps it and keeps the headings.
    let lines = crate::runs_analysis_markdown::pane_lines(&text, 60);
    assert!(
        lines
            .iter()
            .any(|(line, heading)| *heading && line == "Reversals")
    );
    assert!(
        lines
            .iter()
            .all(|(line, _)| line.chars().count() <= 60 || line.contains("  "))
    );
}

#[test]
fn the_command_prints_and_writes() {
    let (dir, sources) = fixture();
    let jobs = sources.jobs.expect("a jobs directory");
    let args: Vec<String> = [
        JOB,
        "--jobs-dir",
        jobs.to_str().expect("a path"),
        "--no-traces",
        "--no-jev",
        "--cache-dir",
        dir.path().join("cache").to_str().expect("a path"),
        "--write",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let mut out = Vec::new();
    assert_eq!(command(&args, &mut out), Ok(0));
    let printed = String::from_utf8(out).expect("text");
    assert!(printed.contains("analysis.md") && printed.contains("analysis.json"));
    assert!(jobs.join(JOB).join(TRIAL).join(JSON_FILE).is_file());
    let mut out = Vec::new();
    assert!(command(&["no-such-run".to_owned(), "--no-jev".to_owned()], &mut out).is_err());
}

#[test]
fn the_parsers_read_what_the_tools_wrote() {
    let changes = patch_changes(
        "*** Begin Patch\n*** Update File: /app/a.py\n@@\n-    old_line(value)\n+    new_line(value)\n+    moved_line(x)\n-    moved_line(x)\n*** Add File: /app/b.py\n+print('hello')\n*** End Patch",
        "/app",
    );
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].file, "a.py");
    assert_eq!(changes[0].removed, vec!["old_line(value)".to_owned()]);
    assert_eq!(changes[0].added, vec!["new_line(value)".to_owned()]);
    assert_eq!(changes[1].file, "b.py");
    assert_eq!(
        red_tests("- T10 (R4) is red, exit 1: x\nT3 is green\nT4 (R1) is red"),
        vec!["T10".to_owned(), "T4".to_owned()]
    );
    assert_eq!(
        docstring("def t():\n    \"\"\"First sentence here. Second.\n\n    More.\"\"\"\n"),
        Some("First sentence here.".to_owned())
    );
    assert_eq!(
        assertions(
            "def t():\n    x = 1\n    assert x == 1, (\n        'no'\n    )\n    assert y\n"
        ),
        vec![
            "assert x == 1, (\n    'no'\n)".to_owned(),
            "assert y".to_owned()
        ]
    );
    assert!(similarity("mmd = K_rr.mean()", "mmd = K_rr.mean() + K_cc.mean()") >= 0.5);
    assert_eq!(offset(210_500), "03:30.5");
    assert_eq!(span(450), "0.45 s");
    assert_eq!(span(207_700), "3:27.7");
    assert_eq!(long(1_034_100), "17 min 14 s");
    assert_eq!(usd(0.035_66), "$0.0357");
    assert_eq!(usd(0.007_55), "$0.00755");
    assert_eq!(usd(0.87), "$0.87");
    let found = suite::symbols(
        "from pkg.stats import mean_of, other as o\nx = np.array([1])\nresult = obj.method(2)\nassert len(x)\n",
    );
    assert_eq!(
        found.into_iter().collect::<Vec<_>>(),
        vec![
            "mean_of".to_owned(),
            "method".to_owned(),
            "other".to_owned()
        ]
    );
    let words = suite::words("test_mmd_uses_unbiased_estimators for the windows");
    assert!(words.contains("unbiased") && words.contains("estimator") && !words.contains("the"));
}

#[test]
fn a_run_without_coder_one_records_still_reads() {
    let (dir, sources) = fixture();
    let trial = dir.path().join("jobs").join(JOB).join(TRIAL);
    std::fs::remove_dir_all(trial.join("agent")).expect("removed");
    let catalog = Catalog::load(sources);
    let run = catalog.find(JOB).expect("the run");
    let analysis = compute(&Records::load(run), None);
    assert!(analysis.timeline.is_none());
    assert!(analysis.reversals.is_empty());
    assert_eq!(analysis.cost.total_usd, 0.004_042);
    let text = crate::runs_analysis_markdown::render(&analysis);
    assert!(text.contains("The run kept no session patches"));
}
