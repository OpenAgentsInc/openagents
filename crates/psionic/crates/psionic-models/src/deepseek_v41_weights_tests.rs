//! Offline tests for the V4.1 safetensors reader. Synthetic shards cover every
//! dtype and layout; the checked-in index and three real shard headers cover
//! the pinned checkpoint's names, dtypes, shapes, and offsets.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::{io::Read as _, path::Path};

use super::*;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/testdata/deepseek_v41_weights"
);

// The reference table, `inference/convert.py` lines 13–15, codes 0..16.
const REFERENCE_FP4_TABLE: [f32; 16] = [
    0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 0.0, -0.5, -1.0, -1.5, -2.0, -3.0, -4.0, -6.0,
];

fn gunzip(path: &str) -> Vec<u8> {
    let file = std::fs::File::open(Path::new(FIXTURES).join(path)).unwrap();
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(file)
        .read_to_end(&mut out)
        .unwrap();
    out
}

// ---------------------------------------------------------------------------
// Scalar decoding
// ---------------------------------------------------------------------------

#[test]
fn e4m3fn_decodes_normals_subnormals_and_nan() {
    assert_eq!(decode_f8_e4m3fn(0x00).to_bits(), 0.0_f32.to_bits());
    assert_eq!(decode_f8_e4m3fn(0x80).to_bits(), (-0.0_f32).to_bits());
    assert_eq!(decode_f8_e4m3fn(0x01), 2.0_f32.powi(-9));
    assert_eq!(decode_f8_e4m3fn(0x07), 7.0 * 2.0_f32.powi(-9));
    assert_eq!(decode_f8_e4m3fn(0x08), 2.0_f32.powi(-6));
    assert_eq!(decode_f8_e4m3fn(0x38), 1.0);
    assert_eq!(decode_f8_e4m3fn(0x3C), 1.5);
    assert_eq!(decode_f8_e4m3fn(0xC0), -2.0);
    assert_eq!(decode_f8_e4m3fn(0x7E), 448.0);
    assert_eq!(decode_f8_e4m3fn(0xFE), -448.0);
    assert!(decode_f8_e4m3fn(0x7F).is_nan());
    assert!(decode_f8_e4m3fn(0xFF).is_nan());
    // Exactly two NaN codes; everything else is finite and sign-symmetric.
    let nans = (0..=255_u8)
        .filter(|&b| decode_f8_e4m3fn(b).is_nan())
        .count();
    assert_eq!(nans, 2);
    for byte in 0..0x7F_u8 {
        assert_eq!(decode_f8_e4m3fn(byte), -decode_f8_e4m3fn(byte | 0x80));
        assert!(decode_f8_e4m3fn(byte) < decode_f8_e4m3fn(byte + 1) || byte == 0x7E);
    }
}

#[test]
fn e8m0fnu_decodes_powers_of_two_and_nan() {
    assert_eq!(decode_f8_e8m0fnu(127), 1.0);
    assert_eq!(decode_f8_e8m0fnu(128), 2.0);
    assert_eq!(decode_f8_e8m0fnu(120), 2.0_f32.powi(-7));
    assert_eq!(decode_f8_e8m0fnu(254), 2.0_f32.powi(127));
    assert_eq!(decode_f8_e8m0fnu(1), 2.0_f32.powi(-126));
    // 2^-127 is subnormal in f32 and still exact.
    assert_eq!(decode_f8_e8m0fnu(0), 2.0_f32.powi(-126) / 2.0);
    assert!(decode_f8_e8m0fnu(255).is_nan());
}

#[test]
fn e2m1_matches_the_reference_table() {
    for code in 0..16_u8 {
        assert_eq!(decode_e2m1(code), REFERENCE_FP4_TABLE[usize::from(code)]);
    }
    // float4_e2m1fn code 8 is negative zero; numerically equal to the table.
    assert!(decode_e2m1(8).is_sign_negative());
}

#[test]
fn fp4_low_nibble_is_the_even_element() {
    // Byte 0x71: low nibble 1 (0.5) is element 0, high nibble 7 (6.0) is
    // element 1 (`convert.py` lines 32–34).
    assert_eq!(unpack_e2m1_pair(0x71), (0x1, 0x7));
    // Byte 0x2F: low 0xF (-6.0), high 0x2 (1.0). Scale 2^1.
    let values = dequantize_fp4_row(&[0x71, 0x2F], &[128]).unwrap();
    assert_eq!(values, vec![1.0, 12.0, -12.0, 2.0]);
}

#[test]
fn fp4_scales_switch_every_32_logical_values() {
    // 64 logical values = 32 bytes; byte 15 holds elements 30, 31 (scale 0),
    // byte 16 holds elements 32, 33 (scale 1).
    let packed = vec![0x22_u8; 32]; // every element 1.0
    let values = dequantize_fp4_row(&packed, &[127, 130]).unwrap();
    assert_eq!(values.len(), 64);
    assert!(values[..32].iter().all(|&v| v == 1.0));
    assert!(values[32..].iter().all(|&v| v == 8.0));
    assert!(dequantize_fp4_row(&packed, &[127]).is_err());
}

#[test]
fn bf16_and_f32_decode_little_endian() {
    assert_eq!(decode_bf16([0x80, 0x3F]), 1.0);
    assert_eq!(decode_bf16([0x00, 0xC0]), -2.0);
    assert_eq!(decode_f32(1.25_f32.to_le_bytes()), 1.25);
}

