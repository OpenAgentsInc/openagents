//! Recorded data only: these tests never read a real account or invoke gh.
#[path = "../src/foreman_report.rs"]
mod report;
use serde_json::{Value, json};
use std::process::Command;
const AGENT: &str = "baseline-2026-10-17";
const DAY: &str = "2026-10-17";
fn baseline() -> report::Input {
    serde_json::from_str(include_str!("../fixtures/foreman/2026-10-17.json")).unwrap()
}
fn event(kind: &str, at: &str) -> Value {
    json!({"agent_id":AGENT,"kind":kind,"at":format!("{DAY}T{at}:00-05:00")})
}
fn input(events: Vec<Value>) -> report::Input {
    report::Input {
        events,
        ..Default::default()
    }
}
fn measure(i: &report::Input) -> Value {
    report::measure(i, AGENT, DAY).unwrap()
}
#[test]
fn audit_completion_delays_and_fast_takeovers() {
    let r = measure(&baseline());
    for (n, minutes) in [
        (11994, 304.0),
        (11995, 300.0),
        (11996, 317.0),
        (11997, 297.0),
        (11998, 317.0),
        (12000, 287.0),
        (12001, 310.0),
        (12007, 313.0),
        (12010, 289.0),
        (12011, 294.0),
    ] {
        let row = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["number"] == n)
            .unwrap();
        assert!(
            (row["minutes"]["code_complete_to_close"].as_f64().unwrap() - minutes).abs() < 5.0,
            "{row}"
        );
    }
    for (n, minutes) in [(12018, 21.0), (12022, 33.0), (12019, 35.0)] {
        let row = r["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["number"] == n)
            .unwrap();
        assert!(
            (row["minutes"]["last_claim_to_close"].as_f64().unwrap() - minutes).abs() < 5.0,
            "{row}"
        );
    }
    assert!((r["stalls"]["own_turn_minutes"].as_f64().unwrap() - 252.0).abs() < 5.0);
    assert!(
        (r["stalls"]["chat_minutes"]["coordinator-a9366ded"]
            .as_f64()
            .unwrap()
            - 71.0)
            .abs()
            < 5.0
    );
    assert_eq!(r["production_writes"]["recorded_count"], 1);
    assert_eq!(r["deploy_permission_surprises"]["recorded_count"], 1);
    assert_eq!(r["check_ins"]["entries"], json!([]));
    assert_eq!(r["owner_wait"]["minutes"], 16.0);
}
#[test]
fn owner_list_must_precede_the_same_deploy_refusal() {
    let mut i = baseline();
    i.events.push(json!({"agent_id":AGENT,"kind":"owner_list","at":"2026-10-17T19:51:00-05:00","deploys":["production"]}));
    assert_eq!(
        measure(&i)["deploy_permission_surprises"]["recorded_count"],
        0
    );
    i.events.last_mut().unwrap()["at"] = json!("2026-10-17T19:53:00-05:00");
    assert_eq!(
        measure(&i)["deploy_permission_surprises"]["recorded_count"],
        1
    );
    i.events.last_mut().unwrap()["at"] = json!("2026-10-17T19:51:00-05:00");
    i.events.last_mut().unwrap()["deploys"] = json!(["staging"]);
    assert_eq!(
        measure(&i)["deploy_permission_surprises"]["recorded_count"],
        1
    );
}
#[test]
fn isolates_agent_ids_and_does_not_invent_missing_measurements() {
    let r = report::measure(&baseline(), "another-agent", DAY).unwrap();
    assert_eq!(r["issues"], json!([]));
    assert_eq!(r["production_writes"]["recorded_count"], 0);
    assert_eq!(r["coverage"]["agent_observations"], false);
    let r = measure(&baseline());
    let epic = r["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["number"] == 12006)
        .unwrap();
    assert!(epic["claimed"].is_null());
    assert!(epic["code_complete"].is_null());
    assert_eq!(r["issues"][0]["code_complete_source"], "ready");
}
#[test]
fn check_in_timing_actions_and_missed_deadlines() {
    let mut due = event("check_in_due", "09:00");
    due["id"] = json!("one");
    let mut wake = event("wake", "09:00");
    wake["id"] = json!("one");
    let mut action = event("action", "09:01");
    action["check_in_id"] = json!("one");
    let mut late = event("check_in", "09:12");
    late["due_at"] = json!("2026-10-17T09:10:00-05:00");
    late["id"] = json!("two");
    let mut missed = event("check_in_due", "09:20");
    missed["id"] = json!("three");
    let i = input(vec![due, wake, action, late, missed]);
    let r = measure(&i);
    assert_eq!(r["check_ins"]["on_time"], 1);
    assert_eq!(r["check_ins"]["late"], 1);
    assert_eq!(r["check_ins"]["entries"][0]["actions"], 1);
    assert_eq!(r["check_ins"]["entries"][1]["late_minutes"], 2.0);
    assert_eq!(r["check_ins"]["missed"].as_array().unwrap().len(), 1);
}
#[test]
fn sweep_counts_crossings_durations_and_carry_in() {
    let mut i = input(vec![]);
    i.build_sweep_log="2026-10-16 11:55:00 PM CDT  sweep  free 90.0 GiB -> 90.0 GiB, freed 0 KiB\n2026-10-17 12:05:00 AM CDT  sweep  free 90.0 GiB -> 110.0 GiB, freed 0 KiB\n2026-10-17 1:00:00 AM CDT  sweep  free 80.0 GiB -> 80.0 GiB, freed 0 KiB\n2026-10-17 1:20:00 AM CDT  sweep  free 100.0 GiB -> 100.0 GiB, freed 0 KiB".into();
    let r = measure(&i);
    let b = r["disk"]["breaches"].as_array().unwrap();
    assert_eq!(b.len(), 2);
    assert_eq!(b[0]["minutes"], 5.0);
    assert_eq!(b[1]["minutes"], 20.0);
    assert_eq!(b[0]["started_before_day"], true);
}
#[test]
fn chicago_day_has_dst_boundaries_and_clips_intervals() {
    let (a, b) = report::window("2026-11-01").unwrap();
    assert_eq!(b - a, 25 * 3600);
    assert!(report::iso(a).unwrap().ends_with('Z'));
    let (a, b) = report::window("2026-03-08").unwrap();
    assert_eq!(b - a, 23 * 3600);
    let i = input(vec![
        json!({"agent_id":AGENT,"kind":"stall","at":"2026-10-17T04:50:00Z","ended_at":"2026-10-17T05:10:00Z","own_turn":true}),
    ]);
    assert_eq!(measure(&i)["stalls"]["own_turn_minutes"], 10.0);
    assert!(report::window("2026-02-30").is_err());
}
#[test]
fn commit_references_are_exact_and_code_complete_is_not_a_merge_guess() {
    let mut i = baseline();
    i.commits = vec![
        json!({"sha":"a","commit":{"message":"Fix #119940","committer":{"date":"2026-10-17T14:30:00-05:00"}}}),
        json!({"sha":"b","commit":{"message":"Fix #11994","committer":{"date":"2026-10-17T14:40:00-05:00"}}}),
    ];
    let r = measure(&i);
    let row = &r["issues"][0];
    assert_eq!(row["first_commit"], "2026-10-17 14:40:00 CDT");
    assert_eq!(
        row["minutes"]["claim_to_first_commit"]
            .as_f64()
            .unwrap()
            .round(),
        13.0
    );
    i.events.retain(|e| e["kind"] != "ready");
    assert!(measure(&i)["issues"][0]["code_complete"].is_null());
    i.issues[0]["comments"][0]["created_at"] = json!("2026-10-17T14:41:00-05:00");
    assert_eq!(
        measure(&i)["issues"][0]["minutes"]["claim_to_first_commit"],
        -1.0
    );
}
#[test]
fn live_chat_status_measures_stalls_and_stops_on_new_output() {
    let i = input(vec![
        json!({"agent_id":AGENT,"kind":"chat_status","at":"2026-10-17T09:00:00-05:00","chat_id":"worker","status":"working","last_output_at":"2026-10-17T09:00:00-05:00"}),
        json!({"agent_id":AGENT,"kind":"chat_status","at":"2026-10-17T09:25:00-05:00","chat_id":"worker","status":"working","last_output_at":"2026-10-17T09:00:00-05:00"}),
        json!({"agent_id":AGENT,"kind":"chat_status","at":"2026-10-17T09:35:00-05:00","chat_id":"worker","status":"idle"}),
    ]);
    assert_eq!(measure(&i)["stalls"]["chat_minutes"]["worker"], 15.0);
}
#[test]
fn cli_fixture_is_read_only_and_json_matches_text() {
    let dir = tempfile::tempdir().unwrap();
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/foreman/2026-10-17.json"
    );
    let run = |json: bool| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_openagents"));
        c.env("HOME", dir.path()).env("PATH", dir.path());
        if json {
            c.arg("--json");
        }
        c.args([
            "foreman",
            "report",
            "--agent",
            AGENT,
            "--day",
            DAY,
            "--fixture",
            file,
        ])
        .output()
        .unwrap()
    };
    let j = run(true);
    assert!(j.status.success(), "{}", String::from_utf8_lossy(&j.stderr));
    let v: Value = serde_json::from_slice(&j.stdout).unwrap();
    assert_eq!(v, measure(&baseline()));
    let t = run(false);
    assert!(t.status.success());
    assert_eq!(
        String::from_utf8(t.stdout).unwrap().trim_end(),
        report::text(&v)
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
#[test]
fn malformed_observations_fail_instead_of_silently_disappearing() {
    let mut e = event("stall", "09:00");
    e["ended_at"] = json!("2026-10-17T08:00:00-05:00");
    assert!(report::measure(&input(vec![e]), AGENT, DAY).is_err());
    assert!(
        report::measure(
            &input(vec![json!({"agent_id":AGENT,"kind":"wake","at":"bad"})]),
            AGENT,
            DAY
        )
        .is_err()
    );
}

#[test]
fn visitor_posts_count_even_when_the_old_log_omits_write_metadata() {
    let mut i = baseline();
    for e in &mut i.events {
        if e["kind"] == "probe" {
            e.as_object_mut().unwrap().remove("wrote_data");
        }
    }
    assert_eq!(measure(&i)["production_writes"]["recorded_count"], 1);
}
