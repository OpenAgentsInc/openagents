//! The shaper against CoreText.
//!
//! `corpus()` lays out a sample of real prose, code, tables, and mixed
//! scripts (`fixtures/shaping-corpus.md`) as transcript rows at several
//! widths and text sizes, and records every paragraph the layout measures.
//! `tools/coretext-lines.swift` records CoreText's line breaks for the same
//! paragraphs: `fixtures/coretext-lines.json` with the bundled Paper Mono,
//! and `fixtures/coretext-lines-system.json` with the faces the desktop
//! draws with on a Mac, SF Pro for prose and SF Mono for code (#11291),
//! read from `/System/Library/Fonts` and never committed; only the break
//! positions are. The system gate runs where those files exist.
//!
//! Each gate: every paragraph breaks exactly where CoreText's does. Line
//! widths stay within half a point where the faces cover the text, and
//! within 2 points where characters the faces lack (CJK, emoji, and Greek
//! and Cyrillic in Paper Mono) are drawn from CoreText's fallback faces,
//! whose widths the shaper takes from a table measured on a Mac
//! (`shape/fallback.rs`, #11301) but without their kerning.
//!
//! Regenerate the ground truth after changing the corpus, the fonts, or the
//! layout's paragraphs:
//!
//! ```text
//! swift crates/rust-native/tools/fallback-metrics.swift \
//!     crates/paper-mono/fonts crates/rust-native/src/layout/shape/fallback.rs
//! RUST_NATIVE_WRITE_CORPUS=/tmp/corpus.json cargo test -p rust-native \
//!     --features shaping --lib shape::tests::write_corpus -- --ignored
//! swift crates/rust-native/tools/coretext-lines.swift /tmp/corpus.json \
//!     crates/paper-mono/fonts crates/rust-native/fixtures/coretext-lines.json
//! RUST_NATIVE_WRITE_CORPUS=/tmp/system.json RUST_NATIVE_CORPUS_FACES=system \
//!     cargo test -p rust-native --features shaping --lib \
//!     shape::tests::write_corpus -- --ignored
//! swift crates/rust-native/tools/coretext-lines.swift /tmp/system.json \
//!     /System/Library/Fonts/SFNS.ttf,/System/Library/Fonts/SFNSMono.ttf \
//!     crates/rust-native/fixtures/coretext-lines-system.json
//! ```

use super::super::testing::FixedMeasurer;
use super::super::{Element, MeasureRun, Measured, Measurer, Node, TranscriptLayout, Update};
use super::*;
use crate::markdown;
use crate::style::Style;
use crate::view::{MessageRole, TextRole};
use serde_json::Value;

/// Records every paragraph the layout measures.
struct Recording {
    inner: FixedMeasurer,
    cases: Vec<(String, Vec<MeasureRun>, Option<f32>)>,
}

impl Measurer for Recording {
    fn measure(&mut self, text: &str, runs: &[MeasureRun], width: Option<f32>) -> Option<Measured> {
        self.cases.push((text.to_owned(), runs.to_vec(), width));
        self.inner.measure(text, runs, width)
    }
}

fn node(key: String, element: Element<()>) -> Node<()> {
    Node {
        key,
        style: Style::default(),
        element,
    }
}

/// The corpus's sections as transcript rows: messages by alternating role,
/// and each code block again as a tool's output.
fn rows() -> Vec<Node<()>> {
    let source = include_str!("../../../fixtures/shaping-corpus.md");
    let mut rows = vec![];
    let mut section = String::new();
    let mut sections = vec![];
    let mut fenced = false;
    for line in source.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        }
        if !fenced && line.starts_with('#') && !section.trim().is_empty() {
            sections.push(std::mem::take(&mut section));
        }
        section.push_str(line);
        section.push('\n');
    }
    sections.push(section);
    for (index, text) in sections.iter().enumerate() {
        let role = if index % 3 == 1 {
            MessageRole::User
        } else {
            MessageRole::Assistant
        };
        rows.push(node(
            format!("m{index}"),
            Element::Message {
                role,
                note: None,
                children: vec![node(
                    format!("m{index}-md"),
                    Element::Markdown {
                        blocks: markdown::parse(text),
                    },
                )],
            },
        ));
        if let Some(code) = text.split("```").nth(1) {
            rows.push(node(
                format!("t{index}"),
                Element::Tool {
                    name: "Bash".into(),
                    detail: code.lines().nth(1).unwrap_or("run").into(),
                    state: crate::view::ToolState::Done,
                    children: vec![node(
                        format!("t{index}-out"),
                        Element::Text {
                            value: code.into(),
                            role: TextRole::Code,
                        },
                    )],
                },
            ));
        }
    }
    rows
}

