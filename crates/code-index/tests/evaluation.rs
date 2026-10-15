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
