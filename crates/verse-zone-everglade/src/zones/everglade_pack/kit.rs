//! The licensed medieval kit: its pieces as the repository knows them, the
//! pinned kit pack, and the committed proxies drawn without it
//! (`docs/verse/everglade-medieval-refactor.md`).
//!
//! Every kit piece has an ID of our own, such as `kit/wall-4`, and a
//! committed box in the kit's own frame: meters, Y up, a wall running along
//! +x from its origin with its outer face toward +z. What the game simulates
//! (collision, navigation, breaking, and the world tree) comes from these
//! boxes, never from the licensed meshes.
//!
//! The kit pack (`compile::kit`) carries the licensed models. Its bytes live
//! only in the private bucket and in builds; the repository holds its digest,
//! [`KIT_SHA256`], which the artifact queue repins
//! (`openagents artifact submit everglade-kit`). [`install`] puts the kit's
//! models into the decoded Everglade pack under their IDs. Without the kit,
//! as in a contributor's checkout, a public build, or a test, it puts a
//! proxy for each piece there instead: the piece's box, or its doorway,
//! window, roof slope, or steps, in the Everglade pack's own public
//! plaster, timber, stone, and tile images. Either way every `kit/` model the
//! layout places exists, so the town is whole without the licensed content.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::compile::kit as compiled;
use super::format::{AlphaMode, Material, Model, Primitive, Vertex, ZonePack};
use super::pinned::PinnedFile;

/// Exact content identity of the reviewed kit pack, or empty while none is
/// published.
pub const KIT_SHA256: &str = "c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e";
/// Transfer size of the reviewed kit pack; zero while none is published.
pub const KIT_BYTES: u64 = 21467658;
// Retain previous reviewed digests here when changing KIT_SHA256.
const KIT_HISTORY: &[&str] = &[KIT_SHA256];
/// Where every client fetches the kit pack: the OpenAgents web origin,
/// `/everglade/kit/<KIT_SHA256>.vtp`.
pub const KIT_ORIGIN: &str = "https://openagents.com/everglade/kit";
/// How far a kit model's bounds may differ from its piece's committed box,
/// m. Unreal's reduced LOD0 meshes stop a few millimeters short of the
/// source's bounds.
pub const BOUNDS_TOLERANCE: f32 = 0.1;
/// Environment variable naming a local kit pack for offline tools
/// ([`super::ZonePack::load_local`]).
pub const LOCAL_ENV: &str = "VERSE_KIT_PACK";
/// Environment variable that lets an offline tool load a kit pack other
/// than the pinned one ([`load_local`]).
pub const UNPINNED_ENV: &str = "VERSE_KIT_UNPINNED";

/// The reviewed kit pack and its source.
#[must_use]
pub fn pinned() -> PinnedFile {
    PinnedFile {
        label: "Everglade kit pack",
        sha256: KIT_SHA256,
        bytes: KIT_BYTES,
        url: format!("{KIT_ORIGIN}/{KIT_SHA256}.vtp"),
        extension: "vtp",
        temp_prefix: ".everglade-kit-",
        history: KIT_HISTORY,
    }
}

/// Exact content identity of the phone tier's kit pack, the pinned pack with
/// its images at most [`compiled::PHONE_EDGE`] pixels (`everglade_kit --phone`),
/// or empty while none is published.
#[rustfmt::skip]
pub const KIT_PHONE_SHA256: &str = "";
/// Transfer size of the phone tier's kit pack; zero while none is published.
pub const KIT_PHONE_BYTES: u64 = 0;
/// The phone tier's kit transfer budget: the plan's 8 MiB.
pub const KIT_PHONE_BUDGET: u64 = 8 * 1024 * 1024;
// Retain previous reviewed digests here when changing KIT_PHONE_SHA256.
const KIT_PHONE_HISTORY: &[&str] = &[KIT_PHONE_SHA256];

/// Which published files a client fetches (#10908). Desktops and the web
/// take the full kit and all light layers; phones take the phone tier when
/// it is published and the full files otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier {
    #[default]
    Full,
    Phone,
}

/// The kit pack a `tier` fetches: the phone pack when the tier is phone and
/// it is published, the full pack otherwise.
#[must_use]
pub fn pinned_for(tier: Tier) -> PinnedFile {
    if tier == Tier::Phone && KIT_PHONE_BYTES > 0 {
        return PinnedFile {
            label: "Everglade phone kit pack",
            sha256: KIT_PHONE_SHA256,
            bytes: KIT_PHONE_BYTES,
            url: format!("{KIT_ORIGIN}/{KIT_PHONE_SHA256}.vtp"),
            extension: "vtp",
            temp_prefix: ".everglade-kit-phone-",
            history: KIT_PHONE_HISTORY,
        };
    }
    pinned()
}

/// How much of a kit image's fine detail [`grade`] keeps: the rest is its
/// local average, which softens the kit's grit toward a painted surface.
pub const GRADE_DETAIL: f32 = 0.6;
/// How far [`grade`] pushes colors from their gray, 1 for no change.
pub const GRADE_SATURATION: f32 = 1.2;
/// The exponent [`grade`] raises each sRGB channel to, under 1 to lighten.
pub const GRADE_GAMMA: f32 = 0.85;
/// A warm shift [`grade`] multiplies red, green, and blue by.
pub const GRADE_WARMTH: [f32; 3] = [1.04, 1.01, 0.95];

