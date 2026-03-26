use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::app_state::{
    PaneStatusAccess, TassadarLabPaneState, TassadarLabPlaybackState, TassadarLabReplayFamily,
    TassadarLabSourceMode, TassadarLabViewMode,
};
use psionic_serve::LocalTassadarLabService;
use serde::{Deserialize, Serialize};

const TASSADAR_LAB_SCHEMA_VERSION: u16 = 1;
const TASSADAR_LAB_STATE_FILENAME: &str = "tassadar-lab.json";
const DEFAULT_SPEED_MULTIPLIER: usize = 3;
const MIN_SPEED_MULTIPLIER: usize = 1;
const MAX_SPEED_MULTIPLIER: usize = 5;
const TRACE_CHUNK_SIZE_OPTIONS: [usize; 3] = [16, 32, 64];
const PLAYBACK_INTERVAL_MS: [u64; 5] = [700, 420, 240, 140, 80];

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedTassadarLabState {
    schema_version: u16,
    updated_at_epoch_ms: u64,
    playback_state: TassadarLabPlaybackState,
    playback_running: bool,
    selected_source_mode: TassadarLabSourceMode,
    selected_view: TassadarLabViewMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_replay_family: Option<TassadarLabReplayFamily>,
    show_help: bool,
    selected_replay: usize,
    selected_article_session_case: usize,
    selected_hybrid_workflow_case: usize,
    selected_update: usize,
    selected_readable_log_line: usize,
    selected_token_chunk: usize,
    selected_fact_line: usize,
    speed_multiplier: usize,
    trace_chunk_size: usize,
    local_events: Vec<String>,
    last_action: Option<String>,
    last_error: Option<String>,
}

pub(crate) fn ensure_loaded(pane_state: &mut TassadarLabPaneState) {
    if pane_state.persistence_loaded {
        return;
    }
    pane_state.persistence_loaded = true;

    let path = tassadar_lab_state_path();
    load_persisted_from_path(pane_state, &path);
    if pane_state.load_state == crate::app_state::PaneLoadState::Loading {
        if let Err(error) = reload_current_source(
            pane_state,
            format!(
                "Loaded {} // {}",
                pane_state.selected_source_mode.hero_label(),
                pane_state.current_source_label()
            ),
        ) {
            pane_state.push_local_event(format!("Load error // {error}"));
            let _ = pane_state.pane_set_error(error);
            pane_state.playback_running = false;
            pane_state.playback_state = TassadarLabPlaybackState::Paused;
        }
    }
}

fn load_persisted_from_path(pane_state: &mut TassadarLabPaneState, path: &Path) {
    let Some(mut persisted) = deserialize_persisted_state(path) else {
        return;
    };
    normalize_persisted_state(&mut persisted);

    let restored_update = persisted.selected_update;
    let restored_readable_log_line = persisted.selected_readable_log_line;
    let restored_token_chunk = persisted.selected_token_chunk;
    let restored_fact_line = persisted.selected_fact_line;
    let restored_playback_state = if persisted.playback_running {
        TassadarLabPlaybackState::Paused
    } else {
        persisted.playback_state
    };

    pane_state.selected_source_mode = persisted.selected_source_mode;
    pane_state.selected_view = persisted.selected_view;
    pane_state.show_help = persisted.show_help;
    pane_state.selected_replay = persisted.selected_replay;
    pane_state.selected_replay_family = persisted.selected_replay_family.unwrap_or_else(|| {
        pane_state
            .replay_catalog
            .get(pane_state.selected_replay)
            .map(|entry| TassadarLabReplayFamily::from_replay_id(entry.replay_id))
            .unwrap_or(TassadarLabReplayFamily::ArticleSessions)
    });
    pane_state.selected_article_session_case = persisted.selected_article_session_case;
    pane_state.selected_hybrid_workflow_case = persisted.selected_hybrid_workflow_case;
    pane_state.speed_multiplier = persisted.speed_multiplier;
    pane_state.trace_chunk_size = persisted.trace_chunk_size;
    pane_state.local_events = persisted.local_events;
    pane_state.last_action = persisted.last_action;
    pane_state.last_error = persisted.last_error;
    clamp_source_indices(pane_state);
    pane_state.align_replay_selection_to_family();
    pane_state.playback_running = false;
    pane_state.playback_state = restored_playback_state;
    pane_state.last_playback_tick_at = None;

    if let Err(error) = reload_current_source(
        pane_state,
        format!(
            "Restored {} // {}",
            pane_state.selected_source_mode.hero_label(),
            pane_state.current_source_label()
        ),
    ) {
        pane_state.push_local_event(format!("Restore error // {error}"));
        let _ = pane_state.pane_set_error(error);
        pane_state.playback_running = false;
        pane_state.playback_state = TassadarLabPlaybackState::Paused;
    } else {
        pane_state.selected_update =
            restored_update.min(pane_state.updates().len().saturating_sub(1));
        pane_state.selected_readable_log_line = restored_readable_log_line.min(
            pane_state
                .snapshot()
                .readable_log
                .as_ref()
                .map_or(0, |excerpt| excerpt.lines.len().saturating_sub(1)),
        );
        pane_state.selected_token_chunk =
            restored_token_chunk.min(pane_state.token_trace_chunk_count().saturating_sub(1));
        pane_state.selected_fact_line =
            restored_fact_line.min(pane_state.snapshot().fact_lines.len().saturating_sub(1));
        if restored_playback_state == TassadarLabPlaybackState::Paused {
            pane_state.pause_playback();
        } else {
            pane_state.playback_running = false;
            pane_state.playback_state = restored_playback_state;
        }
    }
}

