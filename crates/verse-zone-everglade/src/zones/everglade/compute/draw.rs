//! Drawing the Pylon Field from its looks ([`super::look`]): generated
//! geometry, no pack models. Each pylon is a carved stone shaft on a
//! plinth, crowned by a crystal: a hexagonal spire for a unified-memory
//! machine, a square obelisk for a GPU machine, and a cairn for a CPU
//! machine. Rune bands carved up the shaft light one per busy slot, the
//! crystal glows with the pylon's light, and while it serves, a stream of
//! light runs along the ground into the Wellspring. The Wellspring is a
//! wide stone basin on a stepped plinth whose pool glows, churns, ripples,
//! sends up shafts of light and rising motes, and throws caustic light on
//! its inner wall and the standing stones. Its light is also a real lamp
//! ([`Compute::light`]), so at night it lights the stones and the ground.
//! A continuous ribbon of light flows from the basin to Alice's
//! workstation while her studio seat works or one of this computer's jobs
//! runs on a pylon, with a fork from the pylon serving it.
//!
//! Everything is a closed-form function of the states, the field's clock,
//! and the night, so a replay draws the same frame. Brightness follows the
//! data: a pylon that isn't online has no light, an unknown one is grey,
//! and nothing in the basin moves without capacity or jobs behind it.
//!
//! Faces are drawn in the zone's shaded face pass, lifted clear of the
//! ground and of each other so no two faces share a plane. Glows are
//! written most important first, since the renderer keeps only the first
//! glows up to the quality tier's budget, and the field spends at most
//! [`FIELD_QUADS`]: the crystals, lit runes, and the pool first, then the
//! beam and the streams, then the ripples, shafts, motes, and caustics.

use glam::{Quat, Vec3};
use std::f32::consts::{FRAC_PI_4, TAU};
use std::sync::OnceLock;
use world_tree::State;

use super::look::{self, PylonLook, Shape, WellLook};
use super::{Compute, Sample};
use crate::mesh::{Mesh, Vertex};
use crate::pbr::{GlowVertex, Lamp, MAX_LAMPS, Neon};
use coder_ui::theme::Intensity;

use super::super::layout::pylon_field::{self, BASIN, Field, STONES};
use super::super::spells::motes;

pub mod agora;
pub mod dial;

/// The light the field shines with: a cool spring-water blue.
pub const LIGHT: [f32; 3] = [0.3, 0.78, 1.0];
/// The light's white-hot core.
const CORE: [f32; 3] = [0.72, 0.94, 1.0];
/// A lit rune stroke's light, cd/m² before exposure.
const RUNE_LUMINANCE: f32 = 9.0;
/// A crystal's light at full glow.
const TIP_LUMINANCE: f32 = 6.0;
/// The pool's light at full brightness.
const POOL_LUMINANCE: f32 = 3.2;
/// A shaft of light's foot.
const SHAFT_LUMINANCE: f32 = 1.2;
/// The stream along the ground at full flow.
const STREAM_LUMINANCE: f32 = 4.5;
/// The beam's light.
const BEAM_LUMINANCE: f32 = 3.2;
/// Caustic light on the basin's wall and the stones.
const CAUSTIC_LUMINANCE: f32 = 1.5;
/// How far from the field's middle the eye still sees it drawn, m: past
/// the zone's fog.
const DRAW_REACH: f32 = 190.0;
/// How far from the field's middle the eye sees its fine detail, m: the
/// rune strokes, caustics, and motes.
const DETAIL_REACH: f32 = 55.0;
/// The most glow quads the field draws.
pub const FIELD_QUADS: usize = 320;
/// The beam's segments.
const BEAM_SEGMENTS: usize = 40;
/// The rim's height over the ground and the pool's surface under it, m.
const RIM: f32 = 0.46;
const POOL: f32 = 0.28;
/// The basin's sides.
const SIDES: usize = 24;
/// The basin's inner radius, m.
const INNER: f32 = BASIN - 0.38;
/// The basin's lamp: luminous intensity at full brightness, candela, and
/// its reach, m.
const BASIN_LAMP: (f32, f32) = (12_000.0, 11.0);
/// A serving pylon's crystal lamp.
const TIP_LAMP: (f32, f32) = (4_500.0, 6.0);
/// How near the player the field's lamps light, m.
const LAMP_REACH: f32 = 80.0;

/// Stone colors, shaded like the zone's other generated faces.
const BASALT: [f32; 3] = [0.17, 0.17, 0.19];
const PALE_STONE: [f32; 3] = [0.42, 0.41, 0.4];
const FIELDSTONE: [f32; 3] = [0.4, 0.38, 0.34];
const PLINTH: [f32; 3] = [0.34, 0.33, 0.31];
const CARVED: [f32; 3] = [0.06, 0.065, 0.075];
const GREY: [f32; 3] = [0.3, 0.3, 0.3];
const RIM_STONE: [f32; 3] = [0.46, 0.44, 0.4];
const STEP_STONE: [f32; 3] = [0.36, 0.35, 0.32];
const DARK_WATER: [f32; 3] = [0.02, 0.04, 0.055];
const GOLD: [f32; 3] = [0.95, 0.74, 0.3];
const MOSS: [f32; 3] = [0.12, 0.26, 0.06];
/// A crystal unlit, lit, and when its pylon is unknown.
const CRYSTAL: [f32; 3] = [0.2, 0.3, 0.38];
const CRYSTAL_LIT: [f32; 3] = [0.62, 0.86, 1.0];
const CRYSTAL_GREY: [f32; 3] = [0.36, 0.36, 0.38];

/// Where Alice's workstation is: her desk spot in the owner's great room,
/// at a standing chest's height.
#[must_use]
pub fn alice_station() -> Vec3 {
    let ([x, z], _) = super::super::layout::estate::AliceSpot::Desk.world();
    Vec3::new(x, super::super::height(x, z) + 1.4, z)
}

