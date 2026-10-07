use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    TassadarControlledPositionScheme, TassadarScratchpadEncoding, TassadarScratchpadFormatConfig,
};

/// Stable schema version for the locality-preserving scratchpad pass lane.
pub const TASSADAR_LOCALITY_SCRATCHPAD_PASS_SCHEMA_VERSION: u16 = 1;

/// Trace family supported by the locality-preserving scratchpad pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarLocalityScratchpadTraceFamily {
    /// Straight-line symbolic program traces.
    SymbolicStraightLine,
    /// Frame-aware delta-oriented module traces.
    ModuleTraceV2,
}

impl TassadarLocalityScratchpadTraceFamily {
    /// Returns the stable family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SymbolicStraightLine => "symbolic_straight_line",
            Self::ModuleTraceV2 => "module_trace_v2",
        }
    }
}

/// Public compiler-pass contract for one locality-preserving scratchpad pass.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarLocalityScratchpadPass {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable pass identifier.
    pub pass_id: String,
    /// Supported trace family.
    pub trace_family: TassadarLocalityScratchpadTraceFamily,
    /// Formatting profile applied by the pass.
    pub format: TassadarScratchpadFormatConfig,
    /// Explicit maximum scratchpad overhead admitted by the pass.
    pub max_inserted_token_overhead_bps: u32,
    /// Explicit maximum local lookback admitted by the pass target.
    pub max_output_local_position_cap: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the pass surface.
    pub pass_digest: String,
}

impl TassadarLocalityScratchpadPass {
    /// Creates one locality-preserving scratchpad pass contract.
    #[must_use]
    pub fn new(
        pass_id: impl Into<String>,
        trace_family: TassadarLocalityScratchpadTraceFamily,
        format: TassadarScratchpadFormatConfig,
        max_inserted_token_overhead_bps: u32,
        max_output_local_position_cap: u32,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let mut pass = Self {
            schema_version: TASSADAR_LOCALITY_SCRATCHPAD_PASS_SCHEMA_VERSION,
            pass_id: pass_id.into(),
            trace_family,
            format,
            max_inserted_token_overhead_bps,
            max_output_local_position_cap: max_output_local_position_cap.max(1),
            claim_boundary: claim_boundary.into(),
            pass_digest: String::new(),
        };
        pass.pass_digest = stable_digest(b"psionic_tassadar_locality_scratchpad_pass|", &pass);
        pass
    }
}

/// Returns the canonical locality-preserving scratchpad pass contracts.
#[must_use]
pub fn tassadar_locality_preserving_scratchpad_passes() -> Vec<TassadarLocalityScratchpadPass> {
    let claim_boundary = "compiler pass changes only bounded trace formatting and controlled position layout; it must preserve replayable source token truth and refuses over-budget scratchpad expansion instead of widening execution semantics";
    vec![
        TassadarLocalityScratchpadPass::new(
            "tassadar.locality.symbolic.segment_reset.v1",
            TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::SegmentReset,
                4,
            ),
            25_000,
            4,
            claim_boundary,
        ),
        TassadarLocalityScratchpadPass::new(
            "tassadar.locality.module_trace.schema_buckets.v1",
            TassadarLocalityScratchpadTraceFamily::ModuleTraceV2,
            TassadarScratchpadFormatConfig::new(
                TassadarScratchpadEncoding::DelimitedChunkScratchpad,
                TassadarControlledPositionScheme::TraceSchemaBuckets,
                6,
            ),
            25_000,
            6,
            claim_boundary,
        ),
    ]
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("locality-preserving scratchpad pass should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        tassadar_locality_preserving_scratchpad_passes, TassadarLocalityScratchpadTraceFamily,
        TASSADAR_LOCALITY_SCRATCHPAD_PASS_SCHEMA_VERSION,
    };

    #[test]
    fn locality_preserving_scratchpad_passes_are_machine_legible() {
        let passes = tassadar_locality_preserving_scratchpad_passes();
        assert_eq!(passes.len(), 2);
        assert_eq!(
            passes[0].schema_version,
            TASSADAR_LOCALITY_SCRATCHPAD_PASS_SCHEMA_VERSION
        );
        assert_eq!(
            passes[0].trace_family,
            TassadarLocalityScratchpadTraceFamily::SymbolicStraightLine
        );
        assert_eq!(
            passes[1].trace_family,
            TassadarLocalityScratchpadTraceFamily::ModuleTraceV2
        );
        assert!(passes.iter().all(|pass| !pass.pass_digest.is_empty()));
    }
}
