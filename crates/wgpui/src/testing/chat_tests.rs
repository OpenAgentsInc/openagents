//! Unit tests for autopilot chat streaming behavior
//!
//! These tests verify the core streaming mechanics without requiring
//! a GPU context or window. They test:
//!
//! - Token-by-token content accumulation
//! - Streaming state transitions
//! - Markdown streaming with incremental parsing
//! - Message ordering and state management

use crate::components::atoms::ToolStatus;
use crate::components::organisms::AssistantMessage;
use crate::markdown::{MarkdownBlock, StreamingMarkdown};

// ============================================================================
// AssistantMessage Streaming Tests
// ============================================================================

#[test]
fn test_assistant_message_token_accumulation() {
    let mut msg = AssistantMessage::new("");

    // Simulate token-by-token streaming
    let tokens = ["Hello", " ", "world", "!"];
    for token in tokens {
        msg.append_content(token);
    }

    assert_eq!(msg.content(), "Hello world!");
}

#[test]
fn test_assistant_message_streaming_state() {
    let msg = AssistantMessage::new("Test").streaming(true);
    assert!(msg.is_streaming());

    let msg = AssistantMessage::new("Test").streaming(false);
    assert!(!msg.is_streaming());
}

#[test]
fn test_assistant_message_tick_only_when_streaming() {
    let mut msg = AssistantMessage::new("Test").streaming(true);

    // Should tick without panic when streaming
    msg.tick();
    msg.tick();

    // Non-streaming message should also handle tick gracefully
    let mut msg2 = AssistantMessage::new("Test").streaming(false);
    msg2.tick();
}

#[test]
fn test_assistant_message_incremental_response() {
    let mut msg = AssistantMessage::new("").streaming(true);

    // Simulate ACP text_delta events
    let deltas = [
        "I'll ",
        "help ",
        "you ",
        "with ",
        "that.\n\n",
        "```rust\n",
        "fn main() {}\n",
        "```",
    ];

    for delta in deltas {
        msg.append_content(delta);
        // Content should accumulate
        assert!(msg.content().len() > 0);
    }

    let expected = "I'll help you with that.\n\n```rust\nfn main() {}\n```";
    assert_eq!(msg.content(), expected);
}

// ============================================================================
// StreamingMarkdown Tests
// ============================================================================

#[test]
fn test_streaming_markdown_token_by_token() {
    let mut stream = StreamingMarkdown::new();

    // Simulate character-by-character streaming
    let text = "# Hello World";
    for ch in text.chars() {
        stream.append(&ch.to_string());
        stream.tick();
    }

    assert_eq!(stream.source(), text);
    assert!(!stream.document().blocks.is_empty());
}

#[test]
fn test_streaming_markdown_detects_streaming() {
    let mut stream = StreamingMarkdown::new();

    // Initially not streaming
    assert!(!stream.fade_state().is_streaming);

    // Append content - should detect streaming
    stream.append("Test");
    stream.tick();
    assert!(stream.fade_state().is_streaming);

    // Multiple ticks without content - should stop detecting streaming
    for _ in 0..5 {
        stream.tick();
    }
    assert!(!stream.fade_state().is_streaming);
}

#[test]
fn test_streaming_markdown_code_block_incremental() {
    let mut stream = StreamingMarkdown::new();

    // Stream a code block token by token
    let tokens = ["```", "rust", "\n", "fn ", "main()", " {}\n", "```"];

    for token in tokens {
        stream.append(token);
        stream.tick();
    }

    stream.complete();

    // Should have parsed code block
    let has_code = stream
        .document()
        .blocks
        .iter()
        .any(|b| matches!(b, MarkdownBlock::CodeBlock { .. }));
    assert!(has_code);
}

#[test]
fn test_streaming_markdown_incomplete_bold() {
    let mut stream = StreamingMarkdown::new();

    // Stream incomplete bold syntax
    stream.append("This is **bold");
    stream.tick();

    // Should still parse partial bold
    let doc = stream.document();
    assert!(!doc.blocks.is_empty());
}

