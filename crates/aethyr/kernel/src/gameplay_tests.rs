use super::*;

fn call(engine: &mut Engine, value: Value) -> Value {
    engine.handle_json(&serde_json::to_vec(&value).unwrap())
}
fn tick(engine: &mut Engine, count: usize) -> Value {
    let mut result = Value::Null;
    for _ in 0..count {
        result = call(engine, json!({"type":"step","elapsed":1./60.}));
        assert_eq!(result["ok"], true, "{result}");
    }
    result
}
fn cell(id: u32, min: [f32; 3], max: [f32; 3], hp: f32) -> Value {
    json!({"id":id,"sourceId":id/2,"min":min,"max":max,"hp":hp,"anchored":true,"supports":[]})
}
fn engine(objects: Value) -> Engine {
    let mut engine = Engine::default();
    assert_eq!(
        call(
            &mut engine,
            json!({"type":"init","spawn":{"position":[0.,0.,0.],"mode":"walk"}})
        )["ok"],
        true
    );
    let result = call(
        &mut engine,
        json!({"type":"gameplay_init","seed":42,"objects":objects}),
    );
    assert_eq!(result["ok"], true, "{result}");
    engine
}
fn spell(
    engine: &mut Engine,
    id: &str,
    ability: &str,
    origin: [f32; 3],
    direction: [f32; 3],
) -> Value {
    let plan = call(
        engine,
        json!({"type":"gameplay_prepare_cast","castId":id,"ability":ability,"origin":origin,"direction":direction}),
    );
    assert_eq!(plan["ok"], true, "{plan}");
    let mut request = json!({"type":"gameplay_cast","castId":id,"ability":ability,"origin":origin,"direction":direction,"expectedVersion":plan["version"]});
    request["fragmentMasks"] = full_masks(&plan);
    if ability == "destroy_castle" {
        request["castleFragments"] = json!(vec![
            [u32::MAX; 2];
            engine.gameplay.snapshot()["castle"]["total"]
                .as_u64()
                .unwrap() as usize
        ]);
    }
    let result = call(engine, request);
    assert_eq!(result["ok"], true, "{result}");
    result
}
fn full_masks(plan: &Value) -> Value {
    json!(
        plan["prospectiveDestroyed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| json!({"nodeId":id,"mask":[u32::MAX,u32::MAX]}))
            .collect::<Vec<_>>()
    )
}
fn advance_physics(e: &mut Engine, ticks: usize) {
    for _ in 0..ticks {
        e.gameplay.advance_events();
        std::sync::Arc::make_mut(&mut e.static_geometry).set_disabled(&e.gameplay.destroyed);
        e.gameplay.advance_debris(&e.static_geometry, 0., None);
    }
}
fn wall() -> Value {
    json!([
        [[-2, 0, -3], [2, 0, -3], [2, 3, -3]],
        [[-2, 0, -3], [2, 3, -3], [-2, 3, -3]]
    ])
}

#[test]
fn lightning_chips_then_breaks_matching_collision_and_retries_only_once() {
    let mut e = engine(json!([cell(0, [-2., 0., -3.], [2., 3., -2.9], 100.)]));
    assert_eq!(
        call(
            &mut e,
            json!({"type":"set_static_collision","triangles":wall(),"owners":[0,0]})
        )["ok"],
        true
    );
    let pose = e.camera.position;
    let revision = e.revision;
    let aim = call(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1.7,0],"direction":[0,0,-1]}),
    );
    assert_eq!(aim["aim"]["nodeId"], 0);
    assert!((aim["aim"]["distance"].as_f64().unwrap() - 3.).abs() < 0.001);
    let first = spell(
        &mut e,
        "first",
        "lightning_bolt",
        [0., 1.7, 0.],
        [0., 0., -1.],
    );
    assert_eq!(first["gameplay"]["destroyedCount"], 0);
    assert!(first["gameplay"]["damage"][0]["health"].as_f64().unwrap() > 0.);
    let duplicate = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"first","ability":"lightning_bolt","origin":[0,1.7,0],"direction":[0,0,-1],"expectedVersion":0}),
    );
    assert_eq!(duplicate["ok"], true);
    assert_eq!(duplicate["cast"]["duplicate"], true);
    assert_eq!(duplicate["gameplay"]["castCount"], 1);
    assert_eq!(duplicate["gameplay"]["damage"], first["gameplay"]["damage"]);
    for n in 0..20 {
        if !e.gameplay.destroyed.is_empty() {
            break;
        }
        spell(
            &mut e,
            &format!("again-{n}"),
            "lightning_bolt",
            [0., 1.7, 0.],
            [0., 0., -1.],
        );
    }
    assert!(e.gameplay.destroyed.contains(&0));
    assert_eq!(
        call(
            &mut e,
            json!({"type":"query_camera","from":[0,1,0],"to":[0,1,-6]})
        )["fraction"],
        1.0
    );
    assert_eq!(e.camera.position, pose);
    assert_eq!(e.revision, revision);
    assert_eq!(
        call(
            &mut e,
            json!({"type":"set_camera","position":[0,0,-3],"yaw":0,"pitch":0,"mode":"walk"})
        )["ok"],
        true
    );
    let denied = call(&mut e, json!({"type":"gameplay_reset"}));
    assert_eq!(denied["ok"], false);
    assert!(e.gameplay.destroyed.contains(&0));
    call(
        &mut e,
        json!({"type":"set_camera","position":[0,0,0],"yaw":0,"pitch":0,"mode":"walk"}),
    );
    let reset = call(&mut e, json!({"type":"gameplay_reset"}));
    assert_eq!(reset["ok"], true);
    assert_eq!(reset["gameplay"]["destroyedCount"], 0);
    assert!(
        call(
            &mut e,
            json!({"type":"query_camera","from":[0,1,0],"to":[0,1,-6]})
        )["fraction"]
            .as_f64()
            .unwrap()
            < 1.
    );
}

