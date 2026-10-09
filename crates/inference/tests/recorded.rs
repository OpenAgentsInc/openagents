//! Round trips over the recorded upstream streams in
//! `crates/coder/fixtures/gateway/`: decode, check, fold, re-encode, and
//! translate to Chat Completions.

mod common;

use common::{Lcg, RECORDED, lossless, recorded};
use inference::chat::{
    ChunkWriter, CompletionBuilder, completion_from_response, decode_chunk, encode_chunk,
};
use inference::event::{Event, EventBody};
use inference::item::Item;
use inference::sse::{DONE_FRAME, ResponsesDecoder, SseDecoder, StreamItem, encode_event};
use inference::stream::{Accumulator, StreamCheck, Violation};
use inference::{ResponseStatus, sse};

fn decode(name: &str) -> Vec<StreamItem> {
    ResponsesDecoder::decode_all(recorded(name).as_bytes())
        .into_iter()
        .map(|item| item.unwrap_or_else(|error| panic!("{name}: {error}")))
        .collect()
}

fn events(items: &[StreamItem]) -> Vec<&Event> {
    items
        .iter()
        .filter_map(|item| match item {
            StreamItem::Event(event) => Some(event),
            StreamItem::Done => None,
        })
        .collect()
}

#[test]
fn every_frame_decodes_losslessly() {
    for name in RECORDED {
        let mut decoder = SseDecoder::new();
        let text = recorded(name);
        let mut frames = decoder.push(text.as_bytes());
        frames.extend(decoder.finish());
        assert!(!frames.is_empty(), "{name}");
        for frame in frames {
            if frame.data == "[DONE]" {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(&frame.data).unwrap();
            let event: Event = lossless(&value, name);
            if let Some(kind) = &frame.event {
                assert_eq!(kind, event.type_name(), "{name}");
            }
        }
    }
}

#[test]
fn reencoding_gives_the_same_stream() {
    for name in RECORDED {
        let items = decode(name);
        let mut wire = String::new();
        for item in &items {
            match item {
                StreamItem::Event(event) => wire.push_str(&encode_event(event)),
                StreamItem::Done => wire.push_str(DONE_FRAME),
            }
        }
        let again: Vec<StreamItem> = ResponsesDecoder::decode_all(wire.as_bytes())
            .into_iter()
            .map(Result::unwrap)
            .collect();
        assert_eq!(again, items, "{name}");
    }
}

#[test]
fn split_anywhere_decodes_the_same() {
    for name in RECORDED {
        let whole = decode(name);
        let text = recorded(name).replace('\n', "\r\n");
        let bytes = text.as_bytes();
        let mut rng = Lcg(name.len() as u64);
        for _ in 0..20 {
            let mut decoder = ResponsesDecoder::new();
            let mut out = Vec::new();
            let mut at = 0;
            while at < bytes.len() {
                let step = 1 + rng.next(64);
                let end = (at + step).min(bytes.len());
                out.extend(decoder.push(&bytes[at..end]));
                at = end;
            }
            out.extend(decoder.finish());
            let out: Vec<StreamItem> = out.into_iter().map(Result::unwrap).collect();
            assert_eq!(out, whole, "{name} split at random points");
        }
    }
}

#[test]
fn each_upstream_departs_from_the_spec_where_we_know_it_does() {
    let what = |violations: Vec<Violation>| -> Vec<String> {
        violations
            .into_iter()
            .map(|violation| violation.what)
            .collect()
    };
    // Vercel's Gemini lane is in order but never sends [DONE].
    assert_eq!(
        what(StreamCheck::run(&decode("google-gemini-3.8-flash.sse"))),
        ["stream ended without [DONE]"]
    );
    // Vercel's GLM lane streams raw reasoning without announcing its part,
    // and never sends [DONE].
    let glm = what(StreamCheck::run(&decode("zai-glm-5.3-flash.sse")));
    assert_eq!(glm.len(), 35, "{glm:?}");
    assert!(
        glm[..34]
            .iter()
            .all(|what| what.contains("without content_part.added")),
        "{glm:?}"
    );
    assert_eq!(glm[34], "stream ended without [DONE]");
    // OpenRouter follows the spec, under OpenAI's reasoning_text names.
    assert_eq!(
        what(StreamCheck::run(&decode("stealth-space-bunny-alpha.sse"))),
        Vec::<String>::new()
    );
}

#[test]
fn folding_the_stream_gives_the_terminal_response() {
    for name in RECORDED {
        let items = decode(name);
        let terminal = events(&items)
            .into_iter()
            .rev()
            .find(|event| event.body.is_terminal())
            .and_then(|event| event.body.response().cloned())
            .unwrap();
        assert_eq!(terminal.status, ResponseStatus::Completed, "{name}");

        // Fold the deltas alone: drop each item's done copy so the text
        // must come from the deltas.
        let mut accumulator = Accumulator::new();
        for event in events(&items) {
            if matches!(event.body, EventBody::OutputItemDone(_)) || event.body.is_terminal() {
                continue;
            }
            accumulator.push(event);
        }
        let built = accumulator.finish().unwrap();
        assert_eq!(built.output_text(), terminal.output_text(), "{name}");
        assert_eq!(built.output.len(), terminal.output.len(), "{name}");
        for (built, recorded) in built.output.iter().zip(&terminal.output) {
            assert_eq!(built.type_name(), recorded.type_name(), "{name}");
            if let (Item::Reasoning(built), Item::Reasoning(recorded)) = (built, recorded) {
                assert_eq!(built.content_text(), recorded.content_text(), "{name}");
            }
        }

        // Folding everything gives the terminal response itself.
        let mut accumulator = Accumulator::new();
        for event in events(&items) {
            accumulator.push(event);
        }
        assert!(accumulator.is_finished());
        assert_eq!(accumulator.finish().unwrap(), terminal, "{name}");
    }
}

#[test]
fn chat_completions_stream_matches_the_non_streamed_translation() {
    for name in RECORDED {
        let items = decode(name);
        let terminal = events(&items)
            .into_iter()
            .rev()
            .find_map(|event| {
                event
                    .body
                    .is_terminal()
                    .then(|| event.body.response().cloned())
            })
            .flatten()
            .unwrap();
        let mut writer = ChunkWriter::new(true);
        let mut wire = String::new();
        for event in events(&items) {
            for chunk in writer.push(event) {
                wire.push_str(&encode_chunk(&chunk));
            }
        }
        assert!(writer.is_finished(), "{name}");
        wire.push_str(DONE_FRAME);

        // Decode what a Chat Completions client would read.
        let mut decoder = sse::SseDecoder::new();
        let mut frames = decoder.push(wire.as_bytes());
        frames.extend(decoder.finish());
        let mut builder = CompletionBuilder::new();
        let mut saw_done = false;
        let mut usage_last = false;
        for frame in frames {
            match decode_chunk(&frame.data).unwrap() {
                Some(chunk) => {
                    usage_last = chunk.usage.is_some() && chunk.choices.is_empty();
                    builder.push(&chunk);
                }
                None => saw_done = true,
            }
        }
        assert!(saw_done && usage_last, "{name}: usage chunk, then [DONE]");
        let streamed = builder.finish();
        let whole = completion_from_response(&terminal);
        assert_eq!(
            streamed.choices[0].message.content, whole.choices[0].message.content,
            "{name}"
        );
        assert_eq!(
            streamed.choices[0].message.reasoning, whole.choices[0].message.reasoning,
            "{name}"
        );
        assert_eq!(
            streamed.choices[0].finish_reason, whole.choices[0].finish_reason,
            "{name}"
        );
        assert_eq!(streamed.usage, whole.usage, "{name}");
        assert_eq!(streamed.id, terminal.id);
    }
}
