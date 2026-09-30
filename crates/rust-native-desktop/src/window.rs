//! A native window for an [`App`].
//!
//! One `winit` window over a `wgpu` surface. Each frame is laid out and
//! painted in software, then copied into the surface texture; there is no
//! shader. A frame is painted only when the view, the window size, or the
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
//! Input becomes a revision-bound [`Activation`] naming the view's instance,
//! revision, and the button's key; the adapter resolves it against the
//! current view with [`rust_native::ValidatedView::activate`] and hands the
//! application only the intent that view carried.

use crate::backdrop::{Backdrop, Compositor, Gpu as BackdropGpu, Look};
use crate::canvas::Frame;
use crate::layout::{Interaction, Scene, lay_out_window};
use crate::text::Fonts;
use crate::{App, Waker, paint};
use rust_native::Activation;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId};

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
}

impl Default for Options {
    fn default() -> Options {
        Options {
            size: (560.0, 720.0),
            min_size: (420.0, 520.0),
            fill: None,
            zoom: None,
            look: Look::default(),
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
        hold: None,
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
    bgra: bool,
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
    /// No backdrop frame before this: the surface skipped the last one.
    hold: Option<Instant>,
}

impl<A: App> Shell<A> {
    /// Pixels a point: the display's, times the zoom for the window's size.
    fn scale(&self) -> f32 {
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
        let before = self.revision();
        self.wake = self.app.tick(Instant::now());
        if self.revision() != before {
            self.redraw();
        }
    }

    fn revision(&self) -> (String, u64) {
        let view = self.app.view().view();
        (view.instance.clone(), view.revision)
    }

    /// The current scene, laid out again when anything it depends on moved.
    fn scene(&mut self) -> &Scene {
        let (width, height) = self.logical_size();
        let key = LaidOut {
            instance: self.app.view().view().instance.clone(),
            revision: self.app.view().view().revision,
            size: (width.round() as u32, height.round() as u32),
            scale: (self.scale() * 100.0) as u32,
            interaction: self.interaction.clone(),
        };
        if self.scene.is_none() || self.laid_out.as_ref() != Some(&key) {
            let theme = self.app.theme();
            let app = &self.app;
            let scene = lay_out_window(
                app.view().view(),
                &theme,
                &mut self.fonts,
                &|resource, available| app.surface_size(resource, available),
                &self.interaction,
                width,
                height,
            );
            self.scroll = self.scroll.clamp(0.0, (scene.height - height).max(0.0));
            self.scene = Some(scene);
            self.laid_out = Some(key);
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

    fn hover(&mut self) {
        let target = self.target();
        if target != self.interaction.hover {
            if let Some(window) = &self.window {
                window.set_cursor(if target.is_some() {
                    CursorIcon::Pointer
                } else {
                    CursorIcon::Default
                });
            }
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
        self.redraw();
    }

    /// Answers one key. Returns false when the key asks to close.
    fn key(&mut self, key: &Key) -> bool {
        let command = self.modifiers.super_key() || self.modifiers.control_key();
        match key {
            Key::Character(text) if command => {
                if matches!(text.as_str(), "q" | "w") {
                    return false;
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

    /// Drops a backdrop that failed, leaving the plain window.
    fn drop_backdrop(&mut self, error: &str) {
        eprintln!("the backdrop stopped: {error}");
        self.backdrop = None;
        self.compositor = None;
        self.redraw();
    }

    /// Draws the backdrop and lays the views over it.
    fn render_layers(&mut self) -> Result<(), String> {
        let Some((width, height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.config.width, gpu.config.height))
        else {
            return Ok(());
        };
        let look = self.options.look;
        let theme = self.app.theme();
        {
            let gpu = self.gpu.as_ref().expect("the gpu");
            let compositor = self.compositor.as_mut().expect("a compositor");
            if compositor.fit(&gpu.device, width, height, look) {
                self.painted = false;
            }
        }
        if !self.painted {
            let scale = self.scale();
            let scroll = self.scroll;
            self.scene();
            let scene = self.scene.take().expect("a scene");
            let mut frame = Frame::transparent(width as usize, height as usize);
            let app = &mut self.app;
            paint::paint(
                &scene,
                &mut frame,
                scale,
                scroll,
                &mut self.fonts,
                &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
            );
            self.scene = Some(scene);
            let gpu = self.gpu.as_ref().expect("the gpu");
            self.compositor
                .as_ref()
                .expect("a compositor")
                .upload(&gpu.queue, &frame);
            self.painted = true;
        }
        let gpu = self.gpu.as_ref().expect("the gpu");
        let texture = match gpu.surface.get_current_texture() {
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
        self.hold = None;
        if let Some(error) = failed {
            self.drop_backdrop(&error);
        }
        Ok(())
    }

    fn render(&mut self) -> Result<(), String> {
        if self.compositor.is_some() {
            return self.render_layers();
        }
        let Some((width, height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.config.width as usize, gpu.config.height as usize))
        else {
            return Ok(());
        };
        let scale = self.scale();
        let theme = self.app.theme();
        let scroll = self.scroll;
        self.scene();
        let scene = self.scene.take().expect("a scene");
        let mut frame = Frame::new(width, height, theme.background);
        let app = &mut self.app;
        paint::paint(
            &scene,
            &mut frame,
            scale,
            scroll,
            &mut self.fonts,
            &mut |resource, frame, rect| app.paint_surface(resource, frame, rect),
        );
        self.scene = Some(scene);
        let gpu = self.gpu.as_mut().expect("the gpu");
        if gpu.bgra {
            for pixel in frame.pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        let texture = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                gpu.surface.configure(&gpu.device, &gpu.config);
                self.redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.redraw();
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("the surface failed validation".to_string());
            }
        };
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width as u32 * 4),
                rows_per_image: Some(height as u32),
            },
            wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([]);
        texture.present();
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
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
        bgra: matches!(format, F::Bgra8Unorm | F::Bgra8UnormSrgb),
        encoded: format.remove_srgb_suffix(),
    })
}

impl<A: App> ApplicationHandler<()> for Shell<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Some(proxy) = self.proxy.take() {
            let waker = Waker::new(move || {
                let _ = proxy.send_event(());
            });
            self.app.start(waker.clone());
            if let Some(backdrop) = &mut self.backdrop {
                backdrop.start(waker);
            }
        }
        let mut attributes = Window::default_attributes()
            .with_title(self.app.title())
            .with_inner_size(LogicalSize::new(self.options.size.0, self.options.size.1))
            .with_min_inner_size(LogicalSize::new(
                self.options.min_size.0,
                self.options.min_size.1,
            ));
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
                if self.backdrop.is_some() {
                    self.compositor = Some(Compositor::new(&gpu.device, gpu.encoded));
                }
                self.gpu = Some(gpu);
            }
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
                return;
            }
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
        let frame = self.backdrop_due(now);
        if frame.is_some_and(|frame| frame <= now) {
            self.request_frame();
        }
        let wake = match (self.wake, frame.filter(|frame| *frame > now)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        event_loop.set_control_flow(match wake {
            Some(wake) => ControlFlow::WaitUntil(wake),
            None => ControlFlow::Wait,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::Occluded(occluded) => {
                self.set_visible(!occluded);
                self.redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Focused(true) => self.redraw(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.app.input(Instant::now());
                    if !self.key(&event.logical_key) {
                        event_loop.exit();
                        return;
                    }
                    self.tick();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.app.input(Instant::now());
                self.cursor = position;
                self.hover();
            }
            WindowEvent::CursorLeft { .. } => {
                self.cursor = PhysicalPosition::new(-1.0, -1.0);
                self.hover();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.app.input(Instant::now());
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(position) => position.y as f32 / self.scale(),
                };
                let (_, height) = self.logical_size();
                let limit = (self.scene().height - height).max(0.0);
                self.scroll = (self.scroll - lines).clamp(0.0, limit);
                self.hover();
                self.redraw();
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.app.input(Instant::now());
                let target = self.target();
                match state {
                    ElementState::Pressed => {
                        self.interaction.pressed = target;
                        self.interaction.focus = None;
                    }
                    ElementState::Released => {
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
}

#[cfg(test)]
mod tests {
    use super::{Area, Zoom, placement};

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
