/**
 * Behaviour-contract oracle for the unreachable-direction disclosure
 * (registry: `@openagentsinc/behavior-contracts`, market-swap-pair):
 * - openagents_web.swap_pair.unreachable_direction_disclosed.v1
 */
import { describe, expect, test } from "vite-plus/test";

import { directionKey } from "./asset.js";
import {
  directionAvailability,
  foldOfferingCorpus,
  isOutputRailReachable,
  type CorpusFoldConfig,
} from "./corpus.js";
import {
  TEST_CHAIN_ASSET,
  TEST_FOLD_NOW,
  TEST_FRESHNESS_HORIZON,
  TEST_LIGHTNING_ASSET,
  TEST_SECOND_CHAIN_ASSET,
  testOffering,
  testSide,
} from "./testkit.js";

const config: CorpusFoldConfig = {
  nowSeconds: TEST_FOLD_NOW,
  freshnessHorizonSeconds: TEST_FRESHNESS_HORIZON,
};

const submarine = {
  inputAssetId: TEST_CHAIN_ASSET,
  outputAssetId: TEST_LIGHTNING_ASSET,
};
const reverse = {
  inputAssetId: TEST_LIGHTNING_ASSET,
  outputAssetId: TEST_CHAIN_ASSET,
};

describe("empty corpus is a first-class typed state", () => {
  test("zero discovered offerings folds to state=empty, not a silent no-op", () => {
    const fold = foldOfferingCorpus([], config);
    expect(fold.state).toBe("empty");
    expect(
      directionAvailability(fold, submarine),
    ).toEqual({
      reachable: false,
      direction: submarine,
      reason: "no_offering",
      advertisingProviders: [],
    });
    expect(isOutputRailReachable(fold, "lightning")).toBe(false);
  });
});

describe("reachability folds from live Offerings, never a pair list", () => {
  test("one active fresh offering makes exactly its advertised direction reachable", () => {
    const fold = foldOfferingCorpus([testOffering()], config);
    expect(fold.state).toBe("populated");
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(true);
    if (availability.reachable) {
      expect(availability.minSats).toBe(10_000n);
      expect(availability.maxSats).toBe(1_000_000n);
      expect(availability.feeBpsMin).toBe(25n);
      expect(availability.feeBpsMax).toBe(25n);
    }
    // The reverse direction was not advertised: unreachable with reason.
    const reverseAvailability = directionAvailability(fold, reverse);
    expect(reverseAvailability.reachable).toBe(false);
    if (!reverseAvailability.reachable) {
      expect(reverseAvailability.reason).toBe("no_offering");
    }
    expect(isOutputRailReachable(fold, "lightning")).toBe(true);
    expect(isOutputRailReachable(fold, "chain")).toBe(false);
  });

  test("limits fold across providers: smallest min, largest max, fee range", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({ sides: [testSide({ min: "5000", max: "200000", feeBps: "10" })] }),
        testOffering({ sides: [testSide({ min: "10000", max: "900000", feeBps: "40" })] }),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(true);
    if (availability.reachable) {
      expect(availability.minSats).toBe(5_000n);
      expect(availability.maxSats).toBe(900_000n);
      expect(availability.feeBpsMin).toBe(10n);
      expect(availability.feeBpsMax).toBe(40n);
      expect(availability.sides).toHaveLength(2);
    }
  });

  test("assets are keyed by exact asset_id; the second network is distinct", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({
          sides: [
            testSide(),
            testSide({
              inputAssetId: TEST_CHAIN_ASSET,
              outputAssetId: TEST_SECOND_CHAIN_ASSET,
            }),
          ],
        }),
      ],
      config,
    );
    expect(fold.state).toBe("populated");
    if (fold.state === "populated") {
      expect(fold.assets.map((a) => a.assetId).sort()).toEqual(
        [TEST_CHAIN_ASSET, TEST_LIGHTNING_ASSET, TEST_SECOND_CHAIN_ASSET].sort(),
      );
      expect(fold.directions.size).toBe(2);
    }
  });

  test("malformed sides are dropped and disclosed, never silently folded", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({
          sides: [
            testSide(),
            testSide({ min: "0", max: "100" }), // enabled side needs positive min
            testSide({ min: "200", max: "100" }), // min > max
            testSide({ feeBps: "10001" }), // fee_bps above 10000
            testSide({ min: "1.5" }), // not canonical
            testSide({ inputAssetId: "swp:1:bip122:zz:btc:chain" }), // bad grammar
            testSide({
              inputAssetId: TEST_LIGHTNING_ASSET,
              outputAssetId: TEST_LIGHTNING_ASSET,
            }), // lightning→lightning is outside v1
          ],
        }),
      ],
      config,
    );
    expect(fold.state).toBe("populated");
    if (fold.state === "populated") {
      expect(fold.droppedSideCount).toBe(6);
      expect(fold.directions.size).toBe(1);
    }
  });
});

describe("unreachable_direction_disclosed.v1: max=\"0\" and the reason ladder", () => {
  test("a direction with max=\"0\" on every discovered Offering is side_disabled", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({ sides: [testSide({ max: "0" })] }),
        testOffering({ sides: [testSide({ max: "0", min: "0" })] }),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(false);
    if (!availability.reachable) {
      expect(availability.reason).toBe("side_disabled");
      expect(availability.advertisingProviders).toHaveLength(2);
    }
  });

  test("enabled sides carried only by paused providers are provider_paused", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({ providerStatus: "paused" }),
        testOffering({ offeringStatus: "exhausted" }),
        testOffering({ availability: "unavailable" }),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(false);
    if (!availability.reachable) expect(availability.reason).toBe("provider_paused");
  });

  test("enabled sides on active carriers observed beyond the horizon are offerings_stale", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({
          observedAtSeconds: TEST_FOLD_NOW - TEST_FRESHNESS_HORIZON - 1,
        }),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(false);
    if (!availability.reachable) expect(availability.reason).toBe("offerings_stale");
  });

  test("a paused fresh carrier outranks a stale one in the reason ladder", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({
          observedAtSeconds: TEST_FOLD_NOW - TEST_FRESHNESS_HORIZON - 1,
        }),
        testOffering({ providerStatus: "paused" }),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(false);
    if (!availability.reachable) expect(availability.reason).toBe("provider_paused");
  });

  test("one live enabled side outweighs any number of disabled/paused/stale ones", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({ sides: [testSide({ max: "0" })] }),
        testOffering({ providerStatus: "paused" }),
        testOffering({
          observedAtSeconds: TEST_FOLD_NOW - TEST_FRESHNESS_HORIZON - 1,
        }),
        testOffering(),
      ],
      config,
    );
    const availability = directionAvailability(fold, submarine);
    expect(availability.reachable).toBe(true);
    if (availability.reachable) expect(availability.sides).toHaveLength(1);
  });

  test("a boundary-fresh head (exactly at the horizon) still counts as live", () => {
    const fold = foldOfferingCorpus(
      [
        testOffering({
          observedAtSeconds: TEST_FOLD_NOW - TEST_FRESHNESS_HORIZON,
        }),
      ],
      config,
    );
    expect(directionAvailability(fold, submarine).reachable).toBe(true);
  });
});

describe("direction identity", () => {
  test("direction keys are ordered: submarine and reverse are distinct", () => {
    expect(directionKey(submarine)).not.toBe(directionKey(reverse));
  });
});
