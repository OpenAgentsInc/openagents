//! Collective-op failure semantics: chunked transfers, ban-for-round, and
//! partial-result preservation.
//!
//! This module ports the Pluralis agora fault-tolerance rules
//! (`docs/agora-system/fault-tolerance.md` in the read-only
//! `projects/pluralis/repos/agora` lane) into a deterministic,
//! simulation-level round state machine for psionic#1126 (Pluralis roadmap
//! P2.1). The crate plans and observes; no real networking runs here.
//!
//! Ported rules:
//!
//! - Tensors are chunked with per-chunk timeout budgets on both the send path
//!   and the reciprocal return path.
//! - A failing sender is banned for the round, not retried. Failures are
//!   receipted and the round continues.
//! - Partial results are preserved: chunks a member delivered before its ban
//!   still count toward the round's reduce accounting.
//! - A full-round timeout protects against additional stalls; the round
//!   retains its partial reduce result.
//! - A round aborts only when the stage would empty and the typed
//!   standby-promotion decision says no warm standby can be promoted. The
//!   actual dispatcher lives monorepo-side; this module owns the
//!   collective-layer decision record the dispatcher consumes.
//! - Per-round bans accumulate in a typed ledger; persistent failers earn a
//!   receipt-compatible swarm-removal recommendation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Schema version stamped on every failure-semantics record.
pub const FAILURE_SEMANTICS_SCHEMA_VERSION: &str = "psionic.collectives.failure_semantics.v1";

/// Stable contract identifier for the committed fixture.
pub const FAILURE_SEMANTICS_CONTRACT_ID: &str = "psionic.collectives.failure_semantics.v1";

/// Basis-point denominator used by preserved-fraction accounting.
const FRACTION_DENOMINATOR_BPS: u64 = 10_000;

/// Errors returned by collective failure-semantics planning and rounds.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CollectiveFailureSemanticsError {
    /// The chunking config carried a zero-valued field.
    #[error("collective round config field `{field}` must be greater than zero")]
    ZeroConfigField {
        /// Offending config field name.
        field: &'static str,
    },
    /// The tensor to chunk was empty.
    #[error("cannot plan chunks for an empty tensor")]
    EmptyTensor,
    /// The round was created without members.
    #[error("collective round `{round_id}` requires at least one member")]
    EmptyRound {
        /// Round identifier.
        round_id: String,
    },
    /// The round was created with a duplicate member.
    #[error("collective round `{round_id}` declared member `{node_id}` more than once")]
    DuplicateMember {
        /// Round identifier.
        round_id: String,
        /// Duplicated member node id.
        node_id: String,
    },
    /// A chunk outcome referenced a member outside the round.
    #[error("member `{node_id}` is not part of round `{round_id}`")]
    UnknownMember {
        /// Round identifier.
        round_id: String,
        /// Unknown member node id.
        node_id: String,
    },
    /// A chunk outcome referenced a chunk index outside the plan.
    #[error("chunk index {chunk_index} is outside the {chunk_count}-chunk plan")]
    ChunkIndexOutOfRange {
        /// Offending chunk index.
        chunk_index: u64,
        /// Planned chunk count.
        chunk_count: u64,
    },
    /// The round was already finalized.
    #[error("collective round `{round_id}` is already finalized")]
    RoundAlreadyFinalized {
        /// Round identifier.
        round_id: String,
    },
    /// The ban-escalation policy carried a zero-valued field.
    #[error("ban escalation policy field `{field}` must be greater than zero")]
    ZeroEscalationField {
        /// Offending policy field name.
        field: &'static str,
    },
}

/// Config for one chunked collective round. All values are caller-supplied;
/// there are no built-in magic constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveRoundConfig {
    /// Maximum bytes per tensor chunk.
    pub chunk_bytes: u64,
    /// Per-chunk timeout budget on the send path in milliseconds.
    pub per_chunk_timeout_ms: u64,
    /// Per-chunk timeout budget on the reciprocal return path in milliseconds.
    pub reciprocal_per_chunk_timeout_ms: u64,
    /// Full-round timeout in milliseconds.
    pub round_timeout_ms: u64,
}

impl CollectiveRoundConfig {
    /// Creates one round config from explicit budgets.
    #[must_use]
    pub const fn new(
        chunk_bytes: u64,
        per_chunk_timeout_ms: u64,
        reciprocal_per_chunk_timeout_ms: u64,
        round_timeout_ms: u64,
    ) -> Self {
        Self {
            chunk_bytes,
            per_chunk_timeout_ms,
            reciprocal_per_chunk_timeout_ms,
            round_timeout_ms,
        }
    }

    /// Validates that every budget is non-zero.
    pub fn validate(&self) -> Result<(), CollectiveFailureSemanticsError> {
        let fields = [
            ("chunk_bytes", self.chunk_bytes),
            ("per_chunk_timeout_ms", self.per_chunk_timeout_ms),
            (
                "reciprocal_per_chunk_timeout_ms",
                self.reciprocal_per_chunk_timeout_ms,
            ),
            ("round_timeout_ms", self.round_timeout_ms),
        ];
        for (field, value) in fields {
            if value == 0 {
                return Err(CollectiveFailureSemanticsError::ZeroConfigField { field });
            }
        }
        Ok(())
    }
}

/// Ban-escalation policy: per-round bans accumulate to a removal
/// recommendation after `removal_ban_threshold` bans within the trailing
/// `window_rounds` rounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanEscalationPolicy {
    /// Number of round bans inside the window that triggers removal.
    pub removal_ban_threshold: u32,
    /// Trailing round window the threshold is evaluated over.
    pub window_rounds: u64,
}

impl BanEscalationPolicy {
    /// Creates one escalation policy from explicit thresholds.
    #[must_use]
    pub const fn new(removal_ban_threshold: u32, window_rounds: u64) -> Self {
        Self {
            removal_ban_threshold,
            window_rounds,
        }
    }

    /// Validates that the policy thresholds are non-zero.
    pub fn validate(&self) -> Result<(), CollectiveFailureSemanticsError> {
        if self.removal_ban_threshold == 0 {
            return Err(CollectiveFailureSemanticsError::ZeroEscalationField {
                field: "removal_ban_threshold",
            });
        }
        if self.window_rounds == 0 {
            return Err(CollectiveFailureSemanticsError::ZeroEscalationField {
                field: "window_rounds",
            });
        }
        Ok(())
    }
}

/// Deterministic chunk plan for one tensor transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TensorChunkPlan {
    /// Schema version of this record.
    pub schema_version: String,
    /// Total tensor bytes divided across chunks.
    pub tensor_bytes: u64,
    /// Bytes per full chunk.
    pub chunk_bytes: u64,
    /// Number of chunks in the plan.
    pub chunk_count: u64,
    /// Bytes carried by the final chunk.
    pub last_chunk_bytes: u64,
    /// Per-chunk send-path timeout budget in milliseconds.
    pub per_chunk_timeout_ms: u64,
    /// Per-chunk reciprocal return-path timeout budget in milliseconds.
    pub reciprocal_per_chunk_timeout_ms: u64,
    /// Full-round timeout in milliseconds.
    pub round_timeout_ms: u64,
    /// Stable digest over the plan inputs.
    pub plan_digest: String,
}

impl TensorChunkPlan {
    /// Returns the byte size of one planned chunk.
    #[must_use]
    pub const fn chunk_bytes_at(&self, chunk_index: u64) -> u64 {
        if chunk_index >= self.chunk_count {
            0
        } else if chunk_index + 1 == self.chunk_count {
            self.last_chunk_bytes
        } else {
            self.chunk_bytes
        }
    }
}

/// Divides one tensor byte-length into chunks with explicit timeout budgets.
pub fn plan_tensor_chunks(
    tensor_bytes: u64,
    config: &CollectiveRoundConfig,
) -> Result<TensorChunkPlan, CollectiveFailureSemanticsError> {
    config.validate()?;
    if tensor_bytes == 0 {
        return Err(CollectiveFailureSemanticsError::EmptyTensor);
    }
    let chunk_count = tensor_bytes.div_ceil(config.chunk_bytes);
    let remainder = tensor_bytes % config.chunk_bytes;
    let last_chunk_bytes = if remainder == 0 {
        config.chunk_bytes
    } else {
        remainder
    };
    let plan_digest = stable_chunk_plan_digest(tensor_bytes, config, chunk_count, last_chunk_bytes);
    Ok(TensorChunkPlan {
        schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
        tensor_bytes,
        chunk_bytes: config.chunk_bytes,
        chunk_count,
        last_chunk_bytes,
        per_chunk_timeout_ms: config.per_chunk_timeout_ms,
        reciprocal_per_chunk_timeout_ms: config.reciprocal_per_chunk_timeout_ms,
        round_timeout_ms: config.round_timeout_ms,
        plan_digest,
    })
}

/// Transfer path a chunk outcome was observed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectiveTransferPath {
    /// Member sends its tensor chunks toward the reducer.
    Send,
    /// Reducer sends reduced tensor chunks back to the member.
    ReciprocalReturn,
}

/// Observed status of one chunk transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkTransferStatus {
    /// The chunk arrived.
    Delivered,
    /// The chunk transfer exceeded its budget.
    TimedOut,
    /// The peer disconnected during the transfer.
    Disconnected,
}

