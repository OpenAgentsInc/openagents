//! Onyx application handler

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use arboard::Clipboard;
use web_time::Instant;
use wgpui::components::{Component, EventContext, LiveEditor, PaintContext};
use wgpui::renderer::Renderer;
use wgpui::text::FontStyle;
use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem, theme};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

use crate::vault::{FileEntry, Vault};

// Layout constants
const SIDEBAR_WIDTH: f32 = 200.0;
const FILE_ITEM_HEIGHT: f32 = 28.0;
const SIDEBAR_PADDING: f32 = 8.0;

/// Main application state
pub struct OnyxApp {
    state: Option<RenderState>,
}

impl Default for OnyxApp {
    fn default() -> Self {
        Self { state: None }
    }
}

/// Sidebar showing file list
struct FileSidebar {
    files: Vec<FileEntry>,
    selected_path: Option<PathBuf>,
    hovered_index: Option<usize>,
    scroll_offset: f32,
}

impl FileSidebar {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            selected_path: None,
            hovered_index: None,
            scroll_offset: 0.0,
        }
    }

    fn set_files(&mut self, files: Vec<FileEntry>) {
        self.files = files;
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.08, 1.0)),
        );

        // File items
        let mut y = bounds.origin.y + SIDEBAR_PADDING - self.scroll_offset;

        for (i, file) in self.files.iter().enumerate() {
            if y + FILE_ITEM_HEIGHT < bounds.origin.y {
                y += FILE_ITEM_HEIGHT;
                continue;
            }
            if y > bounds.origin.y + bounds.size.height {
                break;
            }

            let item_bounds = Bounds::new(
                bounds.origin.x,
                y,
                bounds.size.width,
                FILE_ITEM_HEIGHT,
            );

            // Highlight selected or hovered
            let is_selected = self.selected_path.as_ref() == Some(&file.path);
            let is_hovered = self.hovered_index == Some(i);

            if is_selected {
                cx.scene.draw_quad(
                    Quad::new(item_bounds).with_background(Hsla::new(210.0, 0.6, 0.4, 0.4)),
                );
            } else if is_hovered {
                cx.scene.draw_quad(
                    Quad::new(item_bounds).with_background(Hsla::new(0.0, 0.0, 0.15, 1.0)),
                );
            }

            // File name text
            let text_color = if is_selected {
                theme::text::PRIMARY
            } else {
                Hsla::new(0.0, 0.0, 0.6, 1.0)
            };

            let text_run = cx.text.layout_styled_mono(
                &file.name,
                Point::new(item_bounds.origin.x + SIDEBAR_PADDING, y + 6.0),
                theme::font_size::SM * 0.9,
                text_color,
                FontStyle::default(),
            );
            cx.scene.draw_text(text_run);

            y += FILE_ITEM_HEIGHT;
        }

        // Border on right edge
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x + bounds.size.width - 1.0,
                bounds.origin.y,
                1.0,
                bounds.size.height,
            ))
            .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
        );
    }

    fn handle_click(&self, x: f32, y: f32, bounds: &Bounds) -> Option<PathBuf> {
        if !bounds.contains(Point::new(x, y)) {
            return None;
        }

        let relative_y = y - bounds.origin.y - SIDEBAR_PADDING + self.scroll_offset;
        let index = (relative_y / FILE_ITEM_HEIGHT) as usize;

        if index < self.files.len() {
            Some(self.files[index].path.clone())
        } else {
            None
        }
    }

    fn update_hover(&mut self, x: f32, y: f32, bounds: &Bounds) {
        if !bounds.contains(Point::new(x, y)) {
            self.hovered_index = None;
            return;
        }

        let relative_y = y - bounds.origin.y - SIDEBAR_PADDING + self.scroll_offset;
        let index = (relative_y / FILE_ITEM_HEIGHT) as usize;

        self.hovered_index = if index < self.files.len() {
            Some(index)
        } else {
            None
        };
    }

    fn handle_scroll(&mut self, dy: f32, bounds: &Bounds) {
        let content_height = self.files.len() as f32 * FILE_ITEM_HEIGHT + SIDEBAR_PADDING * 2.0;
        let max_scroll = (content_height - bounds.size.height).max(0.0);

        self.scroll_offset = (self.scroll_offset - dy * 20.0).clamp(0.0, max_scroll);
    }
}

/// Render state holding all GPU and UI resources
pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub last_tick: Instant,
    pub modifiers: ModifiersState,
    #[allow(dead_code)]
    pub clipboard: Rc<RefCell<Option<Clipboard>>>,
    pub event_context: EventContext,
    pub last_mouse_pos: (f32, f32),

    // File management
    pub vault: Vault,
    sidebar: FileSidebar,
    pub current_file: Option<PathBuf>,
    pub last_saved_content: String,

    // Editor
    pub editor: LiveEditor,
}

