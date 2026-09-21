//! Replay: reading stored decision evidence, not doing it again.
//!
//! A receipt store and a trace are claims about calls that already
//! happened. Replay reads them — the sealed receipts a service left in
//! `receipts.jsonl`, the decision steps a caller's ATIF trace wrote —
//! and says what the evidence shows, how far it supports each claim,
//! and what a complete claim would still need. It never runs anything:
//! no network, no subprocess, no delegation, and no model. A reading
//! that needed to execute would not be a replay.
//!
//! Three rules govern the reading:
//!
//! - **Nothing is fabricated.** A missing file, an unreadable receipt,
//!   and an absent trace are each named per input. A trace step that
//!   recorded no request identity cannot claim a receipt, and a receipt
//!   no step claims is an orphan — both are reported, neither is filled
//!   in.
//! - **The join is the only pairing.** Receipts meet trace steps through
//!   [`Join`], and validation through [`Validate`], so stored evidence is
//!   held to the same rule as a live check: a self-consistent digest is
//!   still not issuer authentication.
//! - **Evidence says what it lacks.** A receipt that validates and joins
//!   is evidence. One that fails validation is invalid, with the reason
//!   named. A joinable receipt with no trace — or a trace step with no
//!   receipt — is partial evidence, never upgraded to verified.
//!
//! What replay is not: a re-run. Comparing stored evidence against fresh
//! output is new inference — a new, explicitly budgeted run — and
//! [`Replay::comparison`] is a typed refusal that says so. The refusal
//! is the whole API for that request.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::execution::{ExecutionReceipt, Lane, Outcome, Registry, Served, Timing, digest_request};
use crate::export::{self, Bundle, Consent, Export, Redaction};
use crate::join::{Cost, Join, Joined, Problem, Row, StepRef, Verification};
use crate::validate::{Support, Validate};

/// The extension an ATIF session log carries — `atif`'s own constant,
/// restated because replay reads the format without depending on the
/// crate.
const TRACE_EXTENSION: &str = "atif.jsonl";

/// The schema a decision call's `extra` carries — `atif`'s
/// `DECISION_CALL_SCHEMA`, restated for the same reason.
const DECISION_SCHEMA: &str = "openagents.decision-call.v1";

/// What one named input supplied — or why it gave nothing.
///
/// Loading never fabricates: every file replay was asked to read is
/// accounted for here, and a file that gave nothing says so.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Input {
    /// What the input was read as.
    pub role: Role,
    /// The file read — absent when the input was never supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// What it supplied.
    pub state: InputState,
}

/// Which side of the evidence an input was read as.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Stored receipts — `*receipt*.json` or `*receipt*.jsonl`.
    Receipts,
    /// An ATIF session trace — `*.atif.jsonl`.
    Trace,
}

/// What one input contributed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum InputState {
    /// A receipts file: what read and what did not.
    Receipts {
        /// Receipts that parsed — the join and the verdicts see them all.
        read: usize,
        /// Records that did not parse — each is named in `unread`.
        unread: usize,
    },
    /// A trace: the decision steps it supplied and how it ended.
    Trace {
        /// Decision-call steps the trace supplied to the join.
        steps: usize,
        /// Lines that did not read as records — a partial trace still
        /// joins what it can.
        faults: usize,
        /// Whether the session closed itself. An interrupted session is
        /// still evidence, and it says so.
        ended: bool,
    },
    /// The input gave nothing — the reason says why.
    Missing {
        /// Why nothing was supplied.
        reason: String,
    },
}

/// A receipt record that did not read — reported, never dropped quietly.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Unread {
    /// The file it came from.
    pub path: PathBuf,
    /// The line within it, when the format is lines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Why it did not read — the parse reason.
    pub reason: String,
}

/// Which trace step a receipt joined to — the locator a reader quotes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StepLocator {
    /// The session the step belongs to.
    pub session: String,
    /// The turn inside the session, when the trace named one.
    pub turn: String,
    /// The step's own identity inside its program or turn.
    pub step: String,
}

/// The ordered view of what the stored evidence shows.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Inspection {
    /// What each named input supplied, or why it gave nothing.
    pub inputs: Vec<Input>,
    /// One view per logical request the store holds, ordered by request
    /// identity — each holds its attempts in order, because a retry's
    /// history is the chain.
    pub requests: Vec<RequestView>,
    /// Trace steps no receipt claimed — calls the service never owned
    /// up to.
    pub unreceipted: Vec<StepRef>,
    /// Receipt records that did not read.
    pub unread: Vec<Unread>,
    /// The problems the join found — mismatches, duplicates, malformed
    /// records — each naming the offending identity.
    pub problems: Vec<Problem>,
}

/// One logical request and the attempts it made, in order.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RequestView {
    /// The request identity the attempts share.
    pub request: String,
    /// The attempts, oldest first — a refused or unavailable attempt and
    /// the retry that followed it stay in the order they happened.
    pub attempts: Vec<AttemptView>,
}

/// One stored attempt: what it bound, what it came to, and which trace
/// step claimed it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AttemptView {
    /// Which attempt of the request, one-based.
    pub attempt: u32,
    /// The attempt's own identity.
    pub attempt_id: String,
    /// The receipt's own digest — the identity a reader quotes.
    pub receipt_digest: String,
    /// Digest of the request envelope the attempt served.
    pub request_digest: String,
    /// Digest of the result it produced, when it produced one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_digest: Option<String>,
    /// The transport that carried the call.
    pub transport: String,
    /// The authorized tenant reference, when the call had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
    /// The registry revision the call was admitted under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<Registry>,
    /// The identity the caller asked for.
    pub requested: Served,
    /// The identity that answered — or that was bound when the attempt
    /// failed before an answer.
    pub served: Served,
    /// Whether what answered honors what was asked: the requested model,
    /// plus the adapter and artifact where the request pinned one. A
    /// substituted artifact is a field-level difference, never merged
    /// into a match.
    pub served_as_requested: bool,
    /// What the attempt came to — `answered`, `refused`, `unavailable`,
    /// `unattempted`, or `unknown`, preserved exactly.
    pub outcome: Outcome,
    /// The refusal or failure cause, when the outcome carries one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
    /// The timing the attempt carried.
    pub timing: Timing,
    /// The trace step this attempt joined to — absent when no step
    /// claimed it, and the view says so rather than inventing one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<StepLocator>,
    /// How far the joined evidence goes, when it joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Verification>,
}

