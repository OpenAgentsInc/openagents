/**
 * Domain model for MKT-SWP destination entry (openagents#9317, SWAP-2).
 *
 * One shared parser serves the address field, the invoice field, and the QR
 * scanner. Its result is a typed discriminated union that survives to the
 * presentation layer: every failure mode keeps its discriminant all the way
 * to the rendered message (the Boltz web app computes six discriminated
 * failure modes and then collapses them into one generic string; we keep
 * the discriminant — teardown §2.5, §3.5).
 *
 * Everything in this package is UX pre-checking. The MKT-SWP engine
 * (Immortal client crate behind the SWAP-0 boundary) owns
 * verify-before-fund truth; nothing here authorises funding. See
 * `verify.ts` for the verdict port the entry state consumes.
 */

/** Chains/rails a destination can name. MKT-SWP v1 is BTC chain + Lightning. */
export type DestinationRail = "chain" | "lightning";

/** The network the current pair settles on (from the Offering's asset id). */
export type BitcoinNetwork = "mainnet" | "testnet" | "signet" | "regtest";

/**
 * Network class recoverable from address bytes alone. Base58 version bytes
 * and the `tb` bech32 HRP cannot distinguish testnet from signet, and
 * base58 cannot distinguish either from regtest.
 */
export type AddressNetworkClass = "mainnet" | "test" | "regtest";

export type OnchainAddressType = "p2pkh" | "p2sh" | "p2wpkh" | "p2wsh" | "p2tr";

/** BIP-21 companion data carried alongside an on-chain address. */
export interface Bip21Details {
  /** Amount in satoshis; malformed BIP-21 amounts are treated as absent. */
  readonly amountSats: bigint | null;
  readonly label: string | null;
  readonly message: string | null;
}

export interface OnchainAddressDestination {
  readonly kind: "onchain_address";
  readonly rail: "chain";
  /** Normalised (lowercase bech32 / verbatim base58) address text. */
  readonly address: string;
  readonly addressType: OnchainAddressType;
  readonly networkClass: AddressNetworkClass;
  readonly bip21: Bip21Details | null;
}

/** Description commitment of a BOLT11 invoice (§7.2: d or h, exactly one). */
export type InvoiceDescriptionCommitment =
  | { readonly kind: "description"; readonly text: string }
  | { readonly kind: "description_hash"; readonly hashHex: string };

export interface Bolt11InvoiceDestination {
  readonly kind: "bolt11_invoice";
  readonly rail: "lightning";
  /** Normalised lowercase invoice text. */
  readonly invoice: string;
  readonly network: BitcoinNetwork;
  /**
   * Amount in millisatoshis. Always present: amountless invoices are
   * invalid in MKT-SWP v1 and never produce a parse success
   * (`invoice_amountless` → `swp_invoice_invalid`).
   */
  readonly amountMsat: bigint;
  readonly paymentHashHex: string;
  /** Invoice creation time (seconds since epoch, from the 35-bit field). */
  readonly timestampSeconds: number;
  /** Relative expiry in seconds (`x` field; BOLT11 default 3600). */
  readonly expirySeconds: number;
  /** Absolute expiry, `timestampSeconds + expirySeconds`. */
  readonly expiresAtSeconds: number;
  /** `c` field (BOLT11 default 18). The engine checks it against the quote. */
  readonly minFinalCltvExpiryDelta: number;
  readonly description: InvoiceDescriptionCommitment;
  /** Route-hint disclosure surface (§7.2 route-hint disclosure policy). */
  readonly routeHintCount: number;
  readonly hasPaymentSecret: boolean;
}

/**
 * Deferred destinations bind their amount at order time, not at entry time
 * (issue #9317 §7): they survive amount edits and resolve to a concrete
 * invoice with a bounded timeout in `resolve.ts`.
 */
export interface LnurlDestination {
  readonly kind: "deferred_lnurl";
  readonly rail: "lightning";
  readonly url: string;
}

export interface LightningAddressDestination {
  readonly kind: "deferred_lightning_address";
  readonly rail: "lightning";
  readonly address: string;
  /** LUD-16 well-known resolution URL. */
  readonly resolveUrl: string;
}

export interface Bolt12OfferDestination {
  readonly kind: "deferred_bolt12_offer";
  readonly rail: "lightning";
  /** Normalised lowercase `lno1…` string. Deep parsing is engine work. */
  readonly offer: string;
}

export type DeferredDestination =
  | LnurlDestination
  | LightningAddressDestination
  | Bolt12OfferDestination;

/**
 * A unified QR / BIP-21 URI carrying both an on-chain address and a
 * Lightning destination. Amount precedence is resolved here, once, so two
 * conflicting amounts can never reach the widget (issue #9317 §6): an
 * amount-bearing invoice suppresses the BIP-21 amount.
 */
export interface UnifiedDestination {
  readonly kind: "unified";
  readonly onchain: OnchainAddressDestination;
  readonly lightning: Bolt11InvoiceDestination | DeferredDestination | null;
  /**
   * Set when the embedded `lightning=` value failed to parse; the on-chain
   * leg remains usable and the failure keeps its discriminant.
   */
  readonly lightningFailure: DestinationParseFailure | null;
  /** True when an invoice amount suppressed a present BIP-21 amount. */
  readonly bip21AmountSuppressed: boolean;
}

export type ParsedDestination =
  | OnchainAddressDestination
  | Bolt11InvoiceDestination
  | DeferredDestination
  | UnifiedDestination;

/**
 * Every distinct parse failure mode. A test asserts that no two modes
 * collapse into one rendered message. Order is presentation-neutral.
 */
