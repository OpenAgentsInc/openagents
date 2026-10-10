//! The scene, in the Grid's look (`docs/verse/grid-robot.md`): a black
//! floor under gray grid lines, near-black faces in four shades, gray and
//! white edge lines, and white glow.
//!
//! **You** are a Grid robot on a white ring. The **sealed provider** is a
//! sealed box with a robot inside: dim until its evidence checks out, lit
//! white when it does, red and cracked when it is refused. The message flies
//! straight from You to the box. The **OpenAgents relay** is a low conduit
//! on the floor, below and in front of that path: it carries the sealed
//! bytes and has no key.
//!
//! Plaintext is an open scroll and only ever appears at You and inside the
//! box. On the way it is a closed, padlocked shard: black with white edges
//! going out (sealed in your browser to the machine's key), white with a
//! dark lock coming back (sealed to your browser's key).
//!
//! Everything here is pure: [`statics`] builds the meshes once, and
//! [`frame`] turns the timeline and the clock into a list of draws, so the
//! animation is tested without a browser.

use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{PI, TAU};

use crate::mesh::{Lines, Mesh, STRIDE, Shape, rgb};
use crate::steps::{Anim, State, Step};

pub const YOU_X: f32 = -6.0;
pub const VAULT_X: f32 = 6.0;

/// How much larger than its rig each robot stands.
pub const YOU_SCALE: f32 = 1.6;
pub const INSIDE_SCALE: f32 = 1.4;
/// Which way the robots face: You toward the box, the robot inside toward
/// You, both turned a little toward the viewer.
pub const YOU_YAW: f32 = 1.0;
pub const INSIDE_YAW: f32 = -1.0;

/// The sealed box and the plinth it stands on.
pub const PLINTH_TOP: f32 = 0.3;
pub const VAULT_MIN: Vec3 = Vec3::new(VAULT_X - 1.4, PLINTH_TOP, -1.4);
pub const VAULT_MAX: Vec3 = Vec3::new(VAULT_X + 1.4, PLINTH_TOP + 4.0, 1.4);
/// Where the message waits at You, where it enters the box, and where it
/// opens inside.
pub const AT_YOU: Vec3 = Vec3::new(YOU_X + 1.5, 3.2, 0.3);
pub const VAULT_DOOR: Vec3 = Vec3::new(VAULT_X - 1.6, 2.7, 0.3);
pub const VAULT_HEART: Vec3 = Vec3::new(VAULT_X - 0.55, 2.9, 0.85);
/// The box's evidence tablet, floating beside it.
pub const TABLET: Vec3 = Vec3::new(VAULT_X + 2.3, 3.6, 0.6);
/// The two fingerprint tablets, before the plinth.
pub const HASHES: Vec3 = Vec3::new(VAULT_X + 2.3, 1.3, 0.9);
/// The receipt's seal, beside You.
pub const SEAL_AT: Vec3 = Vec3::new(YOU_X - 1.35, 2.1, 0.6);

/// The relay's conduit: a low pipe on the floor in front of the stations,
/// below the message's way, from `PIPE_X0` to `PIPE_X1`, and the small
/// courier housing at its middle.
pub const PIPE_Y: f32 = 0.35;
pub const PIPE_Z: f32 = 3.4;
pub const PIPE_X0: f32 = -3.6;
pub const PIPE_X1: f32 = 3.6;
pub const HOUSING: Vec3 = Vec3::new(0.0, 0.75, PIPE_Z);

/// How high the message arcs on its way between You and the box.
pub const ARC: f32 = 1.5;

/// Colours, linear.
#[must_use]
pub fn white() -> [f32; 3] {
    [1.0; 3]
}
#[must_use]
pub fn gray() -> [f32; 3] {
    [0.42; 3]
}
#[must_use]
pub fn red() -> [f32; 3] {
    [1.0, 0.09, 0.05]
}
/// The near-black field: the clear colour and every solid face.
#[must_use]
pub fn field() -> [f32; 3] {
    rgb(0x0B0B0C)
}
fn face() -> [f32; 3] {
    rgb(0x1A1A1B)
}
fn edge() -> [f32; 3] {
    [0.5; 3]
}

/// Shades every face of `mesh` by its direction, in the Grid's four steps.
pub fn grid_shade(mesh: &mut Mesh) {
    let light = Vec3::new(0.35, 0.8, 0.5).normalize();
    for v in mesh.data.chunks_mut(STRIDE) {
        let n = Vec3::new(v[3], v[4], v[5]);
        let lit = (n.dot(light) * 0.5 + 0.5).clamp(0.0, 0.999);
        let k = [0.55, 0.75, 1.0, 1.3][(lit * 4.0) as usize];
        for c in &mut v[6..9] {
            *c *= k;
        }
    }
}

/// The meshes that are drawn as they are, every frame.
pub struct Statics {
    pub solid: Mesh,
    pub lines: Lines,
}

/// The meshes drawn at a place, with a tint, each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Robot,
    RobotEdges,
    Glass,
    VaultEdges,
    Crack,
    Tablet,
    TabletEdges,
    Bar,
    Shard,
    ShardEdges,
    Scroll,
    Lock,
    Ring,
    Seal,
}

impl Part {
    pub const ALL: [Self; 14] = [
        Self::Robot,
        Self::RobotEdges,
        Self::Glass,
        Self::VaultEdges,
        Self::Crack,
        Self::Tablet,
        Self::TabletEdges,
        Self::Bar,
        Self::Shard,
        Self::ShardEdges,
        Self::Scroll,
        Self::Lock,
        Self::Ring,
        Self::Seal,
    ];

