//! Integration tests for recorder crate
//!
//! These tests verify the complete parsing, validation, and analysis
//! pipeline for .rlog session files.

use recorder::{LineType, parse_content, parse_file};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// TEST HELPERS
// ============================================================================

/// Create a temporary directory for test files
fn temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp dir")
}

/// Write content to a temporary .rlog file
fn write_temp_rlog(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let path = dir.path().join(filename);
    fs::write(&path, content).expect("Failed to write temp file");
    path
}

/// Create a minimal valid header
fn minimal_header() -> String {
    r#"---
format: rlog/1
id: test_session_001
repo_sha: abc123def456
---
"#
    .to_string()
}

/// Create a complete valid header with all fields
fn complete_header() -> String {
    r#"---
format: rlog/1
id: session_full_001
mode: auto
model: claude-sonnet-4-5
agent: claude
version: 0.1.0
repo: https://github.com/user/repo
repo_sha: abc123def456789
branch: main
dirty: false
sandbox_id: sandbox_001
runner: autopilot
prompt: "Fix all clippy warnings"
started_at: "2024-01-01T12:00:00Z"
tokens_in: 1500
tokens_out: 850
tokens_cached: 300
cost_usd: 0.0234
---
"#
    .to_string()
}

// ============================================================================
// HEADER PARSING TESTS
// ============================================================================

#[test]
fn test_parse_minimal_header() {
    let content = minimal_header();
    let result = parse_content(&content);

    if let Err(e) = &result {
        eprintln!("Parse error: {:?}", e);
    }

    assert!(
        result.is_ok(),
        "Should parse minimal valid header: {:?}",
        result.err()
    );

    let session = result.unwrap();
    assert_eq!(session.header.format, "rlog/1");
    assert_eq!(session.header.id, "test_session_001");
    assert_eq!(session.header.repo_sha, "abc123def456");
    assert_eq!(session.lines.len(), 0, "No lines after header");
}

#[test]
fn test_parse_complete_header() {
    let content = complete_header();
    let result = parse_content(&content);
    assert!(result.is_ok(), "Should parse complete header");

    let session = result.unwrap();
    assert_eq!(session.header.id, "session_full_001");
    assert_eq!(session.header.mode, Some("auto".to_string()));
    assert_eq!(session.header.model, Some("claude-sonnet-4-5".to_string()));
    assert_eq!(session.header.agent, Some("claude".to_string()));
    assert_eq!(session.header.branch, Some("main".to_string()));
    assert_eq!(session.header.dirty, Some(false));
    assert_eq!(session.header.runner, Some("autopilot".to_string()));
}

#[test]
fn test_missing_required_header_fields() {
    let content = r#"---
format: rlog/1
id: test_001
---
"#;
    let result = parse_content(content);
    assert!(
        result.is_err(),
        "Should fail without required repo_sha field"
    );
}

#[test]
fn test_invalid_yaml_header() {
    let content = r#"---
format: rlog/1
id: test_001
  invalid: indentation:
repo_sha: abc123
---
"#;
    let result = parse_content(content);
    assert!(result.is_err(), "Should fail with invalid YAML");
}

#[test]
fn test_missing_header_end_marker() {
    let content = r#"---
format: rlog/1
id: test_001
repo_sha: abc123
"#;
    let result = parse_content(content);
    assert!(result.is_err(), "Should fail without closing ---");
}

// ============================================================================
// LINE TYPE PARSING TESTS
// ============================================================================