impl RenderState {
    fn open_file(&mut self, path: PathBuf) {
        // Save current file first
        self.save_current();

        if let Ok(content) = self.vault.read_file(&path) {
            self.editor.set_content(&content);
            self.current_file = Some(path.clone());
            self.sidebar.selected_path = Some(path);
            self.last_saved_content = content;
        }
    }

    fn save_current(&mut self) {
        if let Some(path) = &self.current_file {
            let content = self.editor.content();
            if content != self.last_saved_content {
                if self.vault.write_file(path, &content).is_ok() {
                    self.last_saved_content = content;
                    // Refresh file list to update modified times
                    if let Ok(files) = self.vault.list_files() {
                        self.sidebar.set_files(files);
                    }
                }
            }
        }
    }

    fn create_new_file(&mut self) {
        // Save current first
        self.save_current();

        let name = self.vault.generate_unique_name();
        if let Ok(path) = self.vault.create_file(&name) {
            // Refresh file list
            if let Ok(files) = self.vault.list_files() {
                self.sidebar.set_files(files);
            }
            // Open the new file
            self.open_file(path);
        }
    }

    fn check_autosave(&mut self) {
        let content = self.editor.content();
        if content != self.last_saved_content {
            self.save_current();
        }
    }

    fn navigate_file(&mut self, direction: i32) {
        if self.sidebar.files.is_empty() {
            return;
        }

        // Find current index
        let current_index = self.current_file.as_ref().and_then(|current| {
            self.sidebar.files.iter().position(|f| &f.path == current)
        }).unwrap_or(0);

        // Calculate new index
        let new_index = if direction < 0 {
            // Up - go to previous (earlier in list = more recent)
            if current_index == 0 {
                self.sidebar.files.len() - 1
            } else {
                current_index - 1
            }
        } else {
            // Down - go to next (later in list = older)
            if current_index >= self.sidebar.files.len() - 1 {
                0
            } else {
                current_index + 1
            }
        };

        if let Some(file) = self.sidebar.files.get(new_index) {
            let path = file.path.clone();
            self.open_file(path);
        }
    }
}

