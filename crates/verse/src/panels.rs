//! Rust Native panels composited over the world.
//!
//! The HUD in [`crate::hud`] draws the world's own amber interface. A panel
//! is different: it is a Rust Native view laid out and painted by
//! `rust-native-desktop`, in the chat palette the desktop app uses
//! (`openagents_chat_app::visual`), and handed to the renderer as an
//! [`OverlayImage`] that it draws over the finished frame. The first panel
//! carries the two views Agent Studio reads work through: the chat
//! transcript (`rust_native_desktop::transcript`, the same virtualized rows
//! the desktop app's chat paints) and the unified diff pane over
//! `openagents_chat_app::changes`. See `docs/verse/agent-studio.md`.
//!
//! Input reaches a panel only through [`Panel::press`], [`Panel::release`],
//! [`Panel::moved`], [`Panel::wheel`], and [`Panel::key`]. While the panel
//! has focus, the window gives it every key and keeps them from the
//! character controller; a press outside the panel returns focus to the
//! world. The panel owns no task, network, or authority: its intents only
//! change what it shows.

use openagents_chat_app::{changes, visual};
use rust_native::layout::display::{Font, FontFamily, Weight};
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{
    Activation, Axis, Element, MessageRole, Node, TextRole, ToolState, ValidatedView, View,
    markdown,
};
use rust_native_desktop::input::SurfaceInput;
use rust_native_desktop::layout::{Interaction, Scene, lay_out_window};
use rust_native_desktop::text::Fonts;
use rust_native_desktop::transcript::Transcript;
use rust_native_desktop::{Frame, PxRect, Theme};
use serde::Serialize;

use crate::overlay::OverlayImage;

pub mod studio;

/// The panel's view instance.
const INSTANCE: &str = "verse-panel";
/// Space between the panel and the window's edges, in points.
const INSET: f32 = 16.0;
/// Space inside the panel around its content, in points.
const PADDING: f32 = 14.0;
/// A diff line's height, in points, as the desktop app's pane draws it.
const DIFF_LINE: f32 = 18.0;
/// The narrowest and widest the panel docks, in points.
const MIN_WIDTH: f32 = 320.0;
const MAX_WIDTH: f32 = 560.0;
/// Points one wheel line scrolls.
const WHEEL_LINE: f32 = 40.0;

/// What the panel's body shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Tab {
    Transcript,
    Changes,
}

/// What a control in the panel's view does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Intent {
    Show(Tab),
    Close,
}

/// A key the window hands a focused panel. Every other key is still
/// consumed, so it never reaches the character controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Tab,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Other,
}

/// A rectangle in points, origin at the window's top-left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Bounds {
    /// Whether `at` is inside.
    #[must_use]
    pub fn contains(&self, at: [f32; 2]) -> bool {
        at[0] >= self.x && at[0] < self.x + self.w && at[1] >= self.y && at[1] < self.y + self.h
    }
}

/// Where a panel docks in a `window` points wide and tall: against the
/// right edge, full height, about two-fifths of the width.
#[must_use]
pub fn dock(window: [f32; 2]) -> Bounds {
    let w = (window[0] * 0.42)
        .clamp(MIN_WIDTH, MAX_WIDTH)
        .min(window[0] - 2.0 * INSET)
        .max(1.0);
    Bounds {
        x: (window[0] - w - INSET).max(0.0),
        y: INSET,
        w,
        h: (window[1] - 2.0 * INSET).max(1.0),
    }
}

/// What a painted image was painted from, so an unchanged panel is not
/// painted or uploaded again.
#[derive(Clone, Debug, PartialEq)]
struct Painted {
    window: [u32; 2],
    scale: u32,
    revision: u64,
    transcript: u64,
    diff_scroll: u32,
    hover: Option<String>,
    pressed: Option<String>,
}

