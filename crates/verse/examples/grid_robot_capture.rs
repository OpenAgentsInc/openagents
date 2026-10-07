//! Renders the Grid robots in the Grid from the pinned engine pack, the way
//! the OpenAgents app and phones draw the Grid.
//! Usage: grid_robot_capture OUT_DIR
//!
//! Writes `spawn.png` (the phone's portrait spawn view a few seconds in) and
//! `spawn_wide.png` (the same at desktop size), `overview.png` (the plaza
//! from above with all five robots), `patrol_N.png` (each patroller walking
//! beside a line figure for scale), `turn.png` (a patroller idling at the
//! end of its route), `dance_NN.png` (the dancer beside the Everglade arch
//! through one loop of its dance), and `far.png` (the far level of detail
//! from 45 meters). Prints the robot's triangles per level, and the spawn
//! view's GPU time with and without the robots.
use std::path::PathBuf;

use glam::{Mat4, Quat, Vec3};
use verse::grid_robot::{self, Pose};
use verse::imported::Renderer;
use verse::render::View;
use verse::runtime::WorldRuntime;
use verse::{grid_frame, grid_pack};

fn write(path: &std::path::Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels)
        .map_err(|e| e.to_string())?;
    println!("wrote {}", path.display());
    Ok(())
}

fn renderer(width: u32, height: u32, atlas: &verse::ui::Atlas) -> Result<Renderer, String> {
    let pack = grid_pack::load_pinned()?;
    let statics = grid_frame::statics(&pack);
    Renderer::new(
        pack,
        &grid_pack::pinned_dir(),
        width,
        height,
        atlas,
        &statics,
    )
}

/// A camera at `eye` looking at `at`.
fn look(eye: Vec3, at: Vec3, aspect: f32) -> View {
    View {
        view_proj: Mat4::perspective_rh(0.75, aspect, 0.1, 600.0)
            * Mat4::look_at_rh(eye, at, Vec3::Y),
        eye,
    }
}

/// The Everglade arch, which the app stands once it has zone storage; this
/// capture's runtime has none.
fn everglade_arch() -> verse_engine::presentation::Instance {
    let gate = verse::zones::Gate::everglade(&verse::blocks::Layout::grid());
    verse_engine::presentation::Instance {
        mount: None,
        actor: None,
        model: grid_pack::ARCH_EVERGLADE.into(),
        transform: Mat4::from_rotation_translation(Quat::from_rotation_y(gate.yaw), gate.at),
        animation: 0.into(),
        time: 0.,
        animation_epoch: None,
        emission: Vec3::ZERO,
    }
}

