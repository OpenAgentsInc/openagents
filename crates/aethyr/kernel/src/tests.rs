use super::*;

fn request(engine: &mut Engine, value: Value) -> Value {
    engine.handle_json(&serde_json::to_vec(&value).unwrap())
}
fn command(engine: &mut Engine, id: &str, operations: Value) -> Value {
    request(
        engine,
        json!({"type":"command","commandId":id,"baseRevision":engine.revision,"operations":operations}),
    )
}
fn walk_engine() -> Engine {
    let mut e = Engine::default();
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_camera","position":[0,0,0],"yaw":0,"pitch":0,"mode":"walk"})
        )["ok"],
        true
    );
    e
}
fn advance(e: &mut Engine, frames: usize, input: Value) {
    for _ in 0..frames {
        assert_eq!(
            request(e, json!({"type":"step","elapsed":1.0/60.0,"input":input}))["ok"],
            true
        );
    }
}

#[test]
fn canonical_camera_forward_and_right() {
    let mut e = walk_engine();
    advance(&mut e, 60, json!({"forward":1}));
    assert!((e.camera.position[2] + RUN_SPEED).abs() < 0.001);
    advance(&mut e, 60, json!({"right":1}));
    assert!((e.camera.position[0] - RUN_SPEED).abs() < 0.001);
    assert_eq!(e.tick, 120);
}

#[test]
fn falls_to_ground_and_jumps_once_per_press() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"set_camera","position":[0,2,0],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    advance(&mut e, 120, json!({}));
    assert!(e.grounded);
    assert_eq!(e.camera.position[1], 0.0);
    advance(&mut e, 10, json!({"jump":true}));
    assert!(e.camera.position[1] > 0.5);
    advance(&mut e, 120, json!({"jump":true}));
    assert_eq!(e.camera.position[1], 0.0);
}

#[test]
fn thin_wall_blocks_sprint_and_slides() {
    let mut e = walk_engine();
    let init = request(
        &mut e,
        json!({"type":"init","spawn":{"position":[0,0,0],"mode":"walk"},
        "colliders":[{"id":"wall","min":[-10,0,-2],"max":[10,3,-1.9]}]}),
    );
    assert_eq!(init["ok"], true);
    advance(&mut e, 120, json!({"forward":1,"right":0.2,"sprint":true}));
    assert!(e.camera.position[2] >= -1.601);
    assert!(e.camera.position[0] > 1.0);
}

#[test]
fn diagonal_speed_is_not_faster() {
    let mut e = walk_engine();
    advance(&mut e, 60, json!({"forward":1,"right":1}));
    let p = e.camera.position;
    assert!(((p[0] * p[0] + p[2] * p[2]).sqrt() - RUN_SPEED).abs() < 0.001);
}

#[test]
fn large_elapsed_has_bounded_catch_up() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"step","elapsed":1,"input":{"forward":1}}),
    );
    assert_eq!(e.tick, 4);
    assert!(e.dropped > 0.9);
    assert!(e.camera.position[2] > -0.45);
}

#[test]
fn fly_camera_ignores_collision_and_has_vertical_movement() {
    let mut e = Engine::default();
    advance(&mut e, 60, json!({"up":1}));
    assert!((e.camera.position[1] - 3.5).abs() < 0.001);
    let tick = e.tick;
    request(
        &mut e,
        json!({"type":"set_camera","position":[100,20,-50],"yaw":1,"pitch":0.3,"mode":"fly"}),
    );
    assert_eq!(e.tick, tick);
    assert_eq!(e.camera.position, [100.0, 20.0, -50.0]);
}

