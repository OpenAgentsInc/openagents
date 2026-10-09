//! The command palette, chat menu, profile menu, and archive confirmation as
//! one Rust Native view, shared by every window that opens [`commands`].
//!
//! Reimplemented from Zeron's palette and menus (`zeronsh/zeron` `50cf9e97`)
//! for the desktop app (#10029) and moved here from `openagents-desktop`
//! (#10466) so that Verse's Agent Studio draws the same overlay. The caller
//! owns the overlay state and the drawing surfaces; this module owns the
//! layout metrics, the row projection, and the surface sizes.
//!
//! [`commands`]: crate::commands

use crate::commands::{Action, Entry, Kind};
use crate::visual;
use oa_tokens::typography::menu::{DETAIL, HINT, ROW};
use rust_native::layout::display::{Font, FontFamily, Weight};
use rust_native::style::{ButtonDetail, Color, Space, Style, TextAlign, TextWeight, Viewport};
use rust_native::{Axis, Element, Glyph, Icon, Node, TextRole};
use std::collections::BTreeMap;

/// Zeron's palette: at most 30 conversations after filtering, 8-point list
/// padding, 2-point row gaps, and the section rule's 15 points.
pub const HISTORY_LIMIT: usize = 30;
pub const PAD: f32 = 8.0;
pub const GAP: f32 = 2.0;
pub const SEPARATOR: f32 = 15.0;
pub const ACTION_ROW: f32 = 32.0;
pub const HISTORY_ROW: f32 = 50.0;
pub const FADE: u16 = 18;

/// The palette's search glyph surface.
pub const SEARCH_GLYPH: &str = "glyph:command-search";
/// The palette's own shortcut badge surface.
pub const SHORTCUT_GLYPH: &str = "glyph:command-shortcut";
/// The rule between palette actions and history.
pub const RULE: &str = "glyph:command-rule";
/// The rule under the palette's search field; its painted rectangle is the
/// top of the results band.
pub const RULE_HEADER: &str = "glyph:command-rule-header";
/// The rule above the palette's key hints; its painted rectangle is the
/// bottom of the results band.
pub const RULE_FOOTER: &str = "glyph:command-rule-footer";
/// The palette's search field placeholder.
pub const QUERY_PLACEHOLDER: &str = "Search commands and chats…";

/// The fill of every command rule, in the scheme the app paints with
/// (`visual::current`): a faint wash of the scheme's ink.
#[must_use]
pub fn rule_color() -> Color {
    let look = visual::current();
    match look.scheme {
        visual::Scheme::Dark => Color {
            alpha: 15,
            ..look.ink
        },
        visual::Scheme::Light => look.border,
    }
}
/// The fill behind a key cap and the palette's shortcut badge, in the
/// scheme the app paints with.
#[must_use]
pub fn keycap_fill() -> Color {
    let look = visual::current();
    match look.scheme {
        visual::Scheme::Dark => Color {
            alpha: 13,
            ..look.ink
        },
        visual::Scheme::Light => look.selected,
    }
}
/// The corner radius of a key cap, in points.
pub const KEYCAP_RADIUS: f32 = 5.0;
/// The line height of the badge's text, in points.
pub const KEYCAP_LINE_HEIGHT: f32 = 14.0;
fn panel_background() -> Color {
    visual::current().panel
}

/// The open overlay's entries from a registry. The palette keeps every
/// action and at most [`HISTORY_LIMIT`] matching conversations, limited
/// after filtering so each chat remains searchable.
pub fn limit(kind: Option<&Kind>, mut entries: Vec<Entry>) -> Vec<Entry> {
    if kind == Some(&Kind::Palette) {
        let mut history = 0;
        entries.retain(|entry| {
            if matches!(entry.action, Action::Switch(_)) {
                history += 1;
                history <= HISTORY_LIMIT
            } else {
                true
            }
        });
    }
    entries
}

/// The palette results' greatest height at a window height, in points.
pub fn results_height(window_height: f32) -> f32 {
    (window_height - 180.0).clamp(100.0, 360.0)
}

/// Each palette row's scroll extent (a first history row includes the
/// section rule; the end rows include the list's padding) and the content
/// height, in points. Matches the laid-out rows exactly.
pub fn extents(entries: &[Entry], is_history: impl Fn(&Entry) -> bool) -> (Vec<(f32, f32)>, f32) {
    let mut extents = Vec::with_capacity(entries.len());
    let (mut y, mut actions, mut history) = (PAD, false, false);
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            y += GAP;
        }
        let top = if index == 0 { 0.0 } else { y };
        let row_history = is_history(entry);
        if row_history && !history && actions {
            y += SEPARATOR + GAP;
        }
        history |= row_history;
        actions |= !row_history;
        y += if row_history { HISTORY_ROW } else { ACTION_ROW };
        let bottom = if index + 1 == entries.len() {
            y + PAD
        } else {
            y
        };
        extents.push((top, bottom));
    }
    (extents, y + PAD)
}