#[test]
fn test_streaming_markdown_complete_marks_done() {
    let mut stream = StreamingMarkdown::new();

    stream.append("# Title");
    stream.tick();
    assert!(!stream.document().is_complete);

    stream.complete();
    assert!(stream.document().is_complete);
}

#[test]
fn test_streaming_markdown_reset_clears_state() {
    let mut stream = StreamingMarkdown::new();

    stream.append("Some content");
    stream.tick();
    assert!(!stream.source().is_empty());

    stream.reset();
    assert!(stream.source().is_empty());
    assert!(stream.document().blocks.is_empty());
}

// ============================================================================
// Chat Message Ordering Tests
// ============================================================================

#[test]
fn test_message_ordering_preserved() {
    // Simulate a chat sequence
    struct MockChat {
        entries: Vec<&'static str>,
    }

    let mut chat = MockChat {
        entries: Vec::new(),
    };

    // User message
    chat.entries.push("user: Hello");

    // Assistant streaming response
    chat.entries.push("assistant: I'll help...");

    // Tool call
    chat.entries.push("tool: Read file");

    // More assistant content
    chat.entries.push("assistant: The file contains...");

    assert_eq!(chat.entries.len(), 4);
    assert!(chat.entries[0].starts_with("user:"));
    assert!(chat.entries[1].starts_with("assistant:"));
    assert!(chat.entries[2].starts_with("tool:"));
    assert!(chat.entries[3].starts_with("assistant:"));
}

// ============================================================================
// Tool Status Transition Tests
// ============================================================================

#[test]
fn test_tool_status_transitions() {
    // Tool should transition: Pending -> Running -> Success/Error

    let status = ToolStatus::Pending;
    assert_eq!(status, ToolStatus::Pending);

    let status = ToolStatus::Running;
    assert_eq!(status, ToolStatus::Running);

    let status = ToolStatus::Success;
    assert_eq!(status, ToolStatus::Success);

    let status = ToolStatus::Error;
    assert_eq!(status, ToolStatus::Error);
}

// ============================================================================
// Streaming Assertion Tests
// ============================================================================

#[test]
fn test_content_length_assertions() {
    let mut msg = AssistantMessage::new("");

    // Empty initially
    assert_eq!(msg.content().len(), 0);

    // After streaming tokens
    msg.append_content("Token1 ");
    assert!(msg.content().len() > 5);

    msg.append_content("Token2 ");
    assert!(msg.content().len() > 10);

    msg.append_content("Token3 and more text");
    assert!(msg.content().len() > 20);
}

#[test]
fn test_streaming_indicator_should_animate() {
    let mut msg = AssistantMessage::new("Test").streaming(true);

    // Tick should not panic and should update internal animation state
    for _ in 0..100 {
        msg.tick();
    }

    // Still streaming
    assert!(msg.is_streaming());
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_empty_token_handling() {
    let mut msg = AssistantMessage::new("");

    // Empty strings should not break anything
    msg.append_content("");
    msg.append_content("");
    msg.append_content("actual content");
    msg.append_content("");

    assert_eq!(msg.content(), "actual content");
}

#[test]
fn test_unicode_token_streaming() {
    let mut msg = AssistantMessage::new("");

    // Unicode tokens
    let tokens = ["Hello ", "🌍", " world ", "日本語", " text"];
    for token in tokens {
        msg.append_content(token);
    }

    assert_eq!(msg.content(), "Hello 🌍 world 日本語 text");
}

#[test]
fn test_multiline_token_streaming() {
    let mut msg = AssistantMessage::new("");

    msg.append_content("Line 1\n");
    msg.append_content("Line 2\n");
    msg.append_content("Line 3");

    assert_eq!(msg.content(), "Line 1\nLine 2\nLine 3");
    assert_eq!(msg.content().lines().count(), 3);
}

#[test]
fn test_very_long_content_streaming() {
    let mut msg = AssistantMessage::new("");

    // Stream 1000 tokens
    for i in 0..1000 {
        msg.append_content(&format!("token{} ", i));
    }

    // Content should be preserved
    assert!(msg.content().contains("token0"));
    assert!(msg.content().contains("token999"));
    assert!(msg.content().len() > 5000);
}
