//! Offline pictures and frame times of the Water Lab with the shared
//! renderer.
//!
//! Usage: water_capture OUT_DIR [--only NAME,...] [--frames N] [--size WxH]
//! [--sequence N] [--spells] [--orb]
//!
//! Installs the lab as `verse --water-lab` does after the Everglade pack
//! loads, then renders fixed cinematic views: the cove at golden hour and
//! at noon, the shore's foam, the sun's glitter, floating bodies, a splash
//! frame by frame, the falls, the river, and the view under water. With
//! `--spells` it also casts each water spell and renders it into
//! `OUT_DIR/spells`. With `--orb` it grows, holds, throws, and splashes a
//! Water Orb, engulfs a dummy and a crate, strikes the orb and the sea with
//! the Thunderbolt, and renders each, with a frame sequence, into
//! `OUT_DIR/orb`. `--sequence N` writes N frames of a slow pan across
//! the bay into `OUT_DIR/sequence`. Finally it renders `N` frames (240 by
//! default) from the beach and prints the frame times; `VERSE_QUALITY`
//! (`low`, `medium`, `high`) picks the tier.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::{Mat4, Vec3};
use verse::render::{Offscreen, View};
use verse::runtime::WorldRuntime;
use verse::zones::{self, everglade_pack, water};

struct Args {
    out: PathBuf,
    only: Option<Vec<String>>,
    frames: usize,
    width: u32,
    height: u32,
    sequence: usize,
    spells: bool,
    orb: bool,
}

fn args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let mut a = Args {
        out: PathBuf::from(it.next().ok_or("Expected an output directory")?),
        only: None,
        frames: 240,
        width: 1600,
        height: 900,
        sequence: 0,
        spells: false,
        orb: false,
    };
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--only" => {
                a.only = Some(
                    it.next()
                        .ok_or("--only needs names")?
                        .split(',')
                        .map(str::to_owned)
                        .collect(),
                );
            }
            "--frames" => a.frames = it.next().and_then(|v| v.parse().ok()).ok_or("--frames N")?,
            "--sequence" => {
                a.sequence = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--sequence N")?;
            }
            "--spells" => a.spells = true,
            "--orb" => a.orb = true,
            "--size" => {
                let v = it.next().ok_or("--size WxH")?;
                let (w, h) = v.split_once('x').ok_or("--size WxH")?;
                a.width = w.parse().map_err(|_| "bad width")?;
                a.height = h.parse().map_err(|_| "bad height")?;
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    Ok(a)
}

fn view(eye: Vec3, target: Vec3, fov: f32, aspect: f32) -> View {
    let proj = Mat4::perspective_rh(fov, aspect, 0.1, 2000.0);
    View {
        view_proj: proj * Mat4::look_at_rh(eye, target, Vec3::Y),
        eye,
    }
}

