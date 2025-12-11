//! MechaCoder - Claude Code Harness
//!
//! A focused GPUI application for interacting with Claude Code.

use gpui::{AppContext, Application, KeyBinding, WindowOptions, px, size};
use mechacoder::{MechaCoderScreen, Quit, app_menus, init_theme};
use ui_oa::text_input::bind_text_input_keys;

fn main() {
    // Initialize telemetry
    telemetry::init_default("mechacoder");

    Application::new().run(|cx| {
        // Initialize tokio runtime (required for Claude Agent SDK)
        gpui_tokio::init(cx);

        // Initialize theme and settings (required for markdown rendering)
        init_theme(cx);

        // Bind text input keys (backspace, delete, arrows, etc.)
        bind_text_input_keys(cx);

        // Load fonts
        let font_paths = vec![
            // Berkeley Mono fonts
            "assets/fonts/BerkeleyMono/BerkeleyMono-Regular.otf",
            "assets/fonts/BerkeleyMono/BerkeleyMono-Bold.otf",
            "assets/fonts/BerkeleyMono/BerkeleyMono-Italic.otf",
            "assets/fonts/BerkeleyMono/BerkeleyMono-BoldItalic.otf",
        ];

        for path in font_paths {
            if let Ok(font_data) = std::fs::read(path) {
                if let Err(e) = cx.text_system().add_fonts(vec![font_data.into()]) {
                    log::warn!("Failed to load font {}: {}", path, e);
                }
            }
        }

        // Bind global key bindings
        cx.bind_keys([
            // Application
            KeyBinding::new("cmd-q", Quit, None),
            // Messages
            KeyBinding::new("cmd-enter", mechacoder::SendMessage, None),
            KeyBinding::new("escape", mechacoder::CancelGeneration, None),
            // Focus
            KeyBinding::new("cmd-l", mechacoder::FocusMessageInput, None),
        ]);

        // Set application menus
        cx.set_menus(app_menus::app_menus());

        // Register quit handler
        cx.on_action(|_: &Quit, cx| cx.quit());

        // Open main window
        let window_options = WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds {
                origin: gpui::Point::default(),
                size: size(px(1200.0), px(800.0)),
            })),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("MechaCoder".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.open_window(window_options, |_window, cx| {
            cx.new(|cx| MechaCoderScreen::new(cx))
        })
        .expect("Failed to open window");
    });
}
