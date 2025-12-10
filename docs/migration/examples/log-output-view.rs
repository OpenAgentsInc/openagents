/// Log Output Component - GPUI Implementation
///
/// Displays streaming log output with auto-scroll and filtering.
/// Uses GPUI's virtualized list for performance with large logs.
///
/// Key conversions:
/// - Log lines → Vec<LogLine>
/// - Virtualized rendering → uniform_list()
/// - ANSI color codes → GPUI colors
/// - Auto-scroll → scroll position tracking

use gpui::*;

// ============================================================================
// Types
// ============================================================================

#[derive(Clone, Debug)]
pub struct LogLine {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
    Success,
}

impl LogLevel {
    fn color(&self) -> Hsla {
        match self {
            LogLevel::Debug => rgb(0x737373),   // zinc-500
            LogLevel::Info => rgb(0x60a5fa),    // blue-400
            LogLevel::Warn => rgb(0xfbbf24),    // yellow-400
            LogLevel::Error => rgb(0xf87171),   // red-400
            LogLevel::Success => rgb(0x34d399), // emerald-400
        }
    }

    fn label(&self) -> &str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Success => "OK",
        }
    }
}

#[derive(Clone)]
pub struct LogOutputState {
    pub lines: Vec<LogLine>,
    pub auto_scroll: bool,
    pub filter: String,
    pub max_lines: usize,
}

impl Default for LogOutputState {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            auto_scroll: true,
            filter: String::new(),
            max_lines: 10000,
        }
    }
}

// ============================================================================
// View
// ============================================================================

pub struct LogOutputView {
    state: Entity<LogOutputState>,
    scroll_handle: ScrollHandle,
}

impl LogOutputView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| LogOutputState::default());
        let scroll_handle = ScrollHandle::new();

        Self {
            state,
            scroll_handle,
        }
    }

    /// Add a log line
    pub fn add_line(&mut self, line: LogLine, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.lines.push(line);

            // Trim to max lines
            if state.lines.len() > state.max_lines {
                state.lines.drain(0..state.lines.len() - state.max_lines);
            }

            cx.notify();
        });

        // Auto-scroll to bottom
        let state = self.state.read(cx);
        if state.auto_scroll {
            // In real implementation, scroll to bottom
            // self.scroll_handle.scroll_to_bottom(cx);
        }
    }

    /// Clear all logs
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.lines.clear();
            cx.notify();
        });
    }

    /// Render header controls
    fn render_header(&self, state: &LogOutputState, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(rgba(0x27272a, 0.6))
            .bg(rgba(0x18181b, 0.4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_color(rgb(0xfafafa))
                            .font_weight(FontWeight::BOLD)
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text_size(px(14.0))
                            .text(format!("Logs ({} lines)", state.lines.len()))
                    )
                    .child(
                        div()
                            .text_color(rgb(0x737373))
                            .text_size(px(12.0))
                            .text("|")
                    )
                    .child(
                        div()
                            .text_color(if state.auto_scroll { rgb(0x34d399) } else { rgb(0x737373) })
                            .text_size(px(12.0))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.state.update(cx, |state, cx| {
                                    state.auto_scroll = !state.auto_scroll;
                                    cx.notify();
                                });
                            }))
                            .text(if state.auto_scroll { "Auto-scroll: ON" } else { "Auto-scroll: OFF" })
                    )
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(
                        div()
                            .px(px(12.0))
                            .py(px(6.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0xfafafa))
                            .bg(rgba(0x27272a, 0.6))
                            .border_1()
                            .border_color(rgb(0x3f3f46))
                            .rounded(px(4.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgba(0x27272a, 0.8)))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.clear(cx);
                            }))
                            .text("Clear")
                    )
            )
    }

    /// Render a single log line
    fn render_log_line(line: &LogLine) -> Div {
        div()
            .flex()
            .gap(px(12.0))
            .px(px(16.0))
            .py(px(4.0))
            .font_family(".AppleSystemUIFontMonospaced")
            .text_size(px(12.0))
            .hover(|style| style.bg(rgba(0x27272a, 0.2)))
            .child(
                div()
                    .text_color(rgb(0x737373))
                    .text(&line.timestamp)
            )
            .child(
                div()
                    .w(px(60.0))
                    .text_color(line.level.color())
                    .font_weight(FontWeight::BOLD)
                    .text(line.level.label())
            )
            .child(
                div()
                    .flex_1()
                    .text_color(rgb(0xe4e4e7))
                    .text(&line.message)
            )
    }

    /// Get filtered lines
    fn get_filtered_lines(&self, state: &LogOutputState) -> Vec<LogLine> {
        if state.filter.is_empty() {
            state.lines.clone()
        } else {
            state.lines
                .iter()
                .filter(|line| {
                    line.message.contains(&state.filter) || line.raw.contains(&state.filter)
                })
                .cloned()
                .collect()
        }
    }
}

