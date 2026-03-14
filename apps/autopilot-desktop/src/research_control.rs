use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_research::{
    ExperimentComparisonError, ExperimentResult, ExperimentRunStatus, ExperimentScoreEvaluation,
    ExperimentSpec, PromotionDecision, PromotionReasonCode, PromotionRecord, ResearchRunner,
    ResearchRunnerInvocation,
};
use serde::{Deserialize, Serialize};

const DESKTOP_RESEARCH_SCHEMA_VERSION: u16 = 1;
const DESKTOP_RESEARCH_STATE_FILENAME: &str = "research-frontier.json";

static DESKTOP_RESEARCH_CONTROLLER: OnceLock<Mutex<DesktopResearchController>> = OnceLock::new();

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopResearchCandidateStatus {
    pub candidate_id: String,
    pub experiment_id: String,
    pub run_id: String,
    pub status: String,
    pub decision: String,
    pub promotable: bool,
    pub hard_gate_failed: bool,
    pub weighted_score: Option<String>,
    pub reasons: Vec<String>,
    pub result_digest: String,
    pub summary: String,
    pub recorded_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopResearchProgramStatus {
    pub program_id: String,
    pub family: String,
    pub objective: String,
    pub leader_candidate_id: Option<String>,
    pub promoted_candidate_id: Option<String>,
    pub frontier_candidate_ids: Vec<String>,
    pub candidate_count: usize,
    pub last_decision: Option<String>,
    pub candidates: Vec<DesktopResearchCandidateStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DesktopResearchStatus {
    pub schema_version: u16,
    pub storage_path: String,
    pub updated_at_epoch_ms: u64,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub programs: Vec<DesktopResearchProgramStatus>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct PersistedResearchState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    last_action: Option<String>,
    last_error: Option<String>,
    programs: Vec<PersistedResearchProgram>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedResearchProgram {
    program_id: String,
    family: String,
    objective: String,
    leader_candidate_id: Option<String>,
    promoted_candidate_id: Option<String>,
    candidates: Vec<PersistedResearchCandidateRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedResearchCandidateRecord {
    spec: ExperimentSpec,
    result: ExperimentResult,
    evaluation: Option<ExperimentScoreEvaluation>,
    promotion: PromotionRecord,
    recorded_at_epoch_ms: u64,
}

struct DesktopResearchController {
    storage_path: PathBuf,
    state: PersistedResearchState,
}

impl DesktopResearchController {
    fn load(storage_path: PathBuf) -> Self {
        let state = fs::read(storage_path.as_path())
            .ok()
            .and_then(|raw| serde_json::from_slice::<PersistedResearchState>(&raw).ok())
            .unwrap_or_else(|| PersistedResearchState {
                schema_version: DESKTOP_RESEARCH_SCHEMA_VERSION,
                updated_at_epoch_ms: current_epoch_ms(),
                last_action: None,
                last_error: None,
                programs: Vec::new(),
            });
        Self {
            storage_path,
            state,
        }
    }

    fn persist(&mut self) -> Result<(), String> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create research state dir: {error}"))?;
        }
        self.state.schema_version = DESKTOP_RESEARCH_SCHEMA_VERSION;
        self.state.updated_at_epoch_ms = current_epoch_ms();
        let raw = serde_json::to_vec_pretty(&self.state)
            .map_err(|error| format!("Failed to encode research state: {error}"))?;
        fs::write(self.storage_path.as_path(), raw)
            .map_err(|error| format!("Failed to write research state: {error}"))
    }

    fn status(&self) -> DesktopResearchStatus {
        DesktopResearchStatus {
            schema_version: self.state.schema_version,
            storage_path: self.storage_path.display().to_string(),
            updated_at_epoch_ms: self.state.updated_at_epoch_ms,
            last_action: self.state.last_action.clone(),
            last_error: self.state.last_error.clone(),
            programs: self
                .state
                .programs
                .iter()
                .map(status_from_program)
                .collect(),
        }
    }

    fn reset(&mut self) -> Result<DesktopResearchStatus, String> {
        self.state = PersistedResearchState {
            schema_version: DESKTOP_RESEARCH_SCHEMA_VERSION,
            updated_at_epoch_ms: current_epoch_ms(),
            last_action: Some(String::from("Reset research frontier state")),
            last_error: None,
            programs: Vec::new(),
        };
        self.persist()?;
        Ok(self.status())
    }

    fn run_invocations(
        &mut self,
        program_id: &str,
        objective: &str,
        invocations: Vec<ResearchRunnerInvocation>,
    ) -> Result<DesktopResearchProgramStatus, String> {
        let Some(first_invocation) = invocations.first() else {
            return Err(String::from(
                "Research program requires at least one invocation",
            ));
        };
        let family = first_invocation.spec.family.kind().label().to_string();
        let mut program = self
            .state
            .programs
            .iter()
            .position(|existing| existing.program_id == program_id)
            .map(|index| self.state.programs.remove(index))
            .unwrap_or(PersistedResearchProgram {
                program_id: program_id.to_string(),
                family,
                objective: objective.to_string(),
                leader_candidate_id: None,
                promoted_candidate_id: None,
                candidates: Vec::new(),
            });

        for invocation in invocations {
            let record = ResearchRunner::execute_local(&invocation)
                .map_err(|error| format!("Failed to run research invocation: {error}"))?;
            let evaluation = invocation
                .spec
                .score_contract
                .evaluate_result(&record.result)
                .ok();
            let promotion = build_promotion(
                program.candidates.as_slice(),
                &invocation.spec,
                &record.result,
                evaluation.as_ref(),
            );
            if matches!(
                promotion.decision,
                PromotionDecision::Keep | PromotionDecision::Promote
            ) {
                program.leader_candidate_id = Some(invocation.spec.candidate_id.clone());
            }
            let record = PersistedResearchCandidateRecord {
                spec: invocation.spec,
                result: record.result,
                evaluation,
                promotion,
                recorded_at_epoch_ms: current_epoch_ms(),
            };
            program.candidates.push(record);
        }

        self.state.last_error = None;
        self.state.last_action = Some(format!(
            "Ran research program {} with {} candidates",
            program_id,
            program.candidates.len()
        ));
        self.state.programs.push(program);
        self.persist()?;
        self.state
            .programs
            .iter()
            .find(|program| program.program_id == program_id)
            .map(status_from_program)
            .ok_or_else(|| String::from("Research program was not persisted"))
    }

    fn promote_leader(
        &mut self,
        program_id: &str,
        note: &str,
    ) -> Result<DesktopResearchProgramStatus, String> {
        let Some(program_index) = self
            .state
            .programs
            .iter()
            .position(|program| program.program_id == program_id)
        else {
            return Err(format!("Unknown research program `{program_id}`"));
        };
        let program = &mut self.state.programs[program_index];
        let Some(leader_candidate_id) = program.leader_candidate_id.clone() else {
            return Err(format!(
                "Research program `{program_id}` has no leader to promote"
            ));
        };
        let Some(candidate) = program
            .candidates
            .iter_mut()
            .find(|candidate| candidate.spec.candidate_id == leader_candidate_id)
        else {
            return Err(format!(
                "Research program `{program_id}` lost leader `{leader_candidate_id}`"
            ));
        };
        candidate.promotion = PromotionRecord::new(
            format!("promote-{}", candidate.result.run_id),
            &candidate.result,
            PromotionDecision::Promote,
            true,
            vec![PromotionReasonCode::RecheckedWinner],
            Some(note.to_string()),
        );
        program.promoted_candidate_id = Some(leader_candidate_id);
        self.state.last_error = None;
        self.state.last_action = Some(format!("Promoted research leader for {}", program_id));
        self.persist()?;
        Ok(status_from_program(&self.state.programs[program_index]))
    }
}

pub(crate) fn research_state_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(DESKTOP_RESEARCH_STATE_FILENAME)
}

fn research_controller() -> &'static Mutex<DesktopResearchController> {
    DESKTOP_RESEARCH_CONTROLLER
        .get_or_init(|| Mutex::new(DesktopResearchController::load(research_state_path())))
}