/// The glows the field draws, by importance.
#[derive(Default)]
struct Glows {
    /// The crystals, the lit runes, and the pool.
    key: Vec<GlowVertex>,
    /// The beam, its forks, and the streams.
    flow: Vec<GlowVertex>,
    /// Ripples, shafts, motes, and caustics.
    extra: Vec<GlowVertex>,
}

/// What [`draw`] draws besides the field's states.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// The field's clock, s.
    pub time: f32,
    pub eye: Vec3,
    /// Where the beam runs to, when it shows.
    pub beam: Option<Vec3>,
    /// The pylon sites the beam forks from: those serving its jobs.
    pub forks: &'a [usize],
    /// 0 in daylight to 1 at night.
    pub night: f32,
}

impl Compute {
    /// The field as drawn from `eye`: nothing when the layout has no field.
    /// The beam to Alice's workstation shows while the Wellspring has a
    /// state and `alice_working` says her studio seat works, or one of this
    /// computer's jobs runs on a pylon. `night` runs from 0 in daylight to
    /// 1 at night.
    #[must_use]
    pub fn mesh(&self, eye: Vec3, alice_working: bool, night: f32) -> Mesh {
        let mut mesh = Mesh::default();
        let Some(field) = pylon_field::site() else {
            return mesh;
        };
        let jobs = !self.sample.in_flight.is_empty();
        let beam =
            ((alice_working || jobs) && self.wellspring_state().is_some()).then(alice_station);
        draw(
            &mut mesh,
            field,
            &self.sample,
            self.pylon_states(),
            self.wellspring_state(),
            &Scene {
                time: self.clock(),
                eye,
                beam,
                forks: &forks(field, &self.sample, self.pylon_states(), alice_working),
                night,
            },
        );
        mesh
    }

    /// Lights `neon`'s free lamp slots with the field's real light while
    /// the player at `at` is near it: the basin's lamp, as bright as the
    /// pool, and a lamp in each serving pylon's crystal. By day the Sun
    /// drowns them; at night they light the stones and the ground.
    pub fn light(&self, neon: &mut Neon, at: Vec3) {
        let Some(field) = pylon_field::site() else {
            return;
        };
        let center = ground(field.center);
        if at.distance(center) > LAMP_REACH {
            return;
        }
        let well = look::wellspring(self.wellspring_state());
        let mut lamps = Vec::new();
        if well.brightness > 0.0 {
            let swell = 1.0 + 0.08 * well.churn * (self.clock() * 3.1).sin();
            lamps.push(Lamp {
                position: center + Vec3::Y * 1.3,
                color: LIGHT,
                intensity: BASIN_LAMP.0 * well.brightness * swell,
                range: BASIN_LAMP.1,
            });
        }
        for (site, state) in field.sites.iter().zip(self.pylon_states()) {
            if let Some(look) = look::pylon(state)
                && look.burns
            {
                lamps.push(Lamp {
                    position: ground(*site) + Vec3::Y * (look.height + 0.2),
                    color: LIGHT,
                    intensity: TIP_LAMP.0 * look.glow,
                    range: TIP_LAMP.1,
                });
            }
        }
        let mut lamps = lamps.into_iter().take(3);
        for slot in neon.lamps.iter_mut().take(MAX_LAMPS) {
            if slot.lit() {
                continue;
            }
            match lamps.next() {
                Some(lamp) => *slot = lamp,
                None => break,
            }
        }
    }
}

/// The sites the beam forks from: each pylon running one of this
/// computer's jobs, or, for Alice's own work, this computer's pylon while
/// it glows.
#[must_use]
pub fn forks(field: &Field, sample: &Sample, pylons: &[State], alice_working: bool) -> Vec<usize> {
    let mut out: Vec<usize> = (0..field.sites.len().min(sample.pylons.len()))
        .filter(|&i| sample.in_flight.contains(&sample.pylons[i].id))
        .collect();
    if out.is_empty()
        && alice_working
        && pylons
            .first()
            .and_then(look::pylon)
            .is_some_and(|look| look.glow > 0.0)
    {
        out.push(0);
    }
    out
}

