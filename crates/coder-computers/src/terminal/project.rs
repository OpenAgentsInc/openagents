//! The terminal screen as a Rust Native tree in Coder's amber palette.
//!
//! The root stack has three children a platform can place separately:
//!
//! - `terminal-header`: the host, the session's status, gap and refusal
//!   notices, and **Back**, **End terminal**, and **Open a new terminal**.
//! - `terminal-grid`: one node per grid row. A row whose cells share one
//!   look is one `terminal` text node; otherwise it is a horizontal stack of
//!   runs. The platform draws each run monospaced on one line, so the grid
//!   is exactly as wide as the columns it reported.
//! - `terminal-keys`: the accessory row: Esc, Tab, a latching Ctrl, the
//!   arrows, Ctrl-C, and Paste.
//!
//! Colors map onto the single amber hue: brightness carries emphasis, as in
//! every Coder surface. Output text is shown as data; nothing in it becomes
//! a control.

use super::model::{Model, Phase};
use coder_ui::theme::{Intensity, NEAR_BLACK, NEAR_BLACK_TINT};
use coder_vt::{Attrs, Color as VtColor, Flags, Key};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::view::MAX_NODES;
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

/// A key on the accessory row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessoryKey {
    Escape,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
}

impl AccessoryKey {
    #[must_use]
    pub fn key(self) -> Key {
        match self {
            AccessoryKey::Escape => Key::Escape,
            AccessoryKey::Tab => Key::Tab,
            AccessoryKey::Up => Key::Up,
            AccessoryKey::Down => Key::Down,
            AccessoryKey::Left => Key::Left,
            AccessoryKey::Right => Key::Right,
            AccessoryKey::Home => Key::Home,
            AccessoryKey::End => Key::End,
            AccessoryKey::PageUp => Key::PageUp,
            AccessoryKey::PageDown => Key::PageDown,
        }
    }
}

/// What a terminal screen control asks for. It grants nothing: the host
/// checks the `terminal` right on every request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TerminalIntent {
    Key {
        key: AccessoryKey,
    },
    /// Latch Ctrl for the next key, or release it.
    Ctrl,
    /// Send Ctrl-C.
    Interrupt,
    /// Ask the platform for the clipboard's text, to paste.
    Paste,
    /// Detach and return to the Computers screen. The shell keeps running.
    Leave,
    /// End the shell's process tree on the host.
    Close,
    /// Open a new terminal after this one ended.
    Reopen,
}

/// Nodes kept for everything except the grid rows.
const CHROME_NODES: usize = 48;

fn amber(intensity: Intensity) -> Color {
    rgb(intensity.color())
}

