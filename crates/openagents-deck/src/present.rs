//! The presentation: which slide shows, the keys, and the frame.
//!
//! [`Presenter`] is a [`rust_native_desktop::App`]. Its view is one
//! drawing surface the size of the window, which the window adapter lays
//! out, shows, and captures like any other Rust Native view; the surface
//! paints the current slide's parts (see [`crate::compose`]), the
//! overview, the presenter's notes, or a black screen. A slide appears
//! whole the moment it opens: there is no per-slide animation, and the
//! surface is painted again only when something changed.

use crate::compose::{self, Composed, Content, HEIGHT, WIDTH};
use crate::slide::Deck;
use rust_native::layout::display::ColorRole;
use rust_native::style::{Color, Style};
use rust_native::{Axis, Element, Node, ValidatedView, View};
use rust_native_desktop::input::{SurfaceInput, TextInput};
use rust_native_desktop::rich::{self, Rich};
use rust_native_desktop::text::Fonts;
use rust_native_desktop::{App, Frame, PxRect, Theme};
use std::time::Instant;

/// The surface the whole presentation paints into.
const SURFACE: &str = "deck";
/// The share of the window's height the slide keeps when the notes show.
const SLIDE_SHARE_WITH_NOTES: f32 = 0.66;

/// The presentation's state.
pub struct Presenter {
    deck: Deck,
    slides: Vec<Composed>,
    notes_laid: Vec<Rich>,
    index: usize,
    notes: bool,
    overview: bool,
    black: bool,
    zoom: f32,
    /// Digits typed toward a jump, waiting for enter.
    typed: String,
    /// Whether the window is fullscreen, as last reported.
    fullscreen: bool,
    /// A fullscreen change asked for and not made yet.
    pending: Option<Fullscreen>,
    quit: bool,
    /// Changes whenever the painted surface would.
    version: u64,
    viewport: (f32, f32),
    fonts: Fonts,
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
        let slides: Vec<Composed> = (0..deck.len())
            .map(|index| compose::compose(&deck, index))
            .collect();
        let notes_laid = deck
            .slides
            .iter()
            .map(|slide| compose::notes(Some(slide), compose::COLUMN))
            .collect();
        let index = index.min(deck.len().saturating_sub(1));
        let view = view(&deck, &slides, index, 1);
        Presenter {
            index,
            deck,
            slides,
            notes_laid,
            notes: false,
            overview: false,
            black: false,
            zoom: 1.0,
            typed: String::new(),
            fullscreen: false,
            pending: None,
            quit: false,
            version: 1,
            viewport: (WIDTH, HEIGHT),
            fonts: Fonts::new(),
            view,
        }
    }

    /// Shows the presenter's notes under the slide, or hides them.
    pub fn show_notes(&mut self, on: bool) {
        self.notes = on;
        self.changed();
    }

    /// Opens the overview, or closes it.
    pub fn show_overview(&mut self, on: bool) {
        self.overview = on;
        self.changed();
    }

    /// Asks the window to open fullscreen.
    pub fn start_fullscreen(&mut self) {
        self.pending = Some(Fullscreen::Enter);
    }

    /// The slide showing, from 0.
    pub fn index(&self) -> usize {
        self.index
    }

    /// The deck.
    pub fn deck(&self) -> &Deck {
        &self.deck
    }

    /// Every slide, laid out.
    pub fn slides(&self) -> &[Composed] {
        &self.slides
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        let revision = self.view.view().revision + 1;
        self.view = view(&self.deck, &self.slides, self.index, revision);
    }

    /// What the surface says to assistive technology: the showing
    /// slide's text, an image's alternative text included.
    pub fn label(&self) -> String {
        match &self.view.view().root.element {
            Element::Stack { children, .. } => match children.first().map(|c| &c.element) {
                Some(Element::Surface { label, .. }) => label.clone(),
                _ => String::new(),
            },
            _ => String::new(),
        }
    }

    /// Opens slide `index`, clamped to the deck.
    fn go(&mut self, index: usize) {
        self.index = index.min(self.deck.len().saturating_sub(1));
        self.overview = false;
        self.black = false;
        self.changed();
    }

    /// Answers one key. Returns whether it was the presenter's.
    pub fn key(&mut self, key: &str, command: bool) -> bool {
        if command {
            match key {
                "=" | "+" => self.zoom = (self.zoom * 1.1).min(4.0),
                "-" => self.zoom = (self.zoom / 1.1).max(0.25),
                "0" => self.zoom = 1.0,
                _ => return false,
            }
            self.changed();
            return true;
        }
        let columns = overview_columns(self.deck.len());
        match key {
            "ArrowRight" | "Space" | " " | "PageDown" | "n" | "j" => self.go(self.index + 1),
            "ArrowLeft" | "PageUp" | "p" | "k" => self.go(self.index.saturating_sub(1)),
            "ArrowDown" if self.overview => {
                self.go(self.index + columns);
                self.overview = true;
            }
            "ArrowUp" if self.overview => {
                self.go(self.index.saturating_sub(columns));
                self.overview = true;
            }
            "Home" => self.go(0),
            "End" => self.go(self.deck.len()),
            "Enter" => {
                if let Ok(number) = self.typed.parse::<usize>() {
                    self.go(number.saturating_sub(1));
                } else if self.overview {
                    self.overview = false;
                }
                self.typed.clear();
            }
            "Escape" => {
                if self.overview || self.notes || self.black || !self.typed.is_empty() {
                    self.overview = false;
                    self.notes = false;
                    self.black = false;
                    self.typed.clear();
                } else {
                    self.pending = Some(Fullscreen::Leave);
                }
            }
            "o" => self.overview = !self.overview,
            "t" => self.notes = !self.notes,
            "." | "b" => self.black = !self.black,
            "f" => self.pending = Some(Fullscreen::Toggle),
            "q" => self.quit = true,
            digit if !digit.is_empty() && digit.chars().all(|c| c.is_ascii_digit()) => {
                self.typed.push_str(digit)
            }
            _ => return false,
        }
        self.changed();
        true
    }

    /// A click at `x`, `y` points: on the overview, open the slide under
    /// it; on a slide, go on.
    pub fn click(&mut self, x: f32, y: f32) {
        if !self.overview {
            self.go(self.index + 1);
            return;
        }
        let (width, height) = self.viewport;
        if let Some(index) = overview_cells(self.deck.len(), width, height)
            .iter()
            .position(|cell| cell.contains(x, y))
        {
            self.go(index);
        }
    }

    /// Paints the presentation into `rect` of `frame`, in pixels.
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect) {
        let previous = frame.clip_to(rect);
        let theme = Theme::openagents();
        frame.fill(rect, 0.0, theme.background);
        if self.black {
            frame.fill(rect, 0.0, Color::rgb(0, 0, 0));
        } else if self.overview {
            self.paint_overview(frame, rect);
        } else {
            let slide_height = if self.notes {
                rect.h * SLIDE_SHARE_WITH_NOTES
            } else {
                rect.h
            };
            let scale = (rect.w / WIDTH).min(slide_height / HEIGHT) * self.zoom;
            let x = rect.x + ((rect.w - WIDTH * scale) / 2.0).round();
            let y = rect.y + ((slide_height - HEIGHT * scale) / 2.0).round();
            if let Some(slide) = self.slides.get(self.index) {
                paint_slide(frame, &mut self.fonts, slide, x, y, scale);
            }
            if self.notes {
                self.paint_notes(frame, rect, slide_height, x, scale);
            }
            if !self.typed.is_empty() {
                let go = compose::status(&format!("Go to {}", self.typed), 200.0);
                go.paint(
                    frame,
                    &mut self.fonts,
                    x + compose::MARGIN_X * scale,
                    y + 12.0 * scale,
                    scale,
                    1.0,
                    None,
                );
            }
        }
        frame.restore_clip(previous);
    }

    /// The notes under the slide, `top` pixels down `rect`.
    fn paint_notes(&mut self, frame: &mut Frame, rect: PxRect, top: f32, x: f32, scale: f32) {
        let Some(notes) = self.notes_laid.get(self.index) else {
            return;
        };
        notes.paint(
            frame,
            &mut self.fonts,
            x + compose::MARGIN_X * scale,
            rect.y + top,
            scale,
            1.0,
            Some(rect.y + rect.h - 8.0 * scale),
        );
    }

    fn paint_overview(&mut self, frame: &mut Frame, rect: PxRect) {
        let cells = overview_cells(self.deck.len(), rect.w, rect.h);
        let theme = Theme::openagents();
        for (index, cell) in cells.iter().enumerate() {
            let Some(slide) = self.slides.get(index) else {
                continue;
            };
            let scale = cell.w / WIDTH;
            let (x, y) = (rect.x + cell.x, rect.y + cell.y);
            let bounds = PxRect {
                x,
                y,
                w: cell.w,
                h: cell.h,
            };
            let previous = frame.clip_to(bounds);
            paint_slide(frame, &mut self.fonts, slide, x, y, scale);
            frame.restore_clip(previous);
            let (color, width) = if index == self.index {
                (theme.text, 2.0)
            } else {
                (rich::color(ColorRole::Border), 1.0)
            };
            let unit = (rect.w / 1000.0).max(1.0);
            frame.stroke(bounds, 8.0 * unit, width * unit, color);
        }
    }
}

