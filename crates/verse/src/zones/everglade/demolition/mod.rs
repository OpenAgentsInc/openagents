//! The demolition yard: a standalone demo of destructible buildings
//! (`docs/verse/destructible-buildings.md`, phase D1). `verse --demolition`
//! opens Everglade's ground and sky with none of its layout, and two kit
//! cottages ([`cottage`]) in the clearing. The player's character holds a
//! sledgehammer in its right hand and swings it with both (left click or
//! `1`), playing the pack's two-handed chop; the blow lands at the chop's
//! impact. A struck piece darkens and cracks, shows the damage as a
//! floating number, breaks into its chunks ([`chunks`]) at zero hit
//! points, and what it held up drops, leans, and crashes ([`site`]). `R`
//! rebuilds the cottages. The yard's [`hotbar`] replaces the zone panel.
//!
//! The cottages are not in the zone's merged static cells. Every piece
//! draws as part of the frame's one textured figure, after the player's
//! character: its chunks' triangles are posed on the CPU each frame from
//! the piece's body while it is whole and from each chunk's body once it
//! breaks. No light is baked into them.

pub mod chunks;
pub mod cottage;
pub mod hotbar;
pub mod site;
#[cfg(test)]
mod tests;

use super::draw::shade;
use super::player::{BUTT, HEAD_AT, Hold, SwingTrack};
use super::scene::{Copied, copy_material};
use crate::controller::{Footprint, PlayerController};
use crate::mesh::{Mesh, Vertex};
use crate::pbr::textured::{Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex};
use crate::zones::everglade_pack::ZonePack;
use crate::zones::grove::draw::{FLOAT, Floater, Painter};
use glam::{Mat4, Quat, Vec3};
use site::{Blow, Role, Site, Status};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Seed of the yard's dice and debris spread.
const SEED: u64 = 0x5EED_D3B0;
/// A swing without the pack's chop, as a character-less yard swings it:
/// wind-up, strike, and recovery, s.
const WIND_UP: f32 = 0.22;
const STRIKE: f32 = 0.14;
const SWING: f32 = 0.72;
/// That swing's hammer angle from straight up toward the facing, radians:
/// at rest over the shoulder, drawn back, and at the end of the strike.
const REST: f32 = -0.6;
const DRAWN: f32 = -1.9;
const FOLLOW: f32 = 2.0;
/// How far into the strike the blow lands.
const HIT_AT: f32 = 0.8;
/// How fast the pack's chop plays, so a swing feels as heavy as it is.
const CHOP_SPEED: f32 = 1.15;
/// The stretch of the chop before and after its impact the head sweeps
/// for pieces to strike, s of the clip.
const SWEEP: [f32; 2] = [0.24, 0.05];
/// How far from the head's path a piece is struck, m.
const REACH: f32 = 0.5;
/// The sledgehammer: the handle's radius, the grip's wrap, and the head's
/// half extents along the handle, across it, and along its striking face.
const HANDLE_RADIUS: f32 = 0.028;
const WRAP: [f32; 2] = [0.033, 0.34];
const HEAD: Vec3 = Vec3::new(0.075, 0.075, 0.16);
const WOOD: [f32; 3] = [0.34, 0.19, 0.08];
const LEATHER: [f32; 3] = [0.13, 0.065, 0.03];
const IRON: [f32; 3] = [0.12, 0.125, 0.14];
const STEEL: [f32; 3] = [0.24, 0.245, 0.26];
const CRACK: [f32; 3] = [0.035, 0.03, 0.028];
/// The colors a blow's number floats in, as the Grove's damage numbers
/// do: gold for a hit, and fire orange for one that breaks the piece.
const HIT: [f32; 3] = [1.0, 0.74, 0.22];
const BREAK: [f32; 3] = [1.0, 0.42, 0.12];

/// Where one chunk's triangles are in the figure's demolition vertices.
#[derive(Clone, Copy, Debug)]
struct Span {
    piece: usize,
    chunk: usize,
    start: usize,
    len: usize,
}

