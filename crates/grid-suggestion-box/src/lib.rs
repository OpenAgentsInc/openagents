//! The Grid's suggestion box (#12460): a mailbox beside the landing plaza.
//! A visitor who walks up to it is offered "Press E to leave a suggestion",
//! which opens the box in the page's GPUI chat. A suggestion travels as a
//! NIP-17 message sealed to OpenAgents (`playtest::suggestion`).
//!
//! This crate is the box's plain data, so native tests cover it: where it
//! stands ([`POSITION`], [`yaw`]), when a visitor is in reach
//! ([`in_reach`]), its words, and its model ([`add`]). The browser Grid
//! (`crates/everglade-web/src/suggestion_box.rs`) draws the prompt and the
//! label, and opens the box.

use glam::Quat;
use verse_engine::assets::{Model, Pack, Placement, Surface, Topology, Vertex};
use verse_engine::inventory::{Asset, AssetId, Binding, fingerprint};

/// Where the box stands, m: to the spawn's right on the landing plaza,
/// clear of the spawn area, the arches and their approaches, the help desk,
/// the Foreman's walk, the terminal, the build billboard, and the Pylon Bay.
pub const POSITION: [f64; 3] = [12.0, 0.0, -10.0];
/// The point the box's front (its slot) faces: the spawn.
pub const FACES: [f64; 2] = [0.0, -10.0];
/// Where a visitor stands to post, in front of the slot, m.
pub const STAND: f64 = 1.2;
/// A visitor this close to [`STAND`], m, is offered the prompt.
pub const ENTER_RANGE: f64 = 2.4;
/// The prompt stays until the visitor is this far, m.
pub const LEAVE_RANGE: f64 = 3.2;
/// Within this distance, m, the label floats over the box.
pub const LABEL_RANGE: f64 = 30.0;

/// The label over the box.
pub const LABEL: &str = "SUGGESTION BOX";
/// The prompt with a keyboard.
pub const PROMPT_KEYS: &str = "Press E to leave a suggestion";
/// The prompt on a touch screen.
pub const PROMPT_TOUCH: &str = "Tap here to leave a suggestion";
/// The key that opens the box.
pub const KEY: &str = "KeyE";

/// The box's model key.
pub const MODEL: &str = "suggestion-box/mailbox";

/// The mailbox's sizes, m, in its own coordinates (x across, y up, z out
/// the front).
const POST_HALF: f64 = 0.08;
const POST_TOP: f64 = 0.95;
const BOX_HALF: [f64; 2] = [0.34, 0.3];
const BOX_TOP: f64 = 1.5;
const ROOF_RISE: f64 = 0.2;
const SLOT: [f64; 3] = [0.2, 1.36, 1.41];
/// How tall the label floats over the ground, m.
pub const LABEL_LIFT: f64 = BOX_TOP + ROOF_RISE + 0.45;

/// The box's yaw, radians about +Y: its front (+Z) turned toward [`FACES`].
#[must_use]
pub fn yaw() -> f64 {
    (FACES[0] - POSITION[0]).atan2(FACES[1] - POSITION[2])
}

/// The world point at box coordinates `local`.
#[must_use]
pub fn world(local: [f64; 3]) -> [f64; 3] {
    let (s, c) = yaw().sin_cos();
    [
        POSITION[0] + local[0] * c + local[2] * s,
        POSITION[1] + local[1],
        POSITION[2] - local[0] * s + local[2] * c,
    ]
}

/// Where the prompt floats: over the roof.
#[must_use]
pub fn prompt_at() -> [f64; 3] {
    world([0.0, BOX_TOP + ROOF_RISE + 0.25, 0.0])
}

/// Where the label floats.
#[must_use]
pub fn label_at() -> [f64; 3] {
    world([0.0, LABEL_LIFT, 0.0])
}

/// Where a visitor stands to post.
#[must_use]
pub fn stand() -> [f64; 3] {
    world([0.0, 0.0, BOX_HALF[1] + STAND])
}

/// Whether a visitor at `player` is close enough to post, with a little
/// hysteresis so the prompt doesn't flicker at the edge.
#[must_use]
pub fn in_reach(player: [f64; 3], already_near: bool) -> bool {
    let at = stand();
    let range = if already_near {
        LEAVE_RANGE
    } else {
        ENTER_RANGE
    };
    player.iter().all(|v| v.is_finite())
        && (player[1] - at[1]).abs() <= 2.0
        && (player[0] - at[0]).hypot(player[2] - at[2]) <= range
}

