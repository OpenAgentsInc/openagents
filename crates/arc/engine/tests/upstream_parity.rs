use arc_core::{ArcAction, ArcActionKind, ArcGameState};
use arc_engine::{ArcEngine, ArcEngineStepOutcome, load_game_package};
use serde::Deserialize;

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[derive(Debug, Deserialize)]
struct ParityManifest {
    schema_version: u16,
    bounded_scope: String,
    cases: Vec<ParityCase>,
}

#[derive(Debug, Deserialize)]
struct ParityCase {
    game_id: String,
    package_path: String,
    source_files: Vec<ParitySourceFile>,
    scripts: Vec<ParityScript>,
}

#[derive(Debug, Deserialize)]
struct ParitySourceFile {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct ParityScript {
    id: String,
    actions: Vec<ArcAction>,
    reset_expectation: StepExpectation,
    final_expectation: StepExpectation,
}

#[derive(Debug, Deserialize)]
struct StepExpectation {
    full_reset: bool,
    level_completed: bool,
    advanced_level: bool,
    frames_len: usize,
    game_state: ArcGameState,
    levels_completed: u16,
    level_index: usize,
    available_actions: Vec<ArcActionKind>,
}

#[test]
fn upstream_sample_parity_manifest_runs_against_translated_fixtures() {
    let manifest: ParityManifest = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("upstream/parity_manifest.json"))
            .expect("parity manifest should load"),
    )
    .expect("parity manifest should deserialize");

    assert_eq!(manifest.schema_version, 1);
    assert!(manifest.bounded_scope.contains("translated fixtures"));

    for case in manifest.cases {
        assert!(
            !case.source_files.is_empty(),
            "case must pin upstream files"
        );
        for source in &case.source_files {
            assert!(source.path.starts_with("ARC-AGI/"));
            assert_eq!(source.sha256.len(), 64, "sha256 must be hex encoded");
        }

        let package = load_game_package(fixture_path(&case.package_path))
            .expect("translated package fixture should load");
        assert_eq!(package.task_id.as_str(), case.game_id);

        for script in case.scripts {
            assert_eq!(
                script.actions.first(),
                Some(&ArcAction::Reset),
                "script {} must start with RESET",
                script.id
            );

            let mut engine =
                ArcEngine::from_package(package.clone()).expect("engine should initialize");
            let mut outcomes = Vec::with_capacity(script.actions.len());
            for action in &script.actions {
                outcomes.push(
                    engine
                        .step(action.clone())
                        .expect("translated parity action should execute"),
                );
            }

            assert_step_expectation(&outcomes[0], &script.reset_expectation);
            assert_step_expectation(
                outcomes.last().expect("script should emit a final outcome"),
                &script.final_expectation,
            );

            let recording = engine
                .recording()
                .expect("recording should serialize")
                .expect("scripts with steps should emit recordings");
            assert_eq!(recording.steps.len(), script.actions.len());
        }
    }
}

fn assert_step_expectation(outcome: &ArcEngineStepOutcome, expected: &StepExpectation) {
    assert_eq!(outcome.full_reset, expected.full_reset);
    assert_eq!(outcome.level_completed, expected.level_completed);
    assert_eq!(outcome.advanced_level, expected.advanced_level);
    assert_eq!(outcome.frames.len(), expected.frames_len);
    assert_eq!(outcome.observation.game_state, expected.game_state);
    assert_eq!(outcome.levels_completed, expected.levels_completed);
    assert_eq!(outcome.level_index, expected.level_index);
    assert_eq!(
        outcome.observation.available_actions,
        expected.available_actions
    );
}
