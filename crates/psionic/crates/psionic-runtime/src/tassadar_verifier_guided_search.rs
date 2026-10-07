use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tassadar_sudoku_v0_corpus;

/// Search workload family surfaced by the verifier-guided search trace lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVerifierGuidedSearchWorkloadFamily {
    /// Sudoku-class bounded backtracking search.
    SudokuBacktracking,
    /// Synthetic kernel-style bounded search.
    SearchKernel,
}

/// Event kind carried by the verifier-guided search trace lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarVerifierGuidedSearchEventKind {
    /// One explicit guess action.
    Guess,
    /// One verifier-accepted candidate or partial state.
    Verify,
    /// One contradiction certificate.
    Contradiction,
    /// One explicit backtrack action.
    Backtrack,
    /// One final committed solved state.
    Commit,
}

/// One machine-legible verifier certificate attached to a search event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierCertificate {
    /// Stable certificate identifier.
    pub certificate_id: String,
    /// Stable verifier-rule family.
    pub verifier_rule: String,
    /// Stable search subject reference such as a cell, branch, or frontier node.
    pub subject_ref: String,
    /// Whether the certificate proves a contradiction.
    pub contradiction_detected: bool,
}

/// Explicit search-budget contract attached to one search trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchBudget {
    /// Maximum guesses admitted by the trace.
    pub max_guesses: u32,
    /// Maximum backtracks admitted by the trace.
    pub max_backtracks: u32,
    /// Maximum total search steps admitted by the trace.
    pub max_search_steps: u32,
}

/// One event in the verifier-guided search trace lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchEvent {
    /// Step index in search order.
    pub step_index: u32,
    /// Event kind.
    pub event_kind: TassadarVerifierGuidedSearchEventKind,
    /// Current search depth after the event.
    pub depth: u32,
    /// Stable guess label when the event is a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guess_label: Option<String>,
    /// Concrete guessed value when the event is a guess.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guess_value: Option<i32>,
    /// Optional verifier certificate attached to the event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<TassadarVerifierCertificate>,
    /// Explicit backtrack target depth when the event backtracks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backtrack_to_depth: Option<u32>,
    /// Remaining guess budget after the event.
    pub remaining_guess_budget: u32,
    /// Remaining backtrack budget after the event.
    pub remaining_backtrack_budget: u32,
}

/// One machine-legible verifier-guided search trace artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarVerifierGuidedSearchTraceArtifact {
    /// Stable trace identifier.
    pub trace_id: String,
    /// Stable seeded case identifier.
    pub case_id: String,
    /// Search workload family.
    pub workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
    /// Explicit search-budget contract.
    pub budget: TassadarVerifierGuidedSearchBudget,
    /// Ordered search events.
    pub events: Vec<TassadarVerifierGuidedSearchEvent>,
    /// Final committed outputs for the trace.
    pub final_outputs: Vec<i32>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable trace digest.
    pub trace_digest: String,
}

impl TassadarVerifierGuidedSearchTraceArtifact {
    fn new(
        trace_id: impl Into<String>,
        case_id: impl Into<String>,
        workload_family: TassadarVerifierGuidedSearchWorkloadFamily,
        budget: TassadarVerifierGuidedSearchBudget,
        events: Vec<TassadarVerifierGuidedSearchEvent>,
        final_outputs: Vec<i32>,
        claim_boundary: impl Into<String>,
    ) -> Result<Self, TassadarVerifierGuidedSearchTraceError> {
        let mut artifact = Self {
            trace_id: trace_id.into(),
            case_id: case_id.into(),
            workload_family,
            budget,
            events,
            final_outputs,
            claim_boundary: claim_boundary.into(),
            trace_digest: String::new(),
        };
        artifact.validate()?;
        artifact.trace_digest =
            stable_digest(b"psionic_tassadar_verifier_guided_search_trace_artifact|", &artifact);
        Ok(artifact)
    }

