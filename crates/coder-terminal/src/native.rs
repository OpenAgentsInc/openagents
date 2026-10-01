//! A terminal adapter for Rust Native views.
//!
//! This is the build order's RN1 terminal slice: it draws the initial
//! vocabulary (stacks, lists, text, buttons, and surfaces) as ratatui lines
//! on the white ladder, and it turns keyboard focus into a revision-bound
//! [`Activation`]. It does not mount native widgets, edit text, or decide
//! what an intent means; the application resolves the activation against
//! its current view and checks its own authority.
//!
//! Text stays literal. Markdown is shown as its source, and nothing in a
//! view can emit a terminal escape sequence: control characters other than
//! tab and newline are replaced before drawing.
use crate::{Intensity, Ladder};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use rust_native::{Activation, Axis, Element, MessageRole, Node, TextRole, ToolState, View};
use std::collections::BTreeSet;

/// A drawn view.
#[derive(Debug, Default)]
pub struct Rendered {
    pub lines: Vec<Line<'static>>,
    /// Properties and elements this adapter draws differently or not at
    /// all, such as `surface` or `style.background`.
    pub unsupported: BTreeSet<&'static str>,
    /// The line that holds the focused control, for scrolling.
    pub focus_line: Option<usize>,
}

struct Draw<'a> {
    ladder: Ladder,
    focus: Option<&'a str>,
    out: Rendered,
}

fn clean(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_control() && c != '\t' { ' ' } else { c })
        .collect()
}