/// The yard's live state and the meshes it draws.
pub(crate) struct Demolition {
    site: Site,
    /// The yard's own scene: the kit's images and materials and one mesh,
    /// a primitive per material holding every chunk's triangles.
    scene: Arc<TexturedScene>,
    /// Every chunk's triangles in its own frame, in figure order.
    local: Vec<TexturedVertex>,
    spans: Vec<Span>,
    /// This frame's posed triangles.
    posed: Arc<Vec<TexturedVertex>>,
    /// The character's scene and the scene of it and the yard together.
    combined: Option<(Arc<TexturedScene>, Arc<TexturedScene>)>,
    /// Time into the current swing, s: of the pack's chop when the yard
    /// has its track, else of the character-less swing.
    swing: Option<f32>,
    /// The chop as the player's character plays it.
    track: Option<SwingTrack>,
    struck: bool,
    last: Option<Blow>,
    misses: u32,
    revision: Option<u64>,
    /// Seconds since the yard opened, and the blows' numbers in the air.
    clock: f32,
    floaters: Vec<Floater>,
}

impl Demolition {
    /// The yard with both cottages built from `pack`'s kit.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a kit model.
    pub fn new(pack: &ZonePack) -> Result<Self, String> {
        let drafts = cottage::drafts();
        let mut specs = Vec::with_capacity(drafts.len());
        let mut scene = TexturedScene::default();
        let mut copied = Copied::default();
        // Every chunk's triangles by scene material.
        let mut by_material: BTreeMap<usize, Vec<(usize, usize, Vec<TexturedVertex>)>> =
            BTreeMap::new();
        for (piece, draft) in drafts.iter().enumerate() {
            let meshes = chunks::cut(pack, draft)?;
            let mut cuboids = Vec::with_capacity(meshes.len());
            for (chunk, mesh) in meshes.into_iter().enumerate() {
                cuboids.extend(mesh.cuboid);
                for (material, vertices) in mesh.parts {
                    let material = copy_material(pack, material, &mut scene, &mut copied)?;
                    by_material
                        .entry(material)
                        .or_default()
                        .push((piece, chunk, vertices));
                }
            }
            specs.push(draft.spec(cuboids));
        }
        let mut local = Vec::new();
        let mut spans = Vec::new();
        let mut primitives = Vec::new();
        for (material, entries) in by_material {
            let first = local.len();
            for (piece, chunk, vertices) in entries {
                spans.push(Span {
                    piece,
                    chunk,
                    start: local.len(),
                    len: vertices.len(),
                });
                local.extend(vertices);
            }
            let vertices = local[first..].to_vec();
            let indices = (0..vertices.len() as u32).collect();
            primitives.push(Primitive {
                vertices,
                indices,
                material,
            });
        }
        scene.add_mesh(TexturedMesh { primitives });
        let mut demolition = Self {
            site: Site::new(specs, SEED),
            scene: Arc::new(scene),
            posed: Arc::new(local.clone()),
            local,
            spans,
            combined: None,
            swing: None,
            track: None,
            struck: false,
            last: None,
            misses: 0,
            revision: None,
            clock: 0.0,
            floaters: Vec::new(),
        };
        demolition.pose();
        Ok(demolition)
    }

    #[cfg(test)]
    pub fn site(&self) -> &Site {
        &self.site
    }

    /// Swings with the player's character's chop `track` from now on,
    /// or, with `None`, without a character.
    pub fn set_track(&mut self, track: Option<SwingTrack>) {
        self.track = track;
    }

    /// Starts a swing unless one is under way. Returns whether it started.
    pub fn swing(&mut self) -> bool {
        if self.swing.is_some() {
            return false;
        }
        self.swing = Some(0.0);
        self.struck = false;
        true
    }

    /// Seconds into the character's chop while a swing plays it.
    #[must_use]
    pub fn chop(&self) -> Option<f32> {
        self.track.as_ref().and(self.swing)
    }

    /// Rebuilds both cottages.
    pub fn reset(&mut self) {
        self.site.reset();
        self.last = None;
        self.misses = 0;
        self.floaters.clear();
        self.pose();
    }

    /// The hammer's angle `t` seconds into a swing, or at rest.
    fn angle(t: Option<f32>) -> f32 {
        let ease = |x: f32| x * x * (3.0 - 2.0 * x);
        match t {
            None => REST,
            Some(t) if t < WIND_UP => REST + (DRAWN - REST) * ease(t / WIND_UP),
            // The strike accelerates through the blow.
            Some(t) if t < WIND_UP + STRIKE => {
                let x = (t - WIND_UP) / STRIKE;
                DRAWN + (FOLLOW - DRAWN) * x * x
            }
            Some(t) => {
                let x = ((t - WIND_UP - STRIKE) / (SWING - WIND_UP - STRIKE)).clamp(0.0, 1.0);
                FOLLOW + (REST - FOLLOW) * ease(x)
            }
        }
    }