    /// Validates the artifact.
    pub fn validate(&self) -> Result<(), TassadarVerifierGuidedSearchTraceError> {
        if self.trace_id.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceError::MissingTraceId);
        }
        if self.case_id.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceError::MissingCaseId);
        }
        if self.budget.max_guesses == 0 {
            return Err(TassadarVerifierGuidedSearchTraceError::InvalidGuessBudget);
        }
        if self.budget.max_backtracks == 0 {
            return Err(TassadarVerifierGuidedSearchTraceError::InvalidBacktrackBudget);
        }
        if self.budget.max_search_steps == 0 {
            return Err(TassadarVerifierGuidedSearchTraceError::InvalidSearchStepBudget);
        }
        if self.events.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceError::MissingEvents);
        }
        if self.final_outputs.is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceError::MissingFinalOutputs);
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarVerifierGuidedSearchTraceError::MissingClaimBoundary);
        }
        if self.events.len() as u32 > self.budget.max_search_steps {
            return Err(TassadarVerifierGuidedSearchTraceError::SearchStepBudgetExceeded {
                steps: self.events.len() as u32,
                budget: self.budget.max_search_steps,
            });
        }

        let mut guess_count = 0_u32;
        let mut backtrack_count = 0_u32;
        let mut current_depth = 0_u32;
        let mut previous_guess_budget = self.budget.max_guesses;
        let mut previous_backtrack_budget = self.budget.max_backtracks;
        for (expected_step_index, event) in self.events.iter().enumerate() {
            if event.step_index != expected_step_index as u32 {
                return Err(TassadarVerifierGuidedSearchTraceError::NonSequentialStepIndex {
                    expected: expected_step_index as u32,
                    actual: event.step_index,
                });
            }
            if event.remaining_guess_budget > previous_guess_budget {
                return Err(
                    TassadarVerifierGuidedSearchTraceError::GuessBudgetIncreased {
                        step_index: event.step_index,
                    },
                );
            }
            if event.remaining_backtrack_budget > previous_backtrack_budget {
                return Err(
                    TassadarVerifierGuidedSearchTraceError::BacktrackBudgetIncreased {
                        step_index: event.step_index,
                    },
                );
            }

            match event.event_kind {
                TassadarVerifierGuidedSearchEventKind::Guess => {
                    if event.guess_label.as_deref().is_none()
                        || event.guess_value.is_none()
                        || event.certificate.is_some()
                        || event.backtrack_to_depth.is_some()
                    {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::MalformedGuessEvent {
                                step_index: event.step_index,
                            },
                        );
                    }
                    guess_count = guess_count.saturating_add(1);
                    current_depth = current_depth.saturating_add(1);
                    if event.depth != current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::DepthMismatch {
                                step_index: event.step_index,
                                expected: current_depth,
                                actual: event.depth,
                            },
                        );
                    }
                }
                TassadarVerifierGuidedSearchEventKind::Verify => {
                    let Some(certificate) = &event.certificate else {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::MissingCertificate {
                                step_index: event.step_index,
                                event_kind: event.event_kind,
                            },
                        );
                    };
                    if certificate.contradiction_detected {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::VerifyCarriesContradictionCertificate {
                                step_index: event.step_index,
                            },
                        );
                    }
                    if event.depth != current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::DepthMismatch {
                                step_index: event.step_index,
                                expected: current_depth,
                                actual: event.depth,
                            },
                        );
                    }
                }
                TassadarVerifierGuidedSearchEventKind::Contradiction => {
                    let Some(certificate) = &event.certificate else {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::MissingCertificate {
                                step_index: event.step_index,
                                event_kind: event.event_kind,
                            },
                        );
                    };
                    if !certificate.contradiction_detected {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::ContradictionWithoutFlag {
                                step_index: event.step_index,
                            },
                        );
                    }
                    if event.depth != current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::DepthMismatch {
                                step_index: event.step_index,
                                expected: current_depth,
                                actual: event.depth,
                            },
                        );
                    }
                }
                TassadarVerifierGuidedSearchEventKind::Backtrack => {
                    let Some(backtrack_to_depth) = event.backtrack_to_depth else {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::MissingBacktrackTarget {
                                step_index: event.step_index,
                            },
                        );
                    };
                    if backtrack_to_depth >= current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::BacktrackDoesNotReduceDepth {
                                step_index: event.step_index,
                                current_depth,
                                backtrack_to_depth,
                            },
                        );
                    }
                    backtrack_count = backtrack_count.saturating_add(1);
                    current_depth = backtrack_to_depth;
                    if event.depth != current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::DepthMismatch {
                                step_index: event.step_index,
                                expected: current_depth,
                                actual: event.depth,
                            },
                        );
                    }
                }
                TassadarVerifierGuidedSearchEventKind::Commit => {
                    if event.depth != current_depth {
                        return Err(
                            TassadarVerifierGuidedSearchTraceError::DepthMismatch {
                                step_index: event.step_index,
                                expected: current_depth,
                                actual: event.depth,
                            },
                        );
                    }
                }
            }

            previous_guess_budget = event.remaining_guess_budget;
            previous_backtrack_budget = event.remaining_backtrack_budget;
        }

        if guess_count > self.budget.max_guesses {
            return Err(TassadarVerifierGuidedSearchTraceError::GuessBudgetExceeded {
                guesses: guess_count,
                budget: self.budget.max_guesses,
            });
        }
        if backtrack_count > self.budget.max_backtracks {
            return Err(TassadarVerifierGuidedSearchTraceError::BacktrackBudgetExceeded {
                backtracks: backtrack_count,
                budget: self.budget.max_backtracks,
            });
        }
        Ok(())
    }

    /// Returns the total explicit guess count.
    #[must_use]
    pub fn guess_count(&self) -> u32 {
        self.events
            .iter()
            .filter(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Guess)
            .count() as u32
    }

    /// Returns the total explicit backtrack count.
    #[must_use]
    pub fn backtrack_count(&self) -> u32 {
        self.events
            .iter()
            .filter(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Backtrack)
            .count() as u32
    }
}

