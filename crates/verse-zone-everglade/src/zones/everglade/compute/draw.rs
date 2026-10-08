//! Drawing the Pylon Field from its looks ([`super::look`]): generated
//! geometry, no pack models. Each pylon is a shaded crystal spire, obelisk,
//! or cairn with its light as additive glow; the Wellspring is a stone
//! basin whose pool glows, churns, and ripples; each provider's well is a
//! small stone cup; and a beam arcs from the basin to Alice's workstation
//! while her studio seat works. Everything is a closed-form function of
//! the states and the field's clock, so a replay draws the same frame.
//!
//! Faces are drawn in the zone's shaded face pass, lifted clear of the
//! ground and of each other so no two faces share a plane. Glow quads stay
//! well inside the medium tier's budget: about 12 for an idle field, and
//! at most 60 for the beam.

use glam::{Quat, Vec3};
use std::f32::consts::TAU;
use world_tree::State;

use super::look::{self, PylonLook, Shape};
use super::{Compute, Sample};
use crate::mesh::{Mesh, Vertex};
use crate::pbr::GlowVertex;
use coder_ui::theme::Intensity;

use super::super::layout::pylon_field::{self, BASIN, Field};
use super::super::spells::motes;

/// The light the field shines with: a cool spring-water white.
pub const LIGHT: [f32; 3] = [0.3, 0.78, 1.0];
/// A pylon's light at full glow, cd/m² before exposure; Reverse Gravity's
/// edge motes burn at 4, readable over sunlit grass.
const PYLON_LUMINANCE: f32 = 6.0;
/// The pool's light at full brightness.
const POOL_LUMINANCE: f32 = 4.0;
/// The beam's light.
const BEAM_LUMINANCE: f32 = 3.5;
/// How far from the field's middle the eye still sees it drawn, m: past
/// the zone's fog.
const DRAW_REACH: f32 = 190.0;
/// The beam's glow quads.
const BEAM_QUADS: usize = 52;
/// The rim's height over the ground and the pool's surface under it, m.
const RIM: f32 = 0.32;
const POOL: f32 = 0.2;

/// Stone colors, shaded like the zone's other generated faces.
const QUARTZ: [f32; 3] = [0.3, 0.36, 0.5];
const BASALT: [f32; 3] = [0.2, 0.19, 0.19];
const FIELDSTONE: [f32; 3] = [0.4, 0.38, 0.34];
const GREY: [f32; 3] = [0.3, 0.3, 0.3];
const RIM_STONE: [f32; 3] = [0.44, 0.42, 0.38];
const DARK_WATER: [f32; 3] = [0.03, 0.045, 0.06];
const GOLD: [f32; 3] = [0.95, 0.74, 0.3];
const MOSS: [f32; 3] = [0.12, 0.26, 0.06];

/// Where Alice's workstation is: her desk spot in the owner's great room,
/// at a standing chest's height.
#[must_use]
pub fn alice_station() -> Vec3 {
    let ([x, z], _) = super::super::layout::estate::AliceSpot::Desk.world();
    Vec3::new(x, super::super::height(x, z) + 1.4, z)
}

impl Compute {
    /// The field as drawn from `eye`: nothing when the layout has no field.
    /// `alice_working` says her studio seat is working now, which draws the
    /// beam to her workstation while the Wellspring has a state.
    #[must_use]
    pub fn mesh(&self, eye: Vec3, alice_working: bool) -> Mesh {
        let mut mesh = Mesh::default();
        let Some(field) = pylon_field::site() else {
            return mesh;
        };
        let beam = (alice_working && self.wellspring_state().is_some()).then(alice_station);
        draw(
            &mut mesh,
            field,
            &self.sample,
            self.pylon_states(),
            self.wellspring_state(),
            self.clock(),
            eye,
            beam,
        );
        mesh
    }
}

