use arc_core::{ARC_ACTION6_COORDINATE_MAX, ArcAction, ArcActionKind};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::interactive::{
    ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveGameStep,
    ArcInteractiveSessionContext,
};
use crate::interactive_context::ArcInteractivePromptSectionView;

/// Ownership summary for the ADCR-style interactive baseline.
pub const INTERACTIVE_ADCR_BOUNDARY_SUMMARY: &str = "arc-solvers owns the ADCR-style interactive baseline as one baseline lane over the shared runner contract, including baseline-local prompt templates, scratchpad memory, and phase policy without turning ADCR into the library contract";

/// Human-level action vocabulary used by the ADCR baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcAdcrHumanAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    PerformAction,
    ClickObject { x: u8, y: u8 },
    Undo,
    Reset,
}

impl ArcAdcrHumanAction {
    /// Builds a click action with validated coordinates.
    pub fn click_object(x: u8, y: u8) -> Result<Self, ArcAdcrHumanActionError> {
        if x > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcAdcrHumanActionError::CoordinateOutOfRange {
                axis: "x",
                value: x,
            });
        }
        if y > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcAdcrHumanActionError::CoordinateOutOfRange {
                axis: "y",
                value: y,
            });
        }
        Ok(Self::ClickObject { x, y })
    }

    #[must_use]
    pub fn action_kind(&self) -> ArcActionKind {
        match self {
            Self::MoveUp => ArcActionKind::Action1,
            Self::MoveDown => ArcActionKind::Action2,
            Self::MoveLeft => ArcActionKind::Action3,
            Self::MoveRight => ArcActionKind::Action4,
            Self::PerformAction => ArcActionKind::Action5,
            Self::ClickObject { .. } => ArcActionKind::Action6,
            Self::Undo => ArcActionKind::Action7,
            Self::Reset => ArcActionKind::Reset,
        }
    }

    pub fn to_arc_action(&self) -> Result<ArcAction, ArcInteractiveAgentError> {
        match self {
            Self::MoveUp => Ok(ArcAction::Action1),
            Self::MoveDown => Ok(ArcAction::Action2),
            Self::MoveLeft => Ok(ArcAction::Action3),
            Self::MoveRight => Ok(ArcAction::Action4),
            Self::PerformAction => Ok(ArcAction::Action5),
            Self::ClickObject { x, y } => ArcAction::action6(*x, *y)
                .map_err(|error| ArcInteractiveAgentError::message(error.to_string())),
            Self::Undo => Ok(ArcAction::Action7),
            Self::Reset => Ok(ArcAction::Reset),
        }
    }

    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::MoveUp => "Move Up",
            Self::MoveDown => "Move Down",
            Self::MoveLeft => "Move Left",
            Self::MoveRight => "Move Right",
            Self::PerformAction => "Perform Action",
            Self::ClickObject { .. } => "Click object on screen",
            Self::Undo => "Undo",
            Self::Reset => "Reset",
        }
    }
}

/// Human-action validation failure for ADCR.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcAdcrHumanActionError {
    #[error("ADCR click coordinate {axis}={value} must be in 0..=63")]
    CoordinateOutOfRange { axis: &'static str, value: u8 },
}

/// Phase-level progress diagnosis for ADCR review and memory shaping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcAdcrProgressDelta {
    InitialObservation,
    ObservationChanged,
    LevelAdvanced,
    Terminal,
    NoVisibleProgress,
}

/// Typed analysis product for the ADCR Analyze phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcAdcrAnalysis {
    pub summary: String,
    pub progress_delta: ArcAdcrProgressDelta,
    #[serde(default)]
    pub budget_tight: bool,
}

/// Baseline-local prompt template bundle for ADCR.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcAdcrPromptTemplates {
    pub template_id: String,
    pub system_prompt: String,
    pub analyze_instruction: String,
    pub decide_instruction: String,
    pub convert_instruction: String,
    pub review_instruction: String,
}

