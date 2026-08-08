/**
 * The persisted swap-session record (openagents#9320, SWAP-5).
 *
 * What is stored — and, just as deliberately, what is not:
 *
 * Stored (all public, per the MKT-SWP client custody boundary — signed
 * public records, exit templates or complete pre-signed transactions, public
 * commitments, and external effect results):
 * - the user's own signed records and the counterparty records the engine
 *   accepted (RFQ, Quote, Order, Swap Contracts, Status, Cancel, Close);
 * - exit-package records (typed public packages, digest-referenced) — the
 *   §12 artifact that must exist before funding and survive the coordinator;
 * - the external-effect ledger (deterministic effect IDs bound to request
 *   and result digests) that makes resume idempotent;
 * - an opaque engine snapshot document (the wasm engine's public snapshot,
 *   schema owned by the engine) for restore-and-revalidate;
 * - non-secret handles into the SWAP-4 secret store.
 *
 * NOT stored, ever (recursive tripwire in `secret-boundary.ts`): seeds,
 * mnemonics, private or claim/refund keys, preimages, macaroons, NWC
 * connection strings, signing nonces. Key material design belongs to SWAP-4
 * (openagents#9319); this store only carries its opaque handles.
 *
 * This shape stays compatible in spirit with the Rust lab harness record
 * (immortal `crates/immortal-lab/src/state.rs` `SessionRecord`): session id,
 * created-at, relay URL, swap type, provider, offering address, last step,
 * and the signed events themselves — extended with the effect ledger and
 * exit packages the browser needs for keyless recovery.
 */
import { Schema } from "effect";

export const SWAP_TYPES = ["submarine", "reverse", "chain"] as const;
export type SwapType = (typeof SWAP_TYPES)[number];

/** MKT-SWP §15 Close outcomes, plus null while the session is in flight. */
export const TERMINAL_OUTCOMES = [
  "completed",
  "cancelled",
  "expired",
  "failed",
  "refunded",
  "disputed",
  "unresolved",
] as const;
export type TerminalOutcome = (typeof TERMINAL_OUTCOMES)[number];

/** MKT-SWP §11 evidence rungs — monotonic, never inferred upward. */
export const EVIDENCE_RUNGS = [
  "pledged",
  "reserved",
  "measured",
  "verified",
  "paid",
  "settled",
] as const;
export type EvidenceRung = (typeof EVIDENCE_RUNGS)[number];

/** A signed Nostr event exactly as accepted by the engine. */
export const SignedNostrRecord = Schema.Struct({
  id: Schema.String,
  pubkey: Schema.String,
  created_at: Schema.Number,
  kind: Schema.Number,
  tags: Schema.Array(Schema.Array(Schema.String)),
  content: Schema.String,
  sig: Schema.String,
});
export type SignedNostrRecord = typeof SignedNostrRecord.Type;

/**
 * Why a recorded external effect definitively did NOT take effect. A failure
 * may only be recorded when the wallet/provider reported that no side effect
 * happened (the user cancelled the prompt, the call was rejected before
 * anything ran, the provider returned a definitive error). When the outcome
 * is UNKNOWN — a timeout, a crash mid-call — the entry must stay pending so
 * the crash-window replay and the reload guard stay honest.
 */
export const EFFECT_FAILURE_REASONS = ["cancelled", "rejected", "failed"] as const;
export type EffectFailureReason = (typeof EFFECT_FAILURE_REASONS)[number];

/**
 * One external effect (wallet call, payment initiation, broadcast) with its
 * deterministic effect ID. The request is stored as typed public metadata so
 * the same operation can be reconstructed after a crash; a persisted result
 * suppresses the callback on resume. Binding one effect ID to a different
 * request or result digest fails closed (`EffectBindingConflictError`).
 *
 * The ledger has three states, and they mean different things:
 * - `result === null && failure === null` — PENDING: the operation may be
 *   running right now. The reload guard blocks and History pins the exit.
 * - `result !== null` — SUCCEEDED: the operation ran; resume suppresses the
 *   callback and reuses the persisted result.
 * - `failure !== null` (and `result === null`) — FAILED DEFINITIVELY: the
 *   operation did not take effect. The guard releases, History unpins, and
 *   a retry is legitimate — `priorEffectResult` returns null so the caller
 *   re-drives the exact persisted request.
 */
