use psionic_ir::{
    TassadarControlledPositionScheme, TassadarLocalityScratchpadTraceFamily,
    TassadarScratchpadFormattedSequence, TassadarScratchpadSegmentKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// One replay receipt proving that a locality-preserving scratchpad pass left source tokens intact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadReplayReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Stable case identifier.
    pub case_id: String,
    /// Trace family covered by the receipt.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Stable source trace reference.
    pub source_trace_ref: String,
    /// Stable source trace digest.
    pub source_trace_digest: String,
    /// Stable pass identifier.
    pub pass_id: String,
    /// Stable baseline formatted-sequence digest.
    pub baseline_sequence_digest: String,
    /// Stable candidate formatted-sequence digest.
    pub candidate_sequence_digest: String,
    /// Baseline token count.
    pub baseline_token_count: u32,
    /// Candidate token count.
    pub candidate_token_count: u32,
    /// Candidate minus baseline token count.
    pub inserted_token_count: u32,
    /// Candidate overhead in basis points against baseline.
    pub scratchpad_overhead_bps: u32,
    /// Baseline max useful lookback over output tokens.
    pub baseline_max_useful_lookback: u32,
    /// Candidate max useful lookback over output tokens.
    pub candidate_max_useful_lookback: u32,
    /// Whether the recovered candidate output tokens exactly matched the source trace tokens.
    pub replay_exact: bool,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable receipt digest.
    pub receipt_digest: String,
}

impl TassadarLocalityScratchpadReplayReceipt {
    fn new(
        receipt_id: impl Into<String>,
        case_id: impl Into<String>,
        trace_family: TassadarLocalityScratchpadTraceFamily,
        source_trace_ref: impl Into<String>,
        source_trace_digest: impl Into<String>,
        pass_id: impl Into<String>,
        baseline: &TassadarScratchpadFormattedSequence,
        candidate: &TassadarScratchpadFormattedSequence,
        claim_boundary: impl Into<String>,
    ) -> Result<Self, TassadarLocalityScratchpadReplayError> {
        let source_trace_tokens = recover_output_tokens(baseline);
        if source_trace_tokens.is_empty() {
            return Err(TassadarLocalityScratchpadReplayError::MissingSourceTraceTokens);
        }
        let recovered_candidate_tokens = recover_output_tokens(candidate);
        if recovered_candidate_tokens != source_trace_tokens {
            return Err(TassadarLocalityScratchpadReplayError::CandidateReplayMismatch);
        }

        let baseline_token_count = baseline.tokens.len() as u32;
        let candidate_token_count = candidate.tokens.len() as u32;
        let inserted_token_count = candidate_token_count.saturating_sub(baseline_token_count);
        let scratchpad_overhead_bps = if baseline_token_count == 0 {
            0
        } else {
            ((u64::from(inserted_token_count) * 10_000) / u64::from(baseline_token_count)) as u32
        };
        let mut receipt = Self {
            receipt_id: receipt_id.into(),
            case_id: case_id.into(),
            trace_family,
            source_trace_ref: source_trace_ref.into(),
            source_trace_digest: source_trace_digest.into(),
            pass_id: pass_id.into(),
            baseline_sequence_digest: baseline.sequence_digest.clone(),
            candidate_sequence_digest: candidate.sequence_digest.clone(),
            baseline_token_count,
            candidate_token_count,
            inserted_token_count,
            scratchpad_overhead_bps,
            baseline_max_useful_lookback: max_output_local_position_index(baseline),
            candidate_max_useful_lookback: max_output_local_position_index(candidate),
            replay_exact: true,
            claim_boundary: claim_boundary.into(),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest =
            stable_digest(b"psionic_tassadar_locality_scratchpad_replay_receipt|", &receipt);
        Ok(receipt)
    }
}

/// Replay failure for one locality-preserving scratchpad pass artifact.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarLocalityScratchpadReplayError {
    /// The baseline sequence did not contain output tokens.
    #[error("locality-preserving scratchpad replay is missing source trace tokens")]
    MissingSourceTraceTokens,
    /// The candidate sequence changed the recovered output tokens.
    #[error("locality-preserving scratchpad candidate changed the recovered output token stream")]
    CandidateReplayMismatch,
}

/// Builds one replay receipt over a baseline/candidate locality-preserving scratchpad pair.
pub fn build_tassadar_locality_scratchpad_replay_receipt(
    receipt_id: impl Into<String>,
    case_id: impl Into<String>,
    trace_family: TassadarLocalityScratchpadTraceFamily,
    source_trace_ref: impl Into<String>,
    source_trace_digest: impl Into<String>,
    pass_id: impl Into<String>,
    baseline: &TassadarScratchpadFormattedSequence,
    candidate: &TassadarScratchpadFormattedSequence,
    claim_boundary: impl Into<String>,
) -> Result<TassadarLocalityScratchpadReplayReceipt, TassadarLocalityScratchpadReplayError> {
    TassadarLocalityScratchpadReplayReceipt::new(
        receipt_id,
        case_id,
        trace_family,
        source_trace_ref,
        source_trace_digest,
        pass_id,
        baseline,
        candidate,
        claim_boundary,
    )
}

fn recover_output_tokens(sequence: &TassadarScratchpadFormattedSequence) -> Vec<String> {
    sequence
        .tokens
        .iter()
        .filter(|token| token.segment_kind == TassadarScratchpadSegmentKind::Output)
        .map(|token| token.token.clone())
        .collect()
}

fn max_output_local_position_index(sequence: &TassadarScratchpadFormattedSequence) -> u32 {
    sequence
        .tokens
        .iter()
        .filter(|token| token.segment_kind == TassadarScratchpadSegmentKind::Output)
        .map(|token| match sequence.config.position_scheme {
            TassadarControlledPositionScheme::AbsoluteMonotonic
            | TassadarControlledPositionScheme::SegmentReset => token.controlled_position_id,
            TassadarControlledPositionScheme::TraceSchemaBuckets => {
                token.controlled_position_id.saturating_sub(128)
            }
        })
        .max()
        .unwrap_or(0)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("locality-preserving scratchpad receipt should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use psionic_ir::{
        format_tassadar_sequence_with_scratchpad, TassadarControlledPositionScheme,
        TassadarLocalityScratchpadTraceFamily, TassadarScratchpadEncoding,
        TassadarScratchpadFormatConfig,
    };

    use super::build_tassadar_locality_scratchpad_replay_receipt;

    #[test]
    fn locality_scratchpad_replay_receipt_preserves_output_tokens() {
        let prompt_tokens = vec![String::from("<program>")];
        let source_tokens = vec![
            String::from("let"),
            String::from("sum"),
            String::from("add"),
            String::from("output"),
        ];
        let baseline = format_tassadar_sequence_with_scratchpad(
            &prompt_tokens,
            &source_tokens,
            &TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::FlatTrace,
                TassadarControlledPositionScheme::AbsoluteMonotonic,
                4,
            ),
        );
        let candidate = format_tassadar_sequence_with_scratchpad(
            &prompt_tokens,
            &source_tokens,
            &TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::SegmentReset,
                4,
            ),
        );
        let receipt = build_tassadar_locality_scratchpad_replay_receipt(
            "receipt.v1",
            "symbolic_case",
            TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine,
            "symbolic://case",
            "digest",
            "pass.v1",
            &baseline,
            &candidate,
            "test boundary",
        )
        .expect("candidate should preserve output tokens");
        assert!(receipt.replay_exact);
        assert!(
            receipt.candidate_max_useful_lookback < receipt.baseline_max_useful_lookback
        );
    }
}
