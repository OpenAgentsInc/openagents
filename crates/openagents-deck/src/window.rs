//! The deck in a native window.
//!
//! One `winit` window over a `wgpu` surface. Each frame is painted in
//! software by [`Painter`], the same painter `--capture` writes PNG files
//! with, and copied into the surface texture; there is no shader. A slide
//! appears whole the moment it is opened, with no animation, and a frame
//! is painted only when something changed: a key, a click, or a resize.
//! Idle, the event loop waits and paints nothing.

use crate::Options;
use openagents_deck::paint::{FIELD, Frame, Painter, fitting_size};
use openagents_deck::{Canvas, Deck, Grid, notes_grid, overview, overview_press, slide_grid};
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

/// The most rows of presenter's note the notes band shows.
const NOTE_ROWS: usize = 6;

/// Opens the window on `deck` and runs until it closes.
pub fn run(deck: Deck, options: &Options) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|error| format!("no event loop: {error}"))?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut show = Show {
        index: options.slide.min(deck.len().saturating_sub(1)),
        deck,
        notes: options.notes,
        fullscreen: options.fullscreen,
        overview: false,
        black: false,
        zoom: 1.0,
        typed: String::new(),
        modifiers: ModifiersState::empty(),
        cursor: PhysicalPosition::new(0.0, 0.0),
        placement: None,
        window: None,
        gpu: None,
        painter: None,
        error: None,
    };
    event_loop
        .run_app(&mut show)
        .map_err(|error| format!("the event loop failed: {error}"))?;
    match show.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Where the canvas sits in the window, in physical pixels.
#[derive(Clone, Copy, Debug)]
struct Placement {
    x: f32,
    y: f32,
    cell_width: f32,
    cell_height: f32,
}

/// The surface and the device that copies frames into it.
struct Gpu {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// Whether the surface wants blue first.
    bgra: bool,
}

/// The presentation's state.
struct Show {
    deck: Deck,
    index: usize,
    notes: bool,
    fullscreen: bool,
    overview: bool,
    black: bool,
    zoom: f32,
    /// Digits typed toward a jump, waiting for enter.
    typed: String,
    modifiers: ModifiersState,
    cursor: PhysicalPosition<f64>,
    placement: Option<Placement>,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    painter: Option<Painter>,
    error: Option<String>,
}

