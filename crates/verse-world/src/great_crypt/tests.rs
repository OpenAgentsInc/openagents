use super::*;

const DT: f32 = 1.0 / 30.0;

fn run(game: &mut Game, seconds: f32, movement: [f32; 2]) {
    for _ in 0..(seconds / DT).round() as usize {
        game.tick(DT, movement).unwrap();
    }
}

/// Runs the fight, returning to the landing whenever the player falls.
fn run_living(game: &mut Game, seconds: f32) -> u32 {
    let mut deaths = 0;
    for _ in 0..(seconds / DT).round() as usize {
        game.tick(DT, [0.0; 2]).unwrap();
        if game.snapshot().player.hp == 0 {
            deaths += 1;
            game.respawn_player().unwrap();
            assert!(game.player.distance(SPAWN) < 0.2);
        }
    }
    deaths
}

/// Inside the crypt's walls: the nave, the aisles, or a chapel.
fn inside(p: Vec3) -> bool {
    let chapel = CHAPELS.iter().any(|z| (p.z - z).abs() < 1.8) && p.x.abs() < CHAPEL_BACK;
    p.is_finite()
        && p.z > FAR
        && p.z < ENTRY
        && (p.x.abs() < WALL || chapel)
        && p.y > -0.2
        && p.y < SPRING
}

#[test]
fn every_placed_model_has_a_footprint_and_a_folder() {
    for &(name, ..) in LAYOUT {
        assert!(!footprints(name).unwrap().is_empty(), "{name}");
        assert!(
            CHAMBER_MODELS.contains(&name) || CRYPT_MODELS.contains(&name),
            "{name} is not a known model"
        );
    }
    for name in CHAMBER_MODELS.iter().chain(CRYPT_MODELS) {
        assert!(LAYOUT.iter().any(|(n, ..)| n == name), "{name} is unused");
    }
    assert_eq!(model_folder("great_crypt_hall"), "great_crypt");
    assert_eq!(model_folder("brazier"), "chamber");
}

#[test]
fn the_crypt_is_about_four_times_the_lab() {
    let floor = (2.0 * WALL) * (ENTRY - FAR);
    let lab = 12.0 * 17.2;
    assert!((3.0..=4.5).contains(&(floor / lab)), "{}", floor / lab);
}

#[test]
fn the_scene_loads_and_the_player_spawns_on_the_landing() {
    let mut game = game(0).unwrap();
    assert!(game.unlocked());
    assert_eq!(game.scene.collision_profile.as_deref(), Some(PROFILE));
    assert!(!game.colliders.is_empty());
    assert!(game.navigation_work().0 < usize::MAX);
    run(&mut game, 0.5, [0.0; 2]);
    let player = game.player;
    assert!(inside(player), "{player}");
    assert!(
        (player.y - LANDING.1).abs() < 0.05,
        "on the landing: {player}"
    );
    assert!(player.distance(SPAWN) < 0.2, "{player}");
    // Everyone stands inside the crypt, on the floor or the dais.
    for actor in &game.frame().actors {
        assert!(
            inside(actor.actor.position),
            "{} at {}",
            actor.actor.name,
            actor.actor.position
        );
    }
}

#[test]
fn cultists_exist_in_roles_and_the_boss_sleeps_on_the_circle() {
    let game = game(0).unwrap();
    let frame = game.frame();
    let hostiles: Vec<_> = frame
        .actors
        .iter()
        .filter(|a| a.actor.nameplate && !a.actor.friendly)
        .collect();
    assert_eq!(
        hostiles.len(),
        1 + CHANTERS.len() + 1 + GUARDS.len() + CHAPEL_CULTISTS.len()
    );
    assert!(
        hostiles
            .iter()
            .filter(|a| a.actor.model.starts_with("cultist"))
            .count()
            >= 20
    );
    let boss = hostiles.iter().find(|a| a.actor.id == BOSS).unwrap();
    assert_eq!(boss.actor.model, "claude");
    assert!(boss.actor.position.distance(CIRCLE) < 0.1);
    let ritual = super::ritual(&game).unwrap();
    assert!(ritual.holds(BOSS) && ritual.holds(LEADER));
    assert_eq!(ritual.chanters(), CHANTERS.len());
    assert!(GUARDS.iter().all(|id| !ritual.holds(*id)));
    // Chanting acolytes play the cast pose, facing the circle.
    for id in CHANTERS {
        let a = frame.actors.iter().find(|a| a.actor.id == id).unwrap();
        assert_eq!(a.animation, State::Cast.into());
    }
}

