#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The last stage's answer parsing, on the adapter's golden token streams.

use psionic_models::deepseek_v41::fixture;
use psionic_v41_stage::{AnswerParser, AnswerPiece};

const EOS: u32 = 1;

/// Pushes `ids` one token at a time; the last push is `last` when the stream
/// doesn't end with the end token.
fn parse(ids: &[u32], last: bool) -> (String, String, Option<String>, usize) {
    let mut parser = AnswerParser::new(fixture::protocol().tokenizer(), EOS);
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