/// A line tone's color: the page passes the Grid's own ladder.
pub type Tint<'a> = &'a dyn Fn(u8) -> [f32; 3];

/// The faces' tint: the terminal's near-black.
const DARK: [f32; 3] = [0.002; 3];

fn vertex(p: [f64; 3]) -> Vertex {
    Vertex {
        position: p.map(|v| v as f32),
        normal: [0., 1., 0.],
        uv: [0.5, 0.5],
        joints: [0; 4],
        weights: [1., 0., 0., 0.],
    }
}

fn surface(texture: usize, tint: [f32; 3], topology: Topology) -> Surface {
    Surface {
        vertices: vec![],
        indices: vec![],
        texture,
        material: Default::default(),
        blend: 0,
        emissive: false,
        tint,
        topology,
        unlit: true,
    }
}

/// The mailbox's art in its own coordinates: dark quads, edge lines by
/// tone (0 faint to 3 bright), and lit quads.
pub struct Art {
    pub faces: Vec<[[f64; 3]; 4]>,
    pub lines: Vec<([f64; 3], [f64; 3], u8)>,
    pub lit: Vec<[[f64; 3]; 4]>,
}

/// A box from `lo` to `hi`: its six faces and twelve edges in `tone`.
fn cuboid(art: &mut Art, lo: [f64; 3], hi: [f64; 3], tone: u8) {
    let p = |x: bool, y: bool, z: bool| {
        [
            if x { hi[0] } else { lo[0] },
            if y { hi[1] } else { lo[1] },
            if z { hi[2] } else { lo[2] },
        ]
    };
    let (f, t) = (false, true);
    art.faces.extend([
        [p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t)],
        [p(t, f, f), p(f, f, f), p(f, t, f), p(t, t, f)],
        [p(f, f, f), p(f, f, t), p(f, t, t), p(f, t, f)],
        [p(t, f, t), p(t, f, f), p(t, t, f), p(t, t, t)],
        [p(f, t, t), p(t, t, t), p(t, t, f), p(f, t, f)],
        [p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t)],
    ]);
    for (a, b) in [
        ((f, f, f), (t, f, f)),
        ((f, t, f), (t, t, f)),
        ((f, f, t), (t, f, t)),
        ((f, t, t), (t, t, t)),
        ((f, f, f), (f, t, f)),
        ((t, f, f), (t, t, f)),
        ((f, f, t), (f, t, t)),
        ((t, f, t), (t, t, t)),
        ((f, f, f), (f, f, t)),
        ((t, f, f), (t, f, t)),
        ((f, t, f), (f, t, t)),
        ((t, t, f), (t, t, t)),
    ] {
        art.lines.push((p(a.0, a.1, a.2), p(b.0, b.1, b.2), tone));
    }
}

/// The mailbox: a post, a box with a rounded roof, a lit slot on its front,
/// and a raised flag on its side.
#[must_use]
pub fn art() -> Art {
    let mut art = Art {
        faces: vec![],
        lines: vec![],
        lit: vec![],
    };
    cuboid(
        &mut art,
        [-POST_HALF, 0.0, -POST_HALF],
        [POST_HALF, POST_TOP, POST_HALF],
        1,
    );
    let [w, d] = BOX_HALF;
    cuboid(&mut art, [-w, POST_TOP, -d], [w, BOX_TOP, d], 2);
    // The roof: a half round over the box, in six steps across.
    let steps = 6;
    let roof = |i: usize| {
        let a = std::f64::consts::PI * i as f64 / steps as f64;
        [-w * a.cos(), BOX_TOP + ROOF_RISE * a.sin()]
    };
    for i in 0..steps {
        let ([x0, y0], [x1, y1]) = (roof(i), roof(i + 1));
        art.faces
            .push([[x0, y0, d], [x0, y0, -d], [x1, y1, -d], [x1, y1, d]]);
        for z in [d, -d] {
            art.lines.push(([x0, y0, z], [x1, y1, z], 2));
            // The roof's end caps.
            art.faces
                .push([[x0, BOX_TOP, z], [x0, y0, z], [x1, y1, z], [x1, BOX_TOP, z]]);
        }
        art.lines.push(([x0, y0, d], [x0, y0, -d], 1));
    }
    // The slot, lit, on the front.
    let z = d + 0.004;
    art.lit.push([
        [-SLOT[0], SLOT[1], z],
        [SLOT[0], SLOT[1], z],
        [SLOT[0], SLOT[2], z],
        [-SLOT[0], SLOT[2], z],
    ]);
    // The flag, raised, on the right side.
    let x = w + 0.02;
    art.lines
        .push(([x, POST_TOP + 0.2, -0.1], [x, BOX_TOP + 0.25, -0.1], 3));
    art.lit.push([
        [x, BOX_TOP + 0.1, -0.1],
        [x, BOX_TOP + 0.25, -0.1],
        [x, BOX_TOP + 0.25, -0.3],
        [x, BOX_TOP + 0.1, -0.3],
    ]);
    art
}

