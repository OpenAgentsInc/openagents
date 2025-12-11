//! ATIF Components Story
//!
//! Showcases all permutations of ATIF visualization components ported from Effuse.

use crate::story::Story;
use atif::{
    Agent, FinalMetrics, Metrics, Observation, ObservationResult, Step, StepSource,
    SubagentTrajectoryRef, ToolCall, Trajectory,
};
use atif_store::{TrajectoryMetadata, TrajectoryStatus};
use chrono::Utc;
use commander::components::{
    render_category_badge, render_complete_item, render_confidence_bar, render_error_item,
    render_metrics, render_observation, render_progress_item, render_reflection_item,
    render_source_badge, render_step_details, render_step_row, render_test_item_details,
    render_test_item_header, render_tool_call, render_tool_calls, render_trajectory_detail,
    render_trajectory_item, render_trajectory_list, CompleteData, ErrorData, ProgressData,
    ReflectionAction, ReflectionData, TestCategory, TestData,
};
use gpui_oa::*;
use std::collections::HashSet;

pub struct AtifComponentsStory;

impl Render for AtifComponentsStory {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        Story::container()
            .child(Story::title("ATIF Components"))
            .child(Story::description(
                "All permutations of ATIF visualization components ported from Effuse",
            ))
            // Section 1: Source Badges
            .child(render_source_badges_section())
            // Section 2: Category Badges
            .child(render_category_badges_section())
            // Section 3: Confidence Bars
            .child(render_confidence_bars_section())
            // Section 4: Tool Calls
            .child(render_tool_calls_section())
            // Section 5: Observations
            .child(render_observations_section())
            // Section 6: Metrics
            .child(render_metrics_section())
            // Section 7: Step Views
            .child(render_step_views_section())
            // Section 8: Thread Items
            .child(render_thread_items_section())
            // Section 9: Trajectory List Items
            .child(render_trajectory_list_section())
            // Section 10: Trajectory Detail States
            .child(render_trajectory_detail_section())
    }
}

// ============================================================================
// Section 1: Source Badges
// ============================================================================

fn render_source_badges_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("Source Badges"))
        .child(Story::description("Step source indicators with color coding"))
        .child(
            Story::row()
                .child(Story::item("User").child(render_source_badge(&StepSource::User)))
                .child(Story::item("Agent").child(render_source_badge(&StepSource::Agent)))
                .child(Story::item("System").child(render_source_badge(&StepSource::System))),
        )
}

// ============================================================================
// Section 2: Category Badges
// ============================================================================

fn render_category_badges_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("Category Badges"))
        .child(Story::description("Test category indicators for thread items"))
        .child(
            Story::row()
                .child(Story::item("AntiCheat").child(render_category_badge(&TestCategory::AntiCheat)))
                .child(Story::item("Existence").child(render_category_badge(&TestCategory::Existence)))
                .child(Story::item("Correctness").child(render_category_badge(&TestCategory::Correctness))),
        )
        .child(
            Story::row()
                .child(Story::item("Boundary").child(render_category_badge(&TestCategory::Boundary)))
                .child(Story::item("Integration").child(render_category_badge(&TestCategory::Integration)))
                .child(Story::item("Other").child(render_category_badge(&TestCategory::Other("custom".to_string())))),
        )
}

// ============================================================================
// Section 3: Confidence Bars
// ============================================================================

fn render_confidence_bars_section() -> impl IntoElement {
    Story::section()
        .child(Story::section_title("Confidence Bars"))
        .child(Story::description("Visual confidence indicators for test items"))
        .child(
            Story::column()
                .child(Story::item("0%").child(render_confidence_bar(0.0)))
                .child(Story::item("50%").child(render_confidence_bar(0.5)))
                .child(Story::item("85%").child(render_confidence_bar(0.85)))
                .child(Story::item("100%").child(render_confidence_bar(1.0))),
        )
}

// ============================================================================
// Section 4: Tool Calls
// ============================================================================