/// One panel: a title, the transcript and diff it shows, and its focus.
pub struct Panel {
    title: String,
    tab: Tab,
    focused: bool,
    revision: u64,
    view: ValidatedView<Intent>,
    transcript: Transcript,
    rows: Vec<Node<()>>,
    diff: Option<changes::Document>,
    /// The unified diff `diff` was parsed from.
    diff_text: Option<String>,
    diff_scroll: f32,
    highlighter: Option<rust_native::syntax::Highlighter>,
    fonts: Fonts,
    /// The last layout, in panel points, and the body's rectangle in it.
    scene: Option<(Scene, Bounds)>,
    window: [f32; 2],
    pressed: Option<String>,
    hover: Option<String>,
    image: Option<(Painted, OverlayImage)>,
    /// Counts paints, so each painted image has its own revision.
    paints: u64,
}

impl Panel {
    /// A panel titled `title`, showing the transcript tab, without focus.
    #[must_use]
    pub fn new(title: &str) -> Self {
        let mut transcript = Transcript::default();
        transcript.set_font_family(FontFamily::Geist);
        // The desktop app's own metrics are valid by construction.
        let _ = transcript.set_metrics(visual::TRANSCRIPT);
        transcript.set_palette(&visual::COLORS);
        transcript.set_syntax_palette(visual::SYNTAX);
        let mut panel = Self {
            title: title.into(),
            tab: Tab::Transcript,
            focused: false,
            revision: 0,
            view: placeholder(),
            transcript,
            rows: Vec::new(),
            diff: None,
            diff_text: None,
            diff_scroll: 0.0,
            highlighter: None,
            fonts: Fonts::new(),
            scene: None,
            window: [0.0; 2],
            pressed: None,
            hover: None,
            image: None,
            paints: 0,
        };
        panel.rebuild();
        panel
    }

    /// The tab the body shows.
    #[must_use]
    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// Whether the panel takes keys and the pointer.
    #[must_use]
    pub fn focused(&self) -> bool {
        self.focused
    }