pub(crate) fn research_status() -> Result<DesktopResearchStatus, String> {
    research_controller()
        .lock()
        .map_err(|_| String::from("Research controller lock poisoned"))
        .map(|controller| controller.status())
}

pub(crate) fn reset_research_state() -> Result<DesktopResearchStatus, String> {
    research_controller()
        .lock()
        .map_err(|_| String::from("Research controller lock poisoned"))?
        .reset()
}

pub(crate) fn run_research_invocations(
    program_id: &str,
    objective: &str,
    invocations: Vec<ResearchRunnerInvocation>,
) -> Result<DesktopResearchProgramStatus, String> {
    research_controller()
        .lock()
        .map_err(|_| String::from("Research controller lock poisoned"))?
        .run_invocations(program_id, objective, invocations)
}

pub(crate) fn promote_research_leader(
    program_id: &str,
    note: &str,
) -> Result<DesktopResearchProgramStatus, String> {
    research_controller()
        .lock()
        .map_err(|_| String::from("Research controller lock poisoned"))?
        .promote_leader(program_id, note)
}

fn status_from_program(program: &PersistedResearchProgram) -> DesktopResearchProgramStatus {
    let frontier_candidate_ids = program
        .candidates
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.promotion.decision,
                PromotionDecision::Keep | PromotionDecision::Branch | PromotionDecision::Promote
            )
        })
        .map(|candidate| candidate.spec.candidate_id.clone())
        .collect::<Vec<_>>();
    let last_decision = program
        .candidates
        .last()
        .map(|candidate| format!("{:?}", candidate.promotion.decision).to_lowercase());
    DesktopResearchProgramStatus {
        program_id: program.program_id.clone(),
        family: program.family.clone(),
        objective: program.objective.clone(),
        leader_candidate_id: program.leader_candidate_id.clone(),
        promoted_candidate_id: program.promoted_candidate_id.clone(),
        frontier_candidate_ids,
        candidate_count: program.candidates.len(),
        last_decision,
        candidates: program
            .candidates
            .iter()
            .map(|candidate| DesktopResearchCandidateStatus {
                candidate_id: candidate.spec.candidate_id.clone(),
                experiment_id: candidate.spec.experiment_id.clone(),
                run_id: candidate.result.run_id.clone(),
                status: format!("{:?}", candidate.result.status).to_lowercase(),
                decision: format!("{:?}", candidate.promotion.decision).to_lowercase(),
                promotable: candidate.promotion.promotable,
                hard_gate_failed: candidate
                    .evaluation
                    .as_ref()
                    .is_some_and(|evaluation| evaluation.hard_gate_failed),
                weighted_score: candidate
                    .evaluation
                    .as_ref()
                    .map(|evaluation| evaluation.weighted_score.to_string()),
                reasons: candidate
                    .promotion
                    .reasons
                    .iter()
                    .map(|reason| format!("{:?}", reason).to_lowercase())
                    .collect(),
                result_digest: candidate.result.result_digest.clone(),
                summary: candidate
                    .promotion
                    .note
                    .clone()
                    .unwrap_or_else(|| format!("candidate {}", candidate.spec.candidate_id)),
                recorded_at_epoch_ms: candidate.recorded_at_epoch_ms,
            })
            .collect(),
    }
}