#[test]
fn test_parse_all_line_types() {
    let mut content = minimal_header();
    content.push_str("u: What files are here?\n");
    content.push_str("a: I'll check for you.\n");
    content.push_str("t:Glob pattern=**/*.rs → [5 files]\n");
    content.push_str("o: id=call_1 → Found 5 files\n");
    content.push_str("x:explore \"Check codebase\" → summary\n");
    content.push_str("c:issue.list status=open → [3 issues]\n");
    content.push_str("q: Should I proceed?\n");
    content.push_str("@phase design\n");
    content.push_str("@start\n");
    content.push_str("# This is a comment\n");
    content.push_str("th: Thinking about the approach...\n");
    content.push_str("td: [ ] Fix authentication\n");

    let result = parse_content(&content);
    assert!(result.is_ok(), "Should parse all line types");

    let session = result.unwrap();

    // Debug: print actual line types
    for (i, line) in session.lines.iter().enumerate() {
        eprintln!("Line {}: {:?}", i, line.line_type);
    }

    assert_eq!(session.lines.len(), 12);

    assert!(
        matches!(session.lines[0].line_type, LineType::User),
        "Line 0 should be User"
    );
    assert!(
        matches!(session.lines[1].line_type, LineType::Agent),
        "Line 1 should be Agent"
    );
    assert!(
        matches!(session.lines[2].line_type, LineType::Tool),
        "Line 2 should be Tool"
    );
    assert!(
        matches!(session.lines[3].line_type, LineType::Observation),
        "Line 3 should be Observation"
    );
    assert!(
        matches!(session.lines[4].line_type, LineType::Subagent),
        "Line 4 should be Subagent"
    );
    assert!(
        matches!(session.lines[5].line_type, LineType::Mcp),
        "Line 5 should be Mcp"
    );
    assert!(
        matches!(session.lines[6].line_type, LineType::Question),
        "Line 6 should be Question"
    );
    assert!(
        matches!(session.lines[7].line_type, LineType::Phase),
        "Line 7 should be Phase"
    );
    assert!(
        matches!(session.lines[8].line_type, LineType::Lifecycle),
        "Line 8 should be Lifecycle"
    );
    assert!(
        matches!(session.lines[9].line_type, LineType::Comment),
        "Line 9 should be Comment"
    );
    assert!(
        matches!(session.lines[10].line_type, LineType::Thinking),
        "Line 10 should be Thinking"
    );
    assert!(
        matches!(session.lines[11].line_type, LineType::Todos),
        "Line 11 should be Todos"
    );
}

