//! Destructible buildings in Everglade's own town
//! (`docs/verse/destructible-buildings.md`): the demolition yard's rules,
//! pieces, chunks, sledgehammer, and Meteor Swarm, applied to the kit
//! buildings the layout places.
//!
//! The town finds its buildings in the layout: kit wall sections that
//! stand within a section of each other make one building on the 2 m
//! grid, and its corner posts, round-tile roof spans, brick gables,
//! chimney, window glass, shutters, door frames, and door leaves are the
//! kit pieces on its footprint. Each wall section, post, roof span, gable,
//! and chimney becomes a [`super::kit`] piece with its story and roof span,
//! so the yard's support rules hold up storied buildings and wide roofs.
//! The Agent Studio is never touched: a building over the workshop hall,
//! the strongroom, or a station's standing point is protected, and a blow
//! there only says so. Generated whole-model buildings, the open pavilion
//! and bandshell, the arch, the boards, and the furniture aren't kit walls
//! and stay as they are.
//!
//! Destruction is lazy. Every building stays in the zone's merged static
//! cells until a swing or a meteor reaches it. Then it is raised into a
//! [`Site`] as static bodies, at no visible change, and only the pieces
//! that are damaged, loose, or broken leave the static cells: their
//! placements' triangles are made degenerate in the uploaded indices
//! ([`crate::pbr::textured::IndexEdits`]) and their chunks draw in the
//! frame's figure, posed each frame from their bodies. At most
//! [`MAX_LIVE`] buildings are raised at once and at most [`MAX_CHUNKS`]
//! chunks live. A raised building that took no damage goes back to the
//! static cells at once; a damaged one regrows whole after [`REGROW`]
//! seconds of rest with the player away; `R` restores the whole town.
//! While a building is raised, its blockers and roof surface come from its
//! standing pieces, so the player walks through gaps and never stands on a
//! roof that fell.