/// Appends the field to `mesh`: each pylon at its site, the basin, the
/// wells, the DEMO mark for a demo sample, and the beam to `beam` when
/// there is one.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    mesh: &mut Mesh,
    field: &Field,
    sample: &Sample,
    pylons: &[State],
    well: Option<&State>,
    time: f32,
    eye: Vec3,
    beam: Option<Vec3>,
) {
    let center = ground(field.center);
    if let Some(to) = beam {
        draw_beam(&mut mesh.glow, center + Vec3::Y * POOL, to, time, eye);
        // The fork to the pylon serving the job: this computer's.
        if let (Some(site), Some(state)) = (field.sites.first(), pylons.first())
            && let Some(look) = look::pylon(state)
            && look.glow > 0.0
        {
            let top = ground(*site) + Vec3::Y * look.height * 0.7;
            draw_arc(
                &mut mesh.glow,
                center + Vec3::Y * POOL,
                top,
                2.5,
                10,
                time,
                eye,
                0.7,
            );
        }
    }
    if eye.distance(center) > DRAW_REACH {
        return;
    }
    for (i, (site, state)) in field.sites.iter().zip(pylons).enumerate() {
        let Some(look) = look::pylon(state) else {
            continue;
        };
        let owner = sample.pylons.get(i).is_some_and(|p| p.owner);
        draw_pylon(mesh, ground(*site), &look, owner, time, i, eye);
    }
    draw_basin(mesh, center, &look::wellspring(well), time, eye);
    for (i, w) in sample.wells.iter().enumerate() {
        let at = ground(field.well(i));
        draw_well(mesh, at, w.capacity, time, i, eye);
    }
    if sample.demo {
        crate::doors::scene_label(
            mesh,
            "DEMO POOL",
            center + Vec3::Y * 2.8,
            0.45,
            Intensity::Full,
        );
    }
}

/// `[x, z]` on the ground.
fn ground([x, z]: [f32; 2]) -> Vec3 {
    Vec3::new(x, super::super::height(x, z), z)
}

