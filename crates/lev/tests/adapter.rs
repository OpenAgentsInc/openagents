//! The adapter path, proved as far as it can be without trained weights.
//!
//! Training needs Apple's adapter training toolkit, which is not on this
//! machine. Everything around the training is testable now, and this file
//! establishes exactly where the line falls: the package format is right, the
//! runtime loads what we build, the live base signature is readable, and the
//! only thing missing is tensors.

use lev::adapter::{self, Metadata, Package};
use lev::bridge::{Bridge, Call, Sampling};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The base model signature this repository builds against. Read from the
/// live runtime by `base_signature_prefix`, and unchanged since the March
/// 2026 specs were frozen.
const SIGNATURE: &str = "9799725ff8e851184037110b422d891ad3b92ec1";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lev-adapter-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn synthetic(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("synthetic.fmadapter");
    let mut creator_defined = BTreeMap::new();
    creator_defined.insert("purpose".to_string(), serde_json::json!("attach-path test"));
    let metadata = Metadata {
        adapter_identifier: format!("fmadapter-lev-{}", &SIGNATURE[..7]),
        base_model_signature: SIGNATURE.to_string(),
        lora_rank: 32,
        author: Some("openagents".to_string()),
        description: Some("synthetic plumbing probe, not trained".to_string()),
        license: None,
        draft_token_count: None,
        creator_defined,
    };
    // One small fp16 payload. Structurally valid, semantically meaningless.
    let payload: Vec<u8> = std::iter::repeat_n([0x00_u8, 0x3c], 64).flatten().collect();
    adapter::write_package(&path, &metadata, &[(1, payload)]).expect("the package writes");
    path
}

fn bridge() -> Option<Bridge> {
    let mut bridge = Bridge::discover().ok()?;
    match bridge.availability() {
        Ok(availability) if availability.is_available() => Some(bridge),
        other => {
            eprintln!("skipping: the model is not available ({other:?})");
            None
        }
    }
}

#[test]
fn the_device_reports_the_base_signature_it_will_accept() {
    let Some(mut bridge) = bridge() else { return };
    let identifiers = bridge
        .compatible_adapters("lev")
        .expect("the runtime answered");
    eprintln!("compatible adapter identifiers: {identifiers:?}");
    assert!(!identifiers.is_empty());
    assert!(identifiers[0].starts_with("fmadapter-lev-"));

    let prefix = bridge.base_signature_prefix().expect("a signature prefix");
    assert!(
        SIGNATURE.starts_with(&prefix),
        "the device runs base `{prefix}`, and this repository builds against `{SIGNATURE}`. \
         An operating system update changed the base; every adapter and every calibration map \
         fitted against the old one is invalid."
    );
}

#[test]
fn the_runtime_loads_a_package_this_repository_wrote() {
    let Some(mut bridge) = bridge() else { return };
    let dir = scratch("load");
    let path = synthetic(&dir);

    // Our own reader accepts it first.
    let package = Package::open(&path).expect("our reader opens it");
    package
        .check_signature(SIGNATURE)
        .expect("the signature matches");

    // And so does Apple's, which is the part that proves the container layout
    // is right: metadata parsed, blob storage parsed, producer metadata
    // round-tripped.
    let metadata = bridge.load_adapter(&path).expect("the runtime loads it");
    assert_eq!(
        metadata.get("purpose").map(String::as_str),
        Some("attach-path test")
    );
}

#[test]
fn an_untrained_package_loads_and_then_fails_at_inference() {
    let Some(mut bridge) = bridge() else { return };
    let dir = scratch("infer");
    let path = synthetic(&dir);

    // Loading succeeds: the package is well formed.
    bridge.load_adapter(&path).expect("the runtime loads it");

    // Generating does not: the tensors are a single junk vector rather than a
    // LoRA. This is the line the toolkit is needed to cross, and it is worth
    // asserting so that a future run which *passes* is a real signal that
    // trained weights arrived.
    let mut criteria = indexmap::IndexMap::new();
    criteria.insert("billing".to_string(), None);
    criteria.insert("technical".to_string(), None);
    let mut questions = indexmap::IndexMap::new();
    questions.insert(
        "q".to_string(),
        lev::api::Question::Choice {
            instructions: None,
            criteria,
        },
    );
    let request = lev::api::SystemOneRequest {
        state: serde_json::json!("I was charged twice."),
        model: None,
        questions,
        extensions: lev::api::Extensions::default(),
    };
    let compiled = lev::schema::compile(&request).expect("it compiles");
    let call =
        Call::decide(&compiled["q"], Sampling::Greedy).with_adapter(path.display().to_string());

    let outcome = bridge.decide(&call);
    assert!(
        outcome.is_err(),
        "an untrained adapter answered, which means these weights are no longer junk: {outcome:?}"
    );
    eprintln!(
        "untrained adapter refused as expected: {:?}",
        outcome.unwrap_err().code
    );
}
