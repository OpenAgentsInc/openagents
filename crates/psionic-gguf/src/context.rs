//! Hybrid KV + Gated DeltaNet cache sizes for `qwen35`.

use crate::format::GgufMeta;

/// First laptop runtime context. Native 262,144 is trained max, not default.
pub const DEFAULT_RUNTIME_CTX: u64 = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachePlan {
    pub n_ctx: u64,
    pub n_layers: u64,
    pub n_full: u64,
    pub n_gdn: u64,
    pub n_kv_heads: u64,
    pub head_width: u64,
    pub kv_bytes: u64,
    pub gdn_bytes: u64,
}

impl CachePlan {
    pub fn total_bytes(&self) -> u64 {
        self.kv_bytes.saturating_add(self.gdn_bytes)
    }
}

/// Runtime `n_ctx`: operator override, else min(4096, trained) when trained
/// is large, else the trained length (tiny fixtures keep 32).
pub fn runtime_n_ctx(meta: &GgufMeta, requested: Option<u64>) -> u64 {
    let trained = meta
        .kv_u64("qwen35.context_length")
        .unwrap_or(DEFAULT_RUNTIME_CTX);
    if let Some(n) = requested {
        return n.max(1);
    }
    if trained > DEFAULT_RUNTIME_CTX {
        DEFAULT_RUNTIME_CTX
    } else {
        trained.max(1)
    }
}

pub fn plan_caches(meta: &GgufMeta, n_ctx: u64) -> CachePlan {
    let n_layers = meta.kv_u64("qwen35.block_count").unwrap_or(1).max(1);
    let interval = meta
        .kv_u64("qwen35.full_attention_interval")
        .unwrap_or(4)
        .max(1);
    // Layer i is full-attention when i % interval == interval - 1.
    // A one-layer fixture never hits that slot; treat layer 0 as full-attention
    // so KV and GDN are both non-zero on the CI file.
    let counted_full = (0..n_layers)
        .filter(|i| i % interval == interval - 1)
        .count() as u64;
    let n_full = if counted_full == 0 { 1 } else { counted_full };
    let n_gdn = n_layers.saturating_sub(counted_full).max(1);

    let embd = meta.kv_u64("qwen35.embedding_length").unwrap_or(8).max(1);
    let n_kv_heads = meta
        .kv_u64("qwen35.attention.head_count_kv")
        .or_else(|| meta.kv_u64("qwen35.attention.kv_heads"))
        .unwrap_or(if embd >= 5120 { 4 } else { 1 })
        .max(1);
    let n_q_heads = meta
        .kv_u64("qwen35.attention.head_count")
        .unwrap_or(if embd >= 5120 { 24 } else { 1 })
        .max(1);
    let head_width = meta
        .kv_u64("qwen35.attention.key_length")
        .unwrap_or(if embd >= 5120 {
            256
        } else {
            (embd / n_q_heads).max(1)
        })
        .max(1);

    // F16 KV: 2 * n_full * n_kv_heads * head_width * n_ctx * 2
    let kv_bytes = 2u64
        .saturating_mul(n_full)
        .saturating_mul(n_kv_heads)
        .saturating_mul(head_width)
        .saturating_mul(n_ctx)
        .saturating_mul(2);

    let conv_kernel = meta.kv_u64("qwen35.ssm.conv_kernel").unwrap_or(4).max(1);
    let state_size = meta
        .kv_u64("qwen35.ssm.state_size")
        .unwrap_or(if embd >= 5120 { 128 } else { embd.min(8) })
        .max(1);
    let time_step_rank = meta
        .kv_u64("qwen35.ssm.time_step_rank")
        .unwrap_or(if embd >= 5120 { 48 } else { 1 })
        .max(1);
    let inner_size = meta
        .kv_u64("qwen35.ssm.inner_size")
        .unwrap_or(if embd >= 5120 {
            6144
        } else {
            meta.kv_u64("qwen35.feed_forward_length").unwrap_or(16)
        })
        .max(1);

    let conv_bytes = inner_size.saturating_mul(conv_kernel.saturating_sub(1)) * 4;
    let delta_bytes = time_step_rank
        .saturating_mul(state_size)
        .saturating_mul(state_size)
        * 4;
    let gdn_bytes = n_gdn.saturating_mul(conv_bytes.saturating_add(delta_bytes));

    CachePlan {
        n_ctx,
        n_layers,
        n_full,
        n_gdn,
        n_kv_heads,
        head_width,
        kv_bytes,
        gdn_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::write_qwen35_fixture;
    use crate::parse_path;

    #[test]
    fn fixture_plan_is_nonzero_and_matches_formula() {
        let dir = std::env::temp_dir().join("psionic-gguf-ctx");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("qwen35.gguf");
        write_qwen35_fixture(&path).unwrap();
        let meta = parse_path(&path).unwrap();
        let n_ctx = runtime_n_ctx(&meta, None);
        assert_eq!(n_ctx, 32);
        let plan = plan_caches(&meta, n_ctx);
        assert_eq!(plan.n_full, 1);
        assert_eq!(plan.n_gdn, 1);
        assert_eq!(plan.n_kv_heads, 1);
        assert_eq!(plan.head_width, 8);
        assert_eq!(plan.kv_bytes, 2 * 1 * 1 * 8 * 32 * 2);
        assert!(plan.gdn_bytes > 0);
        assert_eq!(plan.kv_bytes, 1024);
    }

    #[test]
    fn twenty_seven_b_shape_is_64_kib_per_token() {
        // Synthetic metadata matching the 27B-class issue formula.
        let meta = GgufMeta {
            version: 3,
            n_tensors: 0,
            n_kv: 0,
            kv: vec![
                (
                    "qwen35.block_count".into(),
                    crate::format::GgufValue::U32(64),
                ),
                (
                    "qwen35.full_attention_interval".into(),
                    crate::format::GgufValue::U32(4),
                ),
                (
                    "qwen35.embedding_length".into(),
                    crate::format::GgufValue::U32(5120),
                ),
                (
                    "qwen35.attention.head_count_kv".into(),
                    crate::format::GgufValue::U32(4),
                ),
                (
                    "qwen35.attention.head_count".into(),
                    crate::format::GgufValue::U32(24),
                ),
                (
                    "qwen35.attention.key_length".into(),
                    crate::format::GgufValue::U32(256),
                ),
                (
                    "qwen35.context_length".into(),
                    crate::format::GgufValue::U32(262144),
                ),
            ],
            tensors: vec![],
            data_offset: 0,
            alignment: 32,
            file_size: 0,
        };
        assert_eq!(runtime_n_ctx(&meta, None), 4096);
        let plan = plan_caches(&meta, 4096);
        assert_eq!(plan.n_full, 16);
        assert_eq!(plan.n_gdn, 48);
        // F16 K+V: 2 * 16 * 4 * 256 * n_ctx * 2 = 64 KiB * n_ctx
        assert_eq!(plan.kv_bytes, 64 * 1024 * 4096);
    }
}