/// What the stored evidence supports for one call.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "claim", rename_all = "kebab-case")]
pub enum Claim {
    /// The receipt validates and a trace step claims the same call —
    /// evidence for it, to the depth the join could check.
    Evidence {
        /// How far the join's evidence goes — verified, self-consistent,
        /// or unsupported, said exactly.
        verification: Verification,
    },
    /// The receipt fails validation — not evidence. The reasons are
    /// validation's own, each naming what it offends.
    Invalid {
        /// Why the receipt cannot support the claim.
        reasons: Vec<String>,
    },
    /// One side without the other — a joinable receipt no step claims,
    /// or a step no receipt claims. Partial evidence, never upgraded to
    /// verified.
    Partial {
        /// What the evidence lacks, named.
        lacking: Vec<String>,
    },
}

/// One stored receipt's verdict.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReceiptVerdict {
    /// The request the attempt belongs to.
    pub request: String,
    /// Which attempt of the request, one-based.
    pub attempt: u32,
    /// The attempt's own identity.
    pub attempt_id: String,
    /// The receipt's own digest — the identity a reader quotes.
    pub receipt_digest: String,
    /// The trace step it joined to, when one claimed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<StepLocator>,
    /// What the evidence supports.
    pub claim: Claim,
}

/// One trace step no receipt claimed.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StepVerdict {
    /// The step as the caller recorded it.
    pub step: StepRef,
    /// Always partial — a call the service never owned up to.
    pub claim: Claim,
}

/// The verdict's tallies.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerdictCounts {
    /// Stored receipts that read.
    pub receipts: usize,
    /// Receipts that are evidence — they validate and join.
    pub evidence: usize,
    /// Receipts reported invalid.
    pub invalid: usize,
    /// Receipts that are partial — no step claims them.
    pub partial: usize,
    /// Trace steps no receipt claimed.
    pub unreceipted: usize,
    /// Records that did not read at all.
    pub unread: usize,
}

/// What the evidence can and cannot support, said once.
///
/// The verdict is honest the way validation is honest: `verified` asks
/// for everything the join can check, a claim that cannot be fully
/// checked says which pieces it never had, and `needed` names what a
/// complete claim would still require rather than rounding up.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Verdict {
    /// One verdict per stored receipt, in request then attempt order.
    pub receipts: Vec<ReceiptVerdict>,
    /// Trace steps no receipt claimed — each partial, each named.
    pub unreceipted: Vec<StepVerdict>,
    /// Receipt records that did not read — invalid with the parse
    /// reason.
    pub unread: Vec<Unread>,
    /// The problems the join found, kept with the verdict so the
    /// evidence travels together.
    pub problems: Vec<Problem>,
    /// What a complete claim would still need — each gap named, never
    /// assumed away.
    pub needed: Vec<String>,
    /// Whether the evidence supports every claim it makes: every
    /// receipt evidence at `verified`, nothing missing, nothing unread.
    pub complete: bool,
    /// The tallies.
    pub counts: VerdictCounts,
}

/// Why replay refused a request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Refusal {
    /// The request asks replay to execute — a new comparison, a stored
    /// command, a delegate. Replay reads what was written; it does not
    /// do it again.
    Reexecution {
        /// What the honest answer is instead.
        reason: String,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reexecution { reason } => f.write_str(reason),
        }
    }
}

impl std::error::Error for Refusal {}

/// Stored decision evidence, read and joined: receipts on one side, the
/// caller's trace steps on the other, and every input accounted for.
#[derive(Debug)]
pub struct Replay {
    inputs: Vec<Input>,
    receipts: Vec<ExecutionReceipt>,
    unread: Vec<Unread>,
    steps: Vec<StepRef>,
    /// Whether a trace was supplied at all. An empty trace is still a
    /// supplied trace; a missing one is a different statement.
    trace_seen: bool,
    joined: Joined,
}

impl Replay {
    /// Read every stored receipt and every ATIF trace in `dir`.
    ///
    /// Receipt files are `*receipt*.json` and `*receipt*.jsonl`; trace
    /// files are `*.atif.jsonl`. What each supplied — or why it gave
    /// nothing — lands in `inputs`.
    #[must_use]
    pub fn load(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let mut replay = Self::empty();
        match fs::read_dir(dir) {
            Err(error) => {
                replay.inputs.push(Input {
                    role: Role::Receipts,
                    path: Some(dir.to_path_buf()),
                    state: InputState::Missing {
                        reason: error.to_string(),
                    },
                });
                replay.inputs.push(Input {
                    role: Role::Trace,
                    path: Some(dir.to_path_buf()),
                    state: InputState::Missing {
                        reason: "the directory could not be read".to_string(),
                    },
                });
            }
            Ok(entries) => {
                let mut paths: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect();
                paths.sort();
                let mut receipts = 0;
                let mut traces = 0;
                for path in &paths {
                    if is_trace(path) {
                        replay.trace_from(path);
                        traces += 1;
                    } else if is_receipts(path) {
                        replay.receipts_from(path);
                        receipts += 1;
                    }
                }
                if receipts == 0 {
                    replay.missing(Role::Receipts, "the directory holds no receipt files");
                }
                if traces == 0 {
                    replay.missing(Role::Trace, "the directory holds no ATIF trace");
                }
            }
        }
        replay.join();
        replay
    }