/// Appends a shaded quad.
fn face(mesh: &mut Mesh, [a, b, c, d]: [Vec3; 4], color: [f32; 3]) {
    let color = super::super::draw::shade(color, a, b, c);
    for p in [a, b, c, a, c, d] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

/// A ring of `sides` points of radius `r` at height `y` over `base`,
/// turned by `turn` radians.
fn ring(base: Vec3, r: f32, y: f32, sides: usize, turn: f32) -> Vec<Vec3> {
    (0..sides)
        .map(|k| {
            let a = turn + TAU * k as f32 / sides as f32;
            base + Vec3::new(a.cos() * r, y, a.sin() * r)
        })
        .collect()
}

/// A tapered prism from radius `r0` at `y0` to `r1` at `y1`, its sides
/// facing out.
#[allow(clippy::too_many_arguments)]
fn prism(
    mesh: &mut Mesh,
    base: Vec3,
    sides: usize,
    r0: f32,
    r1: f32,
    y0: f32,
    y1: f32,
    turn: f32,
    color: [f32; 3],
) {
    let lo = ring(base, r0, y0, sides, turn);
    let hi = ring(base, r1, y1, sides, turn);
    for k in 0..sides {
        let n = (k + 1) % sides;
        face(mesh, [lo[k], hi[k], hi[n], lo[n]], color);
    }
}

/// A point over a ring: the crown of a spire or an obelisk.
fn point(
    mesh: &mut Mesh,
    base: Vec3,
    sides: usize,
    r: f32,
    y0: f32,
    tip: f32,
    turn: f32,
    color: [f32; 3],
) {
    let lo = ring(base, r, y0, sides, turn);
    let top = base + Vec3::Y * tip;
    for k in 0..sides {
        let n = (k + 1) % sides;
        face(mesh, [lo[k], top, top, lo[n]], color);
    }
}

/// A flat band round a pylon's base: `r` out, from `y0` to `y1`.
fn band(mesh: &mut Mesh, base: Vec3, r: f32, y0: f32, y1: f32, color: [f32; 3]) {
    prism(mesh, base, 12, r, r, y0, y1, 0.0, color);
}

/// Blends `color` toward the light by `k`.
fn lit(color: [f32; 3], k: f32) -> [f32; 3] {
    std::array::from_fn(|i| color[i] + (LIGHT[i] - color[i]) * 0.45 * k)
}

/// One pylon at `base`.
fn draw_pylon(
    mesh: &mut Mesh,
    base: Vec3,
    look: &PylonLook,
    owner: bool,
    time: f32,
    seed: usize,
    eye: Vec3,
) {
    let h = look.height;
    let turn = seed as f32 * 0.7;
    let width = match look.shape {
        Shape::Spire => {
            let color = if look.unknown {
                GREY
            } else {
                lit(QUARTZ, look.glow)
            };
            prism(mesh, base, 6, 0.32, 0.16, -0.1, h * 0.82, turn, color);
            point(mesh, base, 6, 0.16, h * 0.82, h, turn, color);
            0.32
        }
        Shape::Obelisk => {
            let color = if look.unknown { GREY } else { BASALT };
            prism(mesh, base, 4, 0.5, 0.32, -0.1, h * 0.86, turn, color);
            point(mesh, base, 4, 0.32, h * 0.86, h, turn, color);
            0.5
        }
        Shape::Cairn => {
            let color = if look.unknown { GREY } else { FIELDSTONE };
            let stones = 4;
            for k in 0..stones {
                let f = k as f32 / stones as f32;
                let size = 0.55 * (1.0 - 0.55 * f);
                let at = base + Vec3::Y * (h * f + size * 0.45);
                let h4 = [0.3, 0.6, 0.5, 0.4].map(|v: f32| (v + 0.17 * k as f32) % 1.0);
                motes::pebble(
                    mesh,
                    at,
                    size,
                    Quat::from_rotation_y(turn + k as f32),
                    color,
                    h4,
                );
            }
            0.55
        }
    };
    // Moss up the base, from uptime.
    if look.moss > 0.0 {
        band(mesh, base, width + 0.03, 0.0, 0.08 + 0.45 * look.moss, MOSS);
    }
    // A band of light per tenfold step in jobs served.
    let gold = if look.unknown { GREY } else { GOLD };
    for b in 0..look.bands {
        let y = 0.15 + 0.14 * b as f32;
        band(mesh, base, width + 0.06, y, y + 0.05, gold);
    }
    if owner {
        crate::doors::scene_label(
            mesh,
            "OWNER",
            base + Vec3::Y * (h + 0.45),
            0.3,
            Intensity::ThreeQuarters,
        );
    }
    if look.unknown {
        // The block letters have no question mark; the panel shows one.
        crate::doors::scene_label(
            mesh,
            "UNKNOWN",
            base + Vec3::Y * (h + 0.9),
            0.3,
            Intensity::Full,
        );
        return;
    }
    if look.glow <= 0.0 {
        return;
    }
    // The light: breathing while it waits, burning steady while it works.
    let phase = seed as f32 * 1.9;
    let level = if look.burns {
        1.0 + 0.06 * (time * 7.0 + phase).sin()
    } else if look.breathes {
        0.55 + 0.45 * (time * 1.3 + phase).sin()
    } else {
        1.0
    };
    let radiance = LIGHT.map(|c| c * PYLON_LUMINANCE * look.glow * level);
    for (k, y) in [0.3_f32, 0.58, 0.86].into_iter().enumerate() {
        let half = width * (1.4 + 0.4 * k as f32) * (0.6 + 0.4 * look.glow);
        motes::blob(
            &mut mesh.glow,
            base + Vec3::Y * (h * y),
            half,
            radiance,
            eye,
        );
    }
    motes::blob(
        &mut mesh.glow,
        base + Vec3::Y * h,
        0.35 + 0.4 * look.glow,
        radiance,
        eye,
    );
}

/// The Wellspring's basin at `center`.
fn draw_basin(mesh: &mut Mesh, center: Vec3, look: &look::WellLook, time: f32, eye: Vec3) {
    const SIDES: usize = 20;
    let inner = BASIN - 0.3;
    // The rim: its outer wall, top, and inner wall down to the pool.
    prism(
        mesh,
        center,
        SIDES,
        BASIN,
        BASIN - 0.05,
        -0.1,
        RIM,
        0.0,
        RIM_STONE,
    );
    let top_out = ring(center, BASIN - 0.05, RIM, SIDES, 0.0);
    let top_in = ring(center, inner, RIM, SIDES, 0.0);
    let low_in = ring(center, inner, POOL - 0.02, SIDES, 0.0);
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        face(
            mesh,
            [top_out[k], top_in[k], top_in[n], top_out[n]],
            RIM_STONE,
        );
        face(
            mesh,
            [top_in[k], low_in[k], low_in[n], top_in[n]],
            RIM_STONE,
        );
    }
    // The pool's surface, dark under its light.
    let surface = ring(center, inner, POOL, SIDES, 0.0);
    let middle = center + Vec3::Y * POOL;
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        face(mesh, [middle, surface[n], surface[k], middle], DARK_WATER);
    }
    if look.rim {
        for p in ring(center, BASIN - 0.15, RIM + 0.05, 12, 0.0) {
            motes::blob(&mut mesh.glow, p, 0.18, LIGHT.map(|c| c * 2.0), eye);
        }
    }
    if look.brightness <= 0.0 {
        return;
    }
    let lift = middle + Vec3::Y * 0.02;
    let glow = LIGHT.map(|c| c * POOL_LUMINANCE * look.brightness);
    motes::quad(&mut mesh.glow, lift, Vec3::X * inner, Vec3::Z * inner, glow);
    // A soft column of light over the pool.
    motes::blob(
        &mut mesh.glow,
        middle + Vec3::Y * 0.9,
        1.1 * look.brightness + 0.3,
        glow.map(|c| c * 0.35),
        eye,
    );
    // The surface churns with busy slots: swirling glints.
    let swirls = (look.churn * 10.0).round() as usize;
    for k in 0..swirls {
        let a = TAU * k as f32 / swirls.max(1) as f32 + time * (0.8 + 0.15 * k as f32);
        let r = inner * (0.25 + 0.6 * ((k as f32 * 0.37 + time * 0.2).fract()));
        let at = lift + Vec3::new(a.cos() * r, 0.01, a.sin() * r);
        motes::quad(
            &mut mesh.glow,
            at,
            Vec3::X * 0.22,
            Vec3::Z * 0.22,
            LIGHT.map(|c| c * 3.0),
        );
    }
    // A ripple runs outward for each job, at the pool's rate.
    if look.ripples > 0.0 {
        const LIFE: f32 = 1.6;
        const RINGS: usize = 4;
        let born = (time * look.ripples).floor();
        for i in 0..RINGS {
            let birth = (born - i as f32) / look.ripples;
            let age = time - birth;
            if !(0.0..LIFE).contains(&age) {
                continue;
            }
            let r = 0.15 + (inner - 0.2) * age / LIFE;
            let fade = 1.0 - age / LIFE;
            for p in ring(lift + Vec3::Y * 0.01, r, 0.0, 10, i as f32) {
                motes::quad(
                    &mut mesh.glow,
                    p,
                    Vec3::X * 0.12,
                    Vec3::Z * 0.12,
                    LIGHT.map(|c| c * 2.5 * fade),
                );
            }
        }
    }
}