/// Every distinct paragraph the layout measures for the corpus.
fn corpus() -> Vec<(String, Vec<MeasureRun>, Option<f32>)> {
    let rows = rows();
    let expanded: Vec<String> = rows
        .iter()
        .filter(|r| r.key.starts_with('t'))
        .map(|r| r.key.clone())
        .collect();
    let mut recording = Recording {
        inner: FixedMeasurer::default(),
        cases: vec![],
    };
    let curves: [Vec<[f32; 2]>; 2] = [
        vec![],
        vec![
            [12.0, 15.0],
            [13.0, 16.0],
            [15.0, 19.0],
            [16.0, 20.0],
            [17.0, 21.0],
            [19.0, 24.0],
            [22.0, 27.0],
        ],
    ];
    for width in [320.0, 390.0, 430.0, 768.0] {
        for curve in &curves {
            let mut layout = TranscriptLayout::new();
            layout
                .update(
                    Update {
                        width,
                        scale: 1.0,
                        order: Some(rows.iter().map(|r| r.key.clone()).collect()),
                        rows: rows.clone(),
                        expanded: expanded.clone(),
                        curve: curve.clone(),
                        ..Update::default()
                    },
                    &mut recording,
                )
                .expect("corpus lays out");
        }
    }
    let mut seen = std::collections::HashSet::new();
    recording
        .cases
        .into_iter()
        .filter(|(text, runs, width)| {
            let key = (
                text.clone(),
                runs.iter()
                    .map(|r| (r.font.bits(), r.start16, r.end16))
                    .collect::<Vec<_>>(),
                width.map(f32::to_bits),
            );
            seen.insert(key)
        })
        .collect()
}

/// A corpus case as `tools/coretext-lines.swift` reads it.
///
/// A struct, not a `json!` object, so the bytes the digest covers do not
/// depend on the build: another crate in the same build can turn on
/// serde_json's `preserve_order`, which keeps a `json!` object's keys in
/// insertion order instead of sorting them. The fields are declared in the
/// sorted order the ground truth's digest was taken over, and the numbers
/// are widened to `f64` as `json!` widened them.
#[derive(serde::Serialize)]
struct Case {
    /// Face, size, `wght`, `opsz`, `calt`, and the run's UTF-16 range.
    runs: Vec<(usize, f64, f64, f64, bool, u32, u32)>,
    text: String,
    width: f64,
}

#[derive(serde::Serialize)]
struct Corpus {
    cases: Vec<Case>,
}

fn corpus_json(measurer: &ShapingMeasurer) -> Corpus {
    let cases = corpus()
        .into_iter()
        .map(|(text, runs, width)| Case {
            runs: runs
                .iter()
                .map(|r| {
                    let spec = measurer.spec(r.font);
                    (
                        spec.face,
                        f64::from(spec.size),
                        f64::from(spec.weight),
                        f64::from(spec.optical),
                        spec.calt,
                        r.start16,
                        r.end16,
                    )
                })
                .collect(),
            text,
            width: width.map_or(0.0, f64::from),
        })
        .collect();
    Corpus { cases }
}

/// FNV-1a, stable across Rust versions, to tie the ground truth to its
/// corpus.
fn digest(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The system faces the desktop draws with on a Mac
/// (`openagents-desktop/src/typeface.rs`): SF Pro for text and SF Mono for
/// code. Read from this computer, never bundled; `None` off a Mac.
fn system_faces() -> Option<(&'static [u8], &'static [u8])> {
    let read = |path: &str| -> Option<&'static [u8]> {
        let bytes = std::fs::read(path).ok()?;
        Some(Box::leak(bytes.into_boxed_slice()))
    };
    Some((
        read("/System/Library/Fonts/SFNS.ttf")?,
        read("/System/Library/Fonts/SFNSMono.ttf")?,
    ))
}

