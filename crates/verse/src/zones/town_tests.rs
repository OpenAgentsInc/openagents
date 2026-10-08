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
    let mut runtime = crate::zones::everglade_tests::entered();
    let ([cx, cz], [_, hz]) = SHOPS[0];
    runtime
        .set_spawn(Vec3::new(cx + 1.0, 0.0, cz - hz - 1.05), 0.0)
        .unwrap();
    // The player has no sledgehammer in Everglade; the town's demolition
    // takes the swing directly, as a world event or the owner's tool will.
    for _ in 0..3 {
        runtime
            .zone_state
            .everglade
            .as_deref_mut()
            .expect("Everglade")
            .demolish(false)
            .unwrap();
        for _ in 0..70 {
            runtime.tick(&InputState::default(), 1.0 / 60.0);
        }
    }
    let town = runtime
        .zone_state
        .everglade
        .as_deref()
        .and_then(|glade| glade.town())
        .expect("the town");
    let shop = building(town, SHOPS[0]);
    assert!(town.raised().contains(&shop), "the chop reached the shop");
    assert!(town.hidden() > 0, "the struck section draws as chunks");
}

#[test]
fn everglade_offers_no_offensive_spell_and_its_demolition_still_runs() {
    use crate::controller::InputState;
    use crate::zones::Intent;
    use crate::zones::everglade::hotbar::COUNT;
    let mut runtime = crate::zones::everglade_tests::entered();
    assert!(!runtime.dev_destruction());
    let slots = runtime.everglade_hotbar().expect("Everglade's hotbar");
    assert_eq!(slots.len(), COUNT);
    // Neither the intents nor the targeting reach the town from input.
    for intent in [
        Intent::MeteorSwarm,
        Intent::Thunderbolt,
        Intent::MegaThunderbolt,
        Intent::Swing,
        Intent::Rebuild,
    ] {
        assert!(runtime.zone_intent(intent).is_err(), "{intent:?}");
    }
    assert!(!runtime.demolition_targeting());
    assert!(!runtime.demolition_confirm());
    assert!(!runtime.demolition_cancel());
    assert!(runtime.everglade_swarm().is_none());
    // A build without the dev feature refuses the switch.
    if !crate::zones::everglade::hotbar::DEV_DESTRUCTION {
        assert!(runtime.set_dev_destruction(true).is_err());
        assert!(!runtime.dev_destruction());
    }
    // The town's demolition still runs when driven directly, as a world
    // event will drive it: Meteor Swarm's circle ahead of the player, then
    // its cast.
    let player = runtime.player.clone();
    let glade = runtime
        .zone_state
        .everglade
        .as_deref_mut()
        .expect("Everglade");
    glade.meteor_swarm(&player).unwrap();
    assert!(
        glade.confirm_swarm(&player),
        "the cast begins at the circle"
    );
    for _ in 0..(6.0 / (1.0 / 60.0)) as usize {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    let town = runtime
        .zone_state
        .everglade
        .as_deref()
        .and_then(|glade| glade.town())
        .expect("the town");
    assert!(town.swarm().status().ready, "no cooldown");
}

#[cfg(feature = "dev-destruction")]
#[test]
fn everglade_casts_meteor_swarm_swings_and_restores_through_its_intents() {
    use crate::controller::InputState;
    use crate::zones::Intent;
    use crate::zones::everglade::demolition::meteor::Strike;
    use crate::zones::everglade::hotbar::{DEV_COUNT, DEV_ORDER, SLOTS};
    let mut runtime = crate::zones::everglade_tests::entered();
    runtime.set_dev_destruction(true).unwrap();
    let slots = runtime.everglade_hotbar().expect("Everglade's hotbar");
    assert_eq!(slots.len(), DEV_COUNT);
    // Meteor Swarm on 1, the Thunderbolt on 2, the Mega Thunderbolt on 3,
    // and Levitate on 4.
    assert_eq!(runtime.everglade_hotbar_order(), DEV_ORDER.to_vec());
    let intents: Vec<Intent> = (0..DEV_COUNT)
        .map(|i| runtime.everglade_slot_intent(i).unwrap())
        .collect();
    assert_eq!(
        intents[..4],
        [
            Intent::MeteorSwarm,
            Intent::Thunderbolt,
            Intent::MegaThunderbolt,
            Intent::Levitate
        ]
    );
    assert_eq!(SLOTS[DEV_ORDER[1]].1, "thunderbolt-icon");
    assert_eq!(SLOTS[DEV_ORDER[2]].1, "mega-thunderbolt-icon");
    assert!(slots[0].enabled, "Meteor Swarm is ready");
    assert!(slots[1].enabled, "the Thunderbolt is ready");
    assert!(slots[2].enabled, "the Mega Thunderbolt is ready");
    // The Mega Thunderbolt aims and lights its own slot.
    runtime.zone_intent(Intent::MegaThunderbolt).unwrap();
    assert_eq!(
        runtime.everglade_swarm().unwrap().strike,
        Strike::MegaLightning
    );
    let slots = runtime.everglade_hotbar().unwrap();
    assert!(slots[2].active && !slots[1].active);
    assert!(runtime.demolition_cancel());
    // The Thunderbolt aims, lights its own slot, and casts.
    runtime.zone_intent(Intent::Thunderbolt).unwrap();
    assert!(runtime.demolition_targeting());
    let swarm = runtime.everglade_swarm().expect("the town's spell");
    assert_eq!(
        swarm.strike,
        crate::zones::everglade::demolition::meteor::Strike::Lightning
    );
    let slots = runtime.everglade_hotbar().unwrap();
    assert!(
        slots[1].active && !slots[0].active,
        "the Thunderbolt's slot lights"
    );
    assert!(runtime.demolition_confirm(), "the bolt's cast begins");
    for _ in 0..(3.0 / (1.0 / 60.0)) as usize {
        runtime.tick(&InputState::default(), 1.0 / 60.0);
    }
    assert!(runtime.everglade_swarm().unwrap().ready, "no cooldown");
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
