//! The scene: three stations on a dark stone floor, in the Greco-futurist
//! look of the relevance visualizer (`docs/verse/relevance-visualizer.md`).
//!
//! Left to right: **You**, a marble plinth with a glowing orb; the **relay
//! and gateway**, a colonnaded gate that only ever carries sealed shards;
//! and the **sealed provider**, a bronze obelisk in a glass vault on a
//! stepped podium, dark until its evidence checks out.
//!
//! Plaintext is a bright open glyph and only ever appears at You and inside
//! the vault; ciphertext is a dark faceted shard with amber edges.
//!
//! Everything here is pure: [`statics`] builds the meshes once, and
//! [`frame`] turns the timeline and the clock into a list of draws, so the
//! animation is tested without a browser.

use glam::{Mat4, Quat, Vec3};
use std::f32::consts::{FRAC_PI_4, PI, TAU};

use crate::mesh::{Lines, Mesh, Shape, rgb};
use crate::steps::{Anim, State, Step};

pub const YOU_X: f32 = -7.5;
pub const GATE_X: f32 = 0.0;
pub const VAULT_X: f32 = 7.5;

/// The top of You's plinth, where the orb floats.
pub const ORB: Vec3 = Vec3::new(YOU_X, 3.15, 0.0);
/// Where packets pass under the gate's lintel.
pub const GATE: Vec3 = Vec3::new(GATE_X, 2.7, 0.0);
/// The obelisk's foot, on top of the podium.
pub const OBELISK_FOOT: Vec3 = Vec3::new(VAULT_X, 2.26, 0.0);
pub const OBELISK_HEIGHT: f32 = 4.2;
/// The vault: a glass case round the obelisk.
pub const VAULT_MIN: Vec3 = Vec3::new(VAULT_X - 1.15, 2.26, -1.15);
pub const VAULT_MAX: Vec3 = Vec3::new(VAULT_X + 1.15, 7.4, 1.15);
/// Where a shard enters the vault, and where it opens.
pub const VAULT_DOOR: Vec3 = Vec3::new(VAULT_X - 1.25, 3.6, 0.0);
pub const VAULT_HEART: Vec3 = Vec3::new(VAULT_X - 0.7, 3.6, 0.55);
/// The provider's evidence tablet, floating before the vault.
pub const TABLET: Vec3 = Vec3::new(VAULT_X - 2.75, 4.35, 0.9);
/// The two fingerprint tablets, before the podium.
pub const HASHES: Vec3 = Vec3::new(VAULT_X, 1.75, 3.2);

/// Colours, linear.
pub fn gold() -> [f32; 3] {
    [1.0, 0.56, 0.16]
}
pub fn amber() -> [f32; 3] {
    [0.95, 0.38, 0.07]
}
pub fn red() -> [f32; 3] {
    [1.0, 0.09, 0.04]
}
pub fn white_hot() -> [f32; 3] {
    [1.0, 0.93, 0.8]
}
fn marble() -> [f32; 3] {
    rgb(0xD8D2C6)
}
fn marble_shade() -> [f32; 3] {
    rgb(0xB9B1A2)
}
fn limestone() -> [f32; 3] {
    rgb(0x8A7D66)
}
pub fn bronze() -> [f32; 3] {
    rgb(0x6A4424)
}
fn floor() -> [f32; 3] {
    rgb(0x15120F)
}

/// The meshes that are drawn as they are, every frame.
pub struct Statics {
    pub solid: Mesh,
    pub inlay: Lines,
}

/// The meshes drawn at a place, with a tint, each frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Obelisk,
    Glass,
    VaultEdges,
    Crack,
    Orb,
    Tablet,
    Bar,
    Shard,
    Glyph,
    Ring,
    Seal,
}

impl Part {
    pub const ALL: [Self; 11] = [
        Self::Obelisk,
        Self::Glass,
        Self::VaultEdges,
        Self::Crack,
        Self::Orb,
        Self::Tablet,
        Self::Bar,
        Self::Shard,
        Self::Glyph,
        Self::Ring,
        Self::Seal,
    ];

    /// True for parts drawn as lines.
    #[must_use]
    pub fn is_lines(self) -> bool {
        matches!(self, Self::VaultEdges | Self::Crack)
    }
}