fn render_tool_calls_section() -> impl IntoElement {
    let single_tool = ToolCall::new(
        "call_001".to_string(),
        "read_file".to_string(),
        serde_json::json!({
            "path": "/src/main.rs",
            "encoding": "utf-8"
        }),
    );

    let tools = vec![
        ToolCall::new(
            "call_002".to_string(),
            "write_file".to_string(),
            serde_json::json!({
                "path": "/src/lib.rs",
                "content": "pub mod utils;"
            }),
        ),
        ToolCall::new(
            "call_003".to_string(),
            "run_command".to_string(),
            serde_json::json!({
                "command": "cargo build",
                "cwd": "/project"
            }),
        ),
    ];

    Story::section()
        .child(Story::section_title("Tool Calls"))
        .child(Story::description("Function invocation displays"))
        .child(
            Story::column()
                .child(Story::item("Single Tool Call").child(render_tool_call(&single_tool)))
                .child(Story::item("Multiple Tool Calls").child(render_tool_calls(&tools))),
        )
}

// ============================================================================
// Section 5: Observations
// ============================================================================

fn render_observations_section() -> impl IntoElement {
    let content_only = Observation::single(ObservationResult::with_content(
        Some("call_001".to_string()),
        "File contents:\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```".to_string(),
    ));

    let with_subagent = Observation::single(ObservationResult::with_subagent(
        Some("call_002".to_string()),
        vec![SubagentTrajectoryRef::new("subagent-session-abc123".to_string())],
    ));

    let multiple = Observation::new(vec![
        ObservationResult::with_content(
            Some("call_003".to_string()),
            "Build successful".to_string(),
        ),
        ObservationResult::with_content(
            Some("call_004".to_string()),
            "Tests passed: 42/42".to_string(),
        ),
    ]);

    Story::section()
        .child(Story::section_title("Observations"))
        .child(Story::description("Tool execution results"))
        .child(
            Story::column()
                .child(Story::item("Content Only").child(render_observation(&content_only)))
                .child(Story::item("With Subagent Ref").child(render_observation(&with_subagent)))
                .child(Story::item("Multiple Results").child(render_observation(&multiple))),
        )
}

// ============================================================================
// Section 6: Metrics
// ============================================================================

fn render_metrics_section() -> impl IntoElement {
    let full_metrics = Metrics {
        prompt_tokens: Some(1500),
        completion_tokens: Some(350),
        cached_tokens: Some(800),
        cost_usd: Some(0.0245),
        ..Default::default()
    };

    let partial_metrics = Metrics {
        prompt_tokens: Some(500),
        completion_tokens: Some(100),
        ..Default::default()
    };

    Story::section()
        .child(Story::section_title("Metrics"))
        .child(Story::description("LLM operational data"))
        .child(
            Story::column()
                .child(Story::item("All Fields").child(render_metrics(&full_metrics)))
                .child(Story::item("Partial Fields").child(render_metrics(&partial_metrics))),
        )
}

// ============================================================================
// Section 7: Step Views
// ============================================================================

fn render_step_views_section() -> impl IntoElement {
    let user_step = Step::user(1, "Please help me refactor this code.".to_string())
        .with_timestamp(Utc::now());

    let agent_step = Step::agent(
        2,
        "I'll help you refactor the code. Let me first read the file to understand its structure."
            .to_string(),
    )
    .with_timestamp(Utc::now())
    .with_model("claude-3-5-sonnet".to_string())
    .with_tool_calls(vec![ToolCall::new(
        "call_005".to_string(),
        "read_file".to_string(),
        serde_json::json!({"path": "/src/main.rs"}),
    )])
    .with_observation(Observation::single(ObservationResult::with_content(
        Some("call_005".to_string()),
        "fn main() { ... }".to_string(),
    )))
    .with_metrics(Metrics {
        prompt_tokens: Some(1200),
        completion_tokens: Some(250),
        ..Default::default()
    });

    let agent_with_reasoning = Step::agent(3, "I've analyzed the code structure.".to_string())
        .with_timestamp(Utc::now())
        .with_model("claude-opus-4".to_string())
        .with_reasoning_content(
            "The user wants to refactor main.rs. Looking at the code, I see several opportunities:\n\
            1. Extract helper functions\n\
            2. Add error handling\n\
            3. Improve naming conventions"
                .to_string(),
        );

    let system_step = Step::system(
        0,
        "You are a helpful coding assistant. Follow best practices.".to_string(),
    );

    Story::section()
        .child(Story::section_title("Step Views"))
        .child(Story::description("Step header and detail rendering"))
        .child(
            Story::column()
                .child(Story::label("Headers (Collapsed)"))
                .child(render_step_row(&user_step, false))
                .child(render_step_row(&agent_step, false))
                .child(render_step_row(&system_step, false)),
        )
        .child(Story::divider())
        .child(
            Story::column()
                .child(Story::label("Full Step (Expanded)"))
                .child(render_step_details(&user_step))
                .child(render_step_details(&agent_step))
                .child(render_step_details(&agent_with_reasoning)),
        )
}

