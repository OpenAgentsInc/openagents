//! The Lagrange 2 demo film and stills: the four arms assembling the sample
//! ship at Earth–Moon L2, then its test burn departing L2
//! (`docs/verse/lagrange-2.md`, issue #11625).
//!
//! Usage:
//!   lagrange2_film film OUT.mp4 [--size 1920x1080] [--stills DIR]
//!   lagrange2_film still OUT.png SIM_SECONDS VIEW [FILM_SECONDS] [--size WxH]
//!
//! The film enters the factory from the Grid's walk-in arch, then drives
//! the zone's own clocks: about 90× while the arms work (the stage-checked
//! assembly takes about 67 factory minutes), slower for the burn. Views for
//! stills: wide, close, aft (the Moon behind the ship), burn, display.
//! The HUD caption is the zone's own.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use glam::{Mat4, Vec3};
use verse::{controller::InputState, runtime::WorldRuntime, zones};

/// One camera: eye, target, vertical field of view.
#[derive(Clone, Copy)]
struct Shot {
    eye: Vec3,
    target: Vec3,
    fov: f32,
}

fn shot(view: &str, t: f32, ship_x: f32) -> Shot {
    shot_with_sky(view, t, ship_x, Vec3::NEG_X)
}

/// A camera that may frame the Moon, whose direction is `moon`.
fn shot_with_sky(view: &str, t: f32, ship_x: f32, moon: Vec3) -> Shot {
    match view {
        // The Moon through a long lens from the deck.
        "moon" => Shot {
            eye: Vec3::new(0.0, 30.0, 0.0),
            target: Vec3::new(0.0, 30.0, 0.0) + moon,
            fov: 0.12,
        },
        // Over the ship's nose toward the Moon: the ship in the cradle with
        // the Moon and the Earth beyond it.
        "aft" => {
            let eye = Vec3::new(30.0, 16.0, 0.0) - moon * 26.0 + Vec3::Y * 10.0;
            Shot {
                eye,
                target: Vec3::new(0.0, 8.0, 0.0) + moon * 60.0,
                fov: 0.9,
            }
        }
        // A slow circle around the deck, high enough to see every arm,
        // turning away from the Sun (with the Sun just outside the frame
        // the renderer's glare draws as a clipped box in the corner).
        "wide" => {
            let a = 2.2 - t * 0.012;
            Shot {
                eye: Vec3::new(a.cos() * 92.0, 44.0, a.sin() * 92.0),
                target: Vec3::new(0.0, 6.0, 0.0),
                fov: 0.75,
            }
        }
        "close" => Shot {
            eye: Vec3::new(-2.0, 22.0, 42.0),
            target: Vec3::new(-4.0, 8.0, 0.0),
            fov: 0.8,
        },
        // From the deck's forward edge, following the ship out.
        "burn" => Shot {
            eye: Vec3::new(40.0, 20.0, 34.0),
            target: Vec3::new(ship_x + 20.0, 8.0, 0.0),
            fov: 0.75,
        },
        // The orbit display above the deck.
        "display" => Shot {
            eye: Vec3::new(6.0 - t * 1.2, 40.0 + t * 0.3, 4.0 - t * 0.2),
            target: Vec3::new(-6.0, 34.0, -30.0),
            fov: 0.9,
        },
        _ => Shot {
            eye: Vec3::new(-60.0, 30.0, 70.0),
            target: Vec3::ZERO,
            fov: 0.8,
        },
    }
}

fn mix(a: Shot, b: Shot, k: f32) -> Shot {
    let k = k.clamp(0.0, 1.0);
    let k = k * k * (3.0 - 2.0 * k);
    Shot {
        eye: a.eye.lerp(b.eye, k),
        target: a.target.lerp(b.target, k),
        fov: a.fov + (b.fov - a.fov) * k,
    }
}

struct Scene {
    runtime: Box<WorldRuntime>,
    renderer: verse::render::Offscreen,
    atlas: verse::ui::Atlas,
    size: (u32, u32),
}

impl Scene {
    fn new(size: (u32, u32)) -> Result<Self, String> {
        // The Grid, and its walk-in arch to Lagrange 2.
        let mut runtime = Box::new(WorldRuntime::bare());
        let gate = runtime
            .lagrange2_gate()
            .ok_or("The Grid has no Lagrange 2 arch")?;
        let (front, yaw) = gate.front();
        runtime.set_spawn(front, yaw + std::f32::consts::PI)?;
        let walk = InputState {
            forward: true,
            ..Default::default()
        };
        for _ in 0..200 {
            runtime.tick(&walk, 1.0 / 30.0);
            if runtime.zone == zones::ZoneId::Lagrange2 {
                break;
            }
        }
        if runtime.zone != zones::ZoneId::Lagrange2 {
            return Err("Walking through the arch did not enter Lagrange 2".into());
        }
        eprintln!("walked in from the Grid at {}", gate.at);
        let idle = InputState::default();
        runtime.tick(&idle, 1.0 / 30.0);
        runtime.settle_zone_light();
        let atlas = verse::ui::Atlas::new((size.1 as f32 / 45.0).round());
        let renderer = verse::render::Offscreen::new(
            size.0,
            size.1,
            &runtime.world.mesh,
            &atlas,
            runtime.atmosphere(),
        )?;
        Ok(Self {
            runtime,
            renderer,
            atlas,
            size,
        })
    }