impl Default for ArcAdcrPromptTemplates {
    fn default() -> Self {
        Self {
            template_id: "arc-adcr-prompts-v1".to_owned(),
            system_prompt: "ADCR interactive ARC baseline".to_owned(),
            analyze_instruction:
                "Analyze the latest typed ARC observation and summarize what changed.".to_owned(),
            decide_instruction:
                "Choose one human-level action from the currently legal action set.".to_owned(),
            convert_instruction:
                "Convert the human-level action into one concrete ARC game action.".to_owned(),
            review_instruction:
                "Write one short scratchpad note describing what was attempted and what changed."
                    .to_owned(),
        }
    }
}

/// Replay fallback when the programmed ADCR action is illegal or exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArcAdcrReplayFallbackPolicy {
    #[default]
    Refuse,
    FirstLegalByPriority,
}

/// Deterministic replay program for one ADCR baseline run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcAdcrReplayProgram {
    pub program_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub human_actions: Vec<ArcAdcrHumanAction>,
    #[serde(default)]
    pub fallback: ArcAdcrReplayFallbackPolicy,
}

impl ArcAdcrReplayProgram {
    pub fn new(
        program_id: impl Into<String>,
        human_actions: Vec<ArcAdcrHumanAction>,
    ) -> Result<Self, ArcAdcrReplayProgramError> {
        Self::with_fallback(
            program_id,
            human_actions,
            ArcAdcrReplayFallbackPolicy::Refuse,
        )
    }

    pub fn with_fallback(
        program_id: impl Into<String>,
        human_actions: Vec<ArcAdcrHumanAction>,
        fallback: ArcAdcrReplayFallbackPolicy,
    ) -> Result<Self, ArcAdcrReplayProgramError> {
        let program_id = normalize_adcr_name(program_id.into())
            .ok_or(ArcAdcrReplayProgramError::EmptyProgramId)?;
        Ok(Self {
            program_id,
            human_actions,
            fallback,
        })
    }
}

/// Replay-program validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcAdcrReplayProgramError {
    #[error("ADCR replay program id must not be empty")]
    EmptyProgramId,
}

/// Execution mode for the ADCR baseline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ArcAdcrMode {
    Heuristic,
    ReplayProgram { program: ArcAdcrReplayProgram },
}

/// Configurable baseline-local ADCR agent behavior.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcAdcrConfig {
    pub memory_note_limit: u32,
    pub default_click_x: u8,
    pub default_click_y: u8,
    pub prompt_templates: ArcAdcrPromptTemplates,
    pub mode: ArcAdcrMode,
}

impl ArcAdcrConfig {
    pub fn heuristic() -> Self {
        Self {
            memory_note_limit: 8,
            default_click_x: 22,
            default_click_y: 22,
            prompt_templates: ArcAdcrPromptTemplates::default(),
            mode: ArcAdcrMode::Heuristic,
        }
    }

    pub fn replay(program: ArcAdcrReplayProgram) -> Self {
        Self {
            mode: ArcAdcrMode::ReplayProgram { program },
            ..Self::heuristic()
        }
    }
}

/// ADCR configuration failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcAdcrConfigError {
    #[error("interactive baseline name must not be empty")]
    EmptyName,
    #[error("ADCR memory_note_limit must allow at least one note")]
    ZeroMemoryNoteLimit,
    #[error("ADCR default click coordinate {axis}={value} must be in 0..=63")]
    CoordinateOutOfRange { axis: &'static str, value: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct ArcAdcrCheckpointState {
    cursor: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    memory_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_human_action: Option<ArcAdcrHumanAction>,
}

/// Analyze -> Decide -> Convert -> Review baseline agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcAdcrBaselineAgent {
    name: String,
    config: ArcAdcrConfig,
    cursor: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    memory_notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    previous_human_action: Option<ArcAdcrHumanAction>,
}

