import { describe, expect, test } from "bun:test"

import {
  decideShipAuthorization,
  REBUILD_SPEND_ESCALATION_THRESHOLD_USD,
} from "../src/coordinator/loop-safety"
import { buildShipReceipt } from "../src/coordinator/ship-receipt"

describe("Pylon coordinator loop safety", () => {
  test("auto-authorizes no ship action", () => {
    expect(
      decideShipAuthorization({
        shipMode: "none",
        targetsLiveDevice: false,
      }),
    ).toMatchObject({
      decision: "auto",
      reason: expect.stringContaining("No ship action"),
    })
  })

  test("auto-authorizes OTA pushes to live devices without spend", () => {
    const authorization = decideShipAuthorization({
      shipMode: "ota",
      targetsLiveDevice: true,
    })

    expect(authorization.decision).toBe("auto")
    expect(authorization.reason).toContain("live device")
    expect(authorization.reason).toContain("does not create new spend")
  })

  test("auto-authorizes OTA pushes that do not target live devices", () => {
    expect(
      decideShipAuthorization({
        shipMode: "ota",
        targetsLiveDevice: false,
      }).decision,
    ).toBe("auto")
  })

  test("escalates rebuilds as cost-bearing EAS builds", () => {
    const authorization = decideShipAuthorization({
      shipMode: "rebuild",
      targetsLiveDevice: false,
    })

    expect(authorization.decision).toBe("escalate")
    expect(authorization.reason).toContain("cost-bearing EAS build")
    expect(authorization.reason).toContain("spend-enable")
  })

  test("escalates rebuild spend above the named autonomous threshold", () => {
    const authorization = decideShipAuthorization({
      shipMode: "rebuild",
      estimatedCostUsd: REBUILD_SPEND_ESCALATION_THRESHOLD_USD + 0.01,
      targetsLiveDevice: true,
    })

    expect(authorization.decision).toBe("escalate")
    expect(authorization.reason).toContain("above the autonomous threshold")
  })

  test("builds refs-only ship receipts with required fields", () => {
    const receipt = buildShipReceipt({
      intentId: "intent.public.pylon.ship.4947",
      shipMode: "ota",
      decision: "auto",
      artifactRef: "artifact.public.pylon.ota.fixture",
      updateId: "update.public.pylon.ota.fixture",
      summary: "OTA receipt projected with public refs only.",
      token: "secret-token",
      rawPrivateContent: "/Users/private/device-log",
    } as Parameters<typeof buildShipReceipt>[0] & {
      token: string
      rawPrivateContent: string
    })

    expect(receipt).toEqual({
      intentId: "intent.public.pylon.ship.4947",
      shipMode: "ota",
      decision: "auto",
      artifactRef: "artifact.public.pylon.ota.fixture",
      updateId: "update.public.pylon.ota.fixture",
      summary: "OTA receipt projected with public refs only.",
    })
    expect(Object.keys(receipt).sort()).toEqual([
      "artifactRef",
      "decision",
      "intentId",
      "shipMode",
      "summary",
      "updateId",
    ])
    expect(JSON.stringify(receipt)).not.toContain("secret-token")
    expect(JSON.stringify(receipt)).not.toContain("/Users/private")
  })
})
