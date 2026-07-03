import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  type BusinessFulfillmentLoopStore,
  type BusinessServicePromiseRecord,
  BUSINESS_FULFILLMENT_LOOP_AGENT_DEFINITION_REF,
  BusinessFulfillmentLoopValidationError,
  buildBusinessFulfillmentEscalationPageReceipt,
  buildBusinessFulfillmentMotionReceipt,
  makeD1BusinessFulfillmentLoopStore,
  runBusinessFulfillmentLoop,
} from './business-fulfillment-loop'

type Row = Record<string, unknown>

const activePromise = (
  overrides?: Partial<BusinessServicePromiseRecord>,
): BusinessServicePromiseRecord => ({
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract.business_001',
  blockedAt: null,
  blockingReasonRef: null,
  cadence: 'daily',
  createdAt: '2026-07-02T00:00:00.000Z',
  crmStateRef: 'crm_state.business.promise_001.public_safe',
  id: 'business_service_promise_001',
  lastEscalationPageRef: null,
  lastMotionReceiptRef: null,
  nextMotionDueAt: '2026-07-03T00:00:00.000Z',
  promiseRef: 'promise.business.fulfillment.001',
  sourceRefs: ['github.public.issue.8096'],
  stakeholderRefs: [
    'stakeholder.business.operator',
    'stakeholder.business.customer_success',
  ],
  state: 'active',
  updatedAt: '2026-07-02T00:00:00.000Z',
  workspaceRef: 'workspace.business.fulfillment.001',
  ...overrides,
})

