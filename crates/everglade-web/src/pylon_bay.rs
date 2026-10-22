//! The Pylon Bay in the Grid's depth-tested world pass (#12423).
//!
//! The Bay's floor and sign are a static placement. Each pylon is a
//! machine model chosen by its shape and load, moved to its spot each
//! frame; a working machine sends a spark up its beam, a job's receipt
//! sends a ring across the floor, a group's pipe carries a packet while
//! the group answers, and brackets mark the machine under the pointer or
//! with its card open.
//!
//! The pylons layer (`crates/pylons-web`) owns the live list and the
//! places; it hands them over as `globalThis.pylonsBay`, a `Float64Array`
//! in [`pylons_view::bay::encode`]'s layout. Every model is built here at
//! start from [`pylons_view::bay::art`], so the Bay adds no download.

use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use pylons_view::bay::{self, Load, Shape, art::Art};
use verse::{imported::flat, palette};
use verse_engine::{
    assets::{Model, Pack, Placement, Surface, Topology, Vertex},
    inventory::{Asset, Binding, fingerprint},
    presentation::Instance,
};

/// The global the pylons layer writes the Bay to.
const GLOBAL: &str = "pylonsBay";
/// The floor, the section names, the lanes, and the sign.
const FLOOR: &str = "bay/floor";
const SPARK: &str = "bay/spark";
const PACKET: &str = "bay/packet";
const FOCUS: &str = "bay/focus";
/// A pulse's rings, brightest first: it fades as it grows.
const RINGS: [&str; 3] = ["bay/ring-3", "bay/ring-2", "bay/ring-1"];
/// How long a pulse lasts, ms.
const PULSE_MS: f64 = 1400.0;
/// How long a spark takes to climb its beam, ms.
const SPARK_MS: f64 = 1600.0;
/// How long a packet takes from one group member to the next, ms.
const HOP_MS: f64 = 650.0;
/// The packet's height over the pipe, m.
const PACKET_LIFT: f32 = 0.4;
/// The dark faces' tint: the terminal's near-black.
const DARK: [f32; 3] = [0.002; 3];

fn machine_key(shape: Shape, load: Load) -> String {
    format!("bay/m{}-{}", shape.index(), load.index())
}

fn pipe_key(parts: u32, lit: bool) -> String {
    format!("bay/pipe{parts}-{}", u8::from(lit))
}

pub(crate) fn tint(tone: u8) -> [f32; 3] {
    let step = Intensity::ALL[usize::from(tone.min(3))];
    palette::neutral(palette::amber(step))
}

fn point(p: [f64; 3]) -> Vec3 {
    Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
}

/// A flat surface that shares each corner among the lines and faces that
/// meet there. The Grid's start digests every vertex of every model
/// (#12423), so fewer vertices start the page sooner.
struct Shared {
    surface: Surface,
}

impl Shared {
    fn new(white: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: flat::surface(white, tint, topology),
        }
    }

    fn at(&mut self, p: [f64; 3]) -> u32 {
        let position = point(p).to_array();
        let vertices = &mut self.surface.vertices;
        // A model has a few hundred corners at most: a scan is enough.
        if let Some(found) = vertices.iter().position(|v| v.position == position) {
            return found as u32;
        }
        vertices.push(Vertex {
            position,
            normal: [0., 1., 0.],
            uv: [0.5, 0.5],
            joints: [0; 4],
            weights: [1., 0., 0., 0.],
        });
        (vertices.len() - 1) as u32
    }

    fn line(&mut self, a: [f64; 3], b: [f64; 3]) {
        let (a, b) = (self.at(a), self.at(b));
        self.surface.indices.extend([a, b]);
    }

    fn quad(&mut self, corners: [[f64; 3]; 4]) {
        let [a, b, c, d] = corners.map(|p| self.at(p));
        self.surface.indices.extend([a, b, c, a, c, d]);
    }

    fn done(self, out: &mut Vec<Surface>) {
        if !self.surface.indices.is_empty() {
            out.push(self.surface);
        }
    }
}

/// A model of `art`: the dark faces, a lines surface for each tone, and a
/// lit surface for each tone of the letters.
fn model(source: &str, art: &Art, white: usize, height: f32) -> Model {
    let mut surfaces = Vec::new();
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for corners in &art.faces {
        faces.quad(*corners);
    }
    faces.done(&mut surfaces);
    for tone in 0..4 {
        let mut lines = Shared::new(white, tint(tone), Topology::Lines);
        for (a, b, _) in art.lines.iter().filter(|l| l.2 == tone) {
            lines.line(*a, *b);
        }
        lines.done(&mut surfaces);
        let mut lit = Shared::new(white, tint(tone), Topology::Triangles);
        for (corners, _) in art.lit.iter().filter(|q| q.1 == tone) {
            lit.quad(*corners);
        }
        lit.done(&mut surfaces);
    }
    flat::model(source, surfaces, height)
}

