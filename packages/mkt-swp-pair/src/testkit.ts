/**
 * Deterministic Offering-corpus fixtures for pair-selection tests. Asset
 * ids follow the exact MKT-SWP §3.1 grammar; amounts are canonical §3.2
 * wire strings. Fixtures never invent identities a validator would refuse.
 */
import type { DiscoveredOffering, OfferingSide } from "./corpus.js";

/** Deterministic 32-lower-hex network references for fixture networks. */
export const TEST_NETWORK_A = "bip122:000000000019d6689c085ae165831e93";
export const TEST_NETWORK_B = "bip122:00000000933ea01ad0ee984209e9d151";

export const testAssetId = (
  rail: "chain" | "lightning",
  networkId: string = TEST_NETWORK_A,
): string => `swp:1:${networkId}:btc:${rail}`;

export const TEST_CHAIN_ASSET = testAssetId("chain");
export const TEST_LIGHTNING_ASSET = testAssetId("lightning");
export const TEST_SECOND_CHAIN_ASSET = testAssetId("chain", TEST_NETWORK_B);

export const testSide = (
  overrides: Partial<OfferingSide> = {},
): OfferingSide => ({
  inputAssetId: TEST_CHAIN_ASSET,
  outputAssetId: TEST_LIGHTNING_ASSET,
  min: "10000",
  max: "1000000",
  feeBps: "25",
  ...overrides,
});

export const TEST_FOLD_NOW = 1_754_265_600; // 2026-08-04T00:00:00Z
export const TEST_FRESHNESS_HORIZON = 3_600;

let fixtureCounter = 0;

/** Reset between suites for stable addresses inside one test. */
export const resetTestOfferingCounter = (): void => {
  fixtureCounter = 0;
};

const hex64 = (n: number): string => n.toString(16).padStart(64, "0");

export const testOffering = (
  overrides: Partial<DiscoveredOffering> = {},
): DiscoveredOffering => {
  fixtureCounter += 1;
  const n = fixtureCounter;
  return {
    offeringAddress: `39601:${hex64(n)}:offering-${n}`,
    providerAddress: `39600:${hex64(n)}:provider-${n}`,
    offeringStatus: "active",
    providerStatus: "active",
    availability: "available",
    publishedAtSeconds: TEST_FOLD_NOW - 60,
    observedAtSeconds: TEST_FOLD_NOW - 30,
    sides: [testSide()],
    ...overrides,
  };
};
