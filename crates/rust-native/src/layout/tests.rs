use super::display::WidgetKind;
use super::testing::FixedMeasurer;
use super::*;
use crate::markdown;
use crate::style::Style;
use crate::view::{MessageRole, TextRole, ToolState};

fn node(key: &str, element: Element<()>) -> Node<()> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

fn message(key: &str, role: MessageRole, text: &str) -> Node<()> {
    node(
        key,
        Element::Message {
            role,
            note: None,
            children: vec![node(
                &format!("{key}-md"),
                Element::Markdown {
                    blocks: markdown::parse(text),
                },
            )],
        },
    )
}

fn tool(key: &str) -> Node<()> {
    node(
        key,
        Element::Tool {
            name: "Bash".into(),
            detail: "cargo test -p ci".into(),
            state: ToolState::Done,
            children: vec![node(
                &format!("{key}-out"),
                Element::Text {
                    value: "test result: ok. 12 passed\nline two".into(),
                    role: TextRole::Code,
                },
            )],
        },
    )
}

fn update(rows: Vec<Node<()>>, width: f32) -> Update {
    Update {
        width,
        scale: 1.0,
        order: Some(rows.iter().map(|r| r.key.clone()).collect()),
        rows,
        expanded: vec![],
        earlier: None,
    }
}

const REPLY: &str = "Fixed it. The test seeded its random number generator from the clock, so \
     the order changed between runs.\n\n- Seeded the RNG\n- Added a retry\n\n\
     ```rust\nlet seed = 7;\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |";

fn conversation(count: usize) -> Vec<Node<()>> {
    (0..count)
        .map(|i| match i % 4 {
            0 => message(
                &format!("u{i}"),
                MessageRole::User,
                &format!("Fix the **flaky** test {i} in `ci.rs`."),
            ),
            1 => tool(&format!("t{i}")),
            2 => message(
                &format!("a{i}"),
                MessageRole::Assistant,
                &format!("{REPLY}\n\nRow {i}."),
            ),
            _ => node(
                &format!("w{i}"),
                Element::Working {
                    label: "Coder is working".into(),
                },
            ),
        })
        .collect()
}

#[test]
fn heights_offsets_and_ranges_are_exact() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let summary = layout
        .update(update(conversation(8), 390.0), &mut measurer)
        .unwrap();
    assert_eq!(summary.count, 8);
    assert_eq!(summary.relaid, 8);
    let mut y = EDGE_INSET;
    for index in 0..layout.len() {
        let placement = layout.placement(index).unwrap();
        assert_eq!(placement.y, y);
        assert!(placement.height >= 1.0 && placement.height.fract() == 0.0);
        let display = layout.display(index, &mut measurer).unwrap();
        assert_eq!(display.height, placement.height);
        assert_eq!(display.version, placement.version);
        // Everything painted fits inside the row.
        for run in &display.runs {
            assert!(
                run.baseline > 0.0 && run.baseline <= placement.height,
                "{run:?}"
            );
            assert!(
                run.x >= 16.0 && run.x + run.width <= 390.0 - 16.0 + 0.01,
                "{run:?}"
            );
        }
        for rect in &display.rects {
            assert!(
                rect.y >= 0.0 && rect.y + rect.h <= placement.height + 0.01,
                "{rect:?}"
            );
        }
        y += placement.height + ROW_GAP;
    }
    assert_eq!(layout.height(), y - ROW_GAP + EDGE_INSET);

    // Binary search: every row that intersects a band, and no other.
    for (y0, y1) in [
        (0.0, 1.0),
        (40.0, 300.0),
        (300.0, 301.0),
        (0.0, 1.0e6),
        (1.0e6, 2.0e6),
    ] {
        let expected: Vec<usize> = (0..layout.len())
            .filter(|&i| {
                let p = layout.placement(i).unwrap();
                p.y < y1 && p.y + p.height >= y0
            })
            .collect();
        let found: Vec<usize> = layout.rows_in(y0, y1).collect();
        assert_eq!(found, expected, "{y0}..{y1}");
    }
}

