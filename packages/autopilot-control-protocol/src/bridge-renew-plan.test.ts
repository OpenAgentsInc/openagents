import { describe, expect, test } from "bun:test"

import { planBridgeRenewal } from "./bridge-renew-plan.js"

const nowIso = "2026-06-13T12:00:00.000Z"

describe("bridge renewal planner", () => {
  test("repairs credentials with no expiry so the bridge can re-pair", () => {
    expect(planBridgeRenewal({ expiresAt: null, nowIso })).toEqual({
      action: "repair",
      reason: "credential_missing_expiry",
    })
  })

  test("repairs expired credentials so the bridge can re-pair", () => {
    expect(planBridgeRenewal({
      expiresAt: "2026-06-13T11:59:59.999Z",
      nowIso,
    })).toEqual({
      action: "repair",
      reason: "credential_expired_repair_pairing",
    })
  })

  test("renews credentials expiring exactly now", () => {
    expect(planBridgeRenewal({ expiresAt: nowIso, nowIso })).toEqual({
      action: "renew",
      reason: "credential_expiring_within_24h",
    })
  })

  test("renews credentials expiring inside the renewal window", () => {
    expect(planBridgeRenewal({
      expiresAt: "2026-06-14T11:59:59.999Z",
      nowIso,
    })).toEqual({
      action: "renew",
      reason: "credential_expiring_within_24h",
    })
  })

  test("keeps credentials valid outside the renewal window", () => {
    expect(planBridgeRenewal({
      expiresAt: "2026-06-14T12:00:00.001Z",
      nowIso,
    })).toEqual({
      action: "none",
      reason: "credential_valid",
    })
  })

  test("repairs malformed dates instead of silently treating them as valid", () => {
    expect(planBridgeRenewal({
      expiresAt: "not-an-iso-date",
      nowIso,
    })).toEqual({
      action: "repair",
      reason: "credential_expiry_invalid",
    })
  })
})