#[test]
#[ignore = "writes the corpus for tools/coretext-lines.swift"]
fn write_corpus() {
    let path = std::env::var("RUST_NATIVE_WRITE_CORPUS").expect("RUST_NATIVE_WRITE_CORPUS");
    // RUST_NATIVE_CORPUS_FACES=system writes the runs as the system faces
    // draw them: code in the second face, and SF Pro's optical size.
    let measurer = match std::env::var("RUST_NATIVE_CORPUS_FACES").as_deref() {
        Ok("system") => {
            let (text, code) = system_faces().expect("SF Pro and SF Mono on this Mac");
            ShapingMeasurer::with_faces(text, code).unwrap()
        }
        _ => ShapingMeasurer::new(),
    };
    let corpus = corpus_json(&measurer);
    let bytes = serde_json::to_vec(&corpus).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    eprintln!(
        "wrote {} cases, digest {}",
        corpus.cases.len(),
        digest(&bytes)
    );
}

/// Line breaks over a set of paragraphs, against CoreText's.
#[derive(Debug, Default)]
struct Tally {
    paragraphs: usize,
    lines: usize,
    /// Lines that start where CoreText's do.
    same: usize,
    /// Paragraphs that break into a different number of lines.
    counts: usize,
    /// Paragraphs with any line break different from CoreText's.
    differ: usize,
}

impl Tally {
    fn ratio(&self) -> f64 {
        self.same as f64 / self.lines.max(1) as f64
    }
}

/// How the shaper's breaks compare with CoreText's: paragraphs the faces
/// cover, and paragraphs a platform fallback face partly draws.
struct Comparison {
    covered: Tally,
    fallback: Tally,
}

