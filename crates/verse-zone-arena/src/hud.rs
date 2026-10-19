//! The arena's screen: health bars over both robots, unit frames, the
//! action bar with its cooldowns, floating numbers, the error line, a short
//! log, and the win or loss banner. Everything is laid out in logical
//! points on a screen of `size`, as the Grove's hotbar is; the host scales
//! the batch to pixels.

use coder_ui::theme::Intensity;
use glam::{Mat4, Vec3};
use verse_gfx::ui::{self, Atlas, UiBatch};

use crate::Arena;
use crate::rules::{Ability, FLOATER_LIFE, Fight, GCD, Outcome, Robot, SLAM_WINDUP, Side};

/// The action bar's slot size and gap, and its lift off the bottom edge.
pub const SLOT: f32 = 54.0;
pub const GAP: f32 = 10.0;
pub const BOTTOM: f32 = 46.0;

/// The keyboard line under the bar on the desktop, where G returns to the
/// plaza.
pub const CONTROLS: &str = "WASD move  Space jump  1-4 abilities  R restart  G leave";
/// The keyboard line on the web page, which has nowhere to leave to.
pub const CONTROLS_WEB: &str = "WASD move  Space jump  1-4 abilities  R restart";

fn white(alpha: f32) -> [f32; 4] {
    let [r, ..] = ui::amber(Intensity::Full, alpha);
    [r, r, r, alpha]
}

fn gray(step: Intensity, alpha: f32) -> [f32; 4] {
    let [r, ..] = ui::amber(step, alpha);
    [r, r, r, alpha]
}

fn amber(alpha: f32) -> [f32; 4] {
    let [r, g, b] = crate::enemy_amber(1.0);
    [r, g, b, alpha]
}

const SHIELD: [f32; 4] = [0.55, 0.8, 1.0, 1.0];

fn side_color(side: Side, alpha: f32) -> [f32; 4] {
    match side {
        Side::Player => white(alpha),
        Side::Enemy => amber(alpha),
    }
}

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

/// One line of text centered on `center`, scaled by `k` about it.
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

/// A health bar `w` by `h` at `(x, y)`: health in its robot's color, the
/// shield after it in the shield's color.
fn bar(ui: &mut UiBatch, atlas: &Atlas, rect: [f32; 4], robot: &Robot, now: f32) {
    let [x, y, w, h] = rect;
    ui.rect(atlas, x - 1.0, y - 1.0, w + 2.0, h + 2.0, ui::field(0.85));
    let health = (robot.hp / robot.max_hp).clamp(0.0, 1.0);
    let flash = now - robot.hit_at < 0.15;
    let color = if flash {
        white(1.0)
    } else {
        side_color(robot.side, 0.95)
    };
    ui.rect(atlas, x, y, w * health, h, color);
    let shield = robot.shield_at(now) / robot.max_hp;
    if shield > 0.0 {
        let start = (x + w * health).min(x + w);
        let width = (w * shield).min(x + w - start).max(2.0);
        ui.rect(atlas, start, y, width, h, SHIELD);
    }
    ui.frame(
        atlas,
        x - 1.0,
        y - 1.0,
        w + 2.0,
        h + 2.0,
        1.0,
        gray(Intensity::Half, 0.9),
    );
}

/// The name each robot's frame shows.
#[must_use]
pub const fn name(side: Side) -> &'static str {
    match side {
        Side::Player => "STRIKER",
        Side::Enemy => "BRUTE",
    }
}

/// Draws the whole screen for `arena` seen through `view_proj`, with
/// `controls` as the keyboard line under the action bar.
pub fn draw(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    view_proj: Mat4,
    arena: &Arena,
    controls: &str,
) {
    let fight = arena.fight();
    let now = fight.now;
    overheads(ui, atlas, size, view_proj, fight);
    floaters(ui, atlas, size, view_proj, fight);
    frames(ui, atlas, size, fight);
    action_bar(ui, atlas, size, fight, controls);
    log(ui, atlas, size, fight);
    if let Some((text, until)) = arena.message() {
        let alpha = ((until - now) / 0.4).clamp(0.0, 1.0);
        centered(
            ui,
            atlas,
            [size[0] * 0.5, size[1] * 0.22],
            text,
            amber(alpha),
            1.0,
        );
    }
    banner(ui, atlas, size, fight);
}

