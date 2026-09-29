//! Places a view's nodes, in points, as a flat list of drawing operations
//! and the rectangles a pointer can activate.
//!
//! The rules, which the README lists as the adapter's support table:
//!
//! - A vertical stack puts its children one under another, `gap` apart. With
//!   `align` `start` (the default) text, stacks, and lists fill the width
//!   and buttons and surfaces keep their own width at the start; with
//!   `center` or `end` every child keeps its own width and is centered or
//!   put at the end.
//! - A horizontal stack puts buttons and surfaces at their own width and
//!   shares what is left among its text, stacks, and lists; children are
//!   centered on the row's height. A wrapping stack flows its children
//!   onto as many rows as they need.
//! - A list is a vertical stack with a rule between rows. Its label is for
//!   assistive technology and is not drawn.
//! - A stack with a `style.background` is a card with rounded corners.
//! - A button is a filled rounded rectangle, a capsule with `pill`, a link
//!   when its `style.background` is transparent, and a checkbox with the
//!   `unchecked` or `checked` glyph. Other glyphs are not drawn; the label
//!   says what the button does.
//! - A surface takes the size the application gives for its resource; an
//!   unregistered one shows its label.
//! - Transcripts, messages, tools, and composers are not supported: they
//!   are recorded in [`Scene::unsupported`] and drawn as their children or
//!   label. Markdown is drawn as its plain text.

use crate::text::{Fonts, Paragraph, font};
use crate::theme::{Theme, space};
use rust_native::layout::display::Weight;
use rust_native::style::{Color, Style, TextAlign, TextWeight};
use rust_native::{Axis, Element, Glyph, Node, TextRole, View};
use std::collections::BTreeSet;
use std::rc::Rc;

/// The side of a checkbox, in points.
pub const CHECKBOX: f32 = 18.0;
/// The space between a checkbox and its label, in points.
const CHECKBOX_GAP: f32 = 10.0;
/// A filled button's padding, in points: sideways and up and down.
const BUTTON_PAD: (f32, f32) = (18.0, 9.0);

/// A rectangle in points.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    /// Whether the point `x`, `y` is inside.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    fn inflate(&self, by: f32) -> Rect {
        Rect {
            x: self.x - by,
            y: self.y - by,
            w: self.w + 2.0 * by,
            h: self.h + 2.0 * by,
        }
    }
}

/// One drawing operation, in points.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    Fill {
        rect: Rect,
        radius: f32,
        color: Color,
    },
    Stroke {
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
    },
    Text {
        paragraph: Rc<Paragraph>,
        x: f32,
        y: f32,
        width: f32,
        align: TextAlign,
        color: Color,
    },
    /// A check mark inside `rect`.
    Check { rect: Rect, color: Color },
    /// The application paints the surface `resource` here.
    Surface { resource: String, rect: Rect },
}

/// A rectangle that activates a node.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub rect: Rect,
    pub key: String,
    pub enabled: bool,
}

/// A laid-out view.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub ops: Vec<Op>,
    /// Every button, in the view's order, which is also the focus order.
    pub hits: Vec<Hit>,
    /// The height of everything drawn, with the margins, in points.
    pub height: f32,
    /// Elements and properties this adapter drew differently, or not at
    /// all, such as `transcript` or `icon.glyph`.
    pub unsupported: BTreeSet<&'static str>,
}

impl Scene {
    /// The enabled button under `x`, `y`, if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Hit> {
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.enabled && hit.rect.contains(x, y))
    }

    /// The keys of every enabled button, in focus order.
    pub fn focus_order(&self) -> Vec<&str> {
        self.hits
            .iter()
            .filter(|hit| hit.enabled)
            .map(|hit| hit.key.as_str())
            .collect()
    }

    /// Every text the scene draws, in order, one entry a paragraph.
    pub fn texts(&self) -> Vec<&str> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                Op::Text { paragraph, .. } => Some(paragraph.text.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// What the pointer and the keyboard are on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Interaction {
    pub hover: Option<String>,
    pub pressed: Option<String>,
    pub focus: Option<String>,
}