    /// Read one file of stored receipts: `.jsonl` lines, a single
    /// `.json` receipt, or a `.json` array of them.
    ///
    /// The trace is a separate input — [`Replay::with_trace`] attaches
    /// it. Until one is supplied the evidence is one-sided: every
    /// receipt can only ever be an orphan, and the verdict says so.
    #[must_use]
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let mut replay = Self::empty();
        if is_trace(path) {
            replay.missing(Role::Receipts, "the path names an ATIF trace, not receipts");
            replay.trace_from(path);
        } else {
            replay.receipts_from(path);
            replay.missing(Role::Trace, "no trace was supplied");
        }
        replay.join();
        replay
    }

    /// Attach an ATIF trace and re-join.
    #[must_use]
    pub fn with_trace(mut self, path: impl AsRef<Path>) -> Self {
        self.trace_from(path.as_ref());
        self.join();
        self
    }

    /// What a request for a new comparison gets.
    ///
    /// Replay reads stored evidence; a comparison against fresh output
    /// needs new inference, and new inference is a new, explicitly
    /// budgeted run. The refusal is the whole API for that request —
    /// there is no way to run a stored command or delegate through this
    /// module.
    pub fn comparison(_dir: impl AsRef<Path>) -> Result<Self, Refusal> {
        Err(Refusal::Reexecution {
            reason: "a comparison against fresh output is new inference — a new, \
                     explicitly budgeted run — and replay reads stored evidence \
                     instead of running it again"
                .to_string(),
        })
    }

    /// What each named input supplied, or why it gave nothing.
    #[must_use]
    pub fn inputs(&self) -> &[Input] {
        &self.inputs
    }

    /// The join the inspection and verdicts are drawn from.
    #[must_use]
    pub fn joined(&self) -> &Joined {
        &self.joined
    }

    /// The ordered view of what the stored evidence shows: every
    /// receipt's request and result binding, its outcome and timing,
    /// the attempts each request made in order, and which trace step —
    /// if any — claimed it.
    #[must_use]
    pub fn inspect(&self) -> Inspection {
        let rows: BTreeMap<(&str, u32), &Row> = self
            .joined
            .rows
            .iter()
            .map(|row| {
                (
                    (row.references.request.as_str(), row.references.attempt),
                    row,
                )
            })
            .collect();
        let mut grouped: BTreeMap<&str, Vec<&ExecutionReceipt>> = BTreeMap::new();
        for receipt in &self.receipts {
            grouped
                .entry(receipt.request.as_str())
                .or_default()
                .push(receipt);
        }
        let requests = grouped
            .into_iter()
            .map(|(request, mut receipts)| {
                receipts.sort_by(|a, b| {
                    a.attempt
                        .cmp(&b.attempt)
                        .then_with(|| a.attempt_id.cmp(&b.attempt_id))
                });
                RequestView {
                    request: request.to_string(),
                    attempts: receipts
                        .into_iter()
                        .map(|receipt| {
                            let row = rows
                                .get(&(receipt.request.as_str(), receipt.attempt))
                                .copied();
                            AttemptView {
                                attempt: receipt.attempt,
                                attempt_id: receipt.attempt_id.clone(),
                                receipt_digest: receipt.digest.clone(),
                                request_digest: receipt.request_digest.clone(),
                                result_digest: receipt.result_digest.clone(),
                                transport: receipt.transport.clone(),
                                tenant: receipt.tenant.clone(),
                                registry: receipt.registry.clone(),
                                requested: receipt.requested.clone(),
                                served: receipt.served.clone(),
                                served_as_requested: served_as_requested(
                                    &receipt.requested,
                                    &receipt.served,
                                ),
                                outcome: receipt.outcome,
                                cause: receipt.cause.clone(),
                                timing: receipt.timing.clone(),
                                step: row.map(|row| locator(&row.step)),
                                verification: row.map(|row| row.verification),
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        Inspection {
            inputs: self.inputs.clone(),
            requests,
            unreceipted: self.joined.unreceipted.clone(),
            unread: self.unread.clone(),
            problems: self.joined.problems(),
        }
    }

    /// What the evidence can and cannot support.
    ///
    /// A receipt that validates and joins is evidence — at the depth the
    /// join checked, `verified` or less, said exactly. A receipt that
    /// fails validation is invalid with the reason. A receipt no step
    /// claims, and a step no receipt claims, are partial evidence —
    /// never upgraded — and `needed` names what a complete claim would
    /// still require.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let report = Validate::report(&self.joined, &self.receipts);
        let rows: BTreeMap<(&str, u32), &Row> = self
            .joined
            .rows
            .iter()
            .map(|row| {
                (
                    (row.references.request.as_str(), row.references.attempt),
                    row,
                )
            })
            .collect();

        let mut needed: Vec<String> = Vec::new();
        let mut need = |text: String| {
            if !needed.contains(&text) {
                needed.push(text);
            }
        };

        if self.receipts.is_empty() {
            need("receipts — none were stored or none could be read".to_string());
        }
        if !self.trace_seen {
            need("an ATIF trace recording the caller's side of the calls".to_string());
        }

        let mut receipts = Vec::new();
        let mut unreceipted = Vec::new();
        for entry in &report.steps {
            if entry.support == Support::Absent {
                let lacking = format!(
                    "a receipt claiming `{}` attempt {}",
                    entry.step.request, entry.step.attempt
                );
                need(lacking.clone());
                unreceipted.push(StepVerdict {
                    step: entry.step.clone(),
                    claim: Claim::Partial {
                        lacking: vec![lacking],
                    },
                });
                continue;
            }
            let key = (entry.step.request.as_str(), entry.step.attempt);
            let Some(row) = rows.get(&key).copied() else {
                continue;
            };
            let claim = if entry.faults.is_empty() {
                // What a complete claim would still need is exactly what
                // kept this row short of verified — validation's own
                // reasons, carried into `needed`.
                if let Support::Legacy { reasons } = &entry.support {
                    for reason in reasons {
                        need(reason.clone());
                    }
                }
                Claim::Evidence {
                    verification: row.verification,
                }
            } else {
                need(format!(
                    "a receipt that validates for `{}` attempt {}",
                    row.references.request, row.references.attempt
                ));
                Claim::Invalid {
                    reasons: entry.faults.iter().map(ToString::to_string).collect(),
                }
            };
            receipts.push(ReceiptVerdict {
                request: row.references.request.clone(),
                attempt: row.references.attempt,
                attempt_id: row.references.attempt_id.clone(),
                receipt_digest: row.receipt_digest.clone(),
                step: Some(locator(&row.step)),
                claim,
            });
        }

        for orphan in &report.orphans {
            let claim = if orphan.faults.is_empty() {
                let lacking = if self.trace_seen {
                    format!(
                        "a trace step claiming `{}` attempt {}",
                        orphan.request, orphan.attempt
                    )
                } else {
                    "an ATIF trace recording the caller's side of the call".to_string()
                };
                need(lacking.clone());
                Claim::Partial {
                    lacking: vec![lacking],
                }
            } else {
                need(format!(
                    "a receipt that validates for `{}` attempt {}",
                    orphan.request, orphan.attempt
                ));
                Claim::Invalid {
                    reasons: orphan.faults.iter().map(ToString::to_string).collect(),
                }
            };
            receipts.push(ReceiptVerdict {
                request: orphan.request.clone(),
                attempt: orphan.attempt,
                attempt_id: orphan.attempt_id.clone(),
                receipt_digest: orphan.receipt_digest.clone(),
                step: None,
                claim,
            });
        }
        receipts.sort_by(|a, b| {
            (&a.request, a.attempt, &a.attempt_id).cmp(&(&b.request, b.attempt, &b.attempt_id))
        });

        for unread in &self.unread {
            need(format!(
                "a readable receipt record for `{}`{}",
                unread.path.display(),
                unread
                    .line
                    .map(|line| format!(" line {line}"))
                    .unwrap_or_default()
            ));
        }

        let counts = VerdictCounts {
            receipts: self.receipts.len(),
            evidence: receipts
                .iter()
                .filter(|verdict| matches!(verdict.claim, Claim::Evidence { .. }))
                .count(),
            invalid: receipts
                .iter()
                .filter(|verdict| matches!(verdict.claim, Claim::Invalid { .. }))
                .count(),
            partial: receipts
                .iter()
                .filter(|verdict| matches!(verdict.claim, Claim::Partial { .. }))
                .count(),
            unreceipted: unreceipted.len(),
            unread: self.unread.len(),
        };
        let problems = self.joined.problems();
        let complete = counts.receipts > 0
            && counts.unread == 0
            && counts.unreceipted == 0
            && needed.is_empty()
            && problems.is_empty()
            && receipts.iter().all(|verdict| {
                matches!(
                    verdict.claim,
                    Claim::Evidence {
                        verification: Verification::Verified
                    }
                )
            });

        Verdict {
            receipts,
            unreceipted,
            unread: self.unread.clone(),
            problems,
            needed,
            complete,
            counts,
        }
    }

    /// Export the joined evidence under a stated policy and a supplied
    /// consent — the same consented redaction every export answers to.
    /// A replay whose evidence cannot support a complete claim exports
    /// marked `partial` or `insufficient`, never thinner evidence
    /// passing as more.
    pub fn export(&self, policy: &Redaction, consent: &Consent) -> Result<Bundle, export::Refusal> {
        Export::bundle(&self.joined, policy, consent)
    }

    fn empty() -> Self {
        Self {
            inputs: Vec::new(),
            receipts: Vec::new(),
            unread: Vec::new(),
            steps: Vec::new(),
            trace_seen: false,
            joined: Join::of(Vec::new(), Vec::new()),
        }
    }

    /// Re-run the join over what the inputs supplied.
    fn join(&mut self) {
        self.joined = Join::of(self.steps.clone(), self.receipts.clone());
    }

    /// Read one receipts file into the store — or record why it gave
    /// nothing.
    fn receipts_from(&mut self, path: &Path) {
        match read_receipt_file(path) {
            Ok((receipts, unread)) => {
                self.inputs.push(Input {
                    role: Role::Receipts,
                    path: Some(path.to_path_buf()),
                    state: InputState::Receipts {
                        read: receipts.len(),
                        unread: unread.len(),
                    },
                });
                self.receipts.extend(receipts);
                self.unread.extend(unread);
            }
            Err(reason) => self.inputs.push(Input {
                role: Role::Receipts,
                path: Some(path.to_path_buf()),
                state: InputState::Missing { reason },
            }),
        }
    }

    /// Read one ATIF trace into the store — or record why it gave
    /// nothing.
    fn trace_from(&mut self, path: &Path) {
        match read_trace_file(path) {
            Ok(trace) => {
                self.trace_seen = true;
                self.inputs.push(Input {
                    role: Role::Trace,
                    path: Some(path.to_path_buf()),
                    state: InputState::Trace {
                        steps: trace.steps.len(),
                        faults: trace.faults,
                        ended: trace.ended,
                    },
                });
                self.steps.extend(trace.steps);
            }
            Err(reason) => self.inputs.push(Input {
                role: Role::Trace,
                path: Some(path.to_path_buf()),
                state: InputState::Missing { reason },
            }),
        }
    }

    /// Record an input that was never supplied.
    fn missing(&mut self, role: Role, reason: impl Into<String>) {
        self.inputs.push(Input {
            role,
            path: None,
            state: InputState::Missing {
                reason: reason.into(),
            },
        });
    }
}

/// Whether `path` names an ATIF session log.
fn is_trace(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(TRACE_EXTENSION))
}

/// Whether `path` names a file of stored receipts.
fn is_receipts(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.contains("receipt") && (name.ends_with(".jsonl") || name.ends_with(".json"))
        })
}

/// Whether what answered honors what was asked — the model the caller
/// requested, plus the adapter and the artifact where the request
/// pinned one.
fn served_as_requested(requested: &Served, served: &Served) -> bool {
    served.model == requested.model
        && (requested.adapter.is_none() || requested.adapter == served.adapter)
        && (requested.artifact_signature.is_empty()
            || requested.artifact_signature == served.artifact_signature)
}

/// The locator a reader quotes for a joined step.
fn locator(step: &StepRef) -> StepLocator {
    StepLocator {
        session: step.session.clone(),
        turn: step.turn.clone(),
        step: step.step.clone(),
    }
}

/// Read one receipts file: a single receipt, an array of them, or the
/// `receipts.jsonl` line format — one receipt per line, with a line
/// that does not read named rather than dropped.
fn read_receipt_file(path: &Path) -> Result<(Vec<ExecutionReceipt>, Vec<Unread>), String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if let Ok(receipt) = serde_json::from_str::<ExecutionReceipt>(&text) {
        return Ok((vec![receipt], Vec::new()));
    }
    if let Ok(receipts) = serde_json::from_str::<Vec<ExecutionReceipt>>(&text) {
        return Ok((receipts, Vec::new()));
    }
    let mut lines: Vec<&str> = text.lines().collect();
    // A final line with no newline was never finished — it is an
    // unread record even when its bytes happen to parse, the same rule
    // the trace reader keeps.
    let torn = !text.ends_with('\n') && !lines.is_empty();
    if torn {
        lines.pop();
    }
    let mut receipts = Vec::new();
    let mut unread = Vec::new();
    if torn {
        unread.push(Unread {
            path: path.to_path_buf(),
            line: Some(lines.len() + 1),
            reason: "the last line was never finished".to_string(),
        });
    }
    for (index, line) in lines.into_iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<ExecutionReceipt>(line) {
            Ok(receipt) => receipts.push(receipt),
            Err(error) => unread.push(Unread {
                path: path.to_path_buf(),
                line: Some(index + 1),
                reason: error.to_string(),
            }),
        }
    }
    Ok((receipts, unread))
}

