import { describe, expect, test } from "vite-plus/test";

import {
  PRICE_FEED_MISMATCH_MODES,
  checkPinnedPriceFeed,
  priceFeedProvenanceView,
  type PinnedPriceFeed,
  type PriceFeedFetchRecord,
} from "./price-feed.js";
import { PRICE_FEED_MISMATCH_MESSAGES } from "./messages.js";

const pinned: PinnedPriceFeed = {
  url: "https://feed.example/rate",
  valuePointer: "/data/value",
  observedValue: "100000000",
  observedAtSeconds: 1_754_265_600,
  maxAgeSeconds: 30,
  responseSha256: "ab".repeat(32),
};

const matchingFetch: PriceFeedFetchRecord = {
  url: pinned.url,
  valuePointer: pinned.valuePointer,
  value: pinned.observedValue,
  fetchedAtSeconds: pinned.observedAtSeconds + 5,
  responseSha256: pinned.responseSha256,
};

const NOW = pinned.observedAtSeconds + 10;

describe("exact price-feed pinning (MKT-SWP §3.4)", () => {
  test("the exact URL, pointer, value, and digest pass within the age window", () => {
    expect(checkPinnedPriceFeed(pinned, matchingFetch, NOW)).toEqual({ ok: true });
  });

  const substitutions: readonly {
    readonly name: string;
    readonly fetched: PriceFeedFetchRecord;
    readonly mode: string;
  }[] = [
    {
      name: "a substituted host",
      fetched: { ...matchingFetch, url: "https://mirror.example/rate" },
      mode: "substituted_url",
    },
    {
      name: "a fallback path on the same host",
      fetched: { ...matchingFetch, url: "https://feed.example/rate-fallback" },
      mode: "substituted_url",
    },
    {
      name: "an http downgrade of the same endpoint",
      fetched: { ...matchingFetch, url: "http://feed.example/rate" },
      mode: "substituted_url",
    },
    {
      name: "an alternate JSON pointer",
      fetched: { ...matchingFetch, valuePointer: "/data/mid" },
      mode: "substituted_pointer",
    },
    {
      name: "a response digest mismatch",
      fetched: { ...matchingFetch, responseSha256: "cd".repeat(32) },
      mode: "digest_mismatch",
    },
    {
      name: "a semantically equivalent but different value",
      fetched: { ...matchingFetch, value: "100000001" },
      mode: "value_mismatch",
    },
  ];
  for (const c of substitutions) {
    test(`${c.name} is refused as swp_price_feed_invalid`, () => {
      expect(checkPinnedPriceFeed(pinned, c.fetched, NOW)).toEqual({
        ok: false,
        error: "swp_price_feed_invalid",
        mode: c.mode,
      });
    });
  }

  test("an observation older than max_age_seconds is swp_price_feed_stale", () => {
    const late = pinned.observedAtSeconds + pinned.maxAgeSeconds + 1;
    expect(checkPinnedPriceFeed(pinned, matchingFetch, late)).toEqual({
      ok: false,
      error: "swp_price_feed_stale",
    });
  });

  test("the age boundary is inclusive: exactly max_age_seconds old still passes", () => {
    const boundary = pinned.observedAtSeconds + pinned.maxAgeSeconds;
    expect(checkPinnedPriceFeed(pinned, matchingFetch, boundary)).toEqual({
      ok: true,
    });
  });

  test("provenance renders every pinned field verbatim", () => {
    expect(priceFeedProvenanceView(pinned)).toEqual({
      url: pinned.url,
      jsonPointer: pinned.valuePointer,
      observedValue: pinned.observedValue,
      observedAtSeconds: pinned.observedAtSeconds,
      maxAgeSeconds: pinned.maxAgeSeconds,
      responseSha256: pinned.responseSha256,
    });
  });

  test("every mismatch mode has its own distinct message", () => {
    const texts = PRICE_FEED_MISMATCH_MODES.map(
      (mode) => PRICE_FEED_MISMATCH_MESSAGES[mode].message,
    );
    expect(new Set(texts).size).toBe(texts.length);
  });
});