#[test]
fn an_unchanged_row_is_not_measured_again() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(40);
    layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let before: Vec<Placement> = (0..40).map(|i| layout.placement(i).unwrap()).collect();

    // Resending every row with the same content lays out nothing.
    let summary = layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    assert_eq!(summary.relaid, 0);
    assert_eq!(summary.measured, 0);

    // A streamed token changes one row: only it is laid out, and only it
    // changes version; rows after it move by its growth.
    let mut next = rows.clone();
    next[2] = message(
        "a2",
        MessageRole::Assistant,
        &format!("{REPLY}\n\nRow 2.\n\nAnd one more sentence that wraps onto another line."),
    );
    let order_only = Update {
        rows: vec![next[2].clone()],
        ..update(next.clone(), 390.0)
    };
    let summary = layout.update(order_only, &mut measurer).unwrap();
    assert_eq!(summary.relaid, 1);
    let after: Vec<Placement> = (0..40).map(|i| layout.placement(i).unwrap()).collect();
    let growth = after[2].height - before[2].height;
    assert!(growth > 0.0);
    for i in 0..40 {
        assert_eq!(after[i].version == before[i].version, i != 2, "row {i}");
        let shift = if i > 2 { growth } else { 0.0 };
        assert_eq!(after[i].y, before[i].y + shift, "row {i}");
    }
}

#[test]
fn prepending_rows_keeps_existing_rows_and_moves_them_down() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(6);
    layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let anchor = layout.placement(layout.find("a2").unwrap()).unwrap();
    let mut earlier: Vec<Node<()>> = (0..3)
        .map(|i| message(&format!("e{i}"), MessageRole::User, "Earlier message"))
        .collect();
    let added: f32 = {
        let mut probe = TranscriptLayout::new();
        probe
            .update(update(earlier.clone(), 390.0), &mut measurer)
            .unwrap();
        (0..3)
            .map(|i| probe.placement(i).unwrap().height + ROW_GAP)
            .sum()
    };
    earlier.extend(rows);
    let mut next = update(earlier, 390.0);
    next.rows.retain(|r| r.key.starts_with('e'));
    next.earlier = Some(EarlierRow {
        label: "Load earlier messages".into(),
        loading: false,
    });
    let summary = layout.update(next, &mut measurer).unwrap();
    // Three new rows plus the earlier control.
    assert_eq!(summary.relaid, 4);
    assert_eq!(layout.find(EARLIER_KEY), Some(0));
    let earlier_height = layout.placement(0).unwrap().height;
    let moved = layout.placement(layout.find("a2").unwrap()).unwrap();
    assert_eq!(moved.version, anchor.version);
    assert_eq!(moved.y, anchor.y + added + earlier_height + ROW_GAP);
    let display = layout.display(0, &mut measurer).unwrap();
    assert!(matches!(
        display.widgets.last().unwrap().kind,
        WidgetKind::Earlier { loading: false }
    ));
    // An unchanged earlier control is not laid out again.
    let mut again = Update {
        order: None,
        ..update(vec![], 390.0)
    };
    again.earlier = Some(EarlierRow {
        label: "Load earlier messages".into(),
        loading: false,
    });
    assert_eq!(layout.update(again, &mut measurer).unwrap().relaid, 0);
}

#[test]
fn expanding_a_tool_lays_out_only_that_row() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(8);
    layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let collapsed = layout.placement(1).unwrap();
    let display = layout.display(1, &mut measurer).unwrap();
    assert!(display.widgets.iter().any(|w| matches!(
        &w.kind,
        WidgetKind::Toggle { key, expanded: false } if key == "t1"
    )));
    assert_eq!(display.accessibility.label, "Bash, done");
    let mut next = update(rows, 390.0);
    next.rows.clear();
    // Expansion alone takes the in-place path: no order, no rows.
    next.order = None;
    next.expanded = vec!["t1".into()];
    let summary = layout.update(next, &mut measurer).unwrap();
    assert_eq!(summary.relaid, 1);
    let open = layout.placement(1).unwrap();
    assert!(open.height > collapsed.height);
    assert_ne!(open.version, collapsed.version);
    let display = layout.display(1, &mut measurer).unwrap();
    assert!(display.texts.iter().any(|t| t.contains("12 passed")));
}

