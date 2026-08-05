/**
 * Deterministic fixtures for quote-comparison tests. Until immortal#14
 * lands a second independent provider, the comparison surface is exercised
 * against this seeded corpus: builders produce conforming Quotes from
 * distinct providers with controllable class, reservation, custody, and
 * expiry, plus engine verify-reports in every gate-relevant shape.
 */
import type {
  CompareQuote,
  CustodyDisclosure,
  CustodyEntry,
  ReservationTerms,
} from "./model.js";
import type { VerifyBeforeFundReport, VerifyCheckRow } from "./verify.js";
import { VERIFY_CHECK_IDS } from "./verify.js";

/** Deterministic 64-lower-hex id from a small seed. */
export const testHexId = (seed: number): string =>
  seed.toString(16).padStart(4, "0").repeat(16);

const NONCUSTODIAL_ENTRIES: readonly CustodyEntry[] = [
  { leg: "source", phase: "pre_funding", holder: "requester" },
  { leg: "source", phase: "funded", holder: "contract" },
  { leg: "destination", phase: "settlement", holder: "consensus" },
];

/** A conforming noncustodial script-route disclosure (§6 v1 rules). */
export const noncustodialScriptCustody = (
  overrides: Partial<CustodyDisclosure> = {},
): CustodyDisclosure => ({
  fundsControl: NONCUSTODIAL_ENTRIES,
  executionControl: [
    { leg: "source", phase: "funded", holder: "contract" },
    { leg: "lightning", phase: "settlement", holder: "lightning_htlc" },
  ],
  settlementAuthority: [
    { leg: "source", phase: "settlement", holder: "consensus" },
    { leg: "lightning", phase: "settlement", holder: "lightning_htlc" },
  ],
  reversibility: [{ leg: "source", phase: "funded", holder: "contract" }],
  recourse: [{ leg: "source", phase: "exit", holder: "contract" }],
  credentialExposure: "none",
  maximumCustodyDurationSeconds: 86_400,
  maximumCustodyHeightBound: 144n,
  ...overrides,
});

/** A mint/federation route: custodial by construction (MKT-MINT). */
export const custodialMintCustody = (
  overrides: Partial<CustodyDisclosure> = {},
): CustodyDisclosure => ({
  ...noncustodialScriptCustody(),
  fundsControl: [
    { leg: "source", phase: "pre_funding", holder: "requester" },
    { leg: "mint", phase: "funded", holder: "federation" },
  ],
  settlementAuthority: [{ leg: "mint", phase: "settlement", holder: "mint" }],
  credentialExposure: "ecash_notes",
  ...overrides,
});

export const testReservation = (
  overrides: Partial<ReservationTerms> = {},
): ReservationTerms => ({
  reservationId: testHexId(0xa1),
  capacityBucketId: "bucket-btc-out",
  reservedAssetId: "btc:mainnet",
  reservedAmountSats: 250_000n,
  reservationExpiresAtSeconds: 1_800_000_600,
  allocationSequence: 42n,
  proofClass: "provider_signed",
  capacityCommitmentSha256: testHexId(0xc0),
  ...overrides,
});

export interface TestQuoteOptions extends Partial<Omit<CompareQuote, "reservation">> {
  readonly reservation?: ReservationTerms | null;
}

/**
 * A conforming indicative/none Quote by default; overrides shape the rest.
 * NOTE: overriding `quoteClass`/`reservationClass` does not auto-supply
 * reservation terms — pass `reservation` explicitly for reserving Quotes.
 */
export const testQuote = (options: TestQuoteOptions = {}): CompareQuote => ({
  eventId: testHexId(1),
  providerPubkey: testHexId(0xf1),
  rfqEventId: testHexId(0xee),
  createdAtSeconds: 1_800_000_000,
  quoteClass: "indicative",
  reservationClass: "none",
  reservation: null,
  custody: noncustodialScriptCustody(),
  inputAmountSats: 1_000_000n,
  outputAmountSats: 990_000n,
  fees: [
    { kind: "provider_fee", amountSats: 6_000n },
    { kind: "chain_fee", amountSats: 4_000n },
  ],
  expiresAtSeconds: 1_800_000_900,
  selectable: null,
  ...options,
});

/** A conforming firm/hard Quote from a second independent provider. */
export const testFirmHardQuote = (
  options: TestQuoteOptions = {},
): CompareQuote =>
  testQuote({
    eventId: testHexId(2),
    providerPubkey: testHexId(0xf2),
    quoteClass: "firm",
    reservationClass: "hard",
    reservation: testReservation({ proofClass: "covenant_reserve" }),
    ...options,
  });

/** All §7 rows passing. */
export const allPassRows = (): readonly VerifyCheckRow[] =>
  VERIFY_CHECK_IDS.map(id => ({ id, status: "pass" as const }));

export const testVerifyReport = (
  overrides: Partial<VerifyBeforeFundReport> = {},
): VerifyBeforeFundReport => ({
  quoteEventId: testHexId(2),
  orderEventId: null,
  epoch: 1,
  rows: allPassRows(),
  verdict: "verification_passed",
  ...overrides,
});
