//! Text System Demo - Standalone Binary
//!
//! Demonstrates the upgraded text system including:
//! - Line wrapping with different widths
//! - Text decorations (underline, strikethrough, background)
//! - Multiple font sizes
//! - Unicode support (Latin, Cyrillic, CJK)
//! - Position/index queries (click detection)
//! - Layout cache statistics
//! - Streaming markdown rendering

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpui::{
    Animation, AnimationController, Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, Scene, Size, SvgQuad, TextSystem, theme,
    // New text system types
    LineWrapper, LineFragment, Boundary, TruncateFrom,
    LineLayout, ShapedRun, ShapedGlyph, FontRun,
    LineLayoutCache,
    // Markdown
    StreamingMarkdown, MarkdownRenderer,
};
use wgpui::components::hud::{
    CornerConfig, DotsGrid, DotsOrigin, DotShape, Frame,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    demo: DemoState,
}

struct DemoState {
    start_time: Instant,
    frame_count: u64,
    mouse_pos: Point,
    scroll_offset: f32,
    // Cache for line layouts
    layout_cache: LineLayoutCache,
    // Wrap width for interactive demo
    wrap_width: f32,
    // Streaming markdown
    streaming_markdown: StreamingMarkdown,
    markdown_renderer: MarkdownRenderer,
    markdown_tokens: VecDeque<String>,
    markdown_started: bool,
    last_token_frame: u64,
    // HUD effects
    dots_grid: DotsGrid,
    dots_anim: Animation<f32>,
    frame_anim: Animation<f32>,
    anim_controller: AnimationController,
}

impl Default for DemoState {
    fn default() -> Self {
        // Pre-tokenize some sample markdown content
        let sample_markdown = r#"# Streaming Markdown Demo

This is a **live demonstration** of the `StreamingMarkdown` system.

## Features

- Token-by-token streaming
- **Bold** and *italic* text
- `inline code` blocks
- Syntax highlighted code

```rust
fn main() {
    println!("Hello, WGPUI!");
}
```

> Blockquotes render beautifully
> with proper styling.

1. Ordered lists
2. Work perfectly
3. In real-time

---

*Press R to restart the stream*
"#;

        // Tokenize into small chunks
        let tokens: VecDeque<String> = sample_markdown
            .chars()
            .collect::<Vec<_>>()
            .chunks(3)
            .map(|chunk| chunk.iter().collect())
            .collect();

        // Initialize dots animation (infinite loop)
        let mut dots_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2000))
            .easing(Easing::EaseOut)
            .iterations(0) // infinite
            .alternate();
        dots_anim.start();

        // Initialize frame animation (infinite loop)
        let mut frame_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(1500))
            .easing(Easing::EaseInOutCubic)
            .iterations(0) // infinite
            .alternate();
        frame_anim.start();

        Self {
            start_time: Instant::now(),
            frame_count: 0,
            mouse_pos: Point::ZERO,
            scroll_offset: 0.0,
            layout_cache: LineLayoutCache::new(),
            wrap_width: 400.0,
            streaming_markdown: StreamingMarkdown::new(),
            markdown_renderer: MarkdownRenderer::new(),
            markdown_tokens: tokens,
            markdown_started: true, // Start immediately
            last_token_frame: 0,
            // HUD effects
            dots_grid: DotsGrid::new()
                .color(Hsla::new(0.0, 0.0, 1.0, 0.15)) // White, subtle
                .shape(DotShape::Cross)
                .distance(32.0)
                .size(4.0)
                .cross_thickness(1.0)
                .origin(DotsOrigin::Center)
                .animation_progress(1.0), // Fully visible, no animation
            dots_anim,
            frame_anim,
            anim_controller: AnimationController::new(),
        }
    }
}

impl DemoState {
    fn restart_markdown_stream(&mut self) {
        // Re-tokenize the sample markdown
        let sample_markdown = r#"# Streaming Markdown Demo

This is a **live demonstration** of the `StreamingMarkdown` system.

## Features

- Token-by-token streaming
- **Bold** and *italic* text
- `inline code` blocks
- Syntax highlighted code

```rust
fn main() {
    println!("Hello, WGPUI!");
}
```

> Blockquotes render beautifully
> with proper styling.

1. Ordered lists
2. Work perfectly
3. In real-time

---

*Press R to restart the stream*
"#;

        self.markdown_tokens = sample_markdown
            .chars()
            .collect::<Vec<_>>()
            .chunks(3)
            .map(|chunk| chunk.iter().collect())
            .collect();

        self.streaming_markdown.reset();
        self.markdown_started = true;
        self.last_token_frame = self.frame_count;
    }

