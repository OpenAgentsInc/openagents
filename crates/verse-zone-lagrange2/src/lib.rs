//! Lagrange 2: a spaceship factory on a halo orbit about Earth–Moon L2,
//! where four arms assemble a ship from parts and the ship flies a test
//! burn away from L2. Physics lives in `verse-lagrange2` (the factory, the
//! plan, the burn) and `verse-lagrange` (the orbit); this crate maps input,
//! runs the clocks, and draws the scene. `verse` re-exports it as
//! `zones::lagrange2`.
//!
//! Scene axes are the factory frame: +X along the cradle (the ship's thrust
//! axis, away from the Moon), +Y the deck normal. The visitor flies free as
//! a camera drone; nothing they do moves the factory.
//!
//! Two clocks, as in Lagrange 1: the factory runs at [`Lagrange2::speed`]
//! times real time (8× by default, so the 67-minute assembly takes about
//! eight minutes), and the orbit runs [`ORBIT_CLOCK`] times faster than the
//! factory, so the daily station-keeping burns come every few minutes.

mod light;
mod scene;

/// The factory's physics, for hosts and captures that drive the zone.
pub use verse_lagrange2;

// The paths this zone was written against inside `crates/verse`.
use verse_core::world;
use verse_pbr::{mesh, pbr};
use verse_world::social::controller;

use std::sync::OnceLock;

use glam::{DQuat, DVec3, Mat4, Quat, Vec3};
use verse_lagrange::earth_moon::{self, HaloOrbit};
use verse_lagrange2::{
    Assembly, Status,
    burn::Burn,
    factory::{CRADLE, DT, Holder},
};

use crate::{
    controller::{InputState, PlayerController, TURN_SPEED},
    mesh::Mesh,
    pbr::{Camera, GlowVertex, LitVertex, bake},
    world::World,
};

/// The return portal, on the deck's aft-port corner.
pub const RETURN_PORTAL: Vec3 = Vec3::new(-30.0, 2.0, 44.0);
/// Where a visitor arrives: in front of the return portal, looking across
/// the deck at the cradle.
pub const SPAWN: Vec3 = Vec3::new(-26.0, 4.0, 36.0);
/// The visitor's flying speed, m/s, and with Shift.
const FLY: f32 = 6.0;
const FLY_FAST: f32 = 18.0;
/// The orbit runs this many times faster than the factory.
pub const ORBIT_CLOCK: f64 = 60.0;
/// Factory seconds between a finished assembly and the test burn.
const BURN_COUNTDOWN: f64 = 40.0;
/// The test burn's velocity change, m/s.
pub const TEST_BURN: f64 = 150.0;
/// Where the orbit display floats, and its scale: meters per kilometer.
const DISPLAY_AT: Vec3 = Vec3::new(-6.0, 34.0, -30.0);
const DISPLAY_SCALE: f32 = 0.000_35;
/// Camera pitch that flies level; tilting beyond the band climbs or dives.
const LEVEL_PITCH: f32 = 0.28;
const LEVEL_BAND: f32 = 0.12;

/// The test burn and the ship it flies.
pub struct TestBurn {
    pub burn: Burn,
    /// The departure in the Earth–Moon rotating frame, km from L2, for the
    /// orbit display.
    pub path: Vec<DVec3>,
}

pub struct Lagrange2 {
    pub assembly: Assembly,
    pub orbit: HaloOrbit,
    pub test: Option<TestBurn>,
    /// Factory time multiplier.
    pub speed: f64,
    pub paused: bool,
    /// Draw arm reach lines and the CMG momentum.
    pub overlay: bool,
    pub art: bool,
    /// Guides in the neutral palette (entered from the Grid).
    pub neutral: bool,
    countdown: f64,
    /// Burn time when the engine stopped, s.
    burn_end: f64,
    /// Turns frame time into whole factory steps
    /// (`verse_lagrange2::assembly::frame_clock`).
    pub clock: verse_lagrange2::physics::FixedStep,
    rendered: Mesh,
    light: light::Light,
    halo: Vec<DVec3>,
}

