//! The heads-up display: chat windows, the chat input, name tags, and
//! speech bubbles over avatars, all in the amber ladder.
//!
//! Layout follows Horse Isle 1: two chat windows along the bottom, world
//! chat on the left and personal chat on the right, with one input line
//! beneath them and a method selector at its left edge. Speech bubbles are
//! the RuneScape touch Horse Isle never had: public lines float over the
//! speaker's head for a few seconds.

use coder_terminal::Intensity;
use glam::{Mat4, Vec3, Vec4};

use crate::chat::{self, Channel, Line, Log};
use crate::ui::{Atlas, UiBatch, amber, field};

/// Chat input state.
#[derive(Clone, Debug, Default)]
pub struct Input {
    /// Whether keys go to the chat line instead of movement.
    pub open: bool,
    /// The line being typed.
    pub text: String,
    /// Index into the method list.
    pub method: usize,
    /// The last line sent, for recall with the up arrow.
    pub last: String,
}

/// Something to draw over the world at a 3D point.
#[derive(Clone, Debug)]
pub struct Overhead {
    /// World position of the speaker's feet.
    pub feet: Vec3,
    /// Name tag, if any.
    pub name: Option<String>,
    /// Speech bubble text, if any.
    pub bubble: Option<String>,
}

/// Everything the HUD needs for one frame.
pub struct Frame<'a> {
    /// Surface width and height in pixels.
    pub size: [f32; 2],
    /// Pixel scale (1 on standard displays, 2 on Retina).
    pub scale: f32,
    /// Camera projection times view.
    pub view_proj: Mat4,
    /// Chat history.
    pub log: &'a Log,
    /// Chat input.
    pub input: &'a Input,
    /// The selected method's label.
    pub method: String,
    /// Left window heading.
    pub world_title: String,
    /// Things over heads.
    pub overheads: &'a [Overhead],
    /// Seconds since start, for the caret blink.
    pub time: f32,
}

/// Builds the HUD.
#[must_use]
pub fn build(atlas: &Atlas, f: &Frame<'_>) -> UiBatch {
    let mut ui = UiBatch::default();
    let s = f.scale;
    let [w, h] = f.size;
    for o in f.overheads {
        overhead(&mut ui, atlas, f, o);
    }

    let m = 12.0 * s;
    let pad = 8.0 * s;
    let bar = atlas.line + 12.0 * s;
    let bar_y = h - m - bar;
    let panel_h = (h * 0.28).clamp(atlas.line * 6.0, atlas.line * 16.0);
    let panel_y = bar_y - 6.0 * s - panel_h;
    let panel_w = (w - 3.0 * m) / 2.0;

    window(
        &mut ui,
        atlas,
        m,
        panel_y,
        panel_w,
        panel_h,
        pad,
        &f.world_title,
        &f.log.world,
    );
    window(
        &mut ui,
        atlas,
        2.0 * m + panel_w,
        panel_y,
        panel_w,
        panel_h,
        pad,
        "PERSONAL · near · here · rooms · private",
        &f.log.personal,
    );

    // The input line.
    let edge = if f.input.open {
        Intensity::ThreeQuarters
    } else {
        Intensity::Quarter
    };
    let bar_w = w - 2.0 * m;
    ui.rect(atlas, m, bar_y, bar_w, bar, field(0.88));
    ui.frame(atlas, m, bar_y, bar_w, bar, s.max(1.0), amber(edge, 1.0));
    let text_y = bar_y + (bar - atlas.line) / 2.0;
    let label = format!("[{}]", f.method);
    let mut x = m + pad;
    x += ui.text(atlas, x, text_y, &label, amber(Intensity::Full, 1.0));
    x += atlas.advance;
    if f.input.open {
        let room = ((bar_w - (x - m) - pad) / atlas.advance) as usize;
        let shown: String = {
            let chars: Vec<char> = f.input.text.chars().collect();
            let start = chars.len().saturating_sub(room.saturating_sub(1));
            chars[start..].iter().collect()
        };
        x += ui.text(atlas, x, text_y, &shown, amber(Intensity::Full, 1.0));
        if (f.time * 2.0).fract() < 0.6 {
            ui.text(atlas, x, text_y, "_", amber(Intensity::Full, 1.0));
        }
    } else {
        ui.text(
            atlas,
            x,
            text_y,
            "Enter to chat · Tab: channel · /a all /$ ads /z zone /n near /h here /r room · /name to whisper",
            amber(Intensity::Quarter, 1.0),
        );
    }
    ui
}

