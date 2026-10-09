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

pub(super) fn message(key: &str, role: MessageRole, text: &str) -> Node<()> {
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
        curve: vec![],
        source: None,
    }
}

const REPLY: &str = "Fixed it. The test seeded its random number generator from the clock, so \
     the order changed between runs.\n\n- Seeded the RNG\n- Added a retry\n\n\
     ```rust\nlet seed = 7;\n```\n\n| a | b |\n|---|--:|\n| 1 | 2 |";

pub(super) fn conversation(count: usize) -> Vec<Node<()>> {
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

/// A working row, as under a reply that is still coming, draws a spinner
/// before its label.
#[test]
fn a_working_row_draws_a_spinner() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    layout
        .update(update(conversation(4), 390.0), &mut measurer)
        .unwrap();
    let display = layout.display(3).unwrap().clone();
    assert_eq!(display.key, "w3");
    assert_eq!(display.widgets.len(), 1);
    let spinner = &display.widgets[0];
    assert!(matches!(spinner.kind, WidgetKind::Spinner));
    let label = display.runs.first().expect("the label");
    assert!(spinner.x + spinner.w < label.x, "{spinner:?} {label:?}");
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
        let display = layout.display(index).unwrap().clone();
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
    let display = layout.display(0).unwrap().clone();
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
    let display = layout.display(1).unwrap().clone();
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
    let display = layout.display(1).unwrap().clone();
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
    let display = layout.display(2).unwrap().clone();
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
        layout.display(index).unwrap();
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
    let display = layout.display(0).unwrap().clone();
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
    let display = layout.display(0).unwrap().clone();
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
        let display = layout.display(index).unwrap().clone();
        assert!(!display.accessibility.label.is_empty(), "{}", display.key);
    }
}

#[test]
fn a_frame_is_a_snapshot_readable_on_another_thread() {
    fn sendable<T: Send>() {}
    fn shared<T: Send + Sync>() {}
    sendable::<TranscriptLayout>();
    shared::<Frame>();
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(8);
    layout
        .update(update(rows.clone(), 390.0), &mut measurer)
        .unwrap();
    let before = layout.frame();
    assert!(
        Arc::ptr_eq(&before, &layout.frame()),
        "an unchanged layout reuses its frame"
    );
    let height = before.height();
    let placement = before.placement(2).unwrap();
    assert_eq!(placement, layout.placement(2).unwrap());
    assert_eq!(before.key(2), Some("a2"));
    assert_eq!(before.find("a2"), Some(2));
    assert_eq!(before.rows_in(0.0, height), layout.rows_in(0.0, height));

    // Lay out on a worker while this thread keeps reading the old frame.
    let mut streamed = rows[2].clone();
    let Element::Message { children, .. } = &mut streamed.element else {
        unreachable!()
    };
    children[0].element = Element::Markdown {
        blocks: markdown::parse(&format!("{REPLY}\n\n{}", "More streamed text. ".repeat(20))),
    };
    let worker = std::thread::spawn(move || {
        let mut measurer = FixedMeasurer::default();
        let mut token = update(vec![streamed], 390.0);
        token.order = None;
        layout.update(token, &mut measurer).unwrap();
        (layout.frame(), layout)
    });
    let reader = {
        let frame = before.clone();
        std::thread::spawn(move || frame.display(2).map(|d| d.version))
    };
    let (after, _layout) = worker.join().unwrap();
    assert_eq!(reader.join().unwrap(), Some(placement.version));
    assert_eq!(before.height(), height, "a published frame never changes");
    assert!(after.height() > height);
    assert_ne!(after.placement(2).unwrap().version, placement.version);
    assert_eq!(after.placement(1), before.placement(1));
    assert_eq!(
        after.display(2).unwrap().height,
        after.placement(2).unwrap().height
    );
}

fn code_row(key: &str, code: &str) -> Node<()> {
    message(
        key,
        MessageRole::Assistant,
        &format!("```rust\n{code}\n```"),
    )
}

