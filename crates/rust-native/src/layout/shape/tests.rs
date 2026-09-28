//! The shaper against CoreText.
//!
//! `corpus()` lays out a sample of real prose, code, tables, and mixed
//! scripts (`fixtures/shaping-corpus.md`) as transcript rows at several
//! widths and text sizes, and records every paragraph the layout measures.
//! `fixtures/coretext-lines.json` holds CoreText's line breaks for the same
//! paragraphs with the same bundled fonts, made by
//! `tools/coretext-lines.swift`. The gate: no paragraph breaks into a
//! different number of lines, and at least 99.9% of lines start where
//! CoreText's do.
//!
//! Regenerate the ground truth after changing the corpus, the fonts, or the
//! layout's paragraphs:
//!
//! ```text
//! RUST_NATIVE_WRITE_CORPUS=/tmp/corpus.json cargo test -p rust-native \
//!     --features shaping --lib shape::tests::write_corpus -- --ignored
//! swift crates/rust-native/tools/coretext-lines.swift /tmp/corpus.json \
//!     crates/rust-native/fonts crates/rust-native/fixtures/coretext-lines.json
//! ```

use super::super::testing::FixedMeasurer;
use super::super::{Element, MeasureRun, Measured, Measurer, Node, TranscriptLayout, Update};
use super::*;
use crate::markdown;
use crate::style::Style;
use crate::view::{MessageRole, TextRole};
use serde_json::{Value, json};

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

fn corpus_json() -> Value {
    let cases: Vec<Value> = corpus()
        .into_iter()
        .map(|(text, runs, width)| {
            let runs: Vec<Value> = runs
                .iter()
                .map(|r| {
                    let spec = FontSpec::of(r.font);
                    json!([
                        spec.face,
                        spec.size,
                        spec.weight,
                        spec.optical,
                        spec.calt,
                        r.start16,
                        r.end16
                    ])
                })
                .collect();
            json!({"text": text, "runs": runs, "width": width.unwrap_or(0.0)})
        })
        .collect();
    json!({ "cases": cases })
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
        corpus_json()["cases"].as_array().unwrap().len(),
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
    let mut worst_width = 0.0f32;
    let mut report = vec![];
    for (index, ((text, runs, width), expected)) in corpus.iter().zip(truth).enumerate() {
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
        if got.len() != ends.len() {
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
                "case {index} width {width:?}: CoreText line {:?} Rust line {:?}",
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
    for line in &report {
        eprintln!("{line}");
    }
    assert_eq!(counts, 0, "paragraphs with a different line count");
    assert!(ratio >= 0.999, "exact line starts {ratio}");
}

#[test]
fn a_paragraph_breaks_at_spaces_and_keeps_trailing_space_on_the_line() {
    let font = Font {
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
    for face in 0..4 {
        let mut len = 0;
        let data = unsafe { rust_native_font_data(face, &mut len) };
        assert!(!data.is_null() && len > 100_000);
    }
    assert!(unsafe { rust_native_font_data(4, std::ptr::null_mut()) }.is_null());
    let code = rust_native_font_spec(14.4, 3, 1, 1);
    assert_eq!(
        (code.face, code.weight, code.optical, code.calt),
        (3, 700.0, 0.0, 0)
    );
    let text = rust_native_font_spec(22.0, 2, 0, 0);
    assert_eq!(
        (text.face, text.weight, text.optical, text.calt),
        (0, 600.0, 22.0, 1)
    );
    let handle = rust_native_layout_create_shaped();
    let rows = super::super::tests::conversation(6);
    let request = serde_json::to_vec(&json!({
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