use super::chunks::{self, ChunkMesh};
use super::hammer::{self, Hammer};
use super::kit::{self, CORNER_TRIM, Draft, SEAM, WALL_TOP};
use super::meteor::{self, Swarm};
use super::site::{Blow, Cuboid, PieceSpec, Role, Side, Site, Status, Target};
use super::{BREAK, HIT, join};
use crate::controller::{Footprint, PlayerController};
use crate::mesh::Mesh;
use crate::pbr::textured::{
    Figure, IndexRange, Primitive, TexturedMesh, TexturedScene, TexturedVertex, UNBAKED,
};
use crate::pbr::textured_bake::AmbientProbes;
use crate::zones::everglade::layout::{self, Placement};
use crate::zones::everglade::player::{Hold, SwingTrack};
use crate::zones::everglade::scene::{Copied, Paint, copy_painted};
use crate::zones::everglade::solids::{self, Roof, Solids};
use crate::zones::everglade::{HALL, STATIONS, STRONGROOM, height};
use crate::zones::everglade_pack::ZonePack;
use crate::zones::grove::draw::{FLOAT, Floater, Painter};
use glam::{DVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Whether this build runs on a browser or a phone, whose budgets are
/// smaller (`docs/verse/destructible-buildings.md`, Performance budgets).
const SMALL: bool = cfg!(any(
    target_arch = "wasm32",
    target_os = "ios",
    target_os = "android"
));
/// Most buildings raised into the rules at once.
pub const MAX_LIVE: usize = if SMALL { 2 } else { 4 };
/// Most chunks alive at once across the town.
pub const MAX_CHUNKS: usize = if SMALL { 96 } else { 220 };
/// Seconds a damaged building rests, with the player at least
/// [`REGROW_DISTANCE`] m away, before it stands whole again.
pub const REGROW: f32 = 60.0;
pub const REGROW_DISTANCE: f32 = 30.0;
/// Seconds a raised building that took no damage waits before it goes
/// back to the static cells.
const SETTLE: f32 = 2.0;
/// Seconds after a hit during which a building is never let go to make
/// room for another.
const FRESH: f32 = 3.0;
/// Seconds the hammer stays in hand after a swing.
const WIELD: f32 = 4.0;
/// How far ahead of the player Meteor Swarm's circle first lands, m.
const AHEAD: f32 = 14.0;
/// How far a kit piece may stand outside its building's wall lines and
/// still belong to it, m.
const NEAR: f32 = 0.6;
/// Half the side of the town's flat ground slab, m: past the clearing.
const GROUND: f64 = 180.0;
/// Seed of the town's dice and debris spread.
const SEED: u64 = 0x70E7_D3B0;
/// The color "Protected" floats in over the studio.
const SHIELD: [f32; 3] = [0.62, 0.8, 1.0];
/// Where a chunk that is gone, or a pool slot nobody uses, is drawn: a
/// point under the ground.
const COLLAPSED: TexturedVertex = TexturedVertex {
    pos: [0.0, -50.0, 0.0],
    normal: [0.0, 1.0, 0.0],
    uv: [0.0, 0.0],
    color: [0, 0, 0, 0],
    light: UNBAKED,
};

/// One kit piece of a town building: its draft, the layout placements
/// drawn with it (its host first), its chunks' shape, and, for a roof
/// span, the surface the player stands on.
#[derive(Clone, Debug)]
pub struct TownPiece {
    pub draft: Draft,
    pub placements: Vec<usize>,
    look: Look,
    roof: Option<Roof>,
}

/// One building of the town.
#[derive(Clone, Debug)]
pub struct Building {
    /// Center and half extents, m.
    pub rect: ([f32; 2], [f32; 2]),
    pub stories: u8,
    /// Part of the Agent Studio.
    pub protected: bool,
    /// Whether its pieces map onto the rules; a building that doesn't
    /// stays as placed.
    pub valid: bool,
    /// The ground under its center, and the highest point of its roof, m.
    pub base: f32,
    pub top: f32,
    pub pieces: Vec<TownPiece>,
    /// What it adds to the solids while it is static.
    blocks: Vec<(Footprint, f32)>,
    roofs: Vec<Roof>,
}

impl Building {
    /// Whether the rules may break it.
    #[must_use]
    pub fn destructible(&self) -> bool {
        self.valid && !self.protected && !self.pieces.is_empty()
    }

    /// The distance from `point` to the building's box, m.
    fn distance(&self, point: Vec3) -> f32 {
        let ([cx, cz], [hx, hz]) = self.rect;
        let dx = ((point.x - cx).abs() - hx - 0.35).max(0.0);
        let dz = ((point.z - cz).abs() - hz - 0.35).max(0.0);
        let dy = (self.base - point.y).max(point.y - self.top).max(0.0);
        Vec3::new(dx, dy, dz).length()
    }

    /// Whether `point` is within `reach` of the building as it stands
    /// whole: inside its walls' footprint and under its roof.
    fn touches(&self, point: Vec3, reach: f32) -> bool {
        let ([cx, cz], [hx, hz]) = self.rect;
        if (point.x - cx).abs() > hx + 0.35 + reach || (point.z - cz).abs() > hz + 0.35 + reach {
            return false;
        }
        let eaves = self.base + WALL_TOP * f32::from(self.stories);
        let roof = self
            .roofs
            .iter()
            .filter_map(|r| surface(r, point.x, point.z))
            .fold(eaves, f32::max);
        point.y >= self.base - reach && point.y <= roof + reach
    }
}

/// The height of `roof`'s surface over `(x, z)`, if it covers that point
/// (`verse_world::social::solids::Roof`).
fn surface(roof: &Roof, x: f32, z: f32) -> Option<f32> {
    let (dx, dz) = (x - roof.center[0], z - roof.center[1]);
    let u = dx * roof.across[0] + dz * roof.across[1];
    let v = -dx * roof.across[1] + dz * roof.across[0];
    (u.abs() <= roof.half[0] && v.abs() <= roof.half[1])
        .then(|| roof.ridge - (roof.ridge - roof.eave) * u.abs() / roof.half[0])
}

/// A building raised into the rules, and when.
#[derive(Clone, Copy, Debug)]
struct Lifted {
    building: usize,
    since: f32,
    /// When it last took damage, s.
    hit: f32,
}

/// The town's buildings and the rules over the raised ones: what the
/// hammer and the spell act on.
struct Wreck {
    buildings: Vec<Building>,
    /// Floors under the buildings that stand above the flat ground, each
    /// a box's center and half extents.
    floors: Vec<(DVec3, DVec3)>,
    site: Site,
    /// For each site piece, its building and its index there.
    refs: Vec<(usize, usize)>,
    lifted: Vec<Lifted>,
    /// Each chunk shape's boxes, in the body frame.
    cuboids: Vec<Vec<Cuboid>>,
    clock: f32,
    /// Where the last blow met a protected building.
    protected_hit: Option<Vec3>,
    /// Bumped when a building is raised or let go.
    revision: u64,
}

impl Wreck {
    fn empty_site(floors: &[(DVec3, DVec3)]) -> Site {
        let mut site = Site::new(Vec::new(), SEED);
        site.set_ground(GROUND, floors.to_vec());
        site.retain(|_| true);
        site.set_max_chunks(MAX_CHUNKS);
        site
    }

    fn lifted(&self, building: usize) -> bool {
        self.lifted.iter().any(|l| l.building == building)
    }

    /// Raises `building` into the rules, letting a quiet one go first when
    /// [`MAX_LIVE`] are up, unless it is in `keep`. Returns whether it is
    /// raised.
    fn lift(&mut self, building: usize, keep: &[usize]) -> bool {
        if self.lifted(building) {
            return true;
        }
        if !self.buildings[building].destructible() {
            return false;
        }
        if self.lifted.len() >= MAX_LIVE {
            let now = self.clock;
            let candidate = self
                .lifted
                .iter()
                .filter(|l| now - l.hit > FRESH && !keep.contains(&l.building))
                .min_by(|a, b| a.hit.total_cmp(&b.hit))
                .map(|l| l.building);
            match candidate {
                Some(old) => self.let_go(old),
                None => return false,
            }
        }
        let b = &self.buildings[building];
        let specs: Vec<PieceSpec> = b
            .pieces
            .iter()
            .map(|p| p.draft.spec(self.cuboids[p.look.shape].clone()))
            .collect();
        let count = specs.len();
        self.site.add(specs);
        self.refs.extend((0..count).map(|k| (building, k)));
        self.lifted.push(Lifted {
            building,
            since: self.clock,
            hit: f32::NEG_INFINITY,
        });
        self.revision += 1;
        true
    }

    /// Takes `building` out of the rules: it stands whole in the static
    /// cells again.
    fn let_go(&mut self, building: usize) {
        self.site.retain(|spec| spec.building != building);
        self.refs.retain(|&(b, _)| b != building);
        self.lifted.retain(|l| l.building != building);
        self.revision += 1;
    }

    /// Raises every destructible building within `reach` of any of
    /// `points`.
    fn lift_near(&mut self, points: &[Vec3], reach: f32) {
        let near: Vec<usize> = (0..self.buildings.len())
            .filter(|&b| self.buildings[b].destructible())
            .filter(|&b| {
                points
                    .iter()
                    .any(|&p| self.buildings[b].distance(p) <= reach)
            })
            .collect();
        for &b in &near {
            self.lift(b, &near);
        }
    }

    /// Notes when the buildings `blows` reached were hit.
    fn note(&mut self, blows: &[Blow]) {
        for blow in blows {
            if let Some(&(building, _)) = self.refs.get(blow.piece)
                && let Some(l) = self.lifted.iter_mut().find(|l| l.building == building)
            {
                l.hit = self.clock;
            }
        }
    }
}

impl Target for Wreck {
    fn touches(&self, point: Vec3, reach: f32) -> bool {
        self.site.touches(point, reach)
            || self
                .buildings
                .iter()
                .enumerate()
                .any(|(b, building)| !self.lifted(b) && building.touches(point, reach))
    }

    fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow> {
        self.lift_near(&[center], radius);
        let blows = self.site.explode(center, radius, damage, speed);
        self.note(&blows);
        blows
    }

    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow> {
        self.lift_near(path, reach + 0.5);
        let blow = self.site.strike(path, push, reach);
        match blow {
            Some(blow) => self.note(&[blow]),
            None => {
                // A blow at the studio only says it is protected.
                self.protected_hit = path.iter().copied().find(|&p| {
                    self.buildings
                        .iter()
                        .any(|b| b.protected && b.distance(p) <= reach)
                });
            }
        }
        blow
    }

    fn roll(&mut self, sides: u32) -> i32 {
        self.site.roll(sides)
    }
}

/// Where one chunk's triangles go in the figure's town vertices.
#[derive(Clone, Copy, Debug)]
struct Span {
    /// The site piece, its chunk, and the chunk's part.
    piece: usize,
    chunk: usize,
    part: usize,
    shape: usize,
    /// The pool material the part draws in.
    material: usize,
    start: usize,
    len: usize,
}

/// How a site piece looks: its chunk shape and its house's paint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Look {
    shape: usize,
    paint: usize,
}