/// Measures the corpus with `measurer` and compares its breaks with
/// `truth` (a `tools/coretext-lines.swift` output), printing a summary and
/// every differing paragraph.
fn compare(measurer: &mut ShapingMeasurer, truth: &str, fixture: &str) -> Comparison {
    let truth: Value = serde_json::from_str(truth).unwrap();
    let corpus = corpus();
    let bytes = serde_json::to_vec(&corpus_json(measurer)).unwrap();
    assert_eq!(
        truth["corpus"].as_str().unwrap(),
        digest(&bytes),
        "the corpus changed; regenerate {fixture} (see this module's docs)"
    );
    let truth = truth["cases"].as_array().unwrap();
    assert_eq!(truth.len(), corpus.len());
    let mut comparison = Comparison {
        covered: Tally::default(),
        fallback: Tally::default(),
    };
    let faces: Vec<FontRef<'_>> = measurer.fonts.clone();
    let mut worst_width = 0.0f32;
    let mut worst_fallback = 0.0f32;
    let mut report = vec![];
    for (index, ((text, runs, width), expected)) in corpus.iter().zip(truth).enumerate() {
        // A paragraph with characters its faces lack is drawn partly in
        // CoreText's fallback faces, whose widths the shaper takes from a
        // measured table (`fallback_advance`).
        let covered = text.chars().all(|c| {
            c.is_control()
                || runs.iter().all(|r| {
                    let face = faces[measurer.spec(r.font).face];
                    face.charmap().map(c) != 0
                })
        });
        let tally = if covered {
            &mut comparison.covered
        } else {
            &mut comparison.fallback
        };
        let measured = measurer.measure(text, runs, *width).unwrap();
        let ends: Vec<u32> = expected["ends"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u32)
            .collect();
        let widths: Vec<f32> = expected["widths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        let got: Vec<u32> = measured.lines.iter().map(|l| l.end16).collect();
        tally.paragraphs += 1;
        tally.lines += ends.len();
        tally.counts += usize::from(got.len() != ends.len());
        tally.differ += usize::from(got != ends);
        let starts = |ends: &[u32]| {
            std::iter::once(0)
                .chain(ends.iter().copied())
                .take(ends.len())
                .collect::<std::collections::HashSet<u32>>()
        };
        let mine = starts(&got);
        tally.same += starts(&ends).intersection(&mine).count();
        if got == ends {
            for (line, width) in measured.lines.iter().zip(&widths) {
                let delta = (line.width - width).abs();
                if covered {
                    worst_width = worst_width.max(delta);
                } else {
                    worst_fallback = worst_fallback.max(delta);
                }
                if delta > 0.05 && std::env::var_os("SHAPE_WIDTHS").is_some() {
                    let units: Vec<u16> = text.encode_utf16().collect();
                    eprintln!(
                        "width {delta:.3} (CoreText {width}): {:?} runs {:?}",
                        String::from_utf16_lossy(
                            &units[line.start16 as usize..line.end16 as usize]
                        ),
                        runs.iter()
                            .map(|r| (r.font.size, r.font.mono, r.start16, r.end16))
                            .collect::<Vec<_>>()
                    );
                }
            }
        } else if report.len() < 80 {
            let units: Vec<u16> = text.encode_utf16().collect();
            let piece = |a: u32, b: u32| {
                String::from_utf16_lossy(&units[a as usize..(b as usize).min(units.len())])
            };
            let cut = got.iter().zip(&ends).position(|(a, b)| a != b).unwrap_or(0);
            let start = if cut == 0 { 0 } else { ends[cut - 1] };
            report.push(format!(
                "{} case {index} width {width:?} ({} lines, Rust {}): CoreText line {:?} Rust line {:?}",
                if covered { "covered" } else { "fallback" },
                ends.len(),
                got.len(),
                piece(start, ends.get(cut).copied().unwrap_or(0)),
                piece(start, got.get(cut).copied().unwrap_or(0)),
            ));
        }
    }
    for (name, tally) in [
        ("covered", &comparison.covered),
        ("fallback-face", &comparison.fallback),
    ] {
        eprintln!(
            "{fixture}: {} {name} paragraphs, {} lines: {} with a different line count, {} with any different break, {:.4}% exact line starts",
            tally.paragraphs,
            tally.lines,
            tally.counts,
            tally.differ,
            tally.ratio() * 100.0
        );
    }
    eprintln!(
        "worst width difference on lines that match: covered {worst_width:.3} pt, fallback-face {worst_fallback:.3} pt"
    );
    for line in &report {
        eprintln!("{line}");
    }
    assert!(
        worst_width < 0.5,
        "covered line widths drift from CoreText's by {worst_width} pt"
    );
    // Fallback widths are each character's alone; Hiragino kerns kana
    // against the long-vowel mark (サー, バー) by up to 0.05 em.
    assert!(
        worst_fallback < 2.0,
        "fallback-face line widths drift from CoreText's by {worst_fallback} pt"
    );
    comparison
}

// Both gates hold every paragraph to CoreText's breaks exactly, those the
// faces cover and those partly drawn from fallback faces (#11291, #11301).

#[test]
fn line_breaks_match_coretext_for_the_bundled_fonts() {
    let Comparison { covered, fallback } = compare(
        &mut ShapingMeasurer::over(None).with_fallback_widths(FallbackWidths::CoreText),
        include_str!("../../../fixtures/coretext-lines.json"),
        "fixtures/coretext-lines.json",
    );
    assert_eq!(covered.differ, 0, "{covered:?}");
    assert_eq!(fallback.differ, 0, "{fallback:?}");
}

/// The desktop's faces on a Mac, SF Pro for prose and SF Mono for code,
/// against CoreText with the same font files. SF Pro is proportional, has
/// an optical-size axis, and a `trak` table CoreText applies by size. Runs
/// only where those files exist; the fixture holds only break positions.
#[test]
fn line_breaks_match_coretext_for_the_system_fonts() {
    let Some((text, code)) = system_faces() else {
        eprintln!("SF Pro and SF Mono are not on this computer; skipping");
        return;
    };
    let Comparison { covered, fallback } = compare(
        &mut ShapingMeasurer::with_faces(text, code)
            .unwrap()
            .with_fallback_widths(FallbackWidths::CoreText),
        include_str!("../../../fixtures/coretext-lines-system.json"),
        "fixtures/coretext-lines-system.json",
    );
    assert_eq!(covered.differ, 0, "{covered:?}");
    assert_eq!(fallback.differ, 0, "{fallback:?}");
}

#[test]
fn a_paragraph_breaks_at_spaces_and_keeps_trailing_space_on_the_line() {
    let font = Font {
        family: Default::default(),
        size: 17.0,
        weight: Weight::Regular,
        italic: false,
        mono: false,
    };
    let text = "hello world again";
    let runs = [MeasureRun {
        font,
        start16: 0,
        end16: 17,
    }];
    let mut measurer = ShapingMeasurer::new();
    let one = measurer.measure(text, &runs, None).unwrap();
    assert_eq!(one.lines.len(), 1);
    let narrow = measurer
        .measure(text, &runs, Some(one.lines[0].width * 0.75))
        .unwrap();
    assert_eq!(narrow.lines.len(), 2);
    assert_eq!(
        narrow.lines[0].end16, 12,
        "the space stays on the first line"
    );
    assert!(narrow.lines[0].width < one.lines[0].width * 0.75);
    // A word wider than the line breaks between characters.
    let long = measurer.measure("abcdefghij", &runs, Some(20.0)).unwrap();
    assert!(long.lines.len() > 2);
    // Hard breaks end lines and stay on them.
    let hard = measurer.measure("a\nb\r\nc", &runs, None).unwrap();
    let ends: Vec<u32> = hard.lines.iter().map(|l| l.end16).collect();
    assert_eq!(ends, [2, 5, 6]);
    assert!(
        hard.lines[0].ascent > 10.0 && hard.lines[0].descent > 2.0,
        "{:?}",
        hard.lines[0]
    );
}

/// Cold layout and streaming with the Rust shaper. Run with
/// `cargo test --release -p rust-native --features shaping --lib
/// shape::tests::bench -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark"]
fn bench() {
    use super::super::tests::{conversation, message};
    use std::time::Instant;
    let rows = conversation(3_000);
    let order: Vec<String> = rows.iter().map(|r| r.key.clone()).collect();
    let mut measurer = ShapingMeasurer::new();
    let mut layout = TranscriptLayout::new();
    let started = Instant::now();
    let summary = layout
        .update(
            Update {
                width: 390.0,
                scale: 1.0,
                order: Some(order),
                rows,
                ..Update::default()
            },
            &mut measurer,
        )
        .unwrap();
    let cold = started.elapsed();
    let mut text = String::from("Streaming reply.");
    let mut total = std::time::Duration::ZERO;
    let mut worst = std::time::Duration::ZERO;
    for token in 0..200 {
        text.push_str(&format!(" token{token}"));
        let started = Instant::now();
        layout
            .update(
                Update {
                    width: 390.0,
                    scale: 1.0,
                    rows: vec![message("a2998", MessageRole::Assistant, &text)],
                    ..Update::default()
                },
                &mut measurer,
            )
            .unwrap();
        layout.frame();
        let spent = started.elapsed();
        total += spent;
        worst = worst.max(spent);
    }
    println!(
        "shaped cold layout of {} rows: {cold:?} ({} measurements); streamed token: mean {:?}, worst {worst:?}",
        summary.count,
        summary.measured,
        total / 200
    );
}

#[cfg(feature = "ffi")]
#[test]
fn the_c_interface_shapes_without_a_measurer_and_hands_out_the_fonts() {
    use super::super::ffi::*;
    let mut len = 0;
    let data = unsafe { rust_native_font_data(0, &mut len) };
    assert!(!data.is_null() && len > 100_000);
    // Every weight, italic, and monospace combination draws Paper Mono, the
    // one bundled face.
    for weight in 0..4 {
        for italic in 0..2 {
            for mono in 0..2 {
                assert_eq!(rust_native_font_spec(15.0, weight, italic, mono).face, 0);
            }
        }
    }
    assert_eq!(FACES.len(), 1);
    assert!(unsafe { rust_native_font_data(1, std::ptr::null_mut()) }.is_null());
    let code = rust_native_font_spec(14.4, 3, 1, 1);
    assert_eq!(
        (code.face, code.weight, code.optical, code.calt),
        (0, 700.0, 0.0, 0)
    );
    let text = rust_native_font_spec(22.0, 2, 0, 0);
    assert_eq!(
        (text.face, text.weight, text.optical, text.calt),
        (0, 600.0, 0.0, 1)
    );
    let handle = rust_native_layout_create_shaped();
    let rows = super::super::tests::conversation(6);
    let request = serde_json::to_vec(&serde_json::json!({
        "width": 390.0,
        "scale": 1.0,
        "order": rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        "rows": rows,
    }))
    .unwrap();
    let reply = unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) };
    let bytes = unsafe { std::slice::from_raw_parts(reply.data, reply.len) }.to_vec();
    unsafe { rust_native_layout_buffer_free(reply) };
    let summary: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(summary["count"], 6);
    assert!(summary["height"].as_f64().unwrap() > 100.0);
    unsafe { rust_native_layout_destroy(handle) };
}