/// The grade every kit image takes when the kit pack is compiled, so the
/// kit's realistic base colors sit with Everglade's painted look
/// (plan phase P3): it softens fine detail toward the local average,
/// lightens, saturates, and warms. Alpha is unchanged. It runs on
/// `width` by `height` sRGB pixels at the pack's texture edge.
pub fn grade(width: u32, height: u32, rgba: &mut [u8]) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() != w * h * 4 {
        return;
    }
    // A box blur of radius r, a sixty-fourth of the image's width, as the
    // local average; summed-area tables keep it linear in the pixels.
    let r = (w.max(h) / 64).max(1);
    let mut sums = vec![[0u64; 3]; (w + 1) * (h + 1)];
    for y in 0..h {
        for x in 0..w {
            let p = &rgba[(y * w + x) * 4..];
            let above = sums[y * (w + 1) + x + 1];
            let left = sums[(y + 1) * (w + 1) + x];
            let diagonal = sums[y * (w + 1) + x];
            let mut s = [0u64; 3];
            for c in 0..3 {
                s[c] = u64::from(p[c]) + above[c] + left[c] - diagonal[c];
            }
            sums[(y + 1) * (w + 1) + x + 1] = s;
        }
    }
    let original = rgba.to_vec();
    for y in 0..h {
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
            let area = ((x1 - x0) * (y1 - y0)) as f32;
            let at = |yy: usize, xx: usize| sums[yy * (w + 1) + xx];
            let i = (y * w + x) * 4;
            let mut rgb = [0.0f32; 3];
            for c in 0..3 {
                let total = at(y1, x1)[c] + at(y0, x0)[c] - at(y0, x1)[c] - at(y1, x0)[c];
                let mean = total as f32 / area;
                let own = f32::from(original[i + c]);
                rgb[c] = (mean + (own - mean) * GRADE_DETAIL) / 255.0;
            }
            let gray = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
            for c in 0..3 {
                let saturated = (gray + (rgb[c] - gray) * GRADE_SATURATION).clamp(0.0, 1.0);
                let lit = saturated.powf(GRADE_GAMMA) * GRADE_WARMTH[c];
                rgba[i + c] = (lit.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
}

/// What a proxy is made of: the Everglade pack's image and a linear tint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coat {
    Plaster,
    Timber,
    Stone,
    Tile,
    Wood,
    Cloth,
    Metal,
    Water,
}

impl Coat {
    /// The Everglade pack image and the linear tint a proxy draws with.
    fn look(self) -> (Option<&'static str>, [f32; 3]) {
        match self {
            Self::Plaster => (Some("village/T_Plaster_BaseColor"), [0.95, 0.92, 0.86]),
            Self::Timber => (Some("village/T_WoodTrim_BaseColor"), [0.8, 0.7, 0.62]),
            Self::Stone => (Some("village/T_RockTrim_BaseColor"), [0.9, 0.88, 0.84]),
            Self::Tile => (Some("village/T_RoundTiles_BaseColor"), [1.0, 1.0, 1.0]),
            Self::Wood => (Some("village/T_WoodTrim_BaseColor"), [1.0, 1.0, 1.0]),
            Self::Cloth => (Some("props/T_Trim_Cloth_BaseColor"), [0.9, 0.45, 0.4]),
            Self::Metal => (Some("props/T_Trim_Metal_BaseColor"), [1.0, 1.0, 1.0]),
            Self::Water => (None, [0.22, 0.36, 0.42]),
        }
    }

    const ALL: [Self; 8] = [
        Self::Plaster,
        Self::Timber,
        Self::Stone,
        Self::Tile,
        Self::Wood,
        Self::Cloth,
        Self::Metal,
        Self::Water,
    ];
}

/// How a proxy is shaped within its piece's box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// The whole box.
    Block,
    /// A wall with a glazed window on its outer (+z) face.
    Window,
    /// A wall open from `from` to `to` along x, up to `top`.
    Doorway { from: f32, to: f32, top: f32 },
    /// A roof slope: the ridge along x at the top of the box at z = 0, the
    /// eave at y = 0 at the box's far z. A gable piece closes its gable end
    /// at x = 0 down to the wall line, `wall` m from the ridge.
    Slope { gable: bool },
    /// Steps rising from the low end at the box's far x to the top at
    /// x = 0, across z.
    Steps,
    /// A market stall: four posts and an awning over the box.
    Stall,
    /// A fountain: a low rim round the box, water inside, and a column in
    /// the middle.
    Fountain,
    /// A lamp post: a slim post and a glowing head at the top.
    Post,
}

/// One kit piece as the repository knows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    /// The pack's model name, `kit/<id>`.
    pub model: &'static str,
    /// The committed box, in the kit's frame, m.
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub shape: Shape,
    pub coat: Coat,
}

const fn piece(
    model: &'static str,
    min: [f32; 3],
    max: [f32; 3],
    shape: Shape,
    coat: Coat,
) -> Piece {
    Piece {
        model,
        min,
        max,
        shape,
        coat,
    }
}