/// What an ATIF trace supplied.
struct TraceRead {
    /// The decision-call steps, as the caller's side of the join.
    steps: Vec<StepRef>,
    /// Lines that did not read as records.
    faults: usize,
    /// Whether the session closed itself.
    ended: bool,
}

/// The step fields replay reads — a decision call is the only step a
/// receipt can join, and a decision call is the only step read.
#[derive(Deserialize)]
struct TraceStep {
    #[serde(default)]
    call: Option<TraceCall>,
}

/// The call fields replay reads — the receipt-binding fields a caller
/// records in `extra`, beside `door`, `model`, and the digests it
/// already keeps.
#[derive(Deserialize)]
struct TraceCall {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    extra: Map<String, Value>,
}

/// Read one ATIF session log, recovering what it can: the session's
/// identity, the decision steps it recorded, whether it ended, and a
/// count of the lines that did not read — a partial trace still joins
/// what it can.
fn read_trace_file(path: &Path) -> Result<TraceRead, String> {
    let text = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut session: Option<String> = None;
    let mut ended = false;
    let mut faults = 0;
    let mut steps = Vec::new();
    let mut lines: Vec<&str> = text.lines().collect();
    // A final line with no newline was never finished — the same rule
    // `atif`'s own reader keeps.
    if !text.ends_with('\n') && !lines.is_empty() {
        faults += 1;
        lines.pop();
    }
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Map<String, Value>>(line) else {
            faults += 1;
            continue;
        };
        match record.get("record").and_then(Value::as_str) {
            Some("session") if session.is_none() => {
                session = record
                    .get("session")
                    .and_then(|session| session.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
            }
            Some("step") if session.is_some() && !ended => {
                match record
                    .get("step")
                    .cloned()
                    .and_then(|step| serde_json::from_value::<TraceStep>(step).ok())
                {
                    Some(step) => {
                        if let Some(step) = step_ref(session.as_deref().unwrap_or_default(), &step)
                        {
                            steps.push(step);
                        }
                    }
                    None => faults += 1,
                }
            }
            Some("end") if session.is_some() && !ended => ended = true,
            Some(_) => faults += 1,
            None => faults += 1,
        }
    }
    if session.is_none() {
        return Err(format!("{} holds no session record", path.display()));
    }
    Ok(TraceRead {
        steps,
        faults,
        ended,
    })
}

