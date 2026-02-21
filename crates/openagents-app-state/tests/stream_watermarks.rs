use openagents_app_state::{AppAction, AppState, apply_action};

#[test]
fn topic_watermark_update_is_monotonic_and_idempotent() {
    let mut state = AppState::default();

    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarkUpdated {
            topic: "runtime.codex_worker_events".to_string(),
            watermark: 42,
        },
    );
    assert_eq!(
        state
            .stream
            .topic_watermarks
            .get("runtime.codex_worker_events"),
        Some(&42)
    );
    assert_eq!(state.stream.last_seq, Some(42));

    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarkUpdated {
            topic: "runtime.codex_worker_events".to_string(),
            watermark: 41,
        },
    );
    assert_eq!(
        state
            .stream
            .topic_watermarks
            .get("runtime.codex_worker_events"),
        Some(&42)
    );
    assert_eq!(state.stream.last_seq, Some(42));

    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarkUpdated {
            topic: "runtime.codex_worker_events".to_string(),
            watermark: 42,
        },
    );
    assert_eq!(
        state
            .stream
            .topic_watermarks
            .get("runtime.codex_worker_events"),
        Some(&42)
    );
    assert_eq!(state.stream.last_seq, Some(42));
}

#[test]
fn topic_watermark_reset_recomputes_last_seq() {
    let mut state = AppState::default();
    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarkUpdated {
            topic: "runtime.codex_worker_events".to_string(),
            watermark: 120,
        },
    );
    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarkUpdated {
            topic: "runtime.codex_worker_summaries".to_string(),
            watermark: 18,
        },
    );
    assert_eq!(state.stream.last_seq, Some(120));

    let _ = apply_action(
        &mut state,
        AppAction::TopicWatermarksReset {
            topics: vec!["runtime.codex_worker_events".to_string()],
        },
    );
    assert_eq!(state.stream.last_seq, Some(18));
    assert_eq!(
        state
            .stream
            .topic_watermarks
            .contains_key("runtime.codex_worker_events"),
        false
    );
}
