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
//!
//! Everything else the town places that isn't ground, water, or a plant is
//! carved ([`super::carve`]): the generated buildings and landmarks, the
//! open pavilion and bandshell, the stalls, the studio's furniture, and
//! fences and props. Carved placements whose bounds touch make one
//! building, each of its blocks a piece that rests on the blocks under it
//! or is carried a short way by the blocks beside it. The Agent Studio is
//! no exception: its hall, strongroom, and stations break like the rest
//! and come back with the town.
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
//! standing pieces, and a carved building's columns from its standing
//! blocks, so the player walks through gaps and never stands on a roof
//! that fell.

use super::carve::{self, Lattice, Split};
use super::chunks::{self, ChunkMesh};
use super::hammer::{self, Hammer};
use super::kit::{self, CORNER_TRIM, Draft, SEAM, WALL_TOP};
use super::meteor::{self, Strike, Swarm};
use super::site::{Blow, Cuboid, Link, Matter, PieceSpec, Role, Side, Site, Status, Target};
use super::{BREAK, HIT, join};
use crate::controller::{Footprint, PlayerController};
use crate::mesh::Mesh;
use crate::pbr::textured::{
    Figure, IndexRange, Primitive, TexturedMesh, TexturedScene, TexturedVertex, UNBAKED,
};
use crate::pbr::textured_bake::AmbientProbes;
use crate::zones::everglade::height;
use crate::zones::everglade::layout::{self, Placement};
use crate::zones::everglade::player::{Hold, SwingTrack};
use crate::zones::everglade::scene::{Copied, Paint, copy_painted};
use crate::zones::everglade::solids::{self, Roof, Solids};
use crate::zones::everglade_pack::ZonePack;
use crate::zones::grove::draw::{FLOAT, Floater, Painter};
use glam::{DVec3, Mat4, Quat, Vec3};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use verse_world::social::columns::Columns;

/// Whether this build runs on a browser or a phone, whose budgets are
/// smaller (`docs/verse/destructible-buildings.md`, Performance budgets).
const SMALL: bool = cfg!(any(
    target_arch = "wasm32",
    target_os = "ios",
    target_os = "android"
));
/// Most buildings raised into the rules at once.
pub const MAX_LIVE: usize = if SMALL { 6 } else { 12 };
/// Most pieces of the raised buildings at once: a big carved hall has a
/// hundred blocks.
pub const MAX_PIECES: usize = if SMALL { 240 } else { 520 };
/// Most chunks alive at once across the town.
pub const MAX_CHUNKS: usize = if SMALL { 128 } else { 260 };
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
/// How close two carved placements' bounds come to make one building, m.
const TOUCH: f32 = 0.05;
/// How near the player a carved building is cut into chunks ahead of a
/// strike, m, and how many blocks a frame.
const WARM: f32 = 50.0;
const WARM_BLOCKS: usize = 2;
/// How far above the ground a carved block's underside may be and still
/// stand on it, m.
const FOOTING: f32 = 0.5;
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
    /// What holds a carved block up.
    link: Link,
    /// A carved block's placement and lattice cell.
    pub carve: Option<(usize, u32)>,
    /// Whether its chunks are cut; a kit piece's always are.
    cut: bool,
}

