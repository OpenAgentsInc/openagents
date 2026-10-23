use aethyrforge_host::*;
#[test]
fn courtyard_is_seeded_bounded_and_preserves_exact_source_text() {
    let recipe = PlanetRecipe::new(42, 1000);
    let template = WorldTemplate {
        world: "personal-planet".into(),
        engine_release: "fixture".into(),
        asset_set_sha256: "a".repeat(64),
        scene_sha256: "b".repeat(64),
    };
    let d = recipe.document(template.clone()).unwrap();
    assert_eq!(d, recipe.document(template.clone()).unwrap());
    assert_eq!(d.entities.len(), 24);
    assert!(
        d.entities
            .values()
            .all(|e| e.position[0].abs() <= 28. && e.position[2].abs() <= 28.)
    );
    assert_ne!(
        d.entities,
        PlanetRecipe::new(43, 1000)
            .document(template)
            .unwrap()
            .entities
    );
    let mut source = recipe.source().unwrap();
    source.text = serde_json::to_string_pretty(&recipe).unwrap();
    source.sha256 = project_hash(source.text.as_bytes());
    assert_eq!(PlanetRecipe::from_source(&source).unwrap(), recipe);
    let kept = source.clone();
    source.text.push(' ');
    assert!(PlanetRecipe::from_source(&source).is_err());
    assert_ne!(kept.sha256, project_hash(source.text.as_bytes()));
    let mut invalid = d;
    invalid.entities.values_mut().next().unwrap().position[0] = 32.;
    assert!(PlanetRecipe::from_document(&invalid).is_err());
}
#[test]
fn day_cycle_advances_wraps_and_pauses_with_distinct_supported_styles() {
    let mut recipe = PlanetRecipe::new(1, 1000);
    recipe.day_minutes = 2;
    assert_eq!(recipe.hour_at(1000), 8.);
    assert!((recipe.hour_at(31_000) - 14.).abs() < 0.00001);
    assert_eq!(recipe.hour_at(121_000), 8.);
    assert_ne!(recipe.lighting_at(1000), recipe.lighting_at(31_000));
    recipe.initial_hour = 22.;
    let dark = recipe.lighting_at(1000);
    assert!(dark["ambientIntensity"] >= 0.65);
    assert!(dark["moonIntensity"] > 0.);
    recipe.cycle_enabled = false;
    assert_eq!(recipe.hour_at(1_000_000), 22.);
    assert_ne!(
        PlanetStyle::SoftPastels.effect(),
        PlanetStyle::BoldFantasy.effect()
    );
    recipe.initial_hour = f64::NAN;
    assert!(recipe.validate().is_err());
}

#[test]
fn spoken_pause_resume_and_style_preserve_elapsed_clock_and_layout_seed() {
    let mut accepted = PlanetRecipe::new(42, 1000);
    accepted.day_minutes = 2;
    let handle = |r: &PlanetRecipe, text: &str, now| {
        r.changed(PlanetChange::from_request(text).unwrap().unwrap(), now)
            .unwrap()
    };
    assert_eq!(accepted.hour_at(31_000), 14.);
    let styled = handle(&accepted, "Use bold fantasy", 31_000);
    assert_eq!(styled.hour_at(31_000), 14.);
    assert_eq!(styled.clock_anchor_unix_ms, 1000);
    let paused = handle(&styled, "Pause day cycle", 31_000);
    assert_eq!(paused.initial_hour, 14.);
    assert_eq!(paused.hour_at(61_000), 14.);
    let resumed = handle(&paused, "Start day cycle", 61_000);
    assert_eq!(resumed.hour_at(91_000), 20.);
    let controls = accepted
        .changed(
            PlanetChange::Settings {
                style: PlanetStyle::BoldFantasy,
                hour: 8.,
                cycle: false,
                day_minutes: 2,
            },
            31_000,
        )
        .unwrap();
    assert_eq!(controls.initial_hour, 14.);
    assert_eq!(controls.seed, 42);
    let time = handle(&resumed, "Set time to 3", 91_000);
    assert_eq!(time.hour_at(91_000), 3.);
    assert_eq!(time.seed, 42);
    assert!(accepted.changed(PlanetChange::Hour(24.), 31_000).is_err());
}

#[test]
fn generated_recipe_action_digest_survives_json_hops() {
    let document = PlanetRecipe::new(42, 1000)
        .document(WorldTemplate {
            world: "personal-planet".into(),
            engine_release: "fixture".into(),
            asset_set_sha256: "a".repeat(64),
            scene_sha256: "b".repeat(64),
        })
        .unwrap();
    let mut action = Action::RestoreProject {
        document,
        checkpoint: None,
    };
    let expected = action_digest(&action).unwrap();
    for _ in 0..4 {
        action = serde_json::from_slice(&serde_json::to_vec(&action).unwrap()).unwrap();
        assert_eq!(action_digest(&action).unwrap(), expected);
    }
}

#[test]
fn review_time_uses_a_normal_clock() {
    let mut r = PlanetRecipe::new(42, 1000);
    for (hour, label) in [
        (8., "08:00"),
        (8.5, "08:30"),
        (14., "14:00"),
        (23.5, "23:30"),
    ] {
        r.initial_hour = hour;
        assert_eq!(r.time_label(), label);
    }
}
