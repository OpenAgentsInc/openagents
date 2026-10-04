//! Chamber tests for authority, projectile cover, damage, and retained spell state.
use super::command::Command;
use crate::{
    play::{Ability, Game},
    playground::Run,
};

#[test]
fn selected_props_and_active_panels_restore_with_projectile_cover() {
    let mut game = Run::new(super::scenarios::area(5)).unwrap().game;
    game.activate(Ability::Spell(1)).unwrap();
    let panel = game.spells.walls[0].wall.panels[0].body;
    game.selected = game
        .spells
        .props
        .iter()
        .find(|p| p.body == panel)
        .unwrap()
        .life
        .entity;
    game.tick(0.1, [0.; 2]).unwrap();
    let mut restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
    for _ in 0..23 {
        game.activate(Ability::FireBolt).unwrap();
        restored.activate(Ability::FireBolt).unwrap();
        for _ in 0..5 {
            game.tick(0.1, [0.; 2]).unwrap();
            restored.tick(0.1, [0.; 2]).unwrap();
        }
        assert_eq!(game.checkpoint().unwrap(), restored.checkpoint().unwrap());
    }
    assert!(game.spells.walls[0].wall.panels[0].destroyed);
    assert!(!game.spells.walls[0].wall.debris.is_empty());
    assert!(game.spells.ledger_error().linear < super::LEDGER_TOLERANCE);
}

#[test]
fn wind_reaim_and_concentration_end_use_admitted_controls() {
    let mut game = Run::new(super::scenarios::area(4)).unwrap().game;
    game.activate(Ability::Spell(4)).unwrap();
    assert!(
        game.activate(Ability::SpellCommand(Command::Wind([1000, 0, 0])))
            .is_err()
    );
    for _ in 0..61 {
        game.tick(0.1, [0.; 2]).unwrap();
    }
    game.activate(Ability::SpellCommand(Command::Wind([1000, 0, 0])))
        .unwrap();
    assert_eq!(game.spells.gusts[0].gust.line.direction, glam::DVec3::X);
    game.activate(Ability::SpellCommand(Command::EndConcentration))
        .unwrap();
    assert!(game.spells.concentration.is_empty());
    assert!(!game.spells.gusts[0].gust.active(game.time as f64));
}

#[test]
fn invalid_effect_body_references_are_refused_before_they_are_indexed() {
    let mut game = Run::new(super::scenarios::area(8)).unwrap().game;
    game.activate(Ability::Spell(8)).unwrap();
    let mut checkpoint: serde_json::Value =
        serde_json::from_slice(&game.checkpoint().unwrap()).unwrap();
    checkpoint["world"]["spells"]["proxies"][0]["body"] = serde_json::json!(u32::MAX);
    assert!(Game::restore(&serde_json::to_vec(&checkpoint).unwrap()).is_err());
}

#[test]
fn every_spell_frame_preserves_momentum() {
    for scenario in crate::playground::scenarios()
        .into_iter()
        .filter(|s| super::CATALOG.iter().any(|spell| spell.key == s.key))
    {
        let key = scenario.key;
        let mut run = Run::new(scenario).unwrap();
        while !run.done() {
            run.advance().unwrap();
            let error = run.game.spells.ledger_error();
            assert!(
                error.linear < super::LEDGER_TOLERANCE && error.angular < super::LEDGER_TOLERANCE,
                "{key} at {}: {:?}; terms {:?}",
                run.game.time,
                error,
                run.game.spells.ledger.external
            );
        }
    }
}

#[test]
fn a_full_prop_budget_still_allows_stone_panels_to_break() {
    let mut game = Run::new(super::scenarios::area(5)).unwrap().game;
    game.activate(Ability::Spell(1)).unwrap();
    let panel = game.spells.walls[0].wall.panels[0].body;
    while game.spells.props.len() < super::MAX_PROPS {
        game.spawn_prop(
            "Budget crate",
            super::PropSpec::reference(super::PropKind::Crate),
            glam::Vec3::new(25., 1., 25.),
            0.,
        )
        .unwrap();
    }
    for _ in 0..23 {
        super::wall_of_stone::hit(
            &mut game.spells,
            panel,
            crate::rules::ProjectileKind::Firebolt,
            0.,
        )
        .unwrap();
    }
    assert!(game.spells.walls[0].wall.panels[0].destroyed);
    assert_eq!(game.spells.props.len(), super::MAX_PROPS);
    assert!(game.spells.ledger_error().linear < super::LEDGER_TOLERANCE);
    Game::restore(&game.checkpoint().unwrap()).unwrap();
}