impl Default for Lagrange2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Lagrange2 {
    pub fn new() -> Self {
        let mut assembly = Assembly::sample();
        // A lighter prediction pace keeps a frame's work bounded on the web.
        assembly.predict_rate = 4;
        assembly.approve();
        let mut zone = Self {
            assembly,
            orbit: HaloOrbit::new(),
            test: None,
            speed: 8.0,
            paused: false,
            overlay: false,
            art: false,
            neutral: false,
            countdown: BURN_COUNTDOWN,
            burn_end: 0.0,
            clock: verse_lagrange2::assembly::frame_clock(),
            rendered: Mesh::default(),
            light: light::Light::new(structure()),
            halo: light::halo_loop(160),
        };
        zone.tick();
        zone
    }

    #[must_use]
    pub fn spawn() -> Vec3 {
        SPAWN
    }

    /// Facing the cradle's center from the spawn.
    #[must_use]
    pub fn spawn_yaw() -> f32 {
        let d = Vec3::new(0.0, 0.0, 0.0) - SPAWN;
        d.x.atan2(d.z)
    }

    /// Fly the visitor and advance the factory by `dt` real seconds.
    pub fn move_player(
        &mut self,
        player: &mut PlayerController,
        input: &InputState,
        camera_pitch: f32,
        dt: f32,
    ) {
        if !input.mouse_look {
            if input.left {
                player.yaw = crate::controller::wrap(player.yaw + TURN_SPEED * dt);
            }
            if input.right {
                player.yaw = crate::controller::wrap(player.yaw - TURN_SPEED * dt);
            }
        }
        let strafe_left = input.strafe_left || (input.mouse_look && input.left);
        let strafe_right = input.strafe_right || (input.mouse_look && input.right);
        let axis = |plus: bool, minus: bool| f32::from(u8::from(plus)) - f32::from(u8::from(minus));
        let ahead = axis(input.forward, input.backward);
        let side = axis(strafe_right, strafe_left);
        let tilt = camera_pitch - LEVEL_PITCH;
        let elevation = if tilt.abs() > LEVEL_BAND {
            -(tilt - LEVEL_BAND * tilt.signum())
        } else {
            0.0
        };
        let forward =
            crate::controller::forward(player.yaw) * elevation.cos() + Vec3::Y * elevation.sin();
        let right = Vec3::new(-player.yaw.cos(), 0.0, player.yaw.sin());
        let mut direction = forward * ahead + right * side;
        if input.jump {
            direction += Vec3::Y;
        }
        let speed = if input.sprint { FLY_FAST } else { FLY };
        let step = direction.normalize_or_zero() * speed * dt;
        player.pos =
            (player.pos + step).clamp(Vec3::new(-400.0, -60.0, -400.0), Vec3::splat(400.0));
        player.speed = step.length() / dt.max(1e-6);
        player.set_surface_height(player.pos.y);
        self.advance(f64::from(dt));
    }

    /// Advance the factory, the orbit, and the burn by `dt` real seconds.
    pub fn advance(&mut self, dt: f64) {
        if self.paused || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        for _ in 0..self.clock.advance(dt * self.speed) {
            self.step();
        }
    }

    /// One factory step.
    pub fn step(&mut self) {
        self.orbit.advance(DT * ORBIT_CLOCK);
        if let Some(test) = &mut self.test {
            test.burn.step();
            // Hand the ship's velocity to the orbit display once the engine
            // stops, and fly the ship on the halo's clock from there.
            if !test.burn.firing && test.path.is_empty() {
                test.path = departure(&self.orbit, test.burn.vel);
            }
            if test.burn.firing {
                self.burn_end = test.burn.time;
            }
            // A minute after the engine stops, the racks refill for the next
            // ship.
            if !test.burn.firing && test.burn.time > self.burn_end + 60.0 {
                self.restart();
            }
            return;
        }
        self.assembly.step();
        if self.assembly.status == Status::Complete {
            self.countdown -= DT;
            if self.countdown <= 0.0 {
                self.test = Some(TestBurn {
                    burn: Burn::new(self.assembly.factory.ship.clone(), TEST_BURN),
                    path: Vec::new(),
                });
            }
        }
    }

