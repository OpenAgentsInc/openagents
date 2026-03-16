use std::collections::BTreeSet;

use arc_client::ArcSessionFrame;
use arc_core::{
    ArcAction, ArcActionKind, ArcFrameData, ArcGameState, ArcInteractiveActionResult,
    ArcInteractiveBudgetState,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Shared ownership summary for typed interactive ARC context and prompt policy.
pub const INTERACTIVE_CONTEXT_BOUNDARY_SUMMARY: &str = "arc-solvers owns bounded interactive ARC-AGI-3 context retention, prompt-policy, and checkpoint-resumable session memory surfaces while keeping them subordinate to typed action, score, and environment contracts";

const INTERACTIVE_CONTEXT_SCHEMA_VERSION: u16 = 1;

/// How resume should behave when bounded context state is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractiveResumeContextMode {
    /// Refuse resume instead of silently dropping prior prompt/memory context.
    #[default]
    RequireMatchingContext,
    /// Allow resume to continue with empty retained context.
    AllowEmptyContext,
}

/// Inspectable, bounded context-retention policy for interactive ARC agents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveContextRetentionPolicy {
    /// Stable policy identifier. Callers should version semantic changes here.
    pub policy_id: String,
    /// Schema version for policy serialization.
    pub schema_version: u16,
    /// Maximum number of retained observation frames exposed in `SessionContext.history`.
    pub history_window: u32,
    /// Maximum number of retained memory entries exposed in `SessionContext.memory`.
    pub memory_window: u32,
    /// Whether step reasoning payloads are preserved in session memory.
    #[serde(default)]
    pub retain_reasoning: bool,
    /// Resume requirement for the policy.
    #[serde(default)]
    pub resume_context_mode: ArcInteractiveResumeContextMode,
}

impl ArcInteractiveContextRetentionPolicy {
    /// Creates a typed context-retention policy.
    pub fn new(
        policy_id: impl Into<String>,
        history_window: u32,
        memory_window: u32,
    ) -> Result<Self, ArcInteractiveContextPolicyError> {
        Ok(Self {
            policy_id: normalize_policy_field("context retention policy id", policy_id.into())?,
            schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
            history_window,
            memory_window,
            retain_reasoning: true,
            resume_context_mode: ArcInteractiveResumeContextMode::RequireMatchingContext,
        })
    }
}

impl Default for ArcInteractiveContextRetentionPolicy {
    fn default() -> Self {
        Self {
            policy_id: "arc-interactive-context-v1".to_owned(),
            schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
            history_window: 4,
            memory_window: 8,
            retain_reasoning: true,
            resume_context_mode: ArcInteractiveResumeContextMode::RequireMatchingContext,
        }
    }
}

/// Prompt-policy sections that a baseline agent may render from the typed context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ArcInteractivePromptSection {
    SessionProgress,
    CurrentObservation,
    AvailableActions,
    RetainedHistory,
    SessionMemory,
    ResumeSummary,
}

/// Versioned, inspectable prompt/context policy for baseline interactive agents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractivePromptPolicy {
    /// Stable policy identifier. Callers should version semantic changes here.
    pub policy_id: String,
    /// Schema version for policy serialization.
    pub schema_version: u16,
    /// Ordered structured sections exposed to the agent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<ArcInteractivePromptSection>,
}

impl ArcInteractivePromptPolicy {
    /// Creates a typed prompt-policy surface.
    pub fn new(
        policy_id: impl Into<String>,
        sections: Vec<ArcInteractivePromptSection>,
    ) -> Result<Self, ArcInteractiveContextPolicyError> {
        let policy_id = normalize_policy_field("prompt policy id", policy_id.into())?;
        let mut seen = BTreeSet::new();
        for section in &sections {
            if !seen.insert(*section) {
                return Err(ArcInteractiveContextPolicyError::DuplicatePromptSection {
                    section: *section,
                });
            }
        }
        Ok(Self {
            policy_id,
            schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
            sections,
        })
    }
}

impl Default for ArcInteractivePromptPolicy {
    fn default() -> Self {
        Self {
            policy_id: "arc-interactive-prompt-v1".to_owned(),
            schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
            sections: vec![
                ArcInteractivePromptSection::SessionProgress,
                ArcInteractivePromptSection::CurrentObservation,
                ArcInteractivePromptSection::AvailableActions,
                ArcInteractivePromptSection::RetainedHistory,
                ArcInteractivePromptSection::SessionMemory,
                ArcInteractivePromptSection::ResumeSummary,
            ],
        }
    }
}

