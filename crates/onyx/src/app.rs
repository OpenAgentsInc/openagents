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

// Voice status colors
const VOICE_RECORDING_COLOR: Hsla = Hsla::new(0.0, 0.8, 0.5, 1.0);      // Red
const VOICE_TRANSCRIBING_COLOR: Hsla = Hsla::new(45.0, 0.9, 0.5, 1.0);  // Orange
const VOICE_SUCCESS_COLOR: Hsla = Hsla::new(120.0, 0.6, 0.5, 1.0);      // Green

// Update status colors
const UPDATE_CHECKING_COLOR: Hsla = Hsla::new(200.0, 0.6, 0.5, 1.0);    // Blue
const UPDATE_AVAILABLE_COLOR: Hsla = Hsla::new(280.0, 0.6, 0.6, 1.0);   // Purple
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::file_watcher::{FileChange, FileWatcher};
use crate::update_checker::{self, UpdateCheckResult};

// Platform-specific imports for macOS transparency
#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSColor;
use crate::vault::{FileEntry, Vault};
use crate::voice::VoiceState;

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

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext, opacity: f32) {
        // Background with configurable opacity
        if opacity > 0.0 {
            cx.scene.draw_quad(
                Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, opacity)),
            );
        }

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

            // Truncate title to fit sidebar width
            let font_size = theme::font_size::SM * 0.9;
            let char_width = font_size * 0.6; // Approximate mono char width
            let available_width = bounds.size.width - SIDEBAR_PADDING * 2.0;
            let max_chars = (available_width / char_width) as usize;
            let char_count = file.title.chars().count();
            let display_title = if char_count > max_chars && max_chars > 3 {
                let truncated: String = file.title.chars().take(max_chars - 3).collect();
                format!("{}...", truncated)
            } else {
                file.title.clone()
            };

            let text_run = cx.text.layout_styled_mono(
                &display_title,
                Point::new(item_bounds.origin.x + SIDEBAR_PADDING, y + 6.0),
                font_size,
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

    // Voice transcription
    pub voice: Option<VoiceState>,
    voice_loading_shown: bool,

    // File watcher for external changes
    pub file_watcher: Option<FileWatcher>,

    // UI state
    pub sidebar_visible: bool,
    pub background_opacity: f32, // 0.0 = fully transparent, 1.0 = fully opaque
}

