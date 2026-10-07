//! Shared reference decoders for GGML block formats used across loaders and backends.

use std::{error::Error, fmt};

use half::f16;

const GGML_SUPER_BLOCK_ELEMENTS: usize = 256;
const Q3_K_BLOCK_BYTES: usize = 110;
const IQ3_S_BLOCK_BYTES: usize = 110;
const IQ4_XS_BLOCK_BYTES: usize = 136;
const IQ4_NL_VALUES: [i8; 16] = [
    -127, -104, -83, -65, -49, -35, -22, -10, 1, 13, 25, 38, 53, 69, 89, 113,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GgmlBlockDecodeError {
    pub block_type: &'static str,
    pub expected_bytes: usize,
    pub actual_bytes: usize,
}

impl fmt::Display for GgmlBlockDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "GGML {} block requires {} bytes, got {}",
            self.block_type, self.expected_bytes, self.actual_bytes
        )
    }
}

impl Error for GgmlBlockDecodeError {}

/// Decodes one GGML Q3_K super-block into 256 logical values.
pub fn decode_q3_k_block(
    bytes: &[u8],
) -> Result<[f32; GGML_SUPER_BLOCK_ELEMENTS], GgmlBlockDecodeError> {
    require_block_len("Q3_K", Q3_K_BLOCK_BYTES, bytes)?;
    let high_masks = &bytes[..32];
    let quants = &bytes[32..96];
    let packed_scales = &bytes[96..108];
    let scale = decode_f16(bytes[108], bytes[109]);
    let scales = decode_q3_k_scales(packed_scales);
    let mut output = [0.0_f32; GGML_SUPER_BLOCK_ELEMENTS];
    let mut output_index = 0usize;
    let mut scale_index = 0usize;
    let mut high_mask = 1u8;
    for half_index in 0..2 {
        let quant_base = half_index * 32;
        for shift in [0, 2, 4, 6] {
            for quant_offset in [0usize, 16] {
                let local_scale = scale * f32::from(scales[scale_index] - 32);
                scale_index += 1;
                for index in 0..16 {
                    let low =
                        i8::try_from((quants[quant_base + quant_offset + index] >> shift) & 3)
                            .expect("two-bit value fits i8");
                    let high = if high_masks[quant_offset + index] & high_mask == 0 {
                        4
                    } else {
                        0
                    };
                    output[output_index] = local_scale * f32::from(low - high);
                    output_index += 1;
                }
            }
            high_mask <<= 1;
        }
    }
    Ok(output)
}

/// Decodes one GGML IQ3_S super-block into 256 logical values.
pub fn decode_iq3_s_block(
    bytes: &[u8],
) -> Result<[f32; GGML_SUPER_BLOCK_ELEMENTS], GgmlBlockDecodeError> {
    require_block_len("IQ3_S", IQ3_S_BLOCK_BYTES, bytes)?;
    let scale = decode_f16(bytes[0], bytes[1]);
    let quants = &bytes[2..66];
    let high_bits = &bytes[66..74];
    let signs = &bytes[74..106];
    let scales = &bytes[106..110];
    let mut output = [0.0_f32; GGML_SUPER_BLOCK_ELEMENTS];
    let mut output_index = 0usize;
    let mut quant_index = 0usize;
    let mut sign_index = 0usize;
    let mut high_index = 0usize;
    for scale_byte in scales {
        for local_scale in [scale_byte & 0x0f, scale_byte >> 4] {
            let local_scale = scale * f32::from(1 + 2 * local_scale);
            let high = high_bits[high_index];
            high_index += 1;
            for lane in 0..4 {
                let first_index = usize::from(quants[quant_index])
                    | usize::from((u16::from(high) << (8 - 2 * lane)) & 0x100);
                let second_index = usize::from(quants[quant_index + 1])
                    | usize::from((u16::from(high) << (7 - 2 * lane)) & 0x100);
                quant_index += 2;
                let first = IQ3_S_GRID[first_index].to_le_bytes();
                let second = IQ3_S_GRID[second_index].to_le_bytes();
                let lane_signs = signs[sign_index];
                sign_index += 1;
                for (index, value) in first.into_iter().chain(second).enumerate() {
                    let sign = if lane_signs & (1 << index) == 0 {
                        1.0
                    } else {
                        -1.0
                    };
                    output[output_index] = local_scale * f32::from(value) * sign;
                    output_index += 1;
                }
            }
        }
    }
    Ok(output)
}

