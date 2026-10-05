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
use physics::{Body, BodyId, BodyKind, Collider, Filter, Material, Shape, Uniform, World};

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

/// What a piece is in its building, which decides what holds it up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Section `index` of `count` along the `side` wall line, from the
    /// west or the south end.
    Wall { side: Side, index: u8, count: u8 },
    /// The corner post where the `a` and `b` lines meet; `a` is south or
    /// north.
    Post { a: Side, b: Side },
    /// The roof, which bears on the west and east wall lines.
    Roof,
    /// The brick gable over the `side` end wall.
    Gable { side: Side },
    /// The chimney, through the roof.
    Chimney,
}

/// What a piece is made of: its dust's color, hit points, and mass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Matter {
    Plaster,
    Timber,
    Tile,
    Brick,
}

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
    /// Every chunk flies away from `center` at up to `speed`, falling off
    /// to nothing at `radius`, as from an explosion.
    From {
        center: DVec3,
        speed: f64,
        radius: f64,
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
pub struct Site {
    specs: Vec<PieceSpec>,
    world: World,
    pieces: Vec<Piece>,
    puffs: Vec<Puff>,
    seed: u64,
    rng: u64,
    /// Simulated time not yet stepped, s.
    pending: f64,
    /// Bumped whenever a piece stops standing, so blockers are rebuilt.
    revision: u64,
    /// Bodies an explosion threw, and until when they may outrun
    /// [`MAX_SPEED`], s; sorted by body.
    thrown: Vec<(BodyId, f64)>,
}

impl Site {
    /// Raises every piece in `specs`, intact and standing.
    #[must_use]
    pub fn new(specs: Vec<PieceSpec>, seed: u64) -> Self {
        let mut site = Self {
            specs,
            world: World::new(STEP),
            pieces: Vec::new(),
            puffs: Vec::new(),
            seed,
            rng: seed,
            pending: 0.0,
            revision: 0,
            thrown: Vec::new(),
        };
        site.raise();
        site
    }

    fn raise(&mut self) {
        let mut world = World::new(STEP);
        let ground = world.add(
            Body::new(1.0, DVec3::ONE, DVec3::new(0.0, -0.5, 0.0)).with_kind(BodyKind::Static),
        );
        world.add_collider(
            Collider::new(
                ground,
                Shape::Cuboid {
                    half: DVec3::new(80.0, 0.5, 80.0),
                },
            )
            .with_material(MATERIAL),
        );
        let mut pieces = Vec::with_capacity(self.specs.len());
        for spec in &self.specs {
            let mut body = Body::new(
                spec.mass,
                Body::box_inertia(spec.mass, spec.size),
                spec.center,
            )
            .with_kind(BodyKind::Static);
            body.orientation = spec.orientation;
            body.prev_orientation = spec.orientation;
            let id = world.add(body);
            // A gable stands under the roof's slopes but does not carry
            // them, so the two never touch.
            let filter = match spec.role {
                Role::Roof => Filter {
                    group: ROOF,
                    mask: !GABLE,
                },
                Role::Gable { .. } => Filter {
                    group: GABLE,
                    mask: !ROOF,
                },
                _ => Filter::ALL,
            };
            for collider in &spec.colliders {
                world.add_collider(
                    Collider::new(
                        id,
                        Shape::Cuboid {
                            half: collider.half,
                        },
                    )
                    .at(collider.center, collider.rotation)
                    .with_material(MATERIAL)
                    .with_filter(filter),
                );
            }
            pieces.push(Piece {
                body: id,
                hit_points: spec.hit_points,
                status: Status::Standing,
                chunks: Vec::new(),
            });
        }
        self.world = world;
        self.pieces = pieces;
        self.puffs.clear();
        self.pending = 0.0;
        self.thrown.clear();
    }

    /// Rebuilds every piece as it was first raised.
    pub fn reset(&mut self) {
        self.rng = self.seed;
        self.raise();
        self.revision += 1;
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
        for collider in self.world.colliders() {
            let Some(index) = self
                .pieces
                .iter()
                .position(|p| p.body == collider.body && p.status != Status::Broken)
            else {
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
            self.world.wake(body);
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
        let body = self.world[id];
        wake_near(&mut self.world, id);
        self.world.remove_body(id);
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
                Push::From {
                    center,
                    speed,
                    radius,
                } => {
                    let k = (1.0 - pos.distance(center) / radius).clamp(0.0, 1.0);
                    // At least a third of the speed, so a chunk at the
                    // edge of the blast still leaves its piece.
                    let speed = speed * (0.35 + 0.65 * k) * (0.8 + 0.4 * self.unit().abs());
                    (blast_direction(center, pos) * speed + spread * 2.0, k)
                }
            };
            chunk.vel = body.vel + omega.cross(pos - body.pos) + kick + spread;
            chunk.omega = DVec3::new(self.unit(), self.unit(), self.unit())
                * if k > 0.0 { 2.0 + 8.0 * k } else { 2.0 };
            let chunk_id = self.world.add(chunk);
            if matches!(push, Push::From { .. }) {
                self.throw(chunk_id);
            }
            // Slightly smaller, so neighbouring chunks start apart.
            self.world.add_collider(
                Collider::new(
                    chunk_id,
                    Shape::Cuboid {
                        half: (cuboid.half * 0.94).max(DVec3::splat(0.03)),
                    },
                )
                .with_material(MATERIAL),
            );
            // The fragments nearest an explosion's heart end first, so
            // the debris cap takes them, hidden in the fireball, before
            // the ones that fly.
            chunks.push(Chunk {
                body: chunk_id,
                until: time + DEBRIS_LIFETIME * (1.0 - 0.6 * k) + self.unit().abs() * 4.0,
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
        self.cap_chunks();
    }

    /// Removes the oldest chunks past [`MAX_CHUNKS`].
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
        if alive.len() <= MAX_CHUNKS {
            return;
        }
        alive.sort_by(|a, b| a.0.total_cmp(&b.0));
        let excess = alive.len() - MAX_CHUNKS;
        for &(_, p, i) in &alive[..excess] {
            let chunk = &mut self.pieces[p].chunks[i];
            chunk.gone = true;
            let body = chunk.body;
            self.world.remove_body(body);
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
            self.world.wake(body);
            let b = &mut self.world[body];
            let scale = (b.mass / 60.0).min(1.0);
            b.apply_impulse_at((push + DVec3::Y * 0.3) * impulse * scale, at);
        }
    }

    /// Whether `point` is within `reach` of a standing or loose piece.
    #[must_use]
    pub fn touches(&self, point: Vec3, reach: f32) -> bool {
        let point = point.as_dvec3();
        self.world.colliders().iter().any(|collider| {
            self.pieces
                .iter()
                .any(|p| p.body == collider.body && p.status != Status::Broken)
                && collider.closest_point(&self.world, point).distance(point) <= f64::from(reach)
        })
    }

    /// An explosion at `center`: every piece within `radius` takes
    /// `damage` scaled from all of it at the center to none at the edge,
    /// what breaks flies outward at up to `speed`, and loose pieces and
    /// chunks in reach are thrown the same way. Returns a blow for each
    /// piece it reached, nearest first.
    pub fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow> {
        let c = center.as_dvec3();
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
            self.world.wake(id);
            let kick = blast_direction(c, at) * speed * k * 0.7;
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
    /// line or a standing corner post, a post a standing wall beside it,
    /// the roof two standing sections under each eave, a gable the roof and
    /// a standing section of its end wall, and the chimney the roof.
    pub fn support(&mut self) {
        loop {
            let falling: Vec<usize> = (0..self.pieces.len())
                .filter(|&i| self.standing(i) && !self.held(i))
                .collect();
            if falling.is_empty() {
                return;
            }
            for piece in falling {
                if self.standing(piece) {
                    self.loosen(piece);
                }
                if self.specs[piece].role == Role::Roof {
                    self.buckle(piece);
                }
            }
        }
    }

    /// When a roof comes loose, what is left of a weak eave line can't
    /// carry it alone: its sections and corner posts crumble outward and
    /// the roof crashes down after them.
    fn buckle(&mut self, roof: usize) {
        let building = self.specs[roof].building;
        for line in [Side::West, Side::East] {
            // The line, its corner posts, and the end walls' sections at
            // its corners.
            let on_line = |spec: &PieceSpec| {
                spec.building == building
                    && match spec.role {
                        Role::Wall { side, .. } if side == line => true,
                        Role::Wall {
                            side: Side::South | Side::North,
                            index,
                            count,
                        } => {
                            (line == Side::West && index == 0)
                                || (line == Side::East && index + 1 == count)
                        }
                        Role::Post { b, .. } => b == line,
                        _ => false,
                    }
            };
            let sections = (0..self.pieces.len())
                .filter(|&i| self.standing(i))
                .filter(|&i| matches!(self.specs[i].role, Role::Wall { side, .. } if side == line))
                .filter(|&i| self.specs[i].building == building)
                .count();
            if sections >= 2 {
                continue;
            }
            let weak: Vec<usize> = (0..self.pieces.len())
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
        let others = || {
            self.specs
                .iter()
                .enumerate()
                .filter(move |(i, s)| *i != piece && s.building == building)
                .filter(|(i, _)| self.standing(*i))
                .map(|(_, s)| s.role)
        };
        let sections = |line: Side| {
            others()
                .filter(|r| matches!(r, Role::Wall { side, .. } if *side == line))
                .count()
        };
        match spec.role {
            Role::Wall { side, index, count } => others().any(|role| match role {
                Role::Wall {
                    side: s, index: i, ..
                } => s == side && (i + 1 == index || index + 1 == i),
                Role::Post { a, b } => {
                    let ends = |end: Side| {
                        (index == 0 && matches!(end, Side::West | Side::South))
                            || (index + 1 == count && matches!(end, Side::East | Side::North))
                    };
                    (a == side && ends(b)) || (b == side && ends(a))
                }
                _ => false,
            }),
            Role::Post { a, b } => others().any(|role| match role {
                Role::Wall { side, index, count } => {
                    let first = index == 0;
                    let last = index + 1 == count;
                    let at = |line: Side, end: Side| {
                        side == line
                            && ((first && matches!(end, Side::West | Side::South))
                                || (last && matches!(end, Side::East | Side::North)))
                    };
                    at(a, b) || at(b, a)
                }
                _ => false,
            }),
            Role::Roof => sections(Side::West) >= 2 && sections(Side::East) >= 2,
            Role::Gable { side } => others().any(|r| r == Role::Roof) && sections(side) >= 1,
            Role::Chimney => others().any(|r| r == Role::Roof),
        }
    }

    /// Makes a standing piece dynamic. Walls, posts, and gables tip outward
    /// from their building so they topple instead of balancing.
    fn loosen(&mut self, piece: usize) {
        let spec = &self.specs[piece];
        let id = self.pieces[piece].body;
        self.pieces[piece].status = Status::Loose;
        self.revision += 1;
        let outward = match spec.role {
            Role::Wall { side, .. } | Role::Gable { side } => Some(side),
            Role::Post { a, .. } => Some(a),
            Role::Roof | Role::Chimney => None,
        }
        .map(|side| match side {
            Side::South => -DVec3::Z,
            Side::North => DVec3::Z,
            Side::West => -DVec3::X,
            Side::East => DVec3::X,
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

    /// Advances the yard by `dt` seconds of wall time.
    pub fn tick(&mut self, dt: f32) {
        self.age_dust(dt);
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
        self.world.step(&gravity);
        let now = self.world.time();
        self.thrown.retain(|&(_, until)| until > now);
        for (index, body) in self.world.bodies_mut().iter_mut().enumerate() {
            if body.kind == BodyKind::Dynamic {
                let thrown = self
                    .thrown
                    .binary_search_by_key(&(index as u32), |(id, _)| id.0)
                    .is_ok();
                let (speed, spin) = if thrown {
                    (BLAST_SPEED, MAX_SPIN * 2.0)
                } else {
                    (MAX_SPEED, MAX_SPIN)
                };
                body.vel = body.vel.clamp_length_max(speed);
                body.omega = body.omega.clamp_length_max(spin);
            }
        }
        // Impacts: the summed contact impulse between each pair of bodies.
        let mut pairs: Vec<(BodyId, BodyId, f64, DVec3)> = Vec::new();
        for report in &self.world.contacts {
            let (a, b) = if report.body_a <= report.body_b {
                (report.body_a, report.body_b)
            } else {
                (report.body_b, report.body_a)
            };
            match pairs.iter_mut().find(|p| p.0 == a && p.1 == b) {
                Some(pair) => pair.2 += report.impulse.length(),
                None => pairs.push((a, b, report.impulse.length(), report.point)),
            }
        }
        let mut hurt: Vec<(usize, i32, DVec3)> = Vec::new();
        for (a, b, impulse, point) in pairs {
            if impulse <= IMPACT {
                continue;
            }
            let damage = ((impulse - IMPACT) / IMPACT_PER_POINT) as i32;
            for body in [a, b] {
                if let Some(piece) = self.pieces.iter().position(|p| p.body == body) {
                    hurt.push((piece, damage, point));
                }
            }
        }
        for (piece, damage, point) in hurt {
            if self.pieces[piece].status != Status::Broken {
                self.dust(point.as_vec3(), 2, 0.8, self.specs[piece].matter);
                self.damage(piece, damage, point, DVec3::ZERO);
            }
        }
        // A loose piece that has fallen past its tilt limit breaks.
        for piece in 0..self.pieces.len() {
            if self.pieces[piece].status != Status::Loose {
                continue;
            }
            let spec = &self.specs[piece];
            let body = &self.world[self.pieces[piece].body];
            let up = body.orientation * DVec3::Y;
            let built = spec.orientation * DVec3::Y;
            let limit = if spec.role == Role::Roof { 50.0 } else { 62.0 };
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
        let time = self.world.time();
        for piece in &mut self.pieces {
            for chunk in &mut piece.chunks {
                if !chunk.gone && time >= chunk.until {
                    chunk.gone = true;
                    self.world.remove_body(chunk.body);
                }
            }
        }
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
            for collider in &spec.colliders {
                let frame = pose * collider.frame();
                let half = collider.half.as_vec3();
                // A lintel over a doorway does not block feet.
                let bottom = frame.transform_point3(Vec3::new(0.0, -half.y, 0.0)).y;
                if bottom > 1.0 {
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

    /// The world pose of a standing or loose piece's body.
    #[must_use]
    pub fn piece_pose(&self, piece: usize) -> Mat4 {
        self.body_pose(self.pieces[piece].body)
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
fn blast_direction(center: DVec3, at: DVec3) -> DVec3 {
    let up = BLAST_UPWARD.to_radians();
    match DVec3::new(at.x - center.x, 0.0, at.z - center.z).try_normalize() {
        Some(out) => out * up.cos() + DVec3::Y * up.sin(),
        None => DVec3::Y,
    }
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