impl RenderState {
    /// Set macOS window transparency based on opacity (0.0 = transparent, 1.0 = opaque)
    #[cfg(target_os = "macos")]
    fn set_macos_transparency(&self, opacity: f32) {
        if let Ok(handle) = self.window.window_handle() {
            if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                // Safety: We're accessing the NSWindow from the AppKit handle
                let ns_view = appkit_handle.ns_view.as_ptr() as *mut objc2::runtime::AnyObject;
                unsafe {
                    let view: &objc2_app_kit::NSView = &*(ns_view as *const _);
                    if let Some(window) = view.window() {
                        // Window is non-opaque when opacity < 1.0
                        window.setOpaque(opacity >= 1.0);
                        if opacity < 1.0 {
                            window.setBackgroundColor(Some(&NSColor::clearColor()));
                        } else {
                            window.setBackgroundColor(Some(&NSColor::windowBackgroundColor()));
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn set_macos_transparency(&self, _opacity: f32) {
        // No-op on non-macOS platforms
    }

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
        if let Some(path) = &self.current_file.clone() {
            let content = self.editor.content();
            if content != self.last_saved_content {
                if self.vault.write_file(path, &content).is_ok() {
                    self.last_saved_content = content.clone();

                    // Check if title changed and rename file if needed
                    if let Some(title) = content.lines().next() {
                        if let Ok(Some(new_path)) = self.vault.rename_file(path, title) {
                            // Update internal state with new path
                            self.current_file = Some(new_path.clone());
                            self.sidebar.selected_path = Some(new_path);
                        }
                    }

                    // Refresh file list to update modified times and names
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
            // Write the title as first line
            let _ = self.vault.write_file(&path, &format!("{}\n\n", name));
            // Refresh file list
            if let Ok(files) = self.vault.list_files() {
                self.sidebar.set_files(files);
            }
            // Open the new file
            self.open_file(path);
            // Select the title so user can immediately type to replace it
            self.editor.select_line(0);
            // Enter insert mode so user can immediately type
            self.editor.enter_insert_mode();
        }
    }

    fn check_autosave(&mut self) {
        let content = self.editor.content();
        if content != self.last_saved_content {
            self.save_current();
        }
    }

    fn archive_current_file(&mut self) {
        if let Some(path) = self.current_file.take() {
            // Find current index before archiving
            let current_index = self.sidebar.files.iter().position(|f| f.path == path);

            // Archive the file
            if self.vault.archive_file(&path).is_ok() {
                // Refresh file list
                if let Ok(files) = self.vault.list_files() {
                    self.sidebar.set_files(files.clone());

                    if files.is_empty() {
                        // No files left, create a new one
                        self.create_new_file();
                    } else if let Some(idx) = current_index {
                        // Open the file at the same position (or previous if at end)
                        let new_idx = idx.min(files.len() - 1);
                        self.open_file(files[new_idx].path.clone());
                    } else {
                        // Fallback to first file
                        self.open_file(files[0].path.clone());
                    }
                }
            }
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

    fn check_for_updates(&mut self) {
        self.editor.set_status("Checking for updates...", UPDATE_CHECKING_COLOR);

        // Create a tokio runtime to run the async check
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                self.editor.set_status(&format!("Update check failed: {}", e), VOICE_RECORDING_COLOR);
                return;
            }
        };

        let result = rt.block_on(update_checker::check_for_updates());

        match result {
            UpdateCheckResult::UpToDate => {
                self.editor.set_status(
                    &format!("Onyx {} is up to date", update_checker::CURRENT_VERSION),
                    VOICE_SUCCESS_COLOR,
                );
            }
            UpdateCheckResult::UpdateAvailable { version, url, release_name } => {
                let name = release_name.as_deref().unwrap_or(version.as_str());
                self.editor.set_status(
                    &format!("Update available: {} - visit github.com/OpenAgentsInc/openagents/releases", name),
                    UPDATE_AVAILABLE_COLOR,
                );
                // Try to open the release URL in the browser
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(&url).spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
                }
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("cmd").args(["/c", "start", &url]).spawn();
                }
            }
            UpdateCheckResult::Error(e) => {
                self.editor.set_status(&format!("Update check failed: {}", e), VOICE_RECORDING_COLOR);
            }
        }
    }

    fn handle_external_changes(&mut self, changes: &[FileChange]) {
        let mut should_refresh_sidebar = false;
        let mut should_reload_current = false;

        for change in changes {
            match change {
                FileChange::Modified(path) => {
                    if self.current_file.as_ref() == Some(path) {
                        should_reload_current = true;
                    }
                    should_refresh_sidebar = true;
                }
                FileChange::Created(_) | FileChange::Deleted(_) => {
                    should_refresh_sidebar = true;
                }
            }
        }

        if should_refresh_sidebar {
            if let Ok(files) = self.vault.list_files() {
                self.sidebar.set_files(files);
            }
        }

        if should_reload_current {
            if let Some(path) = &self.current_file {
                // Only reload if content differs from what we have
                // to avoid losing cursor position during our own saves
                if let Ok(disk_content) = self.vault.read_file(path) {
                    if disk_content != self.last_saved_content {
                        // External change detected - reload
                        self.editor.set_content(&disk_content);
                        self.last_saved_content = disk_content;
                        tracing::info!("Reloaded externally modified file");
                    }
                }
            }
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
            .with_maximized(true)
            .with_transparent(true);

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

            // Prefer PreMultiplied alpha for transparency support
            let alpha_mode = if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PreMultiplied) {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else if surface_caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::PostMultiplied) {
                wgpu::CompositeAlphaMode::PostMultiplied
            } else {
                surface_caps.alpha_modes[0]
            };

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode,
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
                let name = vault.generate_unique_name();
                let welcome_content = format!("{}\n\nStart writing your notes here.\n\nPress **Cmd+N** to create a new note.\n\nUse **Cmd+Shift+Up/Down** to switch between notes.", name);
                if let Ok(path) = vault.create_file(&name) {
                    let _ = vault.write_file(&path, &welcome_content);
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
            editor.clear_status(); // Clear any stale status on startup

            // Initialize voice transcription
            let voice = match VoiceState::new() {
                Ok(v) => {
                    tracing::info!("Voice transcription initialized");
                    Some(v)
                }
                Err(e) => {
                    tracing::warn!("Voice transcription unavailable: {}", e);
                    None
                }
            };

            // Show loading status if voice is initializing
            let voice_loading_shown = if let Some(ref v) = voice {
                if v.is_loading() {
                    editor.set_status("Loading voice model...", VOICE_TRANSCRIBING_COLOR);
                    true
                } else {
                    false
                }
            } else {
                false
            };

            // Initialize file watcher for external changes
            let file_watcher = match FileWatcher::new(vault.path.clone()) {
                Ok(fw) => {
                    tracing::info!("File watcher initialized");
                    Some(fw)
                }
                Err(e) => {
                    tracing::warn!("File watcher unavailable: {}", e);
                    None
                }
            };

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
                voice,
                voice_loading_shown,
                file_watcher,
                sidebar_visible: true,
                background_opacity: 1.0,
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

        let sidebar_width = if state.sidebar_visible { SIDEBAR_WIDTH } else { 0.0 };
        let sidebar_bounds = Bounds::new(0.0, 0.0, sidebar_width, logical_height);
        let editor_bounds = Bounds::new(sidebar_width, 0.0, logical_width - sidebar_width, logical_height);

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
                // Debug: log all key events to diagnose voice key issues
                tracing::debug!("KeyboardInput: physical={:?} logical={:?} state={:?} repeat={}",
                    event.physical_key, event.logical_key, event.state, event.repeat);

                // Handle backtick (`) for voice transcription (hold to record, release to transcribe)
                // Check physical key codes (Backquote, IntlBackslash, Section) AND logical character
                let is_voice_key_physical = matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::Backquote) |
                    PhysicalKey::Code(KeyCode::IntlBackslash) |
                    PhysicalKey::Code(KeyCode::F13)  // Some keyboards map backtick here
                );
                let is_voice_key_logical = matches!(
                    &event.logical_key,
                    Key::Character(c) if c == "`" || c == "§" || c == "±"
                );
                let is_voice_key = is_voice_key_physical || is_voice_key_logical;

                // Debug: log any potential voice key detection
                if is_voice_key_physical || is_voice_key_logical {
                    tracing::info!("Potential voice key: physical={:?} logical={:?}",
                        event.physical_key, event.logical_key);
                }

                // Block ALL voice key events from reaching the editor (including repeats)
                if is_voice_key {
                    // Log ALL voice key events for debugging (before any filtering)
                    tracing::info!("VOICE KEY: state={:?} repeat={}", event.state, event.repeat);

                    // For Pressed: skip repeats. For Released: always process (releases don't repeat logically)
                    let should_process = match event.state {
                        ElementState::Pressed => !event.repeat,
                        ElementState::Released => true, // Always process release
                    };

                    if should_process {
                        // Handle voice recording WITHOUT catch_unwind - let's see actual errors
                        tracing::info!("Processing voice event: {:?}", event.state);

                        if let Some(voice) = &mut state.voice {
                            match event.state {
                                ElementState::Pressed => {
                                    // Start recording on press
                                    if !voice.recording {
                                        tracing::info!("Starting recording...");
                                        match voice.start_recording() {
                                            Ok(()) => {
                                                tracing::info!("Recording started successfully");
                                                state.editor.set_status("● Recording...", VOICE_RECORDING_COLOR);
                                            }
                                            Err(e) => {
                                                tracing::error!("Failed to start recording: {}", e);
                                                state.editor.set_status(&format!("Mic error: {}", e), VOICE_RECORDING_COLOR);
                                            }
                                        }
                                    } else {
                                        tracing::debug!("Already recording, ignoring press");
                                    }
                                }
                                ElementState::Released => {
                                    tracing::info!("Release detected, stopping recording...");
                                    // Stop recording and start background transcription
                                    match voice.stop_recording() {
                                        Ok(true) => {
                                            // Transcription started in background
                                            tracing::info!("Transcription started in background");
                                            state.editor.set_status("⟳ Transcribing...", VOICE_TRANSCRIBING_COLOR);
                                        }
                                        Ok(false) => {
                                            // Quick tap or no audio - discarded
                                            tracing::info!("Recording discarded (quick tap or no audio)");
                                            state.editor.set_status("Recording discarded", VOICE_TRANSCRIBING_COLOR);
                                        }
                                        Err(e) => {
                                            tracing::error!("Recording stop failed: {}", e);
                                            state.editor.set_status(&format!("Voice error: {}", e), VOICE_RECORDING_COLOR);
                                        }
                                    }
                                }
                            }
                        } else {
                            tracing::warn!("Voice not available");
                            state.editor.set_status("Voice not available", VOICE_RECORDING_COLOR);
                        }

                        state.window.request_redraw();
                    } else {
                        tracing::debug!("Skipping repeat voice key press");
                    }
                    // Always return for voice key to prevent backticks in editor
                    return;
                }

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
                        // Handle Cmd+Shift+V for vim mode toggle
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && state.modifiers.shift_key()
                            && (c == "v" || c == "V")
                        {
                            state.editor.toggle_vim_mode();
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+Shift+U for update check
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && state.modifiers.shift_key()
                            && (c == "u" || c == "U")
                        {
                            state.check_for_updates();
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+B for sidebar toggle
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && (c == "b" || c == "B")
                        {
                            state.sidebar_visible = !state.sidebar_visible;
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd++ or Cmd+= for zoom in
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && (c == "+" || c == "=")
                        {
                            state.editor.zoom_in();
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+- for zoom out
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && c == "-"
                        {
                            state.editor.zoom_out();
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+0 for zoom reset
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && c == "0"
                        {
                            state.editor.zoom_reset();
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+Y to reset opacity to 100%
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && (c == "y" || c == "Y")
                        {
                            state.background_opacity = 1.0;
                            tracing::info!("Background opacity reset to 100%");
                            state.set_macos_transparency(state.background_opacity);
                            state.editor.set_background_opacity(state.background_opacity);
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+, to decrease opacity (more transparent)
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && c == ","
                        {
                            state.background_opacity = (state.background_opacity - 0.1).max(0.0);
                            tracing::info!("Background opacity: {:.0}%", state.background_opacity * 100.0);
                            state.set_macos_transparency(state.background_opacity);
                            state.editor.set_background_opacity(state.background_opacity);
                            state.window.request_redraw();
                            return;
                        }
                        // Handle Cmd+. to increase opacity (more opaque)
                        if (state.modifiers.control_key() || state.modifiers.super_key())
                            && c == "."
                        {
                            state.background_opacity = (state.background_opacity + 0.1).min(1.0);
                            tracing::info!("Background opacity: {:.0}%", state.background_opacity * 100.0);
                            state.set_macos_transparency(state.background_opacity);
                            state.editor.set_background_opacity(state.background_opacity);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Handle Cmd+Shift+Up/Down for file navigation, Ctrl+Shift+Backspace for archive
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
                                NamedKey::Backspace => {
                                    state.archive_current_file();
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
                    let mods = wgpui::input::Modifiers {
                        shift: state.modifiers.shift_key(),
                        ctrl: state.modifiers.control_key(),
                        alt: state.modifiers.alt_key(),
                        meta: state.modifiers.super_key(),
                    };
                    wgpui::InputEvent::MouseDown { button: wgpui_button, x, y, modifiers: mods }
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

                // Check voice model loading status
                if state.voice_loading_shown {
                    if let Some(ref voice) = state.voice {
                        if !voice.is_loading() {
                            // Model finished loading
                            state.voice_loading_shown = false;
                            if voice.is_ready() {
                                state.editor.set_status("Voice ready (` to record)", VOICE_SUCCESS_COLOR);
                            } else if let Some(msg) = voice.status_message() {
                                state.editor.set_status(&msg, VOICE_RECORDING_COLOR);
                            }
                        }
                    }
                }

                // Poll for transcription results from background thread
                if let Some(ref mut voice) = state.voice {
                    if let Some(result) = voice.take_transcription_result() {
                        tracing::info!("Got transcription result from background thread");
                        match result {
                            Ok(text) if !text.is_empty() => {
                                // Show the transcribed text briefly
                                let preview = if text.len() > 50 {
                                    format!("\"{}...\"", &text[..47])
                                } else {
                                    format!("\"{}\"", text)
                                };
                                tracing::info!("Transcription success: {}", preview);
                                state.editor.set_status(&preview, VOICE_SUCCESS_COLOR);
                                state.editor.insert_str(&text);
                            }
                            Ok(_) => {
                                // Empty transcription - audio too short or no speech detected
                                tracing::info!("Transcription returned empty (no speech detected)");
                                state.editor.set_status("No speech detected", VOICE_TRANSCRIBING_COLOR);
                            }
                            Err(e) => {
                                tracing::error!("Transcription failed: {}", e);
                                state.editor.set_status(&format!("Error: {}", e), VOICE_RECORDING_COLOR);
                            }
                        }
                    }
                }

                // Build scene
                let mut scene = Scene::new();
                let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);

                // Paint sidebar (if visible)
                if state.sidebar_visible {
                    state.sidebar.paint(sidebar_bounds, &mut paint_cx, state.background_opacity);
                }

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

                // Use transparent clear color when opacity < 1.0
                if state.background_opacity < 1.0 {
                    let transparent = wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
                    state.renderer.render_with_clear(&mut encoder, &view, transparent);
                } else {
                    state.renderer.render(&mut encoder, &view);
                }

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

            // Check for external file changes
            if let Some(ref mut watcher) = state.file_watcher {
                let changes = watcher.take_changes();
                if !changes.is_empty() {
                    state.handle_external_changes(&changes);
                }
            }

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
