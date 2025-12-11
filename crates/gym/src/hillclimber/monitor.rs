//! HillClimber Monitor - Real-time MAP orchestrator visualization

use gpui_oa::prelude::*;
use gpui_oa::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use super::workflow_graph::{WorkflowGraph, GraphNode, NodeStatus, NodeKind};
use super::turn_log::{TurnLog, TurnEntry, TurnAction};
use super::test_results::{TestResults, TestResult, TestOutcome};
use super::controls::HCControls;

/// HillClimber run modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HCMode {
    #[default]
    Quick,      // 3 turns, fast iteration
    Standard,   // 10 turns, balanced
    Full,       // 25 turns, thorough
}

impl HCMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Quick => "Quick (3)",
            Self::Standard => "Standard (10)",
            Self::Full => "Full (25)",
        }
    }

    pub fn max_turns(&self) -> u32 {
        match self {
            Self::Quick => 3,
            Self::Standard => 10,
            Self::Full => 25,
        }
    }
}

/// Session summary
#[derive(Debug, Clone)]
pub struct HCSession {
    pub id: String,
    pub task_id: String,
    pub task_name: String,
    pub mode: HCMode,
    pub status: HCSessionStatus,
    pub current_turn: u32,
    pub max_turns: u32,
    pub tests_passed: u32,
    pub tests_total: u32,
    pub started_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HCSessionStatus {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
}

impl HCSessionStatus {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }
}

/// Main HillClimber Monitor view
pub struct HillClimberMonitor {
    /// Current session being monitored
    pub session: Option<HCSession>,
    /// Workflow graph component
    workflow_graph: Entity<WorkflowGraph>,
    /// Turn log component
    turn_log: Entity<TurnLog>,
    /// Test results component
    test_results: Entity<TestResults>,
    /// Controls component
    #[allow(dead_code)]
    controls: Entity<HCControls>,
    /// Whether the left panel is collapsed
    pub left_collapsed: bool,
    focus_handle: FocusHandle,
}

