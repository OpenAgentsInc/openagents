//! Incremental prop physics uses the same validated authority and history as rooms.
use super::*;

fn request(engine: &mut Engine, value: Value) -> Value {
    engine.handle_json(&serde_json::to_vec(&value).unwrap())
}
fn edit(engine: &mut Engine, id: &str, operations: Value) -> Value {
    request(
        engine,
        json!({"type":"command","commandId":id,"baseRevision":engine.revision,"operations":operations}),
    )
}
fn prop(id: &str, position: [f32; 3]) -> Value {
    json!({"id":id,"kind":"barrel","position":position,"halfExtents":[0.5,1,0.5],"collisionOffset":[0,1,0],"collidable":true})
}
fn walk() -> Engine {
    let mut engine = Engine::default();
    assert_eq!(
        request(
            &mut engine,
            json!({"type":"init","spawn":{"position":[0,0,0],"mode":"walk"}})
        )["ok"],
        true
    );
    engine
}
fn advance(engine: &mut Engine, frames: usize) {
    for _ in 0..frames {
        assert_eq!(
            request(
                engine,
                json!({"type":"step","elapsed":1./60.,"input":{"forward":1}})
            )["ok"],
            true
        );
    }
}
fn pose(engine: &mut Engine, position: [f32; 3]) {
    let result = request(
        engine,
        json!({"type":"set_camera","position":position,"yaw":0,"pitch":0,"mode":"walk"}),
    );
    assert_eq!(result["ok"], true, "{result}");
}

