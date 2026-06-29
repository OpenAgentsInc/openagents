// Tests for the INERT, flag-gated Spark-helper autostart readiness classifier.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.spark_helper_autostart_receipt_missing
import { describe, expect, it } from "bun:test"
import type { SparkBackupReceiveProjection } from "./wallet.js"
import {
  buildSparkHelperAutostartReceipt,
  captureSparkHelperAutostartReceipt,
  classifySparkHelperAutostart,
  serializeSparkHelperAutostartReceipt,
  SPARK_AUTOSTART_ENV,
  verifySparkHelperAutostartReceipt,
  verifySparkHelperAutostartReceiptSet,
} from "./spark-helper-autostart.js"

function receive(
  overrides: Partial<SparkBackupReceiveProjection>,
): SparkBackupReceiveProjection {
  return {
    schema: "openagents.pylon.spark_backup_receive.v0.1",
    enabled: true,
    state: "disabled",
    selectedBecauseRefs: [],
    receiveTargetRef: null,
    lightningAddressRef: null,
    rawTargetAvailableLocally: false,
    credentialReady: false,
    helperReady: false,
    detectedBalanceSats: null,
    unclaimedDepositCount: null,
    blockerRefs: [],
    nextActionRefs: [],
    publicReceiptRefs: [],
    contentRedacted: true,
    ...overrides,
  }
}

describe("classifySparkHelperAutostart", () => {
  it("is INERT by default: returns disabled with the flag unset", () => {
    const result = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { env: {} },
    )
    expect(result.enabled).toBe(false)
    expect(result.state).toBe("disabled")
    expect(result.payoutReady).toBe(false)
    expect(result.readinessReceiptRef).toBeNull()
    expect(result.nextActionRefs).toContain("action.pylon.spark_autostart.opt_in")
  })

  it("stays disabled even when address-ready unless explicitly opted in", () => {
    const result = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { env: { [SPARK_AUTOSTART_ENV]: "0" } },
    )
    expect(result.state).toBe("disabled")
  })

  it("reports credential-missing when opted in without a credential", () => {
    const result = classifySparkHelperAutostart(
      receive({ state: "credential-missing", credentialReady: false }),
      { enabled: true },
    )
    expect(result.state).toBe("credential-missing")
    expect(result.payoutReady).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.wallet.spark_backup.credential_missing",
    )
  })

  it("reports helper-not-ready (carrying the promise blocker) when the helper has not reached a target", () => {
    const result = classifySparkHelperAutostart(
      receive({
        state: "helper-unavailable",
        credentialReady: true,
        helperReady: false,
      }),
      { enabled: true },
    )
    expect(result.state).toBe("helper-not-ready")
    expect(result.payoutReady).toBe(false)
    expect(result.blockerRefs).toContain(
      "blocker.product_promises.spark_helper_autostart_receipt_missing",
    )
  })

  it("is autostart-ready when opted in, credentialed, and address-ready (env flag form)", () => {
    const result = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { env: { [SPARK_AUTOSTART_ENV]: "1" } },
    )
    expect(result.state).toBe("autostart-ready")
    expect(result.payoutReady).toBe(true)
    expect(result.readinessReceiptRef).toBe(
      "receipt.pylon.spark_helper_autostart.address-ready.v0.1",
    )
  })

  it("accepts an offline cached-address-ready target as autostart-ready", () => {
    const result = classifySparkHelperAutostart(
      receive({
        state: "cached-address-ready",
        credentialReady: true,
        helperReady: false,
      }),
      { enabled: true },
    )
    expect(result.state).toBe("autostart-ready")
    expect(result.payoutReady).toBe(true)
  })

  it("never leaks a raw target, balance, or credential into the projection", () => {
    const result = classifySparkHelperAutostart(
      receive({
        state: "address-ready",
        credentialReady: true,
        helperReady: true,
        detectedBalanceSats: 12345,
        receiveTargetRef: "wallet.backup.spark.deadbeef",
      }),
      { enabled: true },
    )
    const serialized = JSON.stringify(result)
    expect(serialized).not.toContain("12345")
    expect(serialized).not.toContain("deadbeef")
    expect(result.contentRedacted).toBe(true)
  })
})

