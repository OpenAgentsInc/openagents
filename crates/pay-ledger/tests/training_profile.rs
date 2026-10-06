//! Synthetic pinned artifact and protected checker, without model training or paid execution.
use pay_ledger::markets::training::*;
fn pin(c: char) -> String {
    c.to_string().repeat(64)
}
fn fixture() -> (Terms, Trust, Evidence) {
    let terms = Terms {
        order: pin('1'),
        corpus: pin('2'),
        dataset: pin('3'),
        baseline_checkpoint: pin('4'),
        recipe: pin('5'),
        seed: 7,
        worker_class: pin('6'),
        deliverable_contract: pin('7'),
        checker: pin('8'),
        evaluation_partition: pin('9'),
        training_groups: vec![pin('a')],
        rights: Rights {
            provenance: pin('b'),
            license: pin('c'),
            train: true,
            evaluate: true,
            disclose: false,
            redistribute: false,
        },
        requires_disclosure: false,
        requires_redistribution: false,
        max_all_in_msat: 100,
        compute_obligation: pin('d'),
        improvement_obligation: pin('e'),
        data_license_obligation: pin('f'),
    };
    let evidence = Evidence {
        terms_fingerprint: terms.fingerprint().unwrap(),
        artifact: pin('0'),
        protected_checker_receipt: pin('1'),
        evaluation_partition: terms.evaluation_partition.clone(),
        accepted: true,
        improvement: 1,
        training_cost_msat: Some(40),
        check_cost_msat: Some(10),
        search_cost_msat: Some(20),
        failed_attempt_cost_msat: Some(5),
    };
    let trust = Trust {
        rights_verified: true,
        checker: terms.checker.clone(),
        evaluation_partition: terms.evaluation_partition.clone(),
        protected_groups: vec![pin('b')],
        worker_cannot_edit_checker: true,
        verified_evaluation: Some(VerifiedEvaluation {
            receipt: evidence.protected_checker_receipt.clone(),
            artifact: evidence.artifact.clone(),
            terms_fingerprint: evidence.terms_fingerprint.clone(),
            evaluation_partition: evidence.evaluation_partition.clone(),
            accepted: true,
            improvement: 1,
        }),
        verified_costs_msat: Some([40, 10, 20, 5]),
    };
    (terms, trust, evidence)
}
#[test]
fn rights_group_leakage_and_worker_editable_checker_refuse() {
    let (mut terms, mut trust, _) = fixture();
    terms.validate(&trust).unwrap();
    terms.rights.train = false;
    assert!(terms.validate(&trust).is_err());
    terms.rights.train = true;
    terms.requires_redistribution = true;
    assert!(terms.validate(&trust).is_err());
    terms.requires_redistribution = false;
    trust.protected_groups = terms.training_groups.clone();
    assert!(terms.validate(&trust).is_err());
    trust.protected_groups = vec![pin('b')];
    trust.worker_cannot_edit_checker = false;
    assert!(terms.validate(&trust).is_err());
}
#[test]
fn exact_artifact_and_protected_checker_evidence_replay() {
    let (terms, mut trust, mut evidence) = fixture();
    let scratch = tempfile::tempdir().unwrap();
    let worker = scratch.path().join("worker");
    let checker = scratch.path().join("protected-checker");
    std::fs::create_dir(&worker).unwrap();
    std::fs::create_dir(&checker).unwrap();
    let bytes = b"synthetic checkpoint: seed 7";
    std::fs::write(worker.join("checkpoint"), bytes).unwrap();
    std::fs::write(checker.join("expected"), bytes).unwrap();
    assert_eq!(
        std::fs::read(worker.join("checkpoint")).unwrap(),
        std::fs::read(checker.join("expected")).unwrap()
    );
    evidence.artifact = pay_ledger::digest(std::str::from_utf8(bytes).unwrap());
    trust.verified_evaluation.as_mut().unwrap().artifact = evidence.artifact.clone();
    let saved = serde_json::to_vec(&evidence).unwrap();
    let recovered: Evidence = serde_json::from_slice(&saved).unwrap();
    assert_eq!(recovered.eligible_improvement(&terms, &trust).unwrap(), 75);
    let mut changed = terms.clone();
    changed.seed += 1;
    assert!(recovered.eligible_improvement(&changed, &trust).is_err());
    evidence.artifact = pin('0');
    assert!(evidence.eligible_improvement(&terms, &trust).is_err());
    println!(
        "{}",
        serde_json::json!({"schema":"openagents.training-qualification.v1","synthetic_checkpoint":true,
       "independent_operators":false,"terms":terms,"evidence":recovered,"fake_all_in_msat":75,
       "restart":"same pinned artifact and checker receipt","real_training":"unverified","funded_qualification":"unverified"})
    );
}
#[test]
fn unknown_over_budget_and_worker_claimed_metric_cannot_earn_improvement() {
    let (terms, trust, mut evidence) = fixture();
    evidence.failed_attempt_cost_msat = None;
    assert!(evidence.eligible_improvement(&terms, &trust).is_err());
    evidence.failed_attempt_cost_msat = Some(5);
    evidence.improvement = 100;
    assert!(evidence.eligible_improvement(&terms, &trust).is_err());
    evidence.improvement = 1;
    let mut limited = terms.clone();
    limited.max_all_in_msat = 74;
    evidence.terms_fingerprint = limited.fingerprint().unwrap();
    let mut approved = trust.clone();
    approved
        .verified_evaluation
        .as_mut()
        .unwrap()
        .terms_fingerprint = evidence.terms_fingerprint.clone();
    assert!(evidence.eligible_improvement(&limited, &approved).is_err());
}
