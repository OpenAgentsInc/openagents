#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Prompt modes and the last stage's answer parsing, on the adapter's golden
//! token streams and the oracle's prompt token IDs.

use psionic_models::deepseek_v41::{DeepSeekV41Protocol, fixture};
use psionic_v41_stage::{AnswerParser, AnswerPiece, PromptMode, Vocabulary};
use serde_json::json;

const EOS: u32 = 1;

/// Pushes `ids` one token at a time; the last push is `last` when the stream
/// doesn't end with the end token.
fn parse(ids: &[u32], last: bool) -> (String, String, Option<String>, usize) {
    parse_in(PromptMode::Thinking, ids, last)
}

fn parse_in(mode: PromptMode, ids: &[u32], last: bool) -> (String, String, Option<String>, usize) {
    let mut parser = AnswerParser::new(fixture::protocol().tokenizer(), EOS, mode);
    let (mut reasoning, mut content, mut finish, mut finishes) =
        (String::new(), String::new(), None, 0);
    for (i, id) in ids.iter().enumerate() {
        let AnswerPiece {
            reasoning: r,
            content: c,
            finish: f,
        } = parser.push(*id, last && i + 1 == ids.len()).unwrap();
        reasoning.push_str(&r);
        content.push_str(&c);
        if f.is_some() {
            finishes += 1;
            finish = f;
        }
    }
    (reasoning, content, finish, finishes)
}

#[test]
fn the_end_of_thinking_token_splits_reasoning_from_the_answer() {
    let (reasoning, content, finish, finishes) =
        parse(&fixture::stream_case("reasoning_then_final"), false);
    assert_eq!(reasoning, "One plus one is two.\n");
    assert!(content.contains("The answer is 2."), "{content:?}");
    assert!(!content.contains("</think>"));
    assert_eq!((finish.as_deref(), finishes), (Some("stop"), 1));
}

#[test]
fn a_literal_end_of_thinking_marker_stays_text() {
    let (reasoning, content, finish, _) =
        parse(&fixture::stream_case("literal_markers_in_text"), false);
    // No special token ended the reasoning, so there is no final answer.
    assert!(reasoning.contains("Write </think> and"), "{reasoning:?}");
    assert_eq!(content, "");
    assert_eq!(finish.as_deref(), Some("stop"));
}

#[test]
fn split_characters_come_out_whole() {
    let (reasoning, _, _, _) = parse(&fixture::stream_case("split_utf8"), false);
    assert!(reasoning.contains("Crab: 🦀 and 𝔘."), "{reasoning:?}");
    assert!(!reasoning.contains('\u{fffd}'));
}

#[test]
fn the_output_limit_reports_length() {
    let (reasoning, content, finish, _) = parse(&fixture::stream_case("truncated_reasoning"), true);
    assert!(reasoning.starts_with("Let me think"), "{reasoning:?}");
    assert_eq!(content, "");
    assert_eq!(finish.as_deref(), Some("length"));
}

/// The oracle's p2 code prompt (`scripts/bench/deepseek-v41/oracle_dump.py`),
/// which it rendered in chat mode.
const P2: &str = "Write a Rust function `fn fib(n: u32) -> Option<u128>` that returns the n-th \
Fibonacci number, using checked arithmetic so it returns None on overflow.";

/// The oracle's prompt token IDs for p2, from its manifest
/// (`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump/manifest.json`,
/// prompt `p2-code`).
const P2_IDS: [u32; 40] = [
    0, 128803, 21750, 260, 58408, 2019, 3608, 28952, 10150, 3913, 28, 549, 2111, 11, 6248, 30397,
    30, 87, 7833, 80225, 396, 10340, 270, 313, 7784, 26509, 1167, 14, 1812, 21002, 29568, 832, 436,
    10340, 9952, 377, 37126, 16, 128804, 128822,
];

/// The model's `</think>` token, which chat mode puts at the start of the
/// assistant turn.
const END_THINKING: u32 = 128822;

fn vocabulary(protocol: DeepSeekV41Protocol) -> Vocabulary {
    Vocabulary::V41(Box::new(protocol))
}

fn p2() -> serde_json::Value {
    json!([{"role": "user", "content": P2}])
}

#[test]
fn chat_mode_renders_the_oracle_p2_prompt_exactly() {
    let v = vocabulary(fixture::protocol());
    let ids = v.encode(Some(&p2()), 0, PromptMode::Chat).unwrap();
    assert_eq!(ids, P2_IDS);
    // Thinking mode adds its instructions and leaves the reasoning open, so
    // it can't stand in for the oracle's prompt.
    let thinking = v.encode(Some(&p2()), 0, PromptMode::Thinking).unwrap();
    assert_ne!(thinking, P2_IDS);
    assert_ne!(thinking.last(), Some(&END_THINKING));
}

#[test]
fn chat_mode_continues_a_conversation_as_the_checkpoint_renders_it() {
    // The checkpoint's second encoding test is a chat-mode conversation. A
    // continuation asks for the next answer after its second question, so
    // its IDs are the test's IDs up to that question's `</think>`.
    let golden = fixture::golden();
    let case = golden
        .prompt_cases
        .iter()
        .find(|c| c.name == "test_2")
        .unwrap();
    let assistant = case
        .token_ids
        .iter()
        .rposition(|id| *id == 128_804)
        .unwrap();
    let expected = &case.token_ids[..=assistant + 1];
    assert_eq!(expected.last(), Some(&END_THINKING));
    let conversation = json!([
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Hello"},
        {"role": "assistant", "content": "Hi there! How can I help you?"},
        {"role": "user", "content": "What is the capital of France?"},
    ]);
    let ids = vocabulary(fixture::protocol())
        .encode(Some(&conversation), 0, PromptMode::Chat)
        .unwrap();
    assert_eq!(ids, expected);
}

#[test]
fn chat_mode_parses_every_token_as_the_answer() {
    // In chat mode the prompt already closed the reasoning.
    let (reasoning, content, finish, _) = parse_in(
        PromptMode::Chat,
        &fixture::stream_case("truncated_reasoning"),
        true,
    );
    assert_eq!(reasoning, "");
    assert!(content.starts_with("Let me think"), "{content:?}");
    assert_eq!(finish.as_deref(), Some("length"));
}

#[test]
fn modes_parse_by_name_and_default_to_thinking() {
    assert_eq!(PromptMode::of(&json!({})).unwrap(), PromptMode::Thinking);
    assert_eq!(
        PromptMode::of(&json!({"mode": "chat"})).unwrap(),
        PromptMode::Chat
    );
    assert!(PromptMode::of(&json!({"mode": "fast"})).is_err());
    assert!(PromptMode::of(&json!({"mode": 1})).is_err());
}