use Coat::{Cloth, Metal, Plaster, Stone, Tile, Timber, Wood};
use Shape::{Block, Fountain, Post, Stall, Steps, Window};

/// Where a 4 m doorway's opening is: x from 1.13 to 2.87 m, 2.62 m tall.
const DOOR_4: Shape = Shape::Doorway {
    from: 1.13,
    to: 2.87,
    top: 2.62,
};
/// A 2 m doorway's opening inside its stone frame.
const DOOR_2: Shape = Shape::Doorway {
    from: 0.2,
    to: 1.8,
    top: 2.55,
};
/// A stone hall's doorway.
const DOOR_B: Shape = Shape::Doorway {
    from: 1.9,
    to: 3.9,
    top: 3.0,
};
const SLOPE: Shape = Shape::Slope { gable: false };
const GABLE: Shape = Shape::Slope { gable: true };

/// Every kit piece, sorted by model name. The boxes are the kit pieces'
/// bounds in its frame, rounded to centimeters.
#[rustfmt::skip]
pub const PIECES: [Piece; 53] = [
    piece("kit/anvil", [-0.25, 0.0, -0.5], [0.25, 0.43, 0.66], Block, Metal),
    piece("kit/awning", [0.0, -1.12, -0.01], [4.0, 0.39, 2.32], Block, Cloth),
    piece("kit/balcony", [0.0, -0.51, -0.2], [2.4, 0.98, 0.83], Block, Wood),
    piece("kit/band-1", [0.0, -0.05, 0.0], [1.0, 0.55, 0.5], Block, Timber),
    piece("kit/band-2", [0.0, -0.05, 0.0], [2.0, 0.55, 0.5], Block, Timber),
    piece("kit/band-4", [0.0, -0.05, 0.0], [4.0, 0.55, 0.5], Block, Timber),
    piece("kit/barrel", [-0.41, -0.02, -0.43], [0.41, 1.15, 0.41], Block, Wood),
    piece("kit/basket", [-0.33, 0.0, -0.32], [0.32, 0.56, 0.33], Block, Wood),
    piece("kit/bench", [-0.38, 0.0, -1.02], [0.51, 1.31, 1.02], Block, Wood),
    piece("kit/bucket", [-0.22, 0.0, -0.23], [0.25, 0.32, 0.23], Block, Wood),
    piece("kit/cart", [-1.24, 0.0, -0.87], [2.08, 1.22, 0.88], Block, Wood),
    piece("kit/chair", [-0.25, -0.01, -0.23], [0.25, 0.92, 0.23], Block, Wood),
    piece("kit/chimney", [-0.57, 0.0, -0.57], [0.57, 3.89, 0.57], Block, Stone),
    piece("kit/chimney-wide", [-0.51, 0.0, -0.71], [0.51, 4.05, 0.71], Block, Stone),
    piece("kit/corner", [-0.1, 0.0, -1.0], [1.0, 4.0, 0.11], Block, Timber),
    piece("kit/corner-b", [0.0, 0.0, 0.0], [2.0, 4.5, 2.0], Block, Stone),
    piece("kit/crate", [-0.65, 0.0, -0.65], [0.65, 1.27, 0.65], Block, Wood),
    piece("kit/door-2", [0.0, 0.0, -0.05], [2.0, 4.0, 0.55], DOOR_2, Plaster),
    piece("kit/door-4", [0.0, 0.0, -0.05], [4.0, 4.0, 0.55], DOOR_4, Plaster),
    piece("kit/door-b", [0.0, 0.0, 0.0], [5.8, 4.5, 1.5], DOOR_B, Stone),
    piece("kit/floor-4", [0.0, 0.0, 0.0], [4.0, 0.5, 4.0], Block, Wood),
    piece("kit/floor-4x2", [0.0, 0.0, 0.0], [4.0, 0.5, 2.0], Block, Wood),
    piece("kit/flowerpot", [-1.34, -0.04, -0.61], [1.34, 0.5, 0.06], Block, Wood),
    piece("kit/forge", [-3.25, 0.0, -1.37], [3.25, 2.91, 0.93], Block, Stone),
    piece("kit/fountain", [-3.09, 0.0, -3.09], [3.09, 2.14, 3.09], Fountain, Stone),
    piece("kit/lamp", [-0.19, 0.0, -0.58], [0.19, 3.44, 0.58], Post, Metal),
    piece("kit/plinth-2", [0.0, 0.0, 0.0], [2.0, 2.0, 0.75], Block, Stone),
    piece("kit/plinth-4", [0.0, 0.0, 0.0], [4.0, 2.0, 0.75], Block, Stone),
    piece("kit/plinth-corner", [0.0, 0.0, -1.25], [1.25, 2.0, 0.0], Block, Stone),
    piece("kit/porch-roof", [-1.1, -0.06, -0.02], [1.1, 0.54, 0.94], Block, Tile),
    piece("kit/ridge-2", [-2.0, 0.0, -0.22], [0.0, 0.37, 0.22], Block, Tile),
    piece("kit/ridge-4", [-4.0, 0.0, -0.22], [0.0, 0.37, 0.22], Block, Tile),
    piece("kit/ridge-end", [-4.0, -0.2, -0.22], [0.5, 0.37, 0.22], Block, Tile),
    piece("kit/ridge-end-mirror", [-0.5, -0.2, -0.22], [4.0, 0.37, 0.22], Block, Tile),
    piece("kit/roof-end", [-4.0, -0.29, 0.0], [1.03, 4.05, 6.11], GABLE, Tile),
    piece("kit/roof-end-mirror", [-1.03, -0.29, 0.0], [4.0, 4.05, 6.11], GABLE, Tile),
    piece("kit/roof-mid", [-4.15, -0.21, 0.0], [0.15, 4.0, 6.01], SLOPE, Tile),
    piece("kit/roof-mid-2", [-2.15, -0.21, 0.0], [0.15, 4.0, 6.0], SLOPE, Tile),
    piece("kit/shutter", [0.0, 0.0, -0.02], [1.0, 2.2, 0.1], Block, Wood),
    piece("kit/sign", [-1.01, 0.0, -0.53], [1.01, 0.6, 0.14], Block, Wood),
    piece("kit/stairs", [0.0, 0.0, 0.0], [2.75, 2.0, 2.0], Steps, Wood),
    piece("kit/stall", [-2.12, -0.01, -0.06], [2.08, 3.48, 2.58], Stall, Cloth),
    piece("kit/table", [-0.5, 0.0, -0.5], [0.5, 0.78, 0.5], Block, Wood),
    piece("kit/wall-2", [0.0, 0.0, -0.04], [2.0, 4.0, 0.54], Block, Plaster),
    piece("kit/wall-2-timber", [0.0, 0.0, -0.12], [2.0, 4.0, 0.63], Block, Plaster),
    piece("kit/wall-4", [0.0, 0.0, -0.04], [4.0, 4.0, 0.54], Block, Plaster),
    piece("kit/wall-4-timber", [0.0, 0.0, -0.12], [4.0, 4.0, 0.63], Block, Plaster),
    piece("kit/wall-b-4", [0.0, 0.0, 0.0], [4.0, 4.5, 1.5], Block, Stone),
    piece("kit/window-4", [0.0, 0.0, -0.06], [4.0, 4.0, 0.56], Window, Plaster),
    piece("kit/window-4-small", [0.0, -0.08, -0.06], [4.0, 4.0, 0.56], Window, Plaster),
    piece("kit/window-4-tall", [0.0, 0.0, -0.06], [4.0, 4.0, 0.64], Window, Plaster),
    piece("kit/window-4-wide", [0.0, 0.0, -0.06], [4.0, 4.0, 0.56], Window, Plaster),
    piece("kit/window-b", [0.0, 0.0, -0.1], [5.7, 4.5, 1.5], Window, Stone),
];

