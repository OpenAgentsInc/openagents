use super::*;

fn call(engine: &mut Engine, request: Value) -> Value {
    engine.handle_json(&serde_json::to_vec(&request).unwrap())
}
fn ok(engine: &mut Engine, request: Value) -> Value {
    let value = call(engine, request);
    assert_eq!(value["ok"], true, "{value}");
    value
}
fn spec(id: u32, x: f32, z: f32, speed: f32) -> Value {
    json!({"id":id,"route":[[x,0,z],[x+0.1,0,z]],"speed":speed})
}
fn engine(specs: Value) -> Engine {
    let mut engine = Engine::default();
    ok(
        &mut engine,
        json!({"type":"init","spawn":{"position":[10,0,10],"mode":"fly"}}),
    );
    ok(
        &mut engine,
        json!({"type":"gameplay_init","objects":[],"seed":42}),
    );
    ok(
        &mut engine,
        json!({"type":"gameplay_init_enemies","enemies":specs}),
    );
    engine
}
fn tick(engine: &mut Engine, count: usize) -> Value {
    let mut value = ok(engine, json!({"type":"snapshot"}));
    for _ in 0..count {
        value = ok(engine, json!({"type":"step","elapsed":1./60.}));
    }
    value
}
fn cast(
    engine: &mut Engine,
    id: &str,
    ability: &str,
    origin: [f32; 3],
    direction: [f32; 3],
) -> Value {
    let plan = ok(
        engine,
        json!({"type":"gameplay_prepare_cast","castId":id,"ability":ability,"origin":origin,"direction":direction}),
    );
    ok(
        engine,
        json!({"type":"gameplay_cast","castId":id,"ability":ability,"origin":origin,"direction":direction,
        "expectedVersion":plan["version"],"fragmentMasks":plan["prospectiveDestroyed"].as_array().unwrap().iter()
        .map(|id|json!({"nodeId":id,"mask":[u32::MAX,u32::MAX]})).collect::<Vec<_>>()}),
    )
}

#[test]
fn enemy_routes_are_deterministic_bounded_and_runtime_only() {
    let specs = json!([{"id":7,"route":[[-2,0,-5],[2,0,-5]],"speed":2}]);
    let mut a = engine(specs.clone());
    let mut b = engine(specs);
    let version = a.gameplay.version;
    let a_state = tick(&mut a, 180);
    let b_state = tick(&mut b, 180);
    assert_eq!(
        a_state["gameplay"]["enemies"],
        b_state["gameplay"]["enemies"]
    );
    assert_eq!(a.gameplay.version, version);
    assert_eq!(a_state["revision"], 0);
    let actor = &a_state["gameplay"]["enemies"]["actors"][0];
    assert!(actor["position"][0].as_f64().unwrap().abs() < 1.);
    assert_eq!(actor["position"][1], 0.);
    assert!((actor["animationTime"].as_f64().unwrap() - 6. / 3.4).abs() < 0.01);
    assert_eq!(a_state["gameplay"]["enemies"]["alive"], 1);
    assert_eq!(a_state["gameplay"]["enemies"]["killed"], 0);
}

#[test]
fn invalid_enemy_batches_preserve_every_existing_actor() {
    let mut e = engine(json!([spec(0, 0., -5., 1.)]));
    let original = e.gameplay.snapshot()["enemies"].clone();
    for specs in [
        json!([spec(0, 0., -5., 1.), spec(0, 2., -5., 1.)]),
        json!(
            (0..17)
                .map(|i| spec(i, i as f32, -5., 1.))
                .collect::<Vec<_>>()
        ),
        json!([{"id":1,"route":[[0,0,0]],"speed":1}]),
        json!([{"id":1,"route":[[0,0,0],[0,0,0]],"speed":1}]),
        json!([{"id":1,"route":[[20001,0,0],[0,0,0]],"speed":1}]),
        json!([spec(1, 0., -5., 6.5)]),
        json!([{"id":1,"route":(0..33).map(|i|[i,0,0]).collect::<Vec<_>>(),"speed":1}]),
    ] {
        assert_eq!(
            call(
                &mut e,
                json!({"type":"gameplay_init_enemies","enemies":specs})
            )["ok"],
            false
        );
        assert_eq!(e.gameplay.snapshot()["enemies"], original);
    }
    assert_eq!(e.revision, 0);
}

