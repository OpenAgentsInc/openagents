import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  hostedMdkDirectPayoutDisabledGate,
  projectMdkPayoutModeGate,
} from './mdk-payout-mode-gate'
import {
  createPartnerPayoutEligibility,
  type PartnerPayoutAsset,
  type PartnerPayoutRole,
  type PartnerPayoutState,
} from './partner-payout-ledger'
import { makePartnerPayoutLedgerRoutes } from './partner-payout-ledger-routes'
import { makeD1PartnerPayoutReceiptStore } from './partner-payout-receipts'

// ---------------------------------------------------------------------------
// D1 fake (reused from partner-payout-ledger.test.ts)
// ---------------------------------------------------------------------------

type StoredPayoutEntry = Readonly<{
  amount: number
  archived_at: string | null
  asset: PartnerPayoutAsset
  beneficiary_user_id: string | null
  caveat_refs_json: string
  created_at: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  partner_ref: string
  partner_role: PartnerPayoutRole
  partner_user_id: string
  payout_ref: string
  period_key: string
  policy_refs_json: string
  previous_entry_id: string | null
  qualifying_amount: number
  qualifying_event_kind: string
  qualifying_event_ref: string
  reversal_of_entry_id: string | null
  state: PartnerPayoutState
  state_reason_ref: string | null
}>

class PayoutStore {
  rows: Array<StoredPayoutEntry> = []
}

class PayoutStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PayoutStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const row =
        this.store.rows.find(
          entry =>
            entry.idempotency_key === String(this.values[0]) &&
            entry.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE payout_ref = ?')) {
      const row =
        this.store.rows
          .filter(
            entry =>
              entry.payout_ref === String(this.values[0]) &&
              entry.archived_at === null,
          )
          .sort((left, right) =>
            right.created_at === left.created_at
              ? right.id.localeCompare(left.id)
              : right.created_at.localeCompare(left.created_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('COUNT(*) AS payout_count')) {
      const [partnerUserId, periodKey, asset] = this.values
      const rows = this.store.rows.filter(
        entry =>
          entry.partner_user_id === partnerUserId &&
          entry.period_key === periodKey &&
          entry.asset === asset &&
          entry.amount > 0 &&
          entry.archived_at === null &&
          ['eligible', 'approved', 'dispatched', 'settled'].includes(
            entry.state,
          ),
      )

      return Promise.resolve({
        payout_amount: rows.reduce((total, row) => total + row.amount, 0),
        payout_count: rows.length,
      } as T)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO partner_payout_ledger_entries')) {
      this.store.rows.push({
        amount: Number(this.values[11]),
        archived_at: null,
        asset: this.values[7] as PartnerPayoutAsset,
        beneficiary_user_id: this.values[6] as string | null,
        caveat_refs_json: String(this.values[19]),
        created_at: String(this.values[20]),
        evidence_refs_json: String(this.values[17]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[2]),
        partner_ref: String(this.values[5]),
        partner_role: this.values[3] as PartnerPayoutRole,
        partner_user_id: String(this.values[4]),
        payout_ref: String(this.values[1]),
        period_key: String(this.values[12]),
        policy_refs_json: String(this.values[18]),
        previous_entry_id: this.values[15] as string | null,
        qualifying_amount: Number(this.values[10]),
        qualifying_event_kind: String(this.values[9]),
        qualifying_event_ref: String(this.values[8]),
        reversal_of_entry_id: this.values[16] as string | null,
        state: this.values[13] as PartnerPayoutState,
        state_reason_ref: this.values[14] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('FROM partner_payout_ledger_entries') &&
      this.query.includes("WHERE state = 'settled'")
    ) {
      const needle = String(this.values[0] ?? '').replaceAll('%', '')
      const rows = this.store.rows
        .filter(
          entry =>
            entry.state === 'settled' &&
            entry.archived_at === null &&
            entry.evidence_refs_json.includes(needle),
        )
        .sort((left, right) =>
          right.created_at === left.created_at
            ? right.id.localeCompare(left.id)
            : right.created_at.localeCompare(left.created_at),
        )

      return Promise.resolve({
        results: rows,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const payoutDb = (store: PayoutStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new PayoutStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

// ---------------------------------------------------------------------------
// Route harness
// ---------------------------------------------------------------------------

type TestEnv = Readonly<{ OPENAGENTS_DB: D1Database }>

let isoCounter = 0
const nowIso = (): string =>
  `2026-06-10T10:0${Math.min(9, isoCounter++)}:00.000Z`

const makeRoutes = (
  store: PayoutStore,
  admin = true,
  overrides: Partial<{
    dispatch: (input: {
      amountSats: number
      idempotencyKey: string
      payoutRef: string
    }) => Promise<{ receiptRef: string }>
    livePayoutClaimAllowed: boolean
  }> = {},
) =>
  makePartnerPayoutLedgerRoutes<TestEnv>({
    dispatchDependencies: {
      adapter: {
        adapterKind: 'test',
        dispatch:
          overrides.dispatch ??
          (async () => ({ receiptRef: 'receipt.partner_payout.test' })),
      },
      nowIso,
      readReadiness: async () =>
        overrides.livePayoutClaimAllowed === true
          ? projectMdkPayoutModeGate({
              hostedFundedKeyVerified: true,
              hostedProgrammaticPayoutsEnabled: true,
              requestedMode: 'hosted_mdk_direct_payout',
            })
          : hostedMdkDirectPayoutDisabledGate(),
    },
    nowIso,
    requireAdminApiToken: () => Promise.resolve(admin),
  })

const env = (store: PayoutStore): TestEnv => ({ OPENAGENTS_DB: payoutDb(store) })

const ctx = {} as ExecutionContext

const runRequest = (
  routes: ReturnType<typeof makePartnerPayoutLedgerRoutes<TestEnv>>,
  store: PayoutStore,
  request: Request,
): Promise<Response> => {
  const effect = routes.routePartnerPayoutLedgerRequest(request, env(store), ctx)

  if (effect === undefined) {
    throw new Error(`Route did not match: ${request.url}`)
  }

  return Effect.runPromise(effect)
}

const TRANSITIONS_URL =
  'https://openagents.com/api/operator/partners/payout-ledger/partner_payout_ref_1/transitions'
const DISPATCH_URL =
  'https://openagents.com/api/operator/partners/payout-ledger/partner_payout_ref_1/dispatch'
const PROJECTION_URL =
  'https://openagents.com/api/operator/partners/payout-ledger/partner_payout_ref_1'

const baseDesignPartner = {
  asset: 'usd' as const,
  beneficiaryUserId: 'github:client',
  id: 'partner_payout_entry_1',
  idempotencyKey: 'partner-payout:design:eligible:1',
  nowIso: '2026-06-10T09:00:00.000Z',
  partnerRef: 'design_partner_acme',
  partnerRole: 'design_partner' as const,
  partnerUserId: 'github:acme_agency',
  payoutRef: 'partner_payout_ref_1',
  periodKey: '2026-06',
  qualifyingAmount: 10000,
  qualifyingEventKind: 'design_partner_engagement',
  qualifyingEventRef: 'partner_event_engagement_1',
}

const transitionRequest = (body: unknown): Request =>
  new Request(TRANSITIONS_URL, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const seedEligible = async (store: PayoutStore): Promise<void> => {
  await createPartnerPayoutEligibility(payoutDb(store), baseDesignPartner)
}

const seedEligibleSats = async (store: PayoutStore): Promise<void> => {
  await createPartnerPayoutEligibility(payoutDb(store), {
    ...baseDesignPartner,
    asset: 'sats',
    idempotencyKey: 'partner-payout:sats:eligible:1',
    qualifyingAmount: 2500,
  })
}

describe('Partner payout ledger routes — transitions', () => {
  test('approves dispatch and projects the new state', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:1',
      }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      payout: { state: string; payoutRef: string }
    }
    expect(json.payout.state).toBe('approved')
    expect(json.payout.payoutRef).toBe('partner_payout_ref_1')
  })

  test('walks approved -> dispatched -> settled with evidence', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:2',
      }),
    )
    await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'mark_dispatched',
        idempotencyKey: 'partner-payout:dispatch:2',
      }),
    )
    const settled = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'mark_settled',
        evidenceRefs: ['settlement_evidence.public.partner_payout.test'],
        idempotencyKey: 'partner-payout:settle:2',
      }),
    )

    expect(settled.status).toBe(200)
    const json = (await settled.json()) as { payout: { state: string } }
    expect(json.payout.state).toBe('settled')
  })

  test('mark_settled without evidence is a 409 invalid transition', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:3',
      }),
    )
    await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'mark_dispatched',
        idempotencyKey: 'partner-payout:dispatch:3',
      }),
    )
    const response = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'mark_settled',
        idempotencyKey: 'partner-payout:settle-no-evidence:3',
      }),
    )

    expect(response.status).toBe(409)
    const json = (await response.json()) as { error: string }
    expect(json.error).toBe('partner_payout_invalid_transition')
  })

  test('illegal transition from eligible is a 409', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'mark_settled',
        evidenceRefs: ['settlement_evidence.public.partner_payout.test'],
        idempotencyKey: 'partner-payout:bad-settle:4',
      }),
    )

    expect(response.status).toBe(409)
  })

  test('transition against an unknown ref is a 409', async () => {
    const store = new PayoutStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:unknown:5',
      }),
    )

    expect(response.status).toBe(409)
  })

  test('malformed body is a 400 bad request', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      transitionRequest({ action: 'not_a_real_action', idempotencyKey: 'x' }),
    )

    expect(response.status).toBe(400)
    const json = (await response.json()) as { error: string }
    expect(json.error).toBe('bad_request')
  })

  test('transitions are idempotent and append no duplicate rows', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const first = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:idem',
      }),
    )
    const repeat = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:idem',
      }),
    )

    const firstJson = (await first.json()) as {
      payout: { currentEntryId: string }
    }
    const repeatJson = (await repeat.json()) as {
      payout: { currentEntryId: string }
    }
    expect(repeatJson.payout.currentEntryId).toBe(
      firstJson.payout.currentEntryId,
    )
    expect(store.rows.map(row => row.state)).toEqual(['eligible', 'approved'])
  })
})

