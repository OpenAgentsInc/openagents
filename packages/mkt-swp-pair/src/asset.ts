/**
 * Asset identity for MKT-SWP pair selection (openagents#9316, SWAP-1).
 *
 * `asset_id` is identity (MKT-SWP §3.1): the exact string
 * `swp:1:bip122:<32-lower-hex>:btc:{chain,lightning}`. Tickers, display
 * names, symbols, and provider-local pair codes are labels only and MUST
 * NOT be used for matching, grouping, pricing, or replay identity. Every
 * comparison in this package is string equality on the full `asset_id`.
 */

/** Settlement rail of one asset. MKT-SWP v1 is BTC chain + Lightning. */
export type SwapRail = "chain" | "lightning";

/** Exact grammar from MKT-SWP §3.1. */
const ASSET_ID_PATTERN = /^swp:1:(bip122:[0-9a-f]{32}):btc:(chain|lightning)$/;

export interface SwapAsset {
  /** The identity: the exact `asset_id` string. */
  readonly assetId: string;
  /** `bip122:<32-lower-hex>` network identifier. */
  readonly networkId: string;
  readonly rail: SwapRail;
}

/** Parse an `asset_id`; `null` when it does not match the v1 grammar. */
export const parseAssetId = (value: string): SwapAsset | null => {
  const match = ASSET_ID_PATTERN.exec(value);
  if (match === null) return null;
  const networkId = match[1];
  const rail = match[2];
  if (networkId === undefined || (rail !== "chain" && rail !== "lightning")) {
    return null;
  }
  return { assetId: value, networkId, rail };
};

/**
 * Display labels for one asset. Labels are presentation only; nothing in
 * this package matches, groups, prices, or keys on them, and a test guards
 * that the corpus fold and selection reducer never read this table.
 */
export interface AssetLabels {
  readonly ticker: string;
  readonly displayName: string;
}

/**
 * Default labels derived from the rail. A deployment may override per
 * exact `asset_id` (never per ticker) via `labelFor`'s override map.
 */
export const defaultAssetLabels = (asset: SwapAsset): AssetLabels =>
  asset.rail === "lightning"
    ? { ticker: "BTC", displayName: "Bitcoin (Lightning)" }
    : { ticker: "BTC", displayName: "Bitcoin" };

export const labelFor = (
  asset: SwapAsset,
  overrides?: Readonly<Record<string, AssetLabels>>,
): AssetLabels => overrides?.[asset.assetId] ?? defaultAssetLabels(asset);

/** An ordered market direction: `[input_asset_id, output_asset_id]`. */
export interface SwapDirection {
  readonly inputAssetId: string;
  readonly outputAssetId: string;
}

/**
 * Map key for one ordered direction. `asset_id` strings cannot contain
 * whitespace, so a space join is collision-free. The key is internal —
 * never displayed, never parsed back.
 */
export const directionKey = (direction: SwapDirection): string =>
  `${direction.inputAssetId} ${direction.outputAssetId}`;

export const reverseDirection = (direction: SwapDirection): SwapDirection => ({
  inputAssetId: direction.outputAssetId,
  outputAssetId: direction.inputAssetId,
});

export type SwapType = "submarine" | "reverse" | "chain";

/**
 * The v1 swap type of an ordered pair (MKT-SWP §3.1), or `null` for a
 * shape outside v1 (lightning→lightning, or chain→chain on one network).
 */
export const swapTypeOf = (direction: SwapDirection): SwapType | null => {
  const input = parseAssetId(direction.inputAssetId);
  const output = parseAssetId(direction.outputAssetId);
  if (input === null || output === null) return null;
  if (input.rail === "chain" && output.rail === "lightning") return "submarine";
  if (input.rail === "lightning" && output.rail === "chain") return "reverse";
  if (input.rail === "chain" && output.rail === "chain") {
    return input.networkId === output.networkId ? null : "chain";
  }
  return null;
};
