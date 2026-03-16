use arc_core::{ArcAction, ArcActionKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::interactive::{
    ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveGameStep,
    ArcInteractiveSessionContext,
};

/// Shared ownership summary for the baseline interactive ARC agents.
pub const INTERACTIVE_BASELINES_BOUNDARY_SUMMARY: &str = "arc-solvers owns seeded random and deterministic scripted ARC-AGI-3 baseline agents as solver lanes over the shared interactive runner contract";

const ACTION_PRIORITY: [ArcActionKind; 7] = [
    ArcActionKind::Action1,
    ArcActionKind::Action2,
    ArcActionKind::Action3,
    ArcActionKind::Action4,
    ArcActionKind::Action5,
    ArcActionKind::Action6,
    ArcActionKind::Action7,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcRandomBaselineConfig {
    pub seed: u64,
    pub action6_min_coordinate: u8,
    pub action6_max_coordinate: u8,
}

impl ArcRandomBaselineConfig {
    pub fn new(seed: u64) -> Result<Self, ArcRandomBaselineConfigError> {
        Self::with_action6_bounds(seed, 0, 63)
    }

    pub fn with_action6_bounds(
        seed: u64,
        action6_min_coordinate: u8,
        action6_max_coordinate: u8,
    ) -> Result<Self, ArcRandomBaselineConfigError> {
        if action6_min_coordinate > 63 {
            return Err(ArcRandomBaselineConfigError::CoordinateOutOfRange {
                axis: "min",
                value: action6_min_coordinate,
            });
        }
        if action6_max_coordinate > 63 {
            return Err(ArcRandomBaselineConfigError::CoordinateOutOfRange {
                axis: "max",
                value: action6_max_coordinate,
            });
        }
        if action6_min_coordinate > action6_max_coordinate {
            return Err(ArcRandomBaselineConfigError::InvertedCoordinateRange {
                min: action6_min_coordinate,
                max: action6_max_coordinate,
            });
        }
        Ok(Self {
            seed,
            action6_min_coordinate,
            action6_max_coordinate,
        })
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcRandomBaselineConfigError {
    #[error("interactive baseline name must not be empty")]
    EmptyName,
    #[error("ACTION6 {axis} coordinate {value} must be in 0..=63")]
    CoordinateOutOfRange { axis: &'static str, value: u8 },
    #[error("ACTION6 minimum coordinate {min} must not exceed maximum coordinate {max}")]
    InvertedCoordinateRange { min: u8, max: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcRandomBaselineAgent {
    name: String,
    config: ArcRandomBaselineConfig,
    rng: ArcBaselineRng,
    actions_emitted: u32,
}

impl ArcRandomBaselineAgent {
    pub fn new(
        name: impl Into<String>,
        config: ArcRandomBaselineConfig,
    ) -> Result<Self, ArcRandomBaselineConfigError> {
        let name =
            normalize_baseline_name(name.into()).ok_or(ArcRandomBaselineConfigError::EmptyName)?;
        Ok(Self {
            name,
            rng: ArcBaselineRng::new(config.seed),
            config,
            actions_emitted: 0,
        })
    }

    #[must_use]
    pub fn config(&self) -> &ArcRandomBaselineConfig {
        &self.config
    }
}

impl ArcInteractiveAgent for ArcRandomBaselineAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        let candidates = legal_action_kinds(&context.latest_frame.available_actions);
        let action = if candidates.is_empty() {
            ArcAction::Reset
        } else {
            let choice = candidates[self.rng.choose_index(candidates.len())];
            match choice {
                ArcActionKind::Action1 => ArcAction::Action1,
                ArcActionKind::Action2 => ArcAction::Action2,
                ArcActionKind::Action3 => ArcAction::Action3,
                ArcActionKind::Action4 => ArcAction::Action4,
                ArcActionKind::Action5 => ArcAction::Action5,
                ArcActionKind::Action6 => {
                    let x = self.rng.next_coordinate(
                        self.config.action6_min_coordinate,
                        self.config.action6_max_coordinate,
                    );
                    let y = self.rng.next_coordinate(
                        self.config.action6_min_coordinate,
                        self.config.action6_max_coordinate,
                    );
                    ArcAction::action6(x, y)
                        .map_err(|error| ArcInteractiveAgentError::message(error.to_string()))?
                }
                ArcActionKind::Action7 => ArcAction::Action7,
                ArcActionKind::Reset => ArcAction::Reset,
            }
        };

        self.actions_emitted = self.actions_emitted.saturating_add(1);
        Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
            "agent": self.name,
            "seed": self.config.seed,
            "actions_emitted": self.actions_emitted,
        })))
    }

    fn checkpoint_state(&self) -> Result<Option<serde_json::Value>, ArcInteractiveAgentError> {
        Ok(Some(json!({
            "rng_state": self.rng.state,
            "actions_emitted": self.actions_emitted,
        })))
    }

    fn restore_checkpoint_state(
        &mut self,
        state: Option<&serde_json::Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        let Some(state) = state else {
            self.rng = ArcBaselineRng::new(self.config.seed);
            self.actions_emitted = 0;
            return Ok(());
        };
        let rng_state = state
            .get("rng_state")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ArcInteractiveAgentError::message("random baseline checkpoint is missing rng_state")
            })?;
        let actions_emitted = state
            .get("actions_emitted")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                ArcInteractiveAgentError::message(
                    "random baseline checkpoint is missing actions_emitted",
                )
            })?;
        self.rng = ArcBaselineRng { state: rng_state };
        self.actions_emitted = u32::try_from(actions_emitted).unwrap_or(u32::MAX);
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcScriptedBaselineProgram {
    pub program_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ArcAction>,
    #[serde(default)]
    pub fallback: ArcScriptedFallbackPolicy,
}