    /// Gives the panel focus, or returns it to the world.
    pub fn set_focus(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            self.rebuild();
        }
    }

    /// The current view, for tests and accessibility.
    #[must_use]
    pub fn view(&self) -> &ValidatedView<Intent> {
        &self.view
    }

    /// Replaces the transcript's rows. Unchanged rows keep their layout.
    pub fn set_rows(&mut self, rows: Vec<Node<()>>) {
        if rows != self.rows {
            self.rows = rows;
            self.relay_transcript();
        }
    }

    /// Renames the panel.
    pub fn set_title(&mut self, title: &str) {
        if self.title != title {
            title.clone_into(&mut self.title);
            self.rebuild();
        }
    }

    /// The unified diff the changes tab shows, as given to
    /// [`Panel::set_diff`].
    #[must_use]
    pub fn diff_source(&self) -> Option<&str> {
        self.diff_text.as_deref()
    }

    /// Shows `diff`, a unified diff, in the changes tab.
    pub fn set_diff(&mut self, diff: &str) {
        self.diff_text = Some(diff.to_owned());
        self.diff = Some(changes::parse(diff)).filter(|doc| !doc.is_empty());
        self.diff_scroll = 0.0;
        self.rebuild();
    }

    /// Where the panel docks in the last window it was painted for.
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        dock(self.window)
    }

    /// Runs an intent its view resolved. Returns false when the panel closes.
    pub fn apply(&mut self, intent: Intent) -> bool {
        match intent {
            Intent::Show(tab) => {
                if self.tab != tab {
                    self.tab = tab;
                    self.rebuild();
                }
                true
            }
            Intent::Close => false,
        }
    }

    /// A press at `at`, in window points. Inside the panel it takes focus
    /// and returns true; outside, the panel gives focus back and returns
    /// false, so the world handles the press.
    pub fn press(&mut self, at: [f32; 2]) -> bool {
        let bounds = self.bounds();
        if !bounds.contains(at) {
            self.set_focus(false);
            return false;
        }
        self.set_focus(true);
        let local = [at[0] - bounds.x, at[1] - bounds.y];
        self.pressed = self.hit(local);
        if self.pressed.is_none()
            && let Some((x, y)) = self.in_body(local)
            && self.tab == Tab::Transcript
        {
            let _ = self
                .transcript
                .pointer(SurfaceInput::Down { x, y, shift: false }, &mut self.fonts);
        }
        true
    }

    /// A release at `at`, in window points, after a press the panel took.
    /// Returns the intent of a control pressed and released inside.
    pub fn release(&mut self, at: [f32; 2]) -> Option<Intent> {
        let bounds = self.bounds();
        let local = [at[0] - bounds.x, at[1] - bounds.y];
        if let Some(pressed) = self.pressed.take() {
            if self.hit(local).as_deref() != Some(pressed.as_str()) {
                return None;
            }
            let activation = Activation {
                instance: INSTANCE.into(),
                revision: self.revision,
                node: pressed,
            };
            return self.view.activate(&activation).ok().copied();
        }
        if self.tab == Tab::Transcript {
            let x = local[0] - self.body().x;
            let y = local[1] - self.body().y;
            let _ = self
                .transcript
                .pointer(SurfaceInput::Up { x, y }, &mut self.fonts);
        }
        None
    }

    /// Pointer motion to `at`, in window points.
    pub fn moved(&mut self, at: [f32; 2]) {
        let bounds = self.bounds();
        let local = [at[0] - bounds.x, at[1] - bounds.y];
        let hover = if bounds.contains(at) {
            self.hit(local)
        } else {
            None
        };
        self.hover = hover;
        if self.tab == Tab::Transcript && self.transcript.dragging() {
            let x = local[0] - self.body().x;
            let y = local[1] - self.body().y;
            let _ = self
                .transcript
                .pointer(SurfaceInput::Move { x, y }, &mut self.fonts);
        }
    }

    /// A wheel turn of `lines` (positive scrolls toward the top) at `at`, in
    /// window points. Returns whether the panel took it.
    pub fn wheel(&mut self, at: [f32; 2], lines: f32) -> bool {
        if !self.bounds().contains(at) {
            return false;
        }
        self.scroll(lines * WHEEL_LINE);
        true
    }

    /// A key pressed while the panel has focus. Escape returns focus to the
    /// world, Tab switches tabs, and the arrows and page keys scroll.
    pub fn key(&mut self, key: Key) -> Option<Intent> {
        let page = (self.body().h - DIFF_LINE).max(DIFF_LINE);
        match key {
            Key::Escape => self.set_focus(false),
            Key::Tab => {
                return Some(Intent::Show(match self.tab {
                    Tab::Transcript => Tab::Changes,
                    Tab::Changes => Tab::Transcript,
                }));
            }
            Key::Up => self.scroll(DIFF_LINE),
            Key::Down => self.scroll(-DIFF_LINE),
            Key::PageUp => self.scroll(page),
            Key::PageDown => self.scroll(-page),
            Key::Home => self.scroll(f32::MAX / 4.0),
            Key::End => self.scroll(-f32::MAX / 4.0),
            Key::Other => {}
        }
        None
    }

    /// The panel painted for a `window` pixels wide and tall at `scale`
    /// pixels a point. An unchanged panel returns the same image and
    /// revision, so the renderer does not upload it again.
    ///
    /// # Errors
    ///
    /// Returns a message when the panel cannot fit the window.
    pub fn image(&mut self, window: [u32; 2], scale: f32) -> Result<&OverlayImage, String> {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let points = [window[0] as f32 / scale, window[1] as f32 / scale];
        if points != self.window {
            self.window = points;
            self.relay_transcript();
        }
        self.transcript.poll_highlights();
        let key = Painted {
            window,
            scale: scale.to_bits(),
            revision: self.revision,
            transcript: self.transcript.version(),
            diff_scroll: self.diff_scroll.to_bits(),
            hover: self.hover.clone(),
            pressed: self.pressed.clone(),
        };
        if self
            .image
            .as_ref()
            .is_none_or(|(painted, _)| *painted != key)
        {
            let image = self.paint(scale)?;
            self.image = Some((key, image));
        }
        Ok(&self.image.as_ref().expect("painted above").1)
    }

    fn paint(&mut self, scale: f32) -> Result<OverlayImage, String> {
        let bounds = self.bounds();
        let (scene, body) = self.lay_out();
        let width = (bounds.w * scale).round().max(1.0) as usize;
        let height = (bounds.h * scale).round().max(1.0) as usize;
        let mut frame = Frame::transparent(width, height);
        let all = PxRect {
            x: 0.0,
            y: 0.0,
            w: width as f32,
            h: height as f32,
        };
        frame.fill(all, 12.0 * scale, visual::CANVAS);
        let edge = if self.focused {
            visual::ACCENT
        } else {
            visual::COMPOSER_BORDER
        };
        frame.stroke(all, 12.0 * scale, scale.max(1.0).round(), edge);
        rust_native_desktop::paint::paint(
            &scene,
            &mut frame,
            scale,
            0.0,
            &mut self.fonts,
            &mut |_, _, _| {},
        );
        let body_px = PxRect {
            x: body.x * scale,
            y: body.y * scale,
            w: body.w * scale,
            h: body.h * scale,
        };
        match self.tab {
            Tab::Transcript if self.rows.is_empty() => {
                self.note(&mut frame, body_px, scale, "Nothing in this transcript yet");
            }
            Tab::Transcript => self
                .transcript
                .paint(&mut frame, body_px, scale, &mut self.fonts),
            Tab::Changes => self.paint_diff(&mut frame, body_px, scale),
        }
        self.scene = Some((scene, body));
        let origin = (
            (bounds.x * scale).round() as i32,
            (bounds.y * scale).round() as i32,
        );
        self.paints += 1;
        OverlayImage::from_premultiplied(
            self.paints,
            origin,
            (width as u32, height as u32),
            &frame.pixels,
        )
    }

    fn paint_diff(&mut self, frame: &mut Frame, rect: PxRect, scale: f32) {
        let viewport = rect.h / scale;
        let Some((first, count)) = self
            .diff
            .as_ref()
            .map(|doc| doc.window(self.diff_scroll, viewport, DIFF_LINE))
        else {
            self.note(frame, rect, scale, "No changes yet");
            return;
        };
        let highlighter = self
            .highlighter
            .get_or_insert_with(|| rust_native::syntax::Highlighter::with_palette(visual::SYNTAX));
        let Some(doc) = &mut self.diff else { return };
        doc.ensure_spans(first, count, highlighter);
        let previous = frame.clip_to(rect);
        let line_px = DIFF_LINE * scale;
        let offset = (self.diff_scroll / DIFF_LINE).fract() * line_px;
        let font = mono(13.0);
        for (index, line) in doc.lines().iter().skip(first).take(count).enumerate() {
            let top = rect.y + index as f32 * line_px - offset;
            let (gutter, color) = match line.kind {
                changes::Kind::Add => (Some(Color::rgb(28, 48, 34)), Color::rgb(163, 190, 140)),
                changes::Kind::Remove => (Some(Color::rgb(58, 32, 36)), Color::rgb(191, 120, 120)),
                changes::Kind::File | changes::Kind::Context => (None, visual::TEXT),
                changes::Kind::Hunk | changes::Kind::Meta => (None, visual::MUTED),
            };
            if let Some(gutter) = gutter {
                frame.fill(
                    PxRect {
                        x: rect.x,
                        y: top,
                        w: rect.w,
                        h: line_px,
                    },
                    0.0,
                    gutter,
                );
            }
            self.fonts.draw_highlighted_run(
                frame,
                &line.text,
                font,
                rect.x + 8.0 * scale,
                top + line_px * 0.78,
                scale,
                color,
                line.spans.as_deref().unwrap_or_default(),
                0,
            );
        }
        frame.restore_clip(previous);
    }

    fn note(&mut self, frame: &mut Frame, rect: PxRect, scale: f32, text: &str) {
        let font = Font {
            size: 13.0,
            weight: Weight::Regular,
            family: FontFamily::Geist,
            italic: false,
            mono: false,
        };
        self.fonts.draw_highlighted_run(
            frame,
            text,
            font,
            rect.x + 4.0 * scale,
            rect.y + 20.0 * scale,
            scale,
            visual::FAINT,
            &[],
            0,
        );
    }

    /// Scrolls the body by `dy` points (positive toward the top).
    fn scroll(&mut self, dy: f32) {
        match self.tab {
            Tab::Transcript => self.transcript.scroll(dy),
            Tab::Changes => {
                let height = self.body().h;
                let limit = self
                    .diff
                    .as_ref()
                    .map_or(0.0, |doc| doc.scroll_limit(height, DIFF_LINE));
                self.diff_scroll = (self.diff_scroll - dy).clamp(0.0, limit);
            }
        }
    }

    /// Lays out the header in panel points and returns it with the body's
    /// rectangle below it.
    fn lay_out(&mut self) -> (Scene, Bounds) {
        let bounds = self.bounds();
        let theme = theme(bounds.w);
        let interaction = Interaction {
            hover: self.hover.clone(),
            pressed: self.pressed.clone(),
            ..Interaction::default()
        };
        // A zero height keeps the header at the top instead of centering it.
        let scene = lay_out_window(
            self.view.view(),
            &theme,
            &mut self.fonts,
            &|_, _| None,
            &interaction,
            bounds.w,
            0.0,
        );
        let top = scene.height.min(bounds.h);
        let body = Bounds {
            x: PADDING,
            y: top,
            w: (bounds.w - 2.0 * PADDING).max(1.0),
            h: (bounds.h - top - PADDING).max(1.0),
        };
        (scene, body)
    }

    fn body(&mut self) -> Bounds {
        match &self.scene {
            Some((_, body)) => *body,
            None => self.lay_out().1,
        }
    }

    fn in_body(&mut self, local: [f32; 2]) -> Option<(f32, f32)> {
        let body = self.body();
        body.contains(local)
            .then(|| (local[0] - body.x, local[1] - body.y))
    }

    /// The enabled control at `local` panel points.
    fn hit(&mut self, local: [f32; 2]) -> Option<String> {
        if self.scene.is_none() {
            let laid = self.lay_out();
            self.scene = Some(laid);
        }
        let (scene, _) = self.scene.as_ref()?;
        scene
            .hit(local[0], local[1])
            .filter(|hit| hit.enabled)
            .map(|hit| hit.key.clone())
    }

    fn relay_transcript(&mut self) {
        self.scene = None;
        let body = self.body();
        let _ = self.transcript.update(self.rows.clone(), body.w, body.h);
    }

    /// Builds the header view for the current state.
    fn rebuild(&mut self) {
        self.revision += 1;
        let tab = |key: &str, label: &str, tab: Tab, selected: bool| {
            let mut node = button(key, label, Intent::Show(tab));
            if !selected {
                node.style.background = Some(Color {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 0,
                });
                node.style.foreground = Some(visual::MUTED);
            }
            node
        };
        let summary = match (self.tab, &self.diff) {
            (Tab::Changes, Some(doc)) => doc.summary(),
            (Tab::Changes, None) => "Nothing changed".into(),
            (Tab::Transcript, _) => match self.rows.len() {
                1 => "1 entry".into(),
                n => format!("{n} entries"),
            },
        };
        let hint = if self.focused {
            "Esc returns to the world · Tab switches"
        } else {
            "Click the panel to read it"
        };
        let mut header = stack(
            "panel",
            Axis::Vertical,
            vec![
                text("panel-title", &self.title, TextRole::Heading),
                text("panel-summary", &summary, TextRole::Status),
                stack(
                    "panel-tabs",
                    Axis::Horizontal,
                    vec![
                        tab(
                            "panel-transcript",
                            "Transcript",
                            Tab::Transcript,
                            self.tab == Tab::Transcript,
                        ),
                        tab(
                            "panel-changes",
                            "What changed",
                            Tab::Changes,
                            self.tab == Tab::Changes,
                        ),
                        button("panel-close", "Close", Intent::Close),
                    ],
                ),
                text("panel-hint", hint, TextRole::Status),
            ],
        );
        header.style.padding_top = Some(Space::Sm);
        self.view = View::new(INSTANCE, self.revision, header)
            .validate()
            .unwrap_or_else(|_| placeholder());
        self.scene = None;
    }
}

