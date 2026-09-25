//! Tests for [`crate::runs_card`]: the three retained v13 trials against
//! the published reconstruction, a trial with almost no records, and the
//! record shapes the sibling issues write.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;
use crate::runs::{Catalog, Sources};
use crate::runs_card_render::{Row, card_json, diff, markdown, rows};

fn traces() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")
}

/// A catalog over only the named retained jobs, linked into a temporary
/// traces directory so the test doesn't read all of them.
fn catalog_of(jobs: &[&str]) -> (tempfile::TempDir, Catalog) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for job in jobs {
        std::os::unix::fs::symlink(traces().join(job), dir.path().join(job)).expect("a link");
    }
    let catalog = Catalog::load(Sources {
        jobs: None,
        traces: Some(dir.path().to_path_buf()),
        tasks: Vec::new(),
        index: None,
    });
    (dir, catalog)
}

fn row<'a>(rows: &'a [Row], id: &str) -> &'a Row {
    rows.iter()
        .find(|row| row.id == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn secs(rows: &[Row], id: &str) -> f64 {
    row(rows, id)
        .value
        .as_f64()
        .unwrap_or_else(|| panic!("{id} is {:?}", row(rows, id).value))
}

fn near(actual: f64, expected: f64, within: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= within,
        "{what}: {actual} against {expected}"
    );
}

const V13: [(&str, &str); 3] = [
    (
        "tb4--coder-one-microluna-v13-retained--embedding-drift-monitor",
        "embedding-drift-monitor__6zRjd9n",
    ),
    (
        "tb4--coder-one-microluna-v13-retained--embedding-drift-monitor-2",
        "embedding-drift-monitor__y2bahob",
    ),
    (
        "tb4--coder-one-microluna-v13-retained--embedding-drift-monitor-3",
        "embedding-drift-monitor__QAHE7De",
    ),
];

/// What `docs/terminal-bench/2026-09-25-microluna-v13-embedding-trials.md`
/// publishes for one trial.
struct Published {
    setup: f64,
    host_before: f64,
    session_1: f64,
    host_between: f64,
    session_2: f64,
    /// The page's "host after" session 2.
    host_after: f64,
    agent: f64,
    gap: f64,
    verifier: f64,
    trial: f64,
    turns: (usize, usize),
    cost: (f64, f64),
    /// Last-edit turn to finish turn in session 1, from the page's rows.
    tail: (f64, usize),
    untouched: &'static str,
    frozen: &'static str,
    close_p: f64,
}

const PUBLISHED: [Published; 3] = [
    Published {
        setup: 8.0,
        host_before: 5.7,
        session_1: 366.9,
        host_between: 20.7,
        session_2: 93.0,
        host_after: 23.4,
        agent: 511.4,
        gap: 25.4,
        verifier: 166.6,
        trial: 711.4,
        turns: (15, 5),
        cost: (0.010686, 0.002661),
        tail: (94.9, 4),
        untouched: "3 of 8",
        frozen: "8 of 8",
        close_p: 0.69,
    },
    Published {
        setup: 7.9,
        host_before: 5.6,
        session_1: 338.2,
        host_between: 8.5,
        session_2: 79.0,
        host_after: 24.8,
        agent: 466.7,
        gap: 41.2,
        verifier: 64.7,
        trial: 580.1,
        turns: (24, 4),
        cost: (0.013002, 0.003024),
        tail: (37.5, 2),
        untouched: "6 of 10",
        frozen: "10 of 10",
        close_p: 0.74,
    },
    Published {
        setup: 7.3,
        host_before: 5.2,
        session_1: 331.1,
        host_between: 7.1,
        session_2: 71.6,
        host_after: 8.2,
        agent: 424.3,
        gap: 62.9,
        verifier: 117.0,
        trial: 611.6,
        turns: (18, 6),
        cost: (0.011805, 0.003194),
        tail: (62.4, 3),
        untouched: "4 of 6",
        frozen: "6 of 6",
        close_p: 0.66,
    },
];