impl Draw<'_> {
    fn style(&self, intensity: Intensity) -> Style {
        self.ladder.style(intensity)
    }

    fn note<I>(&mut self, node: &Node<I>) {
        let style = &node.style;
        for (set, name) in [
            (style.background.is_some(), "style.background"),
            (style.foreground.is_some(), "style.foreground"),
            (
                style.padding_top.is_some()
                    || style.padding_end.is_some()
                    || style.padding_bottom.is_some()
                    || style.padding_start.is_some(),
                "style.padding",
            ),
            (style.gap.is_some(), "style.gap"),
            (style.align.is_some(), "style.align"),
        ] {
            if set {
                self.out.unsupported.insert(name);
            }
        }
    }

    /// Draw one node to its own lines.
    fn node<I>(&mut self, node: &Node<I>, indent: usize) -> Vec<Line<'static>> {
        self.note(node);
        let pad = " ".repeat(indent);
        match &node.element {
            Element::Stack { axis, children } => {
                let parts: Vec<Vec<Line<'static>>> = children
                    .iter()
                    .map(|child| self.node(child, indent))
                    .collect();
                if *axis == Axis::Horizontal && parts.iter().all(|lines| lines.len() == 1) {
                    let mut spans = Vec::new();
                    for (index, mut lines) in parts.into_iter().enumerate() {
                        let line = lines.remove(0);
                        let mut line_spans = line.spans;
                        if index > 0 {
                            // Drop the child's indent; keep one row.
                            if let Some(first) = line_spans.first_mut() {
                                first.content = first.content.trim_start().to_owned().into();
                            }
                            spans.push(Span::raw("  "));
                        }
                        spans.extend(line_spans);
                    }
                    vec![Line::from(spans)]
                } else {
                    parts.into_iter().flatten().collect()
                }
            }
            Element::List { label, children } => {
                let mut lines = vec![Line::from(Span::styled(
                    format!("{pad}{}:", clean(label)),
                    self.style(Intensity::Half),
                ))];
                for child in children {
                    lines.extend(self.node(child, indent + 2));
                }
                lines
            }
            Element::Text { value, role } => {
                if *role == TextRole::Markdown {
                    self.out.unsupported.insert("text.markdown");
                }
                let style = match role {
                    TextRole::Heading => self.style(Intensity::Full).add_modifier(Modifier::BOLD),
                    TextRole::Status => self.style(Intensity::Half),
                    TextRole::Body | TextRole::Code | TextRole::Markdown | TextRole::Terminal => {
                        self.style(Intensity::ThreeQuarters)
                    }
                };
                value
                    .split('\n')
                    .map(|part| Line::from(Span::styled(format!("{pad}{}", clean(part)), style)))
                    .collect()
            }
            Element::Button { label, enabled, .. } => {
                let label = clean(&label.replace('\n', " · "));
                let focused = self.focus == Some(node.key.as_str());
                // Brackets for an available control and parentheses for a
                // disabled one, so the difference survives a colorless
                // terminal.
                let (text, mut style) = if *enabled {
                    (format!("[ {label} ]"), self.style(Intensity::Full))
                } else {
                    (format!("( {label} )"), self.style(Intensity::Quarter))
                };
                if focused {
                    style = style.add_modifier(Modifier::REVERSED);
                }
                let marker = if focused { "> " } else { "" };
                vec![Line::from(Span::styled(
                    format!("{pad}{marker}{text}"),
                    style,
                ))]
            }
            Element::Surface { label, .. } => {
                self.out.unsupported.insert("surface");
                vec![Line::from(Span::styled(
                    format!("{pad}[{}: not shown in the terminal]", clean(label)),
                    self.style(Intensity::Half),
                ))]
            }
            // Conversation elements draw as plain labeled lines.
            Element::Transcript {
                label,
                children,
                earlier,
                ..
            } => {
                let mut lines = vec![Line::from(Span::styled(
                    format!("{pad}{}:", clean(label)),
                    self.style(Intensity::Half),
                ))];
                if let Some(earlier) = earlier {
                    lines.push(self.control(node, &earlier.label, !earlier.loading, &pad));
                }
                for child in children {
                    lines.extend(self.node(child, indent + 2));
                }
                lines
            }
            Element::Message {
                role,
                note,
                children,
            } => {
                let who = match role {
                    MessageRole::User => "You",
                    MessageRole::Assistant => "Assistant",
                    MessageRole::System => "System",
                };
                let heading = match note {
                    Some(note) => format!("{pad}{who} · {}", clean(note)),
                    None => format!("{pad}{who}"),
                };
                let mut lines = vec![Line::from(Span::styled(
                    heading,
                    self.style(Intensity::Half),
                ))];
                for child in children {
                    lines.extend(self.node(child, indent + 2));
                }
                lines
            }
            Element::Markdown { blocks } => rust_native::markdown::plain(blocks)
                .split('\n')
                .map(|part| {
                    Line::from(Span::styled(
                        format!("{pad}{}", clean(part)),
                        self.style(Intensity::ThreeQuarters),
                    ))
                })
                .collect(),
            Element::Tool {
                name,
                detail,
                state,
                ..
            } => {
                let mark = match state {
                    ToolState::Running => "…",
                    ToolState::Done => "✓",
                    ToolState::Failed => "✗",
                };
                vec![Line::from(Span::styled(
                    format!("{pad}{mark} {} {}", clean(name), clean(detail)),
                    self.style(Intensity::Half),
                ))]
            }
            Element::Working { label } => vec![Line::from(Span::styled(
                format!("{pad}{}…", clean(label)),
                self.style(Intensity::Half),
            ))],
            Element::Composer { busy, .. } => {
                self.out.unsupported.insert("composer");
                let label = if *busy { "Stop" } else { "Message" };
                vec![self.control(node, label, *busy, &pad)]
            }
        }
    }

    /// One control line, as a button draws.
    fn control<I>(
        &mut self,
        node: &Node<I>,
        label: &str,
        enabled: bool,
        pad: &str,
    ) -> Line<'static> {
        let label = clean(&label.replace('\n', " · "));
        let focused = self.focus == Some(node.key.as_str());
        let (text, mut style) = if enabled {
            (format!("[ {label} ]"), self.style(Intensity::Full))
        } else {
            (format!("( {label} )"), self.style(Intensity::Quarter))
        };
        if focused {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let marker = if focused { "> " } else { "" };
        Line::from(Span::styled(format!("{pad}{marker}{text}"), style))
    }
}

