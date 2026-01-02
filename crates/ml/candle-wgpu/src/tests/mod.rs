use crate::ops::{self, AttentionParams, BinaryOp, UnaryOp};
use crate::WgpuDevice;

fn maybe_device() -> Option<WgpuDevice> {
    WgpuDevice::new().ok()
}

fn assert_close(actual: f32, expected: f32) {
    let diff = (actual - expected).abs();
    assert!(diff < 1e-3, "expected {expected}, got {actual}");
}

#[test]
fn test_binary_add() {
    let device = match maybe_device() {
        Some(device) => device,
        None => return,
    };
    let out = ops::binary(&device, BinaryOp::Add, &[1.0, 2.0], &[3.0, 4.0]).unwrap();
    assert_eq!(out.len(), 2);
    assert_close(out[0], 4.0);
    assert_close(out[1], 6.0);
}

#[test]
fn test_unary_relu() {
    let device = match maybe_device() {
        Some(device) => device,
        None => return,
    };
    let out = ops::unary(&device, UnaryOp::Relu, &[-1.0, 0.5, 2.0]).unwrap();
    assert_eq!(out.len(), 3);
    assert_close(out[0], 0.0);
    assert_close(out[1], 0.5);
    assert_close(out[2], 2.0);
}

#[test]
fn test_matmul_small() {
    let device = match maybe_device() {
        Some(device) => device,
        None => return,
    };
    let a = vec![
        1.0, 2.0, 3.0, //
        4.0, 5.0, 6.0,
    ];
    let b = vec![
        7.0, 8.0, //
        9.0, 10.0, //
        11.0, 12.0,
    ];
    let out = ops::matmul(&device, &a, &b, 2, 2, 3).unwrap();
    assert_eq!(out.len(), 4);
    assert_close(out[0], 58.0);
    assert_close(out[1], 64.0);
    assert_close(out[2], 139.0);
    assert_close(out[3], 154.0);
}

#[test]
fn test_attention_uniform() {
    let device = match maybe_device() {
        Some(device) => device,
        None => return,
    };
    let params = AttentionParams {
        batch: 1,
        heads: 1,
        seq_len: 4,
        head_dim: 4,
        causal: false,
    };
    let total = params.batch * params.heads * params.seq_len * params.head_dim;
    let q = vec![1.0; total];
    let k = vec![1.0; total];
    let v = vec![1.0; total];
    let out = ops::attention(&device, &q, &k, &v, params).unwrap();
    assert_eq!(out.len(), total);
    for value in out {
        assert_close(value, 1.0);
    }
}
