//! The agent market at the Agora (P4, `docs/compute/verse-compute.md`):
//! the compute counter and the agent-services wall on its forecourt, and
//! a settlement thread from the counter to each pylon a trusted broker's
//! job just ran on.
//!
//! Everything comes from the sample's [`Market`], which a source fills
//! from verified records only: NIP-MKT offerings for the wall, NIP-PYLON
//! receipts for the counter and the threads. With no market the counter
//! reads zero and the wall says nothing is offered. A thread is gold only
//! for a job paid in mainnet sats; test sats, and an agent order's job,
//! which the order pays for and which carries no payment of its own, draw
//! pale, and the counter marks test sats **TEST**.

use super::*;
use crate::zones::everglade::compute::{Market, Sample};
use crate::zones::everglade::layout::agora::AGORA;

/// How far from the counter the eye still sees the Agora's market, m.
const AGORA_REACH: f32 = 120.0;
/// The most services the wall lists.
pub const WALL_ROWS: usize = 6;
/// The counter's and the wall's stone.
const COUNTER_STONE: [f32; 3] = [0.44, 0.42, 0.38];
const WALL_STONE: [f32; 3] = [0.16, 0.15, 0.16];
/// A thread's light at full strength.
const THREAD_LUMINANCE: f32 = 2.6;

/// Where the compute counter stands: on the forecourt, before the stair
/// and east of the walk up it.
#[must_use]
pub fn counter() -> [f32; 2] {
    AGORA.world([-3.2, 3.3])
}

/// Where the services wall stands: on the forecourt's other side.
#[must_use]
pub fn wall() -> [f32; 2] {
    AGORA.world([3.4, 2.4])
}

/// The counter's lines, top first.
#[must_use]
pub fn counter_lines(market: &Market, pylons: &[State]) -> Vec<String> {
    let (mut online, mut free) = (0_u32, 0_u32);
    for state in pylons {
        if let State::Pylon {
            status: world_tree::PylonStatus::Online | world_tree::PylonStatus::Draining,
            busy,
            total,
            ..
        } = state
        {
            online += 1;
            free += total.saturating_sub(*busy);
        }
    }
    let mainnet = market.paid_msat.get("bitcoin").copied().unwrap_or(0);
    let test: u64 = market
        .paid_msat
        .iter()
        .filter(|(network, _)| network.as_str() != "bitcoin")
        .map(|(_, msat)| msat)
        .sum();
    let mut lines = vec![
        "COMPUTE COUNTER".to_string(),
        format!("{online} PYLONS  {free} SLOTS FREE"),
        format!("{} JOBS TODAY  {} SOLD", market.jobs, market.sales),
    ];
    if mainnet > 0 || test == 0 {
        lines.push(format!("{} SATS PAID", mainnet / 1_000));
    }
    if test > 0 {
        lines.push(format!("{} SATS PAID TEST", test / 1_000));
    }
    lines
}

/// The wall's lines, top first: one per service, at most [`WALL_ROWS`].
#[must_use]
pub fn wall_lines(market: &Market) -> Vec<String> {
    let mut lines = vec!["AGENT SERVICES".to_string()];
    if market.services.is_empty() {
        lines.push("NOTHING OFFERED YET".into());
    }
    for service in market.services.iter().take(WALL_ROWS) {
        let offer: String = service.offer.chars().take(14).collect();
        let price = service
            .price_msat
            .map_or_else(String::new, |msat| format!(" {} SATS", msat / 1_000));
        let test = if service.test { " TEST" } else { "" };
        lines.push(format!("{offer}{price}{test}"));
    }
    lines
}

/// A box of half-extents `half` (x, z) from `y0` to `y1` over `base`.
fn block(mesh: &mut Mesh, base: Vec3, half: [f32; 2], [y0, y1]: [f32; 2], color: [f32; 3]) {
    let corners = [
        [half[0], half[1]],
        [-half[0], half[1]],
        [-half[0], -half[1]],
        [half[0], -half[1]],
    ];
    let lo: Vec<Vec3> = corners
        .iter()
        .map(|[x, z]| base + Vec3::new(*x, y0, *z))
        .collect();
    let hi: Vec<Vec3> = corners
        .iter()
        .map(|[x, z]| base + Vec3::new(*x, y1, *z))
        .collect();
    for k in 0..4 {
        let n = (k + 1) % 4;
        face(mesh, [lo[k], hi[k], hi[n], lo[n]], color);
    }
    face(mesh, [hi[0], hi[3], hi[2], hi[1]], color);
}

/// Labels stacked down from `top`, the first one larger.
fn lines(mesh: &mut Mesh, text: &[String], top: Vec3) {
    let mut y = 0.0;
    for (i, line) in text.iter().enumerate() {
        let height = if i == 0 { 0.3 } else { 0.22 };
        y += height + 0.12;
        crate::doors::scene_label(mesh, line, top - Vec3::Y * y, height, Intensity::Full);
    }
}