pub(crate) fn select_view(pane_state: &mut TassadarLabPaneState, view: TassadarLabViewMode) {
    ensure_loaded(pane_state);
    pane_state.selected_view = view;
    let action = format!("Selected {} view", view.label());
    pane_state.push_local_event(action.clone());
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn cycle_view(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
    pane_state.cycle_view();
    let action = format!("Selected {} view", pane_state.selected_view.label());
    pane_state.push_local_event(action.clone());
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn select_source_mode(
    pane_state: &mut TassadarLabPaneState,
    source_mode: TassadarLabSourceMode,
) {
    ensure_loaded(pane_state);
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
    } else {
        persist_or_error(pane_state);
    }
}

pub(crate) fn select_replay_family(
    pane_state: &mut TassadarLabPaneState,
    replay_family: TassadarLabReplayFamily,
) {
    ensure_loaded(pane_state);
    pane_state.selected_source_mode = TassadarLabSourceMode::Replay;
    pane_state.set_replay_family(replay_family);
    if let Err(error) = reload_current_source(
        pane_state,
        format!(
            "Loaded {} // {}",
            replay_family.label(),
            pane_state.current_source_label()
        ),
    ) {
        pane_state.push_local_event(format!(
            "Load error // {} // {}",
            replay_family.label(),
            error
        ));
        let _ = pane_state.pane_set_error(error);
    } else {
        persist_or_error(pane_state);
    }
}

pub(crate) fn move_selected_replay_family(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    pane_state.selected_source_mode = TassadarLabSourceMode::Replay;
    pane_state.move_selected_replay_family(delta);
    if let Err(error) = reload_current_source(
        pane_state,
        format!(
            "Loaded {} // {}",
            pane_state.current_replay_family().label(),
            pane_state.current_source_label()
        ),
    ) {
        pane_state.push_local_event(format!(
            "Load error // {} // {}",
            pane_state.current_replay_family().label(),
            error
        ));
        let _ = pane_state.pane_set_error(error);
    } else {
        persist_or_error(pane_state);
    }
}

pub(crate) fn refresh_current_source(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
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
    } else {
        persist_or_error(pane_state);
    }
}

pub(crate) fn move_selected_replay(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
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
            } else {
                persist_or_error(pane_state);
            }
        }
        Err(error) => {
            let _ = pane_state.pane_set_error(error);
        }
    }
}

pub(crate) fn move_selected_update(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    pane_state.move_selected_update(delta);
    pane_state.pane_set_ready("Moved replay event focus");
    persist_or_error(pane_state);
}

pub(crate) fn move_selected_readable_log_line(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    pane_state.move_selected_readable_log_line(delta);
    pane_state.pane_set_ready("Moved readable-log focus");
    persist_or_error(pane_state);
}

pub(crate) fn move_selected_token_chunk(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    pane_state.move_selected_token_chunk(delta);
    pane_state.pane_set_ready("Moved token-trace focus");
    persist_or_error(pane_state);
}

pub(crate) fn move_selected_fact_line(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    pane_state.move_selected_fact_line(delta);
    pane_state.pane_set_ready("Moved evidence focus");
    persist_or_error(pane_state);
}