// ============================================================================
// Section 8: Thread Items
// ============================================================================

fn render_thread_items_section() -> impl IntoElement {
    let now = Utc::now();

    let progress = ProgressData {
        phase: "Generation".to_string(),
        category: Some("correctness".to_string()),
        round: 3,
        status: "Generating test cases...".to_string(),
    };

    let reflection_refining = ReflectionData {
        category: Some("boundary".to_string()),
        text: "The edge cases need more coverage. Adding tests for empty input and maximum values."
            .to_string(),
        action: ReflectionAction::Refining,
    };

    let reflection_complete = ReflectionData {
        category: None,
        text: "Test generation complete. All categories covered.".to_string(),
        action: ReflectionAction::Complete,
    };

    let test_data = TestData {
        id: "test_boundary_001".to_string(),
        category: "boundary".to_string(),
        input: "process_data([])".to_string(),
        expected_output: Some("[]".to_string()),
        reasoning: "Empty array should return empty array without errors.".to_string(),
        confidence: 0.92,
    };

    let complete = CompleteData {
        total_tests: 47,
        total_rounds: 5,
        comprehensiveness_score: Some(8.5),
        total_tokens_used: 125000,
        duration_ms: 45000,
        uncertainties: vec!["Async error handling edge cases".to_string()],
    };

    let error = ErrorData {
        error: "Connection timeout: Failed to reach model endpoint after 30s".to_string(),
    };

    Story::section()
        .child(Story::section_title("Thread Items"))
        .child(Story::description("TestGen and agent log thread items"))
        .child(
            Story::column()
                .child(Story::item("Progress").child(render_progress_item(now, &progress)))
                .child(
                    Story::item("Reflection (Refining)")
                        .child(render_reflection_item(now, &reflection_refining)),
                )
                .child(
                    Story::item("Reflection (Complete)")
                        .child(render_reflection_item(now, &reflection_complete)),
                )
                .child(
                    Story::item("Test (Collapsed)")
                        .child(render_test_item_header(now, &test_data, false)),
                )
                .child(
                    Story::item("Test (Expanded)")
                        .child(render_test_item_header(now, &test_data, true))
                        .child(render_test_item_details(&test_data)),
                )
                .child(Story::item("Complete").child(render_complete_item(now, &complete)))
                .child(Story::item("Error").child(render_error_item(now, &error))),
        )
}

// ============================================================================
// Section 9: Trajectory List Items
// ============================================================================