    /// True for parts drawn as lines.
    #[must_use]
    pub fn is_lines(self) -> bool {
        matches!(
            self,
            Self::RobotEdges
                | Self::VaultEdges
                | Self::Crack
                | Self::TabletEdges
                | Self::ShardEdges
        )
    }
}

const TABLET_MIN: Vec3 = Vec3::new(-0.45, -0.6, -0.04);
const TABLET_MAX: Vec3 = Vec3::new(0.45, 0.6, 0.04);

fn shard_corners() -> ([Vec3; 4], Vec3, Vec3) {
    let ring = [0, 1, 2, 3].map(|i| {
        let a = i as f32 / 4.0 * TAU;
        Vec3::new(0.3 * a.cos(), 0.08, 0.3 * a.sin())
    });
    (ring, Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.0, -0.5, 0.0))
}

/// A part's mesh, in white (tinted when drawn) unless it has its own
/// colours.
#[must_use]
pub fn part_mesh(part: Part) -> Mesh {
    let mut mesh = Mesh::new();
    let white = [1.0; 3];
    match part {
        Part::Robot => {
            mesh = crate::robot::parse(crate::robot::BAKED)
                .map(|r| r.0)
                .unwrap_or_default();
        }
        Part::Glass => mesh.block(VAULT_MIN, VAULT_MAX, white),
        Part::Tablet => {
            mesh.block(TABLET_MIN, TABLET_MAX, white);
            grid_shade(&mut mesh);
        }
        Part::Bar => mesh.block(Vec3::splat(-0.5), Vec3::splat(0.5), white),
        Part::Shard => {
            mesh.add(Shape::Shard, Mat4::IDENTITY, white);
            grid_shade(&mut mesh);
        }
        Part::Scroll => {
            // An open scroll: a white sheet with lines of ink between two
            // gray rollers, facing +z.
            mesh.block(
                Vec3::new(-0.4, -0.48, -0.015),
                Vec3::new(0.4, 0.48, 0.015),
                [0.92; 3],
            );
            for (i, w) in [0.6_f32, 0.5, 0.62, 0.38, 0.55, 0.3].iter().enumerate() {
                let y = 0.32 - i as f32 * 0.13;
                mesh.block(
                    Vec3::new(-0.3, y - 0.022, 0.015),
                    Vec3::new(-0.3 + w, y + 0.022, 0.03),
                    [0.02; 3],
                );
            }
            for y in [-0.52_f32, 0.52] {
                mesh.add(
                    Shape::Prism {
                        sides: 8,
                        bottom: 0.07,
                        top: 0.07,
                        turn: 0.0,
                    },
                    Mat4::from_translation(Vec3::new(-0.5, y, 0.0))
                        * Mat4::from_rotation_z(-PI / 2.0),
                    [0.35; 3],
                );
            }
        }
        Part::Lock => {
            // A padlock: a body and a shackle, about 1 tall.
            mesh.block(
                Vec3::new(-0.36, -0.5, -0.12),
                Vec3::new(0.36, 0.05, 0.12),
                white,
            );
            mesh.add(
                Shape::Torus {
                    segments: 20,
                    sides: 6,
                    radius: 0.24,
                    tube: 0.07,
                },
                Mat4::from_translation(Vec3::new(0.0, 0.1, 0.0)),
                white,
            );
        }
        Part::Ring => mesh.add(
            Shape::Torus {
                segments: 40,
                sides: 6,
                radius: 0.5,
                tube: 0.02,
            },
            Mat4::IDENTITY,
            white,
        ),
        Part::Seal => {
            mesh.add(
                Shape::Prism {
                    sides: 16,
                    bottom: 0.5,
                    top: 0.5,
                    turn: 0.0,
                },
                Mat4::IDENTITY,
                white,
            );
            grid_shade(&mut mesh);
        }
        _ => {}
    }
    mesh
}

/// A part's lines, in white.
#[must_use]
pub fn part_lines(part: Part) -> Lines {
    let mut lines = Lines::new();
    let white = [1.0; 3];
    match part {
        Part::RobotEdges => {
            lines = crate::robot::parse(crate::robot::BAKED)
                .map(|r| r.1)
                .unwrap_or_default();
        }
        Part::VaultEdges => {
            lines.box_edges(VAULT_MIN, VAULT_MAX, white);
            // Seams round the lid and the base: the box is shut.
            for y in [VAULT_MIN.y + 0.15, VAULT_MAX.y - 0.15] {
                lines.box_edges(
                    Vec3::new(VAULT_MIN.x, y, VAULT_MIN.z),
                    Vec3::new(VAULT_MAX.x, y, VAULT_MAX.z),
                    white,
                );
            }
        }
        Part::TabletEdges => lines.box_edges(TABLET_MIN, TABLET_MAX, white),
        Part::ShardEdges => {
            let (ring, top, bottom) = shard_corners();
            for i in 0..4 {
                lines.line(ring[i], ring[(i + 1) % 4], white);
                lines.line(ring[i], top, white);
                lines.line(ring[i], bottom, white);
            }
        }
        Part::Crack => {
            // A jagged crack across the box's front.
            let z = VAULT_MAX.z + 0.02;
            let pts = [
                (-1.35, 3.9),
                (-0.7, 3.4),
                (-0.9, 2.9),
                (0.0, 2.3),
                (-0.2, 1.7),
                (0.6, 1.1),
                (0.4, 0.7),
                (1.35, 0.45),
            ];
            for w in pts.windows(2) {
                lines.line(
                    Vec3::new(VAULT_X + w[0].0, w[0].1, z),
                    Vec3::new(VAULT_X + w[1].0, w[1].1, z),
                    white,
                );
            }
            for (from, to) in [
                ((-0.7, 3.4), (0.3, 3.7)),
                ((0.0, 2.3), (0.9, 2.6)),
                ((0.6, 1.1), (-0.5, 0.8)),
            ] {
                lines.line(
                    Vec3::new(VAULT_X + from.0, from.1, z),
                    Vec3::new(VAULT_X + to.0, to.1, z),
                    white,
                );
            }
        }
        _ => {}
    }
    lines
}

