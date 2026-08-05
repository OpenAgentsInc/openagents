/**
 * The MKT-SWP §9 state tables as data: the swp_state -> base `state`
 * derivation, the per-flow signer map, the per-flow transition edges, and
 * the evidence requirement each rail-fact state carries.
 *
 * Laws encoded here (issue #9321):
 * - `contract_pending` and `contract_bound` are local projections with NO
 *   Status mapping; a Status claiming them is `swp_status_transition_invalid`.
 * - A value matching no row is `swp_status_transition_invalid`.
 * - Only the signer the state machine admits may claim an action
 *   (`swp_status_signer_invalid`); either party may report an observation
 *   without promoting a rung.
 */
import type { StatusState } from "@openagentsinc/nip-mkt";
import type { EvidenceClass, EvidenceRung, ParticipantRole, SwapFlow } from "./model.js";

export type SwpStateClassification =
  | { readonly ok: true; readonly base: StatusState }
  | { readonly ok: false; readonly error: "swp_status_transition_invalid" };

/**
 * §9 mapping table, implemented as ordered rules. Literal rows are checked
 * before suffix-class rows so `provider_source_claim_pending` maps to
 * `settlement_pending` and not to the `*_claim_pending` executing row.
 *
 * Two documented normalisations where the §9.2-9.4 flow vocabulary is wider
 * than the class table's literal patterns:
 * - `requester_source_broadcast` and `provider_destination_broadcast`
 *   (§9.4 chain funding broadcasts) class with `*_funding_broadcast`.
 * - `lightning_paid` (§9.2/§9.3, mid-flow after payment) classes as
 *   `executing`: both legs are not yet final when it is claimed.
 * `rejected` (§10: an indicative Quote may terminate through a provider
 * `rejected` Status) maps to the base `rejected` state.
 */
const LITERAL_BASE: Readonly<Record<string, StatusState>> = {
  accepted: "accepted",
  rejected: "rejected",
  hold_invoice_ready: "awaiting_input",
  funding_required: "funding_required",
  source_funding_required: "funding_required",
  requester_source_broadcast: "funding_observed",
  provider_destination_broadcast: "funding_observed",
  lightning_settlement_pending: "settlement_pending",
  provider_source_claim_pending: "settlement_pending",
  lightning_paid: "executing",
  completed: "completed",
  invoice_cancel_pending: "refund_pending",
  refunded: "refunded",
  invoice_cancelled: "refunded",
  disputed: "disputed",
  failed: "failed",
  unresolved: "failed",
};

const SUFFIX_BASE: readonly (readonly [suffix: string, base: StatusState])[] = [
  ["_terms_ready", "awaiting_input"],
  ["verification_passed", "awaiting_input"],
  ["_verified", "awaiting_input"],
  ["_funding_broadcast", "funding_observed"],
  ["funding_observed", "funding_observed"],
  ["funding_final", "executing"],
  ["_payment_pending", "executing"],
  ["_htlcs_held", "executing"],
  ["_claim_pending", "executing"],
  ["_claimed", "executing"],
  ["refund_prepared", "refund_pending"],
  ["refund_pending", "refund_pending"],
  ["_refunded", "refunded"],
];

/** Local projections that must never be established by a Status claim. */
export const LOCAL_ONLY_PROJECTIONS = ["contract_pending", "contract_bound"] as const;

export function classifySwpState(swpState: string): SwpStateClassification {
  if ((LOCAL_ONLY_PROJECTIONS as readonly string[]).includes(swpState)) {
    return { ok: false, error: "swp_status_transition_invalid" };
  }
  const literal = LITERAL_BASE[swpState];
  if (literal !== undefined) return { ok: true, base: literal };
  for (const [suffix, base] of SUFFIX_BASE) {
    if (swpState.endsWith(suffix)) return { ok: true, base };
  }
  return { ok: false, error: "swp_status_transition_invalid" };
}

/**
 * Signer admission (§9.2-9.4). `either_observation` states may be reported
 * by either party without promoting a rung.
 */
export type AdmittedSigner = ParticipantRole | "either_observation";

const SUBMARINE_SIGNERS: Readonly<Record<string, AdmittedSigner>> = {
  accepted: "provider",
  rejected: "provider",
  lock_terms_ready: "provider",
  lightning_payment_pending: "provider",
  lightning_paid: "provider",
  provider_claim_pending: "provider",
  provider_claimed: "provider",
  requester_verification_passed: "requester",
  requester_funding_broadcast: "requester",
  refund_prepared: "requester",
  refund_pending: "requester",
};

const REVERSE_SIGNERS: Readonly<Record<string, AdmittedSigner>> = {
  accepted: "provider",
  rejected: "provider",
  hold_invoice_ready: "provider",
  lightning_htlcs_held: "provider",
  provider_lock_terms_ready: "provider",
  provider_funding_broadcast: "provider",
  lightning_settlement_pending: "provider",
  lightning_paid: "provider",
  provider_refund_prepared: "provider",
  provider_refund_pending: "provider",
  provider_refunded: "provider",
  requester_invoice_verified: "requester",
  lightning_payment_pending: "requester",
  requester_lock_verified: "requester",
  requester_claim_pending: "requester",
  requester_claimed: "requester",
};

