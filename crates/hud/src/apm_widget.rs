//! APM Widget: Actions Per Minute display
//!
//! Displays real-time APM metrics for agent velocity tracking.
//! APM = (messages + tool_calls) / duration_minutes
//!
//! # User Stories
//!
//! - HUD-050: Display current session APM
//! - HUD-051: Color-code APM by velocity level
//! - HUD-052: Show session duration and total actions
//! - HUD-053: Display comparison with historical averages
//! - HUD-054: Update APM in real-time as work happens

use gpui::{div, prelude::*, px, Hsla, Render, Window, Context, Entity, SharedString};
use theme_oa::hud;

/// APM velocity level based on actions per minute
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApmLevel {
    /// 0-5 APM - baseline/idle
    Baseline,
    /// 5-15 APM - active work
    Active,
    /// 15-30 APM - high velocity
    High,
    /// 30+ APM - elite performance
    Elite,
}

impl ApmLevel {
    /// Determine APM level from value
    pub fn from_apm(apm: f64) -> Self {
        match apm {
            x if x >= 30.0 => ApmLevel::Elite,
            x if x >= 15.0 => ApmLevel::High,
            x if x >= 5.0 => ApmLevel::Active,
            _ => ApmLevel::Baseline,
        }
    }

    /// Get the color for this APM level
    pub fn color(&self) -> Hsla {
        match self {
            ApmLevel::Baseline => hud::APM_BASELINE,
            ApmLevel::Active => hud::APM_ACTIVE,
            ApmLevel::High => hud::APM_HIGH,
            ApmLevel::Elite => hud::APM_ELITE,
        }
    }

    /// Get display label for this level
    pub fn label(&self) -> &'static str {
        match self {
            ApmLevel::Baseline => "Idle",
            ApmLevel::Active => "Active",
            ApmLevel::High => "High",
            ApmLevel::Elite => "Elite",
        }
    }
}

/// APM snapshot data for historical comparison
#[derive(Debug, Clone, Default)]
pub struct ApmSnapshot {
    /// APM over last hour
    pub apm_1h: f64,
    /// APM over last 6 hours
    pub apm_6h: f64,
    /// APM over last 24 hours
    pub apm_24h: f64,
    /// Total sessions
    pub total_sessions: usize,
    /// Total actions (all time)
    pub total_actions: usize,
}

/// APM comparison data between agents
#[derive(Debug, Clone, Default)]
pub struct ApmComparison {
    /// Claude Code average APM
    pub claude_code_apm: f64,
    /// MechaCoder average APM
    pub mecha_coder_apm: f64,
    /// Efficiency ratio (mecha / claude)
    pub efficiency_ratio: f64,
}

/// Current session APM state
#[derive(Debug, Clone, Default)]
pub struct ApmState {
    /// Current session APM
    pub session_apm: f64,
    /// Recent APM (last 5 minutes)
    pub recent_apm: f64,
    /// Total actions this session
    pub total_actions: usize,
    /// Session duration in minutes
    pub duration_minutes: f64,
    /// Historical snapshot (optional)
    pub snapshot: Option<ApmSnapshot>,
    /// Agent comparison (optional)
    pub comparison: Option<ApmComparison>,
    /// Whether the widget is visible
    pub visible: bool,
}

impl ApmState {
    /// Create a new empty APM state
    pub fn new() -> Self {
        Self {
            visible: true,
            ..Default::default()
        }
    }

    /// Update from an apm_update message
    pub fn update_from_message(&mut self, session_apm: f64, recent_apm: f64, total_actions: usize, duration_minutes: f64) {
        self.session_apm = if session_apm.is_finite() { session_apm } else { 0.0 };
        self.recent_apm = if recent_apm.is_finite() { recent_apm } else { 0.0 };
        self.total_actions = total_actions;
        self.duration_minutes = if duration_minutes.is_finite() { duration_minutes } else { 0.0 };
    }

    /// Update snapshot data
    pub fn update_snapshot(&mut self, snapshot: ApmSnapshot) {
        self.snapshot = Some(snapshot);
    }

