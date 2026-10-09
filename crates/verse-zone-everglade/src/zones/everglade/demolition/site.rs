//! The demolition yard's rules and rigid bodies: kit pieces with hit
//! points, a support graph from the cottages' 2 m grid, and chunks that
//! fly, tumble, and settle in a [`physics::World`].
//!
//! An intact piece that something holds up is a static body, so a cottage
//! costs the solver nothing until it is hit. A piece that loses its support
//! becomes dynamic: walls and posts tip outward and topple, and a roof drops
//! onto whatever walls still stand, leans, and slides off. A piece at zero
//! hit points, or one that falls past a tilt limit, breaks into its chunks
//! with the piece's momentum plus the hammer's push. This follows Wall of
//! Stone's rig in `verse_world::wall_of_stone::rig` (panels with hit points,
//! debris boxes with a lifetime) and the support graph and activation in
//! `docs/verse/destructible-buildings.md`, reimplemented for kit pieces:
//! support is decided by rules over grid adjacency instead of welds.

use glam::{DQuat, DVec3, Mat4, Vec3};
use physics::water::{Water, WaterSet};
use physics::{
    Body, BodyId, BodyKind, Collider, Filter, Joint, JointId, JointKind, Material, Shape, Uniform,
    World,
};
use std::collections::{BTreeMap, BTreeSet};

/// Physics step, s, as the chamber's spell world.
pub const STEP: f64 = 1.0 / 120.0;
/// Most physics steps one frame runs; a long frame drops the rest.
const MAX_STEPS: u32 = 8;
/// How long a chunk lasts, s, as Wall of Stone's debris.
pub const DEBRIS_LIFETIME: f64 = 20.0;
/// Most chunks alive at once; the oldest go first past it.
pub const MAX_CHUNKS: usize = 220;
/// Most dust puffs alive at once.
const MAX_PUFFS: usize = 400;
/// Contact impulse between two pieces in one step below which nothing is
/// damaged, N s: a roof resting on walls loads them far less.
const IMPACT: f64 = 900.0;
/// Contact impulse per point of impact damage above [`IMPACT`], N s.
const IMPACT_PER_POINT: f64 = 120.0;
/// Speed the hammer's push gives chunks next to the blow, m/s.
const PUSH: f64 = 5.0;
/// Impulse the hammer gives a loose piece or chunk it strikes, N s.
const BLOW: f64 = 450.0;
/// Fastest a body in the yard moves, m/s and rad/s. Chunks cut from a
/// kit mesh can start overlapping a neighbour, and the solver would push
/// them apart fast enough to throw them across the field.
const MAX_SPEED: f64 = 10.0;
const MAX_SPIN: f64 = 12.0;
/// Fastest an explosion throws a chunk, m/s, for [`THROWN`] seconds after
/// the blast; past that the ordinary cap applies again.
pub const BLAST_SPEED: f64 = 26.0;
const THROWN: f64 = 1.6;
/// How far above horizontal an explosion throws what it hits, degrees.
const BLAST_UPWARD: f64 = 38.0;
/// Speed at which an unsupported wall's top starts to tip outward, m/s.
const TIP: f64 = 0.9;
/// Speed under which a fallen top's blocks are damped toward rest, m/s,
/// and how much of their motion a step keeps.
const SETTLE_SPEED: f64 = 1.5;
const SETTLE_DAMPING: f64 = 0.95;
/// Debris slower than [`FREEZE_SPEED`] m/s for [`FREEZE_AFTER`] s is
/// frozen in place as a fixed body until something strikes near it, so a
/// rubble pile whose last few pieces still rock costs the solver nothing.
const FREEZE_SPEED: f64 = 0.6;
/// Debris in the water slower than this against something fixed has
/// lodged, m/s.
const LODGED_SPEED: f64 = 0.08;
const FREEZE_AFTER: f64 = 1.5;
/// How near a piece that breaks or comes loose rouses frozen debris, m.
const ROUSE: f64 = 3.0;
/// Levels a carved building needs, and how many times taller than wide it
/// must be, for its top to topple as one body when its base is undercut.
const TALL_LEVELS: u8 = 4;
const TALL_RATIO: f64 = 1.8;
/// The share of a tall building's level, by footprint, that must still
/// stand to carry what is over it; below it, or when the weight over the
/// level is no longer over what stands of it, the top topples.
pub const TOPPLE_LEFT: f64 = 0.4;
/// Tilt at which a toppling top leaves its hinge and falls free, degrees.
const HINGE_RELEASE: f64 = 24.0;
/// Speed, rad/s, at which a toppling top starts to turn over its hinge.
const TIP_SPIN: f64 = 0.4;
/// Fastest a toppling top moves, m/s and rad/s: a 30 m tower's top falls
/// faster than the yard's debris cap.
const FALL_SPEED: f64 = 32.0;
const FALL_SPIN: f64 = 4.0;
/// Tilt past which a toppling top that touches the ground breaks up,
/// degrees.
const CRASH_TILT: f64 = 32.0;
/// Most seconds a top leans without reaching the ground before it breaks
/// up where it lies.
const FALL_LIMIT: f64 = 8.0;
/// Speed above which a block of a top that crashes breaks into its chunks,
/// m/s, and most blocks one crash breaks.
const CRASH_SHATTER: f64 = 4.0;
const CRASH_BREAKS: usize = 12;

/// What one tick's physics steps cost ([`Site::step_cost`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepCost {
    pub steps: u32,
    /// Contact detection and solving over the steps, ms.
    pub detect_ms: f32,
    pub solve_ms: f32,
    /// The most awake bodies and contact points in a step.
    pub awake: usize,
    pub contacts: usize,
}

/// Plaster, timber, and stone surfaces.
const MATERIAL: Material = Material {
    friction: 0.7,
    torsional: 0.0,
    restitution: 0.05,
};

/// Collision groups of roofs and gables.
const ROOF: u32 = 1 << 1;
const GABLE: u32 = 1 << 2;

/// Which way a wall line faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    South,
    North,
    West,
    East,
}

/// What a piece is in its building, which decides what holds it up. A
/// building has one or more stories of wall lines on a rectangle and one
/// 8 m wide roof span or more across its top story, west to east.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Section `index` of `count` along the `side` wall line of `story`,
    /// from the west or the south end.
    Wall {
        side: Side,
        index: u8,
        count: u8,
        story: u8,
    },
    /// The corner post of `story` where the `a` and `b` lines meet; `a` is
    /// south or north.
    Post { a: Side, b: Side, story: u8 },
    /// Roof span `span` of `spans`, from the west, which bears on the top
    /// story's walls under its eaves: the west or east wall line at the
    /// building's ends, and the south and north lines between spans.
    Roof { span: u8, spans: u8 },
    /// The brick gable of roof span `span` over the `side` end wall.
    Gable { side: Side, span: u8 },
    /// A chimney through roof span `span`.
    Chimney { span: u8 },
    /// One block of a model cut on a grid ([`super::carve`]), `level`
    /// blocks over its base. What holds it up is in its spec's [`Link`].
    Block { level: u8 },
}

/// Most blocks a carved building's floor or roof reaches sideways from a
/// block that a column holds up.
const MAX_SPAN: u8 = 2;

/// What holds a carved block up, by the indices of its building's other
/// pieces in the order they were raised.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Link {
    /// It stands on the ground.
    pub footing: bool,
    /// The blocks it rests on.
    pub under: Vec<u16>,
    /// The blocks beside it on its level, which carry it a short way.
    pub beside: Vec<u16>,
}

/// Wall sections under one 8 m roof span.
const SPAN_SECTIONS: u8 = 4;

/// What a piece is made of: its dust's color, hit points, and mass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Matter {
    Plaster,
    Timber,
    Tile,
    Brick,
}

impl Matter {
    /// What it floats or sinks as, kg/m³: seasoned timber floats; plaster,
    /// roof tile, and brick sink.
    #[must_use]
    pub const fn density(self) -> f64 {
        match self {
            Self::Timber => 600.0,
            Self::Plaster => 1700.0,
            Self::Tile => 2000.0,
            Self::Brick => 1900.0,
        }
    }
}

/// Something that struck the water: a splash, or burning debris going out
/// in steam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Splash {
    pub at: Vec3,
    /// How fast it met the surface, m/s, and its mass, kg.
    pub speed: f32,
    pub mass: f32,
    /// Whether it was burning: thrown by a blast moments ago.
    pub hiss: bool,
    pub matter: Matter,
}

/// The ground slab's collision group, which debris in water passes
/// through to the beds under it.
const LAND: u32 = 1 << 30;
/// Debris over water: everything but the slab.
const WET: Filter = Filter {
    group: !LAND,
    mask: !LAND,
};
/// The beds' and banks' boxes.
const BED: Filter = Filter {
    group: !LAND,
    mask: u32::MAX,
};

/// A box in a body's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cuboid {
    /// Center, m.
    pub center: DVec3,
    /// Box frame to body frame.
    pub rotation: DQuat,
    pub half: DVec3,
}

impl Cuboid {
    /// An axis-aligned box from `min` to `max`.
    #[must_use]
    pub fn between(min: DVec3, max: DVec3) -> Self {
        Self {
            center: (min + max) * 0.5,
            rotation: DQuat::IDENTITY,
            half: (max - min) * 0.5,
        }
    }

    #[must_use]
    pub fn volume(&self) -> f64 {
        8.0 * self.half.x * self.half.y * self.half.z
    }

    /// The box frame to the body frame.
    #[must_use]
    pub fn frame(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation.as_quat(), self.center.as_vec3())
    }
}

/// One structural piece as built: everything the site needs to raise it.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceSpec {
    pub building: usize,
    pub role: Role,
    pub matter: Matter,
    /// Body pose in the world.
    pub center: DVec3,
    pub orientation: DQuat,
    /// Mass, kg, and the box its inertia is taken from, m.
    pub mass: f64,
    pub size: DVec3,
    pub hit_points: i32,
    /// Colliders in the body frame.
    pub colliders: Vec<Cuboid>,
    /// The chunks it breaks into, in the body frame.
    pub chunks: Vec<Cuboid>,
    /// Whether the player walks through it: only walls and posts block.
    pub blocks: bool,
    /// What holds a carved block up; empty for a kit piece.
    pub link: Link,
}

impl PieceSpec {
    /// The body frame in the world.
    #[must_use]
    pub fn pose(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.orientation.as_quat(), self.center.as_vec3())
    }
}

/// Where a piece is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Held up, static.
    Standing,
    /// Unsupported and dynamic: toppling, leaning, or at rest where it fell.
    Loose,
    /// Broken into its chunks.
    Broken,
}

/// One chunk of a broken piece.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chunk {
    pub body: BodyId,
    /// When it despawns, s.
    pub until: f64,
    pub gone: bool,
}

/// A piece's live state.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub body: BodyId,
    pub hit_points: i32,
    pub status: Status,
    /// One entry per spec chunk once broken.
    pub chunks: Vec<Chunk>,
    /// While it is part of a toppling top, its frame in that body's frame:
    /// an offset and a rotation.
    pub local: Option<(DVec3, DQuat)>,
    /// Whether it fell as part of a top and lies loose where it crashed,
    /// at any tilt, rather than breaking past a tilt limit.
    pub rubble: bool,
}

impl Piece {
    fn standing(body: BodyId, hit_points: i32) -> Self {
        Self {
            body,
            hit_points,
            status: Status::Standing,
            chunks: Vec::new(),
            local: None,
            rubble: false,
        }
    }
}

