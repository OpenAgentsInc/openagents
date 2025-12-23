//! Unit tests for the rlog writer module
//! Unit tests for the rlog writer module


use autopilot::rlog::RlogWriter;
use autopilot::rlog::RlogWriter;
use autopilot::trajectory::{StepType, TokenUsage, Trajectory};
use autopilot::trajectory::{StepType, TokenUsage, Trajectory};
use serde_json::json;
use serde_json::json;
use std::fs;
use std::fs;
use tempfile::TempDir;
use tempfile::TempDir;


#[test]
#[test]
fn test_header_format() {
fn test_header_format() {
    let traj = create_test_trajectory();
    let traj = create_test_trajectory();
    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    // Verify header contains all required fields
    // Verify header contains all required fields
    assert!(output.contains("---"));
    assert!(output.contains("---"));
    assert!(output.contains("format: rlog/1"));
    assert!(output.contains("format: rlog/1"));
    assert!(output.contains(&format!("id: {}", traj.session_id)));
    assert!(output.contains(&format!("id: {}", traj.session_id)));
    assert!(output.contains(&format!("repo_sha: {}", traj.repo_sha)));
    assert!(output.contains(&format!("repo_sha: {}", traj.repo_sha)));
    assert!(output.contains(&format!("branch: {}", traj.branch.as_ref().unwrap())));
    assert!(output.contains(&format!("branch: {}", traj.branch.as_ref().unwrap())));
    assert!(output.contains(&format!("model: {}", traj.model)));
    assert!(output.contains(&format!("model: {}", traj.model)));
    assert!(output.contains(&format!("cwd: {}", traj.cwd)));
    assert!(output.contains(&format!("cwd: {}", traj.cwd)));
    assert!(output.contains("agent: autopilot"));
    assert!(output.contains("agent: autopilot"));
    assert!(output.contains("version:"));
    assert!(output.contains("version:"));
}
}


#[test]
#[test]
fn test_token_metadata_in_header() {
fn test_token_metadata_in_header() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.usage = TokenUsage {
    traj.usage = TokenUsage {
        input_tokens: 1000,
        input_tokens: 1000,
        output_tokens: 500,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_read_tokens: 200,
        cache_creation_tokens: 100,
        cache_creation_tokens: 100,
        cost_usd: 0.05,
        cost_usd: 0.05,
    };
    };


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("tokens_total_in: 1000"));
    assert!(output.contains("tokens_total_in: 1000"));
    assert!(output.contains("tokens_total_out: 500"));
    assert!(output.contains("tokens_total_out: 500"));
    assert!(output.contains("tokens_cached: 200"));
    assert!(output.contains("tokens_cached: 200"));
}
}


#[test]
#[test]
fn test_start_marker() {
fn test_start_marker() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.session_id = "test_session_123".to_string();
    traj.session_id = "test_session_123".to_string();
    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    // Check @start marker with short ID and timestamp
    // Check @start marker with short ID and timestamp
    assert!(output.contains("@start"));
    assert!(output.contains("@start"));
    assert!(output.contains("id=test_ses")); // First 8 chars
    assert!(output.contains("id=test_ses")); // First 8 chars
    assert!(output.contains("ts="));
    assert!(output.contains("ts="));
}
}


#[test]
#[test]
fn test_end_marker() {
fn test_end_marker() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.usage.input_tokens = 1234;
    traj.usage.input_tokens = 1234;
    traj.usage.output_tokens = 567;
    traj.usage.output_tokens = 567;
    traj.usage.cost_usd = 0.0789;
    traj.usage.cost_usd = 0.0789;


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("@end tokens_in=1234 tokens_out=567 cost_usd=0.0789"));
    assert!(output.contains("@end tokens_in=1234 tokens_out=567 cost_usd=0.0789"));
}
}


#[test]
#[test]
fn test_user_message_format() {
fn test_user_message_format() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "Tell me about Rust".to_string(),
        content: "Tell me about Rust".to_string(),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("u: Tell me about Rust"));
    assert!(output.contains("u: Tell me about Rust"));
}
}


