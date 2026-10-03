use super::*;

fn row(arm: &str, task: &str, passed: bool, cost: f64, wall: f64) -> String {
    json!({"arm": arm, "task": task, "trial": 1, "passed": passed, "cost_usd": cost, "wall_s": wall}).to_string()
}

#[test]
fn wilson_matches_the_studys_interval() {
    let e = wilson(21, 21).unwrap();
    assert!((e.low - 0.845).abs() < 0.005, "{e:?}");
    assert!((e.high - 1.0).abs() < 1e-9);
    assert!(wilson(0, 0).is_none());
}

#[test]
fn old_and_new_rows_read_alike() {
    let text = [
        row("raw-claude", "mi-one", true, 0.2, 40.0),
        row("routed-claude-on", "fix-git", false, 0.3, 60.0),
        json!({"arm": "routed-default", "task": "mi-one", "passed": true, "cost_usd": 0.1, "wall_s": 50.0,
               "engine": "codex", "mode": "routed", "task_class": "repository"})
        .to_string(),
        "not json".into(),
    ]
    .join("\n");
    let rows = study_rows(&text);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].mode, "raw");
    assert_eq!(rows[0].engine.as_deref(), Some("claude"));
    assert_eq!(rows[0].class, "repository");
    assert_eq!(rows[1].engine.as_deref(), Some("claude"));
    assert_eq!(rows[1].class, "terminal-bench");
    assert_eq!(rows[2].engine.as_deref(), Some("codex"));
}

#[test]
fn cost_per_checked_result_charges_the_failures() {
    let rows = study_rows(
        &[
            row("raw-claude", "a", true, 1.0, 10.0),
            row("raw-claude", "a", false, 1.0, 99.0),
            row("raw-claude", "b", true, 2.0, 30.0),
        ]
        .join("\n"),
    );
    let refs: Vec<&Row> = rows.iter().collect();
    let stats = arm_stats("raw-claude", &refs);
    assert_eq!(stats["passed"], 2);
    assert_eq!(stats["cost_per_checked_usd"]["point"], 2.0);
    // The failed run's time is not a time to a checked result.
    assert_eq!(stats["time_to_checked_s"]["point"], 20.0);
    assert_eq!(stats["cost_total_usd"], 4.0);
}

#[test]
fn a_study_compares_each_arm_with_raw_claude_and_says_where_it_loses() {
    let mut lines = Vec::new();
    for task in ["a", "b", "c"] {
        for t in 0..3 {
            let jitter = f64::from(t) * 0.01;
            lines.push(row("raw-claude", task, true, 1.0 + jitter, 10.0 + jitter));
            lines.push(row(
                "routed-default",
                task,
                true,
                0.5 + jitter,
                20.0 + jitter,
            ));
        }
    }
    let s = study("s", "label", "src", &study_rows(&lines.join("\n")));
    let c = &s["comparisons"][0];
    assert_eq!(c["arm"], "routed-default");
    assert_eq!(c["tasks"], 3);
    assert!((c["cost_ratio"]["point"].as_f64().unwrap() - 0.5).abs() < 0.01);
    assert_eq!(c["cost"], "50% cheaper");
    assert_eq!(c["time"], "100% slower");
    let report = report(&[s], &[], &[]);
    let headline = report["findings"][0].as_str().unwrap();
    assert!(
        headline.contains("was 50% cheaper") && headline.contains("does not win on time yet"),
        "{headline}"
    );
    let words = report["findings"][1].as_str().unwrap();
    assert!(
        words.contains("50% cheaper") && words.contains("100% slower"),
        "{words}"
    );
    let text = text(&report, false);
    assert!(text.contains("raw Claude Code"), "{text}");
    assert!(
        text.contains("Shadow baselines on this computer: none yet"),
        "{text}"
    );
}

#[test]
fn an_interval_that_spans_one_is_no_difference() {
    let e = Estimate {
        point: 1.05,
        low: 0.88,
        high: 1.25,
    };
    assert_eq!(verdict(e, "cost"), "no measurable difference");
}

#[test]
fn shadow_records_pair_and_count_their_checks() {
    let records = vec![
        json!({"routed": {"cost_usd": 0.2, "wall_ms": 10_000}, "baseline": {"cost_usd": 0.4, "wall_ms": 5_000},
               "checks": [{"routed": true, "baseline": false}]}),
        json!({"routed": {"cost_usd": 0.2, "wall_ms": 10_000}, "baseline": {"cost_usd": 0.4, "wall_ms": 5_000},
               "checks": []}),
        json!({"routed": {"cost_usd": null}, "baseline": {"cost_usd": 0.9}, "checks": []}),
    ];
    let s = shadow(&records);
    assert_eq!(s["pairs"], 2);
    assert_eq!(s["cost_ratio"]["point"], 0.5);
    assert_eq!(s["time_ratio"]["point"], 2.0);
    assert_eq!(s["routed_checked"], 1);
    assert_eq!(s["routed_passed"], 1);
    assert_eq!(s["raw_passed"], 0);
}

