import { describe, expect, test } from "bun:test"

import { computeCloudQuota, projectCloudQuota } from "./cloud-quota-view.js"

describe("cloud quota view projection", () => {
  test("projects camelCase quota fields with derived remaining and percent", () => {
    expect(projectCloudQuota({
      usedSats: 250,
      capSats: 1000,
      failoverState: "primary",
      spendAuthority: true,
    })).toEqual({
      usedSats: 250,
      capSats: 1000,
      remainingSats: 750,
      percentUsed: 25,
      failoverState: "primary",
      blockers: [],
    })
  })

  test("projects nested snake_case quota fields and source blockers", () => {
    expect(projectCloudQuota({
      cloud_quota: {
        used_sats: "300",
        cap_sats: "1200",
        failover_state: "fallback",
        blockers: ["quota_receipt_pending"],
      },
    })).toEqual({
      usedSats: 300,
      capSats: 1200,
      remainingSats: 900,
      percentUsed: 25,
      failoverState: "failover",
      blockers: ["quota_receipt_pending"],
    })
  })

  test("accepts msat aliases", () => {
    expect(projectCloudQuota({
      quota: {
        usage_msats: "1234000",
        limitMsats: 5000000,
        mode: "healthy",
      },
    })).toEqual({
      usedSats: 1234,
      capSats: 5000,
      remainingSats: 3766,
      percentUsed: 24.68,
      failoverState: "primary",
      blockers: [],
    })
  })

  test("returns a closed read-only projection for bad input", () => {
    expect(projectCloudQuota(null)).toEqual({
      usedSats: null,
      capSats: null,
      remainingSats: null,
      percentUsed: null,
      failoverState: "unknown",
      blockers: ["cloud_quota_payload_unavailable"],
    })
    expect(projectCloudQuota(["not", "an", "object"])).toEqual({
      usedSats: null,
      capSats: null,
      remainingSats: null,
      percentUsed: null,
      failoverState: "unknown",
      blockers: ["cloud_quota_payload_unavailable"],
    })
  })

  test("keeps unknown fields nullable with blockers", () => {
    expect(projectCloudQuota({})).toEqual({
      usedSats: null,
      capSats: null,
      remainingSats: null,
      percentUsed: null,
      failoverState: "unknown",
      blockers: [
        "used_sats_unknown",
        "cap_sats_unknown",
        "failover_state_unknown",
      ],
    })
  })

  test("defensively rejects invalid numeric fields", () => {
    expect(projectCloudQuota({
      usedSats: -1,
      capSats: 1.5,
      failoverState: "degraded",
    })).toEqual({
      usedSats: null,
      capSats: null,
      remainingSats: null,
      percentUsed: null,
      failoverState: "failover",
      blockers: [
        "used_sats_invalid",
        "cap_sats_invalid",
      ],
    })
  })

  test("caps derived values at exhausted quota", () => {
    expect(projectCloudQuota({
      spent_sats: 1500,
      budget_sats: 1000,
      failover_state: true,
    })).toEqual({
      usedSats: 1500,
      capSats: 1000,
      remainingSats: 0,
      percentUsed: 100,
      failoverState: "failover",
      blockers: [],
    })
  })

  test("pure helper returns null derived values without a usable cap", () => {
    expect(computeCloudQuota(100, 0)).toEqual({
      remainingSats: null,
      percentUsed: null,
    })
    expect(computeCloudQuota(null, 100)).toEqual({
      remainingSats: null,
      percentUsed: null,
    })
  })
})
