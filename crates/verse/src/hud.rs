//! The heads-up display: chat windows, channel pills, the chat input,
//! name tags, and speech bubbles, all in the amber ladder.
//!
//! Layout follows Horse Isle 1: two chat windows along the bottom, world
//! chat on the left and personal chat on the right, with one input line
//! beneath them. Horse Isle's channel dropdown becomes a row of clickable
//! channel pills, so the choice is always visible. The input line says who
//! will hear a message before it is sent, typing `/` lists the shortcuts,
//! and ALL and ADS show a character count. The left window has two tabs:
//! this world's chat, and live public notes from popular Nostr relays.
//! Speech bubbles are the RuneScape touch Horse Isle never had. XP, level,
//! and titles sit at the top left, and `B` opens the quest board panel
//! above the chat windows (`crate::xp` writes their lines).

use std::collections::VecDeque;

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
    /// The last line sent, for recall with the up arrow.
    pub last: String,
}

/// Which tab the left window shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeftTab {
    /// This world's ALL, ADS, and ZONE chat.
    #[default]
    World,
    /// Live public notes from popular Nostr relays.
    Nostr,
}

/// Something to draw over the world at a 3D point.
#[derive(Clone, Debug)]
pub struct Overhead {
    /// World position the tag and bubble sit above.
    pub feet: Vec3,
    /// Height above `feet` to place them, in meters.
    pub lift: f32,
    /// Name tag, if any.
    pub name: Option<String>,
    /// How bright the name tag is.
    pub name_step: Intensity,
    /// Speech bubble text, if any.
    pub bubble: Option<String>,
}

/// A screen rectangle in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Left.
    pub x: f32,
    /// Top.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// True when `(px, py)` lies inside.
    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

/// Where the clickable parts landed this frame.
#[derive(Clone, Debug, Default)]
pub struct Layout {
    /// One rectangle per channel pill, in pill order.
    pub pills: Vec<Rect>,
    /// The left window's WORLD and NOSTR tabs.
    pub tabs: [Rect; 2],
    /// The input line.
    pub bar: Rect,
    /// The open quest board, and the furthest it can scroll, in rows.
    pub board: Option<(Rect, usize)>,
    /// The chat windows and pill row: clicks here are for the UI, not the
    /// camera.
    pub panels: Vec<Rect>,
}

impl Layout {
    /// True when a click at `(x, y)` belongs to the HUD.
    #[must_use]
    pub fn owns(&self, x: f32, y: f32) -> bool {
        self.bar.contains(x, y)
            || self.pills.iter().any(|r| r.contains(x, y))
            || self.tabs.iter().any(|r| r.contains(x, y))
            || self.panels.iter().any(|r| r.contains(x, y))
    }
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
    /// Public notes for the NOSTR tab.
    pub nostr: &'a VecDeque<Line>,
    /// The NOSTR tab's heading after its name.
    pub nostr_title: String,
    /// Which left tab shows.
    pub left_tab: LeftTab,
    /// Chat input.
    pub input: &'a Input,
    /// Channel pills: label and whether selected.
    pub pills: Vec<(String, bool)>,
    /// Who hears the selected channel, in words.
    pub hint: String,
    /// Character limit for the selected channel, if it has a tight one.
    pub limit: Option<usize>,
    /// The WORLD tab's heading after its name.
    pub world_title: String,
    /// Things over heads.
    pub overheads: &'a [Overhead],
    /// Seconds since start, for the caret blink.
    pub time: f32,
    /// The XP lines at the top left: XP, level, titles, and the board key.
    pub xp: Vec<(String, Intensity)>,
    /// The quest board's lines, while the board is open.
    pub board: Option<Vec<(String, Intensity)>>,
    /// Rows the board is scrolled down by.
    pub board_scroll: usize,
    /// A running replay's lines at the top right: each side's time, cost,
    /// place, and result, and the labels.
    pub replay: Vec<(String, Intensity)>,
    /// The replay list's lines, while it is open.
    pub picker: Option<Vec<(String, Intensity)>>,
    /// Rows the replay list is scrolled down by.
    pub picker_scroll: usize,
}