/// The piece a model name places, if it is a kit piece.
#[must_use]
pub fn piece_of(model: &str) -> Option<&'static Piece> {
    PIECES
        .binary_search_by(|p| p.model.cmp(model))
        .ok()
        .map(|i| &PIECES[i])
}

/// A pack of the proxies alone, which every kit piece collides as.
#[must_use]
pub fn proxies() -> &'static ZonePack {
    static PROXIES: std::sync::OnceLock<ZonePack> = std::sync::OnceLock::new();
    PROXIES.get_or_init(|| {
        let mut pack = ZonePack {
            textures: Vec::new(),
            materials: Vec::new(),
            models: Vec::new(),
            character: None,
            forms: Vec::new(),
        };
        install(&mut pack, None);
        pack
    })
}

/// Whether `pack` draws the licensed kit rather than the proxies.
#[must_use]
pub fn installed(pack: &ZonePack) -> bool {
    pack.materials
        .iter()
        .any(|m| m.name.starts_with(&format!("{}/", compiled::SET)))
}

/// What [`install`] put into a pack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    /// Pieces drawn from the kit pack.
    pub kit: usize,
    /// Pieces drawn as proxies.
    pub proxies: usize,
    /// Kit models refused because their bounds left their piece's box.
    pub refused: Vec<String>,
}