/// One observed chunk-transfer outcome contributed by the caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkTransferOutcome {
    /// Member the outcome belongs to.
    pub member_node_id: String,
    /// Chunk index inside the plan.
    pub chunk_index: u64,
    /// Path the transfer ran on.
    pub path: CollectiveTransferPath,
    /// Observed transfer status.
    pub status: ChunkTransferStatus,
    /// Observed transfer duration in milliseconds.
    pub elapsed_ms: u64,
    /// Caller-supplied observation timestamp in milliseconds.
    pub observed_at_ms: u64,
}

impl ChunkTransferOutcome {
    /// Creates one chunk-transfer outcome from explicit observation facts.
    #[must_use]
    pub fn new(
        member_node_id: impl Into<String>,
        chunk_index: u64,
        path: CollectiveTransferPath,
        status: ChunkTransferStatus,
        elapsed_ms: u64,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            member_node_id: member_node_id.into(),
            chunk_index,
            path,
            status,
            elapsed_ms,
            observed_at_ms,
        }
    }
}

/// Typed reason one member was banned for the round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BanForRoundReason {
    /// A send-path chunk exceeded its per-chunk budget.
    SendChunkTimedOut,
    /// The peer disconnected on the send path.
    SendPeerDisconnected,
    /// A reciprocal return-path chunk exceeded its per-chunk budget.
    ReciprocalChunkTimedOut,
    /// The peer disconnected on the reciprocal return path.
    ReciprocalPeerDisconnected,
    /// The member had undelivered chunks when the full-round timeout fired.
    FullRoundTimeoutStall,
}

/// Receipt-compatible ban event. There is no retry: the member sits out the
/// rest of the round and the round continues.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BanForRoundEvent {
    /// Schema version of this record.
    pub schema_version: String,
    /// Round the ban applies to.
    pub round_id: String,
    /// Monotonic round index used by ban-escalation accounting.
    pub round_index: u64,
    /// Banned member.
    pub member_node_id: String,
    /// Chunk index the failure was observed on.
    pub chunk_index: u64,
    /// Path the failure was observed on.
    pub path: CollectiveTransferPath,
    /// Typed ban reason.
    pub reason: BanForRoundReason,
    /// Plain-language detail.
    pub detail: String,
    /// Caller-supplied ban timestamp in milliseconds.
    pub banned_at_ms: u64,
    /// Chunk-plan digest this ban references.
    pub plan_digest: String,
    /// Stable digest over the ban event.
    pub event_digest: String,
}

/// Disposition of one recorded chunk outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum ChunkOutcomeDisposition {
    /// The delivered chunk counts toward the partial reduce result.
    CountedTowardReduce,
    /// The failure banned the member for the rest of the round.
    BannedForRound {
        /// Emitted ban event.
        event: BanForRoundEvent,
    },
    /// The member was already banned; the outcome is logged and ignored.
    IgnoredMemberAlreadyBanned,
}

/// Typed reason one chunk was excluded from the reduce accounting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkExclusionReason {
    /// The transfer itself failed.
    FailedTransfer,
    /// The chunk was never delivered before the member's ban.
    ExcludedAfterBan,
    /// The chunk was never delivered before the round closed.
    NotDelivered,
}

/// Delivered chunks one member contributed to the partial reduce result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncludedContribution {
    /// Contributing member.
    pub member_node_id: String,
    /// Delivered send-path chunk indices included in the reduce.
    pub delivered_chunk_indices: Vec<u64>,
    /// Total delivered bytes included in the reduce.
    pub delivered_bytes: u64,
    /// Whether the member was banned after these chunks were delivered.
    pub banned_after_contribution: bool,
}

/// One chunk excluded from the reduce accounting, with its typed reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedContribution {
    /// Member the exclusion belongs to.
    pub member_node_id: String,
    /// Excluded chunk index.
    pub chunk_index: u64,
    /// Typed exclusion reason.
    pub reason: ChunkExclusionReason,
}

/// Typed reason behind one standby-promotion decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StandbyPromotionReason {
    /// A warm standby exists and can be promoted into the emptied stage.
    WarmStandbyAvailable,
    /// No warm standby is registered for the stage.
    NoWarmStandbyRegistered,
}

/// Collective-layer standby-promotion decision record. The monorepo-side
/// dispatcher consumes this record; this crate only decides whether the
/// emptied stage can be refilled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandbyPromotionDecision {
    /// Schema version of this record.
    pub schema_version: String,
    /// Round the decision belongs to.
    pub round_id: String,
    /// Stage that would otherwise empty.
    pub stage_id: String,
    /// Whether the stage would empty without a promotion.
    pub stage_would_empty: bool,
    /// Whether a warm standby can be promoted.
    pub promotable: bool,
    /// Standby selected for promotion when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promoted_node_id: Option<String>,
    /// Warm standbys visible at decision time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warm_standby_node_ids: Vec<String>,
    /// Typed decision reason.
    pub reason: StandbyPromotionReason,
    /// Caller-supplied decision timestamp in milliseconds.
    pub decided_at_ms: u64,
    /// Stable digest over the decision.
    pub decision_digest: String,
}

/// Typed abort reason for one collective round.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectiveRoundAbortReason {
    /// The stage emptied and no warm standby was promotable.
    NoStandbyPromotable,
    /// The full-round timeout emptied the stage and no standby was promotable.
    RoundTimeout,
}

/// Final verdict for one collective round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum CollectiveRoundVerdict {
    /// Every member delivered every chunk inside budget.
    Complete,
    /// The round kept its partial reduce result despite failures.
    Partial {
        /// Included chunk deliveries over expected deliveries, in basis points.
        preserved_fraction_bps: u64,
        /// Members banned for this round.
        banned_node_ids: Vec<String>,
        /// Standby promoted into the stage when the stage emptied.
        #[serde(skip_serializing_if = "Option::is_none")]
        promoted_standby_node_id: Option<String>,
    },
    /// The round aborted: the stage emptied and no standby was promotable.
    Aborted {
        /// Typed abort reason.
        reason: CollectiveRoundAbortReason,
        /// Plain-language detail.
        detail: String,
    },
}

/// Receipt-compatible record for one finalized collective round.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveRoundReceipt {
    /// Schema version of this record.
    pub schema_version: String,
    /// Round identifier.
    pub round_id: String,
    /// Stage identifier.
    pub stage_id: String,
    /// Monotonic round index used by ban-escalation accounting.
    pub round_index: u64,
    /// Chunk-plan digest the round executed against.
    pub plan_digest: String,
    /// Members the round started with.
    pub member_node_ids: Vec<String>,
    /// Caller-supplied round start timestamp in milliseconds.
    pub started_at_ms: u64,
    /// Caller-supplied round finalize timestamp in milliseconds.
    pub finalized_at_ms: u64,
    /// Whether the full-round timeout fired.
    pub round_timed_out: bool,
    /// Final round verdict.
    pub verdict: CollectiveRoundVerdict,
    /// Included chunk deliveries over expected deliveries, in basis points.
    pub preserved_fraction_bps: u64,
    /// Per-member contributions included in the partial reduce result.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub included_contributions: Vec<IncludedContribution>,
    /// Chunks excluded from the reduce accounting, with typed reasons.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_contributions: Vec<ExcludedContribution>,
    /// Ban events emitted during the round.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ban_events: Vec<BanForRoundEvent>,
    /// Standby-promotion decision when the stage would otherwise empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub standby_decision: Option<StandbyPromotionDecision>,
    /// Stable digest over the receipt.
    pub receipt_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MemberRoundState {
    delivered_send_chunks: BTreeSet<u64>,
    ban: Option<BanForRoundEvent>,
}

/// Deterministic round state machine for one chunked collective operation.
///
/// Callers feed observed chunk outcomes; the round bans first-failure members,
/// preserves partial contributions, and produces a receipt at finalize time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectiveFailureRound {
    round_id: String,
    stage_id: String,
    round_index: u64,
    plan: TensorChunkPlan,
    members: BTreeMap<String, MemberRoundState>,
    member_order: Vec<String>,
    started_at_ms: u64,
    ban_order: Vec<String>,
    finalized: bool,
}

impl CollectiveFailureRound {
    /// Starts one round over explicit members and a validated chunk plan.
    pub fn new(
        round_id: impl Into<String>,
        stage_id: impl Into<String>,
        round_index: u64,
        plan: TensorChunkPlan,
        member_node_ids: Vec<String>,
        started_at_ms: u64,
    ) -> Result<Self, CollectiveFailureSemanticsError> {
        let round_id = round_id.into();
        if member_node_ids.is_empty() {
            return Err(CollectiveFailureSemanticsError::EmptyRound { round_id });
        }
        let mut members = BTreeMap::new();
        for node_id in &member_node_ids {
            if members
                .insert(
                    node_id.clone(),
                    MemberRoundState {
                        delivered_send_chunks: BTreeSet::new(),
                        ban: None,
                    },
                )
                .is_some()
            {
                return Err(CollectiveFailureSemanticsError::DuplicateMember {
                    round_id,
                    node_id: node_id.clone(),
                });
            }
        }
        Ok(Self {
            round_id,
            stage_id: stage_id.into(),
            round_index,
            plan,
            members,
            member_order: member_node_ids,
            started_at_ms,
            ban_order: Vec::new(),
            finalized: false,
        })
    }

