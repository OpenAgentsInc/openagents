use super::*;

#[test]
fn field_plan_bounds_geometry_and_counts_every_allocation() {
    let plan = ClefFieldPlan::new(64, 1024, 16, 4096).unwrap();
    assert_eq!(plan.workspace_bytes, 2_883_584);
    assert_eq!(plan.vector_bytes(4).unwrap(), 294_912);
    let tiny = ClefFieldPlan::new(1, 1, 1, 1).unwrap();
    assert_eq!(tiny.workspace_bytes, 8 * 16);
    assert_eq!(tiny.vector_bytes(1).unwrap(), 15 * 16);
    for geometry in [
        (0, 32, 4, 48),
        (65, 32, 4, 48),
        (1, 0, 1, 48),
        (1, 32, 0, 48),
        (1, 32, 3, 48),
        (1, usize::MAX, 1, 48),
        (1, 32, 4, usize::MAX),
        (1, 32, 4, 0),
    ] {
        assert!(ClefFieldPlan::new(geometry.0, geometry.1, geometry.2, geometry.3).is_err());
    }
    assert!(plan.vector_bytes(0).is_err());
    assert!(plan.vector_bytes(17).is_err());
}

fn oracle(q: &[f32], k: &[f32], v: &[f32], rows: usize, heads: usize, width: usize) -> Vec<f32> {
    let mut output = vec![0.0; rows * width];
    let dimension = width / heads;
    for row in 0..rows {
        for head in 0..heads {
            let start = head * dimension;
            let scores: Vec<f64> = (0..rows)
                .map(|key| {
                    (0..dimension)
                        .map(|i| {
                            f64::from(q[row * width + start + i])
                                * f64::from(k[key * width + start + i])
                        })
                        .sum::<f64>()
                        / (dimension as f64).sqrt()
                })
                .collect();
            let maximum = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let exponentials: Vec<f64> = scores.iter().map(|x| (x - maximum).exp()).collect();
            let denominator: f64 = exponentials.iter().sum();
            for i in 0..dimension {
                output[row * width + start + i] = (0..rows)
                    .map(|key| {
                        exponentials[key] / denominator * f64::from(v[key * width + start + i])
                    })
                    .sum::<f64>() as f32;
            }
        }
    }
    output
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn field_attention_matches_f64_and_preserves_sources_and_guards() {
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().unwrap();
        for (width, heads) in [(32, 4), (1024, 16), (30, 3)] {
            for rows in [1, 3, 9, 17, 63, 64] {
                let values = |salt| {
                    (0..rows * width)
                        .map(|i| ((i * 13 + i / width * 7 + salt) % 101) as f32 / 53.0 - 0.9)
                        .collect::<Vec<_>>()
                };
                let inputs = [values(1), values(31), values(67)];
                let buffers = inputs.each_ref().map(|v| metal.buffer_f32(v));
                let parent = metal.buffer_with_bytes(&vec![0xa5; rows * width * 4 + 64]);
                let out = parent.view(32, rows * width * 4).unwrap();
                let batch = metal.batch();
                batch
                    .field_self_attention(
                        &buffers[0],
                        &buffers[1],
                        &buffers[2],
                        &out,
                        rows,
                        heads,
                        width,
                    )
                    .unwrap();
                batch.commit_wait().unwrap();
                let got = out.read_f32(0, rows * width).unwrap();
                let want = oracle(&inputs[0], &inputs[1], &inputs[2], rows, heads, width);
                for (index, (a, b)) in got.iter().zip(&want).enumerate() {
                    assert!(
                        a.is_finite() && (a - b).abs() <= 1e-5,
                        "rows={rows} width={width} index={index}: {a} vs {b}"
                    );
                }
                for (source, expected) in buffers.iter().zip(inputs.iter()) {
                    assert_eq!(source.read_f32(0, expected.len()).unwrap(), *expected);
                }
                assert_eq!(parent.read_bytes(0, 32).unwrap(), vec![0xa5; 32]);
                assert_eq!(
                    parent.read_bytes(32 + rows * width * 4, 32).unwrap(),
                    vec![0xa5; 32]
                );
            }
        }
    });
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn field_helpers_refuse_bad_extents_and_unsafe_aliases_before_dispatch() {
    objc::rc::autoreleasepool(|| {
        let metal = ClefMetal::new().unwrap();
        let a = metal.buffer_f32(&vec![2.0; 128]);
        let b = metal.buffer_f32(&vec![3.0; 128]);
        let out = metal.buffer(512);
        let short = metal.buffer(16);
        let batch = metal.batch();
        for input in 0..3 {
            let mut inputs = [&a, &b, &a];
            inputs[input] = &short;
            assert!(
                batch
                    .field_self_attention(inputs[0], inputs[1], inputs[2], &out, 3, 4, 32)
                    .is_err()
            );
        }
        assert!(
            batch
                .field_self_attention(&a, &b, &a, &short, 3, 4, 32)
                .is_err()
        );
        for alias in [&a, &b] {
            assert!(
                batch
                    .field_self_attention(&a, &b, &a, alias, 3, 4, 32)
                    .is_err()
            );
        }
        assert!(
            batch
                .field_self_attention(&a, &b, &a, &out, 65, 4, 32)
                .is_err()
        );
        assert!(batch.add_f32(&a, &a.view(16, 496).unwrap(), 100).is_err());
        assert!(batch.add_f32(&a, &b, usize::MAX).is_err());
        assert!(batch.add_f32(&short, &b, 5).is_err());
        assert!(!batch.dispatched.get());
        // Read-only aliases are permitted, as is exact in-place residual doubling.
        batch
            .field_self_attention(&a, &a, &a, &out, 3, 4, 32)
            .unwrap();
        batch.add_f32(&out, &out, 96).unwrap();
        batch.commit_wait().unwrap();
        assert_eq!(out.read_f32(0, 96).unwrap(), vec![4.0; 96]);
        assert_eq!(a.read_f32(0, 128).unwrap(), vec![2.0; 128]);
        assert_eq!(b.read_f32(0, 128).unwrap(), vec![3.0; 128]);
    });
}
