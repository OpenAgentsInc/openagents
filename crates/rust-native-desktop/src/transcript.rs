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
    Activate(String),
    OpenLink(String),
    Copy(String),
    Earlier,
}

use rust_native::selection::{Position as Point, Selection};

/// Exact row heights, reading position, selection, and horizontally scrolled blocks.
pub struct Transcript {
    version: u64,
    palette: HashMap<ColorRole, Color>,
    highlights: rust_native::syntax::Cache,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    expanded: BTreeSet<String>,
    pub relaid: usize,
    pub measured: u64,
    pressed_widget: Option<(String, usize)>,
    /// The enabled button a press began on, by key, and where it was.
    pressed_button: Option<(String, PxRect)>,
    pressed_link: Option<(String, usize)>,
    hovered_link: Option<(String, usize)>,
    layout: TranscriptLayout,
    measurer: ShapingMeasurer,
    frame: Arc<rust_native::layout::Frame>,
    rows: HashMap<String, Arc<Node<()>>>,
    order: Vec<String>,
    height: f32,
    offset: f32,
    follow: bool,
    selection: Selection,
    dragging: bool,
    horizontal: HashMap<(String, usize), f32>,
}

impl Default for Transcript {
    fn default() -> Self {
        let mut layout = TranscriptLayout::new();
        let frame = layout.frame();
        Self {
            version: 0,
            palette: HashMap::new(),
            highlights: Default::default(),
            wake: None,
            expanded: BTreeSet::new(),
            relaid: 0,
            measured: 0,
            pressed_widget: None,
            pressed_button: None,
            pressed_link: None,
            hovered_link: None,
            layout,
            measurer: ShapingMeasurer::new(),
            frame,
            rows: HashMap::new(),
            order: vec![],
            height: 1.0,
            offset: 0.0,
            follow: true,
            selection: Selection::default(),
            dragging: false,
            horizontal: HashMap::new(),
        }
    }
}