impl HillClimberMonitor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        // Create sample session for development
        let session = Some(HCSession {
            id: "hc-001".to_string(),
            task_id: "regex-log".to_string(),
            task_name: "Regex Log Parser".to_string(),
            mode: HCMode::Standard,
            status: HCSessionStatus::Running,
            current_turn: 4,
            max_turns: 10,
            tests_passed: 7,
            tests_total: 12,
            started_at: "2024-12-10T14:30:00Z".to_string(),
        });

        // Initialize child components with sample data
        let workflow_graph = cx.new(|cx| {
            let mut graph = WorkflowGraph::new(cx);
            graph.set_nodes(Self::create_sample_nodes());
            graph
        });

        let turn_log = cx.new(|cx| {
            let mut log = TurnLog::new(cx);
            log.set_entries(Self::create_sample_turns());
            log
        });

        let test_results = cx.new(|cx| {
            let mut results = TestResults::new(cx);
            results.set_results(Self::create_sample_results());
            results
        });

        let controls = cx.new(|cx| HCControls::new(cx));

        Self {
            session,
            workflow_graph,
            turn_log,
            test_results,
            controls,
            left_collapsed: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn create_sample_nodes() -> Vec<GraphNode> {
        vec![
            GraphNode {
                id: "task".to_string(),
                kind: NodeKind::Task,
                label: "regex-log".to_string(),
                status: NodeStatus::Completed,
                position: (20.0, 100.0),
            },
            GraphNode {
                id: "testgen".to_string(),
                kind: NodeKind::TestGen,
                label: "TestGen".to_string(),
                status: NodeStatus::Completed,
                position: (160.0, 100.0),
            },
            GraphNode {
                id: "decomposer".to_string(),
                kind: NodeKind::Decomposer,
                label: "Decomposer".to_string(),
                status: NodeStatus::Completed,
                position: (300.0, 100.0),
            },
            GraphNode {
                id: "fm".to_string(),
                kind: NodeKind::FM,
                label: "Apple FM".to_string(),
                status: NodeStatus::Active,
                position: (440.0, 100.0),
            },
            GraphNode {
                id: "verifier".to_string(),
                kind: NodeKind::Verifier,
                label: "Verifier".to_string(),
                status: NodeStatus::Waiting,
                position: (580.0, 100.0),
            },
            GraphNode {
                id: "results".to_string(),
                kind: NodeKind::Results,
                label: "Results".to_string(),
                status: NodeStatus::Waiting,
                position: (720.0, 100.0),
            },
        ]
    }

    fn create_sample_turns() -> Vec<TurnEntry> {
        vec![
            TurnEntry {
                turn: 1,
                action: TurnAction::FMGenerate,
                description: "Generated initial regex pattern".to_string(),
                duration_ms: 1250,
                success: true,
            },
            TurnEntry {
                turn: 1,
                action: TurnAction::Verify,
                description: "Ran 12 TestGen tests".to_string(),
                duration_ms: 340,
                success: false,
            },
            TurnEntry {
                turn: 2,
                action: TurnAction::FMGenerate,
                description: "Refined pattern with lookahead".to_string(),
                duration_ms: 980,
                success: true,
            },
            TurnEntry {
                turn: 2,
                action: TurnAction::Verify,
                description: "5/12 tests passing".to_string(),
                duration_ms: 290,
                success: false,
            },
            TurnEntry {
                turn: 3,
                action: TurnAction::FMGenerate,
                description: "Added word boundary handling".to_string(),
                duration_ms: 1100,
                success: true,
            },
            TurnEntry {
                turn: 3,
                action: TurnAction::Verify,
                description: "7/12 tests passing".to_string(),
                duration_ms: 310,
                success: false,
            },
            TurnEntry {
                turn: 4,
                action: TurnAction::FMGenerate,
                description: "Iterating on edge cases...".to_string(),
                duration_ms: 0,
                success: true,
            },
        ]
    }

    fn create_sample_results() -> Vec<TestResult> {
        vec![
            TestResult {
                id: "test-1".to_string(),
                name: "basic_ip_extraction".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(45),
            },
            TestResult {
                id: "test-2".to_string(),
                name: "timestamp_parsing".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(32),
            },
            TestResult {
                id: "test-3".to_string(),
                name: "multiline_log".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(28),
            },
            TestResult {
                id: "test-4".to_string(),
                name: "error_level_detection".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(38),
            },
            TestResult {
                id: "test-5".to_string(),
                name: "unicode_handling".to_string(),
                outcome: TestOutcome::Failed,
                duration_ms: Some(52),
            },
            TestResult {
                id: "test-6".to_string(),
                name: "empty_log_handling".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(15),
            },
            TestResult {
                id: "test-7".to_string(),
                name: "special_characters".to_string(),
                outcome: TestOutcome::Failed,
                duration_ms: Some(41),
            },
            TestResult {
                id: "test-8".to_string(),
                name: "large_file_performance".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(120),
            },
            TestResult {
                id: "test-9".to_string(),
                name: "concurrent_access".to_string(),
                outcome: TestOutcome::Failed,
                duration_ms: Some(85),
            },
            TestResult {
                id: "test-10".to_string(),
                name: "malformed_input".to_string(),
                outcome: TestOutcome::Passed,
                duration_ms: Some(22),
            },
            TestResult {
                id: "test-11".to_string(),
                name: "boundary_conditions".to_string(),
                outcome: TestOutcome::Failed,
                duration_ms: Some(48),
            },
            TestResult {
                id: "test-12".to_string(),
                name: "regex_timeout".to_string(),
                outcome: TestOutcome::Failed,
                duration_ms: Some(200),
            },
        ]
    }

    fn render_header(&self) -> impl IntoElement {
        let session = self.session.as_ref();

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            .bg(bg::SURFACE)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::BRIGHT)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("HillClimber Monitor")
                    )
                    .when_some(session, |el, s| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("Task:")
                                )
                                .child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::PRIMARY)
                                        .child(s.task_name.clone())
                                )
                        )
                    })
            )
            .when_some(session, |el, s| {
                el.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(16.0))
                        .child(self.render_status_badge(s.status))
                        .child(self.render_progress_indicator(s))
                )
            })
    }

    fn render_status_badge(&self, status: HCSessionStatus) -> impl IntoElement {
        let (bg_color, text_color, label) = match status {
            HCSessionStatus::Idle => (bg::ELEVATED, text::MUTED, "Idle"),
            HCSessionStatus::Running => (status::INFO_BG, status::RUNNING, "Running"),
            HCSessionStatus::Paused => (status::WARNING_BG, status::WARNING, "Paused"),
            HCSessionStatus::Completed => (status::SUCCESS_BG, status::SUCCESS, "Completed"),
            HCSessionStatus::Failed => (status::ERROR_BG, status::ERROR, "Failed"),
        };

        div()
            .px(px(10.0))
            .py(px(4.0))
            .bg(bg_color)
            .text_color(text_color)
            .text_size(px(11.0))
            .font_family(FONT_FAMILY)
            .font_weight(FontWeight::MEDIUM)
            .rounded(px(4.0))
            .child(label)
    }

    fn render_progress_indicator(&self, session: &HCSession) -> impl IntoElement {
        let progress = session.current_turn as f32 / session.max_turns as f32;
        let progress_width = (progress * 120.0).max(4.0);

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(format!("Turn {}/{}", session.current_turn, session.max_turns))
            )
            .child(
                div()
                    .w(px(120.0))
                    .h(px(6.0))
                    .bg(bg::ELEVATED)
                    .rounded(px(3.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(progress_width))
                            .h_full()
                            .bg(status::INFO)
                            .rounded(px(3.0))
                    )
            )
    }

    fn render_metrics_panel(&self) -> impl IntoElement {
        let session = self.session.as_ref();

        div()
            .flex()
            .gap(px(16.0))
            .px(px(20.0))
            .py(px(12.0))
            .bg(bg::SURFACE)
            .border_b_1()
            .border_color(border::DEFAULT)
            .when_some(session, |el, s| {
                el
                    .child(self.render_metric("Tests Passing", format!("{}/{}", s.tests_passed, s.tests_total)))
                    .child(self.render_metric("Pass Rate", format!("{:.0}%", (s.tests_passed as f32 / s.tests_total as f32) * 100.0)))
                    .child(self.render_metric("Mode", s.mode.label().to_string()))
            })
    }

    fn render_metric(&self, label: &str, value: String) -> impl IntoElement {
        let label = label.to_string();

        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(label)
            )
            .child(
                div()
                    .text_size(px(16.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .font_weight(FontWeight::MEDIUM)
                    .child(value)
            )
    }
}

