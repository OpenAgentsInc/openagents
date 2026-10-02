//! Jev's issue question with the pickup option (#10206), on the labeled
//! asks in `crates/coder/fixtures/issue-pick/asks-v1.json`: issue-pickup
//! phrasings, ordinary coding asks and questions about issues, and asks
//! that name an issue.
//!
//! The fixture test always runs. The live read asks Jev once per row and
//! prints the accuracy; it needs Jev's credentials (`~/.openagents/jev.json`):
//!
//! ```text
//! cargo test -p coder --test issue_pick_eval -- --ignored --nocapture
//! ```

use std::path::Path;

use coder::task::issue_run::{Asked, asked_work_blocking};
use serde_json::Value;

fn rows() -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/issue-pick/asks-v1.json");
    let set: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    set["rows"].as_array().unwrap().clone()
}

#[test]
fn every_row_is_labeled_pick_none_or_an_issue() {
    let rows = rows();
    assert!(rows.len() >= 20);
    for row in &rows {
        let truth = row["truth"].as_str().unwrap();
        assert!(
            truth == "pick"
                || truth == "none"
                || truth
                    .strip_prefix('#')
                    .is_some_and(|n| n.parse::<u64>().is_ok()),
            "{row}"
        );
        assert!(row["request"].as_str().is_some_and(|r| !r.is_empty()));
    }
    for kind in ["pick", "none", "#"] {
        assert!(
            rows.iter()
                .any(|row| row["truth"].as_str().unwrap().starts_with(kind)),
            "{kind}"
        );
    }
}

#[test]
#[ignore = "asks Jev live"]
fn jev_tells_a_pickup_from_ordinary_coding_work() {
    let (jev, why) = coder::delegate_door::jev_from(&coder::delegate_door::env_value);
    assert!(jev.is_some(), "Jev is unavailable: {why}");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = rows();
    let mut right = 0;
    for row in &rows {
        let request = row["request"].as_str().unwrap();
        let earlier = row["earlier"].as_str().unwrap_or("");
        let got = match asked_work_blocking(request, earlier, dir) {
            Some(Asked::Pick) => "pick".to_owned(),
            Some(Asked::Issue(reference)) => format!("#{}", reference.number),
            None => "none".to_owned(),
        };
        let truth = row["truth"].as_str().unwrap();
        let ok = got == truth;
        right += usize::from(ok);
        println!(
            "{} {:<10} truth {:<7} got {:<7} {request}",
            if ok { "ok  " } else { "MISS" },
            row["id"].as_str().unwrap(),
            truth,
            got
        );
    }
    println!("accuracy {right}/{}", rows.len());
    assert!(right * 10 >= rows.len() * 9, "accuracy below 90%");
}
