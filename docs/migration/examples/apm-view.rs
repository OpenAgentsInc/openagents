/// APM Widget - GPUI Implementation
///
/// This is a standalone example showing how to port the Effuse APM Component to GPUI.
///
/// Key conversions:
/// - Effuse StateCell → GPUI Entity
/// - HTML template → element builders
/// - Event delegation → direct handlers
/// - WebSocket subscription → async spawn

use gpui::*;
use std::sync::Arc;

// ============================================================================
// State
// ============================================================================

#[derive(Clone)]
pub struct APMState {
    pub session_apm: f64,
    pub recent_apm: f64,
    pub total_actions: u32,
    pub duration_minutes: f64,
    pub apm_1h: f64,
    pub apm_6h: f64,
    pub apm_1d: f64,
    pub apm_lifetime: f64,
    pub claude_code_apm: f64,
    pub mechacoder_apm: f64,
    pub efficiency_ratio: f64,
    pub expanded: bool,
}

impl Default for APMState {
    fn default() -> Self {
        Self {
            session_apm: 0.0,
            recent_apm: 0.0,
            total_actions: 0,
            duration_minutes: 0.0,
            apm_1h: 0.0,
            apm_6h: 0.0,
            apm_1d: 0.0,
            apm_lifetime: 0.0,
            claude_code_apm: 0.0,
            mechacoder_apm: 0.0,
            efficiency_ratio: 0.0,
            expanded: false,
        }
    }
}

// ============================================================================
// View
// ============================================================================

pub struct APMView {
    state: Entity<APMState>,
}

impl APMView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|_cx| APMState::default());
        Self { state }
    }

    /// Get APM color based on value
    fn get_apm_color(apm: f64) -> Hsla {
        if apm >= 30.0 {
            // Emerald for high activity
            rgb(0x34d399)
        } else if apm >= 15.0 {
            // White for good
            rgb(0xe5e5e5)
        } else if apm >= 5.0 {
            // Gray for active
            rgb(0xa1a1a1)
        } else {
            // Darker gray for baseline
            rgb(0x737373)
        }
    }

    /// Get background color based on APM
    fn get_apm_bg(apm: f64) -> Hsla {
        if apm >= 30.0 {
            rgba(0x065f46, 0.2) // emerald-950/20
        } else if apm >= 15.0 {
            rgba(0x18181b, 0.6) // zinc-900/60
        } else if apm >= 5.0 {
            rgba(0x18181b, 0.4) // zinc-900/40
        } else {
            rgba(0x09090b, 0.4) // zinc-950/40
        }
    }

    /// Get border color based on APM
    fn get_apm_border(apm: f64) -> Hsla {
        if apm >= 30.0 {
            rgba(0x166534, 0.4) // emerald-800/40
        } else if apm >= 15.0 {
            rgba(0x3f3f46, 0.5) // zinc-700/50
        } else if apm >= 5.0 {
            rgba(0x3f3f46, 0.4) // zinc-700/40
        } else {
            rgba(0x27272a, 0.3) // zinc-800/30
        }
    }

    /// Format duration nicely
    fn format_duration(minutes: f64) -> String {
        if minutes < 60.0 {
            format!("{}m", minutes.floor())
        } else {
            let hours = (minutes / 60.0).floor();
            let mins = (minutes % 60.0).round();
            if mins > 0.0 {
                format!("{}h {}m", hours, mins)
            } else {
                format!("{}h", hours)
            }
        }
    }

    /// Render compact view
    fn render_compact(&self, state: &APMState, cx: &mut Context<Self>) -> Div {
        let color = Self::get_apm_color(state.session_apm);
        let bg = Self::get_apm_bg(state.session_apm);
        let border = Self::get_apm_border(state.session_apm);

        div()
            .fixed()
            .bottom(px(16.0))
            .right(px(16.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(border)
            .bg(bg)
            .px(px(16.0))
            .py(px(12.0))
            .shadow_lg()
            .cursor_pointer()
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.state.update(cx, |state, cx| {
                    state.expanded = !state.expanded;
                    cx.notify();
                });
            }))
            .hover(|style| style.scale(1.05))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(color)
                            .font_family(".AppleSystemUIFontMonospaced")
                            .text(format!("{:.1}", state.session_apm))
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x737373))
                            .text("APM")
                            .text_transform(TextTransform::Uppercase)
                    )
            )
            .when(state.total_actions > 0, |div| {
                div.child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgb(0x737373))
                        .mt(px(4.0))
                        .text(format!(
                            "{} actions in {}",
                            state.total_actions,
                            Self::format_duration(state.duration_minutes)
                        ))
                )
            })
    }

    /// Render expanded view
    fn render_expanded(&self, state: &APMState, cx: &mut Context<Self>) -> Div {
        let color = Self::get_apm_color(state.session_apm);
        let bg = Self::get_apm_bg(state.session_apm);
        let border = Self::get_apm_border(state.session_apm);

        div()
            .fixed()
            .bottom(px(16.0))
            .right(px(16.0))
            .w(px(288.0)) // w-72
            .rounded(px(12.0))
            .border_1()
            .border_color(border)
            .bg(bg)
            .shadow_xl()
            // Header
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgba(0x27272a, 0.5))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.state.update(cx, |state, cx| {
                            state.expanded = !state.expanded;
                            cx.notify();
                        });
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(color)
                                    .font_family(".AppleSystemUIFontMonospaced")
                                    .text(format!("{:.1}", state.session_apm))
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0x737373))
                                    .text("APM")
                                    .text_transform(TextTransform::Uppercase)
                            )
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(rgb(0x737373))
                            .text("-")
                    )
            )
            // Current Session
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgba(0x27272a, 0.3))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x737373))
                            .text("SESSION")
                            .text_transform(TextTransform::Uppercase)
                            .mb(px(8.0))
                    )
                    .child(
                        div()
                            .grid()
                            .gap(px(8.0))
                            .text_size(px(14.0))
                            .child(
                                div()
                                    .child(
                                        span()
                                            .text_color(rgb(0x737373))
                                            .text("Recent: ")
                                    )
                                    .child(
                                        span()
                                            .text_color(rgb(0xd4d4d8))
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text(format!("{:.1}", state.recent_apm))
                                    )
                            )
                            .child(
                                div()
                                    .child(
                                        span()
                                            .text_color(rgb(0x737373))
                                            .text("Actions: ")
                                    )
                                    .child(
                                        span()
                                            .text_color(rgb(0xd4d4d8))
                                            .font_family(".AppleSystemUIFontMonospaced")
                                            .text(format!("{}", state.total_actions))
                                    )
                            )
                            .child(
                                div()
                                    .child(
                                        span()
                                            .text_color(rgb(0x737373))
                                            .text("Duration: ")
                                    )
                                    .child(
                                        span()
                                            .text_color(rgb(0xd4d4d8))
                                            .text(Self::format_duration(state.duration_minutes))
                                    )
                            )
                    )
            )
            // Historical (conditional)
            .when(state.apm_1h > 0.0 || state.apm_lifetime > 0.0, |div| {
                div.child(
                    div()
                        .px(px(16.0))
                        .py(px(12.0))
                        .border_b_1()
                        .border_color(rgba(0x27272a, 0.3))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x737373))
                                .text("HISTORICAL")
                                .text_transform(TextTransform::Uppercase)
                                .mb(px(8.0))
                        )
                        .child(
                            div()
                                .grid()
                                .gap(px(8.0))
                                .text_size(px(14.0))
                                .child(Self::metric_row("1h:", state.apm_1h))
                                .child(Self::metric_row("6h:", state.apm_6h))
                                .child(Self::metric_row("1d:", state.apm_1d))
                                .child(Self::metric_row("All:", state.apm_lifetime))
                        )
                )
            })
            // Comparison (conditional)
            .when(state.efficiency_ratio > 0.0, |div| {
                div.child(
                    div()
                        .px(px(16.0))
                        .py(px(12.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(0x737373))
                                .text("COMPARISON")
                                .text_transform(TextTransform::Uppercase)
                                .mb(px(8.0))
                        )
                        .child(
                            div()
                                .grid()
                                .gap(px(8.0))
                                .text_size(px(14.0))
                                .child(Self::metric_row("Claude:", state.claude_code_apm))
                                .child(Self::metric_row("MC:", state.mechacoder_apm))
                        )
                        .child(
                            div()
                                .mt(px(8.0))
                                .text_size(px(14.0))
                                .child(
                                    span()
                                        .text_color(rgb(0x34d399))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text(format!("{:.1}x", state.efficiency_ratio))
                                )
                                .child(
                                    span()
                                        .text_color(rgb(0x737373))
                                        .text(" efficiency boost")
                                )
                        )
                )
            })
    }

    /// Helper to render metric row
    fn metric_row(label: &str, value: f64) -> Div {
        div()
            .child(
                span()
                    .text_color(rgb(0x737373))
                    .text(label)
            )
            .child(
                span()
                    .text_color(rgb(0xd4d4d8))
                    .font_family(".AppleSystemUIFontMonospaced")
                    .text(format!(" {:.1}", value))
            )
    }
}

