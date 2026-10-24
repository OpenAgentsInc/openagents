use super::*;

fn call(engine: &mut Engine, request: Value) -> Value {
    engine.handle_json(&serde_json::to_vec(&request).unwrap())
}
fn ok(engine: &mut Engine, request: Value) -> Value {
    let reply = call(engine, request);
    assert_eq!(reply["ok"], true, "{reply}");
    reply
}
fn scene(seed: u32, count: usize) -> Engine {
    let mut engine = Engine::default();
    ok(
        &mut engine,
        json!({"type":"init","spawn":{"position":[0,0,0],"mode":"walk"}}),
    );
    let objects: Vec<_> = (0..count).map(|id| {
        let x = (id % 32) as f32 * 3.;
        let z = -4. - (id / 32) as f32 * 3.;
        json!({"id":id,"sourceId":id/2,"min":[x-0.5,0,z-0.5],"max":[x+0.5,3,z+0.5],"hp":1,"anchored":true,"supports":[]})
    }).collect();
    for (index, chunk) in objects.chunks(8192).enumerate() {
        ok(
            &mut engine,
            json!({"type":"gameplay_init","seed":seed,"objects":chunk,"append":index>0,"complete":false}),
        );
    }
    ok(
        &mut engine,
        json!({"type":"gameplay_init","objects":[],"append":true,"targetGroups":{"castle":(0..count.min(32)).collect::<Vec<_>>()}}),
    );
    engine
}
fn request(ability: &str) -> Value {
    json!({"castId":"forecast","ability":ability,"origin":[0,1,0],"direction":[0,0,-1]})
}
fn begin(engine: &mut Engine, ability: &str) -> Value {
    let mut message = request(ability);
    message["type"] = json!("gameplay_forecast_start");
    ok(engine, message)
}
fn advance(engine: &mut Engine, progress: &Value) -> Value {
    ok(
        engine,
        json!({"type":"gameplay_forecast_continue","forecastId":progress["forecastId"],"expectedVersion":progress["version"]}),
    )
}
fn complete(engine: &mut Engine, mut progress: Value) -> Value {
    let mut calls = 0;
    while progress["status"] == "pending" {
        let previous = progress["ticks"].as_u64().unwrap();
        assert!(serde_json::to_vec(&progress).unwrap().len() < 512);
        progress = advance(engine, &progress);
        assert!(progress["ticks"].as_u64().unwrap() <= previous + 1);
        calls += 1;
        assert!(calls <= 602);
    }
    progress
}
fn commit(engine: &mut Engine, ability: &str, plan: &Value) -> Value {
    let mut message = request(ability);
    message["type"] = json!("gameplay_cast");
    message["expectedVersion"] = plan["version"].clone();
    message["fragmentMasks"] = json!(
        plan["prospectiveDestroyed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| json!({"nodeId":id,"mask":[u32::MAX,u32::MAX]}))
            .collect::<Vec<_>>()
    );
    ok(engine, message)
}

#[test]
fn resumable_forecasts_match_the_original_ordering_for_every_spell_and_scene_seed() {
    for seed in [42, 12999] {
        for ability in [
            "lightning_bolt",
            "thunderbolt",
            "meteor_shower",
            "destroy_castle",
        ] {
            let mut engine = scene(seed, 96);
            let spell: gameplay::Ability = serde_json::from_value(json!(ability)).unwrap();
            let aim = engine
                .gameplay_aim(
                    [0., 1., 0.],
                    [0., 0., -1.],
                    if ability == "lightning_bolt" {
                        30.48
                    } else {
                        80.
                    },
                )
                .unwrap();
            let expected = engine
                .gameplay
                .reference_plan(
                    "forecast".into(),
                    spell,
                    [0., 1., 0.],
                    [0., 0., -1.],
                    aim,
                    &engine.static_geometry,
                )
                .unwrap();
            let before = engine.gameplay.snapshot();
            let progress = begin(&mut engine, ability);
            let plan = complete(&mut engine, progress);
            for field in [
                "version",
                "prospectiveDestroyed",
                "sourceIds",
                "cast",
                "aim",
            ] {
                assert_eq!(plan[field], expected[field], "{ability} seed{seed} {field}");
            }
            assert_eq!(engine.gameplay.snapshot(), before);
            let count = engine.gameplay.forecast_starts.get();
            let result = commit(&mut engine, ability, &plan);
            assert_eq!(
                engine.gameplay.forecast_starts.get(),
                count,
                "commit forecasted again"
            );
            assert_eq!(result["cast"], plan["cast"]);
            let duplicate = commit(&mut engine, ability, &plan);
            assert_eq!(duplicate["cast"]["duplicate"], true);
        }
    }
}

#[test]
fn player_and_enemy_ticks_continue_without_publishing_a_forecast() {
    let mut engine = scene(42, 96);
    ok(
        &mut engine,
        json!({"type":"gameplay_init_enemies","enemies":[{"id":7,"route":[[-3,0,-4],[-3,0,4]],"speed":2}]}),
    );
    let mut control = engine.clone();
    let mut progress = begin(&mut engine, "meteor_shower");
    for _ in 0..20 {
        let message = json!({"type":"step","elapsed":1./60.,"input":{"right":1}});
        assert_eq!(ok(&mut engine, message.clone()), ok(&mut control, message));
        progress = advance(&mut engine, &progress);
    }
    let plan = complete(&mut engine, progress);
    let version = engine.gameplay.version;
    assert_eq!(plan["version"], version);
    assert_eq!(engine.camera.position, control.camera.position);
    assert_eq!(engine.gameplay.snapshot(), control.gameplay.snapshot());
    let mut sync = request("meteor_shower");
    sync["type"] = json!("gameplay_prepare_cast");
    let sync = ok(&mut control, sync);
    assert_eq!(plan["prospectiveDestroyed"], sync["prospectiveDestroyed"]);
    assert_eq!(
        commit(&mut engine, "meteor_shower", &plan),
        commit(&mut control, "meteor_shower", &sync)
    );
}

#[test]
fn forged_handles_and_rejections_keep_the_valid_candidate_and_cancel_revokes_only_its_capability() {
    let mut engine = scene(42, 96);
    let progress = begin(&mut engine, "thunderbolt");
    let before = engine.gameplay.snapshot();
    for message in [
        json!({"type":"gameplay_forecast_start","castId":"other","ability":"meteor_shower","origin":[0,1,0],"direction":[0,0,-1]}),
        json!({"type":"gameplay_forecast_continue","forecastId":999,"expectedVersion":progress["version"]}),
        json!({"type":"gameplay_forecast_continue","forecastId":progress["forecastId"],"expectedVersion":999}),
        json!({"type":"gameplay_forecast_cancel","forecastId":999}),
    ] {
        assert_eq!(call(&mut engine, message)["ok"], false);
    }
    assert_eq!(engine.gameplay.snapshot(), before);
    let plan = complete(&mut engine, progress);
    assert!(engine.gameplay.is_prepared("forecast"));
    ok(
        &mut engine,
        json!({"type":"gameplay_forecast_cancel","forecastId":plan["forecastId"]}),
    );
    assert!(!engine.gameplay.is_prepared("forecast"));
    assert_eq!(engine.gameplay.snapshot(), before);
    let mut cast = request("thunderbolt");
    cast["type"] = json!("gameplay_cast");
    cast["expectedVersion"] = plan["version"].clone();
    assert_eq!(call(&mut engine, cast)["ok"], false);
}

#[test]
fn source_changes_reject_prepared_capabilities_and_invalid_collision_keeps_the_candidate() {
    let mut engine = scene(42, 96);
    let progress = begin(&mut engine, "thunderbolt");
    assert_eq!(
        call(
            &mut engine,
            json!({"type":"set_static_collision","append":true,"triangles":[[[0,0,0],[0,0,0],[0,0,0]]]})
        )["ok"],
        false
    );
    let plan = complete(&mut engine, progress);
    ok(
        &mut engine,
        json!({"type":"command","commandId":"change","baseRevision":0,"operations":[{"op":"add_entity","entity":{"id":"far","kind":"barrel","position":[50,0,50]}}]}),
    );
    let before = engine.gameplay.snapshot();
    let mut cast = request("thunderbolt");
    cast["type"] = json!("gameplay_cast");
    cast["expectedVersion"] = plan["version"].clone();
    cast["fragmentMasks"] = json!([]);
    assert_eq!(call(&mut engine, cast)["ok"], false);
    assert_eq!(engine.gameplay.snapshot(), before);
    assert_eq!(
        call(
            &mut engine,
            json!({"type":"gameplay_forecast_continue","forecastId":plan["forecastId"],"expectedVersion":plan["version"]})
        )["ok"],
        false
    );
}

fn in_flight_meteors() -> Engine {
    let mut engine = scene(42, 96);
    let mut prepare = request("meteor_shower");
    prepare["castId"] = json!("old-meteors");
    prepare["type"] = json!("gameplay_prepare_cast");
    let plan = ok(&mut engine, prepare);
    let mut cast = request("meteor_shower");
    cast["castId"] = json!("old-meteors");
    cast["type"] = json!("gameplay_cast");
    cast["expectedVersion"] = plan["version"].clone();
    cast["fragmentMasks"] = json!(
        plan["prospectiveDestroyed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| json!({"nodeId":id,"mask":[u32::MAX,u32::MAX]}))
            .collect::<Vec<_>>()
    );
    ok(&mut engine, cast);
    for _ in 0..160 {
        ok(&mut engine, json!({"type":"step","elapsed":1./60.}));
    }
    assert!(engine.gameplay.snapshot()["activeCast"].is_null());
    assert!(
        engine.gameplay.snapshot()["pendingImpacts"]
            .as_u64()
            .unwrap()
            > 0
    );
    engine
}

#[test]
fn moving_inflight_event_order_requires_resumable_final_revalidation_and_keeps_original_results() {
    let mut engine = in_flight_meteors();
    let progress = begin(&mut engine, "thunderbolt");
    let stale = complete(&mut engine, progress);
    let mut control = engine.clone();
    for _ in 0..10 {
        let step = json!({"type":"step","elapsed":1./60.});
        assert_eq!(ok(&mut engine, step.clone()), ok(&mut control, step));
    }
    assert_eq!(engine.gameplay.version, stale["version"].as_u64().unwrap());
    let before = engine.gameplay.snapshot();
    let count = engine.gameplay.forecast_starts.get();
    let mut cast = request("thunderbolt");
    cast["type"] = json!("gameplay_cast");
    cast["expectedVersion"] = stale["version"].clone();
    cast["fragmentMasks"] = json!([]);
    let denied = call(&mut engine, cast);
    assert_eq!(denied["ok"], false);
    assert!(
        denied["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timing changed")
    );
    assert_eq!(
        engine.gameplay.forecast_starts.get(),
        count,
        "resumable commit blocked synchronously"
    );
    assert_eq!(engine.gameplay.snapshot(), before);
    let restarted = advance(&mut engine, &stale);
    assert_eq!(restarted["status"], "pending");
    assert_eq!(restarted["restarts"], 1);
    assert_eq!(restarted["ticks"], 0);
    let plan = complete(&mut engine, restarted);
    let aim = control
        .gameplay_aim([0., 1., 0.], [0., 0., -1.], 80.)
        .unwrap();
    let expected = control
        .gameplay
        .reference_plan(
            "forecast".into(),
            gameplay::Ability::Thunderbolt,
            [0., 1., 0.],
            [0., 0., -1.],
            aim,
            &control.static_geometry,
        )
        .unwrap();
    for field in [
        "version",
        "prospectiveDestroyed",
        "sourceIds",
        "cast",
        "aim",
    ] {
        assert_eq!(plan[field], expected[field], "{field}");
    }
    let mut sync = request("thunderbolt");
    sync["type"] = json!("gameplay_prepare_cast");
    let sync = ok(&mut control, sync);
    let count = engine.gameplay.forecast_starts.get();
    assert_eq!(
        commit(&mut engine, "thunderbolt", &plan),
        commit(&mut control, "thunderbolt", &sync)
    );
    assert_eq!(engine.gameplay.forecast_starts.get(), count);
    for _ in 0..100 {
        let step = json!({"type":"step","elapsed":1./60.});
        assert_eq!(ok(&mut engine, step.clone()), ok(&mut control, step));
    }
}

#[test]
fn synchronous_compatibility_reforecasts_when_pending_impact_timing_moves() {
    let mut engine = in_flight_meteors();
    let mut message = request("thunderbolt");
    message["type"] = json!("gameplay_prepare_cast");
    let plan = ok(&mut engine, message);
    for _ in 0..10 {
        ok(&mut engine, json!({"type":"step","elapsed":1./60.}));
    }
    let count = engine.gameplay.forecast_starts.get();
    commit(&mut engine, "thunderbolt", &plan);
    assert_eq!(engine.gameplay.forecast_starts.get(), count + 1);
}

#[test]
#[ignore = "manual release-mode native forecast cost sample"]
fn forecast_native_cost_samples() {
    let mut samples = vec![];
    for count in [1_000, 10_000, 65_536] {
        for ability in ["lightning_bolt", "meteor_shower", "destroy_castle"] {
            let mut engine = scene(12999, count);
            let start = std::time::Instant::now();
            let mut progress = begin(&mut engine, ability);
            let begin_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut longest: f64 = 0.;
            let mut longest_tick: f64 = 0.;
            let mut total = begin_ms;
            let mut calls = 0;
            while progress["status"] == "pending" {
                let ticks = progress["ticks"].as_u64().unwrap();
                let start = std::time::Instant::now();
                progress = advance(&mut engine, &progress);
                let ms = start.elapsed().as_secs_f64() * 1000.;
                longest = longest.max(ms);
                total += ms;
                calls += 1;
                if progress["ticks"].as_u64().unwrap() > ticks {
                    longest_tick = longest_tick.max(ms);
                }
            }
            samples.push(json!({"entities":count,"ability":ability,"beginMs":begin_ms,
                "longestContinueMs":longest,"longestTickMs":longest_tick,"totalRequestMs":total,"continueCalls":calls}));
        }
    }
    println!(
        "{}",
        json!({"schema":"aethyrforge.spell-forecast.native.v1","debug":cfg!(debug_assertions),"samples":samples})
    );
}
