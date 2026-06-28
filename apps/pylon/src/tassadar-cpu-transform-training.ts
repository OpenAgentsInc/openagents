import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export const TASSADAR_CPU_TRANSFORM_ASSIGNMENT_REF =
  "assignment.models.tassadar_percepta_executor.cpu_transform_fixture.v1"
export const TASSADAR_CPU_TRANSFORM_RECEIPT_REF =
  "receipt.models.tassadar_percepta_executor.cpu_transform_training.cpu_transform_fixture_v1"
export const TASSADAR_CPU_TRANSFORM_VERDICT_REF =
  "verdict.tassadar_cpu_transform.exact_replay.cpu_transform_fixture_v1"
export const TASSADAR_CPU_TRANSFORM_ARTIFACT_REF =
  "artifact.tassadar_percepta_executor.cpu_transform_checkpoint.sha256.8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9"

export type TassadarCpuTransformTrainingReceipt = {
  schema: "openagents.pylon.tassadar_cpu_transform_training_receipt.v1"
  receiptRef: typeof TASSADAR_CPU_TRANSFORM_RECEIPT_REF
  assignmentRef: typeof TASSADAR_CPU_TRANSFORM_ASSIGNMENT_REF
  runRef: "run.tassadar_percepta_executor.cpu_transform_fixture.v1"
  executorClassRef: "model_profile.tassadar_percepta_executor.v1"
  trainingKind: "bounded_cpu_computation_transform"
  publicSafe: true
  cpuOnly: true
  fixtureScale: true
  completedSteps: 1
  inputVectorCount: 3
  parameterCount: 2
  learningRateBps: 2500
  lossBeforeMicros: number
  lossAfterMicros: number
  lossImproved: boolean
  artifactRef: typeof TASSADAR_CPU_TRANSFORM_ARTIFACT_REF
  artifactDigest: string
  verifierVerdictRef: typeof TASSADAR_CPU_TRANSFORM_VERDICT_REF
  verifierVerdict: "accepted"
  acceptedWorkReceiptRef: "receipt.nexus_pylon.tassadar_cpu_transform_closeout.cpu_transform_fixture_v1"
  settlementState: "not_settled"
  realBitcoinMoved: false
  clearsBlockerRefs: [
    "blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing",
  ]
  blockerRefs: [
    "blocker.product_promises.tassadar_cpu_transform_real_settlement_missing",
    "blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing",
  ]
  caveatRefs: [
    "caveat.tassadar_cpu_transform.fixture_scale_only",
    "caveat.tassadar_cpu_transform.no_trained_model_claim",
    "caveat.tassadar_cpu_transform.no_settlement_or_earning_claim",
  ]
  sourceRefs: string[]
  unsafeCopy: string
}

export type TassadarCpuTransformTrainingRun = {
  ok: boolean
  receipt: TassadarCpuTransformTrainingReceipt
  serializedReceipt: string
}

type TrainingPoint = Readonly<{
  input: number
  target: number
}>

const fixture: ReadonlyArray<TrainingPoint> = [
  { input: -1, target: -1 },
  { input: 0, target: 1 },
  { input: 1, target: 3 },
]

const initialParameters = {
  biasMicros: 0,
  scaleMicros: 1_000_000,
}

const targetArtifactDigest =
  "sha256:8feaf5488599a4b618b8d2188ed8ea0b68ec9fb5f58a55db3064e52ae9ff73d9"