#[test]
fn width_changes_relay_every_row_and_measurements_are_cached() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(12);
    layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let calls = measurer.calls;
    let narrow = layout.height();
    let summary = layout
        .update(update(rows.clone(), 1024.0), &mut measurer)
        .unwrap();
    assert_eq!(summary.relaid, 12);
    assert!(layout.height() < narrow);
    // The wide reading width caps content at 720 points, centered.
    let display = layout.display(2, &mut measurer).unwrap();
    assert!(
        display
            .runs
            .iter()
            .all(|r| r.x >= 152.0 && r.x + r.width <= 872.01)
    );
    // Back to the first width: every measurement is a cache hit, and display
    // lists never measure what layout measured.
    let before = measurer.calls;
    assert!(before > calls);
    layout.update(update(rows, 390.0), &mut measurer).unwrap();
    for index in 0..layout.len() {
        layout.display(index, &mut measurer).unwrap();
    }
    assert_eq!(measurer.calls, before);
}

#[test]
fn user_bubbles_shrink_to_their_text_and_align_trailing() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    layout
        .update(
            update(vec![message("u", MessageRole::User, "Hi")], 390.0),
            &mut measurer,
        )
        .unwrap();
    let display = layout.display(0, &mut measurer).unwrap();
    let bubble = &display.rects[0];
    assert_eq!(
        bubble.fill,
        Some(display::Ink::Role(display::ColorRole::Bubble))
    );
    assert!((bubble.x + bubble.w - (390.0 - 16.0)).abs() < 0.01);
    assert!(bubble.w < 100.0);
    let run = &display.runs[0];
    assert!(run.x >= bubble.x + 14.0 - 0.01 && run.x + run.width <= bubble.x + bubble.w - 13.0);
    assert_eq!(display.copy.as_deref(), Some("Hi"));
}

#[test]
fn runs_carry_utf8_and_utf16_ranges_styles_and_inert_links() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let row = message(
        "a",
        MessageRole::Assistant,
        "Emoji 🙂 then **bold** and [a link](https://example.com) and `code`.",
    );
    layout
        .update(update(vec![row], 390.0), &mut measurer)
        .unwrap();
    let display = layout.display(0, &mut measurer).unwrap();
    for run in &display.runs {
        let text = &display.texts[run.text as usize];
        let by8 = &text[run.start8 as usize..(run.start8 + run.len8) as usize];
        let units: Vec<u16> = text.encode_utf16().collect();
        let by16 =
            String::from_utf16(&units[run.start16 as usize..(run.start16 + run.len16) as usize])
                .unwrap();
        assert_eq!(by8, by16);
    }
    let bold = display.runs.iter().find(|r| {
        let text = &display.texts[r.text as usize];
        &text[r.start8 as usize..(r.start8 + r.len8) as usize] == "bold"
    });
    let bold = bold.expect("a bold run");
    assert_eq!(
        display.styles[bold.style as usize].font.weight,
        display::Weight::Bold
    );
    assert_eq!(display.links.len(), 1);
    assert_eq!(display.links[0].destination, "https://example.com");
    // The inline code has a background rectangle.
    assert!(
        display
            .rects
            .iter()
            .any(|r| r.fill == Some(display::Ink::Role(display::ColorRole::InlineCode)))
    );
}

#[test]
fn text_scale_grows_rows() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(4);
    let mut first = update(rows.clone(), 390.0);
    layout.update(first.clone(), &mut measurer).unwrap();
    let normal = layout.height();
    first.scale = 1.5;
    layout.update(first, &mut measurer).unwrap();
    assert!(layout.height() > normal * 1.3);
}

#[test]
fn refuses_bad_geometry_unknown_and_duplicate_rows() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    assert_eq!(
        layout.update(update(vec![], f32::NAN), &mut measurer),
        Err(LayoutError::Geometry)
    );
    let mut unknown = update(vec![], 390.0);
    unknown.order = Some(vec!["missing".into()]);
    assert_eq!(
        layout.update(unknown, &mut measurer),
        Err(LayoutError::UnknownRow("missing".into()))
    );
    let mut twice = update(conversation(1), 390.0);
    let order = twice.order.as_mut().unwrap();
    order.push(order[0].clone());
    assert!(matches!(
        layout.update(twice, &mut measurer),
        Err(LayoutError::DuplicateRow(_))
    ));
    let bad = node("bad key", Element::Working { label: "x".into() });
    assert!(matches!(
        layout.update(update(vec![bad], 390.0), &mut measurer),
        Err(LayoutError::Row(_))
    ));
}

