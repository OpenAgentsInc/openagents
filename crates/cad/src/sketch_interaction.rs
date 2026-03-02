use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SketchDrawTool {
    Line,
    Rectangle,
    Circle,
}

impl SketchDrawTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Rectangle => "rectangle",
            Self::Circle => "circle",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SketchConstraintShortcut {
    Horizontal,
    Vertical,
}

impl SketchConstraintShortcut {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SketchShortcut {
    EnterSketchMode,
    LineTool,
    RectangleTool,
    CircleTool,
    HorizontalConstraint,
    VerticalConstraint,
    FinishCurrentShape,
    Escape,
}

impl SketchShortcut {
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::EnterSketchMode => "sketch.enter",
            Self::LineTool => "sketch.tool.line",
            Self::RectangleTool => "sketch.tool.rectangle",
            Self::CircleTool => "sketch.tool.circle",
            Self::HorizontalConstraint => "sketch.constraint.horizontal",
            Self::VerticalConstraint => "sketch.constraint.vertical",
            Self::FinishCurrentShape => "sketch.shape.finish",
            Self::Escape => "sketch.escape",
        }
    }

    pub fn key_binding(self) -> &'static str {
        match self {
            Self::EnterSketchMode => "S",
            Self::LineTool => "L",
            Self::RectangleTool => "R",
            Self::CircleTool => "C",
            Self::HorizontalConstraint => "H",
            Self::VerticalConstraint => "V",
            Self::FinishCurrentShape => "Enter",
            Self::Escape => "Escape",
        }
    }
}