/// The results offset after a wheel of `dy` points, or `None` when it does
/// not move. The offset stays within the content.
pub fn wheel(offset: f32, dy: f32, full: f32, visible: f32) -> Option<f32> {
    if !dy.is_finite() {
        return None;
    }
    let next = (offset - dy).clamp(0.0, (full - visible).max(0.0));
    (next != offset).then_some(next)
}

/// The results offset that keeps a row in view when `reveal` is set, clamped
/// to the content. Hover never sets `reveal`: revealing on hover moves
/// another row under the same pointer.
pub fn scroll(
    extents: &[(f32, f32)],
    full: f32,
    visible: f32,
    selected: usize,
    offset: f32,
    reveal: bool,
) -> f32 {
    let visible = full.min(visible);
    let mut offset = offset;
    if reveal && let Some(&(top, bottom)) = extents.get(selected) {
        if top < offset {
            offset = top;
        } else if bottom > offset + visible {
            offset = bottom - visible;
        }
    }
    offset.clamp(0.0, (full - visible).max(0.0))
}

/// The overlay's width, in points. The profile menu matches the sidebar it
/// opens above.
pub fn width(kind: &Kind, sidebar_width: f32) -> u16 {
    match kind {
        Kind::Palette => 560,
        Kind::Menu => 216,
        Kind::ConfirmArchive => 360,
        Kind::Profile => (sidebar_width - 16.0).round() as u16,
    }
}

/// Whether the overlay dims the window beneath it. Menus do not.
pub fn scrim(kind: &Kind) -> Option<Color> {
    (!matches!(kind, Kind::Menu | Kind::Profile)).then_some(Color {
        alpha: 89,
        ..Color::rgb(0, 0, 0)
    })
}

/// The size of one of this module's surfaces in `available` points of width.
pub fn surface_size(resource: &str, available: f32, macos: bool) -> Option<(f32, f32)> {
    match resource {
        SEARCH_GLYPH => Some((16.0, 16.0)),
        SHORTCUT_GLYPH => Some((if macos { 22.0 } else { 46.0 }, 16.0)),
        RULE | RULE_HEADER | RULE_FOOTER => Some((available, 1.0)),
        _ => None,
    }
}

/// Whether a resource is one of this module's static surfaces.
pub fn is_surface(resource: &str) -> bool {
    matches!(
        resource,
        SEARCH_GLYPH | SHORTCUT_GLYPH | RULE | RULE_HEADER | RULE_FOOTER
    )
}

/// The palette badge's text runs: Cmd+K on macOS, Ctrl+K elsewhere, in
/// Paper Mono, which has the Command sign.
pub fn shortcut_parts(macos: bool) -> Vec<(&'static str, Font)> {
    let font = Font {
        size: 10.0,
        weight: Weight::Regular,
        family: FontFamily::PaperMono,
        mono: true,
        italic: false,
    };
    if macos {
        vec![("⌘K", font)]
    } else {
        vec![("Ctrl+K", font)]
    }
}

/// The glyph a command row shows.
pub fn glyph(action: &Action) -> Glyph {
    match action {
        Action::NewChat => Glyph::Compose,
        Action::Search => Glyph::Search,
        Action::Settings => Glyph::Settings,
        Action::Computers => Glyph::Computer,
        Action::Grid => Glyph::Cloud,
        Action::Map => Glyph::Map,
        Action::Saved => Glyph::History,
        Action::Palette => Glyph::Terminal,
        Action::Stop => Glyph::Stop,
        Action::Rename => Glyph::Edit,
        Action::Pin => Glyph::Pin,
        Action::Archive => Glyph::Archive,
        Action::Restore => Glyph::Restore,
        Action::Feedback => Glyph::Flag,
        Action::Switch(_) => Glyph::Ask,
        _ => Glyph::More,
    }
}

/// A row's label: the chat menu uses Zeron's short verbs.
pub fn label<'a>(kind: &Kind, entry: &'a Entry) -> &'a str {
    if *kind != Kind::Menu {
        return &entry.label;
    }
    match entry.action {
        Action::Rename => "Rename…",
        Action::Pin if entry.label.starts_with("Unpin") => "Unpin",
        Action::Pin => "Pin",
        Action::Archive => "Archive",
        Action::Restore => "Unarchive",
        _ => &entry.label,
    }
}

