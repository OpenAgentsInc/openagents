//! The Hand Duel's screen: both health bars, the belt's potions, what each
//! hand is doing, the coming blow's name, floating numbers, and the
//! banner. Laid out in logical points on a screen of `size`; the host
//! scales the batch to pixels.

use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use verse_gfx::ui::{self, Atlas, UiBatch};

use crate::HandDuel;
use crate::body::Side;
use crate::gesture::{Potion, at_belt, at_mouth};
use crate::rules::{FLOATER_LIFE, Fight, Float, Outcome};

/// The gestures, one line each, in the corner.
pub const GESTURES: [&str; 5] = [
    "Raise your left hand: shield up",
    "Swing your right hand: strike",
    "Push the shield forward: bash",
    "Left hand to your hip, close it, then to your mouth: drink",
    "Right hand over your shoulder: open to sheathe, close to draw",
];

/// What to do before the camera is on.
pub const START: &str = "Press Hands, then hold up both hands";

fn white(alpha: f32) -> [f32; 4] {
    let [r, ..] = ui::amber(Intensity::Full, alpha);
    [r, r, r, alpha]
}

fn gray(alpha: f32) -> [f32; 4] {
    let [r, ..] = ui::amber(Intensity::Half, alpha);
    [r, r, r, alpha]
}

fn amber(alpha: f32) -> [f32; 4] {
    let [r, g, b] = crate::enemy_amber(1.0);
    [r, g, b, alpha]
}

const RED: [f32; 4] = [0.95, 0.2, 0.15, 1.0];
const HEAL: [f32; 4] = [0.45, 1.0, 0.55, 1.0];

/// Where world point `p` lands on a screen of `size`, or `None` behind the
/// eye or off screen.
#[must_use]
pub fn project(view_proj: Mat4, size: [f32; 2], p: Vec3) -> Option<[f32; 2]> {
    let clip = view_proj * p.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.01 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !(-1.2..=1.2).contains(&ndc.x) || !(-1.2..=1.2).contains(&ndc.y) {
        return None;
    }
    Some([(ndc.x * 0.5 + 0.5) * size[0], (0.5 - ndc.y * 0.5) * size[1]])
}

fn centered(
    ui: &mut UiBatch,
    atlas: &Atlas,
    center: [f32; 2],
    text: &str,
    color: [f32; 4],
    k: f32,
) {
    let width = atlas.measure(text);
    let start = ui.vertices.len();
    ui.text(
        atlas,
        center[0] - width * 0.5,
        center[1] - atlas.line * 0.5,
        text,
        color,
    );
    if (k - 1.0).abs() > 1e-3 {
        for v in &mut ui.vertices[start..] {
            v.pos = [
                center[0] + (v.pos[0] - center[0]) * k,
                center[1] + (v.pos[1] - center[1]) * k,
            ];
        }
    }
}

fn bar(ui: &mut UiBatch, atlas: &Atlas, rect: [f32; 4], fill: f32, color: [f32; 4]) {
    let [x, y, w, h] = rect;
    ui.rect(atlas, x - 1.0, y - 1.0, w + 2.0, h + 2.0, ui::field(0.85));
    ui.rect(atlas, x, y, w * fill.clamp(0.0, 1.0), h, color);
    ui.frame(atlas, x - 1.0, y - 1.0, w + 2.0, h + 2.0, 1.0, gray(0.9));
}

/// Draws the screen.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, duel: &HandDuel) {
    let fight = duel.fight();
    health(ui, atlas, size, fight);
    floaters(ui, atlas, size, view_proj, fight);
    belt(ui, atlas, size, duel);
    hands(ui, atlas, size, duel);
    legend(ui, atlas, size);
    if let Some((blow, p)) = fight.telegraph() {
        centered(
            ui,
            atlas,
            [size[0] * 0.5, size[1] * 0.17],
            blow.word(),
            amber(0.5 + 0.5 * p),
            1.0 + 0.5 * p,
        );
    }
    if !duel.tracking() {
        centered(
            ui,
            atlas,
            [size[0] * 0.5, size[1] * 0.45],
            START,
            white(1.0),
            1.5,
        );
    }
    banner(ui, atlas, size, duel);
}

fn health(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    let w = (size[0] * 0.28).clamp(160.0, 320.0);
    let hit = fight.now - fight.player.hit_at < 0.2;
    let healed = fight.now - fight.player.healed_at < 0.4;
    let color = if hit {
        RED
    } else if healed {
        HEAL
    } else {
        white(0.95)
    };
    ui.text(atlas, 24.0, 18.0, "You", white(0.9));
    bar(
        ui,
        atlas,
        [24.0, 22.0 + atlas.line, w, 10.0],
        fight.player.hp / fight.player.max_hp,
        color,
    );
    let text = format!("{:.0} / {:.0}", fight.player.hp, fight.player.max_hp);
    ui.text(atlas, 32.0 + w, 18.0 + atlas.line, &text, gray(0.9));

    let x = size[0] - 24.0 - w;
    // Below the page's Hands button in the top right corner.
    let top = 58.0;
    ui.text(atlas, x, top, "Barrow Warden", amber(0.95));
    let flash = fight.now - fight.warden.hit_at < 0.12;
    bar(
        ui,
        atlas,
        [x, top + 4.0 + atlas.line, w, 10.0],
        fight.warden.hp / fight.warden.max_hp,
        if flash { white(1.0) } else { amber(0.95) },
    );
}