#[test]
fn ggml_mxfp4_is_not_byte_compatible() {
    for format in [
        V41BlockFormat::Fp8Block32x32,
        V41BlockFormat::Fp8Row32,
        V41BlockFormat::Fp4E2M1Row32,
    ] {
        assert!(!format.ggml_mxfp4_byte_compatible());
    }
    // GGML MXFP4 puts element j + 16 in the high nibble of byte j; V4.1 puts
    // element 2j + 1 there. The same 16 bytes decode to a different order.
    let packed: Vec<u8> = (0..16_u8).map(|i| (i % 8) | (((i + 3) % 8) << 4)).collect();
    let v41 = dequantize_fp4_row(&packed, &[127]).unwrap();
    let mut ggml = vec![0.0_f32; 32];
    for (j, byte) in packed.iter().enumerate() {
        ggml[j] = decode_e2m1(byte & 0x0F);
        ggml[j + 16] = decode_e2m1(byte >> 4);
    }
    assert_ne!(v41, ggml);
}

// ---------------------------------------------------------------------------
// Synthetic shards
// ---------------------------------------------------------------------------

struct Tensor {
    name: &'static str,
    dtype: &'static str,
    shape: Vec<u64>,
    data: Vec<u8>,
}

fn shard_bytes(tensors: &[Tensor]) -> Vec<u8> {
    let mut header = serde_json::Map::new();
    header.insert("__metadata__".into(), serde_json::json!({ "format": "pt" }));
    let mut offset = 0_u64;
    for tensor in tensors {
        let end = offset + tensor.data.len() as u64;
        header.insert(
            tensor.name.into(),
            serde_json::json!({
                "dtype": tensor.dtype,
                "shape": tensor.shape,
                "data_offsets": [offset, end],
            }),
        );
        offset = end;
    }
    raw_shard(&serde_json::to_vec(&header).unwrap(), tensors)
}

fn raw_shard(header: &[u8], tensors: &[Tensor]) -> Vec<u8> {
    let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
    bytes.extend_from_slice(header);
    for tensor in tensors {
        bytes.extend_from_slice(&tensor.data);
    }
    bytes
}

/// A deterministic FP8 byte that is never a NaN code.
fn fp8_byte(index: usize) -> u8 {
    let byte = ((index * 37 + 11) % 256) as u8;
    if byte & 0x7F == 0x7F { 0x3A } else { byte }
}

fn scale_bytes(count: usize) -> Vec<u8> {
    (0..count).map(|i| 120 + (i % 14) as u8).collect()
}

const FP8_ROWS: usize = 40; // two block rows, the second only 8 tall
const FP8_COLS: usize = 70; // three block columns, the last only 6 wide

fn synthetic_tensors() -> (Vec<Tensor>, Vec<Tensor>) {
    let fp8 = Tensor {
        name: "layers.0.attn.wkv.weight",
        dtype: "F8_E4M3",
        shape: vec![FP8_ROWS as u64, FP8_COLS as u64],
        data: (0..FP8_ROWS * FP8_COLS).map(fp8_byte).collect(),
    };
    let fp8_scale = Tensor {
        name: "layers.0.attn.wkv.scale",
        dtype: "F8_E8M0",
        shape: vec![2, 3],
        data: scale_bytes(6),
    };
    let fp4 = Tensor {
        name: "layers.0.ffn.experts.0.w1.weight",
        dtype: "I8",
        shape: vec![3, 32],
        data: (0..96).map(|i| ((i * 29 + 3) % 256) as u8).collect(),
    };
    let fp4_scale = Tensor {
        name: "layers.0.ffn.experts.0.w1.scale",
        dtype: "F8_E8M0",
        shape: vec![3, 2],
        data: scale_bytes(6),
    };
    let table = Tensor {
        name: "layers.1.engram.embed.weight",
        dtype: "F8_E4M3",
        shape: vec![5, 64],
        data: (0..320).map(|i| fp8_byte(i + 7)).collect(),
    };
    let table_scale = Tensor {
        name: "layers.1.engram.embed.scale",
        dtype: "F8_E8M0",
        shape: vec![5, 2],
        data: scale_bytes(10).into_iter().rev().collect(),
    };
    let bf16 = Tensor {
        name: "layers.0.attn_norm.weight",
        dtype: "BF16",
        shape: vec![2, 3],
        data: [1.0_f32, -2.0, 0.5, 3.0, 0.0, -0.25]
            .iter()
            .flat_map(|v| {
                let bits = (v.to_bits() >> 16) as u16;
                bits.to_le_bytes()
            })
            .collect(),
    };
    let f32_tensor = Tensor {
        name: "layers.0.attn.attn_sink",
        dtype: "F32",
        shape: vec![4],
        data: [1.5_f32, -1.0, 2.0, 0.125]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect(),
    };
    (
        vec![fp8, fp8_scale, bf16, f32_tensor],
        vec![fp4, fp4_scale, table, table_scale],
    )
}