#[test]
#[test]
fn test_assistant_message_format() {
fn test_assistant_message_format() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::Assistant {
    traj.add_step(StepType::Assistant {
        content: "Rust is a systems programming language".to_string(),
        content: "Rust is a systems programming language".to_string(),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("a: Rust is a systems programming language"));
    assert!(output.contains("a: Rust is a systems programming language"));
}
}


#[test]
#[test]
fn test_thinking_format() {
fn test_thinking_format() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "I need to analyze this carefully".to_string(),
        content: "I need to analyze this carefully".to_string(),
        signature: Some("sig_abc123def456".to_string()),
        signature: Some("sig_abc123def456".to_string()),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("th: I need to analyze this carefully"));
    assert!(output.contains("th: I need to analyze this carefully"));
    assert!(output.contains("sig=sig_abc123def456..."));
    assert!(output.contains("sig=sig_abc123def456..."));
}
}


#[test]
#[test]
fn test_thinking_without_signature() {
fn test_thinking_without_signature() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "Thinking without sig".to_string(),
        content: "Thinking without sig".to_string(),
        signature: None,
        signature: None,
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("th: Thinking without sig"));
    assert!(output.contains("th: Thinking without sig"));
    assert!(!output.contains("sig="));
    assert!(!output.contains("sig="));
}
}


#[test]
#[test]
fn test_tool_call_read() {
fn test_tool_call_read() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Read".to_string(),
        tool: "Read".to_string(),
        tool_id: "toolu_abc123def456".to_string(),
        tool_id: "toolu_abc123def456".to_string(),
        input: json!({"file_path": "/path/to/file.rs"}),
        input: json!({"file_path": "/path/to/file.rs"}),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("t!:Read"));
    assert!(output.contains("t!:Read"));
    assert!(output.contains("id=23def456")); // Last 8 chars of tool_id
    assert!(output.contains("id=23def456")); // Last 8 chars of tool_id
    assert!(output.contains("file_path=/path/to/file.rs"));
    assert!(output.contains("file_path=/path/to/file.rs"));
    assert!(output.contains("→ [running]"));
    assert!(output.contains("→ [running]"));
}
}


#[test]
#[test]
fn test_tool_call_bash() {
fn test_tool_call_bash() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Bash".to_string(),
        tool: "Bash".to_string(),
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        input: json!({"command": "cargo test --all"}),
        input: json!({"command": "cargo test --all"}),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("t!:Bash"));
    assert!(output.contains("t!:Bash"));
    assert!(output.contains("id=tool_1"));
    assert!(output.contains("id=tool_1"));
    assert!(output.contains("cmd=\"cargo test --all\""));
    assert!(output.contains("cmd=\"cargo test --all\""));
}
}


#[test]
#[test]
fn test_tool_call_glob() {
fn test_tool_call_glob() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Glob".to_string(),
        tool: "Glob".to_string(),
        tool_id: "tool_2".to_string(),
        tool_id: "tool_2".to_string(),
        input: json!({"pattern": "**/*.rs"}),
        input: json!({"pattern": "**/*.rs"}),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("t!:Glob"));
    assert!(output.contains("t!:Glob"));
    assert!(output.contains("pattern=\"**/*.rs\""));
    assert!(output.contains("pattern=\"**/*.rs\""));
}
}


#[test]
#[test]
fn test_tool_call_grep() {
fn test_tool_call_grep() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Grep".to_string(),
        tool: "Grep".to_string(),
        tool_id: "tool_3".to_string(),
        tool_id: "tool_3".to_string(),
        input: json!({"pattern": "fn main"}),
        input: json!({"pattern": "fn main"}),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("t!:Grep"));
    assert!(output.contains("t!:Grep"));
    assert!(output.contains("pattern=\"fn main\""));
    assert!(output.contains("pattern=\"fn main\""));
}
}


#[test]
#[test]
fn test_tool_result_success() {
fn test_tool_result_success() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "toolu_abc123".to_string(),
        tool_id: "toolu_abc123".to_string(),
        success: true,
        success: true,
        output: Some("Found 10 files".to_string()),
        output: Some("Found 10 files".to_string()),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("o: id=u_abc123")); // Last 8 chars of "toolu_abc123"
    assert!(output.contains("o: id=u_abc123")); // Last 8 chars of "toolu_abc123"
    assert!(output.contains("→ [ok] Found 10 files"));
    assert!(output.contains("→ [ok] Found 10 files"));
}
}


