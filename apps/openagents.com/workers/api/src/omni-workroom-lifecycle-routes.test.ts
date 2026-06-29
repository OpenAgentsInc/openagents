import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOmniWorkroomLifecycleRoutes } from './omni-workroom-lifecycle-routes'

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

type DecisionRow = Readonly<{
  actor_kind: 'customer' | 'operator' | 'system'
  archived_at: string | null
  artifact_ref: string | null
  created_at: string
  customer_safe_explanation_ref: string
  decision_kind:
    | 'accept'
    | 'reject'
    | 'provisionally_accept'
    | 'reopen'
    | 'request_revision'
    | 'mark_unavailable'
  followup_request_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  no_settlement_implication: number
  receipt_ref: string
  resulting_state:
    | 'accepted'
    | 'rejected'
    | 'provisionally_accepted'
    | 'reopened'
    | 'revision_requested'
    | 'unavailable'
  site_revision_feedback_ref: string | null
  work_kind: WorkKind
  workroom_id: string
}>

class LifecycleStore {
  decisions: Array<DecisionRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

let decisionCounter = 0

const runtime = {
  makeDecisionId: () => {
    decisionCounter = decisionCounter + 1

    return `omni_workroom_lifecycle_decision_generated_${decisionCounter}`
  },
  nowIso: () => '2026-06-06T01:12:00.000Z',
}

class LifecycleStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: LifecycleStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (
      this.query.includes('FROM omni_workroom_lifecycle_decisions') &&
      this.query.includes('idempotency_key = ?')
    ) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.decisions.find(
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
    if (
      this.query.includes(
        'INSERT OR IGNORE INTO omni_workroom_lifecycle_decisions',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.decisions.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.decisions.push({
          actor_kind: this.values[4] as DecisionRow['actor_kind'],
          archived_at: null,
          artifact_ref: this.values[11] as string | null,
          created_at: String(this.values[14]),
          customer_safe_explanation_ref: String(this.values[7]),
          decision_kind: this.values[5] as DecisionRow['decision_kind'],
          followup_request_ref: this.values[10] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[13]),
          no_settlement_implication: Number(this.values[12]),
          receipt_ref: String(this.values[8]),
          resulting_state: this.values[6] as DecisionRow['resulting_state'],
          site_revision_feedback_ref: this.values[9] as string | null,
          work_kind: this.values[3] as DecisionRow['work_kind'],
          workroom_id: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes('FROM omni_workroom_lifecycle_decisions') &&
      this.query.includes('workroom_id = ?')
    ) {
      const workroomId = String(this.values[0])
      const results = this.store.decisions
        .filter(
          item =>
            item.workroom_id === workroomId && item.archived_at === null,
        )
        .sort(
          (left, right) =>
            left.created_at.localeCompare(right.created_at) ||
            left.id.localeCompare(right.id),
        )

      return Promise.resolve({
        results,
        success: true,
      } as unknown as D1Result<T>)
    }

    return Promise.resolve({ results: [] } as unknown as D1Result<T>)
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

const lifecycleDb = (store: LifecycleStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new LifecycleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

type Bindings = Readonly<{ db: D1Database }>

const makeRoutes = (store: LifecycleStore) =>
  makeOmniWorkroomLifecycleRoutes<Bindings>({
    makeDb: env => env.db,
    runtime,
  })

const ctx = {} as ExecutionContext

const dispatch = async (
  store: LifecycleStore,
  request: Request,
): Promise<Response> => {
  const routes = makeRoutes(store)
  const effect = routes.routeOmniWorkroomLifecycleRequest(
    request,
    { db: lifecycleDb(store) },
    ctx,
  )

  if (effect === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(effect)
}

const decisionUrl = (workroomId: string, audience?: string): string => {
  const base = `https://openagents.com/api/omni/workrooms/${workroomId}/lifecycle-decisions`

  return audience === undefined ? base : `${base}?audience=${audience}`
}

const postDecision = (
  store: LifecycleStore,
  workroomId: string,
  body: Readonly<Record<string, unknown>>,
  idempotencyKey?: string,
): Promise<Response> =>
  dispatch(
    store,
    new Request(decisionUrl(workroomId), {
      body: JSON.stringify(body),
      headers:
        idempotencyKey === undefined
          ? { 'content-type': 'application/json' }
          : {
              'content-type': 'application/json',
              'idempotency-key': idempotencyKey,
            },
      method: 'POST',
    }),
  )

const baseAcceptBody = {
  actorKind: 'customer',
  artifactRef: 'site_version_ref_2',
  customerSafeExplanationRef: 'customer_explanation_revision_2_accept',
  decisionKind: 'accept',
  receiptRef: 'receipt_customer_accept_revision_2',
  workKind: 'site',
} as const

describe('Omni workroom lifecycle routes', () => {
  test('POST records an acceptance decision via Idempotency-Key header', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:site-accept-1',
    )
    const json = (await response.json()) as {
      decision: { resultingState: string; idempotencyKey: string }
      directEffectPermitted: boolean
    }

    expect(response.status).toBe(200)
    expect(json.directEffectPermitted).toBe(false)
    expect(json.decision.resultingState).toBe('accepted')
    expect(json.decision.idempotencyKey).toBe('omni-lifecycle:site-accept-1')
    expect(store.decisions).toHaveLength(1)
  })

  test('POST accepts idempotencyKey from the request body', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(store, 'omni_workroom_site_1', {
      ...baseAcceptBody,
      idempotencyKey: 'omni-lifecycle:site-accept-body',
    })

    expect(response.status).toBe(200)
    expect(store.decisions).toHaveLength(1)
    expect(store.decisions[0]?.idempotency_key).toBe(
      'omni-lifecycle:site-accept-body',
    )
  })

  test('POST is idempotent on replay and never duplicates a decision', async () => {
    const store = new LifecycleStore()
    const first = await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:site-accept-replay',
    )
    const replay = await postDecision(
      store,
      'omni_workroom_site_1',
      { ...baseAcceptBody, customerSafeExplanationRef: 'changed_ref' },
      'omni-lifecycle:site-accept-replay',
    )
    const firstJson = (await first.json()) as {
      decision: { id: string; customerSafeExplanationRef: string }
    }
    const replayJson = (await replay.json()) as {
      decision: { id: string; customerSafeExplanationRef: string }
    }

    expect(store.decisions).toHaveLength(1)
    expect(replayJson.decision.id).toBe(firstJson.decision.id)
    expect(replayJson.decision.customerSafeExplanationRef).toBe(
      'customer_explanation_revision_2_accept',
    )
  })

  test('POST rejects a decision without an idempotency key', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
    )
    const json = (await response.json()) as { error: string }

    expect(response.status).toBe(400)
    expect(json.error).toBe('omni_workroom_lifecycle_validation_error')
    expect(store.decisions).toHaveLength(0)
  })

  test('POST returns 404 when the workroom is missing', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(
      store,
      'omni_workroom_missing',
      baseAcceptBody,
      'omni-lifecycle:missing',
    )
    const json = (await response.json()) as { error: string }

    expect(response.status).toBe(404)
    expect(json.error).toBe('omni_workroom_lifecycle_workroom_not_found')
  })