#[test]
fn prop_add_move_remove_and_history_update_real_capsule_and_camera_collision() {
    let mut engine = walk();
    let added = edit(
        &mut engine,
        "add",
        json!([{"op":"add_entity","entity":prop("barrel",[0.,0.,-3.])}]),
    );
    assert_eq!(added["ok"], true, "{added}");
    assert_eq!(
        added["runtimeWork"],
        json!({"entitiesValidated":1,"propRecordsRebuilt":1,"structuralRebuilds":0,"collisionRecordsRebuilt":1})
    );
    advance(&mut engine, 90);
    assert!(engine.camera.position[2] > -2.21);
    let query = request(
        &mut engine,
        json!({"type":"query_camera","from":[0,1,0],"to":[0,1,-6]}),
    );
    assert!(query["fraction"].as_f64().unwrap() < 0.5);
    pose(&mut engine, [0., 0., 0.]);
    assert_eq!(
        edit(
            &mut engine,
            "move",
            json!([{"op":"move_entity","id":"barrel","position":[4,0,-3]}])
        )["ok"],
        true
    );
    advance(&mut engine, 90);
    assert!(engine.camera.position[2] < -5.);
    pose(&mut engine, [0., 0., 0.]);
    assert_eq!(
        edit(&mut engine, "undo-move", json!([{"op":"undo"}]))["runtimeWork"]["structuralRebuilds"],
        0
    );
    assert!(!engine.can_stand([0., 0., -3.]));
    assert_eq!(
        edit(&mut engine, "redo-move", json!([{"op":"redo"}]))["ok"],
        true
    );
    assert!(engine.can_stand([0., 0., -3.]));
    assert!(!engine.can_stand([4., 0., -3.]));
    assert_eq!(
        edit(
            &mut engine,
            "remove",
            json!([{"op":"remove_entity","id":"barrel"}])
        )["runtimeWork"]["collisionRecordsRebuilt"],
        1
    );
    assert!(engine.can_stand([4., 0., -3.]));
    assert_eq!(
        edit(&mut engine, "undo-remove", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert!(!engine.can_stand([4., 0., -3.]));
    assert_eq!(
        edit(&mut engine, "redo-remove", json!([{"op":"redo"}]))["ok"],
        true
    );
    assert!(engine.can_stand([4., 0., -3.]));
}

#[test]
fn one_prop_edit_reuses_other_collision_records_and_all_structural_outputs() {
    let mut engine = walk();
    let entities: Vec<_> = (0..100)
        .map(|i| prop(&format!("p{i}"), [10. + i as f32 * 2., 0., -5.]))
        .collect();
    assert_eq!(
        request(
            &mut engine,
            json!({"type":"init","entities":entities,"rooms":[{"id":"room","center":[20,20],"size":[6,6]}],
        "buildings":[{"id":"house","position":[30,0,30],"plan":{"rooms":[{"id":"inside","center":[0,0],"size":[4,4],"height":3,"floorY":0}],"openings":[],"supports":[{"id":"floor","center":[0,0],"size":[4,4],"floorY":0}],"ramps":[]}}],
        "spawn":{"position":[0,0,0],"mode":"walk"}})
        )["ok"],
        true
    );
    assert!(!engine.building_geometry.supports.is_empty());
    let fixed = engine.collision.0.clone();
    let walls = engine.walls.0.clone();
    let buildings = engine.building_geometry.0.clone();
    let unchanged = engine.prop_collision["p42"].clone();
    let old_changed = engine.prop_collision["p0"].clone();
    let result = edit(
        &mut engine,
        "single",
        json!([{"op":"set_entity_transform","id":"p0","position":[10,0,-8],"yaw":0.5,"scale":2}]),
    );
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        result["runtimeWork"],
        json!({"entitiesValidated":100,"propRecordsRebuilt":1,"structuralRebuilds":0,"collisionRecordsRebuilt":1})
    );
    assert!(Arc::ptr_eq(&fixed, &engine.collision.0));
    assert!(Arc::ptr_eq(&walls, &engine.walls.0));
    assert!(Arc::ptr_eq(&buildings, &engine.building_geometry.0));
    assert!(Arc::ptr_eq(&unchanged, &engine.prop_collision["p42"]));
    assert!(!Arc::ptr_eq(&old_changed, &engine.prop_collision["p0"]));
    let structural = edit(
        &mut engine,
        "room",
        json!([{"op":"add_room","room":{"id":"second","center":[40,20],"size":[6,6]}}]),
    );
    assert_eq!(structural["runtimeWork"]["structuralRebuilds"], 1);
    assert!(!Arc::ptr_eq(&walls, &engine.walls.0));
}

#[test]
fn rejected_occupied_transform_retains_history_counters_records_and_pose() {
    let mut engine = walk();
    edit(
        &mut engine,
        "add",
        json!([{"op":"add_entity","entity":prop("barrel",[4.,0.,0.])}]),
    );
    let before = request(&mut engine, json!({"type":"snapshot"}));
    let collider = engine.prop_collision["barrel"].clone();
    let rejected = edit(
        &mut engine,
        "bad",
        json!([{"op":"set_entity_transform","id":"barrel","position":[0,0,0]}]),
    );
    assert_eq!(rejected["ok"], false);
    let after = request(&mut engine, json!({"type":"snapshot"}));
    assert_eq!(before, after);
    assert!(Arc::ptr_eq(&collider, &engine.prop_collision["barrel"]));
    assert_eq!(engine.undo.len(), 1);
}

#[test]
fn prepare_only_candidates_do_not_commit_work_or_replace_shared_records() {
    let mut engine = walk();
    edit(
        &mut engine,
        "add",
        json!([{"op":"add_entity","entity":prop("barrel",[4.,0.,0.])}]),
    );
    let before = request(&mut engine, json!({"type":"snapshot"}));
    let original = engine.prop_collision["barrel"].clone();
    let prepared = request(
        &mut engine,
        json!({"type":"prepare","commandId":"planned","baseRevision":1,"operations":[{"op":"move_entity","id":"barrel","position":[7,0,0]}]}),
    );
    assert_eq!(prepared["ok"], true, "{prepared}");
    assert_eq!(prepared["entities"][0]["position"], json!([7., 0., 0.]));
    assert_eq!(prepared["runtimeWork"]["collisionRecordsRebuilt"], 1);
    assert_eq!(request(&mut engine, json!({"type":"snapshot"})), before);
    assert!(Arc::ptr_eq(&original, &engine.prop_collision["barrel"]));
    assert_eq!(engine.seen.len(), 1);
}

#[test]
fn explicit_collision_center_rotates_and_scales_and_omission_preserves_old_floor_pivots() {
    let mut engine = walk();
    let mut object = prop("offset", [10., 3., 20.]);
    object["halfExtents"] = json!([1, 2, 3]);
    object["collisionOffset"] = json!([2, 4, 1]);
    object["yaw"] = json!(FRAC_PI_2);
    object["scale"] = json!(2);
    assert_eq!(
        edit(
            &mut engine,
            "offset",
            json!([{"op":"add_entity","entity":object}])
        )["ok"],
        true
    );
    let c = &engine.prop_collision["offset"];
    for (actual, expected) in c.min.iter().zip([6., 7., 14.]) {
        assert!((actual - expected).abs() < 0.0001);
    }
    for (actual, expected) in c.max.iter().zip([18., 15., 18.]) {
        assert!((actual - expected).abs() < 0.0001);
    }
    let mut legacy = prop("legacy", [30., 2., 20.]);
    legacy.as_object_mut().unwrap().remove("collisionOffset");
    edit(
        &mut engine,
        "legacy",
        json!([{"op":"add_entity","entity":legacy}]),
    );
    assert_eq!(engine.prop_collision["legacy"].min[1], 2.);
    assert_eq!(engine.prop_collision["legacy"].max[1], 4.);
    let mut centered = prop("centered", [40., 2., 20.]);
    centered["collisionOffset"] = json!([0, 0, 0]);
    edit(
        &mut engine,
        "centered",
        json!([{"op":"add_entity","entity":centered}]),
    );
    assert_eq!(engine.prop_collision["centered"].min[1], 1.);
    assert_eq!(engine.prop_collision["centered"].max[1], 3.);
}

#[test]
fn unrelated_prop_edit_keeps_the_support_under_a_player_and_removal_is_held() {
    let mut engine = walk();
    edit(
        &mut engine,
        "platform",
        json!([{"op":"add_entity","entity":prop("platform",[0.,0.,-4.])}]),
    );
    pose(&mut engine, [0., 2., -4.]);
    assert!(engine.grounded);
    assert_eq!(
        edit(
            &mut engine,
            "other",
            json!([{"op":"add_entity","entity":prop("other",[10.,0.,0.])}])
        )["ok"],
        true
    );
    assert_eq!(
        edit(
            &mut engine,
            "remove-underfoot",
            json!([{"op":"remove_entity","id":"platform"}])
        )["ok"],
        false
    );
    assert!(engine.prop_collision.contains_key("platform"));
    assert_eq!(engine.camera.position, [0., 2., -4.]);
}

#[test]
fn authored_prop_edits_keep_druid_damage_effects_and_disabled_static_collision() {
    let mut engine = walk();
    assert_eq!(
        request(
            &mut engine,
            json!({"type":"set_static_collision","triangles":[[[-2,0,-3],[2,0,-3],[2,3,-3]],[[-2,0,-3],[2,3,-3],[-2,3,-3]]],"owners":[0,0]})
        )["ok"],
        true
    );
    assert_eq!(
        request(
            &mut engine,
            json!({"type":"gameplay_init","seed":42,"objects":[{"id":0,"sourceId":0,"min":[-2,0,-3],"max":[2,3,-2.9],"hp":12,"anchored":true,"supports":[]}]})
        )["ok"],
        true
    );
    let plan = request(
        &mut engine,
        json!({"type":"gameplay_prepare_cast","castId":"spell","ability":"lightning_bolt","origin":[0,1.7,0],"direction":[0,0,-1]}),
    );
    let cast = request(
        &mut engine,
        json!({"type":"gameplay_cast","castId":"spell","ability":"lightning_bolt","origin":[0,1.7,0],"direction":[0,0,-1],"expectedVersion":plan["version"],"fragmentMasks":[{"nodeId":0,"mask":[1,0]}]}),
    );
    assert_eq!(cast["ok"], true, "{cast}");
    assert_eq!(cast["gameplay"]["destroyedCount"], 1);
    let damage = engine.gameplay.snapshot();
    let static_state = engine.static_geometry.clone();
    for (id, operations) in [
        (
            "add",
            json!([{"op":"add_entity","entity":prop("barrel",[10.,0.,0.])}]),
        ),
        (
            "move",
            json!([{"op":"move_entity","id":"barrel","position":[12,0,0]}]),
        ),
        ("undo", json!([{"op":"undo"}])),
        ("redo", json!([{"op":"redo"}])),
        ("delete", json!([{"op":"remove_entity","id":"barrel"}])),
    ] {
        let applied = edit(&mut engine, id, operations);
        assert_eq!(applied["ok"], true, "{applied}");
        assert_eq!(engine.gameplay.snapshot(), damage);
        assert!(Arc::ptr_eq(&static_state, &engine.static_geometry));
        assert_eq!(
            request(
                &mut engine,
                json!({"type":"query_camera","from":[0,1,0],"to":[0,1,-6]})
            )["fraction"],
            1.0
        );
    }
}

#[test]
fn invalid_bounds_and_partial_batches_preserve_the_last_accepted_runtime() {
    let mut engine = walk();
    let before = request(&mut engine, json!({"type":"snapshot"}));
    let mut invalid = prop("invalid", [10., 0., 0.]);
    invalid["collisionOffset"] = json!([0, 1001, 0]);
    let rejected = edit(
        &mut engine,
        "invalid-offset",
        json!([{"op":"add_entity","entity":prop("first",[5.,0.,0.])},{"op":"add_entity","entity":invalid}]),
    );
    assert_eq!(rejected["ok"], false);
    assert_eq!(request(&mut engine, json!({"type":"snapshot"})), before);
    assert!(engine.prop_collision.is_empty());
    assert!(engine.undo.is_empty());
}

#[test]
fn offset_physics_survives_authored_save_reopen_without_copying_live_damage() {
    let mut engine = walk();
    let mut barrel = prop("saved", [10., 0., 0.]);
    barrel["collisionOffset"] = json!([1, 2, 0]);
    barrel["scale"] = json!(2);
    edit(
        &mut engine,
        "add",
        json!([{"op":"add_entity","entity":barrel}]),
    );
    let saved = request(&mut engine, json!({"type":"snapshot"}));
    let expected = &engine.prop_collision["saved"];
    let mut reopened = Engine::default();
    let loaded = request(
        &mut reopened,
        json!({"type":"init","entities":saved["entities"],"rooms":saved["rooms"],"spawn":{"position":[0,0,0],"mode":"walk"}}),
    );
    assert_eq!(loaded["ok"], true, "{loaded}");
    assert_eq!(
        loaded["entities"][0]["collisionOffset"],
        json!([1., 2., 0.])
    );
    let actual = &reopened.prop_collision["saved"];
    assert_eq!(actual.min, expected.min);
    assert_eq!(actual.max, expected.max);
    assert_eq!(reopened.revision, 0);
    assert!(reopened.undo.is_empty());
}
