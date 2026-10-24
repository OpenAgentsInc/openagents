use serde_json::{Value, json};

use crate::*;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/vectors.json");
const WEB_KNOBS: &str = include_str!("../fixtures/web-knobs.json");
const WEB_PROFILE: &str = include_str!("../fixtures/profile-web.json");

fn web_knobs() -> KnobSet {
    serde_json::from_str(WEB_KNOBS).expect("web knob declarations")
}

fn config(overrides: Value) -> ControllerConfig {
    let mut base = serde_json::to_value(ControllerConfig::default()).unwrap();
    for (key, value) in overrides.as_object().unwrap() {
        base[key] = value.clone();
    }
    serde_json::from_value(base).unwrap()
}

fn pacing(ms: f64) -> Sample {
    Sample {
        interval_ms: ms,
        gpu_ms: None,
    }
}

fn gpu(ms: f64) -> Sample {
    Sample {
        interval_ms: Budget::FULL_MS,
        gpu_ms: Some(ms),
    }
}

fn feed(controller: &mut Controller, frames: usize, sample: Sample) {
    for _ in 0..frames {
        controller.observe(sample);
    }
}

/// A frame whose cost grows with `Detail`; vertical sync rounds short frames
/// up to the budget.
fn plant(controller: &Controller, base_ms: f64, per_detail_ms: f64, with_gpu: bool) -> Sample {
    let cost = base_ms + per_detail_ms * controller.detail().get();
    let budget = controller.config().budget_ms;
    let interval_ms = if cost <= budget { budget } else { cost };
    Sample {
        interval_ms,
        gpu_ms: with_gpu.then_some(cost),
    }
}

// Controller: hysteresis.

#[test]
fn holds_full_detail_at_budget() {
    let mut c = Controller::new(ControllerConfig::default());
    feed(&mut c, 5_000, pacing(16.667));
    assert_eq!(c.detail(), Detail::FULL);
    assert_eq!(c.state().changes, 0);
    assert_eq!(c.state().pressure, Pressure::Holding);
}

#[test]
fn warmup_ignores_first_frames() {
    let mut c = Controller::new(ControllerConfig::default());
    feed(&mut c, 89, pacing(40.0));
    assert_eq!(
        c.detail(),
        Detail::FULL,
        "60 warmup frames plus a partial window"
    );
    c.observe(pacing(40.0));
    assert!(c.detail() < Detail::FULL);
}

#[test]
fn dead_band_neither_lowers_nor_raises() {
    // Between the GPU raise ratio (75%) and the lower ratio (110%).
    let mut c = Controller::new(config(json!({ "start": 0.5 })));
    let budget = c.config().gpu_budget_ms;
    feed(&mut c, 20_000, gpu(budget * 1.0));
    assert_eq!(c.detail().get(), 0.5);
    assert_eq!(c.state().changes, 0);
}

#[test]
fn lowers_faster_than_it_raises() {
    let cfg = ControllerConfig::default();
    assert!(cfg.step_down > cfg.step_up);
    let mut c = Controller::new(cfg);
    feed(&mut c, 60 + 30, pacing(19.0));
    assert_eq!(c.detail().get(), 0.9);
    let mut c = Controller::new(ControllerConfig::default());
    feed(&mut c, 60 + 30, pacing(30.0));
    assert_eq!(c.detail().get(), 0.8, "far over budget takes a double step");
}

#[test]
fn raising_needs_a_run_of_good_windows() {
    let mut c = Controller::new(config(json!({ "start": 0.5 })));
    feed(&mut c, 60 + 30 * 3, gpu(4.0));
    assert_eq!(c.detail().get(), 0.5);
    feed(&mut c, 30, gpu(4.0));
    assert_eq!(c.detail().get(), 0.55);
    // One bad window in the run restarts it.
    let budget = c.config().gpu_budget_ms;
    feed(&mut c, 30 * 3, gpu(4.0));
    feed(&mut c, 30, gpu(budget));
    feed(&mut c, 30 * 3, gpu(4.0));
    assert_eq!(c.detail().get(), 0.55);
    feed(&mut c, 30, gpu(4.0));
    assert_eq!(c.detail().get(), 0.6);
}

#[test]
fn single_spikes_do_not_lower() {
    // Two slow frames in each window of 30 stay under the 90th percentile.
    let mut c = Controller::new(ControllerConfig::default());
    for frame in 0..10_000 {
        c.observe(pacing(if frame % 15 == 0 { 50.0 } else { 16.667 }));
    }
    assert_eq!(c.detail(), Detail::FULL);
}

