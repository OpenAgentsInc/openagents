//! Conformance: `to_record` and `to_answers` against the Python reference's
//! fixture records, and `sanitize` against the delimiter rewrite.

use std::fs;
use std::path::PathBuf;

use indexmap::IndexMap;
use kev::{SystemOneRequest, sanitize, to_answers, to_record};
use serde_json::Value;

fn fixture(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(rel);
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"))
}

fn request_names() -> Vec<String> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/requests");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("fixtures/requests")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

#[test]
fn every_corpus_request_reproduces_its_record() {
    for name in request_names() {
        let body = fixture(&format!("requests/{name}.json"));
        let request: SystemOneRequest = serde_json::from_value(body["request"].clone())
            .unwrap_or_else(|e| panic!("{name}: request parse: {e}"));
        let (record, meta) = to_record(&request).unwrap_or_else(|e| panic!("{name}: to_record: {e}"));
        assert_eq!(
            serde_json::to_value(&record).unwrap(),
            body["record"],
            "{name}: record"
        );
        assert_eq!(
            serde_json::to_value(&meta).unwrap(),
            body["meta"],
            "{name}: meta"
        );
    }
}

#[test]
fn every_golden_answer_set_reproduces() {
    for name in request_names() {
        let body = fixture(&format!("requests/{name}.json"));
        let golden = fixture(&format!("golden/{name}.json"));
        let request: SystemOneRequest =
            serde_json::from_value(body["request"].clone()).expect("request");
        let (_, meta) = to_record(&request).expect("to_record");
        let probs: Vec<Vec<f64>> = serde_json::from_value(golden["probs"].clone()).expect("probs");
        let answers = to_answers(&probs, &meta);
        let got: IndexMap<String, Value> = answers
            .iter()
            .map(|(id, answer)| (id.clone(), serde_json::to_value(answer).unwrap()))
            .collect();
        assert_eq!(
            serde_json::to_value(&got).unwrap(),
            golden["answers"],
            "{name}: answers"
        );
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