fn write_checkpoint(dir: &Path, total_size: Option<u64>) -> (Vec<Tensor>, Vec<Tensor>) {
    let (first, second) = synthetic_tensors();
    std::fs::write(dir.join("a.safetensors"), shard_bytes(&first)).unwrap();
    std::fs::write(dir.join("b.safetensors"), shard_bytes(&second)).unwrap();
    let mut map = serde_json::Map::new();
    for tensor in &first {
        map.insert(tensor.name.into(), "a.safetensors".into());
    }
    for tensor in &second {
        map.insert(tensor.name.into(), "b.safetensors".into());
    }
    let total: u64 = first
        .iter()
        .chain(&second)
        .map(|t| t.data.len() as u64)
        .sum();
    let index = serde_json::json!({
        "metadata": { "total_size": total_size.unwrap_or(total) },
        "weight_map": map,
    });
    std::fs::write(
        dir.join(SAFETENSORS_INDEX_FILE),
        serde_json::to_vec(&index).unwrap(),
    )
    .unwrap();
    (first, second)
}

fn unpinned() -> CheckpointOpenOptions {
    CheckpointOpenOptions {
        headers_only: false,
        pin: None,
        check_model_map: false,
    }
}

fn open_synthetic() -> (
    tempfile::TempDir,
    DeepSeekV41Checkpoint,
    Vec<Tensor>,
    Vec<Tensor>,
) {
    let dir = tempfile::tempdir().unwrap();
    let (first, second) = write_checkpoint(dir.path(), None);
    let checkpoint = DeepSeekV41Checkpoint::open(dir.path(), unpinned()).unwrap();
    (dir, checkpoint, first, second)
}

/// The reference rule for an FP8 block-scaled weight, written independently:
/// `w[r][c] * s[r / 32][c / 32]`.
fn reference_fp8(data: &[u8], scales: &[u8], cols: usize, row: usize, col: usize) -> f32 {
    let scale_cols = cols.div_ceil(32);
    decode_f8_e4m3fn(data[row * cols + col])
        * decode_f8_e8m0fnu(scales[(row / 32) * scale_cols + col / 32])
}

#[test]
fn synthetic_checkpoint_reads_every_dtype_and_layout() {
    let (_dir, checkpoint, first, second) = open_synthetic();
    assert_eq!(checkpoint.index().weight_map.len(), 8);
    assert_eq!(
        checkpoint.block_format("layers.0.attn.wkv.weight").unwrap(),
        V41BlockFormat::Fp8Block32x32
    );
    assert_eq!(
        checkpoint
            .block_format("layers.0.ffn.experts.0.w1.weight")
            .unwrap(),
        V41BlockFormat::Fp4E2M1Row32
    );
    assert_eq!(
        checkpoint
            .block_format("layers.1.engram.embed.weight")
            .unwrap(),
        V41BlockFormat::Fp8Row32
    );
    assert_eq!(
        checkpoint
            .block_format("layers.0.attn_norm.weight")
            .unwrap(),
        V41BlockFormat::Bf16
    );
    assert_eq!(
        checkpoint.block_format("layers.0.attn.attn_sink").unwrap(),
        V41BlockFormat::F32
    );
    assert!(checkpoint.block_format("layers.0.attn.wkv.scale").is_err());

    // Every FP8 block, including the 8-tall bottom row and 6-wide right edge.
    let (data, scales) = (&first[0].data, &first[1].data);
    for block_row in 0..2_u64 {
        for block_col in 0..3_u64 {
            let block = checkpoint
                .fp8_weight_block("layers.0.attn.wkv.weight", block_row, block_col)
                .unwrap();
            let height = if block_row == 1 { 8 } else { 32 };
            let width = if block_col == 2 { 6 } else { 32 };
            assert_eq!((block.rows, block.cols), (height, width));
            for r in 0..height {
                for c in 0..width {
                    let row = block_row as usize * 32 + r;
                    let col = block_col as usize * 32 + c;
                    assert_eq!(
                        block.values[r * width + c],
                        reference_fp8(data, scales, FP8_COLS, row, col)
                    );
                }
            }
        }
    }
    assert!(
        checkpoint
            .fp8_weight_block("layers.0.attn.wkv.weight", 2, 0)
            .is_err()
    );

    // FP8 rows on both sides of the block-row boundary and the last row.
    for row in [0_usize, 31, 32, 39] {
        let values = checkpoint
            .dequantize_row("layers.0.attn.wkv.weight", row as u64)
            .unwrap();
        let expected: Vec<f32> = (0..FP8_COLS)
            .map(|col| reference_fp8(data, scales, FP8_COLS, row, col))
            .collect();
        assert_eq!(values, expected, "row {row}");
    }

    // FP4 rows: element 2i from the low nibble, scale per 32 along K = 64.
    let (packed, fp4_scales) = (&second[0].data, &second[1].data);
    for row in 0..3_usize {
        let values = checkpoint
            .dequantize_row("layers.0.ffn.experts.0.w1.weight", row as u64)
            .unwrap();
        assert_eq!(values.len(), 64);
        for k in 0..64 {
            let byte = packed[row * 32 + k / 2];
            let code = if k % 2 == 0 { byte & 0x0F } else { byte >> 4 };
            let expected = REFERENCE_FP4_TABLE[usize::from(code)]
                * decode_f8_e8m0fnu(fp4_scales[row * 2 + k / 32]);
            assert_eq!(values[k], expected, "row {row} k {k}");
        }
    }

    // Engram rows: `values.unflatten(-1, (-1, 32)) * scales.unsqueeze(-1)`.
    let (table, table_scales) = (&second[2].data, &second[3].data);
    for row in [0_usize, 4] {
        let values = checkpoint.engram_row(1, row as u64).unwrap();
        let expected: Vec<f32> = (0..64)
            .map(|c| {
                decode_f8_e4m3fn(table[row * 64 + c])
                    * decode_f8_e8m0fnu(table_scales[row * 2 + c / 32])
            })
            .collect();
        assert_eq!(values, expected);
    }
    assert!(checkpoint.engram_row(1, 5).is_err());

    assert_eq!(
        checkpoint
            .dequantize_row("layers.0.attn_norm.weight", 1)
            .unwrap(),
        vec![3.0, 0.0, -0.25]
    );
    assert_eq!(
        checkpoint
            .dequantize_row("layers.0.attn.attn_sink", 0)
            .unwrap(),
        vec![1.5, -1.0, 2.0, 0.125]
    );
}