/// A part's mesh, in white (tinted when drawn) unless it has its own
/// colours.
#[must_use]
pub fn part_mesh(part: Part) -> Mesh {
    let mut mesh = Mesh::new();
    let white = [1.0; 3];
    match part {
        Part::Obelisk => {
            mesh.add(
                Shape::Prism {
                    sides: 4,
                    bottom: 0.58,
                    top: 0.4,
                    turn: FRAC_PI_4,
                },
                Mat4::from_scale(Vec3::new(1.0, OBELISK_HEIGHT, 1.0)),
                white,
            );
            mesh.add(
                Shape::Prism {
                    sides: 4,
                    bottom: 0.4,
                    top: 0.0,
                    turn: FRAC_PI_4,
                },
                Mat4::from_translation(Vec3::Y * OBELISK_HEIGHT)
                    * Mat4::from_scale(Vec3::new(1.0, 0.62, 1.0)),
                white,
            );
        }
        Part::Glass => mesh.block(VAULT_MIN, VAULT_MAX, white),
        Part::Orb => mesh.add(
            Shape::Sphere {
                rings: 10,
                segments: 16,
            },
            Mat4::IDENTITY,
            white,
        ),
        Part::Tablet => mesh.block(
            Vec3::new(-0.45, -0.6, -0.04),
            Vec3::new(0.45, 0.6, 0.04),
            white,
        ),
        Part::Bar => mesh.block(Vec3::splat(-0.5), Vec3::splat(0.5), white),
        Part::Shard => mesh.add(Shape::Shard, Mat4::IDENTITY, white),
        Part::Glyph => mesh.add(
            Shape::Prism {
                sides: 4,
                bottom: 0.5,
                top: 0.0,
                turn: 0.0,
            },
            Mat4::IDENTITY,
            white,
        ),
        Part::Ring => mesh.add(
            Shape::Torus {
                segments: 40,
                sides: 6,
                radius: 0.5,
                tube: 0.025,
            },
            Mat4::IDENTITY,
            white,
        ),
        Part::Seal => mesh.add(
            Shape::Prism {
                sides: 16,
                bottom: 0.5,
                top: 0.5,
                turn: 0.0,
            },
            Mat4::IDENTITY,
            white,
        ),
        Part::VaultEdges | Part::Crack => {}
    }
    mesh
}

