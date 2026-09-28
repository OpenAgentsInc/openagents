//! Offline visual acceptance of the Lagrange 1 station with the shared renderer.
//! Usage: lagrange_capture OUTPUT.png [VIEW]
//!
//! Player views: spawn, jig, carry, sun, earth. Fixed cameras: sunside (the
//! station from the sunward side with the Earth behind), wide (a three-quarter
//! view of the whole station), and telephoto views earthzoom, moonzoom, and
//! sunzoom, which frame each body at a narrow field of view. `stars` uses
//! the art preset and looks away from the Sun toward the galactic center.
//! `look EX,EY,EZ TX,TY,TZ [FOV]` places a camera at the eye, aimed at the
//! target, with a vertical field of view in radians (default 1).
use std::path::PathBuf;
use verse::{
    controller::InputState,
    runtime::{Action, WorldRuntime},
    zones,
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = PathBuf::from(args.next().ok_or("Expected an output PNG path")?);
    let view = args.next().unwrap_or_else(|| "spawn".into());
    let point = |text: Option<String>| -> Result<glam::Vec3, String> {
        let text = text.ok_or("look takes EX,EY,EZ TX,TY,TZ [FOV]")?;
        let v: Vec<f32> = text
            .split(',')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{text}: {e}"))?;
        match v.as_slice() {
            [x, y, z] => Ok(glam::Vec3::new(*x, *y, *z)),
            _ => Err(format!("{text}: expected X,Y,Z")),
        }
    };
    let look = if view == "look" {
        let eye = point(args.next())?;
        let target = point(args.next())?;
        let fov = args
            .next()
            .map_or(Ok(1.0), |f| f.parse::<f32>().map_err(|e| e.to_string()))?;
        Some((eye, target, fov))
    } else {
        None
    };
    let mut runtime = WorldRuntime::new();
    runtime.set_spawn(glam::Vec3::new(12.0, 0.0, 9.0), 0.0)?;
    runtime.zone_intent(zones::Intent::Enter)?;
    let idle = InputState::default();
    match view.as_str() {
        "jig" | "carry" => {
            let forward = InputState {
                forward: true,
                ..Default::default()
            };
            runtime.apply(Action::Orbit { dx: 0.0, dy: 120.0 })?;
            for _ in 0..(14 * 20) {
                runtime.tick(&forward, 0.05);
            }
            if view == "carry" {
                runtime
                    .navigate_to([-9.5, -4.0])
                    .map_err(|e| e.to_string())?;
                for _ in 0..(60 * 20) {
                    runtime.tick(&idle, 0.05);
                }
                runtime.player.yaw = -std::f32::consts::FRAC_PI_2;
                runtime.tick(&idle, 0.05);
                let _ = runtime.zone_intent(zones::Intent::Grab);
            }
            runtime.apply(Action::Orbit {
                dx: 250.0,
                dy: -60.0,
            })?;
            runtime.apply(Action::Zoom { lines: -6.0 })?;
        }
        "sun" | "earth" => {
            runtime
                .navigate_to([45.0, 30.0])
                .map_err(|e| e.to_string())?;
            for _ in 0..(80 * 20) {
                runtime.tick(&idle, 0.05);
            }
            runtime.player.yaw = if view == "sun" {
                std::f32::consts::PI
            } else {
                0.0
            };
            runtime.apply(Action::Look { dx: 0.0, dy: -40.0 })?;
            runtime.apply(Action::Zoom { lines: 8.0 })?;
        }
        "spawn" => runtime.apply(Action::Orbit { dx: 90.0, dy: 0.0 })?,
        "stars" => runtime.zone_intent(zones::Intent::Camera)?,
        _ => {}
    }
    for _ in 0..4 {
        runtime.tick(&idle, 0.05);
    }
    runtime.settle_zone_light();
    let atlas = verse::ui::Atlas::new(16.0);
    let mut hud = zones::hud::Hud::default();
    hud.set_bottom_clearance(0.0)?;
    let snapshot = runtime.zone_snapshot(1.6);
    eprintln!("{}", snapshot.caption);
    let ui = hud.draw(&atlas, &hud.snapshot([1280.0, 800.0], &snapshot, true), 1.0);
    let mut dynamic = runtime.dynamic_mesh();
    if view == "sunzoom"
        && let Some(sky) = &mut dynamic.sky
    {
        // A solar filter: about ND 5 over a sunlit exposure.
        sky.camera.auto_exposure = false;
        sky.camera.ev100 = 31.0;
        sky.camera.bloom = 0.0;
        sky.camera.ghosts = 0.0;
    }
    let player = runtime.view(1.6);
    let fixed = |eye: glam::Vec3, dir: glam::Vec3, fov: f32| verse::render::View {
        view_proj: glam::Mat4::perspective_rh(fov, 1.6, 0.1, 2000.0)
            * glam::Mat4::look_to_rh(eye, dir.normalize(), glam::Vec3::Y),
        eye,
    };
    let sky = dynamic.sky.as_ref();
    let view = match (view.as_str(), sky) {
        ("look", _) => {
            let (eye, target, fov) = look.ok_or("look needs an eye and a target")?;
            fixed(eye, target - eye, fov)
        }
        ("sunside", _) => {
            let eye = glam::Vec3::new(-14.0, 14.0, -34.0);
            fixed(eye, glam::Vec3::new(2.0, 0.0, 6.0) - eye, 1.0)
        }
        ("wide", _) => {
            let eye = glam::Vec3::new(46.0, 26.0, -22.0);
            fixed(eye, glam::Vec3::new(0.0, 2.0, 4.0) - eye, 1.0)
        }
        ("earthzoom", Some(sky)) => fixed(player.eye, sky.earth.dir, 0.03),
        ("moonzoom", Some(sky)) => fixed(player.eye, sky.moon.dir, 0.012),
        ("sunzoom", Some(sky)) => fixed(player.eye, sky.sun_dir, 0.03),
        ("stars", Some(sky)) => {
            // The galactic center: right ascension 17h45.6m, declination −28.94°.
            let (ra, dec) = (266.4_f32.to_radians(), (-28.94_f32).to_radians());
            let equatorial = glam::Vec3::new(dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin());
            fixed(player.eye, sky.celestial * equatorial, 1.0)
        }
        _ => player,
    };
    verse::render::capture_with_atmosphere(
        &output,
        1280,
        800,
        &runtime.world.mesh,
        view,
        &dynamic,
        &ui,
        &atlas,
        zones::atmosphere(runtime.zone),
    )
}
