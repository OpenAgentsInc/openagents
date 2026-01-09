use rstest::*;
use serde_json::json;

use dsrs::{LmUsage, Prediction};
use std::collections::HashMap;

#[rstest]
fn test_prediction_initialization() {
    let data = HashMap::from([("a".to_string(), json!("1")), ("b".to_string(), json!("2"))]);
    let prediction = Prediction::new(data, LmUsage::default());
    assert_eq!(
        prediction.data,
        HashMap::from([("a".to_string(), json!("1")), ("b".to_string(), json!("2"))])
    );

    let lm_usage = LmUsage::default();
    assert_eq!(prediction.lm_usage.prompt_tokens, lm_usage.prompt_tokens);
    assert_eq!(
        prediction.lm_usage.completion_tokens,
        lm_usage.completion_tokens
    );
    assert_eq!(prediction.lm_usage.total_tokens, lm_usage.total_tokens);
}

#[rstest]
fn test_prediction_get() {
    let data = HashMap::from([("a".to_string(), json!("1")), ("b".to_string(), json!("2"))]);
    let lm_usage = LmUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };
    let prediction = Prediction::new(data, lm_usage.clone());

    assert_eq!(prediction.get("a", None), "1");
    assert_eq!(prediction.get("b", None), "2");
    assert_eq!(prediction.get("c", None), "");
    assert_eq!(prediction.get("c", Some("3")), "3");
    assert_eq!(prediction.get("a", None), "1");

    assert_eq!(prediction.lm_usage.prompt_tokens, lm_usage.prompt_tokens);
    assert_eq!(
        prediction.lm_usage.completion_tokens,
        lm_usage.completion_tokens
    );
    assert_eq!(prediction.lm_usage.total_tokens, lm_usage.total_tokens);
}

#[rstest]
fn test_prediction_keys() {
    let data = HashMap::from([("a".to_string(), json!("1")), ("b".to_string(), json!("2"))]);
    let prediction = Prediction::new(data, LmUsage::default());

    let mut keys = prediction.keys();
    keys.sort();
    assert_eq!(keys, vec!["a", "b"]);
}

#[rstest]
fn test_prediction_values() {
    let data = HashMap::from([("a".to_string(), json!("1")), ("b".to_string(), json!("2"))]);
    let prediction = Prediction::new(data, LmUsage::default());

    let mut values = prediction.values();
    values.sort_by_key(|v| v.to_string());
    assert_eq!(values, vec![json!("1"), json!("2")]);
}

#[rstest]
fn test_prediction_set_lm_usage() {
    let mut prediction = Prediction::new(HashMap::new(), LmUsage::default());

    let lm_usage = LmUsage::default();
    assert_eq!(prediction.lm_usage.prompt_tokens, lm_usage.prompt_tokens);
    assert_eq!(
        prediction.lm_usage.completion_tokens,
        lm_usage.completion_tokens
    );
    assert_eq!(prediction.lm_usage.total_tokens, lm_usage.total_tokens);

    let lm_usage = LmUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };
    prediction.set_lm_usage(lm_usage.clone());
    assert_eq!(prediction.lm_usage.prompt_tokens, lm_usage.prompt_tokens);
    assert_eq!(
        prediction.lm_usage.completion_tokens,
        lm_usage.completion_tokens
    );
    assert_eq!(prediction.lm_usage.total_tokens, lm_usage.total_tokens);
}