    fn advance_markdown_stream(&mut self) {
        if !self.markdown_started {
            return;
        }

        // Stream tokens every 2 frames for visible effect
        let frames_since_token = self.frame_count - self.last_token_frame;
        if frames_since_token >= 2 && !self.markdown_tokens.is_empty() {
            if let Some(token) = self.markdown_tokens.pop_front() {
                self.streaming_markdown.append(&token);
                self.last_token_frame = self.frame_count;
            }
        }

        // If all tokens consumed, mark as complete
        if self.markdown_tokens.is_empty() && self.streaming_markdown.has_pending() {
            self.streaming_markdown.complete();
        }

        self.streaming_markdown.tick();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("wgpui Text System Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 900));

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

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                demo: DemoState::default(),
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.demo.mouse_pos = Point::new(position.x as f32, position.y as f32);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => -y * 30.0,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => -pos.y as f32,
                };
                state.demo.scroll_offset = (state.demo.scroll_offset + dy).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed {
                    match event.logical_key.as_ref() {
                        winit::keyboard::Key::Character(c) if c == "-" => {
                            state.demo.wrap_width = (state.demo.wrap_width - 20.0).max(100.0);
                            state.window.request_redraw();
                        }
                        winit::keyboard::Key::Character(c) if c == "=" => {
                            state.demo.wrap_width = (state.demo.wrap_width + 20.0).min(800.0);
                            state.window.request_redraw();
                        }
                        winit::keyboard::Key::Character(c) if c == "r" || c == "R" => {
                            // Restart streaming markdown
                            state.demo.restart_markdown_stream();
                            state.window.request_redraw();
                        }
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) => {
                            // Start streaming if not started
                            if !state.demo.markdown_started {
                                state.demo.markdown_started = true;
                            }
                            state.window.request_redraw();
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                // Get scale factor first
                let scale_factor = state.window.scale_factor() as f32;

                // Use LOGICAL dimensions for scene layout
                // config.width/height are physical pixels, divide by scale_factor to get logical
                let logical_width = state.config.width as f32 / scale_factor;
                let logical_height = state.config.height as f32 / scale_factor;

                state.demo.frame_count += 1;
                state.demo.advance_markdown_stream();

                let mut scene = Scene::new();
                build_text_demo(
                    &mut scene,
                    &mut state.text_system,
                    &mut state.demo,
                    logical_width,
                    logical_height,
                );

                let output = state
                    .surface
                    .get_current_texture()
                    .expect("Failed to get surface texture");
                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                // Viewport is in PHYSICAL pixels
                let physical_width = state.config.width as f32;
                let physical_height = state.config.height as f32;
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

                // Pass scale_factor to prepare() for logical->physical coordinate conversion
                state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
                state.renderer.render(&mut encoder, &view);

                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn build_text_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
) {
    let margin = 24.0;
    let col_width = (width - margin * 3.0) / 2.0;

    // Get delta time and update frame animation (dots are static now)
    let delta = demo.anim_controller.delta();
    let _frame_progress = demo.frame_anim.tick(delta);

    // Background
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    // Paint static dots grid as background effect (white, no animation)
    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);

    let mut y = margin - demo.scroll_offset;

    // Header
    draw_header(scene, text_system, margin, &mut y, width, demo);

    let left_x = margin;
    let right_x = margin * 2.0 + col_width;
    let mut left_y = y;
    let mut right_y = y;

    // Left column
    demo_line_wrapping(scene, text_system, demo, left_x, col_width, &mut left_y);
    left_y += 24.0;
    demo_text_decorations(scene, text_system, left_x, col_width, &mut left_y);
    left_y += 24.0;
    demo_font_sizes(scene, text_system, left_x, col_width, &mut left_y);
    left_y += 24.0;
    demo_unicode_support(scene, text_system, left_x, col_width, &mut left_y);

    // Right column
    demo_line_wrapper_api(scene, text_system, demo, right_x, col_width, &mut right_y);
    right_y += 24.0;
    demo_truncation(scene, text_system, right_x, col_width, &mut right_y);
    right_y += 24.0;
    demo_line_layout_api(scene, text_system, right_x, col_width, &mut right_y);
    right_y += 24.0;
    demo_cache_stats(scene, text_system, demo, right_x, col_width, &mut right_y);
    right_y += 24.0;
    demo_svg_rendering(scene, text_system, right_x, col_width, &mut right_y);

    // Full-width streaming markdown demo at the bottom
    let bottom_y = left_y.max(right_y) + 24.0;
    demo_streaming_markdown(scene, text_system, demo, margin, width - margin * 2.0, bottom_y);
}

fn draw_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    y: &mut f32,
    width: f32,
    demo: &DemoState,
) {
    let header_bounds = Bounds::new(margin, *y, width - margin * 2.0, 80.0);

    // Draw animated frame around header
    // Use current_value() since tick() was already called in build_text_demo
    let frame_progress = demo.frame_anim.current_value();
    let mut frame = Frame::nefrex()
        .line_color(theme::accent::PRIMARY)
        .bg_color(theme::bg::SURFACE.with_alpha(0.3))
        .stroke_width(1.5)
        .corner_length(24.0)
        .corner_config(CornerConfig::diagonal())
        .animation_progress(frame_progress);

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    frame.paint(header_bounds, &mut cx);

    let title = "wgpui Text System Demo";
    let subtitle = format!(
        "Frame {} | Wrap: {}px | Keys: -/= wrap width, R restart markdown",
        demo.frame_count,
        demo.wrap_width,
    );

    let title_run = text_system.layout(title, Point::new(margin + 12.0, *y + 24.0), 28.0, theme::text::PRIMARY);
    scene.draw_text(title_run);

    let subtitle_run = text_system.layout(&subtitle, Point::new(margin + 12.0, *y + 54.0), 12.0, theme::text::MUTED);
    scene.draw_text(subtitle_run);

    *y += 92.0;
}

fn draw_section_header(scene: &mut Scene, text_system: &mut TextSystem, x: f32, y: &mut f32, title: &str) {
    let run = text_system.layout(title, Point::new(x, *y + 14.0), 16.0, theme::accent::PRIMARY);
    scene.draw_text(run);
    scene.draw_quad(
        Quad::new(Bounds::new(x, *y + 32.0, 200.0, 1.0)).with_background(theme::accent::PRIMARY.with_alpha(0.4)),
    );
    *y += 44.0;
}

fn demo_line_wrapping(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &DemoState,
    x: f32,
    _width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Line Wrapping");

    let wrap_width = demo.wrap_width;

    // Draw wrap boundary indicator
    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, wrap_width, 200.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(x + wrap_width, *y, 2.0, 200.0))
            .with_background(theme::accent::RED.with_alpha(0.5)),
    );

    let text = "The quick brown fox jumps over the lazy dog. This is a long sentence that demonstrates word-wrapping behavior at different widths.";

    let font_size = 14.0;
    let line_height = font_size * 1.4;

    // Create a line wrapper
    let char_width = font_size * 0.6; // Approximate for monospace
    let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);

    let fragments = [LineFragment::text(text)];
    let boundaries: Vec<Boundary> = wrapper.wrap_line(&fragments, wrap_width - 16.0).collect();

    // Draw wrapped text manually showing wrap boundaries
    let mut line_start = 0;
    let mut line_y = *y + 12.0;

    for (i, boundary) in boundaries.iter().enumerate() {
        let line_text = &text[line_start..boundary.ix];
        let indent = if i > 0 { boundary.next_indent as f32 * char_width } else { 0.0 };

        let run = text_system.layout(line_text.trim_start(), Point::new(x + 8.0 + indent, line_y), font_size, theme::text::PRIMARY);
        scene.draw_text(run);

        line_start = boundary.ix;
        line_y += line_height;
    }

    // Draw remaining text
    let remaining = &text[line_start..];
    if !remaining.is_empty() {
        let run = text_system.layout(remaining.trim_start(), Point::new(x + 8.0, line_y), font_size, theme::text::PRIMARY);
        scene.draw_text(run);
    }

    // Label
    let label = format!("{} wrap boundaries", boundaries.len());
    let label_run = text_system.layout(&label, Point::new(x + 8.0, *y + 180.0), 11.0, theme::text::MUTED);
    scene.draw_text(label_run);

    *y += 210.0;
}

