import { describe, expect, test } from "bun:test"

import {
  MarketRelayPolicy,
  isAllowedMarketKind,
  isParameterizedReplaceableMarketKind,
  marketKindBucket,
  nextPublishBucket,
  relayInformationDocument,
  validateReqFilters,
} from "./market-policy"

describe("market relay policy", () => {
  test("allows scoped market and OpenAgents coordination event kinds", () => {
    expect(isAllowedMarketKind(1)).toBe(true)
    expect(isAllowedMarketKind(3)).toBe(true)
    expect(isAllowedMarketKind(13)).toBe(true)
    expect(isAllowedMarketKind(14)).toBe(true)
    expect(isAllowedMarketKind(1059)).toBe(true)
    expect(isAllowedMarketKind(10002)).toBe(true)
    expect(isAllowedMarketKind(30315)).toBe(true)
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

    expect(isAllowedMarketKind(4999)).toBe(false)
    expect(isAllowedMarketKind(60000)).toBe(false)
  })

  test("classifies health metrics buckets", () => {
    expect(marketKindBucket(1)).toBe("nip01_text_note")
    expect(marketKindBucket(3)).toBe("nip02_contacts")
    expect(marketKindBucket(13)).toBe("nip17_private_dm")
    expect(marketKindBucket(14)).toBe("nip17_private_dm")
    expect(marketKindBucket(1059)).toBe("nip17_private_dm")
    expect(marketKindBucket(10002)).toBe("nip65_relay_list")
    expect(marketKindBucket(30315)).toBe("nip38_status")
    expect(marketKindBucket(5050)).toBe("nip90_request")
    expect(marketKindBucket(6050)).toBe("nip90_result")
    expect(marketKindBucket(7000)).toBe("nip90_feedback")
    expect(marketKindBucket(30404)).toBe("nip_ds")
    expect(marketKindBucket(31989)).toBe("nip89_handler")
    expect(marketKindBucket(60000)).toBeNull()
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
    expect(validateReqFilters([{ kinds: [5050, 60000], limit: 10 }])).toContain(
      "kind 60000",
    )
    expect(
      validateReqFilters([{ kinds: [5050, 1, 3, 10002], limit: 10 }]),
    ).toBeNull()
  })

  test("identifies parameterized replaceable coordination and market kinds", () => {
    expect(isParameterizedReplaceableMarketKind(30315)).toBe(true)
    expect(isParameterizedReplaceableMarketKind(30404)).toBe(true)
    expect(isParameterizedReplaceableMarketKind(30406)).toBe(true)
    expect(isParameterizedReplaceableMarketKind(31989)).toBe(true)
    expect(isParameterizedReplaceableMarketKind(31990)).toBe(true)
    expect(isParameterizedReplaceableMarketKind(10002)).toBe(false)
    expect(isParameterizedReplaceableMarketKind(1)).toBe(false)
  })

  test("advertises expanded coordination scope in NIP-11 metadata", () => {
    expect(relayInformationDocument.supported_nips).toContain(17)
    expect(relayInformationDocument.supported_nips).toContain(38)
    expect(relayInformationDocument.supported_nips).toContain(59)
    expect(relayInformationDocument.supported_nips).toContain(65)
    expect(relayInformationDocument.supported_nips).toContain(90)
    expect(relayInformationDocument.limitation.restricted_writes).toBe(true)
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