impl ArcScriptedBaselineProgram {
    pub fn new(
        program_id: impl Into<String>,
        actions: Vec<ArcAction>,
    ) -> Result<Self, ArcScriptedBaselineConfigError> {
        Self::with_fallback(program_id, actions, ArcScriptedFallbackPolicy::Refuse)
    }

    pub fn with_fallback(
        program_id: impl Into<String>,
        actions: Vec<ArcAction>,
        fallback: ArcScriptedFallbackPolicy,
    ) -> Result<Self, ArcScriptedBaselineConfigError> {
        Ok(Self {
            program_id: normalize_baseline_name(program_id.into())
                .ok_or(ArcScriptedBaselineConfigError::EmptyName)?,
            actions,
            fallback,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArcScriptedFallbackPolicy {
    #[default]
    Refuse,
    FirstLegalByPriority,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcScriptedBaselineConfigError {
    #[error("interactive baseline name must not be empty")]
    EmptyName,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcScriptedBaselineAgent {
    name: String,
    program: ArcScriptedBaselineProgram,
    cursor: usize,
}

impl ArcScriptedBaselineAgent {
    pub fn new(
        name: impl Into<String>,
        program: ArcScriptedBaselineProgram,
    ) -> Result<Self, ArcScriptedBaselineConfigError> {
        Ok(Self {
            name: normalize_baseline_name(name.into())
                .ok_or(ArcScriptedBaselineConfigError::EmptyName)?,
            program,
            cursor: 0,
        })
    }

    pub fn default_tester(name: impl Into<String>) -> Result<Self, ArcScriptedBaselineConfigError> {
        Self::new(
            name,
            ArcScriptedBaselineProgram::with_fallback(
                "default-tester",
                Vec::new(),
                ArcScriptedFallbackPolicy::FirstLegalByPriority,
            )?,
        )
    }

    #[must_use]
    pub fn program(&self) -> &ArcScriptedBaselineProgram {
        &self.program
    }
}

impl ArcInteractiveAgent for ArcScriptedBaselineAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        if let Some(action) = self.program.actions.get(self.cursor).cloned() {
            if scripted_action_is_legal(&action, context) {
                self.cursor = self.cursor.saturating_add(1);
                return Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
                    "agent": self.name,
                    "program_id": self.program.program_id,
                    "cursor": self.cursor,
                })));
            }
            return self.handle_fallback(context, Some(action));
        }
        self.handle_fallback(context, None)
    }