    /// Records one observed chunk outcome. The first failed chunk bans the
    /// member for the rest of the round; chunks delivered before the ban keep
    /// counting toward the partial reduce result.
    pub fn record_chunk_outcome(
        &mut self,
        outcome: &ChunkTransferOutcome,
    ) -> Result<ChunkOutcomeDisposition, CollectiveFailureSemanticsError> {
        if self.finalized {
            return Err(CollectiveFailureSemanticsError::RoundAlreadyFinalized {
                round_id: self.round_id.clone(),
            });
        }
        if outcome.chunk_index >= self.plan.chunk_count {
            return Err(CollectiveFailureSemanticsError::ChunkIndexOutOfRange {
                chunk_index: outcome.chunk_index,
                chunk_count: self.plan.chunk_count,
            });
        }
        let Some(state) = self.members.get_mut(&outcome.member_node_id) else {
            return Err(CollectiveFailureSemanticsError::UnknownMember {
                round_id: self.round_id.clone(),
                node_id: outcome.member_node_id.clone(),
            });
        };
        if state.ban.is_some() {
            return Ok(ChunkOutcomeDisposition::IgnoredMemberAlreadyBanned);
        }
        let budget_ms = match outcome.path {
            CollectiveTransferPath::Send => self.plan.per_chunk_timeout_ms,
            CollectiveTransferPath::ReciprocalReturn => self.plan.reciprocal_per_chunk_timeout_ms,
        };
        let failure_reason = match (outcome.status, outcome.path) {
            (ChunkTransferStatus::Delivered, _) if outcome.elapsed_ms <= budget_ms => None,
            (
                ChunkTransferStatus::Delivered | ChunkTransferStatus::TimedOut,
                CollectiveTransferPath::Send,
            ) => Some(BanForRoundReason::SendChunkTimedOut),
            (
                ChunkTransferStatus::Delivered | ChunkTransferStatus::TimedOut,
                CollectiveTransferPath::ReciprocalReturn,
            ) => Some(BanForRoundReason::ReciprocalChunkTimedOut),
            (ChunkTransferStatus::Disconnected, CollectiveTransferPath::Send) => {
                Some(BanForRoundReason::SendPeerDisconnected)
            }
            (ChunkTransferStatus::Disconnected, CollectiveTransferPath::ReciprocalReturn) => {
                Some(BanForRoundReason::ReciprocalPeerDisconnected)
            }
        };
        if let Some(reason) = failure_reason {
            let detail = format!(
                "member `{}` failed chunk {} on the {} path ({}ms elapsed against a {}ms budget); banned for round `{}`, no retry",
                outcome.member_node_id,
                outcome.chunk_index,
                transfer_path_text(outcome.path),
                outcome.elapsed_ms,
                budget_ms,
                self.round_id,
            );
            let event = build_ban_event(
                &self.round_id,
                self.round_index,
                &outcome.member_node_id,
                outcome.chunk_index,
                outcome.path,
                reason,
                detail,
                outcome.observed_at_ms,
                &self.plan.plan_digest,
            );
            state.ban = Some(event.clone());
            self.ban_order.push(outcome.member_node_id.clone());
            return Ok(ChunkOutcomeDisposition::BannedForRound { event });
        }
        if outcome.path == CollectiveTransferPath::Send {
            state.delivered_send_chunks.insert(outcome.chunk_index);
        }
        Ok(ChunkOutcomeDisposition::CountedTowardReduce)
    }

    /// Finalizes the round into a receipt. `warm_standby_node_ids` carries the
    /// caller-visible warm standbys used by the standby-promotion decision
    /// when the stage would otherwise empty.
    pub fn finalize(
        mut self,
        finalized_at_ms: u64,
        warm_standby_node_ids: &[String],
    ) -> Result<CollectiveRoundReceipt, CollectiveFailureSemanticsError> {
        if self.finalized {
            return Err(CollectiveFailureSemanticsError::RoundAlreadyFinalized {
                round_id: self.round_id.clone(),
            });
        }
        self.finalized = true;
        let round_timed_out =
            finalized_at_ms.saturating_sub(self.started_at_ms) > self.plan.round_timeout_ms;
        if round_timed_out {
            for node_id in self.member_order.clone() {
                let Some(state) = self.members.get_mut(&node_id) else {
                    continue;
                };
                if state.ban.is_some()
                    || state.delivered_send_chunks.len() as u64 == self.plan.chunk_count
                {
                    continue;
                }
                let first_missing = (0..self.plan.chunk_count)
                    .find(|index| !state.delivered_send_chunks.contains(index))
                    .unwrap_or(0);
                let detail = format!(
                    "member `{}` still owed chunk {} when the {}ms full-round timeout fired for round `{}`; banned for round, partial result retained",
                    node_id, first_missing, self.plan.round_timeout_ms, self.round_id,
                );
                let event = build_ban_event(
                    &self.round_id,
                    self.round_index,
                    &node_id,
                    first_missing,
                    CollectiveTransferPath::Send,
                    BanForRoundReason::FullRoundTimeoutStall,
                    detail,
                    finalized_at_ms,
                    &self.plan.plan_digest,
                );
                state.ban = Some(event);
                self.ban_order.push(node_id);
            }
        }

        let mut included_contributions = Vec::new();
        let mut excluded_contributions = Vec::new();
        let mut included_chunk_total = 0u64;
        for node_id in &self.member_order {
            let Some(state) = self.members.get(node_id) else {
                continue;
            };
            let delivered_chunk_indices: Vec<u64> =
                state.delivered_send_chunks.iter().copied().collect();
            let delivered_bytes = delivered_chunk_indices
                .iter()
                .map(|index| self.plan.chunk_bytes_at(*index))
                .sum();
            included_chunk_total += delivered_chunk_indices.len() as u64;
            if !delivered_chunk_indices.is_empty() {
                included_contributions.push(IncludedContribution {
                    member_node_id: node_id.clone(),
                    delivered_chunk_indices,
                    delivered_bytes,
                    banned_after_contribution: state.ban.is_some(),
                });
            }
            for chunk_index in 0..self.plan.chunk_count {
                if state.delivered_send_chunks.contains(&chunk_index) {
                    continue;
                }
                let reason = match &state.ban {
                    Some(ban)
                        if ban.chunk_index == chunk_index
                            && ban.reason != BanForRoundReason::FullRoundTimeoutStall =>
                    {
                        ChunkExclusionReason::FailedTransfer
                    }
                    Some(_) => ChunkExclusionReason::ExcludedAfterBan,
                    None => ChunkExclusionReason::NotDelivered,
                };
                excluded_contributions.push(ExcludedContribution {
                    member_node_id: node_id.clone(),
                    chunk_index,
                    reason,
                });
            }
        }

        let expected_chunk_total = self.member_order.len() as u64 * self.plan.chunk_count;
        let preserved_fraction_bps = if expected_chunk_total == 0 {
            0
        } else {
            included_chunk_total * FRACTION_DENOMINATOR_BPS / expected_chunk_total
        };

        let ban_events: Vec<BanForRoundEvent> = self
            .ban_order
            .iter()
            .filter_map(|node_id| {
                self.members
                    .get(node_id)
                    .and_then(|state| state.ban.clone())
            })
            .collect();
        let banned_node_ids: Vec<String> = self.ban_order.clone();
        let healthy_remaining = self
            .member_order
            .iter()
            .filter(|node_id| {
                self.members
                    .get(*node_id)
                    .is_some_and(|state| state.ban.is_none())
            })
            .count();

        let mut standby_decision = None;
        let verdict = if ban_events.is_empty() && excluded_contributions.is_empty() {
            CollectiveRoundVerdict::Complete
        } else if healthy_remaining > 0 {
            CollectiveRoundVerdict::Partial {
                preserved_fraction_bps,
                banned_node_ids: banned_node_ids.clone(),
                promoted_standby_node_id: None,
            }
        } else {
            let decision = build_standby_decision(
                &self.round_id,
                &self.stage_id,
                warm_standby_node_ids,
                finalized_at_ms,
            );
            let promoted = decision.promoted_node_id.clone();
            let promotable = decision.promotable;
            standby_decision = Some(decision);
            if promotable {
                CollectiveRoundVerdict::Partial {
                    preserved_fraction_bps,
                    banned_node_ids: banned_node_ids.clone(),
                    promoted_standby_node_id: promoted,
                }
            } else {
                let reason = if round_timed_out {
                    CollectiveRoundAbortReason::RoundTimeout
                } else {
                    CollectiveRoundAbortReason::NoStandbyPromotable
                };
                CollectiveRoundVerdict::Aborted {
                    reason,
                    detail: format!(
                        "stage `{}` emptied during round `{}` and no warm standby was promotable",
                        self.stage_id, self.round_id,
                    ),
                }
            }
        };

        let receipt_digest = stable_round_receipt_digest(
            &self.round_id,
            &self.stage_id,
            self.round_index,
            &self.plan.plan_digest,
            &self.member_order,
            self.started_at_ms,
            finalized_at_ms,
            round_timed_out,
            &verdict,
            preserved_fraction_bps,
            &ban_events,
            standby_decision.as_ref(),
        );
        Ok(CollectiveRoundReceipt {
            schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
            round_id: self.round_id,
            stage_id: self.stage_id,
            round_index: self.round_index,
            plan_digest: self.plan.plan_digest,
            member_node_ids: self.member_order,
            started_at_ms: self.started_at_ms,
            finalized_at_ms,
            round_timed_out,
            verdict,
            preserved_fraction_bps,
            included_contributions,
            excluded_contributions,
            ban_events,
            standby_decision,
            receipt_digest,
        })
    }
}

