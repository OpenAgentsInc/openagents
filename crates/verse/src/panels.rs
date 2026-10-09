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
//! change what it shows, except an action button ([`Panel::set_actions`])
//! and the composer's Enter ([`Panel::set_composer`]), which go to the
//! panel's owner to carry out.

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
    /// The action at this index of [`Panel::set_actions`]: its owner
    /// carries it out.
    Action(usize),
    /// Enter in the composer: its owner takes the draft.
    Submit,
}

/// A key the window hands a focused panel. Every other key is still
/// consumed, so it never reaches the character controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Tab,
    /// Shift+Tab.
    BackTab,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    /// A typed character, for the composer.
    Char(char),
    Backspace,
    Enter,
    /// Shift+Enter: a line break in the composer's draft.
    NewLine,
    Other,
}

/// The longest draft the composer holds, in bytes: the longest text a
/// studio intent carries.
pub const MAX_DRAFT: usize = 16 * 1024;
/// The most lines of a draft the composer shows; earlier lines scroll off.
const COMPOSER_LINES: usize = 6;

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
    selected: Option<usize>,
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
    /// Labels of the action buttons under the tabs, and whether each
    /// takes presses.
    actions: Vec<(String, bool)>,
    /// The composer's placeholder, while the panel has one.
    composer: Option<String>,
    /// What the person typed in the composer.
    draft: String,
    /// The diff line the person picked, by index in the diff document.
    selected: Option<usize>,
    /// The desktop app's scheme the panel last painted in
    /// ([`visual::scheme`]).
    scheme: visual::Scheme,
}

