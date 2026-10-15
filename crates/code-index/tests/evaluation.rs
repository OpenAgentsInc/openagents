mod common;
use code_index::{demand::Partition, evaluation::*, recipes::Context};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

fn plan(root: &Path) -> Plan {
    let context = Context::open(root, "HEAD", "test").unwrap();
    let case = Case {
        id: "variant".into(),
        family: "enum".into(),
        partition: Partition::Evaluation,
        available_ms: 20,
        repository: root.into(),
        base: context.snapshot.commit.clone(),
        task: "Add Kind::B and name it b.".into(),
        index: common::artifact(&context),
        reviewed_index: None,
        acceptance: BTreeMap::from([(
            "tests/acceptance.rs".into(),
            "use demo::{Kind,name}; #[test] fn variant() { assert_eq!(name(Kind::B),\"b\"); }"
                .into(),
        )]),
        companion_paths: vec!["src/lib.rs".into()],
        new_paths: vec![],
    };
    Plan {
        purpose: Purpose::Evaluation,
        selectors: BTreeMap::new(),
        selector_minimum_gain: None,
        selector_dataset_digest: None,
        schema: "openagents.task-index-eval-plan.v1".into(),
        frozen_ms: 30,
        index_cutoff_ms: 10,
        training_families: BTreeSet::new(),
        cases: vec![case],
        arms: vec![
            Arm {
                id: "baseline".into(),
                model: "fixture".into(),
                context: ContextKind::Current,
            },
            Arm {
                id: "recipes".into(),
                model: "fixture".into(),
                context: ContextKind::Recipes,
            },
        ],
        repetitions: 1,
        timeout_secs: 30,
        max_output_bytes: 1024 * 1024,
        build_cost_usd: None,
        refresh_cost_usd: None,
        check_cost_usd: None,
        baseline: "baseline".into(),
        candidate: "recipes".into(),
        min_families: 4,
        allowed_quality_loss: 0.0,
        max_cost_ratio: 0.9,
        scope: "synthetic runner check".into(),
    }
}

struct Fixed;
impl Runner for Fixed {
    fn run(
        &mut self,
        root: &Path,
        _: &str,
        arm: &Arm,
        _: &Plan,
        _: &Path,
    ) -> Result<EngineResult, String> {
        if arm.id == "recipes" {
            std::fs::write(root.join("src/lib.rs"),"pub enum Kind { A,B } pub fn name(k:Kind)-> &'static str {match k {Kind::A=>\"a\",Kind::B=>\"b\"}}\n").unwrap();
        }
        Ok(EngineResult {
            origin: Origin::Fixture,
            model: "fixture".into(),
            exit: Some(0),
            timed_out: false,
            usage: None,
            model_cost_usd: None,
            navigation_calls: 0,
            output_digest: "fixture".into(),
            error: None,
        })
    }
}

#[test]
fn learning_requires_replayed_training_outcomes_and_exact_held_out_experiments() {
    use code_index::learning::*;
    let dir = common::fixture();
    let scratch = tempfile::tempdir().unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| scratch.path().join("target"))
        .join("code-index-evaluation");
    let mut training = plan(dir.path());
    training.purpose = Purpose::Training;
    training.cases[0].partition = Partition::Training;
    training.training_families.insert("enum".into());
    let report = run(
        &training,
        &scratch.path().join("training"),
        &target,
        &mut Fixed,
    )
    .unwrap();
    assert!(!report.promotion_passed);
    let trial = &report.trials[1];
    let name = code_index::digest(
        format!(
            "{}:{}:{}:{}",
            trial.plan_digest, trial.case, trial.arm, trial.repetition
        )
        .as_bytes(),
    );
    let patch = std::fs::read(
        scratch
            .path()
            .join("training")
            .join(name)
            .join("change.patch"),
    )
    .unwrap();
    let artifact = &training.cases[0].index;
    let partitions = Partitions {
        schema: "openagents.task-partitions.v1".into(),
        repository: artifact.snapshot.repository.clone(),
        workspace: artifact.snapshot.workspace.clone(),
        training_cutoff_ms: atif::now_ms(),
        groups: BTreeMap::from([
            ("enum".into(), Partition::Training),
            ("heldout".into(), Partition::Evaluation),
        ]),
    };
    let verified = || {
        verify_for_learning(
            &training,
            trial,
            &patch,
            &scratch.path().join("replay"),
            &target,
        )
        .unwrap()
    };
    assert!(admit(verified(), &partitions, false).is_err());
    let mut heldout = partitions.clone();
    heldout.groups.insert("enum".into(), Partition::Locked);
    assert!(admit(verified(), &heldout, true).is_err());
    let mut early = partitions.clone();
    early.training_cutoff_ms = 1;
    assert!(admit(verified(), &early, true).is_err());
    let mut targets = admit(verified(), &partitions, true).unwrap();
    let selector = train(artifact, &partitions, &targets).unwrap();
    assert!(selector.fixture_only);
    targets[0].set_teacher_hint(Some(0.999));
    let changed_hint = train(artifact, &partitions, &targets).unwrap();
    assert_eq!(selector.weights, changed_hint.weights);
    assert_eq!(selector.training_digest, changed_hint.training_digest);
    assert!(train(artifact, &heldout, &targets).is_err());
    let mut wrong_artifact = artifact.clone();
    wrong_artifact.recipes[0].description.push_str(" changed");
    assert!(train(&wrong_artifact, &partitions, &targets).is_err());
    assert!(selector.rank(&wrong_artifact, "kind", &[]).is_err());
    assert!(
        selector
            .rank(artifact, "kind", &["unknown".into()])
            .is_err()
    );

    let cases = vec![TestCase {
        question: "Add a kind and name it.".into(),
        family: "heldout".into(),
        expected: BTreeSet::from(["add-kind".into()]),
    }];
    let mut evaluation = plan(dir.path());
    evaluation.cases[0].family = "heldout".into();
    evaluation.index_cutoff_ms = partitions.training_cutoff_ms;
    evaluation.cases[0].available_ms = partitions.training_cutoff_ms + 1;
    evaluation.frozen_ms = partitions.training_cutoff_ms + 2;
    evaluation.arms[0].context = ContextKind::Recipes;
    evaluation
        .selectors
        .insert(evaluation.candidate.clone(), selector.clone());
    evaluation.selector_minimum_gain = Some(0.01);
    evaluation.selector_dataset_digest = Some(atif::digest(&serde_json::to_value(&cases).unwrap()));
    let result = run(
        &evaluation,
        &scratch.path().join("evaluation"),
        &target,
        &mut Fixed,
    )
    .unwrap();
    let receipt = compare(
        &selector,
        None,
        artifact,
        &partitions,
        &cases,
        &evaluation,
        &result,
        0.01,
    )
    .unwrap();
    assert!(!receipt.passed);
    assert!(
        promote(
            &scratch.path().join("store"),
            &selector,
            &receipt,
            None,
            artifact,
            &partitions,
            &cases,
            &evaluation,
            &result
        )
        .is_err()
    );
    let mut forged = receipt.clone();
    forged.passed = true;
    assert!(
        promote(
            &scratch.path().join("store"),
            &selector,
            &forged,
            None,
            artifact,
            &partitions,
            &cases,
            &evaluation,
            &result
        )
        .is_err()
    );
    assert!(
        active(&scratch.path().join("store"), artifact)
            .unwrap()
            .is_none()
    );
    let mut unrelated = evaluation.clone();
    unrelated.selectors.clear();
    assert!(
        compare(
            &selector,
            None,
            artifact,
            &partitions,
            &cases,
            &unrelated,
            &result,
            0.01
        )
        .is_err()
    );
    let mut changed_cases = cases.clone();
    changed_cases[0]
        .question
        .push_str(" Changed after freezing.");
    assert!(
        compare(
            &selector,
            None,
            artifact,
            &partitions,
            &changed_cases,
            &evaluation,
            &result,
            0.01
        )
        .is_err()
    );
    assert!(rollback(&scratch.path().join("store"), "../escape", "unknown").is_err());
}

