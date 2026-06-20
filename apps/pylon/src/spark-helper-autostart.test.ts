// Tests for the INERT, flag-gated Spark-helper autostart readiness classifier.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.spark_helper_autostart_receipt_missing
import { describe, expect, it } from "bun:test"
import type { SparkBackupReceiveProjection } from "./wallet"
import {
  buildSparkHelperAutostartReceipt,
  classifySparkHelperAutostart,
  SPARK_AUTOSTART_ENV,
} from "./spark-helper-autostart"

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