/// One trace step as the caller's side of the join — a [`StepRef`], or
/// nothing when the step records no decision call.
///
/// The receipt-binding fields live in the decision call's `extra`:
/// `request` and `attempt` are the idempotency pair the caller sent,
/// `request_digest` and `result_digest` the digests it computed, `job`
/// the relay job reference when the lane has one,
/// `origin_authenticated` whether its transport authenticated the
/// answering origin, and `cost` what its accounting settled. When the
/// caller recorded no `request_digest`, replay recomputes the caller's
/// half over the recorded request envelope — the digest is a pure
/// function of what the trace wrote. A `result_digest` is never
/// recomputed: the result bytes are not what the step records, and
/// guessing would be a claim.
fn step_ref(session: &str, step: &TraceStep) -> Option<StepRef> {
    let call = step.call.as_ref()?;
    let extra = &call.extra;
    let decision = extra.get("schema").and_then(Value::as_str) == Some(DECISION_SCHEMA);
    let request = text(extra, "request");
    if !decision && request.is_empty() {
        return None;
    }
    let attempt = extra
        .get("attempt")
        .and_then(Value::as_u64)
        .and_then(|attempt| u32::try_from(attempt).ok())
        .unwrap_or(if request.is_empty() { 0 } else { 1 });
    let request_digest = match text(extra, "request_digest") {
        digest if !digest.is_empty() => digest,
        _ if decision && call.arguments.is_object() => digest_request(&call.arguments),
        _ => String::new(),
    };
    Some(StepRef {
        session: session.to_string(),
        turn: text(extra, "turn"),
        program: text(extra, "program"),
        step: match text(extra, "step") {
            step if !step.is_empty() => step,
            _ => call.id.clone(),
        },
        function: call.name.clone(),
        work_item: text(extra, "work_item"),
        request,
        attempt,
        request_digest,
        result_digest: extra
            .get("result_digest")
            .and_then(Value::as_str)
            .map(str::to_string),
        job: extra.get("job").and_then(Value::as_str).map(str::to_string),
        origin_authenticated: extra
            .get("origin_authenticated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        cost: extra
            .get("cost")
            .and_then(|cost| serde_json::from_value::<Cost>(cost.clone()).ok())
            .unwrap_or(Cost::Unknown),
        lane: extra
            .get("lane")
            .and_then(|lane| serde_json::from_value::<Lane>(lane.clone()).ok()),
        revises: extra
            .get("revises")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// One text field of a call's `extra` — absent stays absent.
fn text(extra: &Map<String, Value>, key: &str) -> String {
    extra
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::{Class, Completeness, Treatment};

    fn digest_of(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    /// A receipt the way a gateway writes one for an answered call.
    fn answered() -> ExecutionReceipt {
        let mut receipt = ExecutionReceipt::for_attempt("http", "req-1", 1, digest_of('q'));
        receipt.attempt_id = "att-1".to_string();
        receipt.tenant = Some("key-ref:acme/2026-09".to_string());
        receipt.registry = Some(Registry {
            digest: digest_of('r'),
            sequence: 3,
        });
        receipt.requested = Served {
            model: "kev-0.6b".to_string(),
            ..Served::default()
        };
        receipt.served = Served {
            model: "kev-0.6b".to_string(),
            artifact_signature: digest_of('a'),
            ..Served::default()
        };
        receipt.outcome = Outcome::Answered;
        receipt.result_digest = Some(digest_of('s'));
        receipt.timing = Timing {
            queued_ms: Some(4),
            latency_ms: Some(87),
            resolved_at: Some("2026-09-21T00:00:00Z".to_string()),
        };
        receipt.usage = Some("resv-881".to_string());
        receipt.seal();
        receipt
    }

    /// A directory the loader can read, removed on drop.
    struct Temp(PathBuf);

    fn temp(tag: &str) -> Temp {
        let path = std::env::temp_dir().join(format!(
            "receipts-replay-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).unwrap();
        Temp(path)
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, name: &str, text: &str) {
        fs::write(dir.join(name), text).unwrap();
    }

    /// Receipts in the `receipts.jsonl` line format the gateway writes.
    fn receipts_jsonl(receipts: &[ExecutionReceipt]) -> String {
        receipts
            .iter()
            .map(|receipt| serde_json::to_string(receipt).unwrap() + "\n")
            .collect()
    }

    /// A step record for a decision call claiming `request`/`attempt` —
    /// the receipt-binding fields a caller records in `extra`.
    fn step_line(
        call: usize,
        request: &str,
        attempt: u32,
        request_digest: &str,
        result_digest: Option<&str>,
    ) -> String {
        let mut extra = serde_json::json!({
            "schema": DECISION_SCHEMA,
            "door": "http://door.test",
            "model": "kev-0.6b",
            "request": request,
            "attempt": attempt,
            "request_digest": request_digest,
            "origin_authenticated": true,
            "cost": {"known": {"millionths": 42, "currency": "USD"}},
        });
        if let Some(digest) = result_digest {
            extra["result_digest"] = serde_json::json!(digest);
        }
        serde_json::json!({
            "record": "step",
            "step": {
                "at": 0,
                "source": "agent",
                "message": "",
                "call": {
                    "id": format!("call-{call}"),
                    "name": "classify",
                    "arguments": {"state": "…", "questions": {}},
                    "output": "",
                    "outcome": "completed",
                    "milliseconds": 1,
                    "extra": extra,
                },
            },
        })
        .to_string()
    }

    /// A trace file: session record, the given lines, end record.
    fn trace_file(lines: &[String]) -> String {
        let mut text = String::from(
            "{\"record\":\"session\",\"schema_version\":\"ATIF-v1.7\",\"at\":0,\
             \"session\":{\"id\":\"ses-1\",\"model\":\"kev-0.6b\",\"door\":\"d\",\
             \"repository\":\"/r\",\"version\":\"0\"}}\n",
        );
        for line in lines {
            text.push_str(line);
            text.push('\n');
        }
        text.push_str("{\"record\":\"end\",\"at\":1,\"state\":\"ended\"}\n");
        text
    }

    /// A shareable policy and the consent covering it — the same shape
    /// export's own tests build.
    fn shareable() -> Redaction {
        let mut policy = Redaction::local();
        policy.name = "shareable".to_string();
        policy.salt = "test-salt".to_string();
        for class in [
            Class::Identities,
            Class::Digests,
            Class::Timing,
            Class::Outcomes,
            Class::Costs,
        ] {
            policy.classes.insert(class, Treatment::Keep);
        }
        policy.classes.insert(Class::Paths, Treatment::Hash);
        policy.seal();
        policy
    }

    fn consent() -> Consent {
        Consent::covering(
            "operator:test",
            [
                Class::Identities,
                Class::Digests,
                Class::Timing,
                Class::Outcomes,
                Class::Costs,
                Class::Paths,
            ],
        )
    }

    #[test]
    fn a_malformed_receipt_is_invalid_with_the_reason() {
        let temp = temp("malformed");
        let mut text = receipts_jsonl(&[answered()]);
        text.push_str("{not a receipt\n");
        text.push_str("{\"v\":\"openagents.receipt.execution.v1\"}\n");
        write(&temp.0, "receipts.jsonl", &text);

        let replay = Replay::load(&temp.0);
        let verdict = replay.verdict();
        // The lines that did not read are invalid, each with its reason
        // and line — and the receipt that read still reports.
        assert_eq!(verdict.counts.unread, 2);
        assert!(
            verdict
                .unread
                .iter()
                .all(|unread| !unread.reason.is_empty())
        );
        assert_eq!(verdict.unread[0].line, Some(2));
        assert_eq!(verdict.counts.receipts, 1);
        // With no trace, the good receipt is partial — never upgraded.
        assert!(matches!(verdict.receipts[0].claim, Claim::Partial { .. }));
    }

    #[test]
    fn a_missing_receipt_is_partial_not_absent_silent() {
        let temp = temp("missing");
        // No receipts file at all; a trace claims a call exists.
        write(
            &temp.0,
            "trace.atif.jsonl",
            &trace_file(&[step_line(
                1,
                "req-9",
                1,
                &digest_of('q'),
                Some(&digest_of('s')),
            )]),
        );
        let replay = Replay::load(&temp.0);
        let receipts_input = replay
            .inputs()
            .iter()
            .find(|input| input.role == Role::Receipts)
            .unwrap();
        assert!(matches!(receipts_input.state, InputState::Missing { .. }));
        let verdict = replay.verdict();
        // The trace's claim is partial: a call the service never owned
        // up to, named rather than silent.
        assert_eq!(verdict.unreceipted.len(), 1);
        assert!(matches!(
            &verdict.unreceipted[0].claim,
            Claim::Partial { lacking } if lacking[0].contains("req-9")
        ));
        assert!(verdict.needed.iter().any(|need| need.contains("receipts")));
        assert!(!verdict.complete);

        // And a path that does not exist reports rather than vanishing.
        let replay = Replay::open(temp.0.join("no-such.jsonl"));
        assert!(matches!(
            replay.inputs()[0].state,
            InputState::Missing { .. }
        ));
    }

    #[test]
    fn a_mismatched_request_result_binding_is_named() {
        let temp1 = temp("mismatch-request");
        write(&temp1.0, "receipts.jsonl", &receipts_jsonl(&[answered()]));
        // The step claims the same attempt but a different request.
        write(
            &temp1.0,
            "trace.atif.jsonl",
            &trace_file(&[step_line(
                1,
                "req-1",
                1,
                &digest_of('z'),
                Some(&digest_of('s')),
            )]),
        );
        let verdict = Replay::load(&temp1.0).verdict();
        assert!(verdict.problems.iter().any(|problem| matches!(
            problem,
            Problem::MismatchedIdentity { field, .. } if field == "request_digest"
        )));
        // Neither side pretends the pair joined.
        assert!(matches!(verdict.receipts[0].claim, Claim::Partial { .. }));
        assert!(matches!(
            verdict.unreceipted[0].claim,
            Claim::Partial { .. }
        ));

        // A wrong result digest is named the same way.
        let temp2 = temp("mismatch-result");
        write(&temp2.0, "receipts.jsonl", &receipts_jsonl(&[answered()]));
        write(
            &temp2.0,
            "trace.atif.jsonl",
            &trace_file(&[step_line(
                1,
                "req-1",
                1,
                &digest_of('q'),
                Some(&digest_of('z')),
            )]),
        );
        let verdict = Replay::load(&temp2.0).verdict();
        assert!(verdict.problems.iter().any(|problem| matches!(
            problem,
            Problem::MismatchedIdentity { field, .. } if field == "result_digest"
        )));
    }

    #[test]
    fn mixed_tenant_and_model_identities_stay_distinct() {
        let temp = temp("mixed");
        let mut other = answered();
        other.attempt = 2;
        other.attempt_id = "att-2".to_string();
        other.tenant = Some("key-ref:other/2026-09".to_string());
        other.requested.model = "kev-4b".to_string();
        other.served.model = "kev-4b".to_string();
        other.timing.resolved_at = Some("2026-09-21T00:00:05Z".to_string());
        other.seal();
        write(
            &temp.0,
            "receipts.jsonl",
            &receipts_jsonl(&[answered(), other]),
        );
        write(
            &temp.0,
            "trace.atif.jsonl",
            &trace_file(&[
                step_line(1, "req-1", 1, &digest_of('q'), Some(&digest_of('s'))),
                step_line(2, "req-1", 2, &digest_of('q'), Some(&digest_of('s'))),
            ]),
        );

        let replay = Replay::load(&temp.0);
        let inspection = replay.inspect();
        let attempts = &inspection.requests[0].attempts;
        // Two identities under one request stay two — never merged.
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].tenant.as_deref(), Some("key-ref:acme/2026-09"));
        assert_eq!(attempts[1].tenant.as_deref(), Some("key-ref:other/2026-09"));
        assert_eq!(attempts[0].served.model, "kev-0.6b");
        assert_eq!(attempts[1].served.model, "kev-4b");

        // The later attempt's divergent tenant is a named fault, not a
        // blend.
        let verdict = replay.verdict();
        let second = verdict
            .receipts
            .iter()
            .find(|verdict| verdict.attempt == 2)
            .unwrap();
        assert!(matches!(
            &second.claim,
            Claim::Invalid { reasons } if reasons.iter().any(|reason| reason.contains("tenant"))
        ));
        assert_eq!(verdict.counts.receipts, 2);
    }

    #[test]
    fn a_partial_trace_still_joins_what_it_can() {
        let temp = temp("partial");
        let mut second = answered();
        second.attempt = 2;
        second.attempt_id = "att-2".to_string();
        second.timing.resolved_at = Some("2026-09-21T00:00:05Z".to_string());
        second.seal();
        write(
            &temp.0,
            "receipts.jsonl",
            &receipts_jsonl(&[answered(), second]),
        );
        // The trace recorded only the first attempt — and one line that
        // never read.
        write(
            &temp.0,
            "trace.atif.jsonl",
            &trace_file(&[
                step_line(1, "req-1", 1, &digest_of('q'), Some(&digest_of('s'))),
                "{a torn record".to_string(),
            ]),
        );

        let replay = Replay::load(&temp.0);
        let trace_input = replay
            .inputs()
            .iter()
            .find(|input| input.role == Role::Trace)
            .unwrap();
        assert!(matches!(
            trace_input.state,
            InputState::Trace {
                steps: 1,
                faults: 1,
                ..
            }
        ));

        let verdict = replay.verdict();
        let first = verdict
            .receipts
            .iter()
            .find(|verdict| verdict.attempt == 1)
            .unwrap();
        let second = verdict
            .receipts
            .iter()
            .find(|verdict| verdict.attempt == 2)
            .unwrap();
        assert!(matches!(
            first.claim,
            Claim::Evidence {
                verification: Verification::Verified
            }
        ));
        // The attempt the trace never recorded is partial, with what it
        // lacks named.
        assert!(matches!(
            &second.claim,
            Claim::Partial { lacking }
                if lacking.iter().any(|lack| lack.contains("req-1") && lack.contains("attempt 2"))
        ));
        assert!(
            verdict
                .needed
                .iter()
                .any(|need| need.contains("req-1") && need.contains("attempt 2"))
        );
    }

    #[test]
    fn a_retry_pair_preserves_both_attempts_in_order() {
        let temp = temp("retry");
        let mut timed_out = answered();
        timed_out.outcome = Outcome::Unavailable;
        timed_out.cause = Some("timeout".to_string());
        timed_out.result_digest = None;
        timed_out.seal();
        let mut second = answered();
        second.attempt = 2;
        second.attempt_id = "att-2".to_string();
        second.timing.resolved_at = Some("2026-09-21T00:00:05Z".to_string());
        second.seal();
        // Stored out of order — the view puts the chain back.
        write(
            &temp.0,
            "receipts.jsonl",
            &receipts_jsonl(&[second, timed_out]),
        );
        write(
            &temp.0,
            "trace.atif.jsonl",
            &trace_file(&[
                step_line(1, "req-1", 1, &digest_of('q'), None),
                step_line(2, "req-1", 2, &digest_of('q'), Some(&digest_of('s'))),
            ]),
        );

        let replay = Replay::load(&temp.0);
        let inspection = replay.inspect();
        let attempts = &inspection.requests[0].attempts;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].attempt, 1);
        assert_eq!(attempts[0].outcome, Outcome::Unavailable);
        assert_eq!(attempts[0].cause.as_deref(), Some("timeout"));
        assert_eq!(attempts[1].attempt, 2);
        assert_eq!(attempts[1].outcome, Outcome::Answered);
        // Each attempt joined its own step, in order.
        assert!(attempts.iter().all(|attempt| attempt.step.is_some()));
        assert_eq!(attempts[0].step.as_ref().unwrap().session.as_str(), "ses-1");
        let verdict = replay.verdict();
        assert_eq!(verdict.counts.evidence, 2);
        assert!(verdict.complete);
    }

    #[test]
    fn a_recorded_envelope_recomputes_the_callers_digest() {
        let temp = temp("recompute");
        let envelope = serde_json::json!({
            "model": "kev-0.6b",
            "state": "the caller's state",
            "questions": {"q1": {"type": "noul"}},
        });
        let mut receipt = answered();
        receipt.request_digest = digest_request(&envelope);
        receipt.seal();
        write(&temp.0, "receipts.jsonl", &receipts_jsonl(&[receipt]));
        // The step records no request_digest — but it recorded the
        // envelope, and the digest is a pure function of it.
        let extra = serde_json::json!({
            "schema": DECISION_SCHEMA,
            "request": "req-1",
            "attempt": 1,
            "result_digest": digest_of('s'),
            "origin_authenticated": true,
        });
        let line = serde_json::json!({
            "record": "step",
            "step": {
                "at": 0,
                "source": "agent",
                "message": "",
                "call": {
                    "id": "call-1",
                    "name": "classify",
                    "arguments": envelope,
                    "output": "",
                    "outcome": "completed",
                    "milliseconds": 1,
                    "extra": extra,
                },
            },
        })
        .to_string();
        write(&temp.0, "trace.atif.jsonl", &trace_file(&[line]));

        let verdict = Replay::load(&temp.0).verdict();
        assert!(matches!(
            verdict.receipts[0].claim,
            Claim::Evidence {
                verification: Verification::Verified
            }
        ));
    }

    #[test]
    fn an_export_marks_evidence_that_cannot_support_a_claim() {
        let temp = temp("export");
        write(&temp.0, "receipts.jsonl", &receipts_jsonl(&[answered()]));
        // No trace: every receipt is an orphan and the export cannot
        // carry the claim.
        let replay = Replay::load(&temp.0);
        let bundle = replay.export(&shareable(), &consent()).unwrap();
        assert!(matches!(
            bundle.completeness,
            Completeness::Insufficient { .. }
        ));

        // A trace that joins one call but records another the service
        // never claimed: partial, never upgraded.
        write(
            &temp.0,
            "trace.atif.jsonl",
            &trace_file(&[
                step_line(1, "req-1", 1, &digest_of('q'), Some(&digest_of('s'))),
                step_line(2, "req-9", 1, &digest_of('q'), None),
            ]),
        );
        let replay = Replay::load(&temp.0);
        let bundle = replay.export(&shareable(), &consent()).unwrap();
        assert_eq!(bundle.completeness, Completeness::Partial);
    }

    #[test]
    fn a_comparison_is_a_typed_refusal_not_a_run() {
        let temp = temp("comparison");
        let refusal = Replay::comparison(&temp.0).unwrap_err();
        assert!(matches!(refusal, Refusal::Reexecution { .. }));
        assert!(refusal.to_string().contains("budget"));
    }
}