impl Focusable for HillClimberMonitor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HillClimberMonitor {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Header
            .child(self.render_header())
            // Metrics strip
            .child(self.render_metrics_panel())
            // Main content area
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Left panel: Workflow Graph
                    .child(
                        div()
                            .w(px(500.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(border::DEFAULT)
                            // Graph header
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(10.0))
                                    .border_b_1()
                                    .border_color(border::SUBTLE)
                                    .child(
                                        div()
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .font_weight(FontWeight::MEDIUM)
                                            .child("Workflow")
                                    )
                            )
                            // Graph
                            .child(
                                div()
                                    .flex_1()
                                    .child(self.workflow_graph.clone())
                            )
                    )
                    // Right panel: Turn Log + Test Results
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            // Turn Log (top)
                            .child(
                                div()
                                    .h(px(300.0))
                                    .flex()
                                    .flex_col()
                                    .border_b_1()
                                    .border_color(border::DEFAULT)
                                    // Header
                                    .child(
                                        div()
                                            .px(px(16.0))
                                            .py(px(10.0))
                                            .border_b_1()
                                            .border_color(border::SUBTLE)
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::MUTED)
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Turn Log")
                                            )
                                    )
                                    // Content
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(self.turn_log.clone())
                                    )
                            )
                            // Test Results (bottom)
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    // Header
                                    .child(
                                        div()
                                            .px(px(16.0))
                                            .py(px(10.0))
                                            .border_b_1()
                                            .border_color(border::SUBTLE)
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::MUTED)
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .child("Test Results")
                                            )
                                            .when_some(self.session.as_ref(), |el, s| {
                                                el.child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .font_family(FONT_FAMILY)
                                                        .text_color(text::MUTED)
                                                        .child(format!("{} passed, {} failed", s.tests_passed, s.tests_total - s.tests_passed))
                                                )
                                            })
                                    )
                                    // Content
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(self.test_results.clone())
                                    )
                            )
                    )
            )
    }
}
