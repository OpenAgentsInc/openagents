use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::time::Duration;
use wgpui::{Bounds, FontStyle, Hsla, Point, Quad, Scene, TextSystem};

fn bench_scene_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_scene");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));

    for quad_count in [500_usize, 1_000, 2_000] {
        group.bench_with_input(
            BenchmarkId::new("build_quads", quad_count),
            &quad_count,
            |b, &count| {
                b.iter(|| {
                    let mut scene = Scene::new();
                    for i in 0..count {
                        let x = (i % 50) as f32 * 8.0;
                        let y = (i / 50) as f32 * 8.0;
                        scene.draw_quad(
                            Quad::new(Bounds::new(x, y, 6.0, 6.0))
                                .with_background(Hsla::new(210.0, 0.6, 0.5, 1.0)),
                        );
                    }

                    black_box(scene.layers().len())
                });
            },
        );
    }

    group.finish();
}

fn bench_text_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("micro_text");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));

    let paragraph =
        "Autopilot keeps local state deterministic while rendering dense markdown and UI overlays."
            .repeat(12);

    group.bench_function("measure_styled_mono_1kb", |b| {
        let mut text = TextSystem::new(1.0);
        b.iter(|| {
            black_box(text.measure_styled_mono(
                &paragraph,
                14.0,
                FontStyle {
                    bold: false,
                    italic: false,
                },
            ))
        });
    });

    group.bench_function("layout_styled_mono_1kb", |b| {
        let mut text = TextSystem::new(1.0);
        b.iter(|| {
            black_box(text.layout_styled_mono(
                &paragraph,
                Point::new(0.0, 0.0),
                14.0,
                Hsla::white(),
                FontStyle {
                    bold: false,
                    italic: false,
                },
            ))
        });
    });

    group.finish();
}

criterion_group!(microbenches, bench_scene_build, bench_text_ops);
criterion_main!(microbenches);