#[test]
fn wide_code_blocks_scroll_sideways_and_keep_their_lines() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let long = format!("let value = {};\nshort();", "x + ".repeat(60));
    layout
        .update(
            update(
                vec![code_row("wide", &long), code_row("narrow", "ok();")],
                390.0,
            ),
            &mut measurer,
        )
        .unwrap();
    let wide = layout.display(0).unwrap();
    assert_eq!(wide.scrollers.len(), 1);
    let scroller = &wide.scrollers[0];
    let (x, w) = content_band(390.0);
    assert!(scroller.x >= x && scroller.x + scroller.w <= x + w + 0.01);
    assert!(scroller.content_w > scroller.w + 100.0);
    // One run per source line: code is not wrapped.
    let runs = &wide.runs[scroller.runs[0] as usize..scroller.runs[1] as usize];
    assert_eq!(runs.len(), 2);
    assert!(runs[0].width > w);
    for run in runs {
        assert!(run.baseline > scroller.y && run.baseline < scroller.y + scroller.h);
    }
    // The header and its copy control stay outside the scroller.
    assert!(
        wide.runs[..scroller.runs[0] as usize]
            .iter()
            .any(|r| r.baseline < scroller.y)
    );
    assert!(
        wide.widgets
            .iter()
            .any(|w| matches!(w.kind, WidgetKind::Copy { .. }))
    );
    assert!(layout.display(1).unwrap().scrollers.is_empty());
}

#[test]
fn wide_tables_scroll_and_narrow_ones_fit() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let header = (0..8).map(|i| format!("Column {i}")).collect::<Vec<_>>();
    let wide = format!(
        "> {}\n> |{}|\n> |{}|",
        "Quoted.",
        header.join("|"),
        header.iter().map(|_| "---").collect::<Vec<_>>().join("|")
    );
    layout
        .update(
            update(
                vec![
                    message(
                        "wide",
                        MessageRole::Assistant,
                        &format!("{wide}\n> |{}|", header.join("|")),
                    ),
                    message(
                        "narrow",
                        MessageRole::Assistant,
                        "| a | b |\n|---|---|\n| 1 | 2 |",
                    ),
                ],
                390.0,
            ),
            &mut measurer,
        )
        .unwrap();
    let display = layout.display(0).unwrap();
    assert_eq!(display.scrollers.len(), 1);
    let scroller = &display.scrollers[0];
    assert!(scroller.content_w > scroller.w);
    // The quote bar was inserted before the table; the range still names the
    // table's own rectangles, from its surface to its border.
    let rects = &display.rects[scroller.rects[0] as usize..scroller.rects[1] as usize];
    assert_eq!(
        rects.first().unwrap().fill,
        Some(display::Ink::Role(display::ColorRole::Surface))
    );
    assert_eq!(
        rects.last().unwrap().stroke,
        Some(display::Ink::Role(display::ColorRole::Border))
    );
    assert!(
        display.rects[..scroller.rects[0] as usize]
            .iter()
            .any(|r| r.w == 3.0 && r.fill == Some(display::Ink::Role(display::ColorRole::Border)))
    );
    let runs = &display.runs[scroller.runs[0] as usize..scroller.runs[1] as usize];
    assert_eq!(runs.len(), 16, "every cell of the header and the row");
    assert!(layout.display(1).unwrap().scrollers.is_empty());
}

