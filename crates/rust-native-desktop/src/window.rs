//! A native window for an [`App`].
//!
//! One `winit` window over a `wgpu` surface. Each frame is laid out and
//! painted in software, then copied into the surface texture. The foreground
//! is retained, and changes repaint only the affected regions. A frame is
//! updated only when the view, the window size, or the
//! pointer's target changes. Between frames the event loop sleeps until the
//! time the application's [`App::tick`] asked for, or until a [`Waker`]
//! wakes it.
//!
//! With a [`Backdrop`] ([`run_with_backdrop`]), the views are painted into a
//! clear frame only when they change, and each frame the window draws the
//! backdrop with the same device and lays the views over it in one pass
//! (`backdrop.rs`). Frames come as the backdrop asks for them, never while
//! the window is hidden.
//!
//! A working indicator in the view animates on the shared
//! [`rust_native::motion::Working`] clock, about 30 frames a second, and
//! repaints only its own cells. Frames come only while one is drawn, the
//! window shows, and motion is not reduced; otherwise the indicator rests
//! and the loop sleeps.
//!
//! Input becomes a revision-bound [`Activation`] naming the view's instance,
//! revision, and the button's key; the adapter resolves it against the
//! current view with [`rust_native::ValidatedView::activate`] and hands the
//! application only the intent that view carried.

use crate::backdrop::{Backdrop, Compositor, Gpu as BackdropGpu, Look};
use crate::input::{NativeInput, SurfaceInput, TextInput};
use crate::layout::{Interaction, Scene, WindowLayout, lay_out_with_overlay};
use crate::text::Fonts;
use crate::theme::Appearance;
use crate::timing::{FrameTiming, Phase, Timings};
use crate::{App, Waker, paint};
use rust_native::Activation;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorGrabMode, CursorIcon, Fullscreen, Window, WindowId};

mod access;

#[cfg(target_os = "macos")]
fn position_header_controls(window: &Window, height: f32) {
    use objc2_app_kit::{NSView, NSWindowButton};
    use objc2_foundation::NSPoint;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    // Winit owns this NSView for the live window. This callback runs on the
    // main event-loop thread, and the borrowed window outlives these accesses.
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    let Some(native) = view.window() else {
        return;
    };
    let Some(close) = native.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    // AppKit keeps the window-control container alive with its buttons.
    let Some(parent) = (unsafe { close.superview() }) else {
        return;
    };
    let mut frame = parent.frame();
    frame.origin.y += frame.size.height - f64::from(height);
    frame.size.height = f64::from(height);
    parent.setFrame(frame);
    let first = close.frame().origin.x;
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = native.standardWindowButton(kind) {
            let frame = button.frame();
            button.setFrameOrigin(NSPoint::new(
                14.0 + frame.origin.x - first,
                f64::from(height) - 14.0 - frame.size.height,
            ));
        }
    }
}

/// How long a backdrop waits after the surface skipped a frame.
const SKIPPED_FRAME_WAIT: std::time::Duration = std::time::Duration::from_millis(250);

/// How the window opens.
#[derive(Clone, Debug)]
pub struct Options {
    /// The window's size, in points, when [`Options::fill`] is unset or the
    /// display cannot be read.
    pub size: (f64, f64),
    /// The smallest the window may be, in points.
    pub min_size: (f64, f64),
    /// Open at this share (0 to 1) of the current display's usable area
    /// (on the Mac its visible frame, without the menu bar and the Dock),
    /// centered on it, instead of at [`Options::size`].
    pub fill: Option<f64>,
    /// Scale the views up when the window is larger than they were drawn
    /// for, so a large window shows larger type and controls rather than a
    /// small column in a large field.
    pub zoom: Option<Zoom>,
    /// How a backdrop sits under the views, when there is one.
    pub look: Look,
    /// Exact physical dimensions for repeatable rendering fixtures.
    pub pixel_size: Option<(u32, u32)>,
    /// Override pixels per view point for rendering fixtures.
    pub render_scale: Option<f32>,
    /// Keep a short-lived rendering fixture above other windows while it samples.
    pub fixture_frontmost: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            size: (560.0, 720.0),
            min_size: (420.0, 520.0),
            fill: None,
            zoom: None,
            look: Look::default(),
            pixel_size: None,
            render_scale: None,
            fixture_frontmost: false,
        }
    }
}

/// How the views grow with the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zoom {
    /// The window size, in points, the views were drawn for. At or below it
    /// they draw at their own size.
    pub design: (f32, f32),
    /// The most the views grow.
    pub max: f32,
}

impl Zoom {
    /// The factor for a window `width` by `height` points: the smaller of
    /// the two growths, between 1 and [`Zoom::max`].
    #[must_use]
    pub fn factor(self, width: f32, height: f32) -> f32 {
        let grow = (width / self.design.0.max(1.0)).min(height / self.design.1.max(1.0));
        if grow.is_finite() {
            grow.clamp(1.0, self.max.max(1.0))
        } else {
            1.0
        }
    }
}

/// A rectangle on the desktop, in points, with the origin at the top left
/// of the primary display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Where a window filling `fill` of `area` goes: its outer top-left corner
/// and its inner size, centered on `area`. `title` is the height of the
/// title bar above the inner size. The size never falls below `min`.
#[must_use]
pub fn placement(area: Area, fill: f64, title: f64, min: (f64, f64)) -> ((f64, f64), (f64, f64)) {
    let fill = fill.clamp(0.1, 1.0);
    let outer_width = (area.width * fill).round().max(min.0);
    let outer_height = (area.height * fill).round().max(min.1 + title);
    (
        centered(area, (outer_width, outer_height)),
        (outer_width, outer_height - title),
    )
}

/// The outer top-left corner that centers a window of `outer` size on
/// `area`.
#[must_use]
pub fn centered(area: Area, outer: (f64, f64)) -> (f64, f64) {
    (
        (area.x + (area.width - outer.0) / 2.0).round(),
        (area.y + (area.height - outer.1) / 2.0).round(),
    )
}

/// Opens a window on `app` and runs until it closes or the application asks
/// to exit.
pub fn run<A: App>(app: A, options: Options) -> Result<(), String> {
    run_shell(app, options, None)
}

/// As [`run`], with `backdrop` drawn behind the views.
pub fn run_with_backdrop<A: App>(
    app: A,
    options: Options,
    backdrop: Box<dyn Backdrop>,
) -> Result<(), String> {
    run_shell(app, options, Some(backdrop))
}

