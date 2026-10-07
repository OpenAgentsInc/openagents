use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::tassadar_alm_backend::{
    TassadarAlmCompiledBundle, TassadarAlmCompiledExecutionError, TassadarAlmCompiledExecutor,
};

/// Stable verification-class identifier for exact ALM trace replay.
pub const TASSADAR_ALM_TRACE_REPLAY_CLASS_ID: &str = "exact_trace_replay.alm_compiled.v1";
/// Claim boundary for the ALM trace-replay verification lane.
pub const TASSADAR_ALM_TRACE_REPLAY_CLAIM_BOUNDARY: &str = "exact trace replay verifies \
     compiled-ALM-bundle executions only, by deterministic re-execution and bitwise comparison; \
     it grants no serving, payment, or settlement authority and makes no claim about non-ALM \
     trace formats";

/// One verification request against a compiled bundle execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmReplayClaim {
    /// Claimed bundle digest (verified against the actual bundle).
    pub bundle_digest: String,
    /// Claimed trace digest over the full execution.
    pub trace_digest: String,
}

/// Typed rejection reason for one replay verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum TassadarAlmReplayRejection {
    /// The claimed bundle digest does not match the supplied bundle.
    BundleDigestMismatch {
        /// Digest of the supplied bundle.
        actual: String,
    },
    /// The replayed trace digest does not match the claim.
    TraceDigestMismatch {
        /// Digest produced by replay.
        actual: String,
    },
    /// One claimed window row differs from the replayed row.
    RowMismatch {
        /// First mismatching step index.
        step: usize,
    },
    /// Replay itself refused with a typed execution error.
    ExecutionRefused {
        /// Display rendering of the execution refusal.
        detail: String,
    },
}

/// Window spot-check failure that prevents a verdict.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TassadarAlmReplayRequestError {
    /// The sampled window is empty or out of range.
    #[error("window [{start}, {end}) is invalid for {steps} steps")]
    InvalidWindow {
        /// Window start (inclusive).
        start: usize,
        /// Window end (exclusive).
        end: usize,
        /// Total steps available.
        steps: usize,
    },
    /// The claimed window rows have the wrong length.
    #[error("claimed {found} rows for a window of {expected}")]
    WindowArityMismatch {
        /// Claimed row count.
        found: usize,
        /// Window length.
        expected: usize,
    },
}

/// Outcome of one replay verification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAlmReplayOutcome {
    /// The claim survived exact replay.
    Verified,
    /// The claim was rejected with a typed reason.
    Rejected(TassadarAlmReplayRejection),
}

/// One deterministic, receipt-embeddable replay verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAlmReplayVerdict {
    /// Verification class identifier.
    pub class_id: String,
    /// Verified or rejected outcome.
    pub outcome: TassadarAlmReplayOutcome,
    /// Steps re-executed during replay.
    pub replayed_steps: usize,
    /// Steps compared against the claim (full length or window length).
    pub compared_steps: usize,
    /// Digest of the supplied bundle.
    pub bundle_digest: String,
}

impl TassadarAlmReplayVerdict {
    /// Returns a stable digest over the verdict encoding.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"tassadar_alm_replay_verdict|");
        hasher.update(serde_json::to_vec(self).unwrap_or_default());
        hex::encode(hasher.finalize())
    }
}

fn execution_rejection(error: &TassadarAlmCompiledExecutionError) -> TassadarAlmReplayRejection {
    TassadarAlmReplayRejection::ExecutionRefused {
        detail: error.to_string(),
    }
}