/// The two lines a palette history row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    pub title: String,
    pub detail: String,
}

/// One frame of an open overlay, as its owner holds it.
pub struct Panel<'a> {
    pub kind: Kind,
    /// The entries [`limit`] returned.
    pub entries: Vec<Entry>,
    pub selected: usize,
    /// Whether menu rows highlight the selection: after a key or the pointer
    /// moved, never on opening.
    pub navigating: bool,
    /// The palette field's composer token and draft.
    pub token: &'a str,
    pub query: &'a str,
    /// The palette results' offset from [`scroll`], and greatest height
    /// from [`results_height`].
    pub offset: f32,
    pub results_height: f32,
    /// The conversation a history row switches to.
    pub history: &'a dyn Fn(&Entry) -> Option<History>,
    /// The account menu's heading: the signed-in name, or this computer's.
    pub identity: &'a str,
}

/// The overlay view and the row index of each enabled entry, keyed by entry
/// key. `intent` builds the intent a row sends from its entry key.
pub fn view<I>(panel: Panel<'_>, intent: impl Fn(&str) -> I) -> (Node<I>, BTreeMap<String, usize>) {
    let kind = panel.kind.clone();
    let palette = kind == Kind::Palette;
    let mut indices = BTreeMap::new();
    let entries: Vec<_> = panel
        .entries
        .into_iter()
        .filter(|entry| kind != Kind::Menu || entry.enabled || entry.action != Action::Restore)
        .collect();
    let mut rows = vec![];
    if kind == Kind::ConfirmArchive {
        let mut heading = text(
            "command-heading",
            "Archive this conversation?",
            TextRole::Body,
        );
        heading.style.text_size = Some(ROW.size as u16);
        heading.style.line_height = Some(ROW.line_height as u16);
        heading.style.padding_points = Some([12, 12, 12, 12]);
        rows.push(heading);
    }
    if palette {
        let query = Node {
            key: "command-query".into(),
            style: Style::default(),
            element: Element::Composer {
                token: panel.token.into(),
                placeholder: QUERY_PLACEHOLDER.into(),
                max_bytes: 128,
                enabled: true,
                busy: false,
                stop: None,
                choices: vec![],
                draft: Some(panel.query.into()),
                focus: true,
            },
        };
        let badge = surface(
            "command-shortcut",
            "Command palette shortcut",
            SHORTCUT_GLYPH,
        );
        let icon = surface("command-search-icon", "Command search", SEARCH_GLYPH);
        let mut header = stack(
            "command-search-header",
            Axis::Horizontal,
            vec![icon, query, badge],
        );
        header.style.padding_points = Some([8, 16, 7, 16]);
        header.style.gap_points = Some(10);
        rows.push(header);
        rows.push(surface(
            "command-header-rule",
            "Header separator",
            RULE_HEADER,
        ));
    }
    if kind == Kind::Profile {
        let mut identity = text("profile-identity", panel.identity, TextRole::Status);
        identity.style.text_size = Some(DETAIL.size as u16);
        identity.style.line_height = Some(DETAIL.line_height as u16);
        identity.style.padding_points = Some([10, 12, 2, 12]);
        rows.push(identity);
    }
    let mut items = vec![];
    if entries.is_empty() {
        items.push(text(
            "command-none",
            "No matching commands.",
            TextRole::Status,
        ));
    }
    let (mut actions_shown, mut history_started) = (false, false);
    for (index, entry) in entries.iter().enumerate() {
        if entry.enabled {
            indices.insert(entry.key.clone(), index);
        }
        let history = if palette {
            (panel.history)(entry)
        } else {
            None
        };
        if history.is_some() && !history_started && actions_shown {
            let mut separator = stack(
                "command-history-separator",
                Axis::Vertical,
                vec![surface("command-history-rule", "History separator", RULE)],
            );
            // Zeron's rule sits inside the first history row with an
            // eight-point margin each side; the list gap supplies two.
            separator.style.padding_points = Some([8, 0, 6, 0]);
            separator.style.gap = Some(Space::None);
            items.push(separator);
        }
        history_started |= history.is_some();
        actions_shown |= history.is_none();
        let row_label = match &history {
            Some(history) => format!("{}\n{}", history.title, history.detail),
            None => label(&kind, entry).to_owned(),
        };
        let mut row = Node {
            key: format!("command-{}", entry.key),
            style: Style::default(),
            element: Element::Button {
                shortcut: if palette {
                    crate::commands::badge(&entry.action, cfg!(target_os = "macos"))
                        .map(str::to_owned)
                } else {
                    None
                },
                label: row_label,
                enabled: entry.enabled,
                icon: history.is_none().then(|| Icon {
                    glyph: glyph(&entry.action),
                    circular: false,
                    pill: false,
                }),
                intent: intent(&entry.key),
            },
        };
        if history.is_some() {
            row.style.button_detail = Some(ButtonDetail {
                text_size: DETAIL.size as u16,
                line_height: DETAIL.line_height as u16,
                color: visual::current().muted,
                leading: true,
            });
        }
        row.style.weight = Some(TextWeight::Normal);
        row.style.glyph_color = Some(visual::current().muted);
        row.style.glyph_size = Some(16);
        row.style.glyph_gap = Some(10);
        row.style.align = Some(TextAlign::Start);
        row.style.text_size = Some(ROW.size as u16);
        row.style.line_height = Some(ROW.line_height as u16);
        row.style.button_padding = Some([8, if palette && history.is_none() { 4 } else { 6 }]);
        row.style.min_height = Some(if kind == Kind::Profile {
            32
        } else if history.is_some() {
            HISTORY_ROW as u16
        } else {
            ACTION_ROW as u16
        });
        row.style.radius = Some(if history.is_some() {
            8
        } else if palette {
            10
        } else {
            7
        });
        let selected = index == panel.selected && (palette || panel.navigating);
        row.style.foreground = Some(if history.is_none() && !selected {
            Color {
                alpha: 230,
                ..visual::current().text
            }
        } else {
            visual::current().text
        });
        row.style.background = Some(if selected {
            visual::current().selected
        } else {
            panel_background()
        });
        // Pointer motion and keys choose one row; a resting pointer does
        // not add a second highlight after keyboard navigation.
        row.style.hover_background = row.style.background;
        items.push(row);
    }
    let mut results = stack("command-results", Axis::Vertical, items);
    results.style.padding_points = Some(if palette { [8; 4] } else { [4; 4] });
    results.style.gap_points = Some(2);
    if palette {
        results.style.background = Some(panel_background());
        results.style.viewport = Some(Viewport {
            max_height: panel.results_height as u16,
            offset: panel.offset.round() as u16,
            fade: FADE,
        });
    }
    rows.push(results);
    if palette {
        rows.push(surface(
            "command-footer-rule",
            "Footer separator",
            RULE_FOOTER,
        ));
        let mut hint = stack(
            "command-footer",
            Axis::Wrap,
            vec![
                key_hint("navigation", "↑ ↓", "Navigate"),
                key_hint("selection", "↵", "Select"),
                key_hint("close", "Esc", "Close"),
            ],
        );
        hint.style.padding_points = Some([7, 16, 7, 16]);
        hint.style.gap_points = Some(12);
        rows.push(hint);
    }
    let mut node = stack("command-panel", Axis::Vertical, rows);
    node.style.background = Some(panel_background());
    node.style.border = Some(visual::current().border);
    node.style.radius = Some(if palette { 16 } else { 12 });
    node.style.gap = Some(Space::None);
    (node, indices)
}

