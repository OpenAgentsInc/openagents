// Tests for the INERT, flag-gated multi-earning-node local ledger.
//
// Promise: pylon.v0_3_multi_earning_node.v1
// Blocker:  blocker.product_promises.multi_earning_mode_receipts_missing
//
// This suite is itself the re-runnable receipt: `bun test src/multi-earning-ledger.test.ts`
// proves the ledger distinguishes all five amount classes, only counts SETTLED
// modes toward the >=2 green bar, stays inert (empty) by default, rejects
// leak-prone entries, and produces a fail-closed, self-verifying receipt.
import { describe, expect, it } from "bun:test"
import {
  buildMultiEarningReceiptRef,
  captureMultiEarningReceipt,
  type MultiEarningLedgerEntry,
  type MultiEarningMode,
  MULTI_EARNING_AMOUNT_CLASSES,
  MULTI_EARNING_LEDGER_ENV,
  serializeMultiEarningReceipt,
  summarizeMultiEarning,
  verifyMultiEarningEntry,
  verifyMultiEarningReceipt,
} from "./multi-earning-ledger.js"

const OBSERVED_AT = "2026-06-23T00:00:00.000Z"

function entry(
  overrides: Partial<MultiEarningLedgerEntry> & { mode: MultiEarningMode },
): MultiEarningLedgerEntry {
  return {
    schema: "openagents.pylon.multi_earning_entry.v0.1",
    mode: overrides.mode,
    amountClass: overrides.amountClass ?? "settled",
    amountSats: overrides.amountSats ?? 21,
    receiptRef: overrides.receiptRef ?? `receipt.pylon.${overrides.mode}.001`,
    sourceRef: overrides.sourceRef ?? `source.pylon.${overrides.mode}.001`,
    observedAt: overrides.observedAt ?? OBSERVED_AT,
    contentRedacted: true,
  }
}

describe("summarizeMultiEarning — inert by default", () => {
  it("is INERT with the flag off: empty projection, zero settled modes, red", () => {
    const summary = summarizeMultiEarning(
      [entry({ mode: "compute" }), entry({ mode: "tips" })],
      OBSERVED_AT,
      { env: {} },
    )
    expect(summary.inert).toBe(true)
    expect(summary.enabled).toBe(false)
    expect(summary.modes).toEqual([])
    expect(summary.settledModeCount).toBe(0)
    expect(summary.meetsMultiEarningBar).toBe(false)
    expect(summary.promiseState).toBe("red")
    expect(summary.clearsBlocker).toBe(
      "blocker.product_promises.multi_earning_mode_receipts_missing",
    )
  })

  it("arms only via the explicit env flag", () => {
    const summary = summarizeMultiEarning([entry({ mode: "compute" })], OBSERVED_AT, {
      env: { [MULTI_EARNING_LEDGER_ENV]: "1" },
    })
    expect(summary.inert).toBe(false)
    expect(summary.enabled).toBe(true)
  })

  it("arms via explicit enabled:true override", () => {
    const summary = summarizeMultiEarning([entry({ mode: "compute" })], OBSERVED_AT, {
      enabled: true,
      env: {},
    })
    expect(summary.inert).toBe(false)
  })

  it("surfaces the three owner-gated blockers it does NOT clear", () => {
    const summary = summarizeMultiEarning([], OBSERVED_AT, { env: {} })
    expect(summary.remainingOwnerGatedBlockers).toEqual([
      "blocker.product_promises.pylon_v1_default_install_not_fully_closed",
      "blocker.product_promises.multi_earning_settlement_refs_missing",
      "blocker.product_promises.safe_public_projection_missing",
    ])
  })
})