/// A surface that shares each corner among its lines or faces.
struct Shared {
    surface: Surface,
    seen: std::collections::HashMap<[u64; 3], u32>,
}

impl Shared {
    fn new(texture: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: surface(texture, tint, topology),
            seen: Default::default(),
        }
    }
    fn at(&mut self, p: [f64; 3]) -> u32 {
        let key = p.map(f64::to_bits);
        if let Some(i) = self.seen.get(&key) {
            return *i;
        }
        self.surface.vertices.push(vertex(p));
        let i = (self.surface.vertices.len() - 1) as u32;
        self.seen.insert(key, i);
        i
    }
    fn quad(&mut self, q: &[[f64; 3]; 4]) {
        let [a, b, c, d] = q.map(|p| self.at(p));
        // Both sides: the art doesn't keep one winding.
        self.surface
            .indices
            .extend([a, b, c, a, c, d, a, c, b, a, d, c]);
    }
}

/// The mailbox as a model, in its own coordinates.
#[must_use]
pub fn model(white: usize, tone: Tint) -> Model {
    let art = art();
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for q in &art.faces {
        faces.quad(q);
    }
    let mut surfaces = vec![faces.surface];
    for t in 0..4u8 {
        let mut lines = Shared::new(white, tone(t), Topology::Lines);
        for (a, b, _) in art.lines.iter().filter(|l| l.2 == t) {
            let (a, b) = (lines.at(*a), lines.at(*b));
            lines.surface.indices.extend([a, b]);
        }
        surfaces.push(lines.surface);
    }
    let mut lit = Shared::new(white, tone(3), Topology::Triangles);
    for q in &art.lit {
        lit.quad(q);
    }
    surfaces.push(lit.surface);
    Model {
        graph: None,
        markers: vec![],
        states: Default::default(),
        skin: None,
        source: MODEL.into(),
        source_sha256: String::new(),
        surfaces: surfaces
            .into_iter()
            .filter(|s| !s.indices.is_empty())
            .collect(),
        bones: vec![],
        clips: vec![],
        height: (BOX_TOP + ROOF_RISE) as f32,
        attachments: vec![],
    }
}

/// The box's placement on the Grid.
#[must_use]
pub fn placement() -> Placement {
    Placement {
        model: MODEL.into(),
        position: POSITION.map(|v| v as f32),
        rotation: Quat::from_rotation_y(yaw() as f32).to_array(),
        scale: 1.0,
    }
}

/// Adds the box to the Grid's `pack`, whose white texel is slot `white`
/// and listed as `white_id`, with lines tinted by `tone`.
///
/// # Errors
///
/// The pack has no inventory, or its white texture isn't listed.
pub fn add(pack: &mut Pack, white: usize, white_id: &str, tone: Tint) -> Result<(), String> {
    let inventory = pack
        .inventory
        .as_mut()
        .ok_or("The Grid's inventory is missing")?;
    let white_id = inventory
        .assets
        .iter()
        .find(|a| a.id.as_str() == white_id)
        .ok_or("The Grid's white texture is not listed")?
        .id
        .clone();
    let model = model(white, tone);
    let (sha256, bytes) = fingerprint(&model)?;
    inventory.assets.push(Asset {
        id: AssetId::new(format!("verse:model:{MODEL}"))?,
        binding: Binding::Model { key: MODEL.into() },
        sha256,
        bytes,
        dependencies: vec![white_id],
    });
    pack.models.insert(MODEL.into(), model);
    pack.placements.push(placement());
    Ok(())
}

#[cfg(test)]
mod tests;
