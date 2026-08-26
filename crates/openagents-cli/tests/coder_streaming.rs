//! Proofs that the renderer streams, and that checkpoint freezing is real.
//!
//! Both of these are easy to fake with a test that only inspects final output.
//! A renderer that buffered every chunk and drew once at the end would still
//! produce the right screen; so would one that reparsed the whole document on
//! every token. The tests here are written so that either of those *fails*:
//!
//! - Streaming is measured against a clock. A chunk must be on screen while
//!   the producer is still sending.
//! - Freezing is measured as cost. The engine reports the bytes it reparsed and
//!   the wrap cache reports the lines it re-wrapped; a non-freezing renderer
//!   blows the budget by orders of magnitude on a long document.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use openagents_cli::coder::transcript::MarkdownContent;
use openagents_cli::coder::tui::{CoderUi, Entry, Role};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

const WIDTH: u16 = 80;
const HEIGHT: u16 = 200;

fn screen(ui: &mut CoderUi) -> String {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            ui.render(f, area);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── streaming, against a clock ───────────────────────────────────────

/// The first chunk must be visible on screen strictly before the producer
/// finishes sending — and by a margin far larger than scheduling noise.
///
/// A renderer that batched until the stream closed would record
/// `first_visible >= stream_finished` and fail.
#[test]
fn a_chunk_renders_before_the_stream_completes() {
    const CHUNKS: usize = 12;
    const GAP: Duration = Duration::from_millis(25);

    let (tx, rx) = mpsc::channel::<Option<String>>();

    let producer = thread::spawn(move || {
        for i in 0..CHUNKS {
            let chunk = if i == 0 {
                "SENTINEL_FIRST is here.\n\n".to_string()
            } else {
                format!("Paragraph number {i} of the streamed answer.\n\n")
            };
            if tx.send(Some(chunk)).is_err() {
                return Instant::now();
            }
            thread::sleep(GAP);
        }
        let finished = Instant::now();
        let _ = tx.send(None);
        finished
    });

    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, String::new()));

    let mut first_visible: Option<Instant> = None;
    let mut chunks_seen_when_first_visible = 0usize;
    let mut received = 0usize;

    while let Ok(message) = rx.recv() {
        match message {
            Some(chunk) => {
                received += 1;
                ui.entries
                    .iter_mut()
                    .rfind(|e| e.role == Role::Assistant)
                    .expect("assistant entry")
                    .push_text(&chunk);

                // Draw a frame, exactly as the event loop does.
                let text = screen(&mut ui);
                if first_visible.is_none() && text.contains("SENTINEL_FIRST") {
                    first_visible = Some(Instant::now());
                    chunks_seen_when_first_visible = received;
                }
            }
            None => break,
        }
    }

    let stream_finished = producer.join().expect("producer thread");
    let first_visible = first_visible.expect("the first chunk never appeared on screen");

    assert!(
        first_visible < stream_finished,
        "the first chunk only became visible after the stream closed \
         ({:?} after) — this renderer is batching, not streaming",
        first_visible.duration_since(stream_finished)
    );

    // The producer sleeps GAP between chunks, so a genuine streaming render of
    // chunk 1 lands at least (CHUNKS - 2) gaps before the producer is done.
    let lead = stream_finished.duration_since(first_visible);
    let expected_lead = GAP * (CHUNKS as u32 - 2);
    assert!(
        lead >= expected_lead,
        "chunk 1 appeared only {lead:?} before the stream closed; \
         streaming should have given it at least {expected_lead:?} of lead"
    );

    assert_eq!(
        chunks_seen_when_first_visible, 1,
        "the first chunk should be on screen after receiving exactly one chunk"
    );
}

/// Streaming through `Entry::push_text` must render each chunk exactly once.
///
/// `Entry` seeds its renderer lazily from `Entry::text`. If the chunk reaches
/// `text` before the renderer is seeded, the seed and the push both carry it
/// and the reader sees the first chunk twice — content the model sent once,
/// rendered twice, which is the same defect class as content dropped.
#[test]
fn streaming_an_entry_renders_each_chunk_exactly_once() {
    let chunks = [
        "* Memory safety without garbage collection\n",
        "* Zero-cost abstractions\n",
        "* Concurrency without data races\n\n",
        "```rust\nfn main() {}\n```\n",
    ];

    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, String::new()));
    for chunk in chunks {
        ui.entries
            .iter_mut()
            .rfind(|e| e.role == Role::Assistant)
            .unwrap()
            .push_text(chunk);
        let _ = screen(&mut ui);
    }
    ui.entries
        .iter_mut()
        .rfind(|e| e.role == Role::Assistant)
        .unwrap()
        .finish_text();

    let text = screen(&mut ui);
    for needle in [
        "Memory safety without garbage collection",
        "Zero-cost abstractions",
        "Concurrency without data races",
        "fn main() {}",
    ] {
        assert_eq!(
            text.matches(needle).count(),
            1,
            "{needle:?} was rendered {} times:\n{text}",
            text.matches(needle).count()
        );
    }

    // And `text` still holds exactly what arrived, once.
    let entry = ui
        .entries
        .iter()
        .rfind(|e| e.role == Role::Assistant)
        .unwrap();
    assert_eq!(entry.text, chunks.concat());
}