/// A near-black block with gray edges.
fn block(solid: &mut Mesh, lines: &mut Lines, min: Vec3, max: Vec3) {
    solid.block(min, max, face());
    lines.box_edges(min, max, edge());
}

/// The floor, its grid, You's ring, the box's plinth and the relay.
#[must_use]
pub fn statics() -> Statics {
    let mut solid = Mesh::new();
    let mut lines = Lines::new();
    solid.block(
        Vec3::new(-60.0, -0.05, -60.0),
        Vec3::new(60.0, 0.0, 60.0),
        field(),
    );
    // The Grid's lines, every 2 m.
    let grid = [0.16; 3];
    for i in -20..=20 {
        let at = i as f32 * 2.0;
        lines.line(
            Vec3::new(at, 0.005, -40.0),
            Vec3::new(at, 0.005, 40.0),
            grid,
        );
        lines.line(
            Vec3::new(-40.0, 0.005, at),
            Vec3::new(40.0, 0.005, at),
            grid,
        );
    }
    // You stand on a white ring, like the Grid's spawn.
    lines.circle(Vec3::new(YOU_X, 0.01, 0.0), 1.15, 64, [0.75; 3]);

    // The box's plinth.
    block(
        &mut solid,
        &mut lines,
        Vec3::new(VAULT_X - 1.75, 0.0, -1.75),
        Vec3::new(VAULT_X + 1.75, PLINTH_TOP, 1.75),
    );

    // The relay: a low pipe on short posts behind the stations, and a small
    // courier housing at its middle with a lamp slit. It never stands
    // between You and the box.
    block(
        &mut solid,
        &mut lines,
        Vec3::new(PIPE_X0, PIPE_Y - 0.14, PIPE_Z - 0.14),
        Vec3::new(PIPE_X1, PIPE_Y + 0.14, PIPE_Z + 0.14),
    );
    let mut x = PIPE_X0 + 0.5;
    while x < PIPE_X1 {
        block(
            &mut solid,
            &mut lines,
            Vec3::new(x - 0.07, 0.0, PIPE_Z - 0.07),
            Vec3::new(x + 0.07, PIPE_Y - 0.14, PIPE_Z + 0.07),
        );
        x += 1.2;
    }
    for end in [PIPE_X0, PIPE_X1] {
        block(
            &mut solid,
            &mut lines,
            Vec3::new(end - 0.25, 0.0, PIPE_Z - 0.25),
            Vec3::new(end + 0.25, PIPE_Y + 0.3, PIPE_Z + 0.25),
        );
    }
    block(
        &mut solid,
        &mut lines,
        Vec3::new(HOUSING.x - 0.55, 0.0, PIPE_Z - 0.45),
        Vec3::new(HOUSING.x + 0.55, HOUSING.y, PIPE_Z + 0.45),
    );
    lines.line(
        Vec3::new(HOUSING.x - 0.35, HOUSING.y - 0.3, PIPE_Z + 0.46),
        Vec3::new(HOUSING.x + 0.35, HOUSING.y - 0.3, PIPE_Z + 0.46),
        [0.9; 3],
    );
    grid_shade(&mut solid);
    Statics { solid, lines }
}

/// One part drawn at a place.
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub part: Part,
    pub model: Mat4,
    /// Multiplies the part's colour.
    pub tint: [f32; 3],
    /// Light the part gives off, added.
    pub glow: [f32; 3],
    pub alpha: f32,
}

/// A soft point of light (a camera-facing sprite).
#[derive(Clone, Debug, PartialEq)]
pub struct Spark {
    pub at: Vec3,
    pub size: f32,
    pub colour: [f32; 3],
}

/// A ribbon of light between two points.
#[derive(Clone, Debug, PartialEq)]
pub struct Beam {
    pub from: Vec3,
    pub to: Vec3,
    pub width: f32,
    pub colour: [f32; 3],
}

/// A floating DOM tag that follows the travelling shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    SealedInYourBrowser,
    SealedToYourBrowser,
}

/// Everything one frame draws besides the statics.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub draws: Vec<Draw>,
    pub sparks: Vec<Spark>,
    pub beams: Vec<Beam>,
    /// A tag that follows the travelling shard, and where.
    pub tag: Option<(Vec3, Tag)>,
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| v * k)
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A point on the message's way from `from` to `to`, `u` from 0 to 1,
/// arcing up by [`ARC`] in the middle.
#[must_use]
pub fn along(from: Vec3, to: Vec3, u: f32) -> Vec3 {
    let u = u.clamp(0.0, 1.0);
    from.lerp(to, u) + Vec3::Y * (u * PI).sin() * ARC
}

/// The point on the relay's conduit under a place on the way: the carrier
/// that moves the sealed bytes along.
#[must_use]
pub fn carrier(at: Vec3) -> Vec3 {
    Vec3::new(at.x.clamp(PIPE_X0, PIPE_X1), PIPE_Y + 0.2, PIPE_Z)
}