describe("buildSparkHelperAutostartReceipt", () => {
  it("returns null unless the projection is autostart-ready", () => {
    const disabled = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { env: {} },
    )
    expect(
      buildSparkHelperAutostartReceipt(disabled, "2026-06-19T00:00:00.000Z"),
    ).toBeNull()
  })

  it("emits a redacted, no-hand-start receipt when autostart-ready", () => {
    const ready = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { enabled: true },
    )
    const receipt = buildSparkHelperAutostartReceipt(
      ready,
      "2026-06-19T00:00:00.000Z",
    )
    expect(receipt).not.toBeNull()
    expect(receipt?.operatorHandStartRequired).toBe(false)
    expect(receipt?.payoutReady).toBe(true)
    expect(receipt?.contentRedacted).toBe(true)
    expect(receipt?.ref).toBe(
      "receipt.pylon.spark_helper_autostart.address-ready.v0.1",
    )
  })
})

describe("verifySparkHelperAutostartReceipt", () => {
  function buildReady() {
    const ready = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { enabled: true },
    )
    const receipt = buildSparkHelperAutostartReceipt(ready, "2026-06-19T00:00:00.000Z")
    if (receipt === null) throw new Error("expected a receipt")
    return receipt
  }

  it("accepts a freshly built autostart receipt and reports it clears the blocker", () => {
    const result = verifySparkHelperAutostartReceipt(buildReady())
    expect(result.valid).toBe(true)
    expect(result.clearsBlocker).toBe(true)
    expect(result.reasons).toEqual([])
  })

  it("round-trips through JSON (the captured-artifact path)", () => {
    const parsed = JSON.parse(JSON.stringify(buildReady()))
    expect(verifySparkHelperAutostartReceipt(parsed).valid).toBe(true)
  })

  it("rejects non-objects", () => {
    expect(verifySparkHelperAutostartReceipt(null).valid).toBe(false)
    expect(verifySparkHelperAutostartReceipt("receipt").valid).toBe(false)
    expect(verifySparkHelperAutostartReceipt([buildReady()]).valid).toBe(false)
  })

  it("rejects any unexpected key (leak-prone field) and does not clear the blocker", () => {
    const tampered = { ...buildReady(), detectedBalanceSats: 12345 }
    const result = verifySparkHelperAutostartReceipt(tampered)
    expect(result.valid).toBe(false)
    expect(result.clearsBlocker).toBe(false)
    expect(result.reasons).toContain("unexpected-key:detectedBalanceSats")
  })

  it("rejects a receipt whose ref encodes a non-payout-ready state", () => {
    const bad = { ...buildReady(), ref: "receipt.pylon.spark_helper_autostart.claim-pending.v0.1" }
    const result = verifySparkHelperAutostartReceipt(bad)
    expect(result.valid).toBe(false)
    expect(result.reasons).toContain("ref-state-not-payout-ready")
  })

  it("rejects a ref whose state disagrees with derivedFromReceiveState", () => {
    const bad = {
      ...buildReady(),
      ref: "receipt.pylon.spark_helper_autostart.cached-address-ready.v0.1",
    }
    const result = verifySparkHelperAutostartReceipt(bad)
    expect(result.valid).toBe(false)
    expect(result.reasons).toContain("ref-state-mismatch")
  })

  it("rejects a receipt that requires an operator hand-start", () => {
    const bad = { ...buildReady(), operatorHandStartRequired: true }
    const result = verifySparkHelperAutostartReceipt(bad)
    expect(result.valid).toBe(false)
    expect(result.reasons).toContain("operator-hand-start-required")
  })

  it("rejects a receipt that is not payout-ready or not redacted", () => {
    expect(
      verifySparkHelperAutostartReceipt({ ...buildReady(), payoutReady: false }).reasons,
    ).toContain("not-payout-ready")
    expect(
      verifySparkHelperAutostartReceipt({ ...buildReady(), contentRedacted: false }).reasons,
    ).toContain("not-redacted")
  })

  it("rejects a non-canonical or missing observedAt timestamp", () => {
    expect(
      verifySparkHelperAutostartReceipt({ ...buildReady(), observedAt: "2026-06-19" }).reasons,
    ).toContain("bad-observed-at")
    expect(
      verifySparkHelperAutostartReceipt({ ...buildReady(), observedAt: "not-a-date" }).reasons,
    ).toContain("bad-observed-at")
  })
})

