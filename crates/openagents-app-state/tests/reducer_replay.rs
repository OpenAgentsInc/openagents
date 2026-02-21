use std::fs;
use std::path::PathBuf;

use openagents_app_state::{AppAction, AppRoute, AppState, AuthStatus, apply_action};

#[test]
fn reducer_replay_fixture_is_deterministic() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("reducer-replay-v1.json");
    let fixture_raw = fs::read_to_string(&fixture_path).expect("fixture must be readable");
    let actions: Vec<AppAction> =
        serde_json::from_str(&fixture_raw).expect("fixture must deserialize into AppAction list");

    let mut first_state = AppState::default();
    for action in actions.clone() {
        let _ = apply_action(&mut first_state, action);
    }
    let first_drain = apply_action(&mut first_state, AppAction::DrainIntents).drained_intents;

    let mut second_state = AppState::default();
    for action in actions {
        let _ = apply_action(&mut second_state, action);
    }
    let second_drain = apply_action(&mut second_state, AppAction::DrainIntents).drained_intents;

    assert_eq!(first_state, second_state);
    assert_eq!(first_drain, second_drain);
    assert_eq!(first_drain.len(), 3);
    assert_eq!(first_drain[0].id.0, 1);
    assert_eq!(first_drain[1].id.0, 2);
    assert_eq!(first_drain[2].id.0, 3);
    assert_eq!(
        first_state.route,
        AppRoute::Chat {
            thread_id: Some("thread-42".to_string())
        }
    );
    assert_eq!(first_state.auth.status, AuthStatus::SignedIn);
}

#[test]
fn navigate_action_tracks_route_history() {
    let mut state = AppState::default();
    let _ = apply_action(
        &mut state,
        AppAction::Navigate {
            route: AppRoute::Workers,
        },
    );
    let _ = apply_action(
        &mut state,
        AppAction::Navigate {
            route: AppRoute::Settings,
        },
    );

    assert_eq!(state.route, AppRoute::Settings);
    assert_eq!(state.route_history, vec![AppRoute::Home, AppRoute::Workers]);
}