/// Draw `view`, marking the control named by `focus`.
pub fn render<I>(view: &View<I>, ladder: Ladder, focus: Option<&str>) -> Rendered {
    let mut draw = Draw {
        ladder,
        focus,
        out: Rendered::default(),
    };
    let lines = draw.node(&view.root, 0);
    draw.out.focus_line = lines.iter().position(|line| {
        line.spans
            .iter()
            .any(|span| span.style.add_modifier.contains(Modifier::REVERSED))
    });
    draw.out.lines = lines;
    draw.out
}

/// Keyboard focus over a view's controls, in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    instance: String,
    revision: u64,
    controls: Vec<(String, bool)>,
    current: Option<usize>,
}

fn controls<I>(node: &Node<I>, out: &mut Vec<(String, bool)>) {
    match &node.element {
        Element::Stack { children, .. } | Element::List { children, .. } => {
            for child in children {
                controls(child, out);
            }
        }
        Element::Button { enabled, .. } => out.push((node.key.clone(), *enabled)),
        Element::Transcript {
            children, earlier, ..
        } => {
            if let Some(earlier) = earlier {
                out.push((node.key.clone(), !earlier.loading));
            }
            for child in children {
                controls(child, out);
            }
        }
        Element::Message { children, .. } | Element::Tool { children, .. } => {
            for child in children {
                controls(child, out);
            }
        }
        Element::Composer { busy, stop, .. } => {
            out.push((node.key.clone(), *busy && stop.is_some()))
        }
        Element::Text { .. }
        | Element::Surface { .. }
        | Element::Markdown { .. }
        | Element::Working { .. } => {}
    }
}

impl Focus {
    /// Focus `keep` if the view still has it; otherwise the first enabled
    /// control. Disabled controls stay focusable so their reason can be read.
    pub fn new<I>(view: &View<I>, keep: Option<&str>) -> Self {
        let mut list = Vec::new();
        controls(&view.root, &mut list);
        let current = keep
            .and_then(|key| list.iter().position(|(control, _)| control == key))
            .or_else(|| list.iter().position(|(_, enabled)| *enabled))
            .or(if list.is_empty() { None } else { Some(0) });
        Self {
            instance: view.instance.clone(),
            revision: view.revision,
            controls: list,
            current,
        }
    }

    pub fn current(&self) -> Option<&str> {
        self.current
            .and_then(|index| self.controls.get(index))
            .map(|(key, _)| key.as_str())
    }

    fn step(&mut self, forward: bool) {
        let count = self.controls.len();
        if count == 0 {
            return;
        }
        self.current = Some(match self.current {
            None => 0,
            Some(index) if forward => (index + 1) % count,
            Some(index) => (index + count - 1) % count,
        });
    }

