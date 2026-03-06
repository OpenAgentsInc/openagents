use std::path::PathBuf;

use codex_client::{AskForApproval, SandboxPolicy, TurnStartParams, UserInput};
use openagents_kernel_core::ids::sha256_prefixed_text;
use openagents_kernel_core::labor::{
    SettlementStatus, Submission, SubmissionStatus, Verdict, VerdictOutcome,
};
use openagents_kernel_core::receipts::{EvidenceRef, TraceContext, VerificationTier};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodexRunTrigger {
    PersonalAgent,
    AutonomousGoal {
        goal_id: String,
        goal_title: String,
    },
    LaborMarket {
        work_unit_id: String,
        contract_id: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodexRunClassification {
    PersonalAgent,
    AutonomousGoal {
        goal_id: String,
        goal_title: String,
    },
    LaborMarket {
        work_unit_id: String,
        contract_id: Option<String>,
    },
}

impl CodexRunClassification {
    pub(crate) fn from_trigger(trigger: CodexRunTrigger) -> Self {
        match trigger {
            CodexRunTrigger::PersonalAgent => Self::PersonalAgent,
            CodexRunTrigger::AutonomousGoal {
                goal_id,
                goal_title,
            } => Self::AutonomousGoal {
                goal_id,
                goal_title,
            },
            CodexRunTrigger::LaborMarket {
                work_unit_id,
                contract_id,
            } => Self::LaborMarket {
                work_unit_id,
                contract_id,
            },
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::PersonalAgent => "personal_agent",
            Self::AutonomousGoal { .. } => "autonomous_goal",
            Self::LaborMarket { .. } => "labor_market",
        }
    }

    pub(crate) fn is_economically_meaningful(&self) -> bool {
        !matches!(self, Self::PersonalAgent)
    }

    pub(crate) fn is_labor_market_bound(&self) -> bool {
        matches!(self, Self::LaborMarket { .. })
    }

    pub(crate) fn timeline_descriptor(&self) -> String {
        let mut descriptor = format!(
            "class={} economic={} labor_bound={}",
            self.label(),
            self.is_economically_meaningful(),
            self.is_labor_market_bound()
        );
        match self {
            Self::PersonalAgent => {}
            Self::AutonomousGoal {
                goal_id,
                goal_title,
            } => {
                descriptor.push_str(&format!(" goal_id={goal_id} goal_title={goal_title:?}"));
            }
            Self::LaborMarket {
                work_unit_id,
                contract_id,
            } => {
                descriptor.push_str(&format!(
                    " work_unit_id={work_unit_id} contract_id={contract_id:?}"
                ));
            }
        }
        descriptor
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct CodexLaborArtifactRef {
    pub kind: String,
    pub uri: String,
    pub digest: String,
}

impl CodexLaborArtifactRef {
    fn as_evidence_ref(&self) -> EvidenceRef {
        EvidenceRef::new(self.kind.clone(), self.uri.clone(), self.digest.clone())
    }
}

fn default_required_artifact_kinds() -> Vec<String> {
    vec!["final_output".to_string(), "transcript".to_string()]
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct CodexLaborApprovalEvent {
    pub kind: String,
    pub item_id: String,
    #[serde(default)]
    pub decision: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub grant_root: Option<String>,
    pub recorded_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct CodexLaborToolInvocation {
    pub request_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub arguments_digest: String,
    #[serde(default)]
    pub response_code: Option<String>,
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub response_message_digest: Option<String>,
    pub recorded_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct CodexLaborProvenanceBundle {
    pub bundle_id: String,
    pub thread_id: String,
    #[serde(default)]
    pub turn_id: Option<String>,
    pub prompt_digest: String,
    #[serde(default)]
    pub selected_model_id: Option<String>,
    #[serde(default)]
    pub selected_skill_names: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub sandbox_policy: Option<String>,
    #[serde(default)]
    pub approval_policy: Option<String>,
    #[serde(default)]
    pub approval_events: Vec<CodexLaborApprovalEvent>,
    #[serde(default)]
    pub tool_invocations: Vec<CodexLaborToolInvocation>,
    #[serde(default)]
    pub produced_artifacts: Vec<CodexLaborArtifactRef>,
    #[serde(default)]
    pub final_output_digest: Option<String>,
    #[serde(default)]
    pub transcript_digest: Option<String>,
}

impl CodexLaborProvenanceBundle {
    pub(crate) fn bundle_digest(&self) -> String {
        let serialized = serde_json::to_string(self)
            .unwrap_or_else(|_| "codex-provenance-bundle-serialization-error".to_string());
        sha256_prefixed_text(serialized.as_str())
    }

    pub(crate) fn set_turn_id(&mut self, turn_id: &str) {
        if self.turn_id.is_none() {
            self.turn_id = Some(turn_id.to_string());
        }
    }

    pub(crate) fn set_selected_skill_names(&mut self, selected_skill_names: Vec<String>) {
        let mut selected_skill_names = selected_skill_names
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        selected_skill_names.sort();
        selected_skill_names.dedup();
        self.selected_skill_names = selected_skill_names;
    }

    pub(crate) fn record_approval_event(&mut self, event: CodexLaborApprovalEvent) {
        self.approval_events.push(event);
        if self.approval_events.len() > 64 {
            let overflow = self.approval_events.len().saturating_sub(64);
            self.approval_events.drain(0..overflow);
        }
    }

    pub(crate) fn record_tool_request(
        &mut self,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        arguments: &str,
        recorded_at_epoch_ms: u64,
    ) {
        if let Some(existing) = self
            .tool_invocations
            .iter_mut()
            .find(|invocation| invocation.request_id == request_id && invocation.call_id == call_id)
        {
            existing.tool_name = tool_name.to_string();
            existing.arguments_digest = sha256_prefixed_text(arguments);
            existing.recorded_at_epoch_ms = recorded_at_epoch_ms;
            return;
        }
        self.tool_invocations.push(CodexLaborToolInvocation {
            request_id: request_id.to_string(),
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments_digest: sha256_prefixed_text(arguments),
            response_code: None,
            success: None,
            response_message_digest: None,
            recorded_at_epoch_ms,
        });
        if self.tool_invocations.len() > 64 {
            let overflow = self.tool_invocations.len().saturating_sub(64);
            self.tool_invocations.drain(0..overflow);
        }
    }

    pub(crate) fn record_tool_result(
        &mut self,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        response_code: &str,
        success: bool,
        response_message: &str,
        recorded_at_epoch_ms: u64,
    ) {
        if let Some(existing) = self
            .tool_invocations
            .iter_mut()
            .find(|invocation| invocation.request_id == request_id && invocation.call_id == call_id)
        {
            existing.tool_name = tool_name.to_string();
            existing.response_code = Some(response_code.to_string());
            existing.success = Some(success);
            existing.response_message_digest = Some(sha256_prefixed_text(response_message));
            existing.recorded_at_epoch_ms = recorded_at_epoch_ms;
            return;
        }
        self.tool_invocations.push(CodexLaborToolInvocation {
            request_id: request_id.to_string(),
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments_digest: sha256_prefixed_text("unknown-tool-arguments"),
            response_code: Some(response_code.to_string()),
            success: Some(success),
            response_message_digest: Some(sha256_prefixed_text(response_message)),
            recorded_at_epoch_ms,
        });
        if self.tool_invocations.len() > 64 {
            let overflow = self.tool_invocations.len().saturating_sub(64);
            self.tool_invocations.drain(0..overflow);
        }
    }

    pub(crate) fn record_output_snapshot(&mut self, work_unit_id: &str, output: &str) {
        let trimmed = output.trim();
        if trimmed.is_empty() {
            return;
        }

        let final_output_digest = sha256_prefixed_text(trimmed);
        self.final_output_digest = Some(final_output_digest.clone());
        self.record_artifact(
            "final_output",
            format!("oa://autopilot/codex/{work_unit_id}/output"),
            final_output_digest,
        );

        let transcript_material = serde_json::json!({
            "thread_id": self.thread_id,
            "turn_id": self.turn_id,
            "prompt_digest": self.prompt_digest,
            "final_output_digest": self.final_output_digest,
        });
        let transcript_material = transcript_material.to_string();
        let transcript_digest = sha256_prefixed_text(transcript_material.as_str());
        self.transcript_digest = Some(transcript_digest.clone());
        self.record_artifact(
            "transcript",
            format!("oa://autopilot/codex/{work_unit_id}/transcript"),
            transcript_digest,
        );
    }

    fn record_artifact(&mut self, kind: impl Into<String>, uri: impl Into<String>, digest: String) {
        let artifact = CodexLaborArtifactRef {
            kind: kind.into(),
            uri: uri.into(),
            digest,
        };
        if self
            .produced_artifacts
            .iter()
            .any(|existing| existing == &artifact)
        {
            return;
        }
        self.produced_artifacts.push(artifact);
        if self.produced_artifacts.len() > 64 {
            let overflow = self.produced_artifacts.len().saturating_sub(64);
            self.produced_artifacts.drain(0..overflow);
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CodexLaborVerifierPath {
    DeterministicOutputGate,
}

impl CodexLaborVerifierPath {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::DeterministicOutputGate => "deterministic_output_gate",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct CodexLaborSubmissionState {
    pub submission: Submission,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    pub verifier_path: CodexLaborVerifierPath,
    pub verifier_id: String,
    pub settlement_ready: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct CodexLaborVerdictState {
    pub verdict: Verdict,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
    pub verifier_path: CodexLaborVerifierPath,
    pub verifier_id: String,
    #[serde(default)]
    pub independence_note: Option<String>,
    #[serde(default)]
    pub correlation_note: Option<String>,
    pub settlement_ready: bool,
    #[serde(default)]
    pub settlement_withheld_reason: Option<String>,
}

impl CodexLaborVerdictState {
    pub(crate) fn outcome_label(&self) -> &'static str {
        match self.verdict.outcome {
            VerdictOutcome::Pass => "pass",
            VerdictOutcome::Fail => "fail",
            VerdictOutcome::Escalated => "escalated",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct CodexLaborVerifierFailure {
    pub code: String,
    pub message: String,
    pub recorded_at_epoch_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct CodexLaborBinding {
    pub work_unit_id: String,
    pub contract_id: String,
    pub idempotency_key: String,
    pub trace: TraceContext,
    pub provenance: CodexLaborProvenanceBundle,
    #[serde(default = "default_required_artifact_kinds")]
    pub required_artifact_kinds: Vec<String>,
    #[serde(default)]
    pub attached_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub incident_evidence_refs: Vec<EvidenceRef>,
    #[serde(default)]
    pub submission: Option<CodexLaborSubmissionState>,
    #[serde(default)]
    pub verdict: Option<CodexLaborVerdictState>,
    #[serde(default)]
    pub verifier_failure: Option<CodexLaborVerifierFailure>,
}

impl CodexLaborBinding {
    pub(crate) fn artifact_scope_root(&self) -> String {
        format!("oa://autopilot/codex/{}/", self.work_unit_id)
    }

    pub(crate) fn record_turn_started(&mut self, turn_id: &str) {
        self.provenance.set_turn_id(turn_id);
    }

    pub(crate) fn set_selected_skill_names(&mut self, selected_skill_names: Vec<String>) {
        self.provenance
            .set_selected_skill_names(selected_skill_names);
    }

    pub(crate) fn record_approval_event(&mut self, event: CodexLaborApprovalEvent) {
        self.provenance.record_approval_event(event);
    }

    pub(crate) fn record_tool_request(
        &mut self,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        arguments: &str,
        recorded_at_epoch_ms: u64,
    ) {
        self.provenance.record_tool_request(
            request_id,
            call_id,
            tool_name,
            arguments,
            recorded_at_epoch_ms,
        );
    }

    pub(crate) fn record_tool_result(
        &mut self,
        request_id: &str,
        call_id: &str,
        tool_name: &str,
        response_code: &str,
        success: bool,
        response_message: &str,
        recorded_at_epoch_ms: u64,
    ) {
        self.provenance.record_tool_result(
            request_id,
            call_id,
            tool_name,
            response_code,
            success,
            response_message,
            recorded_at_epoch_ms,
        );
    }

    pub(crate) fn record_output_snapshot(&mut self, output: &str) {
        self.provenance
            .record_output_snapshot(self.work_unit_id.as_str(), output);
    }

    pub(crate) fn scope_payload(&self) -> serde_json::Value {
        json!({
            "work_unit_id": self.work_unit_id,
            "contract_id": self.contract_id,
            "trace": self.trace,
            "artifact_scope_root": self.artifact_scope_root(),
            "allowed_evidence_uri_prefixes": [self.artifact_scope_root()],
            "required_artifact_kinds": self.required_artifact_kinds,
            "expected_output_refs": {
                "final_output": format!("{}output", self.artifact_scope_root()),
                "transcript": format!("{}transcript", self.artifact_scope_root()),
            },
            "verifier_path": CodexLaborVerifierPath::DeterministicOutputGate.label(),
        })
    }

    pub(crate) fn requirements_payload(&self) -> serde_json::Value {
        json!({
            "work_unit_id": self.work_unit_id,
            "contract_id": self.contract_id,
            "required_artifact_kinds": self.required_artifact_kinds,
            "acceptance_criteria": [
                {
                    "code": "final_output_present",
                    "description": "a final output artifact must be present before verification can complete",
                },
                {
                    "code": "transcript_present",
                    "description": "a transcript artifact must be present before verification can complete",
                },
                {
                    "code": "no_denied_approvals",
                    "description": "approval history must not contain denied command or file-change decisions",
                },
                {
                    "code": "no_failed_tool_invocations",
                    "description": "recorded tool invocations must not include unsuccessful executions",
                }
            ],
            "evidence_gaps": self.required_evidence_gap_objects(),
        })
    }

    pub(crate) fn evidence_payload(&self) -> serde_json::Value {
        json!({
            "work_unit_id": self.work_unit_id,
            "contract_id": self.contract_id,
            "produced": self.provenance.produced_artifacts,
            "attached": self.attached_evidence_refs,
            "incident": self.incident_evidence_refs,
            "evidence_gaps": self.required_evidence_gap_objects(),
            "submission": self.submission,
            "verdict": self.verdict,
            "verifier_failure": self.verifier_failure,
        })
    }

    pub(crate) fn required_evidence_gaps(&self) -> Vec<String> {
        self.required_evidence_gap_objects()
            .into_iter()
            .filter_map(|gap| {
                gap.get("kind")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .collect()
    }

    pub(crate) fn attach_evidence_ref(
        &mut self,
        evidence: EvidenceRef,
        incident: bool,
    ) -> Result<(), String> {
        let kind = evidence.kind.trim();
        if kind.is_empty() {
            return Err("evidence kind must not be empty".to_string());
        }
        let uri = evidence.uri.trim();
        if uri.is_empty() {
            return Err("evidence uri must not be empty".to_string());
        }
        let digest = evidence.digest.trim();
        if digest.is_empty() {
            return Err("evidence digest must not be empty".to_string());
        }
        let scope_root = self.artifact_scope_root();
        if !uri.starts_with(scope_root.as_str()) {
            return Err(format!(
                "evidence uri must stay within contract artifact scope {}",
                scope_root
            ));
        }

        let store = if incident {
            &mut self.incident_evidence_refs
        } else {
            &mut self.attached_evidence_refs
        };
        if store.iter().any(|existing| existing == &evidence) {
            return Ok(());
        }
        store.push(evidence);
        store.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.uri.cmp(&right.uri))
                .then_with(|| left.digest.cmp(&right.digest))
        });
        Ok(())
    }

    pub(crate) fn assemble_submission(
        &mut self,
        created_at_epoch_ms: u64,
    ) -> CodexLaborSubmissionState {
        if let Some(existing) = self.submission.clone() {
            return existing;
        }

        let verifier_path = CodexLaborVerifierPath::DeterministicOutputGate;
        let submission = CodexLaborSubmissionState {
            submission: Submission {
                submission_id: self.submission_id(),
                contract_id: self.contract_id.clone(),
                work_unit_id: self.work_unit_id.clone(),
                created_at_ms: created_at_epoch_ms as i64,
                status: SubmissionStatus::Received,
                output_ref: self.final_output_ref(),
                provenance_digest: Some(self.provenance.bundle_digest()),
                metadata: json!({
                    "thread_id": self.provenance.thread_id.clone(),
                    "turn_id": self.provenance.turn_id.clone(),
                    "selected_model_id": self.provenance.selected_model_id.clone(),
                    "selected_skill_names": self.provenance.selected_skill_names.clone(),
                    "cwd": self.provenance.cwd.clone(),
                    "sandbox_policy": self.provenance.sandbox_policy.clone(),
                    "approval_policy": self.provenance.approval_policy.clone(),
                    "approval_events_count": self.provenance.approval_events.len(),
                    "tool_invocations_count": self.provenance.tool_invocations.len(),
                    "verifier_path": verifier_path.label(),
                }),
            },
            evidence_refs: self.provenance_evidence_refs(),
            verifier_path,
            verifier_id: verifier_id_for_path(verifier_path).to_string(),
            settlement_ready: false,
        };
        self.verifier_failure = None;
        self.submission = Some(submission.clone());
        submission
    }

    pub(crate) fn finalize_verdict(
        &mut self,
        verified_at_epoch_ms: u64,
    ) -> Result<CodexLaborVerdictState, String> {
        if let Some(existing) = self.verdict.clone() {
            return Ok(existing);
        }

        let mut submission_state = self.assemble_submission(verified_at_epoch_ms);
        let Some(output_ref) = submission_state.submission.output_ref.clone() else {
            let message = self.record_verifier_failure(
                "codex_submission_output_missing",
                "codex labor verifier requires a final output reference",
                verified_at_epoch_ms,
            );
            return Err(message);
        };
        let Some(final_output_digest) = self.provenance.final_output_digest.clone() else {
            let message = self.record_verifier_failure(
                "codex_verification_output_digest_missing",
                "codex labor verifier requires a final output digest",
                verified_at_epoch_ms,
            );
            return Err(message);
        };
        let Some(transcript_digest) = self.provenance.transcript_digest.clone() else {
            let message = self.record_verifier_failure(
                "codex_verification_transcript_digest_missing",
                "codex labor verifier requires a transcript digest",
                verified_at_epoch_ms,
            );
            return Err(message);
        };

        let denied_approval = self
            .provenance
            .approval_events
            .iter()
            .find(|event| approval_decision_is_negative(event.decision.as_deref()));
        let failed_tool = self
            .provenance
            .tool_invocations
            .iter()
            .find(|invocation| invocation.success == Some(false));

        let (outcome, reason_code, settlement_ready, settlement_status, settlement_withheld_reason) =
            if let Some(event) = denied_approval {
                (
                    VerdictOutcome::Fail,
                    "codex.verifier.approval_denied",
                    false,
                    SettlementStatus::Disputed,
                    Some(format!("approval denied for item {}", event.item_id)),
                )
            } else if let Some(invocation) = failed_tool {
                (
                    VerdictOutcome::Fail,
                    "codex.verifier.tool_failed",
                    false,
                    SettlementStatus::Disputed,
                    Some(format!(
                        "tool {} reported unsuccessful execution",
                        invocation.tool_name
                    )),
                )
            } else {
                (
                    VerdictOutcome::Pass,
                    "codex.verifier.objective_pass",
                    true,
                    SettlementStatus::Pending,
                    None,
                )
            };

        submission_state.submission.status = if settlement_ready {
            SubmissionStatus::Accepted
        } else {
            SubmissionStatus::Rejected
        };

        let verifier_path = submission_state.verifier_path;
        let verifier_id = submission_state.verifier_id.clone();
        let independence_note =
            Some("deterministic verifier executed after worker completion".to_string());
        let correlation_note = Some(
            "local verifier shares runtime context with the worker and is not a separate model family"
                .to_string(),
        );
        let mut evidence_refs = submission_state.evidence_refs.clone();
        evidence_refs.push(EvidenceRef::new(
            "codex_submission",
            format!(
                "oa://autopilot/codex/{}/submissions/{}",
                self.work_unit_id, submission_state.submission.submission_id
            ),
            sha256_prefixed_text(
                format!("{}:{output_ref}", submission_state.submission.submission_id).as_str(),
            ),
        ));
        evidence_refs.push(EvidenceRef::new(
            "codex_verifier_report",
            format!(
                "oa://autopilot/codex/{}/verifier/{}",
                self.work_unit_id,
                verifier_path.label()
            ),
            sha256_prefixed_text(
                format!(
                    "{}:{}:{}:{}",
                    final_output_digest,
                    transcript_digest,
                    reason_code,
                    verifier_path.label()
                )
                .as_str(),
            ),
        ));

        let verdict = CodexLaborVerdictState {
            verdict: Verdict {
                verdict_id: self.verdict_id(),
                contract_id: self.contract_id.clone(),
                work_unit_id: self.work_unit_id.clone(),
                created_at_ms: verified_at_epoch_ms as i64,
                outcome,
                verification_tier: Some(VerificationTier::TierOObjective),
                settlement_status,
                reason_code: Some(reason_code.to_string()),
                metadata: json!({
                    "submission_id": submission_state.submission.submission_id,
                    "thread_id": self.provenance.thread_id.clone(),
                    "turn_id": self.provenance.turn_id.clone(),
                    "verifier_path": verifier_path.label(),
                    "verifier_id": verifier_id.clone(),
                    "independence_note": independence_note.clone(),
                    "correlation_note": correlation_note.clone(),
                    "output_ref": output_ref,
                    "output_digest": final_output_digest,
                    "transcript_digest": transcript_digest,
                }),
            },
            evidence_refs,
            verifier_path,
            verifier_id,
            independence_note,
            correlation_note,
            settlement_ready,
            settlement_withheld_reason,
        };

        self.verifier_failure = None;
        self.submission = Some(submission_state);
        self.verdict = Some(verdict.clone());
        Ok(verdict)
    }

    pub(crate) fn is_settlement_ready(&self) -> bool {
        self.verdict
            .as_ref()
            .map(|verdict| verdict.settlement_ready)
            .unwrap_or(false)
    }

    fn submission_id(&self) -> String {
        let turn_material = self.provenance.turn_id.as_deref().unwrap_or("pending-turn");
        format!(
            "submission.codex.{}",
            short_hash(
                format!("{}:{}:{turn_material}", self.contract_id, self.work_unit_id).as_str()
            )
        )
    }

    fn verdict_id(&self) -> String {
        let turn_material = self.provenance.turn_id.as_deref().unwrap_or("pending-turn");
        format!(
            "verdict.codex.{}",
            short_hash(
                format!(
                    "{}:{}:{turn_material}",
                    self.contract_id, self.provenance.bundle_id
                )
                .as_str()
            )
        )
    }

    fn final_output_ref(&self) -> Option<String> {
        self.provenance
            .produced_artifacts
            .iter()
            .find(|artifact| artifact.kind == "final_output")
            .map(|artifact| artifact.uri.clone())
    }

    fn provenance_evidence_refs(&self) -> Vec<EvidenceRef> {
        let mut evidence_refs = Vec::new();
        evidence_refs.push(EvidenceRef::new(
            "codex_thread",
            format!("oa://autopilot/codex/threads/{}", self.provenance.thread_id),
            sha256_prefixed_text(self.provenance.thread_id.as_str()),
        ));
        if let Some(turn_id) = self.provenance.turn_id.as_deref() {
            evidence_refs.push(EvidenceRef::new(
                "codex_turn",
                format!("oa://autopilot/codex/turns/{turn_id}"),
                sha256_prefixed_text(turn_id),
            ));
        }
        evidence_refs.push(EvidenceRef::new(
            "codex_provenance_bundle",
            format!(
                "oa://autopilot/codex/{}/bundles/{}",
                self.work_unit_id, self.provenance.bundle_id
            ),
            self.provenance.bundle_digest(),
        ));
        if let Some(selected_model_id) = self.provenance.selected_model_id.as_deref() {
            evidence_refs.push(EvidenceRef::new(
                "codex_model",
                format!("oa://autopilot/codex/models/{selected_model_id}"),
                sha256_prefixed_text(selected_model_id),
            ));
        }
        for artifact in &self.provenance.produced_artifacts {
            evidence_refs.push(artifact.as_evidence_ref());
        }
        evidence_refs.extend(self.attached_evidence_refs.iter().cloned());
        evidence_refs
    }

    fn required_evidence_gap_objects(&self) -> Vec<serde_json::Value> {
        self.required_artifact_kinds
            .iter()
            .filter(|kind| !self.has_evidence_kind(kind.as_str()))
            .map(|kind| {
                json!({
                    "kind": kind,
                    "message": format!("required evidence kind '{}' is missing", kind),
                    "uri_prefix": self.artifact_scope_root(),
                })
            })
            .collect()
    }

    fn has_evidence_kind(&self, kind: &str) -> bool {
        self.provenance
            .produced_artifacts
            .iter()
            .any(|artifact| artifact.kind == kind)
            || self
                .attached_evidence_refs
                .iter()
                .any(|evidence| evidence.kind == kind)
    }

    fn record_verifier_failure(
        &mut self,
        code: &str,
        message: &str,
        recorded_at_epoch_ms: u64,
    ) -> String {
        self.verifier_failure = Some(CodexLaborVerifierFailure {
            code: code.to_string(),
            message: message.to_string(),
            recorded_at_epoch_ms,
        });
        message.to_string()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CodexTurnExecutionRequest {
    pub trigger: CodexRunTrigger,
    pub submitted_at_epoch_ms: u64,
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub model: Option<String>,
}

#[derive(Debug)]
pub(crate) struct CodexTurnExecutionPlan {
    pub classification: CodexRunClassification,
    pub labor_binding: Option<CodexLaborBinding>,
    pub command: crate::codex_lane::CodexLaneCommand,
}

pub(crate) fn orchestrate_codex_turn(request: CodexTurnExecutionRequest) -> CodexTurnExecutionPlan {
    let classification = CodexRunClassification::from_trigger(request.trigger.clone());
    let labor_binding = local_labor_binding(&classification, &request);
    let command = crate::codex_lane::CodexLaneCommand::TurnStart(TurnStartParams {
        thread_id: request.thread_id,
        input: request.input,
        cwd: request.cwd,
        approval_policy: request.approval_policy,
        sandbox_policy: request.sandbox_policy,
        model: request.model,
        effort: None,
        summary: None,
        personality: None,
        output_schema: None,
        collaboration_mode: None,
    });

    CodexTurnExecutionPlan {
        classification,
        labor_binding,
        command,
    }
}

fn local_labor_binding(
    classification: &CodexRunClassification,
    request: &CodexTurnExecutionRequest,
) -> Option<CodexLaborBinding> {
    if !classification.is_economically_meaningful() {
        return None;
    }

    let prompt_digest = prompt_digest_from_input(&request.input);
    let scope_material = classification_scope_material(classification);
    let binding_material = format!(
        "{}:{}:{}:{}",
        request.thread_id, request.submitted_at_epoch_ms, scope_material, prompt_digest
    );
    let binding_digest = sha256_prefixed_text(binding_material.as_str());
    let binding_component = short_hash(binding_digest.as_str());

    let (work_unit_id, contract_id) = match classification {
        CodexRunClassification::PersonalAgent => return None,
        CodexRunClassification::AutonomousGoal { goal_id, .. } => (
            format!(
                "work_unit.codex.goal.{}.{}",
                sanitize_id_component(goal_id),
                binding_component
            ),
            format!(
                "contract.codex.goal.{}.{}",
                sanitize_id_component(goal_id),
                binding_component
            ),
        ),
        CodexRunClassification::LaborMarket {
            work_unit_id,
            contract_id,
        } => (
            work_unit_id.clone(),
            contract_id
                .clone()
                .unwrap_or_else(|| format!("contract.codex.market.{binding_component}")),
        ),
    };

    let selected_skill_names = skill_names_from_input(&request.input);
    let cwd = request
        .cwd
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let sandbox_policy = sandbox_policy_label(request.sandbox_policy.as_ref());
    let approval_policy = approval_policy_label(request.approval_policy.as_ref());
    let idempotency_key = format!("desktop.codex.labor.binding:{binding_component}");

    Some(CodexLaborBinding {
        work_unit_id: work_unit_id.clone(),
        contract_id: contract_id.clone(),
        idempotency_key,
        trace: TraceContext {
            session_id: Some("autopilot.desktop.codex".to_string()),
            trajectory_hash: Some(binding_digest),
            job_hash: Some(prompt_digest.clone()),
            run_id: Some(format!("codex-run.{binding_component}")),
            work_unit_id: Some(work_unit_id.clone()),
            contract_id: Some(contract_id.clone()),
            claim_id: None,
        },
        provenance: CodexLaborProvenanceBundle {
            bundle_id: format!("provenance.codex.{binding_component}"),
            thread_id: request.thread_id.clone(),
            turn_id: None,
            prompt_digest,
            selected_model_id: request.model.clone(),
            selected_skill_names,
            cwd,
            sandbox_policy,
            approval_policy,
            approval_events: Vec::new(),
            tool_invocations: Vec::new(),
            produced_artifacts: Vec::new(),
            final_output_digest: None,
            transcript_digest: None,
        },
        required_artifact_kinds: default_required_artifact_kinds(),
        attached_evidence_refs: Vec::new(),
        incident_evidence_refs: Vec::new(),
        submission: None,
        verdict: None,
        verifier_failure: None,
    })
}

fn classification_scope_material(classification: &CodexRunClassification) -> String {
    match classification {
        CodexRunClassification::PersonalAgent => "personal_agent".to_string(),
        CodexRunClassification::AutonomousGoal { goal_id, .. } => {
            format!("autonomous_goal:{goal_id}")
        }
        CodexRunClassification::LaborMarket {
            work_unit_id,
            contract_id,
        } => format!("labor_market:{work_unit_id}:{contract_id:?}"),
    }
}

fn prompt_digest_from_input(input: &[UserInput]) -> String {
    let prompt = input
        .iter()
        .filter_map(|item| match item {
            UserInput::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    sha256_prefixed_text(prompt.trim())
}

fn skill_names_from_input(input: &[UserInput]) -> Vec<String> {
    let mut names = input
        .iter()
        .filter_map(|item| match item {
            UserInput::Skill { name, .. } => Some(name.trim().to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn approval_policy_label(policy: Option<&AskForApproval>) -> Option<String> {
    policy.map(|policy| match policy {
        AskForApproval::UnlessTrusted => "unless_trusted".to_string(),
        AskForApproval::OnFailure => "on_failure".to_string(),
        AskForApproval::OnRequest => "on_request".to_string(),
        AskForApproval::Reject { .. } => "reject".to_string(),
        AskForApproval::Never => "never".to_string(),
    })
}

fn verifier_id_for_path(path: CodexLaborVerifierPath) -> &'static str {
    match path {
        CodexLaborVerifierPath::DeterministicOutputGate => {
            "autopilot.codex.verifier.deterministic_output_gate.v1"
        }
    }
}

fn approval_decision_is_negative(decision: Option<&str>) -> bool {
    decision.is_some_and(|value| {
        value.eq_ignore_ascii_case("deny")
            || value.eq_ignore_ascii_case("decline")
            || value.eq_ignore_ascii_case("rejected")
            || value.eq_ignore_ascii_case("reject")
    })
}

fn sandbox_policy_label(policy: Option<&SandboxPolicy>) -> Option<String> {
    policy.map(|policy| match policy {
        SandboxPolicy::DangerFullAccess => "danger_full_access".to_string(),
        SandboxPolicy::ReadOnly => "read_only".to_string(),
        SandboxPolicy::ExternalSandbox { .. } => "external_sandbox".to_string(),
        SandboxPolicy::WorkspaceWrite { .. } => "workspace_write".to_string(),
    })
}

fn short_hash(digest: &str) -> String {
    digest
        .strip_prefix("sha256:")
        .unwrap_or(digest)
        .chars()
        .take(16)
        .collect()
}

fn sanitize_id_component(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_client::{AskForApproval, SandboxPolicy, UserInput};

    use super::{
        CodexLaborApprovalEvent, CodexRunClassification, CodexRunTrigger,
        CodexTurnExecutionRequest, orchestrate_codex_turn,
    };

    #[test]
    fn personal_agent_runs_remain_local_only() {
        let classification = CodexRunClassification::from_trigger(CodexRunTrigger::PersonalAgent);

        assert_eq!(classification.label(), "personal_agent");
        assert!(!classification.is_economically_meaningful());
        assert!(!classification.is_labor_market_bound());
    }

    #[test]
    fn autonomous_goal_runs_are_economic_but_not_yet_labor_market_bound() {
        let classification =
            CodexRunClassification::from_trigger(CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            });

        assert_eq!(classification.label(), "autonomous_goal");
        assert!(classification.is_economically_meaningful());
        assert!(!classification.is_labor_market_bound());
    }

    #[test]
    fn labor_market_runs_are_explicitly_labor_bound() {
        let classification = CodexRunClassification::from_trigger(CodexRunTrigger::LaborMarket {
            work_unit_id: "wu-1".to_string(),
            contract_id: Some("contract-1".to_string()),
        });

        assert_eq!(classification.label(), "labor_market");
        assert!(classification.is_economically_meaningful());
        assert!(classification.is_labor_market_bound());
    }

    #[test]
    fn orchestration_preserves_turn_start_params_and_allocates_binding() {
        let request = CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-42".to_string(),
                goal_title: "Close accounting loop".to_string(),
            },
            submitted_at_epoch_ms: 1_700_000_000_000,
            thread_id: "thread-123".to_string(),
            input: vec![
                UserInput::Text {
                    text: "close the books".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "l402".to_string(),
                    path: PathBuf::from("/repo/skills/l402/SKILL.md"),
                },
            ],
            cwd: Some(PathBuf::from("/tmp/work")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: Some("gpt-5.2-codex".to_string()),
        };

        let plan = orchestrate_codex_turn(request);

        assert_eq!(
            plan.classification,
            CodexRunClassification::AutonomousGoal {
                goal_id: "goal-42".to_string(),
                goal_title: "Close accounting loop".to_string(),
            }
        );
        let binding = plan
            .labor_binding
            .clone()
            .expect("goal runs should allocate a labor binding");
        assert!(
            binding
                .work_unit_id
                .starts_with("work_unit.codex.goal.goal-42.")
        );
        assert!(
            binding
                .contract_id
                .starts_with("contract.codex.goal.goal-42.")
        );
        assert_eq!(
            binding.provenance.selected_skill_names,
            vec!["l402".to_string()]
        );
        assert_eq!(
            binding.provenance.selected_model_id.as_deref(),
            Some("gpt-5.2-codex")
        );
        assert_eq!(
            binding.provenance.sandbox_policy.as_deref(),
            Some("danger_full_access")
        );
        let crate::codex_lane::CodexLaneCommand::TurnStart(params) = plan.command else {
            panic!("expected TurnStart command");
        };
        assert_eq!(params.thread_id, "thread-123");
        assert_eq!(params.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(params.cwd, Some(PathBuf::from("/tmp/work")));
        assert!(matches!(
            params.approval_policy,
            Some(AskForApproval::Never)
        ));
        assert_eq!(
            params.sandbox_policy,
            Some(codex_client::SandboxPolicy::DangerFullAccess)
        );
        assert_eq!(params.input.len(), 2);
    }

    #[test]
    fn personal_agent_runs_do_not_allocate_labor_binding() {
        let plan = orchestrate_codex_turn(CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::PersonalAgent,
            submitted_at_epoch_ms: 1_700_000_000_000,
            thread_id: "thread-personal".to_string(),
            input: vec![UserInput::Text {
                text: "draft a reply".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: None,
        });

        assert!(plan.labor_binding.is_none());
    }

    #[test]
    fn labor_binding_generation_is_stable_for_equivalent_requests() {
        let request = CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-stable".to_string(),
                goal_title: "Keep earning".to_string(),
            },
            submitted_at_epoch_ms: 1_700_000_000_555,
            thread_id: "thread-stable".to_string(),
            input: vec![
                UserInput::Text {
                    text: "earn sats".to_string(),
                    text_elements: Vec::new(),
                },
                UserInput::Skill {
                    name: "blink".to_string(),
                    path: PathBuf::from("/repo/skills/blink/SKILL.md"),
                },
            ],
            cwd: Some(PathBuf::from("/repo")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::WorkspaceWrite {
                writable_roots: vec!["/repo".to_string()],
                network_access: true,
                exclude_tmpdir_env_var: false,
                exclude_slash_tmp: false,
            }),
            model: Some("gpt-5.2-codex".to_string()),
        };

        let left = orchestrate_codex_turn(request.clone());
        let right = orchestrate_codex_turn(request);

        assert_eq!(left.labor_binding, right.labor_binding);
        let left_digest = left
            .labor_binding
            .as_ref()
            .expect("binding should exist")
            .provenance
            .bundle_digest();
        let right_digest = right
            .labor_binding
            .as_ref()
            .expect("binding should exist")
            .provenance
            .bundle_digest();
        assert_eq!(left_digest, right_digest);
    }

    #[test]
    fn labor_binding_records_provenance_updates_deterministically() {
        let mut binding = orchestrate_codex_turn(CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-prov".to_string(),
                goal_title: "Provenance".to_string(),
            },
            submitted_at_epoch_ms: 1_700_000_001_000,
            thread_id: "thread-prov".to_string(),
            input: vec![UserInput::Text {
                text: "run the prover".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(PathBuf::from("/repo")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: Some("gpt-5.2-codex".to_string()),
        })
        .labor_binding
        .expect("goal run should bind labor");

        binding.record_turn_started("turn-prov");
        binding.record_approval_event(CodexLaborApprovalEvent {
            kind: "command_request".to_string(),
            item_id: "item-1".to_string(),
            decision: None,
            reason: Some("needs shell access".to_string()),
            command: Some("git status".to_string()),
            cwd: Some("/repo".to_string()),
            grant_root: None,
            recorded_at_epoch_ms: 1_700_000_001_010,
        });
        binding.record_tool_request(
            "request-1",
            "call-1",
            "openagents.files.read",
            "{\"path\":\"README.md\"}",
            1_700_000_001_020,
        );
        binding.record_tool_result(
            "request-1",
            "call-1",
            "openagents.files.read",
            "OK",
            true,
            "read completed",
            1_700_000_001_030,
        );
        binding.record_output_snapshot("final answer");

        assert_eq!(binding.provenance.turn_id.as_deref(), Some("turn-prov"));
        assert_eq!(binding.provenance.approval_events.len(), 1);
        assert_eq!(binding.provenance.tool_invocations.len(), 1);
        assert_eq!(
            binding.provenance.tool_invocations[0]
                .response_code
                .as_deref(),
            Some("OK")
        );
        assert!(binding.provenance.final_output_digest.is_some());
        assert!(binding.provenance.transcript_digest.is_some());
        assert_eq!(binding.provenance.produced_artifacts.len(), 2);
        assert!(
            binding
                .provenance
                .produced_artifacts
                .iter()
                .any(|artifact| artifact.kind == "final_output")
        );
        assert!(
            binding
                .provenance
                .produced_artifacts
                .iter()
                .any(|artifact| artifact.kind == "transcript")
        );
    }
}
