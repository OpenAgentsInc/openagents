/**
 * Local message table for the quote comparison surface.
 *
 * Protocol error identifiers render through the SWAP-8 shared table
 * (`@openagentsinc/swap-i18n`, `swap.error.*` — one message per §17
 * identifier, typed and exhaustive). This file carries only the
 * comparison-local labels: commitment badges, reservation proof classes,
 * custody strip, verify-checklist rows. Keys are stable `swap.compare.*`
 * identifiers shaped for that shared table; they migrate there when the
 * surface is wired into the SWAP-0 shell.
 *
 * Laws encoded by the messages test:
 * - every quote class × reservation class commitment renders a distinct
 *   label (firm/hard can never be styled identically to indicative/none);
 * - every reservation proof class renders a distinct label (a signed claim
 *   and a covenant-enforced reserve are not the same evidence);
 * - every verify-checklist row renders a distinct label (each failing row
 *   is individually identifiable — never a generic error page).
 */
import type { ReservationProofClass } from "./model.js";
import type { VerifyCheckId } from "./verify.js";

export interface CompareMessage {
  /** Stable key, shaped for the SWAP-8 shared table. */
  readonly key: `swap.compare.${string}`;
  /** Default English message. Localisation lives in swap-i18n. */
  readonly message: string;
}

/**
 * Commitment badge per (quoteClass, reservationClass). `firm` copy states a
 * declaration, never a proof: a firm Quote declares that a conforming
 * timely Order is accepted under stated preconditions — it does not prove
 * the declaration or the capacity.
 */
export const COMMITMENT_MESSAGES = {
  "indicative:none": {
    key: "swap.compare.commitment.indicative_none",
    message:
      "Indicative — price only. Nothing is reserved; the provider must still accept your order.",
  },
  "indicative:soft": {
    key: "swap.compare.commitment.indicative_soft",
    message:
      "Indicative with a soft reserve claim. The provider must still accept your order.",
  },
  "indicative:hard": {
    key: "swap.compare.commitment.indicative_hard",
    message:
      "Indicative with a hard reserve behind it. The provider must still accept your order.",
  },
  "firm:none": {
    key: "swap.compare.commitment.firm_none",
    message:
      "Nonconforming: a firm quote must reserve capacity (soft or hard).",
  },
  "firm:soft": {
    key: "swap.compare.commitment.firm_soft",
    message:
      "Firm, soft reserve — the provider declares it will accept a conforming timely order. A declaration, not proof of capacity.",
  },
  "firm:hard": {
    key: "swap.compare.commitment.firm_hard",
    message:
      "Firm, hard reserve — the provider declares acceptance and backs it with a capacity proof. The declaration itself is still not proven.",
  },
} as const satisfies Readonly<Record<`${"indicative" | "firm"}:${"none" | "soft" | "hard"}`, CompareMessage>>;

export type CommitmentKey = keyof typeof COMMITMENT_MESSAGES;

/**
 * Proof-class labels (§5 table). Each names the evidence, because a signed
 * claim and a covenant-enforced reserve must not render the same.
 */
export const PROOF_CLASS_MESSAGES: Readonly<
  Record<ReservationProofClass, CompareMessage>
> = {
  provider_signed: {
    key: "swap.compare.proof.provider_signed",
    message: "Provider-signed claim — no independent capacity proof",
  },
  handler_accounted: {
    key: "swap.compare.proof.handler_accounted",
    message: "Handler-accounted — a bounded accounting view, not solvency",
  },
  utxo_control: {
    key: "swap.compare.proof.utxo_control",
    message: "UTXO control — verifier-checked unspent output",
  },
  lightning_liquidity: {
    key: "swap.compare.proof.lightning_liquidity",
    message: "Lightning liquidity — verifier-checked channel liquidity",
  },
  funded_htlc: {
    key: "swap.compare.proof.funded_htlc",
    message: "Funded HTLC — the swap output already locks the amount",
  },
  covenant_reserve: {
    key: "swap.compare.proof.covenant_reserve",
    message: "Covenant reserve — a covenant enforces the quoted output",
  },
  third_party_guarantee: {
    key: "swap.compare.proof.third_party_guarantee",
    message: "Third-party guarantee — a named guarantor, not the provider",
  },
};