/// A view that always validates, for a header that does not.
fn placeholder() -> ValidatedView<Intent> {
    View::new(INSTANCE, 1, text("panel-title", "Panel", TextRole::Heading))
        .validate()
        .expect("a single text node validates")
}

/// The desktop app's chat theme, sized to a panel `width` points wide.
fn theme(width: f32) -> Theme {
    Theme {
        icons: rust_native_desktop::theme::IconSet::Solar,
        font_family: FontFamily::Geist,
        background: visual::CANVAS,
        text: visual::TEXT,
        muted: visual::MUTED,
        rule: visual::BORDER,
        focus: visual::ACCENT,
        button: visual::SELECTED,
        button_text: visual::TEXT,
        button_radius: 7.0,
        body: 14.0,
        heading: 18.0,
        status: 12.0,
        column: (width - 2.0 * PADDING).max(1.0),
        margin: PADDING,
        ..Theme::openagents()
    }
}

fn mono(size: f32) -> Font {
    Font {
        size,
        weight: Weight::Regular,
        family: FontFamily::Geist,
        italic: false,
        mono: true,
    }
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

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            background: Some(visual::SELECTED),
            foreground: Some(visual::TEXT),
            weight: Some(TextWeight::Normal),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: None,
            intent,
        },
    }
}