/// The Agora's market from `eye`: the counter and the wall with their
/// lines, and a thread to each pylon a broker job just ran on. Threads go
/// to `flow`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    mesh: &mut Mesh,
    flow: &mut Vec<GlowVertex>,
    field: &Field,
    sample: &Sample,
    pylons: &[State],
    time: f32,
    eye: Vec3,
    soft: f32,
) {
    let at = ground(counter());
    if eye.distance(at) > AGORA_REACH {
        return;
    }
    // Faces whose front is -z read from the forecourt: the labels face -z.
    let front = Vec3::new(0.0, 0.0, -0.42);
    block(mesh, at, [1.5, 0.4], [-0.3, 1.05], COUNTER_STONE);
    block(mesh, at, [1.62, 0.5], [1.05, 1.15], COUNTER_STONE);
    lines(
        mesh,
        &counter_lines(&sample.market, pylons),
        at + front + Vec3::Y * 3.1,
    );
    let w = ground(wall());
    block(mesh, w, [2.7, 0.15], [-0.3, 3.7], WALL_STONE);
    lines(
        mesh,
        &wall_lines(&sample.market),
        w + Vec3::new(0.0, 3.55, -0.17),
    );
    if sample.demo {
        crate::doors::scene_label(
            mesh,
            "DEMO MARKET",
            w + Vec3::new(0.0, 4.0, -0.17),
            0.3,
            Intensity::Full,
        );
    }
    let from = at + Vec3::Y * 1.3;
    for thread in &sample.market.threads {
        let Some(i) = sample.pylons.iter().position(|p| p.id == thread.pylon) else {
            continue;
        };
        let (Some(site), Some(state)) = (field.sites.get(i), pylons.get(i)) else {
            continue;
        };
        let Some(look) = look::pylon(state) else {
            continue;
        };
        let to = ground(*site) + Vec3::Y * (look.height + 0.6);
        let color = if thread.mainnet { GOLD } else { TEST_COIN };
        let peak = (from.distance(to) * 0.45).clamp(14.0, 45.0);
        colored_arc(flow, from, to, peak, 32, time, eye, 0.45, soft, color);
    }
}

/// [`arc_ribbon`] in `color`, its packets flowing toward the pylon.
#[allow(clippy::too_many_arguments)]
fn colored_arc(
    out: &mut Vec<GlowVertex>,
    from: Vec3,
    to: Vec3,
    peak: f32,
    segments: usize,
    time: f32,
    eye: Vec3,
    scale: f32,
    soft: f32,
    color: [f32; 3],
) {
    let points: Vec<Vec3> = (0..=segments)
        .map(|k| {
            let s = k as f32 / segments as f32;
            from.lerp(to, s) + Vec3::Y * (peak * 4.0 * s * (1.0 - s))
        })
        .collect();
    let length = from.distance(to) + peak;
    let wide = |_: usize, p: Vec3| scale * (0.012 * eye.distance(p)).clamp(0.14, 0.8);
    let halo_sides = facing(&points, eye, wide);
    let core_sides = facing(&points, eye, |k, p| wide(k, p) * 0.32);
    let flow: Vec<f32> = (0..=segments)
        .map(|k| {
            let s = k as f32 / segments as f32;
            let ends = (s * 10.0).min((1.0 - s) * 10.0).clamp(0.25, 1.0);
            let p = (s * length / 7.0 - time * 1.2).fract();
            (0.45 + 0.55 * packet(p)) * ends
        })
        .collect();
    let halo: Vec<[f32; 3]> = flow
        .iter()
        .map(|f| times(color, THREAD_LUMINANCE * f * soft))
        .collect();
    let core: Vec<[f32; 3]> = flow
        .iter()
        .map(|f| times(mix(color, [1.0; 3], 0.6), THREAD_LUMINANCE * 1.5 * f * soft))
        .collect();
    ribbon(out, &points, &core_sides, &core);
    ribbon(out, &points, &halo_sides, &halo);
}

/// What a player at the counter or the wall reads up close, as caption
/// lines; `None` elsewhere.
#[must_use]
pub fn inspect(sample: &Sample, pylons: &[State], at: [f32; 2]) -> Option<String> {
    let near = |p: [f32; 2]| (p[0] - at[0]).hypot(p[1] - at[1]) <= 2.8;
    let demo = if sample.demo { "DEMO · " } else { "" };
    if near(counter()) {
        let lines = counter_lines(&sample.market, pylons);
        return Some(format!(
            "{demo}Agora compute counter\n{}",
            lines[1..].join("\n")
        ));
    }
    if near(wall()) {
        let mut out = format!("{demo}Agent services");
        if sample.market.services.is_empty() {
            out.push_str("\nNothing offered yet");
        }
        for s in sample.market.services.iter().take(WALL_ROWS) {
            let price = s
                .price_msat
                .map_or_else(String::new, |msat| format!(", {} sats", msat / 1_000));
            let test = if s.test { " (test sats)" } else { "" };
            out.push_str(&format!("\n{}: {}{price}{test}", s.offer, s.summary));
        }
        return Some(out);
    }
    None
}