fn render_trajectory_list_section() -> impl IntoElement {
    let now = Utc::now();

    let completed = TrajectoryMetadata {
        session_id: "sess_abc123def456".to_string(),
        agent_name: "claude-code".to_string(),
        agent_version: "1.0.0".to_string(),
        model_name: Some("claude-3-5-sonnet".to_string()),
        created_at: now,
        completed_at: Some(now),
        status: TrajectoryStatus::Completed,
        total_steps: 15,
    };

    let failed = TrajectoryMetadata {
        session_id: "sess_xyz789uvw012".to_string(),
        agent_name: "openhands".to_string(),
        agent_version: "0.9.0".to_string(),
        model_name: Some("gpt-4-turbo".to_string()),
        created_at: now,
        completed_at: Some(now),
        status: TrajectoryStatus::Failed,
        total_steps: 8,
    };

    let in_progress = TrajectoryMetadata {
        session_id: "sess_qrs345tuv678".to_string(),
        agent_name: "mechacoder".to_string(),
        agent_version: "2.1.0".to_string(),
        model_name: Some("claude-opus-4".to_string()),
        created_at: now,
        completed_at: None,
        status: TrajectoryStatus::InProgress,
        total_steps: 3,
    };

    Story::section()
        .child(Story::section_title("Trajectory List Items"))
        .child(Story::description("Individual trajectory cards with status"))
        .child(
            Story::column()
                .child(Story::item("Completed (Selected)").child(render_trajectory_item(&completed, true)))
                .child(Story::item("Failed").child(render_trajectory_item(&failed, false)))
                .child(Story::item("In Progress").child(render_trajectory_item(&in_progress, false))),
        )
        .child(Story::divider())
        .child(Story::label("Full List View"))
        .child(render_trajectory_list(
            &[completed.clone(), failed.clone(), in_progress.clone()],
            Some(&completed.session_id),
            3,
            0,
            10,
            false,
            None,
            false,
            "",
        ))
        .child(Story::divider())
        .child(Story::label("Loading State"))
        .child(render_trajectory_list(&[], None, 0, 0, 10, true, None, false, ""))
        .child(Story::divider())
        .child(Story::label("Error State"))
        .child(render_trajectory_list(
            &[],
            None,
            0,
            0,
            10,
            false,
            Some("Database connection failed"),
            false,
            "",
        ))
        .child(Story::divider())
        .child(Story::label("Empty State"))
        .child(render_trajectory_list(&[], None, 0, 0, 10, false, None, false, ""))
}

// ============================================================================
// Section 10: Trajectory Detail States
// ============================================================================

fn render_trajectory_detail_section() -> impl IntoElement {
    let agent = Agent::new("claude-code".to_string(), "1.0.0".to_string())
        .with_model("claude-3-5-sonnet".to_string());

    let mut trajectory = Trajectory::v1_4("sess_detail_demo".to_string(), agent);

    trajectory.add_step(
        Step::system(0, "You are a helpful coding assistant.".to_string())
            .with_timestamp(Utc::now()),
    );
    trajectory.add_step(
        Step::user(1, "Help me fix this bug.".to_string()).with_timestamp(Utc::now()),
    );
    trajectory.add_step(
        Step::agent(2, "I'll investigate the issue.".to_string())
            .with_timestamp(Utc::now())
            .with_model("claude-3-5-sonnet".to_string())
            .with_tool_calls(vec![ToolCall::new(
                "call_100".to_string(),
                "read_file".to_string(),
                serde_json::json!({"path": "/src/bug.rs"}),
            )]),
    );

    trajectory = trajectory.with_final_metrics(FinalMetrics {
        total_steps: Some(3),
        total_prompt_tokens: Some(5000),
        total_completion_tokens: Some(1200),
        total_cost_usd: Some(0.085),
        ..Default::default()
    });

    let expanded_ids: HashSet<i64> = [2].into_iter().collect();

    Story::section()
        .child(Story::section_title("Trajectory Detail"))
        .child(Story::description("Full trajectory view states"))
        .child(
            Story::column()
                .child(Story::item("Loading").child(render_trajectory_detail(
                    None,
                    &HashSet::new(),
                    true,
                    None,
                    false,
                )))
                .child(Story::item("Error").child(render_trajectory_detail(
                    None,
                    &HashSet::new(),
                    false,
                    Some("Failed to load trajectory: Network error"),
                    false,
                )))
                .child(Story::item("Empty (No Selection)").child(render_trajectory_detail(
                    None,
                    &HashSet::new(),
                    false,
                    None,
                    false,
                )))
                .child(Story::item("Collapsed").child(render_trajectory_detail(
                    Some(&trajectory),
                    &HashSet::new(),
                    false,
                    None,
                    true,
                )))
                .child(
                    Story::item("Full View (Step 2 Expanded)").child(render_trajectory_detail(
                        Some(&trajectory),
                        &expanded_ids,
                        false,
                        None,
                        false,
                    )),
                ),
        )
}
