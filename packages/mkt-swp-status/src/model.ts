/**
 * Core types for the SWAP-6 status/progress view model (openagents#9321).
 *
 * The structural difference from a Boltz-shaped status view (teardown §6,
 * §2.4): status here is per-signer signed evidence, not one operator's feed.
 * Every type in this file keeps authorship attached — a status is always
 * "one signer's claim", never "the state of the swap".
 */
import type { StatusState } from "@openagentsinc/nip-mkt";

export type SwapFlow = "submarine" | "reverse" | "chain";

export type ParticipantRole = "requester" | "provider";

/**
 * One signed Status record, already unwrapped and signature-checked by the
 * transport layer (the wasm-proven Immortal client engine owns signature and
 * grammar truth; this package owns projection and rendering truth).
 */
export interface StatusClaim {
  /** Event id — the idempotency key for the fold. */
  readonly id: string;
  readonly sessionId: string;
  readonly orderId: string;
  /** Author pubkey (lower hex). */
  readonly author: string;
  /** Which participant this author is in the session's signer map. */
  readonly role: ParticipantRole;
  /** Per-author sequence, starting at 0, increasing by one. */
  readonly seq: number;
  /** Event id of this author's previous Status; absent only at seq 0. */
  readonly previous?: string;
  /** The base `state` tag the record carried, if any. */
  readonly baseState?: string;
  /** The profile `swp_state` value the record carried. */
  readonly swpState: string;
  readonly createdAt: number;
}

/** MKT-SWP §11 evidence rungs, narrowest first. */
export const EVIDENCE_RUNGS = [
  "pledged",
  "reserved",
  "measured",
  "verified",
  "paid",
  "settled",
] as const;

export type EvidenceRung = (typeof EVIDENCE_RUNGS)[number];

export const rungIndex = (rung: EvidenceRung): number => EVIDENCE_RUNGS.indexOf(rung);

/**
 * Who produced a fact. §11: a relay-signed observation, provider Status,
 * explorer response, API response, NIP-57 receipt, or Close can NEVER
 * independently produce `paid` or `settled`. Chain and Lightning facts are
 * always attributed to their verifying source in the view.
 */
export type EvidenceAuthority =
  | "requester_status"
  | "provider_status"
  | "relay_observation"
  | "explorer_response"
  | "provider_api"
  | "nip57_receipt"
  | "allowlisted_verifier"
  | "local_wallet"
  | "bitcoin_adapter"
  | "lightning_adapter";

/**
 * The maximum rung each authority can establish on its own (§11). A status
 * is a signed claim: `pledged`, never more.
 */
export const AUTHORITY_RUNG_CAP: Readonly<Record<EvidenceAuthority, EvidenceRung>> = {
  requester_status: "pledged",
  provider_status: "pledged",
  relay_observation: "measured",
  explorer_response: "measured",
  provider_api: "pledged",
  nip57_receipt: "pledged",
  allowlisted_verifier: "verified",
  local_wallet: "paid",
  bitcoin_adapter: "settled",
  lightning_adapter: "settled",
};

export type EvidenceClass =
  | "invoice"
  | "lightning_htlc"
  | "lightning_payment"
  | "bitcoin_transaction"
  | "bitcoin_output"
  | "bitcoin_spend"
  | "reservation"
  | "covenant_reserve"
  | "claim"
  | "refund"
  | "reorg"
  | "replacement";

export interface SwapEvidence {
  readonly class: EvidenceClass;
  /** The rung this evidence claims to establish. */
  readonly rung: EvidenceRung;
  readonly authority: EvidenceAuthority;
  /** Canonical rail reference (txid:vout, payment hash, ...). */
  readonly reference: string;
  /** Whether the quoted finality rules are satisfied in the source's view. */
  readonly final?: boolean;
}

export type CloseOutcome =
  | "completed"
  | "rejected"
  | "cancelled"
  | "expired"
  | "failed"
  | "refunded"
  | "disputed"
  | "unresolved";

/** §15 loss accounting. Every amount is a canonical decimal string. */
export interface LossAccounting {
  readonly input_asset_id: string;
  readonly output_asset_id: string;
  readonly input_committed: string;
  readonly input_recovered: string;
  readonly output_received: string;
  readonly provider_fee_paid: string;
  readonly miner_fee_paid: string;
  readonly lightning_routing_fee_paid: string;
  readonly guarantee_recovery_received: string;
  readonly principal_unresolved: string;
  readonly reservation_released: string;
}

export interface CloseRecord {
  readonly id: string;
  readonly author: string;
  readonly role: ParticipantRole;
  readonly outcome: CloseOutcome;
  readonly lossAccounting?: LossAccounting;
  /** §15: unknown values are listed here and are never rendered as zero. */
  readonly unknownFields?: readonly (keyof LossAccounting)[];
}

/** The exit the user actually has, stated for every terminal/resting state. */
export type UserExitKind =
  | "none_needed"
  | "claim"
  | "refund"
  | "rescue"
  | "dispute"
  | "keep_watching";

export type { StatusState };