/// Puts a model for every kit piece into `pack`, replacing any it already
/// holds: the kit's own model and far level where `kit` has one that fits
/// the piece's box, and the proxy otherwise.
pub fn install(pack: &mut ZonePack, kit: Option<&ZonePack>) -> Installed {
    pack.models.retain(|m| !compiled::is_kit_model(&m.name));
    let mut report = Installed::default();
    let mut remap: Option<usize> = None;
    let mut proxies: Option<[u16; 8]> = None;
    let pane = |pack: &mut ZonePack| -> u16 { proxy_material(pack, "kit-proxy/pane", None, true) };
    let mut pane_index = None;
    let mut glow_index = None;
    for piece in &PIECES {
        let own = kit.and_then(|k| k.model(piece.model).map(|m| (k, m)));
        if let Some((k, model)) = own {
            if fits(piece, model) {
                let materials = *remap.get_or_insert_with(|| append(pack, k));
                pack.models.push(shifted(model, materials));
                if let Some(far) = k.model(&compiled::far_name(piece.model)) {
                    if fits(piece, far) {
                        pack.models.push(shifted(far, materials));
                    } else {
                        report.refused.push(far.name.clone());
                    }
                }
                report.kit += 1;
                continue;
            }
            report.refused.push(piece.model.to_owned());
        }
        let coats = *proxies.get_or_insert_with(|| {
            let mut out = [0u16; 8];
            for (slot, coat) in out.iter_mut().zip(Coat::ALL) {
                let (image, _) = coat.look();
                let name = format!("kit-proxy/{coat:?}").to_lowercase();
                *slot = proxy_material(pack, &name, image, false);
            }
            out
        });
        let glass = *pane_index.get_or_insert_with(|| pane(pack));
        // A lamp's head glows: Everglade lights a material named `Emit…`.
        let glow = *glow_index
            .get_or_insert_with(|| proxy_material(pack, "kit-proxy/Emit_glow", None, false));
        let coat = coats[Coat::ALL.iter().position(|c| *c == piece.coat).unwrap_or(0)];
        pack.models.push(proxy(piece, coat, glass, glow));
        report.proxies += 1;
    }
    if let Some(k) = kit {
        use crate::zones::everglade::house_lod;
        for house in house_lod::houses() {
            let (low, high) = house_lod::bounds(house);
            for level in 0..3 {
                let name = house_lod::model(house, level);
                let Some(model) = k.model(&name) else {
                    continue;
                };
                let (min, max) = model.bounds();
                let fits = (0..3).all(|axis| {
                    (min[axis] - low[axis]).abs() <= 0.11 && (max[axis] - high[axis]).abs() <= 0.11
                }) && model.triangles() <= house_lod::TRIANGLES[level]
                    && model.primitives.len() <= house_lod::DRAWS[level];
                if fits {
                    if !pack.models.iter().any(|m| m.name == name) {
                        let materials = *remap.get_or_insert_with(|| append(pack, k));
                        pack.models.push(shifted(model, materials));
                    }
                } else {
                    report.refused.push(name);
                }
            }
        }
    }
    pack.models.sort_by(|a, b| a.name.cmp(&b.name));
    report
}

/// Whether a kit model's bounds keep to its piece's committed box.
fn fits(piece: &Piece, model: &Model) -> bool {
    let (min, max) = model.bounds();
    (0..3).all(|i| {
        (min[i] - piece.min[i]).abs() <= BOUNDS_TOLERANCE
            && (max[i] - piece.max[i]).abs() <= BOUNDS_TOLERANCE
    })
}

/// Appends the kit's images and materials to `pack`; returns the offset its
/// material indices take.
fn append(pack: &mut ZonePack, kit: &ZonePack) -> usize {
    let textures = pack.textures.len();
    let materials = pack.materials.len();
    pack.textures.extend(kit.textures.iter().cloned());
    pack.materials
        .extend(kit.materials.iter().map(|m| Material {
            texture: m.texture.map(|t| t + textures as u16),
            ..m.clone()
        }));
    materials
}

/// `model` with its material indices moved by `offset`.
fn shifted(model: &Model, offset: usize) -> Model {
    Model {
        name: model.name.clone(),
        primitives: model
            .primitives
            .iter()
            .map(|p| Primitive {
                material: p.material + offset as u16,
                ..p.clone()
            })
            .collect(),
    }
}

/// The index of a proxy material named `name` sampling the pack image
/// `image`, added once. A missing image draws untextured.
fn proxy_material(pack: &mut ZonePack, name: &str, image: Option<&str>, glass: bool) -> u16 {
    if let Some(i) = pack.materials.iter().position(|m| m.name == name) {
        return i as u16;
    }
    let texture = image
        .and_then(|image| pack.textures.iter().position(|t| t.name == image))
        .map(|i| i as u16);
    pack.materials.push(Material {
        name: name.to_owned(),
        texture,
        base_color: [1.0; 4],
        alpha: AlphaMode::Opaque,
        double_sided: glass,
    });
    (pack.materials.len() - 1) as u16
}

/// Linear color as the pack's 8-bit vertex color.
fn color(c: [f32; 3]) -> [u8; 4] {
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(c[0]), byte(c[1]), byte(c[2]), 255]
}

/// Collects proxy triangles: quads with planar texture coordinates, one
/// repeat every two meters.
struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    color: [u8; 4],
}

impl Builder {
    fn new(tint: [f32; 3]) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            color: color(tint),
        }
    }

    /// A quad through `corners`, counter-clockwise seen from its front.
    fn quad(&mut self, corners: [[f32; 3]; 4]) {
        let [a, b, c, _] = corners.map(glam::Vec3::from);
        let normal = (b - a).cross(c - a).normalize_or_zero();
        if normal == glam::Vec3::ZERO {
            return;
        }
        // Project on the plane the normal faces most.
        let n = normal.abs();
        let base = self.vertices.len() as u32;
        for p in corners {
            let uv = if n.x >= n.y && n.x >= n.z {
                [p[2], p[1]]
            } else if n.y >= n.z {
                [p[0], p[2]]
            } else {
                [p[0], p[1]]
            };
            self.vertices.push(Vertex {
                position: p,
                normal: normal.to_array(),
                uv: [uv[0] / 2.0, -uv[1] / 2.0],
                color: self.color,
            });
        }
        self.indices
            .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A box from `lo` to `hi`.
    fn cuboid(&mut self, lo: [f32; 3], hi: [f32; 3]) {
        let [x0, y0, z0] = lo;
        let [x1, y1, z1] = hi;
        if x1 <= x0 || y1 <= y0 || z1 <= z0 {
            return;
        }
        self.quad([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]]);
        self.quad([[x0, y0, z1], [x0, y1, z1], [x0, y1, z0], [x0, y0, z0]]);
        self.quad([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]]);
        self.quad([[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]]);
        self.quad([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]]);
        self.quad([[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]]);
    }

    fn primitive(self, material: u16) -> Option<Primitive> {
        (!self.indices.is_empty()).then_some(Primitive {
            material,
            vertices: self.vertices,
            indices: self.indices,
        })
    }
}