/// Search trace validation failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarVerifierGuidedSearchTraceError {
    /// Missing trace id.
    #[error("verifier-guided search trace is missing `trace_id`")]
    MissingTraceId,
    /// Missing case id.
    #[error("verifier-guided search trace is missing `case_id`")]
    MissingCaseId,
    /// Invalid guess budget.
    #[error("verifier-guided search trace must declare `budget.max_guesses > 0`")]
    InvalidGuessBudget,
    /// Invalid backtrack budget.
    #[error("verifier-guided search trace must declare `budget.max_backtracks > 0`")]
    InvalidBacktrackBudget,
    /// Invalid search-step budget.
    #[error("verifier-guided search trace must declare `budget.max_search_steps > 0`")]
    InvalidSearchStepBudget,
    /// Missing events.
    #[error("verifier-guided search trace must contain events")]
    MissingEvents,
    /// Missing final outputs.
    #[error("verifier-guided search trace is missing final outputs")]
    MissingFinalOutputs,
    /// Missing claim boundary.
    #[error("verifier-guided search trace is missing `claim_boundary`")]
    MissingClaimBoundary,
    /// Search-step budget exceeded.
    #[error("verifier-guided search trace exceeded search-step budget: steps={steps}, budget={budget}")]
    SearchStepBudgetExceeded {
        /// Observed steps.
        steps: u32,
        /// Declared budget.
        budget: u32,
    },
    /// Step indices were not sequential.
    #[error("verifier-guided search trace step index mismatch: expected {expected}, found {actual}")]
    NonSequentialStepIndex {
        /// Expected step index.
        expected: u32,
        /// Observed step index.
        actual: u32,
    },
    /// Remaining guess budget increased.
    #[error("verifier-guided search trace increased remaining guess budget at step {step_index}")]
    GuessBudgetIncreased {
        /// Step index.
        step_index: u32,
    },
    /// Remaining backtrack budget increased.
    #[error("verifier-guided search trace increased remaining backtrack budget at step {step_index}")]
    BacktrackBudgetIncreased {
        /// Step index.
        step_index: u32,
    },
    /// One guess event was malformed.
    #[error("verifier-guided search trace has malformed guess event at step {step_index}")]
    MalformedGuessEvent {
        /// Step index.
        step_index: u32,
    },
    /// One event was missing a required certificate.
    #[error("verifier-guided search trace event `{event_kind:?}` at step {step_index} is missing a certificate")]
    MissingCertificate {
        /// Step index.
        step_index: u32,
        /// Event kind.
        event_kind: TassadarVerifierGuidedSearchEventKind,
    },
    /// One verify event incorrectly carried a contradiction certificate.
    #[error("verifier-guided search trace verify event at step {step_index} carried a contradiction certificate")]
    VerifyCarriesContradictionCertificate {
        /// Step index.
        step_index: u32,
    },
    /// One contradiction event did not carry a contradiction flag.
    #[error("verifier-guided search trace contradiction event at step {step_index} lacked `contradiction_detected=true`")]
    ContradictionWithoutFlag {
        /// Step index.
        step_index: u32,
    },
    /// One event had a depth mismatch.
    #[error("verifier-guided search trace depth mismatch at step {step_index}: expected {expected}, found {actual}")]
    DepthMismatch {
        /// Step index.
        step_index: u32,
        /// Expected depth.
        expected: u32,
        /// Observed depth.
        actual: u32,
    },
    /// One backtrack event was missing a target depth.
    #[error("verifier-guided search trace backtrack event at step {step_index} is missing `backtrack_to_depth`")]
    MissingBacktrackTarget {
        /// Step index.
        step_index: u32,
    },
    /// One backtrack did not reduce depth.
    #[error("verifier-guided search trace backtrack at step {step_index} did not reduce depth: current={current_depth}, target={backtrack_to_depth}")]
    BacktrackDoesNotReduceDepth {
        /// Step index.
        step_index: u32,
        /// Current depth.
        current_depth: u32,
        /// Requested target depth.
        backtrack_to_depth: u32,
    },
    /// Guess budget exceeded.
    #[error("verifier-guided search trace exceeded guess budget: guesses={guesses}, budget={budget}")]
    GuessBudgetExceeded {
        /// Observed guesses.
        guesses: u32,
        /// Declared budget.
        budget: u32,
    },
    /// Backtrack budget exceeded.
    #[error("verifier-guided search trace exceeded backtrack budget: backtracks={backtracks}, budget={budget}")]
    BacktrackBudgetExceeded {
        /// Observed backtracks.
        backtracks: u32,
        /// Declared budget.
        budget: u32,
    },
}