#[test]
fn everglade_backpedal_sprint_and_measured_animation_velocity() {
    let mut e = walk_engine();
    advance(&mut e, 60, json!({"forward":-1,"sprint":true}));
    assert!((e.camera.position[2] - BACKPEDAL_SPEED).abs() < 0.001);
    let state = request(&mut e, json!({"type":"snapshot"}));
    assert!((state["player"]["speed"].as_f64().unwrap() - BACKPEDAL_SPEED as f64).abs() < 0.001);
    assert_eq!(state["player"]["ahead"], -1.0);
    advance(&mut e, 60, json!({"forward":1,"sprint":true}));
    assert!((e.camera.position[2] - BACKPEDAL_SPEED + RUN_SPEED * SPRINT_MULT).abs() < 0.001);
    advance(&mut e, 1, json!({}));
    assert_eq!(e.velocity, [0.; 3]);
    advance(&mut e, 30, json!({"turn":1}));
    assert!((e.camera.yaw - FRAC_PI_2).abs() < 0.001);
}

fn plane(y: f32) -> Vec<static_collision::Vertices> {
    vec![
        [[-10., y, -10.], [10., y, -10.], [10., y, 10.]],
        [[-10., y, -10.], [10., y, 10.], [-10., y, 10.]],
    ]
}

#[test]
fn static_mesh_floor_and_thin_wall_stop_a_sprinting_capsule() {
    let mut e = walk_engine();
    let mut triangles = plane(1.5);
    triangles.extend([
        [[-4., 1.5, -2.], [4., 1.5, -2.], [4., 4., -2.]],
        [[-4., 1.5, -2.], [4., 4., -2.], [-4., 4., -2.]],
    ]);
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_static_collision","triangles":triangles})
        )["triangleCount"],
        4
    );
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_camera","position":[0.,3.,0.],"yaw":0.,"pitch":0.,"mode":"walk"})
        )["ok"],
        true
    );
    advance(&mut e, 120, json!({}));
    assert!((e.camera.position[1] - 1.5).abs() < 0.001);
    assert!(e.grounded);
    advance(&mut e, 120, json!({"forward":1,"sprint":true}));
    assert!(e.camera.position[2] >= -1.701, "{:?}", e.camera.position);
    assert_eq!(e.velocity, [0.; 3]);
    let fraction = request(
        &mut e,
        json!({"type":"query_camera","from":[0,2,0],"to":[0,2,-6]}),
    )["fraction"]
        .as_f64()
        .unwrap();
    assert!(fraction > 0.31 && fraction < 0.32, "{fraction}");
}

#[test]
fn static_mesh_stair_treads_raise_the_walker_without_teleporting() {
    let mut triangles = plane(0.);
    for step in 0..6 {
        let front = -1. - step as f32 * 0.5;
        let back = front - 0.5;
        let top = (step + 1) as f32 * 0.2;
        triangles.extend([
            [[-2., top, front], [2., top, front], [2., top, back]],
            [[-2., top, front], [2., top, back], [-2., top, back]],
            [
                [-2., top - 0.2, front],
                [2., top - 0.2, front],
                [2., top, front],
            ],
            [[-2., top - 0.2, front], [2., top, front], [-2., top, front]],
        ]);
    }
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"set_static_collision","triangles":triangles}),
    );
    let mut previous = 0.;
    for _ in 0..30 {
        advance(&mut e, 1, json!({"forward":1}));
        assert!(
            (e.camera.position[1] - previous).abs() <= 0.251,
            "{:?}",
            e.camera.position
        );
        previous = e.camera.position[1];
    }
    assert!(e.camera.position[1] >= 0.999, "{:?}", e.camera.position);
    assert!(e.camera.position[2] < -3., "{:?}", e.camera.position);
}

#[test]
fn static_slope_landing_and_uphill_downhill_keep_capsule_support() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
            [[-3,0,0],[3,0,0],[3,4,-8]],
            [[-3,0,0],[3,4,-8],[-3,4,-8]]
        ]}),
    );
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_camera","position":[0,3,-1],"yaw":0,"pitch":0,"mode":"walk"})
        )["ok"],
        true
    );
    advance(&mut e, 120, json!({}));
    let lift = RADIUS * ((1.0f32 + 0.5 * 0.5).sqrt() - 1.0);
    assert!(e.grounded, "{:?}", e.camera.position);
    assert!(
        (e.camera.position[1] - (0.5 + lift)).abs() < 0.002,
        "{:?}",
        e.camera.position
    );
    for input in [json!({"forward":1}), json!({"forward":-1})] {
        for _ in 0..30 {
            advance(&mut e, 1, input.clone());
            assert!(e.grounded, "{:?}", e.camera.position);
            assert!(
                (e.camera.position[1] - (-e.camera.position[2] * 0.5 + lift)).abs() < 0.002,
                "{:?}",
                e.camera.position
            );
        }
    }
    let before = e.camera.position[1];
    advance(&mut e, 1, json!({"jump":true}));
    assert!(!e.grounded);
    assert!(e.camera.position[1] > before);
}