#[allow(clippy::too_many_arguments)]
fn window(
    ui: &mut UiBatch,
    atlas: &Atlas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    pad: f32,
    title: &str,
    lines: &std::collections::VecDeque<Line>,
) {
    ui.rect(atlas, x, y, w, h, field(0.78));
    ui.frame(atlas, x, y, w, h, 1.0, amber(Intensity::Quarter, 1.0));
    ui.text(
        atlas,
        x + pad,
        y + pad * 0.5,
        title,
        amber(Intensity::Half, 1.0),
    );
    let top = y + pad * 0.5 + atlas.line + 2.0;
    ui.rect(
        atlas,
        x + pad,
        top,
        w - 2.0 * pad,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );

    let inner = w - 2.0 * pad;
    let mut cursor = y + h - pad * 0.5;
    for line in lines.iter().rev() {
        let spans = spans(line);
        let rows = layout(atlas, &spans, inner);
        for row in rows.iter().rev() {
            cursor -= atlas.line;
            if cursor < top + 2.0 {
                return;
            }
            let mut px = x + pad;
            for (text, step) in row {
                px += ui.text(atlas, px, cursor, text, amber(*step, 1.0));
            }
        }
    }
}

/// A line as styled spans: channel tag, speaker, text, and note.
fn spans(line: &Line) -> Vec<(String, Intensity)> {
    let Some(channel) = &line.channel else {
        return vec![(line.text.clone(), Intensity::Half)];
    };
    let tag = match channel {
        Channel::All => "all ".to_owned(),
        Channel::Ads => "$ ".to_owned(),
        Channel::Zone => "zone ".to_owned(),
        Channel::Near => "near ".to_owned(),
        Channel::Here => "here ".to_owned(),
        Channel::Room(room) => format!("#{room} "),
        Channel::Pm(_) => "pm ".to_owned(),
    };
    let mut out = vec![(tag, Intensity::Quarter)];
    if line.from.is_empty() {
        out.push((line.text.clone(), Intensity::Half));
        return out;
    }
    let speaker = match &line.to {
        Some(to) => format!("{}>{}: ", line.from, to),
        None => format!("{}: ", line.from),
    };
    out.push((speaker, Intensity::Full));
    let body = if matches!(channel, Channel::Ads | Channel::Pm(_)) {
        Intensity::Full
    } else {
        Intensity::ThreeQuarters
    };
    out.push((line.text.clone(), body));
    if let Some(note) = &line.note {
        out.push((format!(" {note}"), Intensity::Quarter));
    }
    out
}

/// Wraps styled spans into rows no wider than `width`.
fn layout(
    atlas: &Atlas,
    spans: &[(String, Intensity)],
    width: f32,
) -> Vec<Vec<(String, Intensity)>> {
    let per = ((width / atlas.advance).floor() as usize).max(8);
    let mut rows: Vec<Vec<(String, Intensity)>> = vec![Vec::new()];
    let mut used = 0usize;
    for (text, step) in spans {
        let mut rest: Vec<char> = text.chars().collect();
        while !rest.is_empty() {
            let room = per - used;
            if room == 0 {
                rows.push(Vec::new());
                used = 0;
                continue;
            }
            let mut take = rest.len().min(room);
            if take < rest.len()
                && let Some(space) = rest[..take].iter().rposition(|c| *c == ' ')
                && space > 0
            {
                take = space + 1;
            }
            let piece: String = rest.drain(..take).collect();
            used += piece.chars().count();
            rows.last_mut().expect("a row").push((piece, *step));
            if !rest.is_empty() {
                rows.push(Vec::new());
                used = 0;
            }
        }
    }
    rows
}

/// Projects a world point to pixels, or `None` behind the camera.
#[must_use]
pub fn project(view_proj: Mat4, size: [f32; 2], p: Vec3) -> Option<[f32; 2]> {
    let clip: Vec4 = view_proj * p.extend(1.0);
    if clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
        return None;
    }
    Some([(ndc.x * 0.5 + 0.5) * size[0], (0.5 - ndc.y * 0.5) * size[1]])
}