impl ArcAdcrBaselineAgent {
    pub fn new(name: impl Into<String>, config: ArcAdcrConfig) -> Result<Self, ArcAdcrConfigError> {
        let name = normalize_adcr_name(name.into()).ok_or(ArcAdcrConfigError::EmptyName)?;
        if config.memory_note_limit == 0 {
            return Err(ArcAdcrConfigError::ZeroMemoryNoteLimit);
        }
        if config.default_click_x > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcAdcrConfigError::CoordinateOutOfRange {
                axis: "x",
                value: config.default_click_x,
            });
        }
        if config.default_click_y > ARC_ACTION6_COORDINATE_MAX {
            return Err(ArcAdcrConfigError::CoordinateOutOfRange {
                axis: "y",
                value: config.default_click_y,
            });
        }
        Ok(Self {
            name,
            config,
            cursor: 0,
            memory_notes: Vec::new(),
            previous_human_action: None,
        })
    }

    #[must_use]
    pub fn config(&self) -> &ArcAdcrConfig {
        &self.config
    }

    fn analyze(&self, context: &ArcInteractiveSessionContext) -> ArcAdcrAnalysis {
        let progress_delta = match context.memory.entries.last() {
            None => ArcAdcrProgressDelta::InitialObservation,
            Some(_previous) if context.progress.terminal => ArcAdcrProgressDelta::Terminal,
            Some(previous)
                if context.progress.levels_completed > previous.observation.levels_completed =>
            {
                ArcAdcrProgressDelta::LevelAdvanced
            }
            Some(previous)
                if previous.observation.frames
                    != crate::interactive_context::ArcInteractiveContextFrame::from(
                        &context.latest_frame,
                    )
                    .frames =>
            {
                ArcAdcrProgressDelta::ObservationChanged
            }
            Some(_) => ArcAdcrProgressDelta::NoVisibleProgress,
        };
        let summary = match progress_delta {
            ArcAdcrProgressDelta::InitialObservation => {
                "initial observation; no prior action to review".to_owned()
            }
            ArcAdcrProgressDelta::ObservationChanged => {
                "latest observation changed after the previous action".to_owned()
            }
            ArcAdcrProgressDelta::LevelAdvanced => {
                "levels completed increased after the previous action".to_owned()
            }
            ArcAdcrProgressDelta::Terminal => "environment is terminal".to_owned(),
            ArcAdcrProgressDelta::NoVisibleProgress => {
                "no visible progress from the previous action".to_owned()
            }
        };
        ArcAdcrAnalysis {
            summary,
            progress_delta,
            budget_tight: context.remaining_actions <= 2,
        }
    }

    fn decide_human_action(
        &mut self,
        context: &ArcInteractiveSessionContext,
        analysis: &ArcAdcrAnalysis,
    ) -> Result<ArcAdcrHumanAction, ArcInteractiveAgentError> {
        let legal = legal_human_actions(
            context,
            self.config.default_click_x,
            self.config.default_click_y,
        )?;
        if legal.is_empty() {
            return Ok(ArcAdcrHumanAction::Reset);
        }
        match &self.config.mode {
            ArcAdcrMode::Heuristic => choose_heuristic_action(
                legal.as_slice(),
                self.previous_human_action.as_ref(),
                analysis.progress_delta,
            ),
            ArcAdcrMode::ReplayProgram { program } => {
                if let Some(human_action) = program.human_actions.get(self.cursor).cloned() {
                    if adcr_action_is_legal(&human_action, legal.as_slice()) {
                        self.cursor = self.cursor.saturating_add(1);
                        return Ok(human_action);
                    }
                    return match program.fallback {
                        ArcAdcrReplayFallbackPolicy::Refuse => {
                            Err(ArcInteractiveAgentError::message(format!(
                                "ADCR replay program `{}` requested illegal action {:?}",
                                program.program_id, human_action
                            )))
                        }
                        ArcAdcrReplayFallbackPolicy::FirstLegalByPriority => {
                            choose_heuristic_action(
                                legal.as_slice(),
                                self.previous_human_action.as_ref(),
                                analysis.progress_delta,
                            )
                        }
                    };
                }
                match program.fallback {
                    ArcAdcrReplayFallbackPolicy::Refuse => Err(ArcInteractiveAgentError::message(
                        format!("ADCR replay program `{}` is exhausted", program.program_id),
                    )),
                    ArcAdcrReplayFallbackPolicy::FirstLegalByPriority => choose_heuristic_action(
                        legal.as_slice(),
                        self.previous_human_action.as_ref(),
                        analysis.progress_delta,
                    ),
                }
            }
        }
    }

    fn review(
        &mut self,
        context: &ArcInteractiveSessionContext,
        analysis: &ArcAdcrAnalysis,
        human_action: &ArcAdcrHumanAction,
    ) {
        let note = format!(
            "step {}: {} -> {}",
            context.step_index,
            human_action.description(),
            analysis.summary
        );
        self.memory_notes.push(note);
        let max_len = usize::try_from(self.config.memory_note_limit).unwrap_or(usize::MAX);
        if self.memory_notes.len() > max_len {
            let excess = self.memory_notes.len().saturating_sub(max_len);
            self.memory_notes.drain(0..excess);
        }
        self.previous_human_action = Some(human_action.clone());
    }
}

