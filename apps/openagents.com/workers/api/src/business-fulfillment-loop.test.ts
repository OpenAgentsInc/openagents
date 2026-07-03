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
  buildBusinessFulfillmentMotionReceipt,
  makeD1BusinessFulfillmentLoopStore,
  runBusinessFulfillmentLoop,
} from './business-fulfillment-loop'

type Row = Record<string, unknown>

const activePromise = (
  overrides?: Partial<BusinessServicePromiseRecord>,
): BusinessServicePromiseRecord => ({
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract.business_001',
  cadence: 'daily',
  createdAt: '2026-07-02T00:00:00.000Z',
  crmStateRef: 'crm_state.business.promise_001.public_safe',
  id: 'business_service_promise_001',
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
  db.exec(migration('0275_business_fulfillment_cadence_drip_updates.sql'))
  return new SqliteD1(db) as unknown as D1Database
}

class MemoryFulfillmentLoopStore implements BusinessFulfillmentLoopStore {
  public readonly receipts = new Map<string, string>()
  public readonly updates: Array<{
    nextMotionDueAt: string
    promiseId: string
    receiptRef: string
  }> = []

  constructor(
    private readonly promises: ReadonlyArray<BusinessServicePromiseRecord>,
  ) {}

  async claimMotionReceipt(receipt: {
    cadence: string
    motionDate: string
    promiseId: string
    receiptRef: string
  }): Promise<Readonly<{ claimed: boolean }>> {
    const key = `${receipt.promiseId}:${receipt.cadence}:${receipt.motionDate}`
    if (this.receipts.has(key)) {
      return { claimed: false }
    }
    this.receipts.set(key, receipt.receiptRef)
    return { claimed: true }
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
      cadence: 'daily',
      clientCommsDraftRef:
        'draft.business_fulfillment.client_comms.promise_business_fulfillment_001_2026_07_03',
      clientCommsLedgerRef:
        'email_ledger.business_fulfillment.daily.client_comms.promise_business_fulfillment_001_2026_07_03',
      crmStateRef: promise.crmStateRef,
      customerWorkroomUpdateRef:
        'workroom_update.business_fulfillment.customer_visible.daily.promise_business_fulfillment_001_2026_07_03',
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

  test('records weekly motion with weekly drip and customer workroom update refs', async () => {
    const promise = activePromise({
      cadence: 'weekly',
      id: 'business_service_promise_weekly',
      promiseRef: 'promise.business.fulfillment.weekly',
    })
    const receipt = buildBusinessFulfillmentMotionReceipt(promise, runtime)

    expect(receipt).toMatchObject({
      cadence: 'weekly',
      clientCommsLedgerRef:
        'email_ledger.business_fulfillment.weekly.client_comms.promise_business_fulfillment_weekly_2026_07_03',
      customerWorkroomUpdateRef:
        'workroom_update.business_fulfillment.customer_visible.weekly.promise_business_fulfillment_weekly_2026_07_03',
      forwardMotionRef:
        'motion.business_fulfillment.weekly.promise_business_fulfillment_weekly_2026_07_03',
      receiptRef:
        'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_weekly_2026_07_03',
    })
  })

  test('cron tick claims each due active promise once per configured cadence', async () => {
    const store = new MemoryFulfillmentLoopStore([
      activePromise(),
      activePromise({
        cadence: 'weekly',
        id: 'business_service_promise_weekly',
        promiseRef: 'promise.business.fulfillment.weekly',
      }),
    ])
    const result = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )

    expect(result).toEqual({
      duePromiseCount: 2,
      motionReceiptRefs: [
        'receipt.business_fulfillment.daily_motion.promise_business_fulfillment_001_2026_07_03',
        'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_weekly_2026_07_03',
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
      {
        nextMotionDueAt: '2026-07-10T12:00:00.000Z',
        promiseId: 'business_service_promise_weekly',
        receiptRef:
          'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_weekly_2026_07_03',
      },
    ])

    const duplicate = await Effect.runPromise(
      runBusinessFulfillmentLoop({ runtime, store }),
    )
    expect(duplicate).toMatchObject({
      duePromiseCount: 2,
      motionReceiptRefs: [],
      skippedDuplicateCount: 2,
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
      duePromiseCount: 0,
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

  test('D1 store claims one cadence-scoped receipt per active service promise', async () => {
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
        ) VALUES (?, ?, NULL, ?, ?, ?, 'active', ?, ?, NULL, ?, '{}', ?, ?)`,
      )
      .bind(
        'business_service_promise_d1',
        'promise.business.fulfillment.d1',
        'workspace.business.fulfillment.d1',
        'crm_state.business.promise_d1.public_safe',
        JSON.stringify(['stakeholder.business.operator']),
        'weekly',
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
                cadence,
                client_comms_ledger_ref,
                customer_workroom_update_ref,
                client_comms_draft_ref
           FROM business_fulfillment_motion_receipts
          WHERE promise_id = ?`,
      )
      .bind('business_service_promise_d1')
      .first<{
        approval_gate_ref: string
        cadence: string
        client_comms_draft_ref: string
        client_comms_ledger_ref: string
        customer_workroom_update_ref: string
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
      duePromiseCount: 1,
      motionReceiptRefs: [
        'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_d1_2026_07_03',
      ],
      skippedDuplicateCount: 0,
      state: 'completed',
    })
    expect(second).toMatchObject({
      duePromiseCount: 0,
      motionReceiptRefs: [],
      skippedDuplicateCount: 0,
      state: 'skipped',
    })
    expect(receiptRow).toMatchObject({
      approval_gate_ref:
        'approval_gate.business_fulfillment.client_comms.promise_business_fulfillment_d1_2026_07_03',
      cadence: 'weekly',
      client_comms_draft_ref:
        'draft.business_fulfillment.client_comms.promise_business_fulfillment_d1_2026_07_03',
      client_comms_ledger_ref:
        'email_ledger.business_fulfillment.weekly.client_comms.promise_business_fulfillment_d1_2026_07_03',
      customer_workroom_update_ref:
        'workroom_update.business_fulfillment.customer_visible.weekly.promise_business_fulfillment_d1_2026_07_03',
      outbound_allowed: 0,
      receipt_ref:
        'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_d1_2026_07_03',
    })
    expect(promiseRow).toMatchObject({
      last_motion_receipt_ref:
        'receipt.business_fulfillment.weekly_motion.promise_business_fulfillment_d1_2026_07_03',
      next_motion_due_at: '2026-07-10T12:00:00.000Z',
    })
  })
})