/// A transcript message row.
#[must_use]
pub fn message(key: &str, role: MessageRole, body: &str) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Message {
            role,
            note: None,
            children: vec![Node {
                key: format!("{key}-md"),
                style: Style::default(),
                element: Element::Markdown {
                    blocks: markdown::parse(body),
                },
            }],
        },
    }
}

/// A transcript tool row: a call's name, its detail, and what it returned.
#[must_use]
pub fn tool(key: &str, name: &str, detail: &str, output: &str, state: ToolState) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element: Element::Tool {
            name: name.into(),
            detail: detail.into(),
            state,
            children: if output.trim().is_empty() {
                Vec::new()
            } else {
                vec![Node {
                    key: format!("{key}-body"),
                    style: Style {
                        foreground: Some(visual::MUTED),
                        ..Style::default()
                    },
                    element: Element::Text {
                        value: output.into(),
                        role: TextRole::Code,
                    },
                }]
            },
        },
    }
}

/// A fixed seat's work, for captures and tests: a short transcript and the
/// diff it produced.
#[must_use]
pub fn sample() -> Panel {
    let mut panel = Panel::new("Seat 1 · Codex");
    panel.set_rows(vec![
        message(
            "ask",
            MessageRole::User,
            "Make the gym board sort agent runs first.",
        ),
        tool(
            "read",
            "Read",
            "crates/verse/src/app.rs",
            "",
            ToolState::Done,
        ),
        tool(
            "edit",
            "Edit",
            "crates/verse/src/app.rs",
            "",
            ToolState::Done,
        ),
        tool(
            "test",
            "Ran",
            "cargo test -p verse gym",
            "$ cargo test -p verse gym\ntest result: ok. 4 passed; 0 failed",
            ToolState::Done,
        ),
        message(
            "reply",
            MessageRole::Assistant,
            "Agent runs now sort before evaluations, and each group keeps its order. \
             The gym tests pass.",
        ),
    ]);
    panel.set_diff(SAMPLE_DIFF);
    panel
}

