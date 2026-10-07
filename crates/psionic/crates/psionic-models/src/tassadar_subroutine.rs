use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_EXECUTOR_SUBROUTINE_LIBRARY_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_EXECUTOR_SUBROUTINE_LIBRARY_CLAIM_CLASS: &str = "learned_bounded_success";

/// Workload family currently covered by the public subroutine-training library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSubroutineWorkloadFamily {
    Sort,
    ClrsShortestPath,
    SudokuStyle,
}

impl TassadarExecutorSubroutineWorkloadFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sort => "sort",
            Self::ClrsShortestPath => "clrs_shortest_path",
            Self::SudokuStyle => "sudoku_style",
        }
    }
}

/// Reusable subroutine kind for the learned executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarExecutorSubroutineKind {
    CompareCandidates,
    CommitUpdate,
    AdvanceCursor,
    PruneCandidates,
}

impl TassadarExecutorSubroutineKind {
    /// Returns the stable kind label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CompareCandidates => "compare_candidates",
            Self::CommitUpdate => "commit_update",
            Self::AdvanceCursor => "advance_cursor",
            Self::PruneCandidates => "prune_candidates",
        }
    }
}

/// One public reusable subroutine entry for learned training targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineLibraryEntry {
    pub subroutine_id: String,
    pub kind: TassadarExecutorSubroutineKind,
    pub summary: String,
    pub supported_workload_families: Vec<TassadarExecutorSubroutineWorkloadFamily>,
    pub shared_across_workloads: bool,
    pub claim_boundary: String,
}

impl TassadarExecutorSubroutineLibraryEntry {
    /// Returns whether this library entry is valid for the supplied workload family.
    #[must_use]
    pub fn supports_workload(self: &Self, workload_family: TassadarExecutorSubroutineWorkloadFamily) -> bool {
        self.supported_workload_families.contains(&workload_family)
    }
}

/// One labeled subroutine target for a bounded executor-training example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineTarget {
    pub subroutine_id: String,
    pub target_label: String,
    pub workload_family: TassadarExecutorSubroutineWorkloadFamily,
}

/// Public library of reusable subroutine targets for learned executor training.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExecutorSubroutineLibrary {
    pub schema_version: u16,
    pub claim_class: String,
    pub entries: Vec<TassadarExecutorSubroutineLibraryEntry>,
    pub library_digest: String,
}

impl TassadarExecutorSubroutineLibrary {
    fn new(entries: Vec<TassadarExecutorSubroutineLibraryEntry>) -> Self {
        let mut library = Self {
            schema_version: TASSADAR_EXECUTOR_SUBROUTINE_LIBRARY_SCHEMA_VERSION,
            claim_class: String::from(TASSADAR_EXECUTOR_SUBROUTINE_LIBRARY_CLAIM_CLASS),
            entries,
            library_digest: String::new(),
        };
        library.library_digest = stable_digest(&library);
        library
    }

    /// Looks up one subroutine entry by its stable identifier.
    #[must_use]
    pub fn entry(&self, subroutine_id: &str) -> Option<&TassadarExecutorSubroutineLibraryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.subroutine_id == subroutine_id)
    }
}

/// Returns the canonical public subroutine library for bounded learned
/// executor supervision research.
#[must_use]
pub fn tassadar_executor_subroutine_library() -> TassadarExecutorSubroutineLibrary {
    TassadarExecutorSubroutineLibrary::new(vec![
        TassadarExecutorSubroutineLibraryEntry {
            subroutine_id: String::from("tassadar.subroutine.compare_candidates.v1"),
            kind: TassadarExecutorSubroutineKind::CompareCandidates,
            summary: String::from(
                "compare two candidate states and retain the better-scoring continuation",
            ),
            supported_workload_families: vec![
                TassadarExecutorSubroutineWorkloadFamily::Sort,
                TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
            ],
            shared_across_workloads: true,
            claim_boundary: String::from(
                "shared learned supervision primitive only; names one bounded comparison-style target and does not imply whole-program executor closure",
            ),
        },
        TassadarExecutorSubroutineLibraryEntry {
            subroutine_id: String::from("tassadar.subroutine.commit_update.v1"),
            kind: TassadarExecutorSubroutineKind::CommitUpdate,
            summary: String::from(
                "commit one accepted state update back into the working state",
            ),
            supported_workload_families: vec![
                TassadarExecutorSubroutineWorkloadFamily::Sort,
                TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
            ],
            shared_across_workloads: true,
            claim_boundary: String::from(
                "shared learned supervision primitive only; names one bounded commit-style update and does not imply broader execution authority",
            ),
        },
        TassadarExecutorSubroutineLibraryEntry {
            subroutine_id: String::from("tassadar.subroutine.advance_cursor.v1"),
            kind: TassadarExecutorSubroutineKind::AdvanceCursor,
            summary: String::from(
                "advance the active cursor/frontier/window to the next bounded position",
            ),
            supported_workload_families: vec![
                TassadarExecutorSubroutineWorkloadFamily::Sort,
                TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
            ],
            shared_across_workloads: true,
            claim_boundary: String::from(
                "shared learned supervision primitive only; names one bounded cursor/frontier advance target and does not imply global control-flow closure",
            ),
        },
        TassadarExecutorSubroutineLibraryEntry {
            subroutine_id: String::from("tassadar.subroutine.prune_candidates.v1"),
            kind: TassadarExecutorSubroutineKind::PruneCandidates,
            summary: String::from(
                "remove invalid candidates from the bounded working set before the next decision",
            ),
            supported_workload_families: vec![
                TassadarExecutorSubroutineWorkloadFamily::ClrsShortestPath,
                TassadarExecutorSubroutineWorkloadFamily::SudokuStyle,
            ],
            shared_across_workloads: true,
            claim_boundary: String::from(
                "shared learned supervision primitive only; names one bounded candidate-pruning target and does not imply arbitrary search or solver closure",
            ),
        },
    ])
}

fn stable_digest<T: Serialize>(value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"tassadar_executor_subroutine_library|");
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarExecutorSubroutineWorkloadFamily, tassadar_executor_subroutine_library,
    };

    #[test]
    fn subroutine_library_is_machine_legible() {
        let library = tassadar_executor_subroutine_library();
        assert_eq!(library.schema_version, 1);
        assert_eq!(library.claim_class, "learned_bounded_success");
        assert_eq!(library.entries.len(), 4);
        assert!(library
            .entry("tassadar.subroutine.commit_update.v1")
            .expect("commit_update entry")
            .supports_workload(TassadarExecutorSubroutineWorkloadFamily::SudokuStyle));
        assert!(library.entries.iter().all(|entry| entry.shared_across_workloads));
    }
}
