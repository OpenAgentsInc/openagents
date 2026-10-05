//! The `everglade/demolition` tests that run through the world
//! runtime, kept in `verse` when the zone moved into its own crate.

//! The town's destructible buildings: lazy raising, the studio's
//! protection, collapse, the solids, restoring, and the caps.

use crate::zones::everglade::demolition::town::Town;
use crate::zones::everglade::layout::SHOPS;
use glam::Vec3;

/// The building whose footprint is `rect`.
fn building(town: &Town, rect: ([f32; 2], [f32; 2])) -> usize {
    town.buildings()
        .iter()
        .position(|b| {
            (b.rect.0[0] - rect.0[0]).abs() < 0.01 && (b.rect.0[1] - rect.0[1]).abs() < 0.01
        })
        .expect("the survey found the building")
}

#[test]
fn the_characters_chop_cracks_a_shop_front() {
    use crate::controller::InputState;
    use crate::zones::Intent;
    let mut runtime = crate::zones::everglade_tests::entered();
    let ([cx, cz], [_, hz]) = SHOPS[0];
    runtime
        .set_spawn(Vec3::new(cx + 1.0, 0.0, cz - hz - 1.05), 0.0)
        .unwrap();
    for _ in 0..3 {
        runtime.zone_intent(Intent::Swing).unwrap();
        for _ in 0..70 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
        }
    }
    let town = runtime
        .zone_state
        .everglade
        .as_ref()
        .and_then(|glade| glade.town())
        .expect("the town");
    let shop = building(town, SHOPS[0]);
    assert!(town.raised().contains(&shop), "the chop reached the shop");
    assert!(town.hidden() > 0, "the struck section draws as chunks");
}

#[test]
fn everglade_casts_meteor_swarm_swings_and_restores_through_its_intents() {
    use crate::controller::InputState;
    use crate::zones::Intent;
    use crate::zones::everglade::hotbar::COUNT;
    let mut runtime = crate::zones::everglade_tests::entered();
    let slots = runtime.everglade_hotbar().expect("Everglade's hotbar");
    assert!(slots[COUNT - 2].enabled, "Meteor Swarm is ready");
    assert!(slots[COUNT - 1].enabled, "the sledgehammer is in reach");
    // The circle lands ahead of the player at once, for a touch screen.
    runtime.zone_intent(Intent::MeteorSwarm).unwrap();
    assert!(runtime.demolition_targeting());
    let swarm = runtime.everglade_swarm().expect("the town's spell");
    assert!(swarm.targeting);
    assert!(
        runtime.demolition_confirm(),
        "the cast begins at the circle"
    );
    for _ in 0..(6.0 / (1.0 / 60.0)) as usize {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    assert!(runtime.everglade_swarm().unwrap().ready, "no cooldown");
    runtime.zone_intent(Intent::Swing).unwrap();
    for _ in 0..90 {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    runtime.zone_intent(Intent::Rebuild).unwrap();
    // Escape-style cancels leave nothing to cancel afterwards.
    runtime.zone_intent(Intent::MeteorSwarm).unwrap();
    assert!(runtime.demolition_cancel());
    assert!(!runtime.demolition_cancel());
}