export const DESTINATION_PARSE_FAILURE_MODES = [
  "empty",
  "evm_destination_unsupported",
  "uri_scheme_unsupported",
  "uri_required_param_unsupported",
  "address_checksum_invalid",
  "address_encoding_mismatch",
  "address_witness_program_invalid",
  "address_base58_version_unknown",
  "address_network_mismatch",
  "invoice_checksum_invalid",
  "invoice_malformed",
  "invoice_amount_invalid",
  "invoice_network_mismatch",
  "invoice_expired",
  "invoice_amountless",
  "lnurl_malformed",
  "offer_malformed",
  "route_unreachable",
  "unrecognized",
] as const;

export type DestinationParseFailureMode =
  (typeof DESTINATION_PARSE_FAILURE_MODES)[number];

/**
 * MKT-SWP §17 identifiers this surface can carry. Parse-level refusals map
 * to an identifier only where the profile defines one (amountless invoices
 * are refused with `swp_invoice_invalid`, issue #9317 §5). Engine verdicts
 * in `verify.ts` carry the script identifiers.
 */
export type SwpDestinationErrorId =
  | "swp_invoice_invalid"
  | "swp_script_invalid"
  | "swp_script_commitment_mismatch"
  | "swp_invalid_asset_id";

interface FailureBase<Mode extends DestinationParseFailureMode> {
  readonly ok?: undefined;
  readonly mode: Mode;
  /** MKT-SWP §17 identifier, where the profile defines one for this mode. */
  readonly swpError: SwpDestinationErrorId | null;
}

export interface EmptyFailure extends FailureBase<"empty"> {}

export interface EvmDestinationFailure
  extends FailureBase<"evm_destination_unsupported"> {}

export interface UriSchemeFailure extends FailureBase<"uri_scheme_unsupported"> {
  readonly scheme: string;
}

export interface UriRequiredParamFailure
  extends FailureBase<"uri_required_param_unsupported"> {
  readonly param: string;
}

export interface AddressChecksumFailure
  extends FailureBase<"address_checksum_invalid"> {}

export interface AddressEncodingMismatchFailure
  extends FailureBase<"address_encoding_mismatch"> {
  readonly witnessVersion: number;
}

export interface AddressWitnessProgramFailure
  extends FailureBase<"address_witness_program_invalid"> {
  readonly witnessVersion: number;
  readonly programLength: number;
}

export interface AddressBase58VersionFailure
  extends FailureBase<"address_base58_version_unknown"> {
  readonly versionByte: number;
}

export interface AddressNetworkMismatchFailure
  extends FailureBase<"address_network_mismatch"> {
  readonly expected: BitcoinNetwork;
  readonly actual: AddressNetworkClass;
}

export interface InvoiceChecksumFailure
  extends FailureBase<"invoice_checksum_invalid"> {}

export interface InvoiceMalformedFailure extends FailureBase<"invoice_malformed"> {
  readonly detail: string;
}

export interface InvoiceAmountInvalidFailure
  extends FailureBase<"invoice_amount_invalid"> {
  readonly amountText: string;
}

export interface InvoiceNetworkMismatchFailure
  extends FailureBase<"invoice_network_mismatch"> {
  readonly expected: BitcoinNetwork;
  readonly actual: BitcoinNetwork;
}

export interface InvoiceExpiredFailure extends FailureBase<"invoice_expired"> {
  readonly expiresAtSeconds: number;
}

export interface InvoiceAmountlessFailure
  extends FailureBase<"invoice_amountless"> {
  readonly swpError: "swp_invoice_invalid";
}

export interface LnurlMalformedFailure extends FailureBase<"lnurl_malformed"> {}

export interface OfferMalformedFailure extends FailureBase<"offer_malformed"> {}

/**
 * Produced by the entry state when a pasted destination would switch the
 * pair to a direction no live Offering can reach (SWAP-1 reachability).
 */
export interface RouteUnreachableFailure extends FailureBase<"route_unreachable"> {
  readonly requiredRail: DestinationRail;
}

export interface UnrecognizedFailure extends FailureBase<"unrecognized"> {}

export type DestinationParseFailure =
  | EmptyFailure
  | EvmDestinationFailure
  | UriSchemeFailure
  | UriRequiredParamFailure
  | AddressChecksumFailure
  | AddressEncodingMismatchFailure
  | AddressWitnessProgramFailure
  | AddressBase58VersionFailure
  | AddressNetworkMismatchFailure
  | InvoiceChecksumFailure
  | InvoiceMalformedFailure
  | InvoiceAmountInvalidFailure
  | InvoiceNetworkMismatchFailure
  | InvoiceExpiredFailure
  | InvoiceAmountlessFailure
  | LnurlMalformedFailure
  | OfferMalformedFailure
  | RouteUnreachableFailure
  | UnrecognizedFailure;

export type DestinationParseResult =
  | {
      readonly ok: true;
      readonly destination: ParsedDestination;
      /**
       * Set when the destination's rail differs from the field's rail:
       * pasting the wrong kind of destination switches the pair rather
       * than refusing it (teardown §3.5), subject to reachability.
       */
      readonly requiredRail: DestinationRail | null;
    }
  | { readonly ok: false; readonly failure: DestinationParseFailure };

/** Rail of a parsed destination; unified resolves per current rail. */
export const destinationRail = (
  destination: ParsedDestination,
): DestinationRail | "both" =>
  destination.kind === "unified"
    ? destination.lightning === null
      ? "chain"
      : "both"
    : destination.rail;
