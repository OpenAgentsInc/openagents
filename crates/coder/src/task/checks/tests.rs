use super::*;

fn fixture() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    Requirements,
    Context,
    Report,
) {
    let host = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let program = host.path().join("suite.sh");
    let bytes = b"#!/bin/sh\nexit 0\n";
    std::fs::write(&program, bytes).unwrap();
    let program = program.canonicalize().unwrap();
    let manifest = host.path().join("suite.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&capability::executor_document(
            "task-check-test",
            program.to_str().unwrap(),
            vec![program.display().to_string(), "--version".into()],
            json!({"name":"Fixture","invoke":[program],"isolation":["directory"]}),
        ))
        .unwrap(),
    )
    .unwrap();
    let entry = capability::Entry::load(&manifest, capability::Source::Operator).unwrap();
    let suite_digest = digest_bytes(bytes);
    let requirements = Requirements {
        schema: REQUIREMENTS_SCHEMA.into(),
        version: 1,
        requirements: vec![Requirement {
            id: "output".into(),
            statement: "Validate output independently.".into(),
            checks: vec!["content".into()],
        }],
        plan: json!({"schema":verification::SCHEMA,"input_digest":CANDIDATE,"seconds":5,"allow_unrestricted_reads":true,"allow_network":true,"checks":[{"id":"content","manifest":manifest,"manifest_digest":entry.digest,"arguments":[CANDIDATE],"seconds":3,"output_bytes":4096,"acceptance":{"kind":"suite","suite_digest":suite_digest,"input_digest":CANDIDATE}}]}),
        instruction_targets: vec![],
        source_exclusions: vec!["task-source".into()],
    };
    let mut context = Context {
        schema: "openagents.coder.task-context.v1".into(),
        task_revision: 1,
        prompt: "Validate output.".into(),
        instructions: vec![],
        suites: requirements.protected_suites(workspace.path()).unwrap(),
        digest: String::new(),
    };
    context.digest = context.expected_digest();
    let candidate = "a".repeat(64);
    let plan = prepared(&requirements, &candidate).unwrap();
    let report = Report {
        schema: "openagents.coder.task-checks.v1".into(),
        requirements_digest: requirements.digest(),
        context_digest: context.digest.clone(),
        candidate_snapshot: Some(candidate.clone()),
        verdict: Checks::Passed,
        reason: None,
        evidence: Some(
            json!({"schema":verification::SCHEMA,"plan_digest":plan.digest(),"input_digest":candidate,"before_snapshot":candidate,"after_snapshot":candidate,"verdict":"passed","checks":[{"id":"content","verdict":"passed","reason":"typed independent evidence","elapsed_ms":1,"stdout_digest":"1".repeat(64),"stderr_digest":"2".repeat(64),"output_truncated":false,"suite_evidence":{"schema":verification::SCHEMA,"suite_digest":suite_digest,"input_digest":candidate,"verdict":"passed"}}]}),
        ),
    };
    (host, workspace, requirements, context, report)
}
#[test]
fn frozen_suite_arguments_are_data_not_candidate_owned_code() {
    let (_host, _workspace, mut requirements, _, _) = fixture();
    requirements.plan["checks"][0]["arguments"] = json!(["-c", "./candidate-owned-check.sh"]);
    assert!(requirements.validate().is_err());
}
#[test]
fn changed_external_program_or_candidate_owned_program_refuses() {
    let (host, workspace, mut requirements, context, _) = fixture();
    std::fs::write(&context.suites[0].program, b"#!/bin/sh\ntrue\n").unwrap();
    assert!(requirements.protected_suites(workspace.path()).is_err());
    let program = workspace.path().join("candidate-check.sh");
    let script = b"#!/bin/sh\nexit 0\n";
    std::fs::write(&program, script).unwrap();
    let program = program.canonicalize().unwrap();
    let manifest = host.path().join("suite.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&capability::executor_document(
            "task-check-test",
            program.to_str().unwrap(),
            vec![program.display().to_string(), "--version".into()],
            json!({"name":"Fixture","invoke":[program],"isolation":["directory"]}),
        ))
        .unwrap(),
    )
    .unwrap();
    requirements.plan["checks"][0]["manifest_digest"] = json!(
        capability::Entry::load(&manifest, capability::Source::Operator)
            .unwrap()
            .digest
    );
    assert!(requirements.protected_suites(workspace.path()).is_err());
}
#[test]
fn passed_requires_complete_typed_bound_evidence_on_replay() {
    let (_host, _workspace, requirements, context, valid) = fixture();
    let candidate = valid.candidate_snapshot.as_deref();
    valid.validate(&requirements, &context, candidate).unwrap();
    let mut missing = valid.clone();
    missing.evidence = None;
    assert!(
        missing
            .validate(&requirements, &context, candidate)
            .is_err()
    );
    for replacement in [json!({}), json!({"checks":[]})] {
        let mut invalid = valid.clone();
        invalid.evidence = Some(replacement);
        assert!(
            invalid
                .validate(&requirements, &context, candidate)
                .is_err()
        );
    }
    for (field, value) in [
        ("checks", json!([])),
        ("plan_digest", json!("wrong")),
        ("input_digest", json!("wrong")),
    ] {
        let mut invalid = valid.clone();
        invalid.evidence.as_mut().unwrap()[field] = value;
        assert!(
            invalid
                .validate(&requirements, &context, candidate)
                .is_err()
        );
    }
    for (field, value) in [
        ("suite_evidence", Value::Null),
        ("output_truncated", json!(true)),
        ("id", json!("another")),
    ] {
        let mut invalid = valid.clone();
        invalid.evidence.as_mut().unwrap()["checks"][0][field] = value;
        assert!(
            invalid
                .validate(&requirements, &context, candidate)
                .is_err()
        );
    }
    let mut invalid = valid.clone();
    invalid.evidence.as_mut().unwrap()["checks"][0]["suite_evidence"]["suite_digest"] =
        json!("another");
    assert!(
        invalid
            .validate(&requirements, &context, candidate)
            .is_err()
    );
}
#[test]
fn changed_snapshot_is_unavailable_and_never_passed() {
    let (_host, _workspace, requirements, context, mut report) = fixture();
    report.evidence.as_mut().unwrap()["after_snapshot"] = json!("different");
    assert!(
        report
            .validate(
                &requirements,
                &context,
                report.candidate_snapshot.as_deref()
            )
            .is_err()
    );
    report.verdict = Checks::Unavailable;
    report
        .validate(
            &requirements,
            &context,
            report.candidate_snapshot.as_deref(),
        )
        .unwrap();
}