    fn checkpoint_state(&self) -> Result<Option<serde_json::Value>, ArcInteractiveAgentError> {
        Ok(Some(json!({
            "cursor": self.cursor,
            "program_id": self.program.program_id,
        })))
    }

    fn restore_checkpoint_state(
        &mut self,
        state: Option<&serde_json::Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        let cursor = state
            .and_then(|state| state.get("cursor"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        self.cursor = usize::try_from(cursor).unwrap_or(usize::MAX);
        Ok(())
    }
}

impl ArcScriptedBaselineAgent {
    fn handle_fallback(
        &mut self,
        context: &ArcInteractiveSessionContext,
        illegal_action: Option<ArcAction>,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        match self.program.fallback {
            ArcScriptedFallbackPolicy::Refuse => {
                let detail = if let Some(action) = illegal_action {
                    format!(
                        "scripted baseline `{}` cannot emit illegal action {:?} at cursor {}",
                        self.program.program_id, action, self.cursor
                    )
                } else {
                    format!(
                        "scripted baseline `{}` exhausted its programmed actions at cursor {}",
                        self.program.program_id, self.cursor
                    )
                };
                Err(ArcInteractiveAgentError::message(detail))
            }
            ArcScriptedFallbackPolicy::FirstLegalByPriority => {
                let action = first_legal_action(context).unwrap_or(ArcAction::Reset);
                Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
                    "agent": self.name,
                    "program_id": self.program.program_id,
                    "cursor": self.cursor,
                    "fallback": "first_legal_by_priority",
                })))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcBaselineRng {
    state: u64,
}

impl ArcBaselineRng {
    fn new(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn choose_index(&mut self, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        let len = u64::try_from(len).unwrap_or(u64::MAX);
        usize::try_from(self.next_u64() % len).unwrap_or(0)
    }

    fn next_coordinate(&mut self, min: u8, max: u8) -> u8 {
        let span = u64::from(max.saturating_sub(min)) + 1;
        min.saturating_add(u8::try_from(self.next_u64() % span).unwrap_or(0))
    }
}

fn legal_action_kinds(available_actions: &[ArcActionKind]) -> Vec<ArcActionKind> {
    ACTION_PRIORITY
        .iter()
        .copied()
        .filter(|action| available_actions.contains(action))
        .collect()
}

fn first_legal_action(context: &ArcInteractiveSessionContext) -> Option<ArcAction> {
    let action = legal_action_kinds(&context.latest_frame.available_actions)
        .into_iter()
        .next()?;
    Some(match action {
        ArcActionKind::Action1 => ArcAction::Action1,
        ArcActionKind::Action2 => ArcAction::Action2,
        ArcActionKind::Action3 => ArcAction::Action3,
        ArcActionKind::Action4 => ArcAction::Action4,
        ArcActionKind::Action5 => ArcAction::Action5,
        ArcActionKind::Action6 => ArcAction::action6(0, 0).ok()?,
        ArcActionKind::Action7 => ArcAction::Action7,
        ArcActionKind::Reset => ArcAction::Reset,
    })
}

fn scripted_action_is_legal(action: &ArcAction, context: &ArcInteractiveSessionContext) -> bool {
    *action == ArcAction::Reset
        || context
            .latest_frame
            .available_actions
            .contains(&action.kind())
}

fn normalize_baseline_name(name: String) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use arc_client::ArcSessionFrame;
    use arc_core::{ArcFrameData, ArcGameState, ArcTaskId};

    use super::*;

    fn context_with_actions(available_actions: Vec<ArcActionKind>) -> ArcInteractiveSessionContext {
        let latest_frame = ArcSessionFrame {
            game_id: ArcTaskId::new("baseline-fixture").expect("task id should validate"),
            guid: "baseline-guid".to_owned(),
            frames: vec![ArcFrameData::new(1, 1, vec![0]).expect("frame should validate")],
            game_state: ArcGameState::NotFinished,
            levels_completed: 0,
            win_levels: 1,
            action: ArcAction::Reset,
            available_actions,
            full_reset: true,
        };
        let context_retention =
            crate::interactive_context::ArcInteractiveContextRetentionPolicy::default();
        let prompt_policy = crate::interactive_context::ArcInteractivePromptPolicy::default();
        let budget = arc_core::ArcInteractiveBudget::new(4)
            .expect("budget should validate")
            .state(0)
            .expect("budget state should validate");
        ArcInteractiveSessionContext {
            benchmark: arc_core::ArcBenchmark::ArcAgi3,
            game_id: ArcTaskId::new("baseline-fixture").expect("task id should validate"),
            environment_kind: crate::interactive::ArcInteractiveEnvironmentKind::Local,
            scorecard_id: "baseline-card".to_owned(),
            operation_mode: arc_core::ArcOperationMode::Offline,
            session_guid: Some("baseline-guid".to_owned()),
            step_index: 1,
            actions_taken: 0,
            remaining_actions: 4,
            budget,
            progress: crate::interactive_context::build_progress_state(&latest_frame, budget),
            latest_frame: latest_frame.clone(),
            history: Vec::new(),
            omitted_history_steps: 0,
            context_retention: context_retention.clone(),
            prompt_policy: prompt_policy.clone(),
            memory: crate::interactive_context::ArcInteractiveSessionMemory::empty(
                &context_retention,
            ),
            prompt_plan: crate::interactive_context::build_prompt_plan(
                &prompt_policy,
                crate::interactive_context::build_progress_state(&latest_frame, budget),
                &latest_frame,
                &[],
                0,
                &crate::interactive_context::ArcInteractiveSessionMemory::empty(&context_retention),
                None,
            ),
            resume_state: None,
        }
    }

    #[test]
    fn random_baseline_is_repeatable_and_checkpointable() {
        let config = ArcRandomBaselineConfig::new(7).expect("config should validate");
        let mut first =
            ArcRandomBaselineAgent::new("random-a", config.clone()).expect("agent should build");
        let mut second =
            ArcRandomBaselineAgent::new("random-b", config).expect("agent should build");
        let context = context_with_actions(vec![ArcActionKind::Action2, ArcActionKind::Action6]);

        let step_a = first.step(&context).expect("random step should succeed");
        let checkpoint = first
            .checkpoint_state()
            .expect("checkpoint state should serialize");
        let step_b = second.step(&context).expect("random step should succeed");

        assert_eq!(step_a.action, step_b.action);
        second
            .restore_checkpoint_state(checkpoint.as_ref())
            .expect("checkpoint restore should work");
        let resumed = second.step(&context).expect("restored step should succeed");
        let continued = first.step(&context).expect("continued step should succeed");
        assert_eq!(resumed.action, continued.action);
    }

    #[test]
    fn scripted_baseline_refuses_illegal_actions_and_can_fall_back() {
        let refusing_program =
            ArcScriptedBaselineProgram::new("illegal-program", vec![ArcAction::Action1])
                .expect("program should validate");
        let mut refusing = ArcScriptedBaselineAgent::new("scripted", refusing_program)
            .expect("agent should build");
        let context = context_with_actions(vec![ArcActionKind::Action4]);
        let error = refusing
            .step(&context)
            .expect_err("illegal scripted actions should refuse");
        assert!(error.to_string().contains("illegal action"));

        let fallback_program = ArcScriptedBaselineProgram::with_fallback(
            "fallback-program",
            vec![ArcAction::Action1],
            ArcScriptedFallbackPolicy::FirstLegalByPriority,
        )
        .expect("program should validate");
        let mut fallback = ArcScriptedBaselineAgent::new("fallback", fallback_program)
            .expect("agent should build");
        let fallback_step = fallback
            .step(&context)
            .expect("fallback step should succeed");
        assert_eq!(fallback_step.action, ArcAction::Action4);
    }
}