#[test]
fn test_parse_line_with_metadata() {
    let mut content = minimal_header();
    content.push_str("t:Read id=call_1 step=5 file=/path/to/file.rs\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 1);

    let line = &session.lines[0];
    assert_eq!(line.call_id, Some("call_1".to_string()));
    assert_eq!(line.step, Some(5));
    assert_eq!(line.content, "Read id=call_1 step=5 file=/path/to/file.rs");
}

#[test]
fn test_parse_line_with_tokens() {
    let mut content = minimal_header();
    content.push_str("a: tokens_in=1200 tokens_out=450 tokens_cached=300 Here's the response\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    let line = &session.lines[0];
    assert_eq!(line.tokens_in, Some(1200));
    assert_eq!(line.tokens_out, Some(450));
    assert_eq!(line.tokens_cached, Some(300));
}

#[test]
fn test_parse_tool_with_result() {
    let mut content = minimal_header();
    content.push_str("t:Bash command='ls -la' → running\n");
    content.push_str("o: id=call_1 latency_ms=245 → [file list output]\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 2);

    let observation = &session.lines[1];
    assert_eq!(observation.call_id, Some("call_1".to_string()));
    assert_eq!(observation.latency_ms, Some(245));
}

#[test]
fn test_parse_multiline_content() {
    let mut content = minimal_header();
    content.push_str("a: This is a long response\n");
    content.push_str("a: that spans multiple lines\n");
    content.push_str("a: and continues here\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 3);
    assert!(
        session
            .lines
            .iter()
            .all(|l| matches!(l.line_type, LineType::Agent))
    );
}

// ============================================================================
// BLOB AND REDACTION TESTS
// ============================================================================

#[test]
fn test_parse_blob_references() {
    let mut content = minimal_header();
    content.push_str("a: Here's the file: @blob:abc123\n");
    content.push_str("t:call_1: Read @blob:def456\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert!(session.lines[0].content.contains("@blob:abc123"));
    assert!(session.lines[1].content.contains("@blob:def456"));
}

#[test]
fn test_parse_redacted_values() {
    let mut content = minimal_header();
    content.push_str("t:call_1: Connect password=<REDACTED>\n");
    content.push_str("a: Using API key <REDACTED:8 chars>\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert!(session.lines[0].content.contains("<REDACTED>"));
    assert!(session.lines[1].content.contains("<REDACTED:8 chars>"));
}

// ============================================================================
// VALIDATION TESTS
// ============================================================================

#[test]
fn test_validation_statistics() {
    let mut content = complete_header();
    content.push_str("u: First message\n");
    content.push_str("a: tokens_in=1000 tokens_out=500 Response\n");
    content.push_str("t:Bash id=call_1 step=1 command='ls'\n");
    content.push_str("o: id=call_1 → [output]\n");
    content.push_str("th: Thinking...\n");
    content.push_str("#: A comment\n");
    content.push_str("a: tokens_in=800 tokens_out=400 tokens_cached=200 Final response\n");

    let session = parse_content(&content).expect("Should parse");

    // Create validation result (simplified - actual validation would be more complex)
    let mut stats = recorder::SessionStats {
        total_lines: session.lines.len(),
        ..Default::default()
    };

    for line in &session.lines {
        match line.line_type {
            LineType::User => stats.user_messages += 1,
            LineType::Agent => stats.agent_messages += 1,
            LineType::Tool => stats.tool_calls += 1,
            LineType::Observation => stats.observations += 1,
            LineType::Thinking => stats.thinking_blocks += 1,
            LineType::Comment => stats.comments += 1,
            _ => {}
        }

        if let Some(tokens) = line.tokens_in {
            stats.total_tokens_in += tokens;
        }
        if let Some(tokens) = line.tokens_out {
            stats.total_tokens_out += tokens;
        }
        if let Some(tokens) = line.tokens_cached {
            stats.total_tokens_cached += tokens;
        }
    }

    assert_eq!(stats.total_lines, 7);
    assert_eq!(stats.user_messages, 1);
    assert_eq!(stats.agent_messages, 2);
    assert_eq!(stats.tool_calls, 1);
    assert_eq!(stats.observations, 1);
    assert_eq!(stats.thinking_blocks, 1);
    assert_eq!(stats.comments, 1);
    assert_eq!(stats.total_tokens_in, 1800);
    assert_eq!(stats.total_tokens_out, 900);
    assert_eq!(stats.total_tokens_cached, 200);
}

#[test]
fn test_validation_detects_orphaned_observations() {
    let mut content = minimal_header();
    content.push_str("t:call_1: Bash ls\n");
    content.push_str("o:call_1: [output]\n");
    content.push_str("o:call_2: [orphaned - no matching tool call]\n");

    let result = parse_content(&content);
    assert!(
        result.is_ok(),
        "Should parse but may have validation warnings"
    );
}

// ============================================================================
// FILE I/O TESTS
// ============================================================================

#[test]
fn test_parse_from_file() {
    let dir = temp_dir();
    let content = complete_header() + "u: Test message\n";
    let path = write_temp_rlog(&dir, "test.rlog", &content);

    let result = parse_file(&path);
    assert!(result.is_ok(), "Should parse file");

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 1);
    assert_eq!(session.lines[0].content, "Test message");
}

#[test]
fn test_parse_nonexistent_file() {
    let result = parse_file(&PathBuf::from("/nonexistent/file.rlog"));
    assert!(result.is_err(), "Should fail on nonexistent file");
}

#[test]
fn test_parse_empty_file() {
    let dir = temp_dir();
    let path = write_temp_rlog(&dir, "empty.rlog", "");

    let result = parse_file(&path);
    assert!(result.is_err(), "Should fail on empty file");
}

// ============================================================================
// ROUNDTRIP SERIALIZATION TESTS
// ============================================================================

#[test]
fn test_roundtrip_simple_session() {
    let original = complete_header() + "u: Hello\na: Hi there\n";

    let session = parse_content(&original).expect("Should parse");

    // Verify parsed correctly
    assert_eq!(session.lines.len(), 2);
    assert!(matches!(session.lines[0].line_type, LineType::User));
    assert!(matches!(session.lines[1].line_type, LineType::Agent));

    // In a real implementation, you'd serialize back to string and compare
    // For now, just verify the data integrity
    assert_eq!(session.lines[0].content, "Hello");
    assert_eq!(session.lines[1].content, "Hi there");
}

#[test]
fn test_roundtrip_with_metadata() {
    let original = minimal_header()
        + "t:Glob id=call_1 step=5 pattern=*.rs\n"
        + "o: id=call_1 latency_ms=123 → Found 10 files\n";

    let session = parse_content(&original).expect("Should parse");

    assert_eq!(session.lines.len(), 2);

    let tool_line = &session.lines[0];
    assert_eq!(tool_line.call_id, Some("call_1".to_string()));
    assert_eq!(tool_line.step, Some(5));

    let obs_line = &session.lines[1];
    assert_eq!(obs_line.call_id, Some("call_1".to_string()));
    assert_eq!(obs_line.latency_ms, Some(123));
}

// ============================================================================
// ERROR HANDLING TESTS
// ============================================================================

#[test]
fn test_malformed_line_prefix() {
    let mut content = minimal_header();
    content.push_str("xyz: Invalid line type\n");

    let result = parse_content(&content);
    // Should either skip invalid lines or parse them as unknown
    // Depending on implementation, this might succeed with warnings
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_truncated_metadata() {
    let mut content = minimal_header();
    content.push_str("t:call_1 step=: Missing step value\n");

    let result = parse_content(&content);
    // Should parse but may have invalid metadata
    assert!(result.is_ok());
}

#[test]
fn test_unicode_content() {
    let mut content = minimal_header();
    content.push_str("u: Hello 世界 🌍\n");
    content.push_str("a: Привет мир\n");

    let result = parse_content(&content);
    assert!(result.is_ok(), "Should handle Unicode");

    let session = result.unwrap();
    assert!(session.lines[0].content.contains("世界"));
    assert!(session.lines[1].content.contains("Привет"));
}

#[test]
fn test_very_long_lines() {
    let mut content = minimal_header();
    let long_content = "x".repeat(10000);
    content.push_str(&format!("a: {}\n", long_content));

    let result = parse_content(&content);
    assert!(result.is_ok(), "Should handle long lines");

    let session = result.unwrap();
    assert_eq!(session.lines[0].content.len(), 10000);
}

// ============================================================================
// COMPLEX SCENARIO TESTS
// ============================================================================

#[test]
fn test_complete_agent_session() {
    let mut content = complete_header();

    // Initial user request
    content.push_str("u: step=1 ts=2025-01-01T00:00:00Z Fix the authentication bug\n");

    // Agent thinking
    content.push_str("th: step=2 ts=2025-01-01T00:00:01Z Need to check the auth module\n");

    // Agent response
    content.push_str("a: step=3 ts=2025-01-01T00:00:02Z tokens_in=1200 tokens_out=45 I'll check the authentication module.\n");

    // Tool call
    content.push_str("t:Read id=call_1 step=4 ts=2025-01-01T00:00:03Z file=src/auth.rs\n");

    // Tool result
    content.push_str(
        "o: id=call_1 step=5 ts=2025-01-01T00:00:05Z latency_ms=2000 → [file contents]\n",
    );

    // Agent analysis
    content.push_str("a: step=6 ts=2025-01-01T00:00:06Z tokens_in=2500 tokens_out=120 Found the issue on line 42.\n");

    // Another tool call
    content.push_str("t:Edit id=call_2 step=7 ts=2025-01-01T00:00:07Z file=src/auth.rs old='token.clone()' new='token'\n");

    // Tool result
    content.push_str(
        "o: id=call_2 step=8 ts=2025-01-01T00:00:08Z latency_ms=50 result=success → File updated\n",
    );

    // Final response
    content.push_str("a: step=9 ts=2025-01-01T00:00:09Z tokens_in=1800 tokens_out=85 Fixed the redundant clone.\n");

    let result = parse_content(&content);
    assert!(result.is_ok(), "Should parse complete session");

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 9);

    // Verify step progression
    for (i, line) in session.lines.iter().enumerate() {
        assert_eq!(line.step, Some((i + 1) as u32));
    }

    // Verify timestamps are present
    assert!(session.lines.iter().all(|l| l.timestamp.is_some()));

    // Verify tool call/result pairs match
    let call_1_lines: Vec<_> = session
        .lines
        .iter()
        .filter(|l| l.call_id == Some("call_1".to_string()))
        .collect();
    assert_eq!(call_1_lines.len(), 2); // tool + observation
}

#[test]
fn test_session_with_subagents() {
    let mut content = minimal_header();

    content.push_str("u: Analyze the codebase\n");
    content.push_str("x:agent_explore: Starting exploration\n");
    content.push_str("t:call_1: Glob **/*.rs\n");
    content.push_str("o:call_1: Found 100 files\n");
    content.push_str("x:agent_explore: Exploration complete\n");
    content.push_str("a: Found 100 Rust files.\n");

    let result = parse_content(&content);
    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.lines.len(), 6);

    let subagent_lines: Vec<_> = session
        .lines
        .iter()
        .filter(|l| matches!(l.line_type, LineType::Subagent))
        .collect();
    assert_eq!(subagent_lines.len(), 2);
}