    /// Update comparison data
    pub fn update_comparison(&mut self, comparison: ApmComparison) {
        self.comparison = Some(comparison);
    }

    /// Get the current APM level
    pub fn level(&self) -> ApmLevel {
        ApmLevel::from_apm(self.session_apm)
    }

    /// Toggle visibility
    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }
}

/// APM Widget GPUI component
pub struct ApmWidget {
    /// Current APM state
    state: ApmState,
}

impl ApmWidget {
    /// Create a new APM widget
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            state: ApmState::new(),
        }
    }

    /// Update APM state
    pub fn update_state(&mut self, state: ApmState, _cx: &mut Context<Self>) {
        self.state = state;
    }

    /// Update from apm_update message
    pub fn handle_apm_update(&mut self, session_apm: f64, recent_apm: f64, total_actions: usize, duration_minutes: f64, cx: &mut Context<Self>) {
        self.state.update_from_message(session_apm, recent_apm, total_actions, duration_minutes);
        cx.notify();
    }

    /// Get current APM value
    pub fn current_apm(&self) -> f64 {
        self.state.session_apm
    }

    /// Get current state
    pub fn state(&self) -> &ApmState {
        &self.state
    }

    /// Check if visible
    pub fn is_visible(&self) -> bool {
        self.state.visible
    }

    /// Toggle visibility
    pub fn toggle_visibility(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_visibility();
        cx.notify();
    }

    /// Format APM value for display
    fn format_apm(apm: f64) -> SharedString {
        if apm >= 100.0 {
            format!("{:.0}", apm).into()
        } else if apm >= 10.0 {
            format!("{:.1}", apm).into()
        } else {
            format!("{:.2}", apm).into()
        }
    }

    /// Format duration for display
    fn format_duration(minutes: f64) -> SharedString {
        if minutes < 1.0 {
            format!("{:.0}s", minutes * 60.0).into()
        } else if minutes < 60.0 {
            format!("{:.1}m", minutes).into()
        } else {
            let hours = minutes / 60.0;
            format!("{:.1}h", hours).into()
        }
    }

    /// Format efficiency ratio
    fn format_efficiency(ratio: f64) -> SharedString {
        if ratio >= 1.0 {
            format!("+{:.0}%", (ratio - 1.0) * 100.0).into()
        } else {
            format!("-{:.0}%", (1.0 - ratio) * 100.0).into()
        }
    }
}

impl Render for ApmWidget {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if !self.state.visible {
            return div().into_any_element();
        }

        let level = self.state.level();
        let apm_color = level.color();
        let apm_display = Self::format_apm(self.state.session_apm);
        let level_label: SharedString = level.label().into();
        let duration_display = Self::format_duration(self.state.duration_minutes);
        let actions_display: SharedString = format!("{}", self.state.total_actions).into();

        // Build comparison string if available
        let comparison_display: Option<SharedString> = self.state.comparison.as_ref().map(|c| {
            if c.efficiency_ratio > 0.0 {
                format!("vs Claude Code: {}", Self::format_efficiency(c.efficiency_ratio)).into()
            } else {
                "".into()
            }
        });

        // Build historical display if available
        let historical_display: Option<SharedString> = self.state.snapshot.as_ref().map(|s| {
            format!("1h: {:.1} | 6h: {:.1} | 24h: {:.1}", s.apm_1h, s.apm_6h, s.apm_24h).into()
        });

