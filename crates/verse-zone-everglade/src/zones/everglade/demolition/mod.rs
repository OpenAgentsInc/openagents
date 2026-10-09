//! The demolition yard: a standalone demo of destructible buildings
//! (`docs/verse/destructible-buildings.md`, phase D1). `verse --demolition`
//! opens Everglade's ground and sky with none of its layout, and two kit
//! cottages ([`cottage`]) in the clearing. The player's character holds a
//! sledgehammer in its right hand and swings it with both (left click or
//! `1`), playing the pack's two-handed chop; the blow lands at the chop's
//! impact. A struck piece darkens and cracks, shows the damage as a
//! floating number, breaks into its chunks ([`chunks`]) at zero hit
//! points, and what it held up drops, leans, and crashes ([`site`]). `2`
//! aims Meteor Swarm at a circle of ground and calls it down on the
//! cottages ([`meteor`]). `R` rebuilds the cottages and refills the mana.
//! The yard's [`hotbar`] replaces the zone panel.
//!
//! The cottages are not in the zone's merged static cells. Every piece
//! draws as part of the frame's one textured figure, after the player's
//! character: its chunks' triangles are posed on the CPU each frame from
//! the piece's body while it is whole and from each chunk's body once it
//! breaks. No light is baked into them.
//!
//! Everglade's own town uses the same rules, pieces, hammer, and spell
//! ([`town`]), raising a building into the rules only when something
//! first reaches it.

pub mod carve;
#[cfg(test)]
mod carve_tests;
pub mod chunks;
pub mod cottage;
pub mod hammer;
pub mod hotbar;
pub mod instanced;
pub mod kit;
pub mod meteor;
pub mod site;
#[cfg(test)]
mod tests;
pub mod town;
#[cfg(test)]
mod town_tests;

use super::player::{Hold, SwingTrack};
use super::scene::{Copied, copy_material};
use crate::controller::{Footprint, PlayerController};
use crate::mesh::Mesh;
use crate::pbr::textured::{Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex};
use crate::zones::everglade::floaters::{FLOAT, Floater, Painter};
use crate::zones::everglade_pack::ZonePack;
use glam::Vec3;
use hammer::{Hammer, noise};
use site::{Blow, Role, Site, Status};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Seed of the yard's dice and debris spread.
const SEED: u64 = 0x5EED_D3B0;
/// The colors a blow's number floats in, as the Grove's damage numbers
/// do: gold for a hit, and fire orange for one that breaks the piece.
pub const HIT: [f32; 3] = [1.0, 0.74, 0.22];
pub const BREAK: [f32; 3] = [1.0, 0.42, 0.12];

/// Where one chunk's triangles are in the figure's demolition vertices.
#[derive(Clone, Copy, Debug)]
struct Span {
    piece: usize,
    chunk: usize,
    start: usize,
    len: usize,
}

