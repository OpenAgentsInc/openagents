use super::*;
use crate::{controller::PlayerController, palette};
use coder_ui::theme::Intensity;

fn finish(doors: &mut Doors) {
    for _ in 0..60 {
        doors.tick(1.0 / 60.0);
    }
}

#[test]
fn every_catalog_combination_selects_only_its_declared_route() {
    for id in DoorId::ALL {
        for item in DemoItem::ALL {
            let mut doors = Doors::default();
            doors.hold(item);
            let before = doors.document();
            let result = doors.tap(id);
            if let Some(target) = destination(id, item) {
                assert_eq!(result, TapResult::Reacted);
                assert_eq!(doors.state(id).selected, Some(target));
                assert_eq!(doors.state(id).last, Some(item));
                let revision = doors.revision();
                for _ in 0..100 {
                    assert_eq!(doors.tap(id), TapResult::Ignored);
                }
                assert_eq!(doors.state(id).reaction, 1);
                assert_eq!(doors.revision(), revision);
                finish(&mut doors);
                assert_eq!(doors.tap(id), TapResult::Walk(target));
                assert_eq!(doors.state(id).phase, DoorPhase::Cooldown);
                assert_eq!(doors.tap(id), TapResult::Ignored);
                finish(&mut doors);
                assert_eq!(doors.state(id).phase, DoorPhase::Idle);
                assert_eq!(doors.state(id).last, Some(item));
                assert_eq!(doors.state(id).selected, None);
            } else {
                assert_eq!(result, TapResult::Refused);
                assert_eq!(doors.document(), before);
                assert_eq!(doors.state(id).selected, None);
            }
        }
    }
}

#[test]
fn empty_reuses_memory_and_incompatible_keys_never_overwrite_it() {
    let mut doors = Doors::default();
    assert_eq!(doors.tap(DoorId::Spark), TapResult::Reacted);
    doors.hold(DemoItem::Ring);
    assert_eq!(doors.state(DoorId::Spark).selected, None);
    assert_eq!(doors.tap(DoorId::Spark), TapResult::Refused);
    assert_eq!(doors.state(DoorId::Spark).last, Some(DemoItem::Prism));
    assert_eq!(doors.tap(DoorId::Halo), TapResult::Reacted);
    doors.hold(DemoItem::Empty);
    assert_eq!(doors.tap(DoorId::Spark), TapResult::Reacted);
    assert_eq!(
        doors.state(DoorId::Spark).selected,
        Some(Destination::Library)
    );
    assert_eq!(doors.tap(DoorId::Halo), TapResult::Reacted);
    assert_eq!(
        doors.state(DoorId::Halo).selected,
        Some(Destination::Oracle)
    );
    doors.reset(DoorId::Spark);
    assert_eq!(doors.held(), DemoItem::Empty);
    assert_eq!(doors.state(DoorId::Spark).last, None);
    assert_eq!(doors.state(DoorId::Halo).last, Some(DemoItem::Ring));
    assert_eq!(doors.tap(DoorId::Spark), TapResult::Refused);
}

#[test]
fn changing_keys_reselects_and_timers_are_bounded() {
    let mut doors = Doors::default();
    doors.tap(DoorId::Spark);
    finish(&mut doors);
    doors.hold(DemoItem::Bolt);
    assert_eq!(doors.tap(DoorId::Spark), TapResult::Reacted);
    assert_eq!(
        doors.state(DoorId::Spark).selected,
        Some(Destination::GymApproach)
    );
    for dt in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
        doors.tick(dt);
    }
    assert_eq!(doors.state(DoorId::Spark).reaction_progress(), Some(0.0));
    doors.tick(1_000.0);
    assert_eq!(doors.state(DoorId::Spark).phase, DoorPhase::Reacting);
    assert!(doors.state(DoorId::Spark).reaction_progress().unwrap() < 0.2);
    doors.cancel_transient();
    assert_eq!(doors.state(DoorId::Spark).phase, DoorPhase::Idle);
    assert_eq!(doors.state(DoorId::Spark).last, Some(DemoItem::Bolt));
}

