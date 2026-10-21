//! Engram equations from `inference/engram.py` and `model.py:328-366`.
//!
//! Table rows and projections stay float32 in this reference. The official
//! BF16 lookup/projection boundaries still need full-checkpoint qualification.

use super::{CpuError, MatrixRows, finite, positive, product, shape, sigmoid};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use tokenizers::{
    NormalizedString, Normalizer, Tokenizer,
    normalizers::{
        Lowercase, NFD, NFKC, Replace, Sequence, Strip, StripAccents, replace::ReplacePattern,
    },
};

pub const COMPRESSED_VOCAB_SIZE: usize = 99_092;

fn normalizer() -> Result<Sequence, CpuError> {
    let replace = |pattern: ReplacePattern, content: &str| {
        Replace::new(pattern, content).map_err(|e| CpuError::Tokenization(e.to_string()))
    };
    Ok(Sequence::new(vec![
        NFKC.into(),
        NFD.into(),
        StripAccents.into(),
        Lowercase.into(),
        replace(ReplacePattern::Regex("[ \\t\\r\\n]+".into()), " ")?.into(),
        replace(ReplacePattern::Regex("^ $".into()), "\u{e000}")?.into(),
        Strip::new(true, true).into(),
        replace(ReplacePattern::String("\u{e000}".into()), " ")?.into(),
    ]))
}

fn compression_key(normalizer: &Sequence, decoded: &str, raw: &str) -> Result<String, CpuError> {
    if decoded.contains('\u{fffd}') {
        return Ok(raw.to_owned());
    }
    let mut text = NormalizedString::from(decoded);
    normalizer
        .normalize(&mut text)
        .map_err(|e| CpuError::Tokenization(e.to_string()))?;
    Ok(if text.get().is_empty() {
        decoded.to_owned()
    } else {
        text.get().to_owned()
    })
}

#[derive(Clone, Debug)]
pub struct CompressedTokenMap {
    ids: Vec<u32>,
    vocab_size: usize,
}

impl CompressedTokenMap {
    /// Decode each single token with special tokens retained; assign compressed
    /// IDs in token-ID order, matching the training reference's first-seen order.
    pub fn build(tokenizer: &Tokenizer, expected_vocab_size: usize) -> Result<Self, CpuError> {
        let norm = normalizer()?;
        let count = tokenizer.get_vocab_size(true);
        if count == 0 || count > u32::MAX as usize {
            return Err(CpuError::Invalid("tokenizer vocabulary size"));
        }
        let mut keys = HashMap::new();
        let mut ids = Vec::with_capacity(count);
        for id in 0..count as u32 {
            let decoded = tokenizer
                .decode(&[id], false)
                .map_err(|e| CpuError::Tokenization(e.to_string()))?;
            let raw = tokenizer
                .id_to_token(id)
                .ok_or(CpuError::Invalid("missing tokenizer ID"))?;
            let key = compression_key(&norm, &decoded, &raw)?;
            let next = keys.len() as u32;
            ids.push(*keys.entry(key).or_insert(next));
        }
        if keys.len() != expected_vocab_size {
            return Err(CpuError::Invalid(
                "compressed tokenizer vocabulary does not match configuration",
            ));
        }
        Ok(Self {
            ids,
            vocab_size: keys.len(),
        })
    }
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }
    pub fn get(&self, token: u32) -> Result<u32, CpuError> {
        self.ids
            .get(token as usize)
            .copied()
            .ok_or(CpuError::Invalid("Engram token ID out of range"))
    }
}

#[derive(Clone, Debug)]
pub struct LayerLayout {
    pub layer_id: u32,
    pub primes: Vec<u64>,
    pub offsets: Vec<u64>,
    pub multipliers: [u64; 4],
    pub table_rows: u64,
}

#[derive(Clone, Debug)]
pub struct EngramLayout {
    pub layers: Vec<LayerLayout>,
    pub max_ngram_size: usize,
    pub heads: usize,
    pub head_dim: usize,
    pub compressed_vocab_size: usize,
}

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    let mut d = 3;
    while d <= n / d {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 2;
    }
    true
}

