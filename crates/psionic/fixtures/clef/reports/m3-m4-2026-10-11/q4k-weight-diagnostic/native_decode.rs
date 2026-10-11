// Compare the real native CPU decoder with retained independent Q4_K outputs.
// Compile only when the parent coordinates the pinned Rust build lease.
use psionic_core::QuantizationMode;

fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("sample directory"));
    let packed = std::fs::read(root.join("sample-blocks.q4k")).expect("packed samples");
    let expected = std::fs::read(root.join("independent-decoded.f32")).expect("independent samples");
    assert!(!packed.is_empty() && packed.len() <= 2 * 1024 * 1024);
    assert_eq!(packed.len() % 144, 0);
    assert_eq!(expected.len(), packed.len() / 144 * 256 * 4);
    let mut decoded = Vec::new();
    psionic_backend_cpu::decode_quantized_row_into(QuantizationMode::GgmlQ4K, &packed, &mut decoded)
        .expect("native decode");
    assert_eq!(decoded.len() * 4, expected.len());
    let mut max_delta = 0.0f32;
    let mut mismatches = 0usize;
    for (actual, bytes) in decoded.iter().zip(expected.chunks_exact(4)) {
        let want = f32::from_le_bytes(bytes.try_into().unwrap());
        assert!(actual.is_finite() && want.is_finite());
        max_delta = max_delta.max((*actual - want).abs());
        mismatches += usize::from(actual.to_bits() != want.to_bits());
    }
    println!("{{\"values\":{},\"bitwise_mismatches\":{},\"max_abs_delta\":{},\"finite\":true}}",
        decoded.len(), mismatches, max_delta);
    assert_eq!(mismatches, 0, "native decoder differs from independent GGUF output");
}