impl Transcript {
    pub fn set_metrics(
        &mut self,
        metrics: rust_native::layout::Metrics,
    ) -> Result<(), rust_native::layout::LayoutError> {
        self.layout.set_metrics(metrics)?;
        self.rows.clear();
        self.version = self.version.wrapping_add(1);
        Ok(())
    }
    pub fn set_palette(&mut self, colors: &[(ColorRole, Color)]) {
        self.palette = colors.iter().copied().collect();
        self.version = self.version.wrapping_add(1);
    }
    /// Updates foreground spans without invalidating transcript text measurements.
    pub fn set_syntax_palette(&mut self, palette: rust_native::syntax::Palette) {
        if self.highlights.set_palette(palette) {
            self.version = self.version.wrapping_add(1);
        }
    }
    fn ink(&self, value: Ink) -> Color {
        if let Ink::Role(role) = value
            && let Some(color) = self.palette.get(&role)
        {
            return *color;
        }
        ink(value)
    }
    pub fn set_font_family(&mut self, family: rust_native::layout::display::FontFamily) {
        self.layout.set_font_family(family);
        self.rows.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn start(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        self.wake = Some(wake);
    }
    /// Queue only visible code. Completion invalidates paint, never layout.
    pub fn poll_highlights(&mut self) {
        if self.highlights.poll() {
            self.version = self.version.wrapping_add(1);
        }
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let Some(row) = self.frame.display(index) else {
                continue;
            };
            for code in &row.code_blocks {
                if let Some(text) = row.texts.get(code.text as usize) {
                    self.highlights
                        .request(&code.language, text, self.wake.clone());
                }
            }
        }
    }
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
        self.update_shared(rows.into_iter().map(Arc::new).collect(), width, height)
    }
    /// Update shared immutable rows without copying unchanged Markdown trees.
    pub fn update_shared(
        &mut self,
        rows: Vec<Arc<Node<()>>>,
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
            .filter(|row| {
                self.rows
                    .get(&row.key)
                    .is_none_or(|previous| !Arc::ptr_eq(previous, row) && previous != *row)
            })
            .map(|row| (**row).clone())
            .collect();
        if self
            .pressed_widget
            .as_ref()
            .is_some_and(|(key, _)| changed.iter().any(|row| &row.key == key))
        {
            self.pressed_widget = None;
        }
        if self
            .pressed_link
            .as_ref()
            .is_some_and(|(key, _)| changed.iter().any(|row| &row.key == key))
        {
            self.pressed_link = None;
        }
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
            self.pressed_widget = None;
            self.pressed_link = None;
            self.hovered_link = None;
        }
        self.order = order;
        self.rows = rows.into_iter().map(|row| (row.key.clone(), row)).collect();
        self.frame = self.layout.frame();
        self.selection.reconcile(&self.frame);
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
        let mut nearest = None;
        let mut distance = f32::INFINITY;
        // While dragging, clamp to the nearest visible text, including blank
        // margins and paragraph gaps. Scrolling can bring other rows into view.
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y;
            for (run_index, run) in row.runs.iter().enumerate() {
                let style = row.styles.get(run.style as usize)?;
                let mut rx = run.x;
                let mut left = rx;
                let mut right = rx + run.width;
                let mut inside = true;
                for (region, scroller) in row.scrollers.iter().enumerate() {
                    if (scroller.runs[0]..scroller.runs[1]).contains(&(run_index as u32)) {
                        rx -= self
                            .horizontal
                            .get(&(row.key.clone(), region))
                            .copied()
                            .unwrap_or_default();
                        left = rx.max(scroller.x);
                        right = (rx + run.width).min(scroller.x + scroller.w);
                        inside = x >= scroller.x
                            && x <= scroller.x + scroller.w
                            && y >= top + scroller.y
                            && y <= top + scroller.y + scroller.h;
                    }
                }
                if right < left || (!self.dragging && !inside) {
                    continue;
                }
                let lo = top + run.baseline - style.font.size;
                let hi = top + run.baseline + style.font.size * 0.3;
                let dy = (lo - y).max(0.0).max(y - hi);
                let dx = (left - x).max(0.0).max(x - right);
                if !self.dragging && (dy > 0.0 || dx > 8.0) {
                    continue;
                }
                let score = dy * 1024.0 + dx;
                if score < distance {
                    let text = &row.texts[run.text as usize];
                    let start = run.start8 as usize;
                    let end = start + run.len8 as usize;
                    nearest = Some(Point {
                        row: index,
                        text: run.text as usize,
                        byte: start + fonts.caret_byte(&text[start..end], style.font, x - rx),
                    });
                    distance = score;
                }
            }
        }
        nearest
    }

    fn link(&self, x: f32, y: f32) -> Option<(String, usize)> {
        let y = y + self.offset;
        for index in self.frame.rows_in(y, y + 1.0) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y;
            for (number, link) in row.links.iter().enumerate() {
                let mut lx = link.x;
                let mut clipped = false;
                for (region, scroller) in row.scrollers.iter().enumerate() {
                    if (scroller.links[0]..scroller.links[1]).contains(&(number as u32)) {
                        lx -= self
                            .horizontal
                            .get(&(row.key.clone(), region))
                            .copied()
                            .unwrap_or_default();
                        clipped = x < scroller.x
                            || x > scroller.x + scroller.w
                            || y < top + scroller.y
                            || y > top + scroller.y + scroller.h;
                    }
                }
                if !clipped
                    && x >= lx
                    && x <= lx + link.w
                    && y >= top + link.y
                    && y <= top + link.y + link.h
                {
                    return Some((row.key.clone(), number));
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
        let previous = self.selection.endpoints(&self.frame);
        let hovered = self.hovered_link.clone();
        match event {
            SurfaceInput::Down { x, y, shift } => {
                self.dragging = false;
                self.pressed_link = self.link(x, y);
                self.pressed_widget = self.widget(x, y);
                self.pressed_button = self.pressed_widget.as_ref().and_then(|(row, index)| {
                    let at = self.frame.find(row)?;
                    let top = self.frame.placement(at)?.y - self.offset;
                    let widget = self.frame.display(at)?.widgets.get(*index)?;
                    match &widget.kind {
                        WidgetKind::Button { key, enabled: true } => Some((
                            key.clone(),
                            PxRect {
                                x: widget.x,
                                y: top + widget.y,
                                w: widget.w,
                                h: widget.h,
                            },
                        )),
                        _ => None,
                    }
                });
                if self.pressed_widget.is_some() {
                    self.dragging = false;
                    return None;
                }
                if let Some(point) = self.point(x, y, fonts) {
                    self.selection.begin(&self.frame, point, shift);
                    self.dragging = true;
                } else {
                    self.selection.clear();
                }
            }
            SurfaceInput::Move { x, y } => {
                self.hovered_link = self.link(x, y);
                if self.dragging {
                    if y < 0.0 {
                        self.scroll((-y).min(32.0));
                    }
                    if y > self.height {
                        self.scroll(-(y - self.height).min(32.0));
                    }
                    if let Some(point) = self.point(x, y, fonts) {
                        self.selection.extend(&self.frame, point);
                    }
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
                            WidgetKind::Button { key, enabled: true } => {
                                Some(Action::Activate(key))
                            }
                            WidgetKind::Copy { text, .. } => Some(Action::Copy(text)),
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
                                let _ = self.update_shared(rows, self.frame.width(), self.height);
                                None
                            }
                            _ => None,
                        };
                    }
                    return None;
                }
                if let Some(pressed) = self.pressed_link.take()
                    && self.selection.collapsed(&self.frame)
                    && self.link(x, y).as_ref() == Some(&pressed)
                {
                    let row = self.frame.display(self.frame.find(&pressed.0)?)?;
                    let destination = &row.links.get(pressed.1)?.destination;
                    if rust_native::markdown::opens(destination) {
                        return Some(Action::OpenLink(destination.clone()));
                    }
                }
            }
            SurfaceInput::Wheel { x, y, dx, dy } => {
                self.scroll(dy);
                self.scroll_horizontal(x, y, dx);
            }
            // A transcript doesn't zoom.
            SurfaceInput::Zoom { .. } => {}
        }
        if previous != self.selection.endpoints(&self.frame) || hovered != self.hovered_link {
            self.version = self.version.wrapping_add(1);
        }
        None
    }

    /// The enabled button the last press began on, where it was then.
    /// Replacing its row, or the rows' order, drops the press so it cannot
    /// activate; a host that admits a late click ([`rust_native::Press`])
    /// asks here whether a release still lands on the button as it was
    /// pressed, and gets its key when the transcript still shows an enabled
    /// button with that key. Each release ends the press.
    pub fn pressed_button(&self) -> Option<&str> {
        self.pressed_button.as_ref().map(|(key, _)| key.as_str())
    }

    /// Ends the press (see [`Transcript::pressed_button`]): its key when
    /// the release at `x`, `y` is on the button where it was pressed and
    /// the transcript still shows an enabled button with that key.
    pub fn release_pressed_button(&mut self, x: f32, y: f32) -> Option<String> {
        let (key, rect) = self.pressed_button.take()?;
        (x >= rect.x
            && x < rect.x + rect.w
            && y >= rect.y
            && y < rect.y + rect.h
            && self.control_bounds(&key).is_some())
        .then_some(key)
    }

    fn widget(&self, x: f32, y: f32) -> Option<(String, usize)> {
        let y = y + self.offset;
        for index in self.frame.rows_in(y, y + 1.0) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y;
            for (index, widget) in row.widgets.iter().enumerate().rev() {
                if matches!(
                    widget.kind,
                    WidgetKind::Button { enabled: true, .. }
                        | WidgetKind::Copy { .. }
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
    /// Visible bounds of a current enabled transcript button, in viewport points.
    pub fn control_bounds(&self, key: &str) -> Option<PxRect> {
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y - self.offset;
            for widget in &row.widgets {
                if matches!(&widget.kind, WidgetKind::Button { key: current, enabled: true } if current == key)
                {
                    let y = (top + widget.y).max(0.0);
                    let bottom = (top + widget.y + widget.h).min(self.height);
                    if bottom > y {
                        return Some(PxRect {
                            x: widget.x,
                            y,
                            w: widget.w,
                            h: bottom - y,
                        });
                    }
                }
            }
        }
        None
    }
    /// The viewport bounds of the expand toggle of the row keyed `key` (a
    /// tool row with something inside), while it is on screen.
    pub fn toggle_bounds(&self, key: &str) -> Option<PxRect> {
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let row = self.frame.display(index)?;
            let top = self.frame.placement(index)?.y - self.offset;
            for widget in &row.widgets {
                if matches!(&widget.kind, WidgetKind::Toggle { key: current, .. } if current == key)
                {
                    let y = (top + widget.y).max(0.0);
                    let bottom = (top + widget.y + widget.h).min(self.height);
                    if bottom > y {
                        return Some(PxRect {
                            x: widget.x,
                            y,
                            w: widget.w,
                            h: bottom - y,
                        });
                    }
                }
            }
        }
        None
    }

    /// The rows, in order, for screen readers, with the viewport bounds of
    /// the rows and enabled buttons now on screen ([`crate::access`]).
    pub fn access(&self) -> crate::access::Content {
        let mut bounds = HashMap::new();
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let (Some(row), Some(place)) = (self.frame.display(index), self.frame.placement(index))
            else {
                continue;
            };
            let top = place.y - self.offset;
            let clamp = |y: f32, h: f32| {
                let y0 = y.max(0.0);
                let y1 = (y + h).min(self.height);
                (y1 > y0).then_some((y0, y1 - y0))
            };
            if let Some((y, h)) = clamp(top, place.height) {
                bounds.insert(
                    row.key.clone(),
                    crate::layout::Rect {
                        x: 0.0,
                        y,
                        w: self.frame.width(),
                        h,
                    },
                );
            }
            for widget in &row.widgets {
                if let WidgetKind::Button { key, enabled: true } = &widget.kind
                    && let Some((y, h)) = clamp(top + widget.y, widget.h)
                {
                    bounds.insert(
                        key.clone(),
                        crate::layout::Rect {
                            x: widget.x,
                            y,
                            w: widget.w,
                            h,
                        },
                    );
                }
            }
        }
        crate::access::Content {
            rows: self
                .order
                .iter()
                .filter_map(|key| self.rows.get(key).cloned())
                .collect(),
            bounds,
            conversation: true,
        }
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
        self.selection.copy(&self.frame)
    }

    /// The key of the row a non-empty selection starts in.
    pub fn selected_row_key(&self) -> Option<String> {
        if self.selection.collapsed(&self.frame) {
            return None;
        }
        let (start, _) = self.selection.ordered(&self.frame)?;
        self.frame.key(start.row).map(str::to_owned)
    }

    /// Paint only rows in the viewport, with clipping for wide code and tables.
    pub fn paint(&self, frame: &mut Frame, rect: PxRect, scale: f32, fonts: &mut Fonts) {
        let previous = frame.clip_to(rect);
        let selected = self.selection.ordered(&self.frame);
        for index in self.frame.rows_in(self.offset, self.offset + self.height) {
            let row = self.frame.display(index).unwrap();
            let y = rect.y + (self.frame.placement(index).unwrap().y - self.offset) * scale;
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
                    frame.fill_corners(bounds, shape.radii.map(|r| r * scale), self.ink(fill));
                }
                if let Some(stroke) = shape.stroke {
                    frame.stroke_corners(
                        bounds,
                        shape.radii.map(|r| r * scale),
                        scale,
                        self.ink(stroke),
                    );
                }
                if let Some(clip) = clip {
                    frame.restore_clip(clip);
                }
            }
            let highlighted: Vec<_> = row
                .code_blocks
                .iter()
                .filter_map(|code| {
                    self.highlights
                        .get(&code.language, &row.texts[code.text as usize])
                        .map(|spans| (code.text, spans))
                })
                .collect();
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
                            let left = fonts.advance(&text[start..lo], style.font);
                            let width = fonts.advance(&text[start..hi], style.font) - left;
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
                let hovered = self.hovered_link.as_ref().is_some_and(|(key, number)| {
                    key == &row.key
                        && row.links.get(*number).is_some_and(|link| {
                            link.x == run.x
                                && (link.y + link.h >= run.baseline)
                                && link.y <= run.baseline
                        })
                });
                let mut color = if hovered {
                    Color::rgb(210, 231, 252)
                } else {
                    self.ink(style.ink)
                };
                color.alpha = (f32::from(color.alpha) * style.opacity) as u8;
                let truncated = run
                    .truncate
                    .map(|width| fonts.ellipsized(&text[start..end], style.font, width));
                let highlights = highlighted
                    .iter()
                    .find(|(text, _)| *text == run.text)
                    .map_or(&[][..], |(_, spans)| *spans);
                fonts.draw_highlighted_run(
                    frame,
                    truncated.as_deref().unwrap_or(&text[start..end]),
                    style.font,
                    x,
                    baseline,
                    scale,
                    color,
                    highlights,
                    start,
                );
                if style.underline || hovered {
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
                let color = self.ink(Ink::Role(ColorRole::Secondary));
                match &widget.kind {
                    WidgetKind::Copy { icon: true, .. } => {
                        let unit = bounds.h / 22.0;
                        let icon = PxRect {
                            x: bounds.x + (bounds.w - 12.0 * unit) / 2.0,
                            y: bounds.y + (bounds.h - 12.0 * unit) / 2.0,
                            w: 12.0 * unit,
                            h: 12.0 * unit,
                        };
                        crate::solar::draw_copy(frame, icon, color);
                    }
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
                    // A link's target is its text, drawn with the runs; a
                    // click on it is hit-tested against the row's links.
                    WidgetKind::Button { .. }
                    | WidgetKind::Toggle { .. }
                    | WidgetKind::Earlier { .. }
                    | WidgetKind::Link { .. }
                    | WidgetKind::Surface { .. } => {}
                }
            }
        }
        frame.restore_clip(previous);
    }
}

