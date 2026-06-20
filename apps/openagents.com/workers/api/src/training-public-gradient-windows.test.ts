import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingPublicGradientLiveWindowRuntimeBlocker,
  TrainingPublicGradientPromotedWindowReceiptBlocker,
  TrainingPublicGradientSettlementReceiptBlocker,
  TrainingPublicGradientWindowsEndpoint,
  TrainingPublicGradientWindowsProjection,
  projectTrainingPublicGradientWindows,
} from './training-public-gradient-windows'
import { handleTrainingPublicGradientWindowsApi } from './training-public-gradient-windows-routes'

type PublicGradientWindowsBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    greenGateSatisfied: boolean
    intakeAdmissionPredicateAvailable: boolean
    liveWindowRuntimeAvailable: boolean
    promotedWindowReceiptAvailable: boolean
    promotionReceiptEmitterAvailable: boolean
    publicProjectionAvailable: boolean
    regimeGateAvailable: boolean
    remainingBlockerRefs: ReadonlyArray<string>
    settlementReceiptAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
  intakeSurface: Readonly<{
    acceptedSubmissionCount: number
    admittedQuarantineRecordCount: number
    predicateAvailable: boolean
    quarantineRecordFormatAvailable: boolean
    quarantineRecordSchemaVersion: string
    quarantineRecordVerifierAvailable: boolean
    quarantineRecordVerifierSchemaVersion: string
    quarantineRouteAvailable: boolean
    schemaVersion: string
  }>
  receiptSurface: Readonly<{
    emittedReceiptCount: number
    receiptRouteAvailable: boolean
  }>
  runtimeSurface: Readonly<{
    acceptedPublicWindowCount: number
    canonicalCheckpointMutationCount: number
    currentRuntimeState: string
    promotedPublicWindowCount: number
    settlementReceiptCount: number
  }>
}>

describe('training public gradient windows projection', () => {
  test('publishes the receipt surface without claiming live public gradients', () => {
    const projection = projectTrainingPublicGradientWindows({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingPublicGradientWindowsProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(TrainingPublicGradientWindowsEndpoint)
    expect(projection.promiseRef).toBe(
      'promise:training.public_gradient_windows.v1',
    )
    expect(projection.promiseState).toBe('planned')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toEqual({
      clearsBlockerRefs: [],
      greenGateSatisfied: false,
      intakeAdmissionPredicateAvailable: true,
      liveWindowRuntimeAvailable: false,
      promotedWindowReceiptAvailable: false,
      promotionReceiptEmitterAvailable: true,
      publicProjectionAvailable: true,
      regimeGateAvailable: true,
      remainingBlockerRefs: [
        TrainingPublicGradientLiveWindowRuntimeBlocker,
        TrainingPublicGradientPromotedWindowReceiptBlocker,
        TrainingPublicGradientSettlementReceiptBlocker,
      ],
      settlementReceiptAvailable: false,
    })
    expect(projection.receiptSurface).toMatchObject({
      emittedReceiptCount: 0,
      promotionLineageGuardAvailable: true,
      promotionLineageSchemaVersion:
        'openagents.training.public_gradient_window.promotion_lineage.v1',
      receiptRouteAvailable: false,
      receiptSchemaVersion:
        'openagents.training.public_gradient_window.promotion_receipt.v1',
      receiptVerifierAvailable: true,
      receiptVerifierSchemaVersion:
        'openagents.training.public_gradient_window.promotion_receipt_verification.v1',
      receiptFeedFormatAvailable: true,
      receiptFeedSchemaVersion:
        'openagents.training.public_gradient_window.promotion_receipt_feed.v1',
    })
    expect(projection.intakeSurface).toMatchObject({
      acceptedSubmissionCount: 0,
      admittedQuarantineRecordCount: 0,
      predicateAvailable: true,
      quarantineRecordFormatAvailable: true,
      quarantineRecordSchemaVersion:
        'openagents.training.public_gradient_window.quarantine_record.v1',
      quarantineRecordVerifierAvailable: true,
      quarantineRecordVerifierSchemaVersion:
        'openagents.training.public_gradient_window.quarantine_record_verification.v1',
      quarantineRouteAvailable: false,
      schemaVersion:
        'openagents.training.public_gradient_window.intake_admission.v1',
      sourceRefs: [
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-intake.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record.test.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record-verify.ts',
        'apps/openagents.com/workers/api/src/tassadar-gradient-window-quarantine-record-verify.test.ts',
      ],
    })
    expect(projection.runtimeSurface).toEqual({
      acceptedPublicWindowCount: 0,
      canonicalCheckpointMutationCount: 0,
      currentRuntimeState: 'not_live',
      promotedPublicWindowCount: 0,
      settlementReceiptCount: 0,
    })
    expect(projection.stageRefs).toEqual([
      'submitted',
      'quarantined',
      'recomputed',
      'replicated',
      'canary_passed',
      'promoted',
      'blocked',
    ])
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingPublicGradientWindows({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTrainingPublicGradientWindowsApi(
        new Request(
          `https://openagents.com${TrainingPublicGradientWindowsEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as PublicGradientWindowsBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingPublicGradientWindowsEndpoint)
    expect(body.promiseRef).toBe(
      'promise:training.public_gradient_windows.v1',
    )
    expect(body.promiseState).toBe('planned')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.intakeAdmissionPredicateAvailable).toBe(true)
    expect(body.gate.regimeGateAvailable).toBe(true)
    expect(body.gate.promotionReceiptEmitterAvailable).toBe(true)
    expect(body.gate.liveWindowRuntimeAvailable).toBe(false)
    expect(body.gate.promotedWindowReceiptAvailable).toBe(false)
    expect(body.gate.settlementReceiptAvailable).toBe(false)
    expect(body.gate.greenGateSatisfied).toBe(false)
    expect(body.gate.remainingBlockerRefs).toEqual([
      TrainingPublicGradientLiveWindowRuntimeBlocker,
      TrainingPublicGradientPromotedWindowReceiptBlocker,
      TrainingPublicGradientSettlementReceiptBlocker,
    ])
    expect(body.receiptSurface.emittedReceiptCount).toBe(0)
    expect(body.receiptSurface.receiptRouteAvailable).toBe(false)
    expect(body.intakeSurface.predicateAvailable).toBe(true)
    expect(body.intakeSurface.schemaVersion).toBe(
      'openagents.training.public_gradient_window.intake_admission.v1',
    )
    expect(body.intakeSurface.quarantineRouteAvailable).toBe(false)
    expect(body.intakeSurface.acceptedSubmissionCount).toBe(0)
    expect(body.intakeSurface.admittedQuarantineRecordCount).toBe(0)
    expect(body.runtimeSurface.currentRuntimeState).toBe('not_live')
    expect(body.runtimeSurface.acceptedPublicWindowCount).toBe(0)
    expect(body.runtimeSurface.promotedPublicWindowCount).toBe(0)
    expect(body.runtimeSurface.settlementReceiptCount).toBe(0)
    expect(body.runtimeSurface.canonicalCheckpointMutationCount).toBe(0)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingPublicGradientWindowsApi(
        new Request(
          `https://openagents.com${TrainingPublicGradientWindowsEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