#[test]
fn the_three_v13_trials_reproduce_the_published_reconstruction() {
    let jobs: Vec<&str> = V13.iter().map(|(job, _)| *job).collect();
    let (_dir, catalog) = catalog_of(&jobs);
    let options = Options::offline();
    let mut python_turns = 0;
    for (index, ((job, trial), page)) in V13.iter().zip(&PUBLISHED).enumerate() {
        let what = |row: &str| format!("trial {} {row}", index + 1);
        let run = catalog
            .find(&format!("{job}/{trial}"))
            .expect("the retained trial");
        let card = characterize(run, &options);
        let rows = rows(&card);
        // Identity.
        assert_eq!(
            card.identity.policy.as_deref(),
            Some("coder-one-microluna-v13-retained")
        );
        assert_eq!(card.identity.revision.as_deref(), Some("452bf305c6da"));
        assert_eq!(
            card.identity.development,
            Some(true),
            "{}",
            what("development")
        );
        assert_eq!(card.identity.reward, Some(1.0));
        // The phase timeline, to the page's tenth of a second.
        let phase = |key: &str| {
            card.phases
                .iter()
                .find(|p| p.key == key)
                .and_then(|p| p.duration_ms)
                .expect("a phase") as f64
                / 1000.0
        };
        near(
            phase("environment_setup") + phase("agent_setup"),
            page.setup,
            0.11,
            &what("setup"),
        );
        near(
            secs(&rows, "phase.host_before_1.s"),
            page.host_before,
            0.11,
            &what("host before"),
        );
        near(
            secs(&rows, "phase.session_1.s"),
            page.session_1,
            0.05,
            &what("session 1"),
        );
        near(
            secs(&rows, "phase.host_after_1.s"),
            page.host_between,
            0.11,
            &what("host between"),
        );
        near(
            secs(&rows, "phase.session_2.s"),
            page.session_2,
            0.05,
            &what("session 2"),
        );
        near(
            secs(&rows, "identity.agent_s"),
            page.agent,
            0.05,
            &what("agent"),
        );
        near(
            secs(&rows, "identity.trial_s"),
            page.trial,
            0.05,
            &what("trial"),
        );
        near(
            secs(&rows, "phase.verifier.s"),
            page.verifier,
            0.05,
            &what("verifier"),
        );
        let after = phase("host_after_2") + phase("close");
        match index {
            // The page's 23.4 s disagrees with its own rows, which run from
            // +486.3 to +510.1: 23.8 s, the card's host-after plus close.
            0 => near(after, 23.8, 0.05, &what("host after")),
            _ => near(after, page.host_after, 0.05, &what("host after")),
        }
        match index {
            // The page puts the verifier's start 0.4 s after Harbor's
            // record of it; its own phases then sum to 580.5 s against a
            // 580.1 s trial. Harbor's record gives 40.7 s.
            1 => near(
                secs(&rows, "phase.gap_to_verifier.s"),
                40.7,
                0.05,
                &what("gap"),
            ),
            _ => near(
                secs(&rows, "phase.gap_to_verifier.s"),
                page.gap,
                0.05,
                &what("gap"),
            ),
        }
        // Session anatomy.
        assert_eq!(card.sessions.len(), 2);
        assert_eq!(
            (card.sessions[0].turns, card.sessions[1].turns),
            page.turns,
            "{}",
            what("turns")
        );
        near(
            card.sessions[0].cost_usd.unwrap(),
            page.cost.0,
            5e-7,
            &what("cost 1"),
        );
        near(
            card.sessions[1].cost_usd.unwrap(),
            page.cost.1,
            5e-7,
            &what("cost 2"),
        );
        assert_eq!(card.sessions[1].role, "self-check");
        let tail = card.sessions[0].tail.as_ref().expect("an edit");
        near(
            tail.to_finish_ms.unwrap() as f64 / 1000.0,
            page.tail.0,
            0.15,
            &what("tail"),
        );
        assert_eq!(tail.turns, page.tail.1, "{}", what("tail turns"));
        // Evidence provenance: the suspects named three of the six sites.
        assert_eq!(row(&rows, "provenance.suspects").value, json!(6));
        assert_eq!(
            row(&rows, "provenance.defect_sites_named").value,
            json!([3, 6]),
            "{}",
            what("sites named")
        );
        assert_eq!(
            row(&rows, "provenance.defect_sites_edited").value,
            json!([6, 6])
        );
        assert_eq!(
            row(&rows, "provenance.pointer_coverage").value,
            json!([3, 7])
        );
        // The check: first scored on the untouched workspace, rewritten
        // after a code edit, frozen full.
        assert_eq!(row(&rows, "checks.untouched_score").text, page.untouched);
        assert_eq!(row(&rows, "checks.frozen_score").text, page.frozen);
        assert_eq!(row(&rows, "checks.rewrites_after_edit").value, json!(1));
        assert_eq!(row(&rows, "checks.line_grades").text, "not recorded");
        assert_eq!(row(&rows, "executed.host_commands").text, "not recorded");
        // Waste: one `python` turn in every session.
        let python = card
            .waste
            .not_found
            .iter()
            .find(|m| m.program == "python")
            .expect("python turns");
        assert_eq!(python.turns, vec!["1.1", "2.2"], "{}", what("python"));
        python_turns += python.turns.len();
        // Claims.
        near(
            card.claims.close_p.unwrap(),
            page.close_p,
            1e-9,
            &what("close"),
        );
        assert_eq!(card.claims.self_score_agrees, Some(true));
        // The review delta.
        let review = &card.review[0];
        if index == 2 {
            // The page says one docstring; the retained workspaces and the
            // self-check's patch change two in `distance.py`: the module's
            // and `cosine_distance`'s. No code changed.
            assert_eq!(review.files.len(), 1);
            assert_eq!(review.files[0].0, "drift_monitor/distance.py");
            assert_eq!(review.changes[0].code_lines, 0);
            assert_eq!(review.changes[0].docstrings, 2);
            assert_eq!(row(&rows, "review.2.code_changed").text, "no");
        } else {
            assert_eq!(review.words(), "nothing");
        }
        if index == 0 {
            // The published 64% model-latency share of trial 1's session 1.
            assert_eq!(row(&rows, "session.1.model_share").text, "64%");
            near(
                secs(&rows, "session.1.model_s"),
                235.0,
                0.5,
                "trial 1 model latency",
            );
            near(
                secs(&rows, "session.1.command_s"),
                107.0,
                0.5,
                "trial 1 command time",
            );
            assert_eq!(row(&rows, "session.1.calls").value, json!(29));
            assert_eq!(row(&rows, "session.2.calls").value, json!(7));
            // At the line level, the submitted workspace changed three of
            // the six suspects' lines in trial 1.
            assert_eq!(
                row(&rows, "provenance.suspect_line_hits").value,
                json!([3, 6])
            );
            assert_eq!(row(&rows, "sessions.cached_share").text, "85%");
        }
        // The page renders, and the record carries every row.
        let page_text = markdown(&card);
        assert!(page_text.contains("## Phase timeline"));
        assert_eq!(
            card_json(&card)["rows"].as_array().map(Vec::len),
            Some(rows.len())
        );
    }
    // Six wasted `python` turns in three trials.
    assert_eq!(python_turns, 6);
}