#[test]
fn landing_on_a_tread_edge_records_ground_and_can_continue_onto_it() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
            [[-3,1,0],[3,1,0],[3,1,-3]],
            [[-3,1,0],[3,1,-3],[-3,1,-3]]
        ]}),
    );
    request(
        &mut e,
        json!({"type":"set_camera","position":[0,3,0.15],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    advance(&mut e, 120, json!({}));
    let expected = 1.0 + (RADIUS * RADIUS - 0.15 * 0.15).sqrt() - RADIUS;
    assert!(e.grounded, "{:?}", e.camera.position);
    assert!(
        (e.camera.position[1] - expected).abs() < 0.002,
        "{:?}",
        e.camera.position
    );
    advance(&mut e, 20, json!({"forward":1}));
    assert!(e.grounded);
    assert!(e.camera.position[2] < -1.9, "{:?}", e.camera.position);
    assert!((e.camera.position[1] - 1.).abs() < 0.002);
}

#[test]
fn a_tread_edge_too_steep_to_stand_on_slides_down_instead_of_hanging() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
            [[-3,1,0],[3,1,0],[3,1,-3]],
            [[-3,1,0],[3,1,-3],[-3,1,-3]]
        ]}),
    );
    request(
        &mut e,
        json!({"type":"set_camera","position":[0,3,0.27],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    advance(&mut e, 120, json!({}));
    assert!(e.grounded, "{:?}", e.camera.position);
    assert!(e.camera.position[1] < 0.002, "{:?}", e.camera.position);
    assert!(e.camera.position[2] > 0.299, "{:?}", e.camera.position);
}

#[test]
fn static_collision_chunks_are_atomic_and_survive_world_and_camera_changes() {
    let mut e = walk_engine();
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_static_collision","triangles":plane(1.5)})
        )["ok"],
        true
    );
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_static_collision","append":true,"triangles":[[[0,0,0],[0,0,0],[0,0,0]]]})
        )["ok"],
        false
    );
    assert_eq!(e.static_geometry.len(), 2);
    assert_eq!(
        request(
            &mut e,
            json!({"type":"init","spawn":{"position":[0.,1.5,0.],"mode":"walk"}})
        )["ok"],
        true
    );
    assert_eq!(e.static_geometry.len(), 2);
    assert!(e.grounded);
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_camera","position":[0.,1.4,0.],"yaw":0.,"pitch":0.,"mode":"walk"})
        )["ok"],
        false
    );
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_static_collision","triangles":[]})
        )["triangleCount"],
        0
    );
    assert_eq!(
        request(
            &mut e,
            json!({"type":"query_camera","from":[0,0,0],"to":[0,0,10000]})
        )["ok"],
        false
    );
}

#[test]
fn failed_batch_does_not_partially_add_or_advance_revision() {
    let mut e = Engine::default();
    let result = command(
        &mut e,
        "bad-batch",
        json!([
            {"op":"add_entity","entity":{"id":"box","kind":"crate","position":[2,0,2]}},
            {"op":"move_entity","id":"missing","position":[0,0,0]}
        ]),
    );
    assert_eq!(result["ok"], false);
    assert!(e.world.entities.is_empty());
    assert_eq!(e.revision, 0);
}

