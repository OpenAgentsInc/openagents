//! A desktop adapter for Rust Native views.
//!
//! An application describes each screen as a validated
//! [`rust_native::View`] and names what each control does with its own typed
//! intent. This adapter does the rest on a desktop:
//!
//! - [`layout`] places the view's nodes in points: stacks, lists, text,
//!   buttons (including checkboxes), and locally registered drawing
//!   surfaces. Text is broken into lines by `rust_native`'s own shaper with
//!   the bundled Paper Mono, so what is measured is what is painted.
//! - [`paint`] draws the laid-out scene into an RGBA [`Frame`] in software:
//!   antialiased rounded rectangles and glyphs rasterized with `swash`.
//! - [`backdrop`] (with `window`) draws an application's live picture with
//!   the window's own device behind the views, which keep full contrast.
//! - [`window`] (the default `window` feature) shows those frames in a
//!   `winit` window over a `wgpu` surface, turns pointer and keyboard input
//!   into revision-bound [`rust_native::Activation`]s, and resolves each one
//!   against the current view before the application sees its intent.
//!
//! - [`access`] describes the laid-out view to screen readers as an
//!   AccessKit tree, and the window answers their requests as the clicks,
//!   focus moves, and typing it already handles.
//!
//! What the adapter supports and refuses is listed in the crate README. An
//! element it can't draw is recorded in [`layout::Scene::unsupported`] and
//! falls back to its label, never to nothing.
//!
//! The adapter owns no application state, network, credential, or clock
//! source beyond the frame timing a window needs. The application decides
//! what an intent means and checks its own authority first.

pub mod access;
#[cfg(feature = "window")]
pub mod backdrop;
pub mod canvas;
pub mod composer;
mod fallback;
mod icons;
pub use icons::paint as paint_icon;
pub mod image;
pub mod input;
pub mod layout;
pub mod paint;
pub mod rich;
mod solar;
pub mod text;
pub mod theme;
pub mod timing;
pub mod transcript;
pub mod transfer;
#[cfg(all(target_os = "linux", feature = "window"))]
pub mod wayland;
#[cfg(feature = "window")]
pub mod window;

pub use canvas::{Frame, PxRect};
pub use layout::{OverlayLayout, OverlayPlacement, SplitLayout, WindowLayout};
pub use layout::{Rect, Scene};
pub use theme::Theme;

/// A command key chord that activates a node in the current view.
#[derive(Clone, Copy, Debug)]
pub struct KeyBinding {
    pub key: &'static str,
    pub shift: bool,
    pub node: &'static str,
}
/// The `wgpu` the window and a [`backdrop::Backdrop`] draw with.
#[cfg(feature = "window")]
pub use wgpu;

use rust_native::ValidatedView;
use serde::Serialize;
use std::sync::Arc;
use std::time::Instant;

/// Wakes the window's event loop from another thread, for example when a
/// background request finishes and the application has a new view.
#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    /// A waker that runs `wake`.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Waker {
        Waker(Arc::new(wake))
    }

    /// A waker that does nothing, for tests and captures.
    pub fn none() -> Waker {
        Waker::new(|| {})
    }

    /// Asks the event loop to call [`App::tick`] and paint again.
    pub fn wake(&self) {
        (self.0)()
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waker")
    }
}

/// An application the adapter shows.
///
/// The adapter calls [`tick`](App::tick) before every frame and whenever the
/// time it returned arrives, then reads [`view`](App::view). Activations
/// resolve against that view; the application receives only an intent the
/// view itself carried.
pub trait App {
    /// The application's closed intent type.
    type Intent: Clone + Serialize;

    /// The window's title.
    fn title(&self) -> String;

    /// The colors and type sizes the adapter paints with.
    fn theme(&self) -> Theme {
        Theme::default()
    }

    /// The system's light or dark appearance, when the window opens and
    /// whenever it changes (`None` when the platform does not say). An app
    /// whose theme follows the system resolves its scheme here and rebuilds
    /// its view; the window then asks for the [`Theme::appearance`] it
    /// returns. The default ignores it and stays dark.
    fn system_appearance(&mut self, _appearance: Option<theme::Appearance>) {}

    /// Layout of the window's semantic root. The default is a centered column.
    fn window_layout(&self) -> WindowLayout {
        WindowLayout::Column
    }

    /// Optional application-controlled leading scroll offset, in points.
    fn leading_scroll(&self) -> Option<f32> {
        None
    }

    /// The leading pane was resized by dragging its divider, in points.
    fn resize_leading_pane(&mut self, width: f32, now: Instant) {
        let _ = (width, now);
    }

