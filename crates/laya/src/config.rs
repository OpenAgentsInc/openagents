//! The checkpoint's own configuration: `rl_agent_config.json` bounds and
//! temperatures, the encoder's Hugging Face `config.json`, and the
//! tokenizer's special tokens.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::error::{Error, Result};

/// The question-type indexes the head's embeddings and the temperature
/// table use, in the reference's order.
pub const QTYPE_CHOICE: usize = 0;
/// `score` questions index the type embedding at 1.
pub const QTYPE_SCORE: usize = 1;
/// `noul` questions index the type embedding at 2.
pub const QTYPE_NOUL: usize = 2;

/// The wire name of a question-type index.
#[must_use]
pub fn qtype_name(qtype: usize) -> &'static str {
    match qtype {
        QTYPE_CHOICE => "choice",
        QTYPE_SCORE => "score",
        _ => "noul",
    }
}

/// `rl_agent_config.json`: the decision head's shape, the sequence bounds,
/// and the fitted temperatures. Fields the port does not read (training
/// record, batching hints) stay unmodelled; the file is preserved verbatim
/// in the artifact identity.
#[derive(Clone, Debug, Deserialize)]
pub struct HeadConfig {
    /// The Hugging Face id the encoder was trained from; reported on the
    /// model card, never loaded.
    pub encoder: String,
    /// Transformer layers in the decision head.
    pub head_layers: usize,
    /// The sequence-length bound every encoded question obeys.
    pub max_len: usize,
    /// The token budget for the question/options half of a sequence.
    pub head_max_len: usize,
    /// Named escalation actions; the act head emits `len + 1` logits.
    #[serde(default)]
    pub act_costs: BTreeMap<String, f64>,
    /// Per-type temperatures indexed by `choice`, `score`, `noul` order;
    /// fitted post-hoc on the checkpoint's own dev data.
    #[serde(default)]
    pub temperature: Vec<f64>,
    /// Per-(type, option-cardinality) temperature overrides; a missing
    /// bucket falls back to the per-type value.
    #[serde(default)]
    pub temperature_by_options: BTreeMap<String, f64>,
    /// The checkpoint's declared model id (`laya-typed-decisions`); a
    /// card field only — the reference's answer envelope says `rl-agent`
    /// for every checkpoint.
    #[serde(default)]
    pub model_name: Option<String>,
}

impl HeadConfig {
    /// Read `rl_agent_config.json` and check the bounds the port relies on.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] for unreadable or malformed JSON, a
    /// missing temperature table, or a `head_max_len` that cannot bound
    /// the marker region inside `max_len`.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Artifact(format!("read {}: {e}", path.display())))?;
        let cfg: Self = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Artifact(format!("parse {}: {e}", path.display())))?;
        if cfg.temperature.len() != 3 {
            return Err(Error::Artifact(format!(
                "{}: temperature must hold three entries (choice, score, noul)",
                path.display()
            )));
        }
        if cfg.head_max_len >= cfg.max_len {
            return Err(Error::Artifact(format!(
                "{}: head_max_len {} must stay below max_len {}",
                path.display(),
                cfg.head_max_len,
                cfg.max_len
            )));
        }
        if cfg.head_layers == 0 {
            return Err(Error::Artifact(format!(
                "{}: head_layers must be positive",
                path.display()
            )));
        }
        Ok(cfg)
    }

    /// The temperature for a `(type, cardinality)` bucket: the
    /// per-cardinality override when the checkpoint carries one, else the
    /// per-type value. Mirrors `temp_bucket` + the dict lookup in the
    /// reference's `system_one`.
    #[must_use]
    pub fn temperature_for(&self, qtype: usize, k: usize) -> f64 {
        let bucket = temp_bucket(qtype, k);
        self.temperature_by_options
            .get(&bucket)
            .copied()
            .unwrap_or(self.temperature[qtype])
    }
}

/// The key the per-cardinality temperature map uses: a 2-option noul and a
/// 20-option choice need different scaling.
#[must_use]
pub fn temp_bucket(qtype: usize, k: usize) -> String {
    let size = if k <= 2 {
        "2"
    } else if k <= 5 {
        "3-5"
    } else if k <= 10 {
        "6-10"
    } else {
        "11+"
    };
    format!("{}:{size}", qtype_name(qtype))
}

