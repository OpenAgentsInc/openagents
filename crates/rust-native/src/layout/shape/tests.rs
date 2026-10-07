//! The shaper against CoreText.
//!
//! `corpus()` lays out a sample of real prose, code, tables, and mixed
//! scripts (`fixtures/shaping-corpus.md`) as transcript rows at several
//! widths and text sizes, and records every paragraph the layout measures.
//! `fixtures/coretext-lines.json` holds CoreText's line breaks for the same
//! paragraphs with the same bundled font, made by
//! `tools/coretext-lines.swift`. The gate: at most one paragraph the face
//! covers breaks into a different number of lines, and at least 99.7% of
//! lines start where CoreText's do. Paragraphs with characters Paper Mono
//! lacks are reported but not gated, since a platform fallback face draws
//! them.
//!
//! Regenerate the ground truth after changing the corpus, the fonts, or the
//! layout's paragraphs:
//!
//! ```text
//! RUST_NATIVE_WRITE_CORPUS=/tmp/corpus.json cargo test -p rust-native \
//!     --features shaping --lib shape::tests::write_corpus -- --ignored
//! swift crates/rust-native/tools/coretext-lines.swift /tmp/corpus.json \
//!     crates/paper-mono/fonts crates/rust-native/fixtures/coretext-lines.json
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

fn corpus_json() -> Corpus {
    let cases = corpus()
        .into_iter()
        .map(|(text, runs, width)| Case {
            runs: runs
                .iter()
                .map(|r| {
                    let spec = FontSpec::of(r.font);
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

#[test]
#[ignore = "writes the corpus for tools/coretext-lines.swift"]
fn write_corpus() {
    let path = std::env::var("RUST_NATIVE_WRITE_CORPUS").expect("RUST_NATIVE_WRITE_CORPUS");
    let bytes = serde_json::to_vec(&corpus_json()).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    eprintln!(
        "wrote {} cases, digest {}",
        corpus_json().cases.len(),
        digest(&bytes)
    );
}

#[test]
fn line_breaks_match_coretext_for_the_bundled_fonts() {
    let truth: Value =
        serde_json::from_str(include_str!("../../../fixtures/coretext-lines.json")).unwrap();
    let corpus = corpus();
    let bytes = serde_json::to_vec(&corpus_json()).unwrap();
    assert_eq!(
        truth["corpus"].as_str().unwrap(),
        digest(&bytes),
        "the corpus changed; regenerate fixtures/coretext-lines.json (see this module's docs)"
    );
    let truth = truth["cases"].as_array().unwrap();
    assert_eq!(truth.len(), corpus.len());
    let mut measurer = ShapingMeasurer::new();
    let (mut lines, mut same, mut counts) = (0usize, 0usize, 0usize);
    let (mut fallback, mut fallback_counts) = (0usize, 0usize);
    let face = FontRef::from_index(FACES[0], 0).unwrap();
    let mut worst_width = 0.0f32;
    let mut report = vec![];
    for (index, ((text, runs, width), expected)) in corpus.iter().zip(truth).enumerate() {
        // A paragraph with characters Paper Mono lacks is drawn partly in
        // the platform's fallback face, whose proportional widths the
        // shaper only estimates, so its line count is reported, not gated.
        let covered = text
            .chars()
            .all(|c| c.is_control() || face.charmap().map(c) != 0);
        if !covered {
            fallback += 1;
        }
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
        lines += ends.len();
        if got.len() != ends.len() && !covered {
            fallback_counts += 1;
        } else if got.len() != ends.len() {
            counts += 1;
        }
        let starts = |ends: &[u32]| {
            std::iter::once(0)
                .chain(ends.iter().copied())
                .take(ends.len())
                .collect::<std::collections::HashSet<u32>>()
        };
        let mine = starts(&got);
        let matched = starts(&ends).intersection(&mine).count();
        same += matched;
        if got == ends {
            for (line, width) in measured.lines.iter().zip(&widths) {
                let delta = (line.width - width).abs();
                worst_width = worst_width.max(delta);
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
                "case {index} width {width:?} ({} lines, Rust {}): CoreText line {:?} Rust line {:?}",
                ends.len(),
                got.len(),
                piece(start, ends.get(cut).copied().unwrap_or(0)),
                piece(start, got.get(cut).copied().unwrap_or(0)),
            ));
        }
    }
    let ratio = same as f64 / lines.max(1) as f64;
    eprintln!(
        "{} paragraphs, {lines} lines: {counts} line-count differences, {:.4}% exact line starts, worst width difference {worst_width:.3} pt",
        corpus.len(),
        ratio * 100.0
    );
    eprintln!(
        "{fallback} paragraphs use fallback faces; {fallback_counts} of them differ in line count"
    );
    for line in &report {
        eprintln!("{line}");
    }
    // Paper Mono's fixed 0.606 em advance puts more line ends right at the
    // edge than the proportional faces before it did, which exposes where
    // CoreText's UAX #14 tailoring differs from ours: one paragraph breaks
    // after a space before a code run that starts with a period. Paragraphs
    // that need fallback faces are reported above, not gated.
    assert!(counts <= 1, "{counts} paragraphs with a different line count");
    assert!(ratio >= 0.997, "exact line starts {ratio}");
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
    assert_eq!(weights.into_iter().collect::<Vec<_>>(), [400, 500, 600, 700]);
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