fn demo_text_decorations(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Text Decorations");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 140.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 14.0;
    let line_height = 28.0;
    let text_x = x + 12.0;
    let mut text_y = *y + 16.0;

    // Normal text
    let run = text_system.layout("Normal text without decorations", Point::new(text_x, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    // Underline (simulated with a line)
    let underline_text = "Underlined text example";
    let run = text_system.layout(underline_text, Point::new(text_x, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    let underline_width = underline_text.len() as f32 * font_size * 0.6;
    scene.draw_quad(
        Quad::new(Bounds::new(text_x, text_y + font_size + 2.0, underline_width, 1.5))
            .with_background(theme::accent::PRIMARY),
    );
    text_y += line_height;

    // Strikethrough (simulated)
    let strike_text = "Strikethrough text example";
    let run = text_system.layout(strike_text, Point::new(text_x, text_y), font_size, theme::text::MUTED);
    scene.draw_text(run);
    let strike_width = strike_text.len() as f32 * font_size * 0.6;
    scene.draw_quad(
        Quad::new(Bounds::new(text_x, text_y + font_size * 0.5, strike_width, 1.5))
            .with_background(theme::accent::RED),
    );
    text_y += line_height;

    // Background highlight
    let highlight_text = "Highlighted text";
    let highlight_width = highlight_text.len() as f32 * font_size * 0.6;
    scene.draw_quad(
        Quad::new(Bounds::new(text_x - 2.0, text_y - 2.0, highlight_width + 4.0, font_size + 6.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.3)),
    );
    let run = text_system.layout(highlight_text, Point::new(text_x, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);

    *y += 150.0;
}

fn demo_font_sizes(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Font Sizes");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 160.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let sizes = [10.0, 12.0, 14.0, 18.0, 24.0, 32.0];
    let mut text_y = *y + 12.0;

    for size in sizes {
        let text = format!("{}px - The quick brown fox", size as i32);
        let run = text_system.layout(&text, Point::new(x + 12.0, text_y), size, theme::text::PRIMARY);
        scene.draw_text(run);
        text_y += size * 1.3 + 4.0;
    }

    *y += 170.0;
}

fn demo_unicode_support(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Unicode Support");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 180.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 14.0;
    let line_height = 26.0;
    let mut text_y = *y + 12.0;

    let samples = [
        ("ASCII", "Hello, World! 123 @#$%"),
        ("Latin Ext", "Bonjour! Guten Tag! Hola!"),
        ("Accents", "cafe resume naive"),
        ("Cyrillic", "Russian letters may not render"),
        ("CJK", "CJK characters need font support"),
        ("Symbols", "Arrows and math symbols"),
    ];

    for (label, text) in samples {
        let label_run = text_system.layout(label, Point::new(x + 12.0, text_y), 11.0, theme::text::MUTED);
        scene.draw_text(label_run);

        let text_run = text_system.layout(text, Point::new(x + 100.0, text_y), font_size, theme::text::PRIMARY);
        scene.draw_text(text_run);

        text_y += line_height;
    }

    *y += 190.0;
}

fn demo_line_wrapper_api(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    _demo: &DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "LineWrapper API");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 200.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 13.0;
    let line_height = 20.0;
    let mut text_y = *y + 12.0;

    // Show word character detection
    let title_run = text_system.layout("Word Character Detection:", Point::new(x + 8.0, text_y), font_size, theme::accent::GREEN);
    scene.draw_text(title_run);
    text_y += line_height;

    let word_chars = ['a', 'Z', '5', 'e', 'u', '-', '_', '.'];
    let non_word = [' ', '/', '(', '[', '+'];

    let mut info = String::from("  Word: ");
    for c in word_chars {
        if LineWrapper::is_word_char(c) {
            info.push_str(&format!("'{}' ", c));
        }
    }
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    let mut info = String::from("  Break at: ");
    for c in non_word {
        if !LineWrapper::is_word_char(c) {
            let display = if c == ' ' { "SPC" } else { &c.to_string() };
            info.push_str(&format!("'{}' ", display));
        }
    }
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height * 1.5;

    // Show wrap boundaries for a sample text
    let title_run = text_system.layout("Wrap Boundaries Demo:", Point::new(x + 8.0, text_y), font_size, theme::accent::GREEN);
    scene.draw_text(title_run);
    text_y += line_height;

    let sample = "Hello world this wraps at boundaries";
    let char_width = 12.0 * 0.6;
    let mut wrapper = LineWrapper::new_monospace(0, 12.0, char_width);

    let fragments = [LineFragment::text(sample)];
    let boundaries: Vec<_> = wrapper.wrap_line(&fragments, 120.0).collect();

    let info = format!(
        "  \"{}\" @ 120px = {} boundaries",
        sample,
        boundaries.len()
    );
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 11.0, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    for (i, b) in boundaries.iter().enumerate() {
        let info = format!("    [{}] ix={}, indent={}", i, b.ix, b.next_indent);
        let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
        scene.draw_text(run);
        text_y += line_height;
    }

    *y += 210.0;
}

fn demo_truncation(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Text Truncation");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 140.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 13.0;
    let char_width = font_size * 0.6;
    let line_height = 24.0;
    let mut text_y = *y + 12.0;

    let original = "This is a very long piece of text that needs truncation";
    let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);

    // Original
    let run = text_system.layout("Original:", Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
    scene.draw_text(run);
    let run = text_system.layout(original, Point::new(x + 80.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    // Truncate from end
    let truncated_end = wrapper.truncate_line(original, 200.0, "...", TruncateFrom::End);
    let run = text_system.layout("End:", Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
    scene.draw_text(run);
    let run = text_system.layout(&truncated_end, Point::new(x + 80.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    // Truncate from start
    let truncated_start = wrapper.truncate_line(original, 200.0, "...", TruncateFrom::Start);
    let run = text_system.layout("Start:", Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
    scene.draw_text(run);
    let run = text_system.layout(&truncated_start, Point::new(x + 80.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    // Different widths
    let widths = [150.0, 180.0, 220.0];
    for w in widths {
        let t = wrapper.truncate_line(original, w, "...", TruncateFrom::End);
        let label = format!("{:.0}px:", w);
        let run = text_system.layout(&label, Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
        scene.draw_text(run);
        let run = text_system.layout(&t, Point::new(x + 80.0, text_y), font_size, theme::text::PRIMARY);
        scene.draw_text(run);
        text_y += line_height;
    }

    *y += 150.0;
}

fn demo_line_layout_api(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "LineLayout API");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 160.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 14.0;
    let line_height = 22.0;
    let mut text_y = *y + 12.0;

    // Create a sample LineLayout
    let sample_text = "Hello, World!";
    let char_width = font_size * 0.6;

    // Build a mock layout for demonstration
    let glyphs: Vec<ShapedGlyph> = sample_text
        .char_indices()
        .map(|(i, _c)| ShapedGlyph {
            id: i as u16,
            position: Point::new(i as f32 * char_width, 0.0),
            index: i,
            is_emoji: false,
        })
        .collect();

    let run = ShapedRun {
        font_id: 0,
        glyphs,
    };

    let layout = LineLayout {
        font_size,
        width: sample_text.len() as f32 * char_width,
        ascent: font_size * 0.8,
        descent: font_size * 0.2,
        runs: vec![run],
        len: sample_text.len(),
    };

    // Display layout info
    let info = format!(
        "Text: \"{}\" ({} chars)",
        sample_text,
        layout.len
    );
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    let info = format!(
        "Width: {:.1}px, Ascent: {:.1}, Descent: {:.1}",
        layout.width, layout.ascent, layout.descent
    );
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    // Index queries
    let test_x_values = [0.0, 30.0, 60.0, 100.0];
    let title = text_system.layout("Position queries:", Point::new(x + 8.0, text_y), 12.0, theme::accent::GREEN);
    scene.draw_text(title);
    text_y += line_height;

    for test_x in test_x_values {
        let idx = layout.index_for_x(test_x);
        let closest = layout.closest_index_for_x(test_x);
        let info = format!("  x={:.0} -> index={:?}, closest={}", test_x, idx, closest);
        let run = text_system.layout(&info, Point::new(x + 8.0, text_y), 11.0, theme::text::MUTED);
        scene.draw_text(run);
        text_y += line_height;
    }

    *y += 170.0;
}

fn demo_cache_stats(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "Layout Cache");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 12.0;
    let line_height = 20.0;
    let mut text_y = *y + 12.0;

    // Simulate some cache activity
    let font_runs = vec![FontRun { len: 10, font_id: 0 }];
    let _ = demo.layout_cache.layout_line("test text", 14.0, &font_runs, |_text, _size, _runs| {
        LineLayout {
            font_size: 14.0,
            width: 100.0,
            ascent: 12.0,
            descent: 3.0,
            runs: vec![],
            len: 9,
        }
    });

    let stats = demo.layout_cache.stats();

    let info = format!("Current lines: {}", stats.current_lines);
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    let info = format!("Current wrapped: {}", stats.current_wrapped_lines);
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);
    text_y += line_height;

    let info = format!("Previous lines: {}", stats.previous_lines);
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), font_size, theme::accent::GREEN);
    scene.draw_text(run);
    text_y += line_height;

    let info = format!("Previous wrapped: {}", stats.previous_wrapped_lines);
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), font_size, theme::accent::GREEN);
    scene.draw_text(run);
    text_y += line_height;

    let total = stats.current_lines + stats.current_wrapped_lines
              + stats.previous_lines + stats.previous_wrapped_lines;
    let info = format!("Total cached: {}", total);
    let run = text_system.layout(&info, Point::new(x + 8.0, text_y), font_size, theme::text::PRIMARY);
    scene.draw_text(run);

    *y += 130.0;
}

fn demo_svg_rendering(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_header(scene, text_system, x, y, "SVG Rendering");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 180.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let line_height = 20.0;
    let mut text_y = *y + 12.0;

    // Simple SVG icons embedded as bytes (using rgb() for colors to avoid raw string issues)
    let circle_svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><circle cx=\"50\" cy=\"50\" r=\"45\" fill=\"rgb(255,180,0)\" stroke=\"rgb(255,140,0)\" stroke-width=\"4\"/></svg>";

    let checkmark_svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><polyline points=\"20,50 40,70 80,30\" fill=\"none\" stroke=\"rgb(0,230,118)\" stroke-width=\"10\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/></svg>";

    let star_svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><polygon points=\"50,5 61,40 98,40 68,62 79,97 50,75 21,97 32,62 2,40 39,40\" fill=\"rgb(255,215,0)\" stroke=\"rgb(255,165,0)\" stroke-width=\"2\"/></svg>";

    let arrow_svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\"><polygon points=\"50,10 90,90 50,70 10,90\" fill=\"rgb(33,150,243)\"/></svg>";

    // Title
    let title_run = text_system.layout("SVG Icons at different sizes:", Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(title_run);
    text_y += line_height + 8.0;

    // Draw SVGs at different sizes
    let sizes = [16.0, 24.0, 32.0, 48.0];
    let svgs = [
        (circle_svg.as_slice(), "Circle"),
        (checkmark_svg.as_slice(), "Check"),
        (star_svg.as_slice(), "Star"),
        (arrow_svg.as_slice(), "Arrow"),
    ];

    let mut svg_x = x + 12.0;
    for (svg_data, _label) in svgs.iter() {
        for (i, &size) in sizes.iter().enumerate() {
            scene.draw_svg(SvgQuad {
                bounds: Bounds::new(svg_x + (i as f32 * (size + 8.0)), text_y, size, size),
                svg_data: Arc::from(*svg_data),
                tint: None,
            });
        }
        svg_x += 180.0;
    }

    text_y += 60.0;

    // Draw with tint colors
    let tint_title = text_system.layout("With color tinting:", Point::new(x + 8.0, text_y), 12.0, theme::text::PRIMARY);
    scene.draw_text(tint_title);
    text_y += line_height + 8.0;

    let tint_colors = [
        Some(theme::accent::PRIMARY),
        Some(theme::accent::GREEN),
        Some(theme::accent::RED),
        Some(theme::accent::BLUE),
    ];

    for (i, tint) in tint_colors.iter().enumerate() {
        scene.draw_svg(SvgQuad {
            bounds: Bounds::new(x + 12.0 + (i as f32 * 48.0), text_y, 32.0, 32.0),
            svg_data: Arc::from(star_svg.as_slice()),
            tint: *tint,
        });
    }

    text_y += 48.0;

    let info_run = text_system.layout("SVGs are rasterized to RGBA and cached by content hash + size", Point::new(x + 8.0, text_y), 10.0, theme::text::MUTED);
    scene.draw_text(info_run);

    *y += 190.0;
}

fn demo_streaming_markdown(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: f32,
) {
    // DEBUG: Test consecutive span rendering with background boxes
    let debug_y = y + 14.0;
    let font_size = 14.0;

    // Simple test: render "AAAA" with background showing measured width
    let text_run = text_system.layout_styled("AAAA", Point::new(x, debug_y), font_size, theme::text::PRIMARY, wgpui::FontStyle::normal());

    // Draw background for expected width
    let expected_width = text_system.measure("AAAA", font_size);
    scene.draw_quad(Quad::new(Bounds::new(x, debug_y - 2.0, expected_width, font_size + 4.0)).with_background(theme::accent::RED.with_alpha(0.3)));

    scene.draw_text(text_run);

    // Also show measured width
    let debug_info = format!("AAAA measured={:.1}px", expected_width);
    let debug_run = text_system.layout(&debug_info, Point::new(x, debug_y + 20.0), 11.0, theme::text::MUTED);
    scene.draw_text(debug_run);

    // Section header
    let title_run = text_system.layout(
        "Streaming Markdown",
        Point::new(x, y + 54.0),
        16.0,
        theme::accent::PRIMARY,
    );
    scene.draw_text(title_run);
    scene.draw_quad(
        Quad::new(Bounds::new(x, y + 72.0, 200.0, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.4)),
    );

    let content_y = y + 84.0;

    // Get the markdown document and measure its actual height
    let document = demo.streaming_markdown.document();
    let md_width = width - 24.0;
    let md_size = demo.markdown_renderer.measure(document, md_width, text_system);

    // Status bar height + padding + markdown content + bottom padding
    let status_height = 20.0;
    let padding = 12.0;
    let content_height = (status_height + padding + md_size.height + padding).max(60.0);

    // Use Frame for the markdown container
    let frame_bounds = Bounds::new(x, content_y, width, content_height);
    let mut frame = Frame::nefrex()
        .line_color(theme::accent::PRIMARY.with_alpha(0.6))
        .bg_color(theme::bg::SURFACE.with_alpha(0.2))
        .stroke_width(1.0)
        .corner_length(16.0)
        .corner_config(CornerConfig::diagonal())
        .animation_progress(1.0);

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    frame.paint(frame_bounds, &mut cx);

    // Status info
    let status = if demo.markdown_tokens.is_empty() {
        if document.is_complete {
            "Complete - Press R to restart"
        } else {
            "Streaming..."
        }
    } else {
        "Streaming..."
    };

    let tokens_remaining = demo.markdown_tokens.len();
    let chars_rendered = demo.streaming_markdown.source().len();
    let is_streaming = demo.streaming_markdown.fade_state().is_streaming;

    let status_text = format!(
        "{} | {} chars | {} tokens left | streaming: {}",
        status, chars_rendered, tokens_remaining, is_streaming
    );
    let status_run = text_system.layout(
        &status_text,
        Point::new(x + 8.0, content_y + 8.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(status_run);

    // Render the markdown document
    let md_origin = Point::new(x + 12.0, content_y + 28.0);

    demo.markdown_renderer.render(
        document,
        md_origin,
        md_width,
        text_system,
        scene,
    );

    // Streaming cursor indicator removed - the rough heuristic for cursor position
    // doesn't work well with markdown's varying line heights (headers, code blocks, etc.)
    // A proper implementation would need the markdown renderer to track cursor position.
}