/// The encoder's Hugging Face `config.json`, narrowed to the fields the
/// candle `ModernBert` needs plus the ones the port validates.
#[derive(Clone, Debug, Deserialize)]
pub struct EncoderConfig {
    /// Embedding table rows.
    pub vocab_size: usize,
    /// Residual width.
    pub hidden_size: usize,
    /// Transformer block count.
    pub num_hidden_layers: usize,
    /// Attention heads per block.
    pub num_attention_heads: usize,
    /// GeGLU intermediate width.
    pub intermediate_size: usize,
    /// The longest position the RoPE tables cover.
    pub max_position_embeddings: usize,
    /// LayerNorm epsilon.
    #[serde(default = "default_eps")]
    pub layer_norm_eps: f64,
    /// The padding id the encoder was trained with.
    pub pad_token_id: u32,
    /// Every Nth block is full attention; the rest are sliding-window.
    pub global_attn_every_n_layers: usize,
    /// The sliding window's total width.
    pub local_attention: usize,
    /// Flat RoPE theta alias for the full-attention layers
    /// (`attribute_map` in the HF config maps `rope_theta` onto
    /// `global_rope_theta`).
    #[serde(default)]
    pub rope_theta: Option<f64>,
    /// Flat full-attention RoPE theta. This is the only shape the
    /// reference runtime — transformers 4.x — reads; the 5.x nested
    /// `rope_parameters` block parses as an opaque attribute there and
    /// never reaches the attention layers, so it is not consulted here
    /// either.
    #[serde(default)]
    pub global_rope_theta: Option<f64>,
    /// Flat local-window RoPE theta; see `global_rope_theta`.
    #[serde(default)]
    pub local_rope_theta: Option<f64>,
    /// The per-layer attention kinds the checkpoint declares; when present
    /// the port checks them against `global_attn_every_n_layers` rather
    /// than trusting either alone.
    #[serde(default)]
    pub layer_types: Vec<String>,
}

fn default_eps() -> f64 {
    1e-5
}

/// ModernBERT's default full-attention RoPE base (`global_rope_theta` in
/// `ModernBertConfig`).
const DEFAULT_GLOBAL_ROPE_THETA: f64 = 160_000.0;
/// ModernBERT's default sliding-window RoPE base (`local_rope_theta`).
const DEFAULT_LOCAL_ROPE_THETA: f64 = 10_000.0;

impl EncoderConfig {
    /// Read `encoder/config.json`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] for unreadable or malformed JSON.
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| Error::Artifact(format!("read {}: {e}", path.display())))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| Error::Artifact(format!("parse {}: {e}", path.display())))
    }

    /// The full-attention RoPE theta, resolved the way the reference's
    /// transformers 4.x runtime resolves it: flat `global_rope_theta`,
    /// then its `rope_theta` alias, then the ModernBERT default. A
    /// 5.x-style `rope_parameters` block is intentionally not consulted —
    /// the reference never applies it, so honoring it would diverge from
    /// the answers the checkpoint actually produces there.
    #[must_use]
    pub fn global_theta(&self) -> f64 {
        self.global_rope_theta
            .or(self.rope_theta)
            .unwrap_or(DEFAULT_GLOBAL_ROPE_THETA)
    }

    /// The sliding-window RoPE theta, under the same 4.x resolution:
    /// flat `local_rope_theta`, then the ModernBERT default.
    #[must_use]
    pub fn local_theta(&self) -> f64 {
        self.local_rope_theta.unwrap_or(DEFAULT_LOCAL_ROPE_THETA)
    }

    /// Check the declared `layer_types` against `global_attn_every_n_layers`
    /// — the candle implementation derives sliding windows from the period
    /// alone, so a checkpoint whose pattern disagrees cannot be served
    /// faithfully and is refused.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] when the patterns disagree or the count
    /// does not match `num_hidden_layers`.
    pub fn check_layer_pattern(&self) -> Result<()> {
        if self.layer_types.is_empty() {
            return Ok(());
        }
        if self.layer_types.len() != self.num_hidden_layers {
            return Err(Error::Artifact(format!(
                "encoder config declares {} layer_types for {} layers",
                self.layer_types.len(),
                self.num_hidden_layers
            )));
        }
        for (i, kind) in self.layer_types.iter().enumerate() {
            let expect_local = i % self.global_attn_every_n_layers != 0;
            let declared_local = kind == "sliding_attention";
            if expect_local != declared_local {
                return Err(Error::Artifact(format!(
                    "layer {i} is `{kind}` but global_attn_every_n_layers={} expects {}",
                    self.global_attn_every_n_layers,
                    if expect_local {
                        "sliding_attention"
                    } else {
                        "full_attention"
                    }
                )));
            }
        }
        Ok(())
    }
}