/// A tall building's top toppling as one body over the hinge its undercut
/// level left.
#[derive(Clone, Debug, PartialEq)]
struct Fall {
    body: BodyId,
    /// The pieces it carries.
    members: Vec<usize>,
    /// The two point joints on its hinge line while it turns over it.
    joints: Vec<JointId>,
    /// The standing blocks under the hinge line; once none stands, the
    /// joints go and the top falls free rather than hang in the air.
    hinge: Vec<usize>,
    /// Which way it tips, and when it began, s.
    toward: DVec3,
    since: f64,
}

/// A toppled top striking the ground, for its dust, its sound, and the
/// camera's shake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crash {
    pub at: Vec3,
    /// How hard, from 0 to 1.
    pub size: f32,
}

/// A puff of dust: where it is, how it drifts, and how old it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub at: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
    pub size: f32,
    pub color: [f32; 3],
}

/// How a breaking piece's chunks are pushed.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Push {
    /// The chunks nearest `point` take `push`, as from a hammer blow.
    Along { point: DVec3, push: DVec3 },
    /// Every chunk slumps off its piece's plane along `normal`, one way or
    /// the other, at up to `speed`, as a crumbling Wall of Stone's do.
    Slump { normal: DVec3, speed: f64 },
    /// Every chunk flies away from `center` at up to `speed`, falling off
    /// to nothing at `radius`, as from an explosion; out along `face`, a
    /// struck wall's outward normal, when it isn't zero.
    From {
        center: DVec3,
        speed: f64,
        radius: f64,
        face: DVec3,
    },
}

/// What a hammer blow did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blow {
    pub piece: usize,
    pub damage: i32,
    pub hit_points: i32,
    pub broke: bool,
    /// Where the head struck, in the world.
    pub at: Vec3,
}

/// The yard: its pieces as built, the physics world, and what is alive.
#[derive(Clone, Debug)]
pub struct Site {
    specs: Vec<PieceSpec>,
    world: World,
    pieces: Vec<Piece>,
    /// The piece each body is, by body ID, while it stands or is loose.
    owner: Vec<Option<usize>>,
    /// Each building's pieces, and its top story.
    members: BTreeMap<usize, Vec<usize>>,
    tops: BTreeMap<usize, u8>,
    /// Most chunks alive at once ([`MAX_CHUNKS`] unless set).
    max_chunks: usize,
    /// How long a chunk lasts, s ([`DEBRIS_LIFETIME`] unless set).
    debris_lifetime: f64,
    /// Half the side of the flat ground slab at height zero, m, and raised
    /// floors over it, each a center and half extents.
    ground_half: f64,
    floors: Vec<(DVec3, DVec3)>,
    puffs: Vec<Puff>,
    seed: u64,
    rng: u64,
    /// Simulated time not yet stepped, s.
    pending: f64,
    /// What the last tick's steps cost: the solver's detection and
    /// solving, ms, and the most awake bodies and contact points.
    steps: StepCost,
    /// Bumped whenever a piece stops standing, so blockers are rebuilt.
    revision: u64,
    /// Bodies an explosion threw, and until when they may outrun
    /// [`MAX_SPEED`], s; sorted by body.
    thrown: Vec<(BodyId, f64)>,
    /// Tall buildings' tops toppling now.
    falls: Vec<Fall>,
    /// The piece each toppling top's collider stands for, by collider.
    parts: BTreeMap<u32, Vec<usize>>,
    /// Tops that struck the ground since the last [`Site::take_crashes`].
    crashes: Vec<Crash>,
    /// Where the last explosion was, to tip a top cut evenly away from it.
    last_blast: Option<DVec3>,
    /// How long each slow debris body has been slow, s, and the debris
    /// frozen in place, by body.
    resting: BTreeMap<u32, f64>,
    frozen: std::collections::BTreeSet<u32>,
    /// The fixed bodies each frozen body touched when it froze: the
    /// ground, standing pieces, or other frozen debris. It stays frozen
    /// only while one of them is still there and itself held.
    rests: BTreeMap<u32, Vec<u32>>,
    /// Set when something frozen debris may rest on goes: a piece breaks,
    /// comes loose, or joins a toppling top, a chunk ends, or debris
    /// thaws. [`Site::hold_frozen`] then checks what is frozen.
    unsettled: bool,
    /// The water debris floats or sinks in, and the boxes of its beds and
    /// banks, which replace the slab under it.
    water: Option<&'static WaterSet>,
    beds: Vec<(DVec3, DVec3)>,
    /// Debris bodies in the water now, and what struck it since the last
    /// [`Site::take_splashes`].
    wet: BTreeSet<u32>,
    splashes: Vec<Splash>,
}

impl Site {
    /// Raises every piece in `specs`, intact and standing.
    #[must_use]
    pub fn new(specs: Vec<PieceSpec>, seed: u64) -> Self {
        let mut site = Self {
            specs,
            world: World::new(STEP),
            pieces: Vec::new(),
            owner: Vec::new(),
            members: BTreeMap::new(),
            tops: BTreeMap::new(),
            max_chunks: MAX_CHUNKS,
            debris_lifetime: DEBRIS_LIFETIME,
            ground_half: 80.0,
            floors: Vec::new(),
            puffs: Vec::new(),
            seed,
            rng: seed,
            pending: 0.0,
            steps: StepCost::default(),
            revision: 0,
            thrown: Vec::new(),
            falls: Vec::new(),
            parts: BTreeMap::new(),
            crashes: Vec::new(),
            last_blast: None,
            resting: BTreeMap::new(),
            frozen: std::collections::BTreeSet::new(),
            rests: BTreeMap::new(),
            unsettled: false,
            water: None,
            beds: Vec::new(),
            wet: BTreeSet::new(),
            splashes: Vec::new(),
        };
        site.raise();
        site
    }

    /// Keeps at most `max` chunks alive from now on.
    pub fn set_max_chunks(&mut self, max: usize) {
        self.max_chunks = max;
        self.cap_chunks();
    }

    /// Makes the chunks of pieces that break from now on last `seconds`,
    /// and up to a fifth longer, rather than [`DEBRIS_LIFETIME`].
    pub fn set_debris_lifetime(&mut self, seconds: f64) {
        self.debris_lifetime = seconds.max(0.0);
    }

    /// Breaks `piece` into its chunks at once, each slumping off its plane
    /// along `normal`, one way or the other, at up to `speed` m/s, with
    /// the dust of its breaking. Returns whether it broke; a piece already
    /// broken does not.
    pub fn crumble(&mut self, piece: usize, normal: DVec3, speed: f64) -> bool {
        if self
            .pieces
            .get(piece)
            .is_none_or(|p| p.status == Status::Broken)
        {
            return false;
        }
        self.shatter(piece, Push::Slump { normal, speed });
        self.support();
        true
    }

    /// Whether every chunk has ended and every dust puff has settled, so
    /// nothing of a broken piece is left to draw.
    #[must_use]
    pub fn cleared(&self) -> bool {
        self.puffs.is_empty()
            && self
                .pieces
                .iter()
                .all(|p| p.status == Status::Broken && p.chunks.iter().all(|c| c.gone))
    }

    /// Retires the chunks `chunks`, each a piece and a chunk index there,
    /// as if their time ran out: a town retires its oldest debris this way
    /// to stay within its geometry budget.
    pub fn retire_chunks(&mut self, chunks: &[(usize, usize)]) {
        for &(p, i) in chunks {
            let Some(chunk) = self
                .pieces
                .get_mut(p)
                .and_then(|piece| piece.chunks.get_mut(i))
            else {
                continue;
            };
            if chunk.gone {
                continue;
            }
            chunk.gone = true;
            let body = chunk.body;
            self.remove_chunk(body);
        }
    }

    /// Makes the flat ground slab `half` meters either side of the origin,
    /// and adds `floors` over it, each a box's center and half extents, for
    /// ground that rises. Takes effect when the world is next built.
    pub fn set_ground(&mut self, half: f64, floors: Vec<(DVec3, DVec3)>) {
        self.ground_half = half;
        self.floors = floors;
    }

    /// Puts `water` under the site with its beds and banks, `beds` (each a
    /// box's center and half extents): debris over it falls through the
    /// slab onto them, floats as its matter does
    /// (`docs/verse/water.md`, Spells and destruction), drifts with the
    /// current, and lodges against the banks. Takes effect when the world
    /// is next built.
    pub fn set_water(&mut self, water: &'static WaterSet, beds: Vec<(DVec3, DVec3)>) {
        self.water = Some(water);
        self.beds = beds;
    }

    /// What struck the water since the last call.
    pub fn take_splashes(&mut self) -> Vec<Splash> {
        std::mem::take(&mut self.splashes)
    }

    /// Debris bodies in the water now.
    #[must_use]
    pub fn afloat(&self) -> usize {
        self.wet.len()
    }

    /// The debris in the water now: each body's place and velocity, for
    /// the ripple field.
    #[must_use]
    pub fn floating(&self) -> Vec<(Vec3, Vec3)> {
        self.wet
            .iter()
            .filter_map(|&id| self.world.bodies().get(id as usize))
            .filter(|b| !b.removed)
            .map(|b| (b.pos.as_vec3(), b.vel.as_vec3()))
            .collect()
    }