#[test]
fn patrol_capsules_stop_at_real_triangle_walls_and_stand_on_mesh_floors() {
    let mut e = engine(json!([{"id":1,"route":[[0,0,-1],[0,0,-5]],"speed":6.4}]));
    ok(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
        [[-3,0,-3],[3,0,-3],[-3,3,-3]], [[3,0,-3],[3,3,-3],[-3,3,-3]]]}),
    );
    let blocked = tick(&mut e, 120);
    let actor = &blocked["gameplay"]["enemies"]["actors"][0];
    assert!(actor["position"][2].as_f64().unwrap() > -2.71);
    let later = tick(&mut e, 60);
    assert_eq!(
        later["gameplay"]["enemies"]["actors"][0]["position"],
        actor["position"]
    );
    assert_eq!(
        later["gameplay"]["enemies"]["actors"][0]["animationTime"],
        actor["animationTime"]
    );
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_init_enemies","enemies":[{"id":2,"route":[[0,0,-3],[1,0,-3]],"speed":1}]})
        )["ok"],
        false
    );
    ok(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
        [[-3,2,-3],[3,2,-3],[-3,2,3]], [[3,2,-3],[3,2,3],[-3,2,3]]]}),
    );
    ok(
        &mut e,
        json!({"type":"gameplay_init_enemies","enemies":[{"id":2,"route":[[0,2,0],[1,2,0]],"speed":1}]}),
    );
    assert_eq!(
        tick(&mut e, 120)["gameplay"]["enemies"]["actors"][0]["position"][1],
        2.
    );
}

#[test]
fn lightning_hits_living_capsules_but_respects_range_and_wall_cover() {
    let mut e = engine(json!([
        spec(0, 0., -5., 0.1),
        spec(1, 4., -5., 0.1),
        spec(2, 0., -6.2, 0.1)
    ]));
    ok(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
        [[-2,0,-5.5],[2,0,-5.5],[-2,3,-5.5]], [[2,0,-5.5],[2,3,-5.5],[-2,3,-5.5]]]}),
    );
    let aim = ok(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1,0],"direction":[0,0,-1],"maxDistance":30.48}),
    );
    assert_eq!(aim["aim"]["enemyId"], 0);
    assert!((aim["aim"]["distance"].as_f64().unwrap() - 4.7).abs() < 0.001);
    let hit = cast(&mut e, "hit", "lightning_bolt", [0., 1., 0.], [0., 0., -1.]);
    let actors = hit["gameplay"]["enemies"]["actors"].as_array().unwrap();
    assert_eq!(actors[0]["alive"], false);
    assert_eq!(
        actors[1]["alive"], true,
        "A miss outside the blast radius died"
    );
    assert_eq!(
        actors[2]["alive"], true,
        "A knight behind a wall died through cover"
    );
    let dead_position = actors[0]["position"].clone();
    let later = tick(&mut e, 120);
    assert_eq!(
        later["gameplay"]["enemies"]["actors"][0]["position"],
        dead_position
    );
    assert!(
        later["gameplay"]["enemies"]["actors"][0]["deathSeconds"]
            .as_f64()
            .unwrap()
            > 1.9
    );
    let aim = ok(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert!(
        aim["aim"]["enemyId"].is_null(),
        "Dead actors remain targetable"
    );
    let missed = cast(&mut e, "miss", "lightning_bolt", [0., 1., 0.], [1., 0., 0.]);
    assert_eq!(missed["gameplay"]["enemies"]["killed"], 1);
    assert_eq!(missed["revision"], 0);
    let reset = ok(&mut e, json!({"type":"gameplay_reset"}));
    assert_eq!(reset["gameplay"]["enemies"]["killed"], 0);
    assert_eq!(reset["gameplay"]["enemies"]["alive"], 3);
    assert_eq!(
        reset["gameplay"]["enemies"]["actors"][0]["position"],
        json!([0., 0., -5.])
    );
    assert_eq!(
        reset["gameplay"]["enemies"]["actors"][0]["animationTime"],
        0.
    );
}

#[test]
fn moving_targets_do_not_retarget_prepared_casts_and_can_escape_delayed_impacts() {
    let mut e = engine(json!([{"id":7,"route":[[0,0,-5],[20,0,-5]],"speed":6.4}]));
    let request =
        json!({"castId":"frozen","ability":"thunderbolt","origin":[0,1,0],"direction":[0,0,-1]});
    let mut prepare = request.clone();
    prepare["type"] = json!("gameplay_prepare_cast");
    let plan = ok(&mut e, prepare.clone());
    let version = e.gameplay.version;
    tick(&mut e, 20);
    assert_eq!(e.gameplay.version, version);
    let repeated = ok(&mut e, prepare);
    assert_eq!(repeated["aim"], plan["aim"]);
    let mut commit = request;
    commit["type"] = json!("gameplay_cast");
    commit["expectedVersion"] = plan["version"].clone();
    commit["fragmentMasks"] = json!([]);
    let result = ok(&mut e, commit.clone());
    assert_eq!(result["cast"]["aim"], plan["aim"]);
    assert_eq!(
        tick(&mut e, 100)["gameplay"]["enemies"]["alive"],
        1,
        "Delayed damage followed an escaped target"
    );
    let retry = ok(&mut e, commit);
    assert_eq!(retry["cast"]["duplicate"], true);
    assert_eq!(retry["gameplay"]["castCount"], 1);
    assert_eq!(retry["revision"], 0);
}