/// ARC observation retained for bounded prompt/memory context.
///
/// This deliberately excludes transport identifiers like `game_id` and `guid` so
/// context snapshots stay stable across local and remote runtime surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveContextFrame {
    pub frames: Vec<ArcFrameData>,
    pub game_state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    pub action: ArcAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<ArcActionKind>,
    #[serde(default)]
    pub full_reset: bool,
}

impl From<&ArcSessionFrame> for ArcInteractiveContextFrame {
    fn from(frame: &ArcSessionFrame) -> Self {
        Self {
            frames: frame.frames.clone(),
            game_state: frame.game_state,
            levels_completed: frame.levels_completed,
            win_levels: frame.win_levels,
            action: frame.action.clone(),
            available_actions: frame.available_actions.clone(),
            full_reset: frame.full_reset,
        }
    }
}

/// Current typed progress summary derived from the latest observation and budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveProgressState {
    pub game_state: ArcGameState,
    pub levels_completed: u16,
    pub win_levels: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub available_actions: Vec<ArcActionKind>,
    pub budget: ArcInteractiveBudgetState,
    #[serde(default)]
    pub terminal: bool,
}

/// One bounded session-memory entry retained across interactive turns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveMemoryEntry {
    pub step_index: u32,
    pub requested_action: ArcAction,
    pub result: ArcInteractiveActionResult,
    pub observation: ArcInteractiveContextFrame,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Value>,
}

/// Bounded, inspectable memory carried across interactive turns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveSessionMemory {
    pub policy_id: String,
    pub schema_version: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ArcInteractiveMemoryEntry>,
    #[serde(default)]
    pub omitted_entries: u32,
}

impl ArcInteractiveSessionMemory {
    /// Builds an empty memory buffer tied to one retention policy.
    #[must_use]
    pub fn empty(policy: &ArcInteractiveContextRetentionPolicy) -> Self {
        Self {
            policy_id: policy.policy_id.clone(),
            schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
            entries: Vec::new(),
            omitted_entries: 0,
        }
    }
}

/// Checkpoint-resumable retained context owned by the interactive runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractiveContextCheckpointState {
    pub retention_policy: ArcInteractiveContextRetentionPolicy,
    pub prompt_policy: ArcInteractivePromptPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<ArcInteractiveContextFrame>,
    #[serde(default)]
    pub omitted_history_steps: u32,
    pub memory: ArcInteractiveSessionMemory,
}

/// Machine-readable resume summary exposed in structured prompt plans.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractivePromptResumeSummary {
    pub checkpoint_id: String,
    pub next_step_index: u32,
    pub actions_taken: u32,
    pub terminal: bool,
    pub agent_name: String,
}

/// One typed structured prompt-plan section.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcInteractivePromptSectionView {
    SessionProgress {
        progress: ArcInteractiveProgressState,
    },
    CurrentObservation {
        frame: ArcInteractiveContextFrame,
    },
    AvailableActions {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        actions: Vec<ArcActionKind>,
    },
    RetainedHistory {
        omitted_steps: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        frames: Vec<ArcInteractiveContextFrame>,
    },
    SessionMemory {
        memory: ArcInteractiveSessionMemory,
    },
    ResumeSummary {
        summary: ArcInteractivePromptResumeSummary,
    },
}

/// Ordered, inspectable prompt-plan view derived from the typed session context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArcInteractivePromptPlan {
    pub policy_id: String,
    pub schema_version: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<ArcInteractivePromptSectionView>,
}

/// Policy validation failure for interactive context or prompt configuration.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArcInteractiveContextPolicyError {
    /// One policy field was blank after trimming.
    #[error("{field} must not be empty")]
    EmptyField {
        /// Human-readable field name.
        field: &'static str,
    },
    /// The prompt policy repeated the same structured section.
    #[error("interactive prompt policy must not repeat section `{section:?}`")]
    DuplicatePromptSection {
        /// Repeated section.
        section: ArcInteractivePromptSection,
    },
}