/// An entry seeded whole and then streamed onto must not lose or repeat the
/// seed. Session replay builds entries this way.
#[test]
fn pushing_onto_a_prefilled_entry_keeps_one_copy_of_each_half() {
    let mut ui = CoderUi::new();
    ui.entries
        .push(Entry::new(Role::Assistant, "FIRST half of the answer.\n\n"));
    let _ = screen(&mut ui);
    ui.entries
        .iter_mut()
        .rfind(|e| e.role == Role::Assistant)
        .unwrap()
        .push_text("SECOND half of the answer.\n\n");
    let text = screen(&mut ui);

    assert_eq!(text.matches("FIRST half").count(), 1, "{text}");
    assert_eq!(text.matches("SECOND half").count(), 1, "{text}");
}

/// Every chunk must be on screen by the time the next one is pushed. This is
/// the stronger version: no chunk may sit invisible waiting for a later one.
#[test]
fn each_chunk_is_visible_before_the_next_arrives() {
    let mut ui = CoderUi::new();
    ui.entries.push(Entry::new(Role::Assistant, String::new()));

    for i in 0..8 {
        let marker = format!("MARK{i}");
        ui.entries
            .iter_mut()
            .rfind(|e| e.role == Role::Assistant)
            .unwrap()
            .push_text(&format!("{marker} body text for paragraph {i}.\n\n"));

        let text = screen(&mut ui);
        assert!(
            text.contains(&marker),
            "chunk {i} was not on screen after its own push:\n{text}"
        );
        // And nothing earlier fell off.
        for j in 0..=i {
            let earlier = format!("MARK{j}");
            assert!(
                text.contains(&earlier),
                "chunk {j} disappeared once chunk {i} arrived:\n{text}"
            );
        }
    }
}

// ── checkpoint freezing, measured as cost ────────────────────────────

/// Streaming an `n`-byte document must reparse `O(n)` bytes, not `O(n²)`.
///
/// `reparsed_bytes` counts exactly what the parser was handed on each render.
/// Without freezing, chunk `k` reparses the whole document, so the total is
/// about `chunks * total / 2`. With freezing, each render only sees the open
/// tail. The assertion is written against the *no-freezing* cost so it fails
/// loudly if freezing regresses, rather than against a hand-tuned constant.
#[test]
fn freezing_keeps_reparse_cost_linear() {
    const CHUNKS: usize = 400;

    let mut md = MarkdownContent::new();
    let mut total_source = 0usize;
    for i in 0..CHUNKS {
        let chunk = format!("Paragraph {i} of a long streamed answer with some words.\n\n");
        total_source += chunk.len();
        md.push(&chunk);
        // Draw, as the TUI would.
        let _ = md.lines(WIDTH as usize);
    }
    md.finish();

    let actual = md.reparsed_bytes();
    let without_freezing = (CHUNKS as u64 * total_source as u64) / 2;

    assert!(
        actual < without_freezing / 20,
        "reparsed {actual} bytes for a {total_source}-byte document; \
         a renderer with no checkpoint freezing would reparse about \
         {without_freezing}. Freezing is not taking effect."
    );
    // And the absolute shape: linear means a small multiple of the document.
    assert!(
        actual < (total_source as u64) * 4,
        "reparsed {actual} bytes for a {total_source}-byte document — \
         more than 4x linear"
    );
}

/// The same budget for the wrap layer: frozen lines are word-wrapped once.
#[test]
fn freezing_keeps_wrap_cost_linear() {
    const CHUNKS: usize = 300;

    let mut md = MarkdownContent::new();
    for i in 0..CHUNKS {
        md.push(&format!("Paragraph {i} of a long streamed answer.\n\n"));
        let _ = md.lines(WIDTH as usize);
    }
    md.finish();
    let total_lines = md.lines(WIDTH as usize).len();

    let wrapped = md.stats().lines_wrapped;
    let without_freezing = CHUNKS * total_lines / 2;

    assert!(
        wrapped < without_freezing / 10,
        "word-wrapped {wrapped} lines to display {total_lines}; \
         re-wrapping everything each chunk would be about {without_freezing}"
    );
}

