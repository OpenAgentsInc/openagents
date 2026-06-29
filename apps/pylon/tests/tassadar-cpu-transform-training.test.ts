import { describe, expect, test } from "bun:test"
import {
  TASSADAR_CPU_TRANSFORM_ASSIGNMENT_REF,
  TASSADAR_CPU_TRANSFORM_RECEIPT_REF,
  runTassadarCpuTransformTrainingFixture,
} from "../src/tassadar-cpu-transform-training"
import { assertPublicProjectionSafe } from "../src/state"

describe("Tassadar CPU-transform training fixture", () => {
  test("runs one bounded CPU computation-transform training step", () => {
    const result = runTassadarCpuTransformTrainingFixture()

    expect(result.ok).toBe(true)
    expect(result.receipt).toMatchObject({
      receiptRef: TASSADAR_CPU_TRANSFORM_RECEIPT_REF,
      assignmentRef: TASSADAR_CPU_TRANSFORM_ASSIGNMENT_REF,
      trainingKind: "bounded_cpu_computation_transform",
      cpuOnly: true,
      fixtureScale: true,
      completedSteps: 1,
      verifierVerdict: "accepted",
      settlementState: "not_settled",
      realBitcoinMoved: false,
    })
    expect(result.receipt.lossAfterMicros).toBeLessThan(
      result.receipt.lossBeforeMicros,
    )
    expect(result.receipt.artifactDigest).toBe(
      "sha256:8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9",
    )
    expect(result.receipt.clearsBlockerRefs).toEqual([
      "blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing",
    ])
    expect(result.receipt.blockerRefs).toEqual([
      "blocker.product_promises.tassadar_cpu_transform_real_settlement_missing",
      "blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing",
    ])
    expect(result.serializedReceipt).toContain(TASSADAR_CPU_TRANSFORM_RECEIPT_REF)
    assertPublicProjectionSafe(result.receipt)
  })
})
