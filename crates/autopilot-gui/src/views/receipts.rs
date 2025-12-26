//! Receipts panel component for trajectory replay
//!
//! Displays test results, CI status, cost breakdown, and duration.

use std::cell::RefCell;
use std::rc::Rc;

use autopilot::trajectory::{TokenUsage, Trajectory, TrajectoryResult};
use wgpui::{Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Quad, Text, theme};

use crate::views::fit_text;

/// Test result summary
#[derive(Debug, Clone, Default)]
pub struct TestResults {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
}

impl TestResults {
    pub fn total(&self) -> u32 {
        self.passed + self.failed + self.skipped
    }

    pub fn pass_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.passed as f64 / total as f64
        }
    }
}

/// CI check status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CIStatus {
    Pending,
    Running,
    Success,
    Failure,
    Unknown,
}

impl CIStatus {
    pub fn label(&self) -> &'static str {
        match self {
            CIStatus::Pending => "Pending",
            CIStatus::Running => "Running",
            CIStatus::Success => "Success",
            CIStatus::Failure => "Failure",
            CIStatus::Unknown => "Unknown",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            CIStatus::Pending => receipt_colors::pending(),
            CIStatus::Running => receipt_colors::running(),
            CIStatus::Success => receipt_colors::success(),
            CIStatus::Failure => receipt_colors::failure(),
            CIStatus::Unknown => receipt_colors::unknown(),
        }
    }
}

/// State for the receipts panel
pub struct ReceiptsState {
    pub trajectory: Option<Trajectory>,
    pub test_results: TestResults,
    pub ci_status: CIStatus,
}

impl ReceiptsState {
    pub fn new() -> Self {
        Self {
            trajectory: None,
            test_results: TestResults::default(),
            ci_status: CIStatus::Unknown,
        }
    }

    pub fn load_trajectory(&mut self, trajectory: Trajectory) {
        // Extract test results from trajectory if available
        self.test_results = extract_test_results(&trajectory);
        self.trajectory = Some(trajectory);
    }

    pub fn token_usage(&self) -> Option<&TokenUsage> {
        self.trajectory.as_ref().map(|t| &t.usage)
    }

    pub fn result(&self) -> Option<&TrajectoryResult> {
        self.trajectory.as_ref().and_then(|t| t.result.as_ref())
    }

    pub fn duration_formatted(&self) -> String {
        if let Some(result) = self.result() {
            let secs = result.duration_ms / 1000;
            let mins = secs / 60;
            let secs = secs % 60;
            if mins > 0 {
                format!("{}m {}s", mins, secs)
            } else {
                format!("{}s", secs)
            }
        } else {
            "-".to_string()
        }
    }

    pub fn cost_formatted(&self) -> String {
        if let Some(usage) = self.token_usage() {
            format!("${:.4}", usage.cost_usd)
        } else {
            "-".to_string()
        }
    }
}

impl Default for ReceiptsState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract test results from trajectory tool outputs
fn extract_test_results(trajectory: &Trajectory) -> TestResults {
    let mut results = TestResults::default();

    for step in &trajectory.steps {
        if let autopilot::trajectory::StepType::ToolResult { output, success, .. } = &step.step_type {
            if *success {
                if let Some(out) = output {
                    // Look for common test output patterns
                    // Rust: "test result: ok. X passed; Y failed; Z ignored"
                    if let Some(captures) = parse_rust_test_output(out) {
                        results.passed += captures.0;
                        results.failed += captures.1;
                        results.skipped += captures.2;
                    }
                    // npm/jest: "Tests: X passed, Y failed, Z skipped"
                    else if let Some(captures) = parse_jest_test_output(out) {
                        results.passed += captures.0;
                        results.failed += captures.1;
                        results.skipped += captures.2;
                    }
                }
            }
        }
    }

    results
}

