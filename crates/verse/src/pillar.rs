//! The bare world's reset pillar: a fixed column with a button on top that
//! returns the ball and the blocks to where they started, for everyone.
//!
//! The bare world has one stick and no action button, so the pillar is
//! pressed by walking into it: the first touch after the pillar was clear
//! presses the button, and holding against it does not press again. A press
//! waits [`COOLDOWN`] seconds after the last. The pillar is a static box in
//! the ball's physics world, so the ball and the blocks strike it and it never
//! moves, and the player is kept out of it like the ball.
//!
//! The button is lit white while anything sits away from home, and charcoal
//! when there is nothing to reset. It sinks briefly when pressed.

use std::sync::OnceLock;

use glam::{DVec3, Mat4, Vec3};
use physics::{Body, BodyId, BodyKind, Collider, Material, Shape, World};

use crate::blocks::{CHARCOAL, Solid, WHITE, push_box};
use crate::controller::{PlayerController, RADIUS as PLAYER_RADIUS};
use crate::pbr::LitVertex;

/// Where the pillar stands: to the right of the spawn and a little ahead,
/// clear of the ball, the stack, and the dominoes.
pub const AT: DVec3 = DVec3::new(
    crate::world::SPAWN.x as f64 + 6.0,
    0.0,
    crate::world::SPAWN.z as f64 + 4.0,
);
/// Half the column's width and depth, m.
pub const HALF: f64 = 0.45;
/// The column's height to the top of its cap, m.
pub const HEIGHT: f64 = 1.1;
/// Least time between presses, s.
pub const COOLDOWN: f64 = 5.0;
/// How close the player's capsule must come to count as touching, m.
const REACH: f64 = 0.06;
/// How long the button stays down after a press, s.
const PRESSED: f64 = 0.4;
/// How far the button sinks, m.
const SINK: f32 = 0.05;

/// The pillar's button state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pillar {
    touching: bool,
    since: f64,
    pressed: f64,
    presses: u64,
}

impl Pillar {
    /// A pillar whose last press is long past.
    #[must_use]
    pub fn new() -> Self {
        Self {
            since: COOLDOWN,
            ..Self::default()
        }
    }

    /// Adds the pillar's fixed box to `world`.
    pub fn add(world: &mut World) -> BodyId {
        let center = AT + DVec3::Y * (HEIGHT / 2.0);
        let id = world.add(Body::new(1.0, DVec3::ONE, center).with_kind(BodyKind::Static));
        world.add_collider(
            Collider::new(
                id,
                Shape::Cuboid {
                    half: DVec3::new(HALF, HEIGHT / 2.0, HALF),
                },
            )
            .with_material(Material {
                friction: 0.6,
                torsional: 0.01,
                restitution: 0.2,
            }),
        );
        id
    }

    /// How many times the button was pressed.
    #[must_use]
    pub fn presses(&self) -> u64 {
        self.presses
    }

    /// Advances `dt` seconds, keeps the player out of the column, and
    /// returns true when this frame's touch presses the button.
    pub fn touch(&mut self, player: &mut PlayerController, dt: f64) -> bool {
        self.since += dt;
        self.pressed = (self.pressed - dt).max(0.0);
        let r = f64::from(PLAYER_RADIUS);
        let feet = player.pos.as_dvec3();
        let dx = feet.x - AT.x;
        let dz = feet.z - AT.z;
        let near = DVec3::new(dx.clamp(-HALF, HALF), 0.0, dz.clamp(-HALF, HALF));
        let away = DVec3::new(dx, 0.0, dz) - near;
        let distance = away.length();
        let low = feet.y < HEIGHT;
        let touching = low && distance < r + REACH;
        if low && distance < r {
            // Out along the shortest way: straight from the nearest face,
            // or, from inside, through the nearest face.
            let out = if distance > 1e-6 {
                near + away / distance * r
            } else if (HALF - dx.abs()) < (HALF - dz.abs()) {
                DVec3::new(dx.signum() * (HALF + r), 0.0, dz)
            } else {
                DVec3::new(dx, 0.0, dz.signum() * (HALF + r))
            };
            player.pos.x = (AT.x + out.x) as f32;
            player.pos.z = (AT.z + out.z) as f32;
        }
        let press = touching && !self.touching && self.since >= COOLDOWN;
        self.touching = touching;
        if press {
            self.since = 0.0;
            self.pressed = PRESSED;
            self.presses += 1;
        }
        press
    }