/// Adds the Bay's models to the Grid's `pack`, whose white texel is slot
/// `white` and listed as `texture`, and stands its floor.
pub fn add(pack: &mut Pack, white: usize, texture: &str) -> Result<(), String> {
    let mut models: Vec<(String, Model)> = Vec::new();
    for shape in Shape::ALL {
        for load in Load::ALL {
            let key = machine_key(shape, load);
            let height = (shape.top() + load.beam()) as f32;
            models.push((
                key.clone(),
                model(&key, &bay::art::machine(shape, load), white, height),
            ));
        }
    }
    for parts in 1..=bay::MOST_PARTS {
        for lit in [false, true] {
            let key = pipe_key(parts, lit);
            models.push((
                key.clone(),
                model(&key, &bay::art::pipe(parts, lit), white, 0.1),
            ));
        }
    }
    for (tone, key) in [3, 2, 1].into_iter().zip(RINGS) {
        models.push((key.into(), model(key, &bay::art::ring(tone), white, 0.1)));
    }
    models.push((SPARK.into(), model(SPARK, &bay::art::spark(), white, 0.1)));
    models.push((
        PACKET.into(),
        model(PACKET, &bay::art::packet(), white, 0.3),
    ));
    models.push((FOCUS.into(), model(FOCUS, &bay::art::focus(), white, 0.1)));
    models.push((FLOOR.into(), model(FLOOR, &bay::art::floor(), white, 2.5)));
    let inventory = pack
        .inventory
        .as_mut()
        .ok_or("The Grid's inventory is missing")?;
    let texture = inventory
        .assets
        .iter()
        .find(|a| a.id.as_str() == texture)
        .ok_or("The Grid's white texture is not listed")?
        .id
        .clone();
    for (key, model) in models {
        let (sha256, bytes) = fingerprint(&model)?;
        inventory.assets.push(Asset {
            id: verse::imported::inventory::id(&format!("verse:model:{key}"))?,
            binding: Binding::Model { key: key.clone() },
            sha256,
            bytes,
            dependencies: vec![texture.clone()],
        });
        pack.models.insert(key, model);
    }
    pack.placements.push(Placement {
        model: FLOOR.into(),
        position: [0.; 3],
        rotation: [0., 0., 0., 1.],
        scale: 1.,
    });
    Ok(())
}

fn fixed(model: &str, transform: Mat4) -> Instance {
    verse::grid_frame::fixed(model, transform)
}

/// The Bay as the pylons layer last handed it over, if it has.
fn read() -> Option<(Vec<bay::Drawn>, Vec<bay::DrawnPipe>)> {
    let value = js_sys::Reflect::get(&js_sys::global(), &GLOBAL.into()).ok()?;
    let numbers = wasm_bindgen::JsCast::dyn_ref::<js_sys::Float64Array>(&value)?.to_vec();
    bay::decode(&numbers)
}

/// How many Pylons the Bay shows, once the pylons layer has handed it over.
pub fn online() -> Option<usize> {
    read().map(|(stands, _)| stands.len())
}

/// A steady phase in 0..1 for a machine at (`x`, `z`), so neighbors don't
/// pulse in step.
fn phase(x: f64, z: f64) -> f64 {
    ((x * 12.9898 + z * 78.233).sin() * 43_758.545)
        .fract()
        .abs()
}

/// This frame's Bay instances, at page time `now` (ms).
pub fn instances(now: f64, out: &mut Vec<Instance>) {
    let Some((stands, pipes)) = read() else {
        return;
    };
    for stand in &stands {
        let foot = Vec3::new(stand.x as f32, 0.0, stand.z as f32);
        out.push(fixed(
            &machine_key(stand.shape, stand.load),
            Mat4::from_translation(foot),
        ));
        if stand.load.busy() {
            let t = (now / SPARK_MS + phase(stand.x, stand.z)).fract();
            let from = stand.shape.top() + 0.25;
            let y = from + t * (stand.shape.top() + stand.load.beam() - from);
            out.push(fixed(
                SPARK,
                Mat4::from_translation(foot + Vec3::Y * y as f32),
            ));
        }
        let age = now - stand.pulse_at;
        if stand.pulse_at >= 0.0 && (0.0..PULSE_MS).contains(&age) {
            let t = age / PULSE_MS;
            let ring = RINGS[((t * 3.0) as usize).min(2)];
            let scale = (0.9 + t * 1.9) as f32;
            out.push(fixed(
                ring,
                Mat4::from_translation(foot) * Mat4::from_scale(Vec3::new(scale, 1.0, scale)),
            ));
        }
        if stand.focused {
            out.push(fixed(FOCUS, Mat4::from_translation(foot)));
        }
    }
    for pipe in &pipes {
        if pipe.parts == 0 {
            continue;
        }
        let head = Vec3::new(pipe.x as f32, 0.0, pipe.z as f32);
        out.push(fixed(
            &pipe_key(pipe.parts, pipe.lit),
            Mat4::from_translation(head),
        ));
        if pipe.lit {
            // Head, part 1, ... part n, and round again.
            let hops = f64::from(pipe.parts);
            let t = (now / (HOP_MS * hops)).fract();
            let last = bay::member_at(Some(pipe.parts), pipe.parts) - bay::HEAD_AT;
            let along = (t * last) as f32;
            out.push(fixed(
                PACKET,
                Mat4::from_translation(head + Vec3::new(0.0, PACKET_LIFT, along)),
            ));
        }
    }
}
