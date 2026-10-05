use super::*;
use verse_world::social::controller::{InputState, PlayerController, RADIUS};

fn walk(solids: &Solids, player: &mut PlayerController, seconds: f32) {
    let forward = InputState {
        forward: true,
        ..InputState::default()
    };
    let steps = (seconds / 0.02).round() as usize;
    for _ in 0..steps {
        solids.step(player, &forward, 0.02, HALF_EXTENT, true);
    }
}

#[test]
fn every_model_has_a_footprint_and_a_place() {
    for name in MODELS {
        assert!(!footprints(name).unwrap().is_empty(), "{name}");
        assert!(
            LAYOUT.iter().any(|(n, ..)| n == name),
            "{name} is not placed"
        );
    }
    for (name, ..) in LAYOUT {
        assert!(MODELS.contains(name), "{name} is not a model");
    }
}

#[test]
fn the_spawn_is_inside_the_hall_and_clear() {
    let solids = solids().unwrap();
    assert!(SPAWN.x.abs() < HALF_X - RADIUS && SPAWN.z.abs() < HALF_Z - RADIUS);
    assert!(
        solids
            .blocking(0.0)
            .iter()
            .all(|b| !b.contains(SPAWN.x, SPAWN.z, RADIUS))
    );
    assert_eq!(solids.floor(SPAWN.x, SPAWN.z, 0.0), 0.0);
    // Facing the hall, away from the door, and out of the door's reach so
    // a first key press doesn't leave.
    let player = PlayerController::new(SPAWN, SPAWN_YAW);
    assert!(player.forward().z < -0.99);
    assert!(!near_door(SPAWN));
    assert!(near_door(SPAWN + Vec3::Z * 0.6));
}

#[test]
fn the_walls_hold_the_player_in() {
    let solids = solids().unwrap();
    for yaw in [0.0, FRAC_PI_2, -FRAC_PI_2, std::f32::consts::PI] {
        let mut player = PlayerController::new(Vec3::new(0.0, 0.0, 3.0), yaw);
        walk(&solids, &mut player, 8.0);
        let p = player.pos;
        assert!(
            p.x.abs() <= HALF_X - RADIUS + 0.01 && p.z.abs() <= HALF_Z - RADIUS + 0.01,
            "walking at yaw {yaw} left the hall at {p}"
        );
    }
}

#[test]
fn props_and_pillars_block() {
    let solids = solids().unwrap();
    // West into the green cauldron.
    let mut player = PlayerController::new(Vec3::new(-1.5, 0.0, -3.4), -FRAC_PI_2);
    walk(&solids, &mut player, 3.0);
    assert!(
        player.pos.x > -3.2,
        "walked into the cauldron: {}",
        player.pos
    );
    // East into the dissection slab.
    let mut player = PlayerController::new(Vec3::new(1.0, 0.0, -3.75), FRAC_PI_2);
    walk(&solids, &mut player, 3.0);
    assert!(player.pos.x < 3.0, "walked into the slab: {}", player.pos);
    // East into a pillar between the alcoves.
    let mut player = PlayerController::new(Vec3::new(3.0, 0.0, 1.9), FRAC_PI_2);
    walk(&solids, &mut player, 3.0);
    assert!(
        player.pos.x < PILLAR_X - 0.4,
        "walked into a pillar: {}",
        player.pos
    );
    // North into the sarcophagus, over the dais.
    let mut player = PlayerController::new(Vec3::new(0.0, 0.0, -4.5), std::f32::consts::PI);
    walk(&solids, &mut player, 4.0);
    assert!(
        player.pos.z > -6.95,
        "walked into the sarcophagus: {}",
        player.pos
    );
    // South into the door: the door is not a way through.
    let mut player = PlayerController::new(SPAWN, 0.0);
    walk(&solids, &mut player, 3.0);
    assert!(player.pos.z < HALF_Z - RADIUS + 0.01);
}

#[test]
fn the_dais_steps_are_walkable() {
    let solids = solids().unwrap();
    // Up the lower step to the sarcophagus.
    let mut player = PlayerController::new(Vec3::new(0.0, 0.0, -4.5), std::f32::consts::PI);
    walk(&solids, &mut player, 4.0);
    assert!(
        (player.pos.y - 0.18).abs() < 0.01 && player.pos.z < -6.3,
        "stands on the lower step: {}",
        player.pos
    );
    // The upper step is a step up from the lower one, not a wall.
    let (x, z) = (-1.6, -8.5);
    assert!((solids.floor(x, z, 0.18) - 0.36).abs() < 0.01);
    assert!(
        solids
            .blocking_near(x, z, 0.0, 0.18)
            .iter()
            .all(|b| !b.contains(x, z, 0.0))
    );
    // And back down to the floor.
    player.yaw = 0.0;
    walk(&solids, &mut player, 3.0);
    assert!(
        player.pos.y.abs() < 0.01,
        "back on the floor: {}",
        player.pos
    );
    assert!(player.pos.z > -4.0);
}

#[test]
fn the_vault_is_a_ceiling() {
    let solids = solids().unwrap();
    assert_eq!(solids.ceiling(0.0, 0.0, 2.0), Some(SPRING + RISE));
    assert!(solids.ceiling(5.0, 0.0, 2.0).unwrap() <= vault_height(5.0));
    assert!(feet_ceiling(0.0) > 4.0 && feet_ceiling(HALF_X) > 2.0);
}

#[test]
fn the_camera_stays_in_the_hall() {
    let focus = Vec3::new(0.0, 1.6, 7.5);
    // Behind the player, out through the door.
    let eye = keep_eye_inside(focus, Vec3::new(0.0, 3.0, 11.0));
    assert!(eye.z <= HALF_Z - 0.29, "{eye}");
    assert!(eye.z > focus.z);
    // Up through the vault near a wall.
    let focus = Vec3::new(5.0, 1.6, 0.0);
    let eye = keep_eye_inside(focus, Vec3::new(5.4, 9.0, 0.0));
    assert!(eye.y <= vault_height(eye.x) - 0.29, "{eye}");
    // Behind a pillar.
    let focus = Vec3::new(3.5, 1.6, 1.9);
    let eye = keep_eye_inside(focus, Vec3::new(6.5, 2.0, 1.9));
    assert!(eye.x < PILLAR_X - PILLAR_RADIUS, "{eye}");
    // An eye already in the room is left alone.
    let focus = Vec3::new(0.0, 1.6, 0.0);
    let eye = Vec3::new(1.0, 2.5, 3.0);
    assert_eq!(keep_eye_inside(focus, eye), eye);
}

#[test]
fn the_hall_imports_and_lights_itself() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/verse/generated/chamber");
    let hall = Hall::from_dir(&dir).unwrap();
    assert!(!hall.lamps.is_empty() && hall.lamps.len() <= MAX_LAMPS);
    assert!(hall.flames.len() > 20, "{} flames", hall.flames.len());
    assert_eq!(hall.scene.placements.len(), LAYOUT.len());
    let mut crypt = Crypt::new(&hall);
    crypt.tick(0.1);
    let mesh = crypt.mesh(SPAWN + Vec3::Y * 1.7);
    let neon = mesh.neon.unwrap();
    assert!(neon.key.is_some(), "textured scenes draw on a lit stage");
    assert!(!mesh.sprites.is_empty() && !mesh.glow.is_empty());
    assert_eq!(crypt.lamp_count(), hall.lamps.len());
}

#[cfg(feature = "embedded")]
#[test]
fn the_embedded_models_build_the_same_hall() {
    let hall = Hall::embedded().unwrap();
    assert_eq!(hall.scene.placements.len(), LAYOUT.len());
}
