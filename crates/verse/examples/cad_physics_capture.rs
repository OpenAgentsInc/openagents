//! Native proof of placed CAD bodies: push, Reverse Gravity, then a blast.
//! `cargo run -p verse --no-default-features --features capture,dev-destruction
//! --example cad_physics_capture -- OUT_DIR [--measure]`

use glam::Vec3;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{Intent, everglade_pack},
};

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
const FPS: u32 = 12;
fn main() -> Result<(), String> {
    let out = PathBuf::from(std::env::args().nth(1).ok_or("Give a capture directory.")?);
    let measure = std::env::args().any(|a| a == "--measure");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let placements = out.join("placements.json");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let glb = root.join("assets/verse/cad/rover.glb");
    let copies:Vec<_>=[-2.5,2.5].into_iter().enumerate().map(|(i,x)|serde_json::json!({"id":format!("arrival-rover-{i}"),"doc":"rover","version":38,"zone":"verse-everglade","glb":glb,"at":[x,0.,-28.]})).collect();
    std::fs::write(
        &placements,
        serde_json::json!({"kind":"verse-cad-placements.v1","placements":copies}).to_string(),
    )
    .map_err(|e| e.to_string())?;
    let pack = root.join(everglade_pack::PACK_DIRECTORY).join(format!(
        "{}.{}",
        everglade_pack::PACK_SHA256,
        everglade_pack::PACK_EXTENSION
    ));
    let pack = everglade_pack::ZonePack::load_local(&pack)?;
    let mut runtime = WorldRuntime::new();
    runtime.set_town_clock(verse::town_clock::Clock::DAYTIME.pinned(Some(15.)));
    runtime.install_everglade(&pack);
    runtime.set_dev_destruction(true)?;
    runtime.refresh_cad_placements_from(&placements);
    runtime.set_spawn(Vec3::new(-1.15, 0., -28.), -std::f32::consts::FRAC_PI_2)?;
    let atlas = verse::ui::Atlas::new(20.);
    let ui = verse::ui::UiBatch::default();
    let mut renderer = if measure {
        None
    } else {
        Some(verse::render::Offscreen::new(
            WIDTH,
            HEIGHT,
            &runtime.world.mesh,
            &atlas,
            runtime.atmosphere(),
        )?)
    };
    let mut film = if measure {
        None
    } else {
        Some(
            Command::new("ffmpeg")
                .args([
                    "-y",
                    "-loglevel",
                    "error",
                    "-f",
                    "rawvideo",
                    "-pixel_format",
                    "rgba",
                    "-video_size",
                    "960x540",
                    "-framerate",
                    "12",
                    "-i",
                    "-",
                    "-an",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "fast",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .arg(out.join("cad-physics.mp4"))
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?,
        )
    };
    let mut records = Vec::new();
    for frame in 0..FPS * 12 {
        if frame % FPS == 0 {
            eprintln!(
                "frame {frame}: {:?}",
                runtime
                    .cad_report("arrival-rover-0")
                    .map(|r| (r.broken, r.positions[0]))
            );
        }
        if frame == FPS * 2 {
            runtime.set_spawn(Vec3::new(0., 0., -32.), 0.)?;
            runtime.zone_intent(Intent::ReverseGravity)?;
        }
        if frame == FPS * 4 {
            runtime.zone_intent(Intent::ReverseGravity)?;
        }
        let report = runtime
            .cad_report("arrival-rover-0")
            .ok_or("The rover has no bodies.")?;
        let y = report.positions.iter().map(|p| p[1] as f32).sum::<f32>()
            / report.positions.len() as f32;
        let right = runtime
            .cad_report("arrival-rover-1")
            .ok_or("The other rover has no bodies.")?;
        let at = Vec3::from_array(report.positions[0].map(|x| x as f32));
        let other = Vec3::from_array(right.positions[0].map(|x| x as f32));
        let target = Vec3::new((at.x + other.x) * 0.5, y, (at.z + other.z) * 0.5);
        let eye = target + Vec3::new(0., 2.2, -5.);
        runtime.set_shot(Some((eye, target)));
        if frame == FPS * 8 {
            let ground = Vec3::new(at.x, 0., at.z);
            runtime.set_spawn(ground - Vec3::Z * 5., 0.)?;
            runtime.set_shot(Some((ground + Vec3::new(0., 3., -7.), ground)));
            runtime.zone_intent(Intent::MegaThunderbolt)?;
            if !runtime.demolition_aim(WIDTH as f32 / HEIGHT as f32, 0.5, 0.5)
                || !runtime.demolition_confirm()
            {
                return Err("The blast could not be aimed.".into());
            }
            runtime.set_shot(Some((eye, target)));
        }
        for _ in 0..5 {
            let input = InputState {
                forward: frame < FPS / 2,
                ..Default::default()
            };
            runtime.tick(&input, 1. / 60.);
        }
        records.push(serde_json::json!({"frame":frame,"seconds":frame as f32/FPS as f32,"player":runtime.player.pos.to_array(),"intent":[runtime.player.ahead,runtime.player.side],
            "left":runtime.cad_report("arrival-rover-0"),"right":runtime.cad_report("arrival-rover-1")}));
        if let (Some(renderer), Some(film)) = (&mut renderer, &mut film) {
            let mut dynamic = runtime.dynamic_mesh();
            // Show the push, then keep the caster out of the objects' shot.
            let hide_player = frame >= FPS * 2;
            if hide_player || runtime.hides_avatar() {
                let player = runtime.player.clone();
                let gait = runtime.gait.clone();
                dynamic.figure = runtime
                    .everglade_zone_mut()
                    .ok_or("Everglade closed.")?
                    .player_mesh(&player, &gait, hide_player)
                    .figure;
            }
            let pixels =
                renderer.render(runtime.view(WIDTH as f32 / HEIGHT as f32), &dynamic, &ui)?;
            film.stdin
                .as_mut()
                .ok_or("No video input.")?
                .write_all(&pixels)
                .map_err(|e| e.to_string())?;
            if frame % FPS == 0 {
                let file = std::fs::File::create(out.join(format!("frame-{frame:03}.png")))
                    .map_err(|e| e.to_string())?;
                let mut e = png::Encoder::new(file, WIDTH, HEIGHT);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header()
                    .and_then(|mut w| w.write_image_data(&pixels))
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    if let Some(mut film) = film {
        drop(film.stdin.take());
        if !film.wait().map_err(|e| e.to_string())?.success() {
            return Err("Video encoding failed.".into());
        }
    }
    std::fs::write(
        out.join("measurements.json"),
        serde_json::to_vec_pretty(&records).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    eprintln!("wrote {}", out.display());
    Ok(())
}
