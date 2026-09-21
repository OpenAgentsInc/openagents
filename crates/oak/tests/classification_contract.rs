//! Published classification fixtures must agree with the gateway planner.

use gateway::classify::{BackendLimits, Request};
use serde_json::Value;
use std::path::Path;

#[test]
fn published_request_corpus_matches_the_gateway_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/decision-models/fixtures/classify-v1");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["v"], "openagents.classify-fixtures.v1");
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["file"].as_str().unwrap();
        let bytes = std::fs::read(root.join(name)).unwrap();
        let result =
            Request::parse(&bytes).and_then(|request| request.plan(&BackendLimits::product()));
        if let Some(code) = case["expected"]["code"].as_str() {
            assert_eq!(result.unwrap_err().code(), code, "{name}");
        } else {
            let plan = result.unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(
                plan.judgments,
                case["expected"]["judgments"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(plan.work.len() as u64, plan.judgments, "{name}");
            assert!(
                plan.work.iter().all(|work| {
                    serde_json::to_value(work).unwrap()["outcome"] == "unattempted"
                }),
                "{name}: planning must not claim inference"
            );
        }
    }
}

#[test]
fn published_maximum_does_not_override_a_backend_limit() {
    let bytes =
        include_bytes!("../../../docs/decision-models/fixtures/classify-v1/maximum-choice.json");
    let request = Request::parse(bytes).unwrap();
    let limits = BackendLimits {
        max_inputs: 999,
        ..BackendLimits::product()
    };
    assert_eq!(request.plan(&limits).unwrap_err().code(), "too_many_inputs");
}
