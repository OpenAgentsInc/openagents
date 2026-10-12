//! Checked CUDA memory estimates. These checks do not reserve device memory.

use serde::Serialize;

pub(super) const DEFAULT_TOKENS: usize = 16_384;
pub(super) const DEFAULT_CHUNK: usize = 2_048;
pub(super) const HEAD_RESERVE_BYTES: u64 = 64 << 20;
const RUNTIME_MARGIN_BYTES: u64 = 128 << 20;
const SCORE_BUDGET_BYTES: u64 = 512 << 20;
const MEMORY_REFUSAL_PREFIX: &str = "not enough CUDA memory: ";

pub(super) fn is_memory_refusal(error: &str) -> bool {
    error.starts_with(MEMORY_REFUSAL_PREFIX)
}

#[derive(Clone, Copy)]
pub(super) enum MatrixFormat {
    F32,
    Q8_0,
    Q4K,
}

#[derive(Default)]
pub(super) struct Inventory {
    pub resident_bytes: u64,
    pub largest_f16_elements: u64,
}

impl Inventory {
    pub fn f32(&mut self, count: usize) -> Result<(), String> {
        self.resident_bytes = sum(&[self.resident_bytes, bytes(&[count.max(1)], 4)?])?;
        Ok(())
    }

    pub fn matrix(
        &mut self,
        rows: usize,
        columns: usize,
        format: MatrixFormat,
        fused_always: bool,
    ) -> Result<u64, String> {
        if rows == 0 || columns == 0 {
            return Err(String::from("CUDA matrix dimensions must be positive"));
        }
        let elements = bytes(&[rows, columns], 1)?;
        let (block, block_bytes) = match format {
            MatrixFormat::F32 => (1, 4),
            MatrixFormat::Q8_0 => (32, 34),
            MatrixFormat::Q4K => (256, 144),
        };
        if columns % block != 0 {
            return Err(String::from(
                "CUDA matrix row does not contain complete quantization blocks",
            ));
        }
        let resident = (elements / block as u64)
            .checked_mul(block_bytes)
            .ok_or("CUDA memory estimate overflow")?;
        self.resident_bytes = sum(&[self.resident_bytes, resident])?;
        let fused = fused_always
            && rows % 2 == 0
            && match format {
                MatrixFormat::F32 => false,
                MatrixFormat::Q8_0 => columns % 64 == 0,
                MatrixFormat::Q4K => columns % 256 == 0,
            };
        if !fused {
            self.largest_f16_elements = self.largest_f16_elements.max(elements);
        }
        Ok(resident)
    }
}

