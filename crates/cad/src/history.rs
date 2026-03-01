use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::contracts::{CadAnalysis, CadWarning};
use crate::{CadError, CadResult};

/// Snapshot contract captured at each history transition boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHistorySnapshot {
    pub document_revision: u64,
    pub geometry_hash: String,
    pub stable_ids: BTreeMap<String, String>,
    pub warnings: Vec<CadWarning>,
    pub analysis: CadAnalysis,
}

/// Typed CAD command for history stack semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CadHistoryCommand {
    SetParameter {
        name: String,
        previous_value: String,
        next_value: String,
        gesture_id: Option<String>,
    },
    SetMaterial {
        previous_material_id: Option<String>,
        next_material_id: Option<String>,
    },
    ApplyIntent {
        intent_key: String,
        summary: String,
    },
}

impl CadHistoryCommand {
    fn can_coalesce_with(&self, prior: &Self) -> bool {
        match (self, prior) {
            (
                Self::SetParameter {
                    name: current_name,
                    gesture_id: current_gesture,
                    ..
                },
                Self::SetParameter {
                    name: prior_name,
                    gesture_id: prior_gesture,
                    ..
                },
            ) => current_name == prior_name && current_gesture == prior_gesture && current_gesture.is_some(),
            _ => false,
        }
    }

    fn merged_with_prior(&self, prior: &Self) -> Self {
        match (self, prior) {
            (
                Self::SetParameter {
                    name,
                    previous_value: _,
                    next_value,
                    gesture_id,
                },
                Self::SetParameter {
                    name: _,
                    previous_value,
                    next_value: _,
                    gesture_id: _,
                },
            ) => Self::SetParameter {
                name: name.clone(),
                previous_value: previous_value.clone(),
                next_value: next_value.clone(),
                gesture_id: gesture_id.clone(),
            },
            _ => self.clone(),
        }
    }
}

/// Single history transition entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHistoryEntry {
    pub command: CadHistoryCommand,
    pub before: CadHistorySnapshot,
    pub after: CadHistorySnapshot,
}

/// Result returned by undo/redo operations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHistoryTransition {
    pub command: CadHistoryCommand,
    pub snapshot: CadHistorySnapshot,
}

/// Deterministic session-scoped undo/redo stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CadHistoryStack {
    pub session_id: String,
    pub max_steps: usize,
    undo: Vec<CadHistoryEntry>,
    redo: Vec<CadHistoryEntry>,
}