fn main() -> Result<(), String> {
    let a = args()?;
    std::fs::create_dir_all(&a.out).map_err(|e| e.to_string())?;
    let pack = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(everglade_pack::PACK_DIRECTORY)
        .join(format!(
            "{}.{}",
            everglade_pack::PACK_SHA256,
            everglade_pack::PACK_EXTENSION
        ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let mut runtime = WorldRuntime::new();
    let started = Instant::now();
    runtime.install_water_lab(&pack);
    if runtime.zone != zones::ZoneId::WaterLab {
        return Err(format!(
            "The Water Lab did not install: {:?}",
            runtime.zone_snapshot(1.0).error
        ));
    }
    eprintln!(
        "installed in {:.0} ms",
        started.elapsed().as_secs_f64() * 1e3
    );
    let mut atlas = verse::ui::Atlas::new(16.0);
    water::hotbar::add_sprites(&mut atlas)?;
    let mut renderer = Offscreen::new(
        a.width,
        a.height,
        &runtime.world.mesh,
        &atlas,
        zones::atmosphere(zones::ZoneId::WaterLab),
    )?;
    let aspect = a.width as f32 / a.height as f32;
    let ui = verse::ui::UiBatch::default();
    let wanted = |name: &str| a.only.as_ref().is_none_or(|o| o.iter().any(|n| n == name));
    let idle = verse::controller::InputState::default();
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / (1.0 / 60.0)).round() as usize {
            runtime.tick(&idle, 1.0 / 60.0);
        }
    };
    let ground = |x: f32, z: f32| water::ground(x, z);
    let shoot = |runtime: &mut WorldRuntime,
                 renderer: &mut Offscreen,
                 path: &Path,
                 eye: Vec3,
                 target: Vec3,
                 fov: f32|
     -> Result<(), String> {
        let pixels =
            renderer.render(view(eye, target, fov, aspect), &runtime.dynamic_mesh(), &ui)?;
        write_png(path, a.width, a.height, &pixels)?;
        eprintln!("wrote {}", path.display());
        Ok(())
    };
    let place = |runtime: &mut WorldRuntime, x: f32, z: f32, yaw: f32| {
        let _ = runtime.set_spawn(Vec3::new(x, ground(x, z), z), yaw);
    };
    // Let the sea and the floats settle.
    place(&mut runtime, 4.0, 16.0, PI);
    run(&mut runtime, 1.0);

    type Shot = (&'static str, Vec3, Vec3, f32, [f32; 3], bool);
    // Name, eye, target, vertical field of view, the player's x, z, yaw,
    // and noon.
    let shots: Vec<Shot> = vec![
        (
            "cove-golden",
            Vec3::new(30.0, 16.0, 38.0),
            Vec3::new(-6.0, 0.0, -30.0),
            0.9,
            [4.0, 12.0, PI],
            false,
        ),
        (
            "cove-noon",
            Vec3::new(30.0, 16.0, 38.0),
            Vec3::new(-6.0, 0.0, -30.0),
            0.9,
            [4.0, 12.0, PI],
            true,
        ),
        (
            "glint",
            Vec3::new(2.0, 2.2, 10.0),
            Vec3::new(-14.0, 0.0, -60.0),
            0.75,
            [3.0, 7.0, PI],
            false,
        ),
        (
            "shore-foam",
            Vec3::new(6.0, 2.4, 11.5),
            Vec3::new(-2.0, 0.0, -2.0),
            0.85,
            [9.0, 9.0, PI],
            false,
        ),
        (
            "shallows-noon",
            Vec3::new(8.0, 4.5, 8.0),
            Vec3::new(4.0, -0.6, -4.0),
            0.9,
            [10.0, 12.0, PI],
            true,
        ),
        (
            "reef-noon",
            Vec3::new(22.0, 9.0, -12.0),
            Vec3::new(16.0, -1.0, -30.0),
            0.85,
            [4.0, 12.0, PI],
            true,
        ),
        (
            "floats",
            Vec3::new(10.0, 3.2, 2.0),
            Vec3::new(2.0, 0.0, -10.0),
            0.8,
            [4.0, 12.0, PI],
            false,
        ),
        (
            "falls",
            Vec3::new(-30.0, 7.0, 36.0),
            Vec3::new(-46.0, 6.0, 53.0),
            0.95,
            [-38.0, 40.0, 0.6],
            false,
        ),
        (
            "falls-noon",
            Vec3::new(-30.0, 7.0, 36.0),
            Vec3::new(-46.0, 6.0, 53.0),
            0.95,
            [-38.0, 40.0, 0.6],
            true,
        ),
        (
            "river",
            Vec3::new(-28.0, 4.0, 16.0),
            Vec3::new(-38.0, 1.5, 30.0),
            0.9,
            [-30.0, 18.0, 0.0],
            false,
        ),
        (
            "plateau",
            Vec3::new(-50.0, 16.0, 66.0),
            Vec3::new(-42.0, 5.0, 40.0),
            0.9,
            [-50.0, 66.0, 0.0],
            false,
        ),
        (
            "headland",
            Vec3::new(-20.0, 3.0, -2.0),
            Vec3::new(-55.0, 2.0, -20.0),
            0.9,
            [-12.0, 2.0, 0.0],
            false,
        ),
    ];
    for (name, eye, target, fov, [px, pz, yaw], noon) in shots {
        if !wanted(name) {
            continue;
        }
        set_noon(&mut runtime, noon);
        place(&mut runtime, px, pz, yaw);
        run(&mut runtime, 0.3);
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join(format!("{name}.png")),
            eye,
            target,
            fov,
        )?;
    }
    set_noon(&mut runtime, false);

    if wanted("splash") {
        // A crate dropped from a few meters, frame by frame.
        place(&mut runtime, 6.0, 8.0, PI);
        run(&mut runtime, 0.3);
        if let Some(lab) = runtime.water_lab_mut() {
            lab.floats.spawn(
                water::FloatKind::Crate,
                Vec3::new(3.0, 3.6, -3.0),
                0.4,
                Vec3::new(0.0, -2.0, 0.0),
            );
        }
        for k in 0..18 {
            run(&mut runtime, 1.0 / 15.0);
            shoot(
                &mut runtime,
                &mut renderer,
                &a.out.join(format!("splash-{k:02}.png")),
                Vec3::new(8.0, 2.0, 4.0),
                Vec3::new(3.0, 0.6, -3.0),
                0.6,
            )?;
        }
    }

    if wanted("underwater") {
        // With Water Breathing, out past the reef, on the bed.
        runtime.water_press(4, false)?;
        place(&mut runtime, 10.0, -26.0, 0.0);
        run(&mut runtime, 0.5);
        let y = ground(10.0, -26.0);
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join("underwater.png"),
            Vec3::new(8.0, y + 1.6, -22.0),
            Vec3::new(16.0, y + 0.8, -31.0),
            1.0,
        )?;
        shoot(
            &mut runtime,
            &mut renderer,
            &a.out.join("underwater-up.png"),
            Vec3::new(6.0, y + 1.0, -20.0),
            Vec3::new(10.0, 2.0, -28.0),
            1.1,
        )?;
        runtime.water_press(4, false)?;
    }

    if wanted("hud") {
        // The spell bar with each slot's hover card, as the app draws them.
        place(&mut runtime, 4.0, 12.0, PI);
        run(&mut runtime, 0.3);
        let size = [a.width as f32, a.height as f32];
        let v = view(
            Vec3::new(10.0, 4.0, 20.0),
            Vec3::new(-2.0, 0.0, -20.0),
            0.9,
            aspect,
        );
        for index in 0..water::hotbar::COUNT {
            let mut hud = verse::ui::UiBatch::default();
            let slots: Vec<_> = runtime
                .water_bar()
                .ok_or("no Water Lab bar")?
                .into_iter()
                .map(|(_, slot)| slot)
                .collect();
            water::hotbar::draw(&mut hud, &atlas, size, 14.0, &slots);
            water::hotbar::draw_tip(&mut hud, &atlas, size, 14.0, index);
            let pixels = renderer.render(v, &runtime.dynamic_mesh(), &hud)?;
            let path = a.out.join(format!("hud-tip-{}.png", index + 1));
            write_png(&path, a.width, a.height, &pixels)?;
            eprintln!("wrote {}", path.display());
        }
    }

    if a.spells {
        spells(&a, &mut runtime, &mut renderer, aspect)?;
    }
    if a.orb {
        orb(&a, &mut runtime, &mut renderer, aspect)?;
    }

    if a.sequence > 0 {
        let dir = a.out.join("sequence");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        place(&mut runtime, 4.0, 12.0, PI);
        for k in 0..a.sequence {
            run(&mut runtime, 1.0 / 24.0);
            let t = k as f32 / a.sequence.max(1) as f32;
            let eye = Vec3::new(26.0 - 30.0 * t, 5.0 + 2.0 * t, 22.0 - 6.0 * t);
            let target = Vec3::new(-10.0 + 8.0 * t, 0.0, -40.0);
            shoot(
                &mut runtime,
                &mut renderer,
                &dir.join(format!("{k:04}.png")),
                eye,
                target,
                0.85,
            )?;
        }
    }
    times(&mut runtime, &mut renderer, aspect, a.frames)
}