fn parse_rust_test_output(output: &str) -> Option<(u32, u32, u32)> {
    // Match "test result: ok. X passed; Y failed; Z ignored"
    for line in output.lines() {
        if line.contains("test result:") {
            let mut passed = 0u32;
            let mut failed = 0u32;
            let mut ignored = 0u32;

            // Split by semicolon and space to get individual stat tokens
            let parts: Vec<&str> = line.split(|c| c == ';' || c == '.').collect();
            for part in parts {
                let part = part.trim();
                let words: Vec<&str> = part.split_whitespace().collect();

                // Look for patterns like "10 passed" or "2 failed"
                for window in words.windows(2) {
                    if let Ok(num) = window[0].parse::<u32>() {
                        match window[1] {
                            "passed" => passed = num,
                            "failed" => failed = num,
                            "ignored" => ignored = num,
                            _ => {}
                        }
                    }
                }
            }

            if passed > 0 || failed > 0 || ignored > 0 {
                return Some((passed, failed, ignored));
            }
        }
    }
    None
}

fn parse_jest_test_output(output: &str) -> Option<(u32, u32, u32)> {
    // Match "Tests: X passed, Y failed, Z skipped"
    for line in output.lines() {
        if line.contains("Tests:") && (line.contains("passed") || line.contains("failed")) {
            let mut passed = 0u32;
            let mut failed = 0u32;
            let mut skipped = 0u32;

            // Split by comma to get individual stat tokens
            let parts: Vec<&str> = line.split(',').collect();
            for part in parts {
                let part = part.trim();
                let words: Vec<&str> = part.split_whitespace().collect();

                // Look for patterns like "15 passed" or "3 failed"
                for window in words.windows(2) {
                    if let Ok(num) = window[0].parse::<u32>() {
                        match window[1] {
                            "passed" => passed = num,
                            "failed" => failed = num,
                            "skipped" => skipped = num,
                            _ => {}
                        }
                    }
                }
            }

            if passed > 0 || failed > 0 || skipped > 0 {
                return Some((passed, failed, skipped));
            }
        }
    }
    None
}

/// Receipt colors
mod receipt_colors {
    use wgpui::Hsla;