const CHAIN_SIGNERS: Readonly<Record<string, AdmittedSigner>> = {
  accepted: "provider",
  rejected: "provider",
  source_lock_terms_ready: "provider",
  destination_lock_terms_ready: "provider",
  provider_destination_broadcast: "provider",
  provider_destination_refund_prepared: "provider",
  provider_destination_refund_pending: "provider",
  provider_destination_refunded: "provider",
  provider_source_claim_pending: "provider",
  provider_source_claimed: "provider",
  requester_source_verified: "requester",
  requester_source_broadcast: "requester",
  requester_destination_verified: "requester",
  requester_destination_claim_pending: "requester",
  requester_destination_claimed: "requester",
  requester_source_refund_prepared: "requester",
  requester_source_refund_pending: "requester",
  requester_source_refunded: "requester",
};

const SIGNER_MAPS: Readonly<Record<SwapFlow, Readonly<Record<string, AdmittedSigner>>>> = {
  submarine: SUBMARINE_SIGNERS,
  reverse: REVERSE_SIGNERS,
  chain: CHAIN_SIGNERS,
};

export function admittedSignerFor(flow: SwapFlow, swpState: string): AdmittedSigner {
  return SIGNER_MAPS[flow][swpState] ?? "either_observation";
}

/** The happy-path order per flow (§9.2-9.4), used for edges and tiebreaks. */
export const HAPPY_PATH: Readonly<Record<SwapFlow, readonly string[]>> = {
  submarine: [
    "accepted",
    "lock_terms_ready",
    "requester_verification_passed",
    "funding_required",
    "requester_funding_broadcast",
    "funding_observed",
    "funding_final",
    "lightning_payment_pending",
    "lightning_paid",
    "provider_claim_pending",
    "provider_claimed",
    "completed",
  ],
  reverse: [
    "accepted",
    "hold_invoice_ready",
    "requester_invoice_verified",
    "lightning_payment_pending",
    "lightning_htlcs_held",
    "provider_lock_terms_ready",
    "requester_lock_verified",
    "provider_funding_broadcast",
    "funding_observed",
    "funding_final",
    "requester_claim_pending",
    "requester_claimed",
    "lightning_settlement_pending",
    "lightning_paid",
    "completed",
  ],
  chain: [
    "accepted",
    "source_lock_terms_ready",
    "requester_source_verified",
    "source_funding_required",
    "requester_source_broadcast",
    "source_funding_observed",
    "source_funding_final",
    "destination_lock_terms_ready",
    "requester_destination_verified",
    "provider_destination_broadcast",
    "destination_funding_observed",
    "destination_funding_final",
    "requester_destination_claim_pending",
    "requester_destination_claimed",
    "provider_source_claim_pending",
    "provider_source_claimed",
    "completed",
  ],
};

/** Recovery edges per flow (§9.2-9.4 recovery branches). */
const RECOVERY_EDGES: Readonly<Record<SwapFlow, readonly (readonly [string, string])[]>> = {
  submarine: [
    ["requester_funding_broadcast", "refund_prepared"],
    ["funding_observed", "refund_prepared"],
    ["funding_final", "refund_prepared"],
    ["refund_prepared", "refund_pending"],
    ["refund_pending", "refunded"],
  ],
  reverse: [
    ["accepted", "invoice_cancel_pending"],
    ["hold_invoice_ready", "invoice_cancel_pending"],
    ["requester_invoice_verified", "invoice_cancel_pending"],
    ["lightning_payment_pending", "invoice_cancel_pending"],
    ["lightning_htlcs_held", "invoice_cancel_pending"],
    ["provider_lock_terms_ready", "invoice_cancel_pending"],
    ["requester_lock_verified", "invoice_cancel_pending"],
    ["invoice_cancel_pending", "invoice_cancelled"],
    ["provider_funding_broadcast", "provider_refund_prepared"],
    ["funding_observed", "provider_refund_prepared"],
    ["funding_final", "provider_refund_prepared"],
    ["requester_claim_pending", "provider_refund_prepared"],
    ["provider_refund_prepared", "provider_refund_pending"],
    ["provider_refund_pending", "provider_refunded"],
    ["provider_refunded", "invoice_cancelled"],
    ["invoice_cancelled", "refunded"],
  ],
  chain: [
    ["requester_source_broadcast", "requester_source_refund_prepared"],
    ["source_funding_observed", "requester_source_refund_prepared"],
    ["source_funding_final", "requester_source_refund_prepared"],
    ["destination_lock_terms_ready", "requester_source_refund_prepared"],
    ["requester_destination_verified", "requester_source_refund_prepared"],
    ["requester_source_refund_prepared", "requester_source_refund_pending"],
    ["requester_source_refund_pending", "requester_source_refunded"],
    ["requester_source_refunded", "refunded"],
    ["provider_destination_broadcast", "provider_destination_refund_prepared"],
    ["destination_funding_observed", "provider_destination_refund_prepared"],
    ["destination_funding_final", "provider_destination_refund_prepared"],
    ["requester_destination_claim_pending", "provider_destination_refund_prepared"],
    ["provider_destination_refund_prepared", "provider_destination_refund_pending"],
    ["provider_destination_refund_pending", "provider_destination_refunded"],
    ["provider_destination_refunded", "requester_source_refund_pending"],
  ],
};

