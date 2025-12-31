#![cfg(target_arch = "wasm32")]
//! OpenAgents Web - WGPUI Text System Demo
//!
//! A lightweight web demo showcasing GPU-accelerated text rendering.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wgpui::{
    Boundary, Bounds, Cursor, EventContext, EventResult, InputEvent, LineFragment, LineWrapper,
    MarkdownDocument, MarkdownView, MouseButton, PaintContext, Platform, Point, Quad, Scene,
    StreamingMarkdown, TextSystem, TruncateFrom, WebPlatform, run_animation_loop,
    setup_resize_observer, theme,
};

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(&"WGPUI Web Demo initialized".into());
}

fn demo_markdown_source() -> String {
    let source = include_str!("../docs/README.md");
    source
        .lines()
        .take(140)
        .collect::<Vec<_>>()
        .join("\n")
}

fn tokenize_markdown(source: &str) -> VecDeque<String> {
    source
        .chars()
        .collect::<Vec<_>>()
        .chunks(3)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn copy_to_clipboard_web(text: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let promise = window.navigator().clipboard().write_text(&text);
    spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}

struct DemoState {
    frame_count: u64,
    mouse_pos: Point,
    scroll_offset: f32,
    wrap_width: f32,
    // Streaming markdown
    streaming_markdown: StreamingMarkdown,
    markdown_view: MarkdownView,
    markdown_tokens: VecDeque<String>,
    markdown_started: bool,
    last_token_frame: u64,
    markdown_source: String,
    markdown_bounds: Bounds,
    markdown_events: EventContext,
}

impl Default for DemoState {
    fn default() -> Self {
        let markdown_source = demo_markdown_source();
        let tokens = tokenize_markdown(&markdown_source);
        let markdown_view = MarkdownView::new(MarkdownDocument::new())
            .copy_button_on_hover(true)
            .on_copy(|text| copy_to_clipboard_web(text));

        Self {
            frame_count: 0,
            mouse_pos: Point::ZERO,
            scroll_offset: 0.0,
            wrap_width: 400.0,
            streaming_markdown: StreamingMarkdown::new(),
            markdown_view,
            markdown_tokens: tokens,
            markdown_started: true,
            last_token_frame: 0,
            markdown_source,
            markdown_bounds: Bounds::ZERO,
            markdown_events: EventContext::new(),
        }
    }
}

impl DemoState {
    fn advance_markdown_stream(&mut self) {
        if !self.markdown_started {
            return;
        }

        let frames_since_token = self.frame_count - self.last_token_frame;
        if frames_since_token >= 2 && !self.markdown_tokens.is_empty() {
            if let Some(token) = self.markdown_tokens.pop_front() {
                self.streaming_markdown.append(&token);
                self.last_token_frame = self.frame_count;
            }
        }

        if self.markdown_tokens.is_empty() && self.streaming_markdown.has_pending() {
            self.streaming_markdown.complete();
        }

        self.streaming_markdown.tick();
    }

    fn restart_stream(&mut self) {
        self.markdown_tokens = tokenize_markdown(&self.markdown_source);

        self.streaming_markdown.reset();
        self.markdown_started = true;
        self.last_token_frame = self.frame_count;
    }

    fn handle_input_event(&mut self, event: InputEvent) -> EventResult {
        self.markdown_view
            .event(&event, self.markdown_bounds, &mut self.markdown_events)
    }

    fn clear_markdown_hover(&mut self) {
        self.markdown_view.clear_hover();
    }

    fn markdown_cursor(&self) -> Cursor {
        self.markdown_view.cursor()
    }
}

#[wasm_bindgen]
pub async fn start_demo(canvas_id: &str) -> Result<(), JsValue> {
    let platform = WebPlatform::init(canvas_id)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    let platform = Rc::new(RefCell::new(platform));
    let demo = Rc::new(RefCell::new(DemoState::default()));

    // Set up resize observer
    {
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        setup_resize_observer(&canvas, move || {
            platform_clone.borrow_mut().handle_resize();
        });
    }

    // Set up keyboard events
    {
        let demo_clone = demo.clone();
        let window = web_sys::window().unwrap();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::KeyboardEvent| {
            let key = event.key();
            let mut demo = demo_clone.borrow_mut();

            match key.as_str() {
                "r" | "R" => demo.restart_stream(),
                "-" => demo.wrap_width = (demo.wrap_width - 20.0).max(200.0),
                "=" | "+" => demo.wrap_width = (demo.wrap_width + 20.0).min(800.0),
                _ => {}
            }
        });
        window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Set up mouse events
    {
        let demo_clone = demo.clone();
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let cursor = {
                let mut demo = demo_clone.borrow_mut();
                demo.mouse_pos = Point::new(x, y);
                let _ = demo.handle_input_event(InputEvent::MouseMove { x, y });
                demo.markdown_cursor()
            };
            platform_clone.borrow().set_cursor(cursor);
        });
        canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let demo_clone = demo.clone();
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MouseEvent| {
            let button = match event.button() {
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                _ => MouseButton::Left,
            };
            let x = event.offset_x() as f32;
            let y = event.offset_y() as f32;
            let cursor = {
                let mut demo = demo_clone.borrow_mut();
                let _ = demo.handle_input_event(InputEvent::MouseDown { button, x, y });
                demo.markdown_cursor()
            };
            platform_clone.borrow().set_cursor(cursor);
        });
        canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    {
        let demo_clone = demo.clone();
        let platform_clone = platform.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            let cursor = {
                let mut demo = demo_clone.borrow_mut();
                demo.clear_markdown_hover();
                demo.markdown_cursor()
            };
            platform_clone.borrow().set_cursor(cursor);
        });
        canvas.add_event_listener_with_callback("mouseleave", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Set up wheel events
    {
        let demo_clone = demo.clone();
        let canvas = platform.borrow().canvas().clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::WheelEvent| {
            let mut demo = demo_clone.borrow_mut();
            demo.scroll_offset = (demo.scroll_offset + event.delta_y() as f32 * 0.5).max(0.0);
            event.prevent_default();
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // Animation loop
    run_animation_loop(move || {
        let mut platform = platform.borrow_mut();
        let mut demo = demo.borrow_mut();

        demo.frame_count += 1;
        demo.advance_markdown_stream();

        let size = platform.logical_size();
        let scale_factor = platform.scale_factor();
        let mut scene = Scene::new();

        build_demo(
            &mut scene,
            platform.text_system(),
            &mut demo,
            size.width,
            size.height,
            scale_factor,
        );

        if let Err(e) = platform.render_scene(&scene) {
            web_sys::console::error_1(&format!("Render error: {}", e).into());
        }
    });

    Ok(())
}

fn build_demo(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    let margin = 20.0;
    let col_width = (width - margin * 3.0) / 2.0;

    // Background
    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let mut y = margin - demo.scroll_offset;

    // Header
    draw_header(scene, text_system, margin, &mut y, width, demo);

    let left_x = margin;
    let right_x = margin * 2.0 + col_width;
    let mut left_y = y;
    let mut right_y = y;

    // Left column
    demo_line_wrapping(scene, text_system, demo, left_x, col_width, &mut left_y);
    left_y += 20.0;
    demo_font_sizes(scene, text_system, left_x, col_width, &mut left_y);
    left_y += 20.0;
    demo_unicode_support(scene, text_system, left_x, col_width, &mut left_y);

    // Right column
    demo_truncation(scene, text_system, right_x, col_width, &mut right_y);
    right_y += 20.0;
    demo_text_decorations(scene, text_system, right_x, col_width, &mut right_y);

    // Full-width streaming markdown at bottom
    let bottom_y = left_y.max(right_y) + 20.0;
    demo_streaming_markdown(
        scene,
        text_system,
        demo,
        margin,
        width - margin * 2.0,
        bottom_y,
        scale_factor,
    );
}

fn draw_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    margin: f32,
    y: &mut f32,
    width: f32,
    demo: &DemoState,
) {
    let header_bounds = Bounds::new(margin, *y, width - margin * 2.0, 70.0);
    scene.draw_quad(
        Quad::new(header_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::accent::PRIMARY.with_alpha(0.5), 1.0),
    );

    let title = "WGPUI Web Demo";
    let subtitle = format!(
        "Frame {} | Wrap: {}px | Keys: -/= width, R restart",
        demo.frame_count, demo.wrap_width as i32,
    );

    let title_run = text_system.layout(
        title,
        Point::new(margin + 12.0, *y + 20.0),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle_run = text_system.layout(
        &subtitle,
        Point::new(margin + 12.0, *y + 48.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    *y += 82.0;
}

fn draw_section_header(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: &mut f32,
    title: &str,
) {
    let run = text_system.layout(
        title,
        Point::new(x, *y + 12.0),
        14.0,
        theme::accent::PRIMARY,
    );
    scene.draw_text(run);
    scene.draw_quad(
        Quad::new(Bounds::new(x, *y + 28.0, 180.0, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.4)),
    );
    *y += 38.0;
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

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, wrap_width, 160.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(x + wrap_width, *y, 2.0, 160.0))
            .with_background(theme::accent::RED.with_alpha(0.5)),
    );

    let text = "The quick brown fox jumps over the lazy dog. This demonstrates word-wrapping at different widths.";
    let font_size = 13.0;
    let line_height = font_size * 1.4;
    let char_width = font_size * 0.6;

    let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);
    let fragments = [LineFragment::text(text)];
    let boundaries: Vec<Boundary> = wrapper.wrap_line(&fragments, wrap_width - 16.0).collect();

    let mut line_start = 0;
    let mut line_y = *y + 12.0;

    for (i, boundary) in boundaries.iter().enumerate() {
        let line_text = &text[line_start..boundary.ix];
        let indent = if i > 0 {
            boundary.next_indent as f32 * char_width
        } else {
            0.0
        };

        let run = text_system.layout(
            line_text.trim_start(),
            Point::new(x + 8.0 + indent, line_y),
            font_size,
            theme::text::PRIMARY,
        );
        scene.draw_text(run);

        line_start = boundary.ix;
        line_y += line_height;
    }

    let remaining = &text[line_start..];
    if !remaining.is_empty() {
        let run = text_system.layout(
            remaining.trim_start(),
            Point::new(x + 8.0, line_y),
            font_size,
            theme::text::PRIMARY,
        );
        scene.draw_text(run);
    }

    let label = format!("{} wrap boundaries", boundaries.len());
    let label_run = text_system.layout(
        &label,
        Point::new(x + 8.0, *y + 140.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(label_run);

    *y += 170.0;
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
        Quad::new(Bounds::new(x, *y, width, 130.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let sizes = [10.0, 12.0, 14.0, 18.0, 24.0];
    let mut text_y = *y + 10.0;

    for size in sizes {
        let text = format!("{}px - Vera Mono", size as i32);
        let run = text_system.layout(
            &text,
            Point::new(x + 10.0, text_y),
            size,
            theme::text::PRIMARY,
        );
        scene.draw_text(run);
        text_y += size * 1.2 + 4.0;
    }

    *y += 140.0;
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
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 13.0;
    let line_height = 22.0;
    let mut text_y = *y + 10.0;

    let samples = [
        ("ASCII", "Hello, World! 123 @#$%"),
        ("Latin", "Bonjour! Guten Tag!"),
        ("Accents", "cafe resume naive"),
        ("Symbols", "Arrows: -> <- =>"),
    ];

    for (label, text) in samples {
        let label_run = text_system.layout(
            label,
            Point::new(x + 10.0, text_y),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(label_run);

        let text_run = text_system.layout(
            text,
            Point::new(x + 80.0, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        scene.draw_text(text_run);

        text_y += line_height;
    }

    *y += 130.0;
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
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 13.0;
    let char_width = font_size * 0.6;
    let line_height = 22.0;
    let mut text_y = *y + 10.0;

    let original = "This is a very long piece of text that needs truncation";
    let mut wrapper = LineWrapper::new_monospace(0, font_size, char_width);

    let run = text_system.layout(
        "Original:",
        Point::new(x + 8.0, text_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(run);
    let run = text_system.layout(
        original,
        Point::new(x + 70.0, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);
    text_y += line_height;

    let truncated_end = wrapper.truncate_line(original, 180.0, "...", TruncateFrom::End);
    let run = text_system.layout(
        "End:",
        Point::new(x + 8.0, text_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(run);
    let run = text_system.layout(
        &truncated_end,
        Point::new(x + 70.0, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);
    text_y += line_height;

    let truncated_start = wrapper.truncate_line(original, 180.0, "...", TruncateFrom::Start);
    let run = text_system.layout(
        "Start:",
        Point::new(x + 8.0, text_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(run);
    let run = text_system.layout(
        &truncated_start,
        Point::new(x + 70.0, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);
    text_y += line_height;

    let t = wrapper.truncate_line(original, 140.0, "...", TruncateFrom::End);
    let run = text_system.layout(
        "140px:",
        Point::new(x + 8.0, text_y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(run);
    let run = text_system.layout(
        &t,
        Point::new(x + 70.0, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);

    *y += 130.0;
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
        Quad::new(Bounds::new(x, *y, width, 110.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let font_size = 13.0;
    let line_height = 24.0;
    let text_x = x + 10.0;
    let mut text_y = *y + 14.0;

    // Normal
    let run = text_system.layout(
        "Normal text",
        Point::new(text_x, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);
    text_y += line_height;

    // Underline (simulated)
    let underline_text = "Underlined text";
    let run = text_system.layout(
        underline_text,
        Point::new(text_x, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);
    let underline_width = underline_text.len() as f32 * font_size * 0.6;
    scene.draw_quad(
        Quad::new(Bounds::new(
            text_x,
            text_y + font_size + 2.0,
            underline_width,
            1.0,
        ))
        .with_background(theme::accent::PRIMARY),
    );
    text_y += line_height;

    // Highlight
    let highlight_text = "Highlighted text";
    let highlight_width = highlight_text.len() as f32 * font_size * 0.6;
    scene.draw_quad(
        Quad::new(Bounds::new(
            text_x - 2.0,
            text_y - 2.0,
            highlight_width + 4.0,
            font_size + 6.0,
        ))
        .with_background(theme::accent::PRIMARY.with_alpha(0.3)),
    );
    let run = text_system.layout(
        highlight_text,
        Point::new(text_x, text_y),
        font_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(run);

    *y += 120.0;
}

fn demo_streaming_markdown(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    demo: &mut DemoState,
    x: f32,
    width: f32,
    y: f32,
    scale_factor: f32,
) {
    // Section header
    let title_run = text_system.layout(
        "Streaming Markdown",
        Point::new(x, y),
        14.0,
        theme::accent::PRIMARY,
    );
    scene.draw_text(title_run);
    scene.draw_quad(
        Quad::new(Bounds::new(x, y + 16.0, 180.0, 1.0))
            .with_background(theme::accent::PRIMARY.with_alpha(0.4)),
    );

    let content_y = y + 28.0;

    let document = demo.streaming_markdown.document().clone();
    let is_complete = document.is_complete;
    let md_width = width - 20.0;
    demo.markdown_view.set_document(document);
    let md_size = demo.markdown_view.measure(md_width, text_system);

    let content_height = (md_size.height + 50.0).max(100.0);

    scene.draw_quad(
        Quad::new(Bounds::new(x, content_y, width, content_height))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::accent::PRIMARY.with_alpha(0.3), 1.0),
    );

    // Status
    let status = if demo.markdown_tokens.is_empty() {
        if is_complete {
            "Complete - Press R to restart"
        } else {
            "Streaming..."
        }
    } else {
        "Streaming..."
    };

    let status_text = format!(
        "{} | {} chars | {} tokens left",
        status,
        demo.streaming_markdown.source().len(),
        demo.markdown_tokens.len()
    );
    let status_run = text_system.layout(
        &status_text,
        Point::new(x + 8.0, content_y + 8.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(status_run);

    // Render markdown
    let md_bounds = Bounds::new(x + 10.0, content_y + 26.0, md_width, content_height - 34.0);
    demo.markdown_bounds = md_bounds;
    let mut paint_cx = PaintContext::new(scene, text_system, scale_factor);
    demo.markdown_view.paint(md_bounds, &mut paint_cx);
}
