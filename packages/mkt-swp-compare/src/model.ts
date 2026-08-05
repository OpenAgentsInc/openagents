/**
 * Domain model for MKT-SWP multi-provider quote comparison
 * (openagents#9318, SWAP-3).
 *
 * A `CompareQuote` is the client-side projection of one signed `kind:39605`
 * Quote answering one RFQ. Boltz structurally cannot have this surface: its
 * API is the market, so it shows one price with no quote identity, no
 * expiry, and no competing providers. NIP-MKT Quotes are signed records from
 * independent providers, so the comparison table, the expiry countdown, and
 * the per-quote commitment/custody disclosure all become possible — and this
 * model keeps each of those properties as distinct typed data, never
 * flattened into one "price".
 *
 * Everything in this package is UX pre-checking and presentation truth. The
 * MKT-SWP engine (Immortal client crate behind the SWAP-0 boundary) owns
 * verify-before-fund truth; nothing here authorises funding. See `verify.ts`
 * for the engine report port the funding gate consumes.
 *
 * Source of truth for the vocabulary: immortal `nips/openagents/MKT-SWP.md`
 * §4.3 (Quote), §5 (reservation accounting), §6 (custody dimensions),
 * §17 (error identifiers, re-used via `@openagentsinc/swap-i18n`).
 */
import type { SwpErrorIdentifier } from "@openagentsinc/swap-i18n";

/**
 * Quote class (§4.3): `firm` declares that a conforming timely Order is
 * accepted under stated preconditions — a declaration, not a proof of the
 * declaration or of capacity. `indicative` binds nothing until a provider
 * `Status state=accepted` arrives. The two are distinct commitments and the
 * view layer must never render them with one shared badge.
 */
export type QuoteClass = "indicative" | "firm";

export const QUOTE_CLASSES = ["indicative", "firm"] as const;

/** Base reservation class (§5). */
export type ReservationClass = "none" | "soft" | "hard";

export const RESERVATION_CLASSES = ["none", "soft", "hard"] as const;

/**
 * Reservation proof classes (§5). A signed claim (`provider_signed`) and a
 * covenant-enforced reserve (`covenant_reserve`) are not the same evidence
 * and must not render the same.
 */
export const RESERVATION_PROOF_CLASSES = [
  "provider_signed",
  "handler_accounted",
  "utxo_control",
  "lightning_liquidity",
  "funded_htlc",
  "covenant_reserve",
  "third_party_guarantee",
] as const;

export type ReservationProofClass = (typeof RESERVATION_PROOF_CLASSES)[number];

/**
 * Which reservation classes each proof class may back (§5 table).
 * `provider_signed` can never back `hard`; a Quote claiming otherwise is
 * nonconforming (`swp_reservation_proof_invalid`) and is refused for
 * selection, not silently reranked.
 */
export const ALLOWED_RESERVATION_FOR_PROOF: Readonly<
  Record<ReservationProofClass, readonly ReservationClass[]>
> = {
  provider_signed: ["soft"],
  handler_accounted: ["soft", "hard"],
  utxo_control: ["hard"],
  lightning_liquidity: ["hard"],
  funded_htlc: ["hard"],
  covenant_reserve: ["hard"],
  third_party_guarantee: ["hard"],
};

/** Section 5 `reservation_terms`, client projection. */
export interface ReservationTerms {
  /** 64-lower-hex, unique per provider and Quote. */
  readonly reservationId: string;
  readonly capacityBucketId: string;
  readonly reservedAssetId: string;
  readonly reservedAmountSats: bigint;
  readonly reservationExpiresAtSeconds: number;
  /** Canonical decimal string in the record; parsed to bigint here. */
  readonly allocationSequence: bigint;
  readonly proofClass: ReservationProofClass;
  readonly capacityCommitmentSha256: string;
}

/**
 * Who controls value or authority during one phase of one leg (§6). This is
 * a closed vocabulary on purpose: `custody.ts` classifies the route
 * fail-closed, and anything a future record carries that this client does
 * not recognise must be ingested as `unknown` — which classifies as
 * custodial, never as noncustodial.
 */
export const CUSTODY_HOLDERS = [
  /** The requester principal itself (own funds before funding). */
  "requester",
  /** The provider principal itself (own funds before funding). */
  "provider",
  /** The verified hash/timelock script construction. */
  "contract",
  /** Bitcoin consensus. */
  "consensus",
  /** Lightning HTLC rules / BOLT settlement. */
  "lightning_htlc",
  /** A mint. Custodial — MKT-MINT enforces `custody_class` at the relay. */
  "mint",
  /** A federation. Custodial. */
  "federation",
  /** Any other named third party (guarantor, coordinator, service). */
  "third_party",
  /** Unrecognised holder vocabulary — classifies as custodial. */
  "unknown",
] as const;

