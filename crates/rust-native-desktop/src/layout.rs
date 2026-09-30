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
//!   `unchecked` or `checked` glyph. Other glyphs are vector icons beside
//!   the label, or on their own in a circular button.
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
    /// Starts a clipped drawing region; clips nest.
    PushClip(Rect),
    PopClip,
    Glyph {
        rect: Rect,
        glyph: Glyph,
        color: Color,
    },
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
    Check {
        rect: Rect,
        color: Color,
    },
    /// The application paints the surface `resource` here.
    Surface {
        resource: String,
        rect: Rect,
        /// An application-owned drawing revision. `None` refreshes every frame.
        version: Option<u64>,
    },
}

/// A rectangle that activates a node.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub rect: Rect,
    pub key: String,
    pub enabled: bool,
    /// The visible portion of a scroll container, when this control is in one.
    pub clip: Option<Rect>,
}

/// A laid-out view.
#[derive(Clone, Debug, Default)]
pub struct Scene {
    pub ops: Vec<Op>,
    /// Every button, in the view's order, which is also the focus order.
    pub hits: Vec<Hit>,
    /// Bounds of semantic nodes, in logical points, for modal and accessibility adapters.
    pub bounds: std::collections::BTreeMap<String, Rect>,
    /// The height of everything drawn, with the margins, in points.
    pub height: f32,
    /// Elements and properties this adapter drew differently, or not at
    /// all, such as `transcript` or `icon.glyph`.
    pub unsupported: BTreeSet<&'static str>,
    /// Separate scroll regions and the resize seam in a split window.
    pub split: Option<SplitRegions>,
}

impl Scene {
    /// The enabled button under `x`, `y`, if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<&Hit> {
        self.hits.iter().rev().find(|hit| {
            hit.enabled
                && hit.rect.contains(x, y)
                && hit.clip.is_none_or(|clip| clip.contains(x, y))
        })
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
    pub leading_scroll: f32,
    pub content_scroll: f32,
}

/// Window layout outside the semantic view contract. A split view is a
/// horizontal stack of two vertical stacks, each with header, body, and footer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WindowLayout {
    #[default]
    Column,
    Split(SplitLayout),
}

/// Sizing for a leading pane and a content pane, in logical points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitLayout {
    pub leading_width: f32,
    pub min_leading_width: f32,
    pub max_leading_width: f32,
    pub min_content_width: f32,
    pub collapsed: bool,
    pub center_content: bool,
}

impl SplitLayout {
    /// Clamps the leading pane and reserves room for content at narrow sizes.
    pub fn width_at(self, width: f32) -> f32 {
        if self.collapsed {
            return 0.0;
        }
        let min = self.min_leading_width.max(1.0);
        let max = self.max_leading_width.max(min);
        let requested = if self.leading_width.is_finite() {
            self.leading_width
        } else {
            min
        };
        requested
            .clamp(min, max)
            .min((width - self.min_content_width.max(1.0)).max(0.0))
    }
}

