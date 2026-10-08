use glam::Vec3;
use verse_engine::quality::Tier;

use super::def::{Blend, ColorCurve, Curve, Effect, Light};
use super::sheet::{self, SHEETS};
use super::*;

fn library() -> std::sync::Arc<Library> {
    std::sync::Arc::new(Library::builtin())
}

fn system(seed: u32) -> Particles {
    Particles::with_library(library(), u64::MAX, seed)
}

fn flat(_: f32, _: f32) -> f32 {
    0.0
}

#[test]
fn every_effect_parses_and_names_real_sheets_and_frames() {
    let library = Library::builtin();
    assert_eq!(library.effects.len(), library::SOURCES.len());
    for effect in &library.effects {
        for e in &effect.emitters {
            let s = sheet::find(&e.sheet).expect("sheet exists");
            let (a, b) = e.frame_range();
            assert!(a <= b && b < s.frames, "{}: {}", effect.name, e.name);
        }
    }
}

#[test]
fn every_sheet_decodes_at_its_size() {
    for s in &SHEETS {
        let pixels = s.decode().unwrap();
        assert_eq!(pixels.len(), (sheet::SIZE * sheet::SIZE * 4) as usize);
        assert_eq!(s.columns * s.rows >= s.frames, true, "{}", s.name);
        // Something is drawn, and the corners stay clear so neighboring
        // cells never bleed.
        assert!(pixels.chunks(4).any(|p| p[3] > 128), "{}", s.name);
        assert!(pixels[3] < 8, "{}", s.name);
    }
    let levels = sheet::mip_layers(0).unwrap();
    assert_eq!(levels.len(), 10);
    assert_eq!(levels[0].1.len(), SHEETS.len());
    assert_eq!(levels.last().unwrap().0, 1);
    // The low tier skips the largest level.
    assert_eq!(sheet::mip_layers(1).unwrap()[0].0, sheet::SIZE / 2);
}

#[test]
fn bad_effects_are_refused_with_a_reason() {
    let base = r#"
description = "x"
[[emitter]]
name = "a"
sheet = "sparks"
burst = 1
life = [1.0, 1.0]
size = [1.0, 1.0]
"#;
    assert!(Effect::parse("ok", base).is_ok());
    let bad = [
        (base.replace("sparks", "nothing"), "no sheet"),
        (
            base.replace("burst = 1", "frames = [2, 9]\nburst = 1"),
            "frames",
        ),
        (base.replace("burst = 1", "burst = 0"), "burst or a rate"),
        (
            base.replace("life = [1.0, 1.0]", "life = [2.0, 1.0]"),
            "life",
        ),
        (
            base.replace("burst = 1", "burst = 1\nspread = 270.0"),
            "spread",
        ),
        (
            base.replace("burst = 1", "burst = 1\nwobble = 1.0"),
            "unknown field",
        ),
        (
            base.replace("burst = 1", "burst = 1\nalpha = [[0.5, 1.0], [0.2, 0.0]]"),
            "rise",
        ),
    ];
    for (source, why) in bad {
        let error = Effect::parse("bad", &source).unwrap_err();
        assert!(error.contains(why), "{error} should say {why}");
    }
}

#[test]
fn scene_lighting_defaults_preserve_emitted_and_display_color_units() {
    let source = r#"
description = "A test particle."
[[emitter]]
name = "a"
sheet = "smoke"
burst = 1
life = [1.0, 1.0]
size = [1.0, 1.0]
color = [0.2, 0.4, 0.6]
luminance = 50.0
"#;
    let effect = Effect::parse("units", source).unwrap();
    let mut emitter = effect.emitters[0].clone();
    assert!(!emitter.lit);
    assert_eq!(emitter.density, 1.0);
    for (actual, expected) in emitter.color_at(0.0).into_iter().zip([10.0, 20.0, 30.0]) {
        assert!((actual - expected).abs() < 1e-4);
    }
    emitter.light = Light::Lit;
    assert_eq!(emitter.color_at(0.0), [0.2, 0.4, 0.6]);
    emitter.light = Light::Emit;
    emitter.lit = true;
    assert_eq!(emitter.color_at(0.0), [0.2, 0.4, 0.6]);
}

#[test]
fn particle_density_accepts_its_bounds_and_refuses_invalid_values() {
    let source = r#"
description = "A test particle."
[[emitter]]
name = "a"
sheet = "smoke"
burst = 1
life = [1.0, 1.0]
size = [1.0, 1.0]
"#;
    for density in ["0.0", "8.0"] {
        assert!(Effect::parse("density", &format!("{source}density = {density}\n")).is_ok());
    }
    for density in ["-0.1", "8.1", "nan", "inf", "-inf"] {
        let error =
            Effect::parse("density", &format!("{source}density = {density}\n")).unwrap_err();
        assert!(error.contains("density"), "{error}");
    }
}

