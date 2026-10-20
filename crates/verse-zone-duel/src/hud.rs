//! The duel's screen: both fighters' health and stamina, the direction
//! your next blow takes, a cue for the side Skurdo's blow is coming from,
//! the callouts over each hit, a short log, and the win or loss banner.
//! Laid out in logical points on a screen of `size`; the host scales the
//! batch to pixels.

use glam::{Mat4, Vec3};
use verse_gfx::ui::{self, Atlas, UiBatch};

use crate::Duel;
use crate::rules::{Action, CALLOUT_LIFE, Dir, Fight, Fighter, MAX_STAMINA, Outcome, Side, Tone};

/// The keyboard line on the desktop.
pub const CONTROLS: &str = "Mouse picks direction · Left click strike · Right hold shield · Q parry · E bash · Space step · WASD footwork · R restart · G leave";

const TEXT: [f32; 4] = [0.95, 0.92, 0.85, 1.0];
const DIM: [f32; 4] = [0.75, 0.72, 0.66, 0.9];
const GOLD: [f32; 4] = [0.96, 0.76, 0.32, 1.0];
const RED: [f32; 4] = [0.9, 0.24, 0.16, 1.0];
const HEALTH: [f32; 4] = [0.72, 0.13, 0.1, 1.0];
const STAMINA: [f32; 4] = [0.86, 0.74, 0.3, 1.0];

fn alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], c[3] * a]
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

fn centered(ui: &mut UiBatch, atlas: &Atlas, c: [f32; 2], text: &str, color: [f32; 4], k: f32) {
    let width = atlas.measure(text);
    let start = ui.vertices.len();
    ui.text(
        atlas,
        c[0] - width * 0.5,
        c[1] - atlas.line * 0.5,
        text,
        color,
    );
    if (k - 1.0).abs() > 1e-3 {
        for v in &mut ui.vertices[start..] {
            v.pos = [c[0] + (v.pos[0] - c[0]) * k, c[1] + (v.pos[1] - c[1]) * k];
        }
    }
}

fn shadowed(ui: &mut UiBatch, atlas: &Atlas, c: [f32; 2], text: &str, color: [f32; 4], k: f32) {
    centered(
        ui,
        atlas,
        [c[0] + 1.5, c[1] + 1.5],
        text,
        ui::field(color[3] * 0.9),
        k,
    );
    centered(ui, atlas, c, text, color, k);
}

fn meter(ui: &mut UiBatch, atlas: &Atlas, rect: [f32; 4], fill: f32, color: [f32; 4]) {
    let [x, y, w, h] = rect;
    ui.rect(atlas, x - 1.0, y - 1.0, w + 2.0, h + 2.0, ui::field(0.85));
    ui.rect(atlas, x, y, w * fill.clamp(0.0, 1.0), h, color);
    ui.frame(
        atlas,
        x - 1.0,
        y - 1.0,
        w + 2.0,
        h + 2.0,
        1.0,
        alpha(DIM, 0.6),
    );
}

/// Draws the whole screen for `duel` seen through `view_proj`.
pub fn draw(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, duel: &Duel) {
    let fight = duel.fight();
    overhead(ui, atlas, size, view_proj, fight);
    callouts(ui, atlas, size, view_proj, fight);
    let w = 300.0_f32.min(size[0] * 0.3);
    frame(ui, atlas, [20.0, 18.0], w, fight.player(), fight.now);
    frame(
        ui,
        atlas,
        [size[0] - w - 20.0, 18.0],
        w,
        fight.enemy(),
        fight.now,
    );
    incoming(ui, atlas, size, fight);
    stance(ui, atlas, size, duel);
    log(ui, atlas, size, fight);
    if let Some((text, until)) = duel.message() {
        let a = ((until - fight.now) / 0.3).clamp(0.0, 1.0);
        shadowed(
            ui,
            atlas,
            [size[0] * 0.5, size[1] * 0.3],
            text,
            alpha(GOLD, a),
            1.1,
        );
    }
    banner(ui, atlas, size, fight);
    centered(
        ui,
        atlas,
        [size[0] * 0.5, size[1] - 16.0],
        CONTROLS,
        alpha(DIM, 0.85),
        0.85,
    );
}