/// The empty node that stands in for a closed overlay.
pub fn closed<I>() -> Node<I> {
    stack("command-panel", Axis::Vertical, vec![])
}

fn surface<I>(key: &str, label: &str, resource: &str) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Surface {
            label: label.into(),
            resource: resource.into(),
        },
    }
}

fn key_hint<I>(key: &str, keys: &str, label: &str) -> Node<I> {
    let mut caption = text(&format!("command-{key}-label"), label, TextRole::Status);
    caption.style.text_size = Some(HINT.size as u16);
    caption.style.line_height = Some(HINT.line_height as u16);
    caption.style.intrinsic_width = Some(true);
    let mut keys = text(&format!("command-{key}-keys"), keys, TextRole::Code);
    keys.style.text_size = Some(HINT.size as u16);
    keys.style.line_height = Some(HINT.line_height as u16);
    keys.style.foreground = Some(visual::current().muted);
    let mut cap = stack(&format!("command-{key}-cap"), Axis::Vertical, vec![keys]);
    cap.style.padding_points = Some([1, 5, 1, 5]);
    cap.style.intrinsic_width = Some(true);
    cap.style.background = Some(keycap_fill());
    cap.style.radius = Some(5);
    let mut hint = stack(
        &format!("command-{key}-hint"),
        Axis::Horizontal,
        vec![cap, caption],
    );
    hint.style.gap_points = Some(5);
    hint
}