fn floaters(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, fight: &Fight) {
    for f in &fight.floaters {
        let age = (fight.now - f.born) / FLOATER_LIFE;
        let Some([x, y]) = project(view_proj, size, f.at) else {
            continue;
        };
        let alpha = (1.0 - age).clamp(0.0, 1.0);
        let color = match f.kind {
            Float::Dealt => amber(alpha),
            Float::Taken => [RED[0], RED[1], RED[2], alpha],
            Float::Heal => [HEAL[0], HEAL[1], HEAL[2], alpha],
            Float::Word => white(alpha),
        };
        let k = if f.crit { 2.0 } else { 1.4 };
        let text = if f.crit {
            format!("{} crit", f.text)
        } else {
            f.text.clone()
        };
        centered(ui, atlas, [x, y - age * 60.0], &text, color, k);
    }
}

/// The belt's potions at the bottom left, lit when the left hand is near.
fn belt(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], duel: &HandDuel) {
    let g = duel.gestures();
    let near = duel.hand(Side::Left).is_some_and(|h| at_belt(h.pos));
    let holding = matches!(g.potion, Potion::Held { .. });
    let y = size[1] - 46.0;
    let in_belt = g.potions.saturating_sub(u32::from(holding));
    for i in 0..3u32 {
        let cx = 40.0 + i as f32 * 34.0;
        let full = i < in_belt;
        let color = if full { RED } else { ui::field(0.6) };
        ui.disc(atlas, cx, y, 12.0, color);
        ui.ring(
            atlas,
            cx,
            y,
            13.0,
            1.5,
            if near && full { white(1.0) } else { gray(0.8) },
        );
    }
    let label = if holding {
        let at = duel.hand(Side::Left).is_some_and(|h| at_mouth(h.pos));
        if at {
            "Drinking"
        } else {
            "Bring it to your mouth"
        }
    } else {
        "Potions"
    };
    ui.text(atlas, 26.0, y - 34.0, label, white(0.9));
    let p = g.drink_progress(duel.hand_time());
    if p > 0.0 {
        bar(ui, atlas, [26.0, y + 18.0, 96.0, 5.0], p, HEAL);
    }
}

/// What each hand is doing, at the bottom center.
fn hands(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], duel: &HandDuel) {
    if !duel.tracking() {
        return;
    }
    let g = duel.gestures();
    let left = match duel.hand(Side::Left) {
        None => "Left hand: not in view",
        Some(_) if matches!(g.potion, Potion::Held { .. }) => "Left hand: potion",
        Some(_) if g.shield_up => "Left hand: shield up",
        Some(_) => "Left hand: shield down",
    };
    let right = match duel.hand(Side::Right) {
        None => "Right hand: not in view",
        Some(_) if !g.drawn => "Right hand: sword sheathed",
        Some(_) if g.swinging() => "Right hand: swinging",
        Some(_) => "Right hand: sword ready",
    };
    let y = size[1] - 30.0;
    centered(ui, atlas, [size[0] * 0.5 - 130.0, y], left, white(0.9), 1.0);
    centered(
        ui,
        atlas,
        [size[0] * 0.5 + 130.0, y],
        right,
        white(0.9),
        1.0,
    );
    let p = g.sheathe_progress(duel.hand_time());
    if p > 0.0 {
        bar(
            ui,
            atlas,
            [size[0] * 0.5 + 80.0, y + 12.0, 100.0, 4.0],
            p,
            white(1.0),
        );
    }
}

fn legend(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2]) {
    let width = GESTURES
        .iter()
        .map(|l| atlas.measure(l))
        .fold(0.0, f32::max);
    let x = size[0] - 24.0 - width;
    let mut y = size[1] - 30.0 - atlas.line * (GESTURES.len() as f32 + 1.5);
    for line in GESTURES {
        ui.text(atlas, x, y, line, gray(0.75));
        y += atlas.line;
    }
}

fn banner(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], duel: &HandDuel) {
    let fight = duel.fight();
    let (title, color) = match fight.outcome {
        Outcome::Fighting => return,
        Outcome::Won => ("The Warden falls", amber(1.0)),
        Outcome::Lost => ("You fell", RED),
    };
    centered(
        ui,
        atlas,
        [size[0] * 0.5, size[1] * 0.32],
        title,
        color,
        2.2,
    );
    let next = if duel.gestures().drawn {
        "Sheathe your sword over your right shoulder to fight again"
    } else {
        "Draw your sword, then sheathe it to fight again"
    };
    centered(
        ui,
        atlas,
        [size[0] * 0.5, size[1] * 0.32 + atlas.line * 2.0],
        next,
        white(0.9),
        1.0,
    );
}