/// Appends the field to `mesh`: each pylon at its site, the basin, the
/// wells, the DEMO mark for a demo sample, and the beam when there is one.
pub fn draw(
    mesh: &mut Mesh,
    field: &Field,
    sample: &Sample,
    pylons: &[State],
    well: Option<&State>,
    scene: &Scene<'_>,
) {
    let Scene {
        time,
        eye,
        beam,
        forks,
        night,
    } = *scene;
    let center = ground(field.center);
    let faces_from = mesh.faces.len();
    let well_look = look::wellspring(well);
    let mut glows = Glows::default();
    // Glows burn a little softer at night, when the eye opens up to them,
    // so the field stays readable.
    let soft = 1.0 - 0.45 * night.clamp(0.0, 1.0);
    if let Some(to) = beam {
        draw_beam(
            &mut glows.flow,
            center + Vec3::Y * (POOL + 0.15),
            to,
            time,
            eye,
            soft,
        );
        for &i in forks {
            let (Some(site), Some(state)) = (field.sites.get(i), pylons.get(i)) else {
                continue;
            };
            let Some(look) = look::pylon(state) else {
                continue;
            };
            let top = ground(*site) + Vec3::Y * crystal_mid(&look);
            let into = center + Vec3::Y * (POOL + 0.15);
            arc_ribbon(&mut glows.flow, top, into, 2.5, 16, time, eye, 0.55, soft);
        }
    }
    if eye.distance(center) <= DRAW_REACH {
        let detail = eye.distance(center) <= DETAIL_REACH;
        for (i, (site, state)) in field.sites.iter().zip(pylons).enumerate() {
            let Some(look) = look::pylon(state) else {
                continue;
            };
            let owner = sample.pylons.get(i).is_some_and(|p| p.owner);
            let sigil = sample.pylons.get(i).is_some_and(|p| p.sigil);
            let base = ground(*site);
            draw_pylon(
                mesh, &mut glows, base, &look, owner, time, i, eye, detail, soft,
            );
            if sigil && !look.unknown {
                draw_sigil(&mut glows, base, &look, time, eye, soft);
            }
            if let Some(coin) = sample.pylons.get(i).and_then(|p| p.coin)
                && !look.unknown
            {
                draw_coin(
                    mesh, &mut glows, base, &look, coin, time, i, eye, detail, soft,
                );
            }
            if look.stream > 0.0 {
                draw_stream(&mut glows.flow, base, center, look.stream, time, i, soft);
            }
        }
        draw_basin(
            mesh, &mut glows, center, &well_look, time, eye, detail, night, soft,
        );
        if detail && well_look.brightness > 0.0 {
            draw_stone_light(&mut glows.extra, field, &well_look, time, night, soft);
        }
        for (i, w) in sample.wells.iter().enumerate() {
            let at = ground(field.well(i));
            draw_well(mesh, &mut glows.extra, at, w.capacity, time, i, eye, soft);
        }
        if sample.demo {
            crate::doors::scene_label(
                mesh,
                "DEMO POOL",
                center + Vec3::Y * 3.4,
                0.45,
                Intensity::Full,
            );
        }
    }
    agora::draw(
        mesh,
        &mut glows.flow,
        field,
        sample,
        pylons,
        time,
        eye,
        soft,
    );
    dial::draw(&mut glows.flow, well, eye, soft);
    let mut out = glows.key;
    out.extend(glows.flow);
    out.extend(glows.extra);
    out.truncate(FIELD_QUADS * 6);
    mesh.glow.extend(out);
    // Generated faces keep their daytime shade, so at night the stone dims
    // here and the field's light reads against it.
    let dim = 1.0 - 0.55 * night.clamp(0.0, 1.0);
    for v in &mut mesh.faces[faces_from..] {
        v.color = times(v.color, dim);
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

/// Appends a shaded triangle.
fn tri(mesh: &mut Mesh, [a, b, c]: [Vec3; 3], color: [f32; 3]) {
    let color = super::super::draw::shade(color, a, b, c);
    for p in [a, b, c] {
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

/// A flat polygon closing a prism at height `y`.
fn cap(mesh: &mut Mesh, base: Vec3, sides: usize, r: f32, y: f32, turn: f32, color: [f32; 3]) {
    let rim = ring(base, r, y, sides, turn);
    let middle = base + Vec3::Y * y;
    for k in 0..sides {
        tri(mesh, [middle, rim[(k + 1) % sides], rim[k]], color);
    }
}

/// A crystal over `base`: a ring of `sides` points of radius `r` at `mid`,
/// drawn to a point below at `y0` and a point above at `top`, leaning by
/// `lean`.
#[allow(clippy::too_many_arguments)]
fn crystal(
    mesh: &mut Mesh,
    base: Vec3,
    sides: usize,
    r: f32,
    [y0, mid, top]: [f32; 3],
    turn: f32,
    lean: Quat,
    color: [f32; 3],
) {
    let at = |p: Vec3| base + lean * (p - base);
    let waist: Vec<Vec3> = ring(base, r, mid, sides, turn)
        .into_iter()
        .map(at)
        .collect();
    let low = at(base + Vec3::Y * y0);
    let high = at(base + Vec3::Y * top);
    for k in 0..sides {
        let n = (k + 1) % sides;
        // Alternate facets catch a little more light, as a cut stone does.
        let c = if k % 2 == 0 {
            color
        } else {
            color.map(|v| v * 0.86)
        };
        tri(mesh, [waist[k], high, waist[n]], c);
        tri(mesh, [low, waist[k], waist[n]], c.map(|v| v * 0.8));
    }
}

/// Blends `color` toward `to` by `k`.
fn mix(color: [f32; 3], to: [f32; 3], k: f32) -> [f32; 3] {
    let k = k.clamp(0.0, 1.0);
    std::array::from_fn(|i| color[i] + (to[i] - color[i]) * k)
}

/// Scales a color.
fn times(color: [f32; 3], k: f32) -> [f32; 3] {
    color.map(|c| c * k)
}

/// A glow quad with a radiance and a falloff coordinate per corner.
fn glow_quad(out: &mut Vec<GlowVertex>, p: [Vec3; 4], radiance: [[f32; 3]; 4], uv: [[f32; 2]; 4]) {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(GlowVertex {
            pos: p[i].to_array(),
            radiance: radiance[i],
            uv: uv[i],
        });
    }
}

/// A continuous ribbon through `points`, `sides[i]` half its width at
/// each point, bright along its middle and soft at its edges, with a
/// radiance per point.
fn ribbon(out: &mut Vec<GlowVertex>, points: &[Vec3], sides: &[Vec3], radiance: &[[f32; 3]]) {
    for i in 0..points.len().saturating_sub(1) {
        let (a, b) = (points[i], points[i + 1]);
        let (sa, sb) = (sides[i], sides[i + 1]);
        glow_quad(
            out,
            [a - sa, a + sa, b + sb, b - sb],
            [radiance[i], radiance[i], radiance[i + 1], radiance[i + 1]],
            [[0.0, -1.0], [0.0, 1.0], [0.0, 1.0], [0.0, -1.0]],
        );
    }
}

/// Half-width vectors that turn a ribbon through `points` toward `eye`.
fn facing(points: &[Vec3], eye: Vec3, half: impl Fn(usize, Vec3) -> f32) -> Vec<Vec3> {
    (0..points.len())
        .map(|i| {
            let along = points[(i + 1).min(points.len() - 1)] - points[i.saturating_sub(1)];
            let side = along.cross(eye - points[i]).normalize_or(Vec3::X);
            side * half(i, points[i])
        })
        .collect()
}

/// A bright packet of light at phase `p`, 0 to 1 a cycle.
fn packet(p: f32) -> f32 {
    let c = 0.5 + 0.5 * (TAU * p).cos();
    c * c * c
}

/// Where a pylon's crystal is widest, m over its base.
fn crystal_mid(look: &PylonLook) -> f32 {
    match look.shape {
        Shape::Obelisk => look.height * 0.8 + 0.42,
        Shape::Spire => look.height * 0.78 + 0.35,
        Shape::Cairn => look.height + 0.3,
    }
}

/// A pylon's sigil: four gold motes in a slowly turning diamond above its
/// point, drawn only while a trusted checker's verdicts pass it.
fn draw_sigil(glows: &mut Glows, base: Vec3, look: &PylonLook, time: f32, eye: Vec3, soft: f32) {
    let center = base + Vec3::Y * (look.height + 0.8);
    let turn = time * 0.6;
    for k in 0..4 {
        let a = turn + k as f32 * std::f32::consts::FRAC_PI_2;
        let lift = if k % 2 == 0 { 0.22 } else { 0.0 };
        let at = center + Vec3::new(a.cos() * 0.22, lift - 0.11, a.sin() * 0.22);
        motes::blob(
            &mut glows.key,
            at,
            0.08,
            times(GOLD, TIP_LUMINANCE * 0.6 * soft),
            eye,
        );
    }
}

/// Test sats' coin: pale silver, never gold.
const TEST_COIN: [f32; 3] = [0.72, 0.78, 0.86];

/// A pylon's coin-light: a coin of light rising off its point while a
/// receipt with a valid preimage paid it in the last `COIN_SECS`. Gold for
/// mainnet sats; pale and marked TEST for test sats.
#[allow(clippy::too_many_arguments)]
fn draw_coin(
    mesh: &mut Mesh,
    glows: &mut Glows,
    base: Vec3,
    look: &PylonLook,
    coin: super::Coin,
    time: f32,
    seed: usize,
    eye: Vec3,
    detail: bool,
    soft: f32,
) {
    let color = if coin.test { TEST_COIN } else { GOLD };
    let rise = (time * 0.5 + seed as f32 * 0.37).fract();
    let at = base + Vec3::Y * (look.height + 1.3 + 0.9 * rise);
    let fade = 1.0 - rise * rise;
    motes::blob(
        &mut glows.key,
        at,
        0.14,
        times(color, TIP_LUMINANCE * 0.9 * fade * soft),
        eye,
    );
    motes::blob(
        &mut glows.key,
        at,
        0.07,
        times(CORE, TIP_LUMINANCE * fade * soft),
        eye,
    );
    if coin.test && detail {
        crate::doors::scene_label(
            mesh,
            "TEST",
            base + Vec3::Y * (look.height + 2.5),
            0.22,
            Intensity::Full,
        );
    }
}

/// One pylon at `base`.
#[allow(clippy::too_many_arguments)]
fn draw_pylon(
    mesh: &mut Mesh,
    glows: &mut Glows,
    base: Vec3,
    look: &PylonLook,
    owner: bool,
    time: f32,
    seed: usize,
    eye: Vec3,
    detail: bool,
    soft: f32,
) {
    let h = look.height;
    let turn = seed as f32 * 0.7;
    let crystal_color = if look.unknown {
        CRYSTAL_GREY
    } else {
        mix(CRYSTAL, CRYSTAL_LIT, look.glow.max(look.standby))
    };
    let stone = |c: [f32; 3]| if look.unknown { GREY } else { c };
    // The shaft the runes are carved in: its sides, radii, heights, and
    // turn.
    let shaft = match look.shape {
        Shape::Obelisk => {
            prism(mesh, base, 4, 0.82, 0.76, -0.5, 0.24, turn, stone(PLINTH));
            cap(mesh, base, 4, 0.76, 0.24, turn, stone(PLINTH));
            prism(mesh, base, 4, 0.5, 0.34, 0.24, h * 0.8, turn, stone(BASALT));
            prism(
                mesh,
                base,
                4,
                0.41,
                0.39,
                h * 0.8,
                h * 0.8 + 0.1,
                turn,
                stone(PALE_STONE),
            );
            cap(mesh, base, 4, 0.39, h * 0.8 + 0.1, turn, stone(PALE_STONE));
            crystal(
                mesh,
                base,
                4,
                0.27,
                [h * 0.8 + 0.02, crystal_mid(look), h + 0.55],
                turn + FRAC_PI_4,
                Quat::IDENTITY,
                crystal_color,
            );
            Some((4, 0.5, 0.34, 0.24, h * 0.8))
        }
        Shape::Spire => {
            prism(mesh, base, 6, 0.54, 0.48, -0.5, 0.18, turn, stone(PLINTH));
            cap(mesh, base, 6, 0.48, 0.18, turn, stone(PLINTH));
            prism(
                mesh,
                base,
                6,
                0.32,
                0.18,
                0.18,
                h * 0.78,
                turn,
                stone(PALE_STONE),
            );
            prism(
                mesh,
                base,
                6,
                0.25,
                0.23,
                h * 0.78,
                h * 0.78 + 0.07,
                turn,
                stone(BASALT),
            );
            cap(mesh, base, 6, 0.23, h * 0.78 + 0.07, turn, stone(BASALT));
            crystal(
                mesh,
                base,
                6,
                0.19,
                [h * 0.78, crystal_mid(look), h + 0.5],
                turn,
                Quat::IDENTITY,
                crystal_color,
            );
            // Two shards leaning out of the collar.
            for (k, lean) in [0.45_f32, -0.5].into_iter().enumerate() {
                let axis = Vec3::new(
                    (turn + k as f32 * 2.4).cos(),
                    0.0,
                    (turn + k as f32 * 2.4).sin(),
                );
                let foot = base + Vec3::Y * (h * 0.78 + 0.02);
                crystal(
                    mesh,
                    foot,
                    4,
                    0.07,
                    [0.0, 0.16, 0.42],
                    turn,
                    Quat::from_axis_angle(axis, lean),
                    crystal_color,
                );
            }
            Some((6, 0.32, 0.18, 0.18, h * 0.78))
        }
        Shape::Cairn => {
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
                    stone(FIELDSTONE),
                    h4,
                );
            }
            crystal(
                mesh,
                base,
                5,
                0.13,
                [h + 0.05, crystal_mid(look), h + 0.6],
                turn,
                Quat::IDENTITY,
                crystal_color,
            );
            None
        }
    };
    let width = shaft.map_or(0.55, |(_, r0, ..)| r0);
    // Moss up the plinth, from uptime.
    if look.moss > 0.0 {
        let r = if shaft.is_some() {
            width + 0.33
        } else {
            width + 0.03
        };
        prism(mesh, base, 12, r, r, 0.0, 0.05 + 0.2 * look.moss, 0.0, MOSS);
    }
    // A gold band per tenfold step in jobs served, low on the shaft.
    let gold = if look.unknown { GREY } else { GOLD };
    for b in 0..look.bands {
        let (y, r) = match shaft {
            Some((_, r0, r1, y0, y1)) => {
                let y = y0 + 0.08 + 0.075 * b as f32;
                (y, r0 + (r1 - r0) * (y - y0) / (y1 - y0) + 0.012)
            }
            None => (0.15 + 0.1 * b as f32, width + 0.06),
        };
        prism(mesh, base, 12, r, r, y, y + 0.035, 0.0, gold);
    }
    // The rune bands: carved dark rings with glyphs on the faces toward
    // the eye, one band burning per busy slot.
    let phase = seed as f32 * 1.9;
    let breath = 0.6 + 0.4 * (time * 1.3 + phase).sin();
    if let Some((sides, r0, r1, y0, y1)) = shaft {
        let lo = y0 + 0.12 + 0.075 * look.bands as f32 + 0.1;
        let hi = y1 - 0.12;
        let step = (hi - lo) / look.runes.max(1) as f32;
        let tall = (step * 0.72).min(0.26);
        let radius = |y: f32| r0 + (r1 - r0) * (y - y0) / (y1 - y0);
        // A short shaft with many gold bands has no room left for runes.
        let room = if step >= 0.06 { look.runes } else { 0 };
        for b in 0..room {
            let y = lo + step * (b as f32 + 0.5);
            prism(
                mesh,
                base,
                sides,
                radius(y - tall / 2.0) + 0.006,
                radius(y + tall / 2.0) + 0.006,
                y - tall / 2.0,
                y + tall / 2.0,
                turn,
                CARVED,
            );
            let lit = b < look.lit_runes;
            let level = if lit {
                // Burning, with light running up the bands.
                0.75 + 0.25 * (time * 6.0 - b as f32 * 1.4 + phase).sin()
            } else {
                0.14 * look.standby * breath
            };
            if level <= 0.0 || look.unknown {
                continue;
            }
            let radiance = times(
                if lit { CORE } else { LIGHT },
                RUNE_LUMINANCE * level * soft,
            );
            if !detail {
                motes::blob(
                    &mut glows.key,
                    base + Vec3::Y * y,
                    radius(y) + 0.15,
                    times(radiance, 0.4),
                    eye,
                );
                continue;
            }
            let ring_at = ring(base, radius(y) + 0.014, y, sides, turn);
            for k in 0..sides {
                let (a, c) = (ring_at[k], ring_at[(k + 1) % sides]);
                let middle = (a + c) * 0.5;
                let normal =
                    Vec3::new(middle.x - base.x, 0.0, middle.z - base.z).normalize_or(Vec3::Z);
                if normal.dot((eye - middle).normalize_or(Vec3::Y)) < 0.1 {
                    continue;
                }
                let across = (c - a).normalize_or(Vec3::X);
                let size = (a.distance(c) * 0.28).min(tall * 0.42);
                let glyph = (seed * 3 + b as usize * 5 + k) % GLYPHS.len();
                for &([u0, v0], [u1, v1]) in GLYPHS[glyph] {
                    let p0 = middle + across * (u0 * size) + Vec3::Y * (v0 * size * 1.3);
                    let p1 = middle + across * (u1 * size) + Vec3::Y * (v1 * size * 1.3);
                    stroke(&mut glows.key, p0, p1, normal, 0.022, radiance);
                }
            }
        }
    } else if !look.unknown {
        // A cairn's runes: a light between its stones per busy slot.
        for b in 0..look.runes {
            let lit = b < look.lit_runes;
            let level = if lit {
                1.0
            } else {
                0.14 * look.standby * breath
            };
            if level > 0.0 {
                let a = turn + b as f32 * 1.7;
                let at = base + Vec3::new(a.cos() * 0.4, 0.25 + 0.2 * b as f32, a.sin() * 0.4);
                motes::blob(
                    &mut glows.key,
                    at,
                    0.12,
                    times(LIGHT, RUNE_LUMINANCE * 0.5 * level * soft),
                    eye,
                );
            }
        }
    }
    if owner {
        crate::doors::scene_label(
            mesh,
            "OWNER",
            base + Vec3::Y * (h + 1.0),
            0.3,
            Intensity::ThreeQuarters,
        );
    }
    if look.unknown {
        // The block letters have no question mark; the panel shows one.
        crate::doors::scene_label(
            mesh,
            "UNKNOWN",
            base + Vec3::Y * (h + 1.45),
            0.3,
            Intensity::Full,
        );
        return;
    }
    if look.glow <= 0.0 {
        return;
    }
    // The crystal's light: breathing while it waits, burning while it
    // works.
    let level = if look.burns {
        1.0 + 0.06 * (time * 7.0 + phase).sin()
    } else if look.breathes {
        breath
    } else {
        1.0
    };
    let mid = base + Vec3::Y * crystal_mid(look);
    let light = TIP_LUMINANCE * look.glow * level * soft;
    motes::blob(
        &mut glows.key,
        mid,
        0.22 + 0.3 * look.glow,
        times(LIGHT, light),
        eye,
    );
    motes::blob(
        &mut glows.key,
        mid,
        0.1 + 0.08 * look.glow,
        times(CORE, light * 1.4),
        eye,
    );
    if look.burns {
        // A thin flare rising off the crystal's point.
        let top = base + Vec3::Y * (h + 0.5);
        let points: Vec<Vec3> = (0..=4).map(|k| top + Vec3::Y * (0.35 * k as f32)).collect();
        let sides = facing(&points, eye, |k, _| 0.07 * (1.0 - 0.18 * k as f32));
        let radiance: Vec<[f32; 3]> = (0..=4)
            .map(|k| times(CORE, light * 0.6 * (1.0 - k as f32 / 4.0)))
            .collect();
        ribbon(&mut glows.flow, &points, &sides, &radiance);
    }
}

/// The rune glyphs: strokes from one point to another in a glyph's frame,
/// x across the face and y up, each within -1 to 1.
const GLYPHS: [&[([f32; 2], [f32; 2])]; 6] = [
    &[([0.0, -1.0], [0.0, 1.0]), ([0.0, 0.1], [0.7, 0.8])],
    &[([-0.6, -1.0], [0.6, 1.0]), ([0.6, -1.0], [-0.6, 1.0])],
    &[
        ([0.0, -1.0], [0.0, 1.0]),
        ([-0.6, 0.3], [0.6, 0.3]),
        ([-0.6, -0.4], [0.6, -0.4]),
    ],
    &[([-0.6, -1.0], [0.0, 1.0]), ([0.0, 1.0], [0.6, -1.0])],
    &[
        ([0.0, -1.0], [0.0, 1.0]),
        ([0.0, 0.4], [0.6, -0.2]),
        ([0.0, 0.4], [-0.6, -0.2]),
    ],
    &[([-0.5, 1.0], [0.5, 0.0]), ([0.5, 0.0], [-0.5, -1.0])],
];

/// One rune stroke from `a` to `b` on a face whose outward normal is
/// `normal`, `width` m wide.
fn stroke(
    out: &mut Vec<GlowVertex>,
    a: Vec3,
    b: Vec3,
    normal: Vec3,
    width: f32,
    radiance: [f32; 3],
) {
    let side = normal.cross(b - a).normalize_or(Vec3::Y) * (width * 0.5);
    glow_quad(
        out,
        [a - side, a + side, b + side, b - side],
        [radiance; 4],
        [[0.0, -0.6], [0.0, 0.6], [0.0, 0.6], [0.0, -0.6]],
    );
}

/// The stream of light along the ground from a serving pylon at `base`
/// into the basin at `center`, at `flow` of full.
fn draw_stream(
    out: &mut Vec<GlowVertex>,
    base: Vec3,
    center: Vec3,
    flow: f32,
    time: f32,
    seed: usize,
    soft: f32,
) {
    const SEGMENTS: usize = 14;
    let toward = Vec3::new(center.x - base.x, 0.0, center.z - base.z);
    let length = toward.length();
    let dir = toward.normalize_or(Vec3::Z);
    let from = 0.95;
    let to = (length - BASIN - 0.25).max(from + 0.5);
    let side = Vec3::Y.cross(dir).normalize_or(Vec3::X) * (0.12 + 0.08 * flow);
    let mut points = Vec::with_capacity(SEGMENTS + 1);
    let mut radiance = Vec::with_capacity(SEGMENTS + 1);
    for k in 0..=SEGMENTS {
        let s = k as f32 / SEGMENTS as f32;
        let d = from + (to - from) * s;
        let [x, z] = [base.x + dir.x * d, base.z + dir.z * d];
        points.push(Vec3::new(x, super::super::height(x, z) + 0.07, z));
        // Packets of light run toward the basin, faster with more flow.
        let p = (d / 1.8 - time * (0.8 + 1.2 * flow) + seed as f32 * 0.37).fract();
        let ends = (s * 6.0).min((1.0 - s) * 6.0).min(1.0);
        let level = (0.3 + 0.7 * packet(p)) * ends;
        radiance.push(times(LIGHT, STREAM_LUMINANCE * flow * level * soft));
    }
    ribbon(out, &points, &vec![side; points.len()], &radiance);
}

/// The Wellspring's basin at `center`.
#[allow(clippy::too_many_arguments)]
fn draw_basin(
    mesh: &mut Mesh,
    glows: &mut Glows,
    center: Vec3,
    look: &WellLook,
    time: f32,
    eye: Vec3,
    detail: bool,
    night: f32,
    soft: f32,
) {
    // The stepped plinth.
    prism(
        mesh,
        center,
        SIDES,
        BASIN + 0.5,
        BASIN + 0.44,
        -0.8,
        0.1,
        0.0,
        STEP_STONE,
    );
    let step_out = ring(center, BASIN + 0.44, 0.1, SIDES, 0.0);
    let step_in = ring(center, BASIN - 0.02, 0.1, SIDES, 0.0);
    // The rim: carved blocks, each a shade apart, its outer wall, top, and
    // inner wall down to the pool.
    let out_lo = ring(center, BASIN, 0.1, SIDES, 0.0);
    let out_hi = ring(center, BASIN - 0.05, RIM, SIDES, 0.0);
    let in_hi = ring(center, INNER, RIM, SIDES, 0.0);
    let in_lo = ring(center, INNER, POOL - 0.02, SIDES, 0.0);
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        let block = RIM_STONE.map(|c| c * (0.88 + 0.12 * ((k * 7 % 5) as f32 / 4.0)));
        face(
            mesh,
            [step_out[k], step_in[k], step_in[n], step_out[n]],
            STEP_STONE,
        );
        face(mesh, [out_lo[k], out_hi[k], out_hi[n], out_lo[n]], block);
        face(mesh, [out_hi[k], in_hi[k], in_hi[n], out_hi[n]], block);
        face(
            mesh,
            [in_hi[k], in_lo[k], in_lo[n], in_hi[n]],
            times(block, 0.8),
        );
    }
    // A carved channel round the rim's top, where its runes burn.
    let ch_out = ring(center, BASIN - 0.15, RIM + 0.004, SIDES, 0.0);
    let ch_in = ring(center, BASIN - 0.27, RIM + 0.004, SIDES, 0.0);
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        face(mesh, [ch_out[k], ch_in[k], ch_in[n], ch_out[n]], CARVED);
    }
    // The pool's surface, dark under its light.
    let surface = ring(center, INNER, POOL, SIDES, 0.0);
    let middle = center + Vec3::Y * POOL;
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        tri(mesh, [middle, surface[n], surface[k]], DARK_WATER);
    }
    // The rim's runes burn only for a recomputed pool aggregate.
    if look.rim {
        for k in 0..12 {
            let a = TAU * k as f32 / 12.0 + 0.13;
            let r = BASIN - 0.21;
            let at = center + Vec3::new(a.cos() * r, RIM + 0.012, a.sin() * r);
            let along = Vec3::new(-a.sin(), 0.0, a.cos());
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            let glyph = GLYPHS[k % GLYPHS.len()];
            for &([u0, v0], [u1, v1]) in glyph {
                let p0 = at + along * (u0 * 0.09) + out * (v0 * 0.045);
                let p1 = at + along * (u1 * 0.09) + out * (v1 * 0.045);
                stroke(
                    &mut glows.key,
                    p0,
                    p1,
                    Vec3::Y,
                    0.018,
                    times(CORE, RUNE_LUMINANCE * 0.6 * soft),
                );
            }
        }
    }
    if look.brightness <= 0.0 {
        return;
    }
    let lift = middle + Vec3::Y * 0.02;
    let glow = times(LIGHT, POOL_LUMINANCE * look.brightness * soft);
    motes::quad(&mut glows.key, lift, Vec3::X * INNER, Vec3::Z * INNER, glow);
    motes::quad(
        &mut glows.key,
        lift + Vec3::Y * 0.005,
        Vec3::X * INNER * 0.55,
        Vec3::Z * INNER * 0.55,
        times(CORE, POOL_LUMINANCE * 0.8 * look.brightness * soft),
    );
    // The surface churns with busy slots: glints swirling round.
    let swirls = (look.churn * 8.0).round() as usize;
    for k in 0..swirls {
        let a = TAU * k as f32 / swirls.max(1) as f32 + time * (0.8 + 0.15 * k as f32);
        let r = INNER * (0.2 + 0.65 * ((k as f32 * 0.37 + time * 0.2).fract()));
        let at = lift + Vec3::new(a.cos() * r, 0.012, a.sin() * r);
        let along = Vec3::new(-a.sin(), 0.0, a.cos());
        motes::quad(
            &mut glows.key,
            at,
            along * 0.32,
            Vec3::new(a.cos(), 0.0, a.sin()) * 0.07,
            times(CORE, 3.0 * soft),
        );
    }
    // A ripple runs outward for each job, at the pool's rate: a whole
    // ring of light.
    if look.ripples > 0.0 {
        const LIFE: f32 = 2.4;
        const RINGS: usize = 3;
        const SEGMENTS: usize = 16;
        let born = (time * look.ripples).floor();
        for i in 0..RINGS {
            let birth = (born - i as f32) / look.ripples;
            let age = time - birth;
            if !(0.0..LIFE).contains(&age) {
                continue;
            }
            let r = 0.2 + (INNER - 0.25) * age / LIFE;
            let fade = 1.0 - age / LIFE;
            let rad = times(CORE, 2.6 * fade * soft);
            let width = 0.06 + 0.05 * fade;
            let at = lift + Vec3::Y * 0.015;
            for k in 0..SEGMENTS {
                let (a0, a1) = (
                    TAU * k as f32 / SEGMENTS as f32,
                    TAU * (k + 1) as f32 / SEGMENTS as f32,
                );
                let d0 = Vec3::new(a0.cos(), 0.0, a0.sin());
                let d1 = Vec3::new(a1.cos(), 0.0, a1.sin());
                glow_quad(
                    &mut glows.extra,
                    [
                        at + d0 * (r - width),
                        at + d0 * (r + width),
                        at + d1 * (r + width),
                        at + d1 * (r - width),
                    ],
                    [rad; 4],
                    [[0.0, -1.0], [0.0, 1.0], [0.0, 1.0], [0.0, -1.0]],
                );
            }
        }
    }
    // Shafts of light rising off the pool, more with more capacity, and
    // stronger at night.
    for k in 0..look.shafts as usize {
        let a = k as f32 * 2.399 + 0.6;
        let r = INNER * (0.15 + 0.5 * ((k as f32 * 0.618).fract()));
        let foot = lift + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
        let sway = 0.25 * (time * 0.37 + k as f32).sin();
        let top = foot + Vec3::new(sway, 3.6 + 1.2 * ((k as f32 * 0.43).fract()), 0.15 * sway);
        let points = [foot, foot.lerp(top, 0.5), top];
        let half = 0.22 + 0.12 * ((k as f32 * 0.77).fract());
        let sides = facing(&points, eye, |i, _| half * (1.0 + 0.4 * i as f32));
        let shimmer = 0.75 + 0.25 * (time * 0.9 + k as f32 * 1.3).sin();
        let foot_light = SHAFT_LUMINANCE * look.brightness * shimmer * (0.6 + 0.8 * night) * soft;
        let radiance = [
            times(LIGHT, foot_light),
            times(LIGHT, foot_light * 0.45),
            [0.0; 3],
        ];
        ribbon(&mut glows.extra, &points, &sides, &radiance);
    }
    if !detail {
        return;
    }
    // Motes rising off the pool, one stream per busy share.
    for k in 0..look.motes as usize {
        let p = (time * (0.16 + 0.03 * (k % 3) as f32) + k as f32 * 0.618).fract();
        let a = k as f32 * 2.399 + time * 0.4;
        let r = INNER * (0.2 + 0.6 * ((k as f32 * 0.37).fract())) * (1.0 - 0.4 * p);
        let at = lift + Vec3::new(a.cos() * r, 0.15 + 3.4 * p, a.sin() * r);
        let fade = (p * 5.0).min((1.0 - p) * 3.0).min(1.0);
        motes::blob(
            &mut glows.extra,
            at,
            0.06,
            times(CORE, 5.0 * fade * soft),
            eye,
        );
    }
    // Caustic light dancing on the inner wall, from the pool's light.
    for k in 0..SIDES {
        let n = (k + 1) % SIDES;
        let caustic = |i: usize| {
            let a = TAU * i as f32 / SIDES as f32;
            let w = ((a * 5.0 + time * 1.9).sin() * (a * 3.0 - time * 1.3 + 1.0).sin()).max(0.0);
            times(
                CORE,
                CAUSTIC_LUMINANCE * look.brightness * (0.25 + 1.5 * w) * soft,
            )
        };
        let inward = |p: Vec3| p + (center - p).with_y(0.0).normalize_or(Vec3::Z) * 0.012;
        glow_quad(
            &mut glows.extra,
            [
                inward(in_lo[k]),
                inward(in_lo[n]),
                inward(in_hi[n]),
                inward(in_hi[k]),
            ],
            [caustic(k), caustic(n), [0.0; 3], [0.0; 3]],
            [[0.0, 0.0], [0.0, 0.0], [0.0, 0.9], [0.0, 0.9]],
        );
    }
}