#[test]
fn a_refused_measurement_falls_back_to_an_estimate() {
    struct Refuses;
    impl Measurer for Refuses {
        fn measure(&mut self, _: &str, _: &[MeasureRun], _: Option<f32>) -> Option<Measured> {
            Some(Measured {
                lines: vec![Line {
                    start16: 0,
                    end16: 1,
                    ..Line::default()
                }],
                offsets: vec![],
            })
        }
    }
    let mut layout = TranscriptLayout::new();
    layout
        .update(update(conversation(3), 390.0), &mut Refuses)
        .unwrap();
    assert!(layout.placement(2).unwrap().height > 40.0);
}

#[test]
fn builds_from_a_transcript_node_and_the_fixture() {
    let bytes = include_bytes!("../../fixtures/conversation.json");
    let view = View::<serde_json::Value>::from_json(bytes).unwrap();
    let Element::Stack { children, .. } = &view.view().root.element else {
        panic!("fixture root is a stack");
    };
    let update = Update::from_transcript(&children[0], 390.0, 1.0).unwrap();
    assert!(update.earlier.is_some());
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let summary = layout.update(update, &mut measurer).unwrap();
    assert_eq!(summary.count, 6);
    for index in 0..layout.len() {
        let display = layout.display(index, &mut measurer).unwrap();
        assert!(!display.accessibility.label.is_empty(), "{}", display.key);
    }
}

/// Layout of 3,000 rows and the cost of one streamed token. Run with
/// `cargo test --release -p rust-native --lib layout::tests::bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn bench() {
    let rows = conversation(3_000);
    let mut measurer = FixedMeasurer::default();
    let mut layout = TranscriptLayout::new();
    let started = Instant::now();
    let summary = layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let cold = started.elapsed();
    let mut text = String::from("Streaming reply.");
    let mut worst = std::time::Duration::ZERO;
    let mut total = std::time::Duration::ZERO;
    for token in 0..200 {
        text.push_str(&format!(" token{token}"));
        let next = Update {
            rows: vec![message("a2998", MessageRole::Assistant, &text)],
            order: None,
            ..update(vec![], 390.0)
        };
        let started = Instant::now();
        let summary = layout.update(next, &mut measurer).unwrap();
        let spent = started.elapsed();
        assert_eq!(summary.relaid, 1);
        worst = worst.max(spent);
        total += spent;
    }
    let started = Instant::now();
    let visible = layout.rows_in(layout.height() - 900.0, layout.height());
    let count = visible.len();
    for index in visible {
        layout.display(index, &mut measurer).unwrap();
    }
    let display = started.elapsed();
    let started = Instant::now();
    layout.update(update(rows, 430.0), &mut measurer).unwrap();
    let rewidth = started.elapsed();
    println!(
        "cold layout of {} rows: {:?} ({} measurements); streamed token: mean {:?}, worst {:?}; \
         {count} visible displays: {:?}; width change: {:?}; height {}",
        summary.count,
        cold,
        summary.measured,
        total / 200,
        worst,
        display,
        rewidth,
        layout.height()
    );
}

#[cfg(feature = "ffi")]
mod ffi {
    use super::super::ffi::*;
    use super::*;
    use std::ffi::c_void;

