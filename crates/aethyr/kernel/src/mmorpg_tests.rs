use super::*;
use af_mmorpg_core::{Class, Controls, Prediction, Surface, WorldConfig};
struct Terrain;
impl Surface for Terrain {
    fn height(&self, x: f32, z: f32) -> f32 {
        af_mmorpg_core::terrain::height(x as f64, z as f64) as f32
    }
    fn can_occupy(&self, p: af_mmorpg_core::V3, _: f32) -> bool {
        p[1] >= self.height(p[0], p[2]) - 0.001 && !af_mmorpg_core::scenery::tree_blocks(p[0], p[2])
    }
}
#[test]
fn browser_prediction_matches_authoritative_source_terrain_world_and_replays_inputs() {
    let terrain = Terrain;
    let mut world = af_mmorpg_core::World::new(
        WorldConfig {
            spawners: Some(vec![]),
            ..Default::default()
        },
        &terrain,
    )
    .unwrap();
    let id = world.join("Player", Class::Paladin, &terrain).unwrap();
    let mut known = BTreeSet::new();
    let initial = world
        .snapshot(id, &mut known, 0)
        .unwrap()
        .player
        .prediction
        .unwrap();
    let mut engine = Engine::default();
    assert_eq!(
        engine.dispatch(Request::MmorpgPredictionInit {
            world_scale: 1. / 6.,
            class: Class::Paladin,
            state: initial.clone()
        })["ok"],
        true
    );
    let mut inputs = Vec::new();
    for tick in 1..=120 {
        let input = Controls {
            forward: if tick < 75 { 1. } else { 0. },
            turn: if tick < 30 { 0.5 } else { 0. },
            sprint: tick > 30 && tick < 60,
            ..Default::default()
        };
        world.input(id, tick, input).unwrap();
        world.step(&terrain);
        engine.dispatch(Request::MmorpgPredictionStep { input, ticks: 1 });
        let server = world
            .snapshot(id, &mut known, 0)
            .unwrap()
            .player
            .prediction
            .unwrap();
        assert_eq!(
            engine.mmorpg_prediction.as_ref().unwrap().state,
            server,
            "tick{tick}"
        );
        inputs.push(PredictionInput { input, ticks: 1 });
    }
    let before = engine.mmorpg_prediction.as_ref().unwrap().state.clone();
    assert_eq!(
        engine.dispatch(Request::MmorpgPredictionReconcile {
            state: initial,
            inputs
        })["ok"],
        true
    );
    assert_eq!(engine.mmorpg_prediction.as_ref().unwrap().state, before);
    assert_eq!(engine.velocity, [0.; 3]);
}
#[test]
fn invalid_prediction_and_excess_replay_are_atomic_and_legacy_remains_available() {
    let mut engine = Engine::default();
    let mut state = Prediction::new([6., Terrain.height(6., 20.), 20.], 0.);
    state.grounded = true;
    engine.dispatch(Request::MmorpgPredictionInit {
        world_scale: 1. / 6.,
        class: Class::Paladin,
        state: state.clone(),
    });
    let position = engine.camera.position;
    assert_eq!(
        engine.dispatch(Request::MmorpgPredictionStep {
            input: Controls::default(),
            ticks: 5
        })["ok"],
        false
    );
    assert_eq!(
        engine.dispatch(Request::MmorpgPredictionReconcile {
            state: state.clone(),
            inputs: (0..121)
                .map(|_| PredictionInput {
                    input: Controls::default(),
                    ticks: 1
                })
                .collect()
        })["ok"],
        false
    );
    assert_eq!(engine.camera.position, position);
    state.position[0] = f32::NAN;
    assert_eq!(
        engine.dispatch(Request::MmorpgPredictionInit {
            world_scale: 1. / 6.,
            class: Class::Paladin,
            state
        })["ok"],
        false
    );
    assert_eq!(engine.camera.position, position);
    engine.dispatch(Request::SetCharacterController { controller: None });
    assert!(engine.mmorpg_prediction.is_none());
    assert!(engine.quick_combat.is_none());
}
