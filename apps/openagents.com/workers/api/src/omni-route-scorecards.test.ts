import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniRouteScorecardValidationError,
  OmniRouteScorecardWorkroomNotFound,
  customerOmniRouteScorecardProjection,
  operatorOmniRouteScorecardProjection,
  publicOmniRouteScorecardProjection,
  recordOmniRouteScorecard,
} from './omni-route-scorecards'

type WorkroomRow = Readonly<{
  archived_at: string | null
  id: string
  work_kind:
    | 'site'
    | 'coding'
    | 'adjustment'
    | 'existing_project_import'
    | 'business'
    | 'legal_sensitive'
}>

type ScorecardRow = Readonly<{
  archived_at: string | null
  cost_cents: number
  created_at: string
  decision_reason_refs_json: string
  id: string
  idempotency_key: string
  latency_ms: number
  metadata_json: string
  observed_result_kind: 'success' | 'partial' | 'failure' | 'unavailable'
  observed_result_ref: string
  post_closeout_score: number | null
  privacy_tier: 'public' | 'customer' | 'team' | 'operator' | 'private'
  public_caveat_ref: string
  rejected_candidates_json: string
  selected_account_ref: string | null
  selected_model_ref: string
  selected_provider_ref: string
  selected_route_ref: string
  selected_runtime_ref: string
  trust_tier: 'verified' | 'reviewed' | 'unverified' | 'blocked'
  updated_at: string
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

class RouteScorecardStore {
  scorecards: Array<ScorecardRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

const runtime = {
  makeScorecardId: () => 'omni_route_scorecard_generated',
  nowIso: () => '2026-06-06T02:05:00.000Z',
}

class RouteScorecardStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: RouteScorecardStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_route_scorecards')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.scorecards.find(
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

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO omni_route_scorecards')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.scorecards.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.scorecards.push({
          archived_at: null,
          cost_cents: Number(this.values[14]),
          created_at: String(this.values[20]),
          decision_reason_refs_json: String(this.values[10]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          latency_ms: Number(this.values[15]),
          metadata_json: String(this.values[19]),
          observed_result_kind: this
            .values[11] as ScorecardRow['observed_result_kind'],
          observed_result_ref: String(this.values[12]),
          post_closeout_score: this.values[13] as number | null,
          privacy_tier: this.values[16] as ScorecardRow['privacy_tier'],
          public_caveat_ref: String(this.values[18]),
          rejected_candidates_json: String(this.values[9]),
          selected_account_ref: this.values[6] as string | null,
          selected_model_ref: String(this.values[7]),
          selected_provider_ref: String(this.values[5]),
          selected_route_ref: String(this.values[4]),
          selected_runtime_ref: String(this.values[8]),
          trust_tier: this.values[17] as ScorecardRow['trust_tier'],
          updated_at: String(this.values[21]),
          work_kind: this.values[3] as ScorecardRow['work_kind'],
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

const routeScorecardDb = (store: RouteScorecardStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new RouteScorecardStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const recordScorecard = (
  store: RouteScorecardStore,
  overrides: Partial<Parameters<typeof recordOmniRouteScorecard>[1]> = {},
) =>
  Effect.runPromise(
    recordOmniRouteScorecard(
      routeScorecardDb(store),
      {
        costCents: 1200,
        decisionReasonRefs: ['route_reason_cost_privacy_balanced'],
        id: 'omni_route_scorecard_1',
        idempotencyKey: 'omni-route-scorecard:site-1',
        latencyMs: 30000,
        observedResultKind: 'success',
        observedResultRef: 'route_observed_result_site_revision_2',
        postCloseoutScore: 92,
        privacyTier: 'customer',
        publicCaveatRef: 'route_public_caveat_customer_safe',
        rejectedCandidates: [
          {
            candidateRef: 'route_candidate_model_fast_1',
            reasonKind: 'quality',
            reasonRef: 'route_reason_quality_lower',
          },
        ],
        selectedAccountRef: 'account_pool_chatgpt_fleet_1',
        selectedModelRef: 'model_gpt_5_codex',
        selectedProviderRef: 'provider_openai_codex',
        selectedRouteRef: 'route_shc_codex_account_pool_1',
        selectedRuntimeRef: 'runtime_shc_archlinux_1',
        trustTier: 'reviewed',
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni route scorecards', () => {
  test('records idempotent route scorecards with projection splits', async () => {
    const store = new RouteScorecardStore()
    const scorecard = await recordScorecard(store)
    const replay = await recordScorecard(store, {
      postCloseoutScore: 1,
    })
    const publicProjection = publicOmniRouteScorecardProjection(scorecard)
    const customer = customerOmniRouteScorecardProjection(scorecard)
    const operator = operatorOmniRouteScorecardProjection(scorecard)

    expect(replay.postCloseoutScore).toBe(92)
    expect(publicProjection).toEqual({
      observedResultKind: 'success',
      observedResultRef: 'route_observed_result_site_revision_2',
      postCloseoutScore: 92,
      publicCaveatRef: 'route_public_caveat_customer_safe',
      selectedModelRef: 'model_gpt_5_codex',
      selectedRuntimeRef: 'runtime_shc_archlinux_1',
      trustTier: 'reviewed',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
    expect(customer.selectedRouteRef).toBe('route_shc_codex_account_pool_1')
    expect(customer).not.toHaveProperty('selectedAccountRef')
    expect(operator.selectedAccountRef).toBe('account_pool_chatgpt_fleet_1')
    expect(operator.rejectedCandidates).toHaveLength(1)
  })

  test('supports non-Sites coding workrooms', async () => {
    const scorecard = await recordScorecard(new RouteScorecardStore(), {
      id: 'omni_route_scorecard_coding_1',
      idempotencyKey: 'omni-route-scorecard:coding-1',
      observedResultKind: 'partial',
      observedResultRef: 'route_observed_result_pr_42',
      selectedRuntimeRef: 'runtime_probe_coding_1',
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    })

    expect(scorecard.workKind).toBe('coding')
    expect(scorecard.observedResultKind).toBe('partial')
  })

  test('rejects missing workrooms, mismatched work kinds, unsafe refs, and bad math', async () => {
    await expect(
      recordScorecard(new RouteScorecardStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardWorkroomNotFound)

    await expect(
      recordScorecard(new RouteScorecardStore(), {
        workKind: 'coding',
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardValidationError)

    await expect(
      recordScorecard(new RouteScorecardStore(), {
        selectedAccountRef: 'provider_account_access_token_abc',
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardValidationError)

    await expect(
      recordScorecard(new RouteScorecardStore(), {
        metadata: { rawRunLog: 'raw_run_log_private' },
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardValidationError)

    await expect(
      recordScorecard(new RouteScorecardStore(), {
        postCloseoutScore: 101,
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardValidationError)

    await expect(
      recordScorecard(new RouteScorecardStore(), {
        latencyMs: -1,
      }),
    ).rejects.toBeInstanceOf(OmniRouteScorecardValidationError)
  })
})