#[test]
fn a_text_size_curve_scales_each_size_and_relays_rows() {
    let curve = Typography::new(1.0, &[[17.0, 23.0], [12.0, 15.0], [13.0, 16.0]]).unwrap();
    assert!((curve.size(12.0) - 15.0).abs() < 1e-4);
    assert!((curve.size(15.0) - (16.0 + 0.5 * 7.0)).abs() < 1e-4);
    assert!((curve.size(22.0) - 22.0 * 23.0 / 17.0).abs() < 1e-3);
    assert!((curve.size(6.0) - 6.0 * 15.0 / 12.0).abs() < 1e-4);
    assert!((Typography::new(1.5, &[]).unwrap().size(10.0) - 15.0).abs() < 1e-4);
    for bad in [
        vec![[f32::NAN, 12.0]],
        vec![[12.0, 60.0]],
        vec![[0.0, 1.0]],
        vec![[12.0, 12.0]; MAX_CURVE_POINTS + 1],
    ] {
        assert_eq!(Typography::new(1.0, &bad), Err(LayoutError::Geometry));
    }

    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let rows = conversation(8);
    let mut plain = update(rows.clone(), 390.0);
    layout.update(plain.clone(), &mut measurer).unwrap();
    let height = layout.height();
    let body = layout.display(2).unwrap().styles[0].font.size;
    // Only small text grows: body rows keep their size.
    plain.order = None;
    plain.rows = vec![];
    plain.curve = vec![[12.0, 20.0], [13.0, 21.0], [16.0, 16.0]];
    let summary = layout.update(plain.clone(), &mut measurer).unwrap();
    assert_eq!(summary.relaid, 8);
    assert!(layout.height() > height);
    assert_eq!(layout.display(2).unwrap().styles[0].font.size, body);
    plain.curve.push([22.0, 900.0]);
    assert_eq!(
        layout.update(plain, &mut measurer),
        Err(LayoutError::Geometry)
    );
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
        // Each token publishes a frame, as the adapter asks for one.
        layout.frame();
        let spent = started.elapsed();
        assert_eq!(summary.relaid, 1);
        worst = worst.max(spent);
        total += spent;
    }
    let started = Instant::now();
    let visible = layout.rows_in(layout.height() - 900.0, layout.height());
    let count = visible.len();
    for index in visible {
        serde_json::to_vec(layout.display(index).unwrap()).unwrap();
    }
    let display = started.elapsed();
    let started = Instant::now();
    layout.update(update(rows, 430.0), &mut measurer).unwrap();
    let rewidth = started.elapsed();
    println!(
        "cold layout of {} rows: {:?} ({} measurements); streamed token with its frame: \
         mean {:?}, worst {:?}; {count} visible displays as JSON: {:?}; width change: {:?}; \
         height {}",
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
                    family: Default::default(),
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

    #[test]
    fn the_c_interface_publishes_a_source_and_lays_it_out() {
        let mut calls = 0usize;
        let handle = unsafe {
            rust_native_layout_create(&mut calls as *mut usize as *mut c_void, Some(fixed))
        };
        let node = serde_json::to_vec(&serde_json::json!({
            "key": "chat",
            "style": {},
            "element": {"kind": "transcript", "props": {
                "label": "Messages",
                "children": conversation(8),
                "earlier": {"label": "Load earlier messages", "loading": false, "intent": null},
            }},
        }))
        .unwrap();
        let name = b"ffi-test:chat";
        assert_eq!(
            unsafe {
                rust_native_source_publish(name.as_ptr(), name.len(), node.as_ptr(), node.len())
            },
            1
        );
        assert_eq!(
            unsafe { rust_native_source_publish(name.as_ptr(), name.len(), b"{}".as_ptr(), 2) },
            0
        );
        let request = br#"{"width": 390, "scale": 1, "source": "ffi-test:chat"}"#;
        let summary =
            json(unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) });
        assert_eq!(summary["count"], 9);
        unsafe { rust_native_source_retire(name.as_ptr(), name.len()) };
        let refused =
            json(unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) });
        assert!(refused["error"].is_string());
        unsafe { rust_native_layout_destroy(handle) };
    }

    #[test]
    fn frames_outlive_updates_and_answer_on_their_own() {
        let mut calls = 0usize;
        let handle = unsafe {
            rust_native_layout_create(&mut calls as *mut usize as *mut c_void, Some(fixed))
        };
        let rows = conversation(12);
        let request = serde_json::to_vec(&serde_json::json!({
            "width": 390.0,
            "scale": 1.0,
            "order": rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
            "rows": rows,
            "curve": [[12.0, 13.0], [17.0, 18.0]],
        }))
        .unwrap();
        json(unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) });
        let frame = unsafe { rust_native_layout_frame(handle) };
        assert!(!frame.is_null());
        assert_eq!(unsafe { rust_native_frame_count(frame) }, 12);
        let height = unsafe { rust_native_frame_height(frame) };
        assert_eq!(height, unsafe { rust_native_layout_height(handle) });

        // Drop every row but one; the old frame still answers as it was.
        let request = serde_json::to_vec(&serde_json::json!({
            "width": 390.0, "scale": 1.0, "order": ["u0"],
        }))
        .unwrap();
        let summary =
            json(unsafe { rust_native_layout_update(handle, request.as_ptr(), request.len()) });
        assert_eq!(summary["count"], 1);
        unsafe { rust_native_layout_destroy(handle) };

        let mut out = [RustNativeRowPlacement::default(); 32];
        let count = unsafe { rust_native_frame_rows(frame, 0.0, height, out.as_mut_ptr(), 32) };
        assert_eq!(count, 12);
        let key = unsafe { rust_native_frame_key(frame, 5) };
        let bytes = unsafe { std::slice::from_raw_parts(key.data, key.len) }.to_vec();
        unsafe { rust_native_layout_buffer_free(key) };
        assert_eq!(bytes, b"t5");
        let mut found = RustNativeRowPlacement::default();
        assert_eq!(
            unsafe { rust_native_frame_find(frame, b"t5".as_ptr(), 2, &mut found) },
            1
        );
        assert_eq!(found.index, 5);
        assert_eq!(found.version, out[5].version);
        let display = json(unsafe { rust_native_frame_display(frame, 5) });
        assert_eq!(display["key"], "t5");
        assert!(
            unsafe { rust_native_frame_display(frame, 12) }
                .data
                .is_null()
        );
        assert!(unsafe { rust_native_frame_key(frame, 12) }.data.is_null());
        unsafe { rust_native_frame_release(frame) };

        // A null frame answers nothing.
        let none = std::ptr::null();
        assert_eq!(unsafe { rust_native_frame_count(none) }, 0);
        assert_eq!(
            unsafe { rust_native_frame_rows(none, 0.0, 1.0, out.as_mut_ptr(), 1) },
            0
        );
        assert!(unsafe { rust_native_frame_display(none, 0) }.data.is_null());
        unsafe { rust_native_frame_release(none) };
    }

    fn ptr_or_null() -> *const u8 {
        std::ptr::null()
    }

    #[test]
    fn a_missing_measurer_is_refused() {
        assert!(unsafe { rust_native_layout_create(std::ptr::null_mut(), None) }.is_null());
    }
}