pub const SKETCH_SHORTCUT_SEQUENCE: [SketchShortcut; 8] = [
    SketchShortcut::EnterSketchMode,
    SketchShortcut::LineTool,
    SketchShortcut::RectangleTool,
    SketchShortcut::CircleTool,
    SketchShortcut::HorizontalConstraint,
    SketchShortcut::VerticalConstraint,
    SketchShortcut::FinishCurrentShape,
    SketchShortcut::Escape,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SketchInteractionState {
    pub has_parts: bool,
    pub sketch_active: bool,
    pub face_selection_mode: bool,
    pub pending_exit_confirmation: bool,
    pub active_tool: SketchDrawTool,
    pub pending_points: usize,
    pub segment_count: usize,
    pub selected_line_count: usize,
}

impl Default for SketchInteractionState {
    fn default() -> Self {
        Self {
            has_parts: false,
            sketch_active: false,
            face_selection_mode: false,
            pending_exit_confirmation: false,
            active_tool: SketchDrawTool::Rectangle,
            pending_points: 0,
            segment_count: 0,
            selected_line_count: 0,
        }
    }
}

impl SketchInteractionState {
    pub fn with_has_parts(has_parts: bool) -> Self {
        Self {
            has_parts,
            ..Self::default()
        }
    }

    pub fn with_geometry_context(
        mut self,
        segment_count: usize,
        pending_points: usize,
        selected_line_count: usize,
    ) -> Self {
        self.segment_count = segment_count;
        self.pending_points = pending_points;
        self.selected_line_count = selected_line_count;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SketchInteractionCommand {
    EnterFaceSelectionMode,
    EnterSketchModeOnPresetXY,
    EnterSketchModeOnFace,
    SetDrawTool(SketchDrawTool),
    ApplyConstraint(SketchConstraintShortcut),
    RunConstraintSolver,
    FinishCurrentShape,
    CancelCurrentShape,
    RequestExitConfirmation,
    CancelExitConfirmation,
    ExitSketchMode,
    CancelFaceSelection,
}

impl SketchInteractionCommand {
    pub fn stable_code(self) -> String {
        match self {
            Self::EnterFaceSelectionMode => "SKETCH-CMD-ENTER-FACE-SELECTION".to_string(),
            Self::EnterSketchModeOnPresetXY => "SKETCH-CMD-ENTER-SKETCH-XY".to_string(),
            Self::EnterSketchModeOnFace => "SKETCH-CMD-ENTER-SKETCH-FACE".to_string(),
            Self::SetDrawTool(tool) => format!("SKETCH-CMD-SET-TOOL-{}", tool.as_str()),
            Self::ApplyConstraint(constraint) => {
                format!("SKETCH-CMD-APPLY-{}", constraint.as_str())
            }
            Self::RunConstraintSolver => "SKETCH-CMD-RUN-SOLVER".to_string(),
            Self::FinishCurrentShape => "SKETCH-CMD-FINISH-SHAPE".to_string(),
            Self::CancelCurrentShape => "SKETCH-CMD-CANCEL-SHAPE".to_string(),
            Self::RequestExitConfirmation => "SKETCH-CMD-REQUEST-EXIT".to_string(),
            Self::CancelExitConfirmation => "SKETCH-CMD-CANCEL-EXIT".to_string(),
            Self::ExitSketchMode => "SKETCH-CMD-EXIT".to_string(),
            Self::CancelFaceSelection => "SKETCH-CMD-CANCEL-FACE-SELECTION".to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SketchInteractionTransition {
    pub next_state: SketchInteractionState,
    pub commands: Vec<SketchInteractionCommand>,
}

pub fn apply_shortcut(
    state: &SketchInteractionState,
    shortcut: SketchShortcut,
) -> SketchInteractionTransition {
    let mut next = state.clone();
    let mut commands = Vec::<SketchInteractionCommand>::new();

    match shortcut {
        SketchShortcut::EnterSketchMode => {
            if !state.sketch_active && !state.face_selection_mode {
                if state.has_parts {
                    next.face_selection_mode = true;
                    commands.push(SketchInteractionCommand::EnterFaceSelectionMode);
                } else {
                    next.sketch_active = true;
                    next.pending_exit_confirmation = false;
                    next.pending_points = 0;
                    next.selected_line_count = 0;
                    commands.push(SketchInteractionCommand::EnterSketchModeOnPresetXY);
                }
            }
        }
        SketchShortcut::LineTool => {
            if state.sketch_active {
                next.active_tool = SketchDrawTool::Line;
                next.pending_points = 0;
                commands.push(SketchInteractionCommand::SetDrawTool(SketchDrawTool::Line));
            }
        }
        SketchShortcut::RectangleTool => {
            if state.sketch_active {
                next.active_tool = SketchDrawTool::Rectangle;
                next.pending_points = 0;
                commands.push(SketchInteractionCommand::SetDrawTool(
                    SketchDrawTool::Rectangle,
                ));
            }
        }
        SketchShortcut::CircleTool => {
            if state.sketch_active {
                next.active_tool = SketchDrawTool::Circle;
                next.pending_points = 0;
                commands.push(SketchInteractionCommand::SetDrawTool(
                    SketchDrawTool::Circle,
                ));
            }
        }
        SketchShortcut::HorizontalConstraint => {
            if state.sketch_active && state.selected_line_count == 1 {
                next.selected_line_count = 0;
                commands.push(SketchInteractionCommand::ApplyConstraint(
                    SketchConstraintShortcut::Horizontal,
                ));
                commands.push(SketchInteractionCommand::RunConstraintSolver);
            }
        }
        SketchShortcut::VerticalConstraint => {
            if state.sketch_active && state.selected_line_count == 1 {
                next.selected_line_count = 0;
                commands.push(SketchInteractionCommand::ApplyConstraint(
                    SketchConstraintShortcut::Vertical,
                ));
                commands.push(SketchInteractionCommand::RunConstraintSolver);
            }
        }
        SketchShortcut::FinishCurrentShape => {
            if state.sketch_active && state.pending_points > 0 {
                next.pending_points = 0;
                commands.push(SketchInteractionCommand::FinishCurrentShape);
            }
        }
        SketchShortcut::Escape => {
            if state.face_selection_mode {
                next.face_selection_mode = false;
                commands.push(SketchInteractionCommand::CancelFaceSelection);
            } else if state.sketch_active {
                if state.pending_points > 0 {
                    next.pending_points = 0;
                    commands.push(SketchInteractionCommand::CancelCurrentShape);
                } else if state.pending_exit_confirmation {
                    next.pending_exit_confirmation = false;
                    commands.push(SketchInteractionCommand::CancelExitConfirmation);
                } else if state.segment_count == 0 {
                    next.sketch_active = false;
                    next.pending_exit_confirmation = false;
                    next.pending_points = 0;
                    next.selected_line_count = 0;
                    commands.push(SketchInteractionCommand::ExitSketchMode);
                } else {
                    next.pending_exit_confirmation = true;
                    commands.push(SketchInteractionCommand::RequestExitConfirmation);
                }
            }
        }
    }

    SketchInteractionTransition {
        next_state: next,
        commands,
    }
}

pub fn apply_face_selection_confirm(state: &SketchInteractionState) -> SketchInteractionTransition {
    let mut next = state.clone();
    let mut commands = Vec::<SketchInteractionCommand>::new();
    if state.face_selection_mode {
        next.face_selection_mode = false;
        next.sketch_active = true;
        next.pending_exit_confirmation = false;
        next.pending_points = 0;
        next.selected_line_count = 0;
        commands.push(SketchInteractionCommand::EnterSketchModeOnFace);
    }
    SketchInteractionTransition {
        next_state: next,
        commands,
    }
}

pub fn apply_exit_confirm(state: &SketchInteractionState) -> SketchInteractionTransition {
    let mut next = state.clone();
    let mut commands = Vec::<SketchInteractionCommand>::new();
    if state.sketch_active && state.pending_exit_confirmation {
        next.sketch_active = false;
        next.pending_exit_confirmation = false;
        next.pending_points = 0;
        next.selected_line_count = 0;
        next.segment_count = 0;
        commands.push(SketchInteractionCommand::ExitSketchMode);
    }
    SketchInteractionTransition {
        next_state: next,
        commands,
    }
}

pub fn apply_exit_cancel(state: &SketchInteractionState) -> SketchInteractionTransition {
    let mut next = state.clone();
    let mut commands = Vec::<SketchInteractionCommand>::new();
    if state.pending_exit_confirmation {
        next.pending_exit_confirmation = false;
        commands.push(SketchInteractionCommand::CancelExitConfirmation);
    }
    SketchInteractionTransition {
        next_state: next,
        commands,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SketchDrawTool, SketchInteractionCommand, SketchInteractionState, SketchShortcut,
        apply_exit_cancel, apply_exit_confirm, apply_face_selection_confirm, apply_shortcut,
    };

    #[test]
    fn sketch_mode_entry_shortcut_uses_face_selection_when_parts_exist() {
        let idle_with_parts = SketchInteractionState::with_has_parts(true);
        let transition = apply_shortcut(&idle_with_parts, SketchShortcut::EnterSketchMode);
        assert!(!transition.next_state.sketch_active);
        assert!(transition.next_state.face_selection_mode);
        assert_eq!(
            transition.commands,
            vec![SketchInteractionCommand::EnterFaceSelectionMode]
        );

        let face_pick = apply_face_selection_confirm(&transition.next_state);
        assert!(face_pick.next_state.sketch_active);
        assert!(!face_pick.next_state.face_selection_mode);
        assert_eq!(
            face_pick.commands,
            vec![SketchInteractionCommand::EnterSketchModeOnFace]
        );
    }

    #[test]
    fn draw_tool_shortcuts_reset_pending_points_when_sketch_active() {
        let active = SketchInteractionState {
            has_parts: false,
            sketch_active: true,
            face_selection_mode: false,
            pending_exit_confirmation: false,
            active_tool: SketchDrawTool::Rectangle,
            pending_points: 2,
            segment_count: 0,
            selected_line_count: 0,
        };
        let line = apply_shortcut(&active, SketchShortcut::LineTool);
        assert_eq!(line.next_state.active_tool, SketchDrawTool::Line);
        assert_eq!(line.next_state.pending_points, 0);
        assert_eq!(
            line.commands,
            vec![SketchInteractionCommand::SetDrawTool(SketchDrawTool::Line)]
        );
    }

    #[test]
    fn constraint_shortcuts_require_single_selected_line_and_run_solver() {
        let active = SketchInteractionState {
            has_parts: false,
            sketch_active: true,
            face_selection_mode: false,
            pending_exit_confirmation: false,
            active_tool: SketchDrawTool::Line,
            pending_points: 0,
            segment_count: 1,
            selected_line_count: 1,
        };
        let horizontal = apply_shortcut(&active, SketchShortcut::HorizontalConstraint);
        assert_eq!(horizontal.next_state.selected_line_count, 0);
        assert_eq!(
            horizontal
                .commands
                .iter()
                .map(|command| command.stable_code())
                .collect::<Vec<_>>(),
            vec!["SKETCH-CMD-APPLY-horizontal", "SKETCH-CMD-RUN-SOLVER"]
        );
    }

    #[test]
    fn escape_shortcut_matches_editing_flow_exit_rules() {
        let active_with_points = SketchInteractionState {
            has_parts: false,
            sketch_active: true,
            face_selection_mode: false,
            pending_exit_confirmation: false,
            active_tool: SketchDrawTool::Line,
            pending_points: 1,
            segment_count: 3,
            selected_line_count: 0,
        };
        let cancel_shape = apply_shortcut(&active_with_points, SketchShortcut::Escape);
        assert_eq!(cancel_shape.next_state.pending_points, 0);
        assert_eq!(
            cancel_shape.commands,
            vec![SketchInteractionCommand::CancelCurrentShape]
        );

        let request_exit = apply_shortcut(&cancel_shape.next_state, SketchShortcut::Escape);
        assert!(request_exit.next_state.pending_exit_confirmation);
        assert_eq!(
            request_exit.commands,
            vec![SketchInteractionCommand::RequestExitConfirmation]
        );

        let keep_editing = apply_exit_cancel(&request_exit.next_state);
        assert!(!keep_editing.next_state.pending_exit_confirmation);
        assert_eq!(
            keep_editing.commands,
            vec![SketchInteractionCommand::CancelExitConfirmation]
        );

        let request_exit_again = apply_shortcut(&keep_editing.next_state, SketchShortcut::Escape);
        let confirm_exit = apply_exit_confirm(&request_exit_again.next_state);
        assert!(!confirm_exit.next_state.sketch_active);
        assert_eq!(confirm_exit.next_state.segment_count, 0);
        assert_eq!(
            confirm_exit.commands,
            vec![SketchInteractionCommand::ExitSketchMode]
        );
    }
}