impl ApplicationHandler for OnyxApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Onyx")
            .with_maximized(true);

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);

            // Initialize clipboard
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));

            // Initialize EventContext with clipboard
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );

            // Open vault
            let vault_path = Vault::default_path();
            let vault = Vault::open(vault_path).expect("Failed to open vault");
            let files = vault.list_files().unwrap_or_default();

            // Create sidebar
            let mut sidebar = FileSidebar::new();
            sidebar.set_files(files.clone());

            // Open first file or create welcome content
            let (initial_content, current_file) = if let Some(first) = files.first() {
                sidebar.selected_path = Some(first.path.clone());
                (
                    vault.read_file(&first.path).unwrap_or_default(),
                    Some(first.path.clone()),
                )
            } else {
                // Create a welcome note
                let welcome_content = "# Welcome to Onyx\n\nStart writing your notes here.\n\nPress **Cmd+N** to create a new note.\n\nUse **Cmd+Shift+Up/Down** to switch between notes.";
                let name = vault.generate_unique_name();
                if let Ok(path) = vault.create_file(&name) {
                    let _ = vault.write_file(&path, welcome_content);
                    // Refresh file list
                    let files = vault.list_files().unwrap_or_default();
                    sidebar.set_files(files);
                    sidebar.selected_path = Some(path.clone());
                    (welcome_content.to_string(), Some(path))
                } else {
                    (welcome_content.to_string(), None)
                }
            };

            let last_saved_content = initial_content.clone();

            let mut editor = LiveEditor::new(&initial_content).with_id(1);
            editor.focus();

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                last_tick: Instant::now(),
                modifiers: ModifiersState::empty(),
                clipboard,
                event_context,
                last_mouse_pos: (0.0, 0.0),
                vault,
                sidebar,
                current_file,
                last_saved_content,
                editor,
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        let sidebar_bounds = Bounds::new(0.0, 0.0, SIDEBAR_WIDTH, logical_height);
        let editor_bounds = Bounds::new(SIDEBAR_WIDTH, 0.0, logical_width - SIDEBAR_WIDTH, logical_height);

        match event {
            WindowEvent::CloseRequested => {
                state.save_current();
                event_loop.exit();
            }

            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    // Handle Cmd+N for new file
                    if let Key::Character(c) = &event.logical_key {
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && (c == "n" || c == "N")
                        {
                            state.create_new_file();
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Handle Cmd+Shift+Up/Down for file navigation
                    if let Key::Named(named) = &event.logical_key {
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && state.modifiers.shift_key()
                        {
                            match named {
                                NamedKey::ArrowUp => {
                                    state.navigate_file(-1);
                                    state.window.request_redraw();
                                    return;
                                }
                                NamedKey::ArrowDown => {
                                    state.navigate_file(1);
                                    state.window.request_redraw();
                                    return;
                                }
                                _ => {}
                            }
                        }
                    }

                    // Convert winit key to wgpui InputEvent
                    if let Some(wgpui_event) = convert_key_event(&event.logical_key, &state.modifiers) {
                        state.editor.event(&wgpui_event, editor_bounds, &mut state.event_context);
                    }
                }
            }

            WindowEvent::MouseInput { state: button_state, button, .. } => {
                let (x, y) = state.last_mouse_pos;

                if button == winit::event::MouseButton::Left && button_state == ElementState::Pressed {
                    // Check sidebar click first
                    if let Some(path) = state.sidebar.handle_click(x, y, &sidebar_bounds) {
                        state.open_file(path);
                        state.window.request_redraw();
                        return;
                    }
                }

                // Pass to editor
                let wgpui_button = match button {
                    winit::event::MouseButton::Left => wgpui::MouseButton::Left,
                    winit::event::MouseButton::Right => wgpui::MouseButton::Right,
                    winit::event::MouseButton::Middle => wgpui::MouseButton::Middle,
                    _ => wgpui::MouseButton::Left,
                };

                let event = if button_state == ElementState::Pressed {
                    wgpui::InputEvent::MouseDown { button: wgpui_button, x, y }
                } else {
                    wgpui::InputEvent::MouseUp { button: wgpui_button, x, y }
                };
                state.editor.event(&event, editor_bounds, &mut state.event_context);
                state.window.request_redraw();
            }

            WindowEvent::CursorMoved { position, .. } => {
                let logical_x = position.x as f32 / scale_factor;
                let logical_y = position.y as f32 / scale_factor;
                state.last_mouse_pos = (logical_x, logical_y);

                // Update sidebar hover
                state.sidebar.update_hover(logical_x, logical_y, &sidebar_bounds);

                // Pass to editor
                let event = wgpui::InputEvent::MouseMove { x: logical_x, y: logical_y };
                state.editor.event(&event, editor_bounds, &mut state.event_context);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };

                let (x, y) = state.last_mouse_pos;

                // Check if mouse is over sidebar
                if sidebar_bounds.contains(Point::new(x, y)) {
                    state.sidebar.handle_scroll(dy, &sidebar_bounds);
                } else {
                    let scroll_event = wgpui::InputEvent::Scroll { dx: 0.0, dy };
                    state.editor.event(&scroll_event, editor_bounds, &mut state.event_context);
                }
            }

            WindowEvent::RedrawRequested => {
                let physical_width = state.config.width as f32;
                let physical_height = state.config.height as f32;

                state.last_tick = Instant::now();

                // Build scene
                let mut scene = Scene::new();
                let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);

                // Paint sidebar
                state.sidebar.paint(sidebar_bounds, &mut paint_cx);

                // Paint editor
                state.editor.paint(editor_bounds, &mut paint_cx);

                // Render
                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder = state
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });

                state.renderer.resize(
                    &state.queue,
                    Size::new(physical_width, physical_height),
                    1.0,
                );

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            // Autosave on idle
            state.check_autosave();
            state.window.request_redraw();
        }
    }
}

/// Convert winit key event to wgpui InputEvent
fn convert_key_event(key: &Key, modifiers: &ModifiersState) -> Option<wgpui::InputEvent> {
    let wgpui_modifiers = wgpui::input::Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    };

    let wgpui_key = match key {
        Key::Character(c) => wgpui::input::Key::Character(c.to_string()),
        Key::Named(named) => {
            let wgpui_named = match named {
                NamedKey::Enter => wgpui::input::NamedKey::Enter,
                NamedKey::Tab => wgpui::input::NamedKey::Tab,
                NamedKey::Space => wgpui::input::NamedKey::Space,
                NamedKey::Backspace => wgpui::input::NamedKey::Backspace,
                NamedKey::Delete => wgpui::input::NamedKey::Delete,
                NamedKey::Escape => wgpui::input::NamedKey::Escape,
                NamedKey::ArrowUp => wgpui::input::NamedKey::ArrowUp,
                NamedKey::ArrowDown => wgpui::input::NamedKey::ArrowDown,
                NamedKey::ArrowLeft => wgpui::input::NamedKey::ArrowLeft,
                NamedKey::ArrowRight => wgpui::input::NamedKey::ArrowRight,
                NamedKey::Home => wgpui::input::NamedKey::Home,
                NamedKey::End => wgpui::input::NamedKey::End,
                NamedKey::PageUp => wgpui::input::NamedKey::PageUp,
                NamedKey::PageDown => wgpui::input::NamedKey::PageDown,
                _ => return None,
            };
            wgpui::input::Key::Named(wgpui_named)
        }
        _ => return None,
    };

    Some(wgpui::InputEvent::KeyDown {
        key: wgpui_key,
        modifiers: wgpui_modifiers,
    })
}
