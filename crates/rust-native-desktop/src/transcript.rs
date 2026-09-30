//! A virtualized desktop transcript over the shared Rust Native row layout.

use crate::text::Fonts;
use crate::{Frame, PxRect};
use rust_native::Node;
use rust_native::layout::display::{ColorRole, Ink, Weight, WidgetKind};
use rust_native::layout::{TranscriptLayout, Update, shape::ShapingMeasurer};
use rust_native::style::Color;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// An explicit interaction with transcript content, admitted by the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    OpenLink(String),
    Copy(String),
    Earlier,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point {
    row: usize,
    text: usize,
    byte: usize,
}

/// Exact row heights, reading position, selection, and horizontally scrolled blocks.
pub struct Transcript {
    version: u64,
    expanded: BTreeSet<String>,
    pub relaid: usize,
    pub measured: u64,
    pressed_widget: Option<(String, usize)>,
    layout: TranscriptLayout,
    measurer: ShapingMeasurer,
    frame: Arc<rust_native::layout::Frame>,
    rows: HashMap<String, Node<()>>,
    order: Vec<String>,
    height: f32,
    offset: f32,
    follow: bool,
    selection: Option<(Point, Point)>,
    dragging: bool,
    horizontal: HashMap<(String, usize), f32>,
}

impl Default for Transcript {
    fn default() -> Self {
        let mut layout = TranscriptLayout::new();
        let frame = layout.frame();
        Self {
            version: 0,
            expanded: BTreeSet::new(),
            relaid: 0,
            measured: 0,
            pressed_widget: None,
            layout,
            measurer: ShapingMeasurer::new(),
            frame,
            rows: HashMap::new(),
            order: vec![],
            height: 1.0,
            offset: 0.0,
            follow: true,
            selection: None,
            dragging: false,
            horizontal: HashMap::new(),
        }
    }
}