#[test]
#[test]
fn test_tool_result_error() {
fn test_tool_result_error() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "tool_xyz".to_string(),
        tool_id: "tool_xyz".to_string(),
        success: false,
        success: false,
        output: Some("File not found".to_string()),
        output: Some("File not found".to_string()),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("o: id=tool_xyz"));
    assert!(output.contains("o: id=tool_xyz"));
    assert!(output.contains("→ [error] File not found"));
    assert!(output.contains("→ [error] File not found"));
}
}


#[test]
#[test]
fn test_tool_result_no_output() {
fn test_tool_result_no_output() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        success: true,
        success: true,
        output: None,
        output: None,
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("o: id=tool_1 → [ok]"));
    assert!(output.contains("o: id=tool_1 → [ok]"));
}
}


#[test]
#[test]
fn test_system_init() {
fn test_system_init() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::SystemInit {
    traj.add_step(StepType::SystemInit {
        model: "claude-sonnet-4-5".to_string(),
        model: "claude-sonnet-4-5".to_string(),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("@init model=claude-sonnet-4-5"));
    assert!(output.contains("@init model=claude-sonnet-4-5"));
}
}


#[test]
#[test]
fn test_system_status() {
fn test_system_status() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::SystemStatus {
    traj.add_step(StepType::SystemStatus {
        status: "Processing request".to_string(),
        status: "Processing request".to_string(),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("# status: Processing request"));
    assert!(output.contains("# status: Processing request"));
}
}


#[test]
#[test]
fn test_token_metadata_in_steps() {
fn test_token_metadata_in_steps() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    {
    {
        let step = traj.add_step(StepType::Assistant {
        let step = traj.add_step(StepType::Assistant {
            content: "Response".to_string(),
            content: "Response".to_string(),
        });
        });
        step.tokens_in = Some(1200);
        step.tokens_in = Some(1200);
        step.tokens_out = Some(450);
        step.tokens_out = Some(450);
        step.tokens_cached = Some(100);
        step.tokens_cached = Some(100);
    }
    }


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    let lines: Vec<&str> = output.lines().collect();
    let lines: Vec<&str> = output.lines().collect();
    let assistant_line = lines.iter().find(|l| l.starts_with("a:")).expect("Should have assistant line");
    let assistant_line = lines.iter().find(|l| l.starts_with("a:")).expect("Should have assistant line");


    assert!(assistant_line.contains("tokens_in=1200"));
    assert!(assistant_line.contains("tokens_in=1200"));
    assert!(assistant_line.contains("tokens_out=450"));
    assert!(assistant_line.contains("tokens_out=450"));
    assert!(assistant_line.contains("tokens_cached=100"));
    assert!(assistant_line.contains("tokens_cached=100"));
}
}


#[test]
#[test]
fn test_zero_cached_tokens_omitted() {
fn test_zero_cached_tokens_omitted() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    {
    {
        let step = traj.add_step(StepType::Assistant {
        let step = traj.add_step(StepType::Assistant {
            content: "Response".to_string(),
            content: "Response".to_string(),
        });
        });
        step.tokens_in = Some(100);
        step.tokens_in = Some(100);
        step.tokens_out = Some(50);
        step.tokens_out = Some(50);
        step.tokens_cached = Some(0); // Zero should be omitted
        step.tokens_cached = Some(0); // Zero should be omitted
    }
    }


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    let lines: Vec<&str> = output.lines().collect();
    let lines: Vec<&str> = output.lines().collect();
    let assistant_line = lines.iter().find(|l| l.starts_with("a:")).expect("Should have assistant line");
    let assistant_line = lines.iter().find(|l| l.starts_with("a:")).expect("Should have assistant line");


    assert!(!assistant_line.contains("tokens_cached"));
    assert!(!assistant_line.contains("tokens_cached"));
}
}


