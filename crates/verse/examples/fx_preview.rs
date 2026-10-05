//! Look at a particle effect without playing the game
//! (`docs/verse/particles.md`).
//!
//! Usage:
//!   fx_preview EFFECT OUT_DIR [--frames N] [--seconds S] [--sequence]
//!   fx_preview --all OUT_DIR
//!
//! Installs Everglade's demolition yard from the committed, pinned pack and
//! runs the effect named EFFECT (a file stem under
//! `assets/verse/fx/effects/`) on the lawn in front of the west cottage, in
//! daylight, seen from the south-west. It writes `OUT_DIR/EFFECT.png`, a
//! contact sheet of N frames (8 by default), closer together early, over S seconds (by
//! default, until the effect's last particle could die, at most 6 s), each
//! frame's time in its corner, the effect's particle count printed for
//! each. With `--sequence` it also writes every frame as
//! `OUT_DIR/EFFECT/NN.png`.
//!
//! An effect that runs until stopped (a rate and no duration, such as a
//! trail) is moved along a meteor's falling path for the first 70% of the
//! time and then stopped, so trails and heads read as they do in flight.
//!
//! `--all` renders a four-frame strip of every effect into
//! `OUT_DIR/gallery.png`, one row each, in library order.
use std::path::{Path, PathBuf};

use glam::Vec3;
use verse::{
    controller::InputState,
    fx::{self, Particles, Spawn},
    mesh::Mesh,
    runtime::{Action, WorldRuntime},
    zones::{self, everglade::height, everglade_pack},
};

