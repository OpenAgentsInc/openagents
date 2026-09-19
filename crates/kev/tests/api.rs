//! Conformance: `to_record` and `to_answers` against the Python reference's
//! fixture records, and `sanitize` against the delimiter rewrite.
//!
//! Runs over every committed variant's fixtures; the request mapping is
//! checkpoint-independent but each variant's set pins its own outputs.

mod common;

use indexmap::IndexMap;
use kev::{SystemOneRequest, sanitize, to_answers, to_record};

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
    let mut request: SystemOneRequest = serde_json::from_str(
        r#"{"state": "x", "questions": {}}"#,
    )
    .unwrap();
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
