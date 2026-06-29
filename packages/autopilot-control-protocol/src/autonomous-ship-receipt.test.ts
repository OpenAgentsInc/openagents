import { describe, expect, test } from "bun:test"

import { buildShipReceipt, validateShipReceipt } from "./autonomous-ship-receipt.js"

describe("autonomous ship receipt", () => {
  test("builds an allowed OTA receipt", () => {
    expect(buildShipReceipt({
      mode: "ota",
      version: "v1.2.3",
      spendDecision: "allow",
      actor: "autopilot",
      shippedAt: "2026-06-13T12:00:00.000Z",
    })).toEqual({
      kind: "ship_receipt",
      mode: "ota",
      version: "v1.2.3",
      allowed: true,
      actor: "autopilot",
      shippedAt: "2026-06-13T12:00:00.000Z",
      line: "OTA v1.2.3 ship allowed by autopilot at 2026-06-13T12:00:00.000Z.",
    })
  })

  test("builds a denied rebuild receipt", () => {
    expect(buildShipReceipt({
      mode: "rebuild",
      version: "v2.0.0",
      spendDecision: "deny",
      actor: "owner",
      shippedAt: "2026-06-13T12:05:00.000Z",
    })).toEqual({
      kind: "ship_receipt",
      mode: "rebuild",
      version: "v2.0.0",
      allowed: false,
      actor: "owner",
      shippedAt: "2026-06-13T12:05:00.000Z",
      line: "Rebuild v2.0.0 ship denied by owner at 2026-06-13T12:05:00.000Z.",
    })
  })

  test("validates a built receipt", () => {
    const receipt = buildShipReceipt({
      mode: "ota",
      version: "v1.2.4",
      spendDecision: "allow",
      actor: "owner",
      shippedAt: "2026-06-13T12:10:00.000Z",
    })

    expect(validateShipReceipt(receipt)).toBe(true)
  })

  test("rejects non-receipt payloads", () => {
    expect(validateShipReceipt(null)).toBe(false)
    expect(validateShipReceipt(["ship_receipt"])).toBe(false)
    expect(validateShipReceipt({ kind: "ship_status" })).toBe(false)
  })

  test("rejects invalid enum fields", () => {
    const receipt = buildShipReceipt({
      mode: "ota",
      version: "v1.2.5",
      spendDecision: "allow",
      actor: "autopilot",
      shippedAt: "2026-06-13T12:15:00.000Z",
    })

    expect(validateShipReceipt({ ...receipt, mode: "native" })).toBe(false)
    expect(validateShipReceipt({ ...receipt, actor: "system" })).toBe(false)
  })

  test("rejects malformed scalar fields", () => {
    const receipt = buildShipReceipt({
      mode: "rebuild",
      version: "v2.0.1",
      spendDecision: "deny",
      actor: "owner",
      shippedAt: "2026-06-13T12:20:00.000Z",
    })

    expect(validateShipReceipt({ ...receipt, version: 201 })).toBe(false)
    expect(validateShipReceipt({ ...receipt, allowed: "false" })).toBe(false)
    expect(validateShipReceipt({ ...receipt, shippedAt: 1781353200000 })).toBe(false)
  })

  test("rejects a receipt with a mismatched line", () => {
    const receipt = buildShipReceipt({
      mode: "ota",
      version: "v1.2.6",
      spendDecision: "deny",
      actor: "autopilot",
      shippedAt: "2026-06-13T12:25:00.000Z",
    })

    expect(validateShipReceipt({
      ...receipt,
      line: "OTA v1.2.6 ship allowed by autopilot at 2026-06-13T12:25:00.000Z.",
    })).toBe(false)
  })
})
