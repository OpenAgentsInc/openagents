import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  TrainingAblationDeriskingLedgerEndpoint,
  TrainingAblationDeriskingLedgerProjection,
  projectTrainingAblationDeriskingLedger,
} from './training-ablation-derisking-ledger'
import { handleTrainingAblationDeriskingLedgerApi } from './training-ablation-derisking-ledger-routes'

type TrainingAblationLedgerBody = Readonly<{
  endpoint: string
  gate: Readonly<{
    publicProjectionAvailable: boolean
    greenGateSatisfied: boolean
  }>
  promiseRef: string
}>

describe('training ablation derisking ledger projection', () => {
  test('publishes a public-safe candidate ledger without claiming ablation execution', () => {
    const projection = projectTrainingAblationDeriskingLedger({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })

    expect(
      S.decodeUnknownSync(TrainingAblationDeriskingLedgerProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.promiseRef).toBe('promise:training.ablation_system.v1')
    expect(projection.promiseState).toBe('planned')
    expect(projection.status).toBe('candidate_ledger_projection')
    expect(projection.staleness).toMatchObject({
      composition: 'live_at_read',
      contractVersion: 'projection_staleness.v1',
      maxStalenessSeconds: 0,
    })
    expect(projection.gate).toMatchObject({
      ablationHarnessAvailable: false,
      evalSuiteReproductionAvailable: false,
      greenGateSatisfied: false,
      paidAblationDispatchAvailable: false,
      publicProjectionAvailable: true,
    })
    expect(projection.gate.clearsBlockerRefs).toContain(
      'blocker.product_promises.ablation_ledger_projection_missing',
    )
    expect(projection.gate.remainingBlockerRefs).toEqual([
      'blocker.product_promises.ablation_harness_missing',
      'blocker.product_promises.eval_suite_reproduction_missing',
    ])
    expect(projection.ledgerSummary).toMatchObject({
      acceptedVerdictCount: 0,
      entryCount: 3,
      paidAblationCount: 0,
      reproducedEvalCount: 0,
      verifiedManifestCount: 0,
    })
    expect(
      projection.entries.every(
        entry =>
          entry.oneDeltaManifestState === 'candidate_ref_only' &&
          entry.evalReproductionState === 'missing' &&
          entry.paidDispatchState === 'not_dispatched' &&
          entry.verdictState === 'no_openagents_verdict',
      ),
    ).toBe(true)
  })

  test('keeps authority and private material out of the public projection', () => {
    const projection = projectTrainingAblationDeriskingLedger({
      generatedAt: '2026-06-20T12:00:00.000Z',
    })
    const serialized = JSON.stringify(projection)

    expect(projection.authorityBoundary).toContain('grants no')
    expect(projection.unsafeCopy).toContain('Do not claim OpenAgents has run')
    expect(serialized).not.toMatch(
      /wallet|invoice|preimage|payment_hash|secret|raw_prompt|private_repo|\/home\/|\/Users\//i,
    )
  })

  test('serves the public ledger route as no-store JSON', async () => {
    const response = await Effect.runPromise(
      handleTrainingAblationDeriskingLedgerApi(
        new Request(
          `https://openagents.com${TrainingAblationDeriskingLedgerEndpoint}`,
        ),
      ),
    )
    const body = (await response.json()) as TrainingAblationLedgerBody

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.endpoint).toBe(TrainingAblationDeriskingLedgerEndpoint)
    expect(body.promiseRef).toBe('promise:training.ablation_system.v1')
    expect(body.gate.publicProjectionAvailable).toBe(true)
    expect(body.gate.greenGateSatisfied).toBe(false)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handleTrainingAblationDeriskingLedgerApi(
        new Request(
          `https://openagents.com${TrainingAblationDeriskingLedgerEndpoint}`,
          { method: 'POST' },
        ),
      ),
    )

    expect(response.status).toBe(405)
  })
})
