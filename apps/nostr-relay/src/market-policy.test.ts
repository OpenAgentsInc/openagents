import { describe, expect, test } from "bun:test"

import {
  MarketRelayPolicy,
  isAllowedMarketKind,
  marketKindBucket,
  nextPublishBucket,
  validateReqFilters,
} from "./market-policy"

describe("market relay policy", () => {
  test("allows only scoped market event kinds", () => {
    expect(isAllowedMarketKind(5000)).toBe(true)
    expect(isAllowedMarketKind(5050)).toBe(true)
    expect(isAllowedMarketKind(5999)).toBe(true)
    expect(isAllowedMarketKind(6000)).toBe(true)
    expect(isAllowedMarketKind(6999)).toBe(true)
    expect(isAllowedMarketKind(7000)).toBe(true)
    expect(isAllowedMarketKind(30404)).toBe(true)
    expect(isAllowedMarketKind(30406)).toBe(true)
    expect(isAllowedMarketKind(31989)).toBe(true)
    expect(isAllowedMarketKind(31990)).toBe(true)

    expect(isAllowedMarketKind(1)).toBe(false)
    expect(isAllowedMarketKind(4999)).toBe(false)
    expect(isAllowedMarketKind(60000)).toBe(false)
  })

  test("classifies health metrics buckets", () => {
    expect(marketKindBucket(5050)).toBe("nip90_request")
    expect(marketKindBucket(6050)).toBe("nip90_result")
    expect(marketKindBucket(7000)).toBe("nip90_feedback")
    expect(marketKindBucket(30404)).toBe("nip_ds")
    expect(marketKindBucket(31989)).toBe("nip89_handler")
    expect(marketKindBucket(1)).toBeNull()
  })

  test("allows NIP-LBR labor transport kinds", () => {
    expect(isAllowedMarketKind(5930)).toBe(true)
    expect(isAllowedMarketKind(5934)).toBe(true)
    expect(isAllowedMarketKind(6930)).toBe(true)
    expect(isAllowedMarketKind(6934)).toBe(true)
    expect(isAllowedMarketKind(7000)).toBe(true)

    expect(marketKindBucket(5934)).toBe("nip90_request")
    expect(marketKindBucket(6934)).toBe("nip90_result")
    expect(marketKindBucket(7000)).toBe("nip90_feedback")
    expect(
      validateReqFilters([
        { kinds: [5930, 5934, 6930, 6934, 7000], limit: 10 },
      ]),
    ).toBeNull()
  })

  test("rejects oversized REQ filters", () => {
    expect(validateReqFilters([])).toContain("at least one filter")
    expect(
      validateReqFilters(
        Array.from({ length: MarketRelayPolicy.maxFiltersPerReq + 1 }, () => ({
          kinds: [5050],
        })),
      ),
    ).toContain("filters")
    expect(validateReqFilters([{ kinds: [5050], limit: 101 }])).toContain(
      "limit",
    )
    expect(
      validateReqFilters([
        {
          kinds: [5050],
          authors: Array.from(
            { length: MarketRelayPolicy.maxAuthorsPerFilter + 1 },
            (_, index) => String(index).padStart(64, "a"),
          ),
        },
      ]),
    ).toContain("authors")
  })

  test("rejects disallowed kind filters", () => {
    // Kind 9999 is neither a market kind nor a general coordination kind (#5537),
    // so it is still rejected. Kind 1 is now a subscribable general kind and is
    // covered by general-policy.test.ts.
    expect(validateReqFilters([{ kinds: [5050, 9999], limit: 10 }])).toContain(
      "kind 9999",
    )
    expect(validateReqFilters([{ kinds: [5050], limit: 10 }])).toBeNull()
  })

  test("enforces per-pubkey publish buckets", () => {
    let bucket = undefined

    for (
      let index = 0;
      index < MarketRelayPolicy.publishRateLimitMaxEvents;
      index++
    ) {
      const result = nextPublishBucket(bucket, 1_000)
      expect(result.allowed).toBe(true)
      bucket = result.bucket
    }

    const blocked = nextPublishBucket(bucket, 1_000)
    expect(blocked.allowed).toBe(false)

    const reset = nextPublishBucket(
      blocked.bucket,
      1_000 + MarketRelayPolicy.publishRateLimitWindowMs,
    )
    expect(reset.allowed).toBe(true)
    expect(reset.bucket.count).toBe(1)
  })
})