#[test]
#[test]
fn test_content_truncation() {
fn test_content_truncation() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    let long_content = "a".repeat(300); // Longer than 200 char limit for user messages
    let long_content = "a".repeat(300); // Longer than 200 char limit for user messages
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: long_content,
        content: long_content,
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    let lines: Vec<&str> = output.lines().collect();
    let lines: Vec<&str> = output.lines().collect();
    let user_line = lines.iter().find(|l| l.starts_with("u:")).expect("Should have user line");
    let user_line = lines.iter().find(|l| l.starts_with("u:")).expect("Should have user line");


    assert!(user_line.len() < 210); // "u: " + 200 chars + "..."
    assert!(user_line.len() < 210); // "u: " + 200 chars + "..."
    assert!(user_line.ends_with("..."));
    assert!(user_line.ends_with("..."));
}
}


#[test]
#[test]
fn test_multiline_content_first_line_only() {
fn test_multiline_content_first_line_only() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "First line\nSecond line\nThird line".to_string(),
        content: "First line\nSecond line\nThird line".to_string(),
    });
    });


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    let lines: Vec<&str> = output.lines().collect();
    let lines: Vec<&str> = output.lines().collect();
    let user_line = lines.iter().find(|l| l.starts_with("u:")).expect("Should have user line");
    let user_line = lines.iter().find(|l| l.starts_with("u:")).expect("Should have user line");


    assert_eq!(*user_line, "u: First line");
    assert_eq!(*user_line, "u: First line");
}
}


#[test]
#[test]
fn test_streaming_mode_creates_file() {
fn test_streaming_mode_creates_file() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = dir.path().join("test.rlog");
    let file_path = dir.path().join("test.rlog");


    let traj = create_test_trajectory();
    let traj = create_test_trajectory();
    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");
    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");


    writer.write_header(&traj).expect("Failed to write header");
    writer.write_header(&traj).expect("Failed to write header");
    writer.close().expect("Failed to close");
    writer.close().expect("Failed to close");


    assert!(file_path.exists());
    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    assert!(content.contains("format: rlog/1"));
    assert!(content.contains("format: rlog/1"));
}
}


#[test]
#[test]
fn test_streaming_mode_append_step() {
fn test_streaming_mode_append_step() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = dir.path().join("test.rlog");
    let file_path = dir.path().join("test.rlog");


    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "Test message".to_string(),
        content: "Test message".to_string(),
    });
    });


    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");
    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");
    writer.write_header(&traj).expect("Failed to write header");
    writer.write_header(&traj).expect("Failed to write header");
    // Get the step reference after header is written
    // Get the step reference after header is written
    let step = &traj.steps[0];
    let step = &traj.steps[0];
    writer.append_step(step).expect("Failed to append step");
    writer.append_step(step).expect("Failed to append step");
    writer.close().expect("Failed to close");
    writer.close().expect("Failed to close");


    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    assert!(content.contains("u: Test message"));
    assert!(content.contains("u: Test message"));
}
}


#[test]
#[test]
fn test_streaming_mode_footer() {
fn test_streaming_mode_footer() {
    let dir = TempDir::new().expect("Failed to create temp dir");
    let dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = dir.path().join("test.rlog");
    let file_path = dir.path().join("test.rlog");


    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.usage.input_tokens = 999;
    traj.usage.input_tokens = 999;
    traj.usage.output_tokens = 888;
    traj.usage.output_tokens = 888;
    traj.usage.cost_usd = 0.123;
    traj.usage.cost_usd = 0.123;


    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");
    let mut writer = RlogWriter::new_streaming(&file_path).expect("Failed to create streaming writer");
    writer.write_header(&traj).expect("Failed to write header");
    writer.write_header(&traj).expect("Failed to write header");
    writer.write_footer(&traj).expect("Failed to write footer");
    writer.write_footer(&traj).expect("Failed to write footer");
    writer.close().expect("Failed to close");
    writer.close().expect("Failed to close");


    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    assert!(content.contains("@end tokens_in=999 tokens_out=888 cost_usd=0.1230"));
    assert!(content.contains("@end tokens_in=999 tokens_out=888 cost_usd=0.1230"));
}
}


