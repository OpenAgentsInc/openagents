import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TassadarPerceptaArchitectureReceiptBlocker,
  TassadarPerceptaArchitectureReceiptsEndpoint,
  TassadarPerceptaArchitectureReceiptsProjection,
  TassadarPerceptaArchitectureReceiptRef,
  TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker,
  TassadarPerceptaCpuTransformRealSettlementBlocker,
  TassadarPerceptaCpuTransformTrainingReceiptBlocker,
  projectTassadarPerceptaArchitectureReceipts,
} from './tassadar-percepta-architecture-receipts'
import { handleTassadarPerceptaArchitectureReceiptsApi } from './tassadar-percepta-architecture-receipts-routes'

type ArchitectureReceiptsBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    architectureReceiptsAvailable: boolean
    greenGateSatisfied: boolean
    pylonCpuTransformTrainingReceiptsAvailable: boolean
  }>
  promiseRef: string
  promiseState: string
}>

describe('Tassadar Percepta architecture receipts projection', () => {
  test('publishes architecture receipts without claiming a trained model', () => {
    const projection = projectTassadarPerceptaArchitectureReceipts({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TassadarPerceptaArchitectureReceiptsProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.endpoint).toBe(
      TassadarPerceptaArchitectureReceiptsEndpoint,
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
      architectureReceiptsAvailable: true,
      clearsBlockerRefs: [
        TassadarPerceptaArchitectureReceiptBlocker,
        TassadarPerceptaCpuTransformTrainingReceiptBlocker,
      ],
      greenGateSatisfied: false,
      pylonCpuTransformTrainingReceiptsAvailable: true,
      publicProjectionAvailable: true,
      remainingBlockerRefs: [
        TassadarPerceptaCpuTransformRealSettlementBlocker,
        TassadarPerceptaCpuTransformOwnerGreenSignoffBlocker,
      ],
    })
    expect(projection.receiptSummary).toMatchObject({
      architectureReceiptCount: 1,
      componentCount: 4,
      compiledExecutorDeploymentCount: 12,
      greenGateSatisfied: false,
      learnedInterfaceReceiptCount: 1,
    })
    expect(projection.receipts[0]).toMatchObject({
      architectureFamily: 'percepta_executor_hybrid',
      receiptRef: TassadarPerceptaArchitectureReceiptRef,
      receiptState: 'available',
      publicSafe: true,
      learnedInterfaceMetrics: {
        baselineRef: 'baseline_d_frozen_executor_learned_interface',
        exactRolloutPassAt1Bps: 10000,
        outputDigestMatchBps: 10000,
        replayVerifierAcceptanceBps: 10000,
      },
      compiledExecutorCoverage: {
        deploymentCount: 12,
        exactnessPosture: 'exact_trace_and_output',
        traceAbiRef: 'tassadar.trace.v1',
      },
    })
    expect(projection.receipts[0]?.clearsBlockerRefs).toEqual([
      TassadarPerceptaArchitectureReceiptBlocker,
    ])
    expect(projection.receipts[0]?.blockerRefs).toEqual([])
    expect(
      projection.receipts[0]?.components.map(component => component.componentKind),
    ).toEqual([
      'compiled_executor_bundle',
      'learned_interface_bundle',
      'verification_boundary',
      'artifact_lineage',
    ])
    expect(JSON.stringify(projection)).toContain(
      'fixtures/tassadar/w3_student_sweep_20260612/d/eval-report.json',
    )
    expect(JSON.stringify(projection)).toContain(
      'compiled_kernel_suite_v0/deployments/forward_branch_kernel_branches_2/model_descriptor.json',
    )
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTassadarPerceptaArchitectureReceipts({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim a trained')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
    expect(serialized).not.toContain('Apple-M5-Max')
  })

  test('serves the public architecture receipt route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTassadarPerceptaArchitectureReceiptsApi(
        new Request(
          `https://openagents.com${TassadarPerceptaArchitectureReceiptsEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as ArchitectureReceiptsBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TassadarPerceptaArchitectureReceiptsEndpoint)
    expect(body.promiseRef).toBe(
      'promise:models.tassadar_percepta_executor.v1',
    )
    expect(body.promiseState).toBe('planned')
    expect(body.gate.architectureReceiptsAvailable).toBe(true)
    expect(body.gate.pylonCpuTransformTrainingReceiptsAvailable).toBe(true)
    expect(body.gate.greenGateSatisfied).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTassadarPerceptaArchitectureReceiptsApi(
        new Request(
          `https://openagents.com${TassadarPerceptaArchitectureReceiptsEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
