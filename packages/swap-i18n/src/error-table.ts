// The MKT-SWP §17 error-identifier message table — the one place protocol
// error identifiers become user-facing text. Every identifier in
// `SWP_ERROR_IDENTIFIERS` MUST have an entry; the `satisfies` clause makes a
// missing, removed, or renamed identifier a compile error rather than a blank
// string at runtime.
//
// Message policy:
// - A verification refusal reads as "Refused:" plus the reason. It is never
//   softened into a generic "something went wrong".
// - Observed rail or recovery conditions (reorg, replacement, refund failure,
//   coordinator loss, unresolved loss) state what happened and what the swap
//   is doing about it.
// - No message ever includes a raw upstream or counterparty-supplied value.

import type { SwpErrorIdentifier } from "./identifiers.js";

export type SwpErrorMessageKey = `swap.error.${SwpErrorIdentifier}`;

/** The key under which an untrusted, unrecognized error report renders. */
export const UNRECOGNIZED_ERROR_KEY = "swap.error.unrecognized" as const;

export const swapErrorMessages = {
  "swap.error.swp_unsupported_profile":
    "Refused: the record does not use the mkt-swp swap profile.",
  "swap.error.swp_unsupported_version":
    "Refused: the swap uses a profile or verifier revision this app does not support.",
  "swap.error.swp_unsupported_critical_member":
    "Refused: the swap terms contain a critical field this app does not recognize.",
  "swap.error.swp_unsupported_extension":
    "Refused: the swap requests a rail or extension this app does not support.",
  "swap.error.swp_invalid_asset_id":
    "Refused: the asset identifier is malformed or not on the allowlist.",
  "swap.error.swp_invalid_pair": "Refused: the ordered asset pair does not match the swap type.",
  "swap.error.swp_side_disabled": "Refused: the provider is not offering this side of the pair.",
  "swap.error.swp_invalid_amount":
    "Refused: the amount is not a valid canonical amount for this pair, or is outside the offered range.",
  "swap.error.swp_invalid_fee": "Refused: the fee is malformed or exceeds the allowed policy.",
  "swap.error.swp_amount_equation_mismatch":
    "Refused: the output amount does not reproduce from the quoted terms.",
  "swap.error.swp_quote_expired":
    "Refused: the quote has expired. Request a fresh quote before continuing.",
  "swap.error.swp_order_selection_invalid":
    "Refused: the order changes a term the quote does not allow to be selected.",
  "swap.error.swp_contract_missing":
    "Refused: both signed swap contracts must be present before funding, and at least one is missing.",
  "swap.error.swp_contract_signer_invalid":
    "Refused: a swap contract is not signed by the expected party for its role.",
  "swap.error.swp_contract_digest_mismatch":
    "Refused: the two parties' contract records do not commit to the same contract bytes.",
  "swap.error.swp_contract_terms_mismatch":
    "Refused: the swap contract differs from the quote or the allowed order selection.",
  "swap.error.swp_price_feed_invalid": "Refused: the pinned price feed failed verification.",
  "swap.error.swp_price_feed_stale":
    "Refused: the pinned price feed observation is older than the allowed maximum age.",
  "swap.error.swp_terms_mismatch":
    "Refused: the invoice, script, amount, policy, or status differs from the quoted terms.",
  "swap.error.swp_reservation_missing":
    "Refused: the required reservation terms or proof are absent.",
  "swap.error.swp_reservation_expired":
    "Refused: the reservation expired before the order took effect. Request a fresh quote.",
  "swap.error.swp_reservation_overallocated":
    "Refused: the provider's active reservations exceed its committed capacity.",
  "swap.error.swp_reservation_fork":
    "Refused: the provider signed conflicting reservation records.",
  "swap.error.swp_reservation_proof_invalid":
    "Refused: the reservation proof does not establish what it claims.",
  "swap.error.swp_covenant_reserve_invalid":
    "Refused: the covenant reserve failed the covenant, amount, fill-rule, or double-use check.",
  "swap.error.swp_timeout_ladder_unsafe":
    "Refused: the refund timeout ordering is unsafe, so funds could be exposed.",
  "swap.error.swp_invoice_invalid":
    "Refused: the invoice failed a parse, network, amount, expiry, hash, or timelock check.",
  "swap.error.swp_payment_hash_mismatch":
    "Refused: the invoice, script, quote, and local secret do not commit to the same payment hash.",
  "swap.error.swp_script_invalid":
    "Refused: the on-chain script or Taproot tree is malformed or unsupported.",
  "swap.error.swp_script_commitment_mismatch":
    "Refused: the independently re-derived address does not match the one the counterparty supplied.",
  "swap.error.swp_musig_transcript_invalid":
    "Refused: the cooperative signing transcript failed verification.",
  "swap.error.swp_confirmation_insufficient":
    "Refused: the funding transaction has fewer confirmations than the quote requires.",
  "swap.error.swp_rbf_policy_violation":
    "Refused: the funding transaction violates the agreed replacement policy.",
  "swap.error.swp_replacement":
    "A tracked transaction was replaced on the network. The swap has re-entered recovery.",
  "swap.error.swp_reorg":
    "A chain reorganization displaced a previously observed transaction. The swap has re-entered recovery.",
  "swap.error.swp_funding_not_authorized":
    "Refused: funding cannot proceed until every verify-before-fund check has passed.",
  "swap.error.swp_status_signer_invalid":
    "Refused: the status record's author cannot claim the stated action.",
  "swap.error.swp_status_transition_invalid":
    "Refused: the status update is not a legal transition in the swap's state machine.",
  "swap.error.swp_status_gap": "Refused: the counterparty's status sequence has a gap.",
  "swap.error.swp_status_fork":
    "Refused: the counterparty signed two different status records at the same sequence number.",
  "swap.error.swp_cancel_ineffective":
    "Refused: the cancellation lacks required consent or follows an irreversible effect.",
  "swap.error.swp_evidence_unavailable":
    "Refused: the evidence bound to this swap cannot be retrieved, so the claim cannot be verified.",
  "swap.error.swp_evidence_mismatch":
    "Refused: the retrieved evidence does not match its committed digest, terms, or rail view.",
  "swap.error.swp_settlement_overclaim":
    "Refused: the claimed settlement exceeds what the verifier's evidence proves.",
  "swap.error.swp_exit_package_missing":
    "Refused: the exit package was not persisted before funding, so funding cannot proceed.",
  "swap.error.swp_exit_package_mismatch":
    "Refused: the exit package does not match the swap contract's digest or terms.",
  "swap.error.swp_exit_package_unusable":
    "Refused: the exit package still depends on coordination or secrets this device does not hold.",
  "swap.error.swp_secret_material_forbidden":
    "Refused: a relay or server artifact contains key material that must never leave this device.",
  "swap.error.swp_privacy_violation":
    "Refused: a field was shared beyond its permitted audience or receipt consent.",
  "swap.error.swp_external_effect_conflict":
    "Refused: one effect identifier maps to two different external operations.",
  "swap.error.swp_idempotency_conflict":
    "Refused: a signed record or effect key was reused with different input.",
  "swap.error.swp_refund_failed":
    "A required refund was rejected, conflicted, or became unsafe. This swap needs recovery attention.",
  "swap.error.swp_coordinator_unavailable":
    "The coordinator is unreachable. The swap has entered direct recovery using your locally held exit package.",
  "swap.error.swp_unresolved_loss":
    "Unresolved: the final state of principal, fees, or records is unknown. Keep this device's swap data until it is resolved.",
  "swap.error.unrecognized":
    "The counterparty reported an error this app does not recognize. Its content was not displayed. The swap cannot proceed on that report.",
} as const satisfies Record<SwpErrorMessageKey | typeof UNRECOGNIZED_ERROR_KEY, string>;

/** The message key for a typed MKT-SWP §17 error identifier. */
export const swpErrorMessageKey = <I extends SwpErrorIdentifier>(
  identifier: I,
): `swap.error.${I}` => `swap.error.${identifier}`;
