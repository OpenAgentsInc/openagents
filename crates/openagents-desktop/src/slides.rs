//! The slide viewer: a deck from `openagents_deck` shown as an overlay
//! above the current page.
//!
//! [`Slides`] wraps an [`openagents_deck::Viewer`] with the overlay's own
//! state: the open and close animation, full screen, and the controls
//! (previous, next, the slide counter, full screen, and close). The
//! animation runs on the frame clock: the host passes each frame's
//! `Instant` to [`Slides::tick`], and nothing sleeps. Opening scales the
//! viewer from 0.96 to 1 and fades it from 0 to 1 over [`OPEN`], eased
//! out; closing runs the same curve backward. The look is dark only, in
//! gradations of white. Under "Reduce motion" the viewer opens and
//! closes at once.
//!
//! The host shows [`Slides::node`] as the window's overlay, laid over the
//! whole window ([`rust_native_desktop::OverlayPlacement::Cover`]), and
//! routes the [`RESOURCE`] surface's painting and input here.

use openagents_deck::{Outcome, UnknownDeck, Viewer};
use rust_native::style::{Color, Style, TextAlign};
use rust_native::{Element, Node};
use rust_native_desktop::input::SurfaceInput;
use rust_native_desktop::text::{Fonts, font};
use rust_native_desktop::{Frame, PxRect};
use std::time::{Duration, Instant};

/// How long the viewer takes to open, and to close.
pub const OPEN: Duration = Duration::from_millis(240);
/// The scale the viewer opens from.
pub const START_SCALE: f32 = 0.96;
/// The surface resource the viewer paints into.
pub const RESOURCE: &str = "slides-viewer";
/// The overlay's node key, which is also the window's modal root while
/// the viewer shows.
pub const NODE: &str = "slides-overlay";
/// How often the host asks for a frame while the viewer animates.
pub const FRAME: Duration = Duration::from_millis(16);

/// The margin around the viewer's card, in points, when not full screen.
const MARGIN: f32 = 40.0;
/// The height of the control bar under the slide, in points.
const BAR: f32 = 48.0;

/// Where the viewer is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Opening,
    Open,
    Closing,
    /// Fully closed: the host removes the overlay.
    Closed,
}

/// A control on the viewer's bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Previous,
    Next,
    FullScreen,
    Close,
}

/// The slide viewer overlay.
pub struct Slides {
    viewer: Viewer,
    deck_id: String,
    phase: Phase,
    /// When the running animation started, and how far open it was then.
    started: Instant,
    from: f32,
    /// How far open the viewer is, 0 to 1, as of the last tick.
    progress: f32,
    fullscreen: bool,
    /// Open and close at once, without the animation.
    reduce_motion: bool,
    version: u64,
    fonts: Fonts,
    /// The controls' rectangles as last painted, relative to the surface,
    /// in points.
    controls: Vec<(Control, PxRect)>,
    /// The surface's size in points as last painted.
    size: (f32, f32),
    /// Pixels a point.
    unit: f32,
    /// The host can draw a slide's live scene (the Grid) behind it.
    scene_host: bool,
    /// The slide's place in the window as last painted, in points.
    slide_at: Option<PxRect>,
}

/// Ease-out: fast at first, settling at the end.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

impl Slides {
    /// Opens the deck filed under `deck_id`, starting the open animation
    /// at `now`; with `reduce_motion`, fully open at once.
    pub fn open(deck_id: &str, now: Instant, reduce_motion: bool) -> Result<Slides, UnknownDeck> {
        let viewer = Viewer::open(deck_id)?;
        Ok(Slides {
            viewer,
            deck_id: deck_id.to_string(),
            phase: if reduce_motion {
                Phase::Open
            } else {
                Phase::Opening
            },
            started: now,
            from: 0.0,
            progress: if reduce_motion { 1.0 } else { 0.0 },
            fullscreen: false,
            reduce_motion,
            version: 1,
            fonts: Fonts::new(),
            controls: Vec::new(),
            size: (0.0, 0.0),
            unit: 1.0,
            scene_host: false,
            slide_at: None,
        })
    }

    /// The overlay's node: one surface over the whole window.
    pub fn node<I>(&self) -> Node<I> {
        Node {
            key: NODE.into(),
            style: Style {
                fill_height: Some(true),
                ..Style::default()
            },
            element: Element::Surface {
                resource: RESOURCE.into(),
                label: self.viewer.label(),
            },
        }
    }

