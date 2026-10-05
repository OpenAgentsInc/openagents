//! The demolition yard: a standalone demo of destructible buildings
//! (`docs/verse/destructible-buildings.md`, phase D1). `verse --demolition`
//! opens Everglade's ground and sky with none of its layout, and two kit
//! cottages ([`cottage`]) in the clearing. The player swings a sledgehammer
//! (left click or `1`); a struck piece darkens and cracks, breaks into its
//! chunks ([`chunks`]) at zero hit points, and what it held up drops,
//! leans, and crashes ([`site`]). `R` rebuilds the cottages.
//!
//! The cottages are not in the zone's merged static cells. Every piece
//! draws as part of the frame's one textured figure, after the player's
//! character: its chunks' triangles are posed on the CPU each frame from
//! the piece's body while it is whole and from each chunk's body once it
//! breaks. No light is baked into them.

pub mod chunks;
pub mod cottage;
pub mod site;
#[cfg(test)]
mod tests;

use super::draw::shade;
use super::scene::{Copied, copy_material};
use crate::controller::{Footprint, PlayerController};
use crate::mesh::{Mesh, Vertex};
use crate::pbr::textured::{Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex};
use crate::zones::everglade_pack::ZonePack;
use glam::{Mat4, Vec3};
use site::{Blow, Role, Site, Status};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Seed of the yard's dice and debris spread.
const SEED: u64 = 0x5EED_D3B0;
/// A swing: wind-up, strike, and recovery, s.
const WIND_UP: f32 = 0.22;
const STRIKE: f32 = 0.14;
const SWING: f32 = 0.72;
/// The hammer's angle from straight up toward the facing, radians: at
/// rest over the shoulder, drawn back, and at the end of the strike.
const REST: f32 = -0.6;
const DRAWN: f32 = -1.9;
const FOLLOW: f32 = 2.0;
/// How far into the strike the blow lands.
const HIT_AT: f32 = 0.8;
/// How far from the head's path a piece is struck, m.
const REACH: f32 = 0.5;
/// Handle length past the grip and the head's size, m.
const HANDLE: f32 = 1.0;
const HEAD: Vec3 = Vec3::new(0.1, 0.1, 0.24);
const WOOD: [f32; 3] = [0.55, 0.36, 0.18];
const IRON: [f32; 3] = [0.10, 0.10, 0.11];
const CRACK: [f32; 3] = [0.035, 0.03, 0.028];

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
    /// Time into the current swing, s.
    swing: Option<f32>,
    struck: bool,
    last: Option<Blow>,
    misses: u32,
    revision: Option<u64>,
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
            struck: false,
            last: None,
            misses: 0,
            revision: None,
        };
        demolition.pose();
        Ok(demolition)
    }

    #[cfg(test)]
    pub fn site(&self) -> &Site {
        &self.site
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

    /// Rebuilds both cottages.
    pub fn reset(&mut self) {
        self.site.reset();
        self.last = None;
        self.misses = 0;
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

    /// The grip, the handle's direction, and the head's swing direction for
    /// a hammer at `angle` held by `player`.
    fn hammer(player: &PlayerController, angle: f32) -> (Vec3, Vec3, Vec3) {
        let forward = player.forward();
        let right = forward.cross(Vec3::Y);
        let grip = player.pos + Vec3::Y * 1.25 + right * 0.28 + forward * 0.15;
        let along = Vec3::Y * angle.cos() + forward * angle.sin();
        let swing = -Vec3::Y * angle.sin() + forward * angle.cos();
        (grip, along, swing)
    }

    /// Advances the swing, the hammer's blow, and the yard's bodies.
    pub fn tick(&mut self, dt: f32, player: &PlayerController) {
        if let Some(t) = &mut self.swing {
            *t += dt;
            let t = *t;
            if !self.struck && t >= WIND_UP + STRIKE * HIT_AT {
                self.struck = true;
                let path: Vec<Vec3> = (0..=8)
                    .map(|i| {
                        let a = 0.6 + (FOLLOW - 0.6) * i as f32 / 8.0;
                        let (grip, along, _) = Self::hammer(player, a);
                        grip + along * (HANDLE - 0.05)
                    })
                    .collect();
                match self.site.strike(&path, player.forward(), REACH) {
                    Some(blow) => self.last = Some(blow),
                    None => self.misses += 1,
                }
            }
            if t >= SWING {
                self.swing = None;
            }
        }
        self.site.tick(dt);
        self.pose();
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

    /// The hammer, the broken faces of the chunks, cracks, and dust.
    #[must_use]
    pub fn mesh(&self, player: &PlayerController) -> Mesh {
        let mut mesh = Mesh::default();
        // The hammer, and a streak behind its head through the strike.
        let angle = Self::angle(self.swing);
        let (grip, along, swing) = Self::hammer(player, angle);
        let right = along.cross(swing);
        let handle = grip + along * (HANDLE * 0.5 - 0.1);
        solid(
            &mut mesh,
            handle,
            [right, along, swing],
            Vec3::new(0.035, HANDLE * 0.5 + 0.1, 0.035),
            WOOD,
        );
        let head = grip + along * (HANDLE - 0.05);
        solid(&mut mesh, head, [right, along, swing], HEAD, IRON);
        if let Some(t) = self.swing
            && t > WIND_UP
            && t < WIND_UP + STRIKE + 0.08
        {
            let tail = Self::angle(Some((t - 0.07).max(WIND_UP)));
            for i in 0..6 {
                let a = tail + (angle - tail) * i as f32 / 6.0;
                let b = tail + (angle - tail) * (i + 1) as f32 / 6.0;
                let point = |a: f32, r: f32| {
                    let (grip, along, _) = Self::hammer(player, a);
                    grip + along * r
                };
                let fade = 0.35 + 0.1 * i as f32;
                let color = [0.9 * fade, 0.88 * fade, 0.82 * fade];
                quad(
                    &mut mesh,
                    [
                        point(a, HANDLE - 0.25),
                        point(b, HANDLE - 0.25),
                        point(b, HANDLE + 0.1),
                        point(a, HANDLE + 0.1),
                    ],
                    color,
                );
            }
        }
        let specs = self.site.specs();
        for (index, (spec, piece)) in specs.iter().zip(self.site.pieces()).enumerate() {
            match piece.status {
                // A wall section's or post's box fits its chunk, so it shows
                // as the broken core; a roof's, gable's, or chimney's box
                // is much larger than its tiles or bricks and stays unseen.
                Status::Broken if matches!(spec.role, Role::Wall { .. } | Role::Post { .. }) => {
                    let inside = spec.matter.interior();
                    for (i, cuboid) in spec.chunks.iter().enumerate() {
                        let Some(pose) = self.site.chunk_pose(index, i) else {
                            continue;
                        };
                        let axes = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| pose.transform_vector3(a));
                        solid(
                            &mut mesh,
                            pose.transform_point3(Vec3::ZERO),
                            axes,
                            cuboid.half.as_vec3() * 0.96,
                            inside,
                        );
                    }
                }
                _ if piece.hit_points < spec.hit_points
                    && matches!(spec.role, Role::Wall { .. }) =>
                {
                    cracks(&mut mesh, index, spec, piece, self.site.piece_pose(index));
                }
                _ => {}
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
            solid(
                &mut mesh,
                puff.at,
                axes,
                Vec3::splat(size * 0.5),
                puff.color,
            );
        }
        mesh
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