/// Time in seconds since a step's animation started and since it settled.
struct Phase<'a> {
    t: f32,
    settled: Option<f32>,
    state: &'a State,
}

fn phase(anim: &Anim, now: f64) -> Option<Phase<'_>> {
    let started = anim.started?;
    if anim.state == State::Skipped {
        return None;
    }
    Some(Phase {
        t: (now - started).max(0.0) as f32,
        settled: anim.settled.map(|s| (now - s).max(0.0) as f32),
        state: &anim.state,
    })
}

/// A check's light: white when it passed, red when refused, gray while
/// working.
fn verdict_colour(p: &Phase<'_>) -> [f32; 3] {
    match (p.settled, p.state) {
        (Some(_), State::Ok) => white(),
        (Some(_), State::Refused(_)) => red(),
        _ => [0.6; 3],
    }
}

/// Whose key a shard is sealed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seal {
    /// Sealed in your browser to the sealed machine's key.
    ToMachine,
    /// The answer, sealed to your browser's key.
    ToYou,
    /// Stopped: the round was refused.
    Stopped,
}

/// A closed, opaque shard with a padlock on its face.
fn shard(frame: &mut Frame, at: Vec3, spin: f32, size: f32, seal: Seal) {
    let (body, lines, lock, lock_glow) = match seal {
        Seal::ToMachine => ([0.12; 3], white(), [0.2; 3], white()),
        Seal::ToYou => ([0.72; 3], [0.05; 3], [0.02; 3], [0.0; 3]),
        Seal::Stopped => ([0.12; 3], red(), [0.2; 3], red()),
    };
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(1.15, 1.0, 1.15) * size,
        Quat::from_rotation_y(spin) * Quat::from_rotation_z(0.2),
        at,
    );
    frame.draws.push(Draw {
        part: Part::Shard,
        model,
        tint: body,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::ShardEdges,
        model,
        tint: lines,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::Lock,
        model: Mat4::from_scale_rotation_translation(
            Vec3::splat(0.4 * size),
            Quat::IDENTITY,
            at + Vec3::new(0.0, 0.02, 0.42 * size),
        ),
        tint: lock,
        glow: scale(lock_glow, 0.85),
        alpha: 1.0,
    });
    frame.sparks.push(Spark {
        at,
        size: size * 1.3,
        colour: scale(
            if seal == Seal::Stopped {
                red()
            } else {
                white()
            },
            0.22,
        ),
    });
}

/// An open scroll: the message in plain words. Only ever drawn at You or
/// inside the box.
fn scroll(frame: &mut Frame, at: Vec3, tilt: f32, size: f32, bright: f32) {
    frame.draws.push(Draw {
        part: Part::Scroll,
        model: Mat4::from_scale_rotation_translation(
            Vec3::splat(size),
            Quat::from_rotation_y(tilt),
            at,
        ),
        tint: [1.0; 3],
        glow: scale(white(), 0.08 * bright),
        alpha: 1.0,
    });
    frame.sparks.push(Spark {
        at,
        size: size * 2.0,
        colour: scale(white(), 0.3 * bright),
    });
}

/// A near-black tablet with edges, at `at`, turned by `turn`, `size` big.
fn tablet(frame: &mut Frame, at: Vec3, turn: Quat, size: Vec3, edges: [f32; 3]) {
    let model = Mat4::from_scale_rotation_translation(size, turn, at);
    frame.draws.push(Draw {
        part: Part::Tablet,
        model,
        tint: face(),
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::TabletEdges,
        model,
        tint: edges,
        glow: [0.0; 3],
        alpha: 1.0,
    });
}

