use arc_core::{ArcExample, ArcGrid, ArcTask, ArcTaskId, canonicalize_task};
use arc_solvers::{
    ArcCommonVerifier, ArcRecursiveTinyModelBootstrap, ArcRecursiveTinyModelBootstrapMode,
    ArcRecursiveTinyModelConfig, ArcRecursiveTinyModelLane, ArcRecursiveTinyModelState,
    ArcRecursiveTinyModelStepOutput, ArcRecursiveTinyModelTracePhase, ArcTinyModel,
    CandidateVerifier, LaneBatchStatus, SolverRefusalCode, TaskBudget,
};
use serde::Serialize;

fn grid(width: u8, height: u8, rows: &[&[u8]]) -> ArcGrid {
    let cells = rows
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    ArcGrid::new(width, height, cells).expect("grid should validate")
}

fn task(id: &str, pairs: &[(ArcGrid, ArcGrid)], test: Vec<ArcGrid>) -> ArcTask {
    ArcTask::new(
        ArcTaskId::new(id).expect("task id"),
        pairs
            .iter()
            .cloned()
            .map(|(input, output)| ArcExample { input, output })
            .collect(),
        test,
    )
    .expect("task should validate")
}

fn budget() -> TaskBudget {
    TaskBudget {
        max_wall_ms: 10_000,
        max_candidates: 6,
        max_verifier_evals: 4,
        max_train_pair_execs: 64,
        max_refinement_steps: 4,
        max_model_forward_calls: 4,
        max_ttt_updates: 2,
        max_memory_mb: 128,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct ScriptedLatentState {
    label: String,
    updated: bool,
}

#[derive(Clone)]
struct ScriptedStep {
    label: &'static str,
    answer: ArcGrid,
    halt_score: f32,
    continue_score: f32,
}

struct ScriptedTinyModel {
    bootstrap_answer: ArcGrid,
    steps: Vec<ScriptedStep>,
    update_answer: Option<ArcGrid>,
    seen_bootstrap_mode: Option<ArcRecursiveTinyModelBootstrapMode>,
    update_calls: usize,
}

impl ArcTinyModel for ScriptedTinyModel {
    type Error = std::convert::Infallible;
    type LatentState = ScriptedLatentState;

    fn initialize(
        &mut self,
        _task: &arc_core::CanonicalTask,
        mode: ArcRecursiveTinyModelBootstrapMode,
    ) -> Result<ArcRecursiveTinyModelBootstrap<Self::LatentState>, Self::Error> {
        self.seen_bootstrap_mode = Some(mode);
        Ok(ArcRecursiveTinyModelBootstrap {
            state: ArcRecursiveTinyModelState {
                latent_state: ScriptedLatentState {
                    label: String::from("bootstrap"),
                    updated: false,
                },
                answer_grid: self.bootstrap_answer.clone(),
            },
            note: String::from("initialized scripted tiny model"),
        })
    }

    fn test_time_update(
        &mut self,
        _task: &arc_core::CanonicalTask,
        state: &mut ArcRecursiveTinyModelState<Self::LatentState>,
    ) -> Result<Option<String>, Self::Error> {
        self.update_calls += 1;
        let Some(updated_answer) = self.update_answer.clone() else {
            return Ok(None);
        };
        state.latent_state.updated = true;
        state.latent_state.label = String::from("updated");
        state.answer_grid = updated_answer;
        Ok(Some(String::from("applied scripted test-time update")))
    }

    fn step(
        &mut self,
        _task: &arc_core::CanonicalTask,
        _state: &ArcRecursiveTinyModelState<Self::LatentState>,
        step_index: u32,
    ) -> Result<ArcRecursiveTinyModelStepOutput<Self::LatentState>, Self::Error> {
        let scripted = &self.steps[step_index as usize];
        Ok(ArcRecursiveTinyModelStepOutput {
            state: ArcRecursiveTinyModelState {
                latent_state: ScriptedLatentState {
                    label: scripted.label.to_string(),
                    updated: self.update_calls > 0,
                },
                answer_grid: scripted.answer.clone(),
            },
            halt_score: scripted.halt_score,
            continue_score: scripted.continue_score,
            note: format!("scripted recursive step {}", scripted.label),
        })
    }
}

#[test]
fn recursive_lane_tracks_bounded_steps_and_solves() {
    let solved = grid(2, 2, &[&[1, 1], &[1, 1]]);
    let raw_task = task(
        "recursive-tiny-model",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                solved.clone(),
            ),
            (
                grid(2, 2, &[&[0, 1], &[1, 0]]),
                solved.clone(),
            ),
        ],
        vec![grid(2, 2, &[&[1, 0], &[1, 0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let model = ScriptedTinyModel {
        bootstrap_answer: grid(2, 2, &[&[0, 0], &[0, 0]]),
        steps: vec![
            ScriptedStep {
                label: "near-miss",
                answer: grid(2, 2, &[&[1, 1], &[1, 0]]),
                halt_score: 0.2,
                continue_score: 0.8,
            },
            ScriptedStep {
                label: "solved",
                answer: solved.clone(),
                halt_score: 0.95,
                continue_score: 0.05,
            },
        ],
        update_answer: None,
        seen_bootstrap_mode: None,
        update_calls: 0,
    };
    let mut lane = ArcRecursiveTinyModelLane::new(
        model,
        ArcRecursiveTinyModelConfig {
            max_recursive_steps: 4,
            ..ArcRecursiveTinyModelConfig::default()
        },
    );

    let run = lane.run(&task, budget()).expect("lane should run");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Proposed);
    assert_eq!(run.refinement_batch.status, LaneBatchStatus::Proposed);
    assert_eq!(run.step_traces.len(), 3);
    assert_eq!(
        run.step_traces[0].phase,
        ArcRecursiveTinyModelTracePhase::Bootstrap
    );
    assert_eq!(
        run.step_traces[1].phase,
        ArcRecursiveTinyModelTracePhase::RecursiveStep
    );
    assert_eq!(
        run.step_traces[2].phase,
        ArcRecursiveTinyModelTracePhase::RecursiveStep
    );

    let best = run.best_hypothesis().expect("best hypothesis should exist");
    let report = ArcCommonVerifier::default()
        .evaluate(&task, best, budget())
        .expect("verification should succeed");
    assert!(report.verifier_pass);
}

#[test]
fn recursive_lane_applies_test_time_update_and_records_mode() {
    let raw_task = task(
        "recursive-tiny-model-ttt",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 1], &[1, 1]]),
        )],
        vec![grid(2, 2, &[&[0, 1], &[0, 1]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let model = ScriptedTinyModel {
        bootstrap_answer: grid(2, 2, &[&[0, 0], &[0, 0]]),
        steps: vec![ScriptedStep {
            label: "post-update",
            answer: grid(2, 2, &[&[1, 1], &[1, 1]]),
            halt_score: 0.9,
            continue_score: 0.1,
        }],
        update_answer: Some(grid(2, 2, &[&[1, 0], &[1, 0]])),
        seen_bootstrap_mode: None,
        update_calls: 0,
    };
    let mut lane = ArcRecursiveTinyModelLane::new(
        model,
        ArcRecursiveTinyModelConfig {
            bootstrap_mode: ArcRecursiveTinyModelBootstrapMode::Scratch,
            max_test_time_updates: 1,
            max_recursive_steps: 2,
            ..ArcRecursiveTinyModelConfig::default()
        },
    );

    let run = lane.run(&task, budget()).expect("lane should run");
    assert_eq!(run.bootstrap_mode, ArcRecursiveTinyModelBootstrapMode::Scratch);
    assert_eq!(run.ttt_updates_applied, 1);
    assert_eq!(run.step_traces.len(), 3);
    assert_eq!(
        run.step_traces[1].phase,
        ArcRecursiveTinyModelTracePhase::TestTimeUpdate
    );
    assert_eq!(
        run.refinement_batch.proposals[0]
            .hypothesis
            .static_answer
            .as_ref()
            .expect("answer exists"),
        &grid(2, 2, &[&[1, 1], &[1, 1]])
    );
}

#[test]
fn recursive_lane_refuses_when_recursive_budget_is_missing() {
    let raw_task = task(
        "recursive-tiny-model-budget",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 1], &[1, 1]]),
        )],
        vec![grid(2, 2, &[&[1, 0], &[1, 0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");
    let model = ScriptedTinyModel {
        bootstrap_answer: grid(2, 2, &[&[0, 0], &[0, 0]]),
        steps: vec![ScriptedStep {
            label: "unused",
            answer: grid(2, 2, &[&[1, 1], &[1, 1]]),
            halt_score: 1.0,
            continue_score: 0.0,
        }],
        update_answer: None,
        seen_bootstrap_mode: None,
        update_calls: 0,
    };
    let mut lane = ArcRecursiveTinyModelLane::new(model, ArcRecursiveTinyModelConfig::default());

    let mut no_recursive_budget = budget();
    no_recursive_budget.max_refinement_steps = 0;
    let run = lane
        .run(&task, no_recursive_budget)
        .expect("lane should refuse");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Refused);
    assert_eq!(
        run.proposal_batch
            .refusal
            .as_ref()
            .expect("refusal exists")
            .code,
        SolverRefusalCode::MinimumBudgetNotMet
    );
    assert_eq!(run.refinement_batch.status, LaneBatchStatus::Empty);
}