fn run_shell<A: App>(
    app: A,
    options: Options,
    backdrop: Option<Box<dyn Backdrop>>,
) -> Result<(), String> {
    let event_loop = EventLoop::<()>::with_user_event()
        .build()
        .map_err(|error| format!("no event loop: {error}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    let mut shell = Shell {
        app,
        options,
        proxy: Some(proxy),
        window: None,
        gpu: None,
        fonts: Fonts::new(),
        scene: None,
        laid_out: None,
        surface_sizes: Vec::new(),
        modal_surfaces: None,
        interaction: Interaction::default(),
        cursor: PhysicalPosition::new(-1.0, -1.0),
        modifiers: ModifiersState::empty(),
        scroll: 0.0,
        visible: true,
        wake: None,
        error: None,
        backdrop,
        compositor: None,
        painted: false,
        foreground: paint::Retained::default(),
        hold: None,
        resizing: false,
        timings: Timings::from_env(),
        captured_surface: None,
        captured_cursor: false,
        access: None,
        working: rust_native::motion::Working::new(Instant::now()),
        working_due: None,
        #[cfg(target_os = "linux")]
        drops: None,
        #[cfg(target_os = "linux")]
        wake_proxy: None,
        system_appearance: None,
        window_appearance: None,
    };
    event_loop
        .run_app(&mut shell)
        .map_err(|error| format!("the event loop failed: {error}"))?;
    match shell.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// The height of a title bar, in points, above a window's inner size.
const TITLE_BAR: f64 = if cfg!(target_os = "macos") {
    28.0
} else {
    32.0
};

/// The current display's usable area: on the Mac the visible frame of the
/// screen with the key window (the menu bar's, at launch), without the menu
/// bar and the Dock.
#[cfg(target_os = "macos")]
fn usable_area(_: &ActiveEventLoop) -> Option<Area> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    let marker = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(marker)?;
    // AppKit puts the origin at the bottom left of the first screen; winit
    // puts it at the top left.
    let primary = NSScreen::screens(marker).firstObject()?.frame();
    let visible = screen.visibleFrame();
    let top = primary.size.height - (visible.origin.y + visible.size.height);
    Some(Area {
        x: visible.origin.x,
        y: top,
        width: visible.size.width,
        height: visible.size.height,
    })
}

/// The current display's usable area: elsewhere the primary monitor
/// (else the first), whole. A 90% fill centered on it leaves room for a
/// taskbar or a panel.
#[cfg(not(target_os = "macos"))]
fn usable_area(event_loop: &ActiveEventLoop) -> Option<Area> {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())?;
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let position = monitor.position().to_logical::<f64>(scale);
    Some(Area {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

/// The surface and the device that copies frames into it.
struct Gpu {
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The format a render pass writes: the surface's, without sRGB
    /// encoding, since frames are already encoded.
    encoded: wgpu::TextureFormat,
}

/// What a scene was laid out for.
#[derive(Clone, Debug, PartialEq)]
struct LaidOut {
    instance: String,
    revision: u64,
    size: (u32, u32),
    scale: u32,
    interaction: Interaction,
    layout: WindowLayout,
    overlay: Option<crate::layout::OverlayLayout>,
}

/// Intrinsic requests remain distinct from the rectangles assigned by layout.
/// A filling surface can occupy more space without changing its requested size.
#[derive(Clone, Debug)]
struct SurfaceSize {
    resource: String,
    available: f32,
    requested: Option<(f32, f32)>,
}

fn surface_sizes(
    scene: &Scene,
    size: impl Fn(&str, f32) -> Option<(f32, f32)>,
) -> Vec<SurfaceSize> {
    scene
        .ops
        .iter()
        .filter_map(|op| {
            if let crate::layout::Op::Surface { resource, rect, .. } = op {
                Some(SurfaceSize {
                    resource: resource.clone(),
                    available: rect.w,
                    requested: size(resource, rect.w),
                })
            } else {
                None
            }
        })
        .collect()
}

fn surface_sizes_changed(
    previous: &[SurfaceSize],
    size: impl Fn(&str, f32) -> Option<(f32, f32)>,
) -> bool {
    previous.iter().any(|cached| {
        match (cached.requested, size(&cached.resource, cached.available)) {
            (Some((old_w, old_h)), Some((w, h))) => {
                (w - old_w).abs() > 0.5 || (h - old_h).abs() > 0.5
            }
            (None, None) => false,
            _ => true,
        }
    })
}

fn input_requires_redraw(
    previous: Option<&LaidOut>,
    current: &LaidOut,
    scene: Option<&Scene>,
    version: impl Fn(&str) -> Option<u64>,
    resized_surface: bool,
) -> bool {
    previous != Some(current)
        || resized_surface
        || scene.is_none_or(|scene| {
            scene.ops.iter().any(|op| {
                matches!(op, crate::layout::Op::Surface { resource, version: cached, .. }
                    if cached.is_none() || version(resource) != *cached)
            })
        })
}

/// Only surfaces inside the current modal subtree can receive pointer input.
/// Bounds distinguish a resource reused by the popup and the underlying page.
fn modal_surfaces<I>(
    root: &rust_native::Node<I>,
    modal: Option<&str>,
    scene: &Scene,
) -> Option<Vec<(String, crate::layout::Rect)>> {
    use rust_native::Element;
    let modal = modal?;
    let mut pending = vec![(root, false)];
    let mut surfaces = Vec::new();
    while let Some((node, admitted)) = pending.pop() {
        let admitted = admitted || node.key == modal;
        if admitted && let Some(bounds) = scene.bounds.get(&node.key) {
            let resource = match &node.element {
                Element::Surface { resource, .. } => Some(resource.clone()),
                Element::Composer { .. } => Some(format!("composer:{}", node.key)),
                _ => None,
            };
            if let Some(resource) = resource {
                surfaces.push((resource, *bounds));
            }
        }
        if let Element::Stack { children, .. }
        | Element::List { children, .. }
        | Element::Transcript { children, .. }
        | Element::Message { children, .. }
        | Element::Tool { children, .. } = &node.element
        {
            pending.extend(children.iter().map(|child| (child, admitted)));
        }
    }
    Some(surfaces)
}

fn surface_admitted(
    modal: Option<&[(String, crate::layout::Rect)]>,
    resource: &str,
    rect: crate::layout::Rect,
) -> bool {
    modal.is_none_or(|surfaces| {
        surfaces.iter().any(|(name, bounds)| {
            name == resource
                && rect.x >= bounds.x - 0.01
                && rect.y >= bounds.y - 0.01
                && rect.x + rect.w <= bounds.x + bounds.w + 0.01
                && rect.y + rect.h <= bounds.y + bounds.h + 0.01
        })
    })
}

struct Shell<A: App> {
    app: A,
    options: Options,
    proxy: Option<EventLoopProxy<()>>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    fonts: Fonts,
    scene: Option<Scene>,
    laid_out: Option<LaidOut>,
    surface_sizes: Vec<SurfaceSize>,
    modal_surfaces: Option<Vec<(String, crate::layout::Rect)>>,
    interaction: Interaction,
    cursor: PhysicalPosition<f64>,
    modifiers: ModifiersState,
    scroll: f32,
    visible: bool,
    wake: Option<Instant>,
    error: Option<String>,
    /// The picture behind the views, if any.
    backdrop: Option<Box<dyn Backdrop>>,
    compositor: Option<Compositor>,
    /// Whether the views' texture holds the current views.
    painted: bool,
    foreground: paint::Retained,
    /// No backdrop frame before this: the surface skipped the last one.
    hold: Option<Instant>,
    resizing: bool,
    timings: Timings,
    captured_surface: Option<(String, crate::layout::Rect)>,
    captured_cursor: bool,
    /// The screen readers' view of the window ([`crate::access`]).
    access: Option<access::Access>,
    /// The clock every working indicator in the window shares.
    working: rust_native::motion::Working,
    /// When the working indicators next repaint, while one animates.
    working_due: Option<Instant>,
    /// Files dropped on the window through the Wayland seat
    /// ([`crate::wayland`]); winit has no drag and drop there.
    #[cfg(target_os = "linux")]
    drops: Option<crate::wayland::Drops>,
    /// Wakes the event loop from another thread.
    #[cfg(target_os = "linux")]
    wake_proxy: Option<EventLoopProxy<()>>,
    /// The system's appearance as last handed to the app.
    system_appearance: Option<Appearance>,
    /// The appearance the window last asked the system for.
    window_appearance: Option<Appearance>,
}

impl<A: App> Shell<A> {
    /// Pixels a point: the display's, times the zoom for the window's size.
    fn scale(&self) -> f32 {
        if let Some(scale) = self
            .options
            .render_scale
            .filter(|scale| scale.is_finite() && (0.5..=4.0).contains(scale))
        {
            return scale;
        }
        let Some(window) = self.window.as_ref() else {
            return 1.0;
        };
        let display = window.scale_factor() as f32;
        let zoom = match (self.options.zoom, &self.gpu) {
            (Some(zoom), Some(gpu)) => zoom.factor(
                gpu.config.width as f32 / display,
                gpu.config.height as f32 / display,
            ),
            _ => 1.0,
        };
        display * zoom
    }

    fn logical_size(&self) -> (f32, f32) {
        let scale = self.scale();
        self.gpu.as_ref().map_or((1.0, 1.0), |gpu| {
            (
                gpu.config.width as f32 / scale,
                gpu.config.height as f32 / scale,
            )
        })
    }

    /// Hands the system's appearance to the app when it changed, lays the
    /// views out again, and asks the window for the appearance the app's
    /// theme now has.
    fn follow_system_appearance(&mut self, event_loop: &ActiveEventLoop) {
        let system = event_loop.system_theme().map(|theme| match theme {
            winit::window::Theme::Light => Appearance::Light,
            winit::window::Theme::Dark => Appearance::Dark,
        });
        if system != self.system_appearance || self.window.is_none() {
            self.system_appearance = system;
            self.app.system_appearance(system);
            self.scene = None;
        }
        self.match_window_appearance();
    }

    /// Asks the window for the app theme's appearance when it changed.
    fn match_window_appearance(&mut self) {
        let wanted = self.app.theme().appearance;
        if self.window_appearance != Some(wanted)
            && let Some(window) = &self.window
        {
            window.set_theme(Some(winit_theme(wanted)));
            self.window_appearance = Some(wanted);
            self.scene = None;
        }
    }

    /// Paints the views again on the next frame.
    fn redraw(&mut self) {
        self.painted = false;
        self.request_frame();
    }

    /// Asks for a frame, with the views as they are.
    fn request_frame(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Brings the application up to now and repaints when its view moved.
    fn tick(&mut self) {
        let started = Instant::now();
        let before = self.revision();
        let (width, height) = self.logical_size();
        if let Some(window) = &self.window {
            self.app.fullscreen_changed(window.fullscreen().is_some());
        }
        self.app.viewport(width, height, self.scale());
        self.wake = self.app.tick(Instant::now());
        self.sync_capture();
        let previous_scroll = self.interaction.leading_scroll;
        if let Some(offset) = self
            .app
            .leading_scroll()
            .filter(|offset| offset.is_finite())
        {
            self.interaction.leading_scroll = offset.max(0.0);
        }
        if let Some(window) = &self.window {
            let fullscreen = window.fullscreen().is_some();
            if let Some(on) = self
                .app
                .fullscreen_request(fullscreen)
                .filter(|on| *on != fullscreen)
            {
                window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
            }
            if self.app.focus_request() {
                window.set_minimized(false);
                window.focus_window();
            }
            let cursor = self.app.ime_cursor();
            window.set_ime_allowed(cursor.is_some());
            if let Some((x, y)) = cursor {
                let zoom = f64::from(self.scale()) / window.scale_factor();
                window.set_ime_cursor_area(
                    LogicalPosition::new(x * zoom, y * zoom),
                    LogicalSize::new(2.0 * zoom, 20.0 * zoom),
                );
            }
        }
        let surface_changed = self.scene.as_ref().is_some_and(|scene| {
            scene.ops.iter().any(|op| {
                if let crate::layout::Op::Surface {
                    resource,
                    version: Some(version),
                    ..
                } = op
                {
                    self.app.surface_version(resource) != Some(*version)
                } else {
                    false
                }
            })
        });
        if self.revision() != before
            || surface_changed
            || previous_scroll != self.interaction.leading_scroll
        {
            self.redraw();
        }
        self.timings.record(Phase::Tick, started.elapsed(), 0, 0);
    }

    fn sync_capture(&mut self) {
        let capture = self.visible && self.app.cursor_capture();
        if capture == self.captured_cursor {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        if capture {
            if window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_err()
            {
                self.app.capture_failed(Instant::now());
                window.set_cursor_visible(true);
                return;
            }
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!capture);
        self.captured_cursor = capture;
    }

    fn revision(&self) -> (String, u64) {
        let view = self.app.view().view();
        (view.instance.clone(), view.revision)
    }

    /// The values used to lay out the foreground.
    fn layout_key(&self) -> LaidOut {
        let (width, height) = self.logical_size();
        LaidOut {
            instance: self.app.view().view().instance.clone(),
            revision: self.app.view().view().revision,
            size: (width.round() as u32, height.round() as u32),
            scale: (self.scale() * 100.0) as u32,
            interaction: self.interaction.clone(),
            layout: self.app.window_layout(),
            overlay: self.app.overlay_layout(),
        }
    }

    /// Handled text does not require a frame unless its presentation changed.
    /// Surfaces without revision tracking remain conservative.
    fn input_requires_redraw(&self) -> bool {
        input_requires_redraw(
            self.laid_out.as_ref(),
            &self.layout_key(),
            self.scene.as_ref(),
            |resource| self.app.surface_version(resource),
            surface_sizes_changed(&self.surface_sizes, |resource, available| {
                self.app.surface_size(resource, available)
            }),
        )
    }

    /// The current scene, laid out again when anything it depends on moved.
    fn scene(&mut self) -> &Scene {
        let (width, height) = self.logical_size();
        let key = self.layout_key();
        let resized_surface = surface_sizes_changed(&self.surface_sizes, |resource, available| {
            self.app.surface_size(resource, available)
        });
        if self.scene.is_none() || self.laid_out.as_ref() != Some(&key) || resized_surface {
            let started = Instant::now();
            // A person's theme choice changes the theme without a system
            // event: keep the title bar and controls in step.
            self.match_window_appearance();
            let theme = self.app.theme();
            let app = &self.app;
            let mut scene = lay_out_with_overlay(
                app.view().view(),
                &theme,
                &mut self.fonts,
                &|resource, available| app.surface_size(resource, available),
                &self.interaction,
                width,
                height,
                app.window_layout(),
                app.overlay_layout(),
            );
            if let Some(key) = &self.interaction.hover
                && let Some(value) = self.app.tooltip(key)
                && let Some(hit) = scene.hits.iter().find(|hit| &hit.key == key)
            {
                let value: String = value.chars().take(180).collect();
                let font =
                    crate::text::font(12.0, rust_native::layout::display::Weight::Regular, false);
                let paragraph = self.fonts.paragraph(&value, font, Some(260.0));
                let w = (paragraph.width + 16.0).min(width);
                let h = paragraph.height + 12.0;
                let x = hit.rect.x.clamp(0.0, (width - w).max(0.0));
                let y = (hit.rect.y - h - 6.0).clamp(0.0, (height - h).max(0.0));
                scene.ops.push(crate::layout::Op::Fill {
                    rect: crate::layout::Rect { x, y, w, h },
                    radius: 6.0,
                    color: theme.background,
                });
                scene.ops.push(crate::layout::Op::Text {
                    paragraph,
                    x: x + 8.0,
                    y: y + 6.0,
                    width: w - 16.0,
                    align: rust_native::style::TextAlign::Start,
                    color: theme.text,
                });
            }
            if let Some(split) = scene.split {
                self.interaction.leading_scroll = split.leading.offset;
                self.interaction.content_scroll = split.content.offset;
            }
            self.scroll = self.scroll.clamp(0.0, (scene.height - height).max(0.0));
            self.timings.record(Phase::Layout, started.elapsed(), 0, 0);
            self.surface_sizes = surface_sizes(&scene, |resource, available| {
                self.app.surface_size(resource, available)
            });
            self.modal_surfaces =
                modal_surfaces(&self.app.view().view().root, self.app.modal_root(), &scene);
            if self
                .captured_surface
                .as_ref()
                .is_some_and(|(resource, rect)| {
                    !surface_admitted(self.modal_surfaces.as_deref(), resource, *rect)
                })
            {
                self.captured_surface = None;
            }
            self.scene = Some(scene);
            self.laid_out = Some(self.layout_key());
        }
        self.scene.as_ref().expect("a scene")
    }

    /// The button under the pointer, by key.
    fn target(&mut self) -> Option<String> {
        let scale = self.scale();
        let (x, y) = (
            self.cursor.x as f32 / scale,
            self.cursor.y as f32 / scale + self.scroll,
        );
        self.scene().hit(x, y).map(|hit| hit.key.clone())
    }

    fn hover(&mut self, pointer_motion: bool) {
        let target = self.target();
        let scale = self.scale();
        let (x, y) = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
        let divider = self
            .scene()
            .split
            .and_then(|split| split.divider)
            .is_some_and(|divider| divider.contains(x, y));
        if let Some(window) = &self.window {
            window.set_cursor(if divider || self.resizing {
                CursorIcon::ColResize
            } else if target.is_some() {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            });
        }
        let changed = pointer_motion && self.app.pointer_hover(target.as_deref(), Instant::now());
        if target != self.interaction.hover || changed {
            self.interaction.hover = target;
            self.redraw();
        }
    }

    /// Resolves an activation of `node` against the current view and runs
    /// its intent.
    fn activate(&mut self, node: String) {
        let view = self.app.view();
        let event = Activation {
            instance: view.view().instance.clone(),
            revision: view.view().revision,
            node,
        };
        if let Ok(intent) = view.activate(&event) {
            let intent = intent.clone();
            self.app.activate(intent, Instant::now());
        }
        self.tick();
        self.redraw();
    }

    fn move_focus(&mut self, back: bool) {
        let order: Vec<String> = self
            .scene()
            .focus_order()
            .into_iter()
            .map(str::to_string)
            .collect();
        let order: Vec<_> = order
            .into_iter()
            .filter(|key| self.app.allows_focus(key))
            .collect();
        if order.is_empty() {
            return;
        }
        let at = self
            .interaction
            .focus
            .as_ref()
            .and_then(|focus| order.iter().position(|key| key == focus));
        let next = match (at, back) {
            (None, false) => 0,
            (None, true) => order.len() - 1,
            (Some(at), false) => (at + 1) % order.len(),
            (Some(at), true) => (at + order.len() - 1) % order.len(),
        };
        self.interaction.focus = Some(order[next].clone());
        let key = &order[next];
        if let Some(hit) = self
            .scene()
            .hits
            .iter()
            .find(|hit| &hit.key == key)
            .cloned()
            && let Some(clip) = hit.clip
        {
            let delta = if hit.rect.y < clip.y {
                hit.rect.y - clip.y
            } else {
                (hit.rect.y + hit.rect.h - clip.y - clip.h).max(0.0)
            };
            if let Some(split) = self.scene().split {
                if clip == split.leading.rect {
                    self.interaction.leading_scroll =
                        (split.leading.offset + delta).clamp(0.0, split.leading.limit);
                } else if clip == split.content.rect {
                    self.interaction.content_scroll =
                        (split.content.offset + delta).clamp(0.0, split.content.limit);
                }
            }
        }
        self.redraw();
    }

    /// Route pointer input only through an actually visible registered surface.
    fn surface(&mut self, make: impl FnOnce(f32, f32) -> SurfaceInput) -> bool {
        let started = Instant::now();
        let scale = self.scale();
        let (x, y) = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
        // A button drawn over a surface takes the pointer there (#10098).
        let target = self.scene().surface_at(x, y);
        let mut target = self
            .captured_surface
            .clone()
            .or(target)
            .filter(|(resource, rect)| {
                surface_admitted(self.modal_surfaces.as_deref(), resource, *rect)
            });
        if let Some((resource, rect)) = &mut target
            && let Some(scene) = &self.scene
            && let Some(current) = scene.ops.iter().find_map(|op| match op {
                crate::layout::Op::Surface {
                    resource: name,
                    rect,
                    ..
                } if name == resource => Some(*rect),
                _ => None,
            })
        {
            *rect = current;
        }
        let handled = if let Some((resource, rect)) = target {
            let event = make(x - rect.x, y - rect.y);
            let handled = self.app.surface_input(&resource, event, Instant::now());
            if matches!(event, SurfaceInput::Up { .. }) {
                self.captured_surface = None;
            }
            if handled && matches!(event, SurfaceInput::Down { .. }) {
                self.captured_surface = Some((resource, rect));
                self.interaction.pressed = None;
            }
            handled
        } else {
            false
        };
        if handled {
            self.timings.input(started);
            self.timings.record(Phase::Input, started.elapsed(), 0, 0);
        }
        handled
    }

    /// Answers one key. Returns false when the key asks to close.
    fn key(&mut self, key: &Key) -> bool {
        let command = self.modifiers.super_key() || self.modifiers.control_key();
        match key {
            Key::Character(text) if command => {
                if matches!(text.as_str(), "q" | "w") {
                    return false;
                }
                if let Some(binding) = self.app.key_bindings().iter().find(|binding| {
                    binding.key == text.as_str() && binding.shift == self.modifiers.shift_key()
                }) {
                    self.activate(binding.node.to_owned());
                }
            }
            Key::Named(NamedKey::Tab) => self.move_focus(self.modifiers.shift_key()),
            Key::Named(NamedKey::Enter | NamedKey::Space) => {
                if let Some(focus) = self.interaction.focus.clone() {
                    self.activate(focus);
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.interaction.focus = None;
                self.redraw();
            }
            _ => {}
        }
        true
    }

    fn set_visible(&mut self, visible: bool) {
        if visible != self.visible {
            self.visible = visible;
            let now = Instant::now();
            self.app.shown(visible, now);
            if let Some(backdrop) = &mut self.backdrop {
                backdrop.shown(visible, now);
            }
            self.tick();
        }
    }

    /// When the backdrop next wants a frame, while the window shows.
    fn backdrop_due(&mut self, now: Instant) -> Option<Instant> {
        // Asked even while hidden, so the backdrop can keep its own
        // housekeeping; a hidden window draws nothing.
        let due = self.backdrop.as_mut()?.next_frame(now)?;
        if !self.visible {
            return None;
        }
        Some(self.hold.map_or(due, |hold| hold.max(due)))
    }

    /// When the working indicators next want a frame. A deadline that has
    /// passed repaints them now. `None` when no indicator animates.
    fn working_due(&mut self, now: Instant) -> Option<Instant> {
        if self.working_due.is_some_and(|due| due <= now) {
            self.working_due = None;
            if self.visible {
                self.painted = false;
                self.request_frame();
            }
        }
        if self.working_due.is_none() {
            self.working_due = self.scene.as_ref().and_then(|scene| {
                scene.working_frame(
                    &self.working,
                    self.visible,
                    crate::theme::motion::reduced(),
                    now,
                )
            });
        }
        self.working_due
    }

    /// Drops a backdrop that failed, leaving the plain window.
    fn drop_backdrop(&mut self, error: &str) {
        eprintln!("the backdrop stopped: {error}");
        self.app.graphics_failed(error, Instant::now());
        self.sync_capture();
        self.backdrop = None;
        self.redraw();
    }

    /// Draws the backdrop and lays the views over it.
    fn render_layers(&mut self) -> Result<(), String> {
        let started = Instant::now();
        let mut timing = FrameTiming::default();
        let Some((width, height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.config.width, gpu.config.height))
        else {
            return Ok(());
        };
        let scale = self.scale();
        let surface = self
            .backdrop
            .as_ref()
            .and_then(|backdrop| backdrop.surface())
            .map(str::to_owned);
        let placed = self.backdrop.as_ref().and_then(|backdrop| backdrop.rect());
        let region = match (placed, surface) {
            (Some(rect), _) => rect,
            (None, Some(resource)) => self.scene().backdrop_rect(&resource).unwrap_or_default(),
            (None, None) => crate::Rect {
                x: 0.0,
                y: 0.0,
                w: width as f32 / scale,
                h: height as f32 / scale,
            },
        };
        if let Some(backdrop) = &mut self.backdrop {
            backdrop.viewport(region, scale);
        }
        let pixels = crate::PxRect {
            x: (region.x * scale).round().max(0.0),
            y: (region.y * scale).round().max(0.0),
            w: (region.w * scale).round().max(1.0),
            h: (region.h * scale).round().max(1.0),
        };
        let look = if let Some(backdrop) = &self.backdrop {
            backdrop.look().unwrap_or(self.options.look)
        } else {
            Look {
                dim: 1.0,
                blur: 0.0,
                scale: 0.25,
            }
        };
        let theme = self.app.theme();
        {
            let gpu = self.gpu.as_ref().expect("the gpu");
            let compositor = self.compositor.as_mut().expect("a compositor");
            if compositor.fit(&gpu.device, width, height, look, pixels) {
                self.painted = false;
                self.foreground.invalidate();
            }
        }
        let regions = if !self.painted {
            let scale = self.scale();
            let scroll = self.scroll;
            self.scene();
            let mut scene = self.scene.take().expect("a scene");
            if scene.working() {
                let reduced = crate::theme::motion::reduced();
                scene.animate_working(self.working.cells(Instant::now(), reduced));
            }
            for op in &mut scene.ops {
                if let crate::layout::Op::Surface {
                    resource, version, ..
                } = op
                {
                    *version = self.app.surface_version(resource);
                }
            }
            let app = &mut self.app;
            let painting = Instant::now();
            let regions = self.foreground.update(
                &scene,
                (width as usize, height as usize),
                scale,
                scroll,
                None,
                &mut self.fonts,
                &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
            );
            let pixels = regions.iter().map(|rect| (rect.w * rect.h) as u64).sum();
            timing.paint_us = painting.elapsed().as_micros() as u64;
            timing.damaged_pixels = pixels;
            timing.regions = regions.len();
            self.timings
                .record(Phase::Paint, painting.elapsed(), pixels, regions.len());
            self.scene = Some(scene);
            Some(regions)
        } else {
            None
        };
        // Paint on the CPU while the previous frame can finish displaying.
        // Acquire before queuing texture writes: skipped surfaces have no
        // submission to flush wgpu's pending upload allocations.
        let acquiring = Instant::now();
        let gpu = self.gpu.as_ref().expect("the gpu");
        let outcome = gpu.surface.get_current_texture();
        timing.acquire_us = acquiring.elapsed().as_micros() as u64;
        self.timings
            .record(Phase::Acquire, acquiring.elapsed(), 0, 0);
        let skipped = match &outcome {
            wgpu::CurrentSurfaceTexture::Outdated => Some(crate::timing::SkipReason::Outdated),
            wgpu::CurrentSurfaceTexture::Lost => Some(crate::timing::SkipReason::Lost),
            wgpu::CurrentSurfaceTexture::Occluded => Some(crate::timing::SkipReason::Occluded),
            wgpu::CurrentSurfaceTexture::Timeout => Some(crate::timing::SkipReason::Timeout),
            _ => None,
        };
        if let Some(reason) = skipped {
            // The CPU scene advanced, but its pixels were never submitted.
            // Retry the complete foreground even if the next scene is unchanged.
            self.foreground.invalidate();
            self.painted = false;
            let phase = match reason {
                crate::timing::SkipReason::Occluded => Phase::SurfaceOccluded,
                crate::timing::SkipReason::Timeout => Phase::SurfaceTimeout,
                crate::timing::SkipReason::Outdated => Phase::SurfaceOutdated,
                crate::timing::SkipReason::Lost => Phase::SurfaceLost,
            };
            self.timings.record(phase, acquiring.elapsed(), 0, 0);
            self.app.frame_skipped(crate::timing::FrameSkip {
                reason,
                acquire_us: timing.acquire_us,
            });
        }
        let texture = match outcome {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                self.request_frame();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {
                // Try again a little later, not in a loop.
                self.hold = Some(Instant::now() + SKIPPED_FRAME_WAIT);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("the surface failed validation".to_string());
            }
        };
        if let Some(regions) = regions {
            let uploading = Instant::now();
            let gpu = self.gpu.as_ref().expect("the gpu");
            self.compositor
                .as_ref()
                .expect("a compositor")
                .upload_regions(
                    &gpu.queue,
                    self.foreground.frame().expect("a foreground"),
                    &regions,
                );
            timing.upload_us = uploading.elapsed().as_micros() as u64;
            self.timings.record(
                Phase::Upload,
                uploading.elapsed(),
                timing.damaged_pixels,
                regions.len(),
            );
            self.painted = true;
        }
        let presenting = Instant::now();
        let gpu = self.gpu.as_ref().expect("the gpu");
        let output = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(gpu.encoded),
            ..Default::default()
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rust-native-desktop frame"),
            });
        let compositor = self.compositor.as_ref().expect("a compositor");
        let mut failed = None;
        if let (Some(backdrop), Some((target, size))) = (&mut self.backdrop, compositor.target()) {
            let context = BackdropGpu {
                adapter: &gpu.adapter,
                device: &gpu.device,
                queue: &gpu.queue,
            };
            if let Err(error) = backdrop.draw(&context, &mut encoder, target, size, Instant::now())
            {
                failed = Some(error);
            }
        }
        compositor.encode(&gpu.queue, &mut encoder, &output, theme.background, look);
        gpu.queue.submit([encoder.finish()]);
        texture.present();
        timing.present_us = presenting.elapsed().as_micros() as u64;
        self.timings
            .record(Phase::Present, presenting.elapsed(), 0, 0);
        self.timings.record(
            Phase::Frame,
            started.elapsed(),
            u64::from(width) * u64::from(height),
            0,
        );
        self.timings.presented();
        self.timings.modal_presented(
            self.app.modal_root().is_some(),
            started.elapsed(),
            timing.damaged_pixels,
            timing.regions,
        );
        timing.total_us = started.elapsed().as_micros() as u64;
        self.app.frame_presented(timing);
        self.hold = None;
        if let Some(error) = failed {
            self.drop_backdrop(&error);
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), String> {
        if self.compositor.is_some() {
            self.render_layers()
        } else {
            Ok(())
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        #[cfg(target_os = "macos")]
        if let Some(window) = &self.window
            && let Some(height) = self.app.window_layout().header_height()
        {
            position_header_controls(window, height);
        }

        if let Some(gpu) = &mut self.gpu {
            gpu.config.width = width.max(1);
            gpu.config.height = height.max(1);
            gpu.surface.configure(&gpu.device, &gpu.config);
        }
        self.set_visible(width > 0 && height > 0);
        self.redraw();
    }
}

/// A surface on `window` that frames can be copied into.
fn gpu(window: Arc<Window>) -> Result<Gpu, String> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let size = window.inner_size();
    let surface = instance
        .create_surface(window)
        .map_err(|error| format!("cannot create a surface: {error}"))?;
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: false,
        compatible_surface: Some(&surface),
    }))
    .map_err(|error| format!("no graphics adapter: {error}"))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("rust-native-desktop"),
        ..Default::default()
    }))
    .map_err(|error| format!("no graphics device: {error}"))?;
    let caps = surface.get_capabilities(&adapter);
    if !caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
        return Err("the surface does not accept copied frames".to_string());
    }
    use wgpu::TextureFormat as F;
    let format = [F::Bgra8Unorm, F::Rgba8Unorm]
        .into_iter()
        .chain([F::Bgra8UnormSrgb, F::Rgba8UnormSrgb])
        .find(|format| caps.formats.contains(format))
        .ok_or("the surface offers no 8-bit RGBA format")?;
    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            wgpu::CompositeAlphaMode::Auto
        },
        view_formats: if format.is_srgb() {
            vec![format.remove_srgb_suffix()]
        } else {
            vec![]
        },
    };
    surface.configure(&device, &config);
    Ok(Gpu {
        surface,
        adapter,
        device,
        queue,
        config,
        encoded: format.remove_srgb_suffix(),
    })
}