/// Builds one frame: `now` in seconds on the page clock, `lit` as
/// `Show::provider_lit` set it, `motion` false to keep everything still but
/// the essentials.
#[must_use]
pub fn frame(anims: &[Anim; 9], lit: Option<bool>, now: f64, motion: bool) -> Frame {
    let mut frame = Frame::default();
    let time = now as f32;
    let wave = |speed: f32| if motion { (time * speed).sin() } else { 0.0 };
    let get = |step: Step| phase(&anims[step.index()], now);

    // You.
    let you = Mat4::from_scale_rotation_translation(
        Vec3::splat(YOU_SCALE),
        Quat::from_rotation_y(YOU_YAW),
        Vec3::new(YOU_X, 0.0, 0.0),
    );
    frame.draws.push(Draw {
        part: Part::Robot,
        model: you,
        tint: [1.0; 3],
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::RobotEdges,
        model: you,
        tint: [1.0; 3],
        glow: [0.0; 3],
        alpha: 1.0,
    });

    // The sealed box and the robot inside.
    let pulse = 0.85 + 0.15 * wave(1.6);
    let (inside, edges, glass_glow) = match lit {
        Some(true) => (1.0, scale(white(), pulse), scale(white(), 0.035 * pulse)),
        Some(false) => (0.4, red(), scale(red(), 0.05)),
        None => (0.35, [0.45; 3], [0.0; 3]),
    };
    let robot = Mat4::from_scale_rotation_translation(
        Vec3::splat(INSIDE_SCALE),
        Quat::from_rotation_y(INSIDE_YAW),
        Vec3::new(VAULT_X, PLINTH_TOP, 0.0),
    );
    let inside_tint = if lit == Some(false) {
        [inside, inside * 0.3, inside * 0.25]
    } else {
        [inside; 3]
    };
    frame.draws.push(Draw {
        part: Part::Robot,
        model: robot,
        tint: inside_tint,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::RobotEdges,
        model: robot,
        tint: inside_tint,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::VaultEdges,
        model: Mat4::IDENTITY,
        tint: edges,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    if lit == Some(true) {
        frame.sparks.push(Spark {
            at: Vec3::new(VAULT_X, PLINTH_TOP + 2.0, 0.0),
            size: 4.2,
            colour: scale(white(), 0.12 * pulse),
        });
    }
    if lit == Some(false) {
        frame.draws.push(Draw {
            part: Part::Crack,
            model: Mat4::IDENTITY,
            tint: red(),
            glow: [0.0; 3],
            alpha: 1.0,
        });
        frame.sparks.push(Spark {
            at: Vec3::new(VAULT_X, 2.4, VAULT_MAX.z),
            size: 3.4,
            colour: scale(red(), 0.18 * pulse),
        });
    }

    // Fetch: the relay hands You the box's public records, and the
    // evidence tablet appears beside the box.
    if let Some(p) = get(Step::Fetch) {
        let fade = p.settled.map_or(1.0, |s| 1.0 - ease(s / 0.9));
        if fade > 0.0 {
            let from = HOUSING + Vec3::Y * 0.2;
            let to = AT_YOU - Vec3::Y * 0.4;
            frame.beams.push(Beam {
                from,
                to,
                width: 0.06,
                colour: scale(white(), 0.35 * fade),
            });
            for k in 0..3 {
                let u = (p.t * 1.1 + k as f32 / 3.0) % 1.0;
                frame.sparks.push(Spark {
                    at: from.lerp(to, u),
                    size: 0.45,
                    colour: scale(white(), 0.8 * fade),
                });
            }
        }
    }
    let tablet_seen = get(Step::Fetch).map_or(0.0, |p| ease(p.t / 0.6));
    let bob = Vec3::Y * 0.08 * wave(0.9);
    let turn = Quat::from_rotation_y(0.3);
    if tablet_seen > 0.0 {
        let colour = get(Step::Chain).map_or([0.6; 3], |p| verdict_colour(&p));
        tablet(
            &mut frame,
            TABLET + bob,
            turn,
            Vec3::splat(tablet_seen),
            colour,
        );
        for i in 0..5 {
            let y = 0.38 - i as f32 * 0.19;
            let w = if i % 2 == 0 { 0.62 } else { 0.44 };
            frame.draws.push(Draw {
                part: Part::Bar,
                model: Mat4::from_translation(TABLET + bob)
                    * Mat4::from_quat(turn)
                    * Mat4::from_scale_rotation_translation(
                        Vec3::new(w, 0.04, 0.02) * tablet_seen,
                        Quat::IDENTITY,
                        Vec3::new(0.0, y * tablet_seen, 0.05),
                    ),
                tint: [0.0; 3],
                glow: scale(colour, 0.9),
                alpha: 1.0,
            });
        }
    }

    // Chain: a ring of light round the tablet.
    if let Some(p) = get(Step::Chain) {
        let colour = verdict_colour(&p);
        let grow = ease(p.t / 0.45);
        let bright = match p.settled {
            Some(s) => 0.7 + 0.5 * (1.0 - ease(s / 0.6)),
            None => 0.6 + 0.3 * wave(6.0),
        };
        let tilt = turn * Quat::from_rotation_x(-0.2);
        frame.draws.push(Draw {
            part: Part::Ring,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(1.9 * grow),
                tilt,
                TABLET + bob,
            ),
            tint: [0.0; 3],
            glow: scale(colour, bright),
            alpha: 1.0,
        });
        let speed = if p.settled.is_some() { 0.4 } else { 2.4 };
        for k in 0..4 {
            let a = if motion { time * speed } else { 0.0 } + k as f32 / 4.0 * TAU;
            let local = Vec3::new(a.cos(), a.sin(), 0.0) * 0.95 * grow;
            frame.sparks.push(Spark {
                at: TABLET + bob + tilt * local,
                size: 0.35,
                colour: scale(colour, bright),
            });
        }
    }

    // Measure: two fingerprint tablets, one measured in the box, one from
    // the public log, slide into line.
    if let Some(p) = get(Step::Measure) {
        let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
        let colour = verdict_colour(&p);
        let appear = ease(p.t / 0.35);
        let slide = if refused {
            0.45
        } else {
            0.62 * (1.0 - ease((p.t - 0.2) / 0.7))
        };
        let pattern = [0.7, 0.45, 0.62, 0.3, 0.55];
        for (side, offset) in [(-1.0_f32, 0.0_f32), (1.0, slide)] {
            let centre = HASHES
                + Vec3::new(
                    side * 0.45,
                    offset + (1.0 - appear) * 1.5 * side.max(0.0),
                    0.0,
                );
            tablet(
                &mut frame,
                centre,
                Quat::IDENTITY,
                Vec3::new(0.85, 0.8, 1.0) * appear.max(0.01),
                colour,
            );
            for (i, w) in pattern.iter().enumerate() {
                // A changed fingerprint shows different marks.
                let w = if refused && side > 0.0 {
                    pattern[(i + 2) % 5]
                } else {
                    *w
                };
                let y = 0.3 - i as f32 * 0.15;
                frame.draws.push(Draw {
                    part: Part::Bar,
                    model: Mat4::from_scale_rotation_translation(
                        Vec3::new(w * 0.75, 0.04, 0.02) * appear.max(0.01),
                        Quat::IDENTITY,
                        centre + Vec3::new(0.0, y * appear, 0.05),
                    ),
                    tint: [0.0; 3],
                    glow: scale(colour, 0.9),
                    alpha: 1.0,
                });
            }
        }
        if p.settled.is_some() && matches!(p.state, State::Ok) {
            for i in 0..5 {
                let y = 0.3 - i as f32 * 0.15;
                frame.beams.push(Beam {
                    from: HASHES + Vec3::new(-0.45, y, 0.07),
                    to: HASHES + Vec3::new(0.45, y, 0.07),
                    width: 0.03,
                    colour: scale(white(), 0.7),
                });
            }
        }
        if refused {
            frame.sparks.push(Spark {
                at: HASHES + Vec3::Y * 0.2,
                size: 2.2,
                colour: scale(red(), 0.3),
            });
        }
    }

    // Bind: a line of light from the evidence to the machine inside.
    if let Some(p) = get(Step::Bind) {
        let colour = verdict_colour(&p);
        let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
        let target = Vec3::new(VAULT_X - 0.2, PLINTH_TOP + 2.2, 0.2);
        let reach = if refused { 0.55 } else { ease(p.t / 0.55) };
        let end = TABLET.lerp(target, reach);
        frame.beams.push(Beam {
            from: TABLET,
            to: end,
            width: 0.05,
            colour: scale(colour, 0.8),
        });
        frame.sparks.push(Spark {
            at: end,
            size: if refused { 1.3 } else { 0.7 },
            colour: scale(colour, 0.8),
        });
    }

    message(&mut frame, anims, now, motion);

    // Receipt: a seal stamped beside You.
    if let Some(p) = get(Step::Receipt) {
        let colour = verdict_colour(&p);
        let land = ease(p.t / 0.45);
        let at = SEAL_AT + Vec3::Y * (1.0 - land) * 2.0;
        frame.draws.push(Draw {
            part: Part::Seal,
            model: Mat4::from_scale_rotation_translation(
                Vec3::new(0.7, 0.06, 0.7),
                Quat::from_rotation_x(PI / 2.0 * land),
                at,
            ),
            tint: face(),
            glow: [0.0; 3],
            alpha: 1.0,
        });
        frame.draws.push(Draw {
            part: Part::Ring,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(0.72),
                Quat::IDENTITY,
                at + Vec3::Z * 0.05,
            ),
            tint: [0.0; 3],
            glow: colour,
            alpha: 1.0,
        });
        if land >= 1.0 {
            let ring = (p.t - 0.45).max(0.0);
            if ring < 0.8 {
                frame.draws.push(Draw {
                    part: Part::Ring,
                    model: Mat4::from_scale_rotation_translation(
                        Vec3::splat(0.8 + ring * 2.2),
                        Quat::IDENTITY,
                        SEAL_AT + Vec3::Z * 0.06,
                    ),
                    tint: [0.0; 3],
                    glow: scale(colour, 1.0 - ring / 0.8),
                    alpha: 1.0,
                });
            }
        }
    }

    // A ring on the floor round the box while a round is under way.
    if anims.iter().any(|a| a.state == State::Running) {
        frame.draws.push(Draw {
            part: Part::Ring,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(5.0 + 0.2 * wave(2.0)),
                Quat::from_rotation_x(PI / 2.0),
                Vec3::new(VAULT_X, 0.02, 0.0),
            ),
            tint: [0.0; 3],
            glow: [0.35; 3],
            alpha: 1.0,
        });
    }

    // The box's dark glass goes last, over the robot inside.
    frame.draws.push(Draw {
        part: Part::Glass,
        model: Mat4::IDENTITY,
        tint: face(),
        glow: glass_glow,
        alpha: if lit == Some(false) { 0.6 } else { 0.5 },
    });
    frame
}