/// Decodes one GGML IQ4_XS super-block into 256 logical values.
pub fn decode_iq4_xs_block(
    bytes: &[u8],
) -> Result<[f32; GGML_SUPER_BLOCK_ELEMENTS], GgmlBlockDecodeError> {
    require_block_len("IQ4_XS", IQ4_XS_BLOCK_BYTES, bytes)?;
    let scale = decode_f16(bytes[0], bytes[1]);
    let scale_high = u16::from_le_bytes([bytes[2], bytes[3]]);
    let scale_low = &bytes[4..8];
    let quants = &bytes[8..136];
    let mut output = [0.0_f32; GGML_SUPER_BLOCK_ELEMENTS];
    for block in 0..8 {
        let packed_scale = (scale_low[block / 2] >> (4 * (block % 2))) & 0x0f;
        let packed_scale = packed_scale | ((((scale_high >> (2 * block)) & 3) as u8) << 4);
        let local_scale = scale * f32::from(i16::from(packed_scale) - 32);
        let quant_base = block * 16;
        let output_base = block * 32;
        for index in 0..16 {
            let packed = quants[quant_base + index];
            output[output_base + index] =
                local_scale * f32::from(IQ4_NL_VALUES[usize::from(packed & 0x0f)]);
            output[output_base + index + 16] =
                local_scale * f32::from(IQ4_NL_VALUES[usize::from(packed >> 4)]);
        }
    }
    Ok(output)
}

fn require_block_len(
    block_type: &'static str,
    expected_bytes: usize,
    bytes: &[u8],
) -> Result<(), GgmlBlockDecodeError> {
    if bytes.len() == expected_bytes {
        return Ok(());
    }
    Err(GgmlBlockDecodeError {
        block_type,
        expected_bytes,
        actual_bytes: bytes.len(),
    })
}

fn decode_f16(low: u8, high: u8) -> f32 {
    f16::from_bits(u16::from_le_bytes([low, high])).to_f32()
}

fn decode_q3_k_scales(packed: &[u8]) -> [i8; 16] {
    let mut aux = [0_u32; 4];
    for (index, chunk) in packed.chunks_exact(4).enumerate() {
        aux[index] = u32::from_le_bytes(chunk.try_into().expect("four scale bytes"));
    }
    let mask_two = 0x0303_0303_u32;
    let mask_four = 0x0f0f_0f0f_u32;
    let upper = aux[2];
    aux[2] = ((aux[0] >> 4) & mask_four) | (((upper >> 4) & mask_two) << 4);
    aux[3] = ((aux[1] >> 4) & mask_four) | (((upper >> 6) & mask_two) << 4);
    aux[0] = (aux[0] & mask_four) | (((upper >> 0) & mask_two) << 4);
    aux[1] = (aux[1] & mask_four) | (((upper >> 2) & mask_two) << 4);
    let mut scales = [0_i8; 16];
    for (destination, source) in scales
        .iter_mut()
        .zip(aux.into_iter().flat_map(u32::to_le_bytes))
    {
        *destination = i8::from_le_bytes([source]);
    }
    scales
}