    fn rgb(r: u8, g: u8, b: u8) -> Hsla {
        Hsla::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    pub fn pending() -> Hsla { rgb(245, 158, 11) }    // amber
    pub fn running() -> Hsla { rgb(59, 130, 246) }    // blue
    pub fn success() -> Hsla { rgb(16, 185, 129) }    // green
    pub fn failure() -> Hsla { rgb(239, 68, 68) }     // red
    pub fn unknown() -> Hsla { rgb(107, 114, 128) }   // gray
}

/// Receipts panel component
pub struct ReceiptsPanel {
    state: Rc<RefCell<ReceiptsState>>,
}

impl ReceiptsPanel {
    pub fn new(state: Rc<RefCell<ReceiptsState>>) -> Self {
        Self { state }
    }
}

impl Component for ReceiptsPanel {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let line_height = theme::font_size::SM * 1.5;
        let section_spacing = theme::spacing::MD;
        let mut y = bounds.origin.y + padding;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds).with_background(theme::bg::SURFACE),
        );

        // Title
        let mut title = Text::new("Receipts")
            .font_size(theme::font_size::SM)
            .color(theme::text::PRIMARY);
        title.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height + section_spacing;

        // Result summary section
        if let Some(result) = state.result() {
            // Success/Failure indicator
            let success_text = if result.success { "SUCCESS" } else { "FAILURE" };
            let success_color = if result.success { receipt_colors::success() } else { receipt_colors::failure() };

            let mut status = Text::new(success_text)
                .font_size(theme::font_size::BASE)
                .color(success_color);
            status.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height * 1.2),
                cx,
            );
            y += line_height * 1.2;

            // Duration
            let duration_text = format!("Duration: {}", state.duration_formatted());
            let duration_text = fit_text(cx, &duration_text, theme::font_size::XS, available_width);
            let mut duration = Text::new(&duration_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            duration.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;

            // Turns
            let turns_text = format!("Turns: {}", result.num_turns);
            let mut turns = Text::new(&turns_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            turns.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;

            // Issues completed
            if result.issues_completed > 0 {
                let issues_text = format!("Issues completed: {}", result.issues_completed);
                let mut issues = Text::new(&issues_text)
                    .font_size(theme::font_size::XS)
                    .color(receipt_colors::success());
                issues.paint(
                    Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                    cx,
                );
                y += line_height;
            }

            // APM if available
            if let Some(apm) = result.apm {
                let apm_text = format!("APM: {:.1}", apm);
                let mut apm_label = Text::new(&apm_text)
                    .font_size(theme::font_size::XS)
                    .color(theme::text::MUTED);
                apm_label.paint(
                    Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                    cx,
                );
                y += line_height;
            }

            // Errors if any
            if !result.errors.is_empty() {
                y += theme::spacing::SM;
                let mut errors_header = Text::new("Errors:")
                    .font_size(theme::font_size::XS)
                    .color(receipt_colors::failure());
                errors_header.paint(
                    Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                    cx,
                );
                y += line_height;

                for error in result.errors.iter().take(3) {
                    let error_text = fit_text(cx, error, theme::font_size::XS, available_width - padding);
                    let mut error_label = Text::new(&format!("  - {}", error_text))
                        .font_size(theme::font_size::XS)
                        .color(theme::text::MUTED);
                    error_label.paint(
                        Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                        cx,
                    );
                    y += line_height;
                }
            }
        } else {
            let mut no_result = Text::new("No result data")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            no_result.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;
        }

        y += section_spacing;

        // Test Results section
        let mut test_header = Text::new("Test Results")
            .font_size(theme::font_size::XS)
            .color(theme::text::PRIMARY);
        test_header.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height;

        let test_results = &state.test_results;
        if test_results.total() > 0 {
            // Test bar
            let bar_height = 8.0;
            let bar_width = available_width.min(200.0);
            let bar_x = bounds.origin.x + padding;

            // Background
            cx.scene.draw_quad(
                Quad::new(Bounds::new(bar_x, y, bar_width, bar_height))
                    .with_background(theme::border::DEFAULT),
            );

            let total = test_results.total() as f32;
            let mut seg_x = bar_x;

            // Passed (green)
            if test_results.passed > 0 {
                let width = bar_width * (test_results.passed as f32 / total);
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(seg_x, y, width, bar_height))
                        .with_background(receipt_colors::success()),
                );
                seg_x += width;
            }

            // Failed (red)
            if test_results.failed > 0 {
                let width = bar_width * (test_results.failed as f32 / total);
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(seg_x, y, width, bar_height))
                        .with_background(receipt_colors::failure()),
                );
                seg_x += width;
            }

            // Skipped (gray)
            if test_results.skipped > 0 {
                let width = bar_width * (test_results.skipped as f32 / total);
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(seg_x, y, width, bar_height))
                        .with_background(receipt_colors::unknown()),
                );
            }

            y += bar_height + theme::spacing::XS;

            // Test counts
            let test_text = format!(
                "Passed: {} | Failed: {} | Skipped: {}",
                test_results.passed, test_results.failed, test_results.skipped
            );
            let test_text = fit_text(cx, &test_text, theme::font_size::XS, available_width);
            let mut test_counts = Text::new(&test_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            test_counts.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;

            // Pass rate
            let rate_text = format!("Pass rate: {:.1}%", test_results.pass_rate() * 100.0);
            let mut rate = Text::new(&rate_text)
                .font_size(theme::font_size::XS)
                .color(if test_results.pass_rate() >= 0.9 {
                    receipt_colors::success()
                } else if test_results.pass_rate() >= 0.5 {
                    receipt_colors::pending()
                } else {
                    receipt_colors::failure()
                });
            rate.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        } else {
            let mut no_tests = Text::new("No test data")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            no_tests.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        }
        y += line_height + section_spacing;

        // CI Status section
        let mut ci_header = Text::new("CI Status")
            .font_size(theme::font_size::XS)
            .color(theme::text::PRIMARY);
        ci_header.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height;

        let ci_status = state.ci_status;
        let mut ci_label = Text::new(ci_status.label())
            .font_size(theme::font_size::SM)
            .color(ci_status.color());
        ci_label.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height + section_spacing;

        // Cost Breakdown section
        let mut cost_header = Text::new("Cost Breakdown")
            .font_size(theme::font_size::XS)
            .color(theme::text::PRIMARY);
        cost_header.paint(
            Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
            cx,
        );
        y += line_height;

        if let Some(usage) = state.token_usage() {
            // Total cost
            let cost_text = format!("Total: ${:.4}", usage.cost_usd);
            let mut cost = Text::new(&cost_text)
                .font_size(theme::font_size::SM)
                .color(theme::accent::PRIMARY);
            cost.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;

            // Token breakdown
            let tokens_text = format!(
                "Input: {} | Output: {}",
                format_tokens(usage.input_tokens),
                format_tokens(usage.output_tokens)
            );
            let tokens_text = fit_text(cx, &tokens_text, theme::font_size::XS, available_width);
            let mut tokens = Text::new(&tokens_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            tokens.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
            y += line_height;

            // Cache info
            let cache_text = format!(
                "Cache read: {} | Cache create: {}",
                format_tokens(usage.cache_read_tokens),
                format_tokens(usage.cache_creation_tokens)
            );
            let cache_text = fit_text(cx, &cache_text, theme::font_size::XS, available_width);
            let mut cache = Text::new(&cache_text)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            cache.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        } else {
            let mut no_cost = Text::new("No cost data")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            no_cost.paint(
                Bounds::new(bounds.origin.x + padding, y, available_width, line_height),
                cx,
            );
        }
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        // Receipts panel is read-only, no events to handle
        EventResult::Ignored
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        // Fixed width sidebar, flexible height
        (Some(250.0), None)
    }
}