    /// Start a new ship: the racks refill and the arms begin again.
    pub fn restart(&mut self) {
        let mut assembly = Assembly::sample();
        assembly.predict_rate = 4;
        assembly.approve();
        self.assembly = assembly;
        self.test = None;
        self.countdown = BURN_COUNTDOWN;
    }

    /// Rebuild what moves.
    pub fn tick(&mut self) {
        let sun = light::to_scene(self.orbit.sun_direction()).as_vec3();
        let latched = self
            .assembly
            .factory
            .parts
            .iter()
            .filter(|p| matches!(p.holder, Holder::Latched(_)))
            .count();
        let mut parts = Vec::new();
        self.parts_mesh(&mut parts);
        self.light.update(latched, sun, structure(), move || parts);
        self.rendered = self.build_dynamic();
    }

    /// Waits for the light bake so an offline capture sees bounce light.
    pub fn settle_light(&mut self) {
        self.light.wait();
        self.rendered = self.build_dynamic();
    }

    pub fn dynamic(&self) -> &Mesh {
        &self.rendered
    }

    /// The fixed factory.
    pub fn world() -> World {
        World {
            mesh: Mesh {
                lit: structure().to_vec(),
                ..Mesh::default()
            },
            blockers: Vec::new(),
        }
    }

    /// The same in the neutral palette (it has no colored guides).
    pub fn neutral_world() -> World {
        Self::world()
    }

    fn camera(&self) -> Camera {
        if self.art {
            Camera::art()
        } else {
            Camera::helmet()
        }
    }

    fn guide(&self, color: [f32; 3]) -> [f32; 3] {
        if self.neutral {
            [color[0].max(color[1]).max(color[2]); 3]
        } else {
            color
        }
    }