export function runTassadarCpuTransformTrainingFixture(): TassadarCpuTransformTrainingRun {
  const beforeLoss = meanSquaredLossMicros(initialParameters)
  const gradient = gradientMicros(initialParameters)
  const afterParameters = {
    biasMicros:
      initialParameters.biasMicros -
      Math.trunc((gradient.biasMicros * 2500) / 10_000),
    scaleMicros:
      initialParameters.scaleMicros -
      Math.trunc((gradient.scaleMicros * 2500) / 10_000),
  }
  const afterLoss = meanSquaredLossMicros(afterParameters)
  const artifactDigest = digestArtifact({
    afterParameters,
    beforeLoss,
    afterLoss,
  })
  const receipt: TassadarCpuTransformTrainingReceipt = {
    schema: "openagents.pylon.tassadar_cpu_transform_training_receipt.v1",
    receiptRef: TASSADAR_CPU_TRANSFORM_RECEIPT_REF,
    assignmentRef: TASSADAR_CPU_TRANSFORM_ASSIGNMENT_REF,
    runRef: "run.tassadar_percepta_executor.cpu_transform_fixture.v1",
    executorClassRef: "model_profile.tassadar_percepta_executor.v1",
    trainingKind: "bounded_cpu_computation_transform",
    publicSafe: true,
    cpuOnly: true,
    fixtureScale: true,
    completedSteps: 1,
    inputVectorCount: 3,
    parameterCount: 2,
    learningRateBps: 2500,
    lossBeforeMicros: beforeLoss,
    lossAfterMicros: afterLoss,
    lossImproved: afterLoss < beforeLoss,
    artifactRef: TASSADAR_CPU_TRANSFORM_ARTIFACT_REF,
    artifactDigest,
    verifierVerdictRef: TASSADAR_CPU_TRANSFORM_VERDICT_REF,
    verifierVerdict: "accepted",
    acceptedWorkReceiptRef:
      "receipt.nexus_pylon.tassadar_cpu_transform_closeout.cpu_transform_fixture_v1",
    settlementState: "not_settled",
    realBitcoinMoved: false,
    clearsBlockerRefs: [
      "blocker.product_promises.pylon_v03_cpu_transform_training_receipts_missing",
    ],
    blockerRefs: [
      "blocker.product_promises.tassadar_cpu_transform_real_settlement_missing",
      "blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing",
    ],
    caveatRefs: [
      "caveat.tassadar_cpu_transform.fixture_scale_only",
      "caveat.tassadar_cpu_transform.no_trained_model_claim",
      "caveat.tassadar_cpu_transform.no_settlement_or_earning_claim",
    ],
    sourceRefs: [
      "apps/pylon/src/tassadar-cpu-transform-training.ts",
      "apps/pylon/tests/tassadar-cpu-transform-training.test.ts",
      "docs/tassadar/2026-06-21-tassadar-cpu-transform-training-receipt-surface.md",
    ],
    unsafeCopy:
      "Do not claim this fixture-scale CPU transform step is a trained Tassadar model, a public earning path, a settled assignment, a promoted checkpoint, or a green product promise.",
  }

  if (receipt.artifactDigest !== targetArtifactDigest) {
    throw new Error("Tassadar CPU-transform fixture digest drifted")
  }

  assertPublicProjectionSafe(receipt)

  return {
    ok: receipt.lossImproved && receipt.verifierVerdict === "accepted",
    receipt,
    serializedReceipt: serializeReceipt(receipt),
  }
}

function meanSquaredLossMicros(parameters: {
  biasMicros: number
  scaleMicros: number
}): number {
  const lossMicros = fixture.reduce((total, point) => {
    const predictionMicros =
      parameters.biasMicros + parameters.scaleMicros * point.input
    const targetMicros = point.target * 1_000_000
    const errorMicros = predictionMicros - targetMicros
    return total + Math.trunc((errorMicros * errorMicros) / 1_000_000)
  }, 0)

  return Math.trunc(lossMicros / fixture.length)
}

function gradientMicros(parameters: {
  biasMicros: number
  scaleMicros: number
}): { biasMicros: number; scaleMicros: number } {
  const summed = fixture.reduce(
    (total, point) => {
      const predictionMicros =
        parameters.biasMicros + parameters.scaleMicros * point.input
      const targetMicros = point.target * 1_000_000
      const errorMicros = predictionMicros - targetMicros

      return {
        biasMicros: total.biasMicros + 2 * errorMicros,
        scaleMicros: total.scaleMicros + 2 * errorMicros * point.input,
      }
    },
    { biasMicros: 0, scaleMicros: 0 },
  )

  return {
    biasMicros: Math.trunc(summed.biasMicros / fixture.length),
    scaleMicros: Math.trunc(summed.scaleMicros / fixture.length),
  }
}

function digestArtifact(input: {
  afterParameters: { biasMicros: number; scaleMicros: number }
  beforeLoss: number
  afterLoss: number
}): string {
  const artifact = serializeReceipt({
    afterParameters: input.afterParameters,
    beforeLoss: input.beforeLoss,
    afterLoss: input.afterLoss,
    fixtureRef: "fixture.tassadar_cpu_transform.linear_offset.v1",
  })
  return `sha256:${createHash("sha256").update(artifact).digest("hex")}`
}

function serializeReceipt(value: unknown): string {
  return JSON.stringify(sortJson(value))
}

function sortJson(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortJson)
  }
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, sortJson(nested)]),
    )
  }
  return value
}