        div()
            .absolute()
            .top(px(20.0))
            .left(px(20.0))
            .w(px(280.0))
            .bg(hud::APM_WIDGET_BG)
            .border_1()
            .border_color(hud::APM_WIDGET_BORDER)
            .rounded_md()
            .p(px(12.0))
            .child(
                // Main APM display
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(apm_color)
                            .text_size(px(24.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(format!("APM: {}", apm_display))
                    )
                    .child(
                        div()
                            .text_color(apm_color)
                            .text_size(px(12.0))
                            .child(level_label)
                    )
            )
            .child(
                // Session stats
                div()
                    .flex()
                    .gap(px(12.0))
                    .mt(px(8.0))
                    .text_color(theme_oa::text::SECONDARY)
                    .text_size(px(12.0))
                    .child(
                        div().child(format!("{} actions", actions_display))
                    )
                    .child(
                        div().child(duration_display)
                    )
            )
            .when_some(historical_display, |this, display| {
                this.child(
                    div()
                        .mt(px(8.0))
                        .pt(px(8.0))
                        .border_t_1()
                        .border_color(theme_oa::border::SUBTLE)
                        .text_color(theme_oa::text::MUTED)
                        .text_size(px(11.0))
                        .child(display)
                )
            })
            .when_some(comparison_display, |this, display| {
                this.child(
                    div()
                        .mt(px(4.0))
                        .text_color(theme_oa::status::SUCCESS)
                        .text_size(px(11.0))
                        .child(display)
                )
            })
            .into_any_element()
    }
}

/// Create an ApmWidget entity
pub fn apm_widget(cx: &mut gpui::App) -> Entity<ApmWidget> {
    cx.new(|cx| ApmWidget::new(cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apm_level_from_value() {
        assert_eq!(ApmLevel::from_apm(0.0), ApmLevel::Baseline);
        assert_eq!(ApmLevel::from_apm(3.0), ApmLevel::Baseline);
        assert_eq!(ApmLevel::from_apm(5.0), ApmLevel::Active);
        assert_eq!(ApmLevel::from_apm(10.0), ApmLevel::Active);
        assert_eq!(ApmLevel::from_apm(15.0), ApmLevel::High);
        assert_eq!(ApmLevel::from_apm(25.0), ApmLevel::High);
        assert_eq!(ApmLevel::from_apm(30.0), ApmLevel::Elite);
        assert_eq!(ApmLevel::from_apm(100.0), ApmLevel::Elite);
    }

    #[test]
    fn test_apm_state_update() {
        let mut state = ApmState::new();
        assert!(state.visible);
        assert_eq!(state.session_apm, 0.0);

        state.update_from_message(15.5, 12.0, 100, 6.5);
        assert_eq!(state.session_apm, 15.5);
        assert_eq!(state.recent_apm, 12.0);
        assert_eq!(state.total_actions, 100);
        assert_eq!(state.duration_minutes, 6.5);
        assert_eq!(state.level(), ApmLevel::High);
    }

    #[test]
    fn test_apm_state_sanitization() {
        let mut state = ApmState::new();

        // NaN should be sanitized to 0
        state.update_from_message(f64::NAN, f64::INFINITY, 50, f64::NEG_INFINITY);
        assert_eq!(state.session_apm, 0.0);
        assert_eq!(state.recent_apm, 0.0);
        assert_eq!(state.duration_minutes, 0.0);
        assert_eq!(state.total_actions, 50);
    }

    #[test]
    fn test_format_apm() {
        assert_eq!(ApmWidget::format_apm(0.0).as_ref(), "0.00");
        assert_eq!(ApmWidget::format_apm(5.5).as_ref(), "5.50");
        assert_eq!(ApmWidget::format_apm(15.75).as_ref(), "15.8");
        assert_eq!(ApmWidget::format_apm(150.5).as_ref(), "150"); // f64 truncation
        assert_eq!(ApmWidget::format_apm(150.9).as_ref(), "151"); // rounds up
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(ApmWidget::format_duration(0.5).as_ref(), "30s");
        assert_eq!(ApmWidget::format_duration(5.0).as_ref(), "5.0m");
        assert_eq!(ApmWidget::format_duration(90.0).as_ref(), "1.5h");
    }

    #[test]
    fn test_format_efficiency() {
        assert_eq!(ApmWidget::format_efficiency(2.0).as_ref(), "+100%");
        assert_eq!(ApmWidget::format_efficiency(1.5).as_ref(), "+50%");
        assert_eq!(ApmWidget::format_efficiency(0.5).as_ref(), "-50%");
    }

    #[test]
    fn test_visibility_toggle() {
        let mut state = ApmState::new();
        assert!(state.visible);

        state.toggle_visibility();
        assert!(!state.visible);

        state.toggle_visibility();
        assert!(state.visible);
    }
}