pub(crate) fn build_progress_state(
    frame: &ArcSessionFrame,
    budget: ArcInteractiveBudgetState,
) -> ArcInteractiveProgressState {
    ArcInteractiveProgressState {
        game_state: frame.game_state,
        levels_completed: frame.levels_completed,
        win_levels: frame.win_levels,
        available_actions: frame.available_actions.clone(),
        budget,
        terminal: matches!(frame.game_state, ArcGameState::Win | ArcGameState::GameOver),
    }
}

pub(crate) fn append_history_frame(
    history: &mut Vec<ArcInteractiveContextFrame>,
    omitted_history_steps: &mut u32,
    frame: ArcInteractiveContextFrame,
    policy: &ArcInteractiveContextRetentionPolicy,
) {
    if policy.history_window == 0 {
        *omitted_history_steps = omitted_history_steps.saturating_add(1);
        history.clear();
        return;
    }
    history.push(frame);
    let max_len = usize::try_from(policy.history_window).unwrap_or(usize::MAX);
    if history.len() > max_len {
        let excess = history.len().saturating_sub(max_len);
        history.drain(0..excess);
        *omitted_history_steps =
            omitted_history_steps.saturating_add(u32::try_from(excess).unwrap_or(u32::MAX));
    }
}

pub(crate) fn append_memory_entry(
    memory: &mut ArcInteractiveSessionMemory,
    entry: ArcInteractiveMemoryEntry,
    policy: &ArcInteractiveContextRetentionPolicy,
) {
    if policy.memory_window == 0 {
        memory.omitted_entries = memory.omitted_entries.saturating_add(1);
        memory.entries.clear();
        return;
    }
    memory.entries.push(entry);
    let max_len = usize::try_from(policy.memory_window).unwrap_or(usize::MAX);
    if memory.entries.len() > max_len {
        let excess = memory.entries.len().saturating_sub(max_len);
        memory.entries.drain(0..excess);
        memory.omitted_entries = memory
            .omitted_entries
            .saturating_add(u32::try_from(excess).unwrap_or(u32::MAX));
    }
}

pub(crate) fn build_prompt_plan(
    prompt_policy: &ArcInteractivePromptPolicy,
    progress: ArcInteractiveProgressState,
    latest_frame: &ArcSessionFrame,
    history: &[ArcInteractiveContextFrame],
    omitted_history_steps: u32,
    memory: &ArcInteractiveSessionMemory,
    resume_summary: Option<&ArcInteractivePromptResumeSummary>,
) -> ArcInteractivePromptPlan {
    let mut sections = Vec::with_capacity(prompt_policy.sections.len());
    for section in &prompt_policy.sections {
        match section {
            ArcInteractivePromptSection::SessionProgress => {
                sections.push(ArcInteractivePromptSectionView::SessionProgress {
                    progress: progress.clone(),
                });
            }
            ArcInteractivePromptSection::CurrentObservation => {
                sections.push(ArcInteractivePromptSectionView::CurrentObservation {
                    frame: ArcInteractiveContextFrame::from(latest_frame),
                });
            }
            ArcInteractivePromptSection::AvailableActions => {
                sections.push(ArcInteractivePromptSectionView::AvailableActions {
                    actions: latest_frame.available_actions.clone(),
                });
            }
            ArcInteractivePromptSection::RetainedHistory => {
                sections.push(ArcInteractivePromptSectionView::RetainedHistory {
                    omitted_steps: omitted_history_steps,
                    frames: history.to_vec(),
                });
            }
            ArcInteractivePromptSection::SessionMemory => {
                sections.push(ArcInteractivePromptSectionView::SessionMemory {
                    memory: memory.clone(),
                });
            }
            ArcInteractivePromptSection::ResumeSummary => {
                if let Some(summary) = resume_summary {
                    sections.push(ArcInteractivePromptSectionView::ResumeSummary {
                        summary: summary.clone(),
                    });
                }
            }
        }
    }
    ArcInteractivePromptPlan {
        policy_id: prompt_policy.policy_id.clone(),
        schema_version: INTERACTIVE_CONTEXT_SCHEMA_VERSION,
        sections,
    }
}

