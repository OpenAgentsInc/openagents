import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  EARNING_AMOUNT_CLASSES,
  PYLON_MULTI_EARNING_REMAINING_BLOCKERS,
  PYLON_SAFE_PROJECTION_BLOCKER,
  makeInMemoryPylonMultiEarningStore,
  projectPylonMultiEarningNode,
  recordModeEarning,
  settledModeCount,
  settledModeFamilies,
  settledModeFamilyCount,
} from './pylon-multi-earning-node'
import {
  PylonMultiEarningNodeEndpoint,
  handlePylonMultiEarningNodeApi,
  isPylonMultiEarningProjectionEnabled,
} from './pylon-multi-earning-node-routes'

const okRecord = (input: Parameters<typeof recordModeEarning>[0]) => {
  const result = recordModeEarning(input)
  if (!result.ok) {
    throw new Error(result.error.reason)
  }
  return result.record
}

// A store with two settled modes (training + forum tips), one observed-only
// mode (compute, no payment), and one modeled-only mode (labor).
const twoSettledStore = () =>
  makeInMemoryPylonMultiEarningStore([
    okRecord({
      mode: 'training',
      observedCount: 3,
      settledCount: 1,
      settlementReceiptRef:
        'receipt.public.pylon.training.settlement_a',
    }),
    okRecord({
      mode: 'forum_tips',
      observedCount: 5,
      settledCount: 2,
      settlementReceiptRef: 'receipt.public.pylon.forum_tips.settlement_b',
    }),
    okRecord({ mode: 'compute', observedCount: 1 }),
    okRecord({ mode: 'labor', modeledCount: 4 }),
  ])

const request = (suffix = '') =>
  new Request(
    `https://openagents.com${PylonMultiEarningNodeEndpoint}${suffix}`,
  )

describe('pylon multi-earning record (#5527)', () => {
  test('distinguishes all five amount classes', () => {
    expect(EARNING_AMOUNT_CLASSES).toEqual([
      'modeled',
      'observed',
      'pending',
      'paid',
      'settled',
    ])
    const record = okRecord({
      mode: 'compute',
      modeledCount: 1,
      observedCount: 2,
      pendingCount: 3,
      paidCount: 4,
      settledCount: 5,
      settlementReceiptRef: 'receipt.public.pylon.compute.x',
    })
    expect(record.modeledCount).toBe(1)
    expect(record.observedCount).toBe(2)
    expect(record.pendingCount).toBe(3)
    expect(record.paidCount).toBe(4)
    expect(record.settledCount).toBe(5)
  })

  test('a settled count requires a public-safe settlement receipt ref', () => {
    const result = recordModeEarning({ mode: 'training', settledCount: 1 })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.error.reason).toMatch(/settlementReceiptRef/)
    }
  })

  test('a settlement receipt ref requires a settled count > 0', () => {
    const result = recordModeEarning({
      mode: 'training',
      settlementReceiptRef: 'receipt.public.pylon.training.x',
    })
    expect(result.ok).toBe(false)
  })

  test('rejects unsafe refs (no wallet/payment/secret/timestamp material)', () => {
    for (const ref of [
      'lnbc1234',
      'wallet_secret',
      'payout-channel',
      '2026-06-19t12:00:00',
      'customer-jane',
    ]) {
      const result = recordModeEarning({
        mode: 'training',
        settledCount: 1,
        settlementReceiptRef: ref,
      })
      expect(result.ok).toBe(false)
    }
  })

  test('rejects negative or non-integer counts', () => {
    expect(recordModeEarning({ mode: 'training', observedCount: -1 }).ok).toBe(
      false,
    )
    expect(recordModeEarning({ mode: 'training', paidCount: 1.5 }).ok).toBe(
      false,
    )
  })

  test('store is idempotent per mode (one record per mode)', () => {
    const store = makeInMemoryPylonMultiEarningStore([
      okRecord({ mode: 'training', observedCount: 1 }),
      okRecord({ mode: 'training', observedCount: 99 }),
    ])
    expect(store.list()).toHaveLength(1)
    expect(store.list()[0]?.observedCount).toBe(1)
  })

  test('settledModeCount counts only modes with a settled unit', () => {
    expect(settledModeCount(twoSettledStore())).toBe(2)
    expect(
      settledModeCount(
        makeInMemoryPylonMultiEarningStore([
          okRecord({ mode: 'compute', observedCount: 1 }),
        ]),
      ),
    ).toBe(0)
  })

  test('settledModeFamilyCount collapses label-split spellings of one mode', () => {
    // Two settled LABELS, but both are the same earning-mode FAMILY: this is a
    // label-split over-claim and must count as ONE settled mode, not two.
    const split = makeInMemoryPylonMultiEarningStore([
      okRecord({
        mode: 'training',
        settledCount: 1,
        settlementReceiptRef: 'receipt.public.pylon.training.s1',
      }),
      okRecord({
        mode: 'training_v2',
        settledCount: 1,
        settlementReceiptRef: 'receipt.public.pylon.training.s2',
      }),
    ])
    expect(settledModeCount(split)).toBe(2)
    expect(settledModeFamilyCount(split)).toBe(1)
    expect(settledModeFamilies(split)).toEqual(['training'])
  })

  test('settledModeFamilies counts genuinely distinct modes once each', () => {
    expect(settledModeFamilies(twoSettledStore())).toEqual([
      'training',
      'forum_tips',
    ])
    expect(settledModeFamilyCount(twoSettledStore())).toBe(2)
  })
})