/// The `/` shortcuts, with what each does, for the suggestion list.
pub const SHORTCUTS: [(&str, &str); 9] = [
    ("/a", "ALL: everyone in the world"),
    ("/$", "ADS: buy, sell, announce (once a minute)"),
    ("/z", "ZONE: everyone in your district"),
    ("/n", "NEAR: players within 40 m"),
    ("/h", "HERE: players on your spot"),
    ("/r", "/r <room> <text>: a chat room"),
    ("/ai", "AGENT: talk to your agent, privately"),
    ("/<name>", "/<name> <text>: private message"),
    ("!mute", "!mute <channel>, !unmute <channel>"),
];

/// Builds the HUD and reports where its clickable parts are.
#[must_use]
pub fn build(atlas: &Atlas, f: &Frame<'_>) -> (UiBatch, Layout) {
    let mut ui = UiBatch::default();
    let mut layout = Layout::default();
    let s = f.scale;
    let [w, h] = f.size;
    for o in f.overheads {
        overhead(&mut ui, atlas, f, o);
    }

    let m = 12.0 * s;
    let pad = 8.0 * s;
    let bar = atlas.line + 14.0 * s;
    let bar_y = h - m - bar;
    let pill_h = atlas.line + 8.0 * s;
    let pill_y = bar_y - 6.0 * s - pill_h;
    let panel_h = (h * 0.26).clamp(atlas.line * 6.0, atlas.line * 15.0);
    let panel_y = pill_y - 6.0 * s - panel_h;
    let panel_w = (w - 3.0 * m) / 2.0;

    // Left window, with its two tabs as the heading.
    let left = Rect {
        x: m,
        y: panel_y,
        w: panel_w,
        h: panel_h,
    };
    let (lines, empty, rest) = match f.left_tab {
        LeftTab::World => (
            &f.log.world,
            "No world chat yet. Press Enter and say hi to everyone.",
            f.world_title.as_str(),
        ),
        LeftTab::Nostr => (
            f.nostr,
            "Listening for public notes on relay.damus.io and relay.primal.net…",
            f.nostr_title.as_str(),
        ),
    };
    frame_window(&mut ui, atlas, left);
    let mut tx = left.x + pad;
    for (i, (label, tab)) in [("WORLD", LeftTab::World), ("NOSTR", LeftTab::Nostr)]
        .into_iter()
        .enumerate()
    {
        let tw = atlas.measure(label) + 12.0 * s;
        let r = Rect {
            x: tx,
            y: left.y + 3.0 * s,
            w: tw,
            h: atlas.line + 4.0 * s,
        };
        let on = f.left_tab == tab;
        if on {
            ui.rect(atlas, r.x, r.y, r.w, r.h, amber(Intensity::Quarter, 0.55));
            ui.frame(atlas, r.x, r.y, r.w, r.h, 1.0, amber(Intensity::Full, 1.0));
        } else {
            ui.frame(
                atlas,
                r.x,
                r.y,
                r.w,
                r.h,
                1.0,
                amber(Intensity::Quarter, 1.0),
            );
        }
        let step = if on { Intensity::Full } else { Intensity::Half };
        ui.text(atlas, r.x + 6.0 * s, r.y + 2.0 * s, label, amber(step, 1.0));
        layout.tabs[i] = r;
        tx += tw + 5.0 * s;
    }
    ui.text(
        atlas,
        tx + 4.0 * s,
        left.y + 5.0 * s,
        rest,
        amber(Intensity::Quarter, 1.0),
    );
    let top = left.y + atlas.line + 9.0 * s;
    fill_window(&mut ui, atlas, left, top, pad, lines, empty);

    // Right window.
    let right = Rect {
        x: 2.0 * m + panel_w,
        y: panel_y,
        w: panel_w,
        h: panel_h,
    };
    frame_window(&mut ui, atlas, right);
    ui.text(
        atlas,
        right.x + pad,
        right.y + 5.0 * s,
        "PERSONAL · near · here · rooms · private · your agent",
        amber(Intensity::Half, 1.0),
    );
    let top = right.y + atlas.line + 9.0 * s;
    fill_window(
        &mut ui,
        atlas,
        right,
        top,
        pad,
        &f.log.personal,
        "Nearby talk, rooms, private messages, and your agent show up here. Press T to talk to your agent.",
    );
    layout.panels.push(left);
    layout.panels.push(right);

    // Channel pills.
    let mut x = m;
    ui.text(
        atlas,
        x,
        pill_y + 4.0 * s,
        "Send to:",
        amber(Intensity::Half, 1.0),
    );
    x += atlas.measure("Send to: ");
    for (label, selected) in &f.pills {
        let pw = atlas.measure(label) + 14.0 * s;
        let r = Rect {
            x,
            y: pill_y,
            w: pw,
            h: pill_h,
        };
        if *selected {
            ui.rect(atlas, r.x, r.y, r.w, r.h, amber(Intensity::Quarter, 0.55));
            ui.frame(
                atlas,
                r.x,
                r.y,
                r.w,
                r.h,
                s.max(1.0),
                amber(Intensity::Full, 1.0),
            );
        } else {
            ui.rect(atlas, r.x, r.y, r.w, r.h, field(0.8));
            ui.frame(
                atlas,
                r.x,
                r.y,
                r.w,
                r.h,
                1.0,
                amber(Intensity::Quarter, 1.0),
            );
        }
        let step = if *selected {
            Intensity::Full
        } else {
            Intensity::Half
        };
        ui.text(atlas, r.x + 7.0 * s, r.y + 4.0 * s, label, amber(step, 1.0));
        layout.pills.push(r);
        x += pw + 5.0 * s;
    }
    let tip = "Tab or click to switch";
    let tip_x = w - m - atlas.measure(tip);
    if tip_x > x + atlas.advance {
        ui.text(
            atlas,
            tip_x,
            pill_y + 4.0 * s,
            tip,
            amber(Intensity::Quarter, 1.0),
        );
    }
    layout.panels.push(Rect {
        x: m,
        y: pill_y,
        w: w - 2.0 * m,
        h: pill_h,
    });

    // The input line.
    let bar_w = w - 2.0 * m;
    layout.bar = Rect {
        x: m,
        y: bar_y,
        w: bar_w,
        h: bar,
    };
    let edge = if f.input.open {
        Intensity::Full
    } else {
        Intensity::Half
    };
    ui.rect(atlas, m, bar_y, bar_w, bar, field(0.9));
    ui.frame(atlas, m, bar_y, bar_w, bar, s.max(1.0), amber(edge, 1.0));
    let text_y = bar_y + (bar - atlas.line) / 2.0;
    let mut x = m + pad;
    x += ui.text(atlas, x, text_y, "> ", amber(Intensity::Full, 1.0));

    // Right side: who hears it, and the count when it matters.
    let count = f.input.text.chars().count();
    let right_text = match f.limit {
        Some(limit) if f.input.open => format!("to {} · {count}/{limit}", f.hint),
        _ => format!("to {}", f.hint),
    };
    let over = f.limit.is_some_and(|l| count > l);
    let right_x = m + bar_w - pad - atlas.measure(&right_text);
    ui.text(
        atlas,
        right_x,
        text_y,
        &right_text,
        amber(
            if over {
                Intensity::Full
            } else {
                Intensity::Quarter
            },
            1.0,
        ),
    );

    let room = (((right_x - atlas.advance * 2.0) - x) / atlas.advance).max(4.0) as usize;
    if f.input.open && !f.input.text.is_empty() {
        let chars: Vec<char> = f.input.text.chars().collect();
        let start = chars.len().saturating_sub(room.saturating_sub(1));
        let shown: String = chars[start..].iter().collect();
        x += ui.text(atlas, x, text_y, &shown, amber(Intensity::Full, 1.0));
    } else {
        let placeholder = if f.input.open {
            "Type your message · Enter sends · Esc cancels · / for shortcuts"
        } else {
            "Press Enter (or click here) to chat · T to talk to your agent · N for Nostr"
        };
        let clipped: String = placeholder.chars().take(room).collect();
        ui.text(
            atlas,
            x + atlas.advance,
            text_y,
            &clipped,
            amber(Intensity::Quarter, 1.0),
        );
    }
    if f.input.open && (f.time * 2.0).fract() < 0.6 {
        ui.text(atlas, x, text_y, "_", amber(Intensity::Full, 1.0));
    }

    // Shortcut suggestions while typing a slash command.
    if f.input.open && f.input.text.starts_with(['/', '!']) && !f.input.text.contains(' ') {
        let typed = f.input.text.to_lowercase();
        let matches: Vec<&(&str, &str)> = SHORTCUTS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(&typed) || typed.len() == 1)
            .collect();
        if !matches.is_empty() {
            let lw = SHORTCUTS
                .iter()
                .map(|(c, _)| atlas.measure(c))
                .fold(0.0, f32::max);
            let bw = matches
                .iter()
                .map(|(_, d)| lw + atlas.advance * 2.0 + atlas.measure(d))
                .fold(0.0, f32::max)
                + 2.0 * pad;
            let bh = matches.len() as f32 * atlas.line + 2.0 * pad;
            let by = pill_y - 4.0 * s - bh;
            ui.rect(atlas, m, by, bw, bh, field(0.95));
            ui.frame(
                atlas,
                m,
                by,
                bw,
                bh,
                s.max(1.0),
                amber(Intensity::ThreeQuarters, 1.0),
            );
            for (i, (cmd, desc)) in matches.iter().enumerate() {
                let ly = by + pad + i as f32 * atlas.line;
                ui.text(atlas, m + pad, ly, cmd, amber(Intensity::Full, 1.0));
                ui.text(
                    atlas,
                    m + pad + lw + atlas.advance * 2.0,
                    ly,
                    desc,
                    amber(Intensity::Half, 1.0),
                );
            }
        }
    }
    xp_strip(&mut ui, atlas, f, m);
    let replay_bottom = replay_strip(&mut ui, atlas, f, m);
    let top = m + (f.xp.len() as f32 + 0.5) * atlas.line + 12.0 * s;
    if let Some(lines) = &f.board {
        let heading = Heading {
            title: "QUEST BOARD · NIP-XP quests on the relay · read-only",
            close: "B or Esc closes",
            scroll: f.board_scroll,
        };
        let (panel, max) = board(&mut ui, atlas, f, &heading, lines, top, panel_y - 6.0 * s);
        layout.panels.push(panel);
        layout.board = Some((panel, max));
    }
    if let Some(lines) = &f.picker {
        let heading = Heading {
            title: "RUN REPLAYS · retained Microcoder runs that beat Fable 5.1 low",
            close: "R or Esc closes",
            scroll: f.picker_scroll,
        };
        let top = top.max(replay_bottom + 6.0 * s);
        let (panel, _) = board(&mut ui, atlas, f, &heading, lines, top, panel_y - 6.0 * s);
        layout.panels.push(panel);
    }
    (ui, layout)
}