impl EngramLayout {
    pub fn flash() -> Result<Self, CpuError> {
        // NumPy default_rng(10007 * layer).integers(0, (i64::MAX/99092)/2,
        // size=4, dtype=int64) * 2 + 1. Pin the resulting constants rather than
        // substituting another RNG with a different table hash.
        let specifications = [
            (
                1,
                [
                    76_632_096_046_245,
                    4_839_876_093_313,
                    35_959_672_319_349,
                    73_987_337_458_391,
                ],
                384_006_168,
            ),
            (
                14,
                [
                    67_716_810_739_261,
                    51_510_806_800_915,
                    30_921_347_202_721,
                    82_619_226_485_591,
                ],
                384_016_682,
            ),
        ];
        let mut layers = Vec::new();
        let mut next_prime = 16_000_000;
        for (layer_id, multipliers, expected_rows) in specifications {
            let mut primes = Vec::new();
            let mut offsets = Vec::new();
            let mut rows = 0;
            for _ in 0..24 {
                while !is_prime(next_prime) {
                    next_prime += 1;
                }
                offsets.push(rows);
                primes.push(next_prime);
                rows += next_prime;
                next_prime += 1;
            }
            if rows != expected_rows {
                return Err(CpuError::Invalid(
                    "Engram prime layout does not match checkpoint",
                ));
            }
            layers.push(LayerLayout {
                layer_id,
                primes,
                offsets,
                multipliers,
                table_rows: rows,
            });
        }
        Ok(Self {
            layers,
            max_ngram_size: 4,
            heads: 8,
            head_dim: 256,
            compressed_vocab_size: COMPRESSED_VOCAB_SIZE,
        })
    }

    fn validate(&self) -> Result<(), CpuError> {
        if self.max_ngram_size != 4
            || self.heads == 0
            || self.head_dim == 0
            || self.compressed_vocab_size == 0
            || self.layers.is_empty()
        {
            return Err(CpuError::Invalid("Engram layout"));
        }
        let columns = product(self.max_ngram_size - 1, self.heads)?;
        for layer in &self.layers {
            if layer.primes.len() != columns || layer.offsets.len() != columns {
                return Err(CpuError::Invalid("Engram bucket dimensions"));
            }
            let mut end = 0_u64;
            for (p, offset) in layer.primes.iter().zip(&layer.offsets) {
                if *p < 2 || *offset != end {
                    return Err(CpuError::Invalid("Engram bucket range"));
                }
                end = end
                    .checked_add(*p)
                    .ok_or(CpuError::Invalid("Engram row overflow"))?;
            }
            if end != layer.table_rows
                || layer.multipliers.iter().any(|m| {
                    *m == 0
                        || m % 2 == 0
                        || m.checked_mul((self.compressed_vocab_size - 1) as u64)
                            .is_none_or(|v| v > i64::MAX as u64)
                })
            {
                return Err(CpuError::Invalid("Engram multipliers or row count"));
            }
        }
        Ok(())
    }
}

/// One token's `[Engram layer][n-gram size then hash head]` table row IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenHashes {
    pub layers: Vec<Vec<u64>>,
}

#[derive(Clone, Debug)]
pub struct NgramHashState {
    layout: Arc<EngramLayout>,
    pad_id: u32,
    history: VecDeque<Option<u32>>,
    next_position: usize,
}

impl NgramHashState {
    pub fn new(layout: Arc<EngramLayout>, pad_id: u32) -> Result<Self, CpuError> {
        layout.validate()?;
        if pad_id as usize >= layout.compressed_vocab_size {
            return Err(CpuError::Invalid("Engram compressed pad ID"));
        }
        Ok(Self {
            layout,
            pad_id,
            history: VecDeque::new(),
            next_position: 0,
        })
    }
    pub fn next_position(&self) -> usize {
        self.next_position
    }
    pub fn reset(&mut self) {
        self.history.clear();
        self.next_position = 0;
    }

