use serde::{Deserialize, Serialize};

use crate::schema::{ARC_CORE_SCHEMA_VERSION, ArcGrid, ArcTaskId};

/// What belongs in execution envelopes and what must stay out of it.
pub const EXECUTION_ENVELOPE_BOUNDARY_SUMMARY: &str = "Own shared budget, refusal, and solve-result envelopes for ARC crates. Do not absorb benchmark scoring policy, engine step transitions, client sessions, or solver branch internals.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolveBudget {
    pub max_attempts: u32,
    pub max_steps: u32,
    pub max_runtime_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcRefusalCode {
    UnsupportedTask,
    InvalidTaskContract,
    BudgetExhausted,
    CompetitionModeRestricted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcSolveRefusal {
    pub code: ArcRefusalCode,
    pub detail: String,
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
    pub trace_locator: Option<String>,
    pub outcome: ArcSolveOutcome,
}

impl ArcSolveResultEnvelope {
    #[must_use]
    pub fn new(
        task_id: ArcTaskId,
        budget: SolveBudget,
        attempts_used: u32,
        trace_locator: Option<String>,
        outcome: ArcSolveOutcome,
    ) -> Self {
        Self {
            schema_version: ARC_CORE_SCHEMA_VERSION,
            task_id,
            budget,
            attempts_used,
            trace_locator,
            outcome,
        }
    }
}