/// The proxy model of `piece`, in material `coat` with window panes and
/// water in `pane` and a lamp's light in `glow`.
fn proxy(piece: &Piece, coat: u16, pane: u16, glow: u16) -> Model {
    let (_, tint) = piece.coat.look();
    let mut body = Builder::new(tint);
    let mut glass = Builder::new([0.12, 0.14, 0.17]);
    let mut light = Builder::new([1.0, 0.78, 0.45]);
    let [x0, y0, z0] = piece.min;
    let [x1, y1, z1] = piece.max;
    match piece.shape {
        Shape::Block => body.cuboid(piece.min, piece.max),
        Shape::Window => {
            body.cuboid(piece.min, piece.max);
            let w = x1 - x0;
            let (a, b) = (x0 + 0.3 * w, x0 + 0.7 * w);
            let (low, high) = (y0 + 0.25 * (y1 - y0), y0 + 0.75 * (y1 - y0));
            let z = z1 + 0.02;
            glass.quad([[a, low, z], [b, low, z], [b, high, z], [a, high, z]]);
        }
        Shape::Doorway { from, to, top } => {
            body.cuboid([x0, y0, z0], [from, y1, z1]);
            body.cuboid([to, y0, z0], [x1, y1, z1]);
            body.cuboid([from, top, z0], [to, y1, z1]);
        }
        Shape::Slope { gable } => {
            // The ridge at the top at z = 0, the eave at y = 0 at the far z.
            let ridge = y1;
            let at = |z: f32| ridge * (1.0 - z / z1);
            let thick = 0.15;
            body.quad([
                [x0, ridge, 0.0],
                [x0, 0.0, z1],
                [x1, 0.0, z1],
                [x1, ridge, 0.0],
            ]);
            body.quad([
                [x1, ridge - thick, 0.0],
                [x1, -thick, z1],
                [x0, -thick, z1],
                [x0, ridge - thick, 0.0],
            ]);
            body.quad([
                [x0, -thick, z1],
                [x1, -thick, z1],
                [x1, 0.0, z1],
                [x0, 0.0, z1],
            ]);
            if gable {
                // Down to the eave's level, as far out as the wall line, 1 m
                // in from the eave; both faces, so it closes from either side.
                let wall = (z1 - 1.0).max(0.0);
                let tri = |b: &mut Builder, x: f32, flip: bool| {
                    let mut c = [
                        [x, 0.0, 0.0],
                        [x, 0.0, wall],
                        [x, at(wall), wall],
                        [x, ridge, 0.0],
                    ];
                    if flip {
                        c.reverse();
                    }
                    b.quad(c);
                };
                tri(&mut body, 0.0, false);
                tri(&mut body, 0.0, true);
            }
        }
        Shape::Stall => {
            let post = 0.08;
            let awning = y1 - 0.35;
            for x in [x0, x1 - post] {
                for z in [z0, z1 - post] {
                    body.cuboid([x, y0, z], [x + post, awning, z + post]);
                }
            }
            body.cuboid([x0, awning, z0], [x1, y1, z1]);
        }
        Shape::Fountain => {
            let (rim, wall) = (0.6, 0.4);
            body.cuboid([x0, y0, z0], [x1, rim, z0 + wall]);
            body.cuboid([x0, y0, z1 - wall], [x1, rim, z1]);
            body.cuboid([x0, y0, z0 + wall], [x0 + wall, rim, z1 - wall]);
            body.cuboid([x1 - wall, y0, z0 + wall], [x1, rim, z1 - wall]);
            let water = rim - 0.15;
            glass.quad([
                [x0 + wall, water, z1 - wall],
                [x1 - wall, water, z1 - wall],
                [x1 - wall, water, z0 + wall],
                [x0 + wall, water, z0 + wall],
            ]);
            let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
            body.cuboid([cx - 0.3, y0, cz - 0.3], [cx + 0.3, y1, cz + 0.3]);
        }
        Shape::Post => {
            let (cx, cz) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
            let head = y1 - 0.5;
            body.cuboid([cx - 0.07, y0, cz - 0.07], [cx + 0.07, head, cz + 0.07]);
            light.cuboid([cx - 0.17, head, cz - 0.17], [cx + 0.17, y1, cz + 0.17]);
        }
        Shape::Steps => {
            let steps = 8;
            let run = (x1 - x0) / steps as f32;
            for i in 0..steps {
                let top = y0 + (y1 - y0) * (steps - i) as f32 / steps as f32;
                body.cuboid(
                    [x0 + run * i as f32, y0, z0],
                    [x0 + run * (i + 1) as f32, top, z1],
                );
            }
        }
    }
    let primitives = [
        body.primitive(coat),
        glass.primitive(pane),
        light.primitive(glow),
    ]
    .into_iter()
    .flatten()
    .collect();
    Model {
        name: piece.model.to_owned(),
        primitives,
    }
}