fn set_noon(runtime: &mut WorldRuntime, noon: bool) {
    if let Some(lab) = runtime.water_lab_mut() {
        let want = if noon {
            water::Hour::Noon
        } else {
            water::Hour::Golden
        };
        if lab.hour != want {
            lab.turn_hour();
        }
    }
}

/// Casts each spell and renders it into `OUT/spells`.
fn spells(
    a: &Args,
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
) -> Result<(), String> {
    let dir = a.out.join("spells");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let idle = verse::controller::InputState::default();
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / (1.0 / 60.0)).round() as usize {
            runtime.tick(&idle, 1.0 / 60.0);
        }
    };
    let ui = verse::ui::UiBatch::default();
    let mut shoot =
        |runtime: &mut WorldRuntime, name: &str, eye: Vec3, target: Vec3| -> Result<(), String> {
            let pixels =
                renderer.render(view(eye, target, 0.9, aspect), &runtime.dynamic_mesh(), &ui)?;
            let path = dir.join(format!("{name}.png"));
            write_png(&path, a.width, a.height, &pixels)?;
            eprintln!("wrote {}", path.display());
            Ok(())
        };
    let stand = |runtime: &mut WorldRuntime, x: f32, z: f32, yaw: f32| {
        let _ = runtime.set_spawn(Vec3::new(x, water::ground(x, z), z), yaw);
    };
    // Water Walk: out on the sea, the swell under the character's feet.
    stand(runtime, 2.0, 4.0, PI);
    runtime.water_press(0, false)?;
    let walk = verse::controller::InputState {
        forward: true,
        ..Default::default()
    };
    for _ in 0..240 {
        runtime.tick(&walk, 1.0 / 60.0);
    }
    let p = runtime.player.pos;
    shoot(
        runtime,
        "water-walk",
        p + Vec3::new(4.5, 2.2, 5.0),
        p + Vec3::new(0.0, 0.8, 0.0),
    )?;
    runtime.water_press(0, false)?;
    stand(runtime, 2.0, 3.0, PI);
    // Control Water, each mode in turn, cast toward the bay.
    for (k, name) in ["flood", "part-water", "redirect-flow", "whirlpool"]
        .iter()
        .enumerate()
    {
        if k == 3 {
            // The whirlpool needs water 25 feet deep over 50 feet square:
            // cast it from out in the bay, where the bed is that deep.
            stand(runtime, 2.0, -50.0, PI);
        }
        runtime.water_press(1, false)?;
        run(runtime, if k == 0 { 9.0 } else { 6.0 });
        let (eye, target) = match k {
            0 => (Vec3::new(26.0, 7.0, 30.0), Vec3::new(-2.0, 0.0, 0.0)),
            1 => (Vec3::new(16.0, 10.0, 6.0), Vec3::new(2.0, -2.0, -12.0)),
            2 => (Vec3::new(16.0, 10.0, 6.0), Vec3::new(2.0, 0.0, -12.0)),
            _ => (Vec3::new(14.0, 12.0, -48.0), Vec3::new(2.0, -1.5, -64.0)),
        };
        let floats_z = if k == 3 { -60.0 } else { -8.0 };
        if k == 2 || k == 3 {
            // Floats to show the current and the pull.
            if let Some(lab) = runtime.water_lab_mut() {
                for i in 0..4 {
                    lab.floats.spawn(
                        water::FloatKind::ALL[i % 3],
                        Vec3::new(-4.0 + i as f32 * 3.0, 1.0, floats_z - i as f32),
                        i as f32,
                        Vec3::ZERO,
                    );
                }
            }
            run(runtime, 3.0);
        }
        shoot(runtime, name, eye, target)?;
    }
    runtime.water_press(1, true)?;
    run(runtime, 10.0);
    // Create Water: rain on the beach and the shallows; then Destroy.
    stand(runtime, 6.0, 10.0, PI);
    runtime.water_press(2, false)?;
    run(runtime, 5.0);
    shoot(
        runtime,
        "create-water",
        Vec3::new(14.0, 4.0, 14.0),
        Vec3::new(5.0, 0.0, 3.0),
    )?;
    runtime.water_press(2, true)?;
    run(runtime, 0.8);
    shoot(
        runtime,
        "destroy-water",
        Vec3::new(14.0, 4.0, 14.0),
        Vec3::new(5.0, 0.0, 3.0),
    )?;
    run(runtime, 6.0);
    // Sleet Storm over the bay.
    stand(runtime, 2.0, 10.0, PI);
    runtime.water_press(3, false)?;
    run(runtime, 5.0);
    shoot(
        runtime,
        "sleet-storm",
        Vec3::new(18.0, 8.0, 16.0),
        Vec3::new(2.0, 0.0, -4.0),
    )?;
    runtime.water_press(3, false)?;
    run(runtime, 2.5);
    shoot(
        runtime,
        "sleet-thaw",
        Vec3::new(18.0, 8.0, 16.0),
        Vec3::new(2.0, 0.0, -4.0),
    )?;
    run(runtime, 4.0);
    // Water Breathing: on the bed under the bay.
    runtime.water_press(4, false)?;
    stand(runtime, 6.0, -20.0, PI);
    run(runtime, 3.0);
    let p = runtime.player.pos;
    shoot(
        runtime,
        "water-breathing",
        p + Vec3::new(2.5, 1.6, 3.0),
        p + Vec3::new(0.0, 1.0, 0.0),
    )?;
    runtime.water_press(4, false)?;
    // The shared rules' other spells over the bay (phase W8), on a calm sea
    // so walkable ice forms as ice rather than floes.
    if let Some(lab) = runtime.water_lab_mut() {
        lab.set_sea(0);
    }
    let wanted = std::env::var("VERSE_SPELL_VIDEO").ok();
    // `VERSE_SPELL_VIDEO=fireball-volley`: Fireballs thrown one after
    // another at a straw dummy wading in the shallows, a second straw
    // dummy standing beside the line inside the blast, until both are down,
    // then the steam drifting over the bay; frames at 30 a second into
    // `OUT_DIR/spells/fireball-volley/`.
    if wanted.as_deref() == Some("fireball-volley") {
        use zones::grove::dummies::Kind;
        let (x, z) = (2.0, 10.0);
        stand(runtime, x, z, PI);
        let caster = Vec3::new(x, water::ground(x, z), z);
        if let Some(lab) = runtime.water_lab_mut() {
            lab.rules = Default::default();
            // The field's straw dummy wading at (2, -4) is the far one, on
            // the line; the beach's straw dummy steps beside the line inside
            // the blast; the warded dummy wades out of it.
            lab.targets[0] = water::targets::Target::new(Kind::Straw, [3.6, -1.0]);
            lab.targets[4] = water::targets::Target::new(Kind::Warded, [14.0, -7.0]);
            // The armored dummy steps off the camera's path.
            lab.targets[1] = water::targets::Target::new(Kind::Armored, [-12.0, 4.0]);
        }
        let out = dir.join("fireball-volley");
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        // The camera's path, a Catmull-Rom spline through these eyes and
        // where each looks, eased in and out over the whole film: behind
        // the caster's shoulder, then a slow orbit of about 100 degrees
        // round toward the dummies on the shore's side, so their names
        // read, ending risen over the misty bay.
        const TOTAL: usize = 15 * 30;
        let keys: [(Vec3, Vec3); 6] = [
            (Vec3::new(3.4, 2.3, 14.0), Vec3::new(2.2, 1.2, 2.0)),
            (Vec3::new(7.0, 3.2, 14.5), Vec3::new(2.3, 1.0, 0.0)),
            (Vec3::new(13.0, 4.2, 11.0), Vec3::new(2.5, 0.8, -1.0)),
            (Vec3::new(16.5, 5.2, 5.0), Vec3::new(2.6, 0.7, -2.0)),
            (Vec3::new(16.0, 7.5, 0.5), Vec3::new(2.4, 0.4, -2.5)),
            (Vec3::new(12.5, 11.0, 2.0), Vec3::new(2.0, 0.0, -3.5)),
        ];
        let spline = |u: f32| -> (Vec3, Vec3) {
            let u = u.clamp(0.0, 1.0);
            let u = u * u * (3.0 - 2.0 * u);
            let span = u * (keys.len() - 1) as f32;
            let i = (span.floor() as usize).min(keys.len() - 2);
            let t = span - i as f32;
            let at = |k: isize| keys[k.clamp(0, keys.len() as isize - 1) as usize];
            let i = i as isize;
            let (p0, p1, p2, p3) = (at(i - 1), at(i), at(i + 1), at(i + 2));
            let cr = |a: Vec3, b: Vec3, c: Vec3, d: Vec3| {
                0.5 * (2.0 * b
                    + (c - a) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                    + (3.0 * b - a - 3.0 * c + d) * t * t * t)
            };
            (cr(p0.0, p1.0, p2.0, p3.0), cr(p0.1, p1.1, p2.1, p3.1))
        };
        let frame = std::cell::Cell::new(0usize);
        let mut film = |runtime: &mut WorldRuntime, seconds: f32| -> Result<(), String> {
            for _ in 0..(seconds * 30.0).round() as usize {
                run(runtime, 1.0 / 30.0);
                let (eye, look) = spline(frame.get() as f32 / (TOTAL - 1) as f32);
                shoot(
                    runtime,
                    &format!("fireball-volley/{:04}", frame.get()),
                    eye,
                    look,
                )?;
                frame.set(frame.get() + 1);
            }
            Ok(())
        };
        film(runtime, 1.0)?;
        let down = |runtime: &WorldRuntime| {
            runtime
                .water_lab()
                .map(|lab| [lab.targets[3].dummy.down(), lab.targets[0].dummy.down()])
                .unwrap_or_default()
        };
        for cast in 1..=7 {
            let line = runtime.water_cast_demo(water::Demo::Fireball, caster, Vec3::NEG_Z);
            eprintln!("cast {cast}: {line}");
            film(runtime, 1.5)?;
            let hp = runtime
                .water_lab()
                .map(|lab| [lab.targets[3].dummy.hp, lab.targets[0].dummy.hp]);
            eprintln!("after cast {cast}: far and near hit points {hp:?}");
            if down(runtime).iter().all(|d| *d) {
                break;
            }
        }
        let left = TOTAL.saturating_sub(frame.get()) as f32 / 30.0;
        film(runtime, left.max(4.0))?;
        eprintln!("wrote {} frames to {}", frame.get(), out.display());
        return Ok(());
    }
    for spell in water::Demo::ALL {
        let name = spell.name().to_lowercase().replace(' ', "-");
        if name == "sleet-storm" || wanted.as_ref().is_some_and(|w| *w != name) {
            continue;
        }
        let (x, z) = (2.0, 3.0);
        stand(runtime, x, z, PI);
        let caster = Vec3::new(x, water::ground(x, z), z);
        let mut wait = 1.5;
        if let Some(lab) = runtime.water_lab_mut() {
            lab.rules = Default::default();
            if matches!(
                spell,
                water::Demo::GustOfWind | water::Demo::Thunderwave | water::Demo::ReverseGravity
            ) {
                for i in 0..5 {
                    let along = match spell {
                        water::Demo::ReverseGravity => -14.0,
                        water::Demo::Thunderwave => -1.0,
                        _ => -4.0,
                    };
                    lab.floats.spawn(
                        water::FloatKind::ALL[i % 3],
                        Vec3::new(x - 2.0 + i as f32, 0.3, z + along - i as f32 * 0.8),
                        i as f32,
                        Vec3::ZERO,
                    );
                }
                run(runtime, 2.0);
            }
        }
        // `VERSE_SPELL_VIDEO=NAME` records that spell's cast as frames at 30
        // a second into `OUT_DIR/spells/NAME-video/` instead of one still:
        // a second of the caster facing the bay, the cast, the flight, the
        // landing, and the aftermath, the camera easing from over the
        // caster's shoulder out to a view of both caster and target.
        let video = wanted.as_ref().is_some_and(|w| *w == name);
        let mut frame = 0;
        let lead = 30;
        let target_at = Vec3::new(x, water::LEVEL, z) + Vec3::NEG_Z * spell.reach();
        let near = (
            caster + Vec3::new(3.2, 2.1, 3.4),
            caster + Vec3::new(-0.4, 1.3, -2.0),
        );
        let wide = (
            Vec3::new(x + 15.0, 6.5, z + 1.0),
            caster.lerp(target_at, 0.55) + Vec3::Y * 0.8,
        );
        let mut film = |runtime: &mut WorldRuntime, frame: usize| -> Result<(), String> {
            let k = ((frame as f32 - lead as f32) / 60.0).clamp(0.0, 1.0);
            let k = k * k * (3.0 - 2.0 * k);
            let (eye, look) = (near.0.lerp(wide.0, k), near.1.lerp(wide.1, k));
            shoot(runtime, &format!("{name}-video/{frame:04}"), eye, look)
        };
        if video {
            std::fs::create_dir_all(dir.join(format!("{name}-video")))
                .map_err(|e| e.to_string())?;
            while frame < lead {
                run(runtime, 1.0 / 30.0);
                film(runtime, frame)?;
                frame += 1;
            }
        }
        let line = runtime.water_cast_demo(spell, caster, Vec3::NEG_Z);
        eprintln!("{line}");
        if video {
            while frame < lead + 150 {
                run(runtime, 1.0 / 30.0);
                film(runtime, frame)?;
                frame += 1;
            }
            eprintln!("wrote {}", dir.join(format!("{name}-video")).display());
            continue;
        }
        wait = match spell {
            water::Demo::Freeze(_) if name == "storm-of-vengeance" => 26.0,
            water::Demo::GustOfWind => 4.0,
            water::Demo::ReverseGravity => 0.9,
            water::Demo::MeteorSwarm | water::Demo::Thunderwave => 0.5,
            water::Demo::Fireball => 0.8,
            _ => wait,
        };
        run(runtime, wait);
        let (eye, target) = match name.as_str() {
            "storm-of-vengeance" | "meteor-swarm" => {
                (Vec3::new(26.0, 14.0, 12.0), Vec3::new(2.0, 0.0, -18.0))
            }
            "ray-of-frost" | "ice-knife" => (Vec3::new(9.0, 4.5, 4.0), Vec3::new(2.0, 0.0, -5.0)),
            "reverse-gravity" => (Vec3::new(18.0, 6.0, 8.0), Vec3::new(2.0, 3.0, -11.0)),
            "thunderwave" => (Vec3::new(10.0, 5.0, 8.0), Vec3::new(2.0, 0.0, -1.0)),
            _ => (Vec3::new(17.0, 9.0, 9.0), Vec3::new(2.0, 0.0, -10.0)),
        };
        shoot(runtime, &name, eye, target)?;
    }
    if let Some(lab) = runtime.water_lab_mut() {
        lab.rules = Default::default();
        lab.set_sea(water::sea::DEFAULT_SEA);
    }
    // Swimming without it.
    stand(runtime, 0.0, -24.0, PI);
    run(runtime, 3.0);
    let p = runtime.player.pos;
    shoot(
        runtime,
        "swimming",
        p + Vec3::new(-3.0, 1.8, 3.5),
        p + Vec3::new(0.0, 0.8, 0.0),
    )?;
    Ok(())
}