    /// Every chunk alive with its matter.
    #[must_use]
    pub fn chunk_bodies(&self) -> Vec<(BodyId, Matter)> {
        let mut out = Vec::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            for chunk in piece.chunks.iter().filter(|c| !c.gone) {
                out.push((chunk.body, self.specs[index].matter));
            }
        }
        out
    }

    /// A body's position and velocity, m and m/s.
    #[must_use]
    pub fn body_motion(&self, body: BodyId) -> (DVec3, DVec3) {
        let b = &self.world[body];
        (b.pos, b.vel)
    }

    /// Whether body `id` is frozen in place.
    #[must_use]
    pub fn is_frozen(&self, body: BodyId) -> bool {
        self.frozen.contains(&body.0)
    }

    /// A physics world with only the ground in it: body 0, the slab, with
    /// the floors' colliders, and the beds and banks of any water.
    fn ground(&self) -> World {
        let mut world = World::new(STEP);
        let ground = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static),
        );
        let slab = Collider::new(
            ground,
            Shape::Cuboid {
                half: DVec3::new(self.ground_half, 0.5, self.ground_half),
            },
        )
        .with_material(MATERIAL);
        world.add_collider(if self.water.is_some() {
            slab.with_filter(Filter {
                group: LAND,
                mask: LAND,
            })
        } else {
            slab
        });
        for &(center, half) in &self.beds {
            world.add_collider(
                Collider::new(ground, Shape::Cuboid { half })
                    .at(center - DVec3::new(0.0, -0.5, 0.0), DQuat::IDENTITY)
                    .with_material(MATERIAL)
                    .with_filter(BED),
            );
        }
        for &(center, half) in &self.floors {
            world.add_collider(
                Collider::new(ground, Shape::Cuboid { half })
                    .at(center - DVec3::new(0.0, -0.5, 0.0), DQuat::IDENTITY)
                    .with_material(MATERIAL),
            );
        }
        world
    }

    fn raise(&mut self) {
        self.world = self.ground();
        self.pieces = Vec::with_capacity(self.specs.len());
        self.owner.clear();
        for index in 0..self.specs.len() {
            let body = self.piece_body(index);
            let id = self.world.add(body);
            self.add_piece_colliders(index, id);
            self.own(id, Some(index));
            self.pieces
                .push(Piece::standing(id, self.specs[index].hit_points));
        }
        self.index_buildings();
        self.puffs.clear();
        self.pending = 0.0;
        self.thrown.clear();
        self.falls.clear();
        self.parts.clear();
        self.crashes.clear();
        self.last_blast = None;
        self.resting.clear();
        self.frozen.clear();
        self.rests.clear();
        self.unsettled = false;
    }

    /// Piece `index`'s body as built: static, where it was placed.
    fn piece_body(&self, index: usize) -> Body {
        let spec = &self.specs[index];
        let mut body = Body::new(
            spec.mass,
            Body::box_inertia(spec.mass, spec.size),
            spec.center,
        )
        .with_kind(BodyKind::Static);
        body.orientation = spec.orientation;
        body.prev_orientation = spec.orientation;
        body
    }

    /// Adds piece `index`'s colliders to body `id`.
    fn add_piece_colliders(&mut self, index: usize, id: BodyId) {
        let spec = &self.specs[index];
        // A gable stands under the roof's slopes but does not carry
        // them, so the two never touch.
        let filter = match spec.role {
            Role::Roof { .. } => Filter {
                group: ROOF,
                mask: !GABLE,
            },
            Role::Gable { .. } => Filter {
                group: GABLE,
                mask: !ROOF,
            },
            _ => Filter::ALL,
        };
        let colliders: Vec<Collider> = spec
            .colliders
            .iter()
            .map(|collider| {
                Collider::new(
                    id,
                    Shape::Cuboid {
                        half: collider.half,
                    },
                )
                .at(collider.center, collider.rotation)
                .with_material(MATERIAL)
                .with_filter(filter)
            })
            .collect();
        for collider in colliders {
            self.world.add_collider(collider);
        }
    }

    /// Adds a chunk's body with `cuboid`'s collider, slightly smaller so
    /// neighbouring chunks start apart.
    fn add_chunk(&mut self, body: Body, cuboid: &Cuboid) -> BodyId {
        let id = self.world.add(body);
        self.world.add_collider(
            Collider::new(
                id,
                Shape::Cuboid {
                    half: (cuboid.half * 0.94).max(DVec3::splat(0.03)),
                },
            )
            .with_material(MATERIAL),
        );
        id
    }

    /// Records that body `id` is `piece`, or no piece.
    fn own(&mut self, id: BodyId, piece: Option<usize>) {
        let at = id.0 as usize;
        if self.owner.len() <= at {
            self.owner.resize(at + 1, None);
        }
        self.owner[at] = piece;
    }

    /// The standing or loose piece body `id` is.
    fn owner_of(&self, id: BodyId) -> Option<usize> {
        self.owner.get(id.0 as usize).copied().flatten()
    }

    /// The piece collider number `index` on body `body` stands for: the
    /// body's piece, or one piece of a toppling top.
    fn collider_owner(&self, index: usize, body: BodyId) -> Option<usize> {
        self.owner_of(body).or_else(|| {
            self.parts
                .get(&(index as u32))
                .and_then(|p| p.first().copied())
        })
    }

    /// Groups the pieces by building and finds each building's top story.
    fn index_buildings(&mut self) {
        self.members.clear();
        self.tops.clear();
        for (index, spec) in self.specs.iter().enumerate() {
            self.members.entry(spec.building).or_default().push(index);
            if let Role::Wall { story, .. } = spec.role {
                let top = self.tops.entry(spec.building).or_default();
                *top = (*top).max(story);
            }
        }
    }

    /// Raises `specs` beside the pieces already here, intact and standing,
    /// and returns their indices.
    pub fn add(&mut self, specs: Vec<PieceSpec>) -> std::ops::Range<usize> {
        let start = self.specs.len();
        self.specs.extend(specs);
        for index in start..self.specs.len() {
            let body = self.piece_body(index);
            let id = self.world.add(body);
            self.add_piece_colliders(index, id);
            self.own(id, Some(index));
            self.pieces
                .push(Piece::standing(id, self.specs[index].hit_points));
        }
        self.index_buildings();
        start..self.specs.len()
    }

    /// Keeps only the pieces whose spec `keep` accepts, each as it is now:
    /// standing, loose where it lies and moving as it moves, or broken
    /// into the chunks still alive. The physics world is rebuilt without
    /// the rest, so removed bodies don't linger in it. Piece indices
    /// change; the kept pieces keep their order.
    pub fn retain(&mut self, keep: impl Fn(&PieceSpec) -> bool) {
        // A toppling top lies down as rubble first: its pieces share one
        // body, which the rebuilt world keeps per piece.
        while !self.falls.is_empty() {
            self.crash(0, false);
        }
        let fresh = self.ground();
        let old = std::mem::replace(&mut self.world, fresh);
        let then = old.time();
        let now = self.world.time();
        let specs = std::mem::take(&mut self.specs);
        let pieces = std::mem::take(&mut self.pieces);
        self.owner.clear();
        self.thrown.clear();
        self.resting.clear();
        let frozen = std::mem::take(&mut self.frozen);
        self.rests.clear();
        // Frozen debris moves again in the rebuilt world.
        let thaw = |id: BodyId| {
            let mut body = old[id];
            if frozen.contains(&id.0) {
                body.kind = BodyKind::Dynamic;
            }
            body
        };
        let ground = BodyId(0);
        for (spec, mut piece) in specs.into_iter().zip(pieces) {
            if !keep(&spec) {
                continue;
            }
            let index = self.specs.len();
            self.specs.push(spec);
            match piece.status {
                Status::Standing | Status::Loose => {
                    let id = self.world.add(thaw(piece.body));
                    self.add_piece_colliders(index, id);
                    self.own(id, Some(index));
                    piece.body = id;
                }
                Status::Broken => {
                    piece.body = ground;
                    for (k, chunk) in piece.chunks.iter_mut().enumerate() {
                        if chunk.gone {
                            chunk.body = ground;
                            continue;
                        }
                        let cuboid = self.specs[index].chunks[k];
                        chunk.body = self.add_chunk(thaw(chunk.body), &cuboid);
                        chunk.until = chunk.until - then + now;
                    }
                }
            }
            self.pieces.push(piece);
        }
        self.index_buildings();
        self.pending = 0.0;
        self.revision += 1;
        self.unsettled = true;
    }

    /// Rebuilds every piece as it was first raised.
    pub fn reset(&mut self) {
        self.rng = self.seed;
        self.raise();
        self.revision += 1;
        self.unsettled = true;
    }

    #[must_use]
    pub fn specs(&self) -> &[PieceSpec] {
        &self.specs
    }

    #[must_use]
    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    #[must_use]
    pub fn puffs(&self) -> &[Puff] {
        &self.puffs
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Simulated time, s.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.world.time()
    }

    /// A body's world pose.
    #[must_use]
    pub fn body_pose(&self, body: BodyId) -> Mat4 {
        let b = &self.world[body];
        Mat4::from_rotation_translation(b.orientation.as_quat(), b.pos.as_vec3())
    }

    /// The pieces standing or loose that `point` is within `reach` of, the
    /// nearest first, with the nearest point on each.
    fn near(&self, point: DVec3, reach: f64) -> Vec<(usize, f64, DVec3)> {
        let mut found: Vec<(usize, f64, DVec3)> = Vec::new();
        for (c, collider) in self.world.colliders().iter().enumerate() {
            let Some(index) = self.collider_owner(c, collider.body) else {
                continue;
            };
            let on = collider.closest_point(&self.world, point);
            let distance = on.distance(point);
            if distance > reach {
                continue;
            }
            match found.iter_mut().find(|(i, ..)| *i == index) {
                Some(entry) if distance < entry.1 => *entry = (index, distance, on),
                Some(_) => {}
                None => found.push((index, distance, on)),
            }
        }
        found.sort_by(|a, b| a.1.total_cmp(&b.1));
        found
    }

    /// Swings at whatever the hammer's head passes within `reach` of along
    /// `path`, pushing along `push`: rolls the damage, damages the nearest
    /// piece, and knocks loose pieces and chunks there.
    pub fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow> {
        let push = push.as_dvec3().normalize_or_zero();
        let mut best: Option<(usize, f64, DVec3)> = None;
        for point in path {
            if let Some(hit) = self.near(point.as_dvec3(), f64::from(reach)).first()
                && best.is_none_or(|b| hit.1 < b.1)
            {
                best = Some(*hit);
            }
        }
        // Chunks near the blow scatter whether or not a piece was struck.
        if let Some(last) = path.get(path.len() / 2) {
            self.scatter(last.as_dvec3(), push, 1.2, BLOW * 0.25);
        }
        let (piece, _, point) = best?;
        // A maul: 2d6 + 3 bludgeoning, doubled against structures as the
        // SRD's Siege Monster trait does.
        let damage = 2 * (self.roll(6) + self.roll(6) + 3);
        if self.pieces[piece].status == Status::Loose {
            let body = self.pieces[piece].body;
            self.rouse(body);
            self.world[body].apply_impulse_at(push * BLOW * 2.0, point);
        }
        self.dust(point.as_vec3(), 4, 0.6, self.specs[piece].matter);
        let broke = self.damage(piece, damage, point, push * PUSH);
        Some(Blow {
            piece,
            damage,
            hit_points: self.pieces[piece].hit_points,
            broke,
            at: point.as_vec3(),
        })
    }

    /// Takes `amount` hit points from `piece`; at zero it breaks, its chunks
    /// near `point` pushed at `push`. Returns whether it broke.
    pub fn damage(&mut self, piece: usize, amount: i32, point: DVec3, push: DVec3) -> bool {
        self.hurt(piece, amount, Push::Along { point, push })
    }

    /// Takes `amount` hit points from `piece`; at zero it breaks, its chunks
    /// pushed as `push` says. Returns whether it broke.
    fn hurt(&mut self, piece: usize, amount: i32, push: Push) -> bool {
        let state = &mut self.pieces[piece];
        if state.status == Status::Broken || amount <= 0 {
            return false;
        }
        state.hit_points = (state.hit_points - amount).max(0);
        if state.hit_points > 0 {
            return false;
        }
        self.shatter(piece, push);
        self.support();
        true
    }

    /// Breaks `piece` into its chunks with its momentum, pushed as `push`
    /// says.
    fn shatter(&mut self, piece: usize, push: Push) {
        let id = self.pieces[piece].body;
        let body = self.piece_state(piece);
        if self.pieces[piece].local.is_some() {
            self.detach(piece);
        } else {
            wake_near(&mut self.world, id);
            self.rouse_near(body.pos);
            self.world.remove_body(id);
            self.own(id, None);
        }
        let spec = &self.specs[piece];
        let total: f64 = spec
            .chunks
            .iter()
            .map(Cuboid::volume)
            .sum::<f64>()
            .max(1e-6);
        let omega = body.omega_world();
        let time = self.world.time();
        let mut chunks = Vec::with_capacity(spec.chunks.len());
        let mut puffs = Vec::new();
        let matter = spec.matter;
        let piece_mass = spec.mass;
        let cuboids = spec.chunks.clone();
        for cuboid in cuboids {
            let mass = (piece_mass * cuboid.volume() / total).max(5.0);
            let pos = body.to_world(cuboid.center);
            let orientation = body.orientation * cuboid.rotation;
            let mut chunk = Body::new(mass, Body::box_inertia(mass, cuboid.half * 2.0), pos);
            chunk.orientation = orientation;
            chunk.prev_orientation = orientation;
            let spread = DVec3::new(self.unit(), self.unit().abs(), self.unit()) * 0.8;
            // The push on this chunk, and how near an explosion's heart it
            // was, from 0 to 1.
            let (kick, k) = match push {
                Push::Along { point, push } => {
                    let near = (1.0 - pos.distance(point) / 2.5).clamp(0.0, 1.0);
                    (push * near, 0.0)
                }
                Push::Slump { normal, speed } => (normal * speed * self.unit(), 0.0),
                Push::From {
                    center,
                    speed,
                    radius,
                    face,
                } => {
                    let k = (1.0 - pos.distance(center) / radius).clamp(0.0, 1.0);
                    // At least a third of the speed, so a chunk at the
                    // edge of the blast still leaves its piece.
                    let speed = speed * (0.35 + 0.65 * k) * (0.8 + 0.4 * self.unit().abs());
                    (blast_direction(center, pos, face) * speed + spread * 2.0, k)
                }
            };
            chunk.vel = body.vel + omega.cross(pos - body.pos) + kick + spread;
            chunk.omega = DVec3::new(self.unit(), self.unit(), self.unit())
                * if k > 0.0 { 2.0 + 8.0 * k } else { 2.0 };
            // A chunk of a sunk footing that lies wholly under the ground
            // stays buried: under the slab, nothing would hold it up.
            let ground = self.ground_under(pos);
            // How far the turned box reaches up and down from its middle.
            let turn = glam::DMat3::from_quat(orientation);
            let reach = turn.x_axis.y.abs() * cuboid.half.x
                + turn.y_axis.y.abs() * cuboid.half.y
                + turn.z_axis.y.abs() * cuboid.half.z;
            let buried = ground.is_some_and(|g| pos.y + reach < g + 0.02);
            let chunk_id = self.add_chunk(chunk, &cuboid);
            if buried {
                self.remove_chunk(chunk_id);
                chunks.push(Chunk {
                    body: chunk_id,
                    until: time,
                    gone: true,
                });
                continue;
            }
            if matches!(push, Push::From { .. }) {
                self.throw(chunk_id);
            }
            // The fragments nearest an explosion's heart end first, so
            // the debris cap takes them, hidden in the fireball, before
            // the ones that fly.
            chunks.push(Chunk {
                body: chunk_id,
                until: time + self.debris_lifetime * (1.0 - 0.6 * k + 0.2 * self.unit().abs()),
                gone: false,
            });
            puffs.push(pos.as_vec3());
        }
        let state = &mut self.pieces[piece];
        state.status = Status::Broken;
        state.hit_points = 0;
        state.chunks = chunks;
        for at in puffs {
            self.dust(at, 3, 1.0, matter);
        }
        self.revision += 1;
        self.unsettled = true;
        self.cap_chunks();
    }

    /// The top of the ground the site holds up at `at`, m: the slab or a
    /// floor over it, or `None` over water, whose bed lies lower.
    fn ground_under(&self, at: DVec3) -> Option<f64> {
        let over = |&(center, half): &(DVec3, DVec3)| {
            (at.x - center.x).abs() <= half.x && (at.z - center.z).abs() <= half.z
        };
        if self.beds.iter().any(over) {
            return None;
        }
        Some(
            self.floors
                .iter()
                .filter(|f| over(f))
                .map(|&(center, half)| center.y + half.y)
                .fold(0.0, f64::max),
        )
    }

    /// Removes the oldest chunks past the cap ([`MAX_CHUNKS`] unless set).
    fn cap_chunks(&mut self) {
        let mut alive: Vec<(f64, usize, usize)> = self
            .pieces
            .iter()
            .enumerate()
            .flat_map(|(p, piece)| {
                piece
                    .chunks
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| !c.gone)
                    .map(move |(i, c)| (c.until, p, i))
            })
            .collect();
        if alive.len() <= self.max_chunks {
            return;
        }
        alive.sort_by(|a, b| a.0.total_cmp(&b.0));
        let excess = alive.len() - self.max_chunks;
        for &(_, p, i) in &alive[..excess] {
            let chunk = &mut self.pieces[p].chunks[i];
            chunk.gone = true;
            let body = chunk.body;
            self.remove_chunk(body);
        }
    }

    /// Wakes and pushes loose bodies within `radius` of `at`.
    fn scatter(&mut self, at: DVec3, push: DVec3, radius: f64, impulse: f64) {
        let bodies: Vec<BodyId> = self
            .pieces
            .iter()
            .flat_map(|p| p.chunks.iter().filter(|c| !c.gone).map(|c| c.body))
            .filter(|&b| self.world[b].pos.distance(at) < radius)
            .collect();
        for body in bodies {
            self.rouse(body);
            let b = &mut self.world[body];
            let scale = (b.mass / 60.0).min(1.0);
            b.apply_impulse_at((push + DVec3::Y * 0.3) * impulse * scale, at);
        }
    }

    /// Whether `point` is within `reach` of a standing or loose piece.
    #[must_use]
    pub fn touches(&self, point: Vec3, reach: f32) -> bool {
        let point = point.as_dvec3();
        self.world
            .colliders()
            .iter()
            .enumerate()
            .any(|(c, collider)| {
                self.collider_owner(c, collider.body).is_some()
                    && collider.closest_point(&self.world, point).distance(point)
                        <= f64::from(reach)
            })
    }

    /// An explosion at `center`: every piece within `radius` takes
    /// `damage` scaled from all of it at the center to none at the edge,
    /// what breaks flies outward at up to `speed`, and loose pieces and
    /// chunks in reach are thrown the same way. Returns a blow for each
    /// piece it reached, nearest first.
    pub fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow> {
        self.explode_facing(center, radius, damage, speed, Vec3::ZERO)
    }

    /// [`Self::explode`] on a wall whose outward normal is `face`: what
    /// breaks spalls out of the wall's face, a crater around `center`,
    /// rather than flying on into the building.
    pub fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        let c = center.as_dvec3();
        let face = face.as_dvec3().normalize_or_zero();
        self.last_blast = Some(c);
        let (radius, speed) = (f64::from(radius), f64::from(speed));
        let mut blows = Vec::new();
        for (piece, distance, point) in self.near(c, radius) {
            let k = 1.0 - distance / radius;
            let amount = (f64::from(damage) * k).round() as i32;
            // An earlier piece's fall may have broken this one already.
            if self.pieces[piece].status == Status::Broken || amount <= 0 {
                continue;
            }
            let broke = self.hurt(
                piece,
                amount,
                Push::From {
                    center: c,
                    speed,
                    radius,
                    face,
                },
            );
            blows.push(Blow {
                piece,
                damage: amount,
                hit_points: self.pieces[piece].hit_points,
                broke,
                at: point.as_vec3(),
            });
        }
        // What is loose now, and what earlier blows left lying, is thrown.
        let mut bodies: Vec<BodyId> = Vec::new();
        for piece in &self.pieces {
            match piece.status {
                // A toppling top is far too heavy to throw.
                Status::Loose if piece.local.is_some() => {}
                Status::Loose => bodies.push(piece.body),
                Status::Broken => {
                    bodies.extend(piece.chunks.iter().filter(|c| !c.gone).map(|c| c.body));
                }
                Status::Standing => {}
            }
        }
        for id in bodies {
            let at = self.world[id].pos;
            let k = 1.0 - at.distance(c) / radius;
            if k <= 0.0 {
                continue;
            }
            self.rouse(id);
            let kick = blast_direction(c, at, face) * speed * k * 0.7;
            let body = &mut self.world[id];
            // A chunk this blast just threw keeps its speed.
            if body.vel.length() < kick.length() {
                body.vel += kick;
            }
            self.throw(id);
        }
        self.dust(center, 10, 2.6, Matter::Brick);
        self.dust(center, 8, 2.0, Matter::Plaster);
        blows
    }

    /// Lets `body` outrun the speed cap for a moment, as an explosion
    /// throws it.
    fn throw(&mut self, body: BodyId) {
        let until = self.world.time() + THROWN;
        match self.thrown.binary_search_by_key(&body.0, |(id, _)| id.0) {
            Ok(i) => self.thrown[i].1 = until,
            Err(i) => self.thrown.insert(i, (body, until)),
        }
    }

    /// Whether `piece` still stands.
    fn standing(&self, piece: usize) -> bool {
        self.pieces[piece].status == Status::Standing
    }

    /// Sets loose every standing piece nothing holds up any more, until
    /// nothing changes: a wall section needs a standing neighbor on its
    /// line or a standing corner post, and above the ground story the
    /// section under it; a post a standing wall beside it, and the post
    /// under it; a roof span two standing top-story sections under each
    /// eave; a gable its roof span and a standing section of its end wall;
    /// and a chimney its roof span.
    pub fn support(&mut self) {
        loop {
            // A tall building cut through on one side tips over whole
            // before its loose blocks would fall one by one.
            if self.topple() {
                continue;
            }
            let blocks = self.block_holds();
            let falling: Vec<usize> = (0..self.pieces.len())
                .filter(|&i| {
                    self.standing(i)
                        && match self.specs[i].role {
                            Role::Block { .. } => !blocks[i],
                            _ => !self.held(i),
                        }
                })
                .collect();
            if falling.is_empty() {
                return;
            }
            for piece in falling {
                if self.standing(piece) {
                    self.loosen(piece);
                }
                if matches!(self.specs[piece].role, Role::Roof { .. }) {
                    self.buckle(piece);
                }
            }
        }
    }

    /// Topples the top of the first tall carved building whose level is
    /// cut through enough that it can't carry what stands over it. Returns
    /// whether one toppled.
    ///
    /// A carved building at least [`TALL_LEVELS`] levels high and
    /// [`TALL_RATIO`] times taller than wide topples when less than
    /// [`TOPPLE_LEFT`] of a level's footprint still stands under standing
    /// blocks, or when the center of mass of the blocks over the level is
    /// no longer over what stands of it. The level's blocks on the cut side of the top's center of
    /// mass are crushed; those behind it stay as the hinge. Every standing
    /// block over the level becomes one dynamic body with a box collider
    /// per block, pinned to the hinge's edge by two point joints so it
    /// turns over the edge toward the cut, until it leans past
    /// [`HINGE_RELEASE`] and falls free. It breaks up when it strikes the
    /// ground ([`Self::crash`]).
    fn topple(&mut self) -> bool {
        let buildings: Vec<usize> = self.members.keys().copied().collect();
        for building in buildings {
            if let Some(level) = self.undercut(building) {
                self.fell(building, level);
                return true;
            }
        }
        false
    }

    /// The lowest level of a tall carved `building` too cut through to
    /// carry the standing blocks over it, if any.
    fn undercut(&self, building: usize) -> Option<u8> {
        let members = self.members.get(&building)?;
        let blocks: Vec<(usize, u8)> = members
            .iter()
            .filter_map(|&i| match self.specs[i].role {
                Role::Block { level } => Some((i, level)),
                _ => None,
            })
            .collect();
        let top = blocks.iter().map(|b| b.1).max()?;
        if top + 1 < TALL_LEVELS {
            return None;
        }
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        for &(i, _) in &blocks {
            let spec = &self.specs[i];
            let half = world_half(spec.orientation, spec.size * 0.5);
            lo = lo.min(spec.center - half);
            hi = hi.max(spec.center + half);
        }
        let wide = (hi.x - lo.x).max(hi.z - lo.z).max(0.1);
        if hi.y - lo.y < TALL_RATIO * wide {
            return None;
        }
        let area = |i: usize| self.specs[i].size.x * self.specs[i].size.z;
        for level in 0..top {
            let above: Vec<usize> = blocks
                .iter()
                .filter(|&&(i, l)| l > level && self.standing(i))
                .map(|&(i, _)| i)
                .collect();
            if above.is_empty() {
                return None;
            }
            let (mut full, mut left) = (0.0, 0.0);
            let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
            for &(i, l) in &blocks {
                if l == level {
                    full += area(i);
                    if self.standing(i) {
                        left += area(i);
                        let spec = &self.specs[i];
                        let half = world_half(spec.orientation, spec.size * 0.5);
                        lo = lo.min(spec.center - half);
                        hi = hi.max(spec.center + half);
                    }
                }
            }
            if full <= 0.0 {
                continue;
            }
            // Too little left to carry it, or what is left no longer under
            // the weight over it.
            let mass: f64 = above.iter().map(|&i| self.specs[i].mass).sum();
            let com = above
                .iter()
                .map(|&i| self.specs[i].center * self.specs[i].mass)
                .sum::<DVec3>()
                / mass.max(1e-6);
            let inset = 0.3;
            let under = com.x > lo.x + inset
                && com.x < hi.x - inset
                && com.z > lo.z + inset
                && com.z < hi.z - inset;
            if left < TOPPLE_LEFT * full || !under {
                return Some(level);
            }
        }
        None
    }

    /// Topples every standing block of `building` over `level`.
    fn fell(&mut self, building: usize, level: u8) {
        let members = self.members.get(&building).cloned().unwrap_or_default();
        let level_of = |spec: &PieceSpec| match spec.role {
            Role::Block { level } => Some(level),
            _ => None,
        };
        let at_level: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| level_of(&self.specs[i]) == Some(level))
            .collect();
        let upper: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&i| self.standing(i) && level_of(&self.specs[i]).is_some_and(|l| l > level))
            .collect();
        let flat = |v: DVec3| DVec3::new(v.x, 0.0, v.z);
        let weigh = |set: &mut dyn Iterator<Item = usize>| {
            let (mut sum, mut total) = (DVec3::ZERO, 0.0);
            for i in set {
                let a = self.specs[i].size.x * self.specs[i].size.z;
                sum += self.specs[i].center * a;
                total += a;
            }
            (total > 0.0).then(|| sum / total)
        };
        let middle = weigh(&mut at_level.iter().copied()).unwrap_or(DVec3::ZERO);
        let gone = weigh(&mut at_level.iter().copied().filter(|&i| !self.standing(i)));
        // It tips toward the cut, or away from the blast that cut it evenly.
        let toward = gone
            .map(|g| flat(g - middle))
            .filter(|g| g.length() > 0.3)
            .and_then(DVec3::try_normalize)
            .or_else(|| {
                self.last_blast
                    .and_then(|b| flat(b - middle).try_normalize())
            })
            .unwrap_or(DVec3::X);
        let mass: f64 = upper
            .iter()
            .map(|&i| self.specs[i].mass)
            .sum::<f64>()
            .max(1.0);
        let com = upper
            .iter()
            .map(|&i| self.world[self.pieces[i].body].pos * self.specs[i].mass)
            .sum::<DVec3>()
            / mass;
        let side = |p: DVec3| (p - middle).dot(toward);
        let depth = |spec: &PieceSpec, along: DVec3| {
            world_half(spec.orientation, spec.size * 0.5).dot(along.abs())
        };
        // The level's blocks on the cut side of the top's weight are
        // crushed under it; the rest are the hinge.
        let cell = at_level
            .iter()
            .map(|&i| depth(&self.specs[i], toward))
            .fold(0.0_f64, f64::max);
        let crush: Vec<usize> = at_level
            .iter()
            .copied()
            .filter(|&i| self.standing(i) && side(self.specs[i].center) > side(com) - cell)
            .collect();
        for &i in &crush {
            let at = self.specs[i].center;
            self.shatter(
                i,
                Push::Along {
                    point: at,
                    push: toward * 2.0 - DVec3::Y,
                },
            );
        }
        let hinge: Vec<usize> = at_level
            .iter()
            .copied()
            .filter(|&i| self.standing(i))
            .collect();
        // The top: one body at its center of mass, a box for each block.
        let (mut lo, mut hi) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
        let mut boxes = Vec::with_capacity(upper.len());
        for &i in &upper {
            let body = self.world[self.pieces[i].body];
            let spec = &self.specs[i];
            let (mut a, mut b) = (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY));
            for c in &spec.colliders {
                let h = world_half(c.rotation, c.half);
                a = a.min(c.center - h);
                b = b.max(c.center + h);
            }
            if !a.is_finite() || !b.is_finite() {
                a = -spec.size * 0.5;
                b = spec.size * 0.5;
            }
            let offset = body.pos - com;
            let center = offset + body.orientation * ((a + b) * 0.5);
            let half = ((b - a) * 0.5).max(DVec3::splat(0.05));
            let h = world_half(body.orientation, half);
            lo = lo.min(center - h);
            hi = hi.max(center + h);
            boxes.push((i, offset, body.orientation, center, half));
        }
        let mut top = Body::new(mass, Body::box_inertia(mass, hi - lo), com);
        let axis = DVec3::Y.cross(toward).normalize_or(DVec3::Z);
        top.omega = axis * TIP_SPIN;
        let id = self.world.add(top);
        // One box for each level's blocks together, so a tall top is a
        // dozen colliders rather than a hundred.
        let mut levels: BTreeMap<u8, (DVec3, DVec3, Vec<usize>)> = BTreeMap::new();
        for &(i, offset, rotation, center, half) in &boxes {
            let old = self.pieces[i].body;
            self.world.remove_body(old);
            self.own(old, None);
            let h = world_half(rotation, half);
            let entry = levels
                .entry(level_of(&self.specs[i]).unwrap_or(0))
                .or_insert((
                    DVec3::splat(f64::INFINITY),
                    DVec3::splat(f64::NEG_INFINITY),
                    Vec::new(),
                ));
            entry.0 = entry.0.min(center - h);
            entry.1 = entry.1.max(center + h);
            entry.2.push(i);
            let piece = &mut self.pieces[i];
            piece.body = id;
            piece.local = Some((offset, rotation));
            piece.status = Status::Loose;
        }
        for (_, (a, b, members)) in levels {
            let collider = self.world.add_collider(
                Collider::new(
                    id,
                    Shape::Cuboid {
                        half: ((b - a) * 0.5).max(DVec3::splat(0.05)),
                    },
                )
                .at((a + b) * 0.5, DQuat::IDENTITY)
                .with_material(MATERIAL),
            );
            self.parts.insert(collider.0, members);
        }
        // The hinge: the edge of what still stands nearest the cut, at the
        // top's foot, held by two joints to the ground's fixed body.
        let mut joints = Vec::new();
        if !hinge.is_empty() {
            let edge = hinge
                .iter()
                .map(|&i| side(self.specs[i].center) + depth(&self.specs[i], toward))
                .fold(f64::NEG_INFINITY, f64::max);
            let across = hinge
                .iter()
                .map(|&i| {
                    (self.specs[i].center - middle).dot(axis).abs() + depth(&self.specs[i], axis)
                })
                .fold(0.5_f64, f64::max);
            let foot = boxes
                .iter()
                .map(|&(_, _, rotation, center, half)| {
                    com.y + center.y - world_half(rotation, half).y
                })
                .fold(f64::INFINITY, f64::min);
            let pivot = flat(middle) + toward * edge + DVec3::Y * foot;
            // Already turning over the hinge, not about its own middle.
            self.world[id].vel = (axis * TIP_SPIN).cross(com - pivot);
            let ground = self.world[BodyId(0)].pos;
            for end in [-1.0, 1.0] {
                let at = pivot + axis * across * 0.8 * end;
                joints.push(self.world.add_joint(Joint::new(
                    id,
                    at - com,
                    BodyId(0),
                    at - ground,
                    JointKind::Point,
                )));
            }
            let matter = self.specs[hinge[0]].matter;
            self.dust(pivot.as_vec3(), 10, 2.0, matter);
        }
        wake_near(&mut self.world, id);
        self.falls.push(Fall {
            body: id,
            members: upper,
            joints,
            hinge,
            toward,
            since: self.world.time(),
        });
        self.revision += 1;
        self.unsettled = true;
    }

    /// Piece `piece`'s body as it moves now: its own body, or for a piece of
    /// a toppling top, a body at its place in the top moving with it.
    fn piece_state(&self, piece: usize) -> Body {
        let p = &self.pieces[piece];
        let body = self.world[p.body];
        let Some((offset, rotation)) = p.local else {
            return body;
        };
        let mut state = body;
        let pos = body.to_world(offset);
        state.vel = body.vel + body.omega_world().cross(pos - body.pos);
        state.omega = rotation.inverse() * body.omega;
        state.pos = pos;
        state.orientation = body.orientation * rotation;
        state.mass = self.specs[piece].mass;
        state
    }

    /// Stops the colliders a toppling top carries for `piece`.
    fn drop_parts(&mut self, piece: usize) {
        let mut empty = Vec::new();
        for (&c, members) in &mut self.parts {
            members.retain(|&p| p != piece);
            if members.is_empty() {
                empty.push(c);
            }
        }
        for c in empty {
            self.parts.remove(&c);
            self.world.collider_mut(physics::ColliderId(c)).filter = Filter::NONE;
        }
    }

    /// Takes `piece` out of its toppling top: its collider stops colliding,
    /// and a top left with no pieces is removed.
    fn detach(&mut self, piece: usize) {
        self.drop_parts(piece);
        self.pieces[piece].local = None;
        if let Some(f) = self.falls.iter().position(|f| f.members.contains(&piece)) {
            self.falls[f].members.retain(|&m| m != piece);
            if self.falls[f].members.is_empty() {
                let fall = self.falls.remove(f);
                for joint in fall.joints {
                    self.world.remove_joint(joint);
                }
                self.world.remove_body(fall.body);
            }
        }
    }

    /// Toppling top `index` strikes the ground: each of its blocks becomes a
    /// loose body of its own moving as it moved, and with `shatter`, the
    /// fastest of them, which struck hardest, break into their chunks.
    fn crash(&mut self, index: usize, shatter: bool) {
        let fall = self.falls.remove(index);
        for &joint in &fall.joints {
            self.world.remove_joint(joint);
        }
        let mut moving: Vec<(f64, usize)> = Vec::with_capacity(fall.members.len());
        let mut lowest = DVec3::splat(f64::INFINITY);
        for &m in &fall.members {
            let state = self.piece_state(m);
            if state.pos.y < lowest.y {
                lowest = state.pos;
            }
            let spec = &self.specs[m];
            let mut body = Body::new(
                spec.mass,
                Body::box_inertia(spec.mass, spec.size),
                state.pos,
            );
            body.orientation = state.orientation;
            body.prev_orientation = state.orientation;
            let spread = DVec3::new(self.unit(), self.unit().abs(), self.unit()) * 0.6;
            body.vel = state.vel + spread;
            body.omega = state.omega;
            let id = self.world.add(body);
            // One box for the block, the bounds of its chunks' boxes.
            let (a, b) = self.specs[m].colliders.iter().fold(
                (DVec3::splat(f64::INFINITY), DVec3::splat(f64::NEG_INFINITY)),
                |(a, b), c| {
                    let h = world_half(c.rotation, c.half);
                    (a.min(c.center - h), b.max(c.center + h))
                },
            );
            let (center, half) = if a.is_finite() && b.is_finite() {
                ((a + b) * 0.5, ((b - a) * 0.5).max(DVec3::splat(0.05)))
            } else {
                (DVec3::ZERO, self.specs[m].size * 0.5)
            };
            self.world.add_collider(
                Collider::new(id, Shape::Cuboid { half })
                    .at(center, DQuat::IDENTITY)
                    .with_material(MATERIAL),
            );
            self.own(id, Some(m));
            self.drop_parts(m);
            let piece = &mut self.pieces[m];
            piece.body = id;
            piece.local = None;
            piece.rubble = true;
            moving.push((state.vel.length(), m));
        }
        self.world.remove_body(fall.body);
        self.revision += 1;
        self.unsettled = true;
        if !shatter {
            return;
        }
        // The blocks that came down fastest, the far end, burst apart.
        moving.sort_by(|a, b| b.0.total_cmp(&a.0));
        for &(speed, m) in moving.iter().take(CRASH_BREAKS) {
            if speed < CRASH_SHATTER {
                break;
            }
            let at = self.world[self.pieces[m].body].pos;
            let matter = self.specs[m].matter;
            self.shatter(
                m,
                Push::Along {
                    point: at,
                    push: DVec3::Y * (1.0 + 0.2 * speed) + fall.toward * 0.15 * speed,
                },
            );
            self.dust(at.as_vec3(), 2, 2.4, matter);
        }
        for &(_, m) in moving.iter().step_by(3) {
            if self.pieces[m].status == Status::Loose {
                let at = self.world[self.pieces[m].body].pos.as_vec3();
                let matter = self.specs[m].matter;
                self.dust(at, 2, 2.0, matter);
            }
        }
        let size = (fall.members.len() as f32 / 40.0).clamp(0.3, 1.0);
        self.crashes.push(Crash {
            at: lowest.as_vec3(),
            size,
        });
    }

    /// Leaves each toppling top's hinge once it leans far enough, and
    /// breaks up the ones that struck the ground or have leaned too long.
    fn tend_falls(&mut self) {
        let now = self.world.time();
        let mut index = 0;
        while index < self.falls.len() {
            let body = self.world[self.falls[index].body];
            let tilt = (body.orientation * DVec3::Y)
                .dot(DVec3::Y)
                .clamp(-1.0, 1.0)
                .acos();
            // It leaves the hinge once it leans far enough, or once nothing
            // under the hinge stands: the joints hold it to the ground's
            // body at a fixed point, which would leave it hanging there.
            let unhinged = !self.falls[index]
                .hinge
                .iter()
                .any(|&i| self.pieces[i].status == Status::Standing);
            if !self.falls[index].joints.is_empty()
                && (tilt > HINGE_RELEASE.to_radians() || unhinged)
            {
                let joints = std::mem::take(&mut self.falls[index].joints);
                for joint in joints {
                    self.world.remove_joint(joint);
                }
            }
            let fall = &self.falls[index];
            let grounded = self.world.contacts.iter().any(|r| {
                (r.body_a == fall.body && r.body_b == BodyId(0))
                    || (r.body_b == fall.body && r.body_a == BodyId(0))
            });
            let still = body.vel.length() < 0.15 && now - fall.since > 2.0;
            if (grounded && tilt > CRASH_TILT.to_radians())
                || tilt > 80f64.to_radians()
                || now - fall.since > FALL_LIMIT
                || still
            {
                self.crash(index, true);
                self.support();
                continue;
            }
            index += 1;
        }
    }

    /// The physics world's counts and timings for its last step.
    #[must_use]
    pub fn stats(&self) -> physics::StepStats {
        self.world.stats
    }

    /// How many tall buildings' tops are toppling now.
    #[must_use]
    pub fn toppling(&self) -> usize {
        self.falls.len()
    }

    /// The toppled tops that struck the ground since the last call.
    pub fn take_crashes(&mut self) -> Vec<Crash> {
        std::mem::take(&mut self.crashes)
    }

    /// Whether each standing carved block is held up: it stands on the
    /// ground, on a block that is, or within [`MAX_SPAN`] blocks sideways
    /// of one that a column holds up.
    fn block_holds(&self) -> Vec<bool> {
        let mut holds = vec![false; self.pieces.len()];
        for members in self.members.values() {
            if !members
                .iter()
                .any(|&i| matches!(self.specs[i].role, Role::Block { .. }))
            {
                continue;
            }
            let at = |k: u16| members.get(usize::from(k)).copied();
            let mut cost: Vec<u8> = members
                .iter()
                .map(|&i| {
                    if self.standing(i) && self.specs[i].link.footing {
                        0
                    } else {
                        u8::MAX
                    }
                })
                .collect();
            let mut changed = true;
            while changed {
                changed = false;
                for (k, &i) in members.iter().enumerate() {
                    if !self.standing(i) || cost[k] == 0 {
                        continue;
                    }
                    let link = &self.specs[i].link;
                    let mut best = cost[k];
                    for &u in &link.under {
                        if let Some(j) = at(u)
                            && self.standing(j)
                        {
                            best = best.min(cost[usize::from(u)]);
                        }
                    }
                    for &b in &link.beside {
                        if let Some(j) = at(b)
                            && self.standing(j)
                            && cost[usize::from(b)] < MAX_SPAN
                        {
                            best = best.min(cost[usize::from(b)] + 1);
                        }
                    }
                    if best < cost[k] {
                        cost[k] = best;
                        changed = true;
                    }
                }
            }
            for (k, &i) in members.iter().enumerate() {
                holds[i] = cost[k] <= MAX_SPAN;
            }
        }
        holds
    }

    /// The top story of `building`.
    fn top(&self, building: usize) -> u8 {
        self.tops.get(&building).copied().unwrap_or(0)
    }

    /// When a roof span comes loose, what is left of a weak outer eave line
    /// under it can't carry it alone: the top story's sections and corner
    /// posts there crumble outward and the roof crashes down after them.
    fn buckle(&mut self, roof: usize) {
        let building = self.specs[roof].building;
        let top = self.top(building);
        let Role::Roof { span, spans } = self.specs[roof].role else {
            return;
        };
        let members = self.members.get(&building).cloned().unwrap_or_default();
        for line in [Side::West, Side::East] {
            let outer = match line {
                Side::West => span == 0,
                _ => span + 1 == spans,
            };
            if !outer {
                continue;
            }
            // The line, its corner posts, and the end walls' sections at
            // its corners, on the top story.
            let on_line = |spec: &PieceSpec| match spec.role {
                Role::Wall { side, story, .. } if side == line => story == top,
                Role::Wall {
                    side: Side::South | Side::North,
                    index,
                    count,
                    story,
                } => {
                    story == top
                        && ((line == Side::West && index == 0)
                            || (line == Side::East && index + 1 == count))
                }
                Role::Post { b, story, .. } => b == line && story == top,
                _ => false,
            };
            let sections = members
                .iter()
                .filter(|&&i| self.standing(i))
                .filter(|&&i| {
                    matches!(self.specs[i].role,
                        Role::Wall { side, story, .. } if side == line && story == top)
                })
                .count();
            if sections >= 2 {
                continue;
            }
            let weak: Vec<usize> = members
                .iter()
                .copied()
                .filter(|&i| self.standing(i) && on_line(&self.specs[i]))
                .collect();
            let out = match line {
                Side::West => -DVec3::X,
                _ => DVec3::X,
            };
            for piece in weak {
                let at = self.world[self.pieces[piece].body].pos;
                self.shatter(
                    piece,
                    Push::Along {
                        point: at + DVec3::Y,
                        push: out * 2.5,
                    },
                );
            }
        }
    }

    /// Whether what holds `piece` up still stands.
    fn held(&self, piece: usize) -> bool {
        let spec = &self.specs[piece];
        let building = spec.building;
        let top = self.top(building);
        let members = self.members.get(&building).map_or(&[][..], Vec::as_slice);
        let others = || {
            members
                .iter()
                .copied()
                .filter(move |&i| i != piece && self.standing(i))
                .map(|i| self.specs[i].role)
        };
        // Standing top-story sections on `line` whose index is in `range`.
        let sections = |line: Side, range: std::ops::RangeInclusive<i32>| {
            others()
                .filter(|r| {
                    matches!(r, Role::Wall { side, index, story, .. }
                        if *side == line && *story == top && range.contains(&i32::from(*index)))
                })
                .count()
        };
        let every = i32::MIN..=i32::MAX;
        let roof =
            |span: u8| others().any(|r| matches!(r, Role::Roof { span: s, .. } if s == span));
        // An eave between two spans rests on the south and north lines'
        // sections around it.
        let inner = |boundary: u8| {
            let at = i32::from(boundary) * i32::from(SPAN_SECTIONS);
            sections(Side::South, at - 2..=at + 1) + sections(Side::North, at - 2..=at + 1) >= 2
        };
        match spec.role {
            Role::Wall {
                side,
                index,
                count,
                story,
            } => {
                let beside = others().any(|role| match role {
                    Role::Wall {
                        side: s,
                        index: i,
                        story: t,
                        ..
                    } => t == story && s == side && (i + 1 == index || index + 1 == i),
                    Role::Post { a, b, story: t } => {
                        let ends = |end: Side| {
                            (index == 0 && matches!(end, Side::West | Side::South))
                                || (index + 1 == count && matches!(end, Side::East | Side::North))
                        };
                        t == story && ((a == side && ends(b)) || (b == side && ends(a)))
                    }
                    _ => false,
                });
                let below = story == 0
                    || others().any(|r| {
                        matches!(r, Role::Wall { side: s, index: i, story: t, .. }
                            if s == side && i == index && t + 1 == story)
                    });
                beside && below
            }
            Role::Post { a, b, story } => {
                let beside = others().any(|role| match role {
                    Role::Wall {
                        side,
                        index,
                        count,
                        story: t,
                    } => {
                        let first = index == 0;
                        let last = index + 1 == count;
                        let at = |line: Side, end: Side| {
                            side == line
                                && ((first && matches!(end, Side::West | Side::South))
                                    || (last && matches!(end, Side::East | Side::North)))
                        };
                        t == story && (at(a, b) || at(b, a))
                    }
                    _ => false,
                });
                let below = story == 0
                    || others().any(|r| {
                        matches!(r, Role::Post { a: pa, b: pb, story: t }
                            if pa == a && pb == b && t + 1 == story)
                    });
                beside && below
            }
            Role::Roof { span, spans } => {
                let west = if span == 0 {
                    sections(Side::West, every.clone()) >= 2
                } else {
                    inner(span)
                };
                let east = if span + 1 == spans {
                    sections(Side::East, every) >= 2
                } else {
                    inner(span + 1)
                };
                west && east
            }
            Role::Gable { side, span } => {
                let first = i32::from(span) * i32::from(SPAN_SECTIONS);
                roof(span) && sections(side, first..=first + i32::from(SPAN_SECTIONS) - 1) >= 1
            }
            Role::Chimney { span } => roof(span),
            Role::Block { .. } => self.block_holds()[piece],
        }
    }

    /// Makes a standing piece dynamic. Walls, posts, and gables tip outward
    /// from their building so they topple instead of balancing.
    fn loosen(&mut self, piece: usize) {
        let spec = &self.specs[piece];
        let id = self.pieces[piece].body;
        self.pieces[piece].status = Status::Loose;
        self.revision += 1;
        self.unsettled = true;
        let outward = match spec.role {
            Role::Wall { side, .. } | Role::Gable { side, .. } => Some(side),
            Role::Post { a, .. } => Some(a),
            Role::Roof { .. } | Role::Chimney { .. } | Role::Block { .. } => None,
        }
        .map(|side| match side {
            Side::South => -DVec3::Z,
            Side::North => DVec3::Z,
            Side::West => -DVec3::X,
            Side::East => DVec3::X,
        })
        .or_else(|| {
            // A carved block leans away from its building's middle.
            if !matches!(spec.role, Role::Block { .. }) {
                return None;
            }
            let members = self.members.get(&spec.building)?;
            let middle = members.iter().map(|&i| self.specs[i].center).sum::<DVec3>()
                / members.len().max(1) as f64;
            DVec3::new(spec.center.x - middle.x, 0.0, spec.center.z - middle.z).try_normalize()
        });
        let body = &mut self.world[id];
        body.kind = BodyKind::Dynamic;
        body.wake();
        if let Some(out) = outward {
            let top = body.pos + DVec3::Y * spec.size.y * 0.5;
            body.apply_impulse_at(out * body.mass * TIP, top);
        }
        wake_near(&mut self.world, id);
    }

    /// Wakes body `id`, and moves it again if it was frozen at rest.
    fn rouse(&mut self, id: BodyId) {
        if self.frozen.remove(&id.0) && !self.world[id].removed {
            self.world[id].kind = BodyKind::Dynamic;
            self.unsettled = true;
        }
        self.rests.remove(&id.0);
        self.resting.remove(&id.0);
        self.world.wake(id);
    }

    /// Moves frozen debris within [`ROUSE`] m of `at` again, so nothing is
    /// left resting on what is gone.
    fn rouse_near(&mut self, at: DVec3) {
        let near: Vec<u32> = self
            .frozen
            .iter()
            .copied()
            .filter(|&b| self.world[BodyId(b)].pos.distance(at) < ROUSE)
            .collect();
        for b in near {
            self.rouse(BodyId(b));
        }
    }

    /// Whether body `id` holds up debris frozen on it: the ground, a
    /// standing piece, or frozen debris that is itself held.
    fn holds(&self, id: u32) -> bool {
        if id == 0 {
            return true;
        }
        let Some(body) = self.world.bodies().get(id as usize) else {
            return false;
        };
        if body.removed || body.kind != BodyKind::Static {
            return false;
        }
        if self.frozen.contains(&id) {
            return self.rests.contains_key(&id);
        }
        // A static body that isn't frozen is a standing piece.
        self.owner_of(BodyId(id))
            .is_some_and(|piece| self.pieces[piece].status == Status::Standing)
    }

    /// Moves again every frozen body with no chain of frozen debris down to
    /// the ground or a standing piece, and wakes what sleeps on it, so
    /// nothing is left frozen in the air when what it rested on goes. It
    /// runs only after something debris may rest on has gone, and costs
    /// one pass over the frozen debris for each layer that thaws.
    fn hold_frozen(&mut self) {
        if !self.unsettled {
            return;
        }
        loop {
            let thaw: Vec<u32> = self
                .rests
                .iter()
                .filter(|(_, under)| !under.iter().any(|&u| self.holds(u)))
                .map(|(&id, _)| id)
                .collect();
            if thaw.is_empty() {
                break;
            }
            for id in thaw {
                self.rests.remove(&id);
                self.rouse(BodyId(id));
                wake_near(&mut self.world, BodyId(id));
            }
        }
        self.unsettled = false;
    }

    /// Removes chunk body `id` when it ends, waking what sleeps on it and
    /// checking the debris frozen on it.
    fn remove_chunk(&mut self, id: BodyId) {
        wake_near(&mut self.world, id);
        self.world.remove_body(id);
        self.frozen.remove(&id.0);
        self.rests.remove(&id.0);
        self.resting.remove(&id.0);
        self.unsettled = true;
    }

    /// What the last tick's physics steps cost.
    #[must_use]
    pub fn step_cost(&self) -> StepCost {
        self.steps
    }

    /// Advances the yard by `dt` seconds of wall time.
    pub fn tick(&mut self, dt: f32) {
        self.steps = StepCost::default();
        self.age_dust(dt);
        self.hold_frozen();
        let any_moving = self.pieces.iter().any(|p| match p.status {
            Status::Standing => false,
            Status::Loose => true,
            Status::Broken => p.chunks.iter().any(|c| !c.gone),
        });
        if !any_moving {
            self.pending = 0.0;
            return;
        }
        self.pending += f64::from(dt.clamp(0.0, 0.25));
        let mut steps = 0;
        while self.pending >= STEP && steps < MAX_STEPS {
            self.pending -= STEP;
            steps += 1;
            self.step();
        }
        if steps == MAX_STEPS {
            self.pending = 0.0;
        }
    }

    /// One physics step, then impact damage, tilt breaks, and chunk ends.
    fn step(&mut self) {
        let gravity = Uniform(DVec3::new(0.0, -9.81, 0.0));
        self.wet_debris();
        self.world.step(&gravity);
        let stats = &self.world.stats;
        self.steps.steps += 1;
        self.steps.detect_ms += stats.detect.as_secs_f32() * 1000.0;
        self.steps.solve_ms += stats.solve.as_secs_f32() * 1000.0;
        self.steps.awake = self.steps.awake.max(stats.awake);
        self.steps.contacts = self.steps.contacts.max(stats.contact_points);
        let now = self.world.time();
        self.thrown.retain(|&(_, until)| until > now);
        // Bodies that touch something fixed this step (the ground, a floor,
        // or debris already frozen): only these may freeze, so a piece
        // slowed at the top of its arc or resting on moving debris never
        // freezes in midair.
        let mut supported: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        {
            let bodies = self.world.bodies();
            let fixed = |id: physics::BodyId| {
                bodies
                    .get(id.0 as usize)
                    .is_some_and(|b| b.kind == BodyKind::Static)
            };
            for c in &self.world.contacts {
                for (body, other) in [(c.body_a, c.body_b), (c.body_b, c.body_a)] {
                    if fixed(other) {
                        let under = supported.entry(body.0).or_default();
                        if !under.contains(&other.0) {
                            under.push(other.0);
                        }
                    }
                }
            }
        }
        for (index, body) in self.world.bodies_mut().iter_mut().enumerate() {
            if body.kind == BodyKind::Dynamic {
                let thrown = self
                    .thrown
                    .binary_search_by_key(&(index as u32), |(id, _)| id.0)
                    .is_ok();
                let falling = self.falls.iter().any(|f| f.body.0 as usize == index);
                let (speed, spin) = if falling {
                    (FALL_SPEED, FALL_SPIN)
                } else if thrown {
                    (BLAST_SPEED, MAX_SPIN * 2.0)
                } else {
                    (MAX_SPEED, MAX_SPIN)
                };
                body.vel = body.vel.clamp_length_max(speed);
                body.omega = body.omega.clamp_length_max(spin);
                // A chunk or a fallen top's block is debris; a fallen
                // top's blocks barely moving come to rest quickly.
                let (debris, rubble) = match self.owner.get(index).copied().flatten() {
                    None => (!falling, false),
                    Some(piece) => (self.pieces[piece].rubble, self.pieces[piece].rubble),
                };
                if rubble && body.vel.length() < SETTLE_SPEED {
                    body.vel *= SETTLE_DAMPING;
                    body.omega *= SETTLE_DAMPING;
                }
                if debris {
                    // Debris in the water drifts on the current as fast as
                    // rubble settles, so it freezes only once lodged: all
                    // but still against a bank or on the bed.
                    let freeze = if self.wet.contains(&(index as u32)) {
                        LODGED_SPEED
                    } else {
                        FREEZE_SPEED
                    };
                    let slow = body.vel.length() < freeze
                        && body.omega.length() < 2.0 * freeze
                        && supported.contains_key(&(index as u32));
                    let rest = self.resting.entry(index as u32).or_insert(0.0);
                    *rest = if slow { *rest + STEP } else { 0.0 };
                    if *rest > FREEZE_AFTER {
                        body.kind = BodyKind::Static;
                        body.vel = DVec3::ZERO;
                        body.omega = DVec3::ZERO;
                        body.sleeping = false;
                        self.frozen.insert(index as u32);
                        self.resting.remove(&(index as u32));
                        if let Some(under) = supported.remove(&(index as u32)) {
                            self.rests.insert(index as u32, under);
                        }
                    }
                }
            }
        }
        // Impacts: the summed contact impulse between each pair of bodies.
        // In first-contact order, found through a map: a swarm's thousands
        // of contacts made the linear search quadratic.
        let mut pairs: Vec<(BodyId, BodyId, f64, DVec3)> = Vec::new();
        let mut seen: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for report in &self.world.contacts {
            let (a, b) = if report.body_a <= report.body_b {
                (report.body_a, report.body_b)
            } else {
                (report.body_b, report.body_a)
            };
            match seen.get(&(a.0, b.0)) {
                Some(&k) => pairs[k].2 += report.impulse.length(),
                None => {
                    seen.insert((a.0, b.0), pairs.len());
                    pairs.push((a, b, report.impulse.length(), report.point));
                }
            }
        }
        let mut hurt: Vec<(usize, i32, DVec3)> = Vec::new();
        for (a, b, impulse, point) in pairs {
            if impulse <= IMPACT {
                continue;
            }
            let damage = ((impulse - IMPACT) / IMPACT_PER_POINT) as i32;
            for body in [a, b] {
                if let Some(piece) = self.owner_of(body) {
                    hurt.push((piece, damage, point));
                }
            }
        }
        for (piece, damage, point) in hurt {
            // A fallen top's blocks broke up when it struck; tumbling on
            // their pile doesn't break them further.
            if self.pieces[piece].status != Status::Broken && !self.pieces[piece].rubble {
                self.dust(point.as_vec3(), 2, 0.8, self.specs[piece].matter);
                self.damage(piece, damage, point, DVec3::ZERO);
            }
        }
        // A loose piece that has fallen past its tilt limit breaks.
        for piece in 0..self.pieces.len() {
            let state = &self.pieces[piece];
            if state.status != Status::Loose || state.local.is_some() || state.rubble {
                continue;
            }
            let spec = &self.specs[piece];
            let body = &self.world[self.pieces[piece].body];
            let up = body.orientation * DVec3::Y;
            let built = spec.orientation * DVec3::Y;
            let limit = if matches!(spec.role, Role::Roof { .. }) {
                50.0
            } else {
                62.0
            };
            if up.dot(built) < f64::to_radians(limit).cos() {
                let at = body.pos;
                self.shatter(
                    piece,
                    Push::Along {
                        point: at,
                        push: DVec3::ZERO,
                    },
                );
                self.support();
            }
        }
        self.tend_falls();
        let time = self.world.time();
        let mut ended = Vec::new();
        for piece in &mut self.pieces {
            for chunk in &mut piece.chunks {
                if !chunk.gone && time >= chunk.until {
                    chunk.gone = true;
                    ended.push(chunk.body);
                }
            }
        }
        for body in ended {
            self.remove_chunk(body);
        }
        self.hold_frozen();
    }

    /// Water's part of a step: debris over the water passes the slab to the
    /// beds, floats or sinks by its matter, and drifts with the current;
    /// debris that meets the surface splashes, and burning debris hisses.
    fn wet_debris(&mut self) {
        let Some(water) = self.water else {
            return;
        };
        let tick = (self.world.time() / STEP).round() as u64;
        // Every moving debris body and its matter.
        let mut moving: BTreeMap<u32, Matter> = BTreeMap::new();
        for (index, piece) in self.pieces.iter().enumerate() {
            let matter = self.specs[index].matter;
            match piece.status {
                Status::Standing => {}
                Status::Loose => {
                    moving.insert(piece.body.0, matter);
                }
                Status::Broken => {
                    for c in piece.chunks.iter().filter(|c| !c.gone) {
                        moving.insert(c.body.0, matter);
                    }
                }
            }
        }
        moving.retain(|id, _| {
            self.world
                .bodies()
                .get(*id as usize)
                .is_some_and(|b| b.kind == BodyKind::Dynamic && !b.removed)
        });
        let mut over: BTreeMap<u32, Matter> = BTreeMap::new();
        let now = self.world.time();
        for (&id, &matter) in &moving {
            let body = &self.world[BodyId(id)];
            let Some(sample) = water.sample(body.pos.x, body.pos.z, tick) else {
                continue;
            };
            // Over the water: no slab under it.
            over.insert(id, matter);
            let under = body.pos.y < sample.height;
            if under && !self.wet.contains(&id) {
                self.wet.insert(id);
                let hiss = self.thrown.binary_search_by_key(&id, |(b, _)| b.0).is_ok();
                self.splashes.push(Splash {
                    at: Vec3::new(body.pos.x as f32, sample.height as f32, body.pos.z as f32),
                    speed: (-body.vel.y).max(0.0) as f32,
                    mass: body.mass as f32,
                    hiss,
                    matter,
                });
                // Floating debris lasts longer, to drift and lodge.
                let life = self.debris_lifetime * 3.0;
                for piece in &mut self.pieces {
                    for chunk in &mut piece.chunks {
                        if chunk.body.0 == id && !chunk.gone {
                            chunk.until = chunk.until.max(now + life);
                        }
                    }
                }
            } else if !under && body.pos.y > sample.height + 0.5 {
                self.wet.remove(&id);
            }
        }
        self.wet.retain(|id| over.contains_key(id));
        // The slab lies only under debris that is not over the water.
        for index in 0..self.world.colliders().len() {
            let collider = self.world.colliders()[index];
            if !moving.contains_key(&collider.body.0) || collider.filter == Filter::NONE {
                continue;
            }
            let filter = if over.contains_key(&collider.body.0) {
                WET
            } else {
                Filter::ALL
            };
            if collider.filter != filter {
                self.world
                    .collider_mut(physics::ColliderId(index as u32))
                    .filter = filter;
            }
        }
        // Each body floats as its matter does, whatever mass it was given
        // to fall well: its buoyancy is scaled by its mass over the mass
        // its colliders would have in that matter.
        let mut volume: BTreeMap<u32, f64> = BTreeMap::new();
        for c in self.world.colliders() {
            if over.contains_key(&c.body.0) {
                *volume.entry(c.body.0).or_default() += physics::water::volume(&c.shape);
            }
        }
        let lift: BTreeMap<u32, f64> = over
            .iter()
            .filter_map(|(&id, &matter)| {
                let v = *volume.get(&id)?;
                let m = self.world.bodies()[id as usize].mass;
                (v > 0.0).then(|| (id, m / (matter.density() * v)))
            })
            .collect();
        physics::water::apply_scaled(
            &mut self.world,
            water,
            tick,
            STEP,
            &physics::water::Settings::default(),
            |id| lift.get(&id.0).copied(),
        );
    }

    /// Adds `count` puffs of `matter`'s dust at `at`, `scale` times the
    /// size and speed of a hammer blow's.
    pub fn dust(&mut self, at: Vec3, count: usize, scale: f32, matter: Matter) {
        let base = match matter {
            Matter::Tile | Matter::Brick => [0.4, 0.27, 0.21],
            _ => [0.44, 0.4, 0.33],
        };
        for _ in 0..count {
            if self.puffs.len() >= MAX_PUFFS {
                self.puffs.remove(0);
            }
            let vel = Vec3::new(
                self.unit() as f32,
                0.4 + self.unit().abs() as f32 * 0.8,
                self.unit() as f32,
            ) * 1.2
                * scale;
            let shade = 0.85 + 0.15 * self.unit() as f32;
            let life = 1.2 + self.unit().abs() as f32 * 1.3;
            let size = (0.25 + self.unit().abs() as f32 * 0.25) * scale.max(0.5);
            self.puffs.push(Puff {
                at: at + vel * 0.1,
                vel,
                age: 0.0,
                life,
                size,
                color: base.map(|c| c * shade),
            });
        }
    }

    fn age_dust(&mut self, dt: f32) {
        for puff in &mut self.puffs {
            puff.age += dt;
            puff.at += puff.vel * dt;
            puff.vel *= (1.0 - 1.8 * dt).max(0.0);
            puff.vel.y += 0.15 * dt;
        }
        self.puffs.retain(|p| p.age < p.life);
    }

    /// A die roll from 1 to `sides`, from the yard's seeded dice.
    pub fn roll(&mut self, sides: u32) -> i32 {
        (self.next() % u64::from(sides)) as i32 + 1
    }

    /// A value in `-1..1`.
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    }

    /// SplitMix64.
    fn next(&mut self) -> u64 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Standing walls and posts as footprints with their tops, for the
    /// player's controller. Loose pieces and chunks never block, so falling
    /// debris cannot trap the player.
    #[must_use]
    pub fn blocks(&self) -> Vec<(crate::controller::Footprint, f32)> {
        let mut blocks = Vec::new();
        for (spec, piece) in self.specs.iter().zip(&self.pieces) {
            if !spec.blocks || piece.status != Status::Standing {
                continue;
            }
            let pose = spec.pose();
            let bottom = |collider: &Cuboid| {
                let frame = pose * collider.frame();
                let half = collider.half.as_vec3();
                frame.transform_point3(Vec3::new(0.0, -half.y, 0.0)).y
            };
            let lowest = spec
                .colliders
                .iter()
                .map(bottom)
                .fold(f32::INFINITY, f32::min);
            for collider in &spec.colliders {
                let frame = pose * collider.frame();
                let half = collider.half.as_vec3();
                // A lintel over a doorway does not block feet.
                if bottom(collider) > lowest + 1.0 {
                    continue;
                }
                let (mut min, mut max) =
                    (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
                for i in 0..8 {
                    let corner = Vec3::new(
                        if i & 1 == 0 { -half.x } else { half.x },
                        if i & 2 == 0 { -half.y } else { half.y },
                        if i & 4 == 0 { -half.z } else { half.z },
                    );
                    let p = frame.transform_point3(corner);
                    min = min.min(p);
                    max = max.max(p);
                }
                blocks.push((
                    crate::controller::Footprint {
                        min: [min.x, min.z],
                        max: [max.x, max.z],
                    },
                    max.y,
                ));
            }
        }
        blocks
    }

    /// Where, from 0 to 1, the segment from `from` to `to` first enters a
    /// standing piece's box, or `None` when it meets none. Loose pieces and
    /// chunks don't count: a ray at a wall sees through the debris in front
    /// of it. Each box is tested against its bounding sphere first.
    #[must_use]
    pub fn ray(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let along = to - from;
        let length = along.length();
        if !(length > 1e-6) {
            return None;
        }
        let unit = along / length;
        let mut best: Option<f32> = None;
        for (index, (spec, piece)) in self.specs.iter().zip(&self.pieces).enumerate() {
            if piece.status != Status::Standing {
                continue;
            }
            let pose = self.piece_pose(index);
            for collider in &spec.colliders {
                let frame = pose * collider.frame();
                let center = frame.w_axis.truncate();
                let reach = collider.half.as_vec3().length();
                // The closest the segment's line comes to the box's center.
                let k = (center - from).dot(unit).clamp(0.0, length);
                if (from + unit * k).distance_squared(center) > reach * reach {
                    continue;
                }
                let inverse = frame.inverse();
                let half = collider.half.as_vec3();
                if let Some(t) = verse_world::social::sight::box_hit(
                    inverse.transform_point3(from),
                    inverse.transform_point3(to),
                    0.0,
                    -half,
                    half,
                ) && best.is_none_or(|b| t < b)
                {
                    best = Some(t);
                }
            }
        }
        best
    }

    /// The world pose of a standing or loose piece's body.
    #[must_use]
    pub fn piece_pose(&self, piece: usize) -> Mat4 {
        let pose = self.body_pose(self.pieces[piece].body);
        match self.pieces[piece].local {
            Some((offset, rotation)) => {
                pose * Mat4::from_rotation_translation(rotation.as_quat(), offset.as_vec3())
            }
            None => pose,
        }
    }

    /// Whether chunk `index` of a broken piece is frozen where it came to
    /// rest, until something strikes near it.
    #[must_use]
    pub fn chunk_at_rest(&self, piece: usize, index: usize) -> bool {
        self.pieces[piece]
            .chunks
            .get(index)
            .is_some_and(|chunk| !chunk.gone && self.frozen.contains(&chunk.body.0))
    }

    /// The world pose of chunk `index` of a broken piece, or `None` while
    /// the piece is whole or once the chunk is gone.
    #[must_use]
    pub fn chunk_pose(&self, piece: usize, index: usize) -> Option<Mat4> {
        let state = &self.pieces[piece];
        let chunk = state.chunks.get(index)?;
        (!chunk.gone).then(|| self.body_pose(chunk.body))
    }
}

