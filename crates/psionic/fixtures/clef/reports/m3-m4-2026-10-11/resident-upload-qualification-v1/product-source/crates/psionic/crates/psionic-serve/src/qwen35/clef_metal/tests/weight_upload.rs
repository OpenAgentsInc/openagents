use super::*;

fn fixture(path: &std::path::Path, kind: u32, block: usize) -> Vec<u8> {
    let mut payload = vec![0; 3 * 256 / if kind == 8 { 32 } else { 256 } * block];
    for (index, bytes) in payload.chunks_mut(block).enumerate() {
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = (index * 17 + offset * 29 + 11) as u8;
        }
        let scale = if kind == 14 { 208 } else { 0 };
        bytes[scale..scale + 2].copy_from_slice(&0x1400u16.to_le_bytes());
        if kind == 12 {
            bytes[2..4].copy_from_slice(&0x1000u16.to_le_bytes());
        }
    }
    let mut bytes = b"GGUF".to_vec();
    bytes.extend(3u32.to_le_bytes());
    bytes.extend(1u64.to_le_bytes()); // One tensor, no metadata.
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(7u64.to_le_bytes());
    bytes.extend(b"weights");
    bytes.extend(2u32.to_le_bytes());
    bytes.extend(256u64.to_le_bytes());
    bytes.extend(3u64.to_le_bytes());
    bytes.extend(kind.to_le_bytes());
    bytes.extend(0u64.to_le_bytes());
    bytes.resize(bytes.len().div_ceil(32) * 32, 0);
    bytes.extend(&payload);
    std::fs::write(path, bytes).unwrap();
    payload
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn positional_weight_upload_preserves_resident_bytes_and_gemm_outputs() {
    objc::rc::autoreleasepool(|| {
        let temp = tempfile::tempdir().unwrap();
        let metal = ClefMetal::new().unwrap();
        for (kind, block) in [(8, 34), (12, 144), (14, 210)] {
            let path = temp.path().join(format!("weights-{kind}.gguf"));
            let payload = fixture(&path, kind, block);
            let artifact = psionic_models::GgufBlobArtifact::open_path(
                &path,
                psionic_catalog::LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::RequireMemoryMap),
            )
            .unwrap();
            let host = HostMatrix::load(&artifact, "weights").unwrap();
            drop(artifact);
            let HostMatrixKind::Quantized(source) = &host.kind else {
                panic!("quantized fixture")
            };
            let mut copied = vec![0; payload.len()];
            source.storage.read_range_into(0, &mut copied).unwrap();
            assert_eq!(copied, payload);
            let mapped =
                upload_matrix(&metal, &host, "weights", WeightUploadMode::MappedCopy).unwrap();
            let positional =
                upload_matrix(&metal, &host, "weights", WeightUploadMode::Positional).unwrap();
            assert_eq!(
                mapped.buffer.read_bytes(0, 3 * 256 * 2).unwrap(),
                positional.buffer.read_bytes(0, 3 * 256 * 2).unwrap()
            );
            for n in [1, 17] {
                let input: Vec<f32> = (0..n * 256)
                    .map(|i| ((i * 7) % 31) as f32 / 32.0 - 0.5)
                    .collect();
                let input = metal.buffer_f32(&input);
                let half = metal.buffer(n * 256 * 2);
                let batch = metal.batch();
                batch
                    .dequantize_to_f16(ClefMetalWeightFormat::F32, &input, &half, n * 256)
                    .unwrap();
                batch.commit_wait().unwrap();
                for accumulate in [false, true] {
                    let seed = vec![0.375; n * 3];
                    let a = metal.buffer_f32(&seed);
                    let b = metal.buffer_f32(&seed);
                    let batch = metal.batch();
                    batch
                        .gemm(&half, 0, &mapped.buffer, &a, 0, n, 3, 256, accumulate)
                        .unwrap();
                    batch
                        .gemm(&half, 0, &positional.buffer, &b, 0, n, 3, 256, accumulate)
                        .unwrap();
                    batch.commit_wait().unwrap();
                    let a = a.read_f32(0, n * 3).unwrap();
                    let b = b.read_f32(0, n * 3).unwrap();
                    assert!(a.iter().chain(&b).all(|v| v.is_finite()));
                    assert_eq!(
                        a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                        b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
                    );
                }
            }
            assert_eq!(source.storage.bytes().unwrap(), payload);
            let buffered = psionic_models::GgufBlobArtifact::open_path(
                &path,
                psionic_catalog::LocalBlobOpenOptions::default()
                    .with_read_preference(psionic_catalog::BlobReadPreference::PreferBuffered),
            )
            .unwrap();
            let buffered = HostMatrix::load(&buffered, "weights").unwrap();
            assert!(
                upload_matrix(&metal, &buffered, "weights", WeightUploadMode::Positional).is_err()
            );
        }
    });
}