/// The follow camera at `yaw` (around the player), `pitch`, and
/// `distance`, as a player would frame the shot.
fn frame(runtime: &mut WorldRuntime, yaw: f32, pitch: f32, distance: f32) {
    runtime.camera.yaw_offset = yaw;
    runtime.camera.pitch = pitch;
    runtime.camera.distance = distance;
}

/// The Water Orb and the Thunderbolt, into `OUT/orb`, through the follow
/// camera, so billboards face the eye that renders them.
fn orb(
    a: &Args,
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
) -> Result<(), String> {
    let dir = a.out.join("orb");
    std::fs::create_dir_all(dir.join("sequence")).map_err(|e| e.to_string())?;
    let idle = verse::controller::InputState::default();
    let run = |runtime: &mut WorldRuntime, seconds: f32| {
        for _ in 0..(seconds / (1.0 / 60.0)).round() as usize {
            runtime.tick(&idle, 1.0 / 60.0);
        }
    };
    let ui = verse::ui::UiBatch::default();
    let shoot =
        |runtime: &mut WorldRuntime, renderer: &mut Offscreen, path: &Path| -> Result<(), String> {
            let pixels = renderer.render(runtime.view(aspect), &runtime.dynamic_mesh(), &ui)?;
            write_png(path, a.width, a.height, &pixels)?;
            eprintln!("wrote {}", path.display());
            Ok(())
        };
    let stand = |runtime: &mut WorldRuntime, x: f32, z: f32, yaw: f32| {
        let _ = runtime.set_spawn(Vec3::new(x, water::ground(x, z), z), yaw);
    };
    // Aims the pointer from the camera's eye at `at`.
    let aim = |runtime: &mut WorldRuntime, at: Vec3| {
        let eye = runtime.view(aspect).eye;
        if let Some(lab) = runtime.water_lab_mut() {
            lab.set_aim(Some((eye, (at - eye).normalize())));
        }
    };
    let orb_key = water::hotbar::ORB;
    let bolt_key = 6;
    // Growing on the beach, drawing streams up out of the bay.
    stand(runtime, 4.0, 16.0, PI);
    run(runtime, 0.5);
    frame(runtime, 1.1, 0.1, 9.0);
    runtime.water_press(orb_key, false)?;
    run(runtime, 1.0);
    shoot(runtime, renderer, &dir.join("growing.png"))?;
    // Grown to its largest: 12 m across.
    run(runtime, 3.2);
    frame(runtime, 0.55, 0.12, 17.0);
    shoot(runtime, renderer, &dir.join("huge.png"))?;
    frame(runtime, 1.25, 0.1, 24.0);
    shoot(runtime, renderer, &dir.join("huge-side.png"))?;
    // Thrown out over the bay, where floats bob, and its splash.
    if let Some(lab) = runtime.water_lab_mut() {
        for (k, (x, z)) in [(-6.0, -7.0), (3.0, -12.0), (-4.0, -14.0), (5.0, -8.0)]
            .into_iter()
            .enumerate()
        {
            lab.floats.spawn(
                water::FloatKind::ALL[k % 3],
                Vec3::new(x, 0.3, z),
                k as f32,
                Vec3::ZERO,
            );
        }
    }
    run(runtime, 0.5);
    frame(runtime, 0.75, 0.12, 15.0);
    aim(runtime, Vec3::new(-1.0, 0.0, -9.0));
    runtime.water_release(orb_key, false)?;
    run(runtime, 0.3);
    shoot(runtime, renderer, &dir.join("throw.png"))?;
    // Down the beach to watch it land.
    stand(runtime, 3.0, 6.0, PI);
    frame(runtime, 0.25, 0.08, 7.0);
    let mut waited = 0.0;
    while runtime.water_lab().is_some_and(|lab| !lab.orbs.is_empty()) && waited < 4.0 {
        run(runtime, 1.0 / 60.0);
        waited += 1.0 / 60.0;
    }
    run(runtime, 0.12);
    shoot(runtime, renderer, &dir.join("impact.png"))?;
    run(runtime, 0.3);
    shoot(runtime, renderer, &dir.join("splash.png"))?;
    run(runtime, 0.5);
    shoot(runtime, renderer, &dir.join("splash-late.png"))?;
    run(runtime, 3.0);
    // An orb set hovering round the beach's straw dummy, with a crate and
    // a barrel dropped in.
    let dummy = runtime.water_lab().ok_or("no lab")?.targets[0].dummy.pos;
    stand(runtime, dummy.x, dummy.z + 5.2, PI);
    run(runtime, 0.3);
    runtime.water_press(orb_key, false)?;
    run(
        runtime,
        (4.0 - water::orb::MIN_RADIUS) / water::orb::GROW_FROM_WATER,
    );
    runtime.water_release(orb_key, true)?;
    if let Some(lab) = runtime.water_lab_mut() {
        let c = lab.orbs[0].center;
        lab.floats.spawn(
            water::FloatKind::Crate,
            c + Vec3::new(1.4, 1.5, 0.3),
            0.4,
            Vec3::ZERO,
        );
        lab.floats.spawn(
            water::FloatKind::Barrel,
            c + Vec3::new(-1.5, 1.2, -0.4),
            1.0,
            Vec3::ZERO,
        );
    }
    run(runtime, 3.0);
    frame(runtime, 0.85, 0.08, 12.0);
    shoot(runtime, renderer, &dir.join("engulfed.png"))?;
    // The Thunderbolt on the orb: the strike, then the charge crackling
    // through everything inside.
    let center = runtime.water_lab().ok_or("no lab")?.orbs[0].center;
    aim(runtime, center);
    runtime.water_press(bolt_key, false)?;
    run(runtime, 0.05);
    shoot(runtime, renderer, &dir.join("lightning-strike.png"))?;
    run(runtime, 0.2);
    shoot(runtime, renderer, &dir.join("electrified.png"))?;
    run(runtime, 0.5);
    shoot(runtime, renderer, &dir.join("electrified-late.png"))?;
    // The Thunderbolt in the sea between the two wading dummies.
    run(runtime, 2.0);
    stand(runtime, 6.0, 7.0, PI);
    run(runtime, 0.3);
    frame(runtime, 0.4, 0.2, 9.0);
    aim(runtime, Vec3::new(4.5, 0.0, -9.0));
    runtime.water_press(bolt_key, false)?;
    run(runtime, 0.11);
    shoot(runtime, renderer, &dir.join("sea-conduction.png"))?;
    run(runtime, 0.4);
    shoot(runtime, renderer, &dir.join("sea-conduction-after.png"))?;
    // A frame sequence at 24 frames a second: grow, throw, and splash,
    // then an orb round the dummy struck by lightning.
    run(runtime, 11.0);
    let mut frame_index = 0;
    let mut record = |runtime: &mut WorldRuntime,
                      renderer: &mut Offscreen,
                      seconds: f32|
     -> Result<(), String> {
        for _ in 0..(seconds * 24.0).round() as usize {
            run(runtime, 1.0 / 24.0);
            shoot(
                runtime,
                renderer,
                &dir.join("sequence").join(format!("{frame_index:04}.png")),
            )?;
            frame_index += 1;
        }
        Ok(())
    };
    stand(runtime, 4.0, 16.0, PI);
    run(runtime, 0.5);
    frame(runtime, 0.6, 0.14, 12.0);
    runtime.water_press(orb_key, false)?;
    record(runtime, renderer, 2.6)?;
    aim(runtime, Vec3::new(-1.0, 0.0, -9.0));
    runtime.water_release(orb_key, false)?;
    record(runtime, renderer, 0.5)?;
    stand(runtime, 3.0, 6.0, PI);
    frame(runtime, 0.25, 0.08, 7.0);
    record(runtime, renderer, 1.6)?;
    let dummy = runtime.water_lab().ok_or("no lab")?.targets[0].dummy.pos;
    stand(runtime, dummy.x, dummy.z + 5.2, PI);
    frame(runtime, 0.85, 0.08, 12.0);
    runtime.water_press(orb_key, false)?;
    record(runtime, renderer, 2.4)?;
    runtime.water_release(orb_key, true)?;
    record(runtime, renderer, 0.6)?;
    let center = runtime.water_lab().ok_or("no lab")?.orbs[0].center;
    aim(runtime, center);
    runtime.water_press(bolt_key, false)?;
    record(runtime, renderer, 1.6)?;
    Ok(())
}

