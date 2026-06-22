import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
  ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
} from './artanis-distillation-dataset-receipt'
import {
  TassadarPerceptaArchitectureReceiptRef,
  TassadarPerceptaArchitectureReceiptsEndpoint,
  TassadarPerceptaCpuTransformTrainingReceiptBlocker,
} from './tassadar-percepta-architecture-receipts'
import {
  ArtanisTassadarDistillationDatasetEndpoint,
  TassadarPerceptaCpuTransformTrainingReceiptRefPattern,
  TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
  TassadarPerceptaCpuTransformTrainingReceiptsProjection,
  projectTassadarPerceptaCpuTransformTrainingReceipts,
} from './tassadar-percepta-cpu-transform-training-receipts'
import { handleTassadarPerceptaCpuTransformTrainingReceiptsApi } from './tassadar-percepta-cpu-transform-training-receipts-routes'

type CpuTransformTrainingReceiptsBody = Readonly<{
  endpoint: string
  expectedReceiptSurface: Readonly<{
    emittedReceiptCount: number
    expectedReceiptRefPattern: string
    requirements: ReadonlyArray<
      Readonly<{
        available: boolean
        requirementKind: string
      }>
    >
    routePublishesReceipts: boolean
    routePublishesStatusOnly: boolean
  }>
  gate: Readonly<{
    acceptedWorkReceiptAvailable: boolean
    architectureReceiptAvailable: boolean
    cpuTransformTrainingReceiptAvailable: boolean
    distillationDatasetReceiptInputAvailable: boolean
    greenGateSatisfied: boolean
    pylonAssignmentReceiptAvailable: boolean
    realSettlementReceiptAvailable: boolean
    remainingBlockerRefs: ReadonlyArray<string>
    trainedModelArtifactAvailable: boolean
    verifierVerdictReceiptAvailable: boolean
  }>
  inputRefs: ReadonlyArray<Readonly<{ endpoint: string; receiptRef: string }>>
  promiseRef: string
  promiseState: string
  receiptSummary: Readonly<{
    architectureReceiptCount: number
    distillationDatasetReceiptCount: number
    emittedCpuTransformTrainingReceiptCount: number
    requiredAcceptedTraceCount: number
  }>
}>

describe('Tassadar Percepta CPU-transform training receipts projection', () => {
  test('publishes the missing-receipt status without claiming training', () => {
    const projection = projectTassadarPerceptaCpuTransformTrainingReceipts({
      generatedAt: '2026-06-21T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(
        TassadarPerceptaCpuTransformTrainingReceiptsProjection,
      )(projection),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(
      TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
    )
    expect(projection.promiseRef).toBe(
      'promise:models.tassadar_percepta_executor.v1',
    )
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      acceptedWorkReceiptAvailable: false,
      architectureReceiptAvailable: true,
      clearsBlockerRefs: [],
      cpuTransformTrainingReceiptAvailable: false,
      distillationDatasetReceiptInputAvailable: true,
      greenGateSatisfied: false,
      pylonAssignmentReceiptAvailable: false,
      publicProjectionAvailable: true,
      realSettlementReceiptAvailable: false,
      remainingBlockerRefs: [
        TassadarPerceptaCpuTransformTrainingReceiptBlocker,
      ],
      trainedModelArtifactAvailable: false,
      verifierVerdictReceiptAvailable: false,
    })
    expect(projection.inputRefs).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          available: true,
          endpoint: TassadarPerceptaArchitectureReceiptsEndpoint,
          inputKind: 'architecture_receipt',
          receiptRef: TassadarPerceptaArchitectureReceiptRef,
        }),
        expect.objectContaining({
          available: true,
          endpoint: ArtanisTassadarDistillationDatasetEndpoint,
          inputKind: 'distillation_dataset_receipt',
          receiptRef: ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
        }),
      ]),
    )
    expect(projection.expectedReceiptSurface).toMatchObject({
      emittedReceiptCount: 0,
      expectedReceiptRefPattern:
        TassadarPerceptaCpuTransformTrainingReceiptRefPattern,
      routePublishesReceipts: false,
      routePublishesStatusOnly: true,
    })
    expect(
      projection.expectedReceiptSurface.requirements.map(
        requirement => requirement.requirementKind,
      ),
    ).toEqual([
      'pylon_assignment_receipt',
      'accepted_work_receipt',
      'verifier_verdict_receipt',
      'real_settlement_receipt',
      'trained_artifact_digest',
    ])
    expect(
      projection.expectedReceiptSurface.requirements.every(
        requirement => requirement.available === false,
      ),
    ).toBe(true)
    expect(projection.receiptSummary).toEqual({
      architectureReceiptCount: 1,
      distillationDatasetReceiptCount: 1,
      emittedCpuTransformTrainingReceiptCount: 0,
      requiredAcceptedTraceCount: ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
    })
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTassadarPerceptaCpuTransformTrainingReceipts({
      generatedAt: '2026-06-21T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public CPU-transform receipt status route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTassadarPerceptaCpuTransformTrainingReceiptsApi(
        new Request(
          `https://openagents.com${TassadarPerceptaCpuTransformTrainingReceiptsEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as CpuTransformTrainingReceiptsBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(
      TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
    )
    expect(body.promiseRef).toBe('promise:models.tassadar_percepta_executor.v1')
    expect(body.promiseState).toBe('planned')
    expect(body.gate.architectureReceiptAvailable).toBe(true)
    expect(body.gate.distillationDatasetReceiptInputAvailable).toBe(true)
    expect(body.gate.cpuTransformTrainingReceiptAvailable).toBe(false)
    expect(body.gate.pylonAssignmentReceiptAvailable).toBe(false)
    expect(body.gate.acceptedWorkReceiptAvailable).toBe(false)
    expect(body.gate.verifierVerdictReceiptAvailable).toBe(false)
    expect(body.gate.realSettlementReceiptAvailable).toBe(false)
    expect(body.gate.trainedModelArtifactAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      TassadarPerceptaCpuTransformTrainingReceiptBlocker,
    ])
    expect(body.expectedReceiptSurface.emittedReceiptCount).toBe(0)
    expect(body.expectedReceiptSurface.routePublishesReceipts).toBe(false)
    expect(body.expectedReceiptSurface.routePublishesStatusOnly).toBe(true)
    expect(body.receiptSummary).toEqual({
      architectureReceiptCount: 1,
      distillationDatasetReceiptCount: 1,
      emittedCpuTransformTrainingReceiptCount: 0,
      requiredAcceptedTraceCount: ARTANIS_TASSADAR_DISTILLATION_DATASET_TARGET,
    })
    expect(body.inputRefs.map(inputRef => inputRef.receiptRef)).toEqual(
      expect.arrayContaining([
        TassadarPerceptaArchitectureReceiptRef,
        ARTANIS_TASSADAR_DISTILLATION_DATASET_RECEIPT_REF,
      ]),
    )
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTassadarPerceptaCpuTransformTrainingReceiptsApi(
        new Request(
          `https://openagents.com${TassadarPerceptaCpuTransformTrainingReceiptsEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