/// Where the message is and what it looks like.
fn message(frame: &mut Frame, anims: &[Anim; 9], now: f64, motion: bool) {
    let time = now as f32;
    let spin = if motion { time * 1.1 } else { 0.4 };
    let tilt = if motion {
        0.25 * (time * 0.8).sin()
    } else {
        0.0
    };
    let get = |step: Step| phase(&anims[step.index()], now);
    let latest = [Step::Answer, Step::Decrypt, Step::Relay, Step::Encrypt]
        .into_iter()
        .find_map(|s| get(s).map(|p| (s, p)));
    let Some((step, p)) = latest else {
        // Not sealed yet: your message, readable, at You.
        if anims.iter().any(|a| a.started.is_some()) {
            scroll(frame, AT_YOU, tilt, 0.8, 0.8);
        }
        return;
    };
    let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
    let travel = 1.35;
    match step {
        Step::Encrypt => {
            // The scroll shuts and a padlocked shard takes its place.
            let k = ease((p.t - 0.15) / 0.45);
            if k < 1.0 {
                scroll(frame, AT_YOU, tilt, 0.8 * (1.0 - k), 1.0);
            }
            if k > 0.0 {
                let seal = if refused {
                    Seal::Stopped
                } else {
                    Seal::ToMachine
                };
                shard(frame, AT_YOU, spin, 0.8 * k, seal);
            }
        }
        Step::Relay => {
            let mut u = (p.t / travel).min(1.0);
            if refused {
                u = u.min(0.45);
            }
            let at = along(AT_YOU, VAULT_DOOR, u);
            let hover = if u >= 1.0 && p.settled.is_none() && motion {
                Vec3::Y * 0.08 * (time * 4.0).sin()
            } else {
                Vec3::ZERO
            };
            let seal = if refused {
                Seal::Stopped
            } else {
                Seal::ToMachine
            };
            shard(frame, at + hover, spin, 0.8, seal);
            carried(frame, at, u, |b| along(AT_YOU, VAULT_DOOR, b));
            if !refused {
                frame.tag = Some((at + Vec3::Y * 0.85, Tag::SealedInYourBrowser));
            }
        }
        Step::Decrypt => {
            // Inside the box the shard opens and the scroll shows.
            let at = VAULT_DOOR.lerp(VAULT_HEART, ease(p.t / 0.3));
            if refused {
                shard(frame, at, spin, 0.8, Seal::Stopped);
                return;
            }
            let k = ease((p.t - 0.2) / 0.5);
            if k < 1.0 {
                shard(frame, at, spin, 0.8 * (1.0 - k), Seal::ToMachine);
            }
            if k > 0.0 {
                scroll(frame, at, 0.0, 0.65 * k, 1.0);
            }
        }
        Step::Answer => {
            // The answer is sealed to your browser inside the box, flies
            // back, and opens only when it reaches You.
            let seal = if refused { Seal::Stopped } else { Seal::ToYou };
            let close = ease(p.t / 0.35);
            let mut u = ((p.t - 0.35) / travel).clamp(0.0, 1.0);
            if refused {
                u = u.min(0.5);
            }
            let home = u >= 1.0 && p.settled.is_some() && !refused;
            if close < 1.0 {
                scroll(frame, VAULT_HEART, 0.0, 0.65 * (1.0 - close), 1.0);
                shard(frame, VAULT_HEART, spin, 0.8 * close, seal);
            } else if home {
                let s = p.settled.unwrap_or(0.0).min((p.t - 0.35 - travel).max(0.0));
                let k = ease(s / 0.45);
                if k < 1.0 {
                    shard(frame, AT_YOU, spin, 0.8 * (1.0 - k), seal);
                }
                scroll(frame, AT_YOU, tilt, 0.8 * k.max(0.01), 1.0);
            } else {
                let at = along(VAULT_DOOR, AT_YOU, u);
                let hover = if u >= 1.0 && motion {
                    Vec3::Y * 0.08 * (time * 4.0).sin()
                } else {
                    Vec3::ZERO
                };
                shard(frame, at + hover, spin, 0.8, seal);
                carried(frame, at, u, |b| along(VAULT_DOOR, AT_YOU, b));
                if !refused && u > 0.0 {
                    frame.tag = Some((at + Vec3::Y * 0.85, Tag::SealedToYourBrowser));
                }
            }
        }
        _ => {}
    }
}

