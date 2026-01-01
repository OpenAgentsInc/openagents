//! Performance benchmarks for wgpui
//!
//! Tests rendering performance with large message counts.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;

// Mock types for benchmarking without full wgpui dependency
#[derive(Clone)]
struct Message {
    content: String,
}

impl Message {
    fn new(id: usize) -> Self {
        Self {
            content: format!(
                "This is message number {} with some realistic content that might appear in a chat application. It includes multiple sentences to simulate real-world usage patterns.",
                id
            ),
        }
    }
}

/// Simulate virtual list rendering
fn simulate_virtual_list_render(
    messages: &[Message],
    viewport_height: f32,
    item_height: f32,
) -> Vec<usize> {
    let visible_count = (viewport_height / item_height).ceil() as usize + 2; // +2 for buffer
    let start = 0; // Assuming scroll at top
    let end = (start + visible_count).min(messages.len());

    (start..end).collect()
}

/// Simulate scene building for visible items
fn simulate_scene_building(visible_indices: &[usize], messages: &[Message]) -> usize {
    let mut quad_count = 0;
    let mut text_run_count = 0;

    for &idx in visible_indices {
        if let Some(msg) = messages.get(idx) {
            // Simulate quads for message container
            quad_count += 3; // background, border, avatar

            // Simulate text runs
            text_run_count += 1; // author
            text_run_count += (msg.content.len() / 100) + 1; // content lines
        }
    }

    quad_count + text_run_count
}

/// Simulate markdown parsing
fn simulate_markdown_parse(content: &str) -> usize {
    let mut block_count = 0;

    for line in content.lines() {
        if line.starts_with('#') {
            block_count += 1; // heading
        } else if line.starts_with("```") {
            block_count += 1; // code block
        } else if line.starts_with("- ") || line.starts_with("* ") {
            block_count += 1; // list item
        } else if !line.is_empty() {
            block_count += 1; // paragraph
        }
    }

    block_count
}

/// Simulate layout computation
fn simulate_layout(message_count: usize, _viewport_width: f32) -> Vec<(f32, f32)> {
    let mut layouts = Vec::with_capacity(message_count);
    let mut y = 0.0;

    for i in 0..message_count {
        let height = 60.0 + (i % 3) as f32 * 20.0; // Variable heights
        layouts.push((y, height));
        y += height + 8.0; // gap
    }

    layouts
}

fn benchmark_virtual_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("virtual_list");
    group.measurement_time(Duration::from_secs(10));

    for size in [100, 1_000, 10_000, 50_000].iter() {
        let messages: Vec<Message> = (0..*size).map(Message::new).collect();

        group.bench_with_input(BenchmarkId::new("render", size), &messages, |b, msgs| {
            b.iter(|| {
                let visible = simulate_virtual_list_render(msgs, 800.0, 72.0);
                black_box(simulate_scene_building(&visible, msgs))
            })
        });
    }

    group.finish();
}

fn benchmark_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout");
    group.measurement_time(Duration::from_secs(10));

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(BenchmarkId::new("compute", size), size, |b, &size| {
            b.iter(|| black_box(simulate_layout(size, 800.0)))
        });
    }

    group.finish();
}

fn benchmark_markdown(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown");

    let simple = "Hello, world!\n\nThis is a paragraph.";
    let complex = r#"# Heading 1

This is a paragraph with **bold** and *italic* text.

## Code Example

```rust
fn main() {
    println!("Hello, world!");
}
```

- List item 1
- List item 2
- List item 3

Another paragraph with `inline code` and [a link](https://example.com).
"#;

    let very_long: String = (0..100)
        .map(|i| format!("Paragraph {} with some content.\n\n", i))
        .collect();

    group.bench_function("simple", |b| {
        b.iter(|| black_box(simulate_markdown_parse(simple)))
    });

    group.bench_function("complex", |b| {
        b.iter(|| black_box(simulate_markdown_parse(complex)))
    });

    group.bench_function("long", |b| {
        b.iter(|| black_box(simulate_markdown_parse(&very_long)))
    });

    group.finish();
}

fn benchmark_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll");

    let layouts = simulate_layout(10_000, 800.0);

    group.bench_function("find_visible_range", |b| {
        b.iter(|| {
            let scroll_offset = 5000.0;
            let viewport_height = 800.0;

            // Binary search for first visible
            let first = layouts.partition_point(|(y, _)| *y < scroll_offset);
            let last = layouts.partition_point(|(y, h)| *y + *h < scroll_offset + viewport_height);

            black_box((first, last.min(layouts.len())))
        })
    });

    group.finish();
}

fn benchmark_hit_testing(c: &mut Criterion) {
    let mut group = c.benchmark_group("hit_test");

    // Simulate a tree of bounds
    let bounds: Vec<(f32, f32, f32, f32)> = (0..1000)
        .map(|i| {
            let row = i / 10;
            let col = i % 10;
            (col as f32 * 80.0, row as f32 * 60.0, 75.0, 55.0)
        })
        .collect();

    group.bench_function("linear_search", |b| {
        b.iter(|| {
            let point = (450.0, 350.0);
            let hit = bounds.iter().enumerate().find(|(_, (x, y, w, h))| {
                point.0 >= *x && point.0 < x + w && point.1 >= *y && point.1 < y + h
            });
            black_box(hit.map(|(i, _)| i))
        })
    });

    group.finish();
}

fn benchmark_animation(c: &mut Criterion) {
    let mut group = c.benchmark_group("animation");

    group.bench_function("easing_linear", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            for i in 0..100 {
                let t = i as f32 / 100.0;
                sum += black_box(t); // Linear
            }
            sum
        })
    });

    group.bench_function("easing_cubic", |b| {
        b.iter(|| {
            let mut sum = 0.0_f32;
            for i in 0..100 {
                let t = i as f32 / 100.0;
                // Cubic ease-in-out
                let result = if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                };
                sum += black_box(result);
            }
            sum
        })
    });

    group.bench_function("spring_physics", |b| {
        b.iter(|| {
            let mut position = 0.0_f32;
            let mut velocity = 0.0_f32;
            let target = 100.0_f32;
            let stiffness = 100.0_f32;
            let damping = 10.0_f32;
            let dt = 0.016_f32;

            for _ in 0..100 {
                let displacement = position - target;
                let spring_force = -stiffness * displacement;
                let damping_force = -damping * velocity;
                let acceleration = spring_force + damping_force;
                velocity += acceleration * dt;
                position += velocity * dt;
            }

            black_box(position)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_virtual_list,
    benchmark_layout,
    benchmark_markdown,
    benchmark_scroll,
    benchmark_hit_testing,
    benchmark_animation,
);
criterion_main!(benches);