/// A fighter's frame: name, health, stamina, and what ails or helps them.
fn frame(ui: &mut UiBatch, atlas: &Atlas, at: [f32; 2], w: f32, f: &Fighter, now: f32) {
    let [x, y] = at;
    ui.rect(atlas, x - 8.0, y - 6.0, w + 16.0, 88.0, ui::field(0.72));
    ui.text(atlas, x, y, f.kit.name(), TEXT);
    let flash = now - f.hit_at < 0.15;
    let hp = (f.hp / f.max_hp).max(0.0);
    meter(
        ui,
        atlas,
        [x, y + atlas.line + 4.0, w, 10.0],
        hp,
        if flash { TEXT } else { HEALTH },
    );
    meter(
        ui,
        atlas,
        [x, y + atlas.line + 18.0, w, 6.0],
        f.stamina / MAX_STAMINA,
        if f.tired() { RED } else { STAMINA },
    );
    let mut status = vec![f.kit.title().to_owned()];
    if f.bleeding(now) > 0.0 {
        status.push("Bleeding".to_owned());
    }
    if f.tired() {
        status.push("Winded".to_owned());
    }
    if now < f.riposte_until {
        status.push("Riposte!".to_owned());
    }
    if now < f.shield_locked_until {
        status.push("Shield arm spent".to_owned());
    }
    if matches!(f.action, Action::Stagger { .. }) {
        status.push("Staggered".to_owned());
    }
    if f.wounds_leg > 0 {
        status.push("Limping".to_owned());
    }
    ui.text(atlas, x, y + atlas.line + 30.0, &status.join(" · "), DIM);
}

fn overhead(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, fight: &Fight) {
    let e = fight.enemy();
    if !e.alive() {
        return;
    }
    let Some([cx, cy]) = project(view_proj, size, e.pos + Vec3::Y * 2.15) else {
        return;
    };
    let w = 84.0;
    meter(
        ui,
        atlas,
        [cx - w * 0.5, cy, w, 6.0],
        e.hp / e.max_hp,
        HEALTH,
    );
    meter(
        ui,
        atlas,
        [cx - w * 0.5, cy + 9.0, w, 3.0],
        e.stamina / MAX_STAMINA,
        STAMINA,
    );
}

fn callouts(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], view_proj: Mat4, fight: &Fight) {
    for (i, c) in fight.callouts.iter().enumerate() {
        let age = (fight.now - c.born).max(0.0);
        let lift = 0.25 + age * 0.35 + (i % 3) as f32 * 0.12;
        let Some(at) = project(view_proj, size, c.at + Vec3::Y * lift) else {
            continue;
        };
        let a = (1.0 - (age / CALLOUT_LIFE).powi(3)).clamp(0.0, 1.0);
        let color = match c.tone {
            Tone::Good => GOLD,
            Tone::Bad => RED,
            Tone::Plain => TEXT,
        };
        let pop = if age < 0.1 {
            1.0 + (0.1 - age) * 3.0
        } else {
            1.0
        };
        shadowed(
            ui,
            atlas,
            at,
            &c.text,
            alpha(color, a),
            if c.big { 1.15 * pop } else { 0.95 * pop },
        );
    }
}

/// Where on screen the side a blow lands on sits, as a unit offset from
/// the middle: Skurdo's forehand comes at your shield side, his backhand
/// at your weapon side.
#[must_use]
pub fn landing(dir: Dir) -> [f32; 2] {
    match dir {
        Dir::Right => [-1.0, 0.0],
        Dir::Left => [1.0, 0.0],
        Dir::Overhead => [0.0, -1.0],
        Dir::Thrust => [0.0, 0.0],
    }
}

/// The cue for Skurdo's blow: an arrow at the side it will land, filling
/// as it winds up and turning red in the release.
fn incoming(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    let Some((dir, released, progress)) = fight.enemy().threat(fight.now) else {
        return;
    };
    let c = [size[0] * 0.5, size[1] * 0.46];
    let [ox, oy] = landing(dir);
    let r = 110.0;
    let at = [c[0] + ox * r, c[1] + oy * r * 0.9];
    let color = if released {
        RED
    } else {
        alpha(GOLD, 0.6 + 0.4 * progress)
    };
    let s = 22.0;
    if dir == Dir::Thrust {
        // A point coming straight at you: closing rings.
        let k = if released { 0.4 } else { 1.0 - 0.6 * progress };
        ui.ring(atlas, at[0], at[1], s * 2.2 * k + 6.0, 3.0, color);
        ui.ring(atlas, at[0], at[1], s * 1.2 * k + 3.0, 2.0, color);
        ui.disc(atlas, at[0], at[1], 4.0, color);
    } else {
        // An arrowhead pointing in toward you from that side.
        let (dx, dy) = (-ox, -oy);
        let (px, py) = (-dy, dx);
        let tip = [at[0] + dx * s, at[1] + dy * s];
        let back = [at[0] - dx * s * 0.6, at[1] - dy * s * 0.6];
        ui.triangle(
            atlas,
            tip,
            [back[0] + px * s, back[1] + py * s],
            [back[0] - px * s, back[1] - py * s],
            color,
        );
        ui.line(
            atlas,
            back,
            [
                back[0] - dx * s * 1.4 * progress.max(0.2),
                back[1] - dy * s * 1.4 * progress.max(0.2),
            ],
            4.0,
            color,
        );
    }
    let label = format!("{} from {}", dir.name(), side_word(dir));
    shadowed(
        ui,
        atlas,
        [at[0], at[1] + s * 1.6 + 6.0],
        &label,
        color,
        0.9,
    );
}

