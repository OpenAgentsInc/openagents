//! Performance benchmarks for trajectory collection overhead
//!
//! Measures the time and memory overhead of recording trajectory data
//! during autopilot operations.

use autopilot::trajectory::{StepType, Trajectory};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use std::time::Duration;

/// Simulate a tool call step
fn create_tool_call_step(tool: &str) -> StepType {
    StepType::ToolCall {
        tool: tool.to_string(),
        tool_id: format!("tool_{}", rand::random::<u32>()),
        input: json!({"file_path": "/test/path", "content": "test content"}),
    }
}

/// Simulate a tool result step
fn create_tool_result_step(success: bool) -> StepType {
    StepType::ToolResult {
        tool_id: format!("tool_{}", rand::random::<u32>()),
        success,
        output: Some("Tool execution completed".to_string()),
    }
}

/// Bench trajectory creation overhead
fn bench_trajectory_creation(c: &mut Criterion) {
    c.bench_function("trajectory_new", |b| {
        b.iter(|| {
            Trajectory::new(
                black_box("Test prompt".to_string()),
                black_box("sonnet-4".to_string()),
                black_box("/test/cwd".to_string()),
                black_box("abc123".to_string()),
                black_box(Some("main".to_string())),
            )
        })
    });
}

/// Bench adding steps to trajectory
fn bench_adding_steps(c: &mut Criterion) {
    let mut group = c.benchmark_group("add_steps");

    for num_steps in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_steps),
            num_steps,
            |b, &count| {
                b.iter(|| {
                    let mut trajectory = Trajectory::new(
                        "Test prompt".to_string(),
                        "sonnet-4".to_string(),
                        "/test/cwd".to_string(),
                        "abc123".to_string(),
                        Some("main".to_string()),
                    );

                    for _i in 0..count {
                        trajectory.add_step(black_box(create_tool_call_step("Read")));
                        trajectory.add_step(black_box(create_tool_result_step(true)));
                    }

                    black_box(trajectory)
                })
            },
        );
    }

    group.finish();
}

/// Bench trajectory serialization to JSON
fn bench_trajectory_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory_serialization");

    for num_steps in [10, 100, 1000].iter() {
        let mut trajectory = Trajectory::new(
            "Test prompt".to_string(),
            "sonnet-4".to_string(),
            "/test/cwd".to_string(),
            "abc123".to_string(),
            Some("main".to_string()),
        );

        for _i in 0..*num_steps {
            trajectory.add_step(create_tool_call_step("Read"));
            trajectory.add_step(create_tool_result_step(true));
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(num_steps),
            num_steps,
            |b, _count| {
                b.iter(|| {
                    let json = serde_json::to_string(black_box(&trajectory));
                    black_box(json)
                })
            },
        );
    }

    group.finish();
}

/// Bench trajectory deserialization from JSON
fn bench_trajectory_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("trajectory_deserialization");

    for num_steps in [10, 100, 1000].iter() {
        let mut trajectory = Trajectory::new(
            "Test prompt".to_string(),
            "sonnet-4".to_string(),
            "/test/cwd".to_string(),
            "abc123".to_string(),
            Some("main".to_string()),
        );

        for _i in 0..*num_steps {
            trajectory.add_step(create_tool_call_step("Read"));
            trajectory.add_step(create_tool_result_step(true));
        }

        let json = serde_json::to_string(&trajectory).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_steps),
            num_steps,
            |b, _count| {
                b.iter(|| {
                    let parsed: Result<Trajectory, _> = serde_json::from_str(black_box(&json));
                    black_box(parsed)
                })
            },
        );
    }

    group.finish();
}

/// Bench individual tool call recording overhead
fn bench_tool_call_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_call_overhead");

    for tool in ["Read", "Write", "Edit", "Bash", "Glob", "Grep"].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(tool),
            tool,
            |b, &tool_name| {
                let mut trajectory = Trajectory::new(
                    "Test prompt".to_string(),
                    "sonnet-4".to_string(),
                    "/test/cwd".to_string(),
                    "abc123".to_string(),
                    Some("main".to_string()),
                );

                b.iter(|| {
                    trajectory.add_step(black_box(create_tool_call_step(tool_name)));
                })
            },
        );
    }

    group.finish();
}

/// Bench memory overhead of trajectory storage
fn bench_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_overhead");
    group.measurement_time(Duration::from_secs(10));

    for num_steps in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}steps", num_steps)),
            num_steps,
            |b, &count| {
                b.iter(|| {
                    let mut trajectories = Vec::new();

                    // Create multiple trajectories to measure memory usage
                    for _session in 0..10 {
                        let mut trajectory = Trajectory::new(
                            "Test prompt".to_string(),
                            "sonnet-4".to_string(),
                            "/test/cwd".to_string(),
                            "abc123".to_string(),
                            Some("main".to_string()),
                        );

                        for _i in 0..count {
                            trajectory.add_step(create_tool_call_step("Read"));
                            trajectory.add_step(create_tool_result_step(true));
                        }

                        trajectories.push(trajectory);
                    }

                    black_box(trajectories)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_trajectory_creation,
    bench_adding_steps,
    bench_trajectory_serialization,
    bench_trajectory_deserialization,
    bench_tool_call_overhead,
    bench_memory_overhead
);
criterion_main!(benches);