#[test]
fn the_guards_fight_while_the_ritual_runs_and_the_waves_come() {
    let mut game = game(0).unwrap();
    run_living(&mut game, 40.0);
    let encounter = game.encounter.as_ref().unwrap();
    assert!(encounter.enemy_casts > 0, "the guards cast");
    assert!(encounter.damage > 0, "the player was hit");
    let ritual = encounter.ritual.as_ref().unwrap();
    assert!(
        ritual.progress > 30.0,
        "the chant runs: {}",
        ritual.progress
    );
    assert!(ritual.called.contains_key(&0), "the first wave left");
    assert!(WAVES[0].1.iter().all(|id| !ritual.holds(*id)));
    assert!(ritual.awakened.is_none());
    // Everyone stays inside the walls.
    for actor in &game.frame().actors {
        assert!(
            inside(actor.actor.position),
            "{} at {}",
            actor.actor.name,
            actor.actor.position
        );
    }
}

#[test]
fn walls_and_parapets_keep_the_player_in() {
    for yaw in [0.0, FRAC_PI_2, -FRAC_PI_2, PI] {
        let mut game = game(0).unwrap();
        if let Some(e) = game.encounter.as_mut() {
            e.postpone_casts_until(500.0).unwrap();
        }
        game.yaw = yaw;
        run(&mut game, 12.0, [0.0, 1.0]);
        let p = game.player;
        assert!(inside(p), "walking at yaw {yaw} left the crypt at {p}");
    }
}

#[test]
fn the_completed_ritual_wakes_an_empowered_boss_who_fights() {
    let mut game = game(0).unwrap();
    {
        let ritual = game.encounter.as_mut().unwrap().ritual.as_mut().unwrap();
        ritual.progress = RITUAL_SECONDS - 0.2;
    }
    run(&mut game, 1.0, [0.0; 2]);
    let ritual = super::ritual(&game).unwrap();
    assert!(ritual.awakened.is_some() && ritual.empowered);
    assert!(!ritual.holds(BOSS));
    assert!(CHANTERS.iter().all(|id| !ritual.holds(*id)));
    assert!(
        game.encounter.as_ref().unwrap().enrage.is_none(),
        "empowered, not yet enraged"
    );
    // The boss's first strike comes within a few seconds.
    let mut struck = false;
    for _ in 0..(8.0 / DT) as usize {
        game.tick(DT, [0.0; 2]).unwrap();
        struck |= game
            .encounter
            .as_ref()
            .unwrap()
            .casts
            .iter()
            .any(|c| c.actor == BOSS);
    }
    assert!(struck, "the boss casts once awake");
}

#[test]
fn a_ritual_checkpoint_restores() {
    let mut game = game(0).unwrap();
    run(&mut game, 2.0, [0.0; 2]);
    let restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
    let a = ritual(&game).unwrap();
    let b = ritual(&restored).unwrap();
    assert_eq!(a.waiting, b.waiting);
    assert_eq!(a.progress, b.progress);
}

#[test]
fn the_high_priest_rallies_the_chapels_and_his_fall_wakes_claude() {
    let mut game = game(0).unwrap();
    run(&mut game, 0.5, [0.0; 2]);
    // Wounded below half: he joins the fight and calls every chapel.
    game.wound(LEADER, 60).unwrap();
    run(&mut game, 0.2, [0.0; 2]);
    let ritual = super::ritual(&game).unwrap();
    assert!(ritual.rallied && !ritual.holds(LEADER));
    assert_eq!(ritual.waves_called(), WAVES.len());
    assert!(CHAPEL_CULTISTS.iter().all(|id| !ritual.holds(*id)));
    assert!(ritual.awakened.is_none());
    // His fall wakes Claude, not empowered, and stops the chant.
    game.wound(LEADER, 1000).unwrap();
    run(&mut game, 0.2, [0.0; 2]);
    let ritual = super::ritual(&game).unwrap();
    assert!(ritual.awakened.is_some() && !ritual.empowered);
    assert_eq!(ritual.chanters(), 0);
    let progress = ritual.progress;
    run(&mut game, 1.0, [0.0; 2]);
    assert_eq!(super::ritual(&game).unwrap().progress, progress);
}

#[test]
fn slain_acolytes_slow_the_chant() {
    let mut game = game(0).unwrap();
    for id in &CHANTERS[..3] {
        game.wound(*id, 1000).unwrap();
    }
    run(&mut game, 6.0, [0.0; 2]);
    let ritual = super::ritual(&game).unwrap();
    assert_eq!(ritual.chanters(), 3);
    assert!(
        ritual.progress < 3.2,
        "half the acolytes, half the pace: {}",
        ritual.progress
    );
}