#[test]
fn ranged_reads_stay_inside_the_tensor() {
    let (_dir, checkpoint, first, _) = open_synthetic();
    let mut out = [0_u8; 4];
    checkpoint
        .read_tensor_bytes("layers.0.attn.wkv.weight", 10, &mut out)
        .unwrap();
    assert_eq!(&out, &first[0].data[10..14]);
    let len = first[0].data.len() as u64;
    assert!(matches!(
        checkpoint.read_tensor_bytes("layers.0.attn.wkv.weight", len - 3, &mut out),
        Err(V41WeightsError::OutOfRange { .. })
    ));
    assert!(matches!(
        checkpoint.read_tensor_bytes("nope", 0, &mut out),
        Err(V41WeightsError::NoSuchTensor(_))
    ));
    assert_eq!(
        checkpoint.read_tensor("layers.0.attn.attn_sink").unwrap(),
        first[3].data
    );
}

#[test]
fn headers_only_refuses_data_reads() {
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), None);
    let checkpoint = DeepSeekV41Checkpoint::open(
        dir.path(),
        CheckpointOpenOptions {
            headers_only: true,
            ..unpinned()
        },
    )
    .unwrap();
    assert!(checkpoint.headers_only());
    assert!(matches!(
        checkpoint.dequantize_row("layers.0.attn.wkv.weight", 0),
        Err(V41WeightsError::HeadersOnly)
    ));
    assert!(matches!(
        checkpoint.verify_shard_hashes(None),
        Err(V41WeightsError::HeadersOnly)
    ));
}

#[test]
fn shard_hashes_are_a_separate_step() {
    let (dir, checkpoint, _, _) = open_synthetic();
    let identity = checkpoint.identity();
    assert!(identity.shards.values().all(|shard| !shard.verified));
    let verified = checkpoint.verify_shard_hashes(None).unwrap();
    let expected = sha256_file(&dir.path().join("a.safetensors")).unwrap();
    assert_eq!(verified.shards["a.safetensors"].sha256, expected);
    assert!(verified.shards.values().all(|shard| shard.verified));
    assert_ne!(identity.digest(), verified.digest());
}