#[test]
fn invalid_tentacle_target_references_are_refused_on_restore() {
    let mut game = Run::new(super::scenarios::area(6)).unwrap().game;
    game.activate(Ability::Spell(6)).unwrap();
    game.spells.tentacles[0].spell.tentacles[0].mode = crate::black_tentacles::Mode::Seek {
        body: physics::BodyId(u32::MAX),
    };
    assert!(Game::restore(&game.checkpoint().unwrap()).is_err());
}

#[test]
fn gravity_impacts_break_props_without_losing_momentum() {
    let mut game = Run::new(super::scenarios::area(8)).unwrap().game;
    let body = game.spells.props[0].body;
    super::reverse_gravity::break_props(&mut game.spells, &[body]).unwrap();
    assert!(game.spells.props[0].removed);
    assert_eq!(
        game.spells
            .props
            .iter()
            .filter(|p| p.name == "Gravity impact debris")
            .count(),
        8
    );
    assert!(game.spells.ledger_error().linear < super::LEDGER_TOLERANCE);
    assert!(game.spells.ledger_error().angular < super::LEDGER_TOLERANCE);
}

#[test]
fn reverse_gravity_hovers_creature_feet_at_one_hundred_feet() {
    let mut run = Run::new(super::scenarios::area(8)).unwrap();
    while run.game.time < 9. {
        run.advance().unwrap();
    }
    let feet = run.game.actor_position(102).unwrap().y;
    assert!(
        (feet - crate::reverse_gravity::HEIGHT as f32).abs() < 0.2,
        "{feet}"
    );
    while !run.done() {
        run.advance().unwrap();
    }
    assert!(
        run.result()
            .spells
            .log
            .iter()
            .any(|r| r.spell == "Falling" && r.text.contains("10d6")),
        "{:?}",
        run.result().spells.log
    );
}

#[test]
fn tentacle_terrain_halves_admitted_walking_without_a_restraint() {
    let mut normal = Run::new(super::scenarios::area(6)).unwrap().game;
    let mut slowed = Game::restore(&normal.checkpoint().unwrap()).unwrap();
    let actor = slowed.player_actor();
    slowed.spells.dice.force_save(actor, 20).unwrap();
    let center = slowed.player.as_dvec3() + glam::DVec3::X * 2.8;
    super::black_tentacles::cast_at(&mut slowed, center).unwrap();
    let start = normal.player;
    normal.tick(1. / 30., [1., 0.]).unwrap();
    slowed.tick(1. / 30., [1., 0.]).unwrap();
    let distance = |p: glam::Vec3| glam::Vec2::new(p.x - start.x, p.z - start.z).length();
    assert!(distance(normal.player) > 0.2);
    assert!(
        (distance(slowed.player) / distance(normal.player) - 0.5).abs() < 0.01,
        "normal {:?}, slowed {:?}, start {:?}, scale {}",
        normal.player,
        slowed.player,
        start,
        super::black_tentacles::speed_scale(&slowed.spells, actor, slowed.player.as_dvec3())
    );
    let restored = Game::restore(&slowed.checkpoint().unwrap()).unwrap();
    assert_eq!(
        super::black_tentacles::speed_scale(&restored.spells, actor, restored.player.as_dvec3()),
        0.5
    );
}

#[test]
fn hostile_impact_damages_the_stone_panel_and_retains_its_debris() {
    let mut game = Run::new(super::scenarios::area(5)).unwrap().game;
    game.activate(Ability::Spell(1)).unwrap();
    let (body, bounds) = super::wall_of_stone::cover(&game.spells)[0];
    let point = (bounds.min + bounds.max) * 0.5;
    let damage = crate::wall_of_stone::DamageType::Necrotic;
    super::wall_of_stone::struck(&mut game, point, 200, damage).unwrap();
    let panel = game.spells.walls[0]
        .wall
        .panels
        .iter()
        .find(|p| p.body == body)
        .unwrap();
    assert!(panel.destroyed);
    assert!(!game.spells.walls[0].wall.debris.is_empty());
    let restored = Game::restore(&game.checkpoint().unwrap()).unwrap();
    assert!(
        super::wall_of_stone::cover(&restored.spells)
            .iter()
            .all(|(id, _)| *id != body)
    );
    assert!(game.spells.ledger_error().linear < super::LEDGER_TOLERANCE);
}
