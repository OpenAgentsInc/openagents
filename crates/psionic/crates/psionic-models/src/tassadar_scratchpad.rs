use psionic_ir::{
    format_tassadar_sequence_with_scratchpad, TassadarControlledPositionScheme,
    TassadarScratchpadEncoding, TassadarScratchpadFormatConfig,
    TassadarScratchpadFormattedSequence, TassadarScratchpadSegmentKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Workload family used by the scratchpad-framework locality inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarScratchpadWorkloadFamily {
    Arithmetic,
    Algorithmic,
}

impl TassadarScratchpadWorkloadFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Arithmetic => "arithmetic",
            Self::Algorithmic => "algorithmic",
        }
    }
}

/// Public framework config surfaced to bounded executor-model reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadPositionFramework {
    pub framework_id: String,
    pub workload_family: TassadarScratchpadWorkloadFamily,
    pub format: TassadarScratchpadFormatConfig,
    pub claim_boundary: String,
    pub framework_digest: String,
}

impl TassadarScratchpadPositionFramework {
    /// Creates one bounded scratchpad-position framework.
    #[must_use]
    pub fn new(
        framework_id: impl Into<String>,
        workload_family: TassadarScratchpadWorkloadFamily,
        format: TassadarScratchpadFormatConfig,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let mut framework = Self {
            framework_id: framework_id.into(),
            workload_family,
            format,
            claim_boundary: claim_boundary.into(),
            framework_digest: String::new(),
        };
        framework.framework_digest = stable_digest(
            b"psionic_tassadar_scratchpad_position_framework|",
            &framework,
        );
        framework
    }
}

/// Locality evidence extracted from one formatted sequence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarScratchpadLocalityEvidence {
    pub token_count: u32,
    pub output_token_count: u32,
    pub max_output_local_position_index: u32,
    pub mean_output_local_position_index: u32,
    pub position_reset_count: u32,
    pub scratchpad_overhead_bps: u32,
    pub final_output_tokens_preserved: bool,
}

/// Formats one prompt-plus-target sequence and inspects its locality properties.
#[must_use]
pub fn inspect_tassadar_scratchpad_framework(
    prompt_tokens: &[String],
    target_tokens: &[String],
    framework: &TassadarScratchpadPositionFramework,
) -> (
    TassadarScratchpadFormattedSequence,
    TassadarScratchpadLocalityEvidence,
) {
    let formatted =
        format_tassadar_sequence_with_scratchpad(prompt_tokens, target_tokens, &framework.format);
    let output_tokens = formatted
        .tokens
        .iter()
        .filter(|token| token.segment_kind == TassadarScratchpadSegmentKind::Output)
        .collect::<Vec<_>>();
    let output_local_positions = output_tokens
        .iter()
        .map(|token| output_local_position_index(token.controlled_position_id, &formatted))
        .collect::<Vec<_>>();
    let output_token_count = output_tokens.len() as u32;
    let max_output_local_position_index = output_local_positions.iter().copied().max().unwrap_or(0);
    let mean_output_local_position_index = if output_local_positions.is_empty() {
        0
    } else {
        output_local_positions
            .iter()
            .map(|position| u64::from(*position))
            .sum::<u64>() as u32
            / output_local_positions.len() as u32
    };
    let recovered_output_tokens = output_tokens
        .iter()
        .map(|token| token.token.clone())
        .collect::<Vec<_>>();
    let scratchpad_overhead_bps = if target_tokens.is_empty() {
        0
    } else {
        formatted
            .tokens
            .len()
            .saturating_sub(prompt_tokens.len() + target_tokens.len())
            .saturating_mul(10_000)
            / (prompt_tokens.len() + target_tokens.len()).max(1)
    } as u32;
    let evidence = TassadarScratchpadLocalityEvidence {
        token_count: formatted.tokens.len() as u32,
        output_token_count,
        max_output_local_position_index,
        mean_output_local_position_index,
        position_reset_count: formatted.position_reset_count,
        scratchpad_overhead_bps,
        final_output_tokens_preserved: recovered_output_tokens == target_tokens,
    };
    (formatted, evidence)
}

/// Returns the current public framework variants for arithmetic and algorithmic traces.
#[must_use]
pub fn tassadar_scratchpad_position_frameworks() -> Vec<TassadarScratchpadPositionFramework> {
    let claim_boundary = "bounded formatting and locality-inspection framework only; defines how prompt/target traces may be segmented and re-indexed for learned executor experiments, not a served or exact execution claim";
    vec![
        TassadarScratchpadPositionFramework::new(
            "tassadar.scratchpad.flat.absolute.v0",
            TassadarScratchpadWorkloadFamily::Arithmetic,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::FlatTrace,
                TassadarControlledPositionScheme::AbsoluteMonotonic,
                4,
            ),
            claim_boundary,
        ),
        TassadarScratchpadPositionFramework::new(
            "tassadar.scratchpad.chunk.segment_reset.v0",
            TassadarScratchpadWorkloadFamily::Arithmetic,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::SegmentReset,
                4,
            ),
            claim_boundary,
        ),
        TassadarScratchpadPositionFramework::new(
            "tassadar.scratchpad.chunk.trace_schema_buckets.v0",
            TassadarScratchpadWorkloadFamily::Algorithmic,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::TraceSchemaBuckets,
                4,
            ),
            claim_boundary,
        ),
    ]
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("scratchpad position framework should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn output_local_position_index(
    controlled_position_id: u32,
    formatted: &TassadarScratchpadFormattedSequence,
) -> u32 {
    match formatted.config.position_scheme {
        TassadarControlledPositionScheme::AbsoluteMonotonic
        | TassadarControlledPositionScheme::SegmentReset => controlled_position_id,
        TassadarControlledPositionScheme::TraceSchemaBuckets => {
            controlled_position_id.saturating_sub(128)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        inspect_tassadar_scratchpad_framework, TassadarScratchpadPositionFramework,
        TassadarScratchpadWorkloadFamily,
    };
    use psionic_ir::{
        TassadarControlledPositionScheme, TassadarScratchpadEncoding,
        TassadarScratchpadFormatConfig,
    };

    #[test]
    fn scratchpad_framework_reduces_output_position_span_under_segment_reset() {
        let prompt_tokens = vec![String::from("<program>")];
        let target_tokens = vec![
            String::from("t0"),
            String::from("t1"),
            String::from("t2"),
            String::from("t3"),
            String::from("t4"),
            String::from("t5"),
        ];
        let baseline = TassadarScratchpadPositionFramework::new(
            "baseline",
            TassadarScratchpadWorkloadFamily::Arithmetic,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::FlatTrace,
                TassadarControlledPositionScheme::AbsoluteMonotonic,
                4,
            ),
            "test boundary",
        );
        let segment_reset = TassadarScratchpadPositionFramework::new(
            "segment_reset",
            TassadarScratchpadWorkloadFamily::Arithmetic,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::SegmentReset,
                4,
            ),
            "test boundary",
        );

        let (_, baseline_evidence) =
            inspect_tassadar_scratchpad_framework(&prompt_tokens, &target_tokens, &baseline);
        let (_, framework_evidence) =
            inspect_tassadar_scratchpad_framework(&prompt_tokens, &target_tokens, &segment_reset);
        assert!(framework_evidence.final_output_tokens_preserved);
        assert!(
            framework_evidence.max_output_local_position_index
                < baseline_evidence.max_output_local_position_index
        );
        assert!(framework_evidence.position_reset_count > 0);
    }
}