#[test]
fn every_published_study_parses_and_traces_to_its_rows() {
    for p in PUBLISHED {
        let rows = study_rows(p.rows);
        let lines = p.rows.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(rows.len(), lines, "{}: every row reads", p.name);
    }
    let all = studies(&[]);
    // The #10209 study with the #10246 lean arm and #10250's three Codex
    // arms: 189 runs, raw Claude Code 21/21 for $6.24.
    let first = &all[0];
    assert_eq!(first["runs"], 189);
    assert_eq!(first["arms"].as_array().unwrap().len(), 9);
    let raw = &first["arms"][0];
    assert_eq!(raw["arm"], BASELINE);
    assert_eq!(raw["passed"], 21);
    assert!((raw["cost_total_usd"].as_f64().unwrap() - 6.24).abs() < 0.01);
    let claude_on = first["comparisons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["arm"] == "routed-claude-on")
        .unwrap();
    assert!((claude_on["cost_ratio"]["point"].as_f64().unwrap() - 1.68).abs() < 0.01);
}

#[test]
fn route_rows_read_settled_runs_from_the_journal() {
    // A real record from the standing study (routed default, mi-one).
    let line = include_str!("../fixtures/efficiency/route-record.json").trim();
    let mut verified: Value = serde_json::from_str(line).unwrap();
    verified["request"] = json!("another-request");
    verified["runs"][0]["task"] = json!("t2");
    verified["runs"][0]["projection"]["check"] = json!("verified");
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("tasks");
    std::fs::create_dir_all(dir.path().join("routes")).unwrap();
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(
        dir.path()
            .join("routes/38e47ec0b3ea441bc82c4ee5a5b7fd30.jsonl"),
        format!("{line}\n{verified}\n"),
    )
    .unwrap();
    std::fs::write(
        store.join("t2.1.atif.jsonl"),
        r#"{"step":{"extensions":{"delegate_recipe":{"run":{"class":{"class":"change"}}}}}}"#,
    )
    .unwrap();
    let rows = route_rows(&store);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].engine.as_deref(), Some("codex"));
    assert!(!rows[0].checked, "the study's routed run was unchecked");
    assert_eq!(rows[0].class, "repository_change");
    assert_eq!(rows[0].cost_usd, Some(0.040_502));
    assert!(rows[1].checked && rows[1].passed);
    assert_eq!(rows[1].class, "change");
    let r = runs(&rows);
    assert_eq!(r["unchecked"], 1);
    assert_eq!(r["runs"], 2);
}

#[test]
fn a_routed_arm_is_also_compared_with_the_raw_arm_of_its_engine() {
    let mut lines = Vec::new();
    for task in ["a", "b"] {
        for t in 0..2 {
            let j = f64::from(t) * 0.01;
            lines.push(row("raw-claude", task, true, 1.0 + j, 10.0 + j));
            lines.push(row("raw-codex", task, true, 0.4 + j, 10.0 + j));
            lines.push(
                json!({"arm": "routed-default", "task": task, "passed": true, "cost_usd": 0.2 + j,
                       "wall_s": 20.0 + j, "engine": "codex"})
                .to_string(),
            );
        }
    }
    let s = study("s", "l", "src", &study_rows(&lines.join("\n")));
    let matched: Vec<&Value> = s["comparisons"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["baseline"] == "raw-codex")
        .collect();
    assert_eq!(matched.len(), 1);
    assert_eq!(matched[0]["arm"], "routed-default");
    assert!((matched[0]["cost_ratio"]["point"].as_f64().unwrap() - 0.5).abs() < 0.02);
    let f = findings(&[s]);
    assert!(f.iter().any(|l| l.contains("against raw Codex")), "{f:?}");
}

#[test]
fn historical_check_failures_and_recent_executor_successes_stay_distinct() {
    let fixture: Value =
        serde_json::from_str(include_str!("../fixtures/efficiency/route-record.json")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("tasks");
    std::fs::create_dir_all(dir.path().join("routes")).unwrap();
    let mut lines = Vec::new();
    for (index, (state, check, engine)) in [
        ("failed", "check_failed", Some("codex")),
        ("completed", "unchecked", Some("codex")),
        ("completed", "unchecked", Some("codex")),
        ("completed", "unchecked", None),
        ("completed", "verified", Some("claude")),
        ("cancelled", "pending", Some("codex")),
        ("failed", "pending", Some("codex")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut record = fixture.clone();
        record["request"] = json!(format!("request-{index}"));
        record["runs"][0]["task"] = json!(format!("task-{index}"));
        record["runs"][0]["engine"] = json!(engine);
        record["runs"][0]["projection"]["state"] = json!(state);
        record["runs"][0]["projection"]["check"] = json!(check);
        lines.push(record.to_string());
    }
    let path = dir.path().join("routes/thread.jsonl");
    let journal = lines.join("\n");
    std::fs::write(&path, &journal).unwrap();
    let rows = route_rows(&store);
    assert_eq!(rows.len(), 6, "stops are not failed checks");
    let report = report(&[], &[], &rows);
    assert_eq!(report["runs"]["unchecked"], 4);
    let groups = report["runs"]["groups"].as_array().unwrap();
    let codex = groups.iter().find(|g| g["engine"] == "codex").unwrap();
    assert_eq!(codex["checked"], 1);
    assert_eq!(codex["passed"], 0);
    let claude = groups.iter().find(|g| g["engine"] == "claude").unwrap();
    assert_eq!(claude["passed"], 1);
    let output = text(&report, false);
    assert!(
        output.contains("unknown (legacy engine not recorded)"),
        "{output}"
    );
    assert!(output.contains("1 run · no independent check"), "{output}");
    assert!(!output.contains("0/0 passed"));
    for note in LOCAL_EVIDENCE_NOTES {
        assert!(output.contains(note));
    }
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        journal,
        "reporting never rewrites history"
    );
}