fn rgb(value: u32) -> Color {
    Color::rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// A color's brightness, 0 to 255, for placing it on the amber ladder.
fn luminance(color: VtColor) -> Option<u32> {
    let (r, g, b) = match color {
        VtColor::Default => return None,
        VtColor::Indexed(index) => indexed(index),
        VtColor::Rgb(r, g, b) => (r, g, b),
    };
    Some((299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000)
}

/// The xterm RGB value of an indexed color.
fn indexed(index: u8) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => BASE[usize::from(index)],
        16..=231 => {
            let n = index - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            (level(n / 36), level(n / 6 % 6), level(n % 6))
        }
        _ => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

/// The step of the ladder a foreground color takes. Every standard color
/// is readable: red, the color of errors, burns brightest.
fn foreground(color: VtColor) -> Intensity {
    match color {
        VtColor::Default => Intensity::ThreeQuarters,
        VtColor::Indexed(0) => Intensity::Half,
        VtColor::Indexed(1 | 9) => Intensity::Full,
        VtColor::Indexed(8) => Intensity::Half,
        VtColor::Indexed(10..=15) => Intensity::Full,
        VtColor::Indexed(2..=7) => Intensity::ThreeQuarters,
        other => match luminance(other).unwrap_or(192) {
            0..96 => Intensity::Half,
            96..200 => Intensity::ThreeQuarters,
            _ => Intensity::Full,
        },
    }
}

fn dimmer(intensity: Intensity) -> Intensity {
    match intensity {
        Intensity::Full => Intensity::ThreeQuarters,
        Intensity::ThreeQuarters => Intensity::Half,
        _ => Intensity::Quarter,
    }
}

/// A background color's fill: dark ones tint the field, light ones take a
/// dim amber so text stays readable over them.
fn background(color: VtColor) -> Option<Color> {
    let lum = luminance(color)?;
    Some(if lum < 64 {
        rgb(NEAR_BLACK_TINT)
    } else {
        amber(Intensity::Quarter)
    })
}

/// The style of a cell, with the cursor drawn as an inverse block.
fn style(attrs: Attrs, cursor: bool) -> Style {
    let flags = attrs.flags;
    if flags.contains(Flags::MARKER) {
        return Style {
            foreground: Some(amber(Intensity::Full)),
            background: Some(rgb(NEAR_BLACK_TINT)),
            weight: Some(TextWeight::Bold),
            ..Style::default()
        };
    }
    let mut intensity = foreground(attrs.fg);
    if flags.contains(Flags::BOLD) {
        intensity = Intensity::Full;
    }
    if flags.contains(Flags::DIM) {
        intensity = dimmer(intensity);
    }
    let mut fg = amber(intensity);
    let mut bg = background(attrs.bg);
    if flags.contains(Flags::INVERSE) != cursor {
        let back = bg.unwrap_or(rgb(NEAR_BLACK));
        bg = Some(fg);
        fg = back;
    }
    if flags.contains(Flags::HIDDEN) {
        fg = bg.unwrap_or(rgb(NEAR_BLACK));
    }
    Style {
        foreground: Some(fg),
        background: bg,
        weight: flags.contains(Flags::BOLD).then_some(TextWeight::Bold),
        ..Style::default()
    }
}

fn text(key: impl Into<String>, value: impl Into<String>, role: TextRole) -> Node<TerminalIntent> {
    let intensity = match role {
        TextRole::Heading => Intensity::Full,
        TextRole::Status => Intensity::Half,
        _ => Intensity::ThreeQuarters,
    };
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(amber(intensity)),
            weight: (role == TextRole::Heading).then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn button(key: &str, label: &str, intent: TerminalIntent, enabled: bool) -> Node<TerminalIntent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(amber(if enabled {
                Intensity::Full
            } else {
                Intensity::Quarter
            })),
            ..Style::default()
        },
        element: Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            intent,
        },
    }
}

fn stack(
    key: impl Into<String>,
    axis: Axis,
    gap: Space,
    children: Vec<Node<TerminalIntent>>,
) -> Node<TerminalIntent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(gap),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}

/// One row's runs: text and the style it draws in.
type Runs = Vec<(String, Style)>;

/// A row's runs by resolved style, so colors that map to the same amber
/// merge. `cursor` is the column to draw the cursor at.
fn styled_runs(row: &coder_vt::Row, cursor: Option<usize>) -> Runs {
    let mut runs: Runs = Vec::new();
    for (col, cell) in row.cells.iter().enumerate() {
        let look = style(cell.attrs, cursor == Some(col));
        let mut piece = String::new();
        if cell.width != 0 {
            piece.push(cell.ch);
            piece.extend(cell.combining.iter());
        }
        match runs.last_mut() {
            Some((text, current)) if *current == look || cell.width == 0 => text.push_str(&piece),
            _ => runs.push((piece, look)),
        }
    }
    runs
}

/// A row flattened to the default look, keeping only the cursor.
fn plain_runs(row: &coder_vt::Row, cursor: Option<usize>) -> Runs {
    let plain = style(Attrs::default(), false);
    let mut runs: Runs = vec![(String::new(), plain)];
    for (col, cell) in row.cells.iter().enumerate() {
        if cell.width == 0 {
            continue;
        }
        let mut piece = String::from(cell.ch);
        piece.extend(cell.combining.iter());
        if cursor == Some(col) {
            runs.push((piece, style(Attrs::default(), true)));
            runs.push((String::new(), plain));
        } else if let Some((text, _)) = runs.last_mut() {
            text.push_str(&piece);
        }
    }
    runs.retain(|(text, _)| !text.is_empty());
    if runs.is_empty() {
        runs.push((" ".into(), plain));
    }
    runs
}

/// Drops blanks at the end of a row that draw nothing: spaces with no
/// background. A row keeps at least one space, so it keeps its height.
fn trim(mut runs: Runs) -> Runs {
    while let Some((text, look)) = runs.last_mut() {
        if look.background.is_some() {
            break;
        }
        let kept = text.trim_end_matches(' ').len();
        text.truncate(kept);
        if !text.is_empty() {
            break;
        }
        runs.pop();
    }
    if runs.is_empty() {
        runs.push((" ".into(), style(Attrs::default(), false)));
    }
    runs
}

