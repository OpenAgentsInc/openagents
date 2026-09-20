//! Validate the optional fixed-answer provenance before evaluation reads it.

use jev::{Error, RawResponse, SystemOneResponse};
use serde_json::{Value, json};

fn decode(answer: Value) -> jev::Result<SystemOneResponse> {
    SystemOneResponse::decode(RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: serde_json::to_vec(&json!({"model":"fixture","answers":{"q":answer}})).unwrap(),
    })
}

#[test]
fn a_noul_cannot_name_an_option_outside_yes_and_no() {
    for selected in ["", "maybe", "YES"] {
        let error = decode(json!({"type":"noul","noul":0.25,"selected":selected})).unwrap_err();
        assert!(
            matches!(error, Error::ResponseValidation { field_path, .. } if field_path == "answers.q.selected")
        );
    }
    let answer = decode(json!({"type":"noul","noul":0.25,"selected":"yes"})).unwrap();
    assert_eq!(answer.noul("q").unwrap().selected.as_deref(), Some("yes"));
}

#[test]
fn a_score_must_name_an_actual_canonical_level() {
    for selected in ["", "unknown", "2", "00", "-1"] {
        let error = decode(json!({"type":"score","score":0.75,"confidence":0.0,
            "legend":{"0":"low","1":"high"},"probabilities":{"0":0.25,"1":0.75},"selected":selected}))
        .unwrap_err();
        assert!(
            matches!(error, Error::ResponseValidation { field_path, .. } if field_path == "answers.q.selected")
        );
    }
    let answer = decode(json!({"type":"score","score":0.75,"confidence":0.0,
        "legend":{"0":"low","1":"high"},"probabilities":{"0":0.25,"1":0.75},"selected":"0"}))
    .unwrap();
    assert_eq!(answer.score("q").unwrap().selected.as_deref(), Some("0"));
}