/// Returns the seeded verifier-guided search traces for the bounded research lane.
pub fn tassadar_verifier_guided_search_trace_artifacts(
) -> Result<Vec<TassadarVerifierGuidedSearchTraceArtifact>, TassadarVerifierGuidedSearchTraceError> {
    Ok(vec![
        seeded_sudoku_verifier_guided_search_trace()?,
        seeded_search_kernel_verifier_guided_search_trace()?,
    ])
}

fn seeded_sudoku_verifier_guided_search_trace(
) -> Result<TassadarVerifierGuidedSearchTraceArtifact, TassadarVerifierGuidedSearchTraceError> {
    let sudoku_case = tassadar_sudoku_v0_corpus()
        .into_iter()
        .find(|case| case.case_id == "sudoku_v0_train_a")
        .expect("seeded Sudoku search case should exist");
    TassadarVerifierGuidedSearchTraceArtifact::new(
        "tassadar.verifier_guided_search.sudoku_v0_train_a.v1",
        sudoku_case.case_id,
        TassadarVerifierGuidedSearchWorkloadFamily::SudokuBacktracking,
        TassadarVerifierGuidedSearchBudget {
            max_guesses: 3,
            max_backtracks: 2,
            max_search_steps: 8,
        },
        vec![
            TassadarVerifierGuidedSearchEvent {
                step_index: 0,
                event_kind: TassadarVerifierGuidedSearchEventKind::Guess,
                depth: 1,
                guess_label: Some(String::from("cell_r1_c2")),
                guess_value: Some(2),
                certificate: None,
                backtrack_to_depth: None,
                remaining_guess_budget: 2,
                remaining_backtrack_budget: 2,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 1,
                event_kind: TassadarVerifierGuidedSearchEventKind::Verify,
                depth: 1,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.sudoku_v0_train_a.row1.ok"),
                    verifier_rule: String::from("sudoku_row_column_block_consistency"),
                    subject_ref: String::from("cell_r1_c2"),
                    contradiction_detected: false,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 2,
                remaining_backtrack_budget: 2,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 2,
                event_kind: TassadarVerifierGuidedSearchEventKind::Guess,
                depth: 2,
                guess_label: Some(String::from("cell_r2_c4")),
                guess_value: Some(1),
                certificate: None,
                backtrack_to_depth: None,
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 2,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 3,
                event_kind: TassadarVerifierGuidedSearchEventKind::Contradiction,
                depth: 2,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.sudoku_v0_train_a.col4.contradiction"),
                    verifier_rule: String::from("sudoku_row_column_block_consistency"),
                    subject_ref: String::from("cell_r2_c4"),
                    contradiction_detected: true,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 2,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 4,
                event_kind: TassadarVerifierGuidedSearchEventKind::Backtrack,
                depth: 1,
                guess_label: None,
                guess_value: None,
                certificate: None,
                backtrack_to_depth: Some(1),
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 1,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 5,
                event_kind: TassadarVerifierGuidedSearchEventKind::Guess,
                depth: 2,
                guess_label: Some(String::from("cell_r2_c4")),
                guess_value: Some(3),
                certificate: None,
                backtrack_to_depth: None,
                remaining_guess_budget: 0,
                remaining_backtrack_budget: 1,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 6,
                event_kind: TassadarVerifierGuidedSearchEventKind::Verify,
                depth: 2,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.sudoku_v0_train_a.col4.ok"),
                    verifier_rule: String::from("sudoku_row_column_block_consistency"),
                    subject_ref: String::from("cell_r2_c4"),
                    contradiction_detected: false,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 0,
                remaining_backtrack_budget: 1,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 7,
                event_kind: TassadarVerifierGuidedSearchEventKind::Commit,
                depth: 2,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.sudoku_v0_train_a.solution.commit"),
                    verifier_rule: String::from("sudoku_solution_complete"),
                    subject_ref: String::from("grid"),
                    contradiction_detected: false,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 0,
                remaining_backtrack_budget: 1,
            },
        ],
        sudoku_case.validation_case.expected_outputs,
        "research-only verifier-guided search trace over one real Sudoku-v0 search case; explicit guess, contradiction, and backtrack events are machine-legible, but this is not compiled correctness or a general solver claim",
    )
}