    /// Command chords (Ctrl on Linux and Windows, Cmd on macOS).
    fn key_bindings(&self) -> &'static [KeyBinding] {
        &[]
    }

    /// Called once with a waker before the first frame.
    fn start(&mut self, waker: Waker) {
        let _ = waker;
    }

    /// Brings the application's state up to `now` and returns when it next
    /// needs a tick, if ever.
    fn tick(&mut self, now: Instant) -> Option<Instant>;

    /// The current view.
    fn view(&self) -> &ValidatedView<Self::Intent>;

    /// Runs an intent the current view resolved.
    fn activate(&mut self, intent: Self::Intent, now: Instant);

    /// Native text input for a focused local editor. Return whether it was consumed.
    fn text_input(&mut self, event: input::TextInput<'_>, now: Instant) -> bool {
        let _ = (event, now);
        false
    }

    /// Physical input before ordinary text or pointer dispatch. Return true
    /// only when the application's interactive viewport owns the event.
    fn native_input(&mut self, event: input::NativeInput<'_>, now: Instant) -> bool {
        let _ = (event, now);
        false
    }

    /// Whether an interactive viewport currently requests a hidden, grabbed cursor.
    fn cursor_capture(&self) -> bool {
        false
    }

    /// Native capture failed; the application must release its held controls.
    fn capture_failed(&mut self, now: Instant) {
        let _ = now;
    }

    /// The GPU layer failed; dispose its effects before returning to plain UI.
    fn graphics_failed(&mut self, error: &str, now: Instant) {
        let _ = (error, now);
    }

    /// Pointer input in a registered surface, with coordinates in local points.
    fn surface_input(&mut self, resource: &str, event: input::SurfaceInput, now: Instant) -> bool {
        let _ = (resource, event, now);
        false
    }

    /// A pointer press before normal hit dispatch. A modal may consume an outside click.
    fn pointer_down(&mut self, target: Option<&str>, point: (f32, f32), now: Instant) -> bool {
        let _ = (target, point, now);
        false
    }

    /// Explicit pointer motion over a control. Return true when presentation changes.
    fn pointer_hover(&mut self, target: Option<&str>, now: Instant) -> bool {
        let _ = (target, now);
        false
    }

    /// An explicit secondary click. The application admits the current target.
    fn context_menu(&mut self, target: Option<&str>, now: Instant) -> bool {
        let _ = (target, now);
        false
    }

    /// A secondary click with its position in logical window points.
    fn context_menu_at(&mut self, target: Option<&str>, point: (f32, f32), now: Instant) -> bool {
        let _ = point;
        self.context_menu(target, now)
    }

    /// The semantic root of the active modal, if any.
    fn modal_root(&self) -> Option<&str> {
        None
    }

    /// An optional floating third child of a header split's root.
    fn overlay_layout(&self) -> Option<OverlayLayout> {
        None
    }

    /// Restrict native keyboard focus while an application modal is mounted.
    fn allows_focus(&self, key: &str) -> bool {
        let _ = key;
        true
    }

    /// A bounded explanation for a currently hovered control.
    fn tooltip(&self, key: &str) -> Option<String> {
        let _ = key;
        None
    }

    /// Files explicitly dropped into this window. Applications admit their destination.
    fn dropped_file(&mut self, path: std::path::PathBuf, now: Instant) -> bool {
        let _ = (path, now);
        false
    }

    /// The logical viewport changed. Application surfaces can reserve its height.
    fn viewport(&mut self, width: f32, height: f32, scale: f32) {
        let _ = (width, height, scale);
    }

    /// The current text cursor, in window points, for the platform IME panel.
    fn ime_cursor(&self) -> Option<(f64, f64)> {
        None
    }

    /// The window became visible (`true`) or hidden: minimized, covered,
    /// on another space, or behind a locked screen.
    fn shown(&mut self, visible: bool, now: Instant) {
        let _ = (visible, now);
    }

    /// The person used the window: a key, a click, a scroll, or the pointer
    /// moving over it.
    fn input(&mut self, now: Instant) {
        let _ = now;
    }

    /// The size, in points, of the drawing surface `resource` when
    /// `available` points wide. `None` means the application has not
    /// registered it, and the adapter shows the surface's label instead.
    fn surface_size(&self, resource: &str, available: f32) -> Option<(f32, f32)> {
        let _ = (resource, available);
        None
    }

    /// Drawing revision for a surface, including selection, focus, and scrolling.
    /// `None` keeps the legacy behavior of refreshing it on every frame.
    fn surface_version(&self, resource: &str) -> Option<u64> {
        let _ = resource;
        None
    }

    /// Paints the surface `resource` into `rect` of `frame`, in pixels.
    fn paint_surface(&mut self, resource: &str, frame: &mut Frame, rect: PxRect) {
        let _ = (resource, frame, rect);
    }

    /// Local diagnostics when the native surface skips a frame before uploading pixels.
    fn frame_skipped(&mut self, timing: timing::FrameSkip) {
        let _ = timing;
    }

    /// Local diagnostics after a frame was submitted to the native surface.
    fn frame_presented(&mut self, timing: timing::FrameTiming) {
        let _ = timing;
    }

    /// The text a person has typed in the composer node `key`, for screen
    /// readers. `None` reads the composer's `draft`.
    fn access_value(&self, key: &str) -> Option<String> {
        let _ = key;
        None
    }

    /// The view node holding the application's own keyboard focus, such as
    /// a composer with the text cursor, for screen readers. `None` leaves
    /// the window's focus (Tab) as the focus.
    fn access_focus(&self) -> Option<String> {
        None
    }

    /// What the application paints in the surface `resource`, as semantic
    /// rows, for screen readers ([`access::Content`]). `None` leaves the
    /// surface an image named by its label.
    fn access_content(&self, resource: &str) -> Option<access::Content> {
        let _ = resource;
        None
    }

    /// Whether the application asked to close its window.
    fn exit_requested(&self) -> bool {
        false
    }

    /// A fullscreen change the application asks for, given whether the
    /// window is fullscreen now: `Some(true)` to enter, `Some(false)` to
    /// leave. Asked after every tick; answer `None` once the change is
    /// made.
    fn fullscreen_request(&mut self, fullscreen: bool) -> Option<bool> {
        let _ = fullscreen;
        None
    }

    /// Reports the native window's current fullscreen state before layout.
    fn fullscreen_changed(&mut self, fullscreen: bool) {
        let _ = fullscreen;
    }

    /// Whether the application asks for its window to come forward, for
    /// example after the person clicked one of its notifications. Asked
    /// after every tick; answer `true` once per request.
    fn focus_request(&mut self) -> bool {
        false
    }
}

