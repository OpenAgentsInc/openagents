import { describe, expect, test } from "bun:test"

import { projectAccountRegistryDetail } from "./account-registry-detail-view.js"

describe("account registry detail view projection", () => {
  test("returns an empty read-only projection for unavailable payloads", () => {
    expect(projectAccountRegistryDetail(null)).toEqual({
      accounts: [],
      readyCount: 0,
      exhaustedCount: 0,
      total: 0,
    })
    expect(projectAccountRegistryDetail({ accounts: "missing" })).toEqual({
      accounts: [],
      readyCount: 0,
      exhaustedCount: 0,
      total: 0,
    })
  })

  test("projects account rows from top-level array input", () => {
    expect(projectAccountRegistryDetail([
      {
        provider: "openai",
        accountRef: "acct_hash_123",
        ready: true,
        homeState: "us-east",
        capacity: { usedPct: 42, limited: false },
        blockerRefs: ["receipt_hash_1"],
      },
    ])).toEqual({
      accounts: [
        {
          provider: "openai",
          identityLabel: "acct_hash_123",
          ready: true,
          exhausted: false,
          homeState: "us-east",
          capacity: { usedPct: 42, limited: false },
          blockerRefs: ["receipt_hash_1"],
        },
      ],
      readyCount: 1,
      exhaustedCount: 0,
      total: 1,
    })
  })

  test("projects nested registry payloads with snake_case aliases", () => {
    expect(projectAccountRegistryDetail({
      account_registry: [
        {
          provider_id: "anthropic",
          account_ref: "acct_ref_abc",
          is_ready: true,
          home_state: "eu",
          used_pct: "75.5",
          rate_limited: false,
          blocker_refs: ["ref_a", "ref_a", "ref_b"],
        },
      ],
    })).toEqual({
      accounts: [
        {
          provider: "anthropic",
          identityLabel: "acct_ref_abc",
          ready: true,
          exhausted: false,
          homeState: "eu",
          capacity: { usedPct: 75.5, limited: false },
          blockerRefs: ["ref_a", "ref_b"],
        },
      ],
      readyCount: 1,
      exhaustedCount: 0,
      total: 1,
    })
  })

  test("detects exhausted accounts from blocker refs", () => {
    expect(projectAccountRegistryDetail([
      {
        provider: "openai",
        ref: "acct_safe",
        ready: true,
        home: "primary",
        blockers: ["daily_quota_ref"],
      },
      {
        provider: "local",
        ref: "acct_local",
        ready: true,
        home: "local",
        blockers: ["policy_ref"],
      },
    ])).toEqual({
      accounts: [
        {
          provider: "openai",
          identityLabel: "acct_safe",
          ready: true,
          exhausted: true,
          homeState: "primary",
          capacity: null,
          blockerRefs: ["daily_quota_ref"],
        },
        {
          provider: "local",
          identityLabel: "acct_local",
          ready: true,
          exhausted: false,
          homeState: "local",
          capacity: null,
          blockerRefs: ["policy_ref"],
        },
      ],
      readyCount: 2,
      exhaustedCount: 1,
      total: 2,
    })
  })

  test("detects exhausted accounts from limited readiness state", () => {
    expect(projectAccountRegistryDetail([
      {
        provider: "cloud",
        accountRef: "acct_cloud",
        status: "rate_limited",
        usagePercent: 100,
      },
    ])).toEqual({
      accounts: [
        {
          provider: "cloud",
          identityLabel: "acct_cloud",
          ready: false,
          exhausted: true,
          homeState: "unknown",
          capacity: { usedPct: 100, limited: true },
          blockerRefs: [],
        },
      ],
      readyCount: 0,
      exhaustedCount: 1,
      total: 1,
    })
  })

  test("uses email or home for identity labels without reading token fields", () => {
    expect(projectAccountRegistryDetail([
      {
        provider: "email-provider",
        emailHash: "email_hash_123",
        token: "secret-token",
        seed: "secret-seed",
        ready: true,
        home: "home_ref",
      },
      {
        provider: "home-provider",
        token: "another-secret",
        ready: false,
        home: "home_only",
      },
    ])).toEqual({
      accounts: [
        {
          provider: "email-provider",
          identityLabel: "email_hash_123",
          ready: true,
          exhausted: false,
          homeState: "home_ref",
          capacity: null,
          blockerRefs: [],
        },
        {
          provider: "home-provider",
          identityLabel: "home_only",
          ready: false,
          exhausted: false,
          homeState: "home_only",
          capacity: null,
          blockerRefs: [],
        },
      ],
      readyCount: 1,
      exhaustedCount: 0,
      total: 2,
    })
  })

  test("skips invalid rows and falls back to public-safe unknown labels", () => {
    expect(projectAccountRegistryDetail({
      accounts: [
        "bad",
        {
          provider: "",
          ready: "yes",
          usedPct: -1,
          blockerRefs: [123, " limit_ref "],
        },
      ],
    })).toEqual({
      accounts: [
        {
          provider: "unknown",
          identityLabel: "unknown",
          ready: false,
          exhausted: true,
          homeState: "unknown",
          capacity: { usedPct: null, limited: false },
          blockerRefs: ["limit_ref"],
        },
      ],
      readyCount: 0,
      exhaustedCount: 1,
      total: 1,
    })
  })
})