/// The special-token ids a checkpoint's tokenizer assigns, resolved from
/// `tokenizer/tokenizer_config.json` plus the token ids the tokenizer
/// itself reports.
#[derive(Clone, Debug)]
pub struct Specials {
    /// Sequence open: `[CLS]` or `<bos>`.
    pub cls_id: u32,
    /// Segment break and close: `[SEP]` or `<eos>`.
    pub sep_id: u32,
    /// The per-option marker: `[MASK]` or `<mask>`.
    pub mask_id: u32,
    /// Right-padding id.
    pub pad_id: u32,
    /// The mask token's literal text; caller text that spells it is
    /// sanitized to a space so it cannot forge a marker.
    pub mask_token: String,
}

/// The special-token names the reference reads off `AutoTokenizer`.
///
/// Hugging Face configs write each token as a string or as an object with
/// a `content` field; both parse here.
fn token_string(config: &serde_json::Value, key: &str) -> Result<String> {
    match config.get(key) {
        Some(serde_json::Value::String(s)) => Ok(s.clone()),
        Some(serde_json::Value::Object(o)) => o
            .get("content")
            .and_then(|c| c.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                Error::Artifact(format!("tokenizer config `{key}` object lacks `content`"))
            }),
        _ => Err(Error::Artifact(format!("tokenizer config lacks `{key}`"))),
    }
}