fn times(
    runtime: &mut WorldRuntime,
    renderer: &mut Offscreen,
    aspect: f32,
    frames: usize,
) -> Result<(), String> {
    let _ = runtime.set_spawn(water::spawn(), water::SPAWN_YAW);
    let ui = verse::ui::UiBatch::default();
    let idle = verse::controller::InputState::default();
    let mut cpu = Vec::with_capacity(frames);
    let mut gpu = Vec::with_capacity(frames);
    for i in 0..frames {
        let started = Instant::now();
        runtime.tick(&idle, 1.0 / 60.0);
        let t = i as f32 / frames.max(1) as f32;
        let v = view(
            Vec3::new(20.0 - 20.0 * t, 6.0, 26.0),
            Vec3::new(-4.0, 0.0, -30.0),
            0.9,
            aspect,
        );
        let dynamic = runtime.dynamic_mesh();
        let built = Instant::now();
        renderer.render(v, &dynamic, &ui)?;
        let done = Instant::now();
        cpu.push((built - started).as_secs_f64() * 1e3);
        gpu.push((done - built).as_secs_f64() * 1e3);
    }
    let summary = |name: &str, samples: &mut Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        let at = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        println!(
            "{name}: median {:.2} ms, p95 {:.2} ms, max {:.2} ms over {} frames",
            at(0.5),
            at(0.95),
            at(1.0),
            samples.len()
        );
    };
    summary("cpu (tick and frame)", &mut cpu);
    summary("gpu (render and readback)", &mut gpu);
    Ok(())
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(pixels))
        .map_err(|e| format!("{}: {e}", path.display()))
}