/// Receipt-compatible removal recommendation for one persistently failing
/// member. The monorepo-side dispatcher owns the actual lease/ban bookkeeping.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SwarmRemovalRecommendation {
    /// Schema version of this record.
    pub schema_version: String,
    /// Member recommended for removal from the swarm.
    pub member_node_id: String,
    /// Round bans counted inside the policy window.
    pub ban_count_in_window: u32,
    /// Policy threshold that was reached.
    pub removal_ban_threshold: u32,
    /// Policy window the threshold was evaluated over.
    pub window_rounds: u64,
    /// Round indices of the counted bans.
    pub ban_round_indices: Vec<u64>,
    /// Digests of the counted ban events.
    pub ban_event_digests: Vec<String>,
    /// Caller-supplied recommendation timestamp in milliseconds.
    pub recommended_at_ms: u64,
    /// Stable digest over the recommendation.
    pub recommendation_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct LedgerBanRecord {
    member_node_id: String,
    round_index: u64,
    reason: BanForRoundReason,
    ban_event_digest: String,
}

/// Typed ledger of per-member round bans with escalation to removal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveBanLedger {
    /// Escalation policy the ledger evaluates.
    pub policy: BanEscalationPolicy,
    bans: Vec<LedgerBanRecord>,
    recommended_node_ids: BTreeSet<String>,
}

impl CollectiveBanLedger {
    /// Creates one ledger under a validated escalation policy.
    pub fn new(policy: BanEscalationPolicy) -> Result<Self, CollectiveFailureSemanticsError> {
        policy.validate()?;
        Ok(Self {
            policy,
            bans: Vec::new(),
            recommended_node_ids: BTreeSet::new(),
        })
    }

    /// Records every ban in one round receipt and returns any new removal
    /// recommendations the bans escalate into.
    pub fn record_round_receipt(
        &mut self,
        receipt: &CollectiveRoundReceipt,
        recommended_at_ms: u64,
    ) -> Vec<SwarmRemovalRecommendation> {
        let mut recommendations = Vec::new();
        for event in &receipt.ban_events {
            self.bans.push(LedgerBanRecord {
                member_node_id: event.member_node_id.clone(),
                round_index: event.round_index,
                reason: event.reason,
                ban_event_digest: event.event_digest.clone(),
            });
            if self.recommended_node_ids.contains(&event.member_node_id) {
                continue;
            }
            let window_start = receipt
                .round_index
                .saturating_sub(self.policy.window_rounds.saturating_sub(1));
            let in_window: Vec<&LedgerBanRecord> = self
                .bans
                .iter()
                .filter(|ban| {
                    ban.member_node_id == event.member_node_id && ban.round_index >= window_start
                })
                .collect();
            let ban_count = in_window.len() as u32;
            if ban_count >= self.policy.removal_ban_threshold {
                let ban_round_indices: Vec<u64> =
                    in_window.iter().map(|ban| ban.round_index).collect();
                let ban_event_digests: Vec<String> = in_window
                    .iter()
                    .map(|ban| ban.ban_event_digest.clone())
                    .collect();
                let recommendation_digest = stable_removal_recommendation_digest(
                    &event.member_node_id,
                    ban_count,
                    self.policy,
                    &ban_round_indices,
                    &ban_event_digests,
                    recommended_at_ms,
                );
                self.recommended_node_ids
                    .insert(event.member_node_id.clone());
                recommendations.push(SwarmRemovalRecommendation {
                    schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
                    member_node_id: event.member_node_id.clone(),
                    ban_count_in_window: ban_count,
                    removal_ban_threshold: self.policy.removal_ban_threshold,
                    window_rounds: self.policy.window_rounds,
                    ban_round_indices,
                    ban_event_digests,
                    recommended_at_ms,
                    recommendation_digest,
                });
            }
        }
        recommendations
    }
}

/// Authority paths bound into the committed fixture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureSemanticsAuthorityPaths {
    /// Fixture generator binary.
    pub generator_bin: String,
    /// Repo-local check script.
    pub check_script_path: String,
    /// Canonical reference doc.
    pub reference_doc_path: String,
}

/// Committed fixture bundle exercising the failure-semantics contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectiveFailureSemanticsContract {
    /// Schema version of this record.
    pub schema_version: String,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Issue this contract is the canonical record for.
    pub issue_ref: String,
    /// Upstream reference the semantics were ported from.
    pub upstream_reference: String,
    /// Round config used by every simulated round.
    pub config: CollectiveRoundConfig,
    /// Ban-escalation policy used by the ledger.
    pub ban_escalation_policy: BanEscalationPolicy,
    /// Chunk plan shared by the simulated rounds.
    pub chunk_plan: TensorChunkPlan,
    /// Receipts for every simulated round.
    pub round_receipts: Vec<CollectiveRoundReceipt>,
    /// Removal recommendations escalated out of the simulated rounds.
    pub removal_recommendations: Vec<SwarmRemovalRecommendation>,
    /// Authority paths for regeneration and verification.
    pub authority_paths: FailureSemanticsAuthorityPaths,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

/// Errors returned while writing the committed fixture.
#[derive(Debug, Error)]
pub enum FailureSemanticsContractWriteError {
    /// The canonical scenario failed to build.
    #[error(transparent)]
    Semantics(#[from] CollectiveFailureSemanticsError),
    /// Output directory creation failed.
    #[error("failed to create fixture directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Underlying IO error.
        error: std::io::Error,
    },
    /// Fixture serialization failed.
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    /// Fixture write failed.
    #[error("failed to write fixture `{path}`: {error}")]
    Write {
        /// Fixture path.
        path: String,
        /// Underlying IO error.
        error: std::io::Error,
    },
}

const CANONICAL_MEMBERS: [&str; 3] = ["worker-a", "worker-b", "worker-c"];
const CANONICAL_STAGE: &str = "pipeline_stage_0";
const CANONICAL_TENSOR_BYTES: u64 = 30 * 1024 * 1024;
const CANONICAL_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
const CANONICAL_SEND_BUDGET_MS: u64 = 1_500;
const CANONICAL_RECIPROCAL_BUDGET_MS: u64 = 2_000;
const CANONICAL_ROUND_TIMEOUT_MS: u64 = 30_000;
const CANONICAL_REMOVAL_THRESHOLD: u32 = 3;
const CANONICAL_WINDOW_ROUNDS: u64 = 8;

fn deliver_all_send_chunks(
    round: &mut CollectiveFailureRound,
    plan: &TensorChunkPlan,
    node_id: &str,
    base_ts_ms: u64,
) -> Result<(), CollectiveFailureSemanticsError> {
    for chunk_index in 0..plan.chunk_count {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            node_id,
            chunk_index,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Delivered,
            40,
            base_ts_ms + chunk_index * 50,
        ))?;
    }
    Ok(())
}