    /// `None` is a dead token. A lookback cannot cross it; that token and every
    /// older token in the n-gram become pad IDs, as at the sequence boundary.
    pub fn push_compressed(
        &mut self,
        start_position: usize,
        ids: &[Option<u32>],
    ) -> Result<Vec<TokenHashes>, CpuError> {
        if start_position != self.next_position
            || ids
                .iter()
                .flatten()
                .any(|i| *i as usize >= self.layout.compressed_vocab_size)
        {
            return Err(CpuError::Invalid("Engram position or compressed token ID"));
        }
        let end = start_position
            .checked_add(ids.len())
            .ok_or(CpuError::Invalid("Engram position overflow"))?;
        let mut history = self.history.clone();
        let mut outputs = Vec::with_capacity(ids.len());
        for id in ids {
            history.push_back(*id);
            let mut blocked = false;
            let mut tokens = [0_u64; 4];
            for (shift, token) in tokens.iter_mut().enumerate() {
                let source = history
                    .len()
                    .checked_sub(shift + 1)
                    .and_then(|i| history.get(i))
                    .copied()
                    .flatten();
                blocked |= source.is_none();
                *token = if blocked {
                    self.pad_id as u64
                } else {
                    source.unwrap_or(self.pad_id) as u64
                };
            }
            let mut layers = Vec::with_capacity(self.layout.layers.len());
            for layer in &self.layout.layers {
                let mut rolling = tokens[0] * layer.multipliers[0];
                let mut hashes = Vec::with_capacity(layer.primes.len());
                for gram in 1..4 {
                    rolling ^= tokens[gram] * layer.multipliers[gram];
                    for head in 0..self.layout.heads {
                        let col = (gram - 1) * self.layout.heads + head;
                        hashes.push(rolling % layer.primes[col] + layer.offsets[col]);
                    }
                }
                layers.push(hashes);
            }
            outputs.push(TokenHashes { layers });
            if history.len() == self.layout.max_ngram_size {
                history.pop_front();
            }
        }
        self.history = history;
        self.next_position = end;
        Ok(outputs)
    }