/// Which way an explosion at `center` throws something at `at`: outward
/// and [`BLAST_UPWARD`] degrees up, or straight up at the center.
fn blast_direction(center: DVec3, at: DVec3, face: DVec3) -> DVec3 {
    if face != DVec3::ZERO {
        // Off a wall: out of its face, spreading from the strike, and up.
        let spread = (at - center).reject_from(face).normalize_or_zero();
        return (face + spread * 0.55 + DVec3::Y * 0.35).normalize();
    }
    let up = BLAST_UPWARD.to_radians();
    match DVec3::new(at.x - center.x, 0.0, at.z - center.z).try_normalize() {
        Some(out) => out * up.cos() + DVec3::Y * up.sin(),
        None => DVec3::Y,
    }
}

/// The half extents along the world's axes of a box with half extents
/// `half` turned by `rotation`.
fn world_half(rotation: DQuat, half: DVec3) -> DVec3 {
    let m = glam::DMat3::from_quat(rotation);
    DVec3::new(
        m.row(0).abs().dot(half),
        m.row(1).abs().dot(half),
        m.row(2).abs().dot(half),
    )
}

/// Wakes every sleeping body whose colliders come near `body`'s, so what
/// rested on a piece falls when the piece goes (`wall_of_stone::rig`).
fn wake_near(world: &mut World, body: BodyId) {
    let reach = world.solver.margin + 0.1;
    let near: Vec<BodyId> = {
        let colliders = world.colliders();
        let own: Vec<(DVec3, f64)> = colliders
            .iter()
            .filter(|c| c.body == body)
            .map(|c| (c.pose(world).0, c.shape.bound()))
            .collect();
        colliders
            .iter()
            .filter(|c| c.body != body && world[c.body].sleeping)
            .filter(|c| {
                let (at, bound) = (c.pose(world).0, c.shape.bound());
                own.iter().any(|(p, r)| p.distance(at) <= r + bound + reach)
            })
            .map(|c| c.body)
            .collect()
    };
    for id in near {
        world.wake(id);
    }
}

