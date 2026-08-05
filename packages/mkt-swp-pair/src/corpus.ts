/**
 * Live-Offering corpus fold (openagents#9316 §1): reachable directions are
 * a function of the discovered `kind:39601` heads, never a hardcoded pair
 * list. Every `sides` entry (MKT-SWP §3.2) carries `min`, `max`, and
 * `fee_bps` per ordered pair, and `max="0"` explicitly disables that side —
 * so unreachable directions are computable *before* selection, with the
 * reason, instead of being discovered at the primary button (the Boltz
 * asset modal filters on "can send"/"disabled" but not reachability).
 *
 * Typed first-class states, never silent empties:
 * - `empty` corpus (no Offering discovered at all);
 * - per-direction unreachable reasons: `no_offering`, `side_disabled`,
 *   `provider_paused`, `offerings_stale`.
 */
import type { OfferingStatus, ProviderStatus } from "@openagentsinc/nip-mkt/generated";

import { directionKey, parseAssetId, swapTypeOf, type SwapAsset, type SwapDirection } from "./asset.js";
import { satsFromWire } from "./amount.js";

/** MKT-SWP Offering availability (§4.1). */
export type OfferingAvailability = "available" | "limited" | "unavailable";

/** One `sides` entry, verbatim wire strings (MKT-SWP §3.2). */
export interface OfferingSide {
  readonly inputAssetId: string;
  readonly outputAssetId: string;
  readonly min: string;
  readonly max: string;
  readonly feeBps: string;
}

/**
 * One discovered `kind:39601` Offering head, as the discovery layer
 * (nip-mkt subscriptions behind the SWAP-0 boundary) delivers it.
 * Addresses are NIP-MKT `kind:pubkey:d` coordinates and are identity for
 * provider attribution; display names are labels only.
 */
export interface DiscoveredOffering {
  readonly offeringAddress: string;
  readonly providerAddress: string;
  readonly offeringStatus: OfferingStatus;
  readonly providerStatus: ProviderStatus;
  readonly availability: OfferingAvailability;
  /** `published_at` tag, seconds. */
  readonly publishedAtSeconds: number;
  /** When our live subscription last confirmed this head, seconds. */
  readonly observedAtSeconds: number;
  readonly sides: readonly OfferingSide[];
}

export interface CorpusFoldConfig {
  readonly nowSeconds: number;
  /** A head observed longer ago than this is stale for reachability. */
  readonly freshnessHorizonSeconds: number;
}

/** One provider's enabled side of a direction, amounts parsed exactly. */
export interface DirectionSideSource {
  readonly providerAddress: string;
  readonly offeringAddress: string;
  readonly minSats: bigint;
  readonly maxSats: bigint;
  readonly feeBps: bigint;
  readonly observedAtSeconds: number;
  /** Live right now: active offering + active provider + fresh + not unavailable. */
  readonly live: boolean;
  readonly offeringStatus: OfferingStatus;
  readonly providerStatus: ProviderStatus;
  readonly availability: OfferingAvailability;
}

export const DIRECTION_UNREACHABLE_REASONS = [
  /** No discovered Offering advertises this ordered pair at all. */
  "no_offering",
  /** Advertised, but every advertised side has `max="0"` (§3.2 disable). */
  "side_disabled",
  /** Enabled sides exist, but every carrier is paused/exhausted/retired/unavailable. */
  "provider_paused",
  /** Enabled sides exist on active carriers, but none is fresh. */
  "offerings_stale",
] as const;

export type DirectionUnreachableReason =
  (typeof DIRECTION_UNREACHABLE_REASONS)[number];

export type DirectionAvailability =
  | {
      readonly reachable: true;
      readonly direction: SwapDirection;
      /** Every enabled live side; at least one. */
      readonly sides: readonly DirectionSideSource[];
      /** Smallest serviceable input across live sides. */
      readonly minSats: bigint;
      /** Largest serviceable input across live sides. */
      readonly maxSats: bigint;
      readonly feeBpsMin: bigint;
      readonly feeBpsMax: bigint;
      readonly freshestObservedAtSeconds: number;
    }
  | {
      readonly reachable: false;
      readonly direction: SwapDirection;
      readonly reason: DirectionUnreachableReason;
      /** Providers that advertise the pair (who would need to act). */
      readonly advertisingProviders: readonly string[];
    };

export type OfferingCorpusFold =
  | { readonly state: "empty"; readonly foldedAtSeconds: number }
  | {
      readonly state: "populated";
      readonly foldedAtSeconds: number;
      /** Keyed by `directionKey`; one entry per advertised ordered pair. */
      readonly directions: ReadonlyMap<string, DirectionAvailability>;
      /** Every asset appearing in any advertised side, by `asset_id`. */
      readonly assets: readonly SwapAsset[];
      /** Sides dropped for malformed asset ids or amounts (disclosed, not silent). */
      readonly droppedSideCount: number;
    };

interface SideAccumulator {
  readonly direction: SwapDirection;
  readonly advertisingProviders: Set<string>;
  readonly enabled: DirectionSideSource[];
  hasDisabled: boolean;
}

/**
 * Fold the discovered corpus into per-direction availability. Identity is
 * the exact ordered `asset_id` pair throughout; labels never participate.
 */
