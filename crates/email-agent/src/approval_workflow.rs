use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApprovalMode {
    ManualOnly,
    AutoApproveSafe,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DraftApprovalStatus {
    Pending,
    Approved,
    Rejected,
    NeedsEdits,
}

impl DraftApprovalStatus {
    const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::NeedsEdits => "needs_edits",
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApprovalDecisionAction {
    Approve,
    Reject,
    RequestEdits,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ApprovalPolicyPath {
    Manual { actor: String },
    AutoPolicy { policy_id: String },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DraftApprovalItem {
    pub draft_id: String,
    pub queued_at_unix: u64,
    pub status: DraftApprovalStatus,
    pub last_decision_id: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ApprovalDecisionRecord {
    pub decision_id: String,
    pub draft_id: String,
    pub action: ApprovalDecisionAction,
    pub actor: String,
    pub decided_at_unix: u64,
    pub reason: Option<String>,
    pub policy_path: ApprovalPolicyPath,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum QueueControlAction {
    PauseQueue,
    ResumeQueue,
    EngageKillSwitch,
    DisengageKillSwitch,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct QueueControlEvent {
    pub event_id: String,
    pub actor: String,
    pub at_unix: u64,
    pub action: QueueControlAction,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ApprovalWorkflowState {
    pub mode: ApprovalMode,
    pub queue_paused: bool,
    pub kill_switch_engaged: bool,
    pub drafts: BTreeMap<String, DraftApprovalItem>,
    pub decision_log: Vec<ApprovalDecisionRecord>,
    pub control_events: Vec<QueueControlEvent>,
}

impl Default for ApprovalWorkflowState {
    fn default() -> Self {
        Self {
            mode: ApprovalMode::ManualOnly,
            queue_paused: false,
            kill_switch_engaged: false,
            drafts: BTreeMap::new(),
            decision_log: Vec::new(),
            control_events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DraftEnqueueRequest {
    pub draft_id: String,
    pub queued_at_unix: u64,
    pub auto_policy_id: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ApprovalDecisionInput {
    pub draft_id: String,
    pub action: ApprovalDecisionAction,
    pub actor: String,
    pub decided_at_unix: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendAuthorization {
    pub draft_id: String,
    pub decision_id: String,
    pub authorized_at_unix: u64,
    pub policy_path: ApprovalPolicyPath,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum ApprovalWorkflowError {
    #[error("invalid workflow input: {0}")]
    InvalidInput(String),
    #[error("draft not found: {0}")]
    DraftNotFound(String),
    #[error("duplicate draft id: {0}")]
    DuplicateDraft(String),
    #[error("approval queue is paused")]
    QueuePaused,
    #[error("approval kill switch is engaged")]
    KillSwitchEngaged,
    #[error("draft not authorized for send: {0}")]
    Unauthorized(String),
}

pub fn enqueue_draft_for_approval(
    state: &mut ApprovalWorkflowState,
    request: DraftEnqueueRequest,
) -> Result<DraftApprovalItem, ApprovalWorkflowError> {
    if request.draft_id.trim().is_empty() {
        return Err(ApprovalWorkflowError::InvalidInput(
            "draft_id must not be empty".to_string(),
        ));
    }
    if state.drafts.contains_key(request.draft_id.as_str()) {
        return Err(ApprovalWorkflowError::DuplicateDraft(request.draft_id));
    }

    let mut item = DraftApprovalItem {
        draft_id: request.draft_id.clone(),
        queued_at_unix: request.queued_at_unix,
        status: DraftApprovalStatus::Pending,
        last_decision_id: None,
    };

    if state.mode == ApprovalMode::AutoApproveSafe
        && !state.queue_paused
        && !state.kill_switch_engaged
        && request.auto_policy_id.as_deref().is_some_and(|policy| !policy.trim().is_empty())
    {
        let policy_id = request.auto_policy_id.clone().unwrap_or_default();
        let decision = ApprovalDecisionRecord {
            decision_id: next_decision_id(state),
            draft_id: request.draft_id.clone(),
            action: ApprovalDecisionAction::Approve,
            actor: "policy:auto".to_string(),
            decided_at_unix: request.queued_at_unix,
            reason: Some(format!("auto-approved by policy {policy_id}")),
            policy_path: ApprovalPolicyPath::AutoPolicy { policy_id },
        };
        item.status = DraftApprovalStatus::Approved;
        item.last_decision_id = Some(decision.decision_id.clone());
        state.decision_log.push(decision);
    }

    state.drafts.insert(request.draft_id, item.clone());
    Ok(item)
}

pub fn record_approval_decision(
    state: &mut ApprovalWorkflowState,
    input: ApprovalDecisionInput,
) -> Result<ApprovalDecisionRecord, ApprovalWorkflowError> {
    if input.actor.trim().is_empty() {
        return Err(ApprovalWorkflowError::InvalidInput(
            "actor must not be empty".to_string(),
        ));
    }
    if input.draft_id.trim().is_empty() {
        return Err(ApprovalWorkflowError::InvalidInput(
            "draft_id must not be empty".to_string(),
        ));
    }
    if matches!(
        input.action,
        ApprovalDecisionAction::Reject | ApprovalDecisionAction::RequestEdits
    ) && input.reason.as_deref().is_none_or(|reason| reason.trim().is_empty())
    {
        return Err(ApprovalWorkflowError::InvalidInput(
            "reject/request-edits decisions require a reason".to_string(),
        ));
    }

    let decision_id = next_decision_id(state);
    let decision = ApprovalDecisionRecord {
        decision_id,
        draft_id: input.draft_id,
        action: input.action,
        actor: input.actor.clone(),
        decided_at_unix: input.decided_at_unix,
        reason: trim_optional(input.reason),
        policy_path: ApprovalPolicyPath::Manual { actor: input.actor },
    };

    let draft = state
        .drafts
        .get_mut(decision.draft_id.as_str())
        .ok_or_else(|| ApprovalWorkflowError::DraftNotFound(decision.draft_id.clone()))?;
    draft.status = match decision.action {
        ApprovalDecisionAction::Approve => DraftApprovalStatus::Approved,
        ApprovalDecisionAction::Reject => DraftApprovalStatus::Rejected,
        ApprovalDecisionAction::RequestEdits => DraftApprovalStatus::NeedsEdits,
    };
    draft.last_decision_id = Some(decision.decision_id.clone());
    state.decision_log.push(decision.clone());
    Ok(decision)
}

pub fn set_approval_queue_paused(
    state: &mut ApprovalWorkflowState,
    paused: bool,
    actor: &str,
    at_unix: u64,
    reason: Option<&str>,
) -> Result<QueueControlEvent, ApprovalWorkflowError> {
    if actor.trim().is_empty() {
        return Err(ApprovalWorkflowError::InvalidInput(
            "actor must not be empty".to_string(),
        ));
    }

    state.queue_paused = paused;
    let event = QueueControlEvent {
        event_id: next_control_event_id(state),
        actor: actor.to_string(),
        at_unix,
        action: if paused {
            QueueControlAction::PauseQueue
        } else {
            QueueControlAction::ResumeQueue
        },
        reason: trim_optional(reason.map(ToString::to_string)),
    };
    state.control_events.push(event.clone());
    Ok(event)
}

pub fn set_approval_kill_switch(
    state: &mut ApprovalWorkflowState,
    engaged: bool,
    actor: &str,
    at_unix: u64,
    reason: Option<&str>,
) -> Result<QueueControlEvent, ApprovalWorkflowError> {
    if actor.trim().is_empty() {
        return Err(ApprovalWorkflowError::InvalidInput(
            "actor must not be empty".to_string(),
        ));
    }

    state.kill_switch_engaged = engaged;
    let event = QueueControlEvent {
        event_id: next_control_event_id(state),
        actor: actor.to_string(),
        at_unix,
        action: if engaged {
            QueueControlAction::EngageKillSwitch
        } else {
            QueueControlAction::DisengageKillSwitch
        },
        reason: trim_optional(reason.map(ToString::to_string)),
    };
    state.control_events.push(event.clone());
    Ok(event)
}

pub fn authorize_draft_send(
    state: &ApprovalWorkflowState,
    draft_id: &str,
    authorized_at_unix: u64,
) -> Result<SendAuthorization, ApprovalWorkflowError> {
    if state.kill_switch_engaged {
        return Err(ApprovalWorkflowError::KillSwitchEngaged);
    }
    if state.queue_paused {
        return Err(ApprovalWorkflowError::QueuePaused);
    }

    let draft = state
        .drafts
        .get(draft_id)
        .ok_or_else(|| ApprovalWorkflowError::DraftNotFound(draft_id.to_string()))?;
    if draft.status != DraftApprovalStatus::Approved {
        return Err(ApprovalWorkflowError::Unauthorized(format!(
            "draft status is {}",
            draft.status.label()
        )));
    }

    let decision_id = draft
        .last_decision_id
        .as_deref()
        .ok_or_else(|| {
            ApprovalWorkflowError::Unauthorized(
                "approved draft missing decision log entry".to_string(),
            )
        })?
        .to_string();
    let decision = state
        .decision_log
        .iter()
        .find(|entry| entry.decision_id == decision_id)
        .ok_or_else(|| {
            ApprovalWorkflowError::Unauthorized(format!(
                "missing decision log for decision id {decision_id}"
            ))
        })?;
    if decision.action != ApprovalDecisionAction::Approve {
        return Err(ApprovalWorkflowError::Unauthorized(format!(
            "last decision is not approve ({:?})",
            decision.action
        )));
    }

    Ok(SendAuthorization {
        draft_id: draft_id.to_string(),
        decision_id: decision.decision_id.clone(),
        authorized_at_unix,
        policy_path: decision.policy_path.clone(),
    })
}

fn next_decision_id(state: &ApprovalWorkflowState) -> String {
    format!("decision-{:04}", state.decision_log.len().saturating_add(1))
}

fn next_control_event_id(state: &ApprovalWorkflowState) -> String {
    format!(
        "queue-control-{:04}",
        state.control_events.len().saturating_add(1)
    )
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.map(|inner| inner.trim().to_string()).filter(|inner| !inner.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        ApprovalDecisionAction, ApprovalDecisionInput, ApprovalMode, ApprovalPolicyPath,
        ApprovalWorkflowError, ApprovalWorkflowState, DraftApprovalStatus, DraftEnqueueRequest,
        enqueue_draft_for_approval, record_approval_decision, set_approval_kill_switch,
        set_approval_queue_paused,
    };
    use crate::authorize_draft_send;

    #[test]
    fn manual_approval_records_actor_and_allows_send() {
        let mut state = ApprovalWorkflowState::default();
        enqueue_draft_for_approval(
            &mut state,
            DraftEnqueueRequest {
                draft_id: "draft-1".to_string(),
                queued_at_unix: 100,
                auto_policy_id: None,
            },
        )
        .expect("enqueue should succeed");

        let decision = record_approval_decision(
            &mut state,
            ApprovalDecisionInput {
                draft_id: "draft-1".to_string(),
                action: ApprovalDecisionAction::Approve,
                actor: "operator-1".to_string(),
                decided_at_unix: 105,
                reason: Some("ready to send".to_string()),
            },
        )
        .expect("manual approval should succeed");

        assert_eq!(decision.actor, "operator-1");
        assert_eq!(decision.decided_at_unix, 105);
        let authorization =
            authorize_draft_send(&state, "draft-1", 106).expect("send should be authorized");
        assert_eq!(authorization.decision_id, decision.decision_id);
        assert_eq!(
            authorization.policy_path,
            ApprovalPolicyPath::Manual {
                actor: "operator-1".to_string()
            }
        );
    }

    #[test]
    fn rejection_requires_reason_and_blocks_send() {
        let mut state = ApprovalWorkflowState::default();
        enqueue_draft_for_approval(
            &mut state,
            DraftEnqueueRequest {
                draft_id: "draft-2".to_string(),
                queued_at_unix: 100,
                auto_policy_id: None,
            },
        )
        .expect("enqueue should succeed");

        let error = record_approval_decision(
            &mut state,
            ApprovalDecisionInput {
                draft_id: "draft-2".to_string(),
                action: ApprovalDecisionAction::Reject,
                actor: "operator-2".to_string(),
                decided_at_unix: 110,
                reason: None,
            },
        )
        .expect_err("reject without reason should fail");
        assert_eq!(
            error,
            ApprovalWorkflowError::InvalidInput(
                "reject/request-edits decisions require a reason".to_string()
            )
        );

        record_approval_decision(
            &mut state,
            ApprovalDecisionInput {
                draft_id: "draft-2".to_string(),
                action: ApprovalDecisionAction::Reject,
                actor: "operator-2".to_string(),
                decided_at_unix: 111,
                reason: Some("policy mismatch".to_string()),
            },
        )
        .expect("reject with reason should succeed");

        assert_eq!(state.drafts["draft-2"].status, DraftApprovalStatus::Rejected);
        let send_error = authorize_draft_send(&state, "draft-2", 112)
            .expect_err("rejected draft should not be authorized");
        assert_eq!(
            send_error,
            ApprovalWorkflowError::Unauthorized("draft status is rejected".to_string())
        );
    }

    #[test]
    fn auto_approve_policy_path_is_auditable() {
        let mut state = ApprovalWorkflowState {
            mode: ApprovalMode::AutoApproveSafe,
            ..ApprovalWorkflowState::default()
        };

        let draft = enqueue_draft_for_approval(
            &mut state,
            DraftEnqueueRequest {
                draft_id: "draft-3".to_string(),
                queued_at_unix: 200,
                auto_policy_id: Some("safe-policy-v1".to_string()),
            },
        )
        .expect("auto-approval enqueue should succeed");

        assert_eq!(draft.status, DraftApprovalStatus::Approved);
        let authorization =
            authorize_draft_send(&state, "draft-3", 201).expect("auto-approved send allowed");
        assert_eq!(
            authorization.policy_path,
            ApprovalPolicyPath::AutoPolicy {
                policy_id: "safe-policy-v1".to_string()
            }
        );
    }

    #[test]
    fn queue_pause_and_kill_switch_block_send_authorization() {
        let mut state = ApprovalWorkflowState::default();
        enqueue_draft_for_approval(
            &mut state,
            DraftEnqueueRequest {
                draft_id: "draft-4".to_string(),
                queued_at_unix: 300,
                auto_policy_id: None,
            },
        )
        .expect("enqueue should succeed");
        record_approval_decision(
            &mut state,
            ApprovalDecisionInput {
                draft_id: "draft-4".to_string(),
                action: ApprovalDecisionAction::Approve,
                actor: "operator-4".to_string(),
                decided_at_unix: 301,
                reason: Some("validated".to_string()),
            },
        )
        .expect("approval should succeed");

        set_approval_queue_paused(
            &mut state,
            true,
            "operator-4",
            302,
            Some("incident response"),
        )
        .expect("pause should succeed");
        let paused = super::authorize_draft_send(&state, "draft-4", 303)
            .expect_err("paused queue should block send");
        assert_eq!(paused, ApprovalWorkflowError::QueuePaused);

        set_approval_queue_paused(&mut state, false, "operator-4", 304, Some("incident cleared"))
            .expect("resume should succeed");
        set_approval_kill_switch(&mut state, true, "operator-4", 305, Some("manual stop"))
            .expect("kill switch should succeed");
        let blocked = super::authorize_draft_send(&state, "draft-4", 306)
            .expect_err("kill switch should block send");
        assert_eq!(blocked, ApprovalWorkflowError::KillSwitchEngaged);
    }
}
