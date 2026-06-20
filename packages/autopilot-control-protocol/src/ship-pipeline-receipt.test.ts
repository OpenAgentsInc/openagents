import { describe, expect, test } from "bun:test"

import { buildPipelineReceipt, validate } from "./ship-pipeline-receipt.js"

describe("ship pipeline receipt", () => {
  test("builds an allowed OTA pipeline receipt", () => {
    expect(buildPipelineReceipt({
      action: "ota",
      version: "v1.2.3",
      spendDecision: "allow",
      ranAt: "2026-06-13T12:00:00.000Z",
    })).toEqual({
      kind: "ship_pipeline_receipt",
      action: "ota",
      version: "v1.2.3",
      allowed: true,
      ranAt: "2026-06-13T12:00:00.000Z",
      line: "OTA v1.2.3 pipeline allowed at 2026-06-13T12:00:00.000Z.",
    })
  })

  test("builds a denied rebuild pipeline receipt", () => {
    expect(buildPipelineReceipt({
      action: "rebuild",
      version: "v2.0.0",
      spendDecision: "deny",
      ranAt: "2026-06-13T12:05:00.000Z",
    })).toEqual({
      kind: "ship_pipeline_receipt",
      action: "rebuild",
      version: "v2.0.0",
      allowed: false,
      ranAt: "2026-06-13T12:05:00.000Z",
      line: "Rebuild v2.0.0 pipeline denied at 2026-06-13T12:05:00.000Z.",
    })
  })

  test("builds a noop pipeline receipt", () => {
    expect(buildPipelineReceipt({
      action: "noop",
      version: "v2.0.1",
      spendDecision: "allow",
      ranAt: "2026-06-13T12:10:00.000Z",
    })).toEqual({
      kind: "ship_pipeline_receipt",
      action: "noop",
      version: "v2.0.1",
      allowed: true,
      ranAt: "2026-06-13T12:10:00.000Z",
      line: "Noop v2.0.1 pipeline allowed at 2026-06-13T12:10:00.000Z.",
    })
  })

  test("validates a built receipt", () => {
    const receipt = buildPipelineReceipt({
      action: "ota",
      version: "v1.2.4",
      spendDecision: "allow",
      ranAt: "2026-06-13T12:15:00.000Z",
    })

    expect(validate(receipt)).toBe(true)
  })

  test("rejects non-receipt payloads", () => {
    expect(validate(null)).toBe(false)
    expect(validate(["ship_pipeline_receipt"])).toBe(false)
    expect(validate({ kind: "ship_pipeline_status" })).toBe(false)
  })

  test("rejects invalid action fields", () => {
    const receipt = buildPipelineReceipt({
      action: "rebuild",
      version: "v2.0.2",
      spendDecision: "deny",
      ranAt: "2026-06-13T12:20:00.000Z",
    })

    expect(validate({ ...receipt, action: "native" })).toBe(false)
  })

  test("rejects malformed scalar fields", () => {
    const receipt = buildPipelineReceipt({
      action: "noop",
      version: "v2.0.3",
      spendDecision: "allow",
      ranAt: "2026-06-13T12:25:00.000Z",
    })

    expect(validate({ ...receipt, version: 203 })).toBe(false)
    expect(validate({ ...receipt, allowed: "true" })).toBe(false)
    expect(validate({ ...receipt, ranAt: 1781353500000 })).toBe(false)
    expect(validate({ ...receipt, line: null })).toBe(false)
  })

  test("rejects a receipt with a mismatched line", () => {
    const receipt = buildPipelineReceipt({
      action: "ota",
      version: "v1.2.5",
      spendDecision: "deny",
      ranAt: "2026-06-13T12:30:00.000Z",
    })

    expect(validate({
      ...receipt,
      line: "OTA v1.2.5 pipeline allowed at 2026-06-13T12:30:00.000Z.",
    })).toBe(false)
  })
})