/// A scrollable body; header and footer are outside its clip.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollRegion {
    pub rect: Rect,
    pub limit: f32,
    pub offset: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct SplitRegions {
    pub leading: ScrollRegion,
    pub content: ScrollRegion,
    pub divider: Option<Rect>,
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
        paint_clip: None,
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

/// Lays out a column or a split window. A malformed split falls back to the
/// column and reports that the split layout is unsupported.
#[allow(clippy::too_many_arguments)]
pub fn lay_out_with_layout<I>(
    view: &View<I>,
    theme: &Theme,
    fonts: &mut Fonts,
    sizes: &SurfaceSizes<'_>,
    interaction: &Interaction,
    width: f32,
    height: f32,
    layout: WindowLayout,
) -> Scene {
    let WindowLayout::Split(split) = layout else {
        return lay_out_window(view, theme, fonts, sizes, interaction, width, height);
    };
    let Element::Stack {
        axis: Axis::Horizontal,
        children,
    } = &view.root.element
    else {
        let mut scene = lay_out_window(view, theme, fonts, sizes, interaction, width, height);
        scene.unsupported.insert("window.split");
        return scene;
    };
    if children.len() != 2 || children.iter().any(|pane| {
        !matches!(&pane.element, Element::Stack { axis: Axis::Vertical, children } if children.len() == 3)
    }) {
        let mut scene = lay_out_window(view, theme, fonts, sizes, interaction, width, height);
        scene.unsupported.insert("window.split");
        return scene;
    }
    let leading_width = split.width_at(width);
    let mut engine = Engine {
        theme,
        fonts,
        sizes,
        interaction,
        scene: Scene::default(),
        paint_clip: None,
    };
    let leading = if leading_width > 0.0 {
        engine.docked_pane(
            &children[0],
            Rect {
                x: 0.0,
                y: 0.0,
                w: leading_width,
                h: height,
            },
            12.0,
            interaction.leading_scroll,
            false,
        )
    } else {
        ScrollRegion::default()
    };
    let content = engine.docked_pane(
        &children[1],
        Rect {
            x: leading_width + 8.0,
            y: 8.0,
            w: (width - leading_width - 16.0).max(1.0),
            h: (height - 16.0).max(1.0),
        },
        20.0,
        interaction.content_scroll,
        split.center_content,
    );
    let divider = (leading_width > 0.0).then_some(Rect {
        x: leading_width - 4.0,
        y: 0.0,
        w: 8.0,
        h: height,
    });
    if let Some(divider) = divider {
        engine.scene.ops.push(Op::Fill {
            rect: Rect {
                x: divider.x + 3.0,
                w: 1.0,
                ..divider
            },
            radius: 0.0,
            color: theme.rule,
        });
    }
    engine.scene.height = height;
    engine.scene.split = Some(SplitRegions {
        leading,
        content,
        divider,
    });
    engine.scene
}

struct Engine<'a> {
    theme: &'a Theme,
    fonts: &'a mut Fonts,
    sizes: &'a SurfaceSizes<'a>,
    interaction: &'a Interaction,
    scene: Scene,
    paint_clip: Option<Rect>,
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
    if matches!(node.element, Element::Button { .. }) && node.style.align == Some(TextAlign::Start)
    {
        return false;
    }
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
    fn docked_pane<I>(
        &mut self,
        node: &Node<I>,
        rect: Rect,
        inset: f32,
        scroll: f32,
        center: bool,
    ) -> ScrollRegion {
        let Element::Stack { children, .. } = &node.element else {
            unreachable!("checked pane")
        };
        self.scene.ops.push(Op::PushClip(rect));
        if let Some(color) = node.style.background {
            self.scene.ops.push(Op::Fill {
                rect,
                radius: if center { 12.0 } else { 0.0 },
                color,
            });
        }
        let width = (rect.w - 2.0 * inset).max(1.0);
        let header_height = self.size(&children[0], width).1;
        let footer_height = self.size(&children[2], width).1;
        self.place(&children[0], rect.x + inset, rect.y + inset, width);
        let body_rect = Rect {
            x: rect.x + inset,
            y: rect.y + inset + header_height + 16.0,
            w: width,
            h: (rect.h - 2.0 * inset - header_height - footer_height - 32.0).max(1.0),
        };
        let body_width = if center {
            width.min(self.theme.column)
        } else {
            width
        };
        let body_height = self.size(&children[1], body_width).1;
        let limit = (body_height - body_rect.h).max(0.0);
        let offset = if scroll.is_finite() {
            scroll.clamp(0.0, limit)
        } else {
            0.0
        };
        let top = if center {
            ((body_rect.h - body_height) / 2.0).max(0.0)
        } else {
            0.0
        };
        self.scene.ops.push(Op::PushClip(body_rect));
        let first_hit = self.scene.hits.len();
        let previous_clip = self.paint_clip.replace(body_rect);
        self.place(
            &children[1],
            body_rect.x + (width - body_width) / 2.0,
            body_rect.y + top - offset,
            body_width,
        );
        self.paint_clip = previous_clip;
        for hit in &mut self.scene.hits[first_hit..] {
            hit.clip = Some(body_rect);
        }
        self.scene.ops.push(Op::PopClip);
        if limit > 0.0 {
            let h = (body_rect.h * body_rect.h / body_height)
                .max(24.0)
                .min(body_rect.h);
            self.scene.ops.push(Op::Fill {
                rect: Rect {
                    x: rect.x + rect.w - 5.0,
                    y: body_rect.y + offset / limit * (body_rect.h - h),
                    w: 3.0,
                    h,
                },
                radius: 1.5,
                color: self.theme.rule,
            });
        }
        self.place(
            &children[2],
            rect.x + inset,
            rect.y + rect.h - inset - footer_height,
            width,
        );
        self.scene.ops.push(Op::PopClip);
        ScrollRegion {
            rect: body_rect,
            limit,
            offset,
        }
    }

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
                } else if icon.is_some_and(|icon| icon.circular) {
                    (32.0, 32.0)
                } else if transparent(node.style.background) {
                    let icon_width = if icon.is_some() { 24.0 } else { 0.0 };
                    let paragraph = self.paragraph(
                        label,
                        TextRole::Body,
                        &node.style,
                        Some((inner - icon_width).max(1.0)),
                    );
                    (
                        if node.style.align == Some(TextAlign::Start) {
                            inner
                        } else {
                            paragraph.width + icon_width
                        },
                        paragraph.height + 4.0,
                    )
                } else {
                    let icon_width = if icon.is_some() { 24.0 } else { 0.0 };
                    let paragraph = self.paragraph(
                        label,
                        TextRole::Body,
                        &Style {
                            weight: node.style.weight.or(Some(TextWeight::Bold)),
                            ..node.style
                        },
                        Some((inner - 2.0 * BUTTON_PAD.0 - icon_width).max(1.0)),
                    );
                    (
                        if node.style.align == Some(TextAlign::Start) {
                            inner
                        } else {
                            paragraph.width + 2.0 * BUTTON_PAD.0 + icon_width
                        },
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
                if let Some((w, h)) = (self.sizes)(&format!("composer:{}", node.key), inner) {
                    return ((w + start + end).min(available), h + top + bottom);
                }
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
        self.scene.bounds.insert(
            node.key.clone(),
            Rect {
                x,
                y,
                w: width,
                h: height,
            },
        );
        let [top, end, bottom, start] = padding(&node.style);
        let inner = (width - start - end).max(1.0);
        let (ix, iy) = (x + start, y + top);
        if self
            .paint_clip
            .is_some_and(|clip| y + height <= clip.y || y >= clip.y + clip.h)
        {
            match &node.element {
                Element::Button { enabled, .. } => {
                    // Retain keyboard order and scroll targets without creating
                    // foreground operations for rows outside the viewport.
                    self.scene.hits.push(Hit {
                        rect: Rect {
                            x: ix,
                            y: iy,
                            w: inner,
                            h: (height - top - bottom).max(1.0),
                        },
                        key: node.key.clone(),
                        enabled: *enabled,
                        clip: self.paint_clip,
                    });
                    return;
                }
                Element::Text { .. } | Element::Surface { .. } => return,
                _ => {}
            }
        }
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
                let resource = format!("composer:{}", node.key);
                if let Some((w, h)) = (self.sizes)(&resource, inner) {
                    self.scene.ops.push(Op::Surface {
                        version: None,
                        resource,
                        rect: Rect {
                            x: ix,
                            y: iy,
                            w: w.min(inner),
                            h,
                        },
                    });
                    return;
                }
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
                        version: None,
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
        } else if let Some(icon) = icon.filter(|icon| icon.circular) {
            rect = Rect {
                x,
                y,
                w: 32.0,
                h: 32.0,
            };
            let base = node.style.background.unwrap_or(theme.button);
            let color = node.style.foreground.unwrap_or(theme.text);
            let color = if enabled { color } else { theme.muted };
            if base.alpha > 0 || (hovered && enabled) || (pressed && enabled) {
                self.scene.ops.push(Op::Fill {
                    rect,
                    radius: 8.0,
                    color: if hovered || pressed {
                        mix(theme.background, theme.text, 0.08)
                    } else {
                        base
                    },
                });
            }
            self.scene.ops.push(Op::Glyph {
                rect: Rect {
                    x: x + 8.0,
                    y: y + 8.0,
                    w: 16.0,
                    h: 16.0,
                },
                glyph: icon.glyph,
                color,
            });
        } else if transparent(node.style.background) {
            let icon_width = if icon.is_some() { 24.0 } else { 0.0 };
            let paragraph = self.paragraph(
                label,
                TextRole::Body,
                &node.style,
                Some((inner - icon_width).max(1.0)),
            );
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
                w: if node.style.align == Some(TextAlign::Start) {
                    inner
                } else {
                    paragraph.width + icon_width
                },
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
            if let Some(icon) = icon {
                self.scene.ops.push(Op::Glyph {
                    rect: Rect {
                        x,
                        y: y + 2.0,
                        w: 16.0,
                        h: 16.0,
                    },
                    glyph: icon.glyph,
                    color,
                });
            }
            self.text(
                paragraph,
                x + icon_width,
                y + 1.0,
                inner - icon_width,
                TextAlign::Start,
                color,
            );
        } else {
            let icon_width = if icon.is_some() { 24.0 } else { 0.0 };
            let style = Style {
                weight: node.style.weight.or(Some(TextWeight::Bold)),
                ..node.style
            };
            let paragraph = self.paragraph(
                label,
                TextRole::Body,
                &style,
                Some((inner - 2.0 * BUTTON_PAD.0 - icon_width).max(1.0)),
            );
            rect = Rect {
                x,
                y,
                w: if node.style.align == Some(TextAlign::Start) {
                    inner
                } else {
                    paragraph.width + 2.0 * BUTTON_PAD.0 + icon_width
                },
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
                fill = mix(fill, theme.text, 0.06);
            }
            self.scene.ops.push(Op::Fill {
                rect,
                radius,
                color: fill,
            });
            if let Some(icon) = icon {
                self.scene.ops.push(Op::Glyph {
                    rect: Rect {
                        x: x + BUTTON_PAD.0,
                        y: y + BUTTON_PAD.1 + 1.0,
                        w: 16.0,
                        h: 16.0,
                    },
                    glyph: icon.glyph,
                    color,
                });
            }
            self.text(
                paragraph,
                x + BUTTON_PAD.0 + icon_width,
                y + BUTTON_PAD.1,
                rect.w - 2.0 * BUTTON_PAD.0 - icon_width,
                node.style.align.unwrap_or(TextAlign::Center),
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
            clip: None,
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

    fn split_fixture(interaction: &Interaction, collapsed: bool) -> Scene {
        let vertical = |key: &str, children| {
            node(
                key,
                Style::default(),
                Element::Stack {
                    axis: Axis::Vertical,
                    children,
                },
            )
        };
        let rows = (0..30)
            .map(|index| {
                let mut row = button(&format!("chat-{index}"), "A sample chat", None);
                row.style.align = Some(TextAlign::Start);
                row.style.background = Some(Color::rgb(190, 30, 30));
                row
            })
            .collect();
        let leading = vertical(
            "leading",
            vec![
                button(
                    "leading-header",
                    "New chat",
                    Some(Icon {
                        glyph: Glyph::Compose,
                        circular: false,
                        pill: false,
                    }),
                ),
                vertical("leading-body", rows),
                button("leading-footer", "Computers", None),
            ],
        );
        let content = vertical(
            "content",
            vec![
                text("content-header", "Chat"),
                text("content-body", "Welcome"),
                text("content-footer", "Preview"),
            ],
        );
        let view = View::new(
            "split-test",
            1,
            node(
                "root",
                Style::default(),
                Element::Stack {
                    axis: Axis::Horizontal,
                    children: vec![leading, content],
                },
            ),
        );
        lay_out_with_layout(
            &view,
            &Theme::default(),
            &mut Fonts::new(),
            &|_, _| None,
            interaction,
            960.0,
            540.0,
            WindowLayout::Split(SplitLayout {
                leading_width: 280.0,
                min_leading_width: 224.0,
                max_leading_width: 400.0,
                min_content_width: 360.0,
                collapsed,
                center_content: true,
            }),
        )
    }

    #[test]
    fn split_scrolling_clips_rows_and_preserves_header_footer_and_content() {
        let initial = split_fixture(&Interaction::default(), false);
        let scrolled = split_fixture(
            &Interaction {
                leading_scroll: 500.0,
                ..Interaction::default()
            },
            false,
        );
        assert!(initial.unsupported.is_empty());
        assert_eq!(scrolled.split.expect("regions").content.offset, 0.0);
        for key in ["leading-header", "leading-footer"] {
            assert_eq!(
                initial
                    .hits
                    .iter()
                    .find(|hit| hit.key == key)
                    .expect("control")
                    .rect,
                scrolled
                    .hits
                    .iter()
                    .find(|hit| hit.key == key)
                    .expect("control")
                    .rect
            );
        }
        let hidden = scrolled
            .hits
            .iter()
            .find(|hit| hit.key == "chat-0")
            .expect("first row");
        assert!(
            scrolled
                .hit(hidden.rect.x + 2.0, hidden.rect.y + 2.0)
                .is_none()
        );
        let mut before = crate::Frame::new(960, 540, Theme::default().background);
        let mut after = before.clone();
        let mut fonts = Fonts::new();
        crate::paint::paint(
            &initial,
            &mut before,
            1.0,
            0.0,
            &mut fonts,
            &mut |_, _, _| {},
        );
        crate::paint::paint(
            &scrolled,
            &mut after,
            1.0,
            0.0,
            &mut fonts,
            &mut |_, _, _| {},
        );
        for y in 0..65 {
            for x in 0..280 {
                assert_eq!(before.pixel(x, y), after.pixel(x, y), "header at {x},{y}");
            }
        }
        assert_eq!(
            initial.split.expect("regions").leading.rect,
            scrolled.split.expect("regions").leading.rect
        );
    }

    #[test]
    fn collapse_removes_leading_controls_from_pointer_and_focus_access() {
        let scene = split_fixture(&Interaction::default(), true);
        assert!(scene.split.expect("regions").divider.is_none());
        assert!(
            scene
                .hits
                .iter()
                .all(|hit| !hit.key.starts_with("leading-") && !hit.key.starts_with("chat-"))
        );
        let split = SplitLayout {
            leading_width: 900.0,
            min_leading_width: 224.0,
            max_leading_width: 400.0,
            min_content_width: 360.0,
            collapsed: false,
            center_content: true,
        };
        assert_eq!(split.width_at(960.0), 400.0);
        assert_eq!(split.width_at(500.0), 140.0);
    }
}