mod sources {
    use super::*;

    fn pulled(name: &str, width: f32) -> Update {
        Update {
            width,
            scale: 1.0,
            source: Some(name.into()),
            ..Update::default()
        }
    }

    fn transcript(children: Vec<Node<()>>, earlier: bool) -> Node<()> {
        node(
            "chat",
            Element::Transcript {
                label: "Messages".into(),
                children,
                earlier: earlier.then(|| crate::view::Earlier {
                    label: "Load earlier messages".into(),
                    loading: false,
                    intent: (),
                }),
                source: None,
            },
        )
    }

    fn laid_out(rows: Vec<Node<()>>, width: f32) -> TranscriptLayout {
        let mut layout = TranscriptLayout::new();
        layout
            .update(update(rows, width), &mut FixedMeasurer::default())
            .unwrap();
        layout
    }

    #[test]
    fn a_pulled_transcript_lays_out_like_one_sent_whole() {
        let rows = conversation(40);
        let mut root = transcript(rows.clone(), false);
        assert_eq!(source::detach(&mut root, "test-whole").unwrap(), 1);
        let Element::Transcript {
            children, source, ..
        } = &root.element
        else {
            unreachable!()
        };
        assert!(children.is_empty());
        assert_eq!(source.as_deref(), Some("test-whole:chat"));
        let mut layout = TranscriptLayout::new();
        let summary = layout
            .update(
                pulled("test-whole:chat", 390.0),
                &mut FixedMeasurer::default(),
            )
            .unwrap();
        let sent = laid_out(rows, 390.0);
        assert_eq!(summary.count, 40);
        assert_eq!(layout.height(), sent.height());
        for i in 0..40 {
            assert_eq!(layout.placement(i), sent.placement(i));
        }
        // The detached view is small whatever the transcript holds.
        View::new("test", 1, root).validate().unwrap();
    }

    #[test]
    fn a_streamed_token_relays_one_row_and_an_unchanged_publication_none() {
        let mut rows = conversation(30);
        source::publish("test-stream:chat", rows.clone(), None).unwrap();
        let mut layout = TranscriptLayout::new();
        let mut measurer = FixedMeasurer::default();
        layout
            .update(pulled("test-stream:chat", 390.0), &mut measurer)
            .unwrap();
        let again = layout
            .update(pulled("test-stream:chat", 390.0), &mut measurer)
            .unwrap();
        assert_eq!(again.relaid, 0);
        source::publish("test-stream:chat", rows.clone(), None).unwrap();
        let republished = layout
            .update(pulled("test-stream:chat", 390.0), &mut measurer)
            .unwrap();
        assert_eq!(republished.relaid, 0);
        rows[29] = message("u29", MessageRole::Assistant, "A longer streamed reply");
        source::publish("test-stream:chat", rows.clone(), None).unwrap();
        let streamed = layout
            .update(pulled("test-stream:chat", 390.0), &mut measurer)
            .unwrap();
        assert_eq!(streamed.relaid, 1);
        let sent = laid_out(rows, 390.0);
        assert_eq!(layout.height(), sent.height());
        source::retire("test-stream:chat");
    }

