use crate::app_state::{CadDemoPaneState, PaneLoadState, RenderState};
use crate::pane_system::CadDemoPaneAction;

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    apply_cad_demo_action(&mut state.cad_demo, action)
}

fn apply_cad_demo_action(state: &mut CadDemoPaneState, action: CadDemoPaneAction) -> bool {
    match action {
        CadDemoPaneAction::Noop => false,
        CadDemoPaneAction::CycleVariant => {
            if state.variant_ids.is_empty() {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some("CAD demo has no registered variants".to_string());
                state.last_action = Some("Variant cycle rejected: no variants available".to_string());
                return true;
            }

            let current_index = state
                .variant_ids
                .iter()
                .position(|variant| variant == &state.active_variant_id)
                .unwrap_or(0);
            let next_index = (current_index + 1) % state.variant_ids.len();
            state.active_variant_id = state.variant_ids[next_index].clone();
            state.document_revision = state.document_revision.saturating_add(1);
            state.load_state = PaneLoadState::Ready;
            state.last_error = None;
            state.last_action = Some(format!(
                "CAD demo variant switched to {}",
                state.active_variant_id
            ));
            true
        }
        CadDemoPaneAction::ResetSession => {
            let mut reset = CadDemoPaneState::default();
            reset.last_action = Some("CAD demo session reset".to_string());
            *state = reset;
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_cad_demo_action;
    use crate::app_state::CadDemoPaneState;
    use crate::pane_system::CadDemoPaneAction;

    #[test]
    fn noop_action_is_stable_no_op() {
        let mut state = CadDemoPaneState::default();
        let baseline = state.document_revision;
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::Noop);
        assert!(!changed);
        assert_eq!(state.document_revision, baseline);
        assert_eq!(state.active_variant_id, "variant.baseline");
    }

    #[test]
    fn cycle_variant_updates_revision_and_selection() {
        let mut state = CadDemoPaneState::default();
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        assert!(changed);
        assert_eq!(state.document_revision, 1);
        assert_eq!(state.active_variant_id, "variant.lightweight");
    }

    #[test]
    fn reset_restores_default_session_state() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::ResetSession);
        assert!(changed);
        assert_eq!(state.document_revision, 0);
        assert_eq!(state.active_variant_id, "variant.baseline");
        assert_eq!(state.session_id, "cad.session.local");
        assert_eq!(
            state.last_action.as_deref(),
            Some("CAD demo session reset")
        );
    }
}