    /// The window's scale, in pixels a point.
    pub fn set_unit(&mut self, unit: f32) {
        if unit.is_finite() && unit > 0.0 && unit != self.unit {
            self.unit = unit;
            self.changed();
        }
    }

    /// Says the host can draw a slide's live scene behind it
    /// ([`Slides::scene`]).
    pub fn set_scene_host(&mut self, on: bool) {
        if self.scene_host != on {
            self.scene_host = on;
            self.changed();
        }
    }

    /// Where the host draws the showing slide's live scene, in the
    /// window's points: the slide's place while it is fully open and asks
    /// for one (the Episode 289 title slide asks for the Grid).
    pub fn scene(&self) -> Option<rust_native_desktop::Rect> {
        let live = self.scene_host && self.phase == Phase::Open && self.viewer.scene().is_some();
        let at = self.slide_at.filter(|_| live)?;
        Some(rust_native_desktop::Rect {
            x: at.x,
            y: at.y,
            w: at.w,
            h: at.h,
        })
    }

    /// When the host should next call [`Slides::tick`]: a frame from
    /// `now` while animating, otherwise never.
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        self.animating().then(|| now + FRAME)
    }

    pub fn deck_id(&self) -> &str {
        &self.deck_id
    }

    pub fn viewer(&self) -> &Viewer {
        &self.viewer
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether the viewer shows the slide edge to edge.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Whether an animation is running, so the host asks for frames.
    pub fn animating(&self) -> bool {
        matches!(self.phase, Phase::Opening | Phase::Closing)
    }

    /// Whether the overlay is gone and the host should drop it.
    pub fn closed(&self) -> bool {
        self.phase == Phase::Closed
    }

    /// How far open the viewer is, 0 to 1, before easing.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// The eased openness: the opacity, 0 to 1.
    pub fn opacity(&self) -> f32 {
        ease_out(self.progress)
    }

    /// The viewer's scale: 0.96 closed, 1 open.
    pub fn scale(&self) -> f32 {
        START_SCALE + (1.0 - START_SCALE) * self.opacity()
    }

    /// The slide counter, such as "3 / 11".
    pub fn counter(&self) -> String {
        format!("{} / {}", self.viewer.index() + 1, self.viewer.deck().len())
    }

    /// Changes whenever the painting would.
    pub fn version(&self) -> u64 {
        self.version.wrapping_add(self.viewer.version())
    }

    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Advances the animation to `now`, the frame clock's time. Returns
    /// whether anything changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        if !self.animating() {
            return false;
        }
        let t = now.saturating_duration_since(self.started).as_secs_f32() / OPEN.as_secs_f32();
        let t = t.clamp(0.0, 1.0);
        match self.phase {
            Phase::Opening => {
                self.progress = self.from + (1.0 - self.from) * t;
                if t >= 1.0 {
                    self.progress = 1.0;
                    self.phase = Phase::Open;
                }
            }
            Phase::Closing => {
                self.progress = self.from * (1.0 - t);
                if t >= 1.0 {
                    self.progress = 0.0;
                    self.phase = Phase::Closed;
                }
            }
            Phase::Open | Phase::Closed => {}
        }
        self.changed();
        true
    }

    /// Starts the close animation at `now`, from wherever the viewer is.
    pub fn close(&mut self, now: Instant) {
        if matches!(self.phase, Phase::Closing | Phase::Closed) {
            return;
        }
        if self.reduce_motion {
            self.progress = 0.0;
            self.phase = Phase::Closed;
            self.fullscreen = false;
            self.changed();
            return;
        }
        self.tick(now);
        // A viewer still opening closes over only the distance it opened,
        // at the same speed.
        let elapsed = OPEN.mul_f32(1.0 - self.progress);
        self.from = 1.0;
        self.started = now.checked_sub(elapsed).unwrap_or(now);
        self.phase = Phase::Closing;
        self.fullscreen = false;
        self.changed();
    }

    /// Goes full screen, or leaves it.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        self.changed();
    }

    /// Answers a key. Returns whether the viewer took it; while open it
    /// takes every plain key, so none reaches the page under it, and
    /// leaves the window's command keys (quit, close) alone.
    pub fn key(&mut self, key: &str, command: bool, now: Instant) -> bool {
        if matches!(self.phase, Phase::Closing | Phase::Closed) {
            return false;
        }
        if command {
            let taken = self.viewer.key(key, true) != Outcome::Ignored;
            if taken {
                self.changed();
            }
            return taken;
        }
        let viewer = &self.viewer;
        let settled = !viewer.overview() && !viewer.notes() && !viewer.black();
        match key {
            "f" | "F" => {
                self.toggle_fullscreen();
                return true;
            }
            // Esc steps back: out of full screen first, then closed.
            "Escape" if self.fullscreen && settled => {
                self.fullscreen = false;
                self.changed();
                return true;
            }
            _ => {}
        }
        match self.viewer.key(key, false) {
            Outcome::Close | Outcome::Quit => self.close(now),
            Outcome::ToggleFullscreen => self.toggle_fullscreen(),
            Outcome::Handled | Outcome::Ignored => {}
        }
        self.changed();
        true
    }

    /// Runs a control.
    pub fn control(&mut self, control: Control, now: Instant) {
        match control {
            Control::Previous => {
                self.viewer_key("ArrowLeft");
            }
            Control::Next => {
                self.viewer_key("ArrowRight");
            }
            Control::FullScreen => self.toggle_fullscreen(),
            Control::Close => self.close(now),
        }
    }

    fn viewer_key(&mut self, key: &str) {
        self.viewer.key(key, false);
        self.changed();
    }

    /// Pointer input on the [`RESOURCE`] surface, in points relative to
    /// it. The viewer takes all of it while it shows.
    pub fn input(&mut self, event: SurfaceInput, now: Instant) -> bool {
        if self.closed() {
            return false;
        }
        if let SurfaceInput::Down { x, y, .. } = event
            && !matches!(self.phase, Phase::Closing)
        {
            let (width, height) = self.size;
            self.click(x, y, width, height, now);
        }
        true
    }

    /// A click at `x`, `y` relative to the `width` by `height` surface, in
    /// points. A click on a control runs it; a click on the slide goes
    /// on; a click outside the card closes the viewer.
    pub fn click(&mut self, x: f32, y: f32, width: f32, height: f32, now: Instant) {
        if let Some((control, _)) = self
            .controls
            .iter()
            .find(|(_, rect)| contains(*rect, x, y))
            .copied()
        {
            self.control(control, now);
            return;
        }
        let layout = Layout::of(width, height, self.fullscreen, 1.0);
        if contains(layout.slide, x, y) {
            self.viewer_key("ArrowRight");
        } else if !contains(layout.card, x, y) {
            self.close(now);
        }
    }

    /// The controls' rectangles, relative to the surface, as last painted.
    pub fn controls(&self) -> &[(Control, PxRect)] {
        &self.controls
    }

    /// Paints the overlay into `rect` of `frame`, in pixels, at the
    /// window's scale ([`Slides::set_unit`]).
    pub fn paint(&mut self, frame: &mut Frame, rect: PxRect) {
        let unit = self.unit.max(0.1);
        self.size = (rect.w / unit, rect.h / unit);
        let opacity = self.opacity();
        if opacity <= 0.0 {
            self.controls.clear();
            return;
        }
        if opacity >= 1.0 {
            self.paint_opaque(frame, rect, unit);
            return;
        }
        // Mid-animation: paint into a copy of what is under the overlay,
        // then fade that over the page.
        let (w, h) = (rect.w.ceil() as usize, rect.h.ceil() as usize);
        let (x0, y0) = (rect.x.max(0.0) as usize, rect.y.max(0.0) as usize);
        let mut layer = Frame::new(w.max(1), h.max(1), Color::rgb(0, 0, 0));
        copy(frame, x0, y0, &mut layer, 0, 0, w, h);
        let local = PxRect {
            x: 0.0,
            y: 0.0,
            w: rect.w,
            h: rect.h,
        };
        self.paint_opaque(&mut layer, local, unit);
        blend(frame, x0, y0, &layer, w, h, opacity);
    }

    fn paint_opaque(&mut self, frame: &mut Frame, rect: PxRect, unit: f32) {
        let scale = self.scale();
        let unit = unit.max(0.1);
        let layout = Layout::of(rect.w / unit, rect.h / unit, self.fullscreen, scale);
        let px = |r: PxRect| PxRect {
            x: rect.x + r.x * unit,
            y: rect.y + r.y * unit,
            w: r.w * unit,
            h: r.h * unit,
        };
        // The scrim over the page.
        frame.fill(
            rect,
            0.0,
            Color {
                alpha: if self.fullscreen { 255 } else { 214 },
                ..Color::rgb(0, 0, 0)
            },
        );
        if !self.fullscreen {
            frame.fill(px(layout.card), 14.0 * unit * scale, tone(14));
            frame.stroke(px(layout.card), 14.0 * unit * scale, unit, tone(46));
        }
        self.slide_at = Some(PxRect {
            x: rect.x / unit + layout.slide.x,
            y: rect.y / unit + layout.slide.y,
            w: layout.slide.w,
            h: layout.slide.h,
        });
        let live = self.scene_host && self.phase == Phase::Open;
        self.viewer.set_live_scene(live);
        self.viewer.layout(px(layout.slide));
        self.viewer.paint(frame, px(layout.slide));
        self.controls.clear();
        let Some(bar) = layout.bar else {
            return;
        };
        let size = 13.0 * scale;
        let counter = self.counter();
        let buttons = [
            (Control::Previous, "Previous", bar.x),
            (Control::Next, "Next", bar.x + 92.0 * scale),
            (
                Control::FullScreen,
                "Full screen",
                bar.x + bar.w - 196.0 * scale,
            ),
            (Control::Close, "Close", bar.x + bar.w - 84.0 * scale),
        ];
        for (control, label, x) in buttons {
            let width = match control {
                Control::FullScreen => 104.0,
                _ => 84.0,
            } * scale;
            let button = PxRect {
                x,
                y: bar.y + (bar.h - 30.0 * scale) / 2.0,
                w: width,
                h: 30.0 * scale,
            };
            frame.fill(px(button), 8.0 * unit * scale, tone(28));
            frame.stroke(px(button), 8.0 * unit * scale, unit, tone(56));
            self.text(frame, px(button), label, size * unit, tone(236));
            self.controls.push((control, button));
        }
        self.text(frame, px(bar), &counter, size * unit, tone(170));
    }

    fn text(&mut self, frame: &mut Frame, rect: PxRect, value: &str, size: f32, color: Color) {
        let paragraph = self.fonts.paragraph(
            value,
            font(size, rust_native::layout::display::Weight::Medium, false),
            None,
        );
        let line = size * rust_native_desktop::text::LINE_EM;
        self.fonts.draw(
            frame,
            &paragraph,
            rect.x,
            rect.y + ((rect.h - line) / 2.0).round(),
            rect.w,
            TextAlign::Center,
            1.0,
            color,
        );
    }
}