/** Custody strip class labels. */
export const CUSTODY_STRIP_MESSAGES = {
  custodial: {
    key: "swap.compare.custody.custodial",
    message: "Custodial — a third party controls funds on this route",
  },
  noncustodial: {
    key: "swap.compare.custody.noncustodial",
    message: "Noncustodial — only you, the contract, and the chain",
  },
} as const satisfies Readonly<Record<"custodial" | "noncustodial", CompareMessage>>;

/** Both custody-duration bounds, displayed together, never merged. */
export const CUSTODY_DURATION_MESSAGES = {
  wallClockEstimate: {
    key: "swap.compare.custody.duration_estimate",
    message: "Worst-case custody duration (estimate)",
  },
  heightBound: {
    key: "swap.compare.custody.height_bound",
    message: "Exact height-based bound (consensus)",
  },
} as const;

/** Verify-checklist row labels — each row individually identifiable. */
export const VERIFY_CHECK_MESSAGES: Readonly<
  Record<VerifyCheckId, CompareMessage>
> = {
  signatures_and_references: {
    key: "swap.compare.verify.signatures_and_references",
    message: "Signatures, causal references, and terms verified",
  },
  external_effect_ids: {
    key: "swap.compare.verify.external_effect_ids",
    message: "External-effect IDs recomputed",
  },
  script_tree_parsed: {
    key: "swap.compare.verify.script_tree_parsed",
    message: "Scripts and Taproot tree parsed from bytes",
  },
  output_key_rederived: {
    key: "swap.compare.verify.output_key_rederived",
    message: "Output key and address re-derived locally",
  },
  terms_against_quote: {
    key: "swap.compare.verify.terms_against_quote",
    message: "Payment hash, keys, timelocks, and amount match the quote",
  },
  timeout_ladder: {
    key: "swap.compare.verify.timeout_ladder",
    message: "Timeout-ladder inequalities hold against the chain",
  },
  exit_package: {
    key: "swap.compare.verify.exit_package",
    message: "Exit package built, persisted, and digest-checked",
  },
  chain_policy: {
    key: "swap.compare.verify.chain_policy",
    message: "Confirmation, RBF, replacement, and reorg policy as quoted",
  },
  unsupported_constructs_refused: {
    key: "swap.compare.verify.unsupported_constructs_refused",
    message: "No unknown script versions or hidden composition",
  },
  lightning_invoice: {
    key: "swap.compare.verify.lightning_invoice",
    message: "Invoice parsed and verified locally",
  },
  musig_transcript: {
    key: "swap.compare.verify.musig_transcript",
    message: "MuSig2 keys, nonces, and partial signatures verified",
  },
};

/** Fork notice: retained and attributable, never resolved by arrival. */
export const RESERVATION_FORK_MESSAGE = {
  key: "swap.compare.reservation_fork",
  message:
    "This provider equivocated about its reserved capacity. Both records are kept as evidence; neither is trusted.",
} as const satisfies CompareMessage;

/** Expired-quote row notice (enforced locally, not merely styled). */
export const QUOTE_EXPIRED_MESSAGE = {
  key: "swap.compare.quote_expired",
  message: "Expired — this quote can no longer be ordered.",
} as const satisfies CompareMessage;

/** Nonconforming-quote row notice. */
export const QUOTE_NONCONFORMING_MESSAGE = {
  key: "swap.compare.quote_nonconforming",
  message: "Nonconforming — this quote violates the profile and cannot be ordered.",
} as const satisfies CompareMessage;
