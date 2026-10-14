//! Robot Lab inspection capture (`docs/lab/README.md`, slice 1).
//! Usage: robot_lab_capture OUTPUT.png [--urdf FILE.urdf] [JOINT=RADIANS ...]
//!
//! Loads the pinned SO-101 URDF by default, sets the named joints
//! (the rest stay at home), refuses values outside the joints' limits,
//! stands the robot on a 0.75 m table beside a 5 cm red cube, and renders a
//! 1280×800 frame with the shared renderer. It prints each link's position.
use std::path::{Path, PathBuf};

use glam::{Mat4, Vec3};
use verse::mesh::Mesh;
use verse::pbr::{Key, Neon};
use verse::render::View;
use verse::robot_lab::{self, TABLE_HEIGHT};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(
        args.next()
            .ok_or("Usage: robot_lab_capture OUTPUT.png [--urdf FILE.urdf] [JOINT=RADIANS ...]")?,
    );
    let mut urdf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../robot-description/assets/so101/so101_new_calib.urdf");
    let mut joints = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--urdf" {
            urdf = PathBuf::from(args.next().ok_or("--urdf needs a file")?);
            continue;
        }
        let (name, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("{arg} is not JOINT=RADIANS"))?;
        let value: f64 = value
            .parse()
            .map_err(|_| format!("{value} is not a number of radians"))?;
        joints.push((name.to_string(), value));
    }
    let robot = robot_description::load(&urdf).map_err(|e| e.to_string())?;
    let named: Vec<(&str, f64)> = joints.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    let pose = robot.named_pose(&named).map_err(|e| e.to_string())?;
    let scene = robot_lab::scene(&robot, &pose).map_err(|e| e.to_string())?;
    eprintln!("{} at {}", robot.name(), urdf.display());
    for (joint, value) in robot.movable().zip(&pose) {
        let limit = joint.limit.map_or(String::new(), |l| {
            format!(" in [{:.3}, {:.3}]", l.lower, l.upper)
        });
        eprintln!("  joint {} = {value:.3}{limit}", joint.name);
    }
    for (link, at) in robot.links().iter().zip(&scene.links) {
        eprintln!("  link {} at {:.3?} m", link.name, at.to_array());
    }

    let mut lit = scene.props;
    lit.extend(scene.robot);
    let world = Mesh {
        lit,
        ..Mesh::default()
    };
    let center = Vec3::new(0.1, TABLE_HEIGHT + 0.15, 0.0);
    let mut neon = Neon::neutral(0.0);
    neon.key = Some(Key {
        dir: Vec3::new(0.45, 0.8, 0.4).normalize(),
        illuminance: 4_200.0,
        angular_radius: 0.035,
        rim_dir: Vec3::new(-0.5, 0.4, -0.75).normalize(),
        rim_illuminance: 1_800.0,
        rim_angular_radius: 0.12,
        sky: 420.0,
        ground: 60.0,
        ev100: 10.0,
        shadow_center: center,
        shadow_half: 1.5,
        shadow_distance: None,
        cache_far_shadows: false,
    });
    let dynamic = Mesh {
        neon: Some(neon),
        ..Mesh::default()
    };
    let (width, height) = (1280, 800);
    let eye = center + Vec3::new(0.55, 0.35, 1.05);
    let view = View {
        view_proj: Mat4::perspective_rh(0.75, width as f32 / height as f32, 0.05, 50.0)
            * Mat4::look_at_rh(eye, center, Vec3::Y),
        eye,
    };
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    verse::render::capture_with_atmosphere(
        &output,
        width,
        height,
        &world,
        view,
        &dynamic,
        &verse::ui::UiBatch::default(),
        &verse::ui::Atlas::new(16.0),
        verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
    )?;
    eprintln!("wrote {}", output.display());
    Ok(())
}