#[test]
fn stage_selection_totals_bytes_by_layer() {
    let (_dir, checkpoint, first, second) = open_synthetic();
    let layer0 = checkpoint.select(&V41StageSelection::layers(0..1)).unwrap();
    let expected: u64 = first
        .iter()
        .chain(&second[..2])
        .map(|t| t.data.len() as u64)
        .sum();
    assert_eq!(layer0.total_bytes, expected);
    assert_eq!(layer0.tensors.len(), 6);
    assert_eq!(layer0.routed_expert_bytes, 96 + 6);
    let layer1 = checkpoint.select(&V41StageSelection::layers(1..2)).unwrap();
    assert_eq!(layer1.engram_table_bytes, 320 + 10);
    assert_eq!(layer1.bytes_by_shard.len(), 1);
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

fn parse_shard(bytes: &[u8]) -> Result<SafetensorsShardHeader, V41WeightsError> {
    SafetensorsShardHeader::parse("x.safetensors", bytes, Some(bytes.len() as u64))
}

fn one(name: &'static str, dtype: &'static str, shape: Vec<u64>, len: usize) -> Tensor {
    Tensor {
        name,
        dtype,
        shape,
        data: vec![0x38; len],
    }
}

#[test]
fn corrupt_headers_are_refused() {
    // Shorter than the length prefix.
    assert!(matches!(
        parse_shard(&[1, 2, 3]),
        Err(V41WeightsError::InvalidHeader { .. })
    ));
    // Length prefix past the file end.
    let mut bytes = 1000_u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(b"{}");
    assert!(matches!(
        parse_shard(&bytes),
        Err(V41WeightsError::InvalidHeader { .. })
    ));
    // A zero or absurd length.
    assert!(parse_shard(&0_u64.to_le_bytes()).is_err());
    assert!(parse_shard(&u64::MAX.to_le_bytes()).is_err());
    // Invalid JSON.
    assert!(matches!(
        parse_shard(&raw_shard(b"{\"a\": [", &[])),
        Err(V41WeightsError::InvalidHeader { .. })
    ));
    // Byte length that doesn't match the shape.
    let wrong = raw_shard(
        br#"{"t":{"dtype":"BF16","shape":[2,2],"data_offsets":[0,4]}}"#,
        &[one("t", "BF16", vec![], 4)],
    );
    assert!(matches!(
        parse_shard(&wrong),
        Err(V41WeightsError::InvalidHeader { .. })
    ));
}

#[test]
fn unknown_dtypes_are_refused() {
    for dtype in ["F16", "F8_E5M2", "U8", "F4", "bf16"] {
        let header = format!(r#"{{"t":{{"dtype":"{dtype}","shape":[2],"data_offsets":[0,2]}}}}"#);
        let bytes = raw_shard(header.as_bytes(), &[one("t", "I8", vec![], 2)]);
        assert!(
            matches!(
                parse_shard(&bytes),
                Err(V41WeightsError::UnknownDtype { .. })
            ),
            "{dtype}"
        );
    }
}

#[test]
fn overlapping_and_out_of_bounds_ranges_are_refused() {
    let overlap = raw_shard(
        br#"{"a":{"dtype":"I8","shape":[4],"data_offsets":[0,4]},
            "b":{"dtype":"I8","shape":[4],"data_offsets":[2,6]}}"#,
        &[one("x", "I8", vec![], 6)],
    );
    assert!(matches!(
        parse_shard(&overlap),
        Err(V41WeightsError::OverlappingTensors { .. })
    ));
    let past_end = raw_shard(
        br#"{"a":{"dtype":"F32","shape":[4],"data_offsets":[0,16]}}"#,
        &[one("x", "I8", vec![], 8)],
    );
    assert!(matches!(
        parse_shard(&past_end),
        Err(V41WeightsError::PastFileEnd { .. })
    ));
    // Without a known file length (a header-only copy), the same header parses.
    assert!(SafetensorsShardHeader::parse("x", &past_end, None).is_ok());
}

#[test]
fn index_and_shards_must_agree() {
    // The index names a tensor its shard doesn't hold.
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), None);
    let index_path = dir.path().join(SAFETENSORS_INDEX_FILE);
    let mut index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
    index["weight_map"]["layers.0.missing.weight"] = "a.safetensors".into();
    std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    assert!(matches!(
        DeepSeekV41Checkpoint::open(dir.path(), unpinned()),
        Err(V41WeightsError::MissingTensor { .. })
    ));

    // A shard holds a tensor the index doesn't map.
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), None);
    let index_path = dir.path().join(SAFETENSORS_INDEX_FILE);
    let mut index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index_path).unwrap()).unwrap();
    index["weight_map"]
        .as_object_mut()
        .unwrap()
        .remove("layers.0.attn.attn_sink");
    index["metadata"]["total_size"] = serde_json::Value::Null;
    std::fs::write(&index_path, serde_json::to_vec(&index).unwrap()).unwrap();
    assert!(matches!(
        DeepSeekV41Checkpoint::open(dir.path(), unpinned()),
        Err(V41WeightsError::UnindexedTensor { .. })
    ));

    // The declared total doesn't match.
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), Some(1));
    assert!(matches!(
        DeepSeekV41Checkpoint::open(dir.path(), unpinned()),
        Err(V41WeightsError::TotalSizeMismatch { .. })
    ));

    // A missing shard file.
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), None);
    std::fs::remove_file(dir.path().join("b.safetensors")).unwrap();
    assert!(matches!(
        DeepSeekV41Checkpoint::open(dir.path(), unpinned()),
        Err(V41WeightsError::Io { .. })
    ));

    // A shard name that escapes the directory.
    let traversal = br#"{"weight_map":{"t":"../t.safetensors"}}"#;
    assert!(matches!(
        SafetensorsIndex::parse(traversal),
        Err(V41WeightsError::InvalidIndex(_))
    ));
}

#[test]
fn the_pin_refuses_other_checkpoints() {
    let dir = tempfile::tempdir().unwrap();
    write_checkpoint(dir.path(), None);
    assert!(matches!(
        DeepSeekV41Checkpoint::open(dir.path(), CheckpointOpenOptions::pinned_flash().unwrap()),
        Err(V41WeightsError::PinMismatch(_))
    ));
}

// ---------------------------------------------------------------------------
// Oracle parity harness
// ---------------------------------------------------------------------------

#[test]
fn parity_manifest_matches_and_reports_mismatches() {
    let (_dir, checkpoint, _, _) = open_synthetic();
    let block = checkpoint
        .fp8_weight_block("layers.0.attn.wkv.weight", 1, 2)
        .unwrap();
    let row = checkpoint
        .dequantize_row("layers.0.ffn.experts.0.w1.weight", 2)
        .unwrap();
    let engram = checkpoint.engram_row(1, 3).unwrap();
    let bits = |values: &[f32]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    let mut manifest = serde_json::json!({
        "schema": DEQUANT_PARITY_SCHEMA,
        "revision": DEEPSEEK_V41_FLASH_REVISION,
        "cases": [
            {"kind": "fp8_block", "tensor": "layers.0.attn.wkv.weight",
             "block_row": 1, "block_col": 2, "f32_bits": bits(&block.values)},
            {"kind": "row", "tensor": "layers.0.ffn.experts.0.w1.weight",
             "row": 2, "f32_bits": bits(&row)},
            {"kind": "engram_row", "layer": 1, "row": 3, "f32_bits": bits(&engram)},
        ]
    });
    let parsed = DequantParityManifest::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    let results = parsed.check(&checkpoint).unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|result| result.mismatches == 0));
    assert_eq!(results[0].values, 8 * 6);

    // One flipped value is reported, not an error.
    manifest["cases"][2]["f32_bits"][5] = serde_json::json!(1.0_f32.to_bits() ^ 0x8000_0000);
    let parsed = DequantParityManifest::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    let results = parsed.check(&checkpoint).unwrap();
    assert_eq!(results[2].mismatches, 1);
    assert_eq!(results[2].first_mismatch.unwrap().0, 5);

    manifest["schema"] = "other".into();
    assert!(DequantParityManifest::parse(&serde_json::to_vec(&manifest).unwrap()).is_err());
}