#[test]
fn independent_checks_reject_claimed_success_and_fixture_runs_cannot_promote() {
    let dir = common::fixture();
    let scratch = tempfile::tempdir().unwrap();
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(|p| std::path::PathBuf::from(p).join("code-index-evaluation"))
        .unwrap_or_else(|| scratch.path().join("target"));
    let report = run(&plan(dir.path()), scratch.path(), &target, &mut Fixed).unwrap();
    assert!(!report.trials[0].accepted);
    assert!(report.trials[1].accepted);
    assert!(!report.promotion_passed);
    assert!(report.reasons.iter().any(|r| r.contains("Fixture")));
    assert!(report.reasons.iter().any(|r| r.contains("Complete cost")));
    let frozen = plan(dir.path());
    let trial = &report.trials[1];
    let name = code_index::digest(
        format!(
            "{}:{}:{}:{}",
            trial.plan_digest, trial.case, trial.arm, trial.repetition
        )
        .as_bytes(),
    );
    let patch = std::fs::read(scratch.path().join(name).join("change.patch")).unwrap();
    let receipt = replay_trial(
        &frozen,
        trial,
        &patch,
        &scratch.path().join("replay"),
        &target,
    )
    .unwrap();
    assert!(receipt.verified);
    assert_eq!(receipt.checks_exit, Some(0));
    assert!(
        replay_trial(
            &frozen,
            trial,
            b"changed patch",
            &scratch.path().join("bad-replay"),
            &target
        )
        .is_err()
    );
    assert_eq!(
        common::git(dir.path(), &["worktree", "list", "--porcelain"])
            .lines()
            .filter(|l| l.starts_with("worktree "))
            .count(),
        1
    );
    struct Never;
    impl Runner for Never {
        fn run(
            &mut self,
            _: &Path,
            _: &str,
            _: &Arm,
            _: &Plan,
            _: &Path,
        ) -> Result<EngineResult, String> {
            panic!("completed trials must not run again")
        }
    }
    assert_eq!(
        run(&frozen, scratch.path(), &target, &mut Never)
            .unwrap()
            .trials
            .len(),
        2
    );
    let mut tampered = report.trials;
    tampered[0].plan_digest = "wrong".into();
    assert!(code_index::evaluation::report(&plan(dir.path()), tampered).is_err());
}

#[test]
fn partitions_chronology_and_acceptance_paths_are_checked_before_running() {
    let dir = common::fixture();
    let mut plan = plan(dir.path());
    plan.training_families.insert("enum".into());
    assert!(validate(&plan).is_err());
    plan.training_families.clear();
    plan.cases[0].available_ms = 1;
    assert!(validate(&plan).is_err());
    plan.cases[0].available_ms = 20;
    plan.cases[0]
        .acceptance
        .insert("../outside.rs".into(), "bad".into());
    assert!(validate(&plan).is_err());
}
