//! The sledgehammer the yard and the town share: the swing's timing, the
//! head's path through the blow, and the hammer, its streak, cracks, and
//! dust as shaded faces. With the pack's two-handed chop the character
//! plays it and the blow lands at the chop's impact; without it a
//! character-less swing turns the hammer over the shoulder.

use super::site::{self, PieceSpec};
use crate::controller::PlayerController;
use crate::mesh::{Mesh, Vertex};
use crate::zones::everglade::draw::shade;
use crate::zones::everglade::player::{BUTT, HEAD_AT, Hold, SwingTrack};
use glam::{Mat4, Quat, Vec3};

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
pub const REACH: f32 = 0.5;
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

/// The swing under way, if any, and the chop it plays.
#[derive(Default)]
pub struct Hammer {
    /// Time into the current swing, s: of the pack's chop when there is a
    /// track, else of the character-less swing.
    swing: Option<f32>,
    /// The chop as the player's character plays it.
    track: Option<SwingTrack>,
    struck: bool,
}

impl Hammer {
    /// Swings with the player's character's chop `track` from now on,
    /// or, with `None`, without a character.
    pub fn set_track(&mut self, track: Option<SwingTrack>) {
        self.track = track;
    }

    /// Whether a swing is under way.
    #[must_use]
    pub fn swinging(&self) -> bool {
        self.swing.is_some()
    }

    /// Starts a swing unless one is under way. Returns whether it started.
    pub fn start(&mut self) -> bool {
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

    /// Advances the swing by `dt` for `player`. Returns the head's path
    /// through the blow once, on the frame the blow lands.
    pub fn advance(&mut self, dt: f32, player: &PlayerController) -> Option<Vec<Vec3>> {
        let t = self.swing.as_mut()?;
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
        let mut path = None;
        if !self.struck && t >= impact {
            self.struck = true;
            path = Some(self.sweep(player));
        }
        if t >= length {
            self.swing = None;
        }
        path
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

    /// How a character-less swing holds the hammer at `angle` for `player`.
    fn hammer(player: &PlayerController, angle: f32) -> Hold {
        let forward = player.forward();
        let right = forward.cross(Vec3::Y);
        Hold {
            grip: player.pos + Vec3::Y * 1.25 + right * 0.28 + forward * 0.15,
            axis: Vec3::Y * angle.cos() + forward * angle.sin(),
            face: -Vec3::Y * angle.sin() + forward * angle.cos(),
        }
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

    /// Appends the sledgehammer as `hold` holds it (as a character-less
    /// swing holds it without one) and a streak behind its head through
    /// the blow.
    pub fn draw(&self, mesh: &mut Mesh, player: &PlayerController, hold: Option<Hold>) {
        let fallback = self.track.is_none() || hold.is_none();
        let hold = match hold {
            Some(hold) if !fallback => hold,
            _ => Self::hammer(player, Self::angle(self.swing)),
        };
        sledgehammer(mesh, &hold);
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
                mesh,
                [
                    point(&pair[0], HEAD_AT - 0.18),
                    point(&pair[1], HEAD_AT - 0.18),
                    point(&pair[1], HEAD_AT + 0.1),
                    point(&pair[0], HEAD_AT + 0.1),
                ],
                [0.95 * fade, 0.86 * fade, 0.62 * fade],
            );
        }
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

/// Appends the site's dust puffs as sprites in the look of
/// `assets/verse/fx/effects/debris_dust.toml`.
pub fn dust(mesh: &mut Mesh, puffs: &[site::Puff]) {
    let Some(dust) = crate::fx::Style::named("debris_dust") else {
        return;
    };
    for puff in puffs {
        let t = puff.age / puff.life;
        // Steady through the puff's life: what it was born with.
        let seed = puff.life.to_bits() ^ puff.size.to_bits().rotate_left(11);
        mesh.sprites
            .extend(dust.sprite(0, puff.at, puff.vel, t, puff.size, puff.color, seed));
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

pub(super) fn quad(mesh: &mut Mesh, [a, b, c, d]: [Vec3; 4], color: [f32; 3]) {
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
#[must_use]
pub fn noise(n: u32, salt: u32) -> f32 {
    let mut x = n.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

/// Dark jagged cracks over both faces of a damaged wall section, more as
/// its hit points fall, the same on every frame for one `salt`.
pub fn cracks(mesh: &mut Mesh, salt: u32, spec: &PieceSpec, hit_points: i32, pose: Mat4) {
    let damage = 1.0 - hit_points as f32 / spec.hit_points.max(1) as f32;
    let strokes = (damage * 8.0).ceil() as u32;
    let (width, height, depth) = (0.85, 1.35, 0.205);
    let salt = salt * 97 + 13;
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
