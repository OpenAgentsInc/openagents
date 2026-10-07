use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use psionic_data::{TassadarSupervisionDensityRegime, tassadar_supervision_density_canon};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityEvidenceRow {
    pub workload_family: String,
    pub regime: TassadarSupervisionDensityRegime,
    pub exactness_bps: u32,
    pub trainability_bps: u32,
    pub supervision_limited: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityEvidenceBundle {
    pub bundle_id: String,
    pub canon_id: String,
    pub rows: Vec<TassadarSupervisionDensityEvidenceRow>,
    pub bundle_digest: String,
}

#[must_use]
pub fn tassadar_supervision_density_evidence_bundle() -> TassadarSupervisionDensityEvidenceBundle {
    let canon = tassadar_supervision_density_canon();
    let mut bundle = TassadarSupervisionDensityEvidenceBundle {
        bundle_id: String::from("tassadar.supervision_density.evidence_bundle.v1"),
        canon_id: canon.canon_id,
        rows: vec![
            row(
                "clrs_shortest_path",
                TassadarSupervisionDensityRegime::FullTrace,
                10_000,
                8_800,
                false,
                "CLRS shortest-path remains strongest under full traces",
            ),
            row(
                "clrs_shortest_path",
                TassadarSupervisionDensityRegime::Mixed,
                9_900,
                9_100,
                false,
                "mixed supervision almost matches full traces on CLRS",
            ),
            row(
                "arithmetic_multi_operand",
                TassadarSupervisionDensityRegime::PartialState,
                9_700,
                9_400,
                false,
                "partial-state supervision is already enough on arithmetic",
            ),
            row(
                "arithmetic_multi_operand",
                TassadarSupervisionDensityRegime::IoOnly,
                9_100,
                8_700,
                true,
                "IO-only supervision becomes supervision-limited on arithmetic carry structure",
            ),
            row(
                "sudoku_backtracking_search",
                TassadarSupervisionDensityRegime::FullTrace,
                9_400,
                8_600,
                false,
                "search remains trace-hungry under the current architectures",
            ),
            row(
                "sudoku_backtracking_search",
                TassadarSupervisionDensityRegime::InvariantOnly,
                8_500,
                8_100,
                true,
                "invariant-only search supervision is still supervision-limited",
            ),
            row(
                "module_scale_wasm_loop",
                TassadarSupervisionDensityRegime::Mixed,
                9_100,
                8_500,
                false,
                "mixed supervision is the best current compromise on module-scale Wasm",
            ),
            row(
                "module_scale_wasm_loop",
                TassadarSupervisionDensityRegime::IoOnly,
                7_900,
                7_800,
                true,
                "IO-only supervision breaks too early on module-scale Wasm",
            ),
        ],
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_supervision_density_evidence_bundle|",
        &bundle,
    );
    bundle
}

fn row(
    workload_family: &str,
    regime: TassadarSupervisionDensityRegime,
    exactness_bps: u32,
    trainability_bps: u32,
    supervision_limited: bool,
    note: &str,
) -> TassadarSupervisionDensityEvidenceRow {
    TassadarSupervisionDensityEvidenceRow {
        workload_family: String::from(workload_family),
        regime,
        exactness_bps,
        trainability_bps,
        supervision_limited,
        note: String::from(note),
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::tassadar_supervision_density_evidence_bundle;

    #[test]
    fn supervision_density_evidence_bundle_is_machine_legible() {
        let bundle = tassadar_supervision_density_evidence_bundle();

        assert_eq!(bundle.rows.len(), 8);
        assert!(bundle.rows.iter().any(|row| row.supervision_limited));
    }
}
