//! The presentation as its own window.
//!
//! [`Presenter`] is a [`rust_native_desktop::App`] around a
//! [`crate::viewer::Viewer`], which holds the slides, the keys, the
//! overview, the notes, and the black screen. Its view is one drawing
//! surface the size of the window, which the window adapter lays out,
//! shows, and captures like any other Rust Native view. The presenter adds
//! only what a window has: fullscreen and quitting. A slide appears whole
//! the moment it opens: there is no per-slide animation, and the surface
//! is painted again only when something changed.

use crate::compose::{Composed, HEIGHT, WIDTH};
use crate::slide::Deck;
use crate::viewer::{Outcome, Viewer};
use rust_native::style::Style;
use rust_native::{Axis, Element, Node, ValidatedView, View};
use rust_native_desktop::input::{SurfaceInput, TextInput};
use rust_native_desktop::{App, Frame, PxRect, Theme};
use std::time::Instant;

pub use crate::viewer::{Cell, overview_cells, paint_slide};

/// The surface the whole presentation paints into.
const SURFACE: &str = "deck";

/// The presentation's window.
pub struct Presenter {
    viewer: Viewer,
    /// Whether the window is fullscreen, as last reported.
    fullscreen: bool,
    /// A fullscreen change asked for and not made yet.
    pending: Option<Fullscreen>,
    quit: bool,
    /// The window's size in points.
    viewport: (f32, f32),
    /// The viewer's version the view was last built for.
    built: u64,
    view: ValidatedView<()>,
}

/// A fullscreen change the presenter asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fullscreen {
    Toggle,
    Enter,
    Leave,
}

impl Presenter {
    /// A presenter on `deck`, open on slide `index` (from 0).
    pub fn new(deck: Deck, index: usize) -> Presenter {
        let viewer = Viewer::new(deck, index);
        let view = view(viewer.label(), 1);
        Presenter {
            built: viewer.version(),
            viewer,
            fullscreen: false,
            pending: None,
            quit: false,
            viewport: (WIDTH, HEIGHT),
            view,
        }
    }

    /// The viewer the window shows.
    pub fn viewer(&self) -> &Viewer {
        &self.viewer
    }

    /// Shows the presenter's notes under the slide, or hides them.
    pub fn show_notes(&mut self, on: bool) {
        self.viewer.show_notes(on);
        self.rebuild();
    }

    /// Opens the overview, or closes it.
    pub fn show_overview(&mut self, on: bool) {
        self.viewer.show_overview(on);
        self.rebuild();
    }

    /// Asks the window to open fullscreen.
    pub fn start_fullscreen(&mut self) {
        self.pending = Some(Fullscreen::Enter);
    }

    /// The slide showing, from 0.
    pub fn index(&self) -> usize {
        self.viewer.index()
    }

    /// The deck.
    pub fn deck(&self) -> &Deck {
        self.viewer.deck()
    }

    /// Every slide, laid out.
    pub fn slides(&self) -> &[Composed] {
        self.viewer.slides()
    }

    /// What the surface says to assistive technology: the showing
    /// slide's text, an image's alternative text included.
    pub fn label(&self) -> String {
        self.viewer.label()
    }

    /// Rebuilds the view when the viewer changed.
    fn rebuild(&mut self) {
        if self.built != self.viewer.version() {
            self.built = self.viewer.version();
            let revision = self.view.view().revision + 1;
            self.view = view(self.viewer.label(), revision);
        }
    }

    /// Answers one key. Returns whether it was the presenter's.
    pub fn key(&mut self, key: &str, command: bool) -> bool {
        let outcome = self.viewer.key(key, command);
        match outcome {
            Outcome::Ignored => return false,
            Outcome::Handled => {}
            // A window has nothing to close back to: it leaves fullscreen.
            Outcome::Close => self.pending = Some(Fullscreen::Leave),
            Outcome::ToggleFullscreen => self.pending = Some(Fullscreen::Toggle),
            Outcome::Quit => self.quit = true,
        }
        self.rebuild();
        true
    }

    /// A click at `x`, `y` points: on the overview, open the slide under
    /// it; on a slide, go on.
    pub fn click(&mut self, x: f32, y: f32) {
        self.viewer.click(x, y);
        self.rebuild();
    }

    /// Paints the presentation into `rect` of `frame`, in pixels.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect) {
        self.viewer.paint(frame, rect);
    }
}

/// The view: one surface the size of the window, labeled `label`.
fn view(label: String, revision: u64) -> ValidatedView<()> {
    let root = Node {
        key: "deck".into(),
        style: Style::default(),
        element: Element::Stack {
            axis: Axis::Vertical,
            children: vec![Node {
                key: "slides".into(),
                style: Style::default(),
                element: Element::Surface {
                    resource: SURFACE.into(),
                    label,
                },
            }],
        },
    };
    View::new("openagents-deck", revision.max(1), root)
        .validate()
        .expect("the deck's view is valid")
}

impl App for Presenter {
    type Intent = ();