/// A provider's well: a small cup, lit while it has capacity.
fn draw_well(mesh: &mut Mesh, at: Vec3, capacity: bool, time: f32, seed: usize, eye: Vec3) {
    prism(mesh, at, 6, 0.36, 0.3, -0.1, 0.42, 0.0, RIM_STONE);
    let lip = ring(at, 0.3, 0.42, 6, 0.0);
    let middle = at + Vec3::Y * 0.36;
    for k in 0..6 {
        face(mesh, [middle, lip[(k + 1) % 6], lip[k], middle], DARK_WATER);
    }
    if capacity {
        let level = 0.8 + 0.2 * (time * 2.1 + seed as f32).sin();
        motes::blob(
            &mut mesh.glow,
            at + Vec3::Y * 0.6,
            0.3,
            LIGHT.map(|c| c * 2.5 * level),
            eye,
        );
    }
}

/// The beam from the basin at `from` to an agent's station at `to`.
fn draw_beam(out: &mut Vec<GlowVertex>, from: Vec3, to: Vec3, time: f32, eye: Vec3) {
    let peak = (from.distance(to) * 0.18).clamp(8.0, 40.0);
    draw_arc(out, from, to, peak, BEAM_QUADS, time, eye, 1.0);
}

/// `count` glows along an arc from `from` to `to` rising `peak` m at its
/// middle, with pulses running toward `to`.
#[allow(clippy::too_many_arguments)]
fn draw_arc(
    out: &mut Vec<GlowVertex>,
    from: Vec3,
    to: Vec3,
    peak: f32,
    count: usize,
    time: f32,
    eye: Vec3,
    scale: f32,
) {
    for k in 0..=count {
        let s = k as f32 / count as f32;
        let at = from.lerp(to, s) + Vec3::Y * (peak * 4.0 * s * (1.0 - s));
        let pulse = 0.65 + 0.35 * (s * 24.0 - time * 5.0).sin();
        let half = scale * (0.35 + 0.25 * (1.0 - (2.0 * s - 1.0).abs()));
        motes::blob(
            out,
            at,
            half,
            LIGHT.map(|c| c * BEAM_LUMINANCE * pulse),
            eye,
        );
    }
}