/// A gray: the viewer's only colors are gradations of white on black.
fn tone(level: u8) -> Color {
    Color::rgb(level, level, level)
}

fn contains(rect: PxRect, x: f32, y: f32) -> bool {
    x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
}

/// Where the card, the slide, and the control bar sit in a `width` by
/// `height` surface, in points, at `scale` about the center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub card: PxRect,
    pub slide: PxRect,
    pub bar: Option<PxRect>,
}

impl Layout {
    pub fn of(width: f32, height: f32, fullscreen: bool, scale: f32) -> Layout {
        let whole = PxRect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: height,
        };
        if fullscreen {
            return Layout {
                card: whole,
                slide: whole,
                bar: None,
            };
        }
        let aspect = openagents_deck::compose::WIDTH / openagents_deck::compose::HEIGHT;
        let room_w = (width - 2.0 * MARGIN).max(1.0);
        let room_h = (height - 2.0 * MARGIN - BAR).max(1.0);
        let slide_w = room_w.min(room_h * aspect);
        let slide_h = slide_w / aspect;
        let card_w = slide_w;
        let card_h = slide_h + BAR;
        let (cx, cy) = (width / 2.0, height / 2.0);
        let at = |x: f32, y: f32, w: f32, h: f32| PxRect {
            x: cx + (x - cx) * scale,
            y: cy + (y - cy) * scale,
            w: w * scale,
            h: h * scale,
        };
        let left = cx - card_w / 2.0;
        let top = cy - card_h / 2.0;
        Layout {
            card: at(left, top, card_w, card_h),
            slide: at(left, top, slide_w, slide_h),
            bar: Some(at(left + 12.0, top + slide_h, card_w - 24.0, BAR)),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn copy(
    src: &Frame,
    sx: usize,
    sy: usize,
    dst: &mut Frame,
    dx: usize,
    dy: usize,
    w: usize,
    h: usize,
) {
    for row in 0..h {
        let (ys, yd) = (sy + row, dy + row);
        if ys >= src.height || yd >= dst.height {
            break;
        }
        let n = w
            .min(src.width.saturating_sub(sx))
            .min(dst.width.saturating_sub(dx));
        let s = (ys * src.width + sx) * 4;
        let d = (yd * dst.width + dx) * 4;
        dst.pixels[d..d + n * 4].copy_from_slice(&src.pixels[s..s + n * 4]);
    }
}

/// Lays `layer` over `frame` at `x`, `y` with `opacity`.
fn blend(frame: &mut Frame, x: usize, y: usize, layer: &Frame, w: usize, h: usize, opacity: f32) {
    let a = (opacity.clamp(0.0, 1.0) * 256.0) as u32;
    for row in 0..h.min(layer.height) {
        let yd = y + row;
        if yd >= frame.height {
            break;
        }
        let n = w.min(layer.width).min(frame.width.saturating_sub(x));
        for col in 0..n {
            let s = (row * layer.width + col) * 4;
            let d = (yd * frame.width + x + col) * 4;
            for c in 0..4 {
                let (under, over) = (frame.pixels[d + c] as u32, layer.pixels[s + c] as u32);
                frame.pixels[d + c] = ((under * (256 - a) + over * a) >> 8) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DECK: &str = "three-devdays-later";

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Opening progresses over frames, scaling from 0.96 and fading from
    /// 0, eased out, and ends fully open at 240 ms.
    #[test]
    fn opening_animates_over_frames_to_fully_open() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        assert_eq!(slides.phase(), Phase::Opening);
        assert!(slides.animating());
        assert_eq!(slides.opacity(), 0.0);
        assert!((slides.scale() - 0.96).abs() < 1e-6);
        let mut last = 0.0;
        for frame in 1..15 {
            slides.tick(start + ms(frame * 16));
            assert!(slides.opacity() > last, "frame {frame} moves on");
            assert!(slides.opacity() < 1.0);
            last = slides.opacity();
        }
        slides.tick(start + ms(120));
        assert!(
            slides.opacity() > 0.5,
            "ease-out is past halfway at half time"
        );
        slides.tick(start + ms(240));
        assert_eq!(slides.phase(), Phase::Open);
        assert!(!slides.animating());
        assert_eq!(slides.opacity(), 1.0);
        assert_eq!(slides.scale(), 1.0);
    }

    /// Arrows and space move through the deck; the buttons do the same;
    /// the counter follows.
    #[test]
    fn navigation_moves_the_counter() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, false).expect("the deck opens");
        let total = slides.viewer().deck().len();
        assert_eq!(slides.counter(), format!("1 / {total}"));
        assert!(slides.key("ArrowRight", false, now));
        assert!(slides.key("Space", false, now));
        assert_eq!(slides.counter(), format!("3 / {total}"));
        slides.key("ArrowLeft", false, now);
        assert_eq!(slides.counter(), format!("2 / {total}"));
        slides.control(Control::Next, now);
        assert_eq!(slides.counter(), format!("3 / {total}"));
        slides.control(Control::Previous, now);
        slides.control(Control::Previous, now);
        slides.control(Control::Previous, now);
        assert_eq!(slides.counter(), format!("1 / {total}"));
    }

    /// `F` toggles full screen; Esc leaves full screen, then closes.
    #[test]
    fn f_toggles_full_screen_and_escape_steps_back_then_closes() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        slides.key("F", false, start + OPEN);
        assert!(slides.fullscreen());
        slides.key("f", false, start + OPEN);
        assert!(!slides.fullscreen());
        slides.key("f", false, start + OPEN);
        assert!(slides.fullscreen());
        slides.key("Escape", false, start + OPEN);
        assert!(!slides.fullscreen());
        assert_eq!(
            slides.phase(),
            Phase::Open,
            "Esc from full screen returns to the viewer"
        );
        slides.key("Escape", false, start + OPEN);
        assert_eq!(
            slides.phase(),
            Phase::Closing,
            "Esc from the viewer closes it"
        );
    }

    /// Closing runs the open animation backward, then the viewer is
    /// closed and takes no more keys.
    #[test]
    fn closing_animates_then_ends() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        let at = start + ms(300);
        slides.control(Control::Close, at);
        assert!(slides.animating());
        slides.tick(at + ms(16));
        let early = slides.opacity();
        assert!(early < 1.0 && early > 0.5);
        slides.tick(at + ms(120));
        assert!(slides.opacity() < early);
        assert!(slides.scale() < 1.0 && slides.scale() > 0.96);
        assert!(!slides.closed());
        slides.tick(at + OPEN);
        assert!(slides.closed());
        assert_eq!(slides.opacity(), 0.0);
        assert!(!slides.key("ArrowRight", false, at + OPEN));
    }