#[test]
fn edge_workload_does_not_oscillate() {
    // GPU cost crosses the lower threshold between two levels: raising to
    // the upper level always overloads it.
    let mut c = Controller::new(config(json!({ "start": 0.5 })));
    let budget = c.config().gpu_budget_ms;
    let mut directions = Vec::new();
    let mut last = c.detail().get();
    for _ in 0..200_000 {
        let d = c.detail().get();
        let cost = if d > 0.5 { budget * 1.2 } else { budget * 0.5 };
        c.observe(gpu(cost));
        let now = c.detail().get();
        if now != last {
            directions.push(now > last);
            last = now;
        }
    }
    let flips = directions.windows(2).filter(|w| w[0] != w[1]).count();
    // Probes do happen, but backoff spaces them to every 64 windows: at most
    // about 200000 / (30 * 64) round trips, never one per window.
    assert!(
        flips <= 2 * 200_000 / (30 * 64) + 4,
        "{flips} direction changes"
    );
    assert_eq!(c.state().raise_after, c.config().max_raise_after);
    // It sits on the lower level, or on a probe of the upper one.
    assert!(c.detail().get() <= 0.55);
}

#[test]
fn backoff_relaxes_after_a_long_quiet_stretch() {
    let mut c = Controller::new(config(json!({ "start": 0.5 })));
    let budget = c.config().gpu_budget_ms;
    for _ in 0..100_000 {
        let cost = if c.detail().get() > 0.5 {
            budget * 1.2
        } else {
            budget * 0.5
        };
        c.observe(gpu(cost));
    }
    assert_eq!(c.state().raise_after, 64);
    // The workload gets lighter: every level fits.
    feed(&mut c, 400_000, gpu(budget * 0.3));
    assert_eq!(c.detail(), Detail::FULL);
    assert!(c.state().raise_after < 64);
}

#[test]
fn gaps_reset_the_window_and_warmup() {
    let mut c = Controller::new(ControllerConfig::default());
    feed(&mut c, 60 + 29, pacing(40.0));
    c.observe(pacing(1_000.0));
    feed(&mut c, 60 + 29, pacing(40.0));
    assert_eq!(c.detail(), Detail::FULL);
    c.observe(pacing(40.0));
    assert!(c.detail() < Detail::FULL);
}

#[test]
fn rejects_nonsense_samples() {
    let mut c = Controller::new(config(json!({ "warmup": 0, "window": 1 })));
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        assert_eq!(c.observe(pacing(bad)), None);
    }
    assert_eq!(c.state().windows, 0);
    // A bad GPU value falls back to pacing.
    c.observe(Sample {
        interval_ms: 16.0,
        gpu_ms: Some(f64::NAN),
    });
    assert_eq!(c.state().source, Some(Source::Pacing));
}

#[test]
fn source_switch_starts_a_new_window() {
    let mut c = Controller::new(config(json!({ "warmup": 0 })));
    feed(&mut c, 29, pacing(40.0));
    feed(&mut c, 29, gpu(40.0));
    assert_eq!(c.state().windows, 0);
    c.observe(gpu(40.0));
    assert_eq!(c.state().source, Some(Source::Gpu));
    assert!(c.detail() < Detail::FULL);
}

#[test]
fn hold_pins_and_release_resumes() {
    let mut c = Controller::new(config(json!({ "start": 0.6 })));
    c.hold(Detail::FULL);
    feed(&mut c, 5_000, pacing(80.0));
    assert_eq!(c.detail(), Detail::FULL);
    assert!(c.state().held);
    c.release();
    assert_eq!(c.detail().get(), 0.6);
    assert_eq!(c.state().changes, 0);
}

// Controller: floors.

#[test]
fn never_goes_below_the_floor() {
    for floor in [0.0, 0.25, 0.3, 0.7, 1.0] {
        let mut c = Controller::new(config(json!({ "floor": floor })));
        for _ in 0..20_000 {
            c.observe(pacing(100.0));
            assert!(c.detail().get() >= floor, "floor {floor}");
        }
        assert_eq!(c.detail().get(), floor);
        assert_eq!(c.state().pressure, Pressure::AtFloor);
    }
}