    #[test]
    fn prepended_rows_and_the_earlier_control_arrive_through_the_source() {
        let rows = conversation(20);
        let mut root = transcript(rows[10..].to_vec(), true);
        source::detach(&mut root, "test-prepend").unwrap();
        let mut layout = TranscriptLayout::new();
        let mut measurer = FixedMeasurer::default();
        layout
            .update(pulled("test-prepend:chat", 390.0), &mut measurer)
            .unwrap();
        assert_eq!(layout.find(EARLIER_KEY), Some(0));
        assert_eq!(layout.len(), 11);
        let mut root = transcript(rows.clone(), false);
        source::detach(&mut root, "test-prepend").unwrap();
        let summary = layout
            .update(pulled("test-prepend:chat", 390.0), &mut measurer)
            .unwrap();
        assert_eq!(summary.count, 20);
        assert_eq!(summary.relaid, 10);
        assert_eq!(layout.find(EARLIER_KEY), None);
        assert_eq!(layout.find(&rows[10].key), Some(10));
    }

    #[test]
    fn detach_retires_sources_its_scope_no_longer_shows() {
        let mut root = transcript(conversation(3), false);
        source::detach(&mut root, "test-retire").unwrap();
        assert!(source::get("test-retire:chat").is_some());
        let mut other = node(
            "empty",
            Element::Stack {
                axis: crate::view::Axis::Vertical,
                children: vec![],
            },
        );
        assert_eq!(source::detach(&mut other, "test-retire").unwrap(), 0);
        assert!(source::get("test-retire:chat").is_none());
        let mut layout = TranscriptLayout::new();
        assert_eq!(
            layout
                .update(
                    pulled("test-retire:chat", 390.0),
                    &mut FixedMeasurer::default()
                )
                .unwrap_err(),
            LayoutError::UnknownSource("test-retire:chat".into())
        );
    }

    #[test]
    fn a_source_refuses_malformed_updates_and_rows() {
        source::publish("test-refuse:chat", conversation(2), None).unwrap();
        let mut layout = TranscriptLayout::new();
        let mut mixed = pulled("test-refuse:chat", 390.0);
        mixed.order = Some(vec![]);
        assert_eq!(
            layout
                .update(mixed, &mut FixedMeasurer::default())
                .unwrap_err(),
            LayoutError::Source
        );
        assert_eq!(
            source::publish("bad name", vec![], None).unwrap_err(),
            LayoutError::Source
        );
        let twice = vec![
            message("a", MessageRole::User, "x"),
            message("a", MessageRole::User, "y"),
        ];
        assert_eq!(
            source::publish("test-refuse:chat", twice, None).unwrap_err(),
            LayoutError::DuplicateRow("a".into())
        );
        // A refused publication leaves the previous one.
        assert_eq!(source::get("test-refuse:chat").unwrap().len(), 2);
        // A view may not name a source and list rows too.
        let mut both = transcript(conversation(1), false);
        if let Element::Transcript { source, .. } = &mut both.element {
            *source = Some("test-refuse:chat".into());
        }
        assert!(View::new("test", 1, both).validate().is_err());
        source::retire("test-refuse:chat");
    }

    #[test]
    fn a_long_transcript_escapes_the_view_bounds_once_detached() {
        let rows = conversation(3_000);
        let whole = transcript(rows.clone(), false);
        assert!(View::new("test", 1, whole).validate().is_err());
        let mut root = transcript(rows, false);
        source::detach(&mut root, "test-long").unwrap();
        View::new("test", 1, root).validate().unwrap();
        let mut layout = TranscriptLayout::new();
        let summary = layout
            .update(
                pulled("test-long:chat", 390.0),
                &mut FixedMeasurer::default(),
            )
            .unwrap();
        assert_eq!(summary.count, 3_000);
        source::retire("test-long:chat");
    }
}

/// A text row's `style.align` places its lines: at the start by default,
/// centered, or at the end of the content band.
#[test]
fn a_text_row_follows_its_alignment() {
    let x = |align: Option<crate::style::TextAlign>| {
        let mut row = node(
            "t",
            Element::Text {
                value: "Short".into(),
                role: TextRole::Body,
            },
        );
        row.style.align = align;
        let mut layout = TranscriptLayout::new();
        layout
            .update(update(vec![row], 400.0), &mut FixedMeasurer::default())
            .unwrap();
        layout.display(0).unwrap().runs[0].x
    };
    let start = x(None);
    let center = x(Some(crate::style::TextAlign::Center));
    let end = x(Some(crate::style::TextAlign::End));
    assert_eq!(start, content_band(400.0).0);
    assert!(start < center && center < end, "{start} {center} {end}");
}