    /// Painting mid-open fades over the page; fully open paints the
    /// controls; full screen paints the slide edge to edge with no bar.
    #[test]
    fn paints_dark_with_controls_and_edge_to_edge_full_screen() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        let rect = PxRect {
            x: 0.0,
            y: 0.0,
            w: 960.0,
            h: 600.0,
        };
        let page = Color::rgb(200, 30, 30);
        slides.tick(start + ms(60));
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        let corner = &frame.pixels[0..3];
        assert!(
            corner[0] < 200 && corner[0] > 0,
            "mid-open fades over the page: {corner:?}"
        );
        slides.tick(start + OPEN);
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        let names: Vec<Control> = slides.controls().iter().map(|(c, _)| *c).collect();
        assert_eq!(
            names,
            [
                Control::Previous,
                Control::Next,
                Control::FullScreen,
                Control::Close
            ]
        );
        let (_, close) = slides.controls()[3];
        slides.click(close.x + 2.0, close.y + 2.0, 960.0, 600.0, start + OPEN);
        assert_eq!(slides.phase(), Phase::Closing);
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        slides.tick(start + OPEN);
        slides.toggle_fullscreen();
        let mut frame = Frame::new(960, 600, page);
        slides.paint(&mut frame, rect);
        assert!(slides.controls().is_empty());
        let layout = Layout::of(960.0, 600.0, true, 1.0);
        assert_eq!(layout.slide, rect);
        let corner = &frame.pixels[0..3];
        assert_ne!(corner, [200, 30, 30], "full screen covers the page");
    }
    /// Under "Reduce motion" the viewer opens and closes at once.
    #[test]
    fn reduced_motion_opens_and_closes_at_once() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        assert_eq!(slides.phase(), Phase::Open);
        assert!(!slides.animating());
        assert_eq!(slides.opacity(), 1.0);
        assert_eq!(slides.scale(), 1.0);
        assert_eq!(slides.next_wake(now), None);
        slides.key("Escape", false, now);
        assert!(slides.closed(), "closing is instant too");
        assert_eq!(slides.opacity(), 0.0);
    }

    /// The window's command keys (quit, close) pass through; the viewer's
    /// own command keys (zoom) do not; plain keys never reach the page.
    #[test]
    fn command_keys_pass_through_and_plain_keys_are_taken() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        assert!(!slides.key("q", true, now));
        assert!(!slides.key("w", true, now));
        assert!(slides.key("=", true, now));
        assert!(slides.key("x", false, now));
        assert_eq!(slides.phase(), Phase::Open);
    }

    /// Esc with the overview open closes the overview, not full screen.
    #[test]
    fn escape_closes_the_overview_before_full_screen() {
        let now = Instant::now();
        let mut slides = Slides::open(DECK, now, true).expect("the deck opens");
        slides.key("f", false, now);
        slides.key("o", false, now);
        assert!(slides.viewer().overview());
        slides.key("Escape", false, now);
        assert!(!slides.viewer().overview());
        assert!(slides.fullscreen());
        slides.key("Escape", false, now);
        assert!(!slides.fullscreen());
        assert_eq!(slides.phase(), Phase::Open);
    }

    /// While animating the viewer asks for frames; once open it does not.
    #[test]
    fn asks_for_frames_only_while_animating() {
        let start = Instant::now();
        let mut slides = Slides::open(DECK, start, false).expect("the deck opens");
        assert_eq!(slides.next_wake(start), Some(start + FRAME));
        let before = slides.version();
        slides.tick(start + ms(16));
        assert_ne!(slides.version(), before, "each frame repaints");
        slides.tick(start + OPEN);
        assert_eq!(slides.next_wake(start + OPEN), None);
    }
}
