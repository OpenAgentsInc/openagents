import { describe, expect, test } from "bun:test"

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_LEG_REFS,
  TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN,
  TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
  TASSADAR_TS_REPLAY_CLASS_ID,
  TassadarCapabilityShapeError,
  buildTassadarExecutorCapabilityDeclaration,
  decodeTassadarCapabilityMatrixRow,
  hasReceiptedTassadarExecutorCapability,
  runTassadarExecutorSelfTest,
  stripUnreceiptedTassadarExecutorCapability,
} from "./index.js"
import {
  loadPinnedTassadarSelfTestWorkload,
  runPinnedTassadarExecutorSelfTest,
} from "./self-test.js"

const workload = loadPinnedTassadarSelfTestWorkload()

describe("tassadar executor capability envelope (W4.1)", () => {
  test("self-test on the pinned fixture verifies and mints a receipt ref", async () => {
    const receipt = await runPinnedTassadarExecutorSelfTest({
      observedAt: "2026-06-11T00:00:00.000Z",
    })
    expect(receipt.outcome).toBe("verified")
    expect(receipt.replayedTraceDigest).toBe(workload.expectedTraceDigest)
    expect(receipt.receiptRef).toMatch(
      TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN,
    )
    expect(receipt.stepCount).toBe(workload.steps.length)
    expect(receipt.modelDigest).toBe(workload.expectedModelDigest)
    expect(receipt.refusalDetail).toBeNull()
  })

  test("a forged pin fails the self-test and produces no receipt ref", async () => {
    const receipt = await runTassadarExecutorSelfTest({
      workload: { ...workload, expectedTraceDigest: "0".repeat(64) },
    })
    expect(receipt.outcome).toBe("failed")
    expect(receipt.receiptRef).toBeNull()
    expect(receipt.refusalDetail).toBe("trace_digest_mismatch")
  })

  test("a verified receipt declares the envelope profile; a failed one is a typed refusal", async () => {
    const verified = await runPinnedTassadarExecutorSelfTest()
    const declaration = buildTassadarExecutorCapabilityDeclaration(verified)
    if (!declaration.declared) throw new Error("expected declaration")
    expect(declaration.envelope.windowVersionRef).toBe(
      TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
    )
    expect(declaration.envelope.legRefs).toEqual([
      ...TASSADAR_EXECUTOR_LEG_REFS,
    ])
    expect(declaration.envelope.replayClassId).toBe(TASSADAR_TS_REPLAY_CLASS_ID)
    expect(declaration.capabilityRefs).toEqual([
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      verified.receiptRef!,
    ])

    const failed = await runTassadarExecutorSelfTest({
      workload: { ...workload, expectedTraceDigest: "0".repeat(64) },
    })
    const refused = buildTassadarExecutorCapabilityDeclaration(failed)
    expect(refused.declared).toBe(false)
    if (refused.declared) throw new Error("expected refusal")
    expect(refused.refusal.kind).toBe("self_test_failed")
    expect(refused.refusal.refusalRef).toBe(
      "refusal.tassadar_executor.capability_undeclarable",
    )
  })

  test("the capability matrix row derives from compile/replay receipts and is schema-enforced", async () => {
    const receipt = await runPinnedTassadarExecutorSelfTest()
    const declaration = buildTassadarExecutorCapabilityDeclaration(receipt)
    if (!declaration.declared) throw new Error("expected declaration")
    const row = declaration.matrixRow
    expect(row.compileReceiptRef).toBe(
      `receipt.tassadar_compile.model_digest.${workload.expectedModelDigest.slice(0, 16)}`,
    )
    expect(row.replayReceiptRef).toBe(receipt.receiptRef!)
    expect(decodeTassadarCapabilityMatrixRow(row)).toEqual(row)

    // Free-form configuration strings are rejected, field by field.
    expect(() =>
      decodeTassadarCapabilityMatrixRow({
        ...row,
        compileReceiptRef: "configured-by-operator",
      }),
    ).toThrow(TassadarCapabilityShapeError)
    expect(() =>
      decodeTassadarCapabilityMatrixRow({
        ...row,
        replayReceiptRef: "receipt.someone.said.so",
      }),
    ).toThrow(TassadarCapabilityShapeError)
    expect(() =>
      decodeTassadarCapabilityMatrixRow({
        ...row,
        legRefs: ["leg.tassadar_executor.invented.v1"],
      }),
    ).toThrow(TassadarCapabilityShapeError)
    expect(() =>
      decodeTassadarCapabilityMatrixRow({ ...row, posture: "execute_anything" }),
    ).toThrow(TassadarCapabilityShapeError)
  })

  test("publishable-ref helpers strip unreceipted claims and keep receipted ones", async () => {
    const receipt = await runPinnedTassadarExecutorSelfTest()
    const receipted = [
      "pylon.capability.gepa.benchmark_runner.v0.3",
      TASSADAR_EXECUTOR_CAPABILITY_REF,
      receipt.receiptRef!,
    ]
    expect(hasReceiptedTassadarExecutorCapability(receipted)).toBe(true)
    expect(stripUnreceiptedTassadarExecutorCapability(receipted)).toEqual(
      receipted,
    )

    const unreceipted = [
      "pylon.capability.gepa.benchmark_runner.v0.3",
      TASSADAR_EXECUTOR_CAPABILITY_REF,
    ]
    expect(hasReceiptedTassadarExecutorCapability(unreceipted)).toBe(false)
    expect(stripUnreceiptedTassadarExecutorCapability(unreceipted)).toEqual([
      "pylon.capability.gepa.benchmark_runner.v0.3",
    ])
  })
})
