import { describe, expect, test } from "bun:test"

import { mapPylonFleetSupervisorCapacity } from "../src/orchestration/fleet-run-capacity.js"

describe("mapPylonFleetSupervisorCapacity", () => {
  test("preserves mixed Codex and Claude kinds, available slots, and declared cost classes", () => {
    expect(mapPylonFleetSupervisorCapacity([
      {
        accountRef: "codex-owner",
        capacity: { available: 2, ready: 3 },
        marginalCostClass: "subscription",
        paused: false,
        provider: "codex",
        readiness: "ready",
      },
      {
        accountRef: "claude-owner",
        capacity: { available: 1, ready: 4 },
        marginalCostClass: "free",
        paused: false,
        provider: "claude_agent",
        readiness: "available",
      },
    ])).toEqual([
      {
        accountRef: "codex-owner",
        advertisedCapacity: 2,
        marginalCostClass: "subscription",
        workerKind: "codex",
      },
      {
        accountRef: "claude-owner",
        advertisedCapacity: 1,
        marginalCostClass: "free",
        workerKind: "claude",
      },
    ])
  })

  test("filters paused, unready, and disallowed default accounts without inventing cost", () => {
    expect(mapPylonFleetSupervisorCapacity([
      {
        accountRef: "default",
        capacity: { available: 9, ready: 9 },
        isDefaultAccount: true,
        marginalCostClass: "free",
        paused: false,
        provider: "codex",
        readiness: "ready",
      },
      {
        accountRef: "codex-paused",
        capacity: { available: 1, ready: 1 },
        marginalCostClass: "subscription",
        paused: true,
        provider: "codex",
        readiness: "ready",
      },
      {
        accountRef: "claude-missing",
        capacity: { available: 1, ready: 1 },
        marginalCostClass: "subscription",
        paused: false,
        provider: "claude_agent",
        readiness: "credentials_missing",
      },
      {
        accountRef: "claude-owner",
        capacity: { available: null, ready: 2 },
        marginalCostClass: "definitely_free",
        paused: false,
        provider: "claude_agent",
        readiness: "ready",
      },
    ], { allowDefaultAccount: false })).toEqual([{
      accountRef: "claude-owner",
      advertisedCapacity: 2,
      marginalCostClass: "not_measured",
      workerKind: "claude",
    }])
  })

  test("does not silently map Grok or another unsupported provider onto Codex", () => {
    expect(mapPylonFleetSupervisorCapacity([
      {
        accountRef: "grok-owner",
        capacity: { available: 3, ready: 3 },
        marginalCostClass: "api_metered",
        paused: false,
        provider: "grok",
        readiness: "ready",
      },
      {
        accountRef: "unknown-owner",
        capacity: { available: 3, ready: 3 },
        paused: false,
        provider: "unknown",
        readiness: "ready",
      },
    ])).toEqual([])
  })

  test("fails unknown capacity closed to zero instead of inventing one slot", () => {
    expect(mapPylonFleetSupervisorCapacity([{
      accountRef: "codex-unknown-capacity",
      capacity: null,
      marginalCostClass: "subscription",
      paused: false,
      provider: "codex",
      readiness: "ready",
    }])).toEqual([{
      accountRef: "codex-unknown-capacity",
      advertisedCapacity: 0,
      marginalCostClass: "subscription",
      workerKind: "codex",
    }])
  })
})
