import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniWorkroomLifecycleValidationError,
  OmniWorkroomLifecycleWorkroomNotFound,
  customerOmniWorkroomLifecycleProjection,
  operatorOmniWorkroomLifecycleProjection,
  publicOmniWorkroomLifecycleProjection,
  recordOmniWorkroomLifecycleDecision,
} from './omni-workroom-lifecycle'

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
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

class LifecycleStore {
  decisions: Array<DecisionRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

const runtime = {
  makeDecisionId: () => 'omni_workroom_lifecycle_decision_generated',
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
    if (this.query.includes('FROM omni_workroom_lifecycle_decisions')) {
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

const lifecycleDb = (store: LifecycleStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new LifecycleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const recordDecision = (
  store: LifecycleStore,
  overrides: Partial<
    Parameters<typeof recordOmniWorkroomLifecycleDecision>[1]
  > = {},
) =>
  Effect.runPromise(
    recordOmniWorkroomLifecycleDecision(
      lifecycleDb(store),
      {
        actorKind: 'customer',
        artifactRef: 'site_version_ref_2',
        customerSafeExplanationRef: 'customer_explanation_revision_2_accept',
        decisionKind: 'accept',
        id: 'omni_lifecycle_decision_1',
        idempotencyKey: 'omni-lifecycle:site-accept-1',
        receiptRef: 'receipt_customer_accept_revision_2',
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni workroom lifecycle decisions', () => {
  test('records idempotent acceptance with customer-safe projections', async () => {
    const store = new LifecycleStore()
    const decision = await recordDecision(store)
    const replay = await recordDecision(store, {
      customerSafeExplanationRef: 'changed_explanation_ref',
    })
    const publicProjection = publicOmniWorkroomLifecycleProjection(decision)
    const customer = customerOmniWorkroomLifecycleProjection(decision)
    const operator = operatorOmniWorkroomLifecycleProjection(decision)

    expect(replay.customerSafeExplanationRef).toBe(
      'customer_explanation_revision_2_accept',
    )
    expect(publicProjection).toEqual({
      customerSafeExplanationRef: 'customer_explanation_revision_2_accept',
      noSettlementImplication: true,
      receiptRef: 'receipt_customer_accept_revision_2',
      resultingState: 'accepted',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
    expect(customer.artifactRef).toBe('site_version_ref_2')
    expect(operator.actorKind).toBe('customer')
    expect(operator.metadata).toEqual({})
  })

  test('supports Site revision requests through existing feedback refs', async () => {
    const decision = await recordDecision(new LifecycleStore(), {
      customerSafeExplanationRef: 'customer_explanation_revision_requested',
      decisionKind: 'request_revision',
      id: 'omni_lifecycle_decision_revision_1',
      idempotencyKey: 'omni-lifecycle:site-revision-request-1',
      receiptRef: 'receipt_site_revision_requested_1',
      siteRevisionFeedbackRef: 'site_revision_feedback_1',
    })

    expect(decision.resultingState).toBe('revision_requested')
    expect(decision.siteRevisionFeedbackRef).toBe('site_revision_feedback_1')
  })

  test('supports non-Site revision requests through follow-up refs', async () => {
    const decision = await recordDecision(new LifecycleStore(), {
      artifactRef: 'pull_request_ref_42',
      customerSafeExplanationRef: 'customer_explanation_pr_revision_requested',
      decisionKind: 'request_revision',
      followupRequestRef: 'order_fulfillment_feedback_42',
      id: 'omni_lifecycle_decision_coding_revision_1',
      idempotencyKey: 'omni-lifecycle:coding-revision-request-1',
      receiptRef: 'receipt_coding_revision_requested_1',
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    })

    expect(decision.resultingState).toBe('revision_requested')
    expect(decision.followupRequestRef).toBe('order_fulfillment_feedback_42')
  })

  test('covers rejection, provisional acceptance, reopen, and unavailable states', async () => {
    const store = new LifecycleStore()

    await expect(
      recordDecision(store, {
        decisionKind: 'reject',
        id: 'omni_lifecycle_decision_reject',
        idempotencyKey: 'omni-lifecycle:reject',
      }),
    ).resolves.toMatchObject({ resultingState: 'rejected' })

    await expect(
      recordDecision(store, {
        decisionKind: 'provisionally_accept',
        id: 'omni_lifecycle_decision_provisional',
        idempotencyKey: 'omni-lifecycle:provisional',
      }),
    ).resolves.toMatchObject({ resultingState: 'provisionally_accepted' })

    await expect(
      recordDecision(store, {
        decisionKind: 'reopen',
        id: 'omni_lifecycle_decision_reopen',
        idempotencyKey: 'omni-lifecycle:reopen',
      }),
    ).resolves.toMatchObject({ resultingState: 'reopened' })

    await expect(
      recordDecision(store, {
        decisionKind: 'mark_unavailable',
        id: 'omni_lifecycle_decision_unavailable',
        idempotencyKey: 'omni-lifecycle:unavailable',
      }),
    ).resolves.toMatchObject({ resultingState: 'unavailable' })
  })

  test('rejects missing workrooms, missing revision refs, mismatches, and settlement implications', async () => {
    await expect(
      recordDecision(new LifecycleStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomLifecycleWorkroomNotFound)

    await expect(
      recordDecision(new LifecycleStore(), {
        decisionKind: 'request_revision',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomLifecycleValidationError)

    await expect(
      recordDecision(new LifecycleStore(), {
        workKind: 'coding',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomLifecycleValidationError)

    await expect(
      recordDecision(new LifecycleStore(), {
        metadata: { settlement: 'eligible_for_payout' },
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomLifecycleValidationError)

    await expect(
      recordDecision(new LifecycleStore(), {
        receiptRef: 'receipt_settlement_paid_out_1',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomLifecycleValidationError)
  })
})