#[test]
fn at_floor_only_when_already_there() {
    let mut c = Controller::new(config(json!({ "floor": 0.8, "warmup": 0 })));
    feed(&mut c, 30, pacing(19.0));
    assert_eq!(c.state().pressure, Pressure::Lowering);
    assert_eq!(c.detail().get(), 0.9);
    feed(&mut c, 30, pacing(19.0));
    assert_eq!(c.detail().get(), 0.8);
    assert_eq!(c.state().pressure, Pressure::Lowering);
    feed(&mut c, 30, pacing(19.0));
    assert_eq!(c.state().pressure, Pressure::AtFloor);
    feed(&mut c, 30, pacing(16.0));
    assert_eq!(c.state().pressure, Pressure::Holding);
}

#[test]
fn ceiling_caps_raises() {
    let mut c = Controller::new(config(json!({ "start": 0.2, "ceiling": 0.6 })));
    feed(&mut c, 100_000, gpu(1.0));
    assert_eq!(c.detail().get(), 0.6);
}

#[test]
fn config_is_sanitized() {
    let c = Controller::new(config(json!({
        "floor": 0.9, "ceiling": 0.2, "start": 0.1, "window": 0, "budget_ms": -3.0
    })));
    let cfg = c.config();
    assert_eq!(cfg.ceiling, 0.9);
    assert_eq!(cfg.start, 0.9);
    assert_eq!(cfg.window, 1);
    assert_eq!(cfg.budget_ms, Budget::FULL_MS);
}

#[test]
fn sustained_heavy_plant_holds_floor_without_dipping() {
    let mut c = Controller::new(config(json!({ "floor": 0.2 })));
    for _ in 0..50_000 {
        let s = plant(&c, 30.0, 10.0, false);
        c.observe(s);
        assert!(c.detail().get() >= 0.2);
    }
    assert_eq!(c.detail().get(), 0.2);
}

// Knobs, curves, and preferences.

#[test]
fn curves() {
    let d = |v| Detail::new(v).unwrap();
    assert_eq!(Curve::Linear.at(d(0.3)), 0.3);
    assert_eq!(Curve::EaseIn.at(d(0.5)), 0.25);
    assert_eq!(Curve::EaseOut.at(d(0.5)), 0.75);
    assert_eq!(Curve::Steps { count: 4 }.at(d(0.49)), 0.25);
    assert_eq!(Curve::Steps { count: 4 }.at(d(0.5)), 0.5);
    assert_eq!(Curve::Steps { count: 4 }.at(d(1.0)), 1.0);
    for curve in [
        Curve::Linear,
        Curve::EaseIn,
        Curve::EaseOut,
        Curve::Steps { count: 3 },
    ] {
        assert_eq!(curve.at(Detail::FLOOR), 0.0);
        assert_eq!(curve.at(Detail::FULL), 1.0);
        let mut prev = 0.0;
        for i in 0..=100 {
            let v = curve.at(d(f64::from(i) / 100.0));
            assert!(v >= prev, "{curve:?} is monotonic");
            prev = v;
        }
    }
}

#[test]
fn detail_is_clamped_and_finite() {
    assert_eq!(Detail::new(2.0), Some(Detail::FULL));
    assert_eq!(Detail::new(-1.0), Some(Detail::FLOOR));
    assert_eq!(Detail::new(f64::NAN), None);
}

#[test]
fn knob_values_respect_floors_at_every_detail() {
    let set = web_knobs();
    for i in 0..=1000 {
        let detail = Detail::new(f64::from(i) / 1000.0).unwrap();
        for prefs in [
            UserPrefs::default(),
            UserPrefs {
                reduced_motion: true,
                reduced_transparency: true,
                ..UserPrefs::default()
            },
        ] {
            for (decl, value) in set.decls().iter().zip(set.values(detail, &prefs)) {
                assert!(value.value >= decl.floor && value.value <= decl.full);
            }
        }
    }
    let full = set.values(Detail::FULL, &UserPrefs::default());
    assert!(
        full.iter().all(|v| v.value == 1.0),
        "detail 1 is today's renderer"
    );
}