#[test]
fn a_diff_reports_deltas_between_two_cards_on_one_task() {
    let jobs: Vec<&str> = V13.iter().map(|(job, _)| *job).collect();
    let (_dir, catalog) = catalog_of(&jobs);
    let options = Options::offline();
    let card = |at: usize| {
        let (job, trial) = V13[at];
        rows(&characterize(
            catalog.find(&format!("{job}/{trial}")).expect("a trial"),
            &options,
        ))
    };
    let deltas = diff(&card(0), &card(2));
    let turns = deltas
        .iter()
        .find(|d| d.id == "session.1.turns")
        .expect("turns");
    assert_eq!(turns.delta, Some(3.0));
    let review = deltas
        .iter()
        .find(|d| d.id == "review.2.files")
        .expect("review");
    assert_eq!(review.delta, Some(1.0));
}

fn put(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    std::fs::write(path, text).expect("a file");
}

/// A retained trial with a Harbor result and nothing else.
fn bare_trial(root: &Path) {
    let dir = root.join("tb4--coder-one-demo--demo-task/demo-task__abc.episode");
    put(
        &dir.join("harbor-result.json"),
        &json!({
            "trial_name": "demo-task__abc",
            "task_name": "terminal-bench/demo-task",
            "started_at": "2026-09-24T10:00:00Z",
            "finished_at": "2026-09-24T10:05:00Z",
            "agent_execution": {"started_at": "2026-09-24T10:00:10Z", "finished_at": "2026-09-24T10:04:00Z"},
            "verifier": {"started_at": "2026-09-24T10:04:10Z", "finished_at": "2026-09-24T10:05:00Z"},
            "verifier_result": {"rewards": {"reward": 0.0}},
            "agent_result": {"cost_usd": null},
            "config": {"agent": {"name": "coder-one"}},
        })
        .to_string(),
    );
    put(&dir.join("manifest.json"), "{}");
}

