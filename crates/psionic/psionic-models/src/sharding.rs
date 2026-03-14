use psionic_runtime::{
    LocalModelWeightClass, LocalShardingContract, LocalShardingContractError,
    LocalWeightShardingRule, LocalWeightShardingStrategy, ShardedModelLayoutKind,
};
use thiserror::Error;

use crate::GgufDecoderFamily;

/// Failure while deriving one representative GGUF decoder-family sharding contract.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DecoderFamilyShardingContractError {
    /// The requested family does not yet have a truthful local sharding contract.
    #[error(
        "gguf decoder family `{family:?}` does not yet have a representative local tensor-parallel sharding contract"
    )]
    UnsupportedFamily {
        /// Unsupported family.
        family: GgufDecoderFamily,
    },
    /// The generic runtime sharding contract rejected the derived rules.
    #[error(transparent)]
    Contract(#[from] LocalShardingContractError),
}

/// Returns one representative local tensor-parallel sharding contract for a GGUF decoder family.
pub fn gguf_decoder_family_tensor_parallel_contract(
    family: GgufDecoderFamily,
    effective_backend: impl Into<String>,
    device_count: usize,
) -> Result<LocalShardingContract, DecoderFamilyShardingContractError> {
    let family_label = match family {
        GgufDecoderFamily::Llama => "llama",
        GgufDecoderFamily::Qwen => "qwen",
        GgufDecoderFamily::Mistral => "mistral",
        GgufDecoderFamily::GptOss => {
            return Err(DecoderFamilyShardingContractError::UnsupportedFamily { family });
        }
    };
    let effective_backend = effective_backend.into();

    Ok(LocalShardingContract::new(
        format!("gguf-decoder-{family_label}-tensor-parallel-v1"),
        format!("gguf_decoder:{family_label}"),
        effective_backend,
        ShardedModelLayoutKind::TensorSharded,
        device_count,
        Some(device_count),
        Some(8 * 1024 * 1024 * 1024),
        vec![
            LocalWeightShardingRule::new(
                LocalModelWeightClass::TokenEmbedding,
                LocalWeightShardingStrategy::TensorAxis { axis: 1 },
            )
            .with_detail(
                "split the embedding width across local shards while keeping token rows logically global",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::AttentionQuery,
                LocalWeightShardingStrategy::TensorAxis { axis: 0 },
            )
            .with_detail(
                "split attention query heads across shards so each shard owns a disjoint head range",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::AttentionKey,
                LocalWeightShardingStrategy::TensorAxis { axis: 0 },
            )
            .with_detail(
                "split attention key heads across the same tensor axis as the query projection",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::AttentionValue,
                LocalWeightShardingStrategy::TensorAxis { axis: 0 },
            )
            .with_detail(
                "split attention value heads across the same tensor axis as the query projection",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::AttentionOutput,
                LocalWeightShardingStrategy::TensorAxis { axis: 1 },
            )
            .with_detail(
                "split the attention output fan-in across shards to reassemble hidden state locally",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::FeedForwardGate,
                LocalWeightShardingStrategy::TensorAxis { axis: 0 },
            )
            .with_detail(
                "split feed-forward gate rows across shards for tensor-parallel expansion",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::FeedForwardUp,
                LocalWeightShardingStrategy::TensorAxis { axis: 0 },
            )
            .with_detail(
                "split feed-forward up rows across shards for tensor-parallel expansion",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::FeedForwardDown,
                LocalWeightShardingStrategy::TensorAxis { axis: 1 },
            )
            .with_detail(
                "split the feed-forward down fan-in across shards to collapse the expanded hidden state",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::OutputProjection,
                LocalWeightShardingStrategy::TensorAxis { axis: 1 },
            )
            .with_detail(
                "split the output projection hidden dimension across shards on untied or tied decoder heads",
            ),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::AttentionNorm,
                LocalWeightShardingStrategy::Replicated,
            )
            .with_detail("replicate small norm weights on every local shard"),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::FeedForwardNorm,
                LocalWeightShardingStrategy::Replicated,
            )
            .with_detail("replicate small norm weights on every local shard"),
            LocalWeightShardingRule::new(
                LocalModelWeightClass::KvCache,
                LocalWeightShardingStrategy::Replicated,
            )
            .with_detail(
                "keep the representative v1 contract conservative by not claiming tensor-sharded KV ownership yet",
            ),
        ],
    )?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_runtime::{LocalModelWeightClass, LocalWeightShardingStrategy};

    use super::{DecoderFamilyShardingContractError, gguf_decoder_family_tensor_parallel_contract};
    use crate::GgufDecoderFamily;

    #[test]
    fn gguf_decoder_family_tensor_parallel_contract_is_declarative_and_inspectable()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract =
            gguf_decoder_family_tensor_parallel_contract(GgufDecoderFamily::Llama, "cuda", 2)?;

        assert_eq!(contract.model_family, "gguf_decoder:llama");
        assert_eq!(contract.min_device_count, 2);
        assert_eq!(contract.max_device_count, Some(2));
        assert_eq!(
            contract.weight_rules[0].class,
            LocalModelWeightClass::TokenEmbedding
        );
        assert_eq!(
            contract.weight_rules[0].strategy,
            LocalWeightShardingStrategy::TensorAxis { axis: 1 }
        );
        assert!(
            contract
                .weight_rules
                .iter()
                .any(|rule| rule.class == LocalModelWeightClass::AttentionQuery
                    && rule.strategy == LocalWeightShardingStrategy::TensorAxis { axis: 0 })
        );
        assert!(
            contract
                .weight_rules
                .iter()
                .any(|rule| rule.class == LocalModelWeightClass::KvCache
                    && rule.strategy == LocalWeightShardingStrategy::Replicated)
        );
        assert!(!contract.contract_digest.is_empty());
        Ok(())
    }

    #[test]
    fn gguf_decoder_family_tensor_parallel_contract_refuses_gpt_oss_until_moe_rules_exist() {
        let error =
            gguf_decoder_family_tensor_parallel_contract(GgufDecoderFamily::GptOss, "cuda", 2)
                .expect_err("gpt-oss should remain refused");
        assert_eq!(
            error,
            DecoderFamilyShardingContractError::UnsupportedFamily {
                family: GgufDecoderFamily::GptOss,
            }
        );
    }
}
