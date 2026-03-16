use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::schema::{ARC_CORE_SCHEMA_VERSION, ArcGrid, ArcTaskId};

/// What belongs in execution envelopes and what must stay out of it.
pub const EXECUTION_ENVELOPE_BOUNDARY_SUMMARY: &str = "Own shared budget, refusal, and solve-result envelopes for ARC crates. Do not absorb benchmark scoring policy, engine step transitions, client sessions, or solver branch internals.";

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