describe("verifySparkHelperAutostartReceiptSet", () => {
  // Independent captures differ at least in observedAt; the default keeps the
  // legacy timestamp, callers pass distinct ones to model distinct contributors.
  function buildReady(observedAt = "2026-06-19T00:00:00.000Z") {
    const ready = classifySparkHelperAutostart(
      receive({ state: "address-ready", credentialReady: true, helperReady: true }),
      { enabled: true },
    )
    const receipt = buildSparkHelperAutostartReceipt(ready, observedAt)
    if (receipt === null) throw new Error("expected a receipt")
    return receipt
  }

  it("rejects a non-array or empty set (cannot prove any contributor)", () => {
    expect(
      // @ts-expect-error exercising the runtime guard
      verifySparkHelperAutostartReceiptSet(null).reasons,
    ).toContain("not-an-array")
    const empty = verifySparkHelperAutostartReceiptSet([])
    expect(empty.valid).toBe(false)
    expect(empty.clearsBlocker).toBe(false)
    expect(empty.reasons).toContain("empty-set")
  })

  it("accepts distinct contributors each with their own independent receipt and clears at set level", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:contributor-a", receipt: buildReady("2026-06-19T00:00:00.000Z") },
      { contributorRef: "pylon:contributor-b", receipt: buildReady("2026-06-19T01:23:45.000Z") },
    ])
    expect(result.valid).toBe(true)
    expect(result.clearsBlocker).toBe(true)
    expect(result.distinctContributorCount).toBe(2)
    expect(result.reasons).toEqual([])
  })

  it("rejects one captured receipt replicated across two distinct contributor refs", () => {
    const captured = buildReady()
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:real", receipt: captured },
      // The autostart receipt has no contributor binding, so a copy of the same
      // artifact under a fabricated ref must NOT count as a second contributor.
      { contributorRef: "pylon:fabricated", receipt: { ...captured } },
    ])
    expect(result.valid).toBe(false)
    expect(result.clearsBlocker).toBe(false)
    expect(result.reasons).toContain("duplicate-receipt-artifact:pylon:fabricated")
    // The smuggling is in the receipt, not the ref: refs are genuinely distinct.
    expect(result.reasons.some((r) => r.startsWith("duplicate-contributor-ref"))).toBe(
      false,
    )
  })

  it("treats receipts that differ only in observedAt as distinct artifacts", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:a", receipt: buildReady("2026-06-19T00:00:00.000Z") },
      { contributorRef: "pylon:b", receipt: buildReady("2026-06-19T00:00:01.000Z") },
    ])
    expect(result.valid).toBe(true)
    expect(result.reasons.some((r) => r.startsWith("duplicate-receipt-artifact"))).toBe(
      false,
    )
  })

  it("clears the set bar with a single distinct normal contributor", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:solo", receipt: buildReady() },
    ])
    expect(result.clearsBlocker).toBe(true)
    expect(result.distinctContributorCount).toBe(1)
  })

  it("rejects one host passed off as many: a reused contributor ref", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:same", receipt: buildReady() },
      { contributorRef: "pylon:same", receipt: buildReady() },
    ])
    expect(result.valid).toBe(false)
    expect(result.clearsBlocker).toBe(false)
    expect(result.reasons).toContain("duplicate-contributor-ref:pylon:same")
  })

  it("rejects an empty or whitespace-bearing contributor ref by index", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "  ", receipt: buildReady() },
    ])
    expect(result.valid).toBe(false)
    expect(result.reasons).toContain("bad-contributor-ref:0")
  })

  it("fails the set when any entry's receipt does not pass the single bar", () => {
    const result = verifySparkHelperAutostartReceiptSet([
      { contributorRef: "pylon:good", receipt: buildReady() },
      {
        contributorRef: "pylon:bad",
        receipt: { ...buildReady(), operatorHandStartRequired: true },
      },
    ])
    expect(result.valid).toBe(false)
    expect(result.clearsBlocker).toBe(false)
    expect(result.reasons).toContain("entry-receipt-invalid:pylon:bad")
    // The good entry's contributor is still surfaced for traceability.
    expect(result.distinctContributorCount).toBe(1)
  })
})