/// Builds the canonical deterministic fixture scenario for psionic#1126.
pub fn canonical_collective_failure_semantics_contract()
-> Result<CollectiveFailureSemanticsContract, CollectiveFailureSemanticsError> {
    let config = CollectiveRoundConfig::new(
        CANONICAL_CHUNK_BYTES,
        CANONICAL_SEND_BUDGET_MS,
        CANONICAL_RECIPROCAL_BUDGET_MS,
        CANONICAL_ROUND_TIMEOUT_MS,
    );
    let policy = BanEscalationPolicy::new(CANONICAL_REMOVAL_THRESHOLD, CANONICAL_WINDOW_ROUNDS);
    let plan = plan_tensor_chunks(CANONICAL_TENSOR_BYTES, &config)?;
    let members: Vec<String> = CANONICAL_MEMBERS.iter().map(ToString::to_string).collect();
    let mut ledger = CollectiveBanLedger::new(policy)?;
    let mut round_receipts = Vec::new();
    let mut removal_recommendations = Vec::new();
    let no_standby: Vec<String> = Vec::new();
    let warm_standby = vec![String::from("standby-w1")];

    // Round 1: happy path, every chunk delivered on both paths.
    let mut round = CollectiveFailureRound::new(
        "round-001",
        CANONICAL_STAGE,
        1,
        plan.clone(),
        members.clone(),
        1_000,
    )?;
    for node_id in &members {
        deliver_all_send_chunks(&mut round, &plan, node_id, 1_000)?;
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            node_id,
            0,
            CollectiveTransferPath::ReciprocalReturn,
            ChunkTransferStatus::Delivered,
            60,
            1_900,
        ))?;
    }
    round_receipts.push(round.finalize(2_000, &no_standby)?);

    // Round 2: worker-b fails send chunk 2 of 8; chunks 0-1 stay preserved.
    let mut round = CollectiveFailureRound::new(
        "round-002",
        CANONICAL_STAGE,
        2,
        plan.clone(),
        members.clone(),
        10_000,
    )?;
    deliver_all_send_chunks(&mut round, &plan, "worker-a", 10_000)?;
    deliver_all_send_chunks(&mut round, &plan, "worker-c", 10_000)?;
    for chunk_index in 0..2 {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            "worker-b",
            chunk_index,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Delivered,
            45,
            10_000 + chunk_index * 50,
        ))?;
    }
    round.record_chunk_outcome(&ChunkTransferOutcome::new(
        "worker-b",
        2,
        CollectiveTransferPath::Send,
        ChunkTransferStatus::TimedOut,
        1_650,
        10_200,
    ))?;
    round_receipts.push(round.finalize(11_000, &no_standby)?);

    // Round 3: worker-b times out on the reciprocal return path after
    // delivering six send chunks; those six stay preserved.
    let mut round = CollectiveFailureRound::new(
        "round-003",
        CANONICAL_STAGE,
        3,
        plan.clone(),
        members.clone(),
        20_000,
    )?;
    deliver_all_send_chunks(&mut round, &plan, "worker-a", 20_000)?;
    deliver_all_send_chunks(&mut round, &plan, "worker-c", 20_000)?;
    for chunk_index in 0..6 {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            "worker-b",
            chunk_index,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Delivered,
            45,
            20_000 + chunk_index * 50,
        ))?;
    }
    round.record_chunk_outcome(&ChunkTransferOutcome::new(
        "worker-b",
        0,
        CollectiveTransferPath::ReciprocalReturn,
        ChunkTransferStatus::TimedOut,
        2_200,
        20_400,
    ))?;
    round_receipts.push(round.finalize(21_000, &no_standby)?);

    // Round 4: worker-b disconnects immediately; third ban inside the window
    // escalates to a removal recommendation.
    let mut round = CollectiveFailureRound::new(
        "round-004",
        CANONICAL_STAGE,
        4,
        plan.clone(),
        members.clone(),
        30_000,
    )?;
    deliver_all_send_chunks(&mut round, &plan, "worker-a", 30_000)?;
    deliver_all_send_chunks(&mut round, &plan, "worker-c", 30_000)?;
    round.record_chunk_outcome(&ChunkTransferOutcome::new(
        "worker-b",
        0,
        CollectiveTransferPath::Send,
        ChunkTransferStatus::Disconnected,
        10,
        30_050,
    ))?;
    round_receipts.push(round.finalize(31_000, &no_standby)?);

    // Round 5: every member disconnects and no standby exists; the stage
    // would empty, so the round aborts.
    let mut round = CollectiveFailureRound::new(
        "round-005",
        CANONICAL_STAGE,
        5,
        plan.clone(),
        members.clone(),
        40_000,
    )?;
    for node_id in &members {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            node_id,
            0,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Disconnected,
            10,
            40_050,
        ))?;
    }
    round_receipts.push(round.finalize(41_000, &no_standby)?);

    // Round 6: same stage-empty failure, but a warm standby is promotable,
    // so the round stays Partial instead of aborting.
    let mut round = CollectiveFailureRound::new(
        "round-006",
        CANONICAL_STAGE,
        6,
        plan.clone(),
        members.clone(),
        50_000,
    )?;
    for node_id in &members {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            node_id,
            0,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Disconnected,
            10,
            50_050,
        ))?;
    }
    round_receipts.push(round.finalize(51_000, &warm_standby)?);

    // Round 7: worker-c stalls and the full-round timeout fires; healthy
    // contributions and worker-c's four delivered chunks stay preserved, and
    // worker-c's third windowed ban escalates to removal.
    let mut round = CollectiveFailureRound::new(
        "round-007",
        CANONICAL_STAGE,
        7,
        plan.clone(),
        members.clone(),
        60_000,
    )?;
    deliver_all_send_chunks(&mut round, &plan, "worker-a", 60_000)?;
    deliver_all_send_chunks(&mut round, &plan, "worker-b", 60_000)?;
    for chunk_index in 0..4 {
        round.record_chunk_outcome(&ChunkTransferOutcome::new(
            "worker-c",
            chunk_index,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::Delivered,
            45,
            60_000 + chunk_index * 50,
        ))?;
    }
    round_receipts.push(round.finalize(60_000 + CANONICAL_ROUND_TIMEOUT_MS + 1, &no_standby)?);

    for receipt in &round_receipts {
        let recommended_at_ms = receipt.finalized_at_ms + 100;
        removal_recommendations.extend(ledger.record_round_receipt(receipt, recommended_at_ms));
    }

    let authority_paths = FailureSemanticsAuthorityPaths {
        generator_bin: String::from(
            "cargo run -p psionic-collectives --bin collective_failure_semantics_contract",
        ),
        check_script_path: String::from("scripts/check-collective-failure-semantics.sh"),
        reference_doc_path: String::from("docs/PSIONIC_COLLECTIVE_FAILURE_SEMANTICS.md"),
    };
    let contract_digest = stable_contract_digest(
        &config,
        policy,
        &plan,
        &round_receipts,
        &removal_recommendations,
        &authority_paths,
    );
    Ok(CollectiveFailureSemanticsContract {
        schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
        contract_id: FAILURE_SEMANTICS_CONTRACT_ID.to_string(),
        issue_ref: String::from("psionic#1126"),
        upstream_reference: String::from(
            "pluralis agora docs/agora-system/fault-tolerance.md (read-only reference lane projects/pluralis/repos/agora)",
        ),
        config,
        ban_escalation_policy: policy,
        chunk_plan: plan,
        round_receipts,
        removal_recommendations,
        authority_paths,
        contract_digest,
    })
}

/// Writes the canonical fixture to `output_path` and returns the contract.
pub fn write_collective_failure_semantics_contract(
    output_path: impl AsRef<Path>,
) -> Result<CollectiveFailureSemanticsContract, FailureSemanticsContractWriteError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FailureSemanticsContractWriteError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_collective_failure_semantics_contract()?;
    let mut json = serde_json::to_vec_pretty(&contract)?;
    json.push(b'\n');
    fs::write(output_path, json).map_err(|error| FailureSemanticsContractWriteError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(contract)
}

#[allow(clippy::too_many_arguments)]
fn build_ban_event(
    round_id: &str,
    round_index: u64,
    member_node_id: &str,
    chunk_index: u64,
    path: CollectiveTransferPath,
    reason: BanForRoundReason,
    detail: String,
    banned_at_ms: u64,
    plan_digest: &str,
) -> BanForRoundEvent {
    let event_digest = stable_ban_event_digest(
        round_id,
        round_index,
        member_node_id,
        chunk_index,
        path,
        reason,
        banned_at_ms,
        plan_digest,
    );
    BanForRoundEvent {
        schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
        round_id: round_id.to_string(),
        round_index,
        member_node_id: member_node_id.to_string(),
        chunk_index,
        path,
        reason,
        detail,
        banned_at_ms,
        plan_digest: plan_digest.to_string(),
        event_digest,
    }
}

fn build_standby_decision(
    round_id: &str,
    stage_id: &str,
    warm_standby_node_ids: &[String],
    decided_at_ms: u64,
) -> StandbyPromotionDecision {
    let mut sorted: Vec<String> = warm_standby_node_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let promoted_node_id = if sorted.is_empty() {
        None
    } else {
        Some(sorted.remove(0))
    };
    let mut warm_sorted: Vec<String> = warm_standby_node_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    warm_sorted.sort();
    let promotable = promoted_node_id.is_some();
    let reason = if promotable {
        StandbyPromotionReason::WarmStandbyAvailable
    } else {
        StandbyPromotionReason::NoWarmStandbyRegistered
    };
    let decision_digest = stable_standby_decision_digest(
        round_id,
        stage_id,
        promotable,
        promoted_node_id.as_deref(),
        &warm_sorted,
        reason,
        decided_at_ms,
    );
    StandbyPromotionDecision {
        schema_version: FAILURE_SEMANTICS_SCHEMA_VERSION.to_string(),
        round_id: round_id.to_string(),
        stage_id: stage_id.to_string(),
        stage_would_empty: true,
        promotable,
        promoted_node_id,
        warm_standby_node_ids: warm_sorted,
        reason,
        decided_at_ms,
        decision_digest,
    }
}

const fn transfer_path_text(path: CollectiveTransferPath) -> &'static str {
    match path {
        CollectiveTransferPath::Send => "send",
        CollectiveTransferPath::ReciprocalReturn => "reciprocal_return",
    }
}

const fn ban_reason_text(reason: BanForRoundReason) -> &'static str {
    match reason {
        BanForRoundReason::SendChunkTimedOut => "send_chunk_timed_out",
        BanForRoundReason::SendPeerDisconnected => "send_peer_disconnected",
        BanForRoundReason::ReciprocalChunkTimedOut => "reciprocal_chunk_timed_out",
        BanForRoundReason::ReciprocalPeerDisconnected => "reciprocal_peer_disconnected",
        BanForRoundReason::FullRoundTimeoutStall => "full_round_timeout_stall",
    }
}

