// The MKT-SWP §17 error vocabulary. Source of truth:
// immortal `nips/openagents/MKT-SWP.md` §17 "Errors and loss states".
//
// These are stable lowercase protocol identifiers, not display strings. The UI
// maps them to localisable messages through the table in `error-table.ts` and
// never renders a counterparty's prose. Deterministic matching on this bounded
// enum is the typed-identifier case the workspace semantic-routing rule
// permits; free-text matching on upstream error prose is what it forbids.

export const SWP_ERROR_IDENTIFIERS = [
  "swp_unsupported_profile",
  "swp_unsupported_version",
  "swp_unsupported_critical_member",
  "swp_unsupported_extension",
  "swp_invalid_asset_id",
  "swp_invalid_pair",
  "swp_side_disabled",
  "swp_invalid_amount",
  "swp_invalid_fee",
  "swp_amount_equation_mismatch",
  "swp_quote_expired",
  "swp_order_selection_invalid",
  "swp_contract_missing",
  "swp_contract_signer_invalid",
  "swp_contract_digest_mismatch",
  "swp_contract_terms_mismatch",
  "swp_price_feed_invalid",
  "swp_price_feed_stale",
  "swp_terms_mismatch",
  "swp_reservation_missing",
  "swp_reservation_expired",
  "swp_reservation_overallocated",
  "swp_reservation_fork",
  "swp_reservation_proof_invalid",
  "swp_covenant_reserve_invalid",
  "swp_timeout_ladder_unsafe",
  "swp_invoice_invalid",
  "swp_payment_hash_mismatch",
  "swp_script_invalid",
  "swp_script_commitment_mismatch",
  "swp_liquid_network_mismatch",
  "swp_liquid_output_invalid",
  "swp_liquid_unblind_failed",
  "swp_liquid_unblind_mismatch",
  "swp_musig_transcript_invalid",
  "swp_confirmation_insufficient",
  "swp_rbf_policy_violation",
  "swp_replacement",
  "swp_reorg",
  "swp_funding_not_authorized",
  "swp_status_signer_invalid",
  "swp_status_transition_invalid",
  "swp_status_gap",
  "swp_status_fork",
  "swp_cancel_ineffective",
  "swp_evidence_unavailable",
  "swp_evidence_mismatch",
  "swp_settlement_overclaim",
  "swp_exit_package_missing",
  "swp_exit_package_mismatch",
  "swp_exit_package_unusable",
  "swp_secret_material_forbidden",
  "swp_privacy_violation",
  "swp_external_effect_conflict",
  "swp_idempotency_conflict",
  "swp_refund_failed",
  "swp_coordinator_unavailable",
  "swp_unresolved_loss",
] as const;

export type SwpErrorIdentifier = (typeof SWP_ERROR_IDENTIFIERS)[number];

const identifierSet: ReadonlySet<string> = new Set(SWP_ERROR_IDENTIFIERS);

/**
 * Type guard for untrusted input. A counterparty-supplied error string is
 * either one of the protocol's typed identifiers or it is not rendered at
 * all — `messageForReportedError` maps the latter to the typed
 * "unrecognized" message rather than echoing the input.
 */
export const isSwpErrorIdentifier = (value: string): value is SwpErrorIdentifier =>
  identifierSet.has(value);