const runtime = {
  makeId: (prefix: string) => `${prefix}_test`,
  nowIso: () => '2026-07-03T12:00:00.000Z',
}

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async all<T = Row>(): Promise<{ results: Array<T> }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as Array<T>,
    }
  }

  async run(): Promise<{ meta: { changes: number }; success: true }> {
    const result = this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { meta: { changes: Number(result.changes) }, success: true }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(migration('0091_omni_accepted_outcome_contracts.sql'))
  db.exec(migration('0274_business_fulfillment_loop.sql'))
  db.exec(migration('0275_business_fulfillment_escalation_pages.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

class MemoryFulfillmentLoopStore implements BusinessFulfillmentLoopStore {
  public readonly escalationPages = new Map<string, string>()
  public readonly escalationUpdates: Array<{
    pageRef: string
    promiseId: string
  }> = []
  public readonly receipts = new Map<string, string>()
  public readonly updates: Array<{
    nextMotionDueAt: string
    promiseId: string
    receiptRef: string
  }> = []

  constructor(
    private readonly promises: ReadonlyArray<BusinessServicePromiseRecord>,
  ) {}

  async claimBlockedPromisePage(receipt: {
    escalationDate: string
    pageRef: string
    promiseId: string
  }): Promise<Readonly<{ claimed: boolean }>> {
    const key = `${receipt.promiseId}:${receipt.escalationDate}`
    if (this.escalationPages.has(key)) {
      return { claimed: false }
    }
    this.escalationPages.set(key, receipt.pageRef)
    return { claimed: true }
  }

  async claimDailyMotionReceipt(receipt: {
    motionDate: string
    promiseId: string
    receiptRef: string
  }): Promise<Readonly<{ claimed: boolean }>> {
    const key = `${receipt.promiseId}:${receipt.motionDate}`
    if (this.receipts.has(key)) {
      return { claimed: false }
    }
    this.receipts.set(key, receipt.receiptRef)
    return { claimed: true }
  }

  async listBlockedPromises(
    nowIso: string,
    limit: number,
  ): Promise<ReadonlyArray<BusinessServicePromiseRecord>> {
    void nowIso
    return this.promises
      .filter(
        promise =>
          promise.state === 'blocked' && promise.blockingReasonRef !== null,
      )
      .slice(0, limit)
  }

  async listDuePromises(
    nowIso: string,
    limit: number,
  ): Promise<ReadonlyArray<BusinessServicePromiseRecord>> {
    return this.promises
      .filter(
        promise =>
          promise.state === 'active' &&
          (promise.nextMotionDueAt === null || promise.nextMotionDueAt <= nowIso),
      )
      .slice(0, limit)
  }

  async markPromiseEscalationRecorded(
    promiseId: string,
    pageRef: string,
  ): Promise<void> {
    this.escalationUpdates.push({ pageRef, promiseId })
  }

  async markPromiseMotionRecorded(
    promiseId: string,
    receiptRef: string,
    nextMotionDueAt: string,
  ): Promise<void> {
    this.updates.push({ nextMotionDueAt, promiseId, receiptRef })
  }
}

describe('business fulfillment loop', () => {
  test('records a daily motion receipt with approval-gated client comms', async () => {
    const promise = activePromise()
    const receipt = buildBusinessFulfillmentMotionReceipt(promise, runtime)

    expect(receipt).toMatchObject({
      agentDefinitionRef: BUSINESS_FULFILLMENT_LOOP_AGENT_DEFINITION_REF,
      approvalGateRef:
        'approval_gate.business_fulfillment.client_comms.promise_business_fulfillment_001_2026_07_03',
      clientCommsDraftRef:
        'draft.business_fulfillment.client_comms.promise_business_fulfillment_001_2026_07_03',
      crmStateRef: promise.crmStateRef,
      forwardMotionRef:
        'motion.business_fulfillment.daily.promise_business_fulfillment_001_2026_07_03',
      motionDate: '2026-07-03',
      outboundAllowed: false,
      promiseRef: promise.promiseRef,
      receiptRef:
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_001_2026_07_03',
    })
    expect(receipt.stakeholderFlagRefs).toEqual([
      'stakeholder_flag.business_fulfillment.stakeholder_business_operator.20260703',
      'stakeholder_flag.business_fulfillment.stakeholder_business_customer_success.20260703',
    ])
    expect(JSON.stringify(receipt)).not.toMatch(/@|customer_email|raw_crm/i)
  })

  test('cron tick claims each due active promise once per day', async () => {
    const store = new MemoryFulfillmentLoopStore([activePromise()])
    const result = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )

    expect(result).toEqual({
      blockedPromiseCount: 0,
      duePromiseCount: 1,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_001_2026_07_03',
      ],
      skippedDuplicateCount: 0,
      state: 'completed',
    })
    expect(store.updates).toEqual([
      {
        nextMotionDueAt: '2026-07-04T12:00:00.000Z',
        promiseId: 'business_service_promise_001',
        receiptRef:
          'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_001_2026_07_03',
      },
    ])

    const duplicate = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )
    expect(duplicate).toMatchObject({
      blockedPromiseCount: 0,
      duePromiseCount: 1,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [],
      skippedDuplicateCount: 1,
      state: 'completed',
    })
  })

  test('ignores paused promises and unsafe CRM refs', async () => {
    const pausedStore = new MemoryFulfillmentLoopStore([
      activePromise({ state: 'paused' }),
    ])
    await expect(
      Effect.runPromise(runBusinessFulfillmentLoop({ runtime, store: pausedStore })),
    ).resolves.toMatchObject({
      blockedPromiseCount: 0,
      duePromiseCount: 0,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [],
      state: 'skipped',
    })

    let caught: unknown
    try {
      buildBusinessFulfillmentMotionReceipt(
        activePromise({ crmStateRef: 'raw_crm.customer_email@example.com' }),
        runtime,
      )
    } catch (error) {
      caught = error
    }
    expect(caught).toBeInstanceOf(BusinessFulfillmentLoopValidationError)
    expect((caught as BusinessFulfillmentLoopValidationError).reason).toContain(
      'opaque public-safe ref',
    )
  })

  test('blocked promises page the operator with the blocking reason ref', async () => {
    const promise = activePromise({
      blockedAt: '2026-07-03T09:00:00.000Z',
      blockingReasonRef: 'blocker.business_fulfillment.customer_asset_missing',
      state: 'blocked',
    })
    const receipt = buildBusinessFulfillmentEscalationPageReceipt(promise, runtime)

    expect(receipt).toMatchObject({
      agentDefinitionRef: BUSINESS_FULFILLMENT_LOOP_AGENT_DEFINITION_REF,
      blockedAt: '2026-07-03T09:00:00.000Z',
      blockingReasonRef:
        'blocker.business_fulfillment.customer_asset_missing',
      ownerNotificationRef:
        'notification.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_001_2026_07_03',
      pageRef:
        'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_001_2026_07_03',
      receiptRef:
        'receipt.business_fulfillment.blocked_promise_page.promise_business_fulfillment_001_2026_07_03',
    })
    expect(JSON.stringify(receipt)).not.toMatch(/@|customer_email|raw_crm/i)

    const store = new MemoryFulfillmentLoopStore([promise])
    const first = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )
    const second = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )

    expect(first).toEqual({
      blockedPromiseCount: 1,
      duePromiseCount: 0,
      escalationPageRefs: [
        'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_001_2026_07_03',
      ],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [],
      skippedDuplicateCount: 0,
      state: 'completed',
    })
    expect(store.escalationUpdates).toEqual([
      {
        pageRef:
          'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_001_2026_07_03',
        promiseId: 'business_service_promise_001',
      },
    ])
    expect(second).toMatchObject({
      blockedPromiseCount: 1,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 1,
      state: 'completed',
    })
  })

  test('D1 store claims one daily receipt per active service promise', async () => {
    const db = makeDb()
    await db
      .prepare(
        `INSERT INTO business_service_promises (
          id,
          promise_ref,
          accepted_outcome_contract_id,
          workspace_ref,
          crm_state_ref,
          stakeholder_refs_json,
          state,
          cadence,
          next_motion_due_at,
          last_motion_receipt_ref,
          source_refs_json,
          metadata_json,
          created_at,
          updated_at
        ) VALUES (?, ?, NULL, ?, ?, ?, 'active', 'daily', ?, NULL, ?, '{}', ?, ?)`,
      )
      .bind(
        'business_service_promise_d1',
        'promise.business.fulfillment.d1',
        'workspace.business.fulfillment.d1',
        'crm_state.business.promise_d1.public_safe',
        JSON.stringify(['stakeholder.business.operator']),
        '2026-07-03T00:00:00.000Z',
        JSON.stringify(['docs/fable/ROADMAP_BIZ.md#BF-5.1']),
        '2026-07-02T00:00:00.000Z',
        '2026-07-02T00:00:00.000Z',
      )
      .run()

    const first = await Effect.runPromise(
      runBusinessFulfillmentLoop({
        runtime,
        store: makeD1BusinessFulfillmentLoopStore(db),
      }),
    )
    const second = await Effect.runPromise(
      runBusinessFulfillmentLoop({
        runtime,
        store: makeD1BusinessFulfillmentLoopStore(db),
      }),
    )
    const receiptRow = await db
      .prepare(
        `SELECT receipt_ref,
                outbound_allowed,
                approval_gate_ref,
                client_comms_draft_ref
           FROM business_fulfillment_motion_receipts
          WHERE promise_id = ?`,
      )
      .bind('business_service_promise_d1')
      .first<{
        approval_gate_ref: string
        client_comms_draft_ref: string
        outbound_allowed: number
        receipt_ref: string
      }>()
    const promiseRow = await db
      .prepare(
        `SELECT last_motion_receipt_ref, next_motion_due_at
           FROM business_service_promises
          WHERE id = ?`,
      )
      .bind('business_service_promise_d1')
      .first<{
        last_motion_receipt_ref: string
        next_motion_due_at: string
      }>()

    expect(first).toEqual({
      blockedPromiseCount: 0,
      duePromiseCount: 1,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_d1_2026_07_03',
      ],
      skippedDuplicateCount: 0,
      state: 'completed',
    })
    expect(second).toMatchObject({
      blockedPromiseCount: 0,
      duePromiseCount: 0,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [],
      skippedDuplicateCount: 0,
      state: 'skipped',
    })
    expect(receiptRow).toMatchObject({
      approval_gate_ref:
        'approval_gate.business_fulfillment.client_comms.promise_business_fulfillment_d1_2026_07_03',
      client_comms_draft_ref:
        'draft.business_fulfillment.client_comms.promise_business_fulfillment_d1_2026_07_03',
      outbound_allowed: 0,
      receipt_ref:
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_d1_2026_07_03',
    })
    expect(promiseRow).toMatchObject({
      last_motion_receipt_ref:
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_d1_2026_07_03',
      next_motion_due_at: '2026-07-04T12:00:00.000Z',
    })
  })

  test('D1 store claims one blocked-promise page per day', async () => {
    const db = makeDb()
    await db
      .prepare(
        `INSERT INTO business_service_promises (
          id,
          promise_ref,
          accepted_outcome_contract_id,
          workspace_ref,
          crm_state_ref,
          stakeholder_refs_json,
          state,
          cadence,
          next_motion_due_at,
          last_motion_receipt_ref,
          blocking_reason_ref,
          blocked_at,
          last_escalation_page_ref,
          source_refs_json,
          metadata_json,
          created_at,
          updated_at
        ) VALUES (?, ?, NULL, ?, ?, ?, 'blocked', 'daily', NULL, NULL, ?, ?, NULL, ?, '{}', ?, ?)`,
      )
      .bind(
        'business_service_promise_blocked_d1',
        'promise.business.fulfillment.blocked_d1',
        'workspace.business.fulfillment.blocked_d1',
        'crm_state.business.promise_blocked_d1.public_safe',
        JSON.stringify(['stakeholder.business.operator']),
        'blocker.business_fulfillment.operator_decision_needed',
        '2026-07-03T09:00:00.000Z',
        JSON.stringify(['docs/fable/ROADMAP_BIZ.md#BF-5.4']),
        '2026-07-02T00:00:00.000Z',
        '2026-07-03T09:00:00.000Z',
      )
      .run()

    const first = await Effect.runPromise(
      runBusinessFulfillmentLoop({
        runtime,
        store: makeD1BusinessFulfillmentLoopStore(db),
      }),
    )
    const second = await Effect.runPromise(
      runBusinessFulfillmentLoop({
        runtime,
        store: makeD1BusinessFulfillmentLoopStore(db),
      }),
    )
    const pageRow = await db
      .prepare(
        `SELECT page_ref,
                owner_notification_ref,
                blocking_reason_ref,
                blocked_at
           FROM business_fulfillment_escalation_pages
          WHERE promise_id = ?`,
      )
      .bind('business_service_promise_blocked_d1')
      .first<{
        blocked_at: string
        blocking_reason_ref: string
        owner_notification_ref: string
        page_ref: string
      }>()
    const promiseRow = await db
      .prepare(
        `SELECT last_escalation_page_ref
           FROM business_service_promises
          WHERE id = ?`,
      )
      .bind('business_service_promise_blocked_d1')
      .first<{ last_escalation_page_ref: string }>()

    expect(first).toEqual({
      blockedPromiseCount: 1,
      duePromiseCount: 0,
      escalationPageRefs: [
        'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_blocked_d1_2026_07_03',
      ],
      escalationSkippedDuplicateCount: 0,
      motionReceiptRefs: [],
      skippedDuplicateCount: 0,
      state: 'completed',
    })
    expect(second).toMatchObject({
      blockedPromiseCount: 1,
      escalationPageRefs: [],
      escalationSkippedDuplicateCount: 1,
      state: 'completed',
    })
    expect(pageRow).toMatchObject({
      blocked_at: '2026-07-03T09:00:00.000Z',
      blocking_reason_ref:
        'blocker.business_fulfillment.operator_decision_needed',
      owner_notification_ref:
        'notification.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_blocked_d1_2026_07_03',
      page_ref:
        'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_blocked_d1_2026_07_03',
    })
    expect(promiseRow).toMatchObject({
      last_escalation_page_ref:
        'page.owner.business_fulfillment.blocked_promise.promise_business_fulfillment_blocked_d1_2026_07_03',
    })
  })
})