impl Transcript {
    pub fn rows(&self) -> usize {
        self.frame.len()
    }
    pub fn height(&self) -> f32 {
        self.frame.height()
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn dragging(&self) -> bool {
        self.dragging
    }

    /// Update only changed rows. Readers away from the tail keep their visible anchor.
    pub fn update(
        &mut self,
        rows: Vec<Node<()>>,
        width: f32,
        height: f32,
    ) -> Result<(), rust_native::layout::LayoutError> {
        let anchor = self
            .frame
            .rows_in(self.offset, self.offset + 1.0)
            .next()
            .and_then(|index| {
                Some((
                    self.frame.key(index)?.to_owned(),
                    self.offset - self.frame.placement(index)?.y,
                ))
            });
        let order: Vec<String> = rows.iter().map(|row| row.key.clone()).collect();
        let changed: Vec<Node<()>> = rows
            .iter()
            .filter(|row| self.rows.get(&row.key) != Some(*row))
            .cloned()
            .collect();
        let drawing_changed = !changed.is_empty()
            || order != self.order
            || width != self.frame.width()
            || height != self.height;
        let summary = self.layout.update(
            Update {
                width: width.clamp(1.0, 16384.0),
                scale: 1.0,
                order: (order != self.order).then_some(order.clone()),
                rows: changed,
                expanded: self.expanded.iter().cloned().collect(),
                ..Update::default()
            },
            &mut self.measurer,
        )?;
        self.relaid = summary.relaid;
        self.measured = summary.measured;
        if drawing_changed || summary.relaid > 0 {
            self.version = self.version.wrapping_add(1);
        }
        if order != self.order {
            self.selection = None;
            self.pressed_widget = None;
        }
        self.order = order;
        self.rows = rows.into_iter().map(|row| (row.key.clone(), row)).collect();
        self.frame = self.layout.frame();
        self.height = height;
        if self.follow {
            self.offset = self.limit();
        } else if let Some((key, dy)) = anchor
            && let Some(index) = self.frame.find(&key)
        {
            self.offset = self
                .frame
                .placement(index)
                .map_or(self.offset, |row| row.y + dy);
        }
        self.offset = self.offset.clamp(0.0, self.limit());
        Ok(())
    }

    fn limit(&self) -> f32 {
        (self.frame.height() - self.height).max(0.0)
    }
    pub fn at_tail(&self) -> bool {
        self.follow
    }
    pub fn jump_to_tail(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.follow = true;
        self.offset = self.limit();
    }
    pub fn scroll(&mut self, dy: f32) {
        let previous = self.offset;
        self.offset = (self.offset - dy).clamp(0.0, self.limit());
        self.follow = self.limit() - self.offset < 20.0;
        if self.offset != previous {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// Scroll a wide code block or table when the pointer is inside it.
    pub fn scroll_horizontal(&mut self, x: f32, y: f32, dx: f32) {
        let y = y + self.offset;
        for index in self.frame.rows_in(y, y + 1.0) {
            let row = self.frame.display(index).unwrap();
            let top = self.frame.placement(index).unwrap().y;
            for (region, scroller) in row.scrollers.iter().enumerate() {
                if x >= scroller.x
                    && x < scroller.x + scroller.w
                    && y >= top + scroller.y
                    && y < top + scroller.y + scroller.h
                {
                    let entry = self
                        .horizontal
                        .entry((row.key.clone(), region))
                        .or_default();
                    let previous = *entry;
                    *entry = (*entry - dx).clamp(0.0, scroller.content_w - scroller.w);
                    if previous != *entry {
                        self.version = self.version.wrapping_add(1);
                    }
                }
            }
        }
    }

    fn point(&self, x: f32, y: f32, fonts: &mut Fonts) -> Option<Point> {
        let y = y + self.offset;
        for index in self.frame.rows_in(y, y + 1.0) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y;
            for (run_index, run) in row.runs.iter().enumerate() {
                let style = row.styles.get(run.style as usize)?;
                let mut rx = run.x;
                for (region, scroller) in row.scrollers.iter().enumerate() {
                    if (scroller.runs[0]..scroller.runs[1]).contains(&(run_index as u32)) {
                        rx -= self
                            .horizontal
                            .get(&(row.key.clone(), region))
                            .copied()
                            .unwrap_or_default();
                    }
                }
                if y >= top + run.baseline - style.font.size
                    && y <= top + run.baseline + style.font.size * 0.3
                    && x >= rx
                    && x <= rx + run.width + 8.0
                {
                    let text = &row.texts[run.text as usize];
                    let start = run.start8 as usize;
                    let end = start + run.len8 as usize;
                    let nearest = start + fonts.caret_byte(&text[start..end], style.font, x - rx);
                    return Some(Point {
                        row: index,
                        text: run.text as usize,
                        byte: nearest,
                    });
                }
            }
        }
        None
    }

    pub fn pointer(
        &mut self,
        event: crate::input::SurfaceInput,
        fonts: &mut Fonts,
    ) -> Option<Action> {
        use crate::input::SurfaceInput;
        let previous = self.selection;
        match event {
            SurfaceInput::Down { x, y, shift } => {
                self.pressed_widget = self.widget(x, y);
                if self.pressed_widget.is_some() {
                    self.dragging = false;
                    return None;
                }
                if let Some(point) = self.point(x, y, fonts) {
                    let anchor = if shift {
                        self.selection.map_or(point, |(anchor, _)| anchor)
                    } else {
                        point
                    };
                    self.selection = Some((anchor, point));
                    self.dragging = true;
                } else {
                    self.selection = None;
                }
            }
            SurfaceInput::Move { x, y } if self.dragging => {
                if let Some(point) = self.point(x, y, fonts)
                    && let Some((anchor, _)) = self.selection
                {
                    self.selection = Some((anchor, point));
                }
            }
            SurfaceInput::Up { x, y } => {
                self.dragging = false;
                if let Some(pressed) = self.pressed_widget.take() {
                    if self.widget(x, y).as_ref() == Some(&pressed) {
                        let index = self.frame.find(&pressed.0)?;
                        let widget = self
                            .frame
                            .display(index)?
                            .widgets
                            .get(pressed.1)?
                            .kind
                            .clone();
                        return match widget {
                            WidgetKind::Copy { text } => Some(Action::Copy(text)),
                            WidgetKind::Earlier { loading: false } => Some(Action::Earlier),
                            WidgetKind::Toggle { key, expanded } => {
                                if expanded {
                                    self.expanded.remove(&key);
                                } else {
                                    self.expanded.insert(key);
                                }
                                let rows = self
                                    .order
                                    .iter()
                                    .filter_map(|key| self.rows.get(key).cloned())
                                    .collect();
                                let _ = self.update(rows, self.frame.width(), self.height);
                                None
                            }
                            _ => None,
                        };
                    }
                    return None;
                }
                if self.selection.is_none_or(|(a, b)| a == b) {
                    let ry = y + self.offset;
                    for index in self.frame.rows_in(ry, ry + 1.0) {
                        let row = self.frame.display(index)?;
                        let top = self.frame.placement(index)?.y;
                        for link in &row.links {
                            if x >= link.x
                                && x <= link.x + link.w
                                && ry >= top + link.y
                                && ry <= top + link.y + link.h
                            {
                                return Some(Action::OpenLink(link.destination.clone()));
                            }
                        }
                    }
                }
            }
            SurfaceInput::Wheel { x, y, dx, dy } => {
                self.scroll(dy);
                self.scroll_horizontal(x, y, dx);
            }
            _ => {}
        }
        if previous != self.selection {
            self.version = self.version.wrapping_add(1);
        }
        None
    }

    fn widget(&self, x: f32, y: f32) -> Option<(String, usize)> {
        let y = y + self.offset;
        for index in self.frame.rows_in(y, y + 1.0) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y;
            for (index, widget) in row.widgets.iter().enumerate().rev() {
                if matches!(
                    widget.kind,
                    WidgetKind::Copy { .. }
                        | WidgetKind::Toggle { .. }
                        | WidgetKind::Earlier { loading: false }
                ) && x >= widget.x
                    && x < widget.x + widget.w
                    && y >= top + widget.y
                    && y < top + widget.y + widget.h
                {
                    return Some((row.key.clone(), index));
                }
            }
        }
        None
    }
    pub fn visible_rows(&self) -> usize {
        self.frame
            .rows_in(self.offset, self.offset + self.height)
            .len()
    }
    pub fn reading_anchor(&self) -> Option<(String, f32)> {
        let index = self.frame.rows_in(self.offset, self.offset + 1.0).next()?;
        Some((
            self.frame.key(index)?.to_owned(),
            self.offset - self.frame.placement(index)?.y,
        ))
    }

    pub fn selected_text(&self) -> String {
        let Some((mut a, mut b)) = self.selection else {
            return String::new();
        };
        if (a.row, a.text, a.byte) > (b.row, b.text, b.byte) {
            std::mem::swap(&mut a, &mut b);
        }
        let mut pieces = Vec::new();
        for index in a.row..=b.row {
            let Some(row) = self.frame.display(index) else {
                continue;
            };
            for (text_index, text) in row.texts.iter().enumerate() {
                if (index, text_index) < (a.row, a.text) || (index, text_index) > (b.row, b.text) {
                    continue;
                }
                let start = if index == a.row && text_index == a.text {
                    a.byte
                } else {
                    0
                };
                let end = if index == b.row && text_index == b.text {
                    b.byte
                } else {
                    text.len()
                };
                if let Some(part) = text.get(start..end) {
                    pieces.push(part.to_owned());
                }
            }
        }
        pieces.join("\n")
    }

    /// Paint only rows in the viewport, with clipping for wide code and tables.
    pub fn paint(&self, frame: &mut Frame, rect: PxRect, scale: f32, fonts: &mut Fonts) {
        let previous = frame.clip_to(rect);
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let row = self.frame.display(index).unwrap();
            let y = rect.y + (self.frame.placement(index).unwrap().y - self.offset) * scale;
            let selected = self.selection.map(|(mut a, mut b)| {
                if (a.row, a.text, a.byte) > (b.row, b.text, b.byte) {
                    std::mem::swap(&mut a, &mut b);
                }
                (a, b)
            });
            for (i, shape) in row.rects.iter().enumerate() {
                let mut dx = 0.0;
                let mut clip = None;
                for (region, scroller) in row.scrollers.iter().enumerate() {
                    if (scroller.rects[0]..scroller.rects[1]).contains(&(i as u32)) {
                        dx = self
                            .horizontal
                            .get(&(row.key.clone(), region))
                            .copied()
                            .unwrap_or_default();
                        clip = Some(frame.clip_to(PxRect {
                            x: rect.x + scroller.x * scale,
                            y: y + scroller.y * scale,
                            w: scroller.w * scale,
                            h: scroller.h * scale,
                        }));
                    }
                }
                let bounds = PxRect {
                    x: rect.x + (shape.x - dx) * scale,
                    y: y + shape.y * scale,
                    w: shape.w * scale,
                    h: shape.h * scale,
                };
                if let Some(fill) = shape.fill {
                    frame.fill(bounds, shape.radii[0] * scale, ink(fill));
                }
                if let Some(stroke) = shape.stroke {
                    frame.stroke(bounds, shape.radii[0] * scale, scale, ink(stroke));
                }
                if let Some(clip) = clip {
                    frame.restore_clip(clip);
                }
            }
            for (i, run) in row.runs.iter().enumerate() {
                let style = &row.styles[run.style as usize];
                let text = &row.texts[run.text as usize];
                let start = run.start8 as usize;
                let end = start + run.len8 as usize;
                let mut dx = 0.0;
                let mut clip = None;
                for (region, scroller) in row.scrollers.iter().enumerate() {
                    if (scroller.runs[0]..scroller.runs[1]).contains(&(i as u32)) {
                        dx = self
                            .horizontal
                            .get(&(row.key.clone(), region))
                            .copied()
                            .unwrap_or_default();
                        clip = Some(frame.clip_to(PxRect {
                            x: rect.x + scroller.x * scale,
                            y: y + scroller.y * scale,
                            w: scroller.w * scale,
                            h: scroller.h * scale,
                        }));
                    }
                }
                let x = rect.x + (run.x - dx) * scale;
                let baseline = y + run.baseline * scale;
                if let Some((a, b)) = selected {
                    let here = (index, run.text as usize);
                    if here >= (a.row, a.text) && here <= (b.row, b.text) {
                        let lo = if here == (a.row, a.text) {
                            a.byte.max(start)
                        } else {
                            start
                        };
                        let hi = if here == (b.row, b.text) {
                            b.byte.min(end)
                        } else {
                            end
                        };
                        if lo < hi {
                            let left = fonts.paragraph(&text[start..lo], style.font, None).width;
                            let width = fonts.paragraph(&text[lo..hi], style.font, None).width;
                            frame.fill(
                                PxRect {
                                    x: x + left * scale,
                                    y: baseline - style.font.size * scale,
                                    w: width * scale,
                                    h: style.font.size * 1.3 * scale,
                                },
                                0.0,
                                Color::rgb(54, 78, 105),
                            );
                        }
                    }
                }
                let mut color = ink(style.ink);
                color.alpha = (f32::from(color.alpha) * style.opacity) as u8;
                let truncated = run
                    .truncate
                    .map(|width| fonts.ellipsized(&text[start..end], style.font, width));
                fonts.draw_run(
                    frame,
                    truncated.as_deref().unwrap_or(&text[start..end]),
                    style.font,
                    x,
                    baseline,
                    scale,
                    color,
                );
                if style.underline {
                    frame.line(
                        (x, baseline + 2.0 * scale),
                        (x + run.width * scale, baseline + 2.0 * scale),
                        scale,
                        color,
                    );
                }
                if style.strike {
                    frame.line(
                        (x, baseline - style.font.size * 0.3 * scale),
                        (
                            x + run.width * scale,
                            baseline - style.font.size * 0.3 * scale,
                        ),
                        scale,
                        color,
                    );
                }
                if let Some(clip) = clip {
                    frame.restore_clip(clip);
                }
            }
            for widget in &row.widgets {
                let bounds = PxRect {
                    x: rect.x + widget.x * scale,
                    y: y + widget.y * scale,
                    w: widget.w * scale,
                    h: widget.h * scale,
                };
                if !frame.visible(bounds) {
                    continue;
                }
                let color = ink(Ink::Role(ColorRole::Secondary));
                match &widget.kind {
                    WidgetKind::Copy { .. } => {
                        let paragraph = fonts.paragraph(
                            "Copy",
                            crate::text::font(12.0, Weight::Medium, false),
                            None,
                        );
                        fonts.draw(
                            frame,
                            &paragraph,
                            bounds.x,
                            bounds.y + (bounds.h - paragraph.height * scale) / 2.0,
                            widget.w,
                            rust_native::style::TextAlign::Center,
                            scale,
                            color,
                        );
                    }
                    WidgetKind::Chevron { expanded } => {
                        let point =
                            |x: f32, y: f32| (bounds.x + x * bounds.w, bounds.y + y * bounds.h);
                        let (a, b, c) = if *expanded {
                            (point(0.2, 0.3), point(0.5, 0.7), point(0.8, 0.3))
                        } else {
                            (point(0.3, 0.2), point(0.7, 0.5), point(0.3, 0.8))
                        };
                        frame.line(a, b, scale, color);
                        frame.line(b, c, scale, color);
                    }
                    WidgetKind::Checkbox { checked } => {
                        frame.stroke(bounds, 2.0 * scale, scale, color);
                        if *checked {
                            crate::icons::draw(frame, bounds, rust_native::Glyph::Check, color);
                        }
                    }
                    WidgetKind::Status {
                        state: rust_native::ToolState::Done,
                    } => crate::icons::draw(frame, bounds, rust_native::Glyph::Check, color),
                    WidgetKind::Status {
                        state: rust_native::ToolState::Failed,
                    } => {
                        frame.line(
                            (bounds.x, bounds.y),
                            (bounds.x + bounds.w, bounds.y + bounds.h),
                            scale,
                            color,
                        );
                        frame.line(
                            (bounds.x + bounds.w, bounds.y),
                            (bounds.x, bounds.y + bounds.h),
                            scale,
                            color,
                        );
                    }
                    WidgetKind::Spinner
                    | WidgetKind::Working
                    | WidgetKind::Status {
                        state: rust_native::ToolState::Running,
                    } => {
                        for i in 0..3 {
                            frame.fill(
                                PxRect {
                                    x: bounds.x + i as f32 * bounds.w / 3.0,
                                    y: bounds.y + bounds.h * 0.4,
                                    w: bounds.w / 6.0,
                                    h: bounds.w / 6.0,
                                },
                                bounds.w / 12.0,
                                color,
                            );
                        }
                    }
                    WidgetKind::Toggle { .. } | WidgetKind::Earlier { .. } => {}
                }
            }
        }
        frame.restore_clip(previous);
    }
}