/// The view: one surface the size of the window, labeled with the slide at
/// `index`'s text.
fn view(deck: &Deck, slides: &[Composed], index: usize, revision: u64) -> ValidatedView<()> {
    let mut label: Vec<String> = slides
        .get(index)
        .map(|slide| {
            slide
                .parts
                .iter()
                .filter(|part| part.kind != "foot")
                .map(|part| part.text.clone())
                .collect()
        })
        .unwrap_or_default();
    label.push(format!("Slide {} of {}", index + 1, deck.len()));
    let label: String = label.join(". ").chars().take(4_000).collect();
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

/// Paints `slide` with the canvas's top-left corner at `x`, `y` pixels,
/// `scale` pixels a point.
pub fn paint_slide(
    frame: &mut Frame,
    fonts: &mut Fonts,
    slide: &Composed,
    x: f32,
    y: f32,
    scale: f32,
) {
    for part in &slide.parts {
        let (px, py) = (x + part.x * scale, y + part.y * scale);
        match &part.content {
            Content::Rich(rich) => {
                rich.paint(frame, fonts, px, py, scale, part.magnification, None);
            }
            Content::Image {
                image,
                width,
                height,
                grow,
            } => {
                let area = PxRect {
                    x: px,
                    y: py,
                    w: width * scale,
                    h: height * scale,
                };
                // As large as fits, but never past `grow` screen pixels an
                // image pixel (1 unless the slide asks for more), so a
                // screenshot stays sharp.
                let at = rust_native_desktop::image::fit(image.width, image.height, area, *grow);
                rust_native_desktop::image::paint(frame, image, at);
            }
        }
    }
}

/// A rectangle in whatever unit its caller uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Cell {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// How many columns the overview lays `count` slides out in: the fewest
/// that keep the grid no taller than it is wide.
fn overview_columns(count: usize) -> usize {
    let mut columns = 1;
    while columns * columns < count {
        columns += 1;
    }
    columns
}

/// Where each of `count` slides sits in the overview of a `width` by
/// `height` area: a 16:9 card each, in rows, the grid centered.
pub fn overview_cells(count: usize, width: f32, height: f32) -> Vec<Cell> {
    if count == 0 {
        return vec![];
    }
    let columns = overview_columns(count);
    let rows = count.div_ceil(columns);
    let gap = (width.min(height) * 0.02).max(4.0);
    let card_w = ((width - gap * (columns as f32 + 1.0)) / columns as f32)
        .min((height - gap * (rows as f32 + 1.0)) / rows as f32 * WIDTH / HEIGHT)
        .max(1.0);
    let card_h = card_w * HEIGHT / WIDTH;
    let grid_w = columns as f32 * card_w + (columns as f32 - 1.0) * gap;
    let grid_h = rows as f32 * card_h + (rows as f32 - 1.0) * gap;
    let left = ((width - grid_w) / 2.0).round();
    let top = ((height - grid_h) / 2.0).round();
    (0..count)
        .map(|index| Cell {
            x: left + (index % columns) as f32 * (card_w + gap),
            y: top + (index / columns) as f32 * (card_h + gap),
            w: card_w,
            h: card_h,
        })
        .collect()
}

impl App for Presenter {
    type Intent = ();

    fn title(&self) -> String {
        self.deck.title()
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
        if (width, height) != self.viewport {
            self.viewport = (width, height);
            self.changed();
        }
    }

    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        (resource == SURFACE).then_some((available, self.viewport.1))
    }

    fn surface_version(&self, resource: &str) -> Option<u64> {
        (resource == SURFACE).then_some(self.version)
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
        assert!(!p.overview && !p.notes);
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
        assert!(!p.overview);
    }

    #[test]
    fn a_change_repaints_the_surface() {
        let mut p = presenter();
        let before = p.surface_version(SURFACE);
        p.key("ArrowRight", false);
        assert_ne!(p.surface_version(SURFACE), before);
    }

    #[test]
    fn the_overview_cards_are_16_by_9_and_do_not_overlap() {
        let cells = overview_cells(22, 1920.0, 1080.0);
        assert_eq!(cells.len(), 22);
        for (i, a) in cells.iter().enumerate() {
            assert!((a.w / a.h - WIDTH / HEIGHT).abs() < 0.01);
            assert!(a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= 1920.0 && a.y + a.h <= 1080.0);
            for b in &cells[i + 1..] {
                let apart =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }
}