/// The standing stones round the field: where each stands and its scale.
fn stones(field: &Field) -> &'static [(Vec3, f32)] {
    static STONES_FOUND: OnceLock<Vec<(Vec3, f32)>> = OnceLock::new();
    STONES_FOUND.get_or_init(|| {
        super::super::layout::placements()
            .into_iter()
            .filter(|p| {
                p.model.starts_with("foliage/standing_stone")
                    && (p.at[0] - field.center[0]).hypot(p.at[1] - field.center[1]) < STONES + 1.0
            })
            .map(|p| (ground(p.at), p.scale))
            .collect()
    })
}

/// Caustic light from the pool dancing on each standing stone's face
/// toward it, stronger at night.
fn draw_stone_light(
    out: &mut Vec<GlowVertex>,
    field: &Field,
    look: &WellLook,
    time: f32,
    night: f32,
    soft: f32,
) {
    let center = ground(field.center);
    for (i, &(at, scale)) in stones(field).iter().enumerate() {
        let toward = Vec3::new(center.x - at.x, 0.0, center.z - at.z).normalize_or(Vec3::Z);
        let across = Vec3::Y.cross(toward).normalize_or(Vec3::X);
        let face = at + toward * (0.62 * scale) + Vec3::Y * (0.75 * scale);
        for k in 0..2 {
            let seed = i as f32 * 1.7 + k as f32 * 2.9;
            let dx = 0.22 * scale * (time * 0.7 + seed).sin();
            let dy = 0.3 * scale * (time * 0.53 + seed * 1.3).cos();
            let level = 0.5 + 0.5 * (time * 2.1 + seed).sin();
            motes::quad(
                out,
                face + across * dx + Vec3::Y * dy,
                across * (0.42 * scale),
                Vec3::Y * (0.55 * scale),
                times(
                    LIGHT,
                    CAUSTIC_LUMINANCE * look.brightness * level * (0.3 + 0.9 * night) * soft,
                ),
            );
        }
    }
}

