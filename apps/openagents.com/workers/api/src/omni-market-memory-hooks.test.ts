import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniMarketMemoryHookLifecycleDecisionNotFound,
  OmniMarketMemoryHookValidationError,
  OmniMarketMemoryHookWorkroomNotFound,
  operatorOmniMarketMemoryHookProjection,
  publicOmniMarketMemoryHookProjection,
  recordOmniMarketMemoryHook,
} from './omni-market-memory-hooks'

type WorkKind =
  | 'site'
  | 'coding'
  | 'adjustment'
  | 'existing_project_import'
  | 'business'
  | 'legal_sensitive'

type WorkroomRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind: WorkKind
}>

type LifecycleDecisionRow = Readonly<{
  archived_at: string | null
  id: string
  resulting_state:
    | 'accepted'
    | 'rejected'
    | 'provisionally_accepted'
    | 'reopened'
    | 'revision_requested'
    | 'unavailable'
  work_kind: WorkKind
  workroom_id: string
}>

type MarketMemoryHookRow = Readonly<{
  archived_at: string | null
  authority_boundary: 'evidence_only'
  category:
    | 'route_quality'
    | 'account_reliability'
    | 'repo_convention'
    | 'source_quality'
    | 'module_usefulness'
    | 'marketplace_attribution'
  created_at: string
  economics_ref: string | null
  evidence_ref: string
  id: string
  idempotency_key: string
  lifecycle_decision_id: string
  memory_ref: string
  metadata_json: string
  no_module_promotion: number
  no_payout_mutation: number
  no_public_claim_mutation: number
  no_routing_mutation: number
  outcome_state: 'accepted' | 'rejected'
  public_caveat_ref: string
  route_scorecard_ref: string | null
  source_ref: string
  updated_at: string
  work_kind: WorkKind
  workroom_id: string
}>

class MarketMemoryHookStore {
  hooks: Array<MarketMemoryHookRow> = []
  lifecycleDecisions: Array<LifecycleDecisionRow> = [
    {
      archived_at: null,
      id: 'omni_lifecycle_accept_site_1',
      resulting_state: 'accepted',
      work_kind: 'site',
      workroom_id: 'omni_workroom_site_1',
    },
    {
      archived_at: null,
      id: 'omni_lifecycle_reject_coding_1',
      resulting_state: 'rejected',
      work_kind: 'coding',
      workroom_id: 'omni_workroom_coding_1',
    },
  ]
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

const runtime = {
  makeMemoryHookId: () => 'omni_market_memory_hook_generated',
  nowIso: () => '2026-06-06T03:20:00.000Z',
}

class MarketMemoryHookStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: MarketMemoryHookStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_market_memory_hooks')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.hooks.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM omni_workrooms')) {
      const id = String(this.values[0])
      const row =
        this.store.workrooms.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM omni_workroom_lifecycle_decisions')) {
      const id = String(this.values[0])
      const row =
        this.store.lifecycleDecisions.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO omni_market_memory_hooks')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.hooks.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.hooks.push({
          archived_at: null,
          authority_boundary: this.values[13] as 'evidence_only',
          category: this.values[6] as MarketMemoryHookRow['category'],
          created_at: String(this.values[19]),
          economics_ref: this.values[12] as string | null,
          evidence_ref: String(this.values[8]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          lifecycle_decision_id: String(this.values[3]),
          memory_ref: String(this.values[7]),
          metadata_json: String(this.values[18]),
          no_module_promotion: Number(this.values[17]),
          no_payout_mutation: Number(this.values[15]),
          no_public_claim_mutation: Number(this.values[16]),
          no_routing_mutation: Number(this.values[14]),
          outcome_state: this.values[5] as MarketMemoryHookRow['outcome_state'],
          public_caveat_ref: String(this.values[10]),
          route_scorecard_ref: this.values[11] as string | null,
          source_ref: String(this.values[9]),
          updated_at: String(this.values[20]),
          work_kind: this.values[4] as WorkKind,
          workroom_id: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }
}