    /// Move with Tab, Shift-Tab, and the arrow keys. Enter or Space on an
    /// enabled control returns its activation for the view this focus was
    /// built from.
    pub fn handle(&mut self, key: KeyEvent) -> Option<Activation> {
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.step(true),
            KeyCode::BackTab | KeyCode::Up => self.step(false),
            KeyCode::Enter | KeyCode::Char(' ')
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                let (node, enabled) = self.controls.get(self.current?)?;
                return enabled.then(|| Activation {
                    instance: self.instance.clone(),
                    revision: self.revision,
                    node: node.clone(),
                });
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Colors;
    use rust_native::style::Style as NativeStyle;

    fn node(key: &str, element: Element<u8>) -> Node<u8> {
        Node {
            key: key.into(),
            style: NativeStyle::default(),
            element,
        }
    }

    fn fixture() -> View<u8> {
        let button = |key: &str, label: &str, enabled, intent| {
            node(
                key,
                Element::Button {
                    shortcut: None,
                    label: label.into(),
                    enabled,
                    icon: None,
                    intent,
                },
            )
        };
        let mut root = node(
            "root",
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "title",
                        Element::Text {
                            value: "Computers".into(),
                            role: TextRole::Heading,
                        },
                    ),
                    node(
                        "row",
                        Element::Stack {
                            axis: Axis::Horizontal,
                            children: vec![
                                button("off", "Switch off", true, 1),
                                button("revoke", "Revoke", false, 2),
                            ],
                        },
                    ),
                    node(
                        "reason",
                        Element::Text {
                            value: "Unavailable: offline.\u{1b}[31m".into(),
                            role: TextRole::Status,
                        },
                    ),
                    node(
                        "list",
                        Element::List {
                            label: "Devices".into(),
                            children: vec![
                                node(
                                    "md",
                                    Element::Text {
                                        value: "**bold**\nline two".into(),
                                        role: TextRole::Markdown,
                                    },
                                ),
                                button("add", "Add\na computer", true, 3),
                            ],
                        },
                    ),
                    node(
                        "gpu",
                        Element::Surface {
                            resource: "world".into(),
                            label: "Verse world".into(),
                        },
                    ),
                ],
            },
        );
        root.style.background = Some(rust_native::style::Color::rgb(0, 0, 0));
        View::new("terminal:test", 4, root)
    }

    fn plain(rendered: &Rendered) -> Vec<String> {
        rendered
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn draws_the_initial_vocabulary_literally() {
        let view = fixture();
        let rendered = render(&view, Ladder::new(Colors::None), Some("off"));
        assert_eq!(
            plain(&rendered),
            [
                "Computers",
                "> [ Switch off ]  ( Revoke )",
                "Unavailable: offline. [31m",
                "Devices:",
                "  **bold**",
                "  line two",
                "  [ Add · a computer ]",
                "[Verse world: not shown in the terminal]",
            ]
        );
        assert_eq!(rendered.focus_line, Some(1));
        assert_eq!(
            rendered.unsupported.iter().copied().collect::<Vec<_>>(),
            ["style.background", "surface", "text.markdown"]
        );
    }

    #[test]
    fn keyboard_focus_resolves_only_enabled_current_controls() {
        let view = fixture();
        let validated = view.clone().validate().unwrap();
        let mut focus = Focus::new(&view, None);
        assert_eq!(focus.current(), Some("off"));
        let enter = KeyEvent::from(KeyCode::Enter);
        let activation = focus.handle(enter).unwrap();
        assert_eq!(validated.activate(&activation), Ok(&1));
        focus.handle(KeyEvent::from(KeyCode::Tab));
        assert_eq!(focus.current(), Some("revoke"));
        // A disabled control takes focus but never activates.
        assert_eq!(focus.handle(enter), None);
        focus.handle(KeyEvent::from(KeyCode::Down));
        assert_eq!(
            validated.activate(&focus.handle(KeyEvent::from(KeyCode::Char(' '))).unwrap()),
            Ok(&3)
        );
        focus.handle(KeyEvent::from(KeyCode::Tab));
        assert_eq!(focus.current(), Some("off"));
        focus.handle(KeyEvent::from(KeyCode::BackTab));
        assert_eq!(focus.current(), Some("add"));
        // Focus survives a new revision by key; the old activation is stale.
        let stale = focus.handle(enter).unwrap();
        let mut next = view.clone();
        next.revision = 5;
        let next_valid = next.clone().validate().unwrap();
        assert_eq!(
            next_valid.activate(&stale),
            Err(rust_native::ViewError::StaleActivation)
        );
        let mut refocused = Focus::new(&next, focus.current());
        assert_eq!(refocused.current(), Some("add"));
        assert_eq!(
            next_valid.activate(&refocused.handle(enter).unwrap()),
            Ok(&3)
        );
    }
}
