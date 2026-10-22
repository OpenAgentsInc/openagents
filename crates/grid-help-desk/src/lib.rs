//! The Grid's help desk (#12443): a counter signed ASK US near the spawn,
//! with the launch film's MacBook open on it and its iPhone on a stand.
//! A visitor who walks up to it is offered "Press E to ask", which opens the
//! site's free chat with our agent.
//!
//! This crate is the desk's plain data, so native tests cover it: where it
//! stands ([`POSITION`], [`yaw`]), when a visitor is in reach ([`in_reach`]),
//! the words it shows, and its models ([`add`]). The browser Grid
//! (`crates/everglade-web/src/help_desk.rs`) draws the prompt and opens the
//! chat.

pub mod art;
pub mod devices;

#[cfg(test)]
mod compile;

use glam::{Mat4, Quat, Vec3};
use sha2::{Digest, Sha256};
use verse_engine::assets::{Model, Pack, Placement, Surface, Texture, Topology, Vertex};
use verse_engine::inventory::{Asset, AssetId, Binding, License, Origin, fingerprint};

/// Where the desk stands, m: ahead of the spawn and to its right, on the
/// way in and clear of the plaza's middle, the terminal, the arches, the
/// Pylon Bay, and the Gym.
pub const POSITION: [f64; 3] = [-6.0, 0.0, -5.0];
/// The point the desk's front faces: the walk in from the spawn.
pub const FACES: [f64; 2] = [0.0, -7.0];
/// Where a visitor stands to ask, in front of the counter, m.
pub const STAND: f64 = 1.3;
/// A visitor this close to [`STAND`], m, is offered the prompt.
pub const ENTER_RANGE: f64 = 2.6;
/// The prompt stays until the visitor is this far, m.
pub const LEAVE_RANGE: f64 = 3.4;
/// The prompt's height over the floor, m: just over the counter.
pub const PROMPT_LIFT: f64 = 1.75;

/// The sign over the desk.
pub const SIGN: &str = "ASK US";
/// The prompt with a keyboard.
pub const PROMPT_KEYS: &str = "Press E to ask";
/// The prompt on a touch screen.
pub const PROMPT_TOUCH: &str = "Tap here to ask";
/// The key that opens the chat.
pub const KEY: &str = "KeyE";

/// The desk's yaw, radians about +Y: its front (+Z) turned toward
/// [`FACES`].
#[must_use]
pub fn yaw() -> f64 {
    (FACES[0] - POSITION[0]).atan2(FACES[1] - POSITION[2])
}

/// The world point at desk coordinates `local` (x across, y up, z out the
/// front).
#[must_use]
pub fn world(local: [f64; 3]) -> [f64; 3] {
    let (s, c) = yaw().sin_cos();
    [
        POSITION[0] + local[0] * c + local[2] * s,
        POSITION[1] + local[1],
        POSITION[2] - local[0] * s + local[2] * c,
    ]
}

/// Where the prompt floats, over the counter's front.
#[must_use]
pub fn prompt_at() -> [f64; 3] {
    world([0.0, PROMPT_LIFT, 0.6])
}

/// Whether a visitor at `player` is close enough to ask, with a little
/// hysteresis so the prompt doesn't flicker at the edge.
#[must_use]
pub fn in_reach(player: [f64; 3], already_near: bool) -> bool {
    let at = world([0.0, 0.0, STAND]);
    let range = if already_near {
        LEAVE_RANGE
    } else {
        ENTER_RANGE
    };
    player.iter().all(|v| v.is_finite())
        && (player[1] - at[1]).abs() <= 2.0
        && (player[0] - at[0]).hypot(player[2] - at[2]) <= range
}

/// The desk's model keys.
pub const COUNTER: &str = "desk/counter";
pub const MACBOOK: &str = "desk/macbook";
pub const IPHONE: &str = "desk/iphone";
/// The screens' file names in the pack.
pub const MACBOOK_SCREEN_FILE: &str = "help-desk-macbook-screen.png";
pub const IPHONE_SCREEN_FILE: &str = "help-desk-iphone-screen.png";

/// The devices are drawn this much larger than life, so they read from
/// the walk in.
pub const DEVICE_SCALE: f32 = 1.5;
/// The laptop's place on the counter, desk coordinates.
const MACBOOK_AT: [f64; 3] = [-0.42, art::TOP, -0.05];
/// The phone's place on its stand, desk coordinates: its back's middle,
/// high enough that its foot rests on the stand's base. And its lean back
/// from upright, degrees, as the film poses it.
const IPHONE_AT: [f64; 3] = [art::PHONE_STAND[0], art::TOP + 0.123, art::PHONE_STAND[1]];
const IPHONE_LEAN: f32 = 72.0;

