import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type PromiseTransitionReceipt,
  type PromiseTransitionReceiptStore,
  evaluatePromiseTransition,
  handleOperatorPromiseTransitionApi,
  handlePublicPromiseTransitionsApi,
  lastVerifiedAtByPromise,
} from './promise-transition-receipt-routes'

const memoryStore = (): PromiseTransitionReceiptStore &
  Readonly<{ receipts: PromiseTransitionReceipt[] }> => {
  const receipts: PromiseTransitionReceipt[] = []

  return {
    createReceipt: async receipt => {
      receipts.push(receipt)
    },
    listReceipts: async limit => receipts.slice(0, limit),
    receipts,
  }
}

describe('promise transition receipts', () => {
  test('passes a transition whose registry record satisfies the checks', () => {
    const evaluation = evaluatePromiseTransition({
      evidenceRefs: [],
      promiseId: 'autopilot.mission_briefing.v1',
      toState: 'green',
    })

    // 2026-07-04.8 owner-directed revenue-refocus demotion
    expect(evaluation.fromState).toBe('planned')
    expect(evaluation.registryVersion).toMatch(/^\d{4}-\d{2}-\d{2}/)
    expect(
      evaluation.checks.find(check => check.kind === 'promise_exists')?.result,
    ).toBe('passed')
    expect(
      evaluation.checks.find(
        check => check.kind === 'blockers_clear_for_green',
      )?.result,
    ).toBe('failed')
    expect(evaluation.result).toBe('failed')
  })

  test('fails unknown promises and no-op transitions', () => {
    expect(
      evaluatePromiseTransition({
        evidenceRefs: [],
        promiseId: 'does.not.exist.v1',
        toState: 'green',
      }).result,
    ).toBe('failed')
    expect(
      evaluatePromiseTransition({
        evidenceRefs: [],
        promiseId: 'autopilot.mission_briefing.v1',
        // 2026-07-04.8: record is planned; planned->planned is the no-op
        toState: 'planned',
      }).checks.find(check => check.kind === 'from_state_differs')?.result,
    ).toBe('failed')
  })

  test('records receipts through the operator route and serves the public feed', async () => {
    const store = memoryStore()
    const denied = await Effect.runPromise(
      handleOperatorPromiseTransitionApi(
        new Request(
          'https://openagents.com/api/operator/product-promises/transitions',
          {
            body: JSON.stringify({
              promiseId: 'autopilot.mission_briefing.v1',
              toState: 'green',
            }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        {
          requireAdminApiToken: () => Promise.resolve(false),
          store,
        },
      ),
    )
    const recorded = await Effect.runPromise(
      handleOperatorPromiseTransitionApi(
        new Request(
          'https://openagents.com/api/operator/product-promises/transitions',
          {
            body: JSON.stringify({
              evidenceRefs: ['route:/api/public/pylon-capacity-funnel'],
              promiseId: 'autopilot.mission_briefing.v1',
              toState: 'green',
            }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        {
          makeUuid: () => 'test-receipt-1',
          nowIso: () => '2026-06-09T23:00:00.000Z',
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    const recordedBody = (await recorded.json()) as Readonly<{
      receipt: PromiseTransitionReceipt
    }>
    const withException = await Effect.runPromise(
      handleOperatorPromiseTransitionApi(
        new Request(
          'https://openagents.com/api/operator/product-promises/transitions',
          {
            body: JSON.stringify({
              exception: {
                approvedByRef: 'owner:openagents',
                expiresAt: '2026-06-16',
                reasonRef: 'exception.launch_window_demo',
              },
              promiseId: 'autopilot.mission_briefing.v1',
              toState: 'green',
            }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        {
          makeUuid: () => 'test-receipt-2',
          nowIso: () => '2026-06-09T23:05:00.000Z',
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )
    const exceptionBody = (await withException.json()) as Readonly<{
      receipt: PromiseTransitionReceipt
    }>
    const publicFeed = await Effect.runPromise(
      handlePublicPromiseTransitionsApi(
        new Request(
          'https://openagents.com/api/public/product-promises/transitions',
        ),
        { store },
      ),
    )
    const feedBody = (await publicFeed.json()) as Readonly<{
      generatedAt: string
      maxStalenessSeconds: number
      receipts: ReadonlyArray<PromiseTransitionReceipt>
      registryGeneratedAt: string
      registryVersion: string
      staleness: Readonly<{
        composition: string
        maxStalenessSeconds: number
        rebuildsOn: ReadonlyArray<string>
      }>
    }>

    expect(denied.status).toBe(401)
    expect(recorded.status).toBe(201)
    expect(recordedBody.receipt).toMatchObject({
      // 2026-07-04.8 owner-directed revenue-refocus demotion
      fromState: 'planned',
      promiseId: 'autopilot.mission_briefing.v1',
      receiptId: 'promise_transition_test-receipt-1',
      result: 'failed',
      toState: 'green',
    })
    expect(exceptionBody.receipt.result).toBe('exception')
    expect(publicFeed.status).toBe(200)
    expect(feedBody.generatedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/)
    expect(feedBody.registryGeneratedAt).toBe(feedBody.generatedAt)
    expect(feedBody.registryVersion).toMatch(/^\d{4}-\d{2}-\d{2}\./)
    expect(feedBody.maxStalenessSeconds).toBe(0)
    expect(feedBody.staleness.composition).toBe('live_at_read')
    expect(feedBody.staleness.maxStalenessSeconds).toBe(0)
    expect(feedBody.staleness.rebuildsOn).toContain(
      'product_promise_registry_changed',
    )
    expect(feedBody.staleness.rebuildsOn).toContain(
      'product_promise_transition_receipt_recorded',
    )
    expect(feedBody.receipts).toHaveLength(2)

    const verified = lastVerifiedAtByPromise(feedBody.receipts)

    expect(verified.get('autopilot.mission_briefing.v1')).toBe(
      '2026-06-09T23:05:00.000Z',
    )
  })

  test('rejects malformed transition requests', async () => {
    const store = memoryStore()
    const response = await Effect.runPromise(
      handleOperatorPromiseTransitionApi(
        new Request(
          'https://openagents.com/api/operator/product-promises/transitions',
          {
            body: JSON.stringify({ promiseId: '', toState: 'sparkly' }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        {
          requireAdminApiToken: () => Promise.resolve(true),
          store,
        },
      ),
    )

    expect(response.status).toBe(400)
    expect(store.receipts).toHaveLength(0)
  })
})