const SAMPLE_DIFF: &str = "diff --git a/crates/verse/src/app.rs b/crates/verse/src/app.rs
index 1111111..2222222 100644
--- a/crates/verse/src/app.rs
+++ b/crates/verse/src/app.rs
@@ -2641,6 +2641,9 @@ fn prioritize_gym_runs(view: &mut crate::gym::BoardView) {
     let rows = std::mem::take(&mut view.rows);
-    view.rows = rows;
+    let (agents, rest): (Vec<_>, Vec<_>) =
+        rows.into_iter().partition(|row| row.agent);
+    view.rows = agents;
+    view.rows.extend(rest);
 }
";

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: [u32; 2] = [1280, 800];

    fn at(panel: &Panel, key: &str) -> [f32; 2] {
        let (scene, _) = panel.scene.as_ref().expect("painted");
        let hit = scene
            .hits
            .iter()
            .find(|hit| hit.key == key)
            .unwrap_or_else(|| panic!("no control {key}"));
        let bounds = panel.bounds();
        [
            bounds.x + hit.rect.x + hit.rect.w / 2.0,
            bounds.y + hit.rect.y + hit.rect.h / 2.0,
        ]
    }

    #[test]
    fn the_panel_paints_the_chat_palette_and_docks_right() {
        let mut panel = sample();
        let image = panel.image(WINDOW, 1.0).unwrap().clone();
        let bounds = panel.bounds();
        assert!(bounds.x > WINDOW[0] as f32 / 2.0, "docks on the right");
        assert_eq!(image.x, bounds.x.round() as i32);
        assert_eq!(image.width, bounds.w.round() as u32);
        // The middle of the panel is the chat canvas, opaque.
        let middle = ((image.height / 2) * image.width + image.width - 8) as usize * 4;
        assert_eq!(&image.rgba[middle..middle + 4], &[6, 6, 6, 255]);
        // Text was painted: some pixels are the chat's light text.
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .any(|p| p[0] > 200 && p[1] > 200 && p[2] > 200 && p[3] == 255)
        );
        // An unchanged panel is the same image, so it is not uploaded again.
        let again = panel.image(WINDOW, 1.0).unwrap().revision;
        assert_eq!(again, image.revision);
    }

    #[test]
    fn the_panel_switches_to_the_diff_and_closes_through_its_view() {
        let mut panel = sample();
        let _ = panel.image(WINDOW, 1.0).unwrap();
        let tab = at(&panel, "panel-changes");
        assert!(panel.press(tab));
        assert!(panel.focused());
        let intent = panel.release(tab).expect("the tab resolves");
        assert_eq!(intent, Intent::Show(Tab::Changes));
        assert!(panel.apply(intent));
        assert_eq!(panel.tab(), Tab::Changes);
        let before = panel.image(WINDOW, 1.0).unwrap().revision;
        // A diff line's added-line gutter is painted.
        let image = panel.image(WINDOW, 1.0).unwrap();
        assert!(
            image
                .rgba
                .chunks_exact(4)
                .any(|p| p[..3] == [28, 48, 34] && p[3] == 255)
        );
        assert!(panel.key(Key::Down).is_none());
        assert_eq!(panel.image(WINDOW, 1.0).unwrap().revision, before);
        let close = at(&panel, "panel-close");
        assert!(panel.press(close));
        assert_eq!(panel.release(close), Some(Intent::Close));
        assert!(!panel.apply(Intent::Close));
    }

    #[test]
    fn a_press_outside_returns_focus_to_the_world() {
        let mut panel = sample();
        let _ = panel.image(WINDOW, 2.0).unwrap();
        let inside = {
            let b = panel.bounds();
            [b.x + b.w / 2.0, b.y + b.h / 2.0]
        };
        assert!(panel.press(inside));
        let _ = panel.release(inside);
        assert!(panel.focused());
        assert_eq!(panel.key(Key::Tab), Some(Intent::Show(Tab::Changes)));
        assert!(!panel.press([10.0, 10.0]));
        assert!(!panel.focused());
        assert!(panel.press(inside));
        assert!(panel.key(Key::Escape).is_none());
        assert!(!panel.focused());
        assert!(!panel.wheel([10.0, 10.0], 1.0));
        assert!(panel.wheel(inside, 1.0));
    }

    #[test]
    fn the_dock_fits_small_and_large_windows() {
        let small = dock([300.0, 200.0]);
        assert!(small.x >= 0.0 && small.x + small.w <= 300.0);
        let large = dock([3000.0, 1600.0]);
        assert_eq!(large.w, MAX_WIDTH);
        assert_eq!(large.x + large.w + INSET, 3000.0);
    }
}