#[test]
fn weights_check_opens_every_tensor_and_runs_the_oracle() {
    let (dir, checkpoint, _, _) = open_synthetic();
    let row = checkpoint.engram_row(1, 0).unwrap();
    let manifest = serde_json::json!({
        "schema": DEQUANT_PARITY_SCHEMA,
        "revision": "synthetic",
        "cases": [{"kind": "engram_row", "layer": 1, "row": 0,
                   "f32_bits": row.iter().map(|v| v.to_bits()).collect::<Vec<_>>()}],
    });
    let oracle = dir.path().join("oracle.json");
    std::fs::write(&oracle, serde_json::to_vec(&manifest).unwrap()).unwrap();
    let options = WeightsCheckOptions {
        oracle: Some(oracle),
        verify_sha256: true,
        ranges: vec![(0, 1)],
        ..WeightsCheckOptions::default()
    };
    let outcome = check_opened_checkpoint(&checkpoint, &options).unwrap();
    assert_eq!(outcome.tensors_opened, 8);
    // Two rows each for the FP8, BF16, FP4, and Engram weights; one for F32.
    assert_eq!(outcome.rows_dequantized, 9);
    assert_eq!(outcome.nan_values, 0);
    assert!(outcome.verified_identity.is_some());
    assert_eq!(outcome.parity.as_ref().unwrap()[0].mismatches, 0);
    assert_eq!(outcome.report.ranges[0].tensors, 8);
    assert!(outcome.passed());

    // A NaN scale (0xFF) in the last row of the FP4 expert fails the check.
    let entry = checkpoint
        .entry("layers.0.ffn.experts.0.w1.scale")
        .unwrap()
        .clone();
    let path = dir.path().join("b.safetensors");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[(entry.file_offset + entry.byte_len - 1) as usize] = 0xFF;
    std::fs::write(&path, bytes).unwrap();
    let checkpoint = DeepSeekV41Checkpoint::open(dir.path(), unpinned()).unwrap();
    let outcome = check_opened_checkpoint(&checkpoint, &WeightsCheckOptions::default()).unwrap();
    assert_eq!(outcome.nan_values, 32);
    assert!(!outcome.passed());
}

/// Runs against the full checkpoint. Set `DEEPSEEK_V41_DIR` to the checkpoint
/// directory and, after #12371, `DEEPSEEK_V41_DEQUANT_ORACLE` to the parity
/// manifest. The example `deepseek_v41_weights_check` runs the same check and
/// prints the report.
#[test]
#[ignore = "needs the 510 GB checkpoint; set DEEPSEEK_V41_DIR"]
fn full_checkpoint_opens_and_matches_the_oracle() {
    let dir = std::env::var("DEEPSEEK_V41_DIR").expect("set DEEPSEEK_V41_DIR");
    let options = WeightsCheckOptions {
        oracle: std::env::var("DEEPSEEK_V41_DEQUANT_ORACLE")
            .ok()
            .map(PathBuf::from),
        ..WeightsCheckOptions::default()
    };
    let outcome = check_checkpoint(Path::new(&dir), &options).unwrap();
    assert_eq!(outcome.tensors_opened, 96_085);
    assert_eq!(outcome.report.total_bytes, DEEPSEEK_V41_FLASH_TOTAL_BYTES);
    assert!(outcome.passed(), "{:?}", outcome.failures);
}

// ---------------------------------------------------------------------------
// Real index and headers
// ---------------------------------------------------------------------------

fn real_index() -> SafetensorsIndex {
    let bytes = gunzip("model.safetensors.index.json.gz");
    SafetensorsIndex::parse(&bytes).unwrap()
}

#[test]
fn real_index_matches_the_pin() {
    let index = real_index();
    let pin = CheckpointPin::deepseek_v41_flash().unwrap();
    assert_eq!(index.sha256, DEEPSEEK_V41_FLASH_INDEX_SHA256);
    assert_eq!(index.sha256, pin.index_sha256);
    assert_eq!(pin.revision, DEEPSEEK_V41_FLASH_REVISION);
    assert_eq!(pin.repo, DEEPSEEK_V41_FLASH_REPO);
    assert_eq!(index.total_size, Some(DEEPSEEK_V41_FLASH_TOTAL_BYTES));
    assert_eq!(index.weight_map.len(), 96_085);
    let shards = index.shard_names();
    assert_eq!(shards.len(), 48);
    assert!(
        shards
            .iter()
            .copied()
            .eq(pin.shards.keys().map(String::as_str))
    );
    // Pinned shard sizes minus their headers add up to the index's total.
    assert_eq!(
        pin.shards.values().map(|shard| shard.size).sum::<u64>(),
        510_286_023_000 + 10_684_928 + 48 * 8,
        "pin sizes are tensor bytes plus every header"
    );
}