#[test]
fn a_trial_missing_its_records_gets_unknown_rows_not_guesses() {
    let root = tempfile::tempdir().expect("a directory");
    bare_trial(root.path());
    let catalog = Catalog::load(Sources {
        jobs: None,
        traces: Some(root.path().to_path_buf()),
        tasks: Vec::new(),
        index: None,
    });
    let run = catalog.find("demo-task__abc").expect("the trial");
    let card = characterize(run, &Options::default());
    let rows = rows(&card);
    assert!(card.sessions.is_empty());
    for id in [
        "identity.policy",
        "identity.development",
        "provenance.suspect_line_hits",
        "checks.versions",
        "waste.not_found_turns",
        "waste.slow_s",
        "claims.close_p",
    ] {
        assert!(
            row(&rows, id).value.is_null(),
            "{id} is {:?}",
            row(&rows, id)
        );
    }
    assert_eq!(row(&rows, "checks.line_grades").text, "not recorded");
    assert_eq!(row(&rows, "phase.agent_execution.s").value, json!(230.0));
    assert!(
        card.notes
            .iter()
            .any(|n| n.contains("No Microluna session log"))
    );
    assert!(markdown(&card).contains("unknown"));
}

#[test]
fn the_card_reads_grades_and_host_executed_commands_in_their_shapes() {
    let root = tempfile::tempdir().expect("a directory");
    bare_trial(root.path());
    let artifacts = root
        .path()
        .join("tb4--coder-one-demo--demo-task/demo-task__abc.episode/artifacts");
    put(
        &artifacts.join(GRADES_FILE),
        &json!({
            "schema": GRADES_SCHEMA,
            "check": "lean-1/evaluator/score.sh",
            "check_digest": "ab12",
            "frozen_after_session": 1,
            "split": "lines",
            "lines": [
                {"id": "c1", "line": 4, "text": "check(stable_never_alerts)", "grade": "follows", "basis": "task", "p": 0.91,
                 "jev": {"how": "live", "error": null}, "results": [{"session": 1, "passed": false}]},
                {"id": "c2", "line": 9, "text": "check(mmd(x, x) == 0)", "grade": "advisory", "basis": null, "p": 0.12,
                 "jev": {"how": "live", "error": null}, "results": [{"session": 1, "passed": true}]},
            ],
        })
        .to_string(),
    );
    let command = |stage: &str, session: Value, exit: i64| {
        json!({
            "schema": EXECUTED_SCHEMA, "at": 1_790_000_000_000_i64, "stage": stage,
            "session": session, "candidate": null, "kind": "module",
            "command": "python3 -m drift_monitor data/a.npy", "cwd": "/app",
            "exit": exit, "timed_out": false, "ms": 3382,
            "stdout_digest": "00", "stderr_digest": "11",
            "stdout_head": "", "stderr_head": "", "requirements": ["R3"],
            "verdict": if exit == 0 { "ok" } else { "regressed" }, "rule": null,
        })
        .to_string()
    };
    put(
        &artifacts.join(EXECUTED_FILE),
        &format!(
            "{}\n{}\n{{\"schema\": \"something else\"}}\n",
            command("baseline", Value::Null, 0),
            command("after_session", json!(1), 1)
        ),
    );
    let catalog = Catalog::load(Sources {
        jobs: None,
        traces: Some(root.path().to_path_buf()),
        tasks: Vec::new(),
        index: None,
    });
    let card = characterize(
        catalog.find("demo-task__abc").expect("the trial"),
        &Options::default(),
    );
    let rows = rows(&card);
    assert_eq!(row(&rows, "checks.line_grades").value, json!([2, 1]));
    assert_eq!(row(&rows, "executed.host_commands").value, json!(2));
    // Reward 0: a line that failed agrees, a line that passed doesn't.
    assert_eq!(row(&rows, "claims.line_agreement").value, json!([1, 2]));
}