/// The sizes the application gives its surfaces: `(resource, available)`
/// to width and height in points.
pub type SurfaceSizes<'a> = dyn Fn(&str, f32) -> Option<(f32, f32)> + 'a;

/// Lays out `view` in a window `width` by `height` points: a centered
/// column no wider than the theme's, centered vertically when it fits.
pub fn lay_out_window<I>(
    view: &View<I>,
    theme: &Theme,
    fonts: &mut Fonts,
    sizes: &SurfaceSizes<'_>,
    interaction: &Interaction,
    width: f32,
    height: f32,
) -> Scene {
    let column = (width - 2.0 * theme.margin).clamp(1.0, theme.column);
    let mut engine = Engine {
        theme,
        fonts,
        sizes,
        interaction,
        scene: Scene::default(),
    };
    let (_, content) = engine.size(&view.root, column);
    let top = if content + 2.0 * theme.margin <= height {
        ((height - content) / 2.0).round()
    } else {
        theme.margin
    };
    let x = ((width - column) / 2.0).round();
    engine.place(&view.root, x, top, column);
    let mut scene = engine.scene;
    scene.height = top + content + theme.margin;
    scene
}

struct Engine<'a> {
    theme: &'a Theme,
    fonts: &'a mut Fonts,
    sizes: &'a SurfaceSizes<'a>,
    interaction: &'a Interaction,
    scene: Scene,
}

/// Padding: top, end, bottom, start.
fn padding(style: &Style) -> [f32; 4] {
    [
        space(style.padding_top),
        space(style.padding_end),
        space(style.padding_bottom),
        space(style.padding_start),
    ]
}

/// Whether a node keeps its own width in a stack rather than sharing the
/// stack's.
fn keeps_width<I>(node: &Node<I>) -> bool {
    matches!(
        node.element,
        Element::Button { .. } | Element::Surface { .. }
    )
}

fn transparent(color: Option<Color>) -> bool {
    color.is_some_and(|color| color.alpha == 0)
}

/// `a` mixed toward `b` by `t`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Color {
        red: channel(a.red, b.red),
        green: channel(a.green, b.green),
        blue: channel(a.blue, b.blue),
        alpha: a.alpha,
    }
}

fn checkbox_state(icon: Option<rust_native::Icon>) -> Option<bool> {
    match icon.map(|icon| icon.glyph) {
        Some(Glyph::Checked) => Some(true),
        Some(Glyph::Unchecked) => Some(false),
        _ => None,
    }
}