#[derive(Clone, Copy)]
pub(super) struct BackboneShape {
    pub hidden: usize,
    pub ffn: usize,
    pub conv_channels: usize,
    pub conv_kernel: usize,
    pub key_heads: usize,
    pub value_heads: usize,
    pub state: usize,
    pub heads: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub rotary: usize,
    pub width: usize,
    pub hybrid_layers: usize,
    pub attention_layers: usize,
    pub flash: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Estimate {
    pub resident_weight_bytes: u64,
    pub conversion_workspace_bytes: u64,
    pub backbone_scratch_bytes: u64,
    pub backbone_request_bytes: u64,
    pub head_reserve_bytes: u64,
    pub runtime_margin_bytes: u64,
    pub required_bytes: u64,
    pub max_tokens: usize,
    pub chunk_tokens: usize,
}

impl Estimate {
    pub fn new(
        inventory: &Inventory,
        shape: BackboneShape,
        tokens: usize,
        chunk: usize,
    ) -> Result<Self, String> {
        if tokens == 0 || chunk == 0 || shape.hidden == 0 || shape.width == 0 {
            return Err(String::from(
                "CUDA memory estimate requires positive token, chunk, and model dimensions",
            ));
        }
        let chunk = chunk.min(tokens);
        let q = bytes(&[shape.heads, shape.head_dim], 1)?;
        let v = bytes(&[shape.value_heads, shape.state], 1)?;
        let wide = sum(&[q, q])?
            .max(shape.ffn as u64)
            .max(shape.conv_channels as u64);
        let mid = q.max(v).max(shape.hidden as u64);
        let small = bytes(&[shape.kv_heads, shape.head_dim], 1)?
            .max(shape.value_heads as u64)
            .max(1);
        let qn = bytes(&[shape.key_heads, shape.state], 1)?.max(1);
        // x, big a/b, conv, mid a/b, small a/b/c/d, qn/kn,
        // key dot query, rotary table, final rows, normalized rows.
        let f32_per_token = sum(&[
            shape.hidden as u64,
            wide,
            wide,
            shape.conv_channels.max(1) as u64,
            mid,
            mid,
            small,
            small,
            small,
            small,
            qn,
            qn,
            shape.key_heads.max(1) as u64,
            shape.rotary.max(2) as u64,
            shape.hidden as u64,
            shape.hidden as u64,
        ])?;
        let f16_per_token = sum(&[wide.max(mid), q.max(1), wide.max(mid)])?;
        let backbone_scratch_bytes = sum(&[
            f32_per_token
                .checked_mul(chunk as u64)
                .and_then(|v| v.checked_mul(4))
                .ok_or("CUDA memory estimate overflow")?,
            f16_per_token
                .checked_mul(chunk as u64)
                .and_then(|v| v.checked_mul(2))
                .ok_or("CUDA memory estimate overflow")?,
        ])?;
        let group = shape.heads / shape.kv_heads.max(1);
        let score_one = bytes(&[chunk, tokens], 6)?;
        let heads_at_once = (SCORE_BUDGET_BYTES / score_one.max(1)).clamp(1, group.max(1) as u64);
        let flash = shape.flash
            && shape.head_dim == 256
            && shape.heads
                == shape
                    .kv_heads
                    .checked_mul(4)
                    .ok_or("CUDA memory estimate overflow")?;
        let scores = if flash {
            6
        } else {
            score_one
                .checked_mul(heads_at_once)
                .ok_or("CUDA memory estimate overflow")?
        };
        let backbone_request_bytes = sum(&[
            scores,
            bytes(&[tokens, shape.width], 4)?,
            bytes(
                &[
                    shape.hybrid_layers,
                    shape.conv_channels,
                    shape.conv_kernel.saturating_sub(1).max(1),
                ],
                4,
            )?,
            bytes(
                &[
                    shape.hybrid_layers,
                    shape.value_heads,
                    shape.state,
                    shape.state,
                ],
                4,
            )?,
            bytes(
                &[
                    shape.attention_layers,
                    tokens,
                    shape.kv_heads,
                    shape.head_dim,
                ],
                4,
            )?,
        ])?;
        let conversion_workspace_bytes = inventory
            .largest_f16_elements
            .max(1)
            .checked_mul(4)
            .ok_or("CUDA memory estimate overflow")?;
        let required_bytes = sum(&[
            inventory.resident_bytes,
            conversion_workspace_bytes,
            backbone_scratch_bytes,
            backbone_request_bytes,
            HEAD_RESERVE_BYTES,
            RUNTIME_MARGIN_BYTES,
        ])?;
        Ok(Self {
            resident_weight_bytes: inventory.resident_bytes,
            conversion_workspace_bytes,
            backbone_scratch_bytes,
            backbone_request_bytes,
            head_reserve_bytes: HEAD_RESERVE_BYTES,
            runtime_margin_bytes: RUNTIME_MARGIN_BYTES,
            required_bytes,
            max_tokens: tokens,
            chunk_tokens: chunk,
        })
    }

    pub fn admit(&self, free_bytes: u64, total_bytes: u64) -> Result<(), String> {
        if free_bytes <= total_bytes && self.required_bytes <= free_bytes {
            return Ok(());
        }
        Err(format!(
            "{MEMORY_REFUSAL_PREFIX}{}",
            serde_json::json!({
                "stage": "before_weight_upload", "estimate": self,
                "required_bytes": self.required_bytes, "free_bytes": free_bytes, "total_bytes": total_bytes,
                "memory_reserved": false,
            })
        ))
    }
}

pub(super) fn bytes(dimensions: &[usize], scalar_bytes: usize) -> Result<u64, String> {
    dimensions
        .iter()
        .try_fold(scalar_bytes as u64, |value, dimension| {
            value
                .checked_mul(*dimension as u64)
                .ok_or_else(|| String::from("CUDA memory estimate overflow"))
        })
}

pub(super) fn sum(values: &[u64]) -> Result<u64, String> {
    values.iter().try_fold(0u64, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| String::from("CUDA memory estimate overflow"))
    })
}

pub(super) fn capacity(value: usize, minimum: usize) -> Result<usize, String> {
    value
        .checked_next_power_of_two()
        .map(|v| v.max(minimum))
        .ok_or_else(|| String::from("CUDA memory estimate overflow"))
}

pub(super) fn projected_attention_bytes(
    rows: usize,
    heads: usize,
    width: usize,
    length: usize,
) -> Result<u64, String> {
    if heads == 0 {
        return Err(String::from("memory attention requires at least one head"));
    }
    let sides = capacity(
        rows.checked_mul(heads)
            .ok_or("CUDA memory estimate overflow")?,
        64,
    )?;
    let queries = capacity(rows, 16)?.max(sides / heads);
    sum(&[
        bytes(&[queries, width], 8)?,
        bytes(&[sides, width], 8)?,
        bytes(&[sides, length], 4)?,
        bytes(&[width], 4)?,
    ])
}