    unsafe extern "C" fn fixed(
        context: *mut c_void,
        text: *const u8,
        text_len: usize,
        runs: *const RustNativeTextRun,
        run_count: usize,
        width: f32,
        lines: *mut RustNativeTextLine,
        line_capacity: usize,
        line_count: *mut usize,
        offsets: *mut f32,
        offset_capacity: usize,
        offset_count: *mut usize,
    ) -> i32 {
        let calls = unsafe { &mut *(context as *mut usize) };
        *calls += 1;
        let text =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(text, text_len) }).unwrap();
        let runs: Vec<MeasureRun> = unsafe { std::slice::from_raw_parts(runs, run_count) }
            .iter()
            .map(|r| MeasureRun {
                font: display::Font {
                    size: r.size,
                    weight: display::Weight::Regular,
                    italic: r.italic != 0,
                    mono: r.monospace != 0,
                },
                start16: r.start16,
                end16: r.end16,
            })
            .collect();
        let measured = FixedMeasurer::default()
            .measure(text, &runs, (width > 0.0).then_some(width))
            .unwrap();
        unsafe {
            *line_count = measured.lines.len();
            *offset_count = measured.offsets.len();
        }
        if measured.lines.len() > line_capacity || measured.offsets.len() > offset_capacity {
            return 1;
        }
        for (i, l) in measured.lines.iter().enumerate() {
            unsafe {
                lines.add(i).write(RustNativeTextLine {
                    start16: l.start16,
                    end16: l.end16,
                    width: l.width,
                    ascent: l.ascent,
                    descent: l.descent,
                    leading: l.leading,
                })
            };
        }
        for (i, x) in measured.offsets.iter().enumerate() {
            unsafe { offsets.add(i).write(*x) };
        }
        0
    }

    fn json(buffer: RustNativeBuffer) -> serde_json::Value {
        let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) }.to_vec();
        unsafe { rust_native_layout_buffer_free(buffer) };
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn the_c_interface_lays_out_queries_and_paints() {
        let mut calls = 0usize;
        let handle = unsafe {
            rust_native_layout_create(&mut calls as *mut usize as *mut c_void, Some(fixed))
        };
        assert!(!handle.is_null());
        // A long paragraph forces the capacity retry.
        let long = message("long", MessageRole::Assistant, &"word ".repeat(400));
        let mut rows = conversation(30);
        rows.push(long);
        let request = serde_json::to_vec(&serde_json::json!({
            "width": 390.0,
            "scale": 1.0,
            "order": rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
            "rows": rows,
            "expanded": ["t1"],
            "earlier": {"label": "Load earlier messages", "loading": true},
        }))
        .unwrap();
        let summary =
            json(unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) });
        assert_eq!(summary["count"], 32);
        let height = unsafe { rust_native_layout_height(handle) };
        assert_eq!(summary["height"].as_f64().unwrap() as f32, height);

        let mut out = [RustNativeRowPlacement::default(); 64];
        let count =
            unsafe { rust_native_layout_rows(handle, 0.0, height, out.as_mut_ptr(), out.len()) };
        assert_eq!(count, 32);
        let few = unsafe { rust_native_layout_rows(handle, 0.0, height, out.as_mut_ptr(), 2) };
        assert_eq!(few, 32);

        let key = b"long";
        let mut found = RustNativeRowPlacement::default();
        assert_eq!(
            unsafe { rust_native_layout_find(handle, key.as_ptr(), key.len(), &mut found) },
            1
        );
        assert_eq!(found.index, 31);
        assert!(found.height > 300.0);
        assert_eq!(
            unsafe { rust_native_layout_find(handle, b"nope".as_ptr(), 4, &mut found) },
            0
        );

        let display = json(unsafe { rust_native_layout_display(handle, 0) });
        assert_eq!(display["widgets"][0]["kind"], "spinner");
        let display = json(unsafe { rust_native_layout_display(handle, 2) });
        assert_eq!(display["key"], "t1");
        assert_eq!(display["widgets"].as_array().unwrap().len(), 3);
        assert!(
            unsafe { rust_native_layout_display(handle, 99) }
                .data
                .is_null()
        );

        let refused = json(unsafe { rust_native_layout_update(handle, b"{}".as_ptr(), 2) });
        assert!(refused["error"].is_string());
        let empty = unsafe { rust_native_layout_update(handle, ptr_or_null(), 0) };
        assert!(empty.data.is_null());
        unsafe { rust_native_layout_destroy(handle) };
        assert!(calls > 0);
    }

    fn ptr_or_null() -> *const u8 {
        std::ptr::null()
    }

    #[test]
    fn a_missing_measurer_is_refused() {
        assert!(unsafe { rust_native_layout_create(std::ptr::null_mut(), None) }.is_null());
    }
}