/// A provider's well: a small cup, lit while it has capacity.
#[allow(clippy::too_many_arguments)]
fn draw_well(
    mesh: &mut Mesh,
    out: &mut Vec<GlowVertex>,
    at: Vec3,
    capacity: bool,
    time: f32,
    seed: usize,
    eye: Vec3,
    soft: f32,
) {
    prism(mesh, at, 6, 0.36, 0.3, -0.4, 0.42, 0.0, RIM_STONE);
    let lip = ring(at, 0.3, 0.42, 6, 0.0);
    let middle = at + Vec3::Y * 0.36;
    for k in 0..6 {
        tri(mesh, [middle, lip[(k + 1) % 6], lip[k]], DARK_WATER);
    }
    if capacity {
        let level = 0.8 + 0.2 * (time * 2.1 + seed as f32).sin();
        motes::blob(
            out,
            at + Vec3::Y * 0.5,
            0.22,
            times(LIGHT, 2.5 * level * soft),
            eye,
        );
    }
}

/// The beam from the basin at `from` to an agent's station at `to`.
fn draw_beam(out: &mut Vec<GlowVertex>, from: Vec3, to: Vec3, time: f32, eye: Vec3, soft: f32) {
    let peak = (from.distance(to) * 0.18).clamp(8.0, 40.0);
    arc_ribbon(out, from, to, peak, BEAM_SEGMENTS, time, eye, 1.0, soft);
}