fn stack<I>(key: &str, axis: Axis, children: Vec<Node<I>>) -> Node<I> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack { axis, children },
    }
}

fn text<I>(key: &str, value: &str, role: TextRole) -> Node<I> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, action: Action) -> Entry {
        Entry {
            key: key.into(),
            label: key.into(),
            hint: "",
            action,
            enabled: true,
        }
    }

    fn switches(count: usize) -> Vec<Entry> {
        (0..count)
            .map(|index| entry(&format!("chat-{index}"), Action::Switch(index.to_string())))
            .collect()
    }

    #[test]
    fn the_palette_keeps_every_action_and_thirty_conversations() {
        let mut entries = vec![entry("new", Action::NewChat)];
        entries.extend(switches(40));
        assert_eq!(limit(Some(&Kind::Palette), entries.clone()).len(), 31);
        assert_eq!(limit(Some(&Kind::Menu), entries).len(), 41);
    }

    #[test]
    fn extents_add_the_section_rule_before_the_first_history_row() {
        let mut entries = vec![entry("new", Action::NewChat)];
        entries.extend(switches(2));
        let (rows, full) = extents(&entries, |e| matches!(e.action, Action::Switch(_)));
        assert_eq!(rows[0], (0.0, PAD + ACTION_ROW));
        let first = PAD + ACTION_ROW + GAP;
        assert_eq!(rows[1], (first, first + SEPARATOR + GAP + HISTORY_ROW));
        let last = rows[1].1 + GAP;
        assert_eq!(rows[2], (last, last + HISTORY_ROW + PAD));
        assert_eq!(full, rows[2].1);
    }

    #[test]
    fn scrolling_reveals_the_selection_and_stays_within_the_content() {
        let rows = [(0.0, 50.0), (52.0, 100.0), (102.0, 200.0)];
        assert_eq!(scroll(&rows, 200.0, 100.0, 2, 0.0, true), 100.0);
        assert_eq!(scroll(&rows, 200.0, 100.0, 0, 100.0, true), 0.0);
        assert_eq!(scroll(&rows, 200.0, 100.0, 2, 0.0, false), 0.0);
        assert_eq!(scroll(&rows, 200.0, 100.0, 0, 500.0, false), 100.0);
        assert_eq!(wheel(0.0, -40.0, 200.0, 100.0), Some(40.0));
        assert_eq!(wheel(0.0, 40.0, 200.0, 100.0), None);
        assert_eq!(wheel(0.0, f32::NAN, 200.0, 100.0), None);
    }

    #[test]
    fn the_account_menu_is_headed_by_its_name_on_the_menu_scale() {
        let none = |_: &Entry| -> Option<History> { None };
        let (node, _) = view(
            Panel {
                kind: Kind::Profile,
                entries: vec![entry("computers", Action::Computers)],
                selected: 0,
                navigating: false,
                token: "t",
                query: "",
                offset: 0.0,
                results_height: 200.0,
                history: &none,
                identity: "Studio Mac",
            },
            str::to_owned,
        );
        let Element::Stack { children, .. } = &node.element else {
            panic!("a stack");
        };
        let heading = &children[0];
        assert!(
            matches!(&heading.element, Element::Text { value, .. } if value == "Studio Mac"),
            "{heading:?}"
        );
        assert_eq!(heading.style.text_size, Some(DETAIL.size as u16));
    }

    #[test]
    fn the_menu_hides_a_disabled_restore_and_maps_rows_to_entries() {
        let mut restore = entry("restore", Action::Restore);
        restore.enabled = false;
        let entries = vec![entry("rename", Action::Rename), restore];
        let none = |_: &Entry| -> Option<History> { None };
        let (node, indices) = view(
            Panel {
                kind: Kind::Menu,
                entries,
                selected: 0,
                navigating: false,
                token: "t",
                query: "",
                offset: 0.0,
                results_height: 200.0,
                history: &none,
                identity: "Studio Mac",
            },
            str::to_owned,
        );
        assert_eq!(node.key, "command-panel");
        assert_eq!(indices.len(), 1);
        assert_eq!(indices["rename"], 0);
        let Element::Stack { children, .. } = &node.element else {
            panic!("panel is a stack");
        };
        let Element::Stack { children: rows, .. } = &children[0].element else {
            panic!("results are a stack");
        };
        assert_eq!(rows.len(), 1);
        let Element::Button { label, intent, .. } = &rows[0].element else {
            panic!("row is a button");
        };
        assert_eq!((label.as_str(), intent.as_str()), ("Rename…", "rename"));
    }
}