    /// Transform from a body pose in the world to the factory frame.
    fn frame(&self) -> impl Fn(DVec3, DQuat) -> Mat4 + '_ {
        let f = &self.assembly.factory;
        move |pos: DVec3, q: DQuat| {
            let p = f.to_factory(pos).as_vec3();
            let r = f.orientation_in_factory(q).as_quat();
            Mat4::from_rotation_translation(r, p)
        }
    }

    /// The ship's parts as they are now: in the factory, or flying the
    /// test burn.
    fn parts_mesh(&self, out: &mut Vec<LitVertex>) {
        let f = &self.assembly.factory;
        if let Some(test) = &self.test {
            let b = &test.burn;
            let q = b.orientation.as_quat();
            let origin = (CRADLE + b.pos).as_vec3();
            for part in &f.ship.parts {
                let at = origin + q * part.position.as_vec3();
                let t =
                    Mat4::from_rotation_translation(q * Quat::from_rotation_y(part.yaw as f32), at);
                scene::part_mesh(out, part, &t);
            }
            return;
        }
        let frame = self.frame();
        for p in &f.parts {
            let body = &f.world[p.body];
            let t = frame(body.pos, body.orientation);
            scene::part_mesh(out, &f.ship.parts[p.index], &t);
        }
    }

    fn build_dynamic(&self) -> Mesh {
        let mut mesh = Mesh::default();
        let f = &self.assembly.factory;
        let frame = self.frame();
        for arm in 0..f.arms.len() {
            scene::arm_mesh(&mut mesh.lit, f, arm, &frame);
        }
        self.parts_mesh(&mut mesh.lit);
        if let Some(test) = &self.test
            && test.burn.firing
        {
            plume(&mut mesh.glow, test);
        }
        self.orbit_display(&mut mesh);
        if self.overlay {
            self.forces(&mut mesh);
        }
        let turn = self.assembly.factory.world[self.assembly.factory.body].orientation;
        let eye = Vec3::new(0.0, 20.0, 0.0);
        let time = (self.assembly.factory.time()) as f32;
        mesh.sky = Some(self.light.sky(&self.orbit, eye, self.camera(), time, turn));
        mesh
    }

    /// The orbit display: the Moon, L2, the halo, where the factory is on
    /// it, the direction of the Earth, and after the test burn the ship's
    /// path away.
    fn orbit_display(&self, mesh: &mut Mesh) {
        let to = |km: DVec3| DISPLAY_AT + light::to_scene(km).as_vec3() * DISPLAY_SCALE;
        let halo = self.guide([0.35, 0.75, 1.0]);
        for pair in self.halo.windows(2) {
            scene::line(mesh, to(pair[0]), to(pair[1]), halo);
        }
        let l2 = earth_moon::l2();
        let km = earth_moon::DISTANCE / 1_000.0;
        let moon = DVec3::new(1.0 - l2.mu - l2.x, 0.0, 0.0) * km;
        let pole = light::to_scene(DVec3::Z).as_vec3();
        scene::circle(
            mesh,
            to(moon),
            pole,
            ((verse_lagrange::orbit::MOON_RADIUS / 1_000.0) as f32 * DISPLAY_SCALE).max(1.2),
            self.guide([0.85, 0.85, 0.8]),
        );
        // L2 itself, a small cross.
        let c = to(DVec3::ZERO);
        let tick = 0.6;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            scene::line(
                mesh,
                c - axis * tick,
                c + axis * tick,
                self.guide([0.8, 0.8, 0.8]),
            );
        }
        // Toward the Earth, which is off the display.
        let earth_dir = light::to_scene(DVec3::new(-1.0, 0.0, 0.0)).as_vec3();
        let start = to(moon) + earth_dir * 3.0;
        scene::line(
            mesh,
            start,
            start + earth_dir * 6.0,
            self.guide([0.3, 0.6, 1.0]),
        );
        // The factory now.
        let here = (self.orbit.state.pos - l2.point()) * km;
        scene::circle(mesh, to(here), pole, 0.8, self.guide([1.2, 0.9, 0.3]));
        if let Some(test) = &self.test {
            for pair in test.path.windows(2) {
                scene::line(mesh, to(pair[0]), to(pair[1]), self.guide([1.3, 0.55, 0.2]));
            }
        }
    }

    fn forces(&self, mesh: &mut Mesh) {
        let f = &self.assembly.factory;
        // The CMGs' stored momentum, 1 m per 2 kN m s, from the deck center.
        let h = f.cmg.h.as_vec3() / 2_000.0;
        scene::line(
            mesh,
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0) + h,
            self.guide([1.4, 0.3, 0.9]),
        );
        for arm in &f.arms {
            let base = arm.spec.base.as_vec3();
            scene::circle(mesh, base, Vec3::Y, 24.0, self.guide([0.25, 0.4, 0.6]));
        }
    }

    /// One-line status for the HUD.
    #[must_use]
    pub fn caption(&self) -> String {
        let s = self.orbit.snapshot();
        let orbit = format!(
            "L2 halo · day {:.1} · Moon {:.0} km · Earth {} · keeping {:.2} m/s in {} burns",
            s.mission_days,
            s.moon_distance_km,
            if s.earth_visible { "in view" } else { "hidden" },
            s.keeping_dv_m_s,
            s.burns,
        );
        let f = &self.assembly.factory;
        let factory = format!(
            "CMG {:.1} kN m s · attitude {:.3}° · ×{:.0}",
            f.cmg.h.length() / 1e3,
            f.attitude_error().to_degrees(),
            self.speed
        );
        let work = match &self.test {
            Some(test) if test.burn.firing => format!(
                "Test burn: {:.0} of {:.0} m/s · {:.1} t burned",
                test.burn.flown,
                TEST_BURN,
                (test.burn.initial_mass - test.burn.mass()) / 1e3
            ),
            Some(test) => format!(
                "Test burn done: {:.0} m/s, leaving L2 · {:.0} m out",
                test.burn.flown,
                test.burn.pos.length()
            ),
            None if self.assembly.status == Status::Complete => format!(
                "Ship assembled · test burn in {:.0} s",
                self.countdown.max(0.0)
            ),
            None => self.assembly.caption(),
        };
        format!("{orbit}\n{factory}\n{work}")
    }
}