describe("captureSparkHelperAutostartReceipt", () => {
  const readyReceive = receive({
    state: "address-ready",
    credentialReady: true,
    helperReady: true,
  })

  it("does not capture when not opted in (inert by default)", () => {
    const result = captureSparkHelperAutostartReceipt(
      readyReceive,
      "2026-06-19T00:00:00.000Z",
      { env: {} },
    )
    expect(result.captured).toBe(false)
    if (!result.captured) {
      expect(result.reasons).toContain("not-autostart-ready:disabled")
    }
  })

  it("does not capture when opted in but the helper is not ready", () => {
    const result = captureSparkHelperAutostartReceipt(
      receive({ credentialReady: true, helperReady: false, state: "helper-unavailable" }),
      "2026-06-19T00:00:00.000Z",
      { enabled: true },
    )
    expect(result.captured).toBe(false)
    if (!result.captured) {
      expect(result.reasons).toContain("not-autostart-ready:helper-not-ready")
    }
  })

  it("captures a self-verified, gate-valid artifact when autostart-ready", () => {
    const result = captureSparkHelperAutostartReceipt(
      readyReceive,
      "2026-06-19T00:00:00.000Z",
      { enabled: true },
    )
    expect(result.captured).toBe(true)
    if (result.captured) {
      // The emitted receipt passes the very gate an auditor would run.
      expect(result.verification.clearsBlocker).toBe(true)
      expect(verifySparkHelperAutostartReceipt(result.receipt).clearsBlocker).toBe(true)
      // And the persisted (serialized) form round-trips through the gate too.
      const reparsed = JSON.parse(result.serialized)
      expect(verifySparkHelperAutostartReceipt(reparsed).clearsBlocker).toBe(true)
      expect(result.receipt.operatorHandStartRequired).toBe(false)
    }
  })

  it("serializes deterministically and canonically regardless of key order", () => {
    const result = captureSparkHelperAutostartReceipt(
      readyReceive,
      "2026-06-19T00:00:00.000Z",
      { enabled: true },
    )
    expect(result.captured).toBe(true)
    if (result.captured) {
      // Re-serializing a key-shuffled copy of the same receipt is byte-identical.
      const shuffled = {
        contentRedacted: result.receipt.contentRedacted,
        observedAt: result.receipt.observedAt,
        operatorHandStartRequired: result.receipt.operatorHandStartRequired,
        derivedFromReceiveState: result.receipt.derivedFromReceiveState,
        payoutReady: result.receipt.payoutReady,
        ref: result.receipt.ref,
        schema: result.receipt.schema,
      }
      expect(serializeSparkHelperAutostartReceipt(shuffled)).toBe(result.serialized)
      expect(result.serialized.endsWith("\n")).toBe(true)
    }
  })

  it("rejects a non-canonical observation timestamp (fail-closed, no artifact)", () => {
    const result = captureSparkHelperAutostartReceipt(
      readyReceive,
      "2026-06-19 00:00:00",
      { enabled: true },
    )
    expect(result.captured).toBe(false)
    if (!result.captured) {
      expect(result.reasons).toContain("self-verify-failed:bad-observed-at")
    }
  })

  it("two distinct captures differ only by observedAt and are accepted as distinct", () => {
    const a = captureSparkHelperAutostartReceipt(readyReceive, "2026-06-19T00:00:00.000Z", {
      enabled: true,
    })
    const b = captureSparkHelperAutostartReceipt(readyReceive, "2026-06-19T01:23:45.000Z", {
      enabled: true,
    })
    expect(a.captured && b.captured).toBe(true)
    if (a.captured && b.captured) {
      expect(a.serialized).not.toBe(b.serialized)
      const set = verifySparkHelperAutostartReceiptSet([
        { contributorRef: "pylon:alpha", receipt: a.receipt },
        { contributorRef: "pylon:beta", receipt: b.receipt },
      ])
      expect(set.clearsBlocker).toBe(true)
      expect(set.distinctContributorCount).toBe(2)
    }
  })
})