/// The faces' tint: the terminal's near-black.
const DARK: [f32; 3] = [0.002; 3];

/// A line tone's color: the page passes the Grid's own ladder.
pub type Tint<'a> = &'a dyn Fn(u8) -> [f32; 3];

fn vertex(p: [f32; 3], uv: [f32; 2]) -> Vertex {
    Vertex {
        position: p,
        normal: [0., 1., 0.],
        uv,
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

fn model(source: &str, surfaces: Vec<Surface>, height: f32) -> Model {
    Model {
        graph: None,
        markers: vec![],
        states: Default::default(),
        skin: None,
        source: source.into(),
        source_sha256: String::new(),
        surfaces: surfaces
            .into_iter()
            .filter(|s| !s.indices.is_empty())
            .collect(),
        bones: vec![],
        clips: vec![],
        height,
        attachments: vec![],
    }
}

/// A surface that shares each corner among its lines or faces.
struct Shared {
    surface: Surface,
    seen: std::collections::HashMap<[u32; 3], u32>,
}

impl Shared {
    fn new(texture: usize, tint: [f32; 3], topology: Topology) -> Self {
        Self {
            surface: surface(texture, tint, topology),
            seen: Default::default(),
        }
    }
    fn at(&mut self, p: [f32; 3]) -> u32 {
        let key = p.map(f32::to_bits);
        if let Some(i) = self.seen.get(&key) {
            return *i;
        }
        self.surface.vertices.push(vertex(p, [0.5, 0.5]));
        let i = (self.surface.vertices.len() - 1) as u32;
        self.seen.insert(key, i);
        i
    }
}

fn f32s(p: [f64; 3]) -> [f32; 3] {
    p.map(|v| v as f32)
}

/// The counter and its sign as a model, in desk coordinates.
fn counter_model(white: usize, tone: Tint) -> Model {
    let art = art::desk();
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for q in &art.faces {
        let [a, b, c, d] = q.map(|p| faces.at(f32s(p)));
        faces.surface.indices.extend([a, b, c, a, c, d]);
    }
    let mut surfaces = vec![faces.surface];
    for t in 0..4u8 {
        let mut lines = Shared::new(white, tone(t), Topology::Lines);
        for (a, b, _) in art.lines.iter().filter(|l| l.2 == t) {
            let (a, b) = (lines.at(f32s(*a)), lines.at(f32s(*b)));
            lines.surface.indices.extend([a, b]);
        }
        surfaces.push(lines.surface);
        let mut lit = Shared::new(white, tone(t), Topology::Triangles);
        for (q, _) in art.lit.iter().filter(|q| q.1 == t) {
            let [a, b, c, d] = q.map(|p| lit.at(f32s(p)));
            lit.surface.indices.extend([a, b, c, a, c, d]);
        }
        surfaces.push(lit.surface);
    }
    model(COUNTER, surfaces, art::SIGN_TOP as f32)
}

/// A device as a model: its faces dark, its edges in their tones, and its
/// display showing the texture in `screen`.
fn device_model(
    key: &str,
    device: &devices::Device,
    white: usize,
    screen: usize,
    tone: Tint,
) -> Model {
    let mut faces = Shared::new(white, DARK, Topology::Triangles);
    for t in &device.triangles {
        let [a, b, c] = t.map(|i| faces.at(device.corners[usize::from(i)]));
        faces.surface.indices.extend([a, b, c]);
    }
    let mut surfaces = vec![faces.surface];
    for t in 0..4u8 {
        let mut lines = Shared::new(white, tone(t), Topology::Lines);
        for ([a, b], _) in device.edges.iter().filter(|e| e.1 == t) {
            let a = lines.at(device.corners[usize::from(*a)]);
            let b = lines.at(device.corners[usize::from(*b)]);
            lines.surface.indices.extend([a, b]);
        }
        surfaces.push(lines.surface);
    }
    let mut display = surface(screen, [1.0; 3], Topology::Triangles);
    display.emissive = true;
    display.vertices = device
        .screen
        .iter()
        .map(|(p, uv)| vertex(*p, *uv))
        .collect();
    display.indices = device
        .screen_indices
        .iter()
        .map(|i| u32::from(*i))
        .collect();
    surfaces.push(display);
    model(key, surfaces, device.height())
}

/// The placements of the counter, the laptop, and the phone.
#[must_use]
pub fn placements() -> Vec<Placement> {
    let desk = Quat::from_rotation_y(yaw() as f32);
    let place = |model: &str, local: [f64; 3], turn: Quat, scale: f32| Placement {
        model: model.into(),
        position: f32s(world(local)),
        rotation: (desk * turn).to_array(),
        scale,
    };
    vec![
        place(COUNTER, [0.0; 3], Quat::IDENTITY, 1.0),
        place(MACBOOK, MACBOOK_AT, Quat::IDENTITY, DEVICE_SCALE),
        place(
            IPHONE,
            IPHONE_AT,
            Quat::from_rotation_x(IPHONE_LEAN.to_radians()),
            DEVICE_SCALE,
        ),
    ]
}

/// Where each device's screen is in the world, for captures and checks:
/// its transform from model to world.
#[must_use]
pub fn transform(placement: &Placement) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(placement.scale),
        Quat::from_array(placement.rotation),
        Vec3::from_array(placement.position),
    )
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The PNG's width and height.
fn png_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return Err("A help desk screen isn't a PNG".into());
    }
    let read =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok((read(16), read(20)))
}

