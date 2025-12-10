//! Tests for chat command parsing
//!
//! These tests verify the command parser for the chat interface.

use commander::chat::{parse_command, ParsedCommand};

#[test]
fn test_parse_help() {
    assert_eq!(parse_command("help"), ParsedCommand::Help);
    assert_eq!(parse_command("/help"), ParsedCommand::Help);
    assert_eq!(parse_command("HELP"), ParsedCommand::Help);
}

#[test]
fn test_parse_connect() {
    assert_eq!(parse_command("connect"), ParsedCommand::Connect);
    assert_eq!(parse_command("/connect"), ParsedCommand::Connect);
}

#[test]
fn test_parse_join() {
    assert_eq!(
        parse_command("join #bitcoin"),
        ParsedCommand::Join("bitcoin".to_string())
    );
    assert_eq!(
        parse_command("join bitcoin"),
        ParsedCommand::Join("bitcoin".to_string())
    );
    assert_eq!(
        parse_command("/join #nostr-dev"),
        ParsedCommand::Join("nostr-dev".to_string())
    );
}

#[test]
fn test_parse_join_missing_channel() {
    match parse_command("join") {
        ParsedCommand::Invalid(msg) => {
            assert!(msg.contains("Usage"));
        }
        _ => panic!("Expected Invalid"),
    }
}

#[test]
fn test_parse_job() {
    assert_eq!(
        parse_command("job 5050 summarize this text"),
        ParsedCommand::Job(5050, "summarize this text".to_string())
    );
    assert_eq!(
        parse_command("/job 5100 generate image"),
        ParsedCommand::Job(5100, "generate image".to_string())
    );
}

#[test]
fn test_parse_job_invalid_kind() {
    match parse_command("job abc input") {
        ParsedCommand::Invalid(msg) => {
            assert!(msg.contains("Invalid job kind"));
        }
        _ => panic!("Expected Invalid"),
    }
}

#[test]
fn test_parse_job_missing_input() {
    match parse_command("job 5050") {
        ParsedCommand::Invalid(msg) => {
            assert!(msg.contains("Usage"));
        }
        _ => panic!("Expected Invalid"),
    }
}

#[test]
fn test_parse_clear() {
    assert_eq!(parse_command("clear"), ParsedCommand::Clear);
}

#[test]
fn test_parse_message() {
    assert_eq!(
        parse_command("hello world"),
        ParsedCommand::Message("hello world".to_string())
    );
    assert_eq!(
        parse_command("some random text"),
        ParsedCommand::Message("some random text".to_string())
    );
}

#[test]
fn test_parse_empty() {
    assert_eq!(parse_command(""), ParsedCommand::Empty);
    assert_eq!(parse_command("   "), ParsedCommand::Empty);
}

#[test]
fn test_parse_case_insensitive() {
    assert_eq!(parse_command("CONNECT"), ParsedCommand::Connect);
    assert_eq!(parse_command("Connect"), ParsedCommand::Connect);
    assert_eq!(
        parse_command("JOIN #test"),
        ParsedCommand::Join("test".to_string())
    );
}

#[test]
fn test_parse_with_leading_slash() {
    assert_eq!(parse_command("/help"), ParsedCommand::Help);
    assert_eq!(parse_command("/connect"), ParsedCommand::Connect);
    assert_eq!(
        parse_command("/join #channel"),
        ParsedCommand::Join("channel".to_string())
    );
    assert_eq!(parse_command("/clear"), ParsedCommand::Clear);
}

#[test]
fn test_parse_job_kinds() {
    // Text generation (NIP-90)
    assert_eq!(
        parse_command("job 5050 generate text"),
        ParsedCommand::Job(5050, "generate text".to_string())
    );

    // Image generation
    assert_eq!(
        parse_command("job 5100 generate image of cat"),
        ParsedCommand::Job(5100, "generate image of cat".to_string())
    );

    // Speech to text
    assert_eq!(
        parse_command("job 5250 transcribe audio"),
        ParsedCommand::Job(5250, "transcribe audio".to_string())
    );
}

#[test]
fn test_parse_unknown_command_as_message() {
    // Unknown commands are treated as messages
    assert_eq!(
        parse_command("unknown_cmd arg1 arg2"),
        ParsedCommand::Message("unknown_cmd arg1 arg2".to_string())
    );
}