fn normalize_policy_field(
    field: &'static str,
    value: String,
) -> Result<String, ArcInteractiveContextPolicyError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ArcInteractiveContextPolicyError::EmptyField { field });
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use arc_core::{ArcFrameData, ArcInteractiveBudget, ArcTaskId};

    use super::*;

    fn sample_frame() -> ArcSessionFrame {
        ArcSessionFrame {
            game_id: ArcTaskId::new("context-fixture").expect("task id should validate"),
            guid: "guid-1".to_owned(),
            frames: vec![ArcFrameData::new(1, 1, vec![1]).expect("frame should validate")],
            game_state: ArcGameState::NotFinished,
            levels_completed: 0,
            win_levels: 2,
            action: ArcAction::Reset,
            available_actions: vec![ArcActionKind::Action1, ArcActionKind::Action2],
            full_reset: true,
        }
    }

    #[test]
    fn prompt_policy_rejects_duplicate_sections() {
        let error = ArcInteractivePromptPolicy::new(
            "dup-policy",
            vec![
                ArcInteractivePromptSection::CurrentObservation,
                ArcInteractivePromptSection::CurrentObservation,
            ],
        )
        .expect_err("duplicate sections should refuse");
        assert_eq!(
            error,
            ArcInteractiveContextPolicyError::DuplicatePromptSection {
                section: ArcInteractivePromptSection::CurrentObservation,
            }
        );
    }

    #[test]
    fn bounded_history_and_memory_track_omitted_entries() {
        let policy = ArcInteractiveContextRetentionPolicy::new("bounded", 2, 2)
            .expect("policy should validate");
        let frame = ArcInteractiveContextFrame::from(&sample_frame());
        let mut history = Vec::new();
        let mut omitted_history_steps = 0;
        append_history_frame(
            &mut history,
            &mut omitted_history_steps,
            frame.clone(),
            &policy,
        );
        append_history_frame(
            &mut history,
            &mut omitted_history_steps,
            frame.clone(),
            &policy,
        );
        append_history_frame(
            &mut history,
            &mut omitted_history_steps,
            frame.clone(),
            &policy,
        );
        assert_eq!(history.len(), 2);
        assert_eq!(omitted_history_steps, 1);

        let mut memory = ArcInteractiveSessionMemory::empty(&policy);
        let entry = ArcInteractiveMemoryEntry {
            step_index: 1,
            requested_action: ArcAction::Action1,
            result: ArcInteractiveActionResult::Executed {
                game_state: ArcGameState::NotFinished,
                levels_completed: 0,
                win_levels: 2,
                reset: None,
                terminal: false,
            },
            observation: frame,
            reasoning: Some(serde_json::json!({ "note": "bounded" })),
        };
        append_memory_entry(&mut memory, entry.clone(), &policy);
        append_memory_entry(&mut memory, entry.clone(), &policy);
        append_memory_entry(&mut memory, entry, &policy);
        assert_eq!(memory.entries.len(), 2);
        assert_eq!(memory.omitted_entries, 1);
    }

    #[test]
    fn prompt_plan_follows_policy_order() {
        let retention_policy = ArcInteractiveContextRetentionPolicy::new("prompt-order", 2, 2)
            .expect("policy should validate");
        let prompt_policy = ArcInteractivePromptPolicy::new(
            "prompt-order",
            vec![
                ArcInteractivePromptSection::CurrentObservation,
                ArcInteractivePromptSection::SessionMemory,
                ArcInteractivePromptSection::ResumeSummary,
            ],
        )
        .expect("prompt policy should validate");
        let frame = sample_frame();
        let progress = build_progress_state(
            &frame,
            ArcInteractiveBudget::new(4)
                .expect("budget should validate")
                .state(0)
                .expect("budget state should validate"),
        );
        let memory = ArcInteractiveSessionMemory::empty(&retention_policy);
        let plan = build_prompt_plan(
            &prompt_policy,
            progress,
            &frame,
            &[ArcInteractiveContextFrame::from(&frame)],
            0,
            &memory,
            Some(&ArcInteractivePromptResumeSummary {
                checkpoint_id: "checkpoint".to_owned(),
                next_step_index: 3,
                actions_taken: 2,
                terminal: false,
                agent_name: "agent".to_owned(),
            }),
        );
        assert_eq!(plan.sections.len(), 3);
        assert!(matches!(
            plan.sections[0],
            ArcInteractivePromptSectionView::CurrentObservation { .. }
        ));
        assert!(matches!(
            plan.sections[1],
            ArcInteractivePromptSectionView::SessionMemory { .. }
        ));
        assert!(matches!(
            plan.sections[2],
            ArcInteractivePromptSectionView::ResumeSummary { .. }
        ));
    }
}