describe('Partner payout ledger routes — projection', () => {
  test('GET returns the current projection with authority boundary', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      new Request(PROJECTION_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(200)
    const json = (await response.json()) as {
      payout: {
        amount: number
        authorityBoundary: string
        partnerRole: string
        state: string
      }
    }
    expect(json.payout).toMatchObject({
      amount: 2000,
      partnerRole: 'design_partner',
      state: 'eligible',
    })
    expect(json.payout.authorityBoundary).toContain('not spendable')
  })

  test('GET for an unknown ref is a 404', async () => {
    const store = new PayoutStore()
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      new Request(PROJECTION_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(404)
    const json = (await response.json()) as { error: string }
    expect(json.error).toBe('partner_payout_not_found')
  })
})

describe('Partner payout ledger routes — dispatch', () => {
  test('requires admin token for operator dispatch', async () => {
    const store = new PayoutStore()
    await seedEligibleSats(store)
    const routes = makeRoutes(store, false)

    const response = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'POST' }),
    )

    expect(response.status).toBe(401)
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('refuses non-sats partner rows before adapter call', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const dispatchCalls: Array<unknown> = []
    const routes = makeRoutes(store, true, {
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.partner_payout.should_not_dispatch' }
      },
      livePayoutClaimAllowed: true,
    })

    const response = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'POST' }),
    )
    const body = (await response.json()) as {
      dispatch: { _tag: string; reasonRef?: string; state?: string }
    }

    expect(response.status).toBe(200)
    expect(body.dispatch).toMatchObject({
      _tag: 'refused',
      reasonRef:
        'reason.public.partner_payout.non_sats_asset_not_withdrawable_bitcoin',
      state: 'eligible',
    })
    expect(dispatchCalls).toHaveLength(0)
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('refuses while owner-armed payout mode is disabled', async () => {
    const store = new PayoutStore()
    await seedEligibleSats(store)
    const dispatchCalls: Array<unknown> = []
    const routes = makeRoutes(store, true, {
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.partner_payout.should_not_dispatch' }
      },
      livePayoutClaimAllowed: false,
    })

    const response = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'POST' }),
    )
    const body = (await response.json()) as {
      dispatch: { _tag: string; reasonRef?: string; state?: string }
    }

    expect(response.status).toBe(200)
    expect(body.dispatch).toMatchObject({
      _tag: 'refused',
      reasonRef: 'reason.public.partner_payout.payout_target_not_ready',
      state: 'eligible',
    })
    expect(dispatchCalls).toHaveLength(0)
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('settles through an armed adapter and exposes receipt readback', async () => {
    const store = new PayoutStore()
    await seedEligibleSats(store)
    const dispatchCalls: Array<{
      amountSats: number
      idempotencyKey: string
      payoutRef: string
    }> = []
    const routes = makeRoutes(store, true, {
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.partner_payout.hosted_mdk.abc123' }
      },
      livePayoutClaimAllowed: true,
    })

    const response = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'POST' }),
    )
    const body = (await response.json()) as {
      dispatch: {
        _tag: string
        amountSats: number
        payoutRef: string
        receiptRef: string
        state: string
      }
    }

    expect(response.status).toBe(200)
    expect(body.dispatch).toMatchObject({
      _tag: 'settled',
      amountSats: 500,
      payoutRef: 'partner_payout_ref_1',
      receiptRef: 'receipt.partner_payout.hosted_mdk.abc123',
      state: 'settled',
    })
    expect(dispatchCalls).toEqual([
      {
        amountSats: 500,
        idempotencyKey: 'partner_payout.adapter.partner_payout_ref_1',
        payoutRef: 'partner_payout_ref_1',
      },
    ])
    expect(store.rows.map(row => row.state)).toEqual([
      'eligible',
      'approved',
      'dispatched',
      'settled',
    ])
    expect(store.rows.at(-1)?.evidence_refs_json).toContain(
      'receipt.partner_payout.hosted_mdk.abc123',
    )
    expect(store.rows.at(-1)?.evidence_refs_json).toContain(
      'evidence.partner_payout.adapter.test',
    )

    const receiptStore = makeD1PartnerPayoutReceiptStore(payoutDb(store))
    const receipt = await receiptStore.readPartnerPayoutReceipt(
      'receipt.partner_payout.hosted_mdk.abc123',
      '2026-06-10T10:05:00.000Z',
    )

    expect(receipt).toMatchObject({
      amount: 500,
      asset: 'sats',
      receiptRef: 'receipt.partner_payout.hosted_mdk.abc123',
      resolution: {
        settlementRail: 'hosted_mdk',
        state: 'settled',
        status: 'ok',
      },
    })
  })

  test('already-settled dispatch does not call adapter again', async () => {
    const store = new PayoutStore()
    await seedEligibleSats(store)
    const dispatchCalls: Array<unknown> = []
    const routes = makeRoutes(store, true, {
      dispatch: async input => {
        dispatchCalls.push(input)
        return { receiptRef: 'receipt.partner_payout.hosted_mdk.idem' }
      },
      livePayoutClaimAllowed: true,
    })

    await runRequest(routes, store, new Request(DISPATCH_URL, { method: 'POST' }))
    const repeat = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'POST' }),
    )
    const body = (await repeat.json()) as {
      dispatch: { _tag: string; state: string }
    }

    expect(body.dispatch).toMatchObject({
      _tag: 'already_settled',
      state: 'settled',
    })
    expect(dispatchCalls).toHaveLength(1)
    expect(store.rows.map(row => row.state)).toEqual([
      'eligible',
      'approved',
      'dispatched',
      'settled',
    ])
  })
})