/// The kit pack in `cache`, if the pinned one is there; downloaded from
/// [`KIT_ORIGIN`] when `download` allows it.
///
/// # Errors
///
/// Returns a message when no kit is published, the cache holds none and
/// downloading is off, or the transfer or decoding fails.
pub fn fetch(cache: &Path, download: bool, cancel: &AtomicBool) -> Result<ZonePack, String> {
    fetch_tier(cache, download, cancel, Tier::Full)
}

/// [`fetch`] for a client `tier` ([`pinned_for`]).
///
/// # Errors
///
/// As [`fetch`].
pub fn fetch_tier(
    cache: &Path,
    download: bool,
    cancel: &AtomicBool,
    tier: Tier,
) -> Result<ZonePack, String> {
    let file = pinned_for(tier);
    if file.bytes == 0 {
        return Err("No kit pack is published".into());
    }
    let decode = |bytes: &[u8]| {
        file.verify(bytes)?;
        compiled::decode(bytes)
    };
    if download {
        return file.fetch(cache, cancel, &mut |_, _| (), decode);
    }
    let bytes = file.read_bounded(&cache.join(file.cache_name()))?;
    decode(&bytes)
}

/// Verifies the pinned digest, then decodes the bounded kit pack.
///
/// # Errors
///
/// Returns a message when the bytes are not the pinned kit or fail to
/// decode.
pub fn decode_pinned(bytes: &[u8]) -> Result<ZonePack, String> {
    pinned().verify(bytes)?;
    compiled::decode(bytes)
}