const fn standby_reason_text(reason: StandbyPromotionReason) -> &'static str {
    match reason {
        StandbyPromotionReason::WarmStandbyAvailable => "warm_standby_available",
        StandbyPromotionReason::NoWarmStandbyRegistered => "no_warm_standby_registered",
    }
}

const fn abort_reason_text(reason: CollectiveRoundAbortReason) -> &'static str {
    match reason {
        CollectiveRoundAbortReason::NoStandbyPromotable => "no_standby_promotable",
        CollectiveRoundAbortReason::RoundTimeout => "round_timeout",
    }
}

fn stable_chunk_plan_digest(
    tensor_bytes: u64,
    config: &CollectiveRoundConfig,
    chunk_count: u64,
    last_chunk_bytes: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_chunk_plan|");
    hasher.update(tensor_bytes.to_string().as_bytes());
    hasher.update(b"|chunk_bytes|");
    hasher.update(config.chunk_bytes.to_string().as_bytes());
    hasher.update(b"|chunk_count|");
    hasher.update(chunk_count.to_string().as_bytes());
    hasher.update(b"|last_chunk_bytes|");
    hasher.update(last_chunk_bytes.to_string().as_bytes());
    hasher.update(b"|per_chunk_timeout_ms|");
    hasher.update(config.per_chunk_timeout_ms.to_string().as_bytes());
    hasher.update(b"|reciprocal_per_chunk_timeout_ms|");
    hasher.update(
        config
            .reciprocal_per_chunk_timeout_ms
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|round_timeout_ms|");
    hasher.update(config.round_timeout_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn stable_ban_event_digest(
    round_id: &str,
    round_index: u64,
    member_node_id: &str,
    chunk_index: u64,
    path: CollectiveTransferPath,
    reason: BanForRoundReason,
    banned_at_ms: u64,
    plan_digest: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_ban_for_round|");
    hasher.update(round_id.as_bytes());
    hasher.update(b"|round_index|");
    hasher.update(round_index.to_string().as_bytes());
    hasher.update(b"|member|");
    hasher.update(member_node_id.as_bytes());
    hasher.update(b"|chunk|");
    hasher.update(chunk_index.to_string().as_bytes());
    hasher.update(b"|path|");
    hasher.update(transfer_path_text(path).as_bytes());
    hasher.update(b"|reason|");
    hasher.update(ban_reason_text(reason).as_bytes());
    hasher.update(b"|banned_at_ms|");
    hasher.update(banned_at_ms.to_string().as_bytes());
    hasher.update(b"|plan|");
    hasher.update(plan_digest.as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn stable_standby_decision_digest(
    round_id: &str,
    stage_id: &str,
    promotable: bool,
    promoted_node_id: Option<&str>,
    warm_standby_node_ids: &[String],
    reason: StandbyPromotionReason,
    decided_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_standby_promotion_decision|");
    hasher.update(round_id.as_bytes());
    hasher.update(b"|stage|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|promotable|");
    hasher.update(if promotable {
        b"yes".as_slice()
    } else {
        b"no".as_slice()
    });
    hasher.update(b"|promoted|");
    hasher.update(promoted_node_id.unwrap_or("none").as_bytes());
    for node_id in warm_standby_node_ids {
        hasher.update(b"|standby|");
        hasher.update(node_id.as_bytes());
    }
    hasher.update(b"|reason|");
    hasher.update(standby_reason_text(reason).as_bytes());
    hasher.update(b"|decided_at_ms|");
    hasher.update(decided_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn stable_round_receipt_digest(
    round_id: &str,
    stage_id: &str,
    round_index: u64,
    plan_digest: &str,
    member_node_ids: &[String],
    started_at_ms: u64,
    finalized_at_ms: u64,
    round_timed_out: bool,
    verdict: &CollectiveRoundVerdict,
    preserved_fraction_bps: u64,
    ban_events: &[BanForRoundEvent],
    standby_decision: Option<&StandbyPromotionDecision>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_round_receipt|");
    hasher.update(round_id.as_bytes());
    hasher.update(b"|stage|");
    hasher.update(stage_id.as_bytes());
    hasher.update(b"|round_index|");
    hasher.update(round_index.to_string().as_bytes());
    hasher.update(b"|plan|");
    hasher.update(plan_digest.as_bytes());
    for node_id in member_node_ids {
        hasher.update(b"|member|");
        hasher.update(node_id.as_bytes());
    }
    hasher.update(b"|started_at_ms|");
    hasher.update(started_at_ms.to_string().as_bytes());
    hasher.update(b"|finalized_at_ms|");
    hasher.update(finalized_at_ms.to_string().as_bytes());
    hasher.update(b"|round_timed_out|");
    hasher.update(if round_timed_out {
        b"yes".as_slice()
    } else {
        b"no".as_slice()
    });
    match verdict {
        CollectiveRoundVerdict::Complete => {
            hasher.update(b"|verdict|complete");
        }
        CollectiveRoundVerdict::Partial {
            preserved_fraction_bps,
            banned_node_ids,
            promoted_standby_node_id,
        } => {
            hasher.update(b"|verdict|partial|");
            hasher.update(preserved_fraction_bps.to_string().as_bytes());
            for node_id in banned_node_ids {
                hasher.update(b"|banned|");
                hasher.update(node_id.as_bytes());
            }
            hasher.update(b"|promoted|");
            hasher.update(
                promoted_standby_node_id
                    .as_deref()
                    .unwrap_or("none")
                    .as_bytes(),
            );
        }
        CollectiveRoundVerdict::Aborted { reason, detail } => {
            hasher.update(b"|verdict|aborted|");
            hasher.update(abort_reason_text(*reason).as_bytes());
            hasher.update(b"|");
            hasher.update(detail.as_bytes());
        }
    }
    hasher.update(b"|preserved_fraction_bps|");
    hasher.update(preserved_fraction_bps.to_string().as_bytes());
    for event in ban_events {
        hasher.update(b"|ban|");
        hasher.update(event.event_digest.as_bytes());
    }
    if let Some(decision) = standby_decision {
        hasher.update(b"|standby_decision|");
        hasher.update(decision.decision_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

fn stable_removal_recommendation_digest(
    member_node_id: &str,
    ban_count_in_window: u32,
    policy: BanEscalationPolicy,
    ban_round_indices: &[u64],
    ban_event_digests: &[String],
    recommended_at_ms: u64,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_swarm_removal_recommendation|");
    hasher.update(member_node_id.as_bytes());
    hasher.update(b"|count|");
    hasher.update(ban_count_in_window.to_string().as_bytes());
    hasher.update(b"|threshold|");
    hasher.update(policy.removal_ban_threshold.to_string().as_bytes());
    hasher.update(b"|window|");
    hasher.update(policy.window_rounds.to_string().as_bytes());
    for round_index in ban_round_indices {
        hasher.update(b"|round|");
        hasher.update(round_index.to_string().as_bytes());
    }
    for digest in ban_event_digests {
        hasher.update(b"|ban|");
        hasher.update(digest.as_bytes());
    }
    hasher.update(b"|recommended_at_ms|");
    hasher.update(recommended_at_ms.to_string().as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_contract_digest(
    config: &CollectiveRoundConfig,
    policy: BanEscalationPolicy,
    plan: &TensorChunkPlan,
    round_receipts: &[CollectiveRoundReceipt],
    removal_recommendations: &[SwarmRemovalRecommendation],
    authority_paths: &FailureSemanticsAuthorityPaths,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psionic_collective_failure_semantics_contract|");
    hasher.update(FAILURE_SEMANTICS_CONTRACT_ID.as_bytes());
    hasher.update(b"|chunk_bytes|");
    hasher.update(config.chunk_bytes.to_string().as_bytes());
    hasher.update(b"|per_chunk_timeout_ms|");
    hasher.update(config.per_chunk_timeout_ms.to_string().as_bytes());
    hasher.update(b"|reciprocal_per_chunk_timeout_ms|");
    hasher.update(
        config
            .reciprocal_per_chunk_timeout_ms
            .to_string()
            .as_bytes(),
    );
    hasher.update(b"|round_timeout_ms|");
    hasher.update(config.round_timeout_ms.to_string().as_bytes());
    hasher.update(b"|removal_ban_threshold|");
    hasher.update(policy.removal_ban_threshold.to_string().as_bytes());
    hasher.update(b"|window_rounds|");
    hasher.update(policy.window_rounds.to_string().as_bytes());
    hasher.update(b"|plan|");
    hasher.update(plan.plan_digest.as_bytes());
    for receipt in round_receipts {
        hasher.update(b"|round|");
        hasher.update(receipt.receipt_digest.as_bytes());
    }
    for recommendation in removal_recommendations {
        hasher.update(b"|removal|");
        hasher.update(recommendation.recommendation_digest.as_bytes());
    }
    hasher.update(b"|generator|");
    hasher.update(authority_paths.generator_bin.as_bytes());
    hasher.update(b"|check_script|");
    hasher.update(authority_paths.check_script_path.as_bytes());
    hasher.update(b"|reference_doc|");
    hasher.update(authority_paths.reference_doc_path.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: CollectiveRoundConfig =
        CollectiveRoundConfig::new(4 * 1024 * 1024, 1_500, 2_000, 30_000);

    fn members() -> Vec<String> {
        vec![
            String::from("worker-a"),
            String::from("worker-b"),
            String::from("worker-c"),
        ]
    }

    fn eight_chunk_plan() -> Result<TensorChunkPlan, CollectiveFailureSemanticsError> {
        // 30 MiB over 4 MiB chunks: eight chunks, final chunk 2 MiB.
        plan_tensor_chunks(30 * 1024 * 1024, &TEST_CONFIG)
    }

    fn delivered(
        node_id: &str,
        chunk_index: u64,
        path: CollectiveTransferPath,
    ) -> ChunkTransferOutcome {
        ChunkTransferOutcome::new(
            node_id,
            chunk_index,
            path,
            ChunkTransferStatus::Delivered,
            40,
            1_000 + chunk_index * 10,
        )
    }

    fn deliver_all(
        round: &mut CollectiveFailureRound,
        plan: &TensorChunkPlan,
        node_id: &str,
    ) -> Result<(), CollectiveFailureSemanticsError> {
        for chunk_index in 0..plan.chunk_count {
            round.record_chunk_outcome(&delivered(
                node_id,
                chunk_index,
                CollectiveTransferPath::Send,
            ))?;
        }
        Ok(())
    }

    #[test]
    fn chunk_plan_is_config_driven() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        assert_eq!(plan.chunk_count, 8);
        assert_eq!(plan.chunk_bytes, 4 * 1024 * 1024);
        assert_eq!(plan.last_chunk_bytes, 2 * 1024 * 1024);
        assert_eq!(plan.per_chunk_timeout_ms, 1_500);
        assert_eq!(plan.reciprocal_per_chunk_timeout_ms, 2_000);
        assert_eq!(plan.round_timeout_ms, 30_000);
        assert_eq!(plan.chunk_bytes_at(0), 4 * 1024 * 1024);
        assert_eq!(plan.chunk_bytes_at(7), 2 * 1024 * 1024);
        assert_eq!(plan.chunk_bytes_at(8), 0);

        let other_config = CollectiveRoundConfig::new(8 * 1024 * 1024, 900, 900, 10_000);
        let other_plan = plan_tensor_chunks(30 * 1024 * 1024, &other_config)?;
        assert_eq!(other_plan.chunk_count, 4);
        assert_eq!(other_plan.last_chunk_bytes, 6 * 1024 * 1024);
        assert_ne!(other_plan.plan_digest, plan.plan_digest);

        let zero_chunk = CollectiveRoundConfig::new(0, 1, 1, 1);
        assert_eq!(
            plan_tensor_chunks(1024, &zero_chunk),
            Err(CollectiveFailureSemanticsError::ZeroConfigField {
                field: "chunk_bytes"
            })
        );
        assert_eq!(
            plan_tensor_chunks(0, &TEST_CONFIG),
            Err(CollectiveFailureSemanticsError::EmptyTensor)
        );
        Ok(())
    }

    #[test]
    fn happy_round_completes_with_full_preservation() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-1", "stage-0", 1, plan.clone(), members(), 1_000)?;
        for node_id in members() {
            deliver_all(&mut round, &plan, &node_id)?;
            round.record_chunk_outcome(&delivered(
                &node_id,
                0,
                CollectiveTransferPath::ReciprocalReturn,
            ))?;
        }
        let receipt = round.finalize(2_000, &[])?;
        assert_eq!(receipt.verdict, CollectiveRoundVerdict::Complete);
        assert_eq!(receipt.preserved_fraction_bps, 10_000);
        assert!(receipt.ban_events.is_empty());
        assert!(receipt.excluded_contributions.is_empty());
        assert!(!receipt.round_timed_out);
        assert_eq!(receipt.included_contributions.len(), 3);
        assert_eq!(
            receipt.included_contributions[0].delivered_bytes,
            30 * 1024 * 1024
        );
        Ok(())
    }

    #[test]
    fn first_failed_chunk_bans_member_and_preserves_prior_chunks()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-2", "stage-0", 2, plan.clone(), members(), 1_000)?;
        deliver_all(&mut round, &plan, "worker-a")?;
        deliver_all(&mut round, &plan, "worker-c")?;
        // worker-b delivers chunks 0-1, then fails chunk 2 of 8.
        round.record_chunk_outcome(&delivered("worker-b", 0, CollectiveTransferPath::Send))?;
        round.record_chunk_outcome(&delivered("worker-b", 1, CollectiveTransferPath::Send))?;
        let disposition = round.record_chunk_outcome(&ChunkTransferOutcome::new(
            "worker-b",
            2,
            CollectiveTransferPath::Send,
            ChunkTransferStatus::TimedOut,
            1_650,
            1_200,
        ))?;
        let ChunkOutcomeDisposition::BannedForRound { event } = disposition else {
            return Err("expected ban-for-round disposition".into());
        };
        assert_eq!(event.reason, BanForRoundReason::SendChunkTimedOut);
        assert_eq!(event.member_node_id, "worker-b");
        assert_eq!(event.chunk_index, 2);

        // Post-ban outcomes are ignored, not retried.
        let ignored =
            round.record_chunk_outcome(&delivered("worker-b", 3, CollectiveTransferPath::Send))?;
        assert_eq!(ignored, ChunkOutcomeDisposition::IgnoredMemberAlreadyBanned);

        let receipt = round.finalize(2_000, &[])?;
        let CollectiveRoundVerdict::Partial {
            preserved_fraction_bps,
            banned_node_ids,
            promoted_standby_node_id,
        } = &receipt.verdict
        else {
            return Err("expected partial verdict".into());
        };
        // 8 + 8 + 2 delivered chunks over 24 expected.
        assert_eq!(*preserved_fraction_bps, 7_500);
        assert_eq!(banned_node_ids, &vec![String::from("worker-b")]);
        assert_eq!(*promoted_standby_node_id, None);

        let banned_contribution = receipt
            .included_contributions
            .iter()
            .find(|contribution| contribution.member_node_id == "worker-b")
            .ok_or("worker-b contribution missing")?;
        assert_eq!(banned_contribution.delivered_chunk_indices, vec![0, 1]);
        assert!(banned_contribution.banned_after_contribution);

        let failed_exclusion = receipt
            .excluded_contributions
            .iter()
            .find(|exclusion| exclusion.member_node_id == "worker-b" && exclusion.chunk_index == 2)
            .ok_or("failed-chunk exclusion missing")?;
        assert_eq!(
            failed_exclusion.reason,
            ChunkExclusionReason::FailedTransfer
        );
        let after_ban_exclusions = receipt
            .excluded_contributions
            .iter()
            .filter(|exclusion| exclusion.reason == ChunkExclusionReason::ExcludedAfterBan)
            .count();
        assert_eq!(after_ban_exclusions, 5);
        Ok(())
    }

    #[test]
    fn reciprocal_path_timeout_uses_same_ban_semantics() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-3", "stage-0", 3, plan.clone(), members(), 1_000)?;
        deliver_all(&mut round, &plan, "worker-a")?;
        deliver_all(&mut round, &plan, "worker-c")?;
        for chunk_index in 0..6 {
            round.record_chunk_outcome(&delivered(
                "worker-b",
                chunk_index,
                CollectiveTransferPath::Send,
            ))?;
        }
        // Reciprocal return outcome over the 2000ms budget bans the member.
        let disposition = round.record_chunk_outcome(&ChunkTransferOutcome::new(
            "worker-b",
            0,
            CollectiveTransferPath::ReciprocalReturn,
            ChunkTransferStatus::Delivered,
            2_200,
            1_400,
        ))?;
        let ChunkOutcomeDisposition::BannedForRound { event } = disposition else {
            return Err("expected ban-for-round disposition".into());
        };
        assert_eq!(event.reason, BanForRoundReason::ReciprocalChunkTimedOut);
        assert_eq!(event.path, CollectiveTransferPath::ReciprocalReturn);

        let receipt = round.finalize(2_000, &[])?;
        let CollectiveRoundVerdict::Partial {
            preserved_fraction_bps,
            banned_node_ids,
            ..
        } = &receipt.verdict
        else {
            return Err("expected partial verdict".into());
        };
        // worker-b's six delivered send chunks stay preserved: 22 of 24.
        assert_eq!(*preserved_fraction_bps, 9_166);
        assert_eq!(banned_node_ids, &vec![String::from("worker-b")]);
        Ok(())
    }

    #[test]
    fn full_round_timeout_preserves_partial_result() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-4", "stage-0", 4, plan.clone(), members(), 1_000)?;
        deliver_all(&mut round, &plan, "worker-a")?;
        deliver_all(&mut round, &plan, "worker-b")?;
        for chunk_index in 0..4 {
            round.record_chunk_outcome(&delivered(
                "worker-c",
                chunk_index,
                CollectiveTransferPath::Send,
            ))?;
        }
        let receipt = round.finalize(1_000 + 30_000 + 1, &[])?;
        assert!(receipt.round_timed_out);
        let CollectiveRoundVerdict::Partial {
            preserved_fraction_bps,
            banned_node_ids,
            ..
        } = &receipt.verdict
        else {
            return Err("expected partial verdict".into());
        };
        // 8 + 8 + 4 delivered chunks over 24 expected: partial not lost.
        assert_eq!(*preserved_fraction_bps, 8_333);
        assert_eq!(banned_node_ids, &vec![String::from("worker-c")]);
        let stall_ban = receipt
            .ban_events
            .first()
            .ok_or("stall ban event missing")?;
        assert_eq!(stall_ban.reason, BanForRoundReason::FullRoundTimeoutStall);
        assert_eq!(stall_ban.chunk_index, 4);
        Ok(())
    }

    #[test]
    fn stage_empty_without_standby_aborts() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-5", "stage-0", 5, plan, members(), 1_000)?;
        for node_id in members() {
            round.record_chunk_outcome(&ChunkTransferOutcome::new(
                node_id,
                0,
                CollectiveTransferPath::Send,
                ChunkTransferStatus::Disconnected,
                10,
                1_050,
            ))?;
        }
        let receipt = round.finalize(2_000, &[])?;
        let CollectiveRoundVerdict::Aborted { reason, .. } = &receipt.verdict else {
            return Err("expected aborted verdict".into());
        };
        assert_eq!(*reason, CollectiveRoundAbortReason::NoStandbyPromotable);
        let decision = receipt
            .standby_decision
            .as_ref()
            .ok_or("standby decision missing")?;
        assert!(decision.stage_would_empty);
        assert!(!decision.promotable);
        assert_eq!(
            decision.reason,
            StandbyPromotionReason::NoWarmStandbyRegistered
        );
        Ok(())
    }

    #[test]
    fn stage_empty_with_promotable_standby_stays_partial() -> Result<(), Box<dyn std::error::Error>>
    {
        let plan = eight_chunk_plan()?;
        let mut round =
            CollectiveFailureRound::new("round-6", "stage-0", 6, plan, members(), 1_000)?;
        for node_id in members() {
            round.record_chunk_outcome(&ChunkTransferOutcome::new(
                node_id,
                0,
                CollectiveTransferPath::Send,
                ChunkTransferStatus::Disconnected,
                10,
                1_050,
            ))?;
        }
        let standby = vec![String::from("standby-w2"), String::from("standby-w1")];
        let receipt = round.finalize(2_000, &standby)?;
        let CollectiveRoundVerdict::Partial {
            promoted_standby_node_id,
            ..
        } = &receipt.verdict
        else {
            return Err("expected partial verdict".into());
        };
        assert_eq!(
            promoted_standby_node_id.as_deref(),
            Some("standby-w1"),
            "deterministic first sorted standby promoted"
        );
        let decision = receipt
            .standby_decision
            .as_ref()
            .ok_or("standby decision missing")?;
        assert!(decision.promotable);
        assert_eq!(
            decision.reason,
            StandbyPromotionReason::WarmStandbyAvailable
        );
        Ok(())
    }

    #[test]
    fn round_timeout_with_empty_stage_and_no_standby_aborts_with_round_timeout_reason()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let round = CollectiveFailureRound::new("round-7", "stage-0", 7, plan, members(), 1_000)?;
        // Nobody delivers anything; the full-round timeout bans everyone.
        let receipt = round.finalize(1_000 + 30_000 + 1, &[])?;
        assert!(receipt.round_timed_out);
        let CollectiveRoundVerdict::Aborted { reason, .. } = &receipt.verdict else {
            return Err("expected aborted verdict".into());
        };
        assert_eq!(*reason, CollectiveRoundAbortReason::RoundTimeout);
        assert_eq!(receipt.ban_events.len(), 3);
        assert!(
            receipt
                .ban_events
                .iter()
                .all(|event| event.reason == BanForRoundReason::FullRoundTimeoutStall)
        );
        Ok(())
    }

    #[test]
    fn ban_escalation_emits_removal_recommendation() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let policy = BanEscalationPolicy::new(3, 8);
        let mut ledger = CollectiveBanLedger::new(policy)?;
        let mut recommendations = Vec::new();
        for round_index in 1..=3u64 {
            let mut round = CollectiveFailureRound::new(
                format!("round-{round_index}"),
                "stage-0",
                round_index,
                plan.clone(),
                members(),
                1_000,
            )?;
            deliver_all(&mut round, &plan, "worker-a")?;
            deliver_all(&mut round, &plan, "worker-c")?;
            round.record_chunk_outcome(&ChunkTransferOutcome::new(
                "worker-b",
                0,
                CollectiveTransferPath::Send,
                ChunkTransferStatus::Disconnected,
                10,
                1_050,
            ))?;
            let receipt = round.finalize(2_000, &[])?;
            recommendations.extend(ledger.record_round_receipt(&receipt, 2_100));
        }
        assert_eq!(recommendations.len(), 1);
        let recommendation = &recommendations[0];
        assert_eq!(recommendation.member_node_id, "worker-b");
        assert_eq!(recommendation.ban_count_in_window, 3);
        assert_eq!(recommendation.removal_ban_threshold, 3);
        assert_eq!(recommendation.ban_round_indices, vec![1, 2, 3]);
        assert_eq!(recommendation.ban_event_digests.len(), 3);
        Ok(())
    }

    #[test]
    fn ban_escalation_window_excludes_old_bans() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        let policy = BanEscalationPolicy::new(2, 2);
        let mut ledger = CollectiveBanLedger::new(policy)?;
        let mut recommendations = Vec::new();
        // Bans at rounds 1 and 5: never two bans inside a two-round window.
        for round_index in [1u64, 5u64] {
            let mut round = CollectiveFailureRound::new(
                format!("round-{round_index}"),
                "stage-0",
                round_index,
                plan.clone(),
                members(),
                1_000,
            )?;
            deliver_all(&mut round, &plan, "worker-a")?;
            deliver_all(&mut round, &plan, "worker-c")?;
            round.record_chunk_outcome(&ChunkTransferOutcome::new(
                "worker-b",
                0,
                CollectiveTransferPath::Send,
                ChunkTransferStatus::Disconnected,
                10,
                1_050,
            ))?;
            let receipt = round.finalize(2_000, &[])?;
            recommendations.extend(ledger.record_round_receipt(&receipt, 2_100));
        }
        assert!(recommendations.is_empty());
        assert_eq!(
            CollectiveBanLedger::new(BanEscalationPolicy::new(0, 8)),
            Err(CollectiveFailureSemanticsError::ZeroEscalationField {
                field: "removal_ban_threshold"
            })
        );
        Ok(())
    }

    #[test]
    fn round_guards_reject_bad_inputs() -> Result<(), Box<dyn std::error::Error>> {
        let plan = eight_chunk_plan()?;
        assert!(matches!(
            CollectiveFailureRound::new("round-x", "stage-0", 1, plan.clone(), Vec::new(), 0),
            Err(CollectiveFailureSemanticsError::EmptyRound { .. })
        ));
        assert!(matches!(
            CollectiveFailureRound::new(
                "round-x",
                "stage-0",
                1,
                plan.clone(),
                vec![String::from("worker-a"), String::from("worker-a")],
                0,
            ),
            Err(CollectiveFailureSemanticsError::DuplicateMember { .. })
        ));
        let mut round =
            CollectiveFailureRound::new("round-x", "stage-0", 1, plan, members(), 1_000)?;
        assert!(matches!(
            round.record_chunk_outcome(&delivered("worker-z", 0, CollectiveTransferPath::Send)),
            Err(CollectiveFailureSemanticsError::UnknownMember { .. })
        ));
        assert!(matches!(
            round.record_chunk_outcome(&delivered("worker-a", 99, CollectiveTransferPath::Send)),
            Err(CollectiveFailureSemanticsError::ChunkIndexOutOfRange { .. })
        ));
        Ok(())
    }

    #[test]
    fn canonical_contract_is_deterministic_and_receipted() -> Result<(), Box<dyn std::error::Error>>
    {
        let first = canonical_collective_failure_semantics_contract()?;
        let second = canonical_collective_failure_semantics_contract()?;
        assert_eq!(first, second);
        assert_eq!(first.schema_version, FAILURE_SEMANTICS_SCHEMA_VERSION);
        assert_eq!(first.round_receipts.len(), 7);
        assert_eq!(
            first.round_receipts[0].verdict,
            CollectiveRoundVerdict::Complete
        );
        assert!(matches!(
            first.round_receipts[4].verdict,
            CollectiveRoundVerdict::Aborted {
                reason: CollectiveRoundAbortReason::NoStandbyPromotable,
                ..
            }
        ));
        assert!(matches!(
            &first.round_receipts[5].verdict,
            CollectiveRoundVerdict::Partial {
                promoted_standby_node_id: Some(node_id),
                ..
            } if node_id == "standby-w1"
        ));
        assert!(first.round_receipts[6].round_timed_out);
        assert_eq!(first.removal_recommendations.len(), 2);
        assert_eq!(first.removal_recommendations[0].member_node_id, "worker-b");
        assert_eq!(first.removal_recommendations[1].member_node_id, "worker-c");
        // Round-trip through serde stays receipt-compatible.
        let json = serde_json::to_string(&first)?;
        let decoded: CollectiveFailureSemanticsContract = serde_json::from_str(&json)?;
        assert_eq!(decoded, first);
        Ok(())
    }
}