impl CadHistoryStack {
    pub fn new(session_id: impl Into<String>, max_steps: usize) -> CadResult<Self> {
        if max_steps == 0 {
            return Err(CadError::InvalidPolicy {
                reason: "history max_steps must be > 0".to_string(),
            });
        }
        Ok(Self {
            session_id: session_id.into(),
            max_steps,
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn push_transition(
        &mut self,
        command: CadHistoryCommand,
        before: CadHistorySnapshot,
        after: CadHistorySnapshot,
    ) {
        let mut entry = CadHistoryEntry {
            command,
            before,
            after,
        };

        if let Some(previous) = self.undo.last()
            && entry.command.can_coalesce_with(&previous.command)
        {
            let prior = self
                .undo
                .pop()
                .expect("coalesce precondition requires prior entry");
            entry.command = entry.command.merged_with_prior(&prior.command);
            entry.before = prior.before;
        }

        self.undo.push(entry);
        if self.undo.len() > self.max_steps {
            let overflow = self.undo.len().saturating_sub(self.max_steps);
            self.undo.drain(0..overflow);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self) -> Option<CadHistoryTransition> {
        let entry = self.undo.pop()?;
        let snapshot = entry.before.clone();
        let command = entry.command.clone();
        self.redo.push(entry);
        Some(CadHistoryTransition { command, snapshot })
    }

    pub fn redo(&mut self) -> Option<CadHistoryTransition> {
        let entry = self.redo.pop()?;
        let snapshot = entry.after.clone();
        let command = entry.command.clone();
        self.undo.push(entry);
        Some(CadHistoryTransition { command, snapshot })
    }

    pub fn reset_session(&mut self, session_id: impl Into<String>) {
        self.session_id = session_id.into();
        self.undo.clear();
        self.redo.clear();
    }

    pub fn len_undo(&self) -> usize {
        self.undo.len()
    }

    pub fn len_redo(&self) -> usize {
        self.redo.len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};

    use super::{CadHistoryCommand, CadHistorySnapshot, CadHistoryStack};

    fn snapshot(
        revision: u64,
        hash: &str,
        semantic_ref: &str,
        warning_code: CadWarningCode,
    ) -> CadHistorySnapshot {
        CadHistorySnapshot {
            document_revision: revision,
            geometry_hash: hash.to_string(),
            stable_ids: BTreeMap::from([("rack_outer_face".to_string(), semantic_ref.to_string())]),
            warnings: vec![CadWarning {
                code: warning_code,
                severity: CadWarningSeverity::Warning,
                message: "warning".to_string(),
                remediation_hint: "fix".to_string(),
                semantic_refs: vec![semantic_ref.to_string()],
                metadata: BTreeMap::from([("deep_link".to_string(), "cad://feature/base".to_string())]),
            }],
            analysis: crate::contracts::CadAnalysis {
                document_revision: revision,
                variant_id: "variant.baseline".to_string(),
                material_id: Some("al-6061-t6".to_string()),
                volume_mm3: Some(1_000_000.0 + revision as f64),
                mass_kg: Some(2.5 + revision as f64 * 0.1),
                center_of_gravity_mm: Some([10.0, 20.0, 30.0]),
                estimated_cost_usd: Some(100.0 + revision as f64),
                max_deflection_mm: Some(0.4),
                objective_scores: BTreeMap::from([("weight".to_string(), 0.9)]),
            },
        }
    }

    #[test]
    fn parameter_gesture_edits_are_coalesced_into_single_history_step() {
        let mut history = CadHistoryStack::new("cad.session.1", 16).expect("history should init");
        history.push_transition(
            CadHistoryCommand::SetParameter {
                name: "vent_spacing_mm".to_string(),
                previous_value: "10".to_string(),
                next_value: "12".to_string(),
                gesture_id: Some("drag-1".to_string()),
            },
            snapshot(1, "hash-a", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(2, "hash-b", "rack_outer_face", CadWarningCode::SliverFace),
        );
        history.push_transition(
            CadHistoryCommand::SetParameter {
                name: "vent_spacing_mm".to_string(),
                previous_value: "12".to_string(),
                next_value: "14".to_string(),
                gesture_id: Some("drag-1".to_string()),
            },
            snapshot(2, "hash-b", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(3, "hash-c", "rack_outer_face", CadWarningCode::SliverFace),
        );

        assert_eq!(history.len_undo(), 1, "gesture edits should coalesce");
        let undo = history.undo().expect("undo should return transition");
        assert_eq!(undo.snapshot.geometry_hash, "hash-a");
        assert_eq!(history.len_redo(), 1);
    }

    #[test]
    fn multi_step_undo_redo_preserves_hashes_warnings_analysis_and_semantic_refs() {
        let mut history = CadHistoryStack::new("cad.session.2", 8).expect("history should init");
        history.push_transition(
            CadHistoryCommand::ApplyIntent {
                intent_key: "resize-vents".to_string(),
                summary: "resize vents".to_string(),
            },
            snapshot(1, "hash-1", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(2, "hash-2", "rack_outer_face", CadWarningCode::FilletFailed),
        );
        history.push_transition(
            CadHistoryCommand::SetMaterial {
                previous_material_id: Some("al-6061-t6".to_string()),
                next_material_id: Some("al-5052-h32".to_string()),
            },
            snapshot(2, "hash-2", "rack_outer_face", CadWarningCode::FilletFailed),
            snapshot(3, "hash-3", "rack_outer_face", CadWarningCode::SelfIntersection),
        );

        let undo_1 = history.undo().expect("undo step 1");
        let undo_2 = history.undo().expect("undo step 2");
        assert_eq!(undo_1.snapshot.geometry_hash, "hash-2");
        assert_eq!(undo_2.snapshot.geometry_hash, "hash-1");
        assert_eq!(
            undo_2.snapshot.stable_ids.get("rack_outer_face").map(String::as_str),
            Some("rack_outer_face")
        );
        assert_eq!(
            undo_1.snapshot.warnings[0].semantic_refs[0],
            "rack_outer_face"
        );
        assert_eq!(undo_1.snapshot.analysis.variant_id, "variant.baseline");

        let redo_1 = history.redo().expect("redo step 1");
        let redo_2 = history.redo().expect("redo step 2");
        assert_eq!(redo_1.snapshot.geometry_hash, "hash-2");
        assert_eq!(redo_2.snapshot.geometry_hash, "hash-3");
        assert_eq!(redo_2.snapshot.warnings[0].code, CadWarningCode::SelfIntersection);
    }

    #[test]
    fn history_is_session_scoped_and_reset_safe() {
        let mut history = CadHistoryStack::new("cad.session.3", 2).expect("history should init");
        history.push_transition(
            CadHistoryCommand::ApplyIntent {
                intent_key: "a".to_string(),
                summary: "a".to_string(),
            },
            snapshot(1, "h1", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(2, "h2", "rack_outer_face", CadWarningCode::SliverFace),
        );
        history.push_transition(
            CadHistoryCommand::ApplyIntent {
                intent_key: "b".to_string(),
                summary: "b".to_string(),
            },
            snapshot(2, "h2", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(3, "h3", "rack_outer_face", CadWarningCode::SliverFace),
        );
        history.push_transition(
            CadHistoryCommand::ApplyIntent {
                intent_key: "c".to_string(),
                summary: "c".to_string(),
            },
            snapshot(3, "h3", "rack_outer_face", CadWarningCode::SliverFace),
            snapshot(4, "h4", "rack_outer_face", CadWarningCode::SliverFace),
        );

        assert_eq!(history.len_undo(), 2, "max stack policy should evict oldest");
        history.reset_session("cad.session.4");
        assert_eq!(history.session_id, "cad.session.4");
        assert_eq!(history.len_undo(), 0);
        assert_eq!(history.len_redo(), 0);
    }
}