impl Render for APMView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        if state.expanded {
            self.render_expanded(&state, cx)
        } else {
            self.render_compact(&state, cx)
        }
    }
}

// ============================================================================
// Integration Example (WebSocket subscription)
// ============================================================================

// In a real implementation, you would:
// 1. Create a WebSocket service
// 2. Spawn async task to listen for messages
// 3. Update state when APM messages arrive

impl APMView {
    /// Example: Subscribe to APM updates from WebSocket
    pub fn subscribe_to_updates(&mut self, cx: &mut Context<Self>) {
        // In real code, you'd have a WebSocket service
        // For now, this is a placeholder showing the pattern

        cx.spawn(|this, mut cx| async move {
            // Simulated WebSocket stream
            loop {
                // In real code: let msg = websocket.recv().await;
                // For now, just sleep
                // async_runtime::sleep(Duration::from_secs(1)).await;

                // Update state based on message
                this.update(&mut cx, |this, cx| {
                    this.state.update(cx, |state, cx| {
                        // state.session_apm = msg.session_apm;
                        // state.recent_apm = msg.recent_apm;
                        // ... etc
                        cx.notify();
                    });
                }).ok();
            }
        }).detach();
    }
}

// ============================================================================
// Usage Example
// ============================================================================

#[cfg(test)]
mod example {
    use super::*;

    /// Example showing how to use APMView in an application
    fn example_usage() {
        Application::new().run(|cx: &mut App| {
            cx.open_window(
                WindowOptions::default(),
                |_, cx| {
                    let mut view = cx.new(APMView::new);

                    // Subscribe to updates
                    view.update(cx, |view, cx| {
                        view.subscribe_to_updates(cx);
                    });

                    view
                },
            ).ok();
        });
    }
}
