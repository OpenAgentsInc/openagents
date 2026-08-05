/**
 * Exact price-feed pinning checks and provenance (openagents#9316 §6,
 * MKT-SWP §3.4). When a Quote pins a feed, the UI renders the exact URL,
 * RFC 6901 JSON pointer, observed value, max age, and response digest, and
 * refuses a substitute host, mirror, fallback endpoint, alternate pointer,
 * or semantically equivalent price as a term mismatch. Bitcoin/Lightning
 * v1 usually pins no feed; `null` renders as "no feed pinned".
 */

export interface PinnedPriceFeed {
  /** Exact HTTPS URL both parties fetch; no userinfo or fragment. */
  readonly url: string;
  /** RFC 6901 JSON Pointer into the response body. */
  readonly valuePointer: string;
  /** Canonical decimal string. */
  readonly observedValue: string;
  readonly observedAtSeconds: number;
  readonly maxAgeSeconds: number;
  /** SHA-256 of the exact response bytes, 64 lower hex. */
  readonly responseSha256: string;
}

/** The requester's own fetch of the pinned feed, before Order. */
export interface PriceFeedFetchRecord {
  readonly url: string;
  readonly valuePointer: string;
  readonly value: string;
  readonly fetchedAtSeconds: number;
  readonly responseSha256: string;
}

export const PRICE_FEED_MISMATCH_MODES = [
  "substituted_url",
  "substituted_pointer",
  "value_mismatch",
  "digest_mismatch",
] as const;

export type PriceFeedMismatchMode = (typeof PRICE_FEED_MISMATCH_MODES)[number];

export type PriceFeedCheck =
  | { readonly ok: true }
  | {
      readonly ok: false;
      readonly error: "swp_price_feed_invalid";
      readonly mode: PriceFeedMismatchMode;
    }
  | { readonly ok: false; readonly error: "swp_price_feed_stale" };

/**
 * Check the requester's fetch against the pinned terms. URL comparison is
 * exact string equality: a substitute host, mirror, or fallback endpoint —
 * however semantically equivalent — is `swp_price_feed_invalid`
 * (`substituted_url`). Staleness applies the pinned `max_age_seconds` to
 * the pinned observation.
 */
export const checkPinnedPriceFeed = (
  pinned: PinnedPriceFeed,
  fetched: PriceFeedFetchRecord,
  nowSeconds: number,
): PriceFeedCheck => {
  if (fetched.url !== pinned.url) {
    return { ok: false, error: "swp_price_feed_invalid", mode: "substituted_url" };
  }
  if (fetched.valuePointer !== pinned.valuePointer) {
    return {
      ok: false,
      error: "swp_price_feed_invalid",
      mode: "substituted_pointer",
    };
  }
  if (fetched.responseSha256 !== pinned.responseSha256) {
    return { ok: false, error: "swp_price_feed_invalid", mode: "digest_mismatch" };
  }
  if (fetched.value !== pinned.observedValue) {
    return { ok: false, error: "swp_price_feed_invalid", mode: "value_mismatch" };
  }
  if (nowSeconds - pinned.observedAtSeconds > pinned.maxAgeSeconds) {
    return { ok: false, error: "swp_price_feed_stale" };
  }
  return { ok: true };
};

/** Render-ready provenance for a pinned feed (all fields shown verbatim). */
export interface PriceFeedProvenanceView {
  readonly url: string;
  readonly jsonPointer: string;
  readonly observedValue: string;
  readonly observedAtSeconds: number;
  readonly maxAgeSeconds: number;
  readonly responseSha256: string;
}

export const priceFeedProvenanceView = (
  pinned: PinnedPriceFeed,
): PriceFeedProvenanceView => ({
  url: pinned.url,
  jsonPointer: pinned.valuePointer,
  observedValue: pinned.observedValue,
  observedAtSeconds: pinned.observedAtSeconds,
  maxAgeSeconds: pinned.maxAgeSeconds,
  responseSha256: pinned.responseSha256,
});
