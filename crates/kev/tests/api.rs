//! Conformance: `to_record` and `to_answers` against the Python reference's
//! fixture records, and `sanitize` against the delimiter rewrite.
//!
//! Runs over every committed variant's fixtures; the request mapping is
//! checkpoint-independent but each variant's set pins its own outputs.

mod common;

use indexmap::IndexMap;
use kev::{Answer, Meta, SystemOneRequest, sanitize, to_answers, to_record};

use common::{fixture, names, variants};

#[test]
fn every_corpus_request_reproduces_its_record() {
    for variant in variants() {
        for name in names(&variant, "requests") {
            let body = fixture(&variant, &format!("requests/{name}.json"));
            let request: SystemOneRequest = serde_json::from_value(body["request"].clone())
                .unwrap_or_else(|e| panic!("{}: {name}: request parse: {e}", variant.id));
            let (record, meta) = to_record(&request)
                .unwrap_or_else(|e| panic!("{}: {name}: to_record: {e}", variant.id));
            assert_eq!(
                serde_json::to_value(&record).unwrap(),
                body["record"],
                "{}: {name}: record",
                variant.id,
            );
            assert_eq!(
                serde_json::to_value(&meta).unwrap(),
                body["meta"],
                "{}: {name}: meta",
                variant.id,
            );
        }
    }
}

#[test]
fn every_golden_answer_set_reproduces() {
    for variant in variants() {
        for name in names(&variant, "requests") {
            let body = fixture(&variant, &format!("requests/{name}.json"));
            let golden = fixture(&variant, &format!("golden/{name}.json"));
            let request: SystemOneRequest =
                serde_json::from_value(body["request"].clone()).expect("request");
            let (_, meta) = to_record(&request).expect("to_record");
            let probs: Vec<Vec<f64>> =
                serde_json::from_value(golden["probs"].clone()).expect("probs");
            let answers = to_answers(&probs, &meta);
            let got: IndexMap<String, serde_json::Value> = answers
                .iter()
                .map(|(id, answer)| (id.clone(), serde_json::to_value(answer).unwrap()))
                .collect();
            assert_eq!(
                serde_json::to_value(&got).unwrap(),
                golden["answers"],
                "{}: {name}: answers",
                variant.id,
            );
        }
    }
}

/// The level a reader derives from a Score answer, under the workspace
/// convention: equal leaders resolve to the last level listed.
fn argmax_level(probabilities: &IndexMap<String, f64>) -> &str {
    probabilities
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(level, _)| level.as_str())
        .expect("a non-empty distribution")
}

#[test]
fn a_score_reports_the_weighted_mean_while_a_reader_argmaxes() {
    // The contract every door in this workspace answers: `score` is the
    // probability-weighted mean and can land between levels, and the level
    // an evaluator scores is the argmax of `probabilities`. The two
    // disagree on a skew and on an exact tie, and both disagreements are
    // pinned here because docs/decision-models/2026-09-20-score-contract.md
    // rests on them.
    let meta = vec![
        Meta {
            id: "skew".to_string(),
            kind: "score".to_string(),
            keys: None,
            legend: None,
        },
        Meta {
            id: "tie".to_string(),
            kind: "score".to_string(),
            keys: None,
            legend: None,
        },
        Meta {
            id: "choice".to_string(),
            kind: "choice".to_string(),
            keys: Some(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
            legend: None,
        },
    ];
    let probs = vec![
        vec![0.5, 0.375, 0.125], // mean 0.625, reported as 0.62; argmax is level 0
        vec![0.0, 0.0, 0.5, 0.5, 0.0], // mean 2.50; the tie resolves to level 3
        vec![0.4, 0.4, 0.2],     // a and b tie; b is listed last
    ];
    let answers = to_answers(&probs, &meta);

    let Answer::Score(skew) = answers.get("skew").expect("a score answer") else {
        panic!("expected a score");
    };
    assert!(
        (skew.score - 0.62).abs() < 1e-9,
        "the weighted mean, rounded"
    );
    assert_eq!(argmax_level(&skew.probabilities), "0");

    let Answer::Score(tie) = answers.get("tie").expect("a score answer") else {
        panic!("expected a score");
    };
    assert!((tie.score - 2.5).abs() < 1e-9);
    assert_eq!(argmax_level(&tie.probabilities), "3");

    let Answer::Choice(choice) = answers.get("choice").expect("a choice answer") else {
        panic!("expected a choice");
    };
    assert_eq!(choice.choice, "b", "the last of the equal leaders");
}

#[test]
fn sanitize_rewrites_delimiter_spans() {
    assert_eq!(
        sanitize("a literal <|box_start|> in user text"),
        "a literal <¦box_start¦> in user text"
    );
    assert_eq!(
        sanitize("<|fim_suffix|> <|box_end|> x <|name|>"),
        "<¦fim_suffix¦> <¦box_end¦> x <¦name¦>"
    );
    assert_eq!(sanitize("plain text"), "plain text");
    // <| not closed by |> survives untouched.
    assert_eq!(sanitize("<| unclosed"), "<| unclosed");
}

#[test]
fn validation_refuses_bad_requests() {
    let mut request: SystemOneRequest =
        serde_json::from_str(r#"{"state": "x", "questions": {}}"#).unwrap();
    assert!(matches!(
        to_record(&request),
        Err(kev::Error::EmptyQuestions)
    ));

    request = serde_json::from_str(
        r#"{"state": "x", "questions": {"q": {"type": "choice", "instructions": "?", "criteria": {}}}}"#,
    )
    .unwrap();
    assert!(matches!(
        to_record(&request),
        Err(kev::Error::MissingChoiceCriteria { .. })
    ));

    request = serde_json::from_str(
        r#"{"state": "x", "questions": {"q": {"type": "score", "instructions": "?", "criteria": ["only"]}}}"#,
    )
    .unwrap();
    assert!(matches!(
        to_record(&request),
        Err(kev::Error::TooFewLevels { .. })
    ));

    request = serde_json::from_str(
        r#"{"state": "x", "questions": {"q": {"type": "mystery", "instructions": "?", "criteria": {}}}}"#,
    )
    .unwrap();
    assert!(matches!(
        to_record(&request),
        Err(kev::Error::UnsupportedQuestionType { .. })
    ));
}
