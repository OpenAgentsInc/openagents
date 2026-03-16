use arc_core::{ArcExample, ArcGrid, ArcTask, ArcTaskId, canonicalize_task};
use arc_solvers::{
    ArcCommonVerifier, ArcTransductiveLane, ArcTransductiveLaneConfig, CandidateVerifier,
    LaneBatchStatus, PsionicTextGenerationAdapter, SolverRefusalCode, TaskBudget,
};
use psionic_serve::{
    ContextOverflowPolicy, CpuReferenceTextGenerationService, GenerationOptions, GenerationRequest,
    GenerationResponse, ReferenceWordDecoder, TerminationReason, TextGenerationExecutor,
    TokenSequence,
};

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
        max_candidates: 4,
        max_verifier_evals: 4,
        max_train_pair_execs: 64,
        max_refinement_steps: 0,
        max_model_forward_calls: 4,
        max_ttt_updates: 0,
        max_memory_mb: 128,
    }
}

struct StaticPsionicExecutor {
    response_text: String,
}

impl TextGenerationExecutor for StaticPsionicExecutor {
    type Error = std::convert::Infallible;

    fn generate(&mut self, request: &GenerationRequest) -> Result<GenerationResponse, Self::Error> {
        Ok(GenerationResponse::new(
            request,
            None,
            TokenSequence::new(Vec::new()),
            self.response_text.clone(),
            0,
            0,
            TerminationReason::EndOfSequence,
        ))
    }
}

#[test]
fn psionic_reference_adapter_surfaces_parse_refusal_cleanly() {
    let raw_task = task(
        "transductive-reference",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 0], &[0, 1]]),
        )],
        vec![grid(1, 1, &[&[0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let service = CpuReferenceTextGenerationService::new().expect("reference service");
    let mut options = GenerationOptions::greedy(4);
    options.context_overflow_policy = ContextOverflowPolicy::TruncateOldest;
    let adapter = PsionicTextGenerationAdapter::new(
        service,
        ReferenceWordDecoder::new().descriptor().clone(),
        options,
        "arc-transductive",
    );
    let mut lane = ArcTransductiveLane::new(adapter, ArcTransductiveLaneConfig::default());

    let run = lane.run(&task, budget()).expect("lane should run");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Refused);
    assert_eq!(
        run.proposal_batch
            .refusal
            .as_ref()
            .expect("refusal should exist")
            .code,
        SolverRefusalCode::InvalidCandidate
    );
    assert!(run.adapter_response.candidate_grid.is_none());
    assert!(run.adapter_response.parse_error.is_some());
}

#[test]
fn transductive_lane_accepts_psionic_backed_grid_json_candidates() {
    let raw_task = task(
        "transductive-static",
        &[
            (
                grid(2, 2, &[&[1, 0], &[0, 1]]),
                grid(2, 2, &[&[1, 1], &[1, 1]]),
            ),
            (
                grid(2, 2, &[&[0, 1], &[1, 0]]),
                grid(2, 2, &[&[1, 1], &[1, 1]]),
            ),
        ],
        vec![grid(2, 2, &[&[1, 0], &[1, 0]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let adapter = PsionicTextGenerationAdapter::new(
        StaticPsionicExecutor {
            response_text: String::from("{\"grid\": [[1,1],[1,1]]}"),
        },
        ReferenceWordDecoder::new().descriptor().clone(),
        GenerationOptions::greedy(8),
        "arc-transductive",
    );
    let mut lane = ArcTransductiveLane::new(adapter, ArcTransductiveLaneConfig::default());

    let run = lane.run(&task, budget()).expect("lane should run");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Proposed);
    let hypothesis = &run.proposal_batch.proposals[0].hypothesis;
    let report = ArcCommonVerifier::default()
        .evaluate(&task, hypothesis, budget())
        .expect("verification should succeed");
    assert!(report.verifier_pass);
}

#[test]
fn transductive_lane_refuses_multiple_test_inputs() {
    let raw_task = task(
        "transductive-multi-test",
        &[(
            grid(2, 2, &[&[1, 0], &[0, 1]]),
            grid(2, 2, &[&[1, 0], &[0, 1]]),
        )],
        vec![grid(1, 1, &[&[0]]), grid(1, 1, &[&[1]])],
    );
    let task = canonicalize_task(&raw_task).expect("task should canonicalize");

    let adapter = PsionicTextGenerationAdapter::new(
        StaticPsionicExecutor {
            response_text: String::from("{\"grid\": [[0]]}"),
        },
        ReferenceWordDecoder::new().descriptor().clone(),
        GenerationOptions::greedy(4),
        "arc-transductive",
    );
    let mut lane = ArcTransductiveLane::new(adapter, ArcTransductiveLaneConfig::default());

    let run = lane
        .run(&task, budget())
        .expect("lane should return refusal");
    assert_eq!(run.proposal_batch.status, LaneBatchStatus::Refused);
    assert_eq!(
        run.proposal_batch
            .refusal
            .as_ref()
            .expect("refusal should exist")
            .code,
        SolverRefusalCode::UnsupportedTask
    );
}