#[test]
fn a_missing_cell_support_falls_and_matches_the_read_only_cast_plan() {
    let base = cell(0, [-1., 0., -3.], [1., 2., -2.9], 1.);
    let upper = json!({"id":1,"sourceId":0,"min":[-1,3,-3],"max":[1,5,-2.9],"hp":1000,"anchored":false,"supports":[0]});
    let other = cell(2, [20., 0., -3.], [22., 2., -2.9], 27.);
    let mut e = engine(json!([base, upper, other]));
    let before = call(&mut e, json!({"type":"gameplay_snapshot"}));
    let plan = call(
        &mut e,
        json!({"type":"gameplay_prepare_cast","castId":"support","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert_eq!(plan["prospectiveDestroyed"], json!([0, 1]));
    assert_eq!(plan["sourceIds"], json!([0]));
    assert_eq!(
        call(&mut e, json!({"type":"gameplay_snapshot"}))["gameplay"],
        before["gameplay"]
    );
    let cast = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"support","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1],"expectedVersion":plan["version"],"fragmentMasks":full_masks(&plan)}),
    );
    assert_eq!(cast["ok"], true);
    let after = tick(&mut e, 1);
    assert_eq!(after["gameplay"]["destroyed"], plan["prospectiveDestroyed"]);
    assert_eq!(after["gameplay"]["debris"].as_array().unwrap().len(), 128);
    let chunks: Vec<_> = after["gameplay"]["debris"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["nodeId"] == 1)
        .map(|b| b["chunk"].as_u64().unwrap())
        .collect();
    assert_eq!(chunks, (0..64).collect::<Vec<_>>());
}

#[test]
fn meteor_uses_source_cast_flight_stagger_and_one_shared_damage_roll() {
    let mut e = engine(json!([cell(0, [-12., 0., -12.], [12., 2., 12.], 10_000.)]));
    let cast = spell(
        &mut e,
        "meteors",
        "meteor_shower",
        [0., 5., 0.],
        [0., -1., 0.],
    );
    let rolled = cast["cast"]["damage"].as_u64().unwrap();
    assert!((40..=240).contains(&rolled));
    assert_eq!(cast["gameplay"]["pendingImpacts"], 6);
    assert!((cast["gameplay"]["activeCast"]["life"].as_f64().unwrap() - 2.5).abs() < 0.00001);
    let busy = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"busy","ability":"lightning_bolt","origin":[0,5,0],"direction":[0,-1,0]}),
    );
    assert_eq!(busy["ok"], false);
    let before = tick(&mut e, 149);
    assert!(!before["gameplay"]["activeCast"].is_null());
    assert!(before["gameplay"]["effects"].as_array().unwrap().is_empty());
    let flight = tick(&mut e, 1);
    assert!(flight["gameplay"]["activeCast"].is_null());
    assert!(
        flight["gameplay"]["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["kind"] == "meteor")
    );
    assert!(
        tick(&mut e, 59)["gameplay"]["damage"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        tick(&mut e, 1)["gameplay"]["damage"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let done = tick(&mut e, 41);
    let events = done["gameplay"]["damage"].as_array().unwrap();
    assert_eq!(events.len(), 6);
    assert!(
        events
            .iter()
            .all(|event| event["amount"].as_f64().unwrap() == rolled as f64)
    );
    assert_eq!(done["gameplay"]["pendingImpacts"], 0);
}

#[test]
fn wall_meteors_stay_near_the_picked_wall_instead_of_snapping_to_another_street() {
    let objects = json!([cell(0, [-20., -20., 31.], [20., 20., 31.2], 10_000.)]);
    let mut e = engine(objects.clone());
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_init","seed":12999,"objects":objects})
        )["ok"],
        true
    );
    assert_eq!(
        call(
            &mut e,
            json!({"type":"set_static_collision","triangles":[
        [[-20,-20,31.1],[20,-20,31.1],[-20,20,31.1]],
        [[20,-20,31.1],[20,20,31.1],[-20,20,31.1]],
        [[-20,-20,23.625],[20,-20,23.625],[-20,1.5,23.625]],
        [[20,-20,23.625],[20,1.5,23.625],[-20,1.5,23.625]]
    ],"owners":[0,0,u32::MAX,u32::MAX]})
        )["ok"],
        true
    );
    let cast = spell(
        &mut e,
        "local-wall-meteors",
        "meteor_shower",
        [6., 3.45, 20.],
        [0., -0.02, 1.],
    );
    let picked = &cast["cast"]["aim"]["point"];
    assert!((picked[2].as_f64().unwrap() - 31.1).abs() < 0.001);
    let flight = tick(&mut e, 189);
    let meteors: Vec<_> = flight["gameplay"]["effects"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effect| effect["kind"] == "meteor")
        .collect();
    assert_eq!(meteors.len(), 6);
    assert!(
        meteors
            .iter()
            .any(|effect| effect["target"][1].as_f64().unwrap() < 1.5)
    );
    for meteor in meteors {
        let target = &meteor["target"];
        assert!(
            (target[2].as_f64().unwrap() - 31.1).abs() < 0.001,
            "A low foreground surface replaced the selected wall: {meteor}"
        );
        let tangent_distance = ((target[0].as_f64().unwrap() - picked[0].as_f64().unwrap())
            .powi(2)
            + (target[1].as_f64().unwrap() - picked[1].as_f64().unwrap()).powi(2))
        .sqrt();
        assert!(tangent_distance <= 3.001);
    }
}

#[test]
fn thunderbolt_is_a_delayed_sky_strike_not_the_horizontal_beam() {
    let mut e = engine(json!([cell(0, [-1., 0., -3.], [1., 3., -2.9], 1.)]));
    let cast = spell(&mut e, "sky", "thunderbolt", [0., 1.7, 0.], [0., 0., -1.]);
    let damage = cast["cast"]["damage"].as_u64().unwrap();
    assert!((20..=200).contains(&damage));
    let charging = tick(&mut e, 71);
    assert_eq!(charging["gameplay"]["destroyedCount"], 0);
    assert!(
        charging["gameplay"]["effects"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let leader = tick(&mut e, 1);
    let effects = leader["gameplay"]["effects"].as_array().unwrap();
    let sky = effects.iter().find(|e| e["kind"] == "lightning").unwrap();
    assert!(
        (sky["origin"][1].as_f64().unwrap() - sky["target"][1].as_f64().unwrap() - 60.).abs()
            < 0.001
    );
    assert_eq!(tick(&mut e, 5)["gameplay"]["destroyedCount"], 0);
    assert_eq!(tick(&mut e, 1)["gameplay"]["destroyedCount"], 1);
}

#[test]
fn aiming_uses_real_structural_triangles_and_cell_boxes_only_for_decorations() {
    let mut e = engine(json!([cell(0, [-2., 0., -8.], [2., 3., -1.], 27.)]));
    call(
        &mut e,
        json!({"type":"set_static_collision","triangles":[[[-2,0,-8],[2,0,-8],[2,3,-8]],[[-2,0,-8],[2,3,-8],[-2,3,-8]]],"owners":[0,0]}),
    );
    let aim = call(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert!((aim["aim"]["distance"].as_f64().unwrap() - 8.).abs() < 0.001);
    call(
        &mut e,
        json!({"type":"set_static_collision","triangles":[]}),
    );
    let decoration = call(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert!((decoration["aim"]["distance"].as_f64().unwrap() - 1.).abs() < 0.001);
}

#[test]
fn invalid_chunk_and_stale_plan_preserve_the_active_town() {
    let mut e = engine(json!([cell(0, [-1., 0., -3.], [1., 3., -2.9], 100.)]));
    let plan = call(
        &mut e,
        json!({"type":"gameplay_prepare_cast","castId":"old","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    spell(&mut e, "new", "lightning_bolt", [0., 1., 0.], [0., 0., -1.]);
    let before = e.gameplay.snapshot();
    let stale = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"old","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1],"expectedVersion":plan["version"]}),
    );
    assert_eq!(stale["ok"], false);
    assert_eq!(e.gameplay.snapshot(), before);
    let start = call(
        &mut e,
        json!({"type":"gameplay_init","objects":[{"id":4,"sourceId":5,"min":[1,0,1],"max":[2,2,2],"anchored":false,"supports":[5]}],"complete":false}),
    );
    assert_eq!(start["ok"], true);
    assert_eq!(e.gameplay.snapshot(), before);
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_init","objects":[],"append":true})
        )["ok"],
        false
    );
    let finish = call(
        &mut e,
        json!({"type":"gameplay_init","objects":[cell(5,[1.,0.,1.],[2.,2.,2.],27.)],"append":true}),
    );
    assert_eq!(finish["ok"], true);
    assert_eq!(finish["gameplay"]["objectCount"], 2);
}

#[test]
fn debris_is_bounded_deterministic_expires_and_never_sleeps_in_midair() {
    let objects = Value::Array(
        (0..50)
            .map(|id| cell(id, [-1., 1., -3.], [1., 3., -2.9], 1.))
            .collect(),
    );
    let mut a = engine(objects.clone());
    let mut b = engine(objects);
    let one = spell(
        &mut a,
        "burst",
        "lightning_bolt",
        [0., 1.7, 0.],
        [0., 0., -1.],
    );
    let two = spell(
        &mut b,
        "burst",
        "lightning_bolt",
        [0., 1.7, 0.],
        [0., 0., -1.],
    );
    assert_eq!(one["gameplay"], two["gameplay"]);
    assert_eq!(one["gameplay"]["debris"].as_array().unwrap().len(), 256);
    let falling = tick(&mut a, 30);
    assert_eq!(falling["gameplay"], tick(&mut b, 30)["gameplay"]);
    let airborne = falling["gameplay"]["debris"]
        .as_array()
        .unwrap()
        .iter()
        .find(|body| body["position"][1].as_f64().unwrap() > 4.)
        .unwrap();
    let airborne_id = airborne["id"].clone();
    let old_position = airborne["position"].clone();
    let next = tick(&mut a, 1);
    assert_eq!(next["gameplay"], tick(&mut b, 1)["gameplay"]);
    let moving = next["gameplay"]["debris"]
        .as_array()
        .unwrap()
        .iter()
        .find(|body| body["id"] == airborne_id)
        .unwrap();
    assert_ne!(
        moving["position"], old_position,
        "An airborne fragment stopped moving."
    );
    assert_eq!(tick(&mut a, 89)["gameplay"], tick(&mut b, 89)["gameplay"]);
    let final_state = tick(&mut a, 1100);
    assert!(
        final_state["gameplay"]["debris"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let reset = call(&mut a, json!({"type":"gameplay_reset"}));
    assert_eq!(reset["gameplay"]["castCount"], 0);
    assert!(reset["gameplay"]["effects"].as_array().unwrap().is_empty());
    let restarted = spell(
        &mut a,
        "burst",
        "lightning_bolt",
        [0., 1.7, 0.],
        [0., 0., -1.],
    );
    assert_eq!(restarted["cast"]["damage"], one["cast"]["damage"]);
}

#[test]
fn breaking_an_occupied_floor_preserves_the_cast_pose_then_gravity_uses_the_hole() {
    let mut e = engine(json!([cell(0, [-2., 1.5, -2.], [2., 1.6, 2.], 1.)]));
    call(
        &mut e,
        json!({"type":"set_static_collision","triangles":[[[-2,1.5,-2],[2,1.5,-2],[2,1.5,2]],[[-2,1.5,-2],[2,1.5,2],[-2,1.5,2]]],"owners":[0,0]}),
    );
    assert_eq!(
        call(
            &mut e,
            json!({"type":"set_camera","position":[0,1.5,0],"yaw":0,"pitch":0,"mode":"walk"})
        )["ok"],
        true
    );
    let before = e.camera.position;
    let cast = spell(
        &mut e,
        "floor",
        "lightning_bolt",
        [0., 3.12, 0.],
        [0., -1., 0.],
    );
    assert_eq!(cast["gameplay"]["destroyedCount"], 1);
    assert_eq!(e.camera.position, before);
    assert_eq!(
        call(
            &mut e,
            json!({"type":"query_camera","from":[0,3,0],"to":[0,0,0]})
        )["fraction"],
        1.0
    );
    tick(&mut e, 90);
    assert!(e.grounded);
    assert!(e.camera.position[1] < 0.001);
}

#[test]
fn resting_rubble_blocks_player_and_camera_and_reset_clears_it() {
    let mut e = engine(json!([cell(0, [-1., 1., -5.], [1., 3., -4.8], 1.)]));
    spell(
        &mut e,
        "rubble",
        "lightning_bolt",
        [0., 1.7, 0.],
        [0., 0., -1.],
    );
    tick(&mut e, 600);
    assert!(!e.gameplay.rubble_colliders().is_empty());
    let c = e.gameplay.rubble_colliders()[0].clone();
    let x = (c.min[0] + c.max[0]) * 0.5;
    let z = (c.min[2] + c.max[2]) * 0.5;
    assert!(!e.can_stand([x, c.min[1], z]));
    let camera = call(
        &mut e,
        json!({"type":"query_camera","from":[x,c.max[1]+3.,z],"to":[x,c.min[1]-3.,z]}),
    );
    assert!(camera["fraction"].as_f64().unwrap() < 1.);
    assert!((e.support_height([x, c.max[1] + 2., z], c.max[1] + 2.) - c.max[1]).abs() < 0.001);
    call(&mut e, json!({"type":"gameplay_reset"}));
    assert!(e.gameplay.rubble_colliders().is_empty());
    assert!(e.can_stand([x, c.min[1], z]));
}

fn castle_engine(count: u32, hp: f32) -> Engine {
    let objects: Vec<_> = (0..count)
        .map(|i| cell(i, [200. + i as f32 * 12., 50., -3.], [201. + i as f32 * 12., 52., -2.], hp))
        .chain([
            cell(10_000, [200., 50., -3.], [201., 52., -2.], 27.),
            json!({"id":10_001,"sourceId":10_001,"min":[200,53,-3],"max":[201,55,-2],"hp":1000,"anchored":false,"supports":[0]}),
        ])
        .collect();
    let mut e = engine(json!([]));
    let result = call(
        &mut e,
        json!({"type":"gameplay_init","seed":12999,"objects":objects,"targetGroups":{"castle":(0..count).collect::<Vec<_>>()}}),
    );
    assert_eq!(result["ok"], true, "{result}");
    e
}

#[test]
fn castle_forecast_reserves_the_whole_long_barrage_and_preserves_movement() {
    let mut e = castle_engine(90, 600.);
    let before = call(&mut e, json!({"type":"gameplay_snapshot"}));
    let request = json!({"type":"gameplay_prepare_cast","castId":"siege","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1]});
    let plan = call(&mut e, request);
    assert_eq!(plan["ok"], true, "{plan}");
    assert_eq!(plan["prospectiveDestroyed"].as_array().unwrap().len(), 91);
    assert!(
        plan["prospectiveDestroyed"]
            .as_array()
            .unwrap()
            .contains(&json!(10_001))
    );
    assert!(
        !plan["prospectiveDestroyed"]
            .as_array()
            .unwrap()
            .contains(&json!(10_000))
    );
    assert_eq!(
        call(&mut e, json!({"type":"gameplay_snapshot"}))["gameplay"],
        before["gameplay"]
    );
    let cast = spell(
        &mut e,
        "siege",
        "destroy_castle",
        [0., 1.7, 0.],
        [0., 0., 1.],
    );
    assert_eq!(cast["player"]["position"], before["player"]["position"]);
    assert!(cast["cast"]["aim"]["distance"].as_f64().unwrap() > 80.);
    for _ in 0..60 {
        let moved = call(
            &mut e,
            json!({"type":"step","elapsed":1./60.,"input":{"forward":1}}),
        );
        assert_eq!(moved["ok"], true);
    }
    assert!(e.camera.position[2] < -6.);
    assert_eq!(e.revision, before["revision"].as_u64().unwrap());
    let mut at_ten_seconds = Value::Null;
    let mut ticks = 60;
    while ticks < 12_000 {
        e.gameplay.advance_events();
        ticks += 1;
        if ticks == 600 {
            at_ten_seconds = e.gameplay.snapshot();
        }
        if ticks % 120 == 0 {
            let s = e.gameplay.snapshot();
            assert!(s["effects"].as_array().unwrap().len() <= 64);
            assert!(s["debris"].as_array().unwrap().len() <= 256);
            assert!(s["pendingImpacts"].as_u64().unwrap() <= 64);
        }
        if e.gameplay.snapshot()["castle"]["complete"] == true {
            break;
        }
    }
    assert!(at_ten_seconds["castle"]["remaining"].as_u64().unwrap() > 0);
    let completed = e.gameplay.snapshot();
    assert_eq!(completed["castle"]["remaining"], 0, "{completed}");
    assert_eq!(completed["castle"]["active"], false);
    assert_eq!(completed["destroyed"], plan["prospectiveDestroyed"]);
    assert!(ticks > 600);
    assert!(completed["castle"]["waves"].as_u64().unwrap() > 25);
}

#[test]
fn castle_cancel_stops_new_waves_allows_inflight_impacts_and_resumes_idempotently() {
    let mut e = castle_engine(12, 1000.);
    let first = spell(
        &mut e,
        "siege",
        "destroy_castle",
        [0., 1.7, 0.],
        [0., 0., 1.],
    );
    tick(&mut e, 1);
    let duplicate = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"siege","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1],"expectedVersion":0}),
    );
    assert_eq!(duplicate["cast"]["duplicate"], true);
    assert_eq!(
        duplicate["gameplay"]["castCount"],
        first["gameplay"]["castCount"]
    );
    let cancel = call(&mut e, json!({"type":"gameplay_cancel_cast"}));
    assert_eq!(cancel["gameplay"]["castle"]["cancelled"], true);
    let again = call(&mut e, json!({"type":"gameplay_cancel_cast"}));
    assert_eq!(again["gameplay"]["version"], cancel["gameplay"]["version"]);
    let after = tick(&mut e, 90);
    assert_eq!(after["gameplay"]["castle"]["waves"], 1);
    assert_eq!(after["gameplay"]["castle"]["impacts"], 6);
    assert_eq!(after["gameplay"]["pendingImpacts"], 0);
    assert_eq!(after["gameplay"]["castle"]["remaining"], 12);
    let resumed = spell(
        &mut e,
        "resume",
        "destroy_castle",
        [0., 1.7, 0.],
        [1., 0., 0.],
    );
    assert_eq!(resumed["gameplay"]["castle"]["active"], true);
    assert_eq!(resumed["gameplay"]["castle"]["cancelled"], false);
    let reset = call(&mut e, json!({"type":"gameplay_reset"}));
    assert_eq!(reset["gameplay"]["castle"]["active"], false);
    assert_eq!(reset["gameplay"]["castle"]["waves"], 0);
    assert_eq!(reset["gameplay"]["castle"]["remaining"], 12);
    assert_eq!(reset["gameplay"]["pendingImpacts"], 0);
}

#[test]
fn completed_castle_keeps_bounded_source_rubble_on_permanent_scenery() {
    let mut e = castle_engine(8, 27.);
    call(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
        [[150,60,-100],[350,60,-100],[350,60,100]],
        [[150,60,-100],[350,60,100],[150,60,100]]]}),
    );
    spell(
        &mut e,
        "siege",
        "destroy_castle",
        [0., 1.7, 0.],
        [0., 0., 1.],
    );
    let after = tick(&mut e, 1000);
    assert_eq!(after["gameplay"]["castle"]["complete"], true);
    let rubble: Vec<_> = after["gameplay"]["rubble"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|body| body["nodeId"].as_u64().unwrap() < 8)
        .collect();
    assert_eq!(after["gameplay"]["castle"]["rubbleCount"], rubble.len());
    assert!(!rubble.is_empty());
    assert!(rubble.len() <= 1024);
    for fragment in rubble {
        assert!(fragment["position"][1].as_f64().unwrap() >= 60.);
        assert!(fragment["nodeId"].as_u64().unwrap() < 8);
    }
    let unchanged = tick(&mut e, 1300);
    assert_eq!(unchanged["gameplay"]["rubble"], after["gameplay"]["rubble"]);
    assert!(!e.gameplay.rubble_colliders().is_empty());
    let reset = call(&mut e, json!({"type":"gameplay_reset"}));
    assert_eq!(reset["gameplay"]["castle"]["rubbleCount"], 0);
    assert_eq!(reset["gameplay"]["rubble"], json!([]));
    assert!(e.gameplay.rubble_colliders().is_empty());
}

#[test]
fn castle_group_validation_is_atomic_and_only_accepted_on_the_final_chunk() {
    let mut e = castle_engine(2, 27.);
    let before = call(&mut e, json!({"type":"gameplay_snapshot"}));
    for groups in [json!({"castle":[999]}), json!({"castle":[0,0]})] {
        let invalid = call(
            &mut e,
            json!({"type":"gameplay_init","objects":[cell(0,[0.,0.,-3.],[1.,2.,-2.],27.)],"targetGroups":groups}),
        );
        assert_eq!(invalid["ok"], false);
        assert_eq!(
            call(&mut e, json!({"type":"gameplay_snapshot"}))["gameplay"],
            before["gameplay"]
        );
    }
    let early = call(
        &mut e,
        json!({"type":"gameplay_init","objects":[],"complete":false,"targetGroups":{"castle":[]}}),
    );
    assert_eq!(early["ok"], false);
    let empty = call(
        &mut e,
        json!({"type":"gameplay_init","objects":[],"targetGroups":{"castle":[]}}),
    );
    assert_eq!(empty["ok"], true);
    let no_castle = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"absent","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1]}),
    );
    assert_eq!(no_castle["ok"], false);
    assert_eq!(
        call(&mut e, json!({"type":"gameplay_snapshot"}))["gameplay"]["castCount"],
        0
    );
}

#[test]
fn castle_surface_masks_are_required_atomic_idempotent_and_skip_empty_fragments() {
    let mut e = castle_engine(3, 27.);
    let before = call(&mut e, json!({"type":"gameplay_snapshot"}));
    for masks in [
        Value::Null,
        json!([[1, 0]]),
        json!([[0, 0], [0, 0], [0, 0]]),
        json!([[4294967296u64, 0], [1, 0], [1, 0]]),
    ] {
        let mut request = json!({"type":"gameplay_cast","castId":"masks","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1]});
        if !masks.is_null() {
            request["castleFragments"] = masks;
        }
        assert_eq!(call(&mut e, request)["ok"], false);
        assert_eq!(
            call(&mut e, json!({"type":"gameplay_snapshot"}))["gameplay"],
            before["gameplay"]
        );
    }
    let objects: Vec<_> = (0..3)
        .map(|i| {
            cell(
                i,
                [200. + i as f32 * 12., 50., -3.],
                [201. + i as f32 * 12., 52., -2.],
                27.,
            )
        })
        .collect();
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_init","objects":objects,"targetGroups":{"castle":[2,0,1]}})
        )["ok"],
        true
    );
    let cast = json!({"type":"gameplay_cast","castId":"visible","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1],"castleFragments":[[1,0],[0,0],[0,8]]});
    assert_eq!(call(&mut e, cast.clone())["ok"], true);
    let mut changed = cast.clone();
    changed["castleFragments"] = json!([[2, 0], [0, 0], [0, 8]]);
    assert_eq!(call(&mut e, changed)["ok"], false);
    assert_eq!(call(&mut e, cast)["cast"]["duplicate"], true);
    let complete = tick(&mut e, 1000);
    assert_eq!(complete["gameplay"]["castle"]["remaining"], 0);
    let rubble = complete["gameplay"]["rubble"].as_array().unwrap();
    assert_eq!(complete["gameplay"]["castle"]["rubbleCount"], rubble.len());
    assert_eq!(rubble.len(), 2, "{rubble:?}");
    let chunks: std::collections::BTreeSet<_> = rubble
        .iter()
        .map(|b| (b["nodeId"].as_u64().unwrap(), b["chunk"].as_u64().unwrap()))
        .collect();
    assert_eq!(chunks, [(1, 35), (2, 0)].into_iter().collect());
    call(&mut e, json!({"type":"gameplay_reset"}));
    let reused = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"reuse","ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1]}),
    );
    assert_eq!(reused["ok"], true);
}

#[test]
fn normal_fragment_masks_are_atomic_reusable_and_include_the_high_word() {
    let mut e = engine(json!([cell(0, [-1., 1., -3.], [1., 3., -2.9], 1.)]));
    let base = json!({"type":"gameplay_cast","castId":"fine","ability":"lightning_bolt",
        "origin":[0,1.7,0],"direction":[0,0,-1]});
    let before = e.gameplay.snapshot();
    for masks in [
        Value::Null,
        json!([]),
        json!([{"nodeId":9,"mask":[1,0]}]),
        json!([{"nodeId":0,"mask":[1,0]},{"nodeId":0,"mask":[1,0]}]),
        json!([{"nodeId":0,"mask":[0,4294967296u64]}]),
    ] {
        let mut request = base.clone();
        if !masks.is_null() {
            request["fragmentMasks"] = masks;
        }
        assert_eq!(call(&mut e, request)["ok"], false);
        assert_eq!(e.gameplay.snapshot(), before);
    }
    let mut request = base.clone();
    request["fragmentMasks"] = json!([{"nodeId":0,"mask":[1,2147483648u32]}]);
    let cast = call(&mut e, request.clone());
    assert_eq!(cast["ok"], true, "{cast}");
    let bodies = cast["gameplay"]["debris"].as_array().unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(
        bodies
            .iter()
            .map(|body| body["chunk"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![0, 63]
    );
    for body in bodies {
        assert_eq!(body["subdivisions"], 4);
        for axis in 0..3 {
            let span = body["sourceMax"][axis].as_f64().unwrap()
                - body["sourceMin"][axis].as_f64().unwrap();
            assert!((body["halfExtents"][axis].as_f64().unwrap() - span / 8.).abs() < 1e-6);
        }
    }
    let after = e.gameplay.snapshot();
    request["fragmentMasks"][0]["mask"] = json!([2, 0]);
    assert_eq!(call(&mut e, request)["ok"], false);
    assert_eq!(e.gameplay.snapshot(), after);
    assert_eq!(call(&mut e, base.clone())["cast"]["duplicate"], true);
    call(&mut e, json!({"type":"gameplay_reset"}));
    let mut reused = base;
    reused["castId"] = json!("reuse-fine");
    assert_eq!(call(&mut e, reused)["ok"], true);
    advance_physics(&mut e, 1500);
    let settled = e.gameplay.snapshot();
    assert_eq!(settled["rubble"].as_array().unwrap().len(), 2);
    assert_eq!(settled["castle"]["rubbleCount"], 0);
    assert!(settled["castle"]["rubble"].is_null());
    assert!(settled["debris"].as_array().unwrap().is_empty());
}

#[test]
fn shared_rubble_reservoir_keeps_1024_smaller_pieces_across_buildings_after_ttl() {
    let objects = Value::Array(
        (0..40)
            .map(|id| {
                // Separate source placements share the blast footprint.
                cell(
                    id,
                    [
                        -1. + (id % 5) as f32 * 0.25,
                        4. + (id / 5) as f32 * 0.05,
                        -3.,
                    ],
                    [
                        0.8 + (id % 5) as f32 * 0.25,
                        5.8 + (id / 5) as f32 * 0.05,
                        -2.9,
                    ],
                    1.,
                )
            })
            .collect(),
    );
    let mut a = engine(objects.clone());
    let mut b = engine(objects);
    for e in [&mut a, &mut b] {
        spell(e, "dense", "thunderbolt", [0., 5., 0.], [0., 0., -1.]);
        advance_physics(e, 80);
        let g = e.gameplay.snapshot();
        assert!(
            g["rubble"].as_array().unwrap().is_empty(),
            "Pending fragments appeared before landing"
        );
        advance_physics(e, 1600);
    }
    let one = a.gameplay.snapshot();
    let two = b.gameplay.snapshot();
    assert_eq!(one, two);
    let rubble = one["rubble"].as_array().unwrap();
    assert_eq!(rubble.len(), 1024);
    let nodes: std::collections::BTreeSet<_> = rubble
        .iter()
        .map(|body| body["nodeId"].as_u64().unwrap())
        .collect();
    assert!(
        nodes.len() > 30,
        "Reservoir retained only the final batch: {nodes:?}"
    );
    let min_x = rubble
        .iter()
        .map(|b| b["position"][0].as_f64().unwrap())
        .fold(f64::INFINITY, f64::min);
    let max_x = rubble
        .iter()
        .map(|b| b["position"][0].as_f64().unwrap())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(max_x - min_x > 8.);
    assert!(rubble.iter().all(|body| body["subdivisions"] == 4));
    assert_eq!(one["castle"]["rubbleCount"], 0);
    assert!(one["debris"].as_array().unwrap().is_empty());
    advance_physics(&mut a, 1400);
    assert_eq!(a.gameplay.snapshot()["rubble"], one["rubble"]);
    call(&mut a, json!({"type":"gameplay_reset"}));
    assert_eq!(a.gameplay.snapshot()["rubble"], json!([]));
    assert!(a.gameplay.rubble_colliders().is_empty());
}

#[test]
fn generic_masks_can_prepare_the_castle_without_the_legacy_field() {
    let mut e = castle_engine(3, 27.);
    let plan = call(
        &mut e,
        json!({"type":"gameplay_prepare_cast","castId":"generic-castle",
        "ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1]}),
    );
    let result = call(
        &mut e,
        json!({"type":"gameplay_cast","castId":"generic-castle",
        "ability":"destroy_castle","origin":[0,1.7,0],"direction":[0,0,1],
        "fragmentMasks":full_masks(&plan)}),
    );
    assert_eq!(result["ok"], true, "{result}");
    advance_physics(&mut e, 1800);
    let g = e.gameplay.snapshot();
    assert_eq!(g["castle"]["remaining"], 0);
    assert!(!g["rubble"].as_array().unwrap().is_empty());
    assert!(
        (g["castle"]["rubbleCount"].as_u64().unwrap() as usize)
            < g["rubble"].as_array().unwrap().len(),
        "Non-castle support cascade fragments should stay outside the castle count"
    );
}