#[test]
fn preferences_round_trip_only_held_items_and_compatible_memories() {
    let mut doors = Doors::default();
    doors.tap(DoorId::Spark);
    doors.hold(DemoItem::Ring);
    doors.tap(DoorId::Halo);
    let document = doors.document();
    assert!(document.len() < PREFERENCES_LIMIT);
    let mut restored = Doors::default();
    restored.restore(&document).unwrap();
    assert_eq!(restored.held(), DemoItem::Ring);
    assert_eq!(restored.state(DoorId::Spark).last, Some(DemoItem::Prism));
    assert_eq!(restored.state(DoorId::Halo).last, Some(DemoItem::Ring));
    for id in DoorId::ALL {
        assert_eq!(restored.state(id).phase, DoorPhase::Idle);
        assert_eq!(restored.state(id).selected, None);
        assert_eq!(restored.state(id).reaction, 0);
    }
    assert_eq!(restored.route_owner, None);
    assert_eq!(restored.document(), document);
    assert_eq!(restored.revision(), 1);
    restored.reset(DoorId::Halo);
    let document = restored.document();
    doors.restore(&document).unwrap();
    assert_eq!(doors.state(DoorId::Halo).last, None);
}

#[test]
fn invalid_preferences_are_rejected_without_partial_import() {
    let mut doors = Doors::default();
    doors.hold(DemoItem::Bolt);
    let good = doors.document();
    let revision = doors.revision();
    for bad in [
        good.replace("\"v\":1", "\"v\":2"),
        good.replace("verse-plaza", "another-world"),
        good.replace("demo-doors-v1", "demo-doors-v2"),
        good.replace("\"bolt\"", "\"admin\""),
        good.replace("[null,null]", "[\"ring\",null]"),
        good.replace("[null,null]", "[null,\"bolt\"]"),
        good.replace("[null,null]", "[null,\"empty\"]"),
        good.replace("[null,null]", "[null]"),
        good.replace("[null,null]", "[null,null,null]"),
        good.replace("\"v\":1", "\"v\":1,\"phase\":\"selected\""),
        " ".repeat(PREFERENCES_LIMIT + 1),
    ] {
        assert!(doors.restore(&bad).is_err(), "{bad}");
        assert_eq!(doors.document(), good);
        assert_eq!(doors.revision(), revision);
    }
}

#[test]
fn geometry_stays_finite_bounded_and_uses_only_the_product_palette() {
    let mut allowed: Vec<[f32; 3]> = Intensity::ALL.iter().map(|&i| palette::amber(i)).collect();
    allowed.push(palette::field());
    let (static_mesh, blockers) = geometry();
    assert_eq!(blockers.len(), 4);
    assert!(static_mesh.faces.len() < 20_000);
    for id in DoorId::ALL {
        for frame in 0..=48 {
            let effect = mesh::effect(id, frame as f32 / 48.0, u64::MAX);
            assert_eq!(
                effect.lines.len(),
                if id == DoorId::Spark { 96 } else { 192 }
            );
            assert!(effect.faces.is_empty());
            for v in effect.lines {
                let p = Vec3::from(v.pos) - id.position();
                assert!(p.is_finite());
                assert!(p.x.abs() <= 1.35 && p.z.abs() <= 0.25 && (0.0..=3.5).contains(&p.y));
                assert!(allowed.contains(&v.color));
            }
        }
    }
    let player = PlayerController::new(Vec3::ZERO, 0.0);
    for item in DemoItem::ALL {
        let mut doors = Doors::default();
        doors.hold(item);
        let dynamic = doors.mesh(&player);
        assert!(dynamic.faces.len() < 20_000);
        for v in dynamic
            .lines
            .iter()
            .chain(&dynamic.faces)
            .chain(&static_mesh.faces)
            .chain(&static_mesh.lines)
        {
            assert!(v.pos.iter().all(|p| p.is_finite()));
            assert!(allowed.contains(&v.color));
        }
        let held = held_mesh(item, &player);
        for v in held.lines.iter().chain(&held.faces) {
            assert!(Vec3::from(v.pos).length() < 1.5);
        }
    }
}

#[test]
fn intents_serialize_as_closed_native_requests() {
    assert_eq!(
        serde_json::to_value(DoorIntent::Hold(DemoItem::Empty)).unwrap(),
        serde_json::json!({"action":"door_hold","item":"empty"})
    );
    assert_eq!(
        serde_json::to_value(DoorIntent::Tap(DoorId::Spark)).unwrap(),
        serde_json::json!({"action":"door_tap","door":"spark"})
    );
    assert_eq!(
        serde_json::to_value(DoorIntent::Reset(DoorId::Halo)).unwrap(),
        serde_json::json!({"action":"door_reset","door":"halo"})
    );
}