    fn title(&self) -> String {
        self.viewer.deck().title()
    }

    fn theme(&self) -> Theme {
        // The views sit edge to edge: the surface is the whole window.
        Theme {
            margin: 0.0,
            column: 100_000.0,
            ..Theme::openagents()
        }
    }

    fn tick(&mut self, _: Instant) -> Option<Instant> {
        None
    }

    fn view(&self) -> &ValidatedView<()> {
        &self.view
    }

    fn activate(&mut self, _: (), _: Instant) {}

    fn text_input(&mut self, event: TextInput<'_>, _: Instant) -> bool {
        match event {
            TextInput::Key { key, command, .. } => self.key(key, command),
            _ => false,
        }
    }

    fn surface_input(&mut self, resource: &str, event: SurfaceInput, _: Instant) -> bool {
        match event {
            SurfaceInput::Down { x, y, .. } if resource == SURFACE => {
                self.click(x, y);
                true
            }
            _ => false,
        }
    }

    fn viewport(&mut self, width: f32, height: f32, _: f32) {
        self.viewport = (width, height);
        // Clicks arrive in points from the surface's corner.
        self.viewer.layout(PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        });
        self.rebuild();
    }

    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        (resource == SURFACE).then_some((available, self.viewport.1))
    }

    fn surface_version(&self, resource: &str) -> Option<u64> {
        (resource == SURFACE).then_some(self.viewer.version())
    }

    fn paint_surface(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) {
        if resource == SURFACE {
            self.paint(frame, rect);
        }
    }

    fn fullscreen_request(&mut self, fullscreen: bool) -> Option<bool> {
        self.fullscreen = fullscreen;
        let wanted = match self.pending.take()? {
            Fullscreen::Toggle => !fullscreen,
            Fullscreen::Enter => true,
            Fullscreen::Leave => false,
        };
        Some(wanted)
    }

    fn exit_requested(&self) -> bool {
        self.quit
    }
}

/// Paints `presenter` as the window would show it, `width` by `height`
/// pixels, through the window adapter's own layout and paint.
pub fn capture(presenter: &mut Presenter, width: usize, height: usize) -> Frame {
    let scale = 2.0;
    let (w, h) = (width as f32 / scale, height as f32 / scale);
    presenter.viewport(w, h, scale);
    rust_native_desktop::capture(presenter, w, h, scale).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script;

    fn presenter() -> Presenter {
        let deck = script::parse(
            "layout: title\nid: a\ntitle: A\nnotes: Say it\n\n---\n\n\
             layout: statement\nid: b\n\nOne sentence.\n\n---\n\n\
             layout: statement\nid: c\n\nAnother.\n",
        )
        .expect("the script parses");
        Presenter::new(deck, 0)
    }

    #[test]
    fn the_keys_move_through_the_deck() {
        let mut p = presenter();
        assert!(p.key("ArrowRight", false));
        assert_eq!(p.index(), 1);
        p.key("End", false);
        assert_eq!(p.index(), 2);
        p.key("n", false);
        assert_eq!(p.index(), 2, "the last slide stays");
        p.key("Home", false);
        assert_eq!(p.index(), 0);
        p.key("3", false);
        p.key("Enter", false);
        assert_eq!(p.index(), 2);
        p.key("k", false);
        assert_eq!(p.index(), 1);
        assert!(!p.key("x", false), "an unknown key is left to the window");
        assert!(!p.key("q", true), "command-q is left to the window");
        p.key("q", false);
        assert!(p.exit_requested());
    }

    #[test]
    fn escape_closes_what_is_open_then_leaves_fullscreen() {
        let mut p = presenter();
        p.key("o", false);
        p.key("t", false);
        p.key("Escape", false);
        assert!(!p.viewer().overview() && !p.viewer().notes());
        assert_eq!(p.fullscreen_request(true), None);
        p.key("Escape", false);
        assert_eq!(p.fullscreen_request(true), Some(false));
        p.key("f", false);
        assert_eq!(p.fullscreen_request(false), Some(true));
        assert_eq!(p.fullscreen_request(true), None, "asked once");
    }

    #[test]
    fn a_click_goes_on_and_the_overview_opens_the_card_clicked() {
        let mut p = presenter();
        p.viewport(800.0, 450.0, 1.0);
        p.click(10.0, 10.0);
        assert_eq!(p.index(), 1);
        p.key("o", false);
        let cells = overview_cells(3, 800.0, 450.0);
        let last = cells[2];
        p.click(last.x + 5.0, last.y + 5.0);
        assert_eq!(p.index(), 2);
        assert!(!p.viewer().overview());
    }

    #[test]
    fn a_change_repaints_the_surface_and_relabels_the_view() {
        let mut p = presenter();
        let before = p.surface_version(SURFACE);
        let label = p.label();
        p.key("ArrowRight", false);
        assert_ne!(p.surface_version(SURFACE), before);
        assert_ne!(p.label(), label);
        assert!(p.label().contains("Slide 2 of 3"));
    }
}