  test('POST records each decision kind to its resulting state', async () => {
    const store = new LifecycleStore()
    const cases = [
      { decisionKind: 'accept', resultingState: 'accepted', workroom: 'omni_workroom_site_1', workKind: 'site' },
      { decisionKind: 'reject', resultingState: 'rejected', workroom: 'omni_workroom_site_1', workKind: 'site' },
      {
        decisionKind: 'provisionally_accept',
        resultingState: 'provisionally_accepted',
        workroom: 'omni_workroom_site_1',
        workKind: 'site',
      },
      { decisionKind: 'reopen', resultingState: 'reopened', workroom: 'omni_workroom_site_1', workKind: 'site' },
      {
        decisionKind: 'mark_unavailable',
        resultingState: 'unavailable',
        workroom: 'omni_workroom_site_1',
        workKind: 'site',
      },
    ] as const

    for (const item of cases) {
      const response = await postDecision(
        store,
        item.workroom,
        {
          actorKind: 'operator',
          customerSafeExplanationRef: `explanation_${item.decisionKind}`,
          decisionKind: item.decisionKind,
          receiptRef: `receipt_${item.decisionKind}`,
          workKind: item.workKind,
        },
        `omni-lifecycle:kind:${item.decisionKind}`,
      )
      const json = (await response.json()) as {
        decision: { resultingState: string }
      }

      expect(response.status).toBe(200)
      expect(json.decision.resultingState).toBe(item.resultingState)
    }
  })