describe('pylon multi-earning projection (#5527)', () => {
  test('empty (default) store: zero settled modes, bar not met, stays red', () => {
    const projection = projectPylonMultiEarningNode(
      makeInMemoryPylonMultiEarningStore([]),
    )
    expect(projection.promiseState).toBe('red')
    expect(projection.inert).toBe(true)
    expect(projection.modes).toHaveLength(0)
    expect(projection.settledModeCount).toBe(0)
    expect(projection.settledModesBarMet).toBe(false)
    expect(projection.clearsBlocker).toBe(PYLON_SAFE_PROJECTION_BLOCKER)
    expect(projection.remainingOwnerGatedBlockers).toEqual(
      PYLON_MULTI_EARNING_REMAINING_BLOCKERS,
    )
  })

  test('armed store distinguishes classes and reports the >=2 bar honestly', () => {
    const projection = projectPylonMultiEarningNode(twoSettledStore())
    expect(projection.inert).toBe(true)
    // Even with the >=2 bar reported as met, the surface NEVER flips the
    // promise — settlement/receipt blockers stay owner-gated.
    expect(projection.promiseState).toBe('red')
    expect(projection.settledModeCount).toBe(2)
    expect(projection.settledModesRequiredForGreen).toBe(2)
    expect(projection.settledModesBarMet).toBe(true)
    const labor = projection.modes.find(m => m.mode === 'labor')
    expect(labor?.modeledCount).toBe(4)
    expect(labor?.settledCount).toBe(0)
    expect(labor?.settlementReceiptRef).toBeUndefined()
    const training = projection.modes.find(m => m.mode === 'training')
    expect(training?.settledCount).toBe(1)
    expect(training?.settlementReceiptRef).toBe(
      'receipt.public.pylon.training.settlement_a',
    )
  })

  test('bar measures distinct FAMILIES: label-splitting cannot fake it', () => {
    // Two settled labels that are spellings of ONE mode must NOT meet the >=2
    // bar — the multi-earning claim requires two genuinely distinct modes.
    const projection = projectPylonMultiEarningNode(
      makeInMemoryPylonMultiEarningStore([
        okRecord({
          mode: 'training',
          settledCount: 1,
          settlementReceiptRef: 'receipt.public.pylon.training.s1',
        }),
        okRecord({
          mode: 'training_v2',
          settledCount: 1,
          settlementReceiptRef: 'receipt.public.pylon.training.s2',
        }),
      ]),
    )
    expect(projection.settledModeCount).toBe(2)
    expect(projection.settledModeFamilyCount).toBe(1)
    expect(projection.settledModeFamilies).toEqual(['training'])
    expect(projection.settledModesBarMet).toBe(false)
    expect(projection.promiseState).toBe('red')
  })

  test('bar met only with two distinct settled families', () => {
    const projection = projectPylonMultiEarningNode(twoSettledStore())
    expect(projection.settledModeFamilyCount).toBe(2)
    expect(projection.settledModeFamilies).toEqual(['training', 'forum_tips'])
    expect(projection.settledModesBarMet).toBe(true)
  })

  test('never reports a settled mode the store did not carry', () => {
    const projection = projectPylonMultiEarningNode(
      makeInMemoryPylonMultiEarningStore([
        okRecord({ mode: 'compute', observedCount: 9 }),
      ]),
    )
    expect(projection.settledModeCount).toBe(0)
    expect(projection.modes.every(m => m.settledCount === 0)).toBe(true)
    expect(projection.modes.every(m => m.settlementReceiptRef === undefined)).toBe(
      true,
    )
  })
})

describe('pylon multi-earning route flag (#5527)', () => {
  test('flag defaults OFF', () => {
    expect(isPylonMultiEarningProjectionEnabled(undefined)).toBe(false)
    expect(isPylonMultiEarningProjectionEnabled('false')).toBe(false)
    expect(isPylonMultiEarningProjectionEnabled('0')).toBe(false)
    expect(isPylonMultiEarningProjectionEnabled('on')).toBe(true)
    expect(isPylonMultiEarningProjectionEnabled('TRUE')).toBe(true)
  })
})

describe('pylon multi-earning route (#5527)', () => {
  test('is INERT (empty) when disabled, even with a populated store', async () => {
    const response = await Effect.runPromise(
      handlePylonMultiEarningNodeApi(request(), {
        enabled: false,
        store: twoSettledStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      settledModeCount: number
      modes: ReadonlyArray<unknown>
    }
    expect(response.status).toBe(200)
    expect(response.headers.get('cache-control')).toBe('no-store')
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('red')
    expect(body.settledModeCount).toBe(0)
    expect(body.modes).toHaveLength(0)
  })

  test('surfaces records when armed, still reporting inert/red', async () => {
    const response = await Effect.runPromise(
      handlePylonMultiEarningNodeApi(request(), {
        enabled: true,
        store: twoSettledStore(),
      }),
    )
    const body = (await response.json()) as {
      inert: boolean
      promiseState: string
      settledModeCount: number
      clearsBlocker: string
      remainingOwnerGatedBlockers: ReadonlyArray<string>
    }
    expect(body.inert).toBe(true)
    expect(body.promiseState).toBe('red')
    expect(body.settledModeCount).toBe(2)
    expect(body.clearsBlocker).toBe(PYLON_SAFE_PROJECTION_BLOCKER)
    expect(body.remainingOwnerGatedBlockers).toEqual(
      PYLON_MULTI_EARNING_REMAINING_BLOCKERS,
    )
  })

  test('rejects non-GET', async () => {
    const response = await Effect.runPromise(
      handlePylonMultiEarningNodeApi(
        new Request(
          `https://openagents.com${PylonMultiEarningNodeEndpoint}`,
          { method: 'POST' },
        ),
        { enabled: false },
      ),
    )
    expect(response.status).toBe(405)
  })
})