/// One building of the town.
#[derive(Clone, Debug)]
pub struct Building {
    /// Center and half extents, m.
    pub rect: ([f32; 2], [f32; 2]),
    pub stories: u8,
    /// Whether its pieces map onto the rules; a building that doesn't
    /// stays as placed.
    pub valid: bool,
    /// Its carved placements, each with its columns, lattice, and split
    /// model; empty for a kit building.
    pub carved: Vec<Carved>,
    /// Whether its carved blocks' chunks are cut.
    ready: bool,
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
        self.valid && !self.pieces.is_empty()
    }

    /// Whether it is carved rather than built of kit pieces.
    #[must_use]
    pub fn is_carved(&self) -> bool {
        !self.carved.is_empty()
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
        if self.is_carved() {
            // Under the top of its solid columns near the point.
            return self.carved.iter().any(|c| {
                [
                    [0.0, 0.0],
                    [reach, 0.0],
                    [-reach, 0.0],
                    [0.0, reach],
                    [0.0, -reach],
                ]
                .iter()
                .flat_map(|[dx, dz]| c.columns.spans_at(point.x + dx, point.z + dz))
                .any(|s| point.y <= s.hi + reach && point.y >= s.lo - reach)
            });
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

/// One carved placement of a building.
#[derive(Clone)]
pub struct Carved {
    pub placement: usize,
    pub columns: Arc<Columns>,
    pub lattice: Lattice,
    split: Arc<Split>,
    /// Each cell's piece in the building, by cell number.
    cells: BTreeMap<u32, usize>,
}

impl std::fmt::Debug for Carved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Carved")
            .field("placement", &self.placement)
            .field("lattice", &self.lattice)
            .field("cells", &self.cells.len())
            .finish_non_exhaustive()
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
    /// Each chunk shape's boxes, in the body frame, and its meshes.
    cuboids: Vec<Vec<Cuboid>>,
    meshes: Vec<Vec<ChunkMesh>>,
    /// The placements, for cutting carved blocks.
    placements: Vec<crate::zones::everglade::layout::Placement>,
    clock: f32,
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

    /// The pieces of the raised buildings.
    fn live_pieces(&self) -> usize {
        self.lifted
            .iter()
            .map(|l| self.buildings[l.building].pieces.len())
            .sum()
    }

    /// Cuts a carved building's blocks into chunks the first time it is
    /// raised: each block's chunk shape, and its colliders, which are its
    /// chunks' boxes.
    fn prepare(&mut self, building: usize) {
        self.prepare_some(building, usize::MAX);
    }

    /// Cuts up to `budget` of a carved building's blocks that aren't cut
    /// yet, so the town can cut the buildings near the player a few blocks
    /// a frame before anything strikes them.
    fn prepare_some(&mut self, building: usize, budget: usize) {
        if self.buildings[building].ready {
            return;
        }
        let b = &self.buildings[building];
        let mut cut: Vec<(usize, Vec<ChunkMesh>)> = Vec::new();
        for (k, piece) in b.pieces.iter().enumerate() {
            if cut.len() >= budget {
                break;
            }
            let Some((placement, cell)) = piece.carve else {
                continue;
            };
            if piece.cut {
                continue;
            }
            let Some(carved) = b.carved.iter().find(|c| c.placement == placement) else {
                continue;
            };
            let meshes = carve::cut_cell(
                &carved.split,
                &self.placements[placement],
                cell,
                piece.draft.placement,
            );
            cut.push((k, meshes));
        }
        for (k, meshes) in cut {
            let boxes: Vec<Cuboid> = meshes.iter().filter_map(|m| m.cuboid).collect();
            self.cuboids.push(boxes.clone());
            self.meshes.push(meshes);
            let shape = self.meshes.len() - 1;
            let piece = &mut self.buildings[building].pieces[k];
            piece.look.shape = shape;
            piece.cut = true;
            // Colliders a little inside the chunks' boxes, so a block's
            // neighbors don't start touching it.
            piece.draft.colliders = boxes
                .iter()
                .map(|c| Cuboid {
                    half: (c.half - DVec3::splat(0.01)).max(DVec3::splat(0.02)),
                    ..*c
                })
                .collect();
        }
        let b = &mut self.buildings[building];
        b.ready = b.pieces.iter().all(|p| p.cut);
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
        let size = self.buildings[building].pieces.len();
        while self.lifted.len() >= MAX_LIVE || self.live_pieces() + size > MAX_PIECES {
            if self.lifted.is_empty() {
                return false;
            }
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
        self.prepare(building);
        let b = &self.buildings[building];
        let specs: Vec<PieceSpec> = b
            .pieces
            .iter()
            .map(|p| {
                let mut spec = p.draft.spec(self.cuboids[p.look.shape].clone());
                if p.carve.is_some() {
                    spec.link = p.link.clone();
                    spec.blocks = false;
                }
                spec
            })
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
        self.explode_facing(center, radius, damage, speed, Vec3::ZERO)
    }

    fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        self.lift_near(&[center], radius);
        let blows = self
            .site
            .explode_facing(center, radius, damage, speed, face);
        self.note(&blows);
        blows
    }

    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow> {
        self.lift_near(path, reach + 0.5);
        let blow = self.site.strike(path, push, reach);
        if let Some(blow) = blow {
            self.note(&[blow]);
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
    /// The pool material each pack material takes under each paint.
    materials: BTreeMap<(u16, usize), usize>,
    /// For each site piece, how it looks.
    looks: Vec<Look>,
    pool: Pool,
    /// The zone's static scene, its edits, and where each building
    /// placement's triangles are in it: the scene placements that draw it,
    /// its own and its far level of detail's, with their ranges.
    world: Arc<TexturedScene>,
    ranges: BTreeMap<usize, Vec<(usize, IndexRange)>>,
    /// Each carved scene mesh's triangles' cells, by primitive.
    cells: BTreeMap<usize, Arc<Vec<Vec<u32>>>>,
    /// The pieces whose placements are hidden, by building and piece.
    hidden: BTreeSet<(usize, usize)>,
    /// The solids without any building, the solids now, and whether they
    /// changed.
    base: Solids,
    current: Solids,
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
        Self::build(pack, placements, world, true)
    }

    /// The destructible models among `placements` outside Everglade's
    /// town, such as the Grove's tower: no city walls, boards, ponds, or
    /// footbridge, only the placements' own solids.
    ///
    /// # Errors
    ///
    /// Returns a message when the pack lacks a placed model.
    pub fn standalone(
        pack: &ZonePack,
        placements: &[Placement],
        world: Arc<TexturedScene>,
    ) -> Result<Self, String> {
        Self::build(pack, placements, world, false)
    }

    fn build(
        pack: &ZonePack,
        placements: &[Placement],
        world: Arc<TexturedScene>,
        everglade: bool,
    ) -> Result<Self, String> {
        let surveyed: Vec<Surveyed> = survey(placements)
            .into_iter()
            .enumerate()
            .filter(|(i, s)| s.build(*i, placements).valid)
            .map(|(_, s)| s)
            .collect();
        let carved_set = carve::carved(placements);
        let mut members: BTreeSet<usize> = surveyed
            .iter()
            .flat_map(|s| s.members.iter().copied())
            .collect();
        members.extend((0..placements.len()).filter(|&i| carved_set[i]));
        // The zone's solids without the buildings' own: everything else
        // the layout places, and the city's blocks off the buildings.
        let others: Vec<Placement> = placements
            .iter()
            .enumerate()
            .filter(|(index, _)| !members.contains(index))
            .map(|(_, placement)| *placement)
            .collect();
        let mut base = if everglade {
            solids::build(pack, &others)?
        } else {
            solids::build_with(pack, &others, &[])?
        };
        let mut city: Vec<Option<(Footprint, f32)>> = if everglade {
            layout::city::kit_blocks().into_iter().map(Some).collect()
        } else {
            Vec::new()
        };
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
        let paint_of =
            |colors: Paint, paints: &mut Vec<Paint>| match paints.iter().position(|p| *p == colors)
            {
                Some(paint) => paint,
                None => {
                    paints.push(colors);
                    paints.len() - 1
                }
            };
        for s in &surveyed {
            let index = buildings.len();
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
                let paint = paint_of(colors, &mut paints);
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
            buildings.push(building);
        }
        // The carved buildings: carved placements whose bounds touch.
        for group in groups(pack, placements, &carved_set)? {
            let index = buildings.len();
            let building = carved_building(pack, placements, index, &group)?;
            // City wall runs inside it stand in for its own walls.
            for slot in &mut city {
                if let Some((f, top)) = *slot {
                    let center = [(f.min[0] + f.max[0]) * 0.5, (f.min[1] + f.max[1]) * 0.5];
                    if within(building.rect, center, NEAR) {
                        taken.push((f, top));
                        *slot = None;
                    }
                }
            }
            // Every pack material its placements draw in, painted as the
            // scene paints it.
            for c in &building.carved {
                let colors = layout::paint(&placements[c.placement]);
                let paint = paint_of(colors, &mut paints);
                let used: BTreeSet<u16> = c.split.triangles.iter().map(|t| t.1).collect();
                for source in used {
                    if let std::collections::btree_map::Entry::Vacant(entry) =
                        materials.entry((source, paint))
                    {
                        entry.insert(copy_painted(pack, source, &mut kit, &mut copied, colors)?);
                    }
                }
            }
            let mut building = building;
            for piece in &mut building.pieces {
                if let Some((placement, _)) = piece.carve {
                    piece.look.paint = paint_of(layout::paint(&placements[placement]), &mut paints);
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
            .filter(|b| b.destructible() && b.base > 0.05 && b.rect.1[0].max(b.rect.1[1]) > 1.5)
            .map(|b| {
                let ([cx, cz], [hx, hz]) = b.rect;
                let base = f64::from(b.base);
                (
                    DVec3::new(f64::from(cx), (base - 0.5) * 0.5, f64::from(cz)),
                    DVec3::new(f64::from(hx) + 6.0, (base + 0.5) * 0.5, f64::from(hz) + 6.0),
                )
            })
            .collect();
        let fars = crate::zones::everglade::detail::far_placements(pack, placements);
        let ranges = buildings
            .iter()
            .filter(|b| b.destructible())
            .flat_map(|b| b.pieces.iter().flat_map(|p| p.placements.iter().copied()))
            .collect::<BTreeSet<usize>>()
            .into_iter()
            .map(|p| {
                let drawn = std::iter::once(p).chain(fars.get(p).copied().flatten().map(|f| f.1));
                let ranges: Vec<(usize, IndexRange)> = drawn
                    .filter_map(|q| Some(all.get(q)?.iter().map(move |r| (q, *r))))
                    .flatten()
                    .collect();
                (p, ranges)
            })
            .collect::<BTreeMap<usize, Vec<(usize, IndexRange)>>>();
        // Each carved scene mesh's triangle cells, near and far, on its
        // placement's lattice.
        let mut cells: BTreeMap<usize, Arc<Vec<Vec<u32>>>> = BTreeMap::new();
        for b in &buildings {
            for c in &b.carved {
                for (q, _) in ranges.get(&c.placement).into_iter().flatten() {
                    let Some(mesh) = world.placements.get(*q).map(|p| p.mesh) else {
                        continue;
                    };
                    cells.entry(mesh).or_insert_with(|| {
                        Arc::new(
                            world.meshes[mesh]
                                .primitives
                                .iter()
                                .map(|primitive| {
                                    primitive
                                        .indices
                                        .chunks_exact(3)
                                        .map(|t| {
                                            c.lattice.cell_of(
                                                &[0, 1, 2]
                                                    .map(|k| primitive.vertices[t[k] as usize]),
                                            )
                                        })
                                        .collect()
                                })
                                .collect(),
                        )
                    });
                }
            }
        }
        let mut town = Self {
            wreck: Wreck {
                buildings,
                site: Wreck::empty_site(&floors),
                floors,
                refs: Vec::new(),
                lifted: Vec::new(),
                cuboids,
                meshes,
                placements: placements.to_vec(),
                clock: 0.0,
                revision: 0,
            },
            materials,
            looks: Vec::new(),
            pool: Pool::new(kit),
            world,
            ranges,
            cells,
            hidden: BTreeSet::new(),
            current: base.clone(),
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
        self.target(Strike::Meteors, player)
    }

    /// Enters `strike`'s targeting with the circle ahead of `player`,
    /// switches to it, or leaves it.
    ///
    /// # Errors
    ///
    /// Returns why the spell can't be cast now.
    pub fn target(&mut self, strike: Strike, player: &PlayerController) -> Result<(), String> {
        if self.hammer.swinging() {
            return Err("Finish the swing first".into());
        }
        self.swarm.target_with(strike)?;
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
        let solids = &self.current;
        match meteor::surface_aim(origin, direction, &|x, z| solids.top(x, z)) {
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

    /// Where the spell's strikes landed since the last call.
    pub fn take_impacts(&mut self) -> Vec<meteor::Impact> {
        self.swarm.take_impacts()
    }

    /// An explosion of `damage` within `radius` of `center` from another
    /// spell, such as a lightning bolt: it chips what it reaches and lets
    /// tall buildings topple. Returns the blows.
    pub fn blast(&mut self, center: Vec3, radius: f32, damage: i32, face: Vec3) -> Vec<Blow> {
        let blows = self
            .wreck
            .explode_facing(center, radius, damage, 10.0, face);
        self.number(&blows);
        blows
    }

    /// Floats the hardest of `blows`' numbers.
    fn number(&mut self, blows: &[Blow]) {
        let mut blows = blows.to_vec();
        blows.sort_by(|a, b| b.damage.cmp(&a.damage));
        for blow in blows.iter().take(8) {
            self.floaters.push(Floater {
                at: blow.at + Vec3::Y * 0.8,
                text: blow.damage.to_string(),
                color: if blow.broke { BREAK } else { HIT },
                start: self.clock,
            });
        }
    }

    /// Whether `point` is within `reach` of a building something can break.
    #[must_use]
    pub fn touches(&self, point: Vec3, reach: f32) -> bool {
        self.wreck.touches(point, reach)
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
            if let Some(blow) = self.wreck.strike(&path, player.forward(), hammer::REACH) {
                self.floaters.push(Floater {
                    at: blow.at + Vec3::Y * 0.6 - player.forward() * 0.45,
                    text: blow.damage.to_string(),
                    color: if blow.broke { BREAK } else { HIT },
                    start: self.clock,
                });
            }
        }
        let blows = self.swarm.tick(dt, player, &mut self.wreck);
        self.number(&blows);
        let now = self.clock;
        self.floaters.retain(|f| now - f.start < FLOAT);
        self.wreck.site.tick(dt);
        // A toppled top striking the ground: dust, a blast of debris, and
        // a jolt.
        for crash in self.wreck.site.take_crashes() {
            self.swarm.burst("tower_crash", crash.at);
            self.swarm.quake(0.6 + 0.4 * crash.size);
        }

        self.warm(player);
        self.settle(player);
        self.sync();
        self.pose();
    }

    /// Cuts a few blocks of the nearest carved building within [`WARM`] m
    /// of `player` that isn't cut yet, so a strike there raises it without
    /// a pause.
    fn warm(&mut self, player: &PlayerController) {
        let nearest = self
            .wreck
            .buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.ready && b.destructible())
            .map(|(i, b)| (i, b.distance(player.pos)))
            .filter(|&(_, d)| d < WARM)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((building, _)) = nearest {
            self.wreck.prepare_some(building, WARM_BLOCKS);
        }
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
            // A carved placement's ranges are rewritten whole, with the
            // triangles of each of its hidden cells made degenerate.
            let mut carved: BTreeSet<usize> = BTreeSet::new();
            for &(b, k) in self.hidden.symmetric_difference(&hidden) {
                let piece = &self.wreck.buildings[b].pieces[k];
                if let Some((placement, _)) = piece.carve {
                    carved.insert(placement);
                    continue;
                }
                let hide = hidden.contains(&(b, k));
                for &placement in &piece.placements {
                    for (drawn, range) in self.ranges.get(&placement).into_iter().flatten() {
                        let indices = if hide {
                            vec![range.base; range.count as usize]
                        } else {
                            self.world.range_indices(*drawn, range)
                        };
                        edits.write(range.first, indices);
                    }
                }
            }
            let gone: BTreeSet<(usize, u32)> = hidden
                .iter()
                .filter_map(|&(b, k)| self.wreck.buildings[b].pieces[k].carve)
                .collect();
            for placement in carved {
                for (drawn, range) in self.ranges.get(&placement).into_iter().flatten() {
                    let mut indices = self.world.range_indices(*drawn, range);
                    let mesh = self.world.placements[*drawn].mesh;
                    if let Some(cells) = self.cells.get(&mesh).and_then(|c| c.get(range.primitive))
                    {
                        for (t, cell) in cells.iter().enumerate() {
                            if gone.contains(&(placement, *cell))
                                && let Some(tri) = indices.get_mut(3 * t..3 * t + 3)
                            {
                                tri.fill(range.base);
                            }
                        }
                    }
                    edits.write(range.first, indices);
                }
            }
            self.hidden = hidden;
        }
        if drawn != self.pool.drawn || self.pool.spans.len() != self.count_spans(&drawn) {
            if drawn.is_empty() {
                self.pool.clear();
            } else {
                self.pool
                    .pack(drawn, &self.looks, &self.wreck.meshes, &self.materials);
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
                self.wreck.meshes[self.looks[i].shape]
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
        // A raised carved building's standing blocks, by placement and cell.
        let mut standing: BTreeMap<usize, Vec<bool>> = BTreeMap::new();
        for (i, piece) in self.wreck.site.pieces().iter().enumerate() {
            let (b, k) = self.wreck.refs[i];
            if let Some((placement, cell)) = self.wreck.buildings[b].pieces[k].carve {
                let count = self.wreck.buildings[b]
                    .carved
                    .iter()
                    .find(|c| c.placement == placement)
                    .map_or(0, |c| c.lattice.count());
                let cells = standing
                    .entry(placement)
                    .or_insert_with(|| vec![true; count]);
                if let Some(slot) = cells.get_mut(cell as usize) {
                    *slot = piece.status == Status::Standing;
                }
            }
        }
        for (b, building) in self.wreck.buildings.iter().enumerate() {
            let lifted = self.wreck.lifted(b);
            for c in &building.carved {
                let mask = if lifted {
                    standing.remove(&c.placement).map(Arc::new)
                } else {
                    None
                };
                solids.add_columns(c.columns.clone(), mask);
            }
            if lifted {
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
        self.current = solids.clone();
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
            let (_, source) = &self.wreck.meshes[span.shape][span.chunk].parts[span.part];
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
        let solids = &self.current;
        self.swarm
            .draw_over(&mut mesh, eye, &|x, z| solids.top(x, z));
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

/// The kit models that are part of a building besides its walls, with the
/// foliage that hangs on its walls ([`WALL_DRESSING`]).
const PARTS: [&str; 12] = [
    "village/Corner_Exterior_Wood",
    "village/Roof_RoundTiles_8x10",
    layout::HOUSE_ROOF,
    "village/Roof_Front_Brick8",
    "village/Prop_Chimney",
    "village/Window_",
    "village/WindowShutters_",
    "village/DoorFrame_",
    "village/Door_",
    WALL_DRESSING[3],
    WALL_DRESSING[4],
    WALL_DRESSING[5],
];

/// Models placed where a wall piece stands, which break with it: windows,
/// shutters, and door frames, and the ivy, climbing roses, and window
/// boxes `layout::foliage` hangs on the walls.
const WALL_DRESSING: [&str; 6] = [
    "village/Window_",
    "village/WindowShutters_",
    "village/DoorFrame_",
    "foliage/ivy_wall",
    "foliage/rose_trellis",
    "foliage/window_box",
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
        let base = height(cx, cz);
        let mut building = Building {
            rect: self.rect,
            stories: self.stories,
            valid: false,
            carved: Vec::new(),
            ready: true,
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
                let dressing_model = WALL_DRESSING.iter().any(|part| d.model.starts_with(part));
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
                    link: Link::default(),
                    carve: None,
                    cut: true,
                }
            })
            .collect();
        building
    }
}

/// The placements that are pieces of the kit buildings the town's rules
/// map: what stays out of carving ([`carve::carved`]).
#[must_use]
pub(crate) fn kit_members(placements: &[Placement]) -> BTreeSet<usize> {
    survey(placements)
        .iter()
        .enumerate()
        .filter(|(i, s)| s.build(*i, placements).valid)
        .flat_map(|(_, s)| s.members.iter().copied())
        .collect()
}

/// A placement's box in the world, from its model's bounds.
fn world_box(pack: &ZonePack, placement: &Placement) -> Result<(Vec3, Vec3), String> {
    let model = pack
        .model(placement.model)
        .ok_or_else(|| format!("The Everglade pack has no {}", placement.model))?;
    let (min, max) = model.bounds();
    let transform = placement.transform();
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { min[0] } else { max[0] },
            if i & 2 == 0 { min[1] } else { max[1] },
            if i & 4 == 0 { min[2] } else { max[2] },
        );
        let p = transform.transform_point3(corner);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    Ok((lo, hi))
}

/// The carved placements of `placements` (marked in `carved`) whose boxes
/// touch, each group in placement order.
fn groups(
    pack: &ZonePack,
    placements: &[Placement],
    carved: &[bool],
) -> Result<Vec<Vec<usize>>, String> {
    let chosen: Vec<usize> = (0..placements.len()).filter(|&i| carved[i]).collect();
    let boxes: Vec<(Vec3, Vec3)> = chosen
        .iter()
        .map(|&i| world_box(pack, &placements[i]))
        .collect::<Result<_, _>>()?;
    let mut parent: Vec<usize> = (0..chosen.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    // Boxes by the 8 m squares they cover.
    let mut grid: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    let square = |v: f32| (v / 8.0).floor() as i32;
    for (k, (lo, hi)) in boxes.iter().enumerate() {
        for x in square(lo.x - TOUCH)..=square(hi.x + TOUCH) {
            for z in square(lo.z - TOUCH)..=square(hi.z + TOUCH) {
                grid.entry((x, z)).or_default().push(k);
            }
        }
    }
    for members in grid.values() {
        for (n, &a) in members.iter().enumerate() {
            for &b in &members[n + 1..] {
                let (la, ha) = boxes[a];
                let (lb, hb) = boxes[b];
                let touch =
                    (0..3).all(|axis| la[axis] <= hb[axis] + TOUCH && lb[axis] <= ha[axis] + TOUCH);
                if touch {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    if ra != rb {
                        parent[ra.max(rb)] = ra.min(rb);
                    }
                }
            }
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for k in 0..chosen.len() {
        let root = find(&mut parent, k);
        out.entry(root).or_default().push(chosen[k]);
    }
    Ok(out.into_values().collect())
}

/// What a carved placement of `model` is made of.
fn matter(model: &str) -> Matter {
    if model.starts_with("props/")
        || [
            "fence", "cart", "barrel", "bench", "stall", "sign", "Crate", "Wagon",
        ]
        .iter()
        .any(|k| model.contains(k))
    {
        Matter::Timber
    } else if model.contains("Roof") {
        Matter::Tile
    } else {
        Matter::Plaster
    }
}

/// The carved building `index` of the placements in `group`: a block for
/// every lattice cell that holds triangles, each resting on the blocks
/// under it and carried by the blocks beside it.
fn carved_building(
    pack: &ZonePack,
    placements: &[Placement],
    index: usize,
    group: &[usize],
) -> Result<Building, String> {
    struct Block {
        placement: usize,
        cell: u32,
        lo: Vec3,
        hi: Vec3,
        level: u8,
    }
    let mut carved = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    for &p in group {
        let placement = &placements[p];
        let split = carve::split_model(pack, placement.model, placement.scale)?;
        let Some(columns) = carve::columns(pack, placement)? else {
            continue;
        };
        let lattice = split.lattice;
        // Each cell's solid box, from its columns.
        let mut extents: BTreeMap<u32, (Vec3, Vec3)> = BTreeMap::new();
        for (square, spans) in columns.iter() {
            for span in spans {
                let e = extents
                    .entry(span.part)
                    .or_insert((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)));
                e.0 = e.0.min(Vec3::new(square.min[0], span.lo, square.min[1]));
                e.1 = e.1.max(Vec3::new(square.max[0], span.hi, square.max[1]));
            }
        }
        let mut cells = BTreeMap::new();
        for (cell, (lo, hi)) in extents {
            cells.insert(cell, blocks.len());
            blocks.push(Block {
                placement: p,
                cell,
                lo,
                hi,
                level: lattice.index(cell)[1].min(255) as u8,
            });
        }
        carved.push(Carved {
            placement: p,
            columns,
            lattice,
            split,
            cells,
        });
    }
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for b in &blocks {
        lo = lo.min(b.lo);
        hi = hi.max(b.hi);
    }
    let overlap =
        |a0: f32, a1: f32, b0: f32, b1: f32, margin: f32| a0 < b1 + margin && b0 < a1 + margin;
    let mut pieces = Vec::with_capacity(blocks.len());
    for (k, a) in blocks.iter().enumerate() {
        let center = (a.lo + a.hi) * 0.5;
        let footing = a.lo.y <= height(center.x, center.z) + FOOTING;
        let mut under = Vec::new();
        let mut beside = Vec::new();
        for (j, b) in blocks.iter().enumerate() {
            if j == k {
                continue;
            }
            let across = overlap(a.lo.x, a.hi.x, b.lo.x, b.hi.x, -0.05)
                && overlap(a.lo.z, a.hi.z, b.lo.z, b.hi.z, -0.05);
            if across && b.hi.y >= a.lo.y - 0.3 && b.lo.y < a.lo.y - 0.1 {
                under.push(j as u16);
                continue;
            }
            let near = overlap(a.lo.x, a.hi.x, b.lo.x, b.hi.x, 0.3)
                && overlap(a.lo.z, a.hi.z, b.lo.z, b.hi.z, 0.3);
            let tall = (a.hi.y - a.lo.y).min(b.hi.y - b.lo.y).max(0.05);
            let shared = a.hi.y.min(b.hi.y) - a.lo.y.max(b.lo.y);
            if near && shared >= 0.3 * tall && !(across && a.lo.y < b.lo.y - 0.1) {
                beside.push(j as u16);
            }
        }
        let placement = &placements[a.placement];
        let size = a.hi - a.lo;
        let frame = Mat4::from_rotation_translation(Quat::from_rotation_y(placement.yaw), center);
        let draft = Draft {
            building: index,
            role: Role::Block { level: a.level },
            matter: matter(placement.model),
            placement: frame,
            models: Vec::new(),
            colliders: Vec::new(),
            origin: Vec3::ZERO,
            mass: f64::from((size.x * size.y * size.z * 40.0).clamp(30.0, 3000.0)),
            hit_points: if size.max_element() < 1.6 { 12 } else { 27 },
        };
        pieces.push(TownPiece {
            draft,
            placements: vec![a.placement],
            look: Look { shape: 0, paint: 0 },
            roof: None,
            link: Link {
                footing,
                under,
                beside,
            },
            carve: Some((a.placement, a.cell)),
            cut: false,
        });
    }
    let center = [(lo.x + hi.x) * 0.5, (lo.z + hi.z) * 0.5];
    let levels = blocks.iter().map(|b| b.level).max().unwrap_or(0);
    Ok(Building {
        rect: (center, [(hi.x - lo.x) * 0.5, (hi.z - lo.z) * 0.5]),
        stories: levels.saturating_add(1),
        valid: !pieces.is_empty(),
        carved,
        ready: false,
        base: height(center[0], center[1]),
        top: hi.y,
        pieces,
        blocks: Vec::new(),
        roofs: Vec::new(),
    })
}
