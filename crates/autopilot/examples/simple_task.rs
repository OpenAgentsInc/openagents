//! Simple task execution example
//!
//! This example demonstrates creating and analyzing a trajectory manually.
//! For a real Claude SDK integration, see the autopilot CLI source code.
//!
//! Run with:
//! ```bash
//! cargo run --example simple_task
//! ```

use autopilot::trajectory::{StepType, Trajectory};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Autopilot Simple Task Example");
    println!("=================================\n");

    // Define a simple task
    let task = "List all Rust files in the current directory";
    println!("Task: {}\n", task);

    // Create a trajectory
    let mut trajectory = Trajectory::new(
        task.to_string(),
        "claude-sonnet-4-5".to_string(),
        std::env::current_dir()?.to_string_lossy().to_string(),
        "example".to_string(),
        Some("main".to_string()),
    );

    trajectory.session_id = "example_001".to_string();

    // Simulate a simple conversation
    trajectory.add_step(StepType::User {
        content: task.to_string(),
    });

    trajectory.add_step(StepType::Assistant {
        content: "I'll use the Glob tool to list Rust files.".to_string(),
    });

    trajectory.add_step(StepType::ToolCall {
        tool: "Glob".to_string(),
        tool_id: "tool_1".to_string(),
        input: serde_json::json!({
            "pattern": "**/*.rs"
        }),
    });

    trajectory.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        success: true,
        output: Some("src/main.rs\nsrc/lib.rs\ntests/integration.rs".to_string()),
    });

    trajectory.add_step(StepType::Assistant {
        content: "Found 3 Rust files: src/main.rs, src/lib.rs, tests/integration.rs".to_string(),
    });

    // Set final token usage
    trajectory.usage.input_tokens = 850;
    trajectory.usage.output_tokens = 120;
    trajectory.usage.cache_read_tokens = 200;
    trajectory.usage.cost_usd = 0.0045;

    println!("✓ Task completed!\n");

    // Print statistics
    println!("Statistics:");
    println!("  Steps: {}", trajectory.steps.len());
    println!("  Tokens in: {}", trajectory.usage.input_tokens);
    println!("  Tokens out: {}", trajectory.usage.output_tokens);
    println!("  Cache reads: {}", trajectory.usage.cache_read_tokens);
    println!("  Cost: ${:.4}", trajectory.usage.cost_usd);

    // Save to JSON
    let json = trajectory.to_json();
    std::fs::write("simple_task_trajectory.json", json)?;
    println!("\n✓ Trajectory saved to simple_task_trajectory.json");

    Ok(())
}
