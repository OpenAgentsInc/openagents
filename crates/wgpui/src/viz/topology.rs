use crate::{Hsla, theme};

use super::theme as viz_theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyNodeState {
    Idle,
    Active,
    Warning,
    Error,
}

pub fn node_state_color(state: TopologyNodeState) -> Hsla {
    match state {
        TopologyNodeState::Idle => theme::text::MUTED,
        TopologyNodeState::Active => viz_theme::state::ACTIVE,
        TopologyNodeState::Warning => viz_theme::state::WARNING,
        TopologyNodeState::Error => viz_theme::state::ERROR,
    }
}