    pub fn push_tokens(
        &mut self,
        start_position: usize,
        tokens: &[u32],
        token_mask: Option<&[bool]>,
        map: &CompressedTokenMap,
    ) -> Result<Vec<TokenHashes>, CpuError> {
        if map.vocab_size() != self.layout.compressed_vocab_size
            || token_mask.is_some_and(|m| m.len() != tokens.len())
        {
            return Err(CpuError::Invalid("Engram token map or mask dimensions"));
        }
        let mut compressed = Vec::with_capacity(tokens.len());
        for (i, token) in tokens.iter().enumerate() {
            let id = map.get(*token)?;
            compressed.push(if token_mask.is_none_or(|mask| mask[i]) {
                Some(id)
            } else {
                None
            });
        }
        self.push_compressed(start_position, &compressed)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EngramGate {
    pub streams: usize,
    pub width: usize,
    pub epsilon: f32,
}

impl EngramGate {
    pub fn flash() -> Self {
        Self {
            streams: 4,
            width: 5120,
            epsilon: 1e-20,
        }
    }
    fn validate(&self, residual: &[f32]) -> Result<usize, CpuError> {
        if self.streams == 0 || self.width == 0 || !positive(self.epsilon) {
            return Err(CpuError::Invalid("Engram gate dimensions or epsilon"));
        }
        let count = product(self.streams, self.width)?;
        shape("Engram residual", residual, count)?;
        Ok(count)
    }

    /// Normalize each stream separately, then signed sqrt(clamp(abs(dot),1e-6))
    /// and sigmoid. All streams receive the same value with different gates.
    pub fn apply(
        &self,
        residual: &[f32],
        keys: &[f32],
        value: &[f32],
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
    ) -> Result<Vec<f32>, CpuError> {
        self.apply_impl(residual, keys, value, q_weight, k_weight, enabled, None)
    }
    fn apply_impl(
        &self,
        residual: &[f32],
        keys: &[f32],
        value: &[f32],
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
        reference_rows: Option<usize>,
    ) -> Result<Vec<f32>, CpuError> {
        let count = self.validate(residual)?;
        shape("Engram keys", keys, count)?;
        shape("Engram value", value, self.width)?;
        shape("Engram query weight", q_weight, count)?;
        shape("Engram key weight", k_weight, count)?;
        if !enabled {
            return Ok(residual.to_vec());
        }
        let mut output = residual.to_vec();
        for h in 0..self.streams {
            let start = h * self.width;
            let x = &residual[start..start + self.width];
            let key = &keys[start..start + self.width];
            let mean = |values: &[f32]| -> Result<f32, CpuError> {
                if let Some(rows) = reference_rows {
                    super::reference_row_mean(
                        &values.iter().map(|v| v * v).collect::<Vec<_>>(),
                        rows * self.streams,
                    )
                } else {
                    Ok(values.iter().map(|v| v * v).sum::<f32>() / self.width as f32)
                }
            };
            let xvar = mean(x)?;
            let kvar = mean(key)?;
            let rstd = (xvar + self.epsilon).sqrt().recip() * (kvar + self.epsilon).sqrt().recip();
            let terms: Vec<_> = x
                .iter()
                .zip(key)
                .enumerate()
                .map(|(d, (x, k))| x * (q_weight[start + d] * k_weight[start + d]) * k)
                .collect();
            let dot = if let Some(rows) = reference_rows {
                super::reference_row_sum(&terms, rows * self.streams)?
            } else {
                terms.iter().sum::<f32>()
            };
            let dot = dot * rstd * (self.width as f64).powf(-0.5) as f32;
            if !xvar.is_finite() || !kvar.is_finite() || !dot.is_finite() {
                return Err(CpuError::NonFinite("Engram normalized dot product"));
            }
            let gate = sigmoid(dot.abs().max(1e-6).sqrt().copysign(dot));
            for d in 0..self.width {
                output[start + d] += gate * value[d];
            }
        }
        finite("Engram output", &output)?;
        Ok(output)
    }

    /// Fetch only the requested hash rows and project them into keys and value.
    /// A masked token bypasses lookup and projection entirely.
    pub fn lookup_and_apply(
        &self,
        residual: &[f32],
        hash_ids: &[u64],
        table: &dyn MatrixRows,
        projection: &dyn MatrixRows,
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
    ) -> Result<Vec<f32>, CpuError> {
        self.lookup_and_apply_with_arithmetic(
            residual,
            hash_ids,
            table,
            projection,
            q_weight,
            k_weight,
            enabled,
            super::quantization::Arithmetic::Float32,
        )
    }

    pub fn lookup_and_apply_with_arithmetic(
        &self,
        residual: &[f32],
        hash_ids: &[u64],
        table: &dyn MatrixRows,
        projection: &dyn MatrixRows,
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
        arithmetic: super::quantization::Arithmetic,
    ) -> Result<Vec<f32>, CpuError> {
        self.lookup_and_apply_with_reference_rows(
            residual, hash_ids, table, projection, q_weight, k_weight, enabled, arithmetic, 1,
        )
    }
    pub fn lookup_and_apply_with_reference_rows(
        &self,
        residual: &[f32],
        hash_ids: &[u64],
        table: &dyn MatrixRows,
        projection: &dyn MatrixRows,
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
        arithmetic: super::quantization::Arithmetic,
        reference_rows: usize,
    ) -> Result<Vec<f32>, CpuError> {
        self.lookup_and_apply_with_row_observer(
            residual,
            hash_ids,
            table,
            projection,
            q_weight,
            k_weight,
            enabled,
            arithmetic,
            reference_rows,
            &mut |_| Ok(()),
        )
    }
    /// Borrows the dequantized table rows before their projection.
    pub fn lookup_and_apply_with_row_observer(
        &self,
        residual: &[f32],
        hash_ids: &[u64],
        table: &dyn MatrixRows,
        projection: &dyn MatrixRows,
        q_weight: &[f32],
        k_weight: &[f32],
        enabled: bool,
        arithmetic: super::quantization::Arithmetic,
        reference_rows: usize,
        observer: &mut dyn FnMut(&[f32]) -> Result<(), CpuError>,
    ) -> Result<Vec<f32>, CpuError> {
        use super::quantization;
        let count = self.validate(residual)?;
        shape("Engram query weight", q_weight, count)?;
        shape("Engram key weight", k_weight, count)?;
        if !enabled {
            return Ok(residual.to_vec());
        }
        let (rows, width) = table.dimensions();
        let projection_rows = count
            .checked_add(self.width)
            .ok_or(CpuError::Invalid("Engram shape overflow"))?;
        if hash_ids.is_empty()
            || width == 0
            || projection.dimensions() != (projection_rows, product(hash_ids.len(), width)?)
            || hash_ids.iter().any(|i| *i >= rows as u64)
        {
            return Err(CpuError::Invalid("Engram lookup dimensions or row IDs"));
        }
        let mut embedding = Vec::with_capacity(product(hash_ids.len(), width)?);
        for id in hash_ids {
            let row = table.row(*id as usize)?;
            shape("Engram table row", &row, width)?;
            embedding.extend(if arithmetic.is_bf16() {
                quantization::round_bf16(&row)?
            } else {
                row
            });
        }
        observer(&embedding)?;
        let projected = quantization::linear(projection, &embedding, arithmetic)?;
        let output = self.apply_impl(
            residual,
            &projected[..count],
            &projected[count..],
            q_weight,
            k_weight,
            true,
            (arithmetic == super::quantization::Arithmetic::ReferenceBf16Tp8)
                .then_some(reference_rows),
        )?;
        if arithmetic.is_bf16() {
            quantization::round_bf16(&output)
        } else {
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deepseek_v41_cpu::MatrixRef;
    use serde::Deserialize;
    use std::cell::RefCell;

    #[derive(Deserialize)]
    struct Fixture {
        primes: Vec<Vec<u64>>,
        multipliers: Vec<[u64; 4]>,
        compressed: Vec<Option<u32>>,
        hashes: Vec<Vec<Vec<u64>>>,
        residual: Vec<f32>,
        keys: Vec<f32>,
        value: Vec<f32>,
        q_weight: Vec<f32>,
        k_weight: Vec<f32>,
        output: Vec<f32>,
    }
    fn fixture() -> Result<Fixture, serde_json::Error> {
        serde_json::from_str(include_str!("../testdata/deepseek_v41_cpu/engram.json"))
    }

    #[test]
    fn normalization_keys_match_reference_rules() -> Result<(), CpuError> {
        let n = normalizer()?;
        for text in [" The", "the", "THE", "ＴＨＥ", " thé "] {
            assert_eq!(compression_key(&n, text, text)?, "the");
        }
        assert_eq!(compression_key(&n, " \t\r\n ", "raw")?, " ");
        assert_eq!(compression_key(&n, "A\t B\r\nC", "raw")?, "a b c");
        assert_eq!(compression_key(&n, "\u{fffd}", "<0xC3>")?, "<0xC3>");
        assert_eq!(compression_key(&n, "\u{0301}", "raw")?, "\u{0301}");
        Ok(())
    }

    #[test]
    fn token_map_uses_first_seen_ids_and_checks_vocabulary()
    -> Result<(), Box<dyn std::error::Error>> {
        let tokenizer = Tokenizer::from_bytes(br#"{"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":{"[UNK]":0," The":1,"the":2,"THE":3," ":4},"unk_token":"[UNK]"}}"#).map_err(|e| CpuError::Tokenization(e.to_string()))?;
        let map = CompressedTokenMap::build(&tokenizer, 3)?;
        assert_eq!(map.ids(), &[0, 1, 1, 1, 2]);
        assert!(map.get(5).is_err());
        assert!(CompressedTokenMap::build(&tokenizer, 4).is_err());
        Ok(())
    }

    #[test]
    fn pinned_prime_layout_and_numpy_hashes_match() -> Result<(), Box<dyn std::error::Error>> {
        let f = fixture()?;
        let layout = Arc::new(EngramLayout::flash()?);
        assert_eq!(
            layout
                .layers
                .iter()
                .map(|l| l.table_rows)
                .collect::<Vec<_>>(),
            vec![384006168, 384016682]
        );
        for (i, layer) in layout.layers.iter().enumerate() {
            assert_eq!(layer.primes, f.primes[i]);
            assert_eq!(layer.multipliers, f.multipliers[i]);
        }
        let mut state = NgramHashState::new(layout.clone(), 2)?;
        let hashes = state.push_compressed(0, &f.compressed)?;
        assert_eq!(
            hashes.iter().map(|h| h.layers.clone()).collect::<Vec<_>>(),
            f.hashes
        );
        for token in hashes {
            for (layer, ids) in layout.layers.iter().zip(token.layers) {
                for (i, id) in ids.iter().enumerate() {
                    assert!(*id >= layer.offsets[i] && *id < layer.offsets[i] + layer.primes[i]);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn hash_chunks_mask_boundaries_reset_and_failure_are_isolated() -> Result<(), CpuError> {
        let layout = Arc::new(EngramLayout::flash()?);
        let input = [Some(4), Some(5), None, Some(6), Some(7), Some(8)];
        let mut whole = NgramHashState::new(layout.clone(), 2)?;
        let expected = whole.push_compressed(0, &input)?;
        let mut chunked = NgramHashState::new(layout.clone(), 2)?;
        let mut actual = chunked.push_compressed(0, &input[..2])?;
        assert!(chunked.push_compressed(2, &[Some(99092)]).is_err());
        assert_eq!(chunked.next_position(), 2);
        assert!(chunked.push_compressed(1, &input[2..]).is_err());
        actual.extend(chunked.push_compressed(2, &input[2..])?);
        assert_eq!(actual, expected);
        let mut fresh = NgramHashState::new(layout, 2)?;
        assert_eq!(fresh.push_compressed(0, &input[3..])?, expected[3..]);
        chunked.reset();
        assert_eq!(chunked.next_position(), 0);
        assert_eq!(chunked.push_compressed(0, &input)?, expected);
        Ok(())
    }

    #[test]
    fn raw_tokens_apply_mask_after_lookup_and_fail_without_advancing() -> Result<(), CpuError> {
        let map = CompressedTokenMap {
            ids: vec![0, 1, 2],
            vocab_size: 3,
        };
        let layout = Arc::new(EngramLayout {
            compressed_vocab_size: 3,
            ..EngramLayout::flash()?
        });
        let mut raw = NgramHashState::new(layout.clone(), 2)?;
        let mut direct = NgramHashState::new(layout, 2)?;
        assert_eq!(
            raw.push_tokens(0, &[0, 1, 2], Some(&[true, false, true]), &map)?,
            direct.push_compressed(0, &[Some(0), None, Some(2)])?
        );
        assert!(
            raw.push_tokens(3, &[u32::MAX], Some(&[false]), &map)
                .is_err()
        );
        assert_eq!(raw.next_position(), 3);
        assert!(raw.push_tokens(3, &[0], Some(&[]), &map).is_err());
        assert_eq!(raw.next_position(), 3);
        Ok(())
    }

    #[test]
    fn gated_residual_matches_numpy_per_stream_norm() -> Result<(), Box<dyn std::error::Error>> {
        let f = fixture()?;
        let gate = EngramGate {
            streams: 2,
            width: 3,
            epsilon: 1e-20,
        };
        let y = gate.apply(
            &f.residual,
            &f.keys,
            &f.value,
            &f.q_weight,
            &f.k_weight,
            true,
        )?;
        for (x, y) in y.iter().zip(&f.output) {
            assert!((x - y).abs() < 2e-6, "{x} != {y}");
        }
        assert_eq!(
            gate.apply(
                &f.residual,
                &f.keys,
                &f.value,
                &f.q_weight,
                &f.k_weight,
                false
            )?,
            f.residual
        );
        Ok(())
    }

    struct Table {
        calls: RefCell<Vec<usize>>,
    }
    impl MatrixRows for Table {
        fn dimensions(&self) -> (usize, usize) {
            (384006168, 1)
        }
        fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
            self.calls.borrow_mut().push(index);
            Ok(vec![(index % 3) as f32])
        }
    }

    #[test]
    fn lookup_reads_only_requested_rows_and_mask_bypasses_lookup() -> Result<(), CpuError> {
        let table = Table {
            calls: RefCell::new(Vec::new()),
        };
        let gate = EngramGate {
            streams: 1,
            width: 1,
            epsilon: 1e-20,
        };
        let projection = MatrixRef::new(2, 2, &[1.0, 0.0, 0.0, 1.0])?;
        let _ =
            gate.lookup_and_apply(&[1.0], &[4, 5], &table, &projection, &[1.0], &[1.0], true)?;
        assert_eq!(*table.calls.borrow(), vec![4, 5]);
        let _ =
            gate.lookup_and_apply(&[1.0], &[6, 7], &table, &projection, &[1.0], &[1.0], false)?;
        assert_eq!(*table.calls.borrow(), vec![4, 5]);
        assert!(
            gate.lookup_and_apply(
                &[1.0],
                &[u64::MAX, 5],
                &table,
                &projection,
                &[1.0],
                &[1.0],
                true
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn zero_dot_uses_signed_sqrt_floor_and_invalid_input_is_refused() -> Result<(), CpuError> {
        let gate = EngramGate {
            streams: 1,
            width: 1,
            epsilon: 1e-20,
        };
        let y = gate.apply(&[0.0], &[0.0], &[1.0], &[1.0], &[1.0], true)?;
        assert!((y[0] - sigmoid(0.001)).abs() < 1e-7);
        assert!(
            gate.apply(&[f32::NAN], &[0.0], &[1.0], &[1.0], &[1.0], true)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn bf16_lookup_projection_and_residual_boundaries_are_connected() -> Result<(), CpuError> {
        use super::super::quantization::{Arithmetic, MatrixPrecision, round_bf16};
        struct Matrix {
            projection: bool,
        }
        impl MatrixRows for Matrix {
            fn dimensions(&self) -> (usize, usize) {
                if self.projection { (160, 96) } else { (64, 4) }
            }
            fn precision(&self) -> MatrixPrecision {
                MatrixPrecision::Fp8
            }
            fn row(&self, _: usize) -> Result<Vec<f32>, CpuError> {
                Ok(if self.projection {
                    vec![0.03125; 96]
                } else {
                    vec![0.125, 0.25, 0.5, 1.0]
                })
            }
        }
        let gate = EngramGate {
            streams: 4,
            width: 32,
            epsilon: 1e-20,
        };
        let residual: Vec<_> = (0..4).flat_map(|h| vec![(h + 1) as f32; 32]).collect();
        let output = gate.lookup_and_apply_with_arithmetic(
            &residual,
            &[0; 24],
            &Matrix { projection: false },
            &Matrix { projection: true },
            &[1.0; 128],
            &[1.0; 128],
            true,
            Arithmetic::ReferenceBf16,
        )?;
        let g = super::sigmoid((32_f64.powf(0.25)) as f32);
        let expected = round_bf16(&residual.iter().map(|x| x + g * 1.40625).collect::<Vec<_>>())?;
        assert_eq!(output, expected);
        Ok(())
    }
}
