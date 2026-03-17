use crate::app_state::{PaneStatusAccess, TassadarLabPaneState, TassadarLabViewMode};

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

pub(crate) fn move_selected_replay(pane_state: &mut TassadarLabPaneState, delta: isize) {
    match pane_state.move_selected_replay(delta) {
        Ok(()) => pane_state.pane_clear_error(),
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