impl ArcInteractiveAgent for ArcAdcrBaselineAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        let analysis = self.analyze(context);
        let human_action = self.decide_human_action(context, &analysis)?;
        let action = human_action.to_arc_action()?;

        let analyze_prompt = render_phase_prompt(
            &self.config.prompt_templates,
            "analyze",
            context,
            &self.memory_notes,
        );
        let decide_prompt = render_phase_prompt(
            &self.config.prompt_templates,
            "decide",
            context,
            &self.memory_notes,
        );
        let convert_prompt = render_phase_prompt(
            &self.config.prompt_templates,
            "convert",
            context,
            &self.memory_notes,
        );
        self.review(context, &analysis, &human_action);

        Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
            "agent": self.name,
            "phase": "adcr",
            "analysis": analysis,
            "human_action": human_action,
            "prompt_policy_id": context.prompt_policy.policy_id,
            "prompt_sections": context.prompt_policy.sections,
            "prompt_preview": {
                "analyze": truncate_prompt(&analyze_prompt),
                "decide": truncate_prompt(&decide_prompt),
                "convert": truncate_prompt(&convert_prompt),
            },
            "memory_notes": self.memory_notes,
        })))
    }

    fn checkpoint_state(&self) -> Result<Option<serde_json::Value>, ArcInteractiveAgentError> {
        serde_json::to_value(ArcAdcrCheckpointState {
            cursor: self.cursor,
            memory_notes: self.memory_notes.clone(),
            previous_human_action: self.previous_human_action.clone(),
        })
        .map(Some)
        .map_err(|error| ArcInteractiveAgentError::message(error.to_string()))
    }

    fn restore_checkpoint_state(
        &mut self,
        state: Option<&serde_json::Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        let Some(state) = state else {
            self.cursor = 0;
            self.memory_notes.clear();
            self.previous_human_action = None;
            return Ok(());
        };
        let state: ArcAdcrCheckpointState = serde_json::from_value(state.clone())
            .map_err(|error| ArcInteractiveAgentError::message(error.to_string()))?;
        self.cursor = state.cursor;
        self.memory_notes = state.memory_notes;
        self.previous_human_action = state.previous_human_action;
        Ok(())
    }
}

fn legal_human_actions(
    context: &ArcInteractiveSessionContext,
    default_click_x: u8,
    default_click_y: u8,
) -> Result<Vec<ArcAdcrHumanAction>, ArcInteractiveAgentError> {
    let mut actions = Vec::new();
    if context.latest_frame.available_actions.is_empty() {
        actions.push(ArcAdcrHumanAction::Reset);
        return Ok(actions);
    }
    for action in &context.latest_frame.available_actions {
        actions.push(match action {
            ArcActionKind::Action1 => ArcAdcrHumanAction::MoveUp,
            ArcActionKind::Action2 => ArcAdcrHumanAction::MoveDown,
            ArcActionKind::Action3 => ArcAdcrHumanAction::MoveLeft,
            ArcActionKind::Action4 => ArcAdcrHumanAction::MoveRight,
            ArcActionKind::Action5 => ArcAdcrHumanAction::PerformAction,
            ArcActionKind::Action6 => {
                ArcAdcrHumanAction::click_object(default_click_x, default_click_y)
                    .map_err(|error| ArcInteractiveAgentError::message(error.to_string()))?
            }
            ArcActionKind::Action7 => ArcAdcrHumanAction::Undo,
            ArcActionKind::Reset => ArcAdcrHumanAction::Reset,
        });
    }
    Ok(actions)
}