/// The town's figure: its images and materials, one mesh with a primitive
/// per material sized to hold the drawn chunks, and their posed vertices.
struct Pool {
    /// Images and materials, no meshes.
    kit: TexturedScene,
    /// Vertices each material's primitive holds, and where each starts.
    caps: Vec<usize>,
    offsets: Vec<usize>,
    /// Vertices each material used last frame.
    used: Vec<usize>,
    scene: Arc<TexturedScene>,
    posed: Vec<TexturedVertex>,
    spans: Vec<Span>,
    /// The site pieces drawn, in order.
    drawn: Vec<usize>,
}

impl Pool {
    fn new(kit: TexturedScene) -> Self {
        let materials = kit.materials.len();
        let mut pool = Self {
            kit,
            caps: vec![0; materials],
            offsets: vec![0; materials],
            used: vec![0; materials],
            scene: Arc::new(TexturedScene::default()),
            posed: Vec::new(),
            spans: Vec::new(),
            drawn: Vec::new(),
        };
        pool.build();
        pool
    }

    /// Rebuilds the scene for the current capacities.
    fn build(&mut self) {
        let mut scene = self.kit.clone();
        let mut primitives = Vec::new();
        let mut offset = 0;
        for (material, &cap) in self.caps.iter().enumerate() {
            self.offsets[material] = offset;
            offset += cap;
            if cap > 0 {
                primitives.push(Primitive {
                    vertices: vec![COLLAPSED; cap],
                    indices: (0..cap as u32).collect(),
                    material,
                });
            }
        }
        scene.add_mesh(TexturedMesh { primitives });
        self.scene = Arc::new(scene);
        self.posed = vec![COLLAPSED; offset];
        self.used = vec![0; self.caps.len()];
    }

    /// Lays out the chunks of the site pieces `drawn` (by their `looks`),
    /// in the pool materials `materials` holds for each pack material and
    /// paint, growing the capacities when they don't fit.
    fn pack(
        &mut self,
        drawn: Vec<usize>,
        looks: &[Look],
        meshes: &[Vec<ChunkMesh>],
        materials: &BTreeMap<(u16, usize), usize>,
    ) {
        let mut need = vec![0usize; self.caps.len()];
        let mut entries: Vec<(usize, Span)> = Vec::new();
        for &piece in &drawn {
            let Look { shape, paint } = looks[piece];
            for (chunk, mesh) in meshes[shape].iter().enumerate() {
                for (part, (source, vertices)) in mesh.parts.iter().enumerate() {
                    let Some(&material) = materials.get(&(*source, paint)) else {
                        continue;
                    };
                    entries.push((
                        material,
                        Span {
                            piece,
                            chunk,
                            part,
                            shape,
                            material,
                            start: need[material],
                            len: vertices.len(),
                        },
                    ));
                    need[material] += vertices.len();
                }
            }
        }
        if need.iter().zip(&self.caps).any(|(n, c)| n > c) {
            for (cap, &n) in self.caps.iter_mut().zip(&need) {
                if n > *cap {
                    // Whole triangles: `n` is a multiple of three, and so
                    // is the largest multiple of three at or under a cap
                    // at or over it.
                    let grown = n.next_power_of_two().max(2048);
                    *cap = grown - grown % 3;
                }
            }
            self.build();
        }
        self.spans = entries
            .into_iter()
            .map(|(material, mut span)| {
                span.start += self.offsets[material];
                span
            })
            .collect();
        self.drawn = drawn;
    }

    /// Empties the pool back to no capacity.
    fn clear(&mut self) {
        self.caps.iter_mut().for_each(|c| *c = 0);
        self.spans.clear();
        self.drawn.clear();
        self.build();
    }
}

/// Everglade's destructible town: its buildings, the rules over the raised
/// ones, the hammer and the spell, and what they draw.
pub struct Town {
    wreck: Wreck,
    /// Each chunk shape's meshes, in the pack's materials, and the pool
    /// material each pack material takes under each paint.
    meshes: Vec<Vec<ChunkMesh>>,
    materials: BTreeMap<(u16, usize), usize>,
    /// For each site piece, how it looks.
    looks: Vec<Look>,
    pool: Pool,
    /// The zone's static scene, its edits, and where each building
    /// placement's triangles are in it.
    world: Arc<TexturedScene>,
    ranges: BTreeMap<usize, Vec<IndexRange>>,
    /// The pieces whose placements are hidden, by building and piece.
    hidden: BTreeSet<(usize, usize)>,
    /// The solids without any building, and whether they changed.
    base: Solids,
    solids: Option<Solids>,
    seen: (u64, u64),
    /// The character's scene, and the scene of it and the pool together.
    combined: Option<(Arc<TexturedScene>, Arc<TexturedScene>, Arc<TexturedScene>)>,
    hammer: Hammer,
    wield: f32,
    swarm: Swarm,
    floaters: Vec<Floater>,
    clock: f32,
}