/// A running replay's lines, top right. Returns the strip's bottom edge.
fn replay_strip(ui: &mut UiBatch, atlas: &Atlas, f: &Frame<'_>, m: f32) -> f32 {
    if f.replay.is_empty() {
        return m;
    }
    let s = f.scale;
    let pad = 6.0 * s;
    let limit = (f.size[0] * 0.5).max(atlas.advance * 30.0);
    let mut rows: Vec<(String, Intensity)> = Vec::new();
    for (text, step) in &f.replay {
        for row in atlas.wrap(text, limit) {
            rows.push((row, *step));
        }
    }
    let width = rows
        .iter()
        .map(|(t, _)| atlas.measure(t))
        .fold(0.0, f32::max);
    let h = rows.len() as f32 * atlas.line + 2.0 * pad;
    let x = f.size[0] - m - width - 2.0 * pad;
    ui.rect(atlas, x, m, width + 2.0 * pad, h, field(1.0));
    ui.frame(
        atlas,
        x,
        m,
        width + 2.0 * pad,
        h,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
    for (i, (text, step)) in rows.iter().enumerate() {
        ui.text(
            atlas,
            x + pad,
            m + pad + i as f32 * atlas.line,
            text,
            amber(*step, 1.0),
        );
    }
    m + h
}

/// A panel's heading, the key that closes it, and how far it is scrolled.
struct Heading<'a> {
    title: &'a str,
    close: &'a str,
    scroll: usize,
}