#[test]
fn real_index_matches_the_model_map() {
    let index = real_index();
    let map = V41ModelMap::new(&V41WeightConfig::flash());
    assert_eq!(map.len(), 96_085);
    map.check_names(index.weight_map.keys().map(String::as_str))
        .unwrap();

    // A missing or extra name is an error, not a skip.
    let mut names: Vec<&str> = index.weight_map.keys().map(String::as_str).collect();
    names.retain(|name| *name != "layers.20.attn.compressor.wkv.weight");
    match map.check_names(names.iter().copied()) {
        Err(V41WeightsError::ModelMap {
            missing_count: 1,
            first_missing: Some(name),
            extra_count: 0,
            ..
        }) => assert_eq!(name, "layers.20.attn.compressor.wkv.weight"),
        other => panic!("unexpected {other:?}"),
    }
    names.push("layers.20.attn.compressor.wkv.weight");
    names.push("layers.3.attn.compressor.wgate.weight");
    assert!(matches!(
        map.check_names(names),
        Err(V41WeightsError::ModelMap { extra_count: 1, .. })
    ));

    let role = map.role("layers.7.ffn.experts.383.w2.weight").unwrap();
    assert_eq!(role.scope, V41Scope::Layer(7));
    assert_eq!(role.component, V41Component::RoutedExpert);
    assert_eq!(role.expert, Some(383));
    assert_eq!(role.dtype, SafetensorsDtype::I8);
    assert_eq!(
        map.role("mtp.2.markov_head.head.weight").unwrap().scope,
        V41Scope::Mtp(2)
    );
    assert_eq!(
        map.role("layers.14.engram.embed.weight").unwrap().component,
        V41Component::EngramTable
    );
}

fn real_header(shard: &str) -> SafetensorsShardHeader {
    let bytes = gunzip(&format!("{shard}.header.gz"));
    let pin = CheckpointPin::deepseek_v41_flash().unwrap();
    SafetensorsShardHeader::parse(shard, &bytes, Some(pin.shards[shard].size)).unwrap()
}

const SHARD_14: &str = "model-00017-of-00048.safetensors";
const ENGRAM_1: &str = "model-00047-of-00048.safetensors";
const ENGRAM_14: &str = "model-00048-of-00048.safetensors";

#[test]
fn real_headers_match_the_index_and_roles() {
    let index = real_index();
    let map = V41ModelMap::new(&V41WeightConfig::flash());
    let pin = CheckpointPin::deepseek_v41_flash().unwrap();
    for shard in [SHARD_14, ENGRAM_1, ENGRAM_14] {
        let header = real_header(shard);
        let indexed: BTreeSet<&str> = index
            .weight_map
            .iter()
            .filter(|(_, file)| file.as_str() == shard)
            .map(|(name, _)| name.as_str())
            .collect();
        assert!(header.tensors.keys().map(String::as_str).eq(indexed));
        for (name, entry) in &header.tensors {
            assert_eq!(map.role(name).unwrap().dtype, entry.dtype, "{name}");
        }
        // The last tensor ends exactly at the pinned file end.
        let end = header
            .tensors
            .values()
            .map(|entry| entry.file_range().end)
            .max()
            .unwrap();
        assert_eq!(end, pin.shards[shard].size, "{shard}");
    }
}

#[test]
fn real_shapes_match_the_issue_table() {
    let header = real_header(SHARD_14);
    assert_eq!(header.tensors.len(), 2_342);
    let shape = |name: &str| header.tensors[name].shape.clone();
    let dtype = |name: &str| header.tensors[name].dtype;
    assert_eq!(shape("layers.14.attn.wkv.weight"), vec![512, 5120]);
    assert_eq!(dtype("layers.14.attn.wkv.weight"), SafetensorsDtype::F8E4M3);
    assert_eq!(shape("layers.14.attn.wkv.scale"), vec![16, 160]);
    assert_eq!(shape("layers.14.ffn.experts.5.w1.weight"), vec![2304, 2560]);
    assert_eq!(
        dtype("layers.14.ffn.experts.5.w1.weight"),
        SafetensorsDtype::I8
    );
    assert_eq!(shape("layers.14.ffn.experts.5.w1.scale"), vec![2304, 160]);
    assert_eq!(shape("layers.14.ffn.experts.5.w2.weight"), vec![5120, 1152]);
    assert_eq!(shape("layers.14.ffn.experts.5.w2.scale"), vec![5120, 72]);
    assert_eq!(shape("layers.14.hc_attn_fn"), vec![24, 20480]);
    assert_eq!(dtype("layers.14.ffn.gate.bias"), SafetensorsDtype::F32);

    let table = real_header(ENGRAM_1);
    let embed = &table.tensors["layers.1.engram.embed.weight"];
    assert_eq!(embed.shape, vec![384_006_168, 256]);
    assert_eq!(embed.byte_len, 98_305_579_008);
    let scale = &table.tensors["layers.1.engram.embed.scale"];
    assert_eq!(scale.shape, vec![384_006_168, 8]);
    assert_eq!(scale.byte_len, 3_072_049_344);
}