    fn factory(&mut self) -> &mut zones::lagrange2::Lagrange2 {
        self.runtime.lagrange2_mut().expect("in Lagrange 2")
    }

    /// Advance `real` seconds of film at `speed` factory seconds per second.
    fn advance(&mut self, real: f32, speed: f64) {
        self.factory().speed = speed;
        let idle = InputState::default();
        self.runtime.tick(&idle, real);
    }

    fn render(&mut self, s: Shot, hud: bool) -> Result<Vec<u8>, String> {
        let (w, h) = self.size;
        let aspect = w as f32 / h as f32;
        let view = verse::render::View {
            view_proj: Mat4::perspective_rh(s.fov, aspect, 0.5, 2000.0)
                * Mat4::look_at_rh(s.eye, s.target, Vec3::Y),
            eye: s.eye,
        };
        let dynamic = self.runtime.dynamic_mesh();
        let mut ui = verse::ui::UiBatch::default();
        if hud {
            // The zone's own caption, every line, in a quiet box top left.
            let caption = self.factory().caption();
            let lines: Vec<&str> = caption.lines().collect();
            let line_h = self.atlas.advance * 2.0;
            let width = lines
                .iter()
                .map(|l| l.chars().count() as f32 * self.atlas.advance)
                .fold(0.0, f32::max);
            let (x, y) = (28.0, 28.0);
            ui.rect(
                &self.atlas,
                x - 14.0,
                y - 12.0,
                width + 28.0,
                line_h * lines.len() as f32 + 18.0,
                [0.0, 0.0, 0.0, 0.55],
            );
            for (row, line) in lines.iter().enumerate() {
                let color = if row == lines.len() - 1 {
                    [1.0, 0.86, 0.55, 1.0]
                } else {
                    [0.86, 0.88, 0.9, 1.0]
                };
                ui.text(&self.atlas, x, y + row as f32 * line_h, line, color);
            }
        }
        self.renderer.render(view, &dynamic, &ui)
    }

    /// The Moon's direction in the scene.
    fn moon(&self) -> Vec3 {
        self.runtime
            .dynamic_mesh()
            .sky
            .map_or(Vec3::NEG_X, |sky| sky.moon.dir)
    }

    fn ship_x(&mut self) -> f32 {
        self.factory()
            .test
            .as_ref()
            .map_or(0.0, |t| t.burn.pos.x as f32)
    }
}

fn write_png(path: &PathBuf, (w, h): (u32, u32), pixels: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(file, w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut w| w.write_image_data(pixels))
        .map_err(|e| e.to_string())
}

fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let (w, h) = text.split_once('x').ok_or("--size takes WxH")?;
    Ok((
        w.parse().map_err(|_| "bad width")?,
        h.parse().map_err(|_| "bad height")?,
    ))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut size = (1920, 1080);
    let mut stills = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--size" => {
                size = parse_size(args.get(i + 1).ok_or("--size takes WxH")?)?;
                i += 1;
            }
            "--stills" => {
                stills = Some(PathBuf::from(
                    args.get(i + 1).ok_or("--stills takes a folder")?,
                ));
                i += 1;
            }
            other => rest.push(other.to_owned()),
        }
        i += 1;
    }
    verse::fx::set_budget(Some(16_384));
    match rest.first().map(String::as_str) {
        Some("still") => {
            let out = PathBuf::from(rest.get(1).ok_or("still OUT.png SECONDS VIEW")?);
            let seconds: f64 = rest
                .get(2)
                .ok_or("still OUT.png SECONDS VIEW")?
                .parse()
                .map_err(|_| "SECONDS is a number")?;
            let view = rest.get(3).map_or("wide", String::as_str);
            let mut scene = Scene::new(size)?;
            scene.factory().art = true;
            // Reach the factory time asked for in large steps.
            while scene.factory().assembly.factory.time() + scene.test_time() < seconds {
                scene.advance(1.0 / 30.0, 200.0);
            }
            scene.runtime.settle_zone_light();
            eprintln!("{}", scene.factory().caption());
            let ship_x = scene.ship_x();
            let moon = scene.moon();
            eprintln!("moon direction {moon}");
            let t: f32 = rest.get(4).and_then(|t| t.parse().ok()).unwrap_or(0.0);
            let pixels = scene.render(shot_with_sky(view, t, ship_x, moon), true)?;
            write_png(&out, size, &pixels)
        }
        Some("film") => {
            let out = PathBuf::from(rest.get(1).ok_or("film OUT.mp4")?);
            film(&out, size, stills)
        }
        _ => Err("Usage: lagrange2_film film OUT.mp4 | still OUT.png SECONDS VIEW".into()),
    }
}

impl Scene {
    /// Seconds the test burn has run, so stills can ask for burn times.
    fn test_time(&mut self) -> f64 {
        self.factory().test.as_ref().map_or(0.0, |t| t.burn.time)
    }
}

