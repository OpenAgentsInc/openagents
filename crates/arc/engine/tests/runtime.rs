use arc_core::{ArcAction, ArcActionKind, ArcGameState};
use arc_engine::{ArcEngine, ArcPoint};

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[test]
fn engine_loads_fixture_and_renders_initial_state() {
    let engine = ArcEngine::load_from_path(fixture_path("demo_game.json"))
        .expect("fixture package should load");
    let observation = engine
        .observation()
        .expect("initial observation should render");

    assert_eq!(engine.state().current_level_index, 0);
    assert_eq!(engine.state().player_position, ArcPoint { x: 1, y: 1 });
    assert_eq!(observation.frame.width(), 64);
    assert_eq!(observation.frame.height(), 64);
    assert_eq!(pixel(&observation.frame, 0, 0), 3);
    assert_eq!(pixel(&observation.frame, 12, 12), 14);
    assert_eq!(pixel(&observation.frame, 22, 22), 5);
    assert_eq!(pixel(&observation.frame, 32, 22), 12);
    assert_eq!(pixel(&observation.frame, 42, 42), 8);
    assert_eq!(
        observation.available_actions,
        vec![
            ArcActionKind::Action1,
            ArcActionKind::Action2,
            ArcActionKind::Action3,
            ArcActionKind::Action4,
            ArcActionKind::Action6,
        ]
    );
}

#[test]
fn engine_supports_undo_and_reset() {
    let mut engine = ArcEngine::load_from_path(fixture_path("demo_game.json"))
        .expect("fixture package should load");

    engine.step(ArcAction::Action4).expect("move right");
    assert_eq!(engine.state().player_position, ArcPoint { x: 2, y: 1 });

    let undo = engine.step(ArcAction::Action7).expect("undo should work");
    assert_eq!(engine.state().player_position, ArcPoint { x: 1, y: 1 });
    assert!(
        !undo
            .observation
            .available_actions
            .contains(&ArcActionKind::Action7)
    );
    assert!(
        undo.observation
            .available_actions
            .contains(&ArcActionKind::Action6)
    );

    engine.step(ArcAction::Action4).expect("move right again");
    engine.step(ArcAction::Action2).expect("move down");
    engine
        .step(ArcAction::action6(22, 22).expect("coords should validate"))
        .expect("action6 should toggle door");
    assert!(
        !engine
            .observation()
            .expect("observation")
            .frame
            .pixels()
            .contains(&12)
    );

    let reset = engine.step(ArcAction::Reset).expect("reset should work");
    assert_eq!(engine.state().player_position, ArcPoint { x: 1, y: 1 });
    assert_eq!(engine.state().action_count, 0);
    assert_eq!(pixel(&reset.observation.frame, 32, 22), 12);
    assert!(!reset.full_reset);
}

#[test]
fn engine_advances_levels_and_replays_deterministically() {
    let package = arc_engine::load_game_package(fixture_path("demo_game.json"))
        .expect("fixture package should load");
    let actions = vec![
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::action6(22, 22).expect("coords should validate"),
        ArcAction::Action4,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action5,
    ];

    let mut engine = ArcEngine::from_package(package.clone()).expect("engine should initialize");
    let initial_reset = engine
        .step(ArcAction::Reset)
        .expect("reset should initialize run");
    assert!(initial_reset.full_reset);
    assert_eq!(initial_reset.levels_completed, 0);

    for action in &actions[..6] {
        engine
            .step(action.clone())
            .expect("level one actions should succeed");
    }
    let transition = engine
        .step(actions[6].clone())
        .expect("level transition action should succeed");
    assert!(transition.level_completed);
    assert!(transition.advanced_level);
    assert_eq!(transition.frames.len(), 2);
    assert_eq!(engine.state().current_level_index, 1);
    assert_eq!(engine.state().player_position, ArcPoint { x: 1, y: 1 });
    assert!(
        transition
            .observation
            .available_actions
            .contains(&ArcActionKind::Action5)
    );

    for action in &actions[7..11] {
        engine
            .step(action.clone())
            .expect("level two setup should succeed");
    }
    let win_step = engine
        .step(actions[11].clone())
        .expect("final action should win");
    assert!(win_step.level_completed);
    assert_eq!(win_step.observation.game_state, ArcGameState::Win);

    let mut replay_actions = vec![ArcAction::Reset];
    replay_actions.extend(actions.clone());
    let recording_a = ArcEngine::replay(package.clone(), &replay_actions)
        .expect("replay should produce recording");
    let recording_b = ArcEngine::replay(package, &replay_actions)
        .expect("second replay should produce recording");
    assert_eq!(
        recording_a
            .contract_digest()
            .expect("digest should compute"),
        recording_b
            .contract_digest()
            .expect("digest should compute")
    );
    assert_eq!(recording_a.steps.len(), replay_actions.len());
}

fn pixel(frame: &arc_core::ArcFrameData, x: usize, y: usize) -> u8 {
    frame.pixels()[y * usize::from(frame.width()) + x]
}