/// The relay's part in a shard's flight: a pulse in the conduit below it,
/// tied to it by a faint line, and a short wake behind it.
fn carried(frame: &mut Frame, at: Vec3, u: f32, way: impl Fn(f32) -> Vec3) {
    if u > 0.02 && u < 0.98 {
        let below = carrier(at);
        frame.beams.push(Beam {
            from: at,
            to: below,
            width: 0.02,
            colour: [0.12; 3],
        });
        frame.sparks.push(Spark {
            at: below,
            size: 0.8,
            colour: [0.5; 3],
        });
    }
    for k in 1..4 {
        frame.sparks.push(Spark {
            at: way((u - k as f32 * 0.03).max(0.0)),
            size: 0.45 - k as f32 * 0.09,
            colour: [0.2 / k as f32; 3],
        });
    }
}

/// The camera: where it stands and what it sees.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub target: Vec3,
    pub near: f32,
    pub far: f32,
}

/// Half the scene's width that must fit in view, and its height.
const HALF_WIDTH: f32 = 10.2;
const HEIGHT: f32 = 6.5;
/// The height the camera looks at.
pub const TARGET_Y: f32 = 2.2;

/// A camera that fits the stations into a region of the canvas: `region`
/// is that region's width as a share of the canvas, `centre` its centre in
/// clip space (-1 to 1), `sway` the slow drift's angle.
#[must_use]
pub fn camera(aspect: f32, region: f32, centre: f32, sway: f32) -> Camera {
    let fovy = 34.0_f32.to_radians();
    let half = (fovy / 2.0).tan();
    let target = Vec3::new(1.0, TARGET_Y, 0.6);
    let width = (region * aspect).max(0.2);
    let distance = (HALF_WIDTH / (width * half)).max(HEIGHT * 0.5 / half);
    let dir = Quat::from_rotation_y(sway) * Vec3::new(0.0, 0.3, 1.0).normalize();
    let eye = target + dir * distance;
    let near = 0.5;
    let far = distance + 60.0;
    let view_proj = Mat4::from_translation(Vec3::new(centre, 0.0, 0.0))
        * Mat4::perspective_rh_gl(fovy, aspect.max(0.1), near, far)
        * Mat4::look_at_rh(eye, target, Vec3::Y);
    Camera {
        view_proj,
        eye,
        target,
        near,
        far,
    }
}