  test('POST records a Site revision request through feedback refs', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(
      store,
      'omni_workroom_site_1',
      {
        actorKind: 'customer',
        customerSafeExplanationRef: 'explanation_site_revision',
        decisionKind: 'request_revision',
        receiptRef: 'receipt_site_revision',
        siteRevisionFeedbackRef: 'site_revision_feedback_1',
        workKind: 'site',
      },
      'omni-lifecycle:site-revision',
    )
    const json = (await response.json()) as {
      decision: { resultingState: string; siteRevisionFeedbackRef: string }
    }

    expect(response.status).toBe(200)
    expect(json.decision.resultingState).toBe('revision_requested')
    expect(json.decision.siteRevisionFeedbackRef).toBe(
      'site_revision_feedback_1',
    )
  })

  test('POST records a non-Site revision request through follow-up refs', async () => {
    const store = new LifecycleStore()
    const response = await postDecision(
      store,
      'omni_workroom_coding_1',
      {
        actorKind: 'customer',
        customerSafeExplanationRef: 'explanation_coding_revision',
        decisionKind: 'request_revision',
        followupRequestRef: 'order_fulfillment_feedback_42',
        receiptRef: 'receipt_coding_revision',
        workKind: 'coding',
      },
      'omni-lifecycle:coding-revision',
    )
    const json = (await response.json()) as {
      decision: { resultingState: string; followupRequestRef: string }
    }

    expect(response.status).toBe(200)
    expect(json.decision.resultingState).toBe('revision_requested')
    expect(json.decision.followupRequestRef).toBe(
      'order_fulfillment_feedback_42',
    )
  })

  test('GET history defaults to the public projection', async () => {
    const store = new LifecycleStore()
    await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:history-public',
    )
    const response = await dispatch(
      store,
      new Request(decisionUrl('omni_workroom_site_1'), { method: 'GET' }),
    )
    const json = (await response.json()) as {
      audience: string
      decisions: ReadonlyArray<Record<string, unknown>>
    }

    expect(response.status).toBe(200)
    expect(json.audience).toBe('public')
    expect(json.decisions).toHaveLength(1)
    const decision = json.decisions[0] ?? {}
    expect(decision.resultingState).toBe('accepted')
    expect(decision.decisionKind).toBeUndefined()
    expect(decision.artifactRef).toBeUndefined()
    expect(decision.actorKind).toBeUndefined()
    expect(decision.idempotencyKey).toBeUndefined()
  })

  test('GET history customer projection exposes decision detail but not operator audit', async () => {
    const store = new LifecycleStore()
    await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:history-customer',
    )
    const response = await dispatch(
      store,
      new Request(decisionUrl('omni_workroom_site_1', 'customer'), {
        method: 'GET',
      }),
    )
    const json = (await response.json()) as {
      audience: string
      decisions: ReadonlyArray<Record<string, unknown>>
    }

    expect(json.audience).toBe('customer')
    const decision = json.decisions[0] ?? {}
    expect(decision.decisionKind).toBe('accept')
    expect(decision.artifactRef).toBe('site_version_ref_2')
    expect(decision.actorKind).toBeUndefined()
    expect(decision.idempotencyKey).toBeUndefined()
    expect(decision.metadata).toBeUndefined()
  })

  test('GET history operator projection exposes the full audit record', async () => {
    const store = new LifecycleStore()
    await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:history-operator',
    )
    const response = await dispatch(
      store,
      new Request(decisionUrl('omni_workroom_site_1', 'operator'), {
        method: 'GET',
      }),
    )
    const json = (await response.json()) as {
      audience: string
      decisions: ReadonlyArray<Record<string, unknown>>
    }

    expect(json.audience).toBe('operator')
    const decision = json.decisions[0] ?? {}
    expect(decision.actorKind).toBe('customer')
    expect(decision.idempotencyKey).toBe('omni-lifecycle:history-operator')
    expect(decision.metadata).toEqual({})
    expect(decision.id).toBeTruthy()
  })

  test('GET history is scoped to the requested workroom', async () => {
    const store = new LifecycleStore()
    await postDecision(
      store,
      'omni_workroom_site_1',
      baseAcceptBody,
      'omni-lifecycle:scope-site',
    )
    await postDecision(
      store,
      'omni_workroom_coding_1',
      {
        actorKind: 'operator',
        customerSafeExplanationRef: 'explanation_coding_accept',
        decisionKind: 'accept',
        receiptRef: 'receipt_coding_accept',
        workKind: 'coding',
      },
      'omni-lifecycle:scope-coding',
    )
    const response = await dispatch(
      store,
      new Request(decisionUrl('omni_workroom_coding_1', 'operator'), {
        method: 'GET',
      }),
    )
    const json = (await response.json()) as {
      decisions: ReadonlyArray<{ workroomId: string }>
    }

    expect(json.decisions).toHaveLength(1)
    expect(json.decisions[0]?.workroomId).toBe('omni_workroom_coding_1')
  })

  test('unsupported methods return 405', async () => {
    const store = new LifecycleStore()
    const response = await dispatch(
      store,
      new Request(decisionUrl('omni_workroom_site_1'), { method: 'DELETE' }),
    )

    expect(response.status).toBe(405)
  })

  test('non-matching paths return undefined for chaining', () => {
    const store = new LifecycleStore()
    const routes = makeRoutes(store)
    const effect = routes.routeOmniWorkroomLifecycleRequest(
      new Request('https://openagents.com/api/omni/workrooms', {
        method: 'GET',
      }),
      { db: lifecycleDb(store) },
      ctx,
    )

    expect(effect).toBeUndefined()
  })
})