#[test]
fn emitted_particles_carry_scene_lighting_density_and_albedo() {
    let source = r#"
description = "A test smoke puff."
[[emitter]]
name = "smoke"
sheet = "smoke"
blend = "alpha"
lit = true
density = 2.25
luminance = 50.0
burst = 1
life = [1.0, 1.0]
size = [1.0, 1.0]
color = [0.2, 0.4, 0.6]
"#;
    let library = std::sync::Arc::new(Library::parse([("smoke", source)]).unwrap());
    let mut fx = Particles::with_library(library, u64::MAX, 3);
    fx.start("smoke", Spawn::at(Vec3::new(1.0, 2.0, 3.0)))
        .unwrap();
    fx.tick(0.01, flat);
    let mut out = Vec::new();
    fx.draw(&mut out);
    assert_eq!(out.len(), 1);
    let sprite = out[0];
    assert!(sprite.scene_lit);
    assert!(
        !sprite.lit,
        "legacy units remain independent of scene lighting"
    );
    assert_eq!(sprite.density, 2.25);
    assert_eq!(sprite.color, [0.2, 0.4, 0.6]);
    assert_eq!(sprite.additive, 0.0);
}

#[test]
fn showcase_smoke_and_dust_take_scene_light_while_fire_stays_emissive() {
    let library = Library::builtin();
    let mut lit = 0;
    let mut emissive = 0;
    for name in [
        "meteor_explosion",
        "meteor_smolder",
        "meteor_arc_trail",
        "meteor_trail",
        "debris_dust",
        "tower_crash",
        "scorch_embers",
    ] {
        for emitter in &library.get(name).unwrap().emitters {
            if emitter.blend == Blend::Alpha {
                assert!(emitter.lit, "{name}: {} takes scene light", emitter.name);
                assert!(emitter.density > 0.0);
                lit += 1;
            } else {
                assert!(!emitter.lit, "{name}: {} stays emissive", emitter.name);
                assert_eq!(emitter.light, Light::Emit);
                assert_eq!(
                    emitter.color_at(0.5),
                    emitter
                        .color
                        .at(0.5)
                        .map(|channel| channel * emitter.luminance)
                );
                emissive += 1;
            }
        }
    }
    assert_eq!(lit, 10);
    assert!(emissive > 0);
}

#[test]
fn curves_key_start_middle_end_and_explicit_times() {
    let three = Curve::Three([0.0, 1.0, 0.5]);
    assert_eq!(three.at(0.0), 0.0);
    assert_eq!(three.at(0.25), 0.5);
    assert_eq!(three.at(0.5), 1.0);
    assert_eq!(three.at(0.75), 0.75);
    assert_eq!(three.at(1.0), 0.5);
    assert_eq!(three.at(2.0), 0.5);
    let keys = Curve::Keys(vec![[0.0, 0.0], [0.1, 1.0], [1.0, 0.0]]);
    assert!((keys.at(0.05) - 0.5).abs() < 1e-6);
    assert!((keys.at(0.55) - 0.5).abs() < 1e-6);
    assert_eq!(Curve::Constant(3.0).at(0.7), 3.0);
    let color = ColorCurve::Keys(vec![(0.0, [1.0, 0.0, 0.0]), (1.0, [0.0, 0.0, 1.0])]);
    assert_eq!(color.at(0.5), [0.5, 0.0, 0.5]);
}

#[test]
fn flipbook_frames_play_over_life_and_blend_between_neighbors() {
    let s = sheet::find("fireball").unwrap();
    assert_eq!(s.rect(0)[0], 0.5 / 512.0);
    assert_eq!(s.rect(5), {
        let i = 0.5 / 512.0;
        [0.25 + i, 0.25 + i, 0.5 - i, 0.5 - i]
    });
    // A one-particle effect over the whole sheet, life 1 s.
    let effect = Effect::parse(
        "book",
        r#"
description = "x"
[[emitter]]
name = "a"
sheet = "fireball"
burst = 1
life = [1.0, 1.0]
size = [1.0, 1.0]
"#,
    )
    .unwrap();
    let library = std::sync::Arc::new(Library {
        effects: vec![effect],
    });
    let mut fx = Particles::with_library(library, u64::MAX, 3);
    fx.start("book", Spawn::at(Vec3::ZERO)).unwrap();
    let mut frames = Vec::new();
    for _ in 0..10 {
        fx.tick(0.1, flat);
        let mut out = Vec::new();
        fx.draw(&mut out);
        if let Some(sprite) = out.first() {
            frames.push((sprite.rect_a, sprite.rect_b, sprite.mix));
        }
    }
    // Age 0.5 of 1: frame 7.5 of 0..15, halfway from 7 to 8.
    let (a, b, mix) = frames[4];
    assert_eq!(a, s.rect(7));
    assert_eq!(b, s.rect(8));
    assert!((mix - 0.5).abs() < 1e-3);
    // Frames only move forward.
    let index = |r: [f32; 4]| (r[1] * 4.0) as u32 * 4 + (r[0] * 4.0) as u32;
    assert!(frames.windows(2).all(|w| index(w[1].0) >= index(w[0].0)));
}