pub(crate) fn ink(ink: Ink) -> Color {
    match ink {
        Ink::Rgba([red, green, blue, alpha]) => Color {
            red,
            green,
            blue,
            alpha,
        },
        Ink::Role(role) => crate::theme::Roles::DESKTOP.color(role),
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
    fn selection_copies_offscreen_rows_and_survives_prepend_and_stream_append() {
        let mut transcript = Transcript::default();
        let mut rows: Vec<_> = (0..50)
            .map(|i| Node {
                key: format!("row-{i}"),
                style: Style::default(),
                element: Element::Text {
                    value: format!("line{i} é 👩‍💻  "),
                    role: rust_native::TextRole::Body,
                },
            })
            .collect();
        transcript.update(rows.clone(), 500.0, 130.0).unwrap();
        assert!(transcript.visible_rows() < 10);
        let end = transcript.frame.display(49).unwrap().texts[0].len();
        transcript.selection.begin(
            &transcript.frame,
            Point {
                row: 0,
                text: 0,
                byte: 0,
            },
            false,
        );
        transcript.selection.extend(
            &transcript.frame,
            Point {
                row: 49,
                text: 0,
                byte: end,
            },
        );
        let expected = (0..50)
            .map(|i| format!("line{i} é 👩‍💻  "))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(transcript.selected_text(), expected);
        rows.insert(0, row("earlier", "Earlier text"));
        rows.push(row("stream", "Partial"));
        transcript.update(rows.clone(), 380.0, 130.0).unwrap();
        assert_eq!(transcript.selected_text(), expected);
        *rows.last_mut().unwrap() = row("stream", "Partial with more text");
        transcript.update(rows, 380.0, 130.0).unwrap();
        assert_eq!(transcript.selected_text(), expected);
        let mut frame = Frame::transparent(380, 130);
        transcript.paint(
            &mut frame,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 380.0,
                h: 130.0,
            },
            1.0,
            &mut Fonts::new(),
        );
        assert!(frame.pixels.iter().any(|byte| *byte != 0));
    }
    #[test]
    fn pointer_drag_can_cross_blank_margins_and_virtualized_rows() {
        let mut transcript = Transcript::default();
        transcript
            .update(
                (0..50)
                    .map(|i| row(&format!("row-{i}"), &format!("line{i}")))
                    .collect(),
                500.0,
                140.0,
            )
            .unwrap();
        transcript.scroll(100000.0);
        let run = &transcript.frame.display(0).unwrap().runs[0];
        let x = run.x;
        let y = transcript.frame.placement(0).unwrap().y + run.baseline;
        let mut fonts = Fonts::new();
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        transcript.scroll(-100000.0);
        let row = transcript.frame.display(49).unwrap();
        let y =
            transcript.frame.placement(49).unwrap().y - transcript.offset + row.runs[0].baseline;
        transcript.pointer(SurfaceInput::Move { x: 499.0, y }, &mut fonts);
        transcript.pointer(SurfaceInput::Up { x: 499.0, y }, &mut fonts);
        assert_eq!(
            transcript.selected_text(),
            (0..50)
                .map(|i| format!("line{i}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    #[test]
    fn horizontally_scrolled_links_hover_and_open_only_on_matching_release() {
        let mut transcript = Transcript::default();
        transcript.update(vec![row("table","| Very wide heading in column one | Link |\n| --- | --- |\n| Wide content to force scrolling sideways | [Open](https://example.test/docs) |")],220.0,400.0).unwrap();
        let row = transcript.frame.display(0).unwrap();
        let link = &row.links[0];
        let scroller = &row.scrollers[0];
        let x = scroller.x + scroller.w / 2.0;
        let y = transcript.frame.placement(0).unwrap().y + link.y + link.h / 2.0;
        let dx = (link.x + link.w / 2.0 - x).min(scroller.content_w - scroller.w);
        assert!(dx > 0.0);
        transcript.scroll_horizontal(x, y, -dx);
        let row = transcript.frame.display(0).unwrap();
        let x = row.links[0].x + row.links[0].w / 2.0 - dx;
        let mut fonts = Fonts::new();
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            None
        );
        let version = transcript.version();
        transcript.pointer(SurfaceInput::Move { x, y }, &mut fonts);
        assert_eq!(transcript.hovered_link, Some(("table".into(), 0)));
        assert!(transcript.version() > version);
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            Some(Action::OpenLink("https://example.test/docs".into()))
        );
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x: 0.0, y }, &mut fonts),
            None
        );
        // Neither text selection nor link activation reaches clipped content.
        assert!(transcript.link(219.0, y).is_none());
    }
    #[test]
    fn rich_text_fixture_highlights_without_relayout_and_copies_exact_code() {
        let value = "# Markdown\n\n**Bold**, *italic*, ~~strike~~, `inline`, and a [link](https://example.test).\n\n1. Ordered\n2. Another\n\n- [x] Checked\n- Plain\n\n> Quoted words\n\n```rust\n// Café\nfn main() { let answer = 42; println!(\"hello\"); }\n```\n\n| Name | Value |\n| --- | --- |\n| Wide data column with more text | 42 |";
        let mut transcript = Transcript::default();
        transcript
            .update(vec![row("fixture", value)], 600.0, 1100.0)
            .unwrap();
        let height = transcript.height();
        let frame = transcript.frame.clone();
        transcript.poll_highlights();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            transcript.poll_highlights();
            let row = transcript.frame.display(0).unwrap();
            let code = &row.code_blocks[0];
            if transcript
                .highlights
                .get(&code.language, &row.texts[code.text as usize])
                .is_some()
            {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(transcript.height(), height);
        assert!(Arc::ptr_eq(&frame, &transcript.frame));
        assert!(transcript.frame.display(0).unwrap().widgets.iter().any(|widget|
            matches!(&widget.kind,WidgetKind::Copy {text,..} if text == "// Café\nfn main() { let answer = 42; println!(\"hello\"); }\n")));
        let mut pixels = Frame::transparent(600, 1100);
        transcript.paint(
            &mut pixels,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 600.0,
                h: 1100.0,
            },
            1.0,
            &mut Fonts::new(),
        );
        if let Ok(path) = std::env::var("OPENAGENTS_RICHTEXT_CAPTURE") {
            std::fs::write(path, pixels.png().unwrap()).unwrap();
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
            Some(Action::Copy("let answer = 42;\n".into()))
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

#[cfg(test)]
mod button_tests {
    use super::*;
    use crate::input::SurfaceInput;
    use rust_native::style::Style;
    use rust_native::{Element, Node};

    fn button(label: &str, enabled: bool) -> Node<()> {
        Node {
            key: "card-action".into(),
            style: Style::default(),
            element: Element::Button {
                shortcut: None,
                label: label.into(),
                enabled,
                icon: None,
                intent: (),
            },
        }
    }
    fn position(transcript: &Transcript) -> (f32, f32) {
        let widget = &transcript.frame.display(0).unwrap().widgets[0];
        (
            widget.x + widget.w / 2.0,
            transcript.frame.placement(0).unwrap().y + widget.y + widget.h / 2.0,
        )
    }
    #[test]
    fn transcript_buttons_require_enabled_matching_release() {
        let mut transcript = Transcript::default();
        let mut fonts = Fonts::new();
        transcript
            .update(vec![button("Run", true)], 300.0, 300.0)
            .unwrap();
        let (x, y) = position(&transcript);
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            Some(Action::Activate("card-action".into()))
        );
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x: 0.0, y: 0.0 }, &mut fonts),
            None
        );
        transcript
            .update(vec![button("Run", false)], 300.0, 300.0)
            .unwrap();
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            None
        );
    }
    #[test]
    fn a_replaced_press_names_its_button_only_for_a_release_where_it_was_pressed() {
        let mut transcript = Transcript::default();
        let mut fonts = Fonts::new();
        transcript
            .update(vec![button("Stop", true)], 300.0, 300.0)
            .unwrap();
        let (x, y) = position(&transcript);
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(transcript.pressed_button(), Some("card-action"));
        transcript
            .update(vec![button("Stop now", true)], 300.0, 300.0)
            .unwrap();
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            None
        );
        assert_eq!(
            transcript.release_pressed_button(x, y).as_deref(),
            Some("card-action")
        );
        assert_eq!(transcript.release_pressed_button(x, y), None, "once");
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        assert_eq!(transcript.release_pressed_button(0.0, 299.0), None);
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        transcript
            .update(vec![button("Stop", false)], 300.0, 300.0)
            .unwrap();
        assert_eq!(transcript.release_pressed_button(x, y), None);
        transcript.pointer(
            SurfaceInput::Down {
                x: 1.0,
                y: 299.0,
                shift: false,
            },
            &mut fonts,
        );
        assert_eq!(transcript.pressed_button(), None);
    }
    #[test]
    fn replacing_a_card_during_a_press_cannot_activate_the_new_action() {
        let mut transcript = Transcript::default();
        let mut fonts = Fonts::new();
        transcript
            .update(vec![button("First offer", true)], 300.0, 300.0)
            .unwrap();
        let (x, y) = position(&transcript);
        transcript.pointer(SurfaceInput::Down { x, y, shift: false }, &mut fonts);
        transcript
            .update(vec![button("Another offer", true)], 300.0, 300.0)
            .unwrap();
        assert_eq!(
            transcript.pointer(SurfaceInput::Up { x, y }, &mut fonts),
            None
        );
        let mut frame = Frame::transparent(300, 300);
        transcript.paint(
            &mut frame,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 300.0,
                h: 300.0,
            },
            1.0,
            &mut fonts,
        );
        assert!(frame.pixels.iter().any(|byte| *byte != 0));
    }
}