#[test]
fn command_retry_is_idempotent_and_conflicting_identity_fails() {
    let mut e = Engine::default();
    let value = json!({"type":"command","commandId":"one","baseRevision":0,"operations":[
        {"op":"add_entity","entity":{"id":"box","kind":"crate","position":[2,0,2]}}
    ]});
    assert_eq!(request(&mut e, value.clone())["ok"], true);
    assert_eq!(request(&mut e, value.clone())["duplicate"], true);
    assert_eq!(e.world.entities.len(), 1);
    assert_eq!(e.revision, 1);
    let mut changed = value;
    changed["operations"][0]["entity"]["position"] = json!([4, 0, 2]);
    assert_eq!(
        request(&mut e, changed)["error"]["code"],
        "command_conflict"
    );
}

#[test]
fn room_edit_has_open_entrance_and_shared_doorway() {
    let mut e = walk_engine();
    assert_eq!(
        command(
            &mut e,
            "room-a",
            json!([{"op":"add_room","room":{"id":"a","center":[0,0],"size":[6,6]}}])
        )["ok"],
        true
    );
    assert!(can_occupy([0.0, 0.0, 3.0], &e.collision));
    assert!(!can_occupy([2.0, 0.0, 3.0], &e.collision));
    assert_eq!(
        command(
            &mut e,
            "room-b",
            json!([{"op":"add_room","room":{"id":"b","center":[0,-6],"size":[6,6]}}])
        )["ok"],
        true
    );
    assert!(can_occupy([0.0, 0.0, -3.0], &e.collision));
    assert!(!can_occupy([2.0, 0.0, -3.0], &e.collision));
    assert_eq!(e.walls.iter().filter(|w| w.id == "b:south").count(), 0);
    assert_eq!(
        command(
            &mut e,
            "open-north",
            json!([{"op":"remove_wall","wallId":"a:north"}])
        )["ok"],
        true
    );
    assert!(can_occupy([2.0, 0.0, -3.0], &e.collision));
}