fn seeded_search_kernel_verifier_guided_search_trace(
) -> Result<TassadarVerifierGuidedSearchTraceArtifact, TassadarVerifierGuidedSearchTraceError> {
    TassadarVerifierGuidedSearchTraceArtifact::new(
        "tassadar.verifier_guided_search.kernel_two_branch_recovery.v1",
        "search_kernel_two_branch_recovery",
        TassadarVerifierGuidedSearchWorkloadFamily::SearchKernel,
        TassadarVerifierGuidedSearchBudget {
            max_guesses: 2,
            max_backtracks: 1,
            max_search_steps: 5,
        },
        vec![
            TassadarVerifierGuidedSearchEvent {
                step_index: 0,
                event_kind: TassadarVerifierGuidedSearchEventKind::Guess,
                depth: 1,
                guess_label: Some(String::from("branch_a")),
                guess_value: Some(0),
                certificate: None,
                backtrack_to_depth: None,
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 1,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 1,
                event_kind: TassadarVerifierGuidedSearchEventKind::Contradiction,
                depth: 1,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.kernel.branch_a.dead_end"),
                    verifier_rule: String::from("kernel_consistency_check"),
                    subject_ref: String::from("branch_a"),
                    contradiction_detected: true,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 1,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 2,
                event_kind: TassadarVerifierGuidedSearchEventKind::Backtrack,
                depth: 0,
                guess_label: None,
                guess_value: None,
                certificate: None,
                backtrack_to_depth: Some(0),
                remaining_guess_budget: 1,
                remaining_backtrack_budget: 0,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 3,
                event_kind: TassadarVerifierGuidedSearchEventKind::Guess,
                depth: 1,
                guess_label: Some(String::from("branch_b")),
                guess_value: Some(1),
                certificate: None,
                backtrack_to_depth: None,
                remaining_guess_budget: 0,
                remaining_backtrack_budget: 0,
            },
            TassadarVerifierGuidedSearchEvent {
                step_index: 4,
                event_kind: TassadarVerifierGuidedSearchEventKind::Commit,
                depth: 1,
                guess_label: None,
                guess_value: None,
                certificate: Some(TassadarVerifierCertificate {
                    certificate_id: String::from("cert.kernel.branch_b.solution"),
                    verifier_rule: String::from("kernel_solution_complete"),
                    subject_ref: String::from("branch_b"),
                    contradiction_detected: false,
                }),
                backtrack_to_depth: None,
                remaining_guess_budget: 0,
                remaining_backtrack_budget: 0,
            },
        ],
        vec![42],
        "research-only verifier-guided search trace over one synthetic branch-recovery kernel; it keeps guess, contradiction, and recovery steps explicit without implying a general learned or compiled search solver",
    )
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("verifier-guided search trace value should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        tassadar_verifier_guided_search_trace_artifacts, TassadarVerifierGuidedSearchEventKind,
        TassadarVerifierGuidedSearchWorkloadFamily,
    };

    #[test]
    fn verifier_guided_search_trace_artifacts_are_machine_legible() {
        let artifacts = tassadar_verifier_guided_search_trace_artifacts()
            .expect("seeded verifier-guided search traces should build");
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts.iter().any(|artifact| {
            artifact.workload_family == TassadarVerifierGuidedSearchWorkloadFamily::SudokuBacktracking
                && artifact
                    .events
                    .iter()
                    .any(|event| event.event_kind == TassadarVerifierGuidedSearchEventKind::Backtrack)
        }));
        assert!(artifacts.iter().all(|artifact| !artifact.trace_digest.is_empty()));
    }
}