/// The yard's live state and the meshes it draws.
pub struct Demolition {
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
    /// The sledgehammer's swing.
    hammer: Hammer,
    last: Option<Blow>,
    misses: u32,
    revision: Option<u64>,
    /// Seconds since the yard opened, and the blows' numbers in the air.
    clock: f32,
    floaters: Vec<Floater>,
    /// Meteor Swarm: its mana, targeting, cast, and meteors.
    swarm: meteor::Swarm,
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
            hammer: Hammer::default(),
            last: None,
            misses: 0,
            revision: None,
            clock: 0.0,
            floaters: Vec::new(),
            swarm: meteor::Swarm::default(),
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
        self.hammer.set_track(track);
    }

    /// Starts a swing unless one or a cast is under way, leaving Meteor
    /// Swarm's targeting. Returns whether it started.
    pub fn swing(&mut self) -> bool {
        if self.hammer.swinging() || self.swarm.casting() {
            return false;
        }
        self.swarm.cancel();
        self.hammer.start()
    }

    /// Seconds into the character's chop while a swing plays it.
    #[must_use]
    pub fn chop(&self) -> Option<f32> {
        self.hammer.chop()
    }

    /// Enters Meteor Swarm's targeting, or leaves it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now.
    pub fn meteor_swarm(&mut self) -> Result<(), String> {
        if self.hammer.swinging() {
            return Err("Finish the swing first".into());
        }
        self.swarm.target()
    }

    /// Meteor Swarm's state.
    #[must_use]
    pub fn swarm(&self) -> &meteor::Swarm {
        &self.swarm
    }

    /// Puts Meteor Swarm's circle on the first surface the ray from
    /// `origin` along `direction` meets, within range of `player`: the
    /// ground, a roof, or a cottage's wall, inside or out. Returns whether
    /// the circle moved.
    pub fn aim(&mut self, origin: Vec3, direction: Vec3, player: &PlayerController) -> bool {
        if !self.swarm.targeting() {
            return false;
        }
        let site = &self.site;
        match meteor::surface_aim(origin, direction, &|from, to| {
            site::Target::ray(site, from, to)
        }) {
            Some(aim) => {
                self.swarm.aim_on(aim, player);
                true
            }
            None => false,
        }
    }

    /// Casts Meteor Swarm at its circle. Returns whether the cast began.
    pub fn confirm(&mut self, player: &PlayerController) -> bool {
        self.swarm.confirm(player)
    }

    /// Leaves Meteor Swarm's targeting or stops its cast, spending
    /// nothing.
    pub fn cancel(&mut self) {
        self.swarm.cancel();
    }

    /// Where the camera is this frame, shaken by the meteors' blasts.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        self.swarm.shake()
    }

    /// Rebuilds both cottages and refills the mana, clearing Meteor
    /// Swarm's cooldown, as a demo's rest does.
    pub fn reset(&mut self) {
        self.swarm.reset();
        self.site.reset();
        self.last = None;
        self.misses = 0;
        self.floaters.clear();
        self.pose();
    }

    /// Advances the swing, the hammer's blow, the numbers, and the yard's
    /// bodies.
    pub fn tick(&mut self, dt: f32, player: &PlayerController) {
        self.clock += dt;
        if let Some(path) = self.hammer.advance(dt, player) {
            match self.site.strike(&path, player.forward(), hammer::REACH) {
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
        let mut blows = self.swarm.tick(dt, player, &mut self.site);
        // The hardest hits float their numbers; a strike reaches too many
        // pieces to number them all.
        blows.sort_by(|a, b| b.damage.cmp(&a.damage));
        for blow in blows.iter().take(8) {
            self.floaters.push(Floater {
                at: blow.at + Vec3::Y * 0.8,
                text: blow.damage.to_string(),
                color: if blow.broke { BREAK } else { HIT },
                start: self.clock,
            });
        }
        let now = self.clock;
        self.floaters.retain(|f| now - f.start < FLOAT);
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
        self.combined = Some((scene.clone(), Arc::new(join(scene, &self.scene))));
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
        self.hammer.draw(&mut mesh, player, hold);
        let specs = self.site.specs();
        for (index, (spec, piece)) in specs.iter().zip(self.site.pieces()).enumerate() {
            if piece.status != Status::Broken
                && piece.hit_points < spec.hit_points
                && matches!(spec.role, Role::Wall { .. })
            {
                hammer::cracks(
                    &mut mesh,
                    index as u32,
                    spec,
                    piece.hit_points,
                    self.site.piece_pose(index),
                );
            }
        }
        hammer::dust(&mut mesh, self.site.puffs());
        self.swarm.draw(&mut mesh, eye);
        if !self.floaters.is_empty() {
            let mut painter = Painter::new(eye);
            for floater in &self.floaters {
                painter.floater(floater, self.clock);
            }
            mesh.extend(&painter.mesh);
        }
        mesh
    }

    /// The yard's hotbar: whether a swing is under way, Meteor Swarm's
    /// state, and how many pieces are down.
    #[must_use]
    pub fn bar(&self) -> hotbar::Bar {
        let pieces = self.site.pieces();
        hotbar::Bar {
            swinging: self.hammer.swinging(),
            swarm: self.swarm.status(),
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
            "Demolition yard\n{}\n{down} of {} pieces down",
            hotbar::help(&self.swarm.status()),
            pieces.len()
        );
        if let Some(blow) = self.last {
            let spec = &self.site.specs()[blow.piece];
            let name = piece_name(spec.role);
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

/// The character's figure `scene` with `own`'s images, materials, and one
/// mesh's primitives after its own, as one figure scene.
pub fn join(scene: &TexturedScene, own: &TexturedScene) -> TexturedScene {
    let mut joined = scene.clone();
    let images = joined.images.len();
    let materials = joined.materials.len();
    joined.images.extend(own.images.iter().cloned());
    for material in &own.materials {
        let mut material = material.clone();
        material.image = material.image.map(|i| i + images);
        joined.materials.push(material);
    }
    if let Some(mesh) = joined.meshes.first_mut() {
        for primitive in own.meshes.iter().flat_map(|m| &m.primitives) {
            let mut primitive = primitive.clone();
            primitive.material += materials;
            mesh.primitives.push(primitive);
        }
    }
    joined
}

/// What a piece with `role` is called in a caption.
#[must_use]
pub fn piece_name(role: Role) -> &'static str {
    match role {
        Role::Wall { .. } => "wall",
        Role::Post { .. } => "corner post",
        Role::Roof { .. } => "roof",
        Role::Gable { .. } => "gable",
        Role::Chimney { .. } => "chimney",
        Role::Block { .. } => "block",
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