#[test]
fn rebuilding_wall_through_player_is_rejected_without_losing_history() {
    let mut e = walk_engine();
    command(
        &mut e,
        "room",
        json!([{"op":"add_room","room":{"id":"a","center":[0,0],"size":[6,6]}}]),
    );
    command(
        &mut e,
        "open",
        json!([{"op":"remove_wall","wallId":"a:east"}]),
    );
    request(
        &mut e,
        json!({"type":"set_camera","position":[3,0,0],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    let rev = e.revision;
    assert_eq!(
        command(&mut e, "undo-blocked", json!([{"op":"undo"}]))["ok"],
        false
    );
    assert_eq!(e.revision, rev);
    assert_eq!(e.undo.len(), 2);
    assert_eq!(e.camera.position, [3.0, 0.0, 0.0]);
    request(
        &mut e,
        json!({"type":"set_camera","position":[0,0,0],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    assert_eq!(
        command(&mut e, "undo-safe", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert!(!e.world.removed.contains("a:east"));
}

#[test]
fn invalid_init_keeps_working_scene_and_pose() {
    let mut e = walk_engine();
    let position = e.camera.position;
    let result = request(
        &mut e,
        json!({"type":"init","entities":[{"id":"bad","kind":"box","position":[0,0,0],"scale":-1}]}),
    );
    assert_eq!(result["ok"], false);
    assert_eq!(e.camera.position, position);
    assert_eq!(
        request(
            &mut e,
            json!({"type":"step","elapsed":0.1,"input":{"forward":2}})
        )["ok"],
        false
    );
    assert_eq!(e.tick, 0);
}

#[test]
fn step_summary_does_not_mirror_every_entity() {
    let mut e = Engine::default();
    let result = request(&mut e, json!({"type":"step","elapsed":1.0/60.0}));
    assert!(result.get("entities").is_none());
    assert!(result.get("rooms").is_none());
    assert!(result["player"]["eye"].is_array());
}

#[test]
fn jump_pressed_between_ticks_is_not_lost() {
    let mut e = walk_engine();
    request(
        &mut e,
        json!({"type":"step","elapsed":0,"input":{"jump":true}}),
    );
    advance(&mut e, 1, json!({"jump":true}));
    assert!(e.camera.position[1] > 0.0);
}

#[test]
fn undo_room_removal_restores_shared_wall_owner() {
    let mut e = Engine::default();
    command(
        &mut e,
        "two-rooms",
        json!([
            {"op":"add_room","room":{"id":"a","center":[0,0],"size":[6,6]}},
            {"op":"add_room","room":{"id":"b","center":[0,-6],"size":[6,6]}}
        ]),
    );
    let before = serde_json::to_vec(&e.walls).unwrap();
    command(&mut e, "remove-a", json!([{"op":"remove_room","id":"a"}]));
    assert_eq!(
        command(&mut e, "undo-remove", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert_eq!(serde_json::to_vec(&e.walls).unwrap(), before);
}

#[test]
fn room_defaults_to_scene_ground_level() {
    let mut e = Engine::default();
    assert_eq!(
        request(
            &mut e,
            json!({"type":"init","groundY":2,"rooms":[
                {"id":"raised","center":[0,0],"size":[6,6]}
            ]})
        )["ok"],
        true
    );
    assert_eq!(e.world.rooms[0].floor_y, 2.0);
}

#[test]
fn implicit_shared_wall_cannot_report_a_false_removal() {
    let mut e = Engine::default();
    command(
        &mut e,
        "two-rooms",
        json!([
            {"op":"add_room","room":{"id":"a","center":[0,0],"size":[6,6]}},
            {"op":"add_room","room":{"id":"b","center":[0,-6],"size":[6,6]}}
        ]),
    );
    let revision = e.revision;
    assert_eq!(
        command(
            &mut e,
            "wrong-owner",
            json!([{"op":"remove_wall","wallId":"b:south"}])
        )["ok"],
        false
    );
    assert_eq!(e.revision, revision);
}

fn house_plan(upstairs: bool) -> Value {
    let mut rooms = vec![
        json!({"id":"f0-front","center":[0,2.5],"size":[8,5],"height":4,"floorY":0}),
        json!({"id":"f0-back","center":[0,-2.5],"size":[8,5],"height":4,"floorY":0}),
    ];
    let mut openings = vec![
        json!({"roomId":"f0-front","side":"south","center":-2,"width":1.74,"bottom":0,"height":2.6}),
        json!({"roomId":"f0-front","side":"north","center":0,"width":2,"bottom":0,"height":2.6}),
    ];
    let mut supports = vec![json!({"id":"ground","center":[0,0],"size":[8,10],"floorY":0})];
    let mut ramps = vec![];
    if upstairs {
        rooms.push(json!({"id":"f1-front","center":[0,2.5],"size":[8,5],"height":4,"floorY":4}));
        rooms.push(json!({"id":"f1-back","center":[0,-2.5],"size":[8,5],"height":4,"floorY":4}));
        openings.push(json!({"roomId":"f1-front","side":"north","center":0,"width":2,"bottom":0,"height":2.6}));
        for floor in [0, 1] {
            openings.push(json!({"roomId":format!("f{floor}-front"),"side":"north","center":3.025,"width":1.45,"bottom":0,"height":4}));
        }
        supports.extend([
            json!({"id":"upper-left","center":[-0.85,0],"size":[6.3,10],"floorY":4}),
            json!({"id":"upper-right","center":[3.875,0],"size":[0.25,10],"floorY":4}),
            json!({"id":"landing","center":[3.025,-4.5],"size":[1.45,1],"floorY":4}),
            json!({"id":"upper-front","center":[3.025,4],"size":[1.45,2],"floorY":4}),
        ]);
        ramps.push(json!({"id":"stairs","from":[3.025,0,3],"to":[3.025,4,-4],"width":1.45}));
    }
    json!({"rooms":rooms,"openings":openings,"supports":supports,"ramps":ramps})
}

fn building_engine(upstairs: bool) -> Engine {
    let mut e = Engine::default();
    let response = request(
        &mut e,
        json!({"type":"init","groundY":1.55,"entities":[{
        "id":"house","kind":"townhouse","position":[0,1.55,0],"buildingPlan":house_plan(upstairs)
    }],"spawn":{"position":[-2,1.55,6],"mode":"walk"}}),
    );
    assert_eq!(response["ok"], true, "{response}");
    e
}

#[test]
fn cottage_uses_offset_front_entrance_and_interior_partition_door() {
    let mut e = building_engine(false);
    advance(&mut e, 80, json!({"forward":1}));
    assert!(e.camera.position[2] < 2.0, "{:?}", e.camera.position);
    request(
        &mut e,
        json!({"type":"set_camera","position":[-2,1.55,1.2],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    advance(&mut e, 19, json!({"right":1}));
    advance(&mut e, 60, json!({"forward":1}));
    assert!(e.camera.position[2] < -1.0, "{:?}", e.camera.position);
    assert!(!e.can_stand([0.0, 1.55, 5.0]));
    assert!(
        e.walls
            .iter()
            .any(|w| w.id == "house:f0-front:south" && w.local_min.unwrap()[1] >= 2.6)
    );
}

#[test]
fn building_yaw_and_scale_match_gltf_model_transform() {
    let mut e = Engine::default();
    assert_eq!(
        request(
            &mut e,
            json!({"type":"init","groundY":1.55,"entities":[{
        "id":"rotated","kind":"cottage","position":[20,1.55,10],"yaw":FRAC_PI_2,"scale":1.5,"buildingPlan":house_plan(false)
    }],"spawn":{"position":[29,1.55,13],"yaw":-FRAC_PI_2,"mode":"walk"}})
        )["ok"],
        true
    );
    advance(&mut e, 80, json!({"forward":1}));
    assert!(e.camera.position[0] < 27.0, "{:?}", e.camera.position);
    assert!(!e.can_stand([27.5, 1.55, 10.0]));
}

#[test]
fn townhouse_stair_ramp_reaches_upper_floor_and_descends_without_teleport() {
    let mut e = building_engine(true);
    assert_eq!(
        request(
            &mut e,
            json!({"type":"set_camera","position":[3.025,1.55,3.2],"yaw":0,"pitch":0,"mode":"walk"})
        )["ok"],
        true
    );
    let mut previous = e.camera.position[1];
    for _ in 0..135 {
        advance(&mut e, 1, json!({"forward":1}));
        assert!(
            (e.camera.position[1] - previous).abs() <= 0.251,
            "{:?}",
            e.camera.position
        );
        previous = e.camera.position[1];
    }
    assert!(
        (e.camera.position[1] - 5.55).abs() < 0.02,
        "{:?}",
        e.camera.position
    );
    assert!(e.camera.position[2] < -4.0);
    let revision = e.revision;
    assert_eq!(
        command(
            &mut e,
            "remove-occupied-floor",
            json!([{"op":"remove_entity","id":"house"}])
        )["ok"],
        false
    );
    assert_eq!(e.revision, revision);
    advance(&mut e, 130, json!({"forward":-1}));
    assert!(
        (e.camera.position[1] - 1.55).abs() < 0.15,
        "{:?}",
        e.camera.position
    );
}

#[test]
fn saved_building_wall_removal_restores_and_other_instance_is_independent() {
    let mut e = building_engine(false);
    let second = command(
        &mut e,
        "second-house",
        json!([{"op":"add_entity","entity":{
            "id":"other-house","kind":"cottage","position":[20,1.55,0],"buildingPlan":house_plan(false)
        }}]),
    );
    assert_eq!(second["ok"], true, "{second}");
    let edit = command(
        &mut e,
        "remove-wall",
        json!([{"op":"remove_wall","wallId":"house:f0-front:east"}]),
    );
    assert_eq!(edit["ok"], true, "{edit}");
    let mut reopened = Engine::default();
    assert_eq!(
        request(
            &mut reopened,
            json!({"type":"init","groundY":1.55,"entities":e.world.entities,"removedWalls":["house:f0-front:east"]})
        )["ok"],
        true
    );
    assert!(!reopened.walls.iter().any(|w| w.id == "house:f0-front:east"));
    assert!(
        reopened
            .walls
            .iter()
            .any(|w| w.id == "other-house:f0-front:east")
    );
    assert_eq!(
        request(
            &mut reopened,
            json!({"type":"init","removedWalls":["missing:north"]})
        )["ok"],
        false
    );
}

#[test]
fn deleting_then_undoing_a_building_restores_its_wall_edits() {
    let mut e = building_engine(false);
    command(
        &mut e,
        "open",
        json!([{"op":"remove_wall","wallId":"house:f0-front:east"}]),
    );
    request(
        &mut e,
        json!({"type":"set_camera","position":[0,20,20],"yaw":0,"pitch":0,"mode":"fly"}),
    );
    assert_eq!(
        command(
            &mut e,
            "delete",
            json!([{"op":"remove_entity","id":"house"}])
        )["ok"],
        true
    );
    assert!(e.world.removed.is_empty());
    assert_eq!(
        command(&mut e, "undo-delete", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert!(e.world.removed.contains("house:f0-front:east"));
    assert!(!e.walls.iter().any(|wall| wall.id == "house:f0-front:east"));
}

#[test]
fn authoring_and_geometry_share_atomic_history_without_moving_the_player() {
    let mut e = walk_engine();
    let pose = e.camera.position;
    let group = json!([
        {"op":"add_entity","entity":{"id":"pumpkin-1","kind":"pumpkin","position":[4,0,4]}},
        {"op":"set_authoring","state":{"environment":{"fogDensity":0.01},"materials":{"trim":{"roughness":0.8}},"effect":"fn effect() {}"}}
    ]);
    let envelope =
        json!({"type":"command","commandId":"group","baseRevision":0,"operations":group});
    assert_eq!(request(&mut e, envelope.clone())["revision"], 1);
    assert_eq!(request(&mut e, envelope)["duplicate"], true);
    assert_eq!(e.revision, 1);
    assert_eq!(command(&mut e, "undo", json!([{"op":"undo"}]))["ok"], true);
    assert!(e.world.entities.is_empty());
    assert!(e.world.authoring.effect.is_none());
    assert_eq!(command(&mut e, "redo", json!([{"op":"redo"}]))["ok"], true);
    assert_eq!(e.world.entities.len(), 1);
    assert_eq!(e.world.authoring.environment["fogDensity"], 0.01);
    let bad = json!([{"op":"set_authoring","state":{"effect":"x"}},{"op":"remove_entity","id":"missing"}]);
    assert_eq!(command(&mut e, "bad", bad)["ok"], false);
    assert_eq!(e.world.authoring.effect.as_deref(), Some("fn effect() {}"));
    assert_eq!(e.camera.position, pose);
}

#[test]
fn saved_content_replaces_one_generation_clears_local_undo_and_recovers_a_blocked_pose() {
    let mut e = walk_engine();
    command(
        &mut e,
        "old-local",
        json!([{"op":"add_entity","entity":{"id":"old","kind":"prop","position":[5,0,5]}}]),
    );
    let replacement = json!([{"op":"replace_content","entities":[{"id":"saved","kind":"prop","position":[0,0,0],"halfExtents":[1,1,1],"collidable":true}],"rooms":[],"removedWalls":[],"authoring":{"environment":{"fogDensity":0.01},"materials":{},"effect":"saved effect"},"checkpoint":{"position":[0,0,0],"yaw":0,"pitch":0,"mode":"walk"},"fallback":{"position":[0,0,8],"yaw":0,"pitch":0,"mode":"walk"}}]);
    let prepared = request(
        &mut e,
        json!({"type":"prepare","commandId":"load","baseRevision":1,"operations":replacement}),
    );
    assert_eq!(prepared["ok"], true);
    assert_eq!(e.world.entities[0].id, "old");
    assert_eq!(e.camera.position, [0., 0., 0.]);
    let accepted = command(&mut e, "load", replacement);
    assert_eq!(accepted["ok"], true);
    assert_eq!(e.world.entities[0].id, "saved");
    assert_eq!(e.camera.position, [0., 0., 8.]);
    assert!(e.undo.is_empty() && e.redo.is_empty());
    assert_eq!(e.world.authoring.effect.as_deref(), Some("saved effect"));
    let revision = e.revision;
    let invalid = json!([{"op":"replace_content","entities":[{"id":"bad","kind":"prop","position":[0,0,8],"halfExtents":[10,10,10],"collidable":true}],"rooms":[],"removedWalls":[],"authoring":{},"checkpoint":{"position":[0,0,8],"mode":"walk"},"fallback":{"position":[0,0,9],"mode":"walk"}}]);
    assert_eq!(command(&mut e, "blocked", invalid)["ok"], false);
    assert_eq!(e.revision, revision);
    assert_eq!(e.world.entities[0].id, "saved");
    assert_eq!(e.camera.position, [0., 0., 8.]);
}

#[test]
fn face_source_references_share_undo_and_reject_oversized_source_before_changing_state() {
    let mut engine = walk_engine();
    let pose = engine.camera.position;
    let key = "a".repeat(64);
    let text = format!(
        "{{\n  \"schema\": \"fixture-face-source\"\n{} }}",
        " ".repeat(5000)
    );
    let source =
        json!({"name":format!("material-v1-{key}.json"),"sha256":"b".repeat(64),"text":text});
    let state = json!({"materialSources":{key.clone():source}});
    assert_eq!(
        command(
            &mut engine,
            "face",
            json!([{"op":"set_authoring","state":state}])
        )["ok"],
        true
    );
    assert_eq!(engine.world.authoring.material_sources[&key].text, text);
    assert_eq!(engine.camera.position, pose);
    assert_eq!(
        command(&mut engine, "undo-face", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert!(engine.world.authoring.material_sources.is_empty());
    assert_eq!(
        command(&mut engine, "redo-face", json!([{"op":"redo"}]))["ok"],
        true
    );
    let revision = engine.revision;
    let saved = serde_json::to_value(&engine.world.authoring).unwrap();
    let mut invalid = state;
    invalid["materialSources"][&key]["text"] = json!("x".repeat(24 * 1024 + 1));
    assert_eq!(
        command(
            &mut engine,
            "oversize-face",
            json!([{"op":"set_authoring","state":invalid}])
        )["ok"],
        false
    );
    assert_eq!(
        serde_json::to_value(&engine.world.authoring).unwrap(),
        saved
    );
    assert_eq!(engine.revision, revision);
    assert_eq!(engine.camera.position, pose);
}

#[test]
fn planet_source_follows_history_and_invalid_imports_keep_the_working_world() {
    let mut engine = walk_engine();
    let source = |text: &str| json!({"name":"planet-v1.json","text":text,"sha256":"a".repeat(64)});
    let state = |text: &str| json!({"planetSource":source(text),"environment":{},"materials":{},"effect":null});
    assert_eq!(
        command(
            &mut engine,
            "planet-a",
            json!([{"op":"set_authoring","state":state("{\"seed\":1}")}])
        )["ok"],
        true
    );
    assert_eq!(
        command(
            &mut engine,
            "planet-b",
            json!([{"op":"set_authoring","state":state("{\"seed\":2}")}])
        )["ok"],
        true
    );
    assert_eq!(
        command(&mut engine, "planet-undo", json!([{"op":"undo"}]))["ok"],
        true
    );
    assert_eq!(
        engine.world.authoring.planet_source.as_ref().unwrap().text,
        "{\"seed\":1}"
    );
    assert_eq!(
        command(&mut engine, "planet-redo", json!([{"op":"redo"}]))["ok"],
        true
    );
    let before = engine.revision;
    let pose = engine.camera.position;
    let text = format!("{{\"padding\":\"{}\"}}", "x".repeat(24 * 1024));
    assert_eq!(
        command(
            &mut engine,
            "planet-too-large",
            json!([{"op":"set_authoring","state":state(&text)}])
        )["ok"],
        false
    );
    assert_eq!(engine.revision, before);
    assert_eq!(engine.camera.position, pose);
    assert_eq!(
        engine.world.authoring.planet_source.as_ref().unwrap().text,
        "{\"seed\":2}"
    );
}