/// Freezing must be *correct*, not just cheap: once a prefix is frozen, the
/// lines in it never change again, no matter what arrives afterwards.
#[test]
fn the_frozen_prefix_is_byte_identical_across_every_later_push() {
    fn flatten(lines: &[ratatui::text::Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    let mut md = MarkdownContent::new();
    let mut frozen_snapshot: Vec<String> = Vec::new();
    let mut froze_at_least_once = false;

    for i in 0..60 {
        md.push(&format!(
            "Paragraph {i} with **bold**, `code` and a [link](https://example.test/{i}).\n\n"
        ));
        let _ = md.lines(WIDTH as usize);

        let frozen_count = md.frozen_lines_count();
        if frozen_count == 0 {
            continue;
        }
        froze_at_least_once = true;

        let now = flatten(&md.pre_wrap_lines()[..frozen_count]);
        assert!(
            now.len() >= frozen_snapshot.len(),
            "the frozen prefix shrank at chunk {i}: {} -> {}",
            frozen_snapshot.len(),
            now.len()
        );
        assert_eq!(
            now[..frozen_snapshot.len()],
            frozen_snapshot[..],
            "a frozen line changed after chunk {i} — freezing is unsound"
        );
        frozen_snapshot = now;
    }

    assert!(
        froze_at_least_once,
        "nothing ever froze; the checkpoint path is dead"
    );

    // Freezing must also not have cost content: the finished render matches a
    // one-shot render of the same source.
    md.finish();
    let streamed = flatten(md.lines(WIDTH as usize));

    let mut oneshot = MarkdownContent::new();
    oneshot.push(md.source());
    oneshot.finish();
    let batch = flatten(oneshot.lines(WIDTH as usize));

    assert_eq!(
        streamed, batch,
        "streaming a document produced different output than rendering it whole"
    );
}

/// The tail really is only the tail: after freezing, one more push reparses
/// roughly one paragraph, not the whole transcript.
#[test]
fn one_more_push_reparses_only_the_open_tail() {
    let mut md = MarkdownContent::new();
    for i in 0..200 {
        md.push(&format!("Paragraph {i} of the streamed answer.\n\n"));
        let _ = md.lines(WIDTH as usize);
    }

    let before = md.reparsed_bytes();
    let source_len = md.source().len();
    assert!(source_len > 5_000, "document was too small to be a test");

    let addition = "One more paragraph arrives.\n\n";
    md.push(addition);
    let _ = md.lines(WIDTH as usize);
    let delta = md.reparsed_bytes() - before;

    assert!(
        delta < 512,
        "one {}-byte push reparsed {delta} bytes of a {source_len}-byte \
         document; only the open tail should have been reparsed",
        addition.len()
    );
}

/// A resize re-wraps, but does not re-render every past frame's worth of work
/// on subsequent frames at the same width.
#[test]
fn resize_rewraps_once_then_caches() {
    let mut md = MarkdownContent::new();
    for i in 0..40 {
        md.push(&format!(
            "Paragraph {i} of a long streamed answer with words.\n\n"
        ));
    }
    md.finish();

    let _ = md.lines(80);
    let after_first = md.stats();

    let _ = md.lines(40);
    let after_resize = md.stats();
    assert_eq!(after_resize.width_rewraps, after_first.width_rewraps + 1);

    for _ in 0..10 {
        let _ = md.lines(40);
    }
    let after_idle = md.stats();
    assert_eq!(
        after_idle.wrap_passes, after_resize.wrap_passes,
        "idle frames at an unchanged width did wrapping work"
    );
    assert_eq!(after_idle.cache_hits, after_resize.cache_hits + 10);
}

// ── streaming must not corrupt content ───────────────────────────────

/// Chunk boundaries are arbitrary. Splitting a document at every byte must
/// produce exactly what rendering it whole produces.
#[test]
fn byte_by_byte_streaming_matches_a_whole_render() {
    let source = "# Title\n\nProse with **bold** and `code`.\n\n\
                  ```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n\
                  - one\n- two\n\n> quote\n\n\
                  Math \\(E = mc^2\\) inline.\n\n\
                  | a | b |\n|---|---|\n| 1 | 2 |\n";

    let mut streamed = MarkdownContent::new();
    for ch in source.chars() {
        let mut buf = [0u8; 4];
        streamed.push(ch.encode_utf8(&mut buf));
        let _ = streamed.lines(WIDTH as usize);
    }
    streamed.finish();

    let mut whole = MarkdownContent::new();
    whole.push(source);
    whole.finish();

    let flatten = |md: &mut MarkdownContent| -> Vec<String> {
        md.lines(WIDTH as usize)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    };

    assert_eq!(flatten(&mut streamed), flatten(&mut whole));
}

/// A LaTeX delimiter split across a chunk boundary must not leak a stray
/// backslash onto the screen, and must not lose the math either.
#[test]
fn a_delimiter_split_across_chunks_is_not_mangled() {
    let mut md = MarkdownContent::new();
    md.push("energy \\");
    let _ = md.lines(WIDTH as usize);
    md.push("(E = mc^2\\) done\n\n");
    md.finish();

    let text: String = md
        .lines(WIDTH as usize)
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ");

    assert!(text.contains("energy"), "{text}");
    assert!(text.contains("done"), "{text}");
    assert!(text.contains("E = mc²"), "math was not converted: {text}");
}