impl<A: App> ApplicationHandler<()> for Shell<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let access_waker = self.proxy.clone().map(|proxy| {
            Waker::new(move || {
                let _ = proxy.send_event(());
            })
        });
        if let Some(proxy) = self.proxy.take() {
            #[cfg(target_os = "linux")]
            {
                self.wake_proxy = Some(proxy.clone());
            }
            let waker = Waker::new(move || {
                let _ = proxy.send_event(());
            });
            self.app.start(waker.clone());
            if let Some(backdrop) = &mut self.backdrop {
                backdrop.start(waker);
            }
        }
        // The app resolves its scheme from the system's appearance first,
        // and the window asks for the one it chose.
        self.follow_system_appearance(event_loop);
        self.window_appearance = Some(self.app.theme().appearance);
        let mut attributes = Window::default_attributes()
            .with_title(self.app.title())
            .with_theme(Some(winit_theme(self.app.theme().appearance)))
            .with_inner_size(LogicalSize::new(self.options.size.0, self.options.size.1))
            // Shown once the accessibility adapter is attached.
            .with_visible(false)
            .with_min_inner_size(LogicalSize::new(
                self.options.min_size.0,
                self.options.min_size.1,
            ));
        if self.options.fixture_frontmost {
            attributes = attributes.with_window_level(winit::window::WindowLevel::AlwaysOnTop);
        }
        #[cfg(target_os = "macos")]
        if self.app.window_layout().header_height().is_some() {
            use winit::platform::macos::WindowAttributesExtMacOS;
            attributes = attributes
                .with_titlebar_transparent(true)
                .with_title_hidden(true)
                .with_fullsize_content_view(true);
        }
        if let Some((width, height)) = self.options.pixel_size {
            attributes = attributes.with_inner_size(winit::dpi::PhysicalSize::new(width, height));
        }
        let area = self.options.fill.and_then(|_| usable_area(event_loop));
        if let (Some(fill), Some(area)) = (self.options.fill, area) {
            let ((x, y), (width, height)) = placement(area, fill, TITLE_BAR, self.options.min_size);
            attributes = attributes
                .with_inner_size(LogicalSize::new(width, height))
                .with_position(LogicalPosition::new(x, y));
        }
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.error = Some(format!("cannot open a window: {error}"));
                event_loop.exit();
                return;
            }
        };
        self.access = Some(access::Access::new(
            event_loop,
            &window,
            access_waker.unwrap_or_else(Waker::none),
        ));
        window.set_visible(true);
        #[cfg(target_os = "macos")]
        if let Some(height) = self.app.window_layout().header_height() {
            position_header_controls(&window, height);
        }
        // The title bar's real height is known only now, and a position
        // given at creation places the inner area on some systems, so the
        // window is centered again by its outer size.
        if let Some(area) = area {
            let outer = window.outer_size().to_logical::<f64>(window.scale_factor());
            let (x, y) = centered(area, (outer.width, outer.height));
            window.set_outer_position(LogicalPosition::new(x, y));
        }
        match gpu(window.clone()) {
            Ok(gpu) => {
                self.compositor = Some(Compositor::new(&gpu.device, gpu.encoded));
                self.gpu = Some(gpu);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
                return;
            }
        }
        #[cfg(target_os = "linux")]
        if let Some(proxy) = self.wake_proxy.take() {
            self.drops = crate::wayland::attach(&window, move || {
                let _ = proxy.send_event(());
            });
        }
        self.window = Some(window.clone());
        self.app.shown(true, Instant::now());
        if let Some(backdrop) = &mut self.backdrop {
            backdrop.shown(true, Instant::now());
        }
        self.tick();
        window.focus_window();
        window.request_redraw();
    }

    fn user_event(&mut self, _: &ActiveEventLoop, (): ()) {
        #[cfg(target_os = "linux")]
        {
            let paths = self
                .drops
                .as_ref()
                .map(crate::wayland::Drops::take)
                .unwrap_or_default();
            let mut dropped = false;
            for path in paths {
                dropped |= self.app.dropped_file(path, Instant::now());
            }
            if dropped {
                self.redraw();
            }
        }
        self.tick();
        if self.backdrop_due(Instant::now()).is_some() {
            self.request_frame();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.exit_requested() {
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if self.wake.is_some_and(|wake| wake <= now) {
            self.tick();
        }
        if self.app.exit_requested() {
            event_loop.exit();
            return;
        }
        self.sync_access();
        let frame = self.backdrop_due(now);
        if frame.is_some_and(|frame| frame <= now) {
            self.request_frame();
        }
        let working = self.working_due(now);
        let wake = rust_native::motion::earliest(
            rust_native::motion::earliest(self.wake, frame.filter(|frame| *frame > now)),
            working,
        );
        event_loop.set_control_flow(match wake {
            Some(wake) => ControlFlow::WaitUntil(wake),
            None => ControlFlow::Wait,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let (Some(access), Some(window)) = (&mut self.access, &self.window) {
            access.process_event(window, &event);
        }
        let scale = self.scale();
        let x = self.cursor.x as f32 / scale;
        let y = self.cursor.y as f32 / scale;
        let key;
        let native = match &event {
            WindowEvent::KeyboardInput { event, .. } => {
                key = match event.physical_key {
                    winit::keyboard::PhysicalKey::Code(code) => format!("{code:?}"),
                    _ => String::new(),
                };
                Some(NativeInput::Key {
                    code: &key,
                    pressed: event.state == ElementState::Pressed,
                    repeat: event.repeat,
                    command: self.modifiers.super_key() || self.modifiers.control_key(),
                    alt: self.modifiers.alt_key(),
                    control: self.modifiers.control_key(),
                    logo: self.modifiers.super_key(),
                })
            }
            WindowEvent::MouseInput { state, button, .. } => Some(NativeInput::Button {
                button: match button {
                    MouseButton::Left => 0,
                    MouseButton::Right => 1,
                    MouseButton::Middle => 2,
                    MouseButton::Back => 3,
                    MouseButton::Forward => 4,
                    MouseButton::Other(n) => n.saturating_add(5),
                },
                pressed: *state == ElementState::Pressed,
                x,
                y,
            }),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = *position;
                Some(NativeInput::Cursor {
                    x: position.x as f32 / scale,
                    y: position.y as f32 / scale,
                })
            }
            WindowEvent::MouseWheel { delta, .. } => Some(NativeInput::Wheel {
                x,
                y,
                lines: match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                },
            }),
            WindowEvent::Focused(on) => Some(NativeInput::Focus(*on)),
            WindowEvent::CloseRequested
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => Some(NativeInput::Cancel),
            _ => None,
        };
        let over_control = matches!(native, Some(NativeInput::Button { pressed: true, .. }))
            && self.scene().hit(x, y).is_some();
        if over_control {
            self.app.native_input(NativeInput::Cancel, Instant::now());
        }
        if !over_control && native.is_some_and(|input| self.app.native_input(input, Instant::now()))
        {
            self.app.input(Instant::now());
            self.tick();
            self.request_frame();
            return;
        }
        self.sync_capture();
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::DroppedFile(path) => {
                if self.app.dropped_file(path, Instant::now()) {
                    self.redraw();
                }
            }
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::Occluded(occluded) => {
                self.set_visible(!occluded);
                self.redraw();
            }
            WindowEvent::ThemeChanged(_) => {
                self.follow_system_appearance(event_loop);
                self.redraw();
            }
            WindowEvent::Focused(true) => {
                // A system appearance change while another app was in front:
                // on a Mac, a window with its own appearance hears no
                // `ThemeChanged`, so coming to the front reads it again.
                self.follow_system_appearance(event_loop);
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.redraw(),
            WindowEvent::Focused(false) => {
                self.app.text_input(TextInput::FocusLost, Instant::now());
                self.resizing = false;
                self.captured_surface = None;
                self.interaction.pressed = None;
                self.modifiers = ModifiersState::empty();
                self.redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.app.input(Instant::now());
                    let key = match &event.logical_key {
                        Key::Character(value) => value.to_string(),
                        Key::Named(value) => format!("{value:?}"),
                        _ => String::new(),
                    };
                    let input_started = Instant::now();
                    let consumed = self.app.text_input(
                        TextInput::Key {
                            key: &key,
                            text: event.text.as_deref(),
                            control: self.modifiers.control_key(),
                            command: self.modifiers.super_key() || self.modifiers.control_key(),
                            alt: self.modifiers.alt_key(),
                            shift: self.modifiers.shift_key(),
                        },
                        Instant::now(),
                    );
                    self.timings
                        .record(Phase::Input, input_started.elapsed(), 0, 0);
                    if !consumed && !self.key(&event.logical_key) {
                        event_loop.exit();
                        return;
                    }
                    self.tick();
                    if !self.painted || self.input_requires_redraw() {
                        self.timings.input(input_started);
                        if consumed {
                            self.redraw();
                        }
                    }
                }
            }
            WindowEvent::Ime(event) => {
                let input_started = Instant::now();
                match event {
                    winit::event::Ime::Preedit(text, selection) => {
                        self.app.text_input(
                            TextInput::Preedit {
                                text: &text,
                                selection,
                            },
                            Instant::now(),
                        );
                    }
                    winit::event::Ime::Commit(text) => {
                        self.app
                            .text_input(TextInput::Commit(&text), Instant::now());
                    }
                    winit::event::Ime::Disabled => {
                        self.app
                            .text_input(TextInput::CancelComposition, Instant::now());
                    }
                    _ => {}
                }
                self.timings
                    .record(Phase::Input, input_started.elapsed(), 0, 0);
                self.tick();
                if self.input_requires_redraw() {
                    self.timings.input(input_started);
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.app.input(Instant::now());
                self.cursor = position;
                if self.surface(|x, y| SurfaceInput::Move { x, y }) {
                    self.tick();
                    self.redraw();
                    return;
                }
                if self.resizing
                    && let Some(split) = self.app.window_layout().split()
                {
                    let (width, _) = self.logical_size();
                    let requested = (position.x as f32 / self.scale())
                        .clamp(split.min_leading_width, split.max_leading_width);
                    self.app.resize_leading_pane(
                        requested.min((width - split.min_content_width).max(1.0)),
                        Instant::now(),
                    );
                    self.tick();
                    self.redraw();
                }
                self.hover(true);
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = PhysicalPosition::new(-1.0, -1.0);
                self.hover(true);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.app.input(Instant::now());
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / self.scale(),
                };
                let scale = self.scale();
                // With Cmd or Ctrl held, a surface that zooms reads the
                // wheel as zoom: a notch (40 points) is a quarter again.
                let zoom = self.modifiers.super_key() || self.modifiers.control_key();
                if self.surface(|x, y| {
                    if zoom {
                        SurfaceInput::Zoom {
                            x,
                            y,
                            factor: 1.25_f32.powf(lines / 40.0),
                        }
                    } else {
                        SurfaceInput::Wheel {
                            x,
                            y,
                            dx: match delta {
                                MouseScrollDelta::LineDelta(x, _) => x * 40.0,
                                MouseScrollDelta::PixelDelta(p) => p.x as f32 / scale,
                            },
                            dy: lines,
                        }
                    }
                }) {
                    self.tick();
                    self.redraw();
                    return;
                }
                if let Some(split) = self.scene().split {
                    let scale = self.scale();
                    let (x, y) = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
                    if split.leading.rect.contains(x, y) {
                        self.interaction.leading_scroll =
                            (split.leading.offset - lines).clamp(0.0, split.leading.limit);
                    } else if split.content.rect.contains(x, y) {
                        self.interaction.content_scroll =
                            (split.content.offset - lines).clamp(0.0, split.content.limit);
                    }
                } else {
                    let (_, height) = self.logical_size();
                    let limit = (self.scene().height - height).max(0.0);
                    self.scroll = (self.scroll - lines).clamp(0.0, limit);
                }
                self.hover(false);
                self.redraw();
            }
            WindowEvent::PinchGesture { delta, .. } => {
                self.app.input(Instant::now());
                let factor = (1.0 + delta as f32).clamp(0.5, 2.0);
                if self.surface(|x, y| SurfaceInput::Zoom { x, y, factor }) {
                    self.tick();
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                let target = self.target();
                let scale = self.scale();
                let point = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
                if self
                    .app
                    .context_menu_at(target.as_deref(), point, Instant::now())
                {
                    self.tick();
                    self.redraw();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.app.input(Instant::now());
                if state == ElementState::Pressed {
                    let target = self.target();
                    let scale = self.scale();
                    let point = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
                    let modal = self.app.modal_root().map(str::to_owned);
                    let inside_modal = modal
                        .as_ref()
                        .and_then(|key| self.scene().bounds.get(key))
                        .is_some_and(|rect| rect.contains(point.0, point.1));
                    if !inside_modal
                        && self.app.pointer_down(
                            target.as_deref(),
                            (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale),
                            Instant::now(),
                        )
                    {
                        self.interaction.pressed = None;
                        self.captured_surface = None;
                        self.tick();
                        self.redraw();
                        return;
                    }
                }
                if state == ElementState::Pressed
                    && self.target().is_none()
                    && self.app.modal_root().is_none()
                    && self
                        .app
                        .window_layout()
                        .header_height()
                        .is_some_and(|height| self.cursor.y as f32 / self.scale() < height)
                {
                    if let Some(window) = &self.window {
                        let _ = window.drag_window();
                    }
                    return;
                }
                let shift = self.modifiers.shift_key();
                if self.surface(|x, y| match state {
                    ElementState::Pressed => SurfaceInput::Down { x, y, shift },
                    ElementState::Released => SurfaceInput::Up { x, y },
                }) {
                    self.tick();
                    self.redraw();
                    return;
                }
                let target = self.target();
                match state {
                    ElementState::Pressed => {
                        let scale = self.scale();
                        let (x, y) = (self.cursor.x as f32 / scale, self.cursor.y as f32 / scale);
                        self.resizing = self
                            .scene()
                            .split
                            .and_then(|split| split.divider)
                            .is_some_and(|rect| rect.contains(x, y));
                        self.interaction.pressed = if self.resizing { None } else { target };
                        self.interaction.focus = None;
                    }
                    ElementState::Released => {
                        self.resizing = false;
                        if let Some(pressed) = self.interaction.pressed.take()
                            && target.as_deref() == Some(pressed.as_str())
                        {
                            self.activate(pressed);
                        }
                    }
                }
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = &self.window
                    && window.is_minimized() == Some(true)
                {
                    self.set_visible(false);
                }
                if let Err(error) = self.render() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.captured_cursor
            && let DeviceEvent::MouseMotion { delta: (dx, dy) } = event
        {
            let scale = self
                .window
                .as_ref()
                .map_or(1.0, |window| window.scale_factor()) as f32;
            if self.app.native_input(
                NativeInput::Motion {
                    dx: dx as f32 / scale,
                    dy: dy as f32 / scale,
                },
                Instant::now(),
            ) {
                self.sync_capture();
                self.request_frame();
            }
        }
    }
}

/// The window appearance winit asks the system for.
fn winit_theme(appearance: Appearance) -> winit::window::Theme {
    match appearance {
        Appearance::Light => winit::window::Theme::Light,
        Appearance::Dark => winit::window::Theme::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Area, LaidOut, Zoom, input_requires_redraw, modal_surfaces, placement, surface_admitted,
        surface_sizes, surface_sizes_changed,
    };

    #[test]
    fn modal_surfaces_block_the_reader_and_keep_the_popup_editor() {
        use crate::layout::{Rect, Scene};
        use rust_native::{Axis, Element, Node, style::Style};
        let surface = |key: &str, resource: &str| Node::<()> {
            key: key.into(),
            style: Style::default(),
            element: Element::Surface {
                resource: resource.into(),
                label: key.into(),
            },
        };
        let popup = Node {
            key: "popup".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![surface("search", "editor")],
            },
        };
        let root = Node {
            key: "root".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    surface("reader", "reader"),
                    surface("draft", "editor"),
                    popup,
                ],
            },
        };
        let editor = Rect {
            x: 100.0,
            y: 100.0,
            w: 200.0,
            h: 28.0,
        };
        let reader = Rect {
            x: 0.0,
            y: 0.0,
            w: 760.0,
            h: 540.0,
        };
        let mut scene = Scene::default();
        scene.bounds.insert("search".into(), editor);
        let modal = modal_surfaces(&root, Some("popup"), &scene).unwrap();
        assert!(!surface_admitted(Some(&modal), "reader", reader));
        assert!(!surface_admitted(Some(&modal), "editor", reader));
        assert!(surface_admitted(Some(&modal), "editor", editor));
        assert!(surface_admitted(None, "reader", reader));
        assert!(modal_surfaces(&root, None, &scene).is_none());
        let missing = modal_surfaces(&root, Some("missing"), &scene).unwrap();
        assert!(!surface_admitted(Some(&missing), "reader", reader));
    }

    #[test]
    fn unchanged_input_skips_frames_but_editor_and_modal_changes_do_not() {
        use crate::layout::{
            Interaction, Op, OverlayLayout, OverlayPlacement, Rect, Scene, WindowLayout,
        };
        let cached = LaidOut {
            instance: "input-test".into(),
            revision: 1,
            size: (760, 540),
            scale: 200,
            interaction: Interaction::default(),
            layout: WindowLayout::Column,
            overlay: None,
        };
        let mut scene = Scene {
            ops: vec![Op::Surface {
                resource: "editor".into(),
                rect: Rect::default(),
                version: Some(7),
            }],
            ..Scene::default()
        };
        let needs = |current: &LaidOut, scene: &Scene, version| {
            input_requires_redraw(Some(&cached), current, Some(scene), |_| version, false)
        };
        assert!(
            !needs(&cached, &scene, Some(7)),
            "consumed input with identical presentation needs no frame"
        );
        assert!(
            needs(&cached, &scene, Some(8)),
            "editing or selection changes require a frame even with the same semantic view"
        );
        assert!(
            needs(&cached, &scene, None),
            "losing revision tracking must remain conservative"
        );
        assert!(
            input_requires_redraw(Some(&cached), &cached, Some(&scene), |_| Some(7), true),
            "surface geometry changes require a frame even with the same drawing revision"
        );
        let mut current = cached.clone();
        current.overlay = Some(OverlayLayout {
            width: 216,
            placement: OverlayPlacement::Center,
            scrim: None,
        });
        assert!(
            needs(&current, &scene, Some(7)),
            "opening a menu requires a frame"
        );
        current = cached.clone();
        current.interaction.focus = Some("button".into());
        assert!(
            needs(&current, &scene, Some(7)),
            "keyboard focus requires a frame"
        );
        current = cached.clone();
        current.revision += 1;
        assert!(
            needs(&current, &scene, Some(7)),
            "semantic changes require a frame"
        );
        if let Op::Surface { version, .. } = &mut scene.ops[0] {
            *version = None;
        }
        assert!(
            needs(&cached, &scene, None),
            "untracked custom painters must still refresh"
        );
        assert!(input_requires_redraw(
            None,
            &cached,
            Some(&scene),
            |_| Some(7),
            false
        ));
        assert!(input_requires_redraw(
            Some(&cached),
            &cached,
            None,
            |_| Some(7),
            false
        ));
    }

    #[test]
    fn filling_surface_does_not_invalidate_its_intrinsic_size() {
        use crate::layout::{Op, Rect, Scene};
        let scene = Scene {
            ops: vec![Op::Surface {
                resource: "transcript".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 500.0,
                    h: 700.0,
                },
                version: Some(1),
            }],
            ..Scene::default()
        };
        let cached = surface_sizes(&scene, |_, w| Some((w, 400.0)));
        assert!(!surface_sizes_changed(&cached, |_, w| Some((w, 400.0))));
        assert!(surface_sizes_changed(&cached, |_, w| Some((w, 420.0))));
        assert!(surface_sizes_changed(&cached, |_, w| Some((
            w - 10.0,
            400.0
        ))));
        assert!(surface_sizes_changed(&cached, |_, _| None));
    }

    #[test]
    fn a_filling_window_is_centered_on_the_usable_area() {
        // A 14-inch MacBook Pro's visible frame: below the menu bar, above
        // a Dock at the bottom.
        let area = Area {
            x: 0.0,
            y: 38.0,
            width: 1512.0,
            height: 900.0,
        };
        let ((x, y), (width, height)) = placement(area, 0.9, 28.0, (420.0, 520.0));
        assert_eq!((width, height + 28.0), (1361.0, 810.0));
        assert_eq!((x, y), (76.0, 83.0));
        // A second display to the left keeps its own origin.
        let left = Area {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        let ((x, _), (width, _)) = placement(left, 0.9, 32.0, (420.0, 520.0));
        assert_eq!(x, -1920.0 + 96.0);
        assert_eq!(width, 1728.0);
    }

    #[test]
    fn a_small_display_still_gets_the_minimum_size() {
        let area = Area {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 500.0,
        };
        let (_, size) = placement(area, 0.9, 28.0, (420.0, 520.0));
        assert_eq!(size, (420.0, 520.0));
    }

    #[test]
    fn the_views_grow_with_the_window_up_to_the_limit() {
        let zoom = Zoom {
            design: (560.0, 720.0),
            max: 1.6,
        };
        assert_eq!(zoom.factor(560.0, 720.0), 1.0);
        assert_eq!(zoom.factor(420.0, 520.0), 1.0);
        // The shorter side decides.
        assert!((zoom.factor(1361.0, 782.0) - 782.0 / 720.0).abs() < 1e-4);
        assert_eq!(zoom.factor(3000.0, 2000.0), 1.6);
    }
}
