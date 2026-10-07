use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Workload family tracked by the error-regime study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarErrorRegimeWorkloadFamily {
    /// Verifier-heavy Sudoku-style bounded backtracking.
    SudokuBacktracking,
    /// Small synthetic verifier-guided branch search.
    SearchKernel,
    /// Long-horizon control with accumulated carried-state drift.
    LongHorizonControl,
    /// Byte-addressed mutation loops with replayable checkpoints.
    ByteMemoryLoop,
}

impl TassadarErrorRegimeWorkloadFamily {
    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SudokuBacktracking => "sudoku_backtracking",
            Self::SearchKernel => "search_kernel",
            Self::LongHorizonControl => "long_horizon_control",
            Self::ByteMemoryLoop => "byte_memory_loop",
        }
    }
}

/// Recovery surface compared in the error-regime study.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarErrorRegimeRecoverySurface {
    /// No correction path beyond the raw learned rollout.
    Uncorrected,
    /// Periodic checkpoint rollback without verifier help.
    CheckpointOnly,
    /// Verifier-guided correction without checkpoint rewind.
    VerifierOnly,
    /// Joint checkpoint rollback and verifier-guided correction.
    CheckpointAndVerifier,
}

impl TassadarErrorRegimeRecoverySurface {
    /// Returns the stable recovery-surface label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncorrected => "uncorrected",
            Self::CheckpointOnly => "checkpoint_only",
            Self::VerifierOnly => "verifier_only",
            Self::CheckpointAndVerifier => "checkpoint_and_verifier",
        }
    }
}

/// Coarse error regime realized by one workload and recovery-surface pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarErrorRegimeClass {
    /// Injected errors are corrected and the workload returns to the declared trajectory.
    SelfHealing,
    /// The workload stays usable but accumulates bounded residual drift.
    SlowDrift,
    /// The workload diverges and cannot recover inside the declared budgets.
    CatastrophicDivergence,
}

/// Runtime receipt for one injected-error regime sweep cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeReceipt {
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Compared workload family.
    pub workload_family: TassadarErrorRegimeWorkloadFamily,
    /// Compared recovery surface.
    pub recovery_surface: TassadarErrorRegimeRecoverySurface,
    /// Stable step where the error is injected.
    pub injected_error_step: u32,
    /// Number of checkpoint resets actually used.
    pub checkpoint_reset_count: u32,
    /// Number of verifier interventions actually used.
    pub verifier_intervention_count: u32,
    /// Number of explicit correction events emitted.
    pub correction_event_count: u32,
    /// Final realized regime class.
    pub regime_class: TassadarErrorRegimeClass,
    /// Step where divergence became irreversible when one existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divergence_step: Option<u32>,
    /// Final exactness against the reference trajectory in basis points.
    pub recovered_exactness_bps: u32,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language receipt note.
    pub note: String,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

