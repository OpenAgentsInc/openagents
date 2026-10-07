use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_SUPERVISION_DENSITY_CANON_ID: &str =
    "psionic.tassadar_supervision_density_canon.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSupervisionDensityRegime {
    FullTrace,
    PartialState,
    InvariantOnly,
    IoOnly,
    Mixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityWorkloadCase {
    pub workload_family: String,
    pub regimes: Vec<TassadarSupervisionDensityRegime>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSupervisionDensityCanon {
    pub canon_id: String,
    pub workload_cases: Vec<TassadarSupervisionDensityWorkloadCase>,
    pub canon_digest: String,
}

#[must_use]
pub fn tassadar_supervision_density_canon() -> TassadarSupervisionDensityCanon {
    let mut canon = TassadarSupervisionDensityCanon {
        canon_id: String::from(TASSADAR_SUPERVISION_DENSITY_CANON_ID),
        workload_cases: vec![
            TassadarSupervisionDensityWorkloadCase {
                workload_family: String::from("clrs_shortest_path"),
                regimes: all_regimes(),
                note: String::from("CLRS shortest-path remains the shared hint-regime anchor"),
            },
            TassadarSupervisionDensityWorkloadCase {
                workload_family: String::from("arithmetic_multi_operand"),
                regimes: all_regimes(),
                note: String::from("arithmetic reveals where dense hints stop being necessary"),
            },
            TassadarSupervisionDensityWorkloadCase {
                workload_family: String::from("sudoku_backtracking_search"),
                regimes: all_regimes(),
                note: String::from(
                    "search/backtracking reveals where sparse supervision still fails",
                ),
            },
            TassadarSupervisionDensityWorkloadCase {
                workload_family: String::from("module_scale_wasm_loop"),
                regimes: all_regimes(),
                note: String::from("module-scale Wasm keeps learned bounded success explicit"),
            },
        ],
        canon_digest: String::new(),
    };
    canon.canon_digest = stable_digest(b"psionic_tassadar_supervision_density_canon|", &canon);
    canon
}

fn all_regimes() -> Vec<TassadarSupervisionDensityRegime> {
    vec![
        TassadarSupervisionDensityRegime::FullTrace,
        TassadarSupervisionDensityRegime::PartialState,
        TassadarSupervisionDensityRegime::InvariantOnly,
        TassadarSupervisionDensityRegime::IoOnly,
        TassadarSupervisionDensityRegime::Mixed,
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
    use super::{TassadarSupervisionDensityRegime, tassadar_supervision_density_canon};

    #[test]
    fn supervision_density_canon_is_machine_legible() {
        let canon = tassadar_supervision_density_canon();

        assert_eq!(canon.workload_cases.len(), 4);
        assert!(canon.workload_cases.iter().all(|case| {
            case.regimes
                .contains(&TassadarSupervisionDensityRegime::FullTrace)
        }));
    }
}
