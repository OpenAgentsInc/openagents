use serde_json::json;

use super::*;

#[test]
fn issues_are_owner_name_and_a_positive_number() {
    assert_eq!(
        parse_issue("AtlantisPleb/coder-one-sandbox#1"),
        Some(("AtlantisPleb/coder-one-sandbox".into(), 1))
    );
    for bad in ["#1", "a#1", "a/b#0", "a/b#x", "a/b/c#1", "a/b c#1", "a/b"] {
        assert_eq!(parse_issue(bad), None, "{bad}");
    }
}

#[test]
fn the_env_file_quotes_every_value_and_debug_never_shows_a_token() {
    let mut run = Run::new("o/n", 7);
    run.git_name = Some("It's me".into());
    let credentials = Credentials {
        github_token: "ghp_secret$(rm -rf /)".into(),
        openagents_token: Some("oa_sk_secret".into()),
    };
    let file = env_file(&run, &credentials);
    assert!(file.contains("OA_REPO='o/n'\n"));
    assert!(file.contains("OA_ISSUE='7'\n"));
    assert!(file.contains("GH_TOKEN='ghp_secret$(rm -rf /)'\n"));
    assert!(file.contains("OA_GIT_NAME='It'\\''s me'\n"));
    assert!(file.contains("OPENAGENTS_TOKEN='oa_sk_secret'\n"));
    assert!(file.contains("CODER_VERTEX_LOCATION='global'\n"));
    assert!(file.contains(&format!("OA_SOLVER_MODEL='{DEFAULT_MODEL}'\n")));
    let shown = format!("{credentials:?}");
    assert!(!shown.contains("secret"), "{shown}");
}

#[test]
fn the_reader_reports_thoughts_once_tools_once_and_the_summary() {
    let mut reader = Reader::default();
    let mut all = Vec::new();
    for line in [
        r#"{"type":"solver_box_step","step":"clone","state":"done","text":"Cloned o/n"}"#,
        r#"{"set":0,"entry":{"user":"Briefer: plan the change"}}"#,
        r#"{"set":1,"entry":{"assistant":"Looking","model":null,"elapsed_ms":null}}"#,
        r#"{"set":1,"entry":{"assistant":"Looking at slugify","model":null,"elapsed_ms":null}}"#,
        r#"{"set":2,"entry":{"tool":"shell","input":{"command":"cargo test"},"output":null,"running":true}}"#,
        r#"{"set":2,"entry":{"tool":"shell","input":{"command":"cargo test"},"output":"ok","running":false}}"#,
        r##"{"set":3,"entry":{"tool":"Decision","input":{"kind":"stage","title":"Issue and starting commit","detail":"#5 in o/n"},"output":null,"running":true}}"##,
        r##"{"set":3,"entry":{"tool":"Decision","input":{"kind":"stage","title":"Issue and starting commit","detail":"#5 in o/n"},"output":{"error":"Cannot read issue #5 with gh."},"running":false}}"##,
        "not json from stderr",
        r#"{"busy":false}"#,
        r#"{"type":"solver_box_summary","ok":true,"pr":"https://github.com/o/n/pull/2","cost_usd":0.25,"checks":[{"id":"check:test","ok":true},{"id":"check:fmt","ok":false}],"files":["a.py"],"added":3,"removed":1}"#,
    ] {
        all.extend(reader.line(line));
    }
    all.extend(reader.finish());
    let kinds: Vec<&str> = all.iter().map(|p| p.kind).collect();
    assert_eq!(
        kinds,
        [
            "box", "stage", "thinking", "tool", "briefing", "notice", "checks", "pr", "cost"
        ]
    );
    assert_eq!(all[2].text, "Looking at slugify");
    assert_eq!(all[3].text, "shell cargo test");
    assert_eq!(all[4].text, "Issue and starting commit: #5 in o/n");
    assert_eq!(all[5].text, "Cannot read issue #5 with gh.");
    assert_eq!(all[6].text, "1 of 2 checks passed");
    assert_eq!(all[8].text, "$0.25 in model calls");
    assert_eq!(reader.other, ["not json from stderr"]);
    assert!(reader.summary.is_some());
}

#[test]
fn the_result_has_the_work_run_shape_and_unknown_cost_stays_unknown() {
    let summary = json!({
        "type": "solver_box_summary", "ok": true, "pr": "https://github.com/o/n/pull/2",
        "cost_usd": 0.25, "checks": [{"id": "check:test", "ok": true}],
        "files": ["a.py"], "added": 3, "removed": 1, "patch": "diff --git"
    });
    let usage = BoxUsage {
        sandbox: "bx_1".into(),
        usd: Some(0.05),
        secs: Some(600.0),
    };
    let done = result(Some(&summary), None, &usage, 610.0);
    assert_eq!(done["ok"], true);
    assert_eq!(done["engine"], "solver");
    assert_eq!(done["landed"]["url"], "https://github.com/o/n/pull/2");
    assert_eq!(
        done["diff"],
        json!({"files": ["a.py"], "added": 3, "removed": 1})
    );
    assert_eq!(done["checks"], json!([{"name": "check:test", "ok": true}]));
    assert!((done["cost_usd"].as_f64().unwrap() - 0.30).abs() < 1e-9);
    assert_eq!(done["box"]["sandbox"], "bx_1");

    let unknown = result(Some(&summary), None, &BoxUsage::default(), 1.0);
    assert_eq!(unknown["cost_usd"], Value::Null);

    let failed = result(None, Some("No computer was free".into()), &usage, 1.0);
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["error"], "No computer was free");
    assert_eq!(failed["landed"]["url"], Value::Null);
}

#[test]
fn the_box_script_deletes_the_registry_token_and_mounts_only_the_run_dir() {
    assert!(BOX_SCRIPT.contains(r#"rm -f "$HOST_DIR/registry-token""#));
    assert!(BOX_SCRIPT.contains("-v /run/oa-solver:/run/oa-solver"));
    assert!(BOX_SCRIPT.contains(&format!("HOST_DIR={HOST_DIR}")));
    assert!(!BOX_SCRIPT.contains("GH_TOKEN"));
}