fn row_nodes(runs: &Runs) -> usize {
    if runs.len() <= 1 { 1 } else { 1 + runs.len() }
}

/// The grid rows, within the view's node bound: when styled runs would
/// exceed it, the rows with the most runs are drawn plain.
fn grid(model: &Model) -> Vec<Node<TerminalIntent>> {
    let vt = &model.vt;
    let (cursor_row, cursor_col) = vt.cursor();
    let show_cursor = model.phase == Phase::Attached && vt.cursor_visible();
    let cursor = |index: usize| (show_cursor && index == cursor_row).then_some(cursor_col);
    let mut rows: Vec<Runs> = vt
        .screen()
        .iter()
        .enumerate()
        .map(|(index, row)| trim(styled_runs(row, cursor(index))))
        .collect();
    let budget = MAX_NODES.saturating_sub(CHROME_NODES + 1);
    let mut total: usize = rows.iter().map(row_nodes).sum();
    while total > budget {
        let Some((index, _)) = rows
            .iter()
            .enumerate()
            .filter(|(_, runs)| runs.len() > 3)
            .max_by_key(|(_, runs)| runs.len())
        else {
            break;
        };
        total -= row_nodes(&rows[index]);
        rows[index] = trim(plain_runs(&vt.screen()[index], cursor(index)));
        total += row_nodes(&rows[index]);
    }
    rows.into_iter()
        .enumerate()
        .map(|(index, runs)| {
            let key = format!("terminal-row-{index}");
            let mut nodes: Vec<Node<TerminalIntent>> = runs
                .into_iter()
                .enumerate()
                .map(|(part, (value, look))| Node {
                    key: format!("{key}-{part}"),
                    style: look,
                    element: Element::Text {
                        value,
                        role: TextRole::Terminal,
                    },
                })
                .collect();
            if nodes.len() == 1 {
                let mut node = nodes.remove(0);
                node.key = key;
                node
            } else {
                stack(key, Axis::Horizontal, Space::None, nodes)
            }
        })
        .collect()
}

/// The terminal screen for one revision.
#[must_use]
pub fn view(model: &Model, instance: &str, revision: u64) -> View<TerminalIntent> {
    let attached = model.phase == Phase::Attached;
    let mut header = vec![
        text(
            "terminal-title",
            format!("Terminal on {}", model.label),
            TextRole::Heading,
        ),
        text("terminal-status", status(model), TextRole::Status),
    ];
    if model.gaps > 0 {
        let (bytes, unknown) = model.missed;
        let detail = if unknown {
            "some output".to_owned()
        } else {
            format!("{bytes} bytes")
        };
        header.push(text(
            "terminal-gaps",
            format!(
                "The computer discarded {detail} before this screen read it. Marked in the output."
            ),
            TextRole::Status,
        ));
    }
    if let Some(notice) = &model.notice {
        header.push(text("terminal-notice", notice, TextRole::Status));
    }
    let mut actions = vec![button(
        "terminal-close",
        "Back",
        TerminalIntent::Leave,
        true,
    )];
    if model.phase.ended() {
        if model.phase != Phase::Left {
            actions.push(button(
                "terminal-reopen",
                "Open a new terminal",
                TerminalIntent::Reopen,
                true,
            ));
        }
    } else {
        actions.push(button(
            "terminal-end",
            "End terminal",
            TerminalIntent::Close,
            attached,
        ));
    }
    header.push(stack(
        "terminal-actions",
        Axis::Horizontal,
        Space::Md,
        actions,
    ));

    let mut grid_node = stack("terminal-grid", Axis::Vertical, Space::None, grid(model));
    grid_node.style.background = Some(rgb(NEAR_BLACK));

    let key = |key: AccessoryKey, id: &str, label: &str| {
        button(id, label, TerminalIntent::Key { key }, attached)
    };
    let keys = vec![
        key(AccessoryKey::Escape, "terminal-key-escape", "Esc"),
        key(AccessoryKey::Tab, "terminal-key-tab", "Tab"),
        button(
            "terminal-key-ctrl",
            if model.ctrl { "Ctrl on" } else { "Ctrl" },
            TerminalIntent::Ctrl,
            attached,
        ),
        key(AccessoryKey::Left, "terminal-key-left", "Left"),
        key(AccessoryKey::Up, "terminal-key-up", "Up"),
        key(AccessoryKey::Down, "terminal-key-down", "Down"),
        key(AccessoryKey::Right, "terminal-key-right", "Right"),
        button(
            "terminal-key-interrupt",
            "^C",
            TerminalIntent::Interrupt,
            attached,
        ),
        button(
            "terminal-key-paste",
            "Paste",
            TerminalIntent::Paste,
            attached,
        ),
    ];

    let root = stack(
        "terminal",
        Axis::Vertical,
        Space::Sm,
        vec![
            stack("terminal-header", Axis::Vertical, Space::Xs, header),
            grid_node,
            stack("terminal-keys", Axis::Horizontal, Space::Sm, keys),
        ],
    );
    View::new(instance, revision, root)
}