fn film(out: &PathBuf, size: (u32, u32), stills: Option<PathBuf>) -> Result<(), String> {
    const FPS: f32 = 30.0;
    if let Some(dir) = &stills {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let mut scene = Scene::new(size)?;
    scene.factory().art = true;
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
        ])
        .arg(format!("{}x{}", size.0, size.1))
        .args([
            "-r", "30", "-i", "-", "-an", "-c:v", "libx264", "-preset", "slow",
        ])
        .args([
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "16",
            "-movflags",
            "+faststart",
        ])
        .arg(out)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot start ffmpeg: {e}"))?;
    let mut stdin = ffmpeg.stdin.take().ok_or("ffmpeg has no input")?;
    let started = std::time::Instant::now();
    let mut frame = 0usize;
    let mut t = 0.0f32;
    // Film time when the ship was complete, the burn began, and the
    // engine stopped.
    let mut complete_at: Option<f32> = None;
    let mut burn_at: Option<f32> = None;
    let mut stop_at: Option<f32> = None;
    let mut last = shot("wide", 0.0, 0.0);
    loop {
        let (complete, ship) = {
            let f = scene.factory();
            let ship = f.test.as_ref().map(|b| {
                (
                    (verse_lagrange2_cradle() + b.burn.pos).as_vec3(),
                    b.burn.firing,
                    !b.path.is_empty(),
                )
            });
            (
                f.assembly.status == zones::lagrange2::verse_lagrange2::Status::Complete,
                ship,
            )
        };
        if complete && complete_at.is_none() {
            complete_at = Some(t);
        }
        if ship.is_some() && burn_at.is_none() {
            burn_at = Some(t);
        }
        if let Some((_, firing, _)) = ship
            && !firing
            && stop_at.is_none()
        {
            stop_at = Some(t);
        }
        let idle = {
            let f = scene.factory();
            !matches!(
                f.assembly.status,
                zones::lagrange2::verse_lagrange2::Status::Acting
            )
        };
        let speed = match (ship, complete) {
            // Between stages the arms wait while the next one is simulated;
            // the film hurries through that.
            (None, false) if idle => 240.0,
            (None, false) => 90.0,
            (None, true) => 20.0,
            (Some((_, true, _)), _) => 8.0,
            (Some(_), _) => 4.0,
        };
        let moon = scene.moon();
        let s = match (ship, burn_at) {
            (Some((at, _, _)), Some(b)) => {
                // A chase camera beside the ship that falls back as it
                // leaves, then the orbit display once the engine stops.
                let k = ((t - b) / 12.0).clamp(0.0, 1.0);
                // The chase circles the ship as it falls back, so the frame
                // keeps moving while the ship holds its place in it.
                let swing = (t - b) * 0.09;
                let reach = 55.0 + 60.0 * k;
                let chase = Shot {
                    eye: at
                        + Vec3::new(
                            -reach * (0.55 - swing).sin(),
                            14.0 + 10.0 * k,
                            reach * (0.55 - swing).cos(),
                        ),
                    target: at + Vec3::new(14.0, 0.0, 0.0),
                    fov: 0.7,
                };
                let from = mix(last, chase, (t - b) / 1.5);
                // A cut to the orbit display once the engine stops.
                match stop_at {
                    Some(stop) => shot("display", t - stop, 0.0),
                    None => from,
                }
            }
            _ => {
                let wide = shot("wide", t, 0.0);
                let close = shot("close", t, 0.0);
                let aft = shot_with_sky("aft", t, 0.0, moon);
                // The Moon sits low over the deck's aft edge early in the
                // assembly, then the halo carries it below the deck plane.
                // Cuts, not moves, between them: a moving camera would pass
                // through the arms.
                let s = if t < 14.0 {
                    aft
                } else if t < 32.0 {
                    wide
                } else {
                    close
                };
                last = s;
                s
            }
        };
        let pixels = scene.render(s, true)?;
        stdin
            .write_all(&pixels)
            .map_err(|e| format!("ffmpeg: {e}"))?;
        if let Some(dir) = &stills
            && frame % 45 == 0
        {
            write_png(&dir.join(format!("frame-{frame:05}.png")), size, &pixels)?;
        }
        frame += 1;
        t += 1.0 / FPS;
        if frame % 150 == 0 {
            eprintln!(
                "{:.0} s film, {:.0} s factory, {:.0} s wall: {}",
                t,
                scene.factory().assembly.factory.time(),
                started.elapsed().as_secs_f32(),
                scene.factory().caption().lines().last().unwrap_or("")
            );
        }
        // End seven seconds after the engine stops, on the departure path.
        if stop_at.is_some_and(|stop| t > stop + 7.0) || t > 150.0 {
            break;
        }
        scene.advance(1.0 / FPS, speed);
    }
    drop(stdin);
    let status = ffmpeg.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("ffmpeg failed: {status}"));
    }
    eprintln!("wrote {} ({frame} frames)", out.display());
    Ok(())
}

fn verse_lagrange2_cradle() -> glam::DVec3 {
    zones::lagrange2::verse_lagrange2::factory::CRADLE
}