#[test]
fn the_card_shows_which_review_trigger_fired_or_that_none_did() {
    let root = tempfile::tempdir().expect("a directory");
    bare_trial(root.path());
    let catalog = || {
        Catalog::load(Sources {
            jobs: None,
            traces: Some(root.path().to_path_buf()),
            tasks: Vec::new(),
            index: None,
        })
    };
    // No rule recorded: the review ran unconditionally.
    let card = characterize(
        catalog().find("demo-task__abc").expect("trial"),
        &Options::default(),
    );
    let r = rows(&card);
    assert!(row(&r, "review.rule.trigger").value.is_null());
    assert!(
        row(&r, "review.rule.trigger")
            .text
            .starts_with("not recorded")
    );
    let artifacts = root
        .path()
        .join("tb4--coder-one-demo--demo-task/demo-task__abc.episode/artifacts");
    let reading = |trigger: &str, reading: &str| json!({"trigger": trigger, "reading": reading, "detail": format!("{trigger} {reading}")});
    put(
        &artifacts.join("microluna-1.json"),
        &json!({
            "mode": "lean",
            "moves": [
                {"kind": "lean", "after_session": 1},
                {"kind": "lean.review_rule", "session": 1, "review": true,
                 "fired": ["uncovered"], "unknown": ["regressed"], "unknown_fires": true,
                 "trigger": "uncovered",
                 "reason": "the review runs: R3 has no executed check",
                 "triggers": [reading("score", "clear"), reading("regressed", "unknown"),
                              reading("hardcoded", "clear"), reading("uncovered", "fired")]},
                {"kind": "lean", "after_session": 2, "self_check": true},
                {"kind": "lean.review_concerns", "session": 2,
                 "concerns": [{"requirement": "R3", "concern": "rwa.xlsx has a zero RC", "command": "python3 check.py"}]},
            ],
        })
        .to_string(),
    );
    let card = characterize(
        catalog().find("demo-task__abc").expect("trial"),
        &Options::default(),
    );
    let r = rows(&card);
    assert_eq!(row(&r, "review.rule.trigger").value, json!("uncovered"));
    assert_eq!(row(&r, "review.rule.ran").value, json!(true));
    assert_eq!(row(&r, "review.rule.regressed").value, json!("unknown"));
    assert_eq!(row(&r, "review.rule.concerns").value, json!(1));
    let page = markdown(&card);
    assert!(
        page.contains("Concern on R3: rwa.xlsx has a zero RC (`python3 check.py`)"),
        "{page}"
    );
}

#[test]
fn scores_and_missing_programs_read_from_command_output() {
    assert_eq!(score_of("x\nSCORE 3 8\n[stderr]\nwarn"), Some((3, 8)));
    assert_eq!(score_of("no score"), None);
    assert_eq!(
        missing_program("[exit 127]\n[stderr]\n/bin/sh: 1: python: not found").as_deref(),
        Some("python")
    );
    assert_eq!(
        missing_program("bash: git: command not found").as_deref(),
        Some("git")
    );
    assert_eq!(missing_program("FileNotFoundError: data: not found"), None);
}