fn side_word(dir: Dir) -> &'static str {
    match dir {
        Dir::Right => "your shield side",
        Dir::Left => "your sword side",
        Dir::Overhead => "above",
        Dir::Thrust => "the front",
    }
}

/// Your next blow's direction: four marks around a center, the chosen one
/// lit, with the windup's progress and a feint's mark.
fn stance(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], duel: &Duel) {
    let fight = duel.fight();
    let me = fight.player();
    let c = [size[0] * 0.5, size[1] - 96.0];
    ui.disc(atlas, c[0], c[1], 44.0, ui::field(0.55));
    let (active, progress, feinted) = match me.action {
        Action::Windup { dir, dur, feints } => (
            Some(dir),
            (me.elapsed(fight.now) / dur).min(1.0),
            feints > 0,
        ),
        Action::Release { dir, .. } => (Some(dir), 1.0, false),
        _ => (None, 0.0, false),
    };
    for dir in Dir::ALL {
        let [ox, oy] = match dir {
            Dir::Overhead => [0.0, -1.0],
            Dir::Left => [-1.0, 0.0],
            Dir::Right => [1.0, 0.0],
            Dir::Thrust => [0.0, 1.0],
        };
        let chosen = duel.aim_dir() == dir;
        let lit = active == Some(dir);
        let color = if lit {
            GOLD
        } else if chosen {
            TEXT
        } else {
            alpha(DIM, 0.5)
        };
        let at = [c[0] + ox * 28.0, c[1] + oy * 28.0];
        let s = if chosen || lit { 9.0 } else { 6.0 };
        let (px, py) = (-oy, ox);
        ui.triangle(
            atlas,
            [at[0] + ox * s, at[1] + oy * s],
            [at[0] - ox * s * 0.5 + px * s, at[1] - oy * s * 0.5 + py * s],
            [at[0] - ox * s * 0.5 - px * s, at[1] - oy * s * 0.5 - py * s],
            color,
        );
    }
    if active.is_some() {
        ui.ring(
            atlas,
            c[0],
            c[1],
            12.0 + 26.0 * progress,
            2.0,
            alpha(GOLD, 0.8),
        );
    }
    let label = match me.action {
        Action::Windup { dir, .. } if feinted => format!("Feint to {}", dir.name().to_lowercase()),
        Action::Windup { dir, .. } | Action::Release { dir, .. } => dir.name().to_owned(),
        Action::Parry => "Parry".to_owned(),
        Action::Bash { .. } => "Bash".to_owned(),
        Action::Stagger { .. } => "Staggered".to_owned(),
        _ if duel.shield_held() => "Shield up".to_owned(),
        _ => format!("Next: {}", duel.aim_dir().name().to_lowercase()),
    };
    shadowed(ui, atlas, [c[0], c[1] + 58.0], &label, TEXT, 0.95);
}

fn log(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    if size[0] < 900.0 {
        return;
    }
    let lines = fight.log.len();
    for (i, line) in fight.log.iter().enumerate() {
        let y = size[1] - 40.0 - (lines - i) as f32 * atlas.line;
        let a = 0.45 + 0.5 * (i + 1) as f32 / lines as f32;
        ui.text(atlas, 20.0, y, line, alpha(DIM, a));
    }
}

fn banner(ui: &mut UiBatch, atlas: &Atlas, size: [f32; 2], fight: &Fight) {
    let (title, color, line) = match fight.outcome {
        Outcome::Fighting => return,
        Outcome::Won => ("VICTORY", GOLD, "Skurdo has fallen."),
        Outcome::Lost => ("DEFEATED", RED, "You have fallen."),
    };
    let c = [size[0] * 0.5, size[1] * 0.36];
    ui.rect(atlas, 0.0, c[1] - 54.0, size[0], 120.0, ui::field(0.7));
    centered(ui, atlas, c, title, color, 3.0);
    let summary = format!(
        "{line} You dealt {:.0} and took {:.0}.",
        fight.fighters[Side::Player.index()].dealt,
        fight.fighters[Side::Enemy.index()].dealt
    );
    centered(ui, atlas, [c[0], c[1] + 32.0], &summary, TEXT, 1.0);
    centered(
        ui,
        atlas,
        [c[0], c[1] + 32.0 + atlas.line * 1.3],
        "Press R to fight again · G leaves",
        color,
        1.0,
    );
}