    /// How a character-less yard holds the hammer at `angle` for `player`.
    fn hammer(player: &PlayerController, angle: f32) -> Hold {
        let forward = player.forward();
        let right = forward.cross(Vec3::Y);
        Hold {
            grip: player.pos + Vec3::Y * 1.25 + right * 0.28 + forward * 0.15,
            axis: Vec3::Y * angle.cos() + forward * angle.sin(),
            face: -Vec3::Y * angle.sin() + forward * angle.cos(),
        }
    }

    /// Advances the swing, the hammer's blow, the numbers, and the yard's
    /// bodies.
    pub fn tick(&mut self, dt: f32, player: &PlayerController) {
        self.clock += dt;
        if let Some(t) = &mut self.swing {
            let (impact, length) = match &self.track {
                Some(track) => (track.impact, track.duration),
                None => (WIND_UP + STRIKE * HIT_AT, SWING),
            };
            *t += dt
                * if self.track.is_some() {
                    CHOP_SPEED
                } else {
                    1.0
                };
            let t = *t;
            if !self.struck && t >= impact {
                self.struck = true;
                let path = self.sweep(player);
                match self.site.strike(&path, player.forward(), REACH) {
                    Some(blow) => {
                        self.last = Some(blow);
                        self.floaters.push(Floater {
                            // Over the struck spot, toward the player so the
                            // wall does not hide it.
                            at: blow.at + Vec3::Y * 0.6 - player.forward() * 0.45,
                            text: blow.damage.to_string(),
                            color: if blow.broke { BREAK } else { HIT },
                            start: self.clock,
                        });
                    }
                    None => self.misses += 1,
                }
            }
            if t >= length {
                self.swing = None;
            }
        }
        let now = self.clock;
        self.floaters.retain(|f| now - f.start < FLOAT);
        self.site.tick(dt);
        self.pose();
    }

    /// The head's path through the blow for `player`.
    fn sweep(&self, player: &PlayerController) -> Vec<Vec3> {
        match &self.track {
            Some(track) => {
                let root =
                    Mat4::from_rotation_translation(Quat::from_rotation_y(player.yaw), player.pos);
                (0..=8)
                    .map(|i| {
                        let t = track.impact - SWEEP[0] + (SWEEP[0] + SWEEP[1]) * i as f32 / 8.0;
                        root.transform_point3(track.hold(t).head())
                    })
                    .collect()
            }
            None => (0..=8)
                .map(|i| Self::hammer(player, 0.6 + (FOLLOW - 0.6) * i as f32 / 8.0).head())
                .collect(),
        }
    }

    /// The player's blockers when standing pieces changed since the last
    /// call.
    pub fn take_blocks(&mut self) -> Option<Vec<(Footprint, f32)>> {
        let revision = self.site.revision();
        if self.revision == Some(revision) {
            return None;
        }
        self.revision = Some(revision);
        Some(self.site.blocks())
    }

    /// Poses every chunk's triangles for this frame.
    fn pose(&mut self) {
        let mut posed = Vec::with_capacity(self.local.len());
        let pieces = self.site.pieces();
        let specs = self.site.specs();
        for span in &self.spans {
            let piece = &pieces[span.piece];
            let spec = &specs[span.piece];
            let (transform, shade) = match piece.status {
                Status::Broken => (self.site.chunk_pose(span.piece, span.chunk), 0.8),
                _ => {
                    let damage = 1.0 - piece.hit_points as f32 / spec.hit_points.max(1) as f32;
                    let frame = spec.chunks.get(span.chunk).map(site::Cuboid::frame);
                    (
                        frame.map(|f| self.site.piece_pose(span.piece) * f),
                        1.0 - 0.45 * damage,
                    )
                }
            };
            let source = &self.local[span.start..span.start + span.len];
            match transform {
                Some(m) => posed.extend(source.iter().map(|v| TexturedVertex {
                    pos: m.transform_point3(Vec3::from(v.pos)).to_array(),
                    normal: m.transform_vector3(Vec3::from(v.normal)).to_array(),
                    color: [
                        (f32::from(v.color[0]) * shade) as u8,
                        (f32::from(v.color[1]) * shade) as u8,
                        (f32::from(v.color[2]) * shade) as u8,
                        v.color[3],
                    ],
                    ..*v
                })),
                // A chunk that is gone collapses to a point under the ground.
                None => posed.extend(source.iter().map(|v| TexturedVertex {
                    pos: [0.0, -50.0, 0.0],
                    ..*v
                })),
            }
        }
        self.posed = Arc::new(posed);
    }

