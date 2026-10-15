use std::path::PathBuf;

use psionic_inference::{
    Tensors,
    router::{RouterAnswer, RouterTokenizer, answer, pinned_manifest},
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    revision: String,
    requests: Vec<Case>,
    tokenizer_cases: Vec<TokenCase>,
}

#[derive(Deserialize)]
struct Case {
    outputs: Tensors,
    answer: RouterAnswer,
}

#[derive(Deserialize)]
struct TokenCase {
    id: String,
    request: String,
    context: Option<String>,
    input_ids: Vec<i64>,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/s1-router-reference.json")).unwrap()
}

#[test]
fn shared_processing_matches_independent_reference() {
    let manifest = pinned_manifest().unwrap();
    let fixture = fixture();
    assert_eq!(fixture.revision, manifest.revision);
    assert_eq!(fixture.requests.len(), 100);
    for case in fixture.requests {
        let actual = answer(&case.outputs, &manifest).unwrap();
        for (a, b) in [
            (&actual.category, &case.answer.category),
            (&actual.difficulty, &case.answer.difficulty),
            (&actual.stakes, &case.answer.stakes),
        ] {
            assert_eq!(a.label, b.label);
            for (name, p) in &a.probabilities {
                assert!((p - b.probabilities[name]).abs() < 1e-6);
            }
            if let (Some(x), Some(y)) = (a.expected, b.expected) {
                assert!((x - y).abs() < 1e-6);
            }
        }
        for (name, p) in &actual.flags {
            assert!((p - case.answer.flags[name]).abs() < 1e-6);
        }
    }
}

#[test]
fn rejects_missing_outputs_and_invalid_temperatures() {
    let mut manifest = pinned_manifest().unwrap();
    assert!(answer(&Tensors::new(), &manifest).is_err());
    manifest.temperatures.insert("category".into(), 0.0);
    assert!(answer(&fixture().requests[0].outputs, &manifest).is_err());
}

#[test]
#[ignore = "requires pinned model files; set PSIONIC_ROUTER_MODEL_DIR"]
fn pinned_artifacts_and_thousand_tokenizer_cases_match() {
    let root = PathBuf::from(
        std::env::var_os("PSIONIC_ROUTER_MODEL_DIR").expect("set PSIONIC_ROUTER_MODEL_DIR"),
    );
    pinned_manifest().unwrap().verify(&root).unwrap();
    let tokenizer = RouterTokenizer::load(&root.join("tokenizer/tokenizer.json")).unwrap();
    let fixture = fixture();
    assert_eq!(fixture.tokenizer_cases.len(), 1000);
    for case in fixture.tokenizer_cases {
        let actual = tokenizer
            .prepare(&case.request, case.context.as_deref())
            .unwrap();
        assert_eq!(
            actual["input_ids"].integers().unwrap(),
            case.input_ids,
            "case {}",
            case.id
        );
    }
}