/// Checks a concrete allocation against a fresh memory reading. The margin is
/// for allocator and library overhead; other processes can still allocate next.
pub(super) fn admit_allocation(
    stage: &str,
    additional_bytes: u64,
    free_bytes: u64,
    total_bytes: u64,
) -> Result<(), String> {
    let margin_bytes = 16u64 << 20;
    let required_bytes = sum(&[additional_bytes, margin_bytes])?;
    if free_bytes <= total_bytes && required_bytes <= free_bytes {
        return Ok(());
    }
    Err(format!(
        "{MEMORY_REFUSAL_PREFIX}{}",
        serde_json::json!({
            "stage": stage, "additional_bytes": additional_bytes, "margin_bytes": margin_bytes,
            "required_bytes": required_bytes, "free_bytes": free_bytes, "total_bytes": total_bytes,
            "memory_reserved": false,
        })
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flash() -> BackboneShape {
        BackboneShape {
            hidden: 4096,
            ffn: 14336,
            conv_channels: 8192,
            conv_kernel: 4,
            key_heads: 16,
            value_heads: 32,
            state: 128,
            heads: 16,
            kv_heads: 4,
            head_dim: 256,
            rotary: 64,
            width: 1024,
            hybrid_layers: 24,
            attention_layers: 8,
            flash: true,
        }
    }

    #[test]
    fn mixed_quantized_inventory_counts_resident_bytes_and_two_conversion_slots() {
        let mut inventory = Inventory::default();
        assert_eq!(
            inventory.matrix(64, 256, MatrixFormat::Q4K, false).unwrap(),
            64 * 144
        );
        assert_eq!(
            inventory.matrix(2, 64, MatrixFormat::Q8_0, false).unwrap(),
            2 * 2 * 34
        );
        assert_eq!(
            inventory.matrix(3, 4, MatrixFormat::F32, false).unwrap(),
            48
        );
        inventory.f32(0).unwrap();
        inventory.f32(5).unwrap();
        assert_eq!(inventory.resident_bytes, 64 * 144 + 136 + 48 + 4 + 20);
        let estimate = Estimate::new(&inventory, flash(), DEFAULT_TOKENS, DEFAULT_CHUNK).unwrap();
        assert_eq!(estimate.conversion_workspace_bytes, 64 * 256 * 4);
    }

    #[test]
    fn only_supported_always_fused_matrices_avoid_conversion_slots() {
        let mut inventory = Inventory::default();
        inventory.matrix(64, 256, MatrixFormat::Q4K, true).unwrap();
        inventory.matrix(64, 64, MatrixFormat::Q8_0, true).unwrap();
        assert_eq!(inventory.largest_f16_elements, 0);
        inventory.matrix(3, 256, MatrixFormat::Q4K, true).unwrap();
        inventory.matrix(64, 32, MatrixFormat::Q8_0, true).unwrap();
        assert_eq!(inventory.largest_f16_elements, 64 * 32);
        inventory.matrix(64, 256, MatrixFormat::F32, true).unwrap();
        assert_eq!(inventory.largest_f16_elements, 64 * 256);
    }

    #[test]
    fn flash_drops_only_score_workspace_and_keeps_all_caches() {
        let inventory = Inventory::default();
        let fast = Estimate::new(&inventory, flash(), 16384, 2048).unwrap();
        let mut shape = flash();
        shape.flash = false;
        let staged = Estimate::new(&inventory, shape, 16384, 2048).unwrap();
        assert_eq!(
            staged.backbone_request_bytes - fast.backbone_request_bytes,
            2 * 2048 * 16384 * 6 - 6
        );
        let expected = 6
            + 16384 * 1024 * 4
            + 24 * 8192 * 3 * 4
            + 24 * 32 * 128 * 128 * 4
            + 8 * 16384 * 4 * 256 * 4;
        assert_eq!(fast.backbone_request_bytes, expected);
        assert_eq!(fast.backbone_scratch_bytes, 671_744_000);
    }

    #[test]
    fn clef_27b_matrix_inventory_alone_exceeds_16gib() {
        // Header-only inventory of Clef-Q4_K_M.gguf, SHA-256
        // 9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822.
        // This lower bound excludes vector norms, head weights, and scratch.
        let mut inventory = Inventory::default();
        for (rows, columns, count, format) in [
            (5120, 17408, 64, MatrixFormat::Q4K),
            (17408, 5120, 128, MatrixFormat::Q4K),
            (48, 5120, 96, MatrixFormat::Q8_0),
            (1024, 5120, 32, MatrixFormat::Q8_0),
            (5120, 6144, 64, MatrixFormat::Q8_0),
            (6144, 5120, 48, MatrixFormat::Q8_0),
            (10240, 5120, 48, MatrixFormat::Q8_0),
            (12288, 5120, 16, MatrixFormat::Q8_0),
        ] {
            for _ in 0..count {
                inventory.matrix(rows, columns, format, false).unwrap();
            }
        }
        for _ in 0..48 {
            inventory.f32(10240 * 4).unwrap();
        }
        assert_eq!(inventory.resident_bytes, 17_323_950_080);
        assert!(inventory.resident_bytes > 16u64 << 30);
    }

    #[test]
    fn large_model_is_refused_on_16gib_before_any_upload() {
        let inventory = Inventory {
            resident_bytes: 15 << 30,
            largest_f16_elements: 5120 * 17408,
        };
        let mut shape = flash();
        shape.hidden = 5120;
        shape.ffn = 17408;
        shape.heads = 24;
        shape.value_heads = 48;
        shape.hybrid_layers = 48;
        shape.attention_layers = 16;
        let estimate = Estimate::new(&inventory, shape, 16384, 2048).unwrap();
        let error = estimate.admit(16 << 30, 16 << 30).unwrap_err();
        assert!(is_memory_refusal(&error));
        let evidence: serde_json::Value =
            serde_json::from_str(error.strip_prefix("not enough CUDA memory: ").unwrap()).unwrap();
        assert_eq!(evidence["required_bytes"], estimate.required_bytes);
        assert_eq!(evidence["free_bytes"], 16u64 << 30);
        assert_eq!(evidence["total_bytes"], 16u64 << 30);
        assert_eq!(evidence["stage"], "before_weight_upload");
        assert_eq!(evidence["memory_reserved"], false);
        assert!(
            estimate
                .admit(estimate.required_bytes, estimate.required_bytes)
                .is_ok()
        );
        assert!(
            estimate
                .admit(estimate.required_bytes - 1, estimate.required_bytes)
                .is_err()
        );
        assert!(
            estimate
                .admit(estimate.required_bytes + 1, estimate.required_bytes)
                .is_err()
        );
    }

    #[test]
    fn invalid_dimensions_and_overflow_refuse() {
        let mut inventory = Inventory::default();
        assert!(inventory.matrix(1, 33, MatrixFormat::Q8_0, false).is_err());
        assert!(inventory.matrix(1, 255, MatrixFormat::Q4K, false).is_err());
        assert!(inventory.matrix(0, 256, MatrixFormat::F32, false).is_err());
        assert!(
            inventory
                .matrix(usize::MAX, usize::MAX, MatrixFormat::F32, false)
                .is_err()
        );
        assert!(Estimate::new(&inventory, flash(), 0, 1).is_err());
        assert!(Estimate::new(&inventory, flash(), 1, 0).is_err());
        assert!(Estimate::new(&inventory, flash(), usize::MAX, 2048).is_err());
        assert!(sum(&[u64::MAX, 1]).is_err());
        assert!(capacity(usize::MAX, 64).is_err());
        assert!(projected_attention_bytes(usize::MAX, 8, 1024, 16384).is_err());
    }

    #[test]
    fn actual_head_scores_include_rounded_side_and_query_rows() {
        assert_eq!(
            projected_attention_bytes(17, 8, 1024, 33).unwrap(),
            32 * 1024 * 8 + 256 * 1024 * 8 + 256 * 33 * 4 + 1024 * 4
        );
        assert!(projected_attention_bytes(1, 0, 1024, 33).is_err());
    }
    #[test]
    fn request_allocation_uses_live_free_bytes_and_reports_its_stage() {
        let additional = 128u64 << 20;
        let required = additional + (16 << 20);
        assert!(admit_allocation("head_scores", additional, required, 16 << 30).is_ok());
        let error =
            admit_allocation("head_scores", additional, required - 1, 16 << 30).unwrap_err();
        let evidence: serde_json::Value =
            serde_json::from_str(error.strip_prefix("not enough CUDA memory: ").unwrap()).unwrap();
        assert_eq!(evidence["stage"], "head_scores");
        assert_eq!(evidence["additional_bytes"], additional);
        assert_eq!(evidence["required_bytes"], required);
        assert!(admit_allocation("head_scores", u64::MAX, u64::MAX, u64::MAX).is_err());
    }
}