    /// Joins the yard's scene to the character's `scene` once per distinct
    /// character scene.
    pub fn prepare(&mut self, scene: Option<&Arc<TexturedScene>>) {
        let Some(scene) = scene else {
            self.combined = None;
            return;
        };
        if self
            .combined
            .as_ref()
            .is_some_and(|(cast, _)| Arc::ptr_eq(cast, scene))
        {
            return;
        }
        let mut joined = scene.as_ref().clone();
        let images = joined.images.len();
        let materials = joined.materials.len();
        joined.images.extend(self.scene.images.iter().cloned());
        for material in &self.scene.materials {
            let mut material = material.clone();
            material.image = material.image.map(|i| i + images);
            joined.materials.push(material);
        }
        if let Some(mesh) = joined.meshes.first_mut() {
            for primitive in &self.scene.meshes[0].primitives {
                let mut primitive = primitive.clone();
                primitive.material += materials;
                mesh.primitives.push(primitive);
            }
        }
        self.combined = Some((scene.clone(), Arc::new(joined)));
    }

    /// The frame's figure: the character's `cast` figure followed by the
    /// yard, or the yard alone.
    #[must_use]
    pub fn figure(&self, cast: Option<Figure>) -> Figure {
        match (cast, &self.combined) {
            (Some(cast), Some((scene, joined))) if Arc::ptr_eq(scene, &cast.scene) => {
                let mut vertices = Vec::with_capacity(cast.vertices.len() + self.posed.len());
                vertices.extend_from_slice(&cast.vertices);
                vertices.extend_from_slice(&self.posed);
                Figure {
                    scene: joined.clone(),
                    vertices: Arc::new(vertices),
                }
            }
            (Some(cast), _) => cast,
            (None, _) => Figure {
                scene: self.scene.clone(),
                vertices: self.posed.clone(),
            },
        }
    }

    /// The sledgehammer as `hold` holds it (as a character-less yard
    /// swings it without one), a streak behind its head through the blow,
    /// cracks, dust, and the blows' numbers facing `eye`.
    #[must_use]
    pub fn mesh(&self, player: &PlayerController, eye: Vec3, hold: Option<Hold>) -> Mesh {
        let mut mesh = Mesh::default();
        let fallback = self.track.is_none() || hold.is_none();
        let hold = match hold {
            Some(hold) if !fallback => hold,
            _ => Self::hammer(player, Self::angle(self.swing)),
        };
        sledgehammer(&mut mesh, &hold);
        // The head's streak: where it was over the last few hundredths of
        // a second through the blow.
        let streak: Option<Vec<Hold>> = match (&self.track, self.swing) {
            (Some(track), Some(t))
                if !fallback && t > track.impact - SWEEP[0] && t < track.impact + 0.1 =>
            {
                let root =
                    Mat4::from_rotation_translation(Quat::from_rotation_y(player.yaw), player.pos);
                Some(
                    (0..=6)
                        .map(|i| track.hold(t - 0.09 + 0.015 * i as f32).moved(root))
                        .collect(),
                )
            }
            (None, Some(t)) if t > WIND_UP && t < WIND_UP + STRIKE + 0.08 => {
                let angle = Self::angle(Some(t));
                let tail = Self::angle(Some((t - 0.07).max(WIND_UP)));
                Some(
                    (0..=6)
                        .map(|i| Self::hammer(player, tail + (angle - tail) * i as f32 / 6.0))
                        .collect(),
                )
            }
            _ => None,
        };
        for (i, pair) in streak.iter().flat_map(|s| s.windows(2)).enumerate() {
            let point = |h: &Hold, r: f32| h.grip + h.axis * r;
            let fade = 0.35 + 0.1 * i as f32;
            quad(
                &mut mesh,
                [
                    point(&pair[0], HEAD_AT - 0.18),
                    point(&pair[1], HEAD_AT - 0.18),
                    point(&pair[1], HEAD_AT + 0.1),
                    point(&pair[0], HEAD_AT + 0.1),
                ],
                [0.95 * fade, 0.86 * fade, 0.62 * fade],
            );
        }
        let specs = self.site.specs();
        for (index, (spec, piece)) in specs.iter().zip(self.site.pieces()).enumerate() {
            if piece.status != Status::Broken
                && piece.hit_points < spec.hit_points
                && matches!(spec.role, Role::Wall { .. })
            {
                cracks(&mut mesh, index, spec, piece, self.site.piece_pose(index));
            }
        }
        for puff in self.site.puffs() {
            let x = puff.age / puff.life;
            let size = puff.size * (0.4 + 1.2 * x.sqrt()) * (1.0 - x * x);
            let spin = puff.age * 0.7 + puff.at.x;
            let axes = [
                Vec3::new(spin.cos(), 0.0, spin.sin()),
                Vec3::Y,
                Vec3::new(-spin.sin(), 0.0, spin.cos()),
            ];
            cloud(&mut mesh, puff.at, axes, size * 0.5, puff.color);
        }
        if !self.floaters.is_empty() {
            let mut painter = Painter::new(eye);
            for floater in &self.floaters {
                painter.floater(floater, self.clock);
            }
            mesh.extend(&painter.mesh);
        }
        mesh
    }

