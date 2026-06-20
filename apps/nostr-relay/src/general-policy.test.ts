import { describe, expect, test } from "bun:test"

import {
  GENERAL_COORDINATION_KINDS,
  GeneralRelayPolicy,
  authorizeGeneralWrite,
  generalKindBucket,
  isGeneralCoordinationKind,
  nextGeneralPublishBucket,
  parseAuthorizedPubkeys,
  validateNip42AuthClaims,
} from "./general-policy"
import { isAllowedMarketKind, validateReqFilters } from "./market-policy"

const HEX_A = "a".repeat(64)
const HEX_B = "b".repeat(64)

describe("general coordination kind classification", () => {
  test("recognizes the added coordination/discovery kinds", () => {
    expect(isGeneralCoordinationKind(1)).toBe(true) // NIP-01 text note
    expect(isGeneralCoordinationKind(3)).toBe(true) // NIP-02 contacts
    expect(isGeneralCoordinationKind(13)).toBe(true) // NIP-17 seal
    expect(isGeneralCoordinationKind(14)).toBe(true) // NIP-17 rumor
    expect(isGeneralCoordinationKind(1059)).toBe(true) // NIP-59 gift wrap
    expect(isGeneralCoordinationKind(10002)).toBe(true) // NIP-65 relay list
    expect(isGeneralCoordinationKind(30315)).toBe(true) // NIP-38 status
    for (let kind = 40; kind <= 44; kind++) {
      expect(isGeneralCoordinationKind(kind)).toBe(true) // NIP-28 public chat
    }

    expect(generalKindBucket(1)).toBe("nip01_text_note")
    expect(generalKindBucket(30315)).toBe("nip38_status")
    expect(generalKindBucket(42)).toBe("nip28_channel")
  })

  test("does not classify market or unknown kinds as general", () => {
    expect(isGeneralCoordinationKind(5050)).toBe(false)
    expect(isGeneralCoordinationKind(6050)).toBe(false)
    expect(isGeneralCoordinationKind(7000)).toBe(false)
    expect(isGeneralCoordinationKind(30404)).toBe(false)
    expect(isGeneralCoordinationKind(31989)).toBe(false)
    expect(isGeneralCoordinationKind(9999)).toBe(false)
    expect(generalKindBucket(5050)).toBeNull()
  })

  test("general and market kind sets are disjoint", () => {
    for (const kind of GENERAL_COORDINATION_KINDS) {
      expect(isAllowedMarketKind(kind)).toBe(false)
    }
  })
})

describe("REQ subscribes general kinds without auth (read-only)", () => {
  test("allows market and general kinds in REQ filters", () => {
    expect(validateReqFilters([{ kinds: [1, 3, 30315, 10002], limit: 10 }])).toBeNull()
    expect(validateReqFilters([{ kinds: [5050, 1059], limit: 10 }])).toBeNull()
  })

  test("still rejects truly unknown kinds in REQ filters", () => {
    expect(validateReqFilters([{ kinds: [1, 9999], limit: 10 }])).toContain(
      "kind 9999",
    )
  })
})

describe("provisioned-pubkey allowlist parsing", () => {
  test("parses comma/whitespace-separated hex pubkeys, lowercased", () => {
    const set = parseAuthorizedPubkeys(`${HEX_A.toUpperCase()}, ${HEX_B}`)
    expect(set.has(HEX_A)).toBe(true)
    expect(set.has(HEX_B)).toBe(true)
    expect(set.size).toBe(2)
  })

  test("drops malformed entries and empties", () => {
    expect(parseAuthorizedPubkeys(undefined).size).toBe(0)
    expect(parseAuthorizedPubkeys("").size).toBe(0)
    expect(parseAuthorizedPubkeys("not-a-key, deadbeef").size).toBe(0)
  })
})