/// A part's lines, in white.
#[must_use]
pub fn part_lines(part: Part) -> Lines {
    let mut lines = Lines::new();
    let white = [1.0; 3];
    match part {
        Part::VaultEdges => {
            lines.box_edges(VAULT_MIN, VAULT_MAX, white);
            // A frame at the lid and the base, like a reliquary.
            for y in [VAULT_MIN.y + 0.12, VAULT_MAX.y - 0.12] {
                lines.box_edges(
                    Vec3::new(VAULT_MIN.x, y, VAULT_MIN.z),
                    Vec3::new(VAULT_MAX.x, y, VAULT_MAX.z),
                    white,
                );
            }
        }
        Part::Crack => {
            // A jagged crack across the vault's front pane.
            let z = VAULT_MAX.z + 0.02;
            let pts = [
                (-1.1, 6.6),
                (-0.5, 5.9),
                (-0.7, 5.3),
                (0.1, 4.6),
                (-0.1, 4.0),
                (0.6, 3.3),
                (0.4, 2.7),
                (1.1, 2.3),
            ];
            for w in pts.windows(2) {
                lines.line(
                    Vec3::new(VAULT_X + w[0].0, w[0].1, z),
                    Vec3::new(VAULT_X + w[1].0, w[1].1, z),
                    white,
                );
            }
            for (from, to) in [
                ((-0.5, 5.9), (0.4, 6.2)),
                ((0.1, 4.6), (0.9, 4.9)),
                ((0.6, 3.3), (-0.6, 3.0)),
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

/// Stepped blocks, each centred on `at`, from the ground up:
/// `(half width, height, colour)`.
fn steps(mesh: &mut Mesh, at: Vec3, tiers: &[(f32, f32, [f32; 3])]) -> f32 {
    let mut y = at.y;
    for &(half, height, colour) in tiers {
        mesh.block(
            Vec3::new(at.x - half, y, at.z - half),
            Vec3::new(at.x + half, y + height, at.z + half),
            colour,
        );
        y += height;
    }
    y
}

/// The floor, the far colonnade, the three stations' stone.
#[must_use]
pub fn statics() -> Statics {
    let mut solid = Mesh::new();
    let mut inlay = Lines::new();
    solid.add(
        Shape::Prism {
            sides: 72,
            bottom: 48.0,
            top: 48.0,
            turn: 0.0,
        },
        Mat4::from_translation(Vec3::new(0.0, -0.05, 0.0))
            * Mat4::from_scale(Vec3::new(1.0, 0.05, 1.0)),
        floor(),
    );

    // The floor's amber inlay: rings round the plaza and each station,
    // radial joints, and a processional way from You to the vault.
    let dim = amber().map(|c| c * 0.16);
    let faint = amber().map(|c| c * 0.07);
    for r in [12.0, 15.0, 18.5] {
        inlay.circle(Vec3::new(0.0, 0.01, 0.0), r, 128, faint);
    }
    for i in 0..36 {
        let a = i as f32 / 36.0 * TAU;
        let d = Vec3::new(a.cos(), 0.0, a.sin());
        inlay.line(d * 12.0 + Vec3::Y * 0.01, d * 18.5 + Vec3::Y * 0.01, faint);
    }
    for (x, r) in [(YOU_X, 2.3), (GATE_X, 3.2), (VAULT_X, 3.2)] {
        inlay.circle(Vec3::new(x, 0.01, 0.0), r, 64, dim);
        inlay.circle(Vec3::new(x, 0.01, 0.0), r + 0.25, 64, faint);
    }
    for z in [-0.75_f32, 0.75] {
        inlay.line(
            Vec3::new(YOU_X + 2.3, 0.01, z),
            Vec3::new(GATE_X - 3.2, 0.01, z),
            dim,
        );
        inlay.line(
            Vec3::new(GATE_X + 3.2, 0.01, z),
            Vec3::new(VAULT_X - 3.2, 0.01, z),
            dim,
        );
    }

    // The far colonnade: an arc of columns behind the stations, under a
    // continuous architrave.
    let arc = 21.0;
    let count = 15;
    let mut prev: Option<Vec3> = None;
    for i in 0..count {
        let a = PI + 0.32 + i as f32 / (count - 1) as f32 * (PI - 0.64);
        let at = Vec3::new(arc * a.cos(), 0.0, arc * a.sin() * 0.62 - 3.0);
        solid.column(at, 7.5, 0.42, limestone(), bronze());
        if let Some(p) = prev {
            let mid = (p + at) * 0.5;
            let d = at - p;
            let rot = Quat::from_rotation_y(-d.z.atan2(d.x));
            solid.add(
                Shape::Cube,
                Mat4::from_scale_rotation_translation(
                    Vec3::new(d.length() + 1.2, 0.9, 1.2),
                    rot,
                    mid + Vec3::Y * 7.95,
                ),
                limestone(),
            );
        }
        prev = Some(at);
    }

    // You: a stepped marble plinth with a bronze stand and a lens ring.
    let top = steps(
        &mut solid,
        Vec3::new(YOU_X, 0.0, 0.0),
        &[
            (1.5, 0.22, limestone()),
            (1.2, 0.22, marble_shade()),
            (0.8, 1.42, marble()),
            (0.95, 0.18, marble()),
        ],
    );
    solid.add(
        Shape::Prism {
            sides: 8,
            bottom: 0.3,
            top: 0.12,
            turn: 0.0,
        },
        Mat4::from_translation(Vec3::new(YOU_X, top, 0.0))
            * Mat4::from_scale(Vec3::new(1.0, 0.45, 1.0)),
        bronze(),
    );
    solid.add(
        Shape::Torus {
            segments: 36,
            sides: 6,
            radius: 0.62,
            tube: 0.045,
        },
        Mat4::from_translation(ORB),
        bronze(),
    );

    // The gate: a podium, four marble columns, a lintel with bronze panels.
    let deck = steps(
        &mut solid,
        Vec3::new(GATE_X, 0.0, 0.0),
        &[(2.7, 0.2, limestone()), (2.4, 0.2, marble_shade())],
    );
    for x in [-1.65_f32, 1.65] {
        for z in [-0.95_f32, 0.95] {
            solid.column(
                Vec3::new(GATE_X + x, deck, z),
                4.2,
                0.27,
                marble(),
                bronze(),
            );
        }
    }
    let lintel = deck + 4.2;
    solid.block(
        Vec3::new(GATE_X - 2.35, lintel, -1.4),
        Vec3::new(GATE_X + 2.35, lintel + 0.45, 1.4),
        marble(),
    );
    solid.block(
        Vec3::new(GATE_X - 2.25, lintel + 0.45, -1.3),
        Vec3::new(GATE_X + 2.25, lintel + 0.85, 1.3),
        marble_shade(),
    );
    solid.block(
        Vec3::new(GATE_X - 2.5, lintel + 0.85, -1.5),
        Vec3::new(GATE_X + 2.5, lintel + 1.05, 1.5),
        marble(),
    );
    for i in 0..5 {
        let x = GATE_X - 1.6 + i as f32 * 0.8;
        solid.block(
            Vec3::new(x - 0.16, lintel + 0.53, 1.29),
            Vec3::new(x + 0.16, lintel + 0.77, 1.34),
            bronze(),
        );
    }

    // The vault's podium: three steps and a marble plinth.
    steps(
        &mut solid,
        Vec3::new(VAULT_X, 0.0, 0.0),
        &[
            (2.5, 0.3, limestone()),
            (2.1, 0.3, marble_shade()),
            (1.75, 0.3, marble()),
            (1.35, 1.2, marble()),
            (1.5, 0.16, marble()),
        ],
    );
    Statics { solid, inlay }
}

/// One part drawn at a place.
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub part: Part,
    pub model: Mat4,
    /// Multiplies the part's colour.
    pub tint: [f32; 3],
    /// Light the part gives off, added after shading.
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

/// Everything one frame draws besides the statics.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub draws: Vec<Draw>,
    pub sparks: Vec<Spark>,
    pub beams: Vec<Beam>,
}

fn scale(c: [f32; 3], k: f32) -> [f32; 3] {
    c.map(|v| v * k)
}

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A point along the packet path `points`, `u` from 0 to 1, arcing up
/// between stops.
#[must_use]
pub fn along(points: &[Vec3], u: f32) -> Vec3 {
    let legs = points.len() - 1;
    let u = u.clamp(0.0, 1.0) * legs as f32;
    let leg = (u.floor() as usize).min(legs - 1);
    let f = u - leg as f32;
    let (a, b) = (points[leg], points[leg + 1]);
    a.lerp(b, f) + Vec3::Y * (f * PI).sin() * 0.9
}

/// The way a sealed shard travels, You to the vault.
#[must_use]
pub fn outbound() -> [Vec3; 3] {
    [ORB + Vec3::Y * 0.85, GATE, VAULT_DOOR]
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

/// The look of a finished check: gold when it passed, red when refused,
/// dim when skipped, and amber while working.
fn verdict_colour(state: &State) -> [f32; 3] {
    match state {
        State::Ok => gold(),
        State::Refused(_) => red(),
        State::Skipped => [0.12, 0.1, 0.08],
        _ => amber(),
    }
}

fn shard(frame: &mut Frame, at: Vec3, spin: f32, size: f32, edge: [f32; 3]) {
    frame.draws.push(Draw {
        part: Part::Shard,
        model: Mat4::from_scale_rotation_translation(
            Vec3::splat(size),
            Quat::from_rotation_y(spin) * Quat::from_rotation_z(0.25),
            at,
        ),
        tint: [0.06, 0.05, 0.05],
        glow: scale(edge, 0.2),
        alpha: 1.0,
    });
    frame.sparks.push(Spark {
        at,
        size: size * 1.3,
        colour: scale(edge, 0.45),
    });
}

fn glyph(frame: &mut Frame, at: Vec3, spin: f32, size: f32, bright: f32) {
    for flip in [false, true] {
        let rot = Quat::from_rotation_y(spin)
            * if flip {
                Quat::from_rotation_x(PI)
            } else {
                Quat::IDENTITY
            };
        frame.draws.push(Draw {
            part: Part::Glyph,
            model: Mat4::from_scale_rotation_translation(
                Vec3::new(size, size * 0.7, size),
                rot,
                at,
            ),
            tint: [1.0; 3],
            glow: scale(white_hot(), 0.9 * bright),
            alpha: 1.0,
        });
    }
    frame.sparks.push(Spark {
        at,
        size: size * 2.6,
        colour: scale(white_hot(), 0.55 * bright),
    });
}

/// Builds one frame: `now` in seconds on the page clock, `lit` as
/// [`crate::show::Show::provider_lit`] set it, `motion` false to keep
/// everything still but the essentials.
#[must_use]
pub fn frame(anims: &[Anim; 9], lit: Option<bool>, now: f64, motion: bool) -> Frame {
    let mut frame = Frame::default();
    let time = now as f32;
    let wave = |speed: f32| if motion { (time * speed).sin() } else { 0.0 };
    let get = |step: Step| phase(&anims[step.index()], now);

    // The orb at You: always softly lit, brighter while a round runs.
    let any = anims.iter().any(|a| a.started.is_some());
    let orb_glow = if any {
        0.75 + 0.15 * wave(2.2)
    } else {
        0.45 + 0.1 * wave(1.2)
    };
    frame.draws.push(Draw {
        part: Part::Orb,
        model: Mat4::from_scale_rotation_translation(Vec3::splat(0.62), Quat::IDENTITY, ORB),
        tint: [0.9, 0.85, 0.75],
        glow: scale([1.0, 0.8, 0.55], orb_glow),
        alpha: 1.0,
    });
    frame.sparks.push(Spark {
        at: ORB,
        size: 1.9,
        colour: scale([1.0, 0.7, 0.4], 0.35 * orb_glow),
    });

    // The obelisk and its vault.
    let pulse = 0.82 + 0.18 * wave(1.6);
    let (obelisk_glow, edge, glass) = match lit {
        Some(true) => (
            scale(gold(), 0.55 * pulse),
            scale(gold(), 0.95),
            [0.9, 0.7, 0.4],
        ),
        Some(false) => (scale(red(), 0.14), scale(red(), 0.85), [0.9, 0.2, 0.15]),
        None => (
            [0.012, 0.008, 0.005],
            scale(amber(), 0.28),
            [0.5, 0.48, 0.45],
        ),
    };
    frame.draws.push(Draw {
        part: Part::Obelisk,
        model: Mat4::from_translation(OBELISK_FOOT),
        tint: bronze(),
        glow: obelisk_glow,
        alpha: 1.0,
    });
    frame.draws.push(Draw {
        part: Part::VaultEdges,
        model: Mat4::IDENTITY,
        tint: edge,
        glow: [0.0; 3],
        alpha: 1.0,
    });
    if lit == Some(true) {
        frame.sparks.push(Spark {
            at: OBELISK_FOOT + Vec3::Y * (OBELISK_HEIGHT + 0.35),
            size: 2.4 * pulse,
            colour: scale(gold(), 0.5),
        });
        frame.sparks.push(Spark {
            at: OBELISK_FOOT + Vec3::Y * 2.0,
            size: 4.6,
            colour: scale(gold(), 0.18 * pulse),
        });
    }
    if lit == Some(false) {
        frame.draws.push(Draw {
            part: Part::Crack,
            model: Mat4::IDENTITY,
            tint: scale(red(), 1.0),
            glow: [0.0; 3],
            alpha: 1.0,
        });
        frame.sparks.push(Spark {
            at: Vec3::new(VAULT_X, 4.6, VAULT_MAX.z),
            size: 3.4,
            colour: scale(red(), 0.22 * pulse),
        });
    }

    // Fetch: a beam from the gate to You carries the evidence; the
    // evidence tablet appears before the vault.
    let tablet_seen = get(Step::Fetch).map_or(0.0, |p| ease(p.t / 0.6));
    if let Some(p) = get(Step::Fetch) {
        let fade = p.settled.map_or(1.0, |s| 1.0 - ease(s / 0.9));
        if fade > 0.0 {
            let from = GATE + Vec3::Y * 1.0;
            let to = ORB;
            frame.beams.push(Beam {
                from,
                to,
                width: 0.16,
                colour: scale(amber(), 0.55 * fade),
            });
            for k in 0..3 {
                let u = ((p.t * 1.1 + k as f32 / 3.0) % 1.0).clamp(0.0, 1.0);
                frame.sparks.push(Spark {
                    at: from.lerp(to, u),
                    size: 0.55,
                    colour: scale(gold(), 0.9 * fade),
                });
            }
        }
    }
    if tablet_seen > 0.0 {
        let chain = get(Step::Chain);
        let colour = chain
            .as_ref()
            .filter(|p| p.settled.is_some())
            .map_or(amber(), |p| verdict_colour(p.state));
        frame.draws.push(Draw {
            part: Part::Tablet,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(tablet_seen),
                Quat::from_rotation_y(0.35) * Quat::from_rotation_x(0.05 * wave(0.7)),
                TABLET + Vec3::Y * 0.08 * wave(0.9),
            ),
            tint: limestone(),
            glow: scale(colour, 0.12),
            alpha: 1.0,
        });
        // Engraved lines on its face.
        for i in 0..5 {
            let y = 0.38 - i as f32 * 0.19;
            let w = if i % 2 == 0 { 0.62 } else { 0.44 };
            frame.draws.push(Draw {
                part: Part::Bar,
                model: Mat4::from_translation(TABLET + Vec3::Y * 0.08 * wave(0.9))
                    * Mat4::from_quat(Quat::from_rotation_y(0.35))
                    * Mat4::from_scale_rotation_translation(
                        Vec3::new(w, 0.05, 0.02) * tablet_seen,
                        Quat::IDENTITY,
                        Vec3::new(0.0, y * tablet_seen, 0.05),
                    ),
                tint: [0.2; 3],
                glow: scale(colour, 0.7),
                alpha: 1.0,
            });
        }
    }

    // Chain: a ring of light round the tablet.
    if let Some(p) = get(Step::Chain) {
        let colour = if p.settled.is_some() {
            verdict_colour(p.state)
        } else {
            amber()
        };
        let grow = ease(p.t / 0.45);
        let bright = match p.settled {
            Some(s) => 0.6 + 0.6 * (1.0 - ease(s / 0.6)),
            None => 0.5 + 0.3 * wave(6.0),
        };
        let centre = TABLET + Vec3::Y * 0.08 * wave(0.9);
        frame.draws.push(Draw {
            part: Part::Ring,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(1.9 * grow),
                Quat::from_rotation_y(0.35) * Quat::from_rotation_x(-0.25),
                centre,
            ),
            tint: [0.2; 3],
            glow: scale(colour, bright),
            alpha: 1.0,
        });
        // Links of the chain, running round it.
        let speed = if p.settled.is_some() { 0.4 } else { 2.4 };
        for k in 0..4 {
            let a = if motion { time * speed } else { 0.0 } + k as f32 / 4.0 * TAU;
            let local = Vec3::new(a.cos(), a.sin(), 0.0) * 0.95 * grow;
            let at = centre + Quat::from_rotation_y(0.35) * (Quat::from_rotation_x(-0.25) * local);
            frame.sparks.push(Spark {
                at,
                size: 0.42,
                colour: scale(colour, bright),
            });
        }
    }

    // Measure: two fingerprint tablets, one measured at the vault, one from
    // the public log, slide into line.
    if let Some(p) = get(Step::Measure) {
        let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
        let colour = if p.settled.is_some() {
            verdict_colour(p.state)
        } else {
            amber()
        };
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
                    side * 0.55,
                    offset + (1.0 - appear) * 1.5 * side.max(0.0),
                    0.0,
                );
            frame.draws.push(Draw {
                part: Part::Tablet,
                model: Mat4::from_scale_rotation_translation(
                    Vec3::new(1.1, 1.0, 1.0) * appear.max(0.01),
                    Quat::IDENTITY,
                    centre,
                ),
                tint: marble(),
                glow: scale(colour, 0.06),
                alpha: 1.0,
            });
            for (i, w) in pattern.iter().enumerate() {
                // The logged build's tablet shows the same marks; a changed
                // fingerprint shows different ones.
                let w = if refused && side > 0.0 {
                    pattern[(i + 2) % 5]
                } else {
                    *w
                };
                let y = 0.3 - i as f32 * 0.15;
                frame.draws.push(Draw {
                    part: Part::Bar,
                    model: Mat4::from_scale_rotation_translation(
                        Vec3::new(w * 0.95, 0.045, 0.02) * appear.max(0.01),
                        Quat::IDENTITY,
                        centre + Vec3::new(0.0, y * appear, 0.05),
                    ),
                    tint: [0.15; 3],
                    glow: scale(colour, 0.8),
                    alpha: 1.0,
                });
            }
        }
        if p.settled.is_some() && !refused && matches!(p.state, State::Ok) {
            for i in 0..5 {
                let y = 0.3 - i as f32 * 0.15;
                frame.beams.push(Beam {
                    from: HASHES + Vec3::new(-0.55, y, 0.07),
                    to: HASHES + Vec3::new(0.55, y, 0.07),
                    width: 0.035,
                    colour: scale(gold(), 0.8),
                });
            }
        }
        if refused {
            frame.sparks.push(Spark {
                at: HASHES + Vec3::Y * 0.2,
                size: 2.2,
                colour: scale(red(), 0.35),
            });
        }
    }

    // Bind: a line of light from the evidence to the obelisk's key.
    if let Some(p) = get(Step::Bind) {
        let colour = if p.settled.is_some() {
            verdict_colour(p.state)
        } else {
            amber()
        };
        let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
        let target = OBELISK_FOOT + Vec3::new(-0.45, 2.6, 0.45);
        let reach = if refused { 0.55 } else { ease(p.t / 0.55) };
        let end = TABLET.lerp(target, reach);
        frame.beams.push(Beam {
            from: TABLET,
            to: end,
            width: 0.09,
            colour: scale(colour, 0.9),
        });
        frame.sparks.push(Spark {
            at: end,
            size: if refused { 1.4 } else { 0.8 },
            colour: scale(colour, 0.9),
        });
        if p.settled.is_some() && !refused {
            frame.sparks.push(Spark {
                at: TABLET.lerp(target, 0.5),
                size: 0.7,
                colour: scale(gold(), 0.7),
            });
        }
    }

    // The message: plain at You until sealed, a shard on the way, plain
    // again only inside the vault and back at You.
    message(&mut frame, anims, now, motion);

    // Receipt: a seal stamped onto You's plinth.
    if let Some(p) = get(Step::Receipt) {
        let colour = if p.settled.is_some() {
            verdict_colour(p.state)
        } else {
            amber()
        };
        let land = ease(p.t / 0.45);
        let rest = Vec3::new(YOU_X, 2.12, 0.98);
        let at = rest + Vec3::Y * (1.0 - land) * 2.2;
        frame.draws.push(Draw {
            part: Part::Seal,
            model: Mat4::from_scale_rotation_translation(
                Vec3::new(0.7, 0.06, 0.7),
                Quat::from_rotation_x(PI / 2.0 * land),
                at,
            ),
            tint: bronze(),
            glow: scale(colour, 0.45),
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
                        rest + Vec3::Z * 0.06,
                    ),
                    tint: [0.2; 3],
                    glow: scale(colour, 1.0 - ring / 0.8),
                    alpha: 1.0,
                });
            }
            frame.sparks.push(Spark {
                at: rest + Vec3::Z * 0.1,
                size: 1.0,
                colour: scale(colour, 0.5),
            });
        }
    }

    // A halo at the vault's foot while a round is under way.
    let running = anims.iter().any(|a| a.state == State::Running);
    if running {
        frame.draws.push(Draw {
            part: Part::Ring,
            model: Mat4::from_scale_rotation_translation(
                Vec3::splat(5.8 + 0.25 * wave(2.0)),
                Quat::from_rotation_x(PI / 2.0),
                Vec3::new(VAULT_X, 0.04, 0.0),
            ),
            tint: [0.1; 3],
            glow: scale(amber(), 0.35),
            alpha: 1.0,
        });
    }

    // The glass goes last, over everything inside it.
    frame.draws.push(Draw {
        part: Part::Glass,
        model: Mat4::IDENTITY,
        tint: glass,
        glow: scale(edge, 0.03),
        alpha: if lit == Some(false) { 0.3 } else { 0.16 },
    });
    frame
}