/// A table a little wider than the row, once its columns wrap at the
/// readable width, wraps a little more instead of scrolling sideways; a
/// much wider one still scrolls.
#[test]
fn a_table_just_too_wide_wraps_instead_of_scrolling() {
    let table = |cells: usize| {
        let long = "word ".repeat(30);
        let header = vec!["h"; cells].join("|");
        let rule = vec!["---"; cells].join("|");
        let row = vec![long.trim(); cells].join("|");
        node(
            "t",
            Element::Markdown {
                blocks: markdown::parse(&format!("|{header}|\n|{rule}|\n|{row}|")),
            },
        )
    };
    // Three columns capped at the readable width: 780 points of cells.
    let scrolls = |width: f32| {
        let mut layout = TranscriptLayout::new();
        layout
            .update(update(vec![table(3)], width), &mut FixedMeasurer::default())
            .unwrap();
        !layout.display(0).unwrap().scrollers.is_empty()
    };
    let side = 2.0 * rows::SIDE_MARGIN;
    assert!(!scrolls(720.0 + side), "780 of cells in 720 points wraps");
    assert!(scrolls(600.0 + side), "780 of cells in 600 points scrolls");
}

/// A pill chip, or a button that keeps its measured width, is as wide as
/// its label; any other button fills the row; a long chip wraps inside it.
#[test]
fn pill_and_intrinsic_buttons_hug_their_labels() {
    let button = |key: &str, label: &str, pill: bool, intrinsic: bool| Node {
        key: key.into(),
        style: Style {
            intrinsic_width: intrinsic.then_some(true),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: pill.then_some(crate::view::Icon {
                glyph: crate::view::Glyph::Ask,
                circular: false,
                pill: true,
            }),
            intent: (),
        },
    };
    let long = "A follow-up question long enough to need more than one line here";
    let rows = vec![
        button("full", "Start the test", false, false),
        button("pill", "What can you do?", true, false),
        button("hug", "Not now", false, true),
        button("long", long, true, false),
    ];
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    layout.update(update(rows, 390.0), &mut measurer).unwrap();
    let widget = |index: usize| layout.display(index).unwrap().widgets[0].clone();
    let full = widget(0);
    let pill = widget(1);
    let hug = widget(2);
    let long = widget(3);
    assert!(pill.w < full.w / 2.0, "{pill:?} {full:?}");
    assert!(hug.w < pill.w, "{hug:?} {pill:?}");
    assert_eq!(pill.x, full.x);
    assert_eq!(long.w, full.w, "a chip never overflows its row");
    assert!(long.h > pill.h, "a long chip wraps: {long:?}");
    let pill_rect = layout.display(1).unwrap().rects[0].clone();
    assert_eq!(pill_rect.w, pill.w);
    assert!(pill_rect.radii[0] > 7.0, "a capsule: {pill_rect:?}");
}

#[test]
fn a_surface_with_a_height_reserves_a_box_for_the_adapter() {
    let mut layout = TranscriptLayout::new();
    let mut measurer = FixedMeasurer::default();
    let mut card = node(
        "card",
        Element::Surface {
            resource: "link:5f2b1c".into(),
            label: "Example\nexample.com".into(),
        },
    );
    card.style.min_height = Some(220);
    let plain = node(
        "plain",
        Element::Surface {
            resource: "link:5f2b1c".into(),
            label: "Example".into(),
        },
    );
    layout
        .update(update(vec![card, plain], 800.0), &mut measurer)
        .unwrap();
    let display = layout.display(0).unwrap().clone();
    let widget = display.widgets.first().expect("a surface widget");
    assert!(matches!(
        &widget.kind,
        WidgetKind::Surface { resource, .. } if resource == "link:5f2b1c"
    ));
    assert_eq!(widget.h, 220.0);
    assert!(widget.w <= rows::SURFACE_WIDTH);
    assert!(layout.placement(0).unwrap().height >= 220.0);
    // Without a height the label shows, and the adapter draws nothing.
    assert!(layout.display(1).unwrap().widgets.is_empty());
}