impl Engine<'_> {
    fn text_font(&self, role: TextRole, style: &Style) -> rust_native::layout::display::Font {
        let bold = style.weight == Some(TextWeight::Bold);
        let (size, weight, mono) = match role {
            TextRole::Heading => (self.theme.heading, Weight::Semibold, false),
            TextRole::Status => (self.theme.status, Weight::Regular, false),
            TextRole::Code | TextRole::Terminal => (self.theme.code, Weight::Regular, true),
            TextRole::Body | TextRole::Markdown => (self.theme.body, Weight::Regular, false),
        };
        font(size, if bold { Weight::Bold } else { weight }, mono)
    }

    fn text_color(&self, role: TextRole, style: &Style) -> Color {
        style.foreground.unwrap_or(match role {
            TextRole::Status => self.theme.muted,
            _ => self.theme.text,
        })
    }

    fn paragraph(
        &mut self,
        value: &str,
        role: TextRole,
        style: &Style,
        width: Option<f32>,
    ) -> Rc<Paragraph> {
        let font = self.text_font(role, style);
        // A box laid out at a paragraph's own width must not break it
        // again over a rounding error, so every wrap width gets a point of
        // slack.
        let width = if role == TextRole::Terminal {
            None
        } else {
            width.map(|width| width + 1.0)
        };
        self.fonts.paragraph(value, font, width)
    }

    /// A node's width (at most `available`) and its height at that width.
    fn size<I>(&mut self, node: &Node<I>, available: f32) -> (f32, f32) {
        let [top, end, bottom, start] = padding(&node.style);
        let inner = (available - start - end).max(1.0);
        let (w, h) = match &node.element {
            Element::Text { value, role } => {
                let paragraph = self.paragraph(value, *role, &node.style, Some(inner));
                (paragraph.width, paragraph.height)
            }
            Element::Markdown { blocks } => {
                let value = rust_native::markdown::plain(blocks);
                let paragraph = self.paragraph(&value, TextRole::Body, &node.style, Some(inner));
                (paragraph.width, paragraph.height)
            }
            Element::Working { label } => {
                let paragraph = self.paragraph(label, TextRole::Status, &node.style, Some(inner));
                (paragraph.width, paragraph.height)
            }
            Element::Button { label, icon, .. } => {
                if checkbox_state(*icon).is_some() {
                    let paragraph = self.paragraph(
                        label,
                        TextRole::Body,
                        &node.style,
                        Some((inner - CHECKBOX - CHECKBOX_GAP).max(1.0)),
                    );
                    (
                        CHECKBOX + CHECKBOX_GAP + paragraph.width,
                        paragraph.height.max(CHECKBOX),
                    )
                } else if transparent(node.style.background) {
                    let paragraph = self.paragraph(label, TextRole::Body, &node.style, Some(inner));
                    (paragraph.width, paragraph.height + 4.0)
                } else {
                    let paragraph = self.paragraph(
                        label,
                        TextRole::Body,
                        &Style {
                            weight: Some(TextWeight::Bold),
                            ..node.style
                        },
                        Some((inner - 2.0 * BUTTON_PAD.0).max(1.0)),
                    );
                    (
                        paragraph.width + 2.0 * BUTTON_PAD.0,
                        paragraph.height + 2.0 * BUTTON_PAD.1,
                    )
                }
            }
            Element::Surface { resource, label } => match (self.sizes)(resource, inner) {
                Some((w, h)) => (w.min(inner), h),
                None => {
                    let paragraph =
                        self.paragraph(label, TextRole::Status, &node.style, Some(inner));
                    (paragraph.width, paragraph.height)
                }
            },
            Element::Stack {
                axis: Axis::Vertical,
                children,
            }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } => {
                let gap = self.gap(node);
                let mut w: f32 = 0.0;
                let mut h = 0.0;
                for (index, child) in children.iter().enumerate() {
                    let (cw, ch) = self.size(child, inner);
                    w = w.max(cw);
                    h += ch + if index > 0 { gap } else { 0.0 };
                }
                (w, h)
            }
            Element::Stack {
                axis: Axis::Horizontal,
                children,
            } => {
                let widths = self.row_widths(children, inner, self.gap(node));
                let mut h: f32 = 0.0;
                for (child, width) in children.iter().zip(&widths) {
                    h = h.max(self.size(child, *width).1);
                }
                let gaps = self.gap(node) * children.len().saturating_sub(1) as f32;
                (widths.iter().sum::<f32>() + gaps, h)
            }
            Element::Stack {
                axis: Axis::Wrap,
                children,
            } => {
                let gap = self.gap(node);
                let placed = self.flow(children, inner, gap);
                let w = placed.iter().map(|(x, _, w, _)| x + w).fold(0.0, f32::max);
                let h = placed.iter().map(|(_, y, _, h)| y + h).fold(0.0, f32::max);
                (w, h)
            }
            Element::Composer { placeholder, .. } => {
                let paragraph =
                    self.paragraph(placeholder, TextRole::Status, &node.style, Some(inner));
                (paragraph.width, paragraph.height)
            }
        };
        ((w + start + end).min(available), h + top + bottom)
    }

    fn gap<I>(&self, node: &Node<I>) -> f32 {
        space(node.style.gap)
    }

    /// The width each child of a horizontal stack gets within `inner`.
    fn row_widths<I>(&mut self, children: &[Node<I>], inner: f32, gap: f32) -> Vec<f32> {
        let gaps = gap * children.len().saturating_sub(1) as f32;
        let mut widths = vec![0.0; children.len()];
        let mut used = gaps;
        let mut shared = 0;
        for (index, child) in children.iter().enumerate() {
            if keeps_width(child) {
                widths[index] = self.size(child, inner).0;
                used += widths[index];
            } else {
                shared += 1;
            }
        }
        if shared > 0 {
            let share = ((inner - used) / shared as f32).max(1.0);
            for (index, child) in children.iter().enumerate() {
                if !keeps_width(child) {
                    widths[index] = share;
                    // A lone shared child keeps the rest of the row.
                    if shared > 1 {
                        widths[index] = self.size(child, share).0.max(1.0).min(share);
                    }
                }
            }
        }
        widths
    }

    /// Positions, relative to the stack, of a wrapping stack's children:
    /// x, y, width, height.
    fn flow<I>(&mut self, children: &[Node<I>], inner: f32, gap: f32) -> Vec<(f32, f32, f32, f32)> {
        let mut placed = Vec::with_capacity(children.len());
        let (mut x, mut y, mut line) = (0.0, 0.0, 0.0f32);
        for child in children {
            let (w, h) = self.size(child, inner);
            if x > 0.0 && x + w > inner {
                x = 0.0;
                y += line + gap;
                line = 0.0;
            }
            placed.push((x, y, w, h));
            x += w + gap;
            line = line.max(h);
        }
        placed
    }

    /// Places `node` at `x`, `y` in a box `width` points wide.
    fn place<I>(&mut self, node: &Node<I>, x: f32, y: f32, width: f32) {
        let (_, height) = self.size(node, width);
        let [top, end, bottom, start] = padding(&node.style);
        let inner = (width - start - end).max(1.0);
        let (ix, iy) = (x + start, y + top);
        let is_stack = matches!(node.element, Element::Stack { .. } | Element::List { .. });
        if is_stack
            && let Some(background) = node.style.background
            && background.alpha > 0
        {
            self.scene.ops.push(Op::Fill {
                rect: Rect {
                    x,
                    y,
                    w: width,
                    h: height,
                },
                radius: self.theme.card_radius,
                color: background,
            });
        }
        let align = node.style.align.unwrap_or(TextAlign::Start);
        match &node.element {
            Element::Text { value, role } => {
                let paragraph = self.paragraph(value, *role, &node.style, Some(inner));
                let color = self.text_color(*role, &node.style);
                self.text(paragraph, ix, iy, inner, align, color);
            }
            Element::Markdown { blocks } => {
                let value = rust_native::markdown::plain(blocks);
                let paragraph = self.paragraph(&value, TextRole::Body, &node.style, Some(inner));
                let color = self.text_color(TextRole::Body, &node.style);
                self.text(paragraph, ix, iy, inner, align, color);
            }
            Element::Working { label } => {
                let paragraph = self.paragraph(label, TextRole::Status, &node.style, Some(inner));
                let color = self.text_color(TextRole::Status, &node.style);
                self.text(paragraph, ix, iy, inner, align, color);
            }
            Element::Composer { placeholder, .. } => {
                self.scene.unsupported.insert("composer");
                let paragraph =
                    self.paragraph(placeholder, TextRole::Status, &node.style, Some(inner));
                let color = self.theme.muted;
                self.text(paragraph, ix, iy, inner, align, color);
            }
            Element::Button {
                label,
                enabled,
                icon,
                ..
            } => self.button(node, label, *enabled, *icon, ix, iy, inner),
            Element::Surface { resource, label } => match (self.sizes)(resource, inner) {
                Some((w, h)) => {
                    let w = w.min(inner);
                    let offset = match align {
                        TextAlign::Start => 0.0,
                        TextAlign::Center => (inner - w) / 2.0,
                        TextAlign::End => inner - w,
                    };
                    self.scene.ops.push(Op::Surface {
                        resource: resource.clone(),
                        rect: Rect {
                            x: (ix + offset).round(),
                            y: iy.round(),
                            w,
                            h,
                        },
                    });
                }
                None => {
                    self.scene.unsupported.insert("surface");
                    let paragraph =
                        self.paragraph(label, TextRole::Status, &node.style, Some(inner));
                    let color = self.theme.muted;
                    self.text(paragraph, ix, iy, inner, align, color);
                }
            },
            Element::Stack {
                axis: Axis::Vertical,
                children,
            }
            | Element::List { children, .. }
            | Element::Transcript { children, .. }
            | Element::Message { children, .. }
            | Element::Tool { children, .. } => {
                match &node.element {
                    Element::Transcript { .. } => {
                        self.scene.unsupported.insert("transcript");
                    }
                    Element::Message { .. } => {
                        self.scene.unsupported.insert("message");
                    }
                    Element::Tool { .. } => {
                        self.scene.unsupported.insert("tool");
                    }
                    _ => {}
                }
                let list = matches!(node.element, Element::List { .. });
                let gap = self.gap(node);
                let mut cy = iy;
                for (index, child) in children.iter().enumerate() {
                    if index > 0 {
                        if list {
                            let rule = Rect {
                                x: ix,
                                y: (cy + gap / 2.0).round(),
                                w: inner,
                                h: 1.0,
                            };
                            self.scene.ops.push(Op::Fill {
                                rect: rule,
                                radius: 0.0,
                                color: self.theme.rule,
                            });
                        }
                        cy += gap;
                    }
                    let own = keeps_width(child) || align != TextAlign::Start;
                    let (cw, ch) = self.size(child, inner);
                    let (cx, box_width) = if own {
                        let offset = match align {
                            TextAlign::Start => 0.0,
                            TextAlign::Center => ((inner - cw) / 2.0).round(),
                            TextAlign::End => inner - cw,
                        };
                        (ix + offset, cw)
                    } else {
                        (ix, inner)
                    };
                    self.place(child, cx, cy, box_width);
                    cy += ch;
                }
            }
            Element::Stack {
                axis: Axis::Horizontal,
                children,
            } => {
                let gap = self.gap(node);
                let widths = self.row_widths(children, inner, gap);
                let heights: Vec<f32> = children
                    .iter()
                    .zip(&widths)
                    .map(|(child, width)| self.size(child, *width).1)
                    .collect();
                let row = heights.iter().copied().fold(0.0, f32::max);
                let total =
                    widths.iter().sum::<f32>() + gap * children.len().saturating_sub(1) as f32;
                let mut cx = ix
                    + match align {
                        TextAlign::Start => 0.0,
                        TextAlign::Center => ((inner - total) / 2.0).max(0.0).round(),
                        TextAlign::End => (inner - total).max(0.0),
                    };
                for ((child, w), h) in children.iter().zip(&widths).zip(&heights) {
                    self.place(child, cx, iy + ((row - h) / 2.0).round(), *w);
                    cx += w + gap;
                }
            }
            Element::Stack {
                axis: Axis::Wrap,
                children,
            } => {
                let gap = self.gap(node);
                for (child, (cx, cy, w, _)) in children.iter().zip(self.flow(children, inner, gap))
                {
                    self.place(child, ix + cx, iy + cy, w);
                }
            }
        }
        let _ = bottom;
    }

    fn text(
        &mut self,
        paragraph: Rc<Paragraph>,
        x: f32,
        y: f32,
        width: f32,
        align: TextAlign,
        color: Color,
    ) {
        self.scene.ops.push(Op::Text {
            paragraph,
            x,
            y,
            width,
            align,
            color,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn button<I>(
        &mut self,
        node: &Node<I>,
        label: &str,
        enabled: bool,
        icon: Option<rust_native::Icon>,
        x: f32,
        y: f32,
        inner: f32,
    ) {
        let hovered = self.interaction.hover.as_deref() == Some(node.key.as_str());
        let pressed = self.interaction.pressed.as_deref() == Some(node.key.as_str());
        let focused = self.interaction.focus.as_deref() == Some(node.key.as_str());
        let theme = *self.theme;
        if icon.is_some() && checkbox_state(icon).is_none() {
            self.scene.unsupported.insert("icon.glyph");
        }
        let rect;
        if let Some(on) = checkbox_state(icon) {
            let paragraph = self.paragraph(
                label,
                TextRole::Body,
                &node.style,
                Some((inner - CHECKBOX - CHECKBOX_GAP).max(1.0)),
            );
            let line = paragraph.line_height();
            let boxed = Rect {
                x,
                y: (y + ((line - CHECKBOX) / 2.0).max(0.0)).round(),
                w: CHECKBOX,
                h: CHECKBOX,
            };
            let accent = if enabled { theme.button } else { theme.muted };
            if on {
                let fill = if pressed {
                    mix(accent, theme.background, 0.25)
                } else {
                    accent
                };
                self.scene.ops.push(Op::Fill {
                    rect: boxed,
                    radius: 4.0,
                    color: fill,
                });
                self.scene.ops.push(Op::Check {
                    rect: boxed,
                    color: theme.button_text,
                });
            } else {
                let edge = if hovered && enabled {
                    theme.muted
                } else {
                    theme.rule
                };
                self.scene.ops.push(Op::Stroke {
                    rect: boxed,
                    radius: 4.0,
                    width: 1.5,
                    color: mix(edge, theme.text, if hovered { 0.3 } else { 0.0 }),
                });
            }
            let color = if enabled {
                node.style.foreground.unwrap_or(theme.text)
            } else {
                theme.muted
            };
            let height = paragraph.height.max(CHECKBOX);
            let width = CHECKBOX + CHECKBOX_GAP + paragraph.width;
            self.text(
                paragraph,
                x + CHECKBOX + CHECKBOX_GAP,
                y,
                inner - CHECKBOX - CHECKBOX_GAP,
                TextAlign::Start,
                color,
            );
            rect = Rect {
                x,
                y,
                w: width,
                h: height,
            };
        } else if transparent(node.style.background) {
            let paragraph = self.paragraph(label, TextRole::Body, &node.style, Some(inner));
            let color = if enabled {
                node.style.foreground.unwrap_or(theme.link)
            } else {
                theme.muted
            };
            let color = if pressed {
                mix(color, theme.background, 0.3)
            } else {
                color
            };
            rect = Rect {
                x,
                y,
                w: paragraph.width,
                h: paragraph.height + 4.0,
            };
            if hovered && enabled {
                self.scene.ops.push(Op::Fill {
                    rect: Rect {
                        x,
                        y: y + paragraph.height,
                        w: paragraph.width,
                        h: 1.0,
                    },
                    radius: 0.0,
                    color,
                });
            }
            self.text(paragraph, x, y + 1.0, inner, TextAlign::Start, color);
        } else {
            let style = Style {
                weight: Some(TextWeight::Bold),
                ..node.style
            };
            let paragraph = self.paragraph(
                label,
                TextRole::Body,
                &style,
                Some((inner - 2.0 * BUTTON_PAD.0).max(1.0)),
            );
            rect = Rect {
                x,
                y,
                w: paragraph.width + 2.0 * BUTTON_PAD.0,
                h: paragraph.height + 2.0 * BUTTON_PAD.1,
            };
            let pill = icon.is_some_and(|icon| icon.pill);
            let radius = if pill {
                rect.h / 2.0
            } else {
                theme.button_radius
            };
            let mut fill = node.style.background.unwrap_or(theme.button);
            let mut color = node.style.foreground.unwrap_or(theme.button_text);
            if !enabled {
                fill = mix(fill, theme.background, 0.7);
                color = mix(color, fill, 0.5);
            } else if pressed {
                fill = mix(fill, theme.background, 0.3);
            } else if hovered {
                fill = mix(fill, theme.background, 0.12);
            }
            self.scene.ops.push(Op::Fill {
                rect,
                radius,
                color: fill,
            });
            self.text(
                paragraph,
                x + BUTTON_PAD.0,
                y + BUTTON_PAD.1,
                rect.w - 2.0 * BUTTON_PAD.0,
                TextAlign::Center,
                color,
            );
        }
        if focused {
            self.scene.ops.push(Op::Stroke {
                rect: rect.inflate(3.0),
                radius: theme.button_radius + 3.0,
                width: 2.0,
                color: theme.focus,
            });
        }
        self.scene.hits.push(Hit {
            rect,
            key: node.key.clone(),
            enabled,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Icon;

    fn node(key: &str, style: Style, element: Element<u8>) -> Node<u8> {
        Node {
            key: key.into(),
            style,
            element,
        }
    }

    fn text(key: &str, value: &str) -> Node<u8> {
        node(
            key,
            Style::default(),
            Element::Text {
                value: value.into(),
                role: TextRole::Body,
            },
        )
    }

    fn button(key: &str, label: &str, icon: Option<Icon>) -> Node<u8> {
        node(
            key,
            Style::default(),
            Element::Button {
                label: label.into(),
                enabled: true,
                icon,
                intent: 1,
            },
        )
    }

    fn lay_out(root: Node<u8>) -> Scene {
        let view = View::new("test", 1, root);
        let mut fonts = Fonts::new();
        lay_out_window(
            &view,
            &Theme::default(),
            &mut fonts,
            &|resource, _| (resource == "square").then_some((100.0, 100.0)),
            &Interaction::default(),
            600.0,
            800.0,
        )
    }

    #[test]
    fn a_vertical_stack_stacks_and_a_button_is_hit() {
        let scene = lay_out(node(
            "root",
            Style::default(),
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![text("a", "First"), button("b", "Press", None)],
            },
        ));
        assert_eq!(scene.texts(), vec!["First", "Press"]);
        let hit = &scene.hits[0];
        assert_eq!(hit.key, "b");
        let (cx, cy) = (hit.rect.x + 2.0, hit.rect.y + 2.0);
        assert_eq!(scene.hit(cx, cy).map(|hit| hit.key.as_str()), Some("b"));
        assert!(scene.hit(0.0, 0.0).is_none());
        assert!(scene.unsupported.is_empty());
    }

    #[test]
    fn a_centered_stack_centers_a_surface_and_a_checkbox_draws_its_box() {
        let checked = Icon {
            glyph: Glyph::Checked,
            circular: false,
            pill: false,
        };
        let scene = lay_out(node(
            "root",
            Style {
                align: Some(TextAlign::Center),
                ..Style::default()
            },
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "qr",
                        Style::default(),
                        Element::Surface {
                            resource: "square".into(),
                            label: "A code".into(),
                        },
                    ),
                    button("allow", "Allow it", Some(checked)),
                ],
            },
        ));
        let surface = scene
            .ops
            .iter()
            .find_map(|op| match op {
                Op::Surface { rect, .. } => Some(*rect),
                _ => None,
            })
            .expect("the surface");
        assert_eq!(surface.x + surface.w / 2.0, 300.0);
        assert!(scene.ops.iter().any(|op| matches!(op, Op::Check { .. })));
    }

    #[test]
    fn a_row_shares_its_width_with_text_and_keeps_buttons_whole() {
        let scene = lay_out(node(
            "row",
            Style::default(),
            Element::Stack {
                axis: Axis::Horizontal,
                children: vec![
                    text("name", "Kai's iPhone"),
                    button("remove", "Remove", None),
                ],
            },
        ));
        let remove = &scene.hits[0].rect;
        // The button sits at the end of the 520-point column.
        assert!(
            (remove.x + remove.w - (40.0 + 520.0)).abs() < 1.0,
            "{remove:?}"
        );
    }

    #[test]
    fn unsupported_elements_are_named() {
        let scene = lay_out(node(
            "root",
            Style::default(),
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    node(
                        "unknown",
                        Style::default(),
                        Element::Surface {
                            resource: "unregistered".into(),
                            label: "Something".into(),
                        },
                    ),
                    node(
                        "t",
                        Style::default(),
                        Element::Transcript {
                            label: "Chat".into(),
                            children: vec![],
                            earlier: None,
                            source: None,
                        },
                    ),
                ],
            },
        ));
        assert!(scene.unsupported.contains("surface"));
        assert!(scene.unsupported.contains("transcript"));
        assert_eq!(scene.texts(), vec!["Something"]);
    }
}