describe('Partner payout ledger routes — auth + matching', () => {
  test('transitions are rejected with 401 when admin token is absent', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store, false)

    const response = await runRequest(
      routes,
      store,
      transitionRequest({
        action: 'approve_dispatch',
        idempotencyKey: 'partner-payout:approve:noauth',
      }),
    )

    expect(response.status).toBe(401)
    // No state change leaked past the auth gate.
    expect(store.rows.map(row => row.state)).toEqual(['eligible'])
  })

  test('projection is rejected with 401 when admin token is absent', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store, false)

    const response = await runRequest(
      routes,
      store,
      new Request(PROJECTION_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(401)
  })

  test('non-matching paths return undefined (route passthrough)', () => {
    const store = new PayoutStore()
    const routes = makeRoutes(store)

    const effect = routes.routePartnerPayoutLedgerRequest(
      new Request('https://openagents.com/api/operator/something-else', {
        method: 'GET',
      }),
      env(store),
      ctx,
    )

    expect(effect).toBeUndefined()
  })

  test('wrong method on transitions path is 405', async () => {
    const store = new PayoutStore()
    await seedEligible(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      new Request(TRANSITIONS_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(405)
  })

  test('wrong method on dispatch path is 405', async () => {
    const store = new PayoutStore()
    await seedEligibleSats(store)
    const routes = makeRoutes(store)

    const response = await runRequest(
      routes,
      store,
      new Request(DISPATCH_URL, { method: 'GET' }),
    )

    expect(response.status).toBe(405)
  })
})