/// Where each label sits in the scene: You, the relay, the sealed provider.
#[must_use]
pub fn label_anchors() -> [Vec3; 3] {
    [
        Vec3::new(YOU_X, YOU_SCALE * 1.9 + 0.35, 0.0),
        HOUSING + Vec3::Y * 0.25,
        Vec3::new(VAULT_X, VAULT_MAX.y + 0.4, 0.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anims(set: &[(Step, State, f64, Option<f64>)]) -> [Anim; 9] {
        let mut out: [Anim; 9] = Default::default();
        for (step, state, started, settled) in set {
            out[step.index()] = Anim {
                state: state.clone(),
                started: Some(*started),
                settled: *settled,
                ms: None,
            };
        }
        out
    }

    #[test]
    fn the_statics_are_whole() {
        let statics = statics();
        assert!(statics.solid.vertices() > 100);
        assert!(statics.solid.vertices().is_multiple_of(3));
        assert!(statics.lines.vertices().is_multiple_of(2));
        for part in Part::ALL {
            if part.is_lines() {
                assert!(part_lines(part).vertices() > 0, "{part:?}");
            } else {
                assert!(part_mesh(part).vertices() > 0, "{part:?}");
            }
        }
    }

    #[test]
    fn the_box_lights_white_or_cracks_red() {
        let idle: [Anim; 9] = Default::default();
        let edges = |f: &Frame| {
            f.draws
                .iter()
                .find(|d| d.part == Part::VaultEdges)
                .unwrap()
                .tint
        };
        let dark = frame(&idle, None, 1.0, true);
        let lit = frame(&idle, Some(true), 1.0, true);
        let refused = frame(&idle, Some(false), 1.0, true);
        assert!(edges(&lit)[0] > edges(&dark)[0]);
        assert!(edges(&refused)[0] > 0.9 && edges(&refused)[1] < 0.2);
        assert!(refused.draws.iter().any(|d| d.part == Part::Crack));
        assert!(!lit.draws.iter().any(|d| d.part == Part::Crack));
        // Two robots: You, and the one inside. The glass is drawn last.
        assert_eq!(
            lit.draws.iter().filter(|d| d.part == Part::Robot).count(),
            2
        );
        assert_eq!(lit.draws.last().unwrap().part, Part::Glass);
    }

    #[test]
    fn the_message_is_plain_only_at_you_and_in_the_box() {
        let scrolls = |f: &Frame| -> Vec<Vec3> {
            f.draws
                .iter()
                .filter(|d| d.part == Part::Scroll)
                .map(|d| d.model.transform_point3(Vec3::ZERO))
                .collect()
        };
        let shards = |f: &Frame| f.draws.iter().filter(|d| d.part == Part::Shard).count();
        let at_you = |p: &Vec3| (p.x - AT_YOU.x).abs() < 0.5;
        // Before sealing: plain at You.
        let f = frame(
            &anims(&[(Step::Fetch, State::Ok, 0.0, Some(1.0))]),
            None,
            2.0,
            true,
        );
        assert!(!scrolls(&f).is_empty() && scrolls(&f).iter().all(at_you));
        // On the way: only a padlocked shard, tagged sealed in your browser.
        let f = frame(
            &anims(&[
                (Step::Encrypt, State::Ok, 0.0, Some(0.7)),
                (Step::Relay, State::Running, 1.0, None),
            ]),
            Some(true),
            1.6,
            true,
        );
        assert!(scrolls(&f).is_empty());
        assert_eq!(shards(&f), 1);
        assert!(f.draws.iter().any(|d| d.part == Part::Lock));
        assert_eq!(f.tag.map(|t| t.1), Some(Tag::SealedInYourBrowser));
        // Opened inside the box.
        let f = frame(
            &anims(&[
                (Step::Relay, State::Ok, 0.0, Some(1.4)),
                (Step::Decrypt, State::Ok, 1.5, Some(2.2)),
            ]),
            Some(true),
            3.0,
            true,
        );
        let inside = scrolls(&f);
        assert!(!inside.is_empty());
        assert!(
            inside
                .iter()
                .all(|p| p.x > VAULT_MIN.x && p.x < VAULT_MAX.x)
        );
        // On the way back: sealed to your browser.
        let f = frame(
            &anims(&[
                (Step::Decrypt, State::Ok, 0.0, Some(0.6)),
                (Step::Answer, State::Running, 1.0, None),
            ]),
            Some(true),
            2.0,
            true,
        );
        assert!(scrolls(&f).is_empty());
        assert_eq!(f.tag.map(|t| t.1), Some(Tag::SealedToYourBrowser));
        // And open back at You once the answer arrives.
        let f = frame(
            &anims(&[
                (Step::Decrypt, State::Ok, 0.0, Some(0.6)),
                (Step::Answer, State::Ok, 1.0, Some(3.0)),
            ]),
            Some(true),
            4.5,
            true,
        );
        assert!(scrolls(&f).iter().all(at_you));
        assert_eq!(shards(&f), 0);
        assert_eq!(f.tag, None);
    }

    #[test]
    fn the_message_flies_straight_with_the_relay_off_its_way() {
        assert!((along(AT_YOU, VAULT_DOOR, 0.0) - AT_YOU).length() < 1e-4);
        assert!((along(AT_YOU, VAULT_DOOR, 1.0) - VAULT_DOOR).length() < 1e-4);
        for i in 0..=20 {
            let p = along(AT_YOU, VAULT_DOOR, i as f32 / 20.0);
            // The way stays on the line between You and the box, well above
            // the relay's conduit, which runs on the floor in front.
            assert!((p.z - AT_YOU.z).abs() < 1e-4);
            assert!(p.y > HOUSING.y + 1.5);
            assert!((carrier(p).z - PIPE_Z).abs() < 1e-4);
        }
        assert!(PIPE_Z.abs() > 3.0);
    }

    #[test]
    fn the_camera_fits_the_stations() {
        for (aspect, region, centre) in [(1.6, 1.0, 0.0), (1.0, 0.9, 0.0), (0.6, 1.0, 0.0)] {
            let camera = camera(aspect, region, centre, 0.0);
            for x in [YOU_X - 1.6, VAULT_X + 3.3] {
                let clip = camera.view_proj * Vec3::new(x, 1.0, 0.0).extend(1.0);
                let ndc = clip.x / clip.w;
                assert!(
                    (ndc - centre).abs() <= region + 1e-3,
                    "{aspect} {region}: {ndc}"
                );
            }
        }
    }
}
