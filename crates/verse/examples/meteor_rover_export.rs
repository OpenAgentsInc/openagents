//! The Meteor Showcase's rover break-up as data, for another renderer
//! (an Unreal level sequence, Blender, a plot): the same simulated scene
//! `meteor_rover_capture` films (`verse::zones::meteor_rover`: Kinney-Graham
//! blast, bolted joints), stepped without rendering, and written out.
//!
//! Usage: meteor_rover_export OUT_DIR [--rate HZ] [--seconds N]
//!        [--delay S] [--paint KEY=RRGGBB]...
//!
//! Writes into `OUT_DIR`, in Verse's frame (metres, Y up, right-handed):
//!
//! - `bodies/<k>.obj`: every rigid body's mesh in its own frame, written
//!   when it first appears (the rover's solids at the start; a fragment
//!   when a meteor breaks one), named by its index `k`.
//! - `bodies.csv`: `k,instance,first_sample,r,g,b`, its solid and its color
//!   (linear RGB, after `--paint`).
//! - `poses.csv`: `sample,time,k,x,y,z,qx,qy,qz,qw` for every body at
//!   every sample (`--rate` a second, 120 by default), so a world vertex
//!   is `position + rotation * vertex`.
//! - `blasts.csv`: `time,x,y,z,radius,fx,fy,fz`: every meteor that landed
//!   (where, its blast's damage radius, and the direction it flew).
//! - `scene.json`: the rate, the rover's place and heading, the caster's
//!   place, when the cast went up, and the hardest hit on the rover.
//!
//! `--delay` stages the cast that many seconds in (the showcase's own by
//! default); `--paint` repaints as `meteor_cast_capture` does.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::{
    controller::InputState,
    runtime::WorldRuntime,
    zones::{self, everglade, everglade_pack, meteor_rover::Livery, meteor_showcase as showcase},
};

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// A body's mesh as a Wavefront OBJ.
fn obj(positions: &[Vec3], normals: &[Vec3], indices: &[u32]) -> String {
    let mut s = String::new();
    for p in positions {
        let _ = writeln!(s, "v {} {} {}", p.x, p.y, p.z);
    }
    for n in normals {
        let _ = writeln!(s, "vn {} {} {}", n.x, n.y, n.z);
    }
    for t in indices.chunks_exact(3) {
        let [a, b, c] = [t[0] + 1, t[1] + 1, t[2] + 1];
        let _ = writeln!(s, "f {a}//{a} {b}//{b} {c}//{c}");
    }
    s
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let (mut rate, mut seconds, mut delay) = (120.0_f64, 12.0_f64, showcase::DELAY);
    let mut paint = Vec::new();
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--rate" => rate = value()?.parse().map_err(|_| "--rate needs a number")?,
            "--seconds" => seconds = value()?.parse().map_err(|_| "--seconds needs a number")?,
            "--delay" => delay = value()?.parse().map_err(|_| "--delay needs a number")?,
            "--paint" => paint.push(value()?),
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    let livery = Livery::parse(&paint)?;
    std::fs::create_dir_all(out.join("bodies")).map_err(|e| format!("{}: {e}", out.display()))?;
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
    runtime.install_meteor_showcase(&pack);
    if runtime.zone != zones::ZoneId::MeteorShowcase {
        return Err("The Meteor Showcase did not install from the pinned pack".into());
    }
    runtime.set_spawn(Vec3::new(-46.0, 0.0, -80.0), 0.0)?;
    if !runtime.add_meteor_rover() || !runtime.stage_meteor_rover(delay) {
        return Err("The rover and the cast could not be staged".into());
    }
    if let Some(rover) = runtime.meteor_rover_mut() {
        rover.repaint(|instance, color| livery.paint(instance, color));
    }
    let idle = InputState::default();
    // Two simulation steps a sample at the default rate (240 Hz).
    let steps = ((240.0 / rate).round() as usize).max(1);
    let dt = (1.0 / (rate * steps as f64)) as f32;
    let samples = (seconds * rate).round() as usize;
    let mut seen = std::collections::BTreeSet::new();
    let mut bodies = String::from("k,instance,first_sample,r,g,b\n");
    let mut poses = String::from("sample,time,k,x,y,z,qx,qy,qz,qw\n");
    for n in 0..=samples {
        if n > 0 {
            for _ in 0..steps {
                runtime.tick(&idle, dt);
            }
        }
        let rover = runtime.meteor_rover().ok_or("The rover is gone")?;
        let time = n as f64 / rate;
        for (k, instance, pos, rot) in rover.bodies() {
            if seen.insert(k) {
                let (positions, normals, indices, color) =
                    rover.body_mesh(k).ok_or("A body has no mesh")?;
                write(
                    &out.join("bodies").join(format!("{k}.obj")),
                    &obj(&positions, &normals, &indices),
                )?;
                let _ = writeln!(
                    bodies,
                    "{k},{instance},{n},{},{},{}",
                    color[0], color[1], color[2]
                );
            }
            let _ = writeln!(
                poses,
                "{n},{time:.5},{k},{:.5},{:.5},{:.5},{:.6},{:.6},{:.6},{:.6}",
                pos.x, pos.y, pos.z, rot.x, rot.y, rot.z, rot.w
            );
        }
        if n % (rate as usize) == 0 {
            eprintln!("{time:.0} s: {} bodies", rover.bodies().len());
        }
    }
    let rover = runtime.meteor_rover().ok_or("The rover is gone")?;
    let mut blasts = String::from("time,x,y,z,radius,fx,fy,fz\n");
    for &(time, at, radius) in rover.blasts() {
        let f = rover.flight(at);
        let _ = writeln!(
            blasts,
            "{time:.5},{},{},{},{radius},{},{},{}",
            at.x, at.y, at.z, f.x, f.y, f.z
        );
    }
    write(&out.join("bodies.csv"), &bodies)?;
    write(&out.join("poses.csv"), &poses)?;
    write(&out.join("blasts.csv"), &blasts)?;
    let [rx, rz] = showcase::rover_at();
    let [cx, cz] = showcase::CASTER;
    let scene = serde_json::json!({
        "rate_hz": rate,
        "samples": samples + 1,
        "frame": "Verse: metres, Y up, right-handed",
        "rover": {"at": [rx, everglade::land(rx, rz), rz], "heading_rad": showcase::ROVER_HEADING},
        "caster": {"at": [cx, everglade::land(cx, cz), cz]},
        "cast_s": delay,
        "hardest_hit": rover.hardest().map(|(t, knock)| serde_json::json!({"time_s": t, "knock_m_s": knock})),
        "broke_off": rover.broken(),
    });
    write(
        &out.join("scene.json"),
        &serde_json::to_string_pretty(&scene).unwrap_or_default(),
    )?;
    eprintln!("wrote {} bodies to {}", seen.len(), out.display());
    Ok(())
}
