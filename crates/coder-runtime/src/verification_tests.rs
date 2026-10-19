//! Program verification requires authority and records its verdict.

use coder_core::capability::{self, Entry, Trust};
use coder_core::verification::*;
use serde_json::json;

#[path = "../../coder-core/tests/support/verification.rs"]
mod support;
use support::*;

#[tokio::test]
async fn runtime_checks_require_authority_and_record_a_separate_verdict() {
    if !supported() {
        return;
    }
    let (_host, workspace, plan) = fixture("true", Acceptance::ExitSuccess);
    let survey = crate::Survey {
        capabilities: vec![],
        programs: crate::program::Registry::open(&[]),
        sources: crate::source::Registry::open(&[]),
        workspace: workspace.path().into(),
    };
    let runtime = crate::Runtime::using(survey, None).with_verification(
        workspace.path().into(),
        plan,
        Trust::everything(),
    );
    let program:crate::Program=serde_json::from_value(json!({"v":1,"slug":"verify-fixture","steps":[{"name":"gate","kind":"check","bounds":{"refuse_on":"gate_not_met"}}]})).unwrap();
    let inputs = crate::Inputs::read("Run the host-prepared check.", "");
    let refused = runtime
        .run(&program, &inputs, &crate::Grant::none(), None)
        .await;
    assert!(refused.verification.is_empty());
    assert_eq!(refused.stopped.unwrap().code, "unauthorized");
    let missing_effects = crate::Grant::selected(Some("verify-fixture"), Some("reads"));
    assert_eq!(
        runtime
            .run(&program, &inputs, &missing_effects, None)
            .await
            .stopped
            .unwrap()
            .code,
        "unauthorized"
    );
    let run = runtime
        .run(&program, &inputs, &crate::Grant::all(), None)
        .await;
    assert!(run.finished(), "{:?}", run.stopped);
    assert_eq!(run.verification.len(), 1);
    assert_eq!(run.verification[0].verdict, Verdict::Passed);
    assert!(run.delegations.is_empty());
    assert!(run.summary().contains("independent verification"));
}