const marketMemoryDb = (store: MarketMemoryHookStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new MarketMemoryHookStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const recordHook = (
  store: MarketMemoryHookStore,
  overrides: Partial<Parameters<typeof recordOmniMarketMemoryHook>[1]> = {},
) =>
  Effect.runPromise(
    recordOmniMarketMemoryHook(
      marketMemoryDb(store),
      {
        category: 'route_quality',
        economicsRef: 'economics_ref_site_free_beta',
        evidenceRef: 'evidence_bundle_site_revision_2',
        id: 'omni_market_memory_hook_1',
        idempotencyKey: 'market-memory:site:accept:1',
        lifecycleDecisionId: 'omni_lifecycle_accept_site_1',
        memoryRef: 'memory_route_quality_site_otec_revision_2',
        outcomeState: 'accepted',
        publicCaveatRef: 'market_memory_caveat_evidence_only',
        routeScorecardRef: 'route_scorecard_site_revision_2',
        sourceRef: 'source_exa_research_set_otec',
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni market memory hooks', () => {
  test('records accepted outcome hooks as evidence-only memory', async () => {
    const store = new MarketMemoryHookStore()
    const hook = await recordHook(store)
    const replay = await recordHook(store, {
      memoryRef: 'memory_route_quality_changed',
    })
    const operatorProjection = operatorOmniMarketMemoryHookProjection(hook)
    const publicProjection = publicOmniMarketMemoryHookProjection(hook)

    expect(hook).toStrictEqual(replay)
    expect(store.hooks).toHaveLength(1)
    expect(operatorProjection).toMatchObject({
      authorityBoundary: 'evidence_only',
      category: 'route_quality',
      noModulePromotion: true,
      noPayoutMutation: true,
      noPublicClaimMutation: true,
      noRoutingMutation: true,
      outcomeState: 'accepted',
    })
    expect(publicProjection).toEqual({
      authorityBoundary: 'evidence_only',
      category: 'route_quality',
      memoryRef: 'memory_route_quality_site_otec_revision_2',
      noDirectEffects: true,
      outcomeState: 'accepted',
      publicCaveatRef: 'market_memory_caveat_evidence_only',
      publicReceiptRef:
        'omni_market_memory:omni_workroom_site_1:route_quality:market-memory:site:accept:1',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
  })

  test('records rejected coding outcome hooks', async () => {
    const store = new MarketMemoryHookStore()
    const hook = await recordHook(store, {
      category: 'repo_convention',
      evidenceRef: 'evidence_bundle_coding_failed_tests',
      idempotencyKey: 'market-memory:coding:reject:1',
      lifecycleDecisionId: 'omni_lifecycle_reject_coding_1',
      memoryRef: 'memory_repo_convention_test_command_failed',
      outcomeState: 'rejected',
      routeScorecardRef: undefined,
      sourceRef: 'source_repo_conventions_openagents',
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    })

    expect(hook.category).toBe('repo_convention')
    expect(hook.outcomeState).toBe('rejected')
    expect(hook.routeScorecardRef).toBeNull()
  })

  test('rejects missing workrooms and missing lifecycle decisions', async () => {
    await expect(
      recordHook(new MarketMemoryHookStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookWorkroomNotFound)

    await expect(
      recordHook(new MarketMemoryHookStore(), {
        lifecycleDecisionId: 'omni_lifecycle_missing',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookLifecycleDecisionNotFound)
  })

  test('requires lifecycle decision state and workroom to match', async () => {
    await expect(
      recordHook(new MarketMemoryHookStore(), {
        outcomeState: 'rejected',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)

    await expect(
      recordHook(new MarketMemoryHookStore(), {
        lifecycleDecisionId: 'omni_lifecycle_reject_coding_1',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)
  })

  test('redacts unsafe refs and blocks direct payout, routing, claims, and promotion mutations', async () => {
    await expect(
      recordHook(new MarketMemoryHookStore(), {
        memoryRef: 'provider_payload_raw_run_log',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)

    await expect(
      recordHook(new MarketMemoryHookStore(), {
        metadata: { effect: 'route_override' },
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)

    await expect(
      recordHook(new MarketMemoryHookStore(), {
        evidenceRef: 'eligible_for_payout_after_acceptance',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)

    await expect(
      recordHook(new MarketMemoryHookStore(), {
        sourceRef: 'module_promoted_to_runtime',
      }),
    ).rejects.toBeInstanceOf(OmniMarketMemoryHookValidationError)
  })
})