/// The ship's path for five days after the burn, km from L2 in the
/// rotating frame.
fn departure(orbit: &HaloOrbit, dv: DVec3) -> Vec<DVec3> {
    let mu = earth_moon::mu();
    let l2 = earth_moon::l2().point();
    let mut state = orbit.state;
    // The cradle's axes are the rotating frame's turned by the factory's
    // held attitude.
    let rotating = light::attitude().transpose() * dv;
    state.vel += rotating / earth_moon::velocity_unit();
    let unit = earth_moon::time_unit();
    let steps = 150;
    let span = 5.0 * 86_400.0 / unit;
    (0..=steps)
        .map(|i| {
            let s = earth_moon::propagate(mu, &state, span * i as f64 / steps as f64);
            (s.pos - l2) * earth_moon::DISTANCE / 1_000.0
        })
        .collect()
}

/// The main engine's plume: a faint core and a wider haze behind the bell.
fn plume(out: &mut Vec<GlowVertex>, test: &TestBurn) {
    let b = &test.burn;
    let engine = b
        .ship
        .parts
        .iter()
        .find(|p| matches!(p.kind, verse_lagrange2::ship::PartKind::Engine { .. }));
    let Some(engine) = engine else {
        return;
    };
    let q = b.orientation.as_quat();
    let exit = (CRADLE + b.pos).as_vec3() + q * (engine.position.as_vec3() - Vec3::X * 3.0);
    let back = -(q * b.thrust_dir.as_vec3());
    let (u, v) = basis(back);
    // Many small, overlapping soft puffs, each three crossed quads, so the
    // plume reads as a smooth volume from any side.
    let bright = 9.0e3;
    let count = 48;
    for k in 0..count {
        let f = k as f32 / count as f32;
        let along = f * 22.0;
        let radius = 1.4 + along * 0.32;
        let fade = (1.0 - f).powi(2);
        let c = exit + back * along;
        let r = [bright * fade, bright * fade * 0.72, bright * fade * 0.45];
        glow_quad(out, c, u * radius, v * radius, r);
        glow_quad(out, c, u * radius, back * radius * 0.8, r);
        glow_quad(out, c, v * radius, back * radius * 0.8, r);
    }
}

fn glow_quad(out: &mut Vec<GlowVertex>, c: Vec3, u: Vec3, v: Vec3, radiance: [f32; 3]) {
    for (a, b) in [
        (-1.0, -1.0),
        (1.0, -1.0),
        (1.0, 1.0),
        (-1.0, -1.0),
        (1.0, 1.0),
        (-1.0, 1.0),
    ] {
        out.push(GlowVertex {
            pos: (c + u * a + v * b).to_array(),
            radiance,
            uv: [a, b],
        });
    }
}

fn basis(axis: Vec3) -> (Vec3, Vec3) {
    let a = axis.normalize();
    let helper = if a.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let u = a.cross(helper).normalize();
    (u, a.cross(u))
}

/// The fixed structure, built and occlusion-baked once.
fn structure() -> &'static [LitVertex] {
    static BAKED: OnceLock<Vec<LitVertex>> = OnceLock::new();
    BAKED.get_or_init(|| {
        let mut fixed = Vec::new();
        scene::structure(&mut fixed, &verse_lagrange2::Spacecraft::sample());
        let bvh = bake::Bvh::new(&fixed);
        bake::bake_occlusion(&mut fixed, &bvh, 3.0, 16);
        fixed
    })
}