fn build_promotion(
    existing: &[PersistedResearchCandidateRecord],
    spec: &ExperimentSpec,
    result: &ExperimentResult,
    evaluation: Option<&ExperimentScoreEvaluation>,
) -> PromotionRecord {
    if result.status != ExperimentRunStatus::Succeeded {
        return PromotionRecord::new(
            format!("promotion-{}", result.run_id),
            result,
            PromotionDecision::Blocked,
            false,
            vec![PromotionReasonCode::MissingEvidence],
            Some(format!(
                "candidate {} failed before scoring",
                spec.candidate_id
            )),
        );
    }
    let Some(evaluation) = evaluation else {
        return PromotionRecord::new(
            format!("promotion-{}", result.run_id),
            result,
            PromotionDecision::Blocked,
            false,
            vec![PromotionReasonCode::MissingEvidence],
            Some(format!(
                "candidate {} could not be evaluated",
                spec.candidate_id
            )),
        );
    };
    if evaluation.hard_gate_failed {
        let mut reasons = vec![PromotionReasonCode::HardGateFailed];
        if !evaluation.missing_metrics.is_empty() {
            reasons.push(PromotionReasonCode::MissingEvidence);
        }
        return PromotionRecord::new(
            format!("promotion-{}", result.run_id),
            result,
            PromotionDecision::Blocked,
            false,
            reasons,
            Some(format!(
                "candidate {} failed one required gate",
                spec.candidate_id
            )),
        );
    }
    let leader = existing
        .iter()
        .filter_map(|candidate| {
            candidate
                .evaluation
                .as_ref()
                .map(|evaluation| (candidate, evaluation))
        })
        .find(|(candidate, _)| {
            matches!(
                candidate.promotion.decision,
                PromotionDecision::Keep | PromotionDecision::Promote
            )
        });
    match leader {
        None => PromotionRecord::new(
            format!("promotion-{}", result.run_id),
            result,
            PromotionDecision::Keep,
            false,
            vec![],
            Some(format!(
                "candidate {} becomes the initial frontier leader",
                spec.candidate_id
            )),
        ),
        Some((_, leader_evaluation)) => match evaluation.compare_same_contract(leader_evaluation) {
            Ok(std::cmp::Ordering::Greater) => PromotionRecord::new(
                format!("promotion-{}", result.run_id),
                result,
                PromotionDecision::Keep,
                false,
                vec![],
                Some(format!(
                    "candidate {} becomes the new frontier leader",
                    spec.candidate_id
                )),
            ),
            Ok(std::cmp::Ordering::Equal) => PromotionRecord::new(
                format!("promotion-{}", result.run_id),
                result,
                PromotionDecision::Branch,
                false,
                vec![PromotionReasonCode::BranchForFurtherSearch],
                Some(format!(
                    "candidate {} ties the current leader",
                    spec.candidate_id
                )),
            ),
            Ok(std::cmp::Ordering::Less) => PromotionRecord::new(
                format!("promotion-{}", result.run_id),
                result,
                PromotionDecision::Discard,
                false,
                vec![],
                Some(format!(
                    "candidate {} lost to the current leader",
                    spec.candidate_id
                )),
            ),
            Err(ExperimentComparisonError::ContractMismatch { .. }) => PromotionRecord::new(
                format!("promotion-{}", result.run_id),
                result,
                PromotionDecision::Blocked,
                false,
                vec![PromotionReasonCode::MissingEvidence],
                Some(format!(
                    "candidate {} could not be compared because the score contract changed",
                    spec.candidate_id
                )),
            ),
        },
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{DesktopResearchController, status_from_program};
    use psionic_research::{
        CandidateMutation, ExperimentArtifactKind, ExperimentArtifactRef, ExperimentBudget,
        ExperimentFamily, ExperimentFamilyKind, ExperimentScoreContract, ExperimentThreshold,
        ResearchRunnerInvocation, ScoreDirection, ScoreMetricSpec, ServingSchedulerPolicy,
    };

    fn sample_invocation(
        candidate_id: &str,
        batch_tokens: u32,
        prefill_share_bps: u16,
    ) -> ResearchRunnerInvocation {
        let spec = psionic_research::ExperimentSpec::new(
            format!("exp.{candidate_id}"),
            candidate_id.to_string(),
            ExperimentFamily::ServingScheduler {
                model_id: String::from("gpt-oss-20b"),
                benchmark_suite_ref: String::from("benchmark://serve/local-weather"),
                policy: ServingSchedulerPolicy::new(batch_tokens, 8, prefill_share_bps, 5_500, 25),
            },
            vec![ExperimentArtifactRef::new(
                ExperimentArtifactKind::ServedArtifact,
                "served://gpt-oss-20b",
                "served-digest",
            )],
            CandidateMutation::new(
                format!("mutation.{candidate_id}"),
                Some(String::from("baseline")),
                ExperimentFamilyKind::ServingScheduler,
                vec![String::from("serve.scheduler.prefill_share_bps")],
            ),
            psionic_research::ExperimentRuntimeProfile::new("runner-digest")
                .with_sandbox_profile_ref("sandbox://research/local")
                .with_requested_backend("cuda"),
            ExperimentBudget::new(30_000, format!("runs/{candidate_id}")),
            ExperimentScoreContract::new(
                "serve.score.v1",
                ExperimentFamilyKind::ServingScheduler,
                vec![
                    ScoreMetricSpec::new(
                        "throughput_tokens_per_second",
                        "tokens_per_second",
                        ScoreDirection::Maximize,
                        7_000,
                    )
                    .with_hard_gate(ExperimentThreshold::at_least(150_000_000)),
                    ScoreMetricSpec::new(
                        "p95_latency_ms",
                        "milliseconds",
                        ScoreDirection::Minimize,
                        3_000,
                    )
                    .with_hard_gate(ExperimentThreshold::at_most(65_000)),
                ],
            ),
        );
        ResearchRunnerInvocation::new(format!("run.{candidate_id}"), spec, 1_000)
    }

    #[test]
    fn controller_keeps_first_candidate_and_discards_worse_one() {
        let tempdir = tempdir().expect("tempdir should exist");
        let mut controller =
            DesktopResearchController::load(tempdir.path().join("research-frontier.json"));
        let status = controller
            .run_invocations(
                "serve.frontier",
                "serve scheduler frontier",
                vec![
                    sample_invocation("candidate-a", 8192, 4500),
                    sample_invocation("candidate-b", 4096, 6500),
                ],
            )
            .expect("controller should record results");
        assert_eq!(status.leader_candidate_id.as_deref(), Some("candidate-a"));
        assert_eq!(status.candidates.len(), 2);
        assert!(
            status
                .candidates
                .iter()
                .any(|candidate| candidate.decision == "discard")
        );
    }

    #[test]
    fn controller_persists_and_recovers_status() {
        let tempdir = tempdir().expect("tempdir should exist");
        let path = tempdir.path().join("research-frontier.json");
        let mut controller = DesktopResearchController::load(path.clone());
        controller
            .run_invocations(
                "serve.frontier",
                "serve scheduler frontier",
                vec![sample_invocation("candidate-a", 8192, 4500)],
            )
            .expect("controller should record results");
        let recovered = DesktopResearchController::load(path);
        let status = recovered.status();
        assert_eq!(status.programs.len(), 1);
        assert_eq!(
            status.programs[0].leader_candidate_id.as_deref(),
            Some("candidate-a")
        );
        assert_eq!(
            status_from_program(&recovered.state.programs[0]).candidate_count,
            1
        );
    }
}