#[test]
fn every_font_is_paper_mono_at_its_weight() {
    use super::super::display::FontFamily;
    let mut measurer = ShapingMeasurer::new();
    let mut weights = std::collections::BTreeSet::new();
    for mono in [false, true] {
        for italic in [false, true] {
            for weight in [
                Weight::Regular,
                Weight::Medium,
                Weight::Semibold,
                Weight::Bold,
            ] {
                let font = Font {
                    family: FontFamily::PaperMono,
                    size: 14.0,
                    weight,
                    italic,
                    mono,
                };
                let spec = FontSpec::of(font);
                assert_eq!(spec.face, 0);
                assert_eq!(spec.optical, 0.0);
                weights.insert(spec.weight as u32);
                let measured = measurer
                    .measure(
                        "Exactly the same font",
                        &[MeasureRun {
                            font,
                            start16: 0,
                            end16: 21,
                        }],
                        Some(400.0),
                    )
                    .unwrap();
                assert_eq!(measured.lines.len(), 1);
                // Paper Mono is fixed-pitch: 21 characters at 0.606 em.
                assert!((measured.lines[0].width - 21.0 * 14.0 * 0.606).abs() < 0.5);
            }
        }
    }
    assert_eq!(
        weights.into_iter().collect::<Vec<_>>(),
        [400, 500, 600, 700]
    );
    let row = node(
        "font-test".into(),
        Element::Text {
            value: "Typography and spacing".into(),
            role: TextRole::Body,
        },
    );
    let mut layout = TranscriptLayout::new();
    let update = || Update {
        width: 400.0,
        scale: 1.0,
        order: Some(vec![row.key.clone()]),
        rows: vec![row.clone()],
        ..Update::default()
    };
    layout.update(update(), &mut measurer).unwrap();
    assert_eq!(
        layout.frame().display(0).unwrap().styles[0].font.family,
        FontFamily::PaperMono
    );
    // Setting the one family again changes nothing, so nothing is measured.
    layout.set_font_family(FontFamily::PaperMono);
    assert_eq!(layout.update(update(), &mut measurer).unwrap().relaid, 0);
}