fn overhead(ui: &mut UiBatch, atlas: &Atlas, f: &Frame<'_>, o: &Overhead) {
    let s = f.scale;
    let mut y_base = None;
    if let Some(name) = &o.name
        && let Some([x, y]) = project(f.view_proj, f.size, o.feet + Vec3::Y * 2.2)
    {
        let tw = atlas.measure(name);
        ui.text(
            atlas,
            x - tw / 2.0,
            y - atlas.line,
            name,
            amber(Intensity::Half, 1.0),
        );
        y_base = Some(y - atlas.line - 4.0 * s);
    }
    let Some(text) = &o.bubble else { return };
    let Some([x, y]) = y_base
        .map(|yb| project(f.view_proj, f.size, o.feet + Vec3::Y * 2.2).map(|[x, _]| [x, yb]))
        .unwrap_or_else(|| project(f.view_proj, f.size, o.feet + Vec3::Y * 2.4))
    else {
        return;
    };
    let mut rows = atlas.wrap(text, atlas.advance * 28.0);
    if rows.len() > 3 {
        rows.truncate(3);
        if let Some(last) = rows.last_mut() {
            let keep: String = last.chars().take(26).collect();
            *last = format!("{keep}…");
        }
    }
    let pad = 6.0 * s;
    let bw = rows.iter().map(|r| atlas.measure(r)).fold(0.0, f32::max) + 2.0 * pad;
    let bh = rows.len() as f32 * atlas.line + 2.0 * pad;
    let bx = x - bw / 2.0;
    let by = y - bh - 6.0 * s;
    ui.rect(atlas, bx, by, bw, bh, field(0.9));
    ui.frame(
        atlas,
        bx,
        by,
        bw,
        bh,
        s.max(1.0),
        amber(Intensity::Full, 1.0),
    );
    ui.rect(
        atlas,
        x - 1.0 * s,
        by + bh,
        2.0 * s,
        6.0 * s,
        amber(Intensity::Full, 1.0),
    );
    for (i, row) in rows.iter().enumerate() {
        ui.text(
            atlas,
            bx + pad,
            by + pad + i as f32 * atlas.line,
            row,
            amber(Intensity::Full, 1.0),
        );
    }
}

/// The method selector's label for a channel.
#[must_use]
pub fn method_label(channel: &Channel, name_of: impl Fn(&str) -> String) -> String {
    match channel {
        Channel::All => "ALL".into(),
        Channel::Ads => "ADS".into(),
        Channel::Zone => "ZONE".into(),
        Channel::Near => "NEAR".into(),
        Channel::Here => "HERE".into(),
        Channel::Room(room) => format!("#{room}"),
        Channel::Pm(pubkey) => format!("PM {}", name_of(pubkey)),
    }
}

/// The left window's heading: world and zone.
#[must_use]
pub fn world_title(world: &str, pos: Vec3) -> String {
    format!(
        "WORLD {world} · {} · all · ads · zone",
        chat::zone_name(chat::zone_of(pos))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_ahead_projects_to_the_screen_center() {
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::Z, Vec3::Y);
        let proj = Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0);
        let p = project(proj * view, [800.0, 600.0], Vec3::new(0.0, 0.0, 10.0)).expect("visible");
        assert!((p[0] - 400.0).abs() < 1e-3 && (p[1] - 300.0).abs() < 1e-3);
        assert!(project(proj * view, [800.0, 600.0], Vec3::new(0.0, 0.0, -10.0)).is_none());
    }

    #[test]
    fn long_lines_wrap_within_the_window() {
        let atlas = Atlas::new(14.0);
        let line = Line {
            channel: Some(Channel::All),
            from: "north".into(),
            to: None,
            text: "a fairly long message that should wrap across rows".into(),
            note: None,
        };
        let rows = layout(&atlas, &spans(&line), atlas.advance * 20.0);
        assert!(rows.len() >= 3);
        for row in &rows {
            let n: usize = row.iter().map(|(t, _)| t.chars().count()).sum();
            assert!(n <= 20);
        }
    }

    #[test]
    fn the_hud_draws_something_at_rest() {
        let atlas = Atlas::new(14.0);
        let mut log = Log::default();
        log.push(Line::system("Player north has logged in"));
        let frame = Frame {
            size: [1440.0, 900.0],
            scale: 1.0,
            view_proj: Mat4::IDENTITY,
            log: &log,
            input: &Input::default(),
            method: "ALL".into(),
            world_title: world_title("verse-plaza", Vec3::ZERO),
            overheads: &[],
            time: 0.0,
        };
        assert!(!build(&atlas, &frame).vertices.is_empty());
    }
}