    /// The yard's hotbar: whether a swing is under way and how many
    /// pieces are down.
    #[must_use]
    pub fn bar(&self) -> hotbar::Bar {
        let pieces = self.site.pieces();
        hotbar::Bar {
            swinging: self.swing.is_some(),
            down: pieces
                .iter()
                .filter(|p| p.status != Status::Standing)
                .count(),
            total: pieces.len(),
        }
    }
    /// The yard's HUD caption.
    #[must_use]
    pub fn caption(&self) -> String {
        let pieces = self.site.pieces();
        let down = pieces
            .iter()
            .filter(|p| p.status != Status::Standing)
            .count();
        let mut caption = format!(
            "Demolition yard\nClick or press 1 to swing the sledgehammer · R rebuilds\n{down} of {} pieces down",
            pieces.len()
        );
        if let Some(blow) = self.last {
            let spec = &self.site.specs()[blow.piece];
            let name = match spec.role {
                Role::Wall { .. } => "wall",
                Role::Post { .. } => "corner post",
                Role::Roof => "roof",
                Role::Gable { .. } => "gable",
                Role::Chimney => "chimney",
            };
            caption.push_str(&if blow.broke {
                format!(" · {} damage, the {name} breaks", blow.damage)
            } else {
                format!(
                    " · {} damage to the {name}, {} of {} left",
                    blow.damage, blow.hit_points, spec.hit_points
                )
            });
        }
        caption
    }
}

/// Appends the sledgehammer as `hold` holds it: an ash handle with a
/// leather wrap at the grip, an iron collar under the head, and an iron
/// head with steel striking faces.
fn sledgehammer(mesh: &mut Mesh, hold: &Hold) {
    let Hold { grip, axis, face } = *hold;
    let side = axis.cross(face).normalize_or(Vec3::X);
    let head = hold.head();
    prism(mesh, grip - axis * BUTT, head, HANDLE_RADIUS, WOOD);
    prism(
        mesh,
        grip - axis * (BUTT - 0.02),
        grip + axis * (WRAP[1] - BUTT),
        WRAP[0],
        LEATHER,
    );
    // A knob at the butt keeps the hammer from slipping.
    prism(
        mesh,
        grip - axis * (BUTT + 0.02),
        grip - axis * (BUTT - 0.015),
        WRAP[0] + 0.006,
        LEATHER,
    );
    solid(
        mesh,
        head - axis * (HEAD.y + 0.035),
        [side, axis, face],
        Vec3::new(0.034, 0.035, 0.034),
        IRON,
    );
    solid(mesh, head, [side, axis, face], HEAD, IRON);
    for sign in [-1.0, 1.0] {
        solid(
            mesh,
            head + face * sign * (HEAD.z + 0.012),
            [side, axis, face],
            Vec3::new(HEAD.x - 0.01, HEAD.y - 0.01, 0.012),
            STEEL,
        );
    }
}

/// Appends a puff of dust: a shaded octahedron of `radius` about `center`
/// on unit `axes`, flattened a little, so it reads as a cloud rather than
/// a block.
fn cloud(mesh: &mut Mesh, center: Vec3, axes: [Vec3; 3], radius: f32, color: [f32; 3]) {
    let [x, y, z] = [axes[0] * radius, axes[1] * radius * 0.75, axes[2] * radius];
    for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        let (a, b) = (center + x * sx, center + z * sz);
        let (top, bottom) = (center + y, center - y);
        quad(mesh, [a, b, top, top], color);
        quad(mesh, [b, a, bottom, bottom], color);
    }
}