/// CoreText's fallback widths on a Mac (#11301), as
/// `tools/fallback-metrics.swift` measured them: Apple Color Emoji in whole points that grow faster than
/// the size at text sizes, Apple SD Gothic Neo's Hangul at 0.865 em, Han an
/// em, and Menlo under a monospace face.
#[test]
fn fallback_widths_are_coretexts() {
    use Fallback::{Monospace, Proportional};
    for (size, points) in [
        (12.0, 16.0),
        (14.4, 19.0),
        (16.75, 21.0),
        (20.0, 23.0),
        (27.0, 27.0),
        (40.0, 40.0),
    ] {
        for emoji in ["🎉", "🇺🇸", "👨\u{200d}👩\u{200d}👧", "👍🏽", "✔\u{fe0f}"]
        {
            assert_eq!(
                fallback_advance(emoji, size, Proportional),
                points,
                "{emoji} at {size}"
            );
            assert_eq!(
                fallback_advance(emoji, size, Monospace),
                points,
                "{emoji} at {size}"
            );
        }
    }
    let close = |cluster: &str, fallback, em: f64| {
        let got = fallback_advance(cluster, 10.0, fallback) / 10.0;
        assert!((got - em).abs() < 0.001, "{cluster}: {got} em, not {em}");
    };
    close("가", Proportional, 0.865);
    close("한", Monospace, 0.865);
    close("中", Proportional, 1.0);
    close("、", Monospace, 1.0);
    close("✔", Monospace, 0.602);
    close("✔\u{fe0e}", Monospace, 0.602);
    close("✔", Proportional, 0.846);
    close("Ж", Monospace, 0.602);
    // A flag the face shapes as two clusters is one grapheme.
    assert_eq!(grapheme_at("a🇺🇸", 1), Some("🇺🇸"));
    assert_eq!(grapheme_at("a🇺🇸", 5), None);
    // Paper Mono's Regular instance falls back like a proportional face.
    let font = |weight| Font {
        family: Default::default(),
        size: 15.0,
        weight,
        italic: false,
        mono: false,
    };
    assert_eq!(FontSpec::of(font(Weight::Regular)).fallback, Proportional);
    assert_eq!(FontSpec::of(font(Weight::Bold)).fallback, Monospace);
}