#[test]
fn preferences_are_limits_not_inputs() {
    let set = web_knobs();
    let prefs = UserPrefs {
        reduced_motion: true,
        ..UserPrefs::default()
    };
    let at = |d: f64, knob: Knob| {
        set.values(Detail::new(d).unwrap(), &prefs)
            .into_iter()
            .find(|v| v.knob == knob)
            .unwrap()
    };
    for d in [0.0, 0.5, 1.0] {
        assert_eq!(at(d, Knob::Motion).value, 0.0);
        assert!(at(d, Knob::Motion).limited);
        assert_eq!(at(d, Knob::UiEffects).value, 0.0);
    }
    // Other knobs still follow Detail.
    assert_eq!(at(1.0, Knob::BackdropBlur).value, 1.0);
    assert_eq!(at(1.0, Knob::RenderScale).value, 1.0);
    let prefs = UserPrefs {
        reduced_transparency: true,
        ..UserPrefs::default()
    };
    let blur = set.values(Detail::FULL, &prefs);
    assert_eq!(
        blur.iter()
            .find(|v| v.knob == Knob::BackdropBlur)
            .unwrap()
            .value,
        0.0
    );
    // The controller has no preference input at all: same samples, same Detail.
}

#[test]
fn knob_set_validation() {
    let decl = |knob, floor, full, curve| KnobDecl {
        knob,
        curve,
        floor,
        full,
    };
    let lin = Curve::Linear;
    assert_eq!(
        KnobSet::new(vec![
            decl(Knob::Aa, 0.0, 1.0, lin),
            decl(Knob::Aa, 0.0, 1.0, lin)
        ]),
        Err(KnobError::Duplicate(Knob::Aa))
    );
    assert_eq!(
        KnobSet::new(vec![decl(Knob::Aa, 1.0, 0.5, lin)]),
        Err(KnobError::FloorAboveFull(Knob::Aa))
    );
    assert_eq!(
        KnobSet::new(vec![decl(Knob::Aa, f64::NAN, 1.0, lin)]),
        Err(KnobError::NotFinite(Knob::Aa))
    );
    assert_eq!(
        KnobSet::new(vec![decl(Knob::Aa, 0.0, 1.0, Curve::Steps { count: 0 })]),
        Err(KnobError::NoSteps(Knob::Aa))
    );
    assert_eq!(
        KnobSet::new(vec![decl(Knob::Motion, 0.2, 1.0, lin)]),
        Err(KnobError::PreferenceFloor(Knob::Motion))
    );
    let bad = r#"[{"knob":"motion","curve":{"kind":"linear"},"floor":0.5,"full":1}]"#;
    assert!(serde_json::from_str::<KnobSet>(bad).is_err());
}

#[test]
fn css_variables_cover_interface_knobs() {
    let set = web_knobs();
    let detail = Detail::new(0.5).unwrap();
    let vars = css_variables(detail, &set.values(detail, &UserPrefs::default()));
    let names: Vec<_> = vars.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "--detail",
            "--detail-blur",
            "--detail-shadow",
            "--detail-motion",
            "--detail-effects"
        ]
    );
    assert!(vars.iter().all(|(_, v)| v == "0.500"));
}

// Profile and budget.

#[test]
fn web_profile_fixture_parses_and_round_trips() {
    let profile: DeviceProfile = serde_json::from_str(WEB_PROFILE).unwrap();
    assert_eq!(profile.schema, PROFILE_SCHEMA);
    assert_eq!(profile.gpu.classify(), GpuClass::Baseline);
    assert!(profile.gpu.timestamps());
    let again: DeviceProfile =
        serde_json::from_value(serde_json::to_value(&profile).unwrap()).unwrap();
    assert_eq!(again, profile);
    // An unknown field means the TypeScript probe and this crate drifted.
    let mut value: Value = serde_json::from_str(WEB_PROFILE).unwrap();
    value["display"]["user_agent"] = json!("x");
    assert!(serde_json::from_value::<DeviceProfile>(value).is_err());
}

#[test]
fn gpu_classes() {
    let mut gpu = GpuInfo::default();
    assert_eq!(gpu.classify(), GpuClass::None);
    gpu.api = GpuApi::Webgpu;
    assert_eq!(gpu.classify(), GpuClass::Baseline);
    gpu.limits.insert("maxTextureDimension2D".into(), 16_384);
    gpu.limits.insert("maxBufferSize".into(), 1 << 32);
    assert_eq!(gpu.classify(), GpuClass::Extended);
    gpu.fallback = true;
    assert_eq!(gpu.classify(), GpuClass::Software);
}