/// A continuous ribbon of light along an arc from `from` to `to` rising
/// `peak` m at its middle, `segments` long, its width `scale` of the
/// beam's, with packets of light flowing toward `to`. It widens with
/// distance from the eye so it reads from afar.
#[allow(clippy::too_many_arguments)]
fn arc_ribbon(
    out: &mut Vec<GlowVertex>,
    from: Vec3,
    to: Vec3,
    peak: f32,
    segments: usize,
    time: f32,
    eye: Vec3,
    scale: f32,
    soft: f32,
) {
    let points: Vec<Vec3> = (0..=segments)
        .map(|k| {
            let s = k as f32 / segments as f32;
            from.lerp(to, s) + Vec3::Y * (peak * 4.0 * s * (1.0 - s))
        })
        .collect();
    let length = from.distance(to) + peak;
    let ends = |k: usize| {
        let s = k as f32 / segments as f32;
        (s * 10.0).min((1.0 - s) * 10.0).clamp(0.25, 1.0)
    };
    let wide = |_: usize, p: Vec3| scale * (0.012 * eye.distance(p)).clamp(0.14, 0.8);
    let halo_sides = facing(&points, eye, wide);
    let core_sides = facing(&points, eye, |k, p| wide(k, p) * 0.32);
    let flow: Vec<f32> = (0..=segments)
        .map(|k| {
            let s = k as f32 / segments as f32;
            let p = (s * length / 7.0 - time * 1.6).fract();
            (0.45 + 0.55 * packet(p)) * ends(k)
        })
        .collect();
    let halo: Vec<[f32; 3]> = flow
        .iter()
        .map(|f| times(LIGHT, BEAM_LUMINANCE * f * soft))
        .collect();
    let core: Vec<[f32; 3]> = flow
        .iter()
        .map(|f| times(CORE, BEAM_LUMINANCE * 1.6 * f * soft))
        .collect();
    ribbon(out, &points, &core_sides, &core);
    ribbon(out, &points, &halo_sides, &halo);
}
