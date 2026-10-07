//! Renders the Grid robot in the Grid from the pinned engine pack, the way
//! the OpenAgents app and phones draw the Grid.
//! Usage: grid_robot_capture OUT_DIR
//!
//! Writes `spawn.png` (the phone's portrait spawn view a few seconds in, the
//! robot walking its patrol), `spawn_wide.png` (the same at desktop size),
//! close views at moments of the patrol beside a line figure for scale
//! (`walk.png`, `turn.png`, `kneel.png`, `interact.png`), `far.png` (the far
//! level of detail from 45 meters), and `orbit_NN.png`, eight views around
//! the robot standing idle. Prints the robot's triangles per level.
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

    // The spawn view, a few seconds into the patrol.
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

    // Close views at moments of the patrol, a line figure beside it.
    let (width, height) = (1000, 1000);
    let mut r = renderer(width, height, &atlas)?;
    let shot = |r: &mut Renderer, pose: Pose, eye_offset: Vec3, file: &str| -> Result<(), String> {
        let side = Quat::from_rotation_y(pose.yaw) * Vec3::new(-1.1, 0.0, 0.3);
        let figure = grid_frame::figure(
            pose.pos + side,
            Quat::from_rotation_y(pose.yaw),
            &verse::avatar::Gait::default(),
            0.0,
        );
        let eye = pose.pos + Quat::from_rotation_y(pose.yaw) * eye_offset;
        let view = look(eye, pose.pos + Vec3::Y * 0.95, 1.0);
        let robot = grid_robot::instance(&pose, eye);
        let pixels = r.draw(view, &[robot, figure], &ui, &lighting)?;
        write(&out.join(file), width, height, &pixels)
    };
    let walk = grid_robot::pose(3.0);
    shot(&mut r, walk, Vec3::new(1.6, 1.5, 3.2), "walk.png")?;
    let east = 9.0f64.hypot(0.0) / f64::from(grid_robot::WALK_SPEED);
    shot(
        &mut r,
        grid_robot::pose(east + 0.5),
        Vec3::new(1.4, 1.4, 3.0),
        "turn.png",
    )?;
    shot(
        &mut r,
        grid_robot::pose(east + 1.2 + 2.6),
        Vec3::new(1.6, 1.3, 2.8),
        "kneel.png",
    )?;
    let west = east * 2.0 + 1.2 + f64::from(grid_robot::KNEEL_SECONDS) + 1.6;
    shot(
        &mut r,
        grid_robot::pose(west + 1.2 + 1.0),
        Vec3::new(1.4, 1.5, 3.0),
        "interact.png",
    )?;
    let idle = grid_robot::pose(west + 1.2 + 2.0 + 1.5);
    for k in 0..8 {
        let yaw = k as f32 * std::f32::consts::TAU / 8.0;
        let eye = idle.pos + Quat::from_rotation_y(idle.yaw + yaw) * Vec3::new(0.0, 1.35, 3.0);
        let view = look(eye, idle.pos + Vec3::Y * 0.95, 1.0);
        let pixels = r.draw(view, &[grid_robot::instance(&idle, eye)], &ui, &lighting)?;
        write(
            &out.join(format!("orbit_{k:02}.png")),
            width,
            height,
            &pixels,
        )?;
    }
    // The far level of detail.
    let eye = idle.pos + Quat::from_rotation_y(idle.yaw) * Vec3::new(10.0, 6.0, 44.0);
    let view = look(eye, idle.pos + Vec3::Y * 0.95, 1.0);
    let pixels = r.draw(view, &[grid_robot::instance(&idle, eye)], &ui, &lighting)?;
    write(&out.join("far.png"), width, height, &pixels)
}
