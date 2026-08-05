/**
 * Local message table for destination entry.
 *
 * NOTE (SWAP-8 migration): the shared i18n/error table (openagents#9323,
 * SWAP-8) had not landed when this package was written. Keys are stable
 * `swap.destination.*` identifiers shaped for that table; when SWAP-8
 * lands, these entries migrate there and this file becomes a re-export.
 *
 * Law (issue #9317, teardown §2.5): every parse failure mode renders a
 * distinct message — the discriminant computed by the parser survives to
 * the presentation layer, unlike Boltz's collapse of six typed modes into
 * one generic string. A test asserts pairwise distinctness and
 * exhaustiveness over the mode set. No message ever renders a
 * counterparty's prose; MKT-SWP identifiers appear as typed identifiers.
 */
import type { EntryNotice } from "./entry.js";
import type {
  DestinationParseFailure,
  DestinationParseFailureMode,
  SwpDestinationErrorId,
} from "./model.js";

export interface DestinationMessage {
  /** Stable key, shaped for the SWAP-8 shared table. */
  readonly key: `swap.destination.${string}`;
  /** Default English message. Localisation arrives with SWAP-8. */
  readonly message: string;
}

export const DESTINATION_FAILURE_MESSAGES: Readonly<
  Record<DestinationParseFailureMode, DestinationMessage>
> = {
  empty: {
    key: "swap.destination.empty",
    message: "Enter a destination",
  },
  evm_destination_unsupported: {
    key: "swap.destination.evm_unsupported",
    message: "EVM destinations are not supported by this swap profile",
  },
  uri_scheme_unsupported: {
    key: "swap.destination.uri_scheme_unsupported",
    message: "This link type is not a supported destination",
  },
  uri_required_param_unsupported: {
    key: "swap.destination.uri_required_param_unsupported",
    message: "This payment link requires a feature this wallet does not support",
  },
  address_checksum_invalid: {
    key: "swap.destination.address_checksum_invalid",
    message: "This address fails its checksum — check for typos",
  },
  address_encoding_mismatch: {
    key: "swap.destination.address_encoding_mismatch",
    message: "This address uses the wrong encoding for its witness version",
  },
  address_witness_program_invalid: {
    key: "swap.destination.address_witness_program_invalid",
    message: "This address has an unsupported witness version or program size",
  },
  address_base58_version_unknown: {
    key: "swap.destination.address_base58_version_unknown",
    message: "This address has an unknown version prefix",
  },
  address_network_mismatch: {
    key: "swap.destination.address_network_mismatch",
    message: "This address is for a different network",
  },
  invoice_checksum_invalid: {
    key: "swap.destination.invoice_checksum_invalid",
    message: "This invoice fails its checksum — check it was copied completely",
  },
  invoice_malformed: {
    key: "swap.destination.invoice_malformed",
    message: "This invoice is malformed",
  },
  invoice_amount_invalid: {
    key: "swap.destination.invoice_amount_invalid",
    message: "This invoice carries an invalid amount encoding",
  },
  invoice_network_mismatch: {
    key: "swap.destination.invoice_network_mismatch",
    message: "This invoice is for a different network",
  },
  invoice_expired: {
    key: "swap.destination.invoice_expired",
    message: "This invoice has expired",
  },
  invoice_amountless: {
    key: "swap.destination.invoice_amountless",
    message:
      "Amountless invoices are not accepted (swp_invoice_invalid) — request an invoice with an amount",
  },
  lnurl_malformed: {
    key: "swap.destination.lnurl_malformed",
    message: "This LNURL cannot be decoded",
  },
  offer_malformed: {
    key: "swap.destination.offer_malformed",
    message: "This BOLT12 offer cannot be read",
  },
  route_unreachable: {
    key: "swap.destination.route_unreachable",
    message:
      "This destination needs a direction no provider currently offers",
  },
  unrecognized: {
    key: "swap.destination.unrecognized",
    message: "This is not a recognised address, invoice, or payment code",
  },
};

export const ENTRY_NOTICE_MESSAGES: Readonly<
  Record<EntryNotice["notice"], DestinationMessage>
> = {
  route_switched: {
    key: "swap.destination.notice.route_switched",
    message: "Direction switched to match the pasted destination",
  },
  invoice_cleared_by_amount_edit: {
    key: "swap.destination.notice.invoice_cleared_by_amount_edit",
    message: "Invoice removed — its amount no longer matched the edited amount",
  },
  bip21_amount_suppressed: {
    key: "swap.destination.notice.bip21_amount_suppressed",
    message: "Using the invoice amount from this payment code",
  },
  typed_amount_overridden_by_invoice: {
    key: "swap.destination.notice.typed_amount_overridden_by_invoice",
    message: "Amount set from the invoice",
  },
};

/**
 * Verdict identifiers from the engine (MKT-SWP §17). The identifier itself
 * is always shown alongside the message, never replaced by prose.
 */
export const SWP_DESTINATION_ERROR_MESSAGES: Readonly<
  Record<SwpDestinationErrorId, DestinationMessage>
> = {
  swp_invoice_invalid: {
    key: "swap.destination.swp.invoice_invalid",
    message: "The invoice failed local verification",
  },
  swp_script_invalid: {
    key: "swap.destination.swp.script_invalid",
    message: "The swap script failed local parsing",
  },
  swp_script_commitment_mismatch: {
    key: "swap.destination.swp.script_commitment_mismatch",
    message:
      "The counterparty's address does not match the locally re-derived script",
  },
  swp_invalid_asset_id: {
    key: "swap.destination.swp.invalid_asset_id",
    message: "The destination does not match the pair's asset",
  },
};

export const destinationFailureMessage = (
  failure: DestinationParseFailure,
): DestinationMessage => DESTINATION_FAILURE_MESSAGES[failure.mode];