/// Where the message is and what it looks like.
fn message(frame: &mut Frame, anims: &[Anim; 9], now: f64, motion: bool) {
    let time = now as f32;
    let spin = if motion { time * 1.3 } else { 0.4 };
    let get = |step: Step| phase(&anims[step.index()], now);
    let above = ORB + Vec3::Y * 0.85;
    let latest = [Step::Answer, Step::Decrypt, Step::Relay, Step::Encrypt]
        .into_iter()
        .find_map(|s| {
            get(s)
                .filter(|p| *p.state != State::Skipped)
                .map(|p| (s, p))
        });
    let Some((step, p)) = latest else {
        // Not sealed yet: your message, in the open, at You.
        if anims.iter().any(|a| a.started.is_some()) {
            glyph(frame, above, spin, 0.42, 0.8);
        }
        return;
    };
    let refused = matches!(p.state, State::Refused(_)) && p.settled.is_some();
    let edge = if refused { red() } else { amber() };
    match step {
        Step::Encrypt => {
            let k = ease((p.t - 0.15) / 0.4);
            if k < 1.0 {
                glyph(frame, above, spin, 0.42 * (1.0 - k), 1.0);
            }
            if k > 0.0 {
                shard(frame, above, spin, 0.75 * k, edge);
            }
            if (0.0..0.6).contains(&(p.t - 0.25)) {
                frame.sparks.push(Spark {
                    at: above,
                    size: 2.2,
                    colour: scale(gold(), 0.6 * (1.0 - (p.t - 0.25) / 0.6)),
                });
            }
        }
        Step::Relay | Step::Answer => {
            let travel = 1.35;
            let mut u = (p.t / travel).min(1.0);
            if refused {
                u = u.min(0.5);
            }
            let path = outbound();
            let at = if step == Step::Relay {
                along(&path, u)
            } else {
                along(&[path[2], path[1], path[0]], u)
            };
            let hover = if u >= 1.0 && p.settled.is_none() {
                Vec3::Y * 0.08 * (time * 4.0).sin()
            } else {
                Vec3::ZERO
            };
            let arrived_home = step == Step::Answer && u >= 1.0 && p.settled.is_some() && !refused;
            if arrived_home {
                // Back at You, the answer opens.
                let s = p.settled.unwrap_or(0.0).min((p.t - travel).max(0.0));
                let k = ease(s / 0.4);
                if k < 1.0 {
                    shard(frame, above, spin, 0.75 * (1.0 - k), edge);
                }
                glyph(frame, above, spin, 0.42 * k.max(0.01), 1.0);
            } else {
                shard(frame, at + hover, spin, 1.0, edge);
                // A faint wake behind it.
                for k in 1..4 {
                    let back = (u - k as f32 * 0.035).max(0.0);
                    let wake = if step == Step::Relay {
                        along(&path, back)
                    } else {
                        along(&[path[2], path[1], path[0]], back)
                    };
                    frame.sparks.push(Spark {
                        at: wake,
                        size: 0.5 - k as f32 * 0.1,
                        colour: scale(edge, 0.25 / k as f32),
                    });
                }
            }
            if (0.35..0.65).contains(&u) {
                // Passing the gate: the lintel's lamps flicker.
                frame.sparks.push(Spark {
                    at: GATE + Vec3::Y * 1.6,
                    size: 1.6,
                    colour: scale(amber(), 0.25),
                });
            }
        }
        Step::Decrypt => {
            // Inside the vault the shard opens: two halves part and the
            // plain message shines.
            let k = ease(p.t / 0.5);
            let at = VAULT_DOOR.lerp(VAULT_HEART, ease(p.t / 0.3));
            if refused {
                shard(frame, at, spin, 0.75, red());
                return;
            }
            if k < 1.0 {
                for side in [-1.0_f32, 1.0] {
                    frame.draws.push(Draw {
                        part: Part::Shard,
                        model: Mat4::from_scale_rotation_translation(
                            Vec3::new(0.75, 0.38, 0.75) * (1.0 - k * 0.6),
                            Quat::from_rotation_y(spin),
                            at + Vec3::Y * side * 0.4 * k,
                        ),
                        tint: [0.05; 3],
                        glow: scale(amber(), 0.15),
                        alpha: 1.0,
                    });
                }
            }
            glyph(frame, at, spin, 0.34 * k.max(0.01), 1.0);
            if p.t < 0.9 {
                frame.sparks.push(Spark {
                    at,
                    size: 2.6,
                    colour: scale(white_hot(), 0.5 * (1.0 - p.t / 0.9)),
                });
            }
        }
        _ => {}
    }
}