describe("summarizeMultiEarning — honest settled-mode counting", () => {
  it("distinguishes all five amount classes per mode", () => {
    const entries = MULTI_EARNING_AMOUNT_CLASSES.map((cls, i) =>
      entry({
        mode: "compute",
        amountClass: cls,
        amountSats: (i + 1) * 10,
        receiptRef: `receipt.pylon.compute.${cls}`,
      }),
    )
    const summary = summarizeMultiEarning(entries, OBSERVED_AT, { enabled: true })
    const compute = summary.modes.find((m) => m.mode === "compute")!
    expect(compute.amountSatsByClass.modeled).toBe(10)
    expect(compute.amountSatsByClass.observed).toBe(20)
    expect(compute.amountSatsByClass.pending).toBe(30)
    expect(compute.amountSatsByClass.paid).toBe(40)
    expect(compute.amountSatsByClass.settled).toBe(50)
    expect(compute.recordCountByClass.settled).toBe(1)
  })

  it("counts a mode toward green ONLY when it carries a settled receipt", () => {
    // compute settled, labor only paid (not settled): exactly ONE settled mode.
    const summary = summarizeMultiEarning(
      [
        entry({ mode: "compute", amountClass: "settled" }),
        entry({ mode: "labor", amountClass: "paid" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(summary.settledModes).toEqual(["compute"])
    expect(summary.settledModeCount).toBe(1)
    expect(summary.meetsMultiEarningBar).toBe(false)
  })

  it("meets the >=2 bar with settled receipts from two distinct modes", () => {
    const summary = summarizeMultiEarning(
      [
        entry({ mode: "compute", amountClass: "settled" }),
        entry({ mode: "tips", amountClass: "settled" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(summary.settledModeCount).toBe(2)
    expect(summary.settledModes).toEqual(["compute", "tips"])
    expect(summary.meetsMultiEarningBar).toBe(true)
    // Still red: the projection never flips the promise.
    expect(summary.promiseState).toBe("red")
  })

  it("does NOT double-count two settled receipts in the SAME mode as two modes", () => {
    const summary = summarizeMultiEarning(
      [
        entry({ mode: "compute", amountClass: "settled", receiptRef: "receipt.a" }),
        entry({ mode: "compute", amountClass: "settled", receiptRef: "receipt.b" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(summary.settledModeCount).toBe(1)
    expect(summary.meetsMultiEarningBar).toBe(false)
    const compute = summary.modes.find((m) => m.mode === "compute")!
    expect(compute.settledReceiptRefs).toEqual(["receipt.a", "receipt.b"])
  })

  it("dedupes identical settled receipt refs within a mode", () => {
    const summary = summarizeMultiEarning(
      [
        entry({ mode: "tips", amountClass: "settled", receiptRef: "receipt.dup", sourceRef: "s1" }),
        entry({ mode: "tips", amountClass: "settled", receiptRef: "receipt.dup", sourceRef: "s2" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    const tips = summary.modes.find((m) => m.mode === "tips")!
    expect(tips.settledReceiptRefs).toEqual(["receipt.dup"])
  })
})

describe("verifyMultiEarningEntry — public-safety guard", () => {
  it("accepts a well-formed entry", () => {
    expect(verifyMultiEarningEntry(entry({ mode: "compute" })).valid).toBe(true)
  })

  it("rejects an unknown key (leak vector)", () => {
    const bad = { ...entry({ mode: "compute" }), rawSparkAddress: "sp1qxyz" }
    const v = verifyMultiEarningEntry(bad)
    expect(v.valid).toBe(false)
    expect(v.reasons).toContain("unexpected-key:rawSparkAddress")
  })

  it("rejects a ref carrying whitespace (free-text / leak shape)", () => {
    const v = verifyMultiEarningEntry(entry({ mode: "compute", receiptRef: "lnbc1 raw invoice" }))
    expect(v.valid).toBe(false)
    expect(v.reasons).toContain("bad-receipt-ref")
  })

  it("rejects a negative or non-integer amount", () => {
    expect(verifyMultiEarningEntry(entry({ mode: "compute", amountSats: -1 })).valid).toBe(false)
    expect(verifyMultiEarningEntry(entry({ mode: "compute", amountSats: 1.5 })).valid).toBe(false)
  })

  it("rejects a non-canonical timestamp", () => {
    const v = verifyMultiEarningEntry(entry({ mode: "compute", observedAt: "yesterday" }))
    expect(v.valid).toBe(false)
    expect(v.reasons).toContain("bad-observed-at")
  })

  it("rejects an unknown mode and amount class", () => {
    expect(verifyMultiEarningEntry({ ...entry({ mode: "compute" }), mode: "moon" }).valid).toBe(false)
    expect(
      verifyMultiEarningEntry({ ...entry({ mode: "compute" }), amountClass: "imaginary" }).valid,
    ).toBe(false)
  })

  it("rejects unsafe entries inside summarize, counting them, never crediting", () => {
    const summary = summarizeMultiEarning(
      [
        entry({ mode: "compute", amountClass: "settled" }),
        { ...entry({ mode: "tips", amountClass: "settled" }), leak: "raw" } as unknown as MultiEarningLedgerEntry,
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(summary.rejectedEntryCount).toBe(1)
    expect(summary.settledModes).toEqual(["compute"])
    expect(summary.meetsMultiEarningBar).toBe(false)
  })
})

describe("captureMultiEarningReceipt — fail-closed dereferenceable receipt", () => {
  it("does not capture when inert", () => {
    const result = captureMultiEarningReceipt(
      [entry({ mode: "compute" }), entry({ mode: "tips" })],
      OBSERVED_AT,
      { env: {} },
    )
    expect(result.captured).toBe(false)
    if (!result.captured) expect(result.reasons).toContain("inert")
  })

  it("does not capture below the >=2 settled-mode bar", () => {
    const result = captureMultiEarningReceipt(
      [entry({ mode: "compute", amountClass: "settled" })],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(result.captured).toBe(false)
    if (!result.captured) expect(result.reasons).toContain("below-bar:1")
  })

  it("captures a self-verifying, round-trip-clean receipt at >=2 settled modes", () => {
    const result = captureMultiEarningReceipt(
      [
        entry({ mode: "compute", amountClass: "settled", receiptRef: "receipt.compute.001" }),
        entry({ mode: "tips", amountClass: "settled", receiptRef: "receipt.tips.001" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    expect(result.captured).toBe(true)
    if (!result.captured) throw new Error("expected capture")
    expect(result.receipt.settledModeCount).toBe(2)
    expect(result.receipt.settledModes).toEqual(["compute", "tips"])
    expect(result.receipt.settledReceiptRefs).toEqual([
      "receipt.compute.001",
      "receipt.tips.001",
    ])
    expect(result.receipt.ref).toBe(buildMultiEarningReceiptRef(2))
    // The serialized artifact re-audits clean.
    const reaudit = verifyMultiEarningReceipt(JSON.parse(result.serialized))
    expect(reaudit.clearsBlocker).toBe(true)
  })

  it("serializes deterministically regardless of key insertion order", () => {
    const result = captureMultiEarningReceipt(
      [
        entry({ mode: "compute", amountClass: "settled" }),
        entry({ mode: "labor", amountClass: "settled" }),
      ],
      OBSERVED_AT,
      { enabled: true },
    )
    if (!result.captured) throw new Error("expected capture")
    const shuffled = {
      contentRedacted: true as const,
      observedAt: result.receipt.observedAt,
      meetsMultiEarningBar: true as const,
      settledReceiptRefs: result.receipt.settledReceiptRefs,
      settledModes: result.receipt.settledModes,
      settledModeCount: result.receipt.settledModeCount,
      promiseId: result.receipt.promiseId,
      ref: result.receipt.ref,
      schema: result.receipt.schema,
    }
    expect(serializeMultiEarningReceipt(shuffled)).toBe(result.serialized)
  })
})

describe("verifyMultiEarningReceipt — auditor gate", () => {
  it("rejects a receipt claiming the bar with only one settled mode", () => {
    const v = verifyMultiEarningReceipt({
      schema: "openagents.pylon.multi_earning_receipt.v0.1",
      ref: buildMultiEarningReceiptRef(1),
      promiseId: "pylon.v0_3_multi_earning_node.v1",
      settledModeCount: 1,
      settledModes: ["compute"],
      settledReceiptRefs: ["receipt.a"],
      meetsMultiEarningBar: true,
      observedAt: OBSERVED_AT,
      contentRedacted: true,
    })
    expect(v.clearsBlocker).toBe(false)
  })

  it("rejects a ref whose encoded count does not match the body", () => {
    const v = verifyMultiEarningReceipt({
      schema: "openagents.pylon.multi_earning_receipt.v0.1",
      ref: buildMultiEarningReceiptRef(5),
      promiseId: "pylon.v0_3_multi_earning_node.v1",
      settledModeCount: 2,
      settledModes: ["compute", "tips"],
      settledReceiptRefs: ["receipt.a", "receipt.b"],
      meetsMultiEarningBar: true,
      observedAt: OBSERVED_AT,
      contentRedacted: true,
    })
    expect(v.clearsBlocker).toBe(false)
    expect(v.reasons).toContain("ref-count-mismatch")
  })

  it("rejects an unknown key (leak vector) in a captured receipt", () => {
    const v = verifyMultiEarningReceipt({
      schema: "openagents.pylon.multi_earning_receipt.v0.1",
      ref: buildMultiEarningReceiptRef(2),
      promiseId: "pylon.v0_3_multi_earning_node.v1",
      settledModeCount: 2,
      settledModes: ["compute", "tips"],
      settledReceiptRefs: ["receipt.a", "receipt.b"],
      meetsMultiEarningBar: true,
      observedAt: OBSERVED_AT,
      contentRedacted: true,
      rawMnemonic: "twelve words here",
    })
    expect(v.clearsBlocker).toBe(false)
    expect(v.reasons).toContain("unexpected-key:rawMnemonic")
  })
})
