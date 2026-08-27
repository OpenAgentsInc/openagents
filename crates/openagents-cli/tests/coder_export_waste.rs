//! The export's waste section: repeat execution made visible.
//!
//! A session that runs one suite twice pays the second run to relearn what
//! the first taught. The export names those families and what the
//! repetitions cost, so nobody re-derives it from timestamps by hand.
//!
//! Lives in its own file because the exporter resolves its output directory
//! from the process `HOME`, and integration tests run one process per file —
//! the swap cannot race a sibling test's export.

use openagents_cli::coder::export::export_trajectory;
use openagents_cli::coder::tui::{Entry, Role, now_ms};

fn delegate_call() -> openagents_cli::coder::tui::ToolCall {
    openagents_cli::coder::tui::ToolCall {
        call_id: "template".to_string(),
        function_name: "delegate".to_string(),
        arguments: serde_json::json!({}),
        output: None,
        error: None,
        done: false,
        duration_ms: None,
    }
}

#[test]
fn export_reports_repeated_command_execution_as_waste() {
    // The shape trajectory 2026-08-27 wasted minutes inside: two full-suite
    // runs, the second existing only to read what the first had truncated.
    // The export names the family and what the repetition cost, so no one
    // has to reconstruct the waste from step timestamps by hand.
    let scratch = std::env::temp_dir().join(format!("Coder-waste-{}", now_ms()));
    std::fs::create_dir_all(&scratch).unwrap();

    let suite = |call_id: &str, seconds: u64| {
        let mut entry = Entry::tool_call("shell: suite");
        entry.output = Some("8 failed | 2289 passed".to_string());
        let mut call = delegate_call();
        call.call_id = call_id.to_string();
        call.function_name = "shell".to_string();
        call.arguments = serde_json::json!({
            "command": "npx vp test --run 2>&1 | tail -8",
        });
        call.done = true;
        call.settle_duration_ms(seconds * 1000);
        entry.tool = Some(call);
        entry
    };

    let entries = vec![
        Entry::new(Role::You, "fix the tests"),
        suite("call-a", 150),
        suite("call-b", 150),
        Entry::new(Role::Assistant, "Done."),
    ];

    let previous = std::env::var("HOME").ok();
    unsafe { std::env::set_var("HOME", &scratch) };
    let result = export_trajectory(&entries, "coder-auto", "openagents", "main");
    unsafe {
        match previous {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }

    let body = std::fs::read_to_string(&result.path).unwrap();
    let document: serde_json::Value = serde_json::from_str(&body).unwrap();
    let families = document["extra"]["waste"]["repeated_command_heads"]
        .as_array()
        .expect("the waste section exists");
    let suite = families
        .iter()
        .find(|family| family["head"] == "npx vp test")
        .expect("the suite head is reported as repeated: {families:#?}");
    assert_eq!(suite["executions"], 2);
    assert_eq!(suite["approx_wasted_seconds"], 150);
    // The downstream `tail` consumer repeats too — same line, same story.
    assert!(families.iter().any(|family| family["head"] == "tail"));
}