#[test]
#[test]
fn test_complete_session_workflow() {
fn test_complete_session_workflow() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();


    // Add various step types
    // Add various step types
    traj.add_step(StepType::SystemInit {
    traj.add_step(StepType::SystemInit {
        model: "claude-sonnet-4".to_string(),
        model: "claude-sonnet-4".to_string(),
    });
    });
    traj.add_step(StepType::User {
    traj.add_step(StepType::User {
        content: "List all Rust files".to_string(),
        content: "List all Rust files".to_string(),
    });
    });
    traj.add_step(StepType::Thinking {
    traj.add_step(StepType::Thinking {
        content: "I'll use Glob to find Rust files".to_string(),
        content: "I'll use Glob to find Rust files".to_string(),
        signature: None,
        signature: None,
    });
    });
    traj.add_step(StepType::ToolCall {
    traj.add_step(StepType::ToolCall {
        tool: "Glob".to_string(),
        tool: "Glob".to_string(),
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        input: json!({"pattern": "**/*.rs"}),
        input: json!({"pattern": "**/*.rs"}),
    });
    });
    traj.add_step(StepType::ToolResult {
    traj.add_step(StepType::ToolResult {
        tool_id: "tool_1".to_string(),
        tool_id: "tool_1".to_string(),
        success: true,
        success: true,
        output: Some("Found 42 files".to_string()),
        output: Some("Found 42 files".to_string()),
    });
    });
    traj.add_step(StepType::Assistant {
    traj.add_step(StepType::Assistant {
        content: "Found 42 Rust files in the project".to_string(),
        content: "Found 42 Rust files in the project".to_string(),
    });
    });


    traj.usage = TokenUsage {
    traj.usage = TokenUsage {
        input_tokens: 500,
        input_tokens: 500,
        output_tokens: 100,
        output_tokens: 100,
        cache_read_tokens: 50,
        cache_read_tokens: 50,
        cache_creation_tokens: 25,
        cache_creation_tokens: 25,
        cost_usd: 0.02,
        cost_usd: 0.02,
    };
    };


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    // Verify structure
    // Verify structure
    assert!(output.starts_with("---"));
    assert!(output.starts_with("---"));
    assert!(output.contains("@start"));
    assert!(output.contains("@start"));
    assert!(output.contains("@init"));
    assert!(output.contains("@init"));
    assert!(output.contains("u:"));
    assert!(output.contains("u:"));
    assert!(output.contains("th:"));
    assert!(output.contains("th:"));
    assert!(output.contains("t!:"));
    assert!(output.contains("t!:"));
    assert!(output.contains("o:"));
    assert!(output.contains("o:"));
    assert!(output.contains("a:"));
    assert!(output.contains("a:"));
    assert!(output.ends_with("@end tokens_in=500 tokens_out=100 cost_usd=0.0200"));
    assert!(output.ends_with("@end tokens_in=500 tokens_out=100 cost_usd=0.0200"));
}
}


#[test]
#[test]
fn test_branch_optional() {
fn test_branch_optional() {
    let traj = Trajectory::new(
    let traj = Trajectory::new(
        "Test".to_string(),
        "Test".to_string(),
        "claude".to_string(),
        "claude".to_string(),
        "/test".to_string(),
        "/test".to_string(),
        "sha".to_string(),
        "sha".to_string(),
        None, // No branch
        None, // No branch
    );
    );


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    // Should not contain branch field
    // Should not contain branch field
    assert!(!output.contains("branch:"));
    assert!(!output.contains("branch:"));
}
}


#[test]
#[test]
fn test_short_session_id() {
fn test_short_session_id() {
    let mut traj = create_test_trajectory();
    let mut traj = create_test_trajectory();
    traj.session_id = "short".to_string(); // Less than 8 chars
    traj.session_id = "short".to_string(); // Less than 8 chars


    let mut writer = RlogWriter::new();
    let mut writer = RlogWriter::new();
    let output = writer.write(&traj);
    let output = writer.write(&traj);


    assert!(output.contains("id=short")); // Should use full ID
    assert!(output.contains("id=short")); // Should use full ID
}
}


/// Helper function to create a basic test trajectory
/// Helper function to create a basic test trajectory
fn create_test_trajectory() -> Trajectory {
fn create_test_trajectory() -> Trajectory {
    Trajectory::new(
    Trajectory::new(
        "Test prompt".to_string(),
        "Test prompt".to_string(),
        "claude-sonnet-4".to_string(),
        "claude-sonnet-4".to_string(),
        "/test/cwd".to_string(),
        "/test/cwd".to_string(),
        "abc123".to_string(),
        "abc123".to_string(),
        Some("main".to_string()),
        Some("main".to_string()),
    )
    )
}
}