    /// Adds the column, its cap, and the button to `out`. `armed` lights the
    /// button: something sits away from home.
    pub fn draw(&self, armed: bool, out: &mut Vec<LitVertex>) {
        let sink = if self.pressed > 0.0 { SINK } else { 0.0 };
        let at = AT.as_vec3();
        let place = |solid: &Solid, offset: Vec3, color: [f32; 3], out: &mut Vec<LitVertex>| {
            let t = Mat4::from_translation(at + offset);
            out.extend(solid.vertices.iter().map(|v| LitVertex {
                pos: t.transform_point3(Vec3::from(v.pos)).to_array(),
                color,
                ..*v
            }));
        };
        let [column, cap, button] = parts();
        place(column, Vec3::ZERO, CHARCOAL, out);
        place(cap, Vec3::ZERO, WHITE, out);
        let lit = if armed { WHITE } else { CHARCOAL };
        place(button, Vec3::new(0.0, -sink, 0.0), lit, out);
    }
}

/// The column, its cap, and the button in pillar space, built once.
fn parts() -> &'static [Solid; 3] {
    static PARTS: OnceLock<[Solid; 3]> = OnceLock::new();
    PARTS.get_or_init(|| {
        let solid = |center: Vec3, half: Vec3| {
            let mut solid = Solid::default();
            push_box(&mut solid, center, half, false);
            solid
        };
        let half = HALF as f32;
        let top = HEIGHT as f32;
        [
            solid(
                Vec3::new(0.0, (top - 0.06) / 2.0, 0.0),
                Vec3::new(half, (top - 0.06) / 2.0, half),
            ),
            solid(
                Vec3::new(0.0, top - 0.03, 0.0),
                Vec3::new(half + 0.04, 0.03, half + 0.04),
            ),
            solid(Vec3::new(0.0, top + 0.04, 0.0), Vec3::new(0.22, 0.04, 0.22)),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::InputState;

    #[test]
    fn walking_into_the_pillar_presses_once_and_stops_the_player() {
        let mut pillar = Pillar::new();
        // Stand south of the pillar, facing it (+Z).
        let start = Vec3::new(AT.x as f32, 0.0, AT.z as f32 - 4.0);
        let mut player = PlayerController::new(start, 0.0);
        let input = InputState {
            forward: true,
            ..InputState::default()
        };
        let mut presses = 0;
        for _ in 0..180 {
            player.update(&input, 1.0 / 60.0, &[], crate::world::HALF);
            presses += u32::from(pillar.touch(&mut player, 1.0 / 60.0));
        }
        assert_eq!(presses, 1, "leaning on the pillar presses it once");
        let reach = f64::from(player.pos.z) - (AT.z - HALF);
        assert!(reach < 0.0, "the player stays out: {:?}", player.pos);
        // Stepping back and in again, within the cooldown, does not press.
        player.pos.z -= 1.0;
        assert!(!pillar.touch(&mut player, 0.1));
        player.pos.z += 1.0;
        assert!(!pillar.touch(&mut player, 0.1));
        // After the cooldown, a new touch presses again.
        player.pos.z -= 1.0;
        assert!(!pillar.touch(&mut player, COOLDOWN));
        player.pos.z += 1.0;
        assert!(pillar.touch(&mut player, 0.1));
        assert_eq!(pillar.presses(), 2);
    }

    #[test]
    fn jumping_over_the_cap_does_not_press() {
        let mut pillar = Pillar::new();
        let mut player = PlayerController::new(AT.as_vec3(), 0.0);
        player.pos.y = HEIGHT as f32 + 0.2;
        assert!(!pillar.touch(&mut player, 0.1));
        assert_eq!(pillar.presses(), 0);
    }

    #[test]
    fn the_button_lights_only_when_armed() {
        let pillar = Pillar::new();
        let (mut armed, mut idle) = (Vec::new(), Vec::new());
        pillar.draw(true, &mut armed);
        pillar.draw(false, &mut idle);
        assert_eq!(armed.len(), idle.len());
        let button = parts()[2].vertices.len();
        let top = |v: &[LitVertex]| v[v.len() - button..].iter().all(|v| v.color == WHITE);
        assert!(top(&armed) && !top(&idle));
        assert!(armed.iter().all(|v| v.pos.iter().all(|x| x.is_finite())));
    }
}