export const foldOfferingCorpus = (
  offerings: readonly DiscoveredOffering[],
  config: CorpusFoldConfig,
): OfferingCorpusFold => {
  if (offerings.length === 0) {
    return { state: "empty", foldedAtSeconds: config.nowSeconds };
  }

  const byDirection = new Map<string, SideAccumulator>();
  const assets = new Map<string, SwapAsset>();
  let droppedSideCount = 0;

  for (const offering of offerings) {
    const fresh =
      config.nowSeconds - offering.observedAtSeconds <=
      config.freshnessHorizonSeconds;
    for (const side of offering.sides) {
      const input = parseAssetId(side.inputAssetId);
      const output = parseAssetId(side.outputAssetId);
      const minSats = satsFromWire(side.min);
      const maxSats = satsFromWire(side.max);
      const feeBps = satsFromWire(side.feeBps);
      const direction: SwapDirection = {
        inputAssetId: side.inputAssetId,
        outputAssetId: side.outputAssetId,
      };
      if (
        input === null ||
        output === null ||
        minSats === null ||
        maxSats === null ||
        feeBps === null ||
        feeBps > 10_000n ||
        swapTypeOf(direction) === null
      ) {
        droppedSideCount += 1;
        continue;
      }

      assets.set(input.assetId, input);
      assets.set(output.assetId, output);

      const key = directionKey(direction);
      let accumulator = byDirection.get(key);
      if (accumulator === undefined) {
        accumulator = {
          direction,
          advertisingProviders: new Set(),
          enabled: [],
          hasDisabled: false,
        };
        byDirection.set(key, accumulator);
      }
      accumulator.advertisingProviders.add(offering.providerAddress);

      // `max="0"` disables this exact side (MKT-SWP §3.2). An enabled side
      // has positive min and max with min <= max; a violating side is
      // malformed and dropped.
      if (maxSats === 0n) {
        accumulator.hasDisabled = true;
        continue;
      }
      if (minSats === 0n || minSats > maxSats) {
        droppedSideCount += 1;
        continue;
      }
      accumulator.enabled.push({
        providerAddress: offering.providerAddress,
        offeringAddress: offering.offeringAddress,
        minSats,
        maxSats,
        feeBps,
        observedAtSeconds: offering.observedAtSeconds,
        live:
          fresh &&
          offering.offeringStatus === "active" &&
          offering.providerStatus === "active" &&
          offering.availability !== "unavailable",
        offeringStatus: offering.offeringStatus,
        providerStatus: offering.providerStatus,
        availability: offering.availability,
      });
    }
  }

  const directions = new Map<string, DirectionAvailability>();
  for (const [key, accumulator] of byDirection) {
    directions.set(key, availabilityOf(accumulator, config));
  }

  return {
    state: "populated",
    foldedAtSeconds: config.nowSeconds,
    directions,
    assets: [...assets.values()],
    droppedSideCount,
  };
};

const availabilityOf = (
  accumulator: SideAccumulator,
  config: CorpusFoldConfig,
): DirectionAvailability => {
  const live = accumulator.enabled.filter((side) => side.live);
  if (live.length > 0) {
    let minSats = live[0]?.minSats ?? 0n;
    let maxSats = 0n;
    let feeBpsMin = live[0]?.feeBps ?? 0n;
    let feeBpsMax = 0n;
    let freshest = 0;
    for (const side of live) {
      if (side.minSats < minSats) minSats = side.minSats;
      if (side.maxSats > maxSats) maxSats = side.maxSats;
      if (side.feeBps < feeBpsMin) feeBpsMin = side.feeBps;
      if (side.feeBps > feeBpsMax) feeBpsMax = side.feeBps;
      if (side.observedAtSeconds > freshest) freshest = side.observedAtSeconds;
    }
    return {
      reachable: true,
      direction: accumulator.direction,
      sides: live,
      minSats,
      maxSats,
      feeBpsMin,
      feeBpsMax,
      freshestObservedAtSeconds: freshest,
    };
  }

  // Unreachable: state the single most proximate reason. Precedence walks
  // from "closest to working": a paused carrier could re-activate
  // (provider_paused), a stale head could refresh (offerings_stale), an
  // explicit disable needs the provider to re-enable (side_disabled), and
  // otherwise no provider advertises the pair at all (no_offering).
  const fresh = (side: DirectionSideSource): boolean =>
    config.nowSeconds - side.observedAtSeconds <=
    config.freshnessHorizonSeconds;
  const paused = accumulator.enabled.some(
    (side) =>
      fresh(side) &&
      (side.offeringStatus !== "active" ||
        side.providerStatus !== "active" ||
        side.availability === "unavailable"),
  );
  const reason: DirectionUnreachableReason = paused
    ? "provider_paused"
    : accumulator.enabled.length > 0
      ? "offerings_stale"
      : accumulator.hasDisabled
        ? "side_disabled"
        : "no_offering";
  return {
    reachable: false,
    direction: accumulator.direction,
    reason,
    advertisingProviders: [...accumulator.advertisingProviders].sort(),
  };
};

/** Availability for one ordered direction, `no_offering` when unadvertised. */
export const directionAvailability = (
  fold: OfferingCorpusFold,
  direction: SwapDirection,
): DirectionAvailability =>
  (fold.state === "populated"
    ? fold.directions.get(directionKey(direction))
    : undefined) ?? {
    reachable: false,
    direction,
    reason: "no_offering",
    advertisingProviders: [],
  };

/**
 * Whether any live Offering serves a direction whose output settles on the
 * given rail — the reachability gate SWAP-2's paste-driven route switching
 * consumes (`EntryConfig.isRailReachable`).
 */
export const isOutputRailReachable = (
  fold: OfferingCorpusFold,
  rail: SwapAsset["rail"],
): boolean => {
  if (fold.state !== "populated") return false;
  for (const availability of fold.directions.values()) {
    if (!availability.reachable) continue;
    const output = parseAssetId(availability.direction.outputAssetId);
    if (output !== null && output.rail === rail) return true;
  }
  return false;
};