/// Verifies one full-execution claim by exact replay.
#[must_use]
pub fn tassadar_alm_verify_full_replay(
    bundle: &TassadarAlmCompiledBundle,
    steps: &[Vec<i64>],
    claim: &TassadarAlmReplayClaim,
) -> TassadarAlmReplayVerdict {
    let bundle_digest = bundle.stable_digest();
    if claim.bundle_digest != bundle_digest {
        return TassadarAlmReplayVerdict {
            class_id: TASSADAR_ALM_TRACE_REPLAY_CLASS_ID.to_string(),
            outcome: TassadarAlmReplayOutcome::Rejected(
                TassadarAlmReplayRejection::BundleDigestMismatch {
                    actual: bundle_digest.clone(),
                },
            ),
            replayed_steps: 0,
            compared_steps: 0,
            bundle_digest,
        };
    }
    match TassadarAlmCompiledExecutor::execute(bundle, steps) {
        Err(error) => TassadarAlmReplayVerdict {
            class_id: TASSADAR_ALM_TRACE_REPLAY_CLASS_ID.to_string(),
            outcome: TassadarAlmReplayOutcome::Rejected(execution_rejection(&error)),
            replayed_steps: 0,
            compared_steps: 0,
            bundle_digest,
        },
        Ok(trace) => {
            let outcome = if trace.trace_digest == claim.trace_digest {
                TassadarAlmReplayOutcome::Verified
            } else {
                TassadarAlmReplayOutcome::Rejected(
                    TassadarAlmReplayRejection::TraceDigestMismatch {
                        actual: trace.trace_digest.clone(),
                    },
                )
            };
            TassadarAlmReplayVerdict {
                class_id: TASSADAR_ALM_TRACE_REPLAY_CLASS_ID.to_string(),
                outcome,
                replayed_steps: trace.step_count,
                compared_steps: trace.step_count,
                bundle_digest,
            }
        }
    }
}

