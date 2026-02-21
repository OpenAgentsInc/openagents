use serde_json::Value;
use thiserror::Error;

use crate::types::RunStatus;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunTransition {
    Start,
    RequestCancel,
    FinishSucceeded,
    FinishFailed,
    FinishCanceled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionOutcome {
    pub next_status: RunStatus,
    pub terminal: bool,
    pub idempotent: bool,
}

#[derive(Debug, Error)]
pub enum RunStateMachineError {
    #[error("run.finished requires payload.status")]
    MissingTerminalStatus,
    #[error("invalid run.finished payload.status: {0}")]
    InvalidTerminalStatus(String),
    #[error("invalid run transition: {from:?} -> {transition:?}")]
    InvalidTransition {
        from: RunStatus,
        transition: RunTransition,
    },
}

pub fn transition_for_event(
    event_type: &str,
    payload: &Value,
) -> Result<Option<RunTransition>, RunStateMachineError> {
    match event_type {
        "run.started" => Ok(Some(RunTransition::Start)),
        "run.cancel_requested" => Ok(Some(RunTransition::RequestCancel)),
        "run.finished" => {
            let status = payload
                .get("status")
                .and_then(Value::as_str)
                .ok_or(RunStateMachineError::MissingTerminalStatus)?;
            match status {
                "succeeded" => Ok(Some(RunTransition::FinishSucceeded)),
                "failed" => Ok(Some(RunTransition::FinishFailed)),
                "canceled" | "cancelled" => Ok(Some(RunTransition::FinishCanceled)),
                other => Err(RunStateMachineError::InvalidTerminalStatus(
                    other.to_string(),
                )),
            }
        }
        _ => Ok(None),
    }
}

pub fn apply_transition(
    current: &RunStatus,
    transition: &RunTransition,
) -> Result<TransitionOutcome, RunStateMachineError> {
    let outcome = match (current, transition) {
        (RunStatus::Created, RunTransition::Start) => TransitionOutcome {
            next_status: RunStatus::Running,
            terminal: false,
            idempotent: false,
        },
        (RunStatus::Running, RunTransition::Start) => TransitionOutcome {
            next_status: RunStatus::Running,
            terminal: false,
            idempotent: true,
        },
        (RunStatus::Created, RunTransition::RequestCancel)
        | (RunStatus::Running, RunTransition::RequestCancel) => TransitionOutcome {
            next_status: RunStatus::Canceling,
            terminal: false,
            idempotent: false,
        },
        (RunStatus::Canceling, RunTransition::RequestCancel) => TransitionOutcome {
            next_status: RunStatus::Canceling,
            terminal: false,
            idempotent: true,
        },
        (RunStatus::Created, RunTransition::FinishSucceeded)
        | (RunStatus::Running, RunTransition::FinishSucceeded) => TransitionOutcome {
            next_status: RunStatus::Succeeded,
            terminal: true,
            idempotent: false,
        },
        (RunStatus::Succeeded, RunTransition::FinishSucceeded) => TransitionOutcome {
            next_status: RunStatus::Succeeded,
            terminal: true,
            idempotent: true,
        },
        (RunStatus::Created, RunTransition::FinishFailed)
        | (RunStatus::Running, RunTransition::FinishFailed)
        | (RunStatus::Canceling, RunTransition::FinishFailed) => TransitionOutcome {
            next_status: RunStatus::Failed,
            terminal: true,
            idempotent: false,
        },
        (RunStatus::Failed, RunTransition::FinishFailed) => TransitionOutcome {
            next_status: RunStatus::Failed,
            terminal: true,
            idempotent: true,
        },
        (RunStatus::Created, RunTransition::FinishCanceled)
        | (RunStatus::Running, RunTransition::FinishCanceled)
        | (RunStatus::Canceling, RunTransition::FinishCanceled) => TransitionOutcome {
            next_status: RunStatus::Canceled,
            terminal: true,
            idempotent: false,
        },
        (RunStatus::Canceled, RunTransition::FinishCanceled) => TransitionOutcome {
            next_status: RunStatus::Canceled,
            terminal: true,
            idempotent: true,
        },
        _ => {
            return Err(RunStateMachineError::InvalidTransition {
                from: current.clone(),
                transition: transition.clone(),
            });
        }
    };
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use anyhow::{Result, anyhow};
    use serde_json::json;

    use super::{RunStateMachineError, RunTransition, apply_transition, transition_for_event};
    use crate::types::RunStatus;

    #[test]
    fn transition_for_event_maps_terminal_status() -> Result<()> {
        let transition = transition_for_event("run.finished", &json!({"status": "succeeded"}))?
            .ok_or_else(|| anyhow!("missing transition"))?;
        assert_eq!(transition, RunTransition::FinishSucceeded);
        Ok(())
    }

    #[test]
    fn transition_for_event_rejects_invalid_terminal_status() -> Result<()> {
        let result = transition_for_event("run.finished", &json!({"status": "unknown"}));
        if !matches!(
            result,
            Err(RunStateMachineError::InvalidTerminalStatus(status)) if status == "unknown"
        ) {
            return Err(anyhow!("expected invalid terminal status error"));
        }
        Ok(())
    }

    #[test]
    fn transition_matrix_enforces_terminal_invariants() -> Result<()> {
        let ok = apply_transition(&RunStatus::Running, &RunTransition::FinishFailed)?;
        assert_eq!(ok.next_status, RunStatus::Failed);
        assert!(ok.terminal);

        let invalid = apply_transition(&RunStatus::Succeeded, &RunTransition::RequestCancel);
        if invalid.is_ok() {
            return Err(anyhow!("expected invalid transition from succeeded"));
        }
        Ok(())
    }

    #[test]
    fn transition_matrix_exhaustive_rules() {
        let statuses = [
            RunStatus::Created,
            RunStatus::Running,
            RunStatus::Canceling,
            RunStatus::Canceled,
            RunStatus::Succeeded,
            RunStatus::Failed,
        ];
        let transitions = [
            RunTransition::Start,
            RunTransition::RequestCancel,
            RunTransition::FinishSucceeded,
            RunTransition::FinishFailed,
            RunTransition::FinishCanceled,
        ];

        let allowed: HashSet<(RunStatus, RunTransition)> = HashSet::from([
            (RunStatus::Created, RunTransition::Start),
            (RunStatus::Running, RunTransition::Start),
            (RunStatus::Created, RunTransition::RequestCancel),
            (RunStatus::Running, RunTransition::RequestCancel),
            (RunStatus::Canceling, RunTransition::RequestCancel),
            (RunStatus::Created, RunTransition::FinishSucceeded),
            (RunStatus::Running, RunTransition::FinishSucceeded),
            (RunStatus::Succeeded, RunTransition::FinishSucceeded),
            (RunStatus::Created, RunTransition::FinishFailed),
            (RunStatus::Running, RunTransition::FinishFailed),
            (RunStatus::Canceling, RunTransition::FinishFailed),
            (RunStatus::Failed, RunTransition::FinishFailed),
            (RunStatus::Created, RunTransition::FinishCanceled),
            (RunStatus::Running, RunTransition::FinishCanceled),
            (RunStatus::Canceling, RunTransition::FinishCanceled),
            (RunStatus::Canceled, RunTransition::FinishCanceled),
        ]);

        for status in statuses {
            for transition in transitions.clone() {
                let result = apply_transition(&status, &transition);
                let is_allowed = allowed.contains(&(status.clone(), transition.clone()));
                assert_eq!(
                    result.is_ok(),
                    is_allowed,
                    "transition mismatch for status={status:?}, transition={transition:?}"
                );
            }
        }
    }
}