fn status(model: &Model) -> String {
    let mut line = model.phase.describe();
    if model.phase == Phase::Attached {
        if let Some(route) = &model.route {
            line = format!("Connected {route}");
        }
        let title = model.vt.title();
        if !title.is_empty() {
            line = format!("{line} · {title}");
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;

    fn attached(rows: u16, cols: u16) -> Model {
        let mut model = Model::new("ab".repeat(32), "Studio Mac", rows, cols);
        model.phase = Phase::Attached;
        model
    }

    fn texts(node: &Node<TerminalIntent>, out: &mut Vec<(String, String)>) {
        match &node.element {
            Element::Text { value, .. } => out.push((node.key.clone(), value.clone())),
            Element::Stack { children, .. } | Element::List { children, .. } => {
                children.iter().for_each(|child| texts(child, out));
            }
            _ => {}
        }
    }

    fn find<'a>(node: &'a Node<TerminalIntent>, key: &str) -> Option<&'a Node<TerminalIntent>> {
        if node.key == key {
            return Some(node);
        }
        match &node.element {
            Element::Stack { children, .. } | Element::List { children, .. } => {
                children.iter().find_map(|child| find(child, key))
            }
            _ => None,
        }
    }

    fn row_text(view: &View<TerminalIntent>, row: usize) -> String {
        let node = find(&view.root, &format!("terminal-row-{row}")).unwrap();
        let mut parts = Vec::new();
        texts(node, &mut parts);
        parts.into_iter().map(|(_, value)| value).collect()
    }

    #[test]
    fn the_grid_has_one_row_node_per_row_without_trailing_blanks() {
        let mut model = attached(4, 12);
        model.vt.feed(b"$ echo hi\r\nhi\r\n$ ");
        let view = view(&model, "terminal:1", 1).validate().unwrap();
        let view = view.view();
        assert_eq!(row_text(view, 0), "$ echo hi");
        assert_eq!(row_text(view, 1), "hi");
        // An empty row keeps one space, so it keeps its height.
        assert_eq!(row_text(view, 3), " ");
        // The cursor splits the prompt row into a run of its own.
        let prompt = find(&view.root, "terminal-row-2").unwrap();
        let Element::Stack { children, .. } = &prompt.element else {
            panic!("the cursor row is a stack of runs");
        };
        assert_eq!(children.len(), 2);
        let Element::Text { value, role } = &children[1].element else {
            panic!("a run is text")
        };
        assert_eq!((value.as_str(), *role), (" ", TextRole::Terminal));
        // Inverse: near-black text on amber.
        assert_eq!(children[1].style.foreground, Some(rgb(NEAR_BLACK)));
        // A colored background keeps its blanks.
        let mut model = attached(2, 8);
        model.vt.feed(b"\x1b[?25l\x1b[44mab  \x1b[0m");
        let view = super::view(&model, "terminal:1", 1);
        assert_eq!(row_text(&view, 0), "ab  ");
    }

    #[test]
    fn colors_map_onto_the_amber_ladder() {
        let mut model = attached(2, 20);
        model
            .vt
            .feed(b"\x1b[?25l\x1b[31merr\x1b[0m ok \x1b[1;32mbold\x1b[0m\x1b[2mdim");
        let view = view(&model, "terminal:1", 1);
        let row = find(&view.root, "terminal-row-0").unwrap();
        let Element::Stack { children, .. } = &row.element else {
            panic!()
        };
        let looks: Vec<(String, Option<Color>, Option<TextWeight>)> = children
            .iter()
            .map(|child| {
                let Element::Text { value, .. } = &child.element else {
                    panic!()
                };
                (value.clone(), child.style.foreground, child.style.weight)
            })
            .collect();
        assert_eq!(looks[0], ("err".into(), Some(amber(Intensity::Full)), None));
        assert_eq!(
            looks[1],
            (" ok ".into(), Some(amber(Intensity::ThreeQuarters)), None)
        );
        assert_eq!(
            looks[2],
            (
                "bold".into(),
                Some(amber(Intensity::Full)),
                Some(TextWeight::Bold)
            )
        );
        assert_eq!(looks[3].1, Some(amber(Intensity::Half)));
        // Every color in the tree is on the amber ladder or the dark field.
        let allowed: Vec<Color> = Intensity::ALL
            .iter()
            .map(|i| amber(*i))
            .chain([rgb(NEAR_BLACK), rgb(NEAR_BLACK_TINT)])
            .collect();
        fn colors(node: &Node<TerminalIntent>, out: &mut Vec<Color>) {
            out.extend(node.style.foreground);
            out.extend(node.style.background);
            if let Element::Stack { children, .. } = &node.element {
                children.iter().for_each(|child| colors(child, out));
            }
        }
        let mut used = Vec::new();
        colors(&view.root, &mut used);
        assert!(used.iter().all(|color| allowed.contains(color)));
    }

    #[test]
    fn a_busy_screen_stays_within_the_node_bound() {
        let mut model = attached(80, 240);
        // Every cell a different color: far more runs than the bound allows.
        for row in 0..80 {
            for col in 0..240u32 {
                let code = if col % 2 == 0 { 31 } else { 2 };
                model
                    .vt
                    .feed(format!("\x1b[{};{}H\x1b[{code}mx", row + 1, col + 1).as_bytes());
            }
        }
        let view = view(&model, "terminal:1", 1)
            .validate()
            .expect("the view stays within Rust Native's bounds");
        let mut parts = Vec::new();
        texts(&view.view().root, &mut parts);
        assert!(parts.len() <= MAX_NODES);
        assert_eq!(row_text(view.view(), 0).chars().count(), 240);
    }

    #[test]
    fn controls_follow_the_phase() {
        let mut model = attached(3, 20);
        let current = view(&model, "terminal:1", 1).validate().unwrap();
        let press = |view: &rust_native::ValidatedView<TerminalIntent>, node: &str| {
            view.activate(&Activation {
                instance: "terminal:1".into(),
                revision: view.view().revision,
                node: node.into(),
            })
            .cloned()
        };
        assert_eq!(
            press(&current, "terminal-key-up"),
            Ok(TerminalIntent::Key {
                key: AccessoryKey::Up
            })
        );
        assert_eq!(press(&current, "terminal-end"), Ok(TerminalIntent::Close));
        assert!(find(&current.view().root, "terminal-reopen").is_none());

        model.phase = Phase::Lost;
        let ended = view(&model, "terminal:1", 2).validate().unwrap();
        assert!(press(&ended, "terminal-key-up").is_err());
        assert_eq!(press(&ended, "terminal-reopen"), Ok(TerminalIntent::Reopen));
        let mut parts = Vec::new();
        texts(&ended.view().root, &mut parts);
        let status = parts
            .iter()
            .find(|(key, _)| key == "terminal-status")
            .unwrap();
        assert!(status.1.starts_with("Lost"));
        // No cursor is drawn once the session ended.
        assert!(matches!(
            find(&ended.view().root, "terminal-row-0").unwrap().element,
            Element::Text { .. }
        ));
    }

    #[test]
    fn a_gap_shows_in_the_header_and_the_grid() {
        let mut model = attached(4, 50);
        model.gap(Some(900));
        let view = view(&model, "terminal:1", 1);
        let mut parts = Vec::new();
        texts(&view.root, &mut parts);
        assert!(
            parts
                .iter()
                .any(|(key, value)| key == "terminal-gaps" && value.contains("900 bytes"))
        );
        assert!(row_text(&view, 0).starts_with("[output lost: 900 bytes"));
        let marker = find(&view.root, "terminal-row-0").unwrap();
        let first = match &marker.element {
            Element::Stack { children, .. } => &children[0],
            _ => marker,
        };
        assert_eq!(first.style.weight, Some(TextWeight::Bold));
    }
}