fn overheads(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, fight: &Fight) {
    let now = fight.now;
    for robot in [&fight.player, &fight.enemy] {
        if !robot.alive() {
            continue;
        }
        let height = match robot.side {
            Side::Player => 2.9,
            Side::Enemy => 3.9,
        };
        let Some([cx, cy]) = project(view_proj, size, robot.pos + Vec3::Y * height) else {
            continue;
        };
        let w = 92.0;
        bar(ui, atlas, [cx - w * 0.5, cy, w, 7.0], robot, now);
        // The target's name over its bar; the player's own bar goes bare.
        if robot.side == Side::Enemy {
            centered(
                ui,
                atlas,
                [cx, cy - atlas.line * 0.6],
                name(Side::Enemy),
                side_color(robot.side, 0.95),
                1.0,
            );
        }
        // The slam's cast bar under the enemy's health.
        if robot.side == Side::Enemy
            && let Some(windup) = fight.brain.windup
        {
            let t = ((now - windup.started) / SLAM_WINDUP).clamp(0.0, 1.0);
            let y = cy + 12.0;
            ui.rect(atlas, cx - w * 0.5, y, w, 5.0, ui::field(0.85));
            ui.rect(atlas, cx - w * 0.5, y, w * t, 5.0, amber(1.0));
            centered(
                ui,
                atlas,
                [cx, y + 5.0 + atlas.line * 0.6],
                "Ground Slam",
                amber(1.0),
                1.0,
            );
        }
    }
}

fn floaters(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, fight: &Fight) {
    for f in &fight.floaters {
        let age = (fight.now - f.born).max(0.0);
        let rise = age * 1.1;
        let Some(at) = project(view_proj, size, f.at + Vec3::Y * rise) else {
            continue;
        };
        let alpha = (1.0 - (age / FLOATER_LIFE).powi(2)).clamp(0.0, 1.0);
        let color = if f.good {
            [SHIELD[0], SHIELD[1], SHIELD[2], alpha]
        } else {
            match f.over {
                // What the player dealt reads white; what it took, amber.
                Side::Enemy => white(alpha),
                Side::Player => amber(alpha),
            }
        };
        let pop = if age < 0.12 {
            1.0 + (0.12 - age) * 4.0
        } else {
            1.0
        };
        let k = if f.crit { 2.0 * pop } else { 1.35 * pop };
        // A dark backing keeps a number readable over bright lines.
        centered(
            ui,
            atlas,
            [at[0] + 1.5, at[1] + 1.5],
            &f.text,
            ui::field(alpha * 0.9),
            k,
        );
        centered(ui, atlas, at, &f.text, color, k);
    }
}

/// The player's frame at the top left and the target's at the top middle.
fn frames(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    let now = fight.now;
    let unit = |ui: &mut UiBatch, x: f32, y: f32, w: f32, robot: &Robot| {
        ui.rect(atlas, x - 8.0, y - 6.0, w + 16.0, 46.0, ui::field(0.7));
        ui.text(atlas, x, y, name(robot.side), side_color(robot.side, 1.0));
        let numbers = format!("{:.0} / {:.0}", robot.hp.max(0.0), robot.max_hp);
        let nw = atlas.measure(&numbers);
        ui.text(atlas, x + w - nw, y, &numbers, gray(Intensity::Full, 0.9));
        bar(ui, atlas, [x, y + atlas.line + 4.0, w, 10.0], robot, now);
        let mut status = Vec::new();
        if robot.shield_at(now) > 0.0 {
            status.push(format!(
                "Shield {:.0} ({:.1} s)",
                robot.shield_at(now),
                robot.shield_until - now
            ));
        }
        if robot.stunned(now) && robot.alive() {
            status.push(format!("Stunned {:.1} s", robot.stunned_until - now));
        }
        if !status.is_empty() {
            ui.text(
                atlas,
                x,
                y + atlas.line + 18.0,
                &status.join("  "),
                gray(Intensity::Full, 0.85),
            );
        }
    };
    unit(ui, 20.0, 18.0, 220.0, &fight.player);
    let w = 260.0_f32.min(size[0] - 300.0).max(160.0);
    unit(
        ui,
        (size[0] - w) * 0.5 + 60.0_f32.min(size[0] * 0.05),
        18.0,
        w,
        &fight.enemy,
    );
}

