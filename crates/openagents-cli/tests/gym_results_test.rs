//! `gym results` scoring, chain verification, and compare/trend tests.

use openagents_cli::gym::results::{
    BENCH_RESULT_SCHEMA_V2, ChainVerdict, ResultsAction, ResultsArgs, ScoreReport, append_refusal,
    read_result_rows, receipt_of, run_results, score_harbor_job, verify_result_chain,
};
use openagents_cli::gym::suite::suite_meta;
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/gym-fixtures")
        .join(name)
}

#[test]
fn score_reproduces_typescript_priced_lane_verdict() {
    let report = score_harbor_job(&fixture("priced-lane"), "tb2-cross-section", "proxy").unwrap();
    assert_eq!(report.accepted, 2);
    assert_eq!(report.rejected, 2);
    assert_eq!(report.ungraded, 0);
    assert_eq!(report.success_rate, Some(0.5));
    assert_eq!(report.models, vec!["gemini-3.7-flash".to_string()]);
    // gemini-3.7-flash is in bench/rates.json, so the run prices instead of
    // reporting unknown: $0.07875 total over 2 accepted outcomes.
    assert_eq!(report.cost_disposition, "known");
    assert!((report.cost_per_accepted_outcome_usd.unwrap() - 0.039375).abs() < 1e-12);
}

#[test]
fn model_absent_from_the_catalog_stays_cost_unknown() {
    let report = score_harbor_job(&fixture("unpriced-lane"), "tb2-cross-section", "proxy").unwrap();
    assert_eq!(report.models, vec!["gpt-5.6-luna".to_string()]);
    assert_eq!(report.cost_disposition, "cost_unknown");
    assert_eq!(report.cost_per_accepted_outcome_usd, None);
    assert_eq!(report.total_cost_usd, None);
}

#[test]
fn local_lane_records_unmetered_cost_and_the_harbor_model() {
    let report = score_harbor_job(&fixture("priced-lane"), "tb2-cross-section", "local").unwrap();
    assert_eq!(report.cost_disposition, "unmetered_local_lane");
    assert_eq!(report.models, vec!["gemini-3.7-flash".to_string()]);
}

#[test]
fn score_keeps_ungraded_out_of_the_denominator() {
    let report =
        score_harbor_job(&fixture("crashed-verifier"), "tb2-cross-section", "proxy").unwrap();
    assert_eq!(report.accepted, 1);
    assert_eq!(report.ungraded, 2);
    assert_eq!(report.success_rate, Some(1.0));
}

#[test]
fn score_command_prints_without_appending() {
    let args = ResultsArgs {
        action: ResultsAction::Score {
            job_dir: fixture("priced-lane"),
            suite: "tb2-cross-section".into(),
            lane: Some("proxy".into()),
            append: false,
        },
    };
    run_results(args, false).expect("score without --append must succeed");
}

#[test]
fn trend_stops_on_a_broken_chain_and_names_the_row() {
    let tmp = tempfile::tempdir().unwrap();
    let store = tmp.path().join("bench-results").join("broken.jsonl");
    std::fs::create_dir_all(store.parent().unwrap()).unwrap();
    let mut first = serde_json::json!({
        "schema": BENCH_RESULT_SCHEMA_V2,
        "recordedAt": "2026-08-25T10:00:00Z",
        "suite": "broken",
        "lane": "proxy",
        "suiteKey": "suite:aaaa",
        "successRate": 1.0,
        "costPerAcceptedOutcomeUsd": null,
        "costDisposition": "cost_unknown",
        "previousReceipt": null,
    });
    first["receipt"] = Value::String(receipt_of(&first));
    let mut second = first.clone();
    second["recordedAt"] = Value::String("2026-08-25T11:00:00Z".into());
    second["successRate"] = serde_json::json!(0.5);
    second["previousReceipt"] = first["receipt"].clone();
    second["receipt"] = Value::String(receipt_of(&second));
    second["successRate"] = serde_json::json!(0.0); // tamper after receipt
    let body = format!("{}\n{}\n", first, second);
    std::fs::write(&store, body).unwrap();

    let rows = read_result_rows(&store).unwrap();
    match verify_result_chain(&rows) {
        ChainVerdict::Break { index, detail } => {
            assert_eq!(index, 1);
            assert!(
                detail.contains("edited after it was written") || detail.contains("row 1"),
                "{detail}"
            );
        }
        ChainVerdict::Ok { .. } => panic!("tampered store must not verify"),
    }
}

#[test]
fn smoke_tier_append_is_refused_with_the_tier_named() {
    let report = score_harbor_job(&fixture("priced-lane"), "smoke", "proxy").unwrap();
    let meta = suite_meta("smoke").expect("smoke suite is checked in");
    let reason = append_refusal(&report, &meta).expect("smoke must refuse");
    assert!(reason.contains("smoke_run"), "{reason}");
    assert!(reason.contains("smoke"), "{reason}");
}

#[test]
fn complete_coderbench_smoke_run_may_append_as_smoke() {
    let meta = suite_meta("coderbench-agent-building-v1").expect("D5 suite is checked in");
    assert_eq!(meta.tier, "smoke");
    let report = ScoreReport {
        suite: "coderbench-agent-building-v1".into(),
        lane: "proxy".into(),
        job_id: Some("job-coderbench-smoke".into()),
        trials_total: 2,
        accepted: 1,
        rejected: 1,
        ungraded: 0,
        graded: 2,
        success_rate: Some(0.5),
        ungraded_ratio: 0.0,
        cost_disposition: "cost_unknown".into(),
        cost_per_accepted_outcome_usd: None,
        total_cost_usd: None,
        cost_coverage: "unknown".into(),
        rate_basis: None,
        rate_catalog_version: "openagents.coder-rate-catalog.2026-08-25".into(),
        unpriced_reasons: Vec::new(),
        prompt_tokens: None,
        completion_tokens: None,
        cached_input_tokens: 0,
        tool_calls: None,
        tasks: vec!["openssl-selfsigned-cert".into(), "regex-log".into()],
        models: Vec::new(),
        wall_clock_seconds: None,
    };
    assert_eq!(
        append_refusal(&report, &meta),
        None,
        "a complete smoke-tier CoderBench run must be recordable as smoke"
    );
}

#[test]
fn real_tb2_quick_store_verifies() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench-results/tb2-quick.jsonl");
    let rows = read_result_rows(&path).unwrap();
    assert!(!rows.is_empty());
    match verify_result_chain(&rows) {
        ChainVerdict::Ok { rows: n, .. } => assert_eq!(n, rows.len()),
        ChainVerdict::Break { detail, .. } => panic!("{detail}"),
    }
}

#[test]
fn compare_renders_two_suites_side_by_side() {
    let tmp = tempfile::tempdir().unwrap();
    // compare_suites reads bench-results/<id>.jsonl from repo root. Use the
    // checked-in stores, which already chain-verify.
    let args = ResultsArgs {
        action: ResultsAction::Compare {
            suite_id: "tb2-quick".into(),
            other: Some("tb2-cross-section".into()),
        },
    };
    let _ = tmp;
    run_results(args, false).expect("cross-suite compare of chained stores");
}