/// Format token count with K/M suffix
fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_results_default() {
        let results = TestResults::default();
        assert_eq!(results.total(), 0);
        assert_eq!(results.pass_rate(), 0.0);
    }

    #[test]
    fn test_test_results_pass_rate() {
        let results = TestResults {
            passed: 8,
            failed: 2,
            skipped: 0,
        };
        assert_eq!(results.total(), 10);
        assert!((results.pass_rate() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_ci_status_label() {
        assert_eq!(CIStatus::Pending.label(), "Pending");
        assert_eq!(CIStatus::Running.label(), "Running");
        assert_eq!(CIStatus::Success.label(), "Success");
        assert_eq!(CIStatus::Failure.label(), "Failure");
        assert_eq!(CIStatus::Unknown.label(), "Unknown");
    }

    #[test]
    fn test_receipts_state_new() {
        let state = ReceiptsState::new();
        assert!(state.trajectory.is_none());
        assert_eq!(state.ci_status, CIStatus::Unknown);
        assert_eq!(state.test_results.total(), 0);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_parse_rust_test_output() {
        let output = "test result: ok. 10 passed; 2 failed; 1 ignored; 0 measured; 0 filtered out";
        let result = parse_rust_test_output(output);
        assert_eq!(result, Some((10, 2, 1)));
    }

    #[test]
    fn test_parse_jest_test_output() {
        let output = "Tests: 15 passed, 3 failed, 2 skipped";
        let result = parse_jest_test_output(output);
        assert_eq!(result, Some((15, 3, 2)));
    }

    #[test]
    fn test_duration_formatted() {
        let mut state = ReceiptsState::new();
        assert_eq!(state.duration_formatted(), "-");

        // Create a trajectory with result
        let mut traj = Trajectory::new(
            "test".to_string(),
            "claude".to_string(),
            "/test".to_string(),
            "abc".to_string(),
            None,
        );
        traj.result = Some(TrajectoryResult {
            success: true,
            duration_ms: 125000, // 2m 5s
            num_turns: 10,
            result_text: None,
            errors: vec![],
            issues_completed: 1,
            apm: Some(15.0),
        });

        state.load_trajectory(traj);
        assert_eq!(state.duration_formatted(), "2m 5s");
    }
}