/// Opens the three real shard headers as a partial, headers-only checkpoint
/// (the index cut down to those shards) and checks the block formats and the
/// layer-14 totals against the retained report.
#[test]
fn real_headers_give_layer_14_and_engram_totals() {
    let dir = tempfile::tempdir().unwrap();
    let full = real_index();
    let keep = [SHARD_14, ENGRAM_1, ENGRAM_14];
    let map: serde_json::Map<String, serde_json::Value> = full
        .weight_map
        .iter()
        .filter(|(_, shard)| keep.contains(&shard.as_str()))
        .map(|(name, shard)| (name.clone(), shard.clone().into()))
        .collect();
    std::fs::write(
        dir.path().join(SAFETENSORS_INDEX_FILE),
        serde_json::to_vec(&serde_json::json!({ "weight_map": map })).unwrap(),
    )
    .unwrap();
    for shard in keep {
        std::fs::write(
            dir.path().join(shard),
            gunzip(&format!("{shard}.header.gz")),
        )
        .unwrap();
    }
    let checkpoint = DeepSeekV41Checkpoint::open(
        dir.path(),
        CheckpointOpenOptions {
            headers_only: true,
            pin: None,
            check_model_map: false,
        },
    )
    .unwrap();
    for (weight, format) in [
        ("layers.14.attn.wkv.weight", V41BlockFormat::Fp8Block32x32),
        ("layers.14.attn.wo_a.weight", V41BlockFormat::Fp8Block32x32),
        (
            "layers.14.attn.indexer.wq_b.weight",
            V41BlockFormat::Fp8Block32x32,
        ),
        (
            "layers.14.ffn.shared_experts.w2.weight",
            V41BlockFormat::Fp8Block32x32,
        ),
        (
            "layers.14.ffn.experts.0.w1.weight",
            V41BlockFormat::Fp4E2M1Row32,
        ),
        (
            "layers.14.ffn.experts.383.w2.weight",
            V41BlockFormat::Fp4E2M1Row32,
        ),
        ("layers.14.engram.embed.weight", V41BlockFormat::Fp8Row32),
        ("layers.14.engram.wkv.weight", V41BlockFormat::Fp8Block32x32),
        ("layers.1.engram.embed.weight", V41BlockFormat::Fp8Row32),
        ("layers.14.ffn.gate.weight", V41BlockFormat::Bf16),
    ] {
        assert_eq!(checkpoint.block_format(weight).unwrap(), format, "{weight}");
    }

    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(FIXTURES).join("weights_report_headers.json")).unwrap(),
    )
    .unwrap();
    let layer14 = checkpoint
        .select(&V41StageSelection::layers(14..15))
        .unwrap();
    let report = &report["report"];
    assert_eq!(report["layers"][14]["bytes"], layer14.total_bytes);
    assert_eq!(report["layers"][14]["tensors"], layer14.tensors.len());
    assert_eq!(
        report["layers"][14]["engram_table_bytes"],
        layer14.engram_table_bytes
    );
    let engram1 = checkpoint.select(&V41StageSelection::layers(1..2)).unwrap();
    assert_eq!(engram1.engram_table_bytes, 98_305_579_008 + 3_072_049_344);
    assert_eq!(
        report["layers"][1]["engram_table_bytes"],
        engram1.engram_table_bytes
    );
}

/// The retained report was made by the example in headers-only mode over all
/// 48 real headers. Its totals must add up.
#[test]
fn retained_report_totals_add_up() {
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(FIXTURES).join("weights_report_headers.json")).unwrap(),
    )
    .unwrap();
    let report = &report["report"];
    assert_eq!(report["schema"], WEIGHTS_REPORT_SCHEMA);
    assert_eq!(report["mode"], "headers_only");
    assert_eq!(report["identity"]["revision"], DEEPSEEK_V41_FLASH_REVISION);
    assert_eq!(report["tensor_count"], 96_085);
    assert_eq!(report["total_bytes"], DEEPSEEK_V41_FLASH_TOTAL_BYTES);
    let bytes = |value: &serde_json::Value| value["bytes"].as_u64().unwrap();
    let layers = report["layers"].as_array().unwrap();
    assert_eq!(layers.len(), 40);
    let layer_sum: u64 = layers.iter().map(bytes).sum();
    let part_sum: u64 = report["parts"]
        .as_object()
        .unwrap()
        .values()
        .map(bytes)
        .sum();
    assert_eq!(layer_sum + part_sum, DEEPSEEK_V41_FLASH_TOTAL_BYTES);
    let tensors: u64 = layers
        .iter()
        .chain(report["parts"].as_object().unwrap().values())
        .map(|value| value["tensors"].as_u64().unwrap())
        .sum();
    assert_eq!(tensors, 96_085);
    for range in report["ranges"].as_array().unwrap() {
        let label = range["label"].as_str().unwrap();
        let (first, last) = label.trim_start_matches("layers ").split_once('-').unwrap();
        let (first, last): (usize, usize) = (first.parse().unwrap(), last.parse().unwrap());
        let sum: u64 = layers[first..=last].iter().map(bytes).sum();
        assert_eq!(bytes(range), sum, "{label}");
    }
}

/// Recomputes the per-range tensor counts from the real index names alone, so
/// the retained report can't drift from the model map.
#[test]
fn retained_report_counts_match_the_index() {
    let index = real_index();
    let map = V41ModelMap::new(&V41WeightConfig::flash());
    let report: serde_json::Value = serde_json::from_slice(
        &std::fs::read(Path::new(FIXTURES).join("weights_report_headers.json")).unwrap(),
    )
    .unwrap();
    for (i, &(first, last)) in V41_REPORT_RANGES.iter().enumerate() {
        let selection = V41StageSelection::layers(first..last + 1);
        let count = index
            .weight_map
            .keys()
            .filter(|name| selection.includes(map.role(name).unwrap()))
            .count();
        assert_eq!(report["report"]["ranges"][i]["tensors"], count);
    }
}