/** States from which `disputed`/`failed`/`unresolved` are reachable: any funded state. */
const FIRST_FUNDED_STATE: Readonly<Record<SwapFlow, string>> = {
  submarine: "requester_funding_broadcast",
  reverse: "lightning_htlcs_held",
  chain: "requester_source_broadcast",
};

const TERMINAL_FROM_FUNDED = ["disputed", "failed", "unresolved"] as const;

export function allowedSuccessors(flow: SwapFlow, from: string): readonly string[] {
  const path = HAPPY_PATH[flow];
  const successors: string[] = [];
  const index = path.indexOf(from);
  if (index >= 0 && index + 1 < path.length) successors.push(path[index + 1]!);
  for (const [edgeFrom, edgeTo] of RECOVERY_EDGES[flow]) {
    if (edgeFrom === from) successors.push(edgeTo);
  }
  const fundedIndex = path.indexOf(FIRST_FUNDED_STATE[flow]);
  if (index >= fundedIndex && index >= 0) successors.push(...TERMINAL_FROM_FUNDED);
  // Recovery states are all post-funding; they can also degrade.
  if (index < 0 && RECOVERY_EDGES[flow].some(([f]) => f === from)) {
    successors.push(...TERMINAL_FROM_FUNDED);
  }
  return successors;
}

/** Every swp_state a flow can legally carry in a Status. */
export function knownStates(flow: SwapFlow): readonly string[] {
  const states = new Set<string>(HAPPY_PATH[flow]);
  for (const [from, to] of RECOVERY_EDGES[flow]) {
    states.add(from);
    states.add(to);
  }
  states.add("rejected");
  for (const terminal of TERMINAL_FROM_FUNDED) states.add(terminal);
  states.add("refunded");
  return [...states];
}

/**
 * Rail-fact states: claiming them advances the display only when attributed
 * evidence at the minimum rung exists (§9: the projection "advances a
 * verified execution rung only when the required signer and evidence
 * exist"). Absent classes mean the state is an actor's own action claim.
 */
export interface EvidenceRequirement {
  readonly classes: readonly EvidenceClass[];
  readonly minRung: EvidenceRung;
  readonly requiresFinal?: boolean;
}

const EVIDENCE_REQUIREMENTS: Readonly<Record<string, EvidenceRequirement>> = {
  funding_observed: { classes: ["bitcoin_output", "bitcoin_transaction"], minRung: "measured" },
  source_funding_observed: {
    classes: ["bitcoin_output", "bitcoin_transaction"],
    minRung: "measured",
  },
  destination_funding_observed: {
    classes: ["bitcoin_output", "bitcoin_transaction"],
    minRung: "measured",
  },
  funding_final: {
    classes: ["bitcoin_output", "bitcoin_transaction"],
    minRung: "settled",
    requiresFinal: true,
  },
  source_funding_final: {
    classes: ["bitcoin_output", "bitcoin_transaction"],
    minRung: "settled",
    requiresFinal: true,
  },
  destination_funding_final: {
    classes: ["bitcoin_output", "bitcoin_transaction"],
    minRung: "settled",
    requiresFinal: true,
  },
  lightning_htlcs_held: { classes: ["lightning_htlc"], minRung: "measured" },
  lightning_paid: { classes: ["lightning_payment"], minRung: "paid" },
  completed: {
    classes: ["bitcoin_spend", "lightning_payment", "claim"],
    minRung: "settled",
    requiresFinal: true,
  },
  refunded: { classes: ["refund", "bitcoin_spend"], minRung: "settled", requiresFinal: true },
  provider_refunded: {
    classes: ["refund", "bitcoin_spend"],
    minRung: "settled",
    requiresFinal: true,
  },
  requester_source_refunded: {
    classes: ["refund", "bitcoin_spend"],
    minRung: "settled",
    requiresFinal: true,
  },
  provider_destination_refunded: {
    classes: ["refund", "bitcoin_spend"],
    minRung: "settled",
    requiresFinal: true,
  },
};

export function evidenceRequirementFor(swpState: string): EvidenceRequirement | undefined {
  return EVIDENCE_REQUIREMENTS[swpState];
}