export const ExternalEffectRecord = Schema.Struct({
  effectId: Schema.String,
  /** SHA-256 (canonical JSON) of `request`. */
  requestDigestHex: Schema.String,
  /** The typed public request document (engine-owned schema, opaque here). */
  request: Schema.Unknown,
  result: Schema.NullOr(
    Schema.Struct({
      /** SHA-256 (canonical JSON) of `result`. */
      resultDigestHex: Schema.String,
      /** External identifier (txid, payment id) when one exists. */
      externalId: Schema.NullOr(Schema.String),
      observedAt: Schema.Number,
      result: Schema.Unknown,
    }),
  ),
  /**
   * The latest definitive failure of this effect, or null. Cleared when a
   * retry succeeds (`result` set). Never set while `result` is non-null.
   */
  failure: Schema.NullOr(
    Schema.Struct({
      reason: Schema.Literals([...EFFECT_FAILURE_REASONS]),
      /** Host-supplied public detail (an error code, never key material). */
      detail: Schema.String,
      observedAt: Schema.Number,
    }),
  ),
});
export type ExternalEffectRecord = typeof ExternalEffectRecord.Type;

/**
 * True while the effect's outcome is unknown: requested, no result, no
 * definitive failure. This is the predicate the reload guard and the History
 * exit pin share — a definitively failed effect is NOT pending.
 */
export const isEffectPending = (entry: ExternalEffectRecord): boolean =>
  entry.result === null && entry.failure === null;

/**
 * A §12 exit package reference: the typed public package document plus its
 * engine-computed digest. Contains templates or complete pre-signed
 * transactions — never a key or preimage.
 */
export const ExitPackageRecord = Schema.Struct({
  packageDigestHex: Schema.String,
  package: Schema.Unknown,
});
export type ExitPackageRecord = typeof ExitPackageRecord.Type;

/**
 * The locally computed session projection. This is a cache of the engine's
 * per-signer status fold for list rendering; History actionability is
 * computed from chain facts and tip height, never from this alone.
 */
export const SessionProjection = Schema.Struct({
  /** Last valid profile `swp_state` (or local projection label). */
  state: Schema.String,
  terminal: Schema.Boolean,
  outcome: Schema.NullOr(Schema.Literals([...TERMINAL_OUTCOMES])),
  /** Highest locally verified evidence rung; a claim is not a rung. */
  rung: Schema.NullOr(Schema.Literals([...EVIDENCE_RUNGS])),
  /**
   * True while an output attributable to this user remains claimable —
   * terminal-but-unclaimed sessions must keep their subscriptions on resume.
   */
  unclaimedFunds: Schema.Boolean,
});
export type SessionProjection = typeof SessionProjection.Type;

export const StoredSwapSession = Schema.Struct({
  sessionId: Schema.String,
  createdAt: Schema.Number,
  updatedAt: Schema.Number,
  relayUrl: Schema.String,
  swapType: Schema.Literals([...SWAP_TYPES]),
  requesterPubkey: Schema.String,
  providerPubkey: Schema.String,
  offeringAddress: Schema.String,
  projection: SessionProjection,
  /** Append-only; exact replay is idempotent, changed bytes fail closed. */
  signedRecords: Schema.Array(SignedNostrRecord),
  exitPackages: Schema.Array(ExitPackageRecord),
  effectLedger: Schema.Array(ExternalEffectRecord),
  /** Opaque public engine snapshot for restore-and-revalidate; may be null. */
  engineSnapshot: Schema.Unknown,
  /**
   * SWAP-4 boundary (openagents#9319): opaque, non-secret handles into the
   * browser secret store. The handle proves nothing and spends nothing; the
   * secret store resolves it. This store never sees the material behind it.
   */
  secretHandles: Schema.Array(Schema.String),
});
export type StoredSwapSession = typeof StoredSwapSession.Type;

export const decodeStoredSwapSession = Schema.decodeUnknownSync(StoredSwapSession);
