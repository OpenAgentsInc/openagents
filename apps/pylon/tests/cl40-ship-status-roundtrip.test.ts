import { describe, expect, test } from "bun:test"

import { buildShipReceipt } from "../src/coordinator/ship-receipt"
import { projectShipStatusRoundtrip } from "../src/coordinator/ship-status-roundtrip"

describe("CL-40 ship status roundtrip projection", () => {
  test("projects an OTA ship as shipped for the originating intent", () => {
    const receipt = buildShipReceipt({
      intentId: "intent.public.pylon.mobile.4946",
      shipMode: "ota",
      decision: "auto",
      updateId: "update.public.pylon.ota.cl40",
      summary: "OTA update shipped to the phone.",
    })

    expect(
      projectShipStatusRoundtrip({
        intentRef: "intent.mobile.origin.4946",
        receipt,
        updatedAt: "2026-06-13T10:00:00.000Z",
      }),
    ).toEqual({
      intentRef: "intent.mobile.origin.4946",
      shipMode: "ota",
      state: "shipped",
      summary: "OTA update shipped to the phone.",
      receiptRef: "update.public.pylon.ota.cl40",
      updatedAt: "2026-06-13T10:00:00.000Z",
    })
  })

  test("projects a rebuild ship as shipped for the originating intent", () => {
    const receipt = buildShipReceipt({
      intentId: "intent.public.pylon.mobile.4946",
      shipMode: "rebuild",
      decision: "escalate",
      buildId: "build.public.pylon.rebuild.cl40",
      summary: "Native rebuild shipped to the phone.",
    })

    expect(
      projectShipStatusRoundtrip({
        intentRef: "intent.mobile.origin.rebuild.4946",
        receipt,
        updatedAt: "2026-06-13T10:05:00.000Z",
      }),
    ).toEqual({
      intentRef: "intent.mobile.origin.rebuild.4946",
      shipMode: "rebuild",
      state: "shipped",
      summary: "Native rebuild shipped to the phone.",
      receiptRef: "build.public.pylon.rebuild.cl40",
      updatedAt: "2026-06-13T10:05:00.000Z",
    })
  })

  test("projects a failed ship without changing the receipt mode", () => {
    const receipt = buildShipReceipt({
      intentId: "intent.public.pylon.mobile.4946",
      shipMode: "ota",
      decision: "auto",
      artifactRef: "artifact.public.pylon.ota.failed.cl40",
      summary: "OTA publish failed before the phone could update.",
    })

    expect(
      projectShipStatusRoundtrip({
        intentRef: "intent.mobile.origin.failed.4946",
        receipt,
        receiptRef: "receipt.public.pylon.ship.failed.cl40",
        state: "failed",
        summary: "EAS Update rejected the publish.",
        updatedAt: "2026-06-13T10:10:00.000Z",
      }),
    ).toEqual({
      intentRef: "intent.mobile.origin.failed.4946",
      shipMode: "ota",
      state: "failed",
      summary: "EAS Update rejected the publish.",
      receiptRef: "receipt.public.pylon.ship.failed.cl40",
      updatedAt: "2026-06-13T10:10:00.000Z",
    })
  })

  test("re-projects the same input idempotently", () => {
    const input = {
      intentRef: "intent.mobile.origin.idempotent.4946",
      receipt: buildShipReceipt({
        intentId: "intent.public.pylon.mobile.4946",
        shipMode: "ota",
        decision: "auto",
        updateId: "update.public.pylon.ota.idempotent.cl40",
        summary: "OTA update shipped once.",
      }),
      updatedAt: "2026-06-13T10:15:00.000Z",
    } as const

    expect(projectShipStatusRoundtrip(input)).toEqual(
      projectShipStatusRoundtrip(input),
    )
  })

  test("handles missing or unknown intent refs with stable fallbacks", () => {
    expect(
      projectShipStatusRoundtrip({
        intentRef: null,
        receipt: buildShipReceipt({
          intentId: "intent.public.pylon.mobile.receipt-fallback.4946",
          shipMode: "ota",
          decision: "auto",
          summary: "Receipt intent id is available.",
        }),
        updatedAt: "2026-06-13T10:20:00.000Z",
      }).intentRef,
    ).toBe("intent.public.pylon.mobile.receipt-fallback.4946")

    const unknown = projectShipStatusRoundtrip({
      intentRef: " ",
      receipt: buildShipReceipt({
        intentId: "",
        shipMode: "rebuild",
        decision: "escalate",
        summary: "No originating intent ref was recorded.",
      }),
      updatedAt: "2026-06-13T10:25:00.000Z",
    })

    expect(unknown).toMatchObject({
      intentRef: "intent.unknown",
      receiptRef: "ship-receipt:intent.unknown",
    })
  })
})
