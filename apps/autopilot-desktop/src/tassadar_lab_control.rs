use crate::app_state::{
    PaneStatusAccess, TassadarLabPaneState, TassadarLabPlaybackState, TassadarLabSourceMode,
    TassadarLabViewMode,
};
use psionic_serve::LocalTassadarLabService;

pub(crate) fn select_view(pane_state: &mut TassadarLabPaneState, view: TassadarLabViewMode) {
    pane_state.selected_view = view;
    pane_state.pane_set_ready(format!("Selected {} view", view.label()));
}

pub(crate) fn cycle_view(pane_state: &mut TassadarLabPaneState) {
    pane_state.cycle_view();
    pane_state.pane_set_ready(format!(
        "Selected {} view",
        pane_state.selected_view.label()
    ));
}

pub(crate) fn select_source_mode(
    pane_state: &mut TassadarLabPaneState,
    source_mode: TassadarLabSourceMode,
) {
    pane_state.set_source_mode(source_mode);
    if let Err(error) = reload_current_source(
        pane_state,
        format!(
            "Loaded {} // {}",
            source_mode.hero_label(),
            pane_state.current_source_label()
        ),
    ) {
        pane_state.push_local_event(format!(
            "Load error // {} // {}",
            source_mode.hero_label(),
            error
        ));
        let _ = pane_state.pane_set_error(error);
    }
}

pub(crate) fn refresh_current_source(pane_state: &mut TassadarLabPaneState) {
    let source_mode = pane_state.selected_source_mode;
    if let Err(error) = reload_current_source(
        pane_state,
        format!(
            "Refreshed {} // {}",
            source_mode.hero_label(),
            pane_state.current_source_label()
        ),
    ) {
        pane_state.push_local_event(format!(
            "Refresh error // {} // {}",
            source_mode.hero_label(),
            error
        ));
        let _ = pane_state.pane_set_error(error);
    }
}

pub(crate) fn move_selected_replay(pane_state: &mut TassadarLabPaneState, delta: isize) {
    match pane_state.move_selected_source_item(delta) {
        Ok(()) => {
            if let Err(error) = reload_current_source(
                pane_state,
                format!(
                    "Loaded {} // {}",
                    pane_state.selected_source_mode.hero_label(),
                    pane_state.current_source_label()
                ),
            ) {
                pane_state.push_local_event(format!("Load error // {}", error));
                let _ = pane_state.pane_set_error(error);
            }
        }
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn move_selected_update(pane_state: &mut TassadarLabPaneState, delta: isize) {
    pane_state.move_selected_update(delta);
    pane_state.pane_set_ready("Moved replay event focus");
}

pub(crate) fn move_selected_readable_log_line(pane_state: &mut TassadarLabPaneState, delta: isize) {
    pane_state.move_selected_readable_log_line(delta);
    pane_state.pane_set_ready("Moved readable-log focus");
}

pub(crate) fn move_selected_token_chunk(pane_state: &mut TassadarLabPaneState, delta: isize) {
    pane_state.move_selected_token_chunk(delta);
    pane_state.pane_set_ready("Moved token-trace focus");
}

pub(crate) fn move_selected_fact_line(pane_state: &mut TassadarLabPaneState, delta: isize) {
    pane_state.move_selected_fact_line(delta);
    pane_state.pane_set_ready("Moved evidence focus");
}

pub(crate) fn toggle_help(pane_state: &mut TassadarLabPaneState) {
    pane_state.show_help = !pane_state.show_help;
    pane_state.pane_set_ready(if pane_state.show_help {
        "Opened Tassadar Lab help"
    } else {
        "Closed Tassadar Lab help"
    });
}

fn reload_current_source(
    pane_state: &mut TassadarLabPaneState,
    action: impl Into<String>,
) -> Result<(), String> {
    let prepared_view = LocalTassadarLabService::new()
        .prepare(&pane_state.current_source_request())
        .map_err(|error| error.to_string())?;
    let playback_state = match pane_state.selected_source_mode {
        TassadarLabSourceMode::Replay => TassadarLabPlaybackState::Replay,
        TassadarLabSourceMode::LiveArticleSession
        | TassadarLabSourceMode::LiveArticleHybridWorkflow => TassadarLabPlaybackState::Live,
    };
    pane_state.apply_prepared_view(prepared_view, playback_state, action);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarLabSourceMode, move_selected_replay, refresh_current_source, select_source_mode,
    };
    use crate::app_state::TassadarLabPaneState;
    use psionic_serve::TassadarLabSourceKind;

    #[test]
    fn live_article_session_loads_and_accumulates_local_events() {
        let mut state = TassadarLabPaneState::default();
        select_source_mode(&mut state, TassadarLabSourceMode::LiveArticleSession);

        assert_eq!(
            state.snapshot().source_kind,
            TassadarLabSourceKind::LiveArticleSession
        );
        assert!(state.snapshot().proof_identity.is_some());
        assert!(
            state
                .local_events
                .iter()
                .any(|line| line.contains("Loaded Live article session"))
        );
    }

    #[test]
    fn live_hybrid_workflow_refreshes_and_keeps_route_truth() {
        let mut state = TassadarLabPaneState::default();
        select_source_mode(&mut state, TassadarLabSourceMode::LiveArticleHybridWorkflow);
        refresh_current_source(&mut state);

        assert_eq!(
            state.snapshot().source_kind,
            TassadarLabSourceKind::LiveArticleHybridWorkflow
        );
        assert!(state.snapshot().route_state_label.is_some());
        assert!(
            state
                .local_events
                .iter()
                .any(|line| line.contains("Refreshed Live hybrid workflow"))
        );
    }

    #[test]
    fn case_navigation_uses_active_source_mode() {
        let mut state = TassadarLabPaneState::default();
        select_source_mode(&mut state, TassadarLabSourceMode::LiveArticleSession);
        move_selected_replay(&mut state, 1);

        assert_eq!(
            state.snapshot().source_kind,
            TassadarLabSourceKind::LiveArticleSession
        );
        assert_eq!(
            state.current_source_label(),
            "Fallback branch-heavy article session"
        );
        assert!(
            state
                .snapshot()
                .route_detail
                .as_deref()
                .is_some_and(|detail| !detail.is_empty())
        );
    }
}