fn choose_heuristic_action(
    legal: &[ArcAdcrHumanAction],
    previous_human_action: Option<&ArcAdcrHumanAction>,
    progress_delta: ArcAdcrProgressDelta,
) -> Result<ArcAdcrHumanAction, ArcInteractiveAgentError> {
    const PRIORITY: &[ArcActionKind] = &[
        ArcActionKind::Action4,
        ArcActionKind::Action2,
        ArcActionKind::Action6,
        ArcActionKind::Action5,
        ArcActionKind::Action3,
        ArcActionKind::Action1,
        ArcActionKind::Action7,
    ];
    let mut candidates = legal.to_vec();
    candidates.sort_by_key(|action| {
        PRIORITY
            .iter()
            .position(|priority| *priority == action.action_kind())
            .unwrap_or(PRIORITY.len())
    });
    if progress_delta == ArcAdcrProgressDelta::NoVisibleProgress {
        if let Some(previous) = previous_human_action {
            if let Some(next) = candidates.iter().find(|candidate| *candidate != previous) {
                return Ok(next.clone());
            }
        }
    }
    candidates
        .into_iter()
        .next()
        .ok_or_else(|| ArcInteractiveAgentError::message("ADCR found no legal human actions"))
}

fn adcr_action_is_legal(action: &ArcAdcrHumanAction, legal: &[ArcAdcrHumanAction]) -> bool {
    legal.iter().any(|candidate| candidate == action)
}

fn render_phase_prompt(
    templates: &ArcAdcrPromptTemplates,
    stage: &str,
    context: &ArcInteractiveSessionContext,
    memory_notes: &[String],
) -> String {
    let instruction = match stage {
        "analyze" => templates.analyze_instruction.as_str(),
        "decide" => templates.decide_instruction.as_str(),
        "convert" => templates.convert_instruction.as_str(),
        _ => templates.review_instruction.as_str(),
    };
    format!(
        "{}\n\nStage: {stage}\nInstruction: {instruction}\nPrompt policy: {}\nContext:\n{}\nScratchpad:\n{}",
        templates.system_prompt,
        context.prompt_policy.policy_id,
        render_prompt_plan(context.prompt_plan.sections.as_slice()),
        if memory_notes.is_empty() {
            "<empty>".to_owned()
        } else {
            memory_notes.join("\n")
        }
    )
}