impl Render for LogOutputView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let filtered_lines = self.get_filtered_lines(&state);

        div()
            .flex()
            .flex_col()
            .rounded(px(16.0))
            .border_1()
            .border_color(rgba(0x27272a, 0.6))
            .bg(rgba(0x09090b, 0.8))
            .shadow_xl()
            .h_full()
            .overflow_hidden()
            // Header
            .child(self.render_header(&state, cx))
            // Log content
            .child(
                div()
                    .flex_1()
                    .overflow_y_auto()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        // Use uniform_list for virtualized rendering
                        // For simplicity, rendering all lines here
                        // In production, use uniform_list() for better performance
                        {
                            let mut container = div().flex().flex_col();
                            for line in &filtered_lines {
                                container = container.child(Self::render_log_line(line));
                            }
                            container
                        }
                    )
            )
            // Footer with stats
            .child(
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(rgba(0x27272a, 0.6))
                    .bg(rgba(0x18181b, 0.4))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x737373))
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(if state.filter.is_empty() {
                                format!("{} total lines", state.lines.len())
                            } else {
                                format!(
                                    "{} filtered / {} total",
                                    filtered_lines.len(),
                                    state.lines.len()
                                )
                            })
                    )
            )
    }
}

// ============================================================================
// Virtualized List Implementation (Production-Ready)
// ============================================================================

/// Production version using GPUI's virtualized list
impl LogOutputView {
    /// Render using virtualized list for better performance
    #[allow(dead_code)]
    fn render_virtualized(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let filtered_lines = self.get_filtered_lines(&state);
        let line_count = filtered_lines.len();

        div()
            .flex()
            .flex_col()
            .rounded(px(16.0))
            .border_1()
            .border_color(rgba(0x27272a, 0.6))
            .bg(rgba(0x09090b, 0.8))
            .h_full()
            .child(self.render_header(&state, cx))
            .child(
                div()
                    .flex_1()
                    .overflow_y_auto()
                    .child(
                        // Virtualized list - only renders visible items
                        uniform_list(
                            cx.view().clone(),
                            "log-lines",
                            line_count,
                            |_cx, range| {
                                range
                                    .map(|i| {
                                        Self::render_log_line(&filtered_lines[i])
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(&self.scroll_handle)
                    )
            )
    }
}

// ============================================================================
// Integration Example (WebSocket log stream)
// ============================================================================

impl LogOutputView {
    /// Subscribe to log stream from WebSocket
    pub fn subscribe_to_logs(&mut self, cx: &mut Context<Self>) {
        // Spawn async task to receive log lines
        cx.spawn(|this, mut cx| async move {
            // In real code: let mut stream = websocket.subscribe_logs().await;
            loop {
                // In real code: let line = stream.recv().await;
                // For now, just a placeholder
                // let line = LogLine {
                //     timestamp: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
                //     level: LogLevel::Info,
                //     message: "Log message here".to_string(),
                //     raw: "Raw log line".to_string(),
                // };

                // this.update(&mut cx, |this, cx| {
                //     this.add_line(line, cx);
                // }).ok();
            }
        })
        .detach();
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod example {
    use super::*;

    fn example_usage() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(WindowOptions::default(), |_, cx| {
                let mut view = cx.new(LogOutputView::new);

                // Add some sample log lines
                view.update(cx, |view, cx| {
                    view.add_line(
                        LogLine {
                            timestamp: "12:00:00.000".to_string(),
                            level: LogLevel::Info,
                            message: "Application started".to_string(),
                            raw: "[INFO] Application started".to_string(),
                        },
                        cx,
                    );

                    view.add_line(
                        LogLine {
                            timestamp: "12:00:01.234".to_string(),
                            level: LogLevel::Success,
                            message: "Connected to server".to_string(),
                            raw: "[OK] Connected to server".to_string(),
                        },
                        cx,
                    );

                    view.add_line(
                        LogLine {
                            timestamp: "12:00:02.456".to_string(),
                            level: LogLevel::Warn,
                            message: "High memory usage detected".to_string(),
                            raw: "[WARN] High memory usage detected".to_string(),
                        },
                        cx,
                    );

                    // Subscribe to log stream
                    view.subscribe_to_logs(cx);
                });

                view
            })
            .ok();
        });
    }
}