#[test]
fn the_same_seed_and_steps_give_the_same_particles() {
    let run = |seed| {
        let mut fx = system(seed);
        fx.start("meteor_explosion", Spawn::at(Vec3::new(1.0, 2.0, 3.0)));
        let trail = fx
            .start("meteor_trail", Spawn::at(Vec3::new(0.0, 20.0, 0.0)))
            .unwrap();
        for i in 0..90 {
            let at = Vec3::new(0.0, 20.0 - i as f32 * 0.3, 0.0);
            fx.place(trail, at, Vec3::new(0.0, -18.0, 0.0));
            fx.tick(1.0 / 60.0, flat);
        }
        let mut out = Vec::new();
        fx.draw(&mut out);
        out
    };
    let (a, b, c) = (run(7), run(7), run(8));
    assert!(!a.is_empty());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn burst_effects_end_by_themselves_and_trails_end_when_stopped() {
    let mut fx = system(1);
    let blast = fx.start("meteor_explosion", Spawn::at(Vec3::ZERO)).unwrap();
    let trail = fx.start("meteor_trail", Spawn::at(Vec3::Y * 10.0)).unwrap();
    for _ in 0..120 {
        fx.tick(1.0 / 30.0, flat);
    }
    assert!(fx.alive(trail));
    fx.stop(trail);
    for _ in 0..600 {
        fx.tick(1.0 / 30.0, flat);
    }
    assert!(!fx.alive(blast) && !fx.alive(trail));
    assert!(fx.is_empty());
    assert_eq!(fx.effects(), 0);
}

#[test]
fn particles_and_effects_stay_within_their_caps() {
    let mut fx = system(5);
    for i in 0..400 {
        fx.start("meteor_explosion", Spawn::at(Vec3::X * i as f32));
    }
    assert_eq!(fx.effects(), system::MAX_EFFECTS);
    fx.tick(0.05, flat);
    assert!(fx.len() <= system::MAX_PARTICLES);
    assert!(fx.start("no such effect", Spawn::at(Vec3::ZERO)).is_none());
}

#[test]
fn a_frame_keeps_within_the_tier_budget_by_priority_then_size() {
    let sprite = |at: Vec3, half: f32, priority: u8| Sprite {
        at,
        half,
        angle: 0.0,
        tail: Vec3::ZERO,
        facing: Facing::Camera,
        color: [1.0; 3],
        alpha: 1.0,
        additive: 1.0,
        lit: false,
        scene_lit: false,
        density: 1.0,
        layer: 0,
        rect_a: [0.0, 0.0, 1.0, 1.0],
        rect_b: [0.0, 0.0, 1.0, 1.0],
        mix: 0.0,
        priority,
    };
    let eye = Vec3::new(0.0, 0.0, 50.0);
    let mut sprites: Vec<Sprite> = (0..500)
        .map(|i| sprite(Vec3::new(i as f32 * 0.01, 0.0, 0.0), 0.1, 0))
        .collect();
    sprites.push(sprite(Vec3::new(0.0, 0.0, -30.0), 0.1, 9));
    sprites.push(sprite(Vec3::ZERO, 5.0, 0));
    let mut out = Vec::new();
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        vertices(&sprites, eye, budget(tier), &mut out);
        assert_eq!(out.len(), budget(tier).min(sprites.len()) * 6);
    }
    vertices(&sprites, eye, 2, &mut out);
    // The priority sprite and the big one, the far one first.
    assert_eq!(out.len(), 12);
    assert!(out[0].pos[2] < -29.0);
    assert!(out[6..].iter().any(|v| v.pos[0].abs() > 4.0));
    assert!(budget(Tier::Low) < budget(Tier::Medium));
    assert!(budget(Tier::Medium) < budget(Tier::High));
}