/// What the sledgehammer and Meteor Swarm act on: the yard's site, or a
/// town that raises its buildings into a site when something reaches them.
pub trait Target {
    /// Whether `point` is within `reach` of something a meteor bursts on.
    fn touches(&self, point: Vec3, reach: f32) -> bool;
    /// An explosion at `center`, as [`Site::explode`].
    fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow>;
    /// An explosion on a wall facing `face`, as [`Site::explode_facing`].
    fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        let _ = face;
        self.explode(center, radius, damage, speed)
    }
    /// A hammer blow along `path`, as [`Site::strike`].
    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow>;
    /// A die roll from 1 to `sides`.
    fn roll(&mut self, sides: u32) -> i32;
    /// Where, from 0 to 1, the segment from `from` to `to` first meets
    /// something standing or the ground, or `None` when it stays clear:
    /// what a targeting ray sees and what a meteor's path must miss.
    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32>;
}

impl Target for Site {
    fn touches(&self, point: Vec3, reach: f32) -> bool {
        Site::touches(self, point, reach)
    }

    fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow> {
        Site::explode(self, center, radius, damage, speed)
    }

    fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        Site::explode_facing(self, center, radius, damage, speed, face)
    }

    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow> {
        Site::strike(self, path, push, reach)
    }

    fn roll(&mut self, sides: u32) -> i32 {
        Site::roll(self, sides)
    }

    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let ground =
            verse_world::social::sight::ground_hit(from, to, 0.0, &crate::zones::everglade::height);
        match (Site::ray(self, from, to), ground) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

#[cfg(test)]
#[path = "sky_tests.rs"]
mod sky_tests;