impl Show {
    /// Opens slide `index`, clamped to the deck.
    fn go(&mut self, index: usize) {
        let last = self.deck.len().saturating_sub(1);
        self.index = index.min(last);
        self.overview = false;
        self.black = false;
    }

    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
        if let Some(window) = &self.window {
            window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
            window.set_cursor_visible(!on);
        }
    }

    /// Answers one key. Returns false when the key asks to quit.
    fn key(&mut self, key: &Key) -> bool {
        let command = self.modifiers.super_key() || self.modifiers.control_key();
        match key {
            Key::Character(text) if command => match text.as_str() {
                "=" | "+" => self.zoom = (self.zoom * 1.1).min(4.0),
                "-" => self.zoom = (self.zoom / 1.1).max(0.25),
                "0" => self.zoom = 1.0,
                "q" | "w" => return false,
                _ => {}
            },
            Key::Named(NamedKey::ArrowRight | NamedKey::Space | NamedKey::PageDown) => {
                self.go(self.index + 1)
            }
            Key::Named(NamedKey::ArrowLeft | NamedKey::PageUp) => {
                self.go(self.index.saturating_sub(1))
            }
            Key::Named(NamedKey::ArrowDown) if self.overview => self.go(self.index + 3),
            Key::Named(NamedKey::ArrowUp) if self.overview => self.go(self.index.saturating_sub(3)),
            Key::Named(NamedKey::Home) => self.go(0),
            Key::Named(NamedKey::End) => self.go(self.deck.len()),
            Key::Named(NamedKey::Enter) => {
                if let Ok(number) = self.typed.parse::<usize>() {
                    self.go(number.saturating_sub(1));
                } else if self.overview {
                    self.overview = false;
                }
                self.typed.clear();
            }
            Key::Named(NamedKey::Escape) => {
                if self.overview || self.notes || self.black || !self.typed.is_empty() {
                    self.overview = false;
                    self.notes = false;
                    self.black = false;
                    self.typed.clear();
                } else if self.fullscreen {
                    self.set_fullscreen(false);
                }
            }
            Key::Character(text) => match text.as_str() {
                "n" | "j" => self.go(self.index + 1),
                "p" | "k" => self.go(self.index.saturating_sub(1)),
                "o" => self.overview = !self.overview,
                "t" => self.notes = !self.notes,
                "." | "b" => self.black = !self.black,
                "f" => self.set_fullscreen(!self.fullscreen),
                "q" => return false,
                digit if digit.chars().all(|c| c.is_ascii_digit()) => self.typed.push_str(digit),
                _ => {}
            },
            _ => {}
        }
        true
    }

    /// A click: on the overview, open the card under the pointer; on a
    /// slide, go on.
    fn click(&mut self) {
        if !self.overview {
            self.go(self.index + 1);
            return;
        }
        let Some(placement) = self.placement else {
            return;
        };
        let col = (self.cursor.x as f32 - placement.x) / placement.cell_width;
        let row = (self.cursor.y as f32 - placement.y) / placement.cell_height;
        if col < 0.0 || row < 0.0 {
            return;
        }
        let grid = overview(&self.deck, self.index, Canvas::DEFAULT);
        if let Some(index) = overview_press(&grid, col as usize, row as usize) {
            self.go(index);
        }
    }

    /// Paints the current state into a frame `width` by `height` pixels.
    fn frame(&mut self, width: usize, height: usize) -> Frame {
        let mut frame = Frame::new(width, height, FIELD);
        if self.black {
            return frame;
        }
        let canvas = Canvas::DEFAULT;
        let notes = if self.notes && !self.overview {
            let mut grid = notes_grid(&self.deck, self.index, canvas.body_cells());
            grid.truncate(NOTE_ROWS);
            Some(grid)
        } else {
            None
        };
        let rows = canvas.rows
            + notes
                .as_ref()
                .map_or(0, |grid| NOTE_ROWS.max(grid.height()) + 2);
        let size = fitting_size(width as f32, height as f32, canvas.cells, rows) * self.zoom;
        let grid: Grid = if self.overview {
            overview(&self.deck, self.index, canvas)
        } else {
            slide_grid(&self.deck, self.index, canvas)
        };
        let painter = match &mut self.painter {
            Some(painter) if (painter.size() - size).abs() < 0.01 => painter,
            slot => slot.insert(Painter::new(size)),
        };
        let (w, h) = painter.extent(canvas.cells, rows);
        let x = ((width as f32 - w) / 2.0).round();
        let y = ((height as f32 - h) / 2.0).round();
        painter.paint(&mut frame, &grid, x, y);
        if let Some(notes) = notes {
            let top = y + (canvas.rows + 1) as f32 * painter.cell_height();
            let rule_y = top - painter.cell_height() * 0.5;
            let weight = (painter.size() / 16.0).round().max(1.0) as i64;
            frame.fill(
                x as i64,
                rule_y as i64,
                (x + w) as i64,
                rule_y as i64 + weight,
                openagents_deck::palette::color(coder_ui::theme::Intensity::Quarter),
            );
            let left = x + openagents_deck::canvas::PAD_COLS as f32 * painter.cell_width();
            painter.paint(&mut frame, &notes, left, top);
        }
        if !self.typed.is_empty() {
            let mut jump = Grid::new(12, 1);
            jump.put_str(
                0,
                0,
                &format!("go to {}", self.typed),
                openagents_deck::grid::Style::at(coder_ui::theme::Intensity::Half),
            );
            painter.paint(&mut frame, &jump, x + painter.cell_width(), y);
        }
        self.placement = Some(Placement {
            x,
            y,
            cell_width: painter.cell_width(),
            cell_height: painter.cell_height(),
        });
        frame
    }

    /// Paints and presents one frame.
    fn render(&mut self) -> Result<(), String> {
        let Some((width, height)) = self
            .gpu
            .as_ref()
            .map(|gpu| (gpu.config.width as usize, gpu.config.height as usize))
        else {
            return Ok(());
        };
        let mut frame = self.frame(width, height);
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
            // A hidden window paints again when it is shown (the
            // `Occluded(false)` event); a timeout tries once more.
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
        label: Some("openagents-deck"),
        ..Default::default()
    }))
    .map_err(|error| format!("no graphics device: {error}"))?;
    let caps = surface.get_capabilities(&adapter);
    if !caps.usages.contains(wgpu::TextureUsages::COPY_DST) {
        return Err("the surface does not accept copied frames".to_string());
    }
    use wgpu::TextureFormat as F;
    let format = [
        F::Bgra8Unorm,
        F::Rgba8Unorm,
        F::Bgra8UnormSrgb,
        F::Rgba8UnormSrgb,
    ]
    .into_iter()
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
        view_formats: vec![],
    };
    surface.configure(&device, &config);
    Ok(Gpu {
        surface,
        device,
        queue,
        config,
        bgra: matches!(format, F::Bgra8Unorm | F::Bgra8UnormSrgb),
    })
}

impl ApplicationHandler for Show {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title(self.deck.title())
            .with_inner_size(LogicalSize::new(1280.0, 720.0))
            .with_min_inner_size(LogicalSize::new(480.0, 270.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                self.error = Some(format!("cannot open a window: {error}"));
                event_loop.exit();
                return;
            }
        };
        match gpu(window.clone()) {
            Ok(gpu) => self.gpu = Some(gpu),
            Err(error) => {
                self.error = Some(error);
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window.clone());
        if self.fullscreen {
            self.set_fullscreen(true);
        }
        window.focus_window();
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            // A change of scale, a window shown again, and the end of a move
            // into or out of a fullscreen space all want a fresh frame.
            WindowEvent::ScaleFactorChanged { .. }
            | WindowEvent::Occluded(false)
            | WindowEvent::Focused(true)
            | WindowEvent::Moved(_) => self.redraw(),
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if !self.key(&event.logical_key) {
                        event_loop.exit();
                        return;
                    }
                    self.redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => self.cursor = position,
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.click();
                self.redraw();
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.render() {
                    self.error = Some(error);
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }
}