#[test]
fn a_tail_lays_the_quad_along_the_flight() {
    let mut s = Sprite {
        at: Vec3::ZERO,
        half: 0.1,
        angle: 0.0,
        tail: Vec3::new(2.0, 0.0, 0.0),
        facing: Facing::Camera,
        color: [1.0; 3],
        alpha: 1.0,
        additive: 1.0,
        lit: false,
        scene_lit: false,
        density: 1.0,
        layer: 2,
        rect_a: [0.0, 0.0, 1.0, 1.0],
        rect_b: [0.0, 0.0, 1.0, 1.0],
        mix: 0.0,
        priority: 0,
    };
    let eye = Vec3::new(0.0, 0.0, 20.0);
    let mut out = Vec::new();
    vertices(&[s], eye, 10, &mut out);
    let xs: Vec<f32> = out.iter().map(|v| v.pos[0]).collect();
    let ys: Vec<f32> = out.iter().map(|v| v.pos[1]).collect();
    let span = |v: &[f32]| {
        v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!((span(&xs) - 2.2).abs() < 1e-4);
    assert!((span(&ys) - 0.2).abs() < 1e-4);
    assert_eq!(out[0].params[1], 2.0);
    // On the ground, the quad lies flat.
    s.tail = Vec3::ZERO;
    s.facing = Facing::Ground;
    vertices(&[s], eye, 10, &mut out);
    assert!(out.iter().all(|v| v.pos[1] == 0.0));
}

#[test]
fn the_compiled_effects_fit_the_medium_budget_for_a_full_swarm() {
    // Six meteors' heads and trails at their peak, and six explosions,
    // stay within the high tier; the medium tier keeps the priorities.
    let library = Library::builtin();
    let peak = |name: &str| library.get(name).unwrap().peak();
    let swarm = 6.0 * (peak("meteor_head") + peak("meteor_trail") + peak("meteor_explosion"));
    assert!(swarm < (system::MAX_PARTICLES as f32), "{swarm}");
    assert!(swarm < 3.0 * budget(Tier::High) as f32, "{swarm}");
    // A single explosion fits the low tier's budget.
    assert!(peak("meteor_explosion") <= budget(Tier::Low) as f32);
}

#[test]
fn a_style_draws_external_particles_with_an_effects_look() {
    let style = Style::named("debris_dust").unwrap();
    let young = style
        .sprite(0, Vec3::ZERO, Vec3::ZERO, 0.0, 0.5, [0.4, 0.3, 0.2], 9)
        .unwrap();
    let old = style
        .sprite(0, Vec3::ZERO, Vec3::ZERO, 0.5, 0.5, [0.4, 0.3, 0.2], 9)
        .unwrap();
    assert!(old.half > young.half);
    assert!(old.alpha > young.alpha);
    assert!(young.lit && young.additive == 0.0);
    assert!(young.scene_lit && old.scene_lit);
    assert_eq!(young.density, 1.0);
    assert_eq!(old.density, young.density);
    assert!(
        style
            .sprite(9, Vec3::ZERO, Vec3::ZERO, 0.5, 0.5, [1.0; 3], 0)
            .is_none()
    );
    assert!(Style::named("no such effect").is_none());
}

#[test]
fn swirl_turns_particles_about_the_axis_and_pull_draws_them_in() {
    let run = |pull: f32| {
        let text = format!(
            "description = \"A ring that turns.\"\n[[emitter]]\nname = \"a\"\nsheet = \"sparks\"\nburst = 8\nlife = [5.0, 5.0]\nsize = [0.1, 0.1]\nshape = \"ring\"\nradius = 2.0\nswirl = 3.14159265\npull = {pull}\n"
        );
        let library = Library::parse([("ring", text.as_str())]).unwrap();
        let mut fx = Particles::with_library(std::sync::Arc::new(library), u64::MAX, 3);
        let center = Vec3::new(1.0, 2.0, 3.0);
        fx.start("ring", Spawn::at(center)).unwrap();
        fx.tick(1e-4, flat);
        let mut before = Vec::new();
        fx.draw(&mut before);
        for _ in 0..50 {
            fx.tick(0.01, flat);
        }
        let mut after = Vec::new();
        fx.draw(&mut after);
        (center, before, after)
    };
    // Half a second at half a turn a second: each particle a quarter turn
    // on, at the same radius and height.
    let (center, before, after) = run(0.0);
    assert_eq!(before.len(), 8);
    for (a, b) in before.iter().zip(&after) {
        let (a, b) = (a.at - center, b.at - center);
        assert!((b.length() - 2.0).abs() < 0.02, "{b}");
        assert!(b.y.abs() < 1e-4);
        assert!(a.dot(b).abs() < 0.05, "{a} {b}");
    }
    // Pulled in at half its distance a second, it spirals to about 1.5 m.
    let (center, _, after) = run(0.5);
    for b in &after {
        let r = (b.at - center).length();
        assert!((1.45..1.6).contains(&r), "{r}");
    }
}
