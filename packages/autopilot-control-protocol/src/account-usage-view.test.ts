import { describe, expect, test } from "bun:test"

import { projectAccountUsage } from "./account-usage-view.js"

describe("account usage view projection", () => {
  test("projects per-account usage from camelCase fields", () => {
    expect(projectAccountUsage([
      {
        provider: "openai",
        usedPercent: 42.5,
        resetAt: "2026-06-14T00:00:00Z",
        limited: false,
      },
    ])).toEqual([
      {
        provider: "openai",
        usedPercent: 42.5,
        resetAt: "2026-06-14T00:00:00Z",
        limited: false,
        blockers: [],
      },
    ])
  })

  test("projects nested account usage and snake_case aliases", () => {
    expect(projectAccountUsage({
      account_usage: [
        {
          provider_id: "anthropic",
          usage_percent: "75",
          quota_reset_at: "2026-06-13T18:00:00Z",
          rate_limited: true,
          blockers: ["quota_receipt_pending", "quota_receipt_pending"],
        },
      ],
    })).toEqual([
      {
        provider: "anthropic",
        usedPercent: 75,
        resetAt: "2026-06-13T18:00:00Z",
        limited: true,
        blockers: ["quota_receipt_pending"],
      },
    ])
  })

  test("derives used percent from usage and limit", () => {
    expect(projectAccountUsage([
      {
        account_provider: "local",
        requests_used: 25,
        requests_limit: 100,
        status: "ready",
      },
    ])).toEqual([
      {
        provider: "local",
        usedPercent: 25,
        resetAt: null,
        limited: false,
        blockers: [],
      },
    ])
  })

  test("caps exhausted usage and infers limited state", () => {
    expect(projectAccountUsage([
      {
        name: "cloud",
        used: 120,
        limit: 100,
      },
    ])).toEqual([
      {
        provider: "cloud",
        usedPercent: 100,
        resetAt: null,
        limited: true,
        blockers: [],
      },
    ])
  })

  test("returns an empty read-only projection for unavailable payloads", () => {
    expect(projectAccountUsage(null)).toEqual([])
    expect(projectAccountUsage({ accounts: "missing" })).toEqual([])
  })

  test("skips non-record rows and marks invalid fields with blockers", () => {
    expect(projectAccountUsage([
      "bad",
      {
        provider: "",
        used_percent: -1,
        reset_at: 123,
      },
    ])).toEqual([
      {
        provider: "unknown",
        usedPercent: null,
        resetAt: null,
        limited: false,
        blockers: [
          "provider_unknown",
          "used_percent_invalid",
          "reset_at_invalid",
          "limited_status_unknown",
        ],
      },
    ])
  })

  test("does not mutate source blockers", () => {
    const blockers = ["source_blocker"]
    const projected = projectAccountUsage([
      {
        provider: "openai",
        usedPercent: 5,
        blockers,
      },
    ])

    projected[0]?.blockers.push("view_only")

    expect(blockers).toEqual(["source_blocker"])
  })
})