/// Lays out and paints `app`'s current view into a frame `width` by
/// `height` points at `scale` pixels a point, as the window would show it
/// with nothing hovered or focused. Tests and `--capture` use it.
pub fn capture<A: App>(app: &mut A, width: f32, height: f32, scale: f32) -> (Frame, Scene) {
    let background = app.theme().background;
    capture_into(app, width, height, scale, |w, h| {
        Frame::new(w, h, background)
    })
}

/// As [`capture`], into a clear, premultiplied frame: the views alone, as a
/// window with a backdrop lays them over it.
pub fn capture_views<A: App>(app: &mut A, width: f32, height: f32, scale: f32) -> (Frame, Scene) {
    capture_into(app, width, height, scale, Frame::transparent)
}

fn capture_into<A: App>(
    app: &mut A,
    width: f32,
    height: f32,
    scale: f32,
    frame: impl FnOnce(usize, usize) -> Frame,
) -> (Frame, Scene) {
    app.viewport(width, height, scale);
    let theme = app.theme();
    let mut fonts = text::Fonts::new();
    let scene = layout::lay_out_with_overlay(
        app.view().view(),
        &theme,
        &mut fonts,
        &|resource, available| app.surface_size(resource, available),
        &layout::Interaction::default(),
        width,
        height,
        app.window_layout(),
        app.overlay_layout(),
    );
    let mut frame = frame(
        (width * scale).round() as usize,
        (height * scale).round() as usize,
    );
    paint::paint(
        &scene,
        &mut frame,
        scale,
        0.0,
        &mut fonts,
        &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
    );
    (frame, scene)
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    struct CaptureApp {
        view: ValidatedView<()>,
        viewport: Option<(f32, f32, f32)>,
        painted: bool,
    }
    impl App for CaptureApp {
        type Intent = ();
        fn title(&self) -> String {
            "Capture test".into()
        }
        fn tick(&mut self, _: Instant) -> Option<Instant> {
            None
        }
        fn view(&self) -> &ValidatedView<()> {
            &self.view
        }
        fn activate(&mut self, _: (), _: Instant) {}
        fn viewport(&mut self, width: f32, height: f32, scale: f32) {
            self.viewport = Some((width, height, scale));
        }
        fn surface_size(&self, _: &str, _: f32) -> Option<(f32, f32)> {
            assert_eq!(self.viewport, Some((300.0, 200.0, 2.0)));
            Some((40.0, 20.0))
        }
        fn paint_surface(&mut self, _: &str, _: &mut Frame, rect: PxRect) {
            assert_eq!(self.viewport, Some((300.0, 200.0, 2.0)));
            assert_eq!((rect.w, rect.h), (80.0, 40.0));
            self.painted = true;
        }
    }
    #[test]
    fn captures_supply_the_viewport_before_measuring_and_painting_surfaces() {
        let view = rust_native::View::new(
            "capture-test",
            1,
            rust_native::Node {
                key: "surface".into(),
                style: Default::default(),
                element: rust_native::Element::Surface {
                    label: "Test surface".into(),
                    resource: "test".into(),
                },
            },
        )
        .validate()
        .unwrap();
        let mut app = CaptureApp {
            view,
            viewport: None,
            painted: false,
        };
        let (frame, _) = capture(&mut app, 300.0, 200.0, 2.0);
        assert_eq!((frame.width, frame.height), (600, 400));
        assert!(app.painted);
    }
}