/// Appends an eight-sided shaded rod of `radius` from `a` to `b`.
fn prism(mesh: &mut Mesh, a: Vec3, b: Vec3, radius: f32, color: [f32; 3]) {
    let along = (b - a).normalize_or(Vec3::Y);
    let u = along.any_orthonormal_vector();
    let v = along.cross(u);
    let ring = |center: Vec3, i: usize| {
        let angle = std::f32::consts::TAU * i as f32 / 8.0;
        center + (u * angle.cos() + v * angle.sin()) * radius
    };
    for i in 0..8 {
        quad(
            mesh,
            [ring(a, i), ring(a, i + 1), ring(b, i + 1), ring(b, i)],
            color,
        );
    }
    for i in 1..7 {
        quad(
            mesh,
            [ring(b, 0), ring(b, i), ring(b, i + 1), ring(b, i + 1)],
            color,
        );
        quad(
            mesh,
            [ring(a, 0), ring(a, i + 1), ring(a, i), ring(a, i)],
            color,
        );
    }
}

/// Appends a shaded box centered at `center` with unit `axes` and `half`
/// extents along them.
fn solid(mesh: &mut Mesh, center: Vec3, axes: [Vec3; 3], half: Vec3, color: [f32; 3]) {
    let [x, y, z] = [axes[0] * half.x, axes[1] * half.y, axes[2] * half.z];
    let corner = |i: usize| {
        center
            + if i & 1 == 0 { -x } else { x }
            + if i & 2 == 0 { -y } else { y }
            + if i & 4 == 0 { -z } else { z }
    };
    let c: [Vec3; 8] = std::array::from_fn(corner);
    for [a, b, cc, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        quad(mesh, [c[a], c[b], c[cc], c[d]], color);
    }
}

fn quad(mesh: &mut Mesh, [a, b, c, d]: [Vec3; 4], color: [f32; 3]) {
    let color = shade(color, a, b, c);
    for p in [a, b, c, a, c, d] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

/// A value in `0..1` for `n` in stream `salt`.
fn noise(n: u32, salt: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Dark jagged cracks over both faces of a damaged wall section, more as
/// its hit points fall, the same on every frame.
fn cracks(mesh: &mut Mesh, index: usize, spec: &site::PieceSpec, piece: &site::Piece, pose: Mat4) {
    let damage = 1.0 - piece.hit_points as f32 / spec.hit_points.max(1) as f32;
    let strokes = (damage * 8.0).ceil() as u32;
    let (width, height, depth) = (0.85, 1.35, 0.205);
    let salt = index as u32 * 97 + 13;
    for stroke in 0..strokes {
        let n = |k: u32| noise(stroke * 31 + k, salt) * 2.0 - 1.0;
        let mut at = Vec3::new(n(0) * width, n(1) * height, 0.0);
        let mut heading = n(2) * std::f32::consts::PI;
        for segment in 0..4 {
            heading += n(10 + segment) * 0.9;
            let length = 0.18 + 0.2 * noise(stroke * 31 + 20 + segment, salt);
            let next = at + Vec3::new(heading.cos(), heading.sin(), 0.0) * length;
            let across = Vec3::new(-heading.sin(), heading.cos(), 0.0) * 0.018;
            for face in [depth, -depth] {
                let z = Vec3::Z * face;
                let corners = [
                    at - across + z,
                    next - across + z,
                    next + across + z,
                    at + across + z,
                ]
                .map(|p| pose.transform_point3(p));
                quad(mesh, corners, CRACK);
            }
            at = next;
        }
    }
}

/// A ring of the glade's trees around the yard, for a horizon.
#[must_use]
pub fn trees() -> Vec<super::layout::Placement> {
    const TREES: [&str; 4] = [
        "nature/CommonTree_5",
        "nature/Pine_2",
        "nature/CommonTree_3",
        "nature/Pine_1",
    ];
    (0..18u32)
        .map(|i| {
            let angle = std::f32::consts::TAU * (i as f32 + 0.4 * noise(i, 3)) / 18.0;
            let radius = 40.0 + 8.0 * noise(i, 5);
            super::layout::Placement {
                model: TREES[i as usize % TREES.len()],
                at: [angle.cos() * radius, angle.sin() * radius],
                lift: 0.0,
                yaw: noise(i, 7) * std::f32::consts::TAU,
                scale: 0.9 + 0.3 * noise(i, 11),
                collision: super::layout::Collision::None,
            }
        })
        .collect()
}

/// The yard's blockers as an empty start: no layout, only the ground.
#[must_use]
pub fn solids() -> super::solids::Solids {
    super::solids::Solids::over(super::height)
}