/// A kit pack from a local file, for offline tools: the pinned pack, or,
/// while none is pinned or with [`UNPINNED_ENV`] set, any kit pack that
/// decodes, such as an earlier build to compare a grade against.
///
/// # Errors
///
/// Returns a message when the file is unreadable, is not the pinned kit, or
/// fails to decode.
pub fn load_local(path: &Path) -> Result<ZonePack, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if KIT_BYTES == 0 || std::env::var_os(UNPINNED_ENV).is_some() {
        return compiled::decode(&bytes);
    }
    decode_pinned(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured tier budgets (#10908, the refactor plan's B4 entry).
    #[test]
    fn each_tier_pins_its_files_within_budget() {
        use super::super::kit_bake;
        // Desktops and the web: the 512 px kit and all four suns.
        assert!(KIT_BYTES <= 24 * 1024 * 1024);
        assert!(kit_bake::KIT_BAKE_BYTES <= 56 * 1024 * 1024);
        assert_eq!(pinned_for(Tier::Full).sha256, KIT_SHA256);
        assert_eq!(
            kit_bake::pinned_for(Tier::Full).sha256,
            kit_bake::KIT_BAKE_SHA256
        );
        // Phones: the 256 px kit and two suns, when published; the full
        // files until then.
        if KIT_PHONE_BYTES > 0 {
            assert!(KIT_PHONE_BYTES <= KIT_PHONE_BUDGET);
            assert_eq!(pinned_for(Tier::Phone).sha256, KIT_PHONE_SHA256);
        } else {
            assert_eq!(pinned_for(Tier::Phone).sha256, KIT_SHA256);
        }
        if kit_bake::KIT_BAKE_PHONE_BYTES > 0 {
            assert!(kit_bake::KIT_BAKE_PHONE_BYTES < kit_bake::KIT_BAKE_BYTES);
            assert!(kit_bake::KIT_BAKE_PHONE_BYTES <= kit_bake::KIT_BAKE_PHONE_BUDGET);
            assert_eq!(
                kit_bake::pinned_for(Tier::Phone).sha256,
                kit_bake::KIT_BAKE_PHONE_SHA256
            );
        }
    }

    fn bare() -> ZonePack {
        ZonePack {
            textures: Vec::new(),
            materials: Vec::new(),
            models: Vec::new(),
            character: None,
            forms: Vec::new(),
        }
    }

    #[test]
    fn the_grade_lightens_saturates_and_softens_but_keeps_alpha() {
        // A gray-green checkerboard with a transparent corner.
        let (w, h) = (16u32, 16u32);
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = if (x + y) % 2 == 0 { 70 } else { 110 };
                let a = if x == 0 && y == 0 { 0 } else { 255 };
                rgba.extend([v, v + 20, v - 10, a]);
            }
        }
        let before = rgba.clone();
        grade(w, h, &mut rgba);
        let mean = |px: &[u8], c: usize| {
            px.chunks_exact(4).map(|p| f32::from(p[c])).sum::<f32>() / (w * h) as f32
        };
        // Lighter on average, and greener than it is red, by more.
        assert!(mean(&rgba, 1) > mean(&before, 1));
        assert!(mean(&rgba, 1) - mean(&rgba, 0) > mean(&before, 1) - mean(&before, 0));
        // The checkerboard's contrast drops.
        let spread = |px: &[u8]| i32::from(px[4 * 17]) - i32::from(px[4 * 18]);
        assert!(spread(&rgba).abs() < spread(&before).abs());
        assert_eq!(rgba[3], 0);
        assert_eq!(rgba[7], 255);
        // A wrong size changes nothing.
        let mut odd = before.clone();
        grade(w + 1, h, &mut odd);
        assert_eq!(odd, before);
    }

    #[test]
    fn the_pieces_are_sorted_kit_names_with_real_boxes() {
        for pair in PIECES.windows(2) {
            assert!(pair[0].model < pair[1].model, "{}", pair[1].model);
        }
        for piece in &PIECES {
            assert!(compiled::is_kit_model(piece.model), "{}", piece.model);
            assert!(
                (0..3).all(|i| piece.max[i] > piece.min[i]),
                "{}",
                piece.model
            );
            assert_eq!(piece_of(piece.model), Some(piece));
        }
        assert!(piece_of("kit/nothing").is_none());
    }

    #[test]
    fn the_recipe_builds_exactly_the_committed_pieces() {
        let recipe: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../scripts/unreal/medieval_kit_recipe.json"
        ))
        .unwrap();
        let ids: Vec<String> = recipe["pieces"]
            .as_object()
            .unwrap()
            .keys()
            .map(|id| format!("kit/{id}"))
            .collect();
        let pieces: Vec<String> = PIECES.iter().map(|p| p.model.to_owned()).collect();
        assert_eq!(ids, pieces);
    }

    #[test]
    fn without_the_kit_every_piece_is_a_proxy_within_its_box() {
        let mut pack = bare();
        let report = install(&mut pack, None);
        assert_eq!((report.kit, report.proxies), (0, PIECES.len()));
        assert!(!installed(&pack));
        for piece in &PIECES {
            let model = pack.model(piece.model).expect("every piece has a model");
            let (min, max) = model.bounds();
            for i in 0..3 {
                assert!(min[i] >= piece.min[i] - 0.2, "{} min", piece.model);
                assert!(max[i] <= piece.max[i] + 0.05, "{} max", piece.model);
            }
            assert!(model.triangles() > 0);
        }
        // Installing again replaces rather than duplicates.
        install(&mut pack, None);
        assert_eq!(pack.models.len(), PIECES.len());
    }

    #[test]
    fn a_kit_model_that_fits_replaces_its_proxy_and_one_that_does_not_is_refused() {
        // The sample's pieces are 1 m cubes, far from a 4 m wall's box, so
        // the wall is refused; the bucket's cube is reshaped to its box.
        let bytes = compiled::sample(&["bucket", "wall-4"]);
        let mut kit = compiled::decode(&bytes).unwrap();
        // Shape both bucket levels to the bucket's box.
        let bucket = piece_of("kit/bucket").unwrap();
        for model in kit
            .models
            .iter_mut()
            .filter(|m| m.name == "kit/bucket" || m.name == "lod/kit.bucket")
        {
            for primitive in &mut model.primitives {
                for v in &mut primitive.vertices {
                    for i in 0..3 {
                        v.position[i] = if v.position[i] > 0.2 {
                            bucket.max[i]
                        } else {
                            bucket.min[i]
                        };
                    }
                }
            }
        }
        let mut pack = bare();
        let report = install(&mut pack, Some(&kit));
        assert_eq!(report.kit, 1);
        assert_eq!(report.refused, ["kit/wall-4"]);
        assert_eq!(report.proxies, PIECES.len() - 1);
        assert!(installed(&pack));
        // The wall's sample had a far level, but the wall was refused, so
        // no far level comes with it.
        assert!(pack.model("lod/kit.wall-4").is_none());
        let model = pack.model("kit/bucket").unwrap();
        let material = &pack.materials[model.primitives[0].material as usize];
        assert_eq!(material.name, "kit/T_sample");
        assert_eq!(
            pack.textures[material.texture.unwrap() as usize].name,
            "kit/T_sample"
        );
    }

    #[test]
    fn far_levels_must_keep_to_the_same_piece_box() {
        let mut source = compiled::decode(&compiled::sample(&["bucket"])).unwrap();
        let bucket = piece_of("kit/bucket").unwrap();
        for model in &mut source.models {
            for primitive in &mut model.primitives {
                for vertex in &mut primitive.vertices {
                    for i in 0..3 {
                        vertex.position[i] = if vertex.position[i] > 0.2 {
                            bucket.max[i]
                        } else {
                            bucket.min[i]
                        };
                    }
                }
            }
        }
        let mut pack = bare();
        assert!(install(&mut pack, Some(&source)).refused.is_empty());
        assert!(pack.model("lod/kit.bucket").is_some());
        source
            .models
            .iter_mut()
            .find(|m| m.name == "lod/kit.bucket")
            .unwrap()
            .primitives[0]
            .vertices[0]
            .position[0] = bucket.max[0] + 1.0;
        let report = install(&mut pack, Some(&source));
        assert_eq!(report.refused, ["lod/kit.bucket"]);
        assert!(pack.model("kit/bucket").is_some());
        assert!(pack.model("lod/kit.bucket").is_none());
    }

    #[test]
    fn no_kit_is_fetched_while_none_is_published() {
        if KIT_BYTES != 0 {
            return;
        }
        let cache = tempfile::tempdir().unwrap();
        let error = fetch(cache.path(), true, &AtomicBool::new(false)).unwrap_err();
        assert!(error.contains("published"), "{error}");
    }
}