/// The four slots, bottom center: a drawn icon, the key, the cooldown's
/// sweep and seconds, and the name under each.
fn action_bar(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight, controls: &str) {
    let count = Ability::ALL.len() as f32;
    let width = count * SLOT + (count - 1.0) * GAP;
    let x0 = (size[0] - width) * 0.5;
    let y = size[1] - BOTTOM - SLOT - atlas.line;
    ui.rect(
        atlas,
        x0 - 10.0,
        y - 8.0,
        width + 20.0,
        SLOT + atlas.line + 14.0,
        ui::field(0.75),
    );
    for (i, ability) in Ability::ALL.into_iter().enumerate() {
        let x = x0 + i as f32 * (SLOT + GAP);
        let usable = fight.usable(ability);
        let ready = fight.remaining(ability) <= 0.0;
        let lit = usable.is_ok();
        let color = if lit {
            white(1.0)
        } else {
            gray(Intensity::Half, 0.9)
        };
        ui.rect(atlas, x, y, SLOT, SLOT, ui::field(0.95));
        icon(ui, atlas, [x, y], ability, color, fight.now);
        let remaining = fight.remaining(ability);
        if remaining > 0.0 {
            let total = if fight.on_global(ability) {
                GCD
            } else {
                ability.cooldown().max(GCD)
            };
            ui.cooldown(atlas, [x, y, SLOT], remaining / total);
            if !fight.on_global(ability) {
                centered(
                    ui,
                    atlas,
                    [x + SLOT * 0.5, y + SLOT * 0.5],
                    &format!("{remaining:.0}"),
                    white(1.0),
                    1.4,
                );
            }
        }
        let frame = if ready && usable == Err(crate::rules::Refusal::OutOfRange) {
            // Ready but too far: the frame says so.
            amber(0.9)
        } else {
            color
        };
        ui.frame(atlas, x, y, SLOT, SLOT, if lit { 2.0 } else { 1.0 }, frame);
        ui.text(atlas, x + 4.0, y + 2.0, &ability.key().to_string(), color);
        centered(
            ui,
            atlas,
            [x + SLOT * 0.5, y + SLOT + atlas.line * 0.6],
            ability.short(),
            gray(Intensity::Full, 0.9),
            0.85,
        );
    }
    centered(
        ui,
        atlas,
        [size[0] * 0.5, size[1] - BOTTOM * 0.45],
        controls,
        gray(Intensity::Half, 0.9),
        0.9,
    );
}

/// Each ability's icon, drawn in lines inside its slot.
fn icon(
    ui: &mut UiBatch,
    atlas: &Atlas,
    at: [f32; 2],
    ability: Ability,
    color: [f32; 4],
    now: f32,
) {
    let c = [at[0] + SLOT * 0.5, at[1] + SLOT * 0.55];
    let s = SLOT * 0.3;
    match ability {
        Ability::Laser => {
            ui.line(
                atlas,
                [c[0] - s, c[1] + s * 0.5],
                [c[0] + s, c[1] - s * 0.5],
                3.0,
                color,
            );
            ui.disc(atlas, c[0] + s, c[1] - s * 0.5, 3.5, color);
            ui.line(
                atlas,
                [c[0] - s, c[1] + s * 0.5],
                [c[0] - s * 0.6, c[1] + s * 0.9],
                2.0,
                color,
            );
        }
        Ability::Missiles => {
            for k in [-1.0_f32, 0.0, 1.0] {
                let x = c[0] + k * s * 0.7;
                let top = c[1] - s * 0.9 + k.abs() * s * 0.3;
                ui.line(atlas, [x, c[1] + s * 0.8], [x, top], 2.0, color);
                ui.triangle(
                    atlas,
                    [x - 4.0, top + 5.0],
                    [x + 4.0, top + 5.0],
                    [x, top - 3.0],
                    color,
                );
            }
        }
        Ability::Shield => {
            ui.ring(atlas, c[0], c[1], s * 1.05, 2.0, color);
            ui.ring(atlas, c[0], c[1], s * 0.55, 1.5, color);
        }
        Ability::Emp => {
            ui.disc(atlas, c[0], c[1], 3.5, color);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU + now * 0.6;
                let (sn, cs) = a.sin_cos();
                ui.line(
                    atlas,
                    [c[0] + cs * s * 0.45, c[1] + sn * s * 0.45],
                    [c[0] + cs * s, c[1] + sn * s],
                    2.0,
                    color,
                );
            }
        }
    }
}

/// The last few log lines at the bottom left.
fn log(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    if size[0] < 760.0 {
        return;
    }
    let lines = fight.log.len();
    for (i, line) in fight.log.iter().enumerate() {
        let y = size[1] - BOTTOM - (lines - i) as f32 * atlas.line - 8.0;
        let alpha = 0.45 + 0.5 * (i + 1) as f32 / lines as f32;
        ui.text(atlas, 20.0, y, line, gray(Intensity::Full, alpha));
    }
}

/// The win or loss banner and how to fight again.
fn banner(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    let (title, color) = match fight.outcome {
        Outcome::Fighting => return,
        Outcome::Won => ("VICTORY", white(1.0)),
        Outcome::Lost => ("DESTROYED", amber(1.0)),
    };
    let c = [size[0] * 0.5, size[1] * 0.38];
    ui.rect(atlas, 0.0, c[1] - 52.0, size[0], 112.0, ui::field(0.7));
    centered(ui, atlas, c, title, color, 3.0);
    let summary = format!(
        "You dealt {:.0} damage and took {:.0}",
        fight.dealt[Side::Player as usize],
        fight.dealt[Side::Enemy as usize]
    );
    centered(
        ui,
        atlas,
        [c[0], c[1] + 30.0],
        &summary,
        gray(Intensity::Full, 0.9),
        1.0,
    );
    centered(
        ui,
        atlas,
        [c[0], c[1] + 30.0 + atlas.line * 1.2],
        "Press R to fight again",
        color,
        1.0,
    );
}