fn ink(ink: Ink) -> Color {
    match ink {
        Ink::Rgba([red, green, blue, alpha]) => Color {
            red,
            green,
            blue,
            alpha,
        },
        Ink::Role(role) => match role {
            ColorRole::Primary => Color::rgb(230, 232, 235),
            ColorRole::Tertiary => Color::rgb(103, 111, 122),
            ColorRole::Secondary => Color::rgb(150, 155, 163),
            ColorRole::Link => Color::rgb(158, 201, 242),
            ColorRole::Bubble => Color::rgb(35, 40, 48),
            ColorRole::Surface => Color::rgb(20, 23, 28),
            ColorRole::Raised => Color::rgb(38, 43, 51),
            ColorRole::Border => Color::rgb(61, 68, 78),
            ColorRole::InlineCode => Color::rgb(41, 46, 55),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::SurfaceInput;
    use rust_native::{Element, MessageRole, ToolState, style::Style};
    fn row(key: &str, value: &str) -> Node<()> {
        Node {
            key: key.into(),
            style: Style::default(),
            element: Element::Message {
                role: MessageRole::Assistant,
                note: None,
                children: vec![Node {
                    key: format!("{key}-body"),
                    style: Style::default(),
                    element: Element::Markdown {
                        blocks: rust_native::markdown::parse(value),
                    },
                }],
            },
        }
    }
    #[test]
    fn reading_anchor_survives_streaming_and_prepending_and_tail_can_resume() {
        let mut transcript = Transcript::default();
        let mut rows: Vec<_> = (0..40)
            .map(|i| {
                row(
                    &format!("row-{i}"),
                    "A paragraph with some text.\n\nAnother paragraph.",
                )
            })
            .collect();
        transcript.update(rows.clone(), 500.0, 180.0).unwrap();
        assert!(transcript.at_tail());
        assert!(transcript.visible_rows() < 10);
        transcript.scroll(300.0);
        assert!(!transcript.at_tail());
        let anchor = transcript.reading_anchor().unwrap();
        rows[39] = row(
            "row-39",
            "A much longer streaming response.\n\nAdditional paragraphs.\n\nMore words.",
        );
        transcript.update(rows.clone(), 500.0, 180.0).unwrap();
        assert_eq!(transcript.relaid, 1);
        assert_eq!(transcript.reading_anchor().unwrap(), anchor);
        rows.insert(0, row("earlier", "An earlier page."));
        transcript.update(rows.clone(), 500.0, 180.0).unwrap();
        assert_eq!(transcript.reading_anchor().unwrap(), anchor);
        let version = transcript.version();
        transcript.update(rows, 500.0, 180.0).unwrap();
        assert_eq!(transcript.relaid, 0);
        assert_eq!(transcript.measured, 0);
        assert_eq!(transcript.version(), version);
        transcript.jump_to_tail();
        assert!(transcript.at_tail());
    }
    #[test]
    fn code_copy_and_tool_toggle_require_matching_pointer_release() {
        let mut transcript = Transcript::default();
        transcript
            .update(
                vec![row("code", "```rust\nlet answer = 42;\n```")],
                500.0,
                500.0,
            )
            .unwrap();
        let copy = transcript
            .frame
            .display(0)
            .unwrap()
            .widgets
            .iter()
            .find(|w| matches!(w.kind, WidgetKind::Copy { .. }))
            .unwrap();
        let x = copy.x + copy.w / 2.0;
        let y = transcript.frame.placement(0).unwrap().y + copy.y + copy.h / 2.0;
        let mut fonts = Fonts::new();
        assert_eq!(
            transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts),
            None
        );
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            Some(Action::Copy("let answer = 42;".into()))
        );
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x: 0.0, y: 0.0 }, &mut fonts),
            None
        );
        let tool = Node {
            key: "tool".into(),
            style: Style::default(),
            element: Element::Tool {
                name: "Read file".into(),
                detail: "src/main.rs".into(),
                state: ToolState::Done,
                children: vec![row("result", "The file contents.")],
            },
        };
        transcript.update(vec![tool], 500.0, 500.0).unwrap();
        let initial = transcript.height();
        let toggle = transcript
            .frame
            .display(0)
            .unwrap()
            .widgets
            .iter()
            .find(|w| matches!(w.kind, WidgetKind::Toggle { .. }))
            .unwrap();
        let x = toggle.x + toggle.w / 2.0;
        let y = transcript.frame.placement(0).unwrap().y + toggle.y + toggle.h / 2.0;
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts);
        assert!(transcript.height() > initial);
        assert!(
            transcript
                .frame
                .display(0)
                .unwrap()
                .widgets
                .iter()
                .any(|w| matches!(w.kind, WidgetKind::Chevron { expanded: true }))
        );
        let mut frame = Frame::transparent(500, 500);
        transcript.paint(
            &mut frame,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 500.0,
                h: 500.0,
            },
            1.0,
            &mut fonts,
        );
        assert!(frame.pixels.iter().any(|value| *value > 0));
    }
}