/// The screens' files, by the names [`add`] gives them in the pack.
#[must_use]
pub fn files() -> [(&'static str, &'static [u8]); 2] {
    [
        (MACBOOK_SCREEN_FILE, devices::MACBOOK_SCREEN),
        (IPHONE_SCREEN_FILE, devices::IPHONE_SCREEN),
    ]
}

/// Adds the desk to the Grid's `pack`, whose white texel is slot `white`
/// and listed as `white_id`, with lines tinted by `tone`: the counter and the two devices, their two
/// screens, and their placements. The page admits the screens with
/// [`files`].
///
/// # Errors
///
/// The pinned devices can't be read, or the pack has no inventory.
pub fn add(pack: &mut Pack, white: usize, white_id: &str, tone: Tint) -> Result<(), String> {
    let devices = devices::pinned()?;
    let device = |name: &str| {
        devices
            .iter()
            .find(|d| d.name == name)
            .ok_or_else(|| format!("The help desk has no {name}"))
    };
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
    let source = AssetId::new("verse:source:help-desk")?;
    inventory.assets.push(Asset {
        id: source.clone(),
        binding: Binding::Source {
            origin: Origin {
                creator: "OpenAgents contributors".into(),
                license: License::Apache2,
                revision: sha(devices::PINNED),
                format: "gltf".into(),
            },
        },
        sha256: sha(devices::PINNED),
        bytes: devices::PINNED.len() as u64,
        dependencies: vec![],
    });
    let mut slots = Vec::new();
    for (file, bytes) in files() {
        let (width, height) = png_size(bytes)?;
        pack.textures.push(Texture {
            file: file.into(),
            sha256: sha(bytes),
            width,
            height,
        });
        let slot = pack.textures.len() - 1;
        let id = AssetId::new(format!("verse:texture:{}", file.trim_end_matches(".png")))?;
        inventory.assets.push(Asset {
            id: id.clone(),
            binding: Binding::Texture { slot },
            sha256: sha(bytes),
            bytes: bytes.len() as u64,
            dependencies: vec![source.clone()],
        });
        slots.push((slot, id));
    }
    let models = [
        (COUNTER.to_owned(), counter_model(white, tone), None),
        (
            MACBOOK.to_owned(),
            device_model(MACBOOK, device("macbook")?, white, slots[0].0, tone),
            Some(slots[0].1.clone()),
        ),
        (
            IPHONE.to_owned(),
            device_model(IPHONE, device("iphone")?, white, slots[1].0, tone),
            Some(slots[1].1.clone()),
        ),
    ];
    for (key, model, screen) in models {
        let (sha256, bytes) = fingerprint(&model)?;
        let mut dependencies = vec![white_id.clone()];
        dependencies.extend(screen);
        inventory.assets.push(Asset {
            id: AssetId::new(format!("verse:model:{key}"))?,
            binding: Binding::Model { key: key.clone() },
            sha256,
            bytes,
            dependencies,
        });
        pack.models.insert(key, model);
    }
    pack.placements.extend(placements());
    Ok(())
}

#[cfg(test)]
mod tests;