#[test]
fn moving_capsules_keep_the_exact_prepared_building_damage_and_fragment_set() {
    let mut e = engine(json!([]));
    ok(
        &mut e,
        json!({"type":"gameplay_init","seed":42,"objects":[
            {"id":0,"sourceId":0,"min":[-1,0,-6.1],"max":[1,3,-6],"hp":1,"anchored":true},
            {"id":1,"sourceId":1,"min":[-1,0,-7.6],"max":[1,3,-7.5],"hp":1,"anchored":true}
        ]}),
    );
    ok(
        &mut e,
        json!({"type":"gameplay_init_enemies","enemies":[{"id":7,"route":[[0,0,-5],[10,0,-5]],"speed":6.4}]}),
    );
    ok(
        &mut e,
        json!({"type":"set_static_collision","triangles":[
        [[-1,0,-6],[1,0,-6],[-1,3,-6]], [[1,0,-6],[1,3,-6],[-1,3,-6]],
        [[-1,0,-7.5],[1,0,-7.5],[-1,3,-7.5]], [[1,0,-7.5],[1,3,-7.5],[-1,3,-7.5]]
    ],"owners":[0,0,1,1]}),
    );
    let plan = ok(
        &mut e,
        json!({"type":"gameplay_prepare_cast","castId":"same-masks","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert_eq!(plan["prospectiveDestroyed"], json!([0]));
    tick(&mut e, 20);
    let moved = ok(
        &mut e,
        json!({"type":"gameplay_aim","origin":[0,1,0],"direction":[0,0,-1]}),
    );
    assert_eq!(
        moved["aim"]["nodeId"], 0,
        "The actor should have moved off the camera ray"
    );
    let applied = ok(
        &mut e,
        json!({"type":"gameplay_cast","castId":"same-masks","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1],
        "expectedVersion":plan["version"],"fragmentMasks":[{"nodeId":0,"mask":[u32::MAX,u32::MAX]}]}),
    );
    assert_eq!(applied["cast"]["aim"], plan["aim"]);
    assert_eq!(
        applied["gameplay"]["destroyed"],
        plan["prospectiveDestroyed"]
    );
    assert_eq!(
        applied["gameplay"]["enemies"]["alive"], 1,
        "The escaped actor was followed"
    );
    assert_eq!(applied["revision"], 0);
}

#[test]
fn delayed_meteors_and_thunder_kill_targets_still_inside_their_actual_impacts() {
    for ability in ["meteor_shower", "thunderbolt"] {
        let mut e = engine(json!([spec(1, 0., -5., 0.1)]));
        cast(&mut e, ability, ability, [0., 1., 0.], [0., 0., -1.]);
        assert_eq!(e.gameplay.snapshot()["enemies"]["alive"], 1);
        let done = tick(&mut e, 300);
        assert_eq!(
            done["gameplay"]["enemies"]["killed"], 1,
            "{ability}: {done}"
        );
        assert_eq!(done["revision"], 0);
    }
}

#[test]
fn delayed_area_damage_does_not_cross_an_intact_building_wall() {
    for ability in ["meteor_shower", "thunderbolt"] {
        let mut e = engine(json!([spec(1, 0., -4.5, 0.1), spec(2, 0., -6.2, 0.1)]));
        ok(
            &mut e,
            json!({"type":"set_static_collision","triangles":[
            [[-10,-10,-5.5],[10,-10,-5.5],[-10,10,-5.5]],
            [[10,-10,-5.5],[10,10,-5.5],[-10,10,-5.5]]]}),
        );
        cast(&mut e, ability, ability, [0., 1., 0.], [0., 0., -1.]);
        let done = tick(&mut e, 300);
        let actors = done["gameplay"]["enemies"]["actors"].as_array().unwrap();
        assert_eq!(
            actors[0]["alive"], false,
            "The exposed knight survived {ability}"
        );
        assert_eq!(actors[1]["alive"], true, "{ability} crossed an intact wall");
    }
}

#[test]
fn prepared_capabilities_are_bounded_and_reject_changed_or_cancelled_requests() {
    let mut e = engine(json!([spec(0, 0., -5., 0.1)]));
    let prepare = |id: &str| json!({"type":"gameplay_prepare_cast","castId":id,"ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1]});
    let first = ok(&mut e, prepare("first"));
    for i in 0..16 {
        ok(&mut e, prepare(&format!("later-{i}")));
    }
    let commit = json!({"type":"gameplay_cast","castId":"first","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1],"expectedVersion":first["version"],"fragmentMasks":[]});
    assert_eq!(call(&mut e, commit)["ok"], false);
    let plan = ok(&mut e, prepare("mismatch"));
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_cast","castId":"mismatch","ability":"lightning_bolt","origin":[1,1,0],"direction":[0,0,-1],"expectedVersion":plan["version"],"fragmentMasks":[]})
        )["ok"],
        false
    );
    ok(&mut e, json!({"type":"gameplay_cancel_cast"}));
    assert_eq!(
        call(
            &mut e,
            json!({"type":"gameplay_cast","castId":"mismatch","ability":"lightning_bolt","origin":[0,1,0],"direction":[0,0,-1],"expectedVersion":plan["version"],"fragmentMasks":[]})
        )["ok"],
        false
    );
    assert_eq!(e.gameplay.snapshot()["enemies"]["killed"], 0);
    assert_eq!(e.revision, 0);
}