impl Town {
    /// The town of `placements` with the models in `pack`, whose static
    /// scene `world` the zone drew from the same placements in order.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn new(
        pack: &ZonePack,
        placements: &[Placement],
        world: Arc<TexturedScene>,
    ) -> Result<Self, String> {
        let surveyed = survey(placements);
        let members: BTreeSet<usize> = surveyed
            .iter()
            .flat_map(|s| s.members.iter().copied())
            .collect();
        // The zone's solids without the buildings' own: everything else
        // the layout places, and the city's blocks off the buildings.
        let others: Vec<Placement> = placements
            .iter()
            .enumerate()
            .filter(|(index, _)| !members.contains(index))
            .map(|(_, placement)| *placement)
            .collect();
        let mut base = solids::build(pack, &others)?;
        let mut city: Vec<Option<(Footprint, f32)>> =
            layout::city::blocks().into_iter().map(Some).collect();
        let mut taken: Vec<(Footprint, f32)> = Vec::new();
        // Chunk shapes, cut once each and shared by every piece like it,
        // and each house's paint.
        let mut kit = TexturedScene::default();
        let mut copied = Copied::default();
        let mut keys: BTreeMap<String, usize> = BTreeMap::new();
        let mut meshes: Vec<Vec<ChunkMesh>> = Vec::new();
        let mut cuboids: Vec<Vec<Cuboid>> = Vec::new();
        let mut paints: Vec<Paint> = Vec::new();
        let mut materials: BTreeMap<(u16, usize), usize> = BTreeMap::new();
        let mut buildings = Vec::with_capacity(surveyed.len());
        for (index, s) in surveyed.iter().enumerate() {
            let mut building = s.build(index, placements);
            // The city's wall runs on this building's lines are its own.
            for slot in &mut city {
                let Some((f, top)) = *slot else {
                    continue;
                };
                let center = [(f.min[0] + f.max[0]) * 0.5, (f.min[1] + f.max[1]) * 0.5];
                if within(s.rect, center, NEAR) {
                    building.blocks.push((f, top));
                    taken.push((f, top));
                    *slot = None;
                }
            }
            for &member in &s.members {
                let (blocks, roof) = solids::of_placement(pack, &placements[member])?;
                building.blocks.extend(blocks);
                building.roofs.extend(roof);
            }
            if building.destructible() {
                for piece in &mut building.pieces {
                    let key = piece.draft.shape();
                    let shape = match keys.get(&key) {
                        Some(&shape) => shape,
                        None => {
                            let cut = chunks::cut(pack, &piece.draft)?;
                            cuboids.push(cut.iter().filter_map(|m| m.cuboid).collect());
                            meshes.push(cut);
                            keys.insert(key, meshes.len() - 1);
                            meshes.len() - 1
                        }
                    };
                    let host = piece.placements[0];
                    let colors = layout::paint(&placements[host]);
                    let paint = match paints.iter().position(|p| *p == colors) {
                        Some(paint) => paint,
                        None => {
                            paints.push(colors);
                            paints.len() - 1
                        }
                    };
                    for mesh in &meshes[shape] {
                        for (source, _) in &mesh.parts {
                            if let std::collections::btree_map::Entry::Vacant(entry) =
                                materials.entry((*source, paint))
                            {
                                entry.insert(copy_painted(
                                    pack,
                                    *source,
                                    &mut kit,
                                    &mut copied,
                                    colors,
                                )?);
                            }
                        }
                    }
                    piece.look = Look { shape, paint };
                    if matches!(piece.draft.role, Role::Roof { .. }) {
                        piece.roof = solids::of_placement(pack, &placements[host])?.1;
                    }
                }
            }
            buildings.push(building);
        }
        // The buildings' wall runs come and go with them.
        base.retain_blocks(|footprint, top| {
            !taken
                .iter()
                .any(|(f, t)| f == footprint && t.to_bits() == top.to_bits())
        });
        let all = world.index_ranges();
        // Raised ground under buildings on the hill, so their debris lands
        // on it rather than at the flat ground's height.
        let floors: Vec<(DVec3, DVec3)> = buildings
            .iter()
            .filter(|b| b.destructible() && b.base > 0.05)
            .map(|b| {
                let ([cx, cz], [hx, hz]) = b.rect;
                let base = f64::from(b.base);
                (
                    DVec3::new(f64::from(cx), (base - 0.5) * 0.5, f64::from(cz)),
                    DVec3::new(f64::from(hx) + 6.0, (base + 0.5) * 0.5, f64::from(hz) + 6.0),
                )
            })
            .collect();
        let ranges = buildings
            .iter()
            .filter(|b| b.destructible())
            .flat_map(|b| b.pieces.iter().flat_map(|p| p.placements.iter().copied()))
            .filter_map(|p| Some((p, all.get(p)?.clone())))
            .collect();
        let mut town = Self {
            wreck: Wreck {
                buildings,
                site: Wreck::empty_site(&floors),
                floors,
                refs: Vec::new(),
                lifted: Vec::new(),
                cuboids,
                clock: 0.0,
                protected_hit: None,
                revision: 0,
            },
            meshes,
            materials,
            looks: Vec::new(),
            pool: Pool::new(kit),
            world,
            ranges,
            hidden: BTreeSet::new(),
            base,
            solids: None,
            seen: (u64::MAX, u64::MAX),
            combined: None,
            hammer: Hammer::default(),
            wield: 0.0,
            swarm: Swarm::default(),
            floaters: Vec::new(),
            clock: 0.0,
        };
        town.refresh_solids();
        Ok(town)
    }

    /// The town's buildings.
    #[must_use]
    pub fn buildings(&self) -> &[Building] {
        &self.wreck.buildings
    }

    /// The rules over the raised buildings.
    #[must_use]
    pub fn site(&self) -> &Site {
        &self.wreck.site
    }

    /// The raised buildings, in the order they were raised.
    #[must_use]
    pub fn raised(&self) -> Vec<usize> {
        self.wreck.lifted.iter().map(|l| l.building).collect()
    }

    /// The building and piece each site piece is.
    #[must_use]
    pub fn refs(&self) -> &[(usize, usize)] {
        &self.wreck.refs
    }

    /// How many pieces' placements are out of the static cells.
    #[must_use]
    pub fn hidden(&self) -> usize {
        self.hidden.len()
    }

    /// Swings with the player's character's chop `track` from now on.
    pub fn set_track(&mut self, track: Option<SwingTrack>) {
        self.hammer.set_track(track);
    }

    /// Takes up the sledgehammer and swings it, unless a swing or a cast
    /// is under way, leaving Meteor Swarm's targeting. Returns whether it
    /// swung.
    pub fn swing(&mut self) -> bool {
        if self.hammer.swinging() || self.swarm.casting() {
            return false;
        }
        self.swarm.cancel();
        self.wield = WIELD;
        self.hammer.start()
    }

    /// Seconds into the character's chop while a swing plays it.
    #[must_use]
    pub fn chop(&self) -> Option<f32> {
        self.hammer.chop()
    }

    /// Enters Meteor Swarm's targeting with the circle ahead of `player`,
    /// or leaves it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now.
    pub fn meteor_swarm(&mut self, player: &PlayerController) -> Result<(), String> {
        if self.hammer.swinging() {
            return Err("Finish the swing first".into());
        }
        self.swarm.target()?;
        if self.swarm.targeting() && self.swarm.aim().is_none() {
            self.swarm.aim_ahead(player, AHEAD);
        }
        Ok(())
    }

    /// Meteor Swarm's state.
    #[must_use]
    pub fn swarm(&self) -> &Swarm {
        &self.swarm
    }

    /// Puts Meteor Swarm's circle on the ground the ray from `origin`
    /// along `direction` meets, within range of `player`. Returns whether
    /// the circle moved.
    pub fn aim(&mut self, origin: Vec3, direction: Vec3, player: &PlayerController) -> bool {
        if !self.swarm.targeting() {
            return false;
        }
        match meteor::ground_hit(origin, direction) {
            Some(ground) => {
                self.swarm.aim_at(ground, player);
                true
            }
            None => false,
        }
    }

    /// Casts Meteor Swarm at its circle. Returns whether the cast began.
    pub fn confirm(&mut self, player: &PlayerController) -> bool {
        self.swarm.confirm(player)
    }

    /// Leaves Meteor Swarm's targeting or stops its cast.
    pub fn cancel(&mut self) {
        self.swarm.cancel();
    }

    /// Where the camera is this frame, shaken by the meteors' blasts.
    #[must_use]
    pub fn shake(&self) -> Vec3 {
        self.swarm.shake()
    }

    /// Whether the hammer is in hand or swinging, and Meteor Swarm's state.
    #[must_use]
    pub fn bar(&self) -> (bool, meteor::Status) {
        (
            self.hammer.swinging() || self.wield > 0.0,
            self.swarm.status(),
        )
    }

    /// Stands every building whole in the static cells again and ends the
    /// spell's meteors and marks.
    pub fn restore(&mut self) {
        let lifted: Vec<usize> = self.wreck.lifted.iter().map(|l| l.building).collect();
        for building in lifted {
            self.wreck.let_go(building);
        }
        self.wreck.site = Wreck::empty_site(&self.wreck.floors);
        self.wreck.refs.clear();
        self.swarm.reset();
        self.floaters.clear();
        self.sync();
    }

    /// Advances the hammer, the spell, the rules, and what draws; lets go
    /// of buildings that rest; and notes when the solids change. `player`
    /// is the player's controller.
    pub fn tick(&mut self, dt: f32, player: &PlayerController) {
        self.clock += dt;
        self.wreck.clock = self.clock;
        self.wield = (self.wield - dt).max(0.0);
        if let Some(path) = self.hammer.advance(dt, player) {
            self.wield = WIELD;
            match self.wreck.strike(&path, player.forward(), hammer::REACH) {
                Some(blow) => self.floaters.push(Floater {
                    at: blow.at + Vec3::Y * 0.6 - player.forward() * 0.45,
                    text: blow.damage.to_string(),
                    color: if blow.broke { BREAK } else { HIT },
                    start: self.clock,
                }),
                None => {
                    if let Some(at) = self.wreck.protected_hit.take() {
                        self.floaters.push(Floater {
                            at: at + Vec3::Y * 0.6,
                            text: "Protected".into(),
                            color: SHIELD,
                            start: self.clock,
                        });
                    }
                }
            }
        }
        let mut blows = self.swarm.tick(dt, player, &mut self.wreck);
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
        self.wreck.site.tick(dt);
        self.settle(player);
        self.sync();
        self.pose();
    }

    /// Lets go of raised buildings that took no damage, and regrows
    /// damaged ones that rest with the player away.
    fn settle(&mut self, player: &PlayerController) {
        let now = self.clock;
        let site = &self.wreck.site;
        let mut done = Vec::new();
        for l in &self.wreck.lifted {
            let pieces = || {
                site.specs()
                    .iter()
                    .zip(site.pieces())
                    .filter(move |(s, _)| s.building == l.building)
            };
            let whole =
                pieces().all(|(s, p)| p.status == Status::Standing && p.hit_points == s.hit_points);
            let quiet = pieces().all(|(_, p)| match p.status {
                Status::Standing => true,
                Status::Loose => false,
                Status::Broken => p.chunks.iter().all(|c| c.gone),
            });
            let away = self.wreck.buildings[l.building].distance(player.pos) > REGROW_DISTANCE;
            let rested = now - l.hit.max(l.since);
            if (whole && rested > SETTLE)
                || (quiet && away && rested > REGROW)
                || (away && rested > 2.0 * REGROW)
            {
                done.push(l.building);
            }
        }
        for building in done {
            self.wreck.let_go(building);
        }
    }

    /// Brings the drawn pieces, the hidden placements, and the solids up
    /// to the rules.
    fn sync(&mut self) {
        let site = &self.wreck.site;
        // How each site piece looks.
        self.looks = self
            .wreck
            .refs
            .iter()
            .map(|&(b, k)| self.wreck.buildings[b].pieces[k].look)
            .collect();
        let drawn: Vec<usize> = site
            .specs()
            .iter()
            .zip(site.pieces())
            .enumerate()
            .filter(|(_, (s, p))| p.status != Status::Standing || p.hit_points < s.hit_points)
            .map(|(i, _)| i)
            .collect();
        let hidden: BTreeSet<(usize, usize)> = drawn.iter().map(|&i| self.wreck.refs[i]).collect();
        if hidden != self.hidden {
            let edits = &self.world.edits;
            for &(b, k) in self.hidden.symmetric_difference(&hidden) {
                let hide = hidden.contains(&(b, k));
                for &placement in &self.wreck.buildings[b].pieces[k].placements {
                    for range in self.ranges.get(&placement).into_iter().flatten() {
                        let indices = if hide {
                            vec![range.base; range.count as usize]
                        } else {
                            self.world.range_indices(placement, range)
                        };
                        edits.write(range.first, indices);
                    }
                }
            }
            self.hidden = hidden;
        }
        if drawn != self.pool.drawn || self.pool.spans.len() != self.count_spans(&drawn) {
            if drawn.is_empty() {
                self.pool.clear();
            } else {
                self.pool
                    .pack(drawn, &self.looks, &self.meshes, &self.materials);
            }
        }
        let seen = (self.wreck.revision, self.wreck.site.revision());
        if seen != self.seen {
            self.seen = seen;
            self.refresh_solids();
        }
    }

    /// How many spans the site pieces `drawn` lay out.
    fn count_spans(&self, drawn: &[usize]) -> usize {
        drawn
            .iter()
            .map(|&i| {
                self.meshes[self.looks[i].shape]
                    .iter()
                    .map(|m| m.parts.len())
                    .sum::<usize>()
            })
            .sum()
    }

    /// The solids: everything but the buildings, each static building's
    /// blocks and roofs, and each raised building's standing pieces.
    fn refresh_solids(&mut self) {
        let mut solids = self.base.clone();
        for (b, building) in self.wreck.buildings.iter().enumerate() {
            if self.wreck.lifted(b) {
                continue;
            }
            for &(footprint, top) in &building.blocks {
                solids.add_block(footprint, top);
            }
            for &roof in &building.roofs {
                solids.add_roof(roof);
            }
        }
        let site = &self.wreck.site;
        for (footprint, top) in site.blocks() {
            solids.add_block(footprint, top);
        }
        for (i, piece) in site.pieces().iter().enumerate() {
            let (b, k) = self.wreck.refs[i];
            if piece.status == Status::Standing
                && let Some(roof) = self.wreck.buildings[b].pieces[k].roof
            {
                solids.add_roof(roof);
            }
        }
        self.solids = Some(solids);
    }

    /// The solids when they changed since the last call.
    pub fn take_solids(&mut self) -> Option<Solids> {
        self.solids.take()
    }

    /// Poses every drawn chunk's triangles for this frame.
    fn pose(&mut self) {
        let site = &self.wreck.site;
        let pool = &mut self.pool;
        let mut used = vec![0usize; pool.caps.len()];
        for span in &pool.spans {
            let piece = &site.pieces()[span.piece];
            let spec = &site.specs()[span.piece];
            let (transform, shade) = match piece.status {
                Status::Broken => (site.chunk_pose(span.piece, span.chunk), 0.8),
                _ => {
                    let damage = 1.0 - piece.hit_points as f32 / spec.hit_points.max(1) as f32;
                    let frame = spec.chunks.get(span.chunk).map(Cuboid::frame);
                    (
                        frame.map(|f| site.piece_pose(span.piece) * f),
                        1.0 - 0.45 * damage,
                    )
                }
            };
            let (_, source) = &self.meshes[span.shape][span.chunk].parts[span.part];
            let out = &mut pool.posed[span.start..span.start + span.len];
            match transform {
                Some(m) => {
                    for (o, v) in out.iter_mut().zip(source) {
                        *o = TexturedVertex {
                            pos: m.transform_point3(Vec3::from(v.pos)).to_array(),
                            normal: m.transform_vector3(Vec3::from(v.normal)).to_array(),
                            color: [
                                (f32::from(v.color[0]) * shade) as u8,
                                (f32::from(v.color[1]) * shade) as u8,
                                (f32::from(v.color[2]) * shade) as u8,
                                v.color[3],
                            ],
                            ..*v
                        };
                    }
                }
                None => out.fill(COLLAPSED),
            }
            let material = span.material;
            used[material] = used[material].max(span.start + span.len - pool.offsets[material]);
        }
        // What the last frame used past this frame's spans collapses.
        for material in 0..pool.caps.len() {
            let from = pool.offsets[material] + used[material];
            let to = pool.offsets[material] + pool.used[material].min(pool.caps[material]);
            if to > from {
                pool.posed[from..to].fill(COLLAPSED);
            }
        }
        pool.used = used;
    }

    /// Joins the town's chunks to the character's `scene` once per
    /// distinct pair of scenes.
    pub fn prepare(&mut self, scene: Option<&Arc<TexturedScene>>) {
        let Some(scene) = scene else {
            self.combined = None;
            return;
        };
        if self.pool.spans.is_empty() {
            return;
        }
        if self.combined.as_ref().is_some_and(|(cast, own, _)| {
            Arc::ptr_eq(cast, scene) && Arc::ptr_eq(own, &self.pool.scene)
        }) {
            return;
        }
        let joined = join(scene, &self.pool.scene);
        self.combined = Some((scene.clone(), self.pool.scene.clone(), Arc::new(joined)));
    }

    /// The frame's figure: the character's `cast` figure followed by the
    /// town's drawn chunks, lit by `probes` when the bake has them.
    #[must_use]
    pub fn figure(&self, cast: Figure, probes: Option<&AmbientProbes>) -> Figure {
        if self.pool.spans.is_empty() {
            return cast;
        }
        match &self.combined {
            Some((scene, own, joined))
                if Arc::ptr_eq(scene, &cast.scene) && Arc::ptr_eq(own, &self.pool.scene) =>
            {
                let mut vertices = Vec::with_capacity(cast.vertices.len() + self.pool.posed.len());
                vertices.extend_from_slice(&cast.vertices);
                let start = vertices.len();
                vertices.extend_from_slice(&self.pool.posed);
                if let Some(probes) = probes {
                    for (material, &used) in self.pool.used.iter().enumerate() {
                        let from = start + self.pool.offsets[material];
                        probes.shade(&mut vertices[from..from + used]);
                    }
                }
                Figure {
                    scene: joined.clone(),
                    vertices: Arc::new(vertices),
                }
            }
            _ => cast,
        }
    }

    /// The town's chunks alone as a figure, for a zone without a
    /// character.
    #[must_use]
    pub fn own_figure(&self) -> Option<Figure> {
        (!self.pool.spans.is_empty()).then(|| Figure {
            scene: self.pool.scene.clone(),
            vertices: Arc::new(self.pool.posed.clone()),
        })
    }

    /// The hammer in hand as `hold` holds it, cracks on damaged walls,
    /// dust, the spell, and the blows' numbers facing `eye`.
    #[must_use]
    pub fn mesh(&self, player: &PlayerController, eye: Vec3, hold: Option<Hold>) -> Mesh {
        let mut mesh = Mesh::default();
        if self.hammer.swinging() || self.wield > 0.0 {
            self.hammer.draw(&mut mesh, player, hold);
        }
        let site = &self.wreck.site;
        for (index, (spec, piece)) in site.specs().iter().zip(site.pieces()).enumerate() {
            if piece.status != Status::Broken
                && piece.hit_points < spec.hit_points
                && matches!(spec.role, Role::Wall { .. })
            {
                let (b, k) = self.wreck.refs[index];
                let salt = (b * 997 + k) as u32;
                hammer::cracks(
                    &mut mesh,
                    salt,
                    spec,
                    piece.hit_points,
                    site.piece_pose(index),
                );
            }
        }
        hammer::dust(&mut mesh, site.puffs());
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
}