pub(crate) fn toggle_playback(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
    if pane_state.playback_running {
        pane_state.pause_playback();
        pane_state.push_local_event(String::from("Paused local Tassadar playback"));
        pane_state.pane_set_ready("Paused local Tassadar playback");
    } else {
        if pane_state.is_playback_complete() {
            pane_state.reset_focus();
        }
        pane_state.playback_running = true;
        pane_state.last_playback_tick_at = None;
        let action = format!(
            "{} local Tassadar playback",
            if pane_state.selected_update == 0 {
                "Started"
            } else {
                "Resumed"
            }
        );
        pane_state.push_local_event(action.clone());
        pane_state.pane_set_ready(action);
    }
    persist_or_error(pane_state);
}

pub(crate) fn pause_playback(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
    pane_state.pause_playback();
    pane_state.push_local_event(String::from("Paused local Tassadar playback"));
    pane_state.pane_set_ready("Paused local Tassadar playback");
    persist_or_error(pane_state);
}

pub(crate) fn reset_playback(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
    pane_state.reset_focus();
    pane_state.push_local_event(String::from("Reset local Tassadar playback cursor"));
    pane_state.pane_set_ready("Reset local Tassadar playback cursor");
    persist_or_error(pane_state);
}

pub(crate) fn adjust_speed(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    let next = (pane_state.speed_multiplier as isize + delta)
        .clamp(MIN_SPEED_MULTIPLIER as isize, MAX_SPEED_MULTIPLIER as isize)
        as usize;
    pane_state.speed_multiplier = next;
    let action = format!(
        "Set Tassadar playback speed to {}",
        pane_state.speed_multiplier
    );
    pane_state.push_local_event(action.clone());
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn set_speed_multiplier(pane_state: &mut TassadarLabPaneState, speed_multiplier: usize) {
    ensure_loaded(pane_state);
    pane_state.speed_multiplier =
        speed_multiplier.clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
    let action = format!(
        "Set Tassadar playback speed to {}",
        pane_state.speed_multiplier
    );
    pane_state.push_local_event(action.clone());
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn adjust_trace_chunk_size(pane_state: &mut TassadarLabPaneState, delta: isize) {
    ensure_loaded(pane_state);
    let current_index = TRACE_CHUNK_SIZE_OPTIONS
        .iter()
        .position(|size| *size == pane_state.trace_chunk_size)
        .unwrap_or(1);
    let next_index = (current_index as isize + delta)
        .clamp(0, TRACE_CHUNK_SIZE_OPTIONS.len().saturating_sub(1) as isize)
        as usize;
    pane_state.trace_chunk_size = TRACE_CHUNK_SIZE_OPTIONS[next_index];
    pane_state.selected_token_chunk = pane_state
        .selected_token_chunk
        .min(pane_state.token_trace_chunk_count().saturating_sub(1));
    let action = format!(
        "Set Tassadar token window to {} symbols",
        pane_state.trace_chunk_size
    );
    pane_state.push_local_event(action.clone());
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn toggle_help(pane_state: &mut TassadarLabPaneState) {
    ensure_loaded(pane_state);
    pane_state.show_help = !pane_state.show_help;
    let action = if pane_state.show_help {
        "Opened Tassadar Lab help"
    } else {
        "Closed Tassadar Lab help"
    };
    pane_state.push_local_event(String::from(action));
    pane_state.pane_set_ready(action);
    persist_or_error(pane_state);
}

pub(crate) fn background_tick(pane_state: &mut TassadarLabPaneState) -> bool {
    if !pane_state.persistence_loaded {
        return false;
    }
    if pane_state.load_state == crate::app_state::PaneLoadState::Loading {
        return false;
    }
    if !pane_state.playback_running {
        return false;
    }

    let now = Instant::now();
    let interval = playback_interval(pane_state.speed_multiplier);
    if pane_state
        .last_playback_tick_at
        .is_some_and(|last_tick| now.duration_since(last_tick) < interval)
    {
        return false;
    }
    pane_state.last_playback_tick_at = Some(now);

    if pane_state.is_playback_complete() {
        pane_state.pause_playback();
        pane_state.push_local_event(String::from("Completed local Tassadar playback"));
        pane_state.pane_set_ready("Completed local Tassadar playback");
        persist_or_error(pane_state);
        return true;
    }

    pane_state.selected_update =
        (pane_state.selected_update + 1).min(pane_state.updates().len().saturating_sub(1));
    pane_state.focus_current_update();
    if pane_state.is_playback_complete() {
        pane_state.pause_playback();
        pane_state.push_local_event(String::from("Completed local Tassadar playback"));
        pane_state.pane_set_ready("Completed local Tassadar playback");
    }
    persist_or_error(pane_state);
    true
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

fn playback_interval(speed_multiplier: usize) -> Duration {
    let index = speed_multiplier
        .clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER)
        .saturating_sub(1);
    Duration::from_millis(PLAYBACK_INTERVAL_MS[index])
}

fn clamp_source_indices(pane_state: &mut TassadarLabPaneState) {
    pane_state.selected_replay = pane_state
        .selected_replay
        .min(pane_state.replay_catalog.len().saturating_sub(1));
    pane_state.selected_article_session_case = pane_state
        .selected_article_session_case
        .min(pane_state.article_session_catalog.len().saturating_sub(1));
    pane_state.selected_hybrid_workflow_case = pane_state
        .selected_hybrid_workflow_case
        .min(pane_state.hybrid_workflow_catalog.len().saturating_sub(1));
}

fn persist_or_error(pane_state: &mut TassadarLabPaneState) {
    if let Err(error) = persist_state(pane_state) {
        pane_state.push_local_event(format!("Persist error // {error}"));
        let _ = pane_state.pane_set_error(error);
    }
}

fn persist_state(pane_state: &TassadarLabPaneState) -> Result<(), String> {
    persist_state_to_path(pane_state, &tassadar_lab_state_path())
}

fn persist_state_to_path(pane_state: &TassadarLabPaneState, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create Tassadar lab state dir: {error}"))?;
    }
    let persisted = PersistedTassadarLabState {
        schema_version: TASSADAR_LAB_SCHEMA_VERSION,
        updated_at_epoch_ms: current_epoch_ms(),
        playback_state: pane_state.playback_state,
        playback_running: pane_state.playback_running,
        selected_source_mode: pane_state.selected_source_mode,
        selected_view: pane_state.selected_view,
        selected_replay_family: Some(pane_state.selected_replay_family),
        show_help: pane_state.show_help,
        selected_replay: pane_state.selected_replay,
        selected_article_session_case: pane_state.selected_article_session_case,
        selected_hybrid_workflow_case: pane_state.selected_hybrid_workflow_case,
        selected_update: pane_state.selected_update,
        selected_readable_log_line: pane_state.selected_readable_log_line,
        selected_token_chunk: pane_state.selected_token_chunk,
        selected_fact_line: pane_state.selected_fact_line,
        speed_multiplier: pane_state.speed_multiplier,
        trace_chunk_size: pane_state.trace_chunk_size,
        local_events: pane_state.local_events.clone(),
        last_action: pane_state.last_action.clone(),
        last_error: pane_state.last_error.clone(),
    };
    let bytes = serde_json::to_vec_pretty(&persisted)
        .map_err(|error| format!("failed to encode Tassadar lab state: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("failed to write Tassadar lab state: {error}"))
}

fn deserialize_persisted_state(path: &Path) -> Option<PersistedTassadarLabState> {
    fs::read(path)
        .ok()
        .and_then(|raw| serde_json::from_slice::<PersistedTassadarLabState>(&raw).ok())
}

fn normalize_persisted_state(state: &mut PersistedTassadarLabState) {
    state.speed_multiplier = state
        .speed_multiplier
        .clamp(MIN_SPEED_MULTIPLIER, MAX_SPEED_MULTIPLIER);
    if !TRACE_CHUNK_SIZE_OPTIONS.contains(&state.trace_chunk_size) {
        state.trace_chunk_size = TRACE_CHUNK_SIZE_OPTIONS[1];
    }
    if state.playback_running {
        state.playback_running = false;
        state.playback_state = TassadarLabPlaybackState::Paused;
    }
    if state.local_events.len() > 12 {
        let excess = state.local_events.len() - 12;
        state.local_events.drain(0..excess);
    }
}

fn tassadar_lab_state_path() -> PathBuf {
    crate::runtime_log::autopilot_log_dir().join(TASSADAR_LAB_STATE_FILENAME)
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarLabSourceMode, TassadarLabViewMode, adjust_trace_chunk_size, background_tick,
        load_persisted_from_path, move_selected_fact_line, move_selected_readable_log_line,
        move_selected_replay, move_selected_replay_family, move_selected_update,
        persist_state_to_path, refresh_current_source, reset_playback, select_replay_family,
        select_source_mode, select_view, set_speed_multiplier, toggle_help, toggle_playback,
    };
    use crate::app_state::{
        TassadarLabPaneState, TassadarLabPlaybackState, TassadarLabReplayFamily,
    };
    use psionic_serve::TassadarLabSourceKind;
    use tempfile::tempdir;

    fn isolated_state() -> TassadarLabPaneState {
        let mut state = TassadarLabPaneState::default();
        state.persistence_loaded = true;
        state
    }

    #[test]
    fn live_article_session_loads_and_accumulates_local_events() {
        let mut state = isolated_state();
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
        let mut state = isolated_state();
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
        let mut state = isolated_state();
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

    #[test]
    fn persisted_state_restores_focus_and_normalizes_active_playback() {
        let temp = tempdir().expect("tempdir");
        let state_path = temp.path().join("tassadar-lab.json");

        let mut state = isolated_state();
        select_source_mode(&mut state, TassadarLabSourceMode::LiveArticleHybridWorkflow);
        move_selected_replay(&mut state, 1);
        select_view(&mut state, TassadarLabViewMode::Evidence);
        move_selected_update(&mut state, -1);
        move_selected_readable_log_line(&mut state, 1);
        adjust_trace_chunk_size(&mut state, -1);
        move_selected_fact_line(&mut state, 1);
        set_speed_multiplier(&mut state, 5);
        toggle_help(&mut state);
        toggle_playback(&mut state);

        let expected_update = state.selected_update;
        let expected_readable_log_line = state.selected_readable_log_line;
        let expected_token_chunk = state.selected_token_chunk;
        let expected_fact_line = state.selected_fact_line;

        persist_state_to_path(&state, &state_path).expect("persisted state");

        let mut restored = isolated_state();
        load_persisted_from_path(&mut restored, &state_path);

        assert_eq!(
            restored.selected_source_mode,
            TassadarLabSourceMode::LiveArticleHybridWorkflow
        );
        assert_eq!(
            restored.current_source_label(),
            "Fallback branch-heavy hybrid workflow"
        );
        assert_eq!(restored.selected_view, TassadarLabViewMode::Evidence);
        assert_eq!(restored.speed_multiplier, 5);
        assert_eq!(restored.trace_chunk_size, 16);
        assert!(restored.show_help);
        assert_eq!(restored.selected_update, expected_update);
        assert_eq!(
            restored.selected_readable_log_line,
            expected_readable_log_line
        );
        assert_eq!(restored.selected_token_chunk, expected_token_chunk);
        assert_eq!(restored.selected_fact_line, expected_fact_line);
        assert!(!restored.playback_running);
        assert_eq!(restored.playback_state, TassadarLabPlaybackState::Paused);
        assert!(
            restored
                .local_events
                .iter()
                .any(|line| line.contains("Restored Live hybrid workflow"))
        );
    }

    #[test]
    fn background_playback_tick_advances_and_completes() {
        let mut state = isolated_state();
        reset_playback(&mut state);

        assert_eq!(state.selected_update, 0);
        assert_eq!(state.playback_state, TassadarLabPlaybackState::Replay);

        toggle_playback(&mut state);
        assert!(state.playback_running);
        assert_eq!(state.playback_button_label(), "Pause");

        assert!(background_tick(&mut state));
        assert!(state.selected_update > 0);

        state.selected_update = state.updates().len().saturating_sub(1);
        state.playback_running = true;
        state.last_playback_tick_at = None;

        assert!(background_tick(&mut state));
        assert!(!state.playback_running);
        assert_eq!(state.playback_state, TassadarLabPlaybackState::Paused);
        assert!(
            state
                .local_events
                .iter()
                .any(|line| line.contains("Completed local Tassadar playback"))
        );
    }

    #[test]
    fn replay_family_selection_switches_between_canonical_artifact_roots() {
        let mut state = isolated_state();
        select_replay_family(&mut state, TassadarLabReplayFamily::CompiledClosure);

        assert_eq!(state.selected_source_mode, TassadarLabSourceMode::Replay);
        assert_eq!(
            state.current_replay_family(),
            TassadarLabReplayFamily::CompiledClosure
        );
        assert_eq!(state.source_case_count(), 1);
        assert_eq!(
            state.current_source_label(),
            "Compiled article closure report"
        );

        move_selected_replay_family(&mut state, 1);
        assert_eq!(
            state.current_replay_family(),
            TassadarLabReplayFamily::Acceptance
        );
        assert_eq!(state.current_source_label(), "Tassadar acceptance report");
    }
}
