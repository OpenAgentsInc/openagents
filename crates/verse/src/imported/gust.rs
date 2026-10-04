//! Gust of Wind presentation: scene flames (candles and lanterns) and
//! their light, wind puffs streaming down each active Line, and the Line's
//! outline on the spell playground overlay.
use super::Instance;
use super::lighting::{Light, Lighting, MAX_LIGHTS};
use crate::ui::{Atlas, UiBatch};
use glam::{Mat4, Vec3};
use verse_world::gust::{HEIGHT, LENGTH, WIDTH, WIND_SPEED};
use verse_world::play::Game;
use verse_world::spells::gust_of_wind;

/// Wind puffs drawn along each Line.
const PUFFS: u32 = 48;

/// A box `size` meters across centered on `center`, drawn with a unit-cube
/// prop model.
fn block(model: &str, center: Vec3, size: Vec3, time: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(center)
            * Mat4::from_scale(size / 0.9144)
            * super::chamber::basis(),
        animation: 0.into(),
        time,
        emission: Vec3::ONE,
    }
}

fn particle(model: &str, center: Vec3, scale: f32, time: f32) -> Instance {
    Instance {
        mount: None,
        actor: None,
        model: model.into(),
        transform: Mat4::from_translation(center) * Mat4::from_scale(Vec3::splat(scale)),
        animation: 0.into(),
        time,
        emission: Vec3::ONE,
    }
}

/// A deterministic fraction in [0, 1) for puff `k` and salt `s`.
fn hash(k: u32, s: u32) -> f32 {
    let mut x = k.wrapping_mul(0x9E37_79B9) ^ s.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_536.0
}

/// Candles, lanterns, and wind puffs.
pub fn instances(game: &Game) -> Vec<Instance> {
    let state = &game.spells.gust;
    let time = game.time;
    let mut out = Vec::new();
    for flame in &state.flames {
        let p = flame.position.as_vec3();
        let floor = 0.0f32.min(p.y);
        let stem = (p.y - floor - 0.06).max(0.05);
        if flame.protected {
            out.push(block(
                "prop-anvil",
                Vec3::new(p.x, floor + stem * 0.5 - 0.08, p.z),
                Vec3::new(0.05, stem - 0.16, 0.05),
                time,
            ));
            out.push(block(
                "prop-anvil",
                Vec3::new(p.x, p.y - 0.1, p.z),
                Vec3::new(0.2, 0.05, 0.2),
                time,
            ));
            out.push(block(
                "prop-anvil",
                Vec3::new(p.x, p.y + 0.16, p.z),
                Vec3::new(0.22, 0.05, 0.22),
                time,
            ));
        } else {
            out.push(block(
                "prop-paper",
                Vec3::new(p.x, floor + stem * 0.5, p.z),
                Vec3::new(0.07, stem, 0.07),
                time,
            ));
        }
        if !flame.lit {
            continue;
        }
        let wild = gust_of_wind::flickers(state, time, flame);
        let phase = time * if wild { 37.0 } else { 9.0 } + flame.id as f32 * 1.7;
        let size = if wild {
            0.06 + 0.035 * phase.sin().abs()
        } else {
            0.07 + 0.006 * phase.sin()
        };
        let lean = state
            .lines
            .iter()
            .find(|l| wild && l.gust.line.contains(flame.position))
            .map_or(Vec3::ZERO, |l| {
                l.gust.line.direction.as_vec3() * (0.04 + 0.03 * phase.cos())
            });
        out.push(particle("effect-fire", p + lean, size, time));
    }
    for line in &state.lines {
        if !line.gust.active(f64::from(time)) {
            continue;
        }
        let l = line.gust.line;
        let (origin, ahead, side) = (
            l.origin.as_vec3(),
            l.direction.as_vec3(),
            l.side().as_vec3(),
        );
        let length = LENGTH as f32;
        for k in 0..PUFFS {
            let start = hash(k, 1) * length;
            let speed = WIND_SPEED as f32 * (0.55 + 0.45 * hash(k, 2));
            let along = (start + (time - line.gust.cast_at as f32) * speed).rem_euclid(length);
            let across = (hash(k, 3) - 0.5) * WIDTH as f32 * 0.9;
            let up = 0.15 + hash(k, 4) * (HEIGHT as f32 - 0.4);
            let fade = (along / 1.5).min((length - along) / 1.5).clamp(0.0, 1.0);
            out.push(particle(
                "effect-mist",
                origin + ahead * along + side * across + Vec3::Y * up,
                0.08 + 0.16 * fade,
                time,
            ));
        }
    }
    out
}

/// Light from every lit flame, flickering hard in the wind.
pub fn lights(game: &Game, lighting: &mut Lighting) {
    let state = &game.spells.gust;
    for flame in state.flames.iter().filter(|f| f.lit) {
        if lighting.lights.len() >= MAX_LIGHTS {
            break;
        }
        let wild = gust_of_wind::flickers(state, game.time, flame);
        let phase = game.time * if wild { 31.0 } else { 7.0 } + flame.id as f32 * 1.7;
        let flicker = if wild {
            0.45 + 0.55 * phase.sin().abs()
        } else {
            0.9 + 0.1 * phase.sin()
        };
        lighting.lights.push(Light {
            position: flame.position.as_vec3() + Vec3::Y * 0.08,
            color: Vec3::new(1.0, 0.62, 0.25),
            intensity: if flame.protected { 9.0 } else { 5.0 } * flicker,
            range: 3.5,
        });
    }
}

fn project(view_proj: Mat4, p: Vec3, width: f32, height: f32) -> Option<[f32; 2]> {
    let clip = view_proj * p.extend(1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some([(ndc.x + 1.0) * 0.5 * width, (1.0 - ndc.y) * 0.5 * height])
}

/// The outline of every active Line, 60 by 10 by 10 feet.
pub fn outline(
    ui: &mut UiBatch,
    atlas: &Atlas,
    game: &Game,
    view_proj: Mat4,
    width: f32,
    height: f32,
) {
    for line in &game.spells.gust.lines {
        if !line.gust.active(f64::from(game.time)) {
            continue;
        }
        let c = line.gust.line.corners().map(|p| p.as_vec3());
        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        for (a, b) in edges {
            if let (Some(a), Some(b)) = (
                project(view_proj, c[a], width, height),
                project(view_proj, c[b], width, height),
            ) {
                ui.line(atlas, a, b, 1.5, [0.55, 0.85, 1.0, 0.75]);
            }
        }
    }
}