export type CustodyHolder = (typeof CUSTODY_HOLDERS)[number];

/** One entry of a §6 custody array: leg + phase + who holds it. */
export interface CustodyEntry {
  /** e.g. "source", "destination", "lightning". */
  readonly leg: string;
  /** e.g. "pre_funding", "funded", "settlement", "exit". */
  readonly phase: string;
  readonly holder: CustodyHolder;
}

/**
 * The six §6 dimensions plus both custody-duration bounds. The two bounds
 * are distinct data on purpose: `maximumCustodyDurationSeconds` is a
 * wall-clock ESTIMATE and `maximumCustodyHeightBound` is the exact
 * height-based bound. Clients display both and never convert the estimate
 * into consensus authority (§6, last paragraph).
 */
export interface CustodyDisclosure {
  readonly fundsControl: readonly CustodyEntry[];
  readonly executionControl: readonly CustodyEntry[];
  readonly settlementAuthority: readonly CustodyEntry[];
  readonly reversibility: readonly CustodyEntry[];
  readonly recourse: readonly CustodyEntry[];
  readonly credentialExposure: string;
  /** Worst-case wall-clock estimate, first funding → last unilateral exit. */
  readonly maximumCustodyDurationSeconds: number;
  /** The exact height-based bound the Quote also carries. */
  readonly maximumCustodyHeightBound: bigint;
}

/** One line of the Quote's fee breakdown (§3.3). */
export interface FeeLine {
  readonly kind: "provider_fee" | "chain_fee" | "lightning_fee";
  readonly amountSats: bigint;
}

/**
 * The Quote's finite selection lists (§4.4). An Order may choose only from
 * these; a Quote with `selectable: null` permits no Order selection at all.
 */
export interface QuoteSelectable {
  /** Explicitly offered input-amount range, or null when fixed. */
  readonly inputAmountRangeSats: {
    readonly minSats: bigint;
    readonly maxSats: bigint;
  } | null;
  readonly feePayers: readonly string[];
  readonly confirmationPolicies: readonly string[];
  readonly publicReceiptConsent: readonly string[];
}

/** Client projection of one signed `kind:39605` Quote answering one RFQ. */
export interface CompareQuote {
  /** Nostr event id, 64-lower-hex. The identity an Order commits to. */
  readonly eventId: string;
  readonly providerPubkey: string;
  readonly rfqEventId: string;
  /** Signed record `created_at` — the freshness column. */
  readonly createdAtSeconds: number;
  readonly quoteClass: QuoteClass;
  readonly reservationClass: ReservationClass;
  /** Present when the Quote reserves capacity (§5); null for `none`. */
  readonly reservation: ReservationTerms | null;
  readonly custody: CustodyDisclosure;
  readonly inputAmountSats: bigint;
  readonly outputAmountSats: bigint;
  readonly fees: readonly FeeLine[];
  /** Quote `expiration` (absolute Unix seconds). */
  readonly expiresAtSeconds: number;
  readonly selectable: QuoteSelectable | null;
}

/**
 * Structural conformance issues this client can detect from the record
 * alone. These are UX pre-checks: the engine re-verifies everything behind
 * the SWAP-0 boundary. A nonconforming Quote stays visible in the table
 * (retained evidence) but is never selectable and ranks below every
 * conforming Quote.
 */
export interface QuoteConformanceIssue {
  readonly error: SwpErrorIdentifier;
  readonly detail: string;
}

/**
 * §5: a firm Quote MUST reserve `soft` or `hard` and carry reservation
 * terms; a reserving Quote's proof class must be allowed to back its
 * reservation class.
 */
export const quoteConformance = (
  quote: CompareQuote,
): readonly QuoteConformanceIssue[] => {
  const issues: QuoteConformanceIssue[] = [];
  if (quote.quoteClass === "firm" && quote.reservationClass === "none") {
    issues.push({
      error: "swp_reservation_missing",
      detail: "firm quote declares no reservation; §5 requires soft or hard",
    });
  }
  if (quote.reservationClass !== "none" && quote.reservation === null) {
    issues.push({
      error: "swp_reservation_missing",
      detail: `reservation class "${quote.reservationClass}" without reservation_terms`,
    });
  }
  if (quote.reservation !== null) {
    const allowed = ALLOWED_RESERVATION_FOR_PROOF[quote.reservation.proofClass];
    if (
      quote.reservationClass === "none" ||
      !allowed.includes(quote.reservationClass)
    ) {
      issues.push({
        error: "swp_reservation_proof_invalid",
        detail: `proof class "${quote.reservation.proofClass}" cannot back reservation "${quote.reservationClass}"`,
      });
    }
  }
  return issues;
};