const DT: f32 = 1.0 / 60.0;
const WIDTH: u32 = 640;
const HEIGHT: u32 = 400;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let effect = args
        .next()
        .ok_or("Usage: fx_preview EFFECT|--all OUT_DIR [--frames N] [--seconds S] [--sequence]")?;
    let dir = PathBuf::from(args.next().ok_or("Expected an output directory")?);
    let (mut frames, mut seconds, mut sequence) = (8usize, None, false);
    while let Some(flag) = args.next() {
        let mut value = |name: &str| -> Result<f32, String> {
            let v = args.next().ok_or(format!("{name} needs a value"))?;
            v.parse()
                .map_err(|_| format!("{name} is a number, got {v}"))
        };
        match flag.as_str() {
            "--frames" => frames = value("--frames")?.clamp(1.0, 64.0) as usize,
            "--seconds" => seconds = Some(value("--seconds")?.clamp(0.05, 60.0)),
            "--sequence" => sequence = true,
            other => return Err(format!("Unknown flag {other}")),
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut stage = Stage::new()?;
    if effect == "--all" {
        let library = fx::Library::builtin();
        let mut rows = Vec::new();
        for e in &library.effects {
            eprintln!("{}: {}", e.name, e.description);
            rows.push(stage.strip(&e.name, 4, None, None)?);
        }
        let sheet = stack(&rows);
        let path = dir.join("gallery.png");
        write(&path, &sheet)?;
        println!("{}", path.display());
        return Ok(());
    }
    let sequence_dir = sequence.then(|| dir.join(&effect));
    if let Some(d) = &sequence_dir {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let strip = stage.strip(&effect, frames, seconds, sequence_dir.as_deref())?;
    let path = dir.join(format!("{effect}.png"));
    write(&path, &strip)?;
    println!("{}", path.display());
    Ok(())
}

/// The yard, the camera, and where effects play.
struct Stage {
    runtime: WorldRuntime,
    atlas: verse::ui::Atlas,
    at: Vec3,
}

/// An image: width, height, RGBA8 rows top first.
struct Image(u32, u32, Vec<u8>);

impl Stage {
    fn new() -> Result<Self, String> {
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
        runtime.set_demolition(true);
        runtime.install_everglade(&pack);
        if !runtime.in_demolition() {
            return Err("The demolition yard did not install from the pinned pack".into());
        }
        runtime.settle_zone_light();
        runtime.set_spawn(Vec3::new(-14.0, 0.0, -27.0), 0.5)?;
        runtime.apply(Action::Zoom { lines: 3.0 })?;
        for _ in 0..6 {
            runtime.tick(&InputState::default(), DT);
        }
        let (x, z) = (-8.5, -19.0);
        Ok(Self {
            runtime,
            atlas: verse::ui::Atlas::new(16.0),
            at: Vec3::new(x, height(x, z), z),
        })
    }

    /// `frames` frames of `name` over `seconds`, side by side in rows of
    /// four, each also written into `sequence` when given.
    fn strip(
        &mut self,
        name: &str,
        frames: usize,
        seconds: Option<f32>,
        sequence: Option<&Path>,
    ) -> Result<Image, String> {
        let effect = fx::system::effect(name).ok_or(format!("No effect named {name}"))?;
        let endless = effect
            .emitters
            .iter()
            .any(|e| e.rate > 0.0 && e.duration <= 0.0);
        let last = effect
            .emitters
            .iter()
            .map(|e| e.delay + e.duration + e.life[1])
            .fold(0.0, f32::max);
        let seconds = seconds.unwrap_or(if endless { 2.0 } else { last.min(6.0) });
        // A meteor's path: from high behind and left, down to the lawn.
        let from = self.at + Vec3::new(-5.0, 9.0, -4.0);
        let path = |t: f32| from + (self.at - from) * t.clamp(0.0, 1.0);
        let flight = 0.7 * seconds;
        let mut particles = Particles::new(0x5EED);
        let start = if endless {
            Spawn::at(path(0.0))
                .moving((self.at - from) / flight)
                .along(from - self.at)
        } else {
            Spawn::at(self.at + Vec3::Y * 0.4)
        };
        let handle = particles
            .start(name, start)
            .ok_or(format!("{name} did not start"))?;
        let times: Vec<f32> = (1..=frames)
            .map(|i| seconds * (i as f32 / frames as f32).powf(1.6))
            .collect();
        let mut clock = 0.0;
        let mut shots = Vec::new();
        for (n, &t) in times.iter().enumerate() {
            while clock + DT * 0.5 < t {
                clock += DT;
                if endless {
                    if clock < flight {
                        particles.place(handle, path(clock / flight), (self.at - from) / flight);
                    } else {
                        particles.stop(handle);
                    }
                }
                particles.tick(DT, height);
            }
            let mut dynamic: Mesh = self.runtime.dynamic_mesh();
            particles.draw(&mut dynamic.sprites);
            eprintln!(
                "{name} at {t:.2} s: {} particles, {} sprites",
                particles.len(),
                dynamic.sprites.len()
            );
            let pixels = verse::render::capture_rgba(
                WIDTH,
                HEIGHT,
                &self.runtime.world.mesh,
                self.runtime.view(WIDTH as f32 / HEIGHT as f32),
                &dynamic,
                &verse::ui::UiBatch::default(),
                &self.atlas,
                zones::atmosphere(self.runtime.zone),
            )?;
            let mut shot = Image(WIDTH, HEIGHT, pixels);
            stamp(&mut shot, t);
            if let Some(dir) = sequence {
                write(&dir.join(format!("{n:02}.png")), &shot)?;
            }
            shots.push(shot);
        }
        Ok(grid(&shots, 4))
    }
}

/// Marks a frame's time as a bar along its top edge: the bar's length is
/// the time over 6 s, so frames read in order at a glance.
fn stamp(image: &mut Image, t: f32) {
    let w = (image.0 as f32 * (t / 6.0).clamp(0.0, 1.0)) as u32;
    for y in 0..4 {
        for x in 0..w {
            let i = ((y * image.0 + x) * 4) as usize;
            image.2[i..i + 4].copy_from_slice(&[255, 200, 60, 255]);
        }
    }
}

/// `images` in rows of `columns`, with a 4 px dark gutter.
fn grid(images: &[Image], columns: usize) -> Image {
    let (w, h) = (images[0].0, images[0].1);
    let columns = columns.min(images.len());
    let rows = images.len().div_ceil(columns);
    let gap = 4;
    let (sw, sh) = (
        columns as u32 * (w + gap) + gap,
        rows as u32 * (h + gap) + gap,
    );
    let mut out = vec![0u8; (sw * sh * 4) as usize];
    for px in out.chunks_exact_mut(4) {
        px.copy_from_slice(&[18, 20, 24, 255]);
    }
    for (k, image) in images.iter().enumerate() {
        let (cx, cy) = ((k % columns) as u32, (k / columns) as u32);
        let (ox, oy) = (gap + cx * (w + gap), gap + cy * (h + gap));
        for y in 0..h {
            let src = &image.2[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
            let at = (((oy + y) * sw + ox) * 4) as usize;
            out[at..at + src.len()].copy_from_slice(src);
        }
    }
    Image(sw, sh, out)
}

/// `rows` one above another.
fn stack(rows: &[Image]) -> Image {
    let width = rows.iter().map(|r| r.0).max().unwrap_or(1);
    let height = rows.iter().map(|r| r.1).sum();
    let mut out = vec![0u8; (width * height * 4) as usize];
    let mut top = 0;
    for row in rows {
        for y in 0..row.1 {
            let src = &row.2[(y * row.0 * 4) as usize..((y + 1) * row.0 * 4) as usize];
            let at = (((top + y) * width) * 4) as usize;
            out[at..at + src.len()].copy_from_slice(src);
        }
        top += row.1;
    }
    Image(width, height, out)
}

fn write(path: &Path, image: &Image) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("cannot create {}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.0, image.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&image.2))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}