fn render_prompt_plan(sections: &[ArcInteractivePromptSectionView]) -> String {
    sections
        .iter()
        .map(|section| match section {
            ArcInteractivePromptSectionView::SessionProgress { progress } => format!(
                "- session_progress: levels_completed={}/{} remaining_actions={}",
                progress.levels_completed, progress.win_levels, progress.budget.remaining_actions
            ),
            ArcInteractivePromptSectionView::CurrentObservation { frame } => format!(
                "- current_observation: frame_count={} available_actions={:?}",
                frame.frames.len(),
                frame.available_actions
            ),
            ArcInteractivePromptSectionView::AvailableActions { actions } => {
                format!("- available_actions: {:?}", actions)
            }
            ArcInteractivePromptSectionView::RetainedHistory {
                omitted_steps,
                frames,
            } => format!(
                "- retained_history: retained={} omitted={}",
                frames.len(),
                omitted_steps
            ),
            ArcInteractivePromptSectionView::SessionMemory { memory } => format!(
                "- session_memory: retained={} omitted={}",
                memory.entries.len(),
                memory.omitted_entries
            ),
            ArcInteractivePromptSectionView::ResumeSummary { summary } => format!(
                "- resume_summary: checkpoint={} next_step_index={}",
                summary.checkpoint_id, summary.next_step_index
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_prompt(prompt: &str) -> String {
    const MAX_CHARS: usize = 240;
    if prompt.chars().count() <= MAX_CHARS {
        return prompt.to_owned();
    }
    prompt.chars().take(MAX_CHARS).collect()
}

fn normalize_adcr_name(name: String) -> Option<String> {
    let trimmed = name.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use arc_client::ArcSessionFrame;
    use arc_core::{
        ArcBenchmark, ArcFrameData, ArcGameState, ArcInteractiveBudget, ArcOperationMode, ArcTaskId,
    };

    use super::*;

    fn context_with_actions(available_actions: Vec<ArcActionKind>) -> ArcInteractiveSessionContext {
        let latest_frame = ArcSessionFrame {
            game_id: ArcTaskId::new("adcr-fixture").expect("task id should validate"),
            guid: "adcr-guid".to_owned(),
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
        let budget = ArcInteractiveBudget::new(4)
            .expect("budget should validate")
            .state(0)
            .expect("budget state should validate");
        let memory =
            crate::interactive_context::ArcInteractiveSessionMemory::empty(&context_retention);
        ArcInteractiveSessionContext {
            benchmark: ArcBenchmark::ArcAgi3,
            game_id: ArcTaskId::new("adcr-fixture").expect("task id should validate"),
            environment_kind: crate::interactive::ArcInteractiveEnvironmentKind::Local,
            scorecard_id: "adcr-card".to_owned(),
            operation_mode: ArcOperationMode::Offline,
            session_guid: Some("adcr-guid".to_owned()),
            step_index: 1,
            actions_taken: 0,
            remaining_actions: 4,
            budget,
            progress: crate::interactive_context::build_progress_state(&latest_frame, budget),
            latest_frame: latest_frame.clone(),
            history: vec![
                crate::interactive_context::ArcInteractiveContextFrame::from(&latest_frame),
            ],
            omitted_history_steps: 0,
            context_retention: context_retention.clone(),
            prompt_policy: prompt_policy.clone(),
            memory: memory.clone(),
            prompt_plan: crate::interactive_context::build_prompt_plan(
                &prompt_policy,
                crate::interactive_context::build_progress_state(&latest_frame, budget),
                &latest_frame,
                &[crate::interactive_context::ArcInteractiveContextFrame::from(&latest_frame)],
                0,
                &memory,
                None,
            ),
            resume_state: None,
        }
    }

    #[test]
    fn replay_adcr_program_falls_back_to_first_legal_action() {
        let program = ArcAdcrReplayProgram::with_fallback(
            "fallback",
            vec![ArcAdcrHumanAction::MoveRight],
            ArcAdcrReplayFallbackPolicy::FirstLegalByPriority,
        )
        .expect("program should validate");
        let mut agent = ArcAdcrBaselineAgent::new("adcr", ArcAdcrConfig::replay(program))
            .expect("agent should build");
        let context = context_with_actions(vec![ArcActionKind::Action2]);
        let step = agent.step(&context).expect("fallback should succeed");
        assert_eq!(step.action, ArcAction::Action2);
    }

    #[test]
    fn adcr_checkpoint_state_round_trips_cursor_and_memory() {
        let program = ArcAdcrReplayProgram::new("demo", vec![ArcAdcrHumanAction::MoveDown])
            .expect("program should validate");
        let mut first = ArcAdcrBaselineAgent::new("adcr-a", ArcAdcrConfig::replay(program.clone()))
            .expect("agent should build");
        let mut second = ArcAdcrBaselineAgent::new("adcr-b", ArcAdcrConfig::replay(program))
            .expect("agent should build");
        let context = context_with_actions(vec![ArcActionKind::Action2]);
        let _ = first.step(&context).expect("step should succeed");
        let checkpoint = first
            .checkpoint_state()
            .expect("checkpoint should serialize");
        second
            .restore_checkpoint_state(checkpoint.as_ref())
            .expect("checkpoint should restore");
        assert_eq!(first.cursor, second.cursor);
        assert_eq!(first.memory_notes, second.memory_notes);
    }
}
