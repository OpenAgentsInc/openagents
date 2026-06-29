import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type PromiseTransitionReceipt,
  type PromiseTransitionReceiptStore,
} from './promise-transition-receipt-routes'
import {
  buildPromiseAuditProjection,
  handlePublicPromiseAuditApi,
} from './promise-transition-audit-routes'
import { publicProductPromisesDocument } from './product-promises'

const greenPromiseId = (): string => {
  const document = publicProductPromisesDocument()
  const green = document.promises.find(promise => promise.state === 'green')
  if (green === undefined) {
    throw new Error('expected at least one green promise in the registry')
  }
  return green.promiseId
}

const receipt = (
  overrides: Partial<PromiseTransitionReceipt> &
    Pick<PromiseTransitionReceipt, 'promiseId'>,
): PromiseTransitionReceipt => ({
  checkedAt: '2026-06-18T00:00:00.000Z',
  checks: [],
  evidenceRefs: [],
  exception: null,
  fromState: 'yellow',
  receiptId: 'promise_transition_test',
  registryVersion: '2026-06-18.0',
  result: 'passed',
  toState: 'green',
  ...overrides,
})

const memoryStore = (
  receipts: ReadonlyArray<PromiseTransitionReceipt>,
): PromiseTransitionReceiptStore => ({
  createReceipt: async () => {},
  listReceipts: async limit => receipts.slice(0, limit),
})

describe('promise claim-upgrade audit projection', () => {
  test('joins receipts against the live registry and tallies green backing', () => {
    const promiseId = greenPromiseId()
    const projection = buildPromiseAuditProjection({
      generatedAt: '2026-06-20T00:00:00.000Z',
      receipts: [
        receipt({
          fromState: 'green',
          promiseId,
          receiptId: 'promise_transition_a',
          result: 'exception',
          exception: {
            approvedByRef: 'owner:openagents',
            expiresAt: '2026-07-01',
            reasonRef: 'exception.already_applied_green',
          },
          toState: 'green',
        }),
      ],
    })

    expect(projection.kind).toBe('product_promise_claim_upgrade_audit')
    expect(projection.registryVersion).toMatch(/^\d{4}-\d{2}-\d{2}/)

    const row = projection.rows.find(r => r.promiseId === promiseId)
    expect(row).toBeDefined()
    expect(row?.currentState).toBe('green')
    expect(row?.greenReceiptBacked).toBe(true)
    expect(row?.greenFlipReceiptCount).toBe(1)
    expect(row?.lastVerifiedAt).toBe('2026-06-18T00:00:00.000Z')
    expect(row?.transitionReceipts[0]?.alreadyApplied).toBe(true)
    expect(row?.transitionReceipts[0]?.ownerSignoff?.approvedByRef).toBe(
      'owner:openagents',
    )

    expect(projection.summary.greenPromisesReceiptBacked).toBeGreaterThanOrEqual(
      1,
    )
    expect(projection.summary.ownerSignedExceptionCount).toBe(1)
    expect(projection.summary.greenPromisesWithoutReceipt).not.toContain(
      promiseId,
    )
  })

  test('reports green promises with no recorded green-flip receipt', () => {
    const projection = buildPromiseAuditProjection({
      generatedAt: '2026-06-20T00:00:00.000Z',
      receipts: [],
    })

    const greenCount = projection.summary.greenPromiseCount
    expect(greenCount).toBeGreaterThan(0)
    // With no receipts, every green promise trails the registry.
    expect(projection.summary.greenPromisesReceiptBacked).toBe(0)
    expect(projection.summary.greenPromisesWithoutReceipt).toHaveLength(
      greenCount,
    )
  })

  test('a failed green receipt does not count as backing', () => {
    const promiseId = greenPromiseId()
    const projection = buildPromiseAuditProjection({
      generatedAt: '2026-06-20T00:00:00.000Z',
      receipts: [
        receipt({ promiseId, result: 'failed', receiptId: 'promise_transition_f' }),
      ],
    })

    const row = projection.rows.find(r => r.promiseId === promiseId)
    expect(row?.greenReceiptBacked).toBe(false)
    expect(projection.summary.greenPromisesWithoutReceipt).toContain(promiseId)
    expect(projection.summary.failedReceiptCount).toBe(1)
  })

  test('greenOnly filter narrows rows but summary stays registry-wide', () => {
    const projection = buildPromiseAuditProjection({
      filter: { greenOnly: true },
      generatedAt: '2026-06-20T00:00:00.000Z',
      receipts: [],
    })

    expect(projection.rows.every(row => row.currentState === 'green')).toBe(true)
    // Summary is over the full registry, so it counts non-green promises too.
    expect(projection.summary.promiseCount).toBeGreaterThan(
      projection.rows.length,
    )
  })

  test('promiseId filter returns only the requested promise', () => {
    const promiseId = greenPromiseId()
    const projection = buildPromiseAuditProjection({
      filter: { promiseId },
      generatedAt: '2026-06-20T00:00:00.000Z',
      receipts: [],
    })

    expect(projection.rows).toHaveLength(1)
    expect(projection.rows[0]?.promiseId).toBe(promiseId)
  })

  test('http handler serves the projection with query filters and no-store', async () => {
    const response = await Effect.runPromise(
      handlePublicPromiseAuditApi(
        new Request(
          'https://openagents.com/api/public/product-promises/audit?greenOnly=true',
        ),
        {
          nowIso: () => '2026-06-20T00:00:00.000Z',
          store: memoryStore([]),
        },
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')

    const body = (await response.json()) as Readonly<{
      filter: Readonly<{ greenOnly?: boolean }>
      publicSafe: boolean
      rows: ReadonlyArray<Readonly<{ currentState: string }>>
    }>
    expect(body.publicSafe).toBe(true)
    expect(body.filter.greenOnly).toBe(true)
    expect(body.rows.every(row => row.currentState === 'green')).toBe(true)
  })

  test('rejects non-GET methods', async () => {
    const response = await Effect.runPromise(
      handlePublicPromiseAuditApi(
        new Request(
          'https://openagents.com/api/public/product-promises/audit',
          { method: 'POST' },
        ),
        { store: memoryStore([]) },
      ),
    )

    expect(response.status).toBe(405)
  })
})