/// On Linux and Windows a character the faces lack is measured from the
/// fallback face the painter draws it with (#11304): the measured width is
/// that face's shaped advance, and CoreText's only where no face on this
/// computer has the character. Where the usual faces are installed, their
/// widths are pinned.
#[test]
fn fallback_widths_are_the_system_faces_where_the_platform_draws_them() {
    use swash::{StringId, shape::ShapeContext};
    let font = Font {
        family: Default::default(),
        size: 20.0,
        weight: Weight::Regular,
        italic: false,
        mono: false,
    };
    let mut measurer = ShapingMeasurer::over(None).with_fallback_widths(FallbackWidths::System);
    let spec = measurer.spec(font);
    let mut context = ShapeContext::new();
    // (cluster, the face's family, its width in ems)
    let pins: &[(&str, &str, f64)] = if cfg!(target_os = "linux") {
        &[
            ("中", "Noto Sans CJK", 1.0),
            ("あ", "Noto Sans CJK", 1.0),
            ("가", "Noto Sans CJK", 0.92),
            ("😀", "Noto Color Emoji", 1.2451),
            ("🇺🇸", "Noto Color Emoji", 1.2451),
            ("👨\u{200d}👩\u{200d}👧", "Noto Color Emoji", 1.2451),
            ("Ж", "DejaVu Sans", 1.0771), // check-fonts: allow
            ("α", "DejaVu Sans", 0.6592), // check-fonts: allow
        ]
    } else {
        &[("中", "Microsoft YaHei", 1.0)]
    };
    let mut pinned = 0;
    for cluster in [
        "中",
        "あ",
        "가",
        "😀",
        "🇺🇸",
        "👨\u{200d}👩\u{200d}👧",
        "Ж",
        "α",
        "✔",
    ] {
        let length = cluster.encode_utf16().count() as u32;
        let run = MeasureRun {
            start16: 0,
            end16: length,
            font,
        };
        let measured = f64::from(measurer.measure(cluster, &[run], None).unwrap().lines[0].width);
        let Some(glyphs) = system::fallback_glyphs(&mut context, cluster, &spec) else {
            let table = fallback_advance(cluster, 20.0, spec.fallback);
            assert!((measured - table).abs() < 1e-3, "{cluster}: {measured}");
            continue;
        };
        assert!(
            (measured - glyphs.advance).abs() < 1e-3,
            "{cluster}: measured {measured}, drawn {}",
            glyphs.advance
        );
        let face = system::face_at(glyphs.face).unwrap();
        let family = face
            .localized_strings()
            .find_by_id(StringId::Family, Some("en"))
            .map(|name| name.chars().collect::<String>())
            .unwrap_or_default();
        eprintln!("{cluster}: {family}, {:.4} em", measured / 20.0);
        if cfg!(any(target_os = "linux", target_os = "windows")) {
            for (pin, name, em) in pins {
                if *pin == cluster && family.starts_with(name) {
                    pinned += 1;
                    assert!(
                        (measured / 20.0 - em).abs() < 1e-3,
                        "{cluster} in {family}: {} em, not {em}",
                        measured / 20.0
                    );
                }
            }
        }
    }
    eprintln!("{pinned} widths pinned");
}