// The 512-entry IQ3_S codebook is generated from ggml-common.h at the audited
// llama.cpp revision recorded in docs/qwen38/LLAMA_CPP_CODE_AUDIT.md (MIT).
const IQ3_S_GRID: [u32; 512] = [
    0x01010101, 0x01010103, 0x01010105, 0x0101010b, 0x0101010f, 0x01010301, 0x01010303, 0x01010305,
    0x01010309, 0x0101030d, 0x01010501, 0x01010503, 0x0101050b, 0x01010707, 0x01010901, 0x01010905,
    0x0101090b, 0x0101090f, 0x01010b03, 0x01010b07, 0x01010d01, 0x01010d05, 0x01010f03, 0x01010f09,
    0x01010f0f, 0x01030101, 0x01030103, 0x01030105, 0x01030109, 0x01030301, 0x01030303, 0x0103030b,
    0x01030501, 0x01030507, 0x0103050f, 0x01030703, 0x0103070b, 0x01030909, 0x01030d03, 0x01030d0b,
    0x01030f05, 0x01050101, 0x01050103, 0x0105010b, 0x0105010f, 0x01050301, 0x01050307, 0x0105030d,
    0x01050503, 0x0105050b, 0x01050701, 0x01050709, 0x01050905, 0x0105090b, 0x0105090f, 0x01050b03,
    0x01050b07, 0x01050f01, 0x01050f07, 0x01070107, 0x01070303, 0x0107030b, 0x01070501, 0x01070505,
    0x01070703, 0x01070707, 0x0107070d, 0x01070909, 0x01070b01, 0x01070b05, 0x01070d0f, 0x01070f03,
    0x01070f0b, 0x01090101, 0x01090307, 0x0109030f, 0x01090503, 0x01090509, 0x01090705, 0x01090901,
    0x01090907, 0x01090b03, 0x01090f01, 0x010b0105, 0x010b0109, 0x010b0501, 0x010b0505, 0x010b050d,
    0x010b0707, 0x010b0903, 0x010b090b, 0x010b090f, 0x010b0d0d, 0x010b0f07, 0x010d010d, 0x010d0303,
    0x010d0307, 0x010d0703, 0x010d0b05, 0x010d0f03, 0x010f0101, 0x010f0105, 0x010f0109, 0x010f0501,
    0x010f0505, 0x010f050d, 0x010f0707, 0x010f0b01, 0x010f0b09, 0x03010101, 0x03010103, 0x03010105,
    0x03010109, 0x03010301, 0x03010303, 0x03010307, 0x0301030b, 0x0301030f, 0x03010501, 0x03010505,
    0x03010703, 0x03010709, 0x0301070d, 0x03010b09, 0x03010b0d, 0x03010d03, 0x03010f05, 0x03030101,
    0x03030103, 0x03030107, 0x0303010d, 0x03030301, 0x03030309, 0x03030503, 0x03030701, 0x03030707,
    0x03030903, 0x03030b01, 0x03030b05, 0x03030f01, 0x03030f0d, 0x03050101, 0x03050305, 0x0305030b,
    0x0305030f, 0x03050501, 0x03050509, 0x03050705, 0x03050901, 0x03050907, 0x03050b0b, 0x03050d01,
    0x03050f05, 0x03070103, 0x03070109, 0x0307010f, 0x03070301, 0x03070307, 0x03070503, 0x0307050f,
    0x03070701, 0x03070709, 0x03070903, 0x03070d05, 0x03070f01, 0x03090107, 0x0309010b, 0x03090305,
    0x03090309, 0x03090703, 0x03090707, 0x03090905, 0x0309090d, 0x03090b01, 0x03090b09, 0x030b0103,
    0x030b0301, 0x030b0307, 0x030b0503, 0x030b0701, 0x030b0705, 0x030b0b03, 0x030d0501, 0x030d0509,
    0x030d050f, 0x030d0909, 0x030d090d, 0x030f0103, 0x030f0107, 0x030f0301, 0x030f0305, 0x030f0503,
    0x030f070b, 0x030f0903, 0x030f0d05, 0x030f0f01, 0x05010101, 0x05010103, 0x05010107, 0x0501010b,
    0x0501010f, 0x05010301, 0x05010305, 0x05010309, 0x0501030d, 0x05010503, 0x05010507, 0x0501050f,
    0x05010701, 0x05010705, 0x05010903, 0x05010907, 0x0501090b, 0x05010b01, 0x05010b05, 0x05010d0f,
    0x05010f01, 0x05010f07, 0x05010f0b, 0x05030101, 0x05030105, 0x05030301, 0x05030307, 0x0503030f,
    0x05030505, 0x0503050b, 0x05030703, 0x05030709, 0x05030905, 0x05030b03, 0x05050103, 0x05050109,
    0x0505010f, 0x05050503, 0x05050507, 0x05050701, 0x0505070f, 0x05050903, 0x05050b07, 0x05050b0f,
    0x05050f03, 0x05050f09, 0x05070101, 0x05070105, 0x0507010b, 0x05070303, 0x05070505, 0x05070509,
    0x05070703, 0x05070707, 0x05070905, 0x05070b01, 0x05070d0d, 0x05090103, 0x0509010f, 0x05090501,
    0x05090507, 0x05090705, 0x0509070b, 0x05090903, 0x05090f05, 0x05090f0b, 0x050b0109, 0x050b0303,
    0x050b0505, 0x050b070f, 0x050b0901, 0x050b0b07, 0x050b0f01, 0x050d0101, 0x050d0105, 0x050d010f,
    0x050d0503, 0x050d0b0b, 0x050d0d03, 0x050f010b, 0x050f0303, 0x050f050d, 0x050f0701, 0x050f0907,
    0x050f0b01, 0x07010105, 0x07010303, 0x07010307, 0x0701030b, 0x0701030f, 0x07010505, 0x07010703,
    0x07010707, 0x0701070b, 0x07010905, 0x07010909, 0x0701090f, 0x07010b03, 0x07010d07, 0x07010f03,
    0x07030103, 0x07030107, 0x0703010b, 0x07030309, 0x07030503, 0x07030507, 0x07030901, 0x07030d01,
    0x07030f05, 0x07030f0d, 0x07050101, 0x07050305, 0x07050501, 0x07050705, 0x07050709, 0x07050b01,
    0x07070103, 0x07070301, 0x07070309, 0x07070503, 0x07070507, 0x0707050f, 0x07070701, 0x07070903,
    0x07070907, 0x0707090f, 0x07070b0b, 0x07070f07, 0x07090107, 0x07090303, 0x0709030d, 0x07090505,
    0x07090703, 0x07090b05, 0x07090d01, 0x07090d09, 0x070b0103, 0x070b0301, 0x070b0305, 0x070b050b,
    0x070b0705, 0x070b0909, 0x070b0b0d, 0x070b0f07, 0x070d030d, 0x070d0903, 0x070f0103, 0x070f0107,
    0x070f0501, 0x070f0505, 0x070f070b, 0x09010101, 0x09010109, 0x09010305, 0x09010501, 0x09010509,
    0x0901050f, 0x09010705, 0x09010903, 0x09010b01, 0x09010f01, 0x09030105, 0x0903010f, 0x09030303,
    0x09030307, 0x09030505, 0x09030701, 0x0903070b, 0x09030907, 0x09030b03, 0x09030b0b, 0x09050103,
    0x09050107, 0x09050301, 0x0905030b, 0x09050503, 0x09050707, 0x09050901, 0x09050b0f, 0x09050d05,
    0x09050f01, 0x09070109, 0x09070303, 0x09070307, 0x09070501, 0x09070505, 0x09070703, 0x0907070b,
    0x09090101, 0x09090105, 0x09090509, 0x0909070f, 0x09090901, 0x09090f03, 0x090b010b, 0x090b010f,
    0x090b0503, 0x090b0d05, 0x090d0307, 0x090d0709, 0x090d0d01, 0x090f0301, 0x090f030b, 0x090f0701,
    0x090f0907, 0x090f0b03, 0x0b010105, 0x0b010301, 0x0b010309, 0x0b010505, 0x0b010901, 0x0b010909,
    0x0b01090f, 0x0b010b05, 0x0b010d0d, 0x0b010f09, 0x0b030103, 0x0b030107, 0x0b03010b, 0x0b030305,
    0x0b030503, 0x0b030705, 0x0b030f05, 0x0b050101, 0x0b050303, 0x0b050507, 0x0b050701, 0x0b05070d,
    0x0b050b07, 0x0b070105, 0x0b07010f, 0x0b070301, 0x0b07050f, 0x0b070909, 0x0b070b03, 0x0b070d0b,
    0x0b070f07, 0x0b090103, 0x0b090109, 0x0b090501, 0x0b090705, 0x0b09090d, 0x0b0b0305, 0x0b0b050d,
    0x0b0b0b03, 0x0b0b0b07, 0x0b0d0905, 0x0b0f0105, 0x0b0f0109, 0x0b0f0505, 0x0d010303, 0x0d010307,
    0x0d01030b, 0x0d010703, 0x0d010707, 0x0d010d01, 0x0d030101, 0x0d030501, 0x0d03050f, 0x0d030d09,
    0x0d050305, 0x0d050709, 0x0d050905, 0x0d050b0b, 0x0d050d05, 0x0d050f01, 0x0d070101, 0x0d070309,
    0x0d070503, 0x0d070901, 0x0d09050b, 0x0d090907, 0x0d090d05, 0x0d0b0101, 0x0d0b0107, 0x0d0b0709,
    0x0d0b0d01, 0x0d0d010b, 0x0d0d0901, 0x0d0f0303, 0x0d0f0307, 0x0f010101, 0x0f010109, 0x0f01010f,
    0x0f010501, 0x0f010505, 0x0f01070d, 0x0f010901, 0x0f010b09, 0x0f010d05, 0x0f030105, 0x0f030303,
    0x0f030509, 0x0f030907, 0x0f03090b, 0x0f050103, 0x0f050109, 0x0f050301, 0x0f05030d, 0x0f050503,
    0x0f050701, 0x0f050b03, 0x0f070105, 0x0f070705, 0x0f07070b, 0x0f070b07, 0x0f090103, 0x0f09010b,
    0x0f090307, 0x0f090501, 0x0f090b01, 0x0f0b0505, 0x0f0b0905, 0x0f0d0105, 0x0f0d0703, 0x0f0f0101,
];

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    const REFERENCE_VECTORS: &str =
        include_str!("../../../fixtures/qwen38/qwen38_ggml_quant_reference_vectors_v1.json");

    #[derive(Deserialize)]
    struct ReferenceReport {
        vectors: Vec<ReferenceVector>,
    }

    #[derive(Deserialize)]
    struct ReferenceVector {
        block_type: String,
        block_bytes_hex: String,
        block_sha256: String,
        decoded_f32_le_sha256: String,
        decoded_values: Vec<f32>,
    }

    #[test]
    fn ggml_super_block_decoders_reject_wrong_byte_lengths() {
        assert!(decode_q3_k_block(&[0; Q3_K_BLOCK_BYTES - 1]).is_err());
        assert!(decode_iq3_s_block(&[0; IQ3_S_BLOCK_BYTES - 1]).is_err());
        assert!(decode_iq4_xs_block(&[0; IQ4_XS_BLOCK_BYTES - 1]).is_err());
    }

    #[test]
    fn ggml_super_block_decoders_match_pinned_llama_cpp_vectors() {
        let report = serde_json::from_str::<ReferenceReport>(REFERENCE_VECTORS)
            .expect("reference report must parse");
        assert_eq!(report.vectors.len(), 3);
        for vector in report.vectors {
            let bytes = hex::decode(&vector.block_bytes_hex).expect("block hex must decode");
            assert_eq!(hex::encode(Sha256::digest(&bytes)), vector.block_sha256);
            let decoded = match vector.block_type.as_str() {
                "Q3_K" => decode_q3_k_block(&bytes),
                "IQ3_S" => decode_iq3_s_block(&bytes),
                "IQ4_XS" => decode_iq4_xs_block(&bytes),
                other => panic!("unexpected reference block type {other}"),
            }
            .expect("reference block must decode");
            assert_eq!(decoded.as_slice(), vector.decoded_values.as_slice());
            let decoded_bytes = decoded
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(
                hex::encode(Sha256::digest(&decoded_bytes)),
                vector.decoded_f32_le_sha256
            );
        }
    }
}