/// XP, level, titles, and the board key, top left.
fn xp_strip(ui: &mut UiBatch, atlas: &Atlas, f: &Frame<'_>, m: f32) {
    let s = f.scale;
    let width =
        f.xp.iter()
            .map(|(t, _)| atlas.measure(t))
            .fold(0.0, f32::max);
    if width <= 0.0 {
        return;
    }
    let pad = 6.0 * s;
    let h = f.xp.len() as f32 * atlas.line + 2.0 * pad;
    ui.rect(atlas, m, m, width + 2.0 * pad, h, field(0.9));
    for (i, (text, step)) in f.xp.iter().enumerate() {
        ui.text(
            atlas,
            m + pad,
            m + pad + i as f32 * atlas.line,
            text,
            amber(*step, 1.0),
        );
    }
}

/// A read-only panel, such as the quest board, centered between the XP
/// strip and the chat windows and scrolled down by `heading.scroll` rows.
/// Returns its rectangle and the furthest it can scroll.
fn board(
    ui: &mut UiBatch,
    atlas: &Atlas,
    f: &Frame<'_>,
    heading: &Heading<'_>,
    lines: &[(String, Intensity)],
    top: f32,
    bottom: f32,
) -> (Rect, usize) {
    let s = f.scale;
    let pad = 10.0 * s;
    let w = (f.size[0] * 0.62).clamp(atlas.advance * 40.0, atlas.advance * 110.0);
    let mut rows: Vec<(String, Intensity)> = Vec::new();
    for (text, step) in lines {
        for row in atlas.wrap(text, w - 2.0 * pad) {
            rows.push((row, *step));
        }
    }
    // Heading, rule, and padding above the rows; padding below.
    let chrome = pad * 0.6 + atlas.line + 9.0 * s + pad;
    let room = (bottom - top).max(atlas.line * 6.0);
    let fit = ((room - chrome) / atlas.line).floor().max(2.0) as usize;
    let mut max = 0;
    if rows.len() > fit {
        // One row goes to the scroll position.
        let shown = fit - 1;
        max = rows.len() - shown;
        let from = heading.scroll.min(max);
        let total = rows.len();
        rows = rows.into_iter().skip(from).take(shown).collect();
        rows.push((
            format!(
                "rows {}-{} of {total} · mouse wheel or Page Up and Page Down scroll",
                from + 1,
                from + shown
            ),
            Intensity::Quarter,
        ));
    }
    let r = Rect {
        x: (f.size[0] - w) / 2.0,
        y: top,
        w,
        h: (chrome + rows.len() as f32 * atlas.line).min(room),
    };
    ui.rect(atlas, r.x, r.y, r.w, r.h, field(1.0));
    ui.frame(
        atlas,
        r.x,
        r.y,
        r.w,
        r.h,
        s.max(1.0),
        amber(Intensity::Full, 1.0),
    );
    let heading_y = r.y + pad * 0.6;
    ui.text(
        atlas,
        r.x + pad,
        heading_y,
        heading.title,
        amber(Intensity::Full, 1.0),
    );
    let close = heading.close;
    ui.text(
        atlas,
        r.x + r.w - pad - atlas.measure(close),
        heading_y,
        close,
        amber(Intensity::Quarter, 1.0),
    );
    let rule = heading_y + atlas.line + 3.0 * s;
    ui.rect(
        atlas,
        r.x + pad,
        rule,
        r.w - 2.0 * pad,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
    let first = rule + 6.0 * s;
    for (i, (row, step)) in rows.iter().enumerate() {
        ui.text(
            atlas,
            r.x + pad,
            first + i as f32 * atlas.line,
            row,
            amber(*step, 1.0),
        );
    }
    (r, max)
}

fn frame_window(ui: &mut UiBatch, atlas: &Atlas, r: Rect) {
    ui.rect(atlas, r.x, r.y, r.w, r.h, field(0.78));
    ui.frame(
        atlas,
        r.x,
        r.y,
        r.w,
        r.h,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
}

fn fill_window(
    ui: &mut UiBatch,
    atlas: &Atlas,
    r: Rect,
    top: f32,
    pad: f32,
    lines: &VecDeque<Line>,
    empty: &str,
) {
    ui.rect(
        atlas,
        r.x + pad,
        top,
        r.w - 2.0 * pad,
        1.0,
        amber(Intensity::Quarter, 1.0),
    );
    let inner = r.w - 2.0 * pad;
    if lines.is_empty() {
        for (i, row) in atlas.wrap(empty, inner).iter().enumerate() {
            ui.text(
                atlas,
                r.x + pad,
                top + 6.0 + i as f32 * atlas.line,
                row,
                amber(Intensity::Quarter, 1.0),
            );
        }
        return;
    }
    let mut cursor = r.y + r.h - pad * 0.5;
    for line in lines.iter().rev() {
        let spans = spans(line);
        let rows = layout(atlas, &spans, inner);
        for row in rows.iter().rev() {
            cursor -= atlas.line;
            if cursor < top + 2.0 {
                return;
            }
            let mut px = r.x + pad;
            for (text, step) in row {
                px += ui.text(atlas, px, cursor, text, amber(*step, 1.0));
            }
        }
    }
}

/// A line as styled spans: channel tag, speaker, text, and note.
fn spans(line: &Line) -> Vec<(String, Intensity)> {
    let Some(channel) = &line.channel else {
        if line.from.is_empty() {
            return vec![(line.text.clone(), Intensity::Half)];
        }
        // A public Nostr note: speaker and text, the relay as the note.
        let mut out = vec![
            (format!("{}: ", line.from), Intensity::Full),
            (line.text.clone(), Intensity::ThreeQuarters),
        ];
        if let Some(note) = &line.note {
            out.push((format!(" {note}"), Intensity::Quarter));
        }
        return out;
    };
    let tag = match channel {
        Channel::All => "all ".to_owned(),
        Channel::Ads => "$ ".to_owned(),
        Channel::Zone => "zone ".to_owned(),
        Channel::Near => "near ".to_owned(),
        Channel::Here => "here ".to_owned(),
        Channel::Room(room) => format!("#{room} "),
        Channel::Pm(_) => "pm ".to_owned(),
        Channel::Agent => "ai ".to_owned(),
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
    let body = if matches!(channel, Channel::Ads | Channel::Pm(_) | Channel::Agent) {
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
    let Some([x, y]) = project(f.view_proj, f.size, o.feet + Vec3::Y * o.lift) else {
        return;
    };
    let mut y = y;
    if let Some(name) = &o.name {
        let tw = atlas.measure(name);
        ui.text(
            atlas,
            x - tw / 2.0,
            y - atlas.line,
            name,
            amber(o.name_step, 1.0),
        );
        y -= atlas.line + 4.0 * s;
    }
    let Some(text) = &o.bubble else { return };
    let mut rows = atlas.wrap(text, atlas.advance * 30.0);
    if rows.len() > 4 {
        rows.truncate(4);
        if let Some(last) = rows.last_mut() {
            let keep: String = last.chars().take(28).collect();
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

/// A channel pill's label.
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
        Channel::Agent => "AGENT".into(),
    }
}

/// Who hears a channel, in words, for the input line.
#[must_use]
pub fn audience(
    channel: &Channel,
    world: &str,
    zone: &str,
    near: usize,
    here: usize,
    name_of: impl Fn(&str) -> String,
) -> String {
    match channel {
        Channel::All => format!("everyone in {world}"),
        Channel::Ads => "everyone, as an ad".into(),
        Channel::Zone => format!("everyone in the {}", chat::zone_name(zone)),
        Channel::Near => format!("{near} players within 40 m"),
        Channel::Here => format!("{here} players on this spot"),
        Channel::Room(room) => format!("the #{room} room"),
        Channel::Pm(pubkey) => format!("only {}", name_of(pubkey)),
        Channel::Agent => "only your agent (private)".into(),
    }
}

/// The WORLD tab's heading after its name.
#[must_use]
pub fn world_title(world: &str, pos: Vec3) -> String {
    format!(
        "{world} · {} · all · ads · zone",
        chat::zone_name(chat::zone_of(pos))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame<'a>(log: &'a Log, nostr: &'a VecDeque<Line>, input: &'a Input) -> Frame<'a> {
        Frame {
            size: [1440.0, 900.0],
            scale: 1.0,
            view_proj: Mat4::IDENTITY,
            log,
            nostr,
            nostr_title: "damus · primal".into(),
            left_tab: LeftTab::World,
            input,
            pills: vec![("ALL".into(), true), ("AGENT".into(), false)],
            hint: "everyone".into(),
            limit: Some(150),
            world_title: world_title("verse-plaza", Vec3::ZERO),
            overheads: &[],
            time: 0.0,
            xp: vec![("XP 6 · level 1".into(), Intensity::Full)],
            board: None,
            board_scroll: 0,
            replay: Vec::new(),
            picker: None,
            picker_scroll: 0,
        }
    }

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
    fn pills_tabs_and_the_bar_are_clickable() {
        let atlas = Atlas::new(14.0);
        let log = Log::default();
        let nostr = VecDeque::new();
        let input = Input::default();
        let (ui, layout) = build(&atlas, &frame(&log, &nostr, &input));
        assert!(!ui.vertices.is_empty());
        assert_eq!(layout.pills.len(), 2);
        let p = layout.pills[1];
        assert!(layout.owns(p.x + 1.0, p.y + 1.0));
        let t = layout.tabs[1];
        assert!(t.w > 0.0 && layout.owns(t.x + 1.0, t.y + 1.0));
        assert!(layout.bar.contains(layout.bar.x + 5.0, layout.bar.y + 5.0));
        assert!(!layout.owns(700.0, 100.0), "the sky is not UI");
    }

    #[test]
    fn a_slash_lists_the_shortcuts() {
        let atlas = Atlas::new(14.0);
        let log = Log::default();
        let nostr = VecDeque::new();
        let closed = Input::default();
        let typing = Input {
            open: true,
            text: "/".into(),
            last: String::new(),
        };
        let (plain, _) = build(&atlas, &frame(&log, &nostr, &closed));
        let (listed, _) = build(&atlas, &frame(&log, &nostr, &typing));
        assert!(listed.vertices.len() > plain.vertices.len() + 200);
    }

    #[test]
    fn the_open_board_is_a_clickable_panel() {
        let atlas = Atlas::new(14.0);
        let log = Log::default();
        let nostr = VecDeque::new();
        let input = Input::default();
        let (closed, closed_layout) = build(&atlas, &frame(&log, &nostr, &input));
        let mut open = frame(&log, &nostr, &input);
        open.board = Some(vec![("A quest".into(), Intensity::Full); 200]);
        let (drawn, layout) = build(&atlas, &open);
        assert!(drawn.vertices.len() > closed.vertices.len());
        assert_eq!(layout.panels.len(), closed_layout.panels.len() + 1);
        assert!(layout.owns(720.0, 200.0), "the board takes clicks");
        let (_, max) = layout.board.expect("the board is open");
        assert!(max > 0, "200 rows don't fit, so the board scrolls");
        open.board_scroll = max + 50;
        let (scrolled, _) = build(&atlas, &open);
        assert!(!scrolled.vertices.is_empty());
    }

    #[test]
    fn a_replay_shows_top_right_and_its_list_is_a_panel() {
        let atlas = Atlas::new(14.0);
        let log = Log::default();
        let nostr = VecDeque::new();
        let input = Input::default();
        let (plain, plain_layout) = build(&atlas, &frame(&log, &nostr, &input));
        let mut f = frame(&log, &nostr, &input);
        f.replay = vec![("REPLAY · task · 10× · playing".into(), Intensity::Full)];
        let (drawn, layout) = build(&atlas, &f);
        assert!(drawn.vertices.len() > plain.vertices.len());
        assert_eq!(
            layout.panels.len(),
            plain_layout.panels.len(),
            "the strip takes no clicks"
        );
        f.picker = Some(vec![("> a run".into(), Intensity::Full); 3]);
        let (_, layout) = build(&atlas, &f);
        assert_eq!(layout.panels.len(), plain_layout.panels.len() + 1);
        assert!(layout.board.is_none(), "the list is not the quest board");
    }
}