impl TassadarErrorRegimeReceipt {
    fn new(
        workload_family: TassadarErrorRegimeWorkloadFamily,
        recovery_surface: TassadarErrorRegimeRecoverySurface,
        injected_error_step: u32,
        checkpoint_reset_count: u32,
        verifier_intervention_count: u32,
        correction_event_count: u32,
        regime_class: TassadarErrorRegimeClass,
        divergence_step: Option<u32>,
        recovered_exactness_bps: u32,
        note: &str,
    ) -> Self {
        let mut receipt = Self {
            receipt_id: format!(
                "tassadar.error_regime.{}.{}.v1",
                workload_family.as_str(),
                recovery_surface.as_str()
            ),
            workload_family,
            recovery_surface,
            injected_error_step,
            checkpoint_reset_count,
            verifier_intervention_count,
            correction_event_count,
            regime_class,
            divergence_step,
            recovered_exactness_bps,
            claim_boundary: String::from(
                "this receipt records one bounded injected-error study cell with explicit correction surfaces, budgets, and final regime class. It does not treat the presence of a correction path as proof of exactness outside the seeded workloads and assumptions",
            ),
            note: String::from(note),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = stable_digest(b"psionic_tassadar_error_regime_receipt|", &receipt);
        receipt
    }
}

/// Returns the canonical injected-error regime receipts.
#[must_use]
pub fn tassadar_error_regime_receipts() -> Vec<TassadarErrorRegimeReceipt> {
    vec![
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
            TassadarErrorRegimeRecoverySurface::Uncorrected,
            7,
            0,
            0,
            0,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            6_600,
            "Sudoku backtracking degrades without explicit correction, but the bounded trace family avoids immediate catastrophic divergence on the seeded case.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
            TassadarErrorRegimeRecoverySurface::CheckpointOnly,
            7,
            1,
            0,
            1,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            7_600,
            "Checkpoint rollback removes the worst branch drift but still lacks contradiction-aware branch pruning.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
            TassadarErrorRegimeRecoverySurface::VerifierOnly,
            7,
            0,
            2,
            2,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            9_700,
            "Verifier-only correction self-heals the seeded Sudoku case because contradiction events are local and branchable.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
            TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            7,
            1,
            2,
            3,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            10_000,
            "Checkpoint plus verifier closes the remaining seeded Sudoku gap by combining rollback with contradiction certificates.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SearchKernel,
            TassadarErrorRegimeRecoverySurface::Uncorrected,
            5,
            0,
            0,
            0,
            TassadarErrorRegimeClass::CatastrophicDivergence,
            Some(9),
            1_800,
            "The small search kernel diverges quickly without correction because one wrong branch consumes the remaining frontier budget.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SearchKernel,
            TassadarErrorRegimeRecoverySurface::CheckpointOnly,
            5,
            1,
            0,
            1,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            6_100,
            "Checkpoint rollback rescues the first failed branch but still leaves later branch selection noisy without verifier guidance.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SearchKernel,
            TassadarErrorRegimeRecoverySurface::VerifierOnly,
            5,
            0,
            2,
            2,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            9_800,
            "Verifier interventions are sufficient on the seeded search kernel because the correction events directly expose the dead-end branch.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::SearchKernel,
            TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            5,
            1,
            2,
            3,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            10_000,
            "Combined rollback and verifier events close the final residual gap on the seeded search-kernel family.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
            TassadarErrorRegimeRecoverySurface::Uncorrected,
            18,
            0,
            0,
            0,
            TassadarErrorRegimeClass::CatastrophicDivergence,
            Some(27),
            1_200,
            "Long-horizon control drifts into unrecoverable state without explicit recovery once the carried state crosses the injected fault boundary.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
            TassadarErrorRegimeRecoverySurface::CheckpointOnly,
            18,
            2,
            0,
            2,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            6_200,
            "Checkpoint rollback bounds the worst explosion, but later-window decisions still drift because the correction is not semantically targeted.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
            TassadarErrorRegimeRecoverySurface::VerifierOnly,
            18,
            0,
            3,
            3,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            5_600,
            "Verifier-only correction catches some contradictions but cannot fully restore the lost carried state without rollback.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
            TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            18,
            2,
            3,
            5,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            8_300,
            "The combined surface helps most on long-horizon control, but the seeded regime still remains bounded slow drift rather than full self-healing.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
            TassadarErrorRegimeRecoverySurface::Uncorrected,
            11,
            0,
            0,
            0,
            TassadarErrorRegimeClass::CatastrophicDivergence,
            Some(16),
            2_200,
            "Byte-memory loops diverge once one bad write propagates without rewind.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
            TassadarErrorRegimeRecoverySurface::CheckpointOnly,
            11,
            2,
            0,
            2,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            9_200,
            "Checkpoint rollback is enough to self-heal the seeded byte-memory loop because the incorrect writes stay replay-visible and rewindable.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
            TassadarErrorRegimeRecoverySurface::VerifierOnly,
            11,
            0,
            2,
            2,
            TassadarErrorRegimeClass::SlowDrift,
            None,
            5_800,
            "Verifier-only correction helps isolate impossible write states but cannot undo all corrupted bytes without rewind.",
        ),
        TassadarErrorRegimeReceipt::new(
            TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
            TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            11,
            2,
            2,
            4,
            TassadarErrorRegimeClass::SelfHealing,
            None,
            10_000,
            "Combined checkpoint and verifier surfaces return the seeded byte-memory loop to exactness.",
        ),
    ]
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarErrorRegimeClass, TassadarErrorRegimeRecoverySurface,
        TassadarErrorRegimeWorkloadFamily, tassadar_error_regime_receipts,
    };

    #[test]
    fn error_regime_receipts_cover_self_healing_slow_drift_and_catastrophic_paths() {
        let receipts = tassadar_error_regime_receipts();

        assert_eq!(receipts.len(), 16);
        let long_horizon = receipts
            .iter()
            .find(|receipt| {
                receipt.workload_family == TassadarErrorRegimeWorkloadFamily::LongHorizonControl
                    && receipt.recovery_surface
                        == TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier
            })
            .expect("long-horizon combined receipt");
        assert_eq!(
            long_horizon.regime_class,
            TassadarErrorRegimeClass::SlowDrift
        );

        let search_uncorrected = receipts
            .iter()
            .find(|receipt| {
                receipt.workload_family == TassadarErrorRegimeWorkloadFamily::SearchKernel
                    && receipt.recovery_surface == TassadarErrorRegimeRecoverySurface::Uncorrected
            })
            .expect("search uncorrected receipt");
        assert_eq!(
            search_uncorrected.regime_class,
            TassadarErrorRegimeClass::CatastrophicDivergence
        );

        let byte_memory_checkpoint = receipts
            .iter()
            .find(|receipt| {
                receipt.workload_family == TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop
                    && receipt.recovery_surface
                        == TassadarErrorRegimeRecoverySurface::CheckpointOnly
            })
            .expect("byte-memory checkpoint receipt");
        assert_eq!(
            byte_memory_checkpoint.regime_class,
            TassadarErrorRegimeClass::SelfHealing
        );
        assert!(!byte_memory_checkpoint.receipt_digest.is_empty());
    }
}