impl Panel {
    /// A panel titled `title`, showing the transcript tab, without focus.
    #[must_use]
    pub fn new(title: &str) -> Self {
        let mut transcript = Transcript::default();
        transcript.set_font_family(FontFamily::PaperMono);
        // The desktop app's own metrics are valid by construction.
        let _ = transcript.set_metrics(visual::current().transcript);
        transcript.set_palette(&visual::current().colors);
        transcript.set_syntax_palette(visual::current().syntax);
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
            actions: Vec::new(),
            composer: None,
            draft: String::new(),
            selected: None,
            scheme: visual::scheme(),
        };
        panel.rebuild();
        panel
    }

    /// Shows a button under the tabs for each of `actions`, a label and
    /// whether it takes presses. Pressing the one at index `i` resolves to
    /// [`Intent::Action`]`(i)`.
    pub fn set_actions(&mut self, actions: Vec<(String, bool)>) {
        if self.actions != actions {
            self.actions = actions;
            self.rebuild();
        }
    }

    /// Gives the panel a composer showing `placeholder` while empty, or
    /// takes it away. Typed characters go to its draft while the panel has
    /// focus, and Enter resolves to [`Intent::Submit`].
    pub fn set_composer(&mut self, placeholder: Option<&str>) {
        if self.composer.as_deref() != placeholder {
            self.composer = placeholder.map(str::to_owned);
            if self.composer.is_none() {
                self.draft.clear();
            }
            self.rebuild();
        }
    }

    /// Whether the panel has a composer.
    #[must_use]
    pub fn has_composer(&self) -> bool {
        self.composer.is_some()
    }

    /// What the person typed in the composer.
    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }

    /// Replaces the composer's draft, such as with a completion or a line
    /// from the history. Does nothing without a composer.
    pub fn set_draft(&mut self, draft: &str) {
        if self.composer.is_some() && self.draft != draft && draft.len() <= MAX_DRAFT {
            draft.clone_into(&mut self.draft);
            self.rebuild();
        }
    }

    /// Takes the composer's draft, leaving it empty.
    pub fn take_draft(&mut self) -> String {
        let draft = std::mem::take(&mut self.draft);
        self.rebuild();
        draft
    }

    /// The diff line the person picked in the changes tab, by its index in
    /// [`Panel::diff_document`].
    #[must_use]
    pub fn selected_line(&self) -> Option<usize> {
        self.selected
    }

    /// The diff the changes tab shows, parsed.
    #[must_use]
    pub fn diff_document(&self) -> Option<&changes::Document> {
        self.diff.as_ref()
    }

    /// The index of the diff line at the top of the changes tab.
    #[must_use]
    pub fn diff_top(&self) -> usize {
        (self.diff_scroll / DIFF_LINE).floor() as usize
    }

    /// Scrolls the changes tab so diff line `index` is at its top, as far
    /// as the diff scrolls.
    pub fn scroll_diff_to(&mut self, index: usize) {
        let height = self.body().h;
        let limit = self
            .diff
            .as_ref()
            .map_or(0.0, |doc| doc.scroll_limit(height, DIFF_LINE));
        let scroll = (index as f32 * DIFF_LINE).clamp(0.0, limit);
        if scroll != self.diff_scroll {
            self.diff_scroll = scroll;
            self.rebuild();
        }
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
        self.selected = None;
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
            // The panel's owner carries these out.
            Intent::Action(_) | Intent::Submit => true,
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
        {
            match self.tab {
                Tab::Transcript => {
                    let _ = self
                        .transcript
                        .pointer(SurfaceInput::Down { x, y, shift: false }, &mut self.fonts);
                }
                Tab::Changes => {
                    // Picks the diff line under the press, for a comment.
                    let index = ((y + self.diff_scroll) / DIFF_LINE).floor();
                    let lines = self.diff.as_ref().map_or(0, |doc| doc.lines().len());
                    let picked =
                        (index >= 0.0 && (index as usize) < lines).then_some(index as usize);
                    if picked != self.selected {
                        self.selected = picked;
                        self.rebuild();
                    }
                }
            }
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
    /// world, Tab switches tabs, and the arrows and page keys scroll. With
    /// a composer, characters and Backspace edit its draft, and Enter
    /// submits a draft that is not blank.
    pub fn key(&mut self, key: Key) -> Option<Intent> {
        let page = (self.body().h - DIFF_LINE).max(DIFF_LINE);
        if self.composer.is_some() {
            match key {
                Key::Char(ch) => {
                    if !ch.is_control() && self.draft.len() + ch.len_utf8() <= MAX_DRAFT {
                        self.draft.push(ch);
                        self.rebuild();
                    }
                    return None;
                }
                Key::Backspace => {
                    if self.draft.pop().is_some() {
                        self.rebuild();
                    }
                    return None;
                }
                Key::NewLine => {
                    if !self.draft.is_empty() && self.draft.len() < MAX_DRAFT {
                        self.draft.push('\n');
                        self.rebuild();
                    }
                    return None;
                }
                Key::Enter => {
                    return (!self.draft.trim().is_empty()).then_some(Intent::Submit);
                }
                _ => {}
            }
        }
        match key {
            Key::Escape => self.set_focus(false),
            Key::Tab | Key::BackTab => {
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
            Key::Char(_) | Key::Backspace | Key::Enter | Key::NewLine | Key::Other => {}
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
        self.follow_scheme();
        self.transcript.poll_highlights();
        let key = Painted {
            window,
            scale: scale.to_bits(),
            revision: self.revision,
            transcript: self.transcript.version(),
            diff_scroll: self.diff_scroll.to_bits(),
            selected: self.selected,
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

    /// Repaints in the scheme the desktop app paints with when it changed
    /// (#11028): the transcript's palette and syntax colors, the diff's
    /// highlighter, and the header. The Verse's own scenes keep their art.
    fn follow_scheme(&mut self) {
        let scheme = visual::scheme();
        if scheme == self.scheme {
            return;
        }
        self.scheme = scheme;
        let look = visual::current();
        // The desktop app's own metrics are valid by construction.
        let _ = self.transcript.set_metrics(look.transcript);
        self.transcript.set_palette(&look.colors);
        self.transcript.set_syntax_palette(look.syntax);
        self.highlighter = None;
        if let Some(doc) = &mut self.diff {
            doc.clear_spans();
        }
        self.rebuild();
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
        frame.fill(all, 12.0 * scale, visual::current().canvas);
        let edge = if self.focused {
            visual::current().accent
        } else {
            visual::current().composer_border
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
        let highlighter = self.highlighter.get_or_insert_with(|| {
            rust_native::syntax::Highlighter::with_palette(visual::current().syntax)
        });
        let Some(doc) = &mut self.diff else { return };
        doc.ensure_spans(first, count, highlighter);
        let previous = frame.clip_to(rect);
        let line_px = DIFF_LINE * scale;
        let offset = (self.diff_scroll / DIFF_LINE).fract() * line_px;
        let font = mono(13.0);
        for (index, line) in doc.lines().iter().skip(first).take(count).enumerate() {
            let top = rect.y + index as f32 * line_px - offset;
            let (gutter, color) = match line.kind {
                changes::Kind::Add => (
                    Some(visual::current().diff_add_bg),
                    visual::current().diff_add,
                ),
                changes::Kind::Remove => (
                    Some(visual::current().diff_remove_bg),
                    visual::current().diff_remove,
                ),
                changes::Kind::File | changes::Kind::Context => (None, visual::current().text),
                changes::Kind::Hunk | changes::Kind::Meta => (None, visual::current().muted),
            };
            // The line picked for a comment.
            let gutter = if self.selected == Some(first + index) {
                Some(visual::pick(
                    Color::rgb(44, 52, 72),
                    Color::rgb(229, 243, 255),
                ))
            } else {
                gutter
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
            family: FontFamily::PaperMono,
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
            visual::current().faint,
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
                node.style.foreground = Some(visual::current().muted);
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
        let hint = match (self.focused, self.composer.is_some()) {
            (true, true) => {
                "Type, then Enter sends · Shift+Enter adds a line · Esc returns to the world"
            }
            (true, false) => "Esc returns to the world · Tab switches",
            (false, true) => "Click the panel to read it or type",
            (false, false) => "Click the panel to read it",
        };
        let mut children = vec![
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
        ];
        // The actions, three to a row so a row fits the panel.
        let buttons: Vec<Node<Intent>> = self
            .actions
            .iter()
            .enumerate()
            .map(|(index, (label, enabled))| {
                let mut node = button(
                    &format!("panel-action-{index}"),
                    label,
                    Intent::Action(index),
                );
                if let Element::Button { enabled: on, .. } = &mut node.element {
                    *on = *enabled;
                }
                node
            })
            .collect();
        for (row, chunk) in buttons.chunks(3).enumerate() {
            children.push(stack(
                &format!("panel-actions-{row}"),
                Axis::Horizontal,
                chunk.to_vec(),
            ));
        }
        if let Some(placeholder) = &self.composer {
            let (value, color) = if self.draft.is_empty() {
                (placeholder.clone(), visual::current().muted)
            } else {
                // The draft's end, at most six lines, so a long draft keeps
                // the header short.
                let lines: Vec<&str> = self.draft.split('\n').collect();
                let tail = lines[lines.len().saturating_sub(COMPOSER_LINES)..].join("\n");
                let skip = tail.chars().count().saturating_sub(240);
                let end: String = tail.chars().skip(skip).collect();
                let more = if skip > 0 || lines.len() > COMPOSER_LINES {
                    "…"
                } else {
                    ""
                };
                (format!("› {more}{end}"), visual::current().text)
            };
            let mut composer = text("panel-composer", &value, TextRole::Body);
            composer.style.foreground = Some(color);
            children.push(composer);
        }
        children.push(text("panel-hint", hint, TextRole::Status));
        let mut header = stack("panel", Axis::Vertical, children);
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
        font_family: FontFamily::PaperMono,
        background: visual::current().canvas,
        text: visual::current().text,
        muted: visual::current().muted,
        rule: visual::current().border,
        focus: visual::current().accent,
        button: visual::current().selected,
        button_text: visual::current().text,
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
        family: FontFamily::PaperMono,
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
            background: Some(visual::current().selected),
            foreground: Some(visual::current().text),
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
                        foreground: Some(visual::current().muted),
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
    fn the_composer_takes_typed_text_and_its_owner_takes_the_actions() {
        let mut panel = sample();
        panel.set_composer(Some("Type a goal"));
        panel.set_actions(vec![("Pause".into(), true), ("Stop".into(), true)]);
        let _ = panel.image(WINDOW, 1.0).unwrap();
        assert_eq!(panel.key(Key::Enter), None, "a blank draft sends nothing");
        for ch in "Ship it".chars() {
            assert!(panel.key(Key::Char(ch)).is_none());
        }
        assert!(panel.key(Key::Backspace).is_none());
        assert_eq!(panel.draft(), "Ship i");
        assert_eq!(panel.key(Key::Enter), Some(Intent::Submit));
        assert!(panel.apply(Intent::Submit), "the panel stays open");
        assert_eq!(panel.take_draft(), "Ship i");
        assert!(panel.draft().is_empty());
        let _ = panel.image(WINDOW, 1.0).unwrap();
        let stop = at(&panel, "panel-action-1");
        assert!(panel.press(stop));
        assert_eq!(panel.release(stop), Some(Intent::Action(1)));
        // Shift+Enter breaks a line; a draft never starts with one.
        assert!(panel.key(Key::NewLine).is_none());
        assert!(panel.draft().is_empty());
        for key in [Key::Char('a'), Key::NewLine, Key::Char('b')] {
            assert!(panel.key(key).is_none());
        }
        assert_eq!(panel.draft(), "a\nb");
        panel.set_draft("@ada ");
        assert_eq!(panel.draft(), "@ada ");
        // Without a composer, typing does nothing.
        panel.set_composer(None);
        assert!(panel.key(Key::Char('x')).is_none());
        assert!(panel.draft().is_empty());
        panel.set_draft("ignored");
        assert!(panel.draft().is_empty());
    }

    #[test]
    fn the_diff_scrolls_to_a_line() {
        let mut panel = sample();
        panel.apply(Intent::Show(Tab::Changes));
        let _ = panel.image(WINDOW, 1.0).unwrap();
        assert_eq!(panel.diff_top(), 0);
        // The sample fits the panel, so it does not scroll past its top.
        panel.scroll_diff_to(5);
        assert_eq!(panel.diff_top(), 0);
        assert_eq!(panel.key(Key::BackTab), Some(Intent::Show(Tab::Transcript)));
    }

    #[test]
    fn a_press_on_the_diff_picks_a_line() {
        let mut panel = sample();
        panel.apply(Intent::Show(Tab::Changes));
        let _ = panel.image(WINDOW, 1.0).unwrap();
        let bounds = panel.bounds();
        let body = panel.body();
        // The middle of the diff's third line.
        let at = [
            bounds.x + body.x + 20.0,
            bounds.y + body.y + DIFF_LINE * 2.5,
        ];
        assert!(panel.press(at));
        let _ = panel.release(at);
        assert_eq!(panel.selected_line(), Some(2));
        assert!(panel.diff_document().is_some());
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