#[test]
fn report_serializes() {
    let profile: DeviceProfile = serde_json::from_str(WEB_PROFILE).unwrap();
    let budget = Budget::for_profile(&profile);
    let controller = Controller::new(ControllerConfig::for_budget(&budget));
    let knobs = web_knobs().values(controller.detail(), &profile.prefs);
    let report = DetailReport::new(profile, budget, controller.state(), knobs);
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["schema"], REPORT_SCHEMA);
    assert_eq!(value["controller"]["detail"], 1.0);
    assert_eq!(value["knobs"][0]["knob"], "render_scale");
    let back: DetailReport = serde_json::from_value(value).unwrap();
    assert_eq!(back, report);
}

// Shared vectors: the TypeScript mirror runs the same file.

fn budget_profile(case: &Value) -> DeviceProfile {
    let mut p = DeviceProfile::default();
    p.display.refresh_hz = case["refresh_hz"].as_f64();
    p.power.battery_level = case["battery_level"].as_f64();
    p.power.charging = case["charging"].as_bool();
    p.memory.device_memory_gb = case["device_memory_gb"].as_f64();
    p.network.save_data = case["save_data"].as_bool();
    p.network.effective_type = case["effective_type"].as_str().map(str::to_owned);
    p
}

fn run_controller(case: &Value) -> Value {
    let mut c = Controller::new(config(case["config"].clone()));
    let mut frame = 0u64;
    let mut trace = Vec::new();
    let mut after = Vec::new();
    for segment in case["segments"].as_array().unwrap() {
        if let Some(d) = segment["hold"].as_f64() {
            c.hold(Detail::new(d).unwrap());
        } else if segment["release"].as_bool() == Some(true) {
            c.release();
        } else {
            let frames = segment["frames"].as_u64().unwrap();
            for _ in 0..frames {
                let sample = match segment.get("plant") {
                    Some(p) => plant(
                        &c,
                        p["base_ms"].as_f64().unwrap(),
                        p["per_detail_ms"].as_f64().unwrap(),
                        p["gpu"].as_bool().unwrap(),
                    ),
                    None => Sample {
                        interval_ms: segment["interval_ms"].as_f64().unwrap(),
                        gpu_ms: segment["gpu_ms"].as_f64(),
                    },
                };
                if let Some(d) = c.observe(sample) {
                    trace.push(json!([frame, d.get()]));
                }
                frame += 1;
            }
        }
        let s = c.state();
        after.push(json!({
            "detail": s.detail.get(),
            "changes": s.changes,
            "windows": s.windows,
            "raise_after": s.raise_after,
            "pressure": s.pressure,
            "held": s.held,
        }));
    }
    json!({ "after": after, "trace": trace })
}

fn expected(vectors: &Value) -> Value {
    let set = web_knobs();
    let knobs: Vec<Value> = vectors["knobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let prefs = UserPrefs {
                reduced_motion: case["prefs"]["reduced_motion"].as_bool().unwrap_or(false),
                reduced_transparency: case["prefs"]["reduced_transparency"]
                    .as_bool()
                    .unwrap_or(false),
                ..UserPrefs::default()
            };
            let detail = Detail::new(case["detail"].as_f64().unwrap()).unwrap();
            let values = set.values(detail, &prefs);
            json!({ "values": values, "css": css_variables(detail, &values) })
        })
        .collect();
    let budgets: Vec<Value> = vectors["budgets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| serde_json::to_value(Budget::for_profile(&budget_profile(case))).unwrap())
        .collect();
    let controllers: Vec<Value> = vectors["controllers"]
        .as_array()
        .unwrap()
        .iter()
        .map(run_controller)
        .collect();
    json!({ "knobs": knobs, "budgets": budgets, "controllers": controllers })
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| close(a, b))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| close(v, w)))
        }
        _ => a == b,
    }
}

/// `AETHYR_PROFILE_WRITE=1 cargo test -p aethyr-profile` refreshes the
/// expected half of the vectors after a deliberate behavior change.
#[test]
fn shared_vectors() {
    let text = std::fs::read_to_string(VECTORS).unwrap();
    let mut vectors: Value = serde_json::from_str(&text).unwrap();
    let got = expected(&vectors);
    if std::env::var_os("AETHYR_PROFILE_WRITE").is_some() {
        vectors["expected"] = got;
        let out = serde_json::to_string_pretty(&vectors).unwrap() + "\n";
        std::fs::write(VECTORS, out).unwrap();
        return;
    }
    assert!(
        close(&vectors["expected"], &got),
        "fixtures/vectors.json is stale; rerun with AETHYR_PROFILE_WRITE=1 and review the diff"
    );
}