impl Specials {
    /// Resolve the four special ids for a checkpoint's tokenizer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Artifact`] when `tokenizer_config.json` lacks a
    /// token the sequence format needs or the tokenizer does not assign
    /// it an id.
    pub fn load(tokenizer: &Tokenizer, config_path: &Path) -> Result<Self> {
        let bytes = std::fs::read(config_path)
            .map_err(|e| Error::Artifact(format!("read {}: {e}", config_path.display())))?;
        let config: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| Error::Artifact(format!("parse {}: {e}", config_path.display())))?;
        let id_of = |name: &str, token: &str| -> Result<u32> {
            tokenizer.token_to_id(token).ok_or_else(|| {
                Error::Artifact(format!("tokenizer assigns no id to {name} token `{token}`"))
            })
        };
        let cls = token_string(&config, "cls_token")?;
        let sep = token_string(&config, "sep_token")?;
        let mask = token_string(&config, "mask_token")?;
        let pad = token_string(&config, "pad_token")?;
        Ok(Self {
            cls_id: id_of("cls", &cls)?,
            sep_id: id_of("sep", &sep)?,
            mask_id: id_of("mask", &mask)?,
            pad_id: id_of("pad", &pad)?,
            mask_token: mask,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_buckets_cover_the_cardinality_ranges() {
        assert_eq!(temp_bucket(QTYPE_NOUL, 2), "noul:2");
        assert_eq!(temp_bucket(QTYPE_CHOICE, 2), "choice:2");
        assert_eq!(temp_bucket(QTYPE_CHOICE, 3), "choice:3-5");
        assert_eq!(temp_bucket(QTYPE_CHOICE, 5), "choice:3-5");
        assert_eq!(temp_bucket(QTYPE_CHOICE, 6), "choice:6-10");
        assert_eq!(temp_bucket(QTYPE_SCORE, 10), "score:6-10");
        assert_eq!(temp_bucket(QTYPE_SCORE, 11), "score:11+");
        assert_eq!(temp_bucket(QTYPE_CHOICE, 255), "choice:11+");
    }

    #[test]
    fn head_config_falls_back_to_per_type_temperature() {
        let cfg = HeadConfig {
            encoder: "e".to_string(),
            head_layers: 2,
            max_len: 512,
            head_max_len: 192,
            act_costs: BTreeMap::new(),
            temperature: vec![1.6, 1.2, 1.9],
            temperature_by_options: [("choice:2".to_string(), 0.5)].into_iter().collect(),
            model_name: None,
        };
        assert_eq!(cfg.temperature_for(QTYPE_CHOICE, 2), 0.5);
        assert_eq!(cfg.temperature_for(QTYPE_CHOICE, 3), 1.6);
        assert_eq!(cfg.temperature_for(QTYPE_SCORE, 4), 1.2);
        assert_eq!(cfg.temperature_for(QTYPE_NOUL, 2), 1.9);
    }

    #[test]
    fn encoder_config_resolves_thetas_like_transformers_4x() {
        // The 5.x nested `rope_parameters` block is parsed as an opaque
        // attribute by the reference's transformers 4.x and never reaches
        // the attention layers — the thetas resolve to the ModernBERT
        // defaults even when the block declares other values.
        let nested: EncoderConfig = serde_json::from_value(serde_json::json!({
            "vocab_size": 1, "hidden_size": 1, "num_hidden_layers": 1,
            "num_attention_heads": 1, "intermediate_size": 1,
            "max_position_embeddings": 1, "pad_token_id": 0,
            "global_attn_every_n_layers": 3, "local_attention": 128,
            "rope_parameters": {
                "full_attention": {"rope_theta": 42.0, "rope_type": "default"},
                "sliding_attention": {"rope_theta": 7.0, "rope_type": "default"}
            }
        }))
        .unwrap();
        assert_eq!(nested.global_theta(), 160_000.0);
        assert_eq!(nested.local_theta(), 10_000.0);
        // Flat fields are read, and `rope_theta` aliases the global one.
        let flat: EncoderConfig = serde_json::from_value(serde_json::json!({
            "vocab_size": 1, "hidden_size": 1, "num_hidden_layers": 1,
            "num_attention_heads": 1, "intermediate_size": 1,
            "max_position_embeddings": 1, "pad_token_id": 0,
            "global_attn_every_n_layers": 3, "local_attention": 128,
            "global_rope_theta": 500000.0, "local_rope_theta": 2000.0
        }))
        .unwrap();
        assert_eq!(flat.global_theta(), 500_000.0);
        assert_eq!(flat.local_theta(), 2_000.0);
        let aliased: EncoderConfig = serde_json::from_value(serde_json::json!({
            "vocab_size": 1, "hidden_size": 1, "num_hidden_layers": 1,
            "num_attention_heads": 1, "intermediate_size": 1,
            "max_position_embeddings": 1, "pad_token_id": 0,
            "global_attn_every_n_layers": 3, "local_attention": 128,
            "rope_theta": 64000.0
        }))
        .unwrap();
        assert_eq!(aliased.global_theta(), 64_000.0);
    }

    #[test]
    fn layer_pattern_must_match_the_attention_period() {
        let mut config: EncoderConfig = serde_json::from_value(serde_json::json!({
            "vocab_size": 1, "hidden_size": 1, "num_hidden_layers": 3,
            "num_attention_heads": 1, "intermediate_size": 1,
            "max_position_embeddings": 1, "pad_token_id": 0,
            "global_attn_every_n_layers": 3, "local_attention": 128,
            "layer_types": ["full_attention", "sliding_attention", "sliding_attention"]
        }))
        .unwrap();
        config.check_layer_pattern().unwrap();
        config.layer_types[1] = "full_attention".to_string();
        assert!(config.check_layer_pattern().is_err());
        config.num_hidden_layers = 4;
        config.layer_types[1] = "sliding_attention".to_string();
        assert!(config.check_layer_pattern().is_err());
    }
}
