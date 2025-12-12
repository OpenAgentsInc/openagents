//! Preview panel component - Live app preview

use gpui::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

/// Render the preview panel showing the live app
pub fn render_preview_panel() -> impl IntoElement {
    div()
        .id("preview-panel")
        .w(px(400.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .h(px(36.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(12.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("PREVIEW"),
                        )
                        .child(
                            div()
                                .w(px(6.0))
                                .h(px(6.0))
                                
                                .bg(status::SUCCESS),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        // Device switcher
                        .child(render_device_button("D", true))
                        .child(render_device_button("T", false))
                        .child(render_device_button("M", false))
                        // Refresh
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("R"),
                        )
                        // Open in new tab
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .cursor_pointer()
                                .hover(|s| s.text_color(text::PRIMARY))
                                .child("^"),
                        ),
                ),
        )
        // URL bar
        .child(
            div()
                .h(px(28.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(12.0))
                .bg(bg::ELEVATED)
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("localhost:5173"),
                ),
        )
        // Preview content (mock app UI)
        .child(
            div()
                .flex_1()
                .bg(Hsla { h: 0.0, s: 0.0, l: 0.98, a: 1.0 }) // Light background for preview
                .overflow_hidden()
                .child(render_mock_app_preview()),
        )
        // Console output footer
        .child(
            div()
                .h(px(80.0))
                .w_full()
                .flex()
                .flex_col()
                .bg(bg::APP)
                .border_t_1()
                .border_color(border::DEFAULT)
                // Console header
                .child(
                    div()
                        .h(px(24.0))
                        .w_full()
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .border_b_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("CONSOLE"),
                        ),
                )
                // Console messages
                .child(
                    div()
                        .id("preview-console")
                        .flex_1()
                        .p(px(8.0))
                        .overflow_y_scroll()
                        .child(render_console_message("info", "[HMR] Connected"))
                        .child(render_console_message("info", "[vite] page reload src/App.tsx")),
                ),
        )
}

/// Render a device button (Desktop/Tablet/Mobile)
fn render_device_button(label: &str, is_active: bool) -> impl IntoElement {
    let label = label.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .px(px(6.0))
        .py(px(2.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(label),
        )
}

/// Render a console message
fn render_console_message(level: &str, message: &str) -> impl IntoElement {
    let message = message.to_string();
    let color = match level {
        "error" => status::ERROR,
        "warn" => status::WARNING,
        _ => text::MUTED,
    };

    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .py(px(2.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(format!("[{}]", level)),
        )
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(message),
        )
}

/// Render a mock app preview (simplified representation)
fn render_mock_app_preview() -> impl IntoElement {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        // Mock header
        .child(
            div()
                .h(px(48.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 })
                .border_b_1()
                .border_color(Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 })
                // Logo
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .w(px(24.0))
                                .h(px(24.0))
                                
                                .bg(Hsla { h: 0.6, s: 0.7, l: 0.5, a: 1.0 }),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Hsla { h: 0.0, s: 0.0, l: 0.1, a: 1.0 })
                                .child("MyApp"),
                        ),
                )
                // Nav
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(render_mock_nav_item("Dashboard", true))
                        .child(render_mock_nav_item("Settings", false))
                        .child(render_mock_nav_item("Profile", false)),
                )
                // User avatar
                .child(
                    div()
                        .w(px(32.0))
                        .h(px(32.0))
                        
                        .bg(Hsla { h: 0.3, s: 0.5, l: 0.6, a: 1.0 }),
                ),
        )
        // Mock content area
        .child(
            div()
                .flex_1()
                .flex()
                // Sidebar
                .child(
                    div()
                        .w(px(56.0))
                        .h_full()
                        .bg(Hsla { h: 0.0, s: 0.0, l: 0.95, a: 1.0 })
                        .border_r_1()
                        .border_color(Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 })
                        .p(px(8.0))
                        .gap(px(8.0))
                        .flex()
                        .flex_col()
                        .child(render_mock_sidebar_item(true))
                        .child(render_mock_sidebar_item(false))
                        .child(render_mock_sidebar_item(false))
                        .child(render_mock_sidebar_item(false)),
                )
                // Main content
                .child(
                    div()
                        .flex_1()
                        .p(px(16.0))
                        // Dashboard cards
                        .child(
                            div()
                                .flex()
                                .gap(px(12.0))
                                .mb(px(16.0))
                                .child(render_mock_stat_card("Users", "1,247", "+12%"))
                                .child(render_mock_stat_card("Revenue", "$45.2K", "+8%"))
                                .child(render_mock_stat_card("Orders", "892", "+23%")),
                        )
                        // Chart placeholder
                        .child(
                            div()
                                .w_full()
                                .h(px(120.0))
                                
                                .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 })
                                .border_1()
                                .border_color(Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 })
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .text_color(Hsla { h: 0.0, s: 0.0, l: 0.6, a: 1.0 })
                                        .child("Chart Visualization"),
                                ),
                        ),
                ),
        )
}

fn render_mock_nav_item(label: &str, active: bool) -> impl IntoElement {
    let label = label.to_string();
    let color = if active {
        Hsla { h: 0.6, s: 0.7, l: 0.5, a: 1.0 }
    } else {
        Hsla { h: 0.0, s: 0.0, l: 0.4, a: 1.0 }
    };

    div()
        .text_size(px(12.0))
        .text_color(color)
        .child(label)
}

fn render_mock_sidebar_item(active: bool) -> impl IntoElement {
    let bg_color = if active {
        Hsla { h: 0.6, s: 0.7, l: 0.9, a: 1.0 }
    } else {
        Hsla::transparent_black()
    };

    div()
        .w(px(40.0))
        .h(px(40.0))
        
        .bg(bg_color)
        .border_1()
        .border_color(if active {
            Hsla { h: 0.6, s: 0.7, l: 0.5, a: 1.0 }
        } else {
            Hsla { h: 0.0, s: 0.0, l: 0.85, a: 1.0 }
        })
}

fn render_mock_stat_card(label: &str, value: &str, change: &str) -> impl IntoElement {
    let label = label.to_string();
    let value = value.to_string();
    let change = change.to_string();
    div()
        .flex_1()
        .p(px(12.0))
        
        .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: 1.0 })
        .border_1()
        .border_color(Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 })
        .child(
            div()
                .text_size(px(10.0))
                .text_color(Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 })
                .mb(px(4.0))
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Hsla { h: 0.0, s: 0.0, l: 0.1, a: 1.0 })
                        .child(value),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(Hsla { h: 0.35, s: 0.7, l: 0.4, a: 1.0 })
                        .child(change),
                ),
        )
}
