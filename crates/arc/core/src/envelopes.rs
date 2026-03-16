use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::schema::{
    ARC_CORE_SCHEMA_VERSION, ArcGrid, ArcTaskId, ContractSerializationError, canonical_json_string,
    canonical_sha256_hex,
};

/// What belongs in execution envelopes and what must stay out of it.
pub const EXECUTION_ENVELOPE_BOUNDARY_SUMMARY: &str = "Own shared static and interactive budget, refusal, and solve-result envelopes for ARC crates. Do not absorb benchmark scoring policy, engine step transitions, client sessions, or solver branch internals.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolveBudget {
    pub max_attempts: u32,
    pub max_steps: u32,
    pub max_runtime_millis: u64,
}

impl SolveBudget {
    pub fn new(
        max_attempts: u32,
        max_steps: u32,
        max_runtime_millis: u64,
    ) -> Result<Self, SolveBudgetError> {
        if max_attempts == 0 {
            return Err(SolveBudgetError::ZeroAttempts);
        }
        if max_steps == 0 {
            return Err(SolveBudgetError::ZeroSteps);
        }
        if max_runtime_millis == 0 {
            return Err(SolveBudgetError::ZeroRuntimeMillis);
        }

        Ok(Self {
            max_attempts,
            max_steps,
            max_runtime_millis,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SolveBudgetError {
    #[error("ARC solve budgets must allow at least one attempt")]
    ZeroAttempts,
    #[error("ARC solve budgets must allow at least one step")]
    ZeroSteps,
    #[error("ARC solve budgets must allow non-zero runtime")]
    ZeroRuntimeMillis,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcRefusalCode {
    UnsupportedTask,
    InvalidTaskContract,
    BudgetExhausted,
    CompetitionModeRestricted,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraceLocator(String);

impl TraceLocator {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn new(raw: impl Into<String>) -> Result<Self, TraceLocatorError> {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(TraceLocatorError::Empty);
        }
        if trimmed.chars().any(char::is_whitespace) {
            return Err(TraceLocatorError::ContainsWhitespace(trimmed.to_owned()));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveBudget {
    pub max_actions: u32,
}

impl ArcInteractiveBudget {
    pub fn new(max_actions: u32) -> Result<Self, ArcInteractiveBudgetError> {
        if max_actions == 0 {
            return Err(ArcInteractiveBudgetError::ZeroMaxActions);
        }
        Ok(Self { max_actions })
    }

    pub fn state(
        self,
        actions_taken: u32,
    ) -> Result<ArcInteractiveBudgetState, ArcInteractiveBudgetError> {
        if actions_taken > self.max_actions {
            return Err(ArcInteractiveBudgetError::ActionsExceedBudget {
                actions_taken,
                max_actions: self.max_actions,
            });
        }
        Ok(ArcInteractiveBudgetState {
            max_actions: self.max_actions,
            actions_taken,
            remaining_actions: self.max_actions.saturating_sub(actions_taken),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveBudgetState {
    pub max_actions: u32,
    pub actions_taken: u32,
    pub remaining_actions: u32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveBudgetError {
    #[error("interactive ARC runs must allow at least one counted action")]
    ZeroMaxActions,
    #[error(
        "interactive ARC budget actions_taken {actions_taken} exceeds max_actions {max_actions}"
    )]
    ActionsExceedBudget {
        actions_taken: u32,
        max_actions: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveResetKind {
    FullGame,
    LevelOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveRefusalCode {
    InvalidAction,
    BudgetExhausted,
    TerminalState,
    ClosedScorecard,
    PolicyRefusal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveRefusal {
    pub code: ArcInteractiveRefusalCode,
    pub step_index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<crate::schema::ArcAction>,
    pub detail: String,
}

impl ArcInteractiveRefusal {
    pub fn new(
        code: ArcInteractiveRefusalCode,
        step_index: u32,
        action: Option<crate::schema::ArcAction>,
        detail: impl Into<String>,
    ) -> Result<Self, ArcInteractiveRefusalError> {
        let detail = detail.into();
        let trimmed = detail.trim();
        if trimmed.is_empty() {
            return Err(ArcInteractiveRefusalError::EmptyDetail);
        }
        Ok(Self {
            code,
            step_index,
            action,
            detail: trimmed.to_owned(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveRefusalError {
    #[error("interactive ARC refusal detail must not be empty")]
    EmptyDetail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcInteractiveActionResult {
    Executed {
        game_state: crate::schema::ArcGameState,
        levels_completed: u16,
        win_levels: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reset: Option<ArcInteractiveResetKind>,
        terminal: bool,
    },
    Refused {
        refusal: ArcInteractiveRefusal,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveTurnResult {
    pub step_index: u32,
    pub requested_action: crate::schema::ArcAction,
    pub budget: ArcInteractiveBudgetState,
    pub result: ArcInteractiveActionResult,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcInteractiveExecutionOutcome {
    Completed {
        final_state: crate::schema::ArcGameState,
        budget: ArcInteractiveBudgetState,
    },
    Refused {
        refusal: ArcInteractiveRefusal,
    },
}

impl Serialize for TraceLocator {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TraceLocator {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(de::Error::custom)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TraceLocatorError {
    #[error("ARC trace locator must not be empty")]
    Empty,
    #[error("ARC trace locator must not contain whitespace: {0}")]
    ContainsWhitespace(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSolveRefusal {
    pub code: ArcRefusalCode,
    pub detail: String,
}

impl ArcSolveRefusal {
    pub fn new(
        code: ArcRefusalCode,
        detail: impl Into<String>,
    ) -> Result<Self, ArcSolveRefusalError> {
        let detail = detail.into();
        let trimmed = detail.trim();
        if trimmed.is_empty() {
            return Err(ArcSolveRefusalError::EmptyDetail);
        }
        Ok(Self {
            code,
            detail: trimmed.to_owned(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcSolveRefusalError {
    #[error("ARC refusal detail must not be empty")]
    EmptyDetail,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcSolveOutcome {
    Solved { output: ArcGrid },
    Unsolved,
    Refused(ArcSolveRefusal),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSolveResultEnvelope {
    pub schema_version: u32,
    pub task_id: ArcTaskId,
    pub budget: SolveBudget,
    pub attempts_used: u32,
    pub trace_locator: Option<TraceLocator>,
    pub outcome: ArcSolveOutcome,
}

impl ArcSolveResultEnvelope {
    pub fn new(
        task_id: ArcTaskId,
        budget: SolveBudget,
        attempts_used: u32,
        trace_locator: Option<TraceLocator>,
        outcome: ArcSolveOutcome,
    ) -> Result<Self, ArcSolveResultEnvelopeError> {
        if attempts_used > budget.max_attempts {
            return Err(ArcSolveResultEnvelopeError::AttemptsExceedBudget {
                attempts_used,
                max_attempts: budget.max_attempts,
            });
        }

        Ok(Self {
            schema_version: ARC_CORE_SCHEMA_VERSION,
            task_id,
            budget,
            attempts_used,
            trace_locator,
            outcome,
        })
    }

    pub fn canonical_json(&self) -> Result<String, ContractSerializationError> {
        canonical_json_string(self)
    }

    pub fn contract_digest(&self) -> Result<String, ContractSerializationError> {
        canonical_sha256_hex(self)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArcSolveResultEnvelopeError {
    #[error(
        "ARC solve result attempts_used {attempts_used} exceeds budget max_attempts {max_attempts}"
    )]
    AttemptsExceedBudget {
        attempts_used: u32,
        max_attempts: u32,
    },
}