describe("general write authorization (anti-abuse gate)", () => {
  test("ALLOW: allowlisted pubkey may write general kinds", () => {
    const result = authorizeGeneralWrite({
      kind: 30315,
      pubkey: HEX_A,
      allowlist: new Set([HEX_A]),
      authenticatedPubkey: null,
    })
    expect(result.allowed).toBe(true)
  })

  test("ALLOW: NIP-42-authenticated pubkey may write general kinds", () => {
    const result = authorizeGeneralWrite({
      kind: 30315,
      pubkey: HEX_A,
      allowlist: new Set<string>(),
      authenticatedPubkey: HEX_A,
    })
    expect(result.allowed).toBe(true)
  })

  test("ALLOW: gift wrap (1059) on any authenticated connection (ephemeral wire key)", () => {
    const result = authorizeGeneralWrite({
      kind: 1059,
      pubkey: HEX_A, // throwaway NIP-59 wire key, not the auth identity
      allowlist: new Set<string>(),
      authenticatedPubkey: HEX_B,
    })
    expect(result.allowed).toBe(true)
  })

  test("REJECT: gift wrap (1059) on an UNauthenticated connection is blocked", () => {
    const result = authorizeGeneralWrite({
      kind: 1059,
      pubkey: HEX_A,
      allowlist: new Set<string>(),
      authenticatedPubkey: null,
    })
    expect(result.allowed).toBe(false)
  })

  test("REJECT: unauthenticated, non-allowlisted pubkey is blocked", () => {
    const result = authorizeGeneralWrite({
      kind: 30315,
      pubkey: HEX_A,
      allowlist: new Set([HEX_B]),
      authenticatedPubkey: null,
    })
    expect(result.allowed).toBe(false)
    expect(result.reason).toContain("auth-required")
  })

  test("REJECT: non-giftwrap authenticated as a DIFFERENT pubkey does not authorize", () => {
    const result = authorizeGeneralWrite({
      kind: 30315,
      pubkey: HEX_A,
      allowlist: new Set<string>(),
      authenticatedPubkey: HEX_B,
    })
    expect(result.allowed).toBe(false)
  })
})

describe("general per-pubkey publish rate limit", () => {
  test("enforces the tighter general publish budget", () => {
    let bucket: ReturnType<typeof nextGeneralPublishBucket>["bucket"] | undefined =
      undefined
    for (let i = 0; i < GeneralRelayPolicy.publishRateLimitMaxEvents; i++) {
      const result = nextGeneralPublishBucket(bucket, 1_000)
      expect(result.allowed).toBe(true)
      bucket = result.bucket
    }
    const blocked = nextGeneralPublishBucket(bucket, 1_000)
    expect(blocked.allowed).toBe(false)

    const reset = nextGeneralPublishBucket(
      blocked.bucket,
      1_000 + GeneralRelayPolicy.publishRateLimitWindowMs,
    )
    expect(reset.allowed).toBe(true)
    expect(reset.bucket.count).toBe(1)
  })
})

describe("NIP-42 AUTH claim validation", () => {
  const baseEvent = {
    id: "f".repeat(64),
    pubkey: HEX_A,
    kind: 22242,
    created_at: 1_000_000,
    sig: "0".repeat(128),
    content: "",
    tags: [
      ["relay", "wss://relay.openagents.com"],
      ["challenge", "the-challenge"],
    ],
  }

  test("ALLOW: well-formed auth event with matching challenge + relay", () => {
    const result = validateNip42AuthClaims({
      event: baseEvent,
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.pubkey).toBe(HEX_A)
  })

  test("ALLOW: trailing-slash relay tag still matches", () => {
    const result = validateNip42AuthClaims({
      event: { ...baseEvent, tags: [["relay", "wss://relay.openagents.com/"], ["challenge", "the-challenge"]] },
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(true)
  })

  test("ALLOW: ws scheme relay tag matches a wss relay (same host)", () => {
    const result = validateNip42AuthClaims({
      event: { ...baseEvent, tags: [["relay", "ws://relay.openagents.com"], ["challenge", "the-challenge"]] },
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(true)
  })

  test("ALLOW: matches any of several relay hostnames", () => {
    const result = validateNip42AuthClaims({
      event: { ...baseEvent, tags: [["relay", "wss://nexus.openagents.com"], ["challenge", "the-challenge"]] },
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com", "wss://nexus.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(true)
  })

  test("REJECT: wrong challenge", () => {
    const result = validateNip42AuthClaims({
      event: baseEvent,
      expectedChallenge: "different-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toContain("challenge")
  })

  test("REJECT: relay tag for a different relay", () => {
    const result = validateNip42AuthClaims({
      event: { ...baseEvent, tags: [["relay", "wss://evil.example.com"], ["challenge", "the-challenge"]] },
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toContain("relay")
  })

  test("REJECT: wrong kind", () => {
    const result = validateNip42AuthClaims({
      event: { ...baseEvent, kind: 1 },
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000,
    })
    expect(result.ok).toBe(false)
  })

  test("REJECT: stale auth event", () => {
    const result = validateNip42AuthClaims({
      event: baseEvent,
      expectedChallenge: "the-challenge",
      relayUrls: ["wss://relay.openagents.com"],
      nowSeconds: 1_000_000 + 5_000,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toContain("stale")
  })
})