/// Verifies claimed output rows for one sampled step window by replay.
///
/// ALM execution is sequential, so the window check replays from step
/// zero; at homework scale this remains the cheapest verification grade
/// available because replay costs the same as the original work.
pub fn tassadar_alm_verify_window(
    bundle: &TassadarAlmCompiledBundle,
    steps: &[Vec<i64>],
    window_start: usize,
    claimed_rows: &[Vec<i64>],
) -> Result<TassadarAlmReplayVerdict, TassadarAlmReplayRequestError> {
    let window_end = window_start + claimed_rows.len();
    if claimed_rows.is_empty() || window_end > steps.len() {
        return Err(TassadarAlmReplayRequestError::InvalidWindow {
            start: window_start,
            end: window_end,
            steps: steps.len(),
        });
    }
    let bundle_digest = bundle.stable_digest();
    match TassadarAlmCompiledExecutor::execute(bundle, steps) {
        Err(error) => Ok(TassadarAlmReplayVerdict {
            class_id: TASSADAR_ALM_TRACE_REPLAY_CLASS_ID.to_string(),
            outcome: TassadarAlmReplayOutcome::Rejected(execution_rejection(&error)),
            replayed_steps: 0,
            compared_steps: 0,
            bundle_digest,
        }),
        Ok(trace) => {
            let mut outcome = TassadarAlmReplayOutcome::Verified;
            for (offset, claimed_row) in claimed_rows.iter().enumerate() {
                let step = window_start + offset;
                if &trace.step_outputs[step] != claimed_row {
                    outcome = TassadarAlmReplayOutcome::Rejected(
                        TassadarAlmReplayRejection::RowMismatch { step },
                    );
                    break;
                }
            }
            Ok(TassadarAlmReplayVerdict {
                class_id: TASSADAR_ALM_TRACE_REPLAY_CLASS_ID.to_string(),
                outcome,
                replayed_steps: trace.step_count,
                compared_steps: claimed_rows.len(),
                bundle_digest,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use psionic_ir::tassadar_alm_running_sum_workload;

    use super::*;
    use crate::tassadar_alm_backend::compile_tassadar_alm_graph;
    use crate::tassadar_alm_stack_isa::{
        tassadar_alm_stack_isa_interpreter, TassadarStackIsaInstruction,
    };

    fn stack_isa_bundle_and_inputs() -> (TassadarAlmCompiledBundle, Vec<Vec<i64>>) {
        use TassadarStackIsaInstruction as I;
        let program = vec![I::Push(3), I::Push(5), I::Add, I::Out];
        let (graph, steps) = tassadar_alm_stack_isa_interpreter(&program, 4).expect("builds");
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        (bundle, vec![vec![0_i64]; steps])
    }

    #[test]
    fn honest_full_replay_claims_verify() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        let trace = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        let claim = TassadarAlmReplayClaim {
            bundle_digest: bundle.stable_digest(),
            trace_digest: trace.trace_digest.clone(),
        };
        let verdict = tassadar_alm_verify_full_replay(&bundle, &inputs, &claim);
        assert_eq!(verdict.outcome, TassadarAlmReplayOutcome::Verified);
        assert_eq!(verdict.replayed_steps, inputs.len());
        assert_eq!(verdict.class_id, TASSADAR_ALM_TRACE_REPLAY_CLASS_ID);
    }

    #[test]
    fn tampered_trace_digests_reject() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        let claim = TassadarAlmReplayClaim {
            bundle_digest: bundle.stable_digest(),
            trace_digest: "forged".to_string(),
        };
        let verdict = tassadar_alm_verify_full_replay(&bundle, &inputs, &claim);
        assert!(matches!(
            verdict.outcome,
            TassadarAlmReplayOutcome::Rejected(
                TassadarAlmReplayRejection::TraceDigestMismatch { .. }
            )
        ));
    }

    #[test]
    fn tampered_bundle_digests_reject_before_replay() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        let claim = TassadarAlmReplayClaim {
            bundle_digest: "forged".to_string(),
            trace_digest: "irrelevant".to_string(),
        };
        let verdict = tassadar_alm_verify_full_replay(&bundle, &inputs, &claim);
        assert_eq!(verdict.replayed_steps, 0);
        assert!(matches!(
            verdict.outcome,
            TassadarAlmReplayOutcome::Rejected(
                TassadarAlmReplayRejection::BundleDigestMismatch { .. }
            )
        ));
    }

    #[test]
    fn window_spot_checks_verify_honest_rows_and_name_the_tampered_step() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        let trace = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        let honest: Vec<Vec<i64>> = trace.step_outputs[1..3].to_vec();
        let verdict = tassadar_alm_verify_window(&bundle, &inputs, 1, &honest).expect("verifies");
        assert_eq!(verdict.outcome, TassadarAlmReplayOutcome::Verified);
        assert_eq!(verdict.compared_steps, 2);
        let mut tampered = honest.clone();
        tampered[1][0] += 1;
        let verdict = tassadar_alm_verify_window(&bundle, &inputs, 1, &tampered).expect("verifies");
        assert_eq!(
            verdict.outcome,
            TassadarAlmReplayOutcome::Rejected(TassadarAlmReplayRejection::RowMismatch { step: 2 })
        );
    }

    #[test]
    fn invalid_windows_refuse() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        assert_eq!(
            tassadar_alm_verify_window(&bundle, &inputs, 3, &[vec![0, 0], vec![0, 0]])
                .expect_err("refuses"),
            TassadarAlmReplayRequestError::InvalidWindow {
                start: 3,
                end: 5,
                steps: 4
            }
        );
        assert!(matches!(
            tassadar_alm_verify_window(&bundle, &inputs, 0, &[]),
            Err(TassadarAlmReplayRequestError::InvalidWindow { .. })
        ));
    }

    #[test]
    fn execution_refusals_propagate_as_typed_rejections() {
        let graph = tassadar_alm_running_sum_workload();
        let bundle = compile_tassadar_alm_graph(&graph).expect("compiles");
        // Wrong arity: two fields supplied for a one-field graph.
        let bad_inputs = vec![vec![1_i64, 2_i64]];
        let claim = TassadarAlmReplayClaim {
            bundle_digest: bundle.stable_digest(),
            trace_digest: "irrelevant".to_string(),
        };
        let verdict = tassadar_alm_verify_full_replay(&bundle, &bad_inputs, &claim);
        assert!(matches!(
            verdict.outcome,
            TassadarAlmReplayOutcome::Rejected(TassadarAlmReplayRejection::ExecutionRefused { .. })
        ));
    }

    #[test]
    fn verdicts_are_digest_stable() {
        let (bundle, inputs) = stack_isa_bundle_and_inputs();
        let trace = TassadarAlmCompiledExecutor::execute(&bundle, &inputs).expect("executes");
        let claim = TassadarAlmReplayClaim {
            bundle_digest: bundle.stable_digest(),
            trace_digest: trace.trace_digest,
        };
        let a = tassadar_alm_verify_full_replay(&bundle, &inputs, &claim);
        let b = tassadar_alm_verify_full_replay(&bundle, &inputs, &claim);
        assert_eq!(a.stable_digest(), b.stable_digest());
    }
}