/// The median of `samples`.
fn median(mut samples: Vec<f64>) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn main() -> Result<(), String> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Expected an output directory")?,
    );
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let pack = grid_pack::load_pinned()?;
    for name in [grid_robot::ROBOT, grid_robot::ROBOT_FAR] {
        let triangles: usize = pack.models[name]
            .surfaces
            .iter()
            .filter(|s| s.topology == verse_engine::assets::Topology::Triangles)
            .map(|s| s.indices.len() / 3)
            .sum();
        let lines: usize = pack.models[name]
            .surfaces
            .iter()
            .filter(|s| s.topology == verse_engine::assets::Topology::Lines)
            .map(|s| s.indices.len() / 2)
            .sum();
        println!("{name}: {triangles} triangles, {lines} lines");
    }
    let atlas = verse::ui::Atlas::new(16.0);
    let ui = verse::ui::UiBatch::default();

    // The spawn view, a few seconds in.
    let mut runtime = WorldRuntime::bare();
    for _ in 0..(4.0 * 60.0) as usize {
        runtime.tick(&verse::controller::InputState::default(), 1.0 / 60.0);
    }
    let lighting = grid_frame::lighting(&runtime.atmosphere());
    for (file, width, height) in [("spawn.png", 590, 1280), ("spawn_wide.png", 1600, 1000)] {
        let mut r = renderer(width, height, &atlas)?;
        let view = runtime.view(width as f32 / height as f32);
        let dynamic = grid_frame::dynamic(&runtime, &[], &[]);
        let pixels = r.draw(view, &dynamic, &ui, &lighting)?;
        write(&out.join(file), width, height, &pixels)?;
    }

    // The frame cost of the five robots: the desktop spawn view drawn with
    // and without them.
    {
        let (width, height) = (1920, 1080);
        let mut r = renderer(width, height, &atlas)?;
        let view = runtime.view(width as f32 / height as f32);
        let with = grid_frame::dynamic(&runtime, &[], &[]);
        let without: Vec<_> = with
            .iter()
            .filter(|i| !i.model.as_str().starts_with(grid_robot::ROBOT))
            .cloned()
            .collect();
        let near = with
            .iter()
            .filter(|i| i.model.as_str() == grid_robot::ROBOT)
            .count();
        let mut gpu = |dynamic: &[verse_engine::presentation::Instance]| {
            let mut samples = Vec::new();
            for _ in 0..40 {
                r.draw(view, dynamic, &ui, &lighting)?;
                samples.push(r.last_timings.gpu_wait_ms);
            }
            Ok::<f64, String>(median(samples))
        };
        let (base, robots) = (gpu(&without)?, gpu(&with)?);
        println!(
            "spawn view at {width}x{height}: GPU wait {base:.2} ms without the robots, \
             {robots:.2} ms with five ({near} near, {} far)",
            with.len() - without.len() - near
        );
    }

    let (width, height) = (1000, 1000);
    let mut r = renderer(width, height, &atlas)?;

    // The plaza from above and behind the spawn, every robot in view.
    {
        let spawn = verse::world::SPAWN;
        let eye = spawn + Vec3::new(0.0, 34.0, -30.0);
        let view = look(eye, spawn + Vec3::new(0.0, 0.0, 10.0), 1.0);
        let mut dynamic = grid_frame::dynamic(&runtime, &[], &[]);
        dynamic.push(everglade_arch());
        let pixels = r.draw(view, &dynamic, &ui, &lighting)?;
        write(&out.join("overview.png"), width, height, &pixels)?;
    }

    // Close views beside a line figure for scale.
    let shot = |r: &mut Renderer,
                pose: Pose,
                eye_offset: Vec3,
                file: &str,
                extra: &[verse_engine::presentation::Instance]|
     -> Result<(), String> {
        let side = Quat::from_rotation_y(pose.yaw) * Vec3::new(-1.5, 0.0, 0.4);
        let figure = grid_frame::figure(
            pose.pos + side,
            Quat::from_rotation_y(pose.yaw),
            &verse::avatar::Gait::default(),
            0.0,
        );
        let eye = pose.pos + Quat::from_rotation_y(pose.yaw) * eye_offset;
        let view = look(eye, pose.pos + Vec3::Y * 1.4, 1.0);
        let mut dynamic = vec![grid_robot::instance(&pose, eye), figure];
        dynamic.extend_from_slice(extra);
        let pixels = r.draw(view, &dynamic, &ui, &lighting)?;
        write(&out.join(file), width, height, &pixels)
    };
    for route in 0..grid_robot::ROUTES.len() {
        shot(
            &mut r,
            grid_robot::patroller(route, 3.0 + route as f64 * 1.7),
            Vec3::new(2.4, 2.2, 4.8),
            &format!("patrol_{route}.png"),
            &[],
        )?;
    }
    // Patroller 0's first pause, a moment into its turn.
    let [a, b] = grid_robot::ROUTES[0];
    let leg = f64::from((b[0] - a[0]).hypot(b[1] - a[1]) / grid_robot::WALK_SPEED);
    shot(
        &mut r,
        grid_robot::patroller(0, leg + 0.7),
        Vec3::new(2.2, 2.0, 4.6),
        "turn.png",
        &[],
    )?;

    // The dancer beside the Everglade arch, through one loop of its dance,
    // seen from the approach with the arch in frame.
    let arch = verse::zones::Gate::everglade(&verse::blocks::Layout::grid());
    let steps = 8;
    for k in 0..steps {
        let clock = f64::from(grid_robot::DANCE_SECONDS) * k as f64 / steps as f64;
        let pose = grid_robot::dancer(clock);
        let toward = Quat::from_rotation_y(arch.yaw) * Vec3::new(0.6, 0.0, -1.0);
        let eye = pose.pos + toward.normalize() * 7.0 + Vec3::Y * 2.4;
        let target = pose.pos.lerp(arch.at, 0.35) + Vec3::Y * 1.6;
        let view = look(eye, target, 1.0);
        let figure = grid_frame::figure(
            pose.pos + Quat::from_rotation_y(pose.yaw) * Vec3::new(-1.6, 0.0, 0.3),
            Quat::from_rotation_y(pose.yaw),
            &verse::avatar::Gait::default(),
            0.0,
        );
        let mut dynamic = vec![everglade_arch()];
        dynamic.push(grid_robot::instance(&pose, eye));
        dynamic.push(figure);
        let pixels = r.draw(view, &dynamic, &ui, &lighting)?;
        write(
            &out.join(format!("dance_{k:02}.png")),
            width,
            height,
            &pixels,
        )?;
    }

    // The far level of detail.
    let idle = grid_robot::dancer(0.0);
    let eye = idle.pos + Quat::from_rotation_y(idle.yaw) * Vec3::new(10.0, 6.0, 44.0);
    let view = look(eye, idle.pos + Vec3::Y * 1.4, 1.0);
    let pixels = r.draw(view, &[grid_robot::instance(&idle, eye)], &ui, &lighting)?;
    write(&out.join("far.png"), width, height, &pixels)
}