/// A building as the survey found it: its footprint, its stories, and the
/// layout placements that are its kit pieces.
#[derive(Clone, Debug)]
struct Surveyed {
    rect: ([f32; 2], [f32; 2]),
    stories: u8,
    walls: Vec<usize>,
    members: Vec<usize>,
}

/// Whether `point` lies within `rect` grown by `margin`.
fn within(rect: ([f32; 2], [f32; 2]), point: [f32; 2], margin: f32) -> bool {
    let ([cx, cz], [hx, hz]) = rect;
    (point[0] - cx).abs() <= hx + margin && (point[1] - cz).abs() <= hz + margin
}

/// The kit models that are part of a building besides its walls.
const PARTS: [&str; 9] = [
    "village/Corner_Exterior_Wood",
    "village/Roof_RoundTiles_8x10",
    layout::HOUSE_ROOF,
    "village/Roof_Front_Brick8",
    "village/Prop_Chimney",
    "village/Window_",
    "village/WindowShutters_",
    "village/DoorFrame_",
    "village/Door_",
];

/// The buildings among `placements`: kit wall sections within a section's
/// width of each other are one building, and the kit parts on its
/// footprint belong to it.
fn survey(placements: &[Placement]) -> Vec<Surveyed> {
    let walls: Vec<usize> = placements
        .iter()
        .enumerate()
        .filter(|(_, p)| kit::is_wall(p.model))
        .map(|(i, _)| i)
        .collect();
    // Union-find over a 4 m grid of wall centers.
    let mut parent: Vec<usize> = (0..walls.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let cell = |p: &Placement| {
        (
            (p.at[0] / 4.0).floor() as i32,
            (p.at[1] / 4.0).floor() as i32,
        )
    };
    let mut grid: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (w, &i) in walls.iter().enumerate() {
        grid.entry(cell(&placements[i])).or_default().push(w);
    }
    for (w, &i) in walls.iter().enumerate() {
        let p = &placements[i];
        let (cx, cz) = cell(p);
        for dx in -1..=1 {
            for dz in -1..=1 {
                for &o in grid.get(&(cx + dx, cz + dz)).into_iter().flatten() {
                    let q = &placements[walls[o]];
                    if o != w && joined(p, q) {
                        let (a, b) = (find(&mut parent, w), find(&mut parent, o));
                        if a != b {
                            parent[a.max(b)] = a.min(b);
                        }
                    }
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for w in 0..walls.len() {
        let root = find(&mut parent, w);
        groups.entry(root).or_default().push(walls[w]);
    }
    let mut out: Vec<Surveyed> = groups
        .into_values()
        .map(|walls| {
            let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
            let mut stories = 1;
            for &i in &walls {
                let p = &placements[i];
                min = [min[0].min(p.at[0]), min[1].min(p.at[1])];
                max = [max[0].max(p.at[0]), max[1].max(p.at[1])];
                stories = stories.max(story(p.lift) + 1);
            }
            Surveyed {
                rect: (
                    [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5],
                    [(max[0] - min[0]) * 0.5, (max[1] - min[1]) * 0.5],
                ),
                stories,
                members: walls.clone(),
                walls,
            }
        })
        .collect();
    for (index, p) in placements.iter().enumerate() {
        if !PARTS.iter().any(|part| p.model.starts_with(part)) {
            continue;
        }
        if let Some(s) = out.iter_mut().find(|s| within(s.rect, p.at, NEAR)) {
            s.members.push(index);
        }
    }
    for s in &mut out {
        s.members.sort_unstable();
    }
    out
}

/// Whether wall sections `p` and `q` are neighbors in one building: one
/// over the other, side by side along one line, or meeting at a corner.
/// Two buildings' walls that face each other across a narrow gap are not.
fn joined(p: &Placement, q: &Placement) -> bool {
    let d = [q.at[0] - p.at[0], q.at[1] - p.at[1]];
    let distance = d[0].hypot(d[1]);
    let turn = q.yaw - p.yaw;
    let parallel = turn.sin().abs() < 0.05;
    if distance < 0.05 {
        return parallel && turn.cos() > 0.0;
    }
    // The line a section runs along: its model's +x axis.
    let along = [p.yaw.cos(), -p.yaw.sin()];
    let ahead = d[0] * along[0] + d[1] * along[1];
    let across = d[0] * along[1] - d[1] * along[0];
    let side_by_side = parallel && across.abs() < 0.05 && (ahead.abs() - 2.0).abs() < 0.05;
    let corner = turn.cos().abs() < 0.05 && (distance - std::f32::consts::SQRT_2).abs() < 0.05;
    side_by_side || corner
}

/// The story a piece `lift` meters up stands on.
fn story(lift: f32) -> u8 {
    (lift / WALL_TOP).round().clamp(0.0, 15.0) as u8
}

impl Surveyed {
    /// The building, its pieces mapped onto the rules when its walls fit
    /// them; its solids are filled in by the caller.
    fn build(&self, index: usize, placements: &[Placement]) -> Building {
        let ([cx, cz], [hx, hz]) = self.rect;
        let (west, east, south, north) = (cx - hx, cx + hx, cz - hz, cz + hz);
        let near = |a: f32, b: f32| (a - b).abs() < 0.1;
        let protected = [HALL, STRONGROOM]
            .iter()
            .any(|&(c, h)| overlaps(self.rect, (c, h)))
            || STATIONS.iter().any(|s| within(self.rect, s.at, 0.5));
        let base = height(cx, cz);
        let mut building = Building {
            rect: self.rect,
            stories: self.stories,
            protected,
            valid: false,
            base,
            top: base + WALL_TOP * f32::from(self.stories) + 6.0,
            pieces: Vec::new(),
            blocks: Vec::new(),
            roofs: Vec::new(),
        };
        // Walls on the 2 m grid, each on a line of its footprint.
        let side_of = |at: [f32; 2]| {
            if near(at[1], south) {
                Some(Side::South)
            } else if near(at[1], north) {
                Some(Side::North)
            } else if near(at[0], west) {
                Some(Side::West)
            } else if near(at[0], east) {
                Some(Side::East)
            } else {
                None
            }
        };
        let width = (hx).round() as i32;
        let depth = (hz).round() as i32;
        let grid = |half: f32| (half - half.round()).abs() < 0.05 && half >= 1.0;
        if !grid(hx) || !grid(hz) {
            return building;
        }
        let mut seen = BTreeSet::new();
        let mut pieces = Vec::new();
        let mut dressing_used = BTreeSet::new();
        for &w in &self.walls {
            let p = &placements[w];
            let Some(side) = side_of(p.at) else {
                return building;
            };
            let (along, count) = match side {
                Side::South | Side::North => (p.at[0] - west, width),
                Side::West | Side::East => (p.at[1] - south, depth),
            };
            let index = ((along - 1.0) / 2.0).round() as i32;
            let story = story(p.lift);
            if index < 0 || index >= count || !seen.insert((side as u8, index, story)) {
                return building;
            }
            let reversed = matches!(side, Side::South | Side::East);
            let (first, last) = (index == 0, index + 1 == count);
            let (low, high) = if reversed {
                (last, first)
            } else {
                (first, last)
            };
            let host = p.transform();
            let to_host = host.inverse();
            let mut dressing = Vec::new();
            let mut drawn = vec![w];
            for &m in &self.members {
                let d = &placements[m];
                let dressing_model = [
                    "village/Window_",
                    "village/WindowShutters_",
                    "village/DoorFrame_",
                ]
                .iter()
                .any(|part| d.model.starts_with(part));
                if dressing_model
                    && (d.at[0] - p.at[0]).abs() < 0.05
                    && (d.at[1] - p.at[1]).abs() < 0.05
                    && (d.lift - p.lift).abs() < 0.05
                    && dressing_used.insert(m)
                {
                    dressing.push((d.model, to_host * d.transform()));
                    drawn.push(m);
                }
            }
            let draft = kit::wall(
                0,
                Role::Wall {
                    side,
                    index: index as u8,
                    count: count as u8,
                    story,
                },
                p.model,
                dressing,
                host,
                [
                    if low { CORNER_TRIM } else { SEAM },
                    if high { CORNER_TRIM } else { SEAM },
                ],
            );
            pieces.push((draft, drawn));
        }
        // Every line of every story is whole.
        let expected = 2 * (width + depth) as usize * usize::from(self.stories);
        if seen.len() != expected {
            return building;
        }
        // Door leaves hang in the nearest doorway of the ground story.
        for &m in &self.members {
            let d = &placements[m];
            if !d.model.starts_with("village/Door_") {
                continue;
            }
            let nearest = pieces
                .iter()
                .enumerate()
                .filter(|(_, (draft, _))| {
                    draft.models[0].0 == "village/Wall_Plaster_Door_Round"
                        && matches!(draft.role, Role::Wall { story: 0, .. })
                })
                .map(|(i, (draft, _))| {
                    let at = draft.placement.w_axis;
                    (i, (at.x - d.at[0]).hypot(at.z - d.at[1]))
                })
                .filter(|&(_, distance)| distance < 1.8)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = nearest {
                let (draft, drawn) = &mut pieces[i];
                let to_host = draft.placement.inverse();
                draft.models.push((d.model, to_host * d.transform()));
                drawn.push(m);
            }
        }
        let spans = ((2.0 * hx / 8.0).round() as i32).max(1);
        let span_of = |x: f32| (((x - west) / 8.0).floor() as i32).clamp(0, spans - 1) as u8;
        let mut roofs = BTreeSet::new();
        for &m in &self.members {
            let d = &placements[m];
            let role = if d.model == "village/Corner_Exterior_Wood" {
                let a = if near(d.at[1], south) {
                    Side::South
                } else if near(d.at[1], north) {
                    Side::North
                } else {
                    continue;
                };
                let b = if near(d.at[0], west) {
                    Side::West
                } else if near(d.at[0], east) {
                    Side::East
                } else {
                    continue;
                };
                Role::Post {
                    a,
                    b,
                    story: story(d.lift),
                }
            } else if d.model.starts_with("village/Roof_RoundTiles")
                || d.model == layout::HOUSE_ROOF
            {
                // The kit's roof runs its ridge north and south.
                if d.yaw.sin().abs() > 0.01 {
                    return building;
                }
                let span = span_of(d.at[0]);
                if !roofs.insert(span) {
                    return building;
                }
                Role::Roof {
                    span,
                    spans: spans as u8,
                }
            } else if d.model == "village/Roof_Front_Brick8" {
                let side = if (d.at[1] - south).abs() < (d.at[1] - north).abs() {
                    Side::South
                } else {
                    Side::North
                };
                Role::Gable {
                    side,
                    span: span_of(d.at[0]),
                }
            } else if d.model == "village/Prop_Chimney" {
                Role::Chimney {
                    span: span_of(d.at[0]),
                }
            } else {
                continue;
            };
            let placement = d.transform();
            let draft = match role {
                Role::Post { .. } => kit::post(0, role, placement),
                Role::Roof { .. } => kit::roof_of(d.model, 0, role, placement),
                Role::Gable { .. } => kit::gable(0, role, placement),
                _ => kit::chimney(0, role, placement),
            };
            pieces.push((draft, vec![m]));
        }
        if roofs.len() != spans as usize {
            return building;
        }
        building.valid = true;
        building.pieces = pieces
            .into_iter()
            .map(|(mut draft, placements)| {
                draft.building = index;
                TownPiece {
                    draft,
                    placements,
                    look: Look { shape: 0, paint: 0 },
                    roof: None,
                }
            })
            .collect();
        building
    }
}

/// Whether two rectangles, each a center and half extents, overlap.
fn overlaps(a: ([f32; 2], [f32; 2]), b: ([f32; 2], [f32; 2])) -> bool {
    (a.0[0] - b.0[0]).abs() < a.1[0] + b.1[0] && (a.0[1] - b.0[1]).abs() < a.1[1] + b.1[1]
}