/// The camera: where it stands and what it sees.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub near: f32,
    pub far: f32,
}

/// Half the scene's width that must fit in view, and its height.
const HALF_WIDTH: f32 = 11.8;
const HEIGHT: f32 = 9.0;
/// The height the camera looks at.
pub const TARGET_Y: f32 = 3.4;

/// A camera that fits the three stations into a region of the canvas:
/// `region` is that region's width as a share of the canvas, `centre` its
/// centre in clip space (-1 to 1), `sway` the slow drift's angle.
#[must_use]
pub fn camera(aspect: f32, region: f32, centre: f32, sway: f32) -> Camera {
    let fovy = 32.0_f32.to_radians();
    let half = (fovy / 2.0).tan();
    let target = Vec3::new(0.0, TARGET_Y, 0.4);
    let width = (region * aspect).max(0.2);
    let distance = (HALF_WIDTH / (width * half)).max(HEIGHT * 0.5 / half) + 0.8;
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
        near,
        far,
    }
}

/// Where each station's label sits in the scene.
#[must_use]
pub fn label_anchors() -> [Vec3; 3] {
    [
        Vec3::new(YOU_X, 4.6, 0.0),
        Vec3::new(GATE_X, 6.0, 0.0),
        Vec3::new(VAULT_X, 8.3, 0.0),
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
        assert!(statics.solid.vertices() > 1000);
        assert!(statics.solid.vertices().is_multiple_of(3));
        assert!(statics.inlay.vertices().is_multiple_of(2));
        for part in Part::ALL {
            if part.is_lines() {
                assert!(part_lines(part).vertices() > 0, "{part:?}");
            } else {
                assert!(part_mesh(part).vertices() > 0, "{part:?}");
            }
        }
    }

    #[test]
    fn the_vault_lights_gold_or_cracks_red() {
        let idle: [Anim; 9] = Default::default();
        let dark = frame(&idle, None, 1.0, true);
        let obelisk = |f: &Frame| {
            f.draws
                .iter()
                .find(|d| d.part == Part::Obelisk)
                .unwrap()
                .glow
        };
        assert!(obelisk(&dark)[0] < 0.05);
        let lit = frame(&idle, Some(true), 1.0, true);
        assert!(obelisk(&lit)[0] > 0.3 && obelisk(&lit)[1] > 0.1);
        let refused = frame(&idle, Some(false), 1.0, true);
        assert!(refused.draws.iter().any(|d| d.part == Part::Crack));
        assert!(!lit.draws.iter().any(|d| d.part == Part::Crack));
        // The glass is drawn last.
        assert_eq!(lit.draws.last().unwrap().part, Part::Glass);
    }

    #[test]
    fn the_message_is_plain_only_at_you_and_in_the_vault() {
        let glyphs = |f: &Frame| -> Vec<Vec3> {
            f.draws
                .iter()
                .filter(|d| d.part == Part::Glyph)
                .map(|d| d.model.transform_point3(Vec3::ZERO))
                .collect()
        };
        let shards = |f: &Frame| f.draws.iter().filter(|d| d.part == Part::Shard).count();
        // Before sealing: plain at You.
        let f = frame(
            &anims(&[(Step::Fetch, State::Ok, 0.0, Some(1.0))]),
            None,
            2.0,
            true,
        );
        assert!(glyphs(&f).iter().all(|p| (p.x - YOU_X).abs() < 0.5));
        assert!(!glyphs(&f).is_empty());
        // On the way: only a shard, half way between You and the vault.
        let f = frame(
            &anims(&[
                (Step::Encrypt, State::Ok, 0.0, Some(0.7)),
                (Step::Relay, State::Running, 1.0, None),
            ]),
            Some(true),
            1.6,
            true,
        );
        assert!(glyphs(&f).is_empty());
        assert_eq!(shards(&f), 1);
        // Opened inside the vault.
        let f = frame(
            &anims(&[
                (Step::Relay, State::Ok, 0.0, Some(1.4)),
                (Step::Decrypt, State::Ok, 1.5, Some(2.2)),
            ]),
            Some(true),
            3.0,
            true,
        );
        let inside = glyphs(&f);
        assert!(!inside.is_empty());
        assert!(
            inside
                .iter()
                .all(|p| p.x > VAULT_MIN.x && p.x < VAULT_MAX.x)
        );
        // And back at You once the answer arrives.
        let f = frame(
            &anims(&[
                (Step::Decrypt, State::Ok, 0.0, Some(0.6)),
                (Step::Answer, State::Ok, 1.0, Some(2.5)),
            ]),
            Some(true),
            4.0,
            true,
        );
        assert!(glyphs(&f).iter().all(|p| (p.x - YOU_X).abs() < 0.5));
        assert_eq!(shards(&f), 0);
    }

    #[test]
    fn packets_arc_between_stops() {
        let path = outbound();
        assert!((along(&path, 0.0) - path[0]).length() < 1e-4);
        assert!((along(&path, 0.5) - path[1]).length() < 1e-4);
        assert!((along(&path, 1.0) - path[2]).length() < 1e-4);
        assert!(along(&path, 0.25).y > path[0].y.min(path[1].y) + 0.5);
    }

    #[test]
    fn the_camera_fits_the_stations() {
        for (aspect, region, centre) in [(1.6, 1.0, 0.0), (1.8, 0.45, 0.05), (0.6, 1.0, 0.0)] {
            let camera = camera(aspect, region, centre, 0.0);
            for x in [YOU_X - 1.6, VAULT_X + 2.5] {
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
