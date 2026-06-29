import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniWorkroomAssignmentNotFound,
  OmniWorkroomOrderNotFound,
  OmniWorkroomSiteNotFound,
  OmniWorkroomValidationError,
  customerOmniWorkroomProjection,
  operatorOmniWorkroomProjection,
  promoteOmniWorkroom,
  publicOmniWorkroomProjection,
} from './omni-workrooms'

type RefRow = Readonly<{ archived_at: string | null; id: string }>
type WorkroomRow = Readonly<{
  accepted_outcome_contract_id: string | null
  archived_at: string | null
  artifact_refs_json: string
  assignment_id: string | null
  blocker_refs_json: string
  classification_caveat_ref: string
  created_at: string
  customer_intent_ref: string
  data_classification:
    | 'public'
    | 'customer'
    | 'team'
    | 'operator'
    | 'private'
    | 'legal_sensitive'
    | 'provider_private'
    | 'payment_private'
    | 'secret_bearing'
  email_refs_json: string
  id: string
  idempotency_key: string
  metadata_json: string
  public_receipt_ref: string
  receipt_refs_json: string
  site_id: string | null
  software_order_id: string
  source_refs_json: string
  status:
    | 'queued'
    | 'active'
    | 'blocked'
    | 'waiting_review'
    | 'completed'
    | 'unavailable'
    | 'archived'
  task_packet_ref: string | null
  trust_tier: 'verified' | 'reviewed' | 'unverified' | 'blocked'
  updated_at: string
  visibility: 'private' | 'customer' | 'team' | 'public'
  work_kind:
    | 'site'
    | 'coding'
    | 'adjustment'
    | 'existing_project_import'
    | 'business'
    | 'legal_sensitive'
}>

class OmniWorkroomStore {
  assignments: Array<RefRow> = [
    { archived_at: null, id: 'adjutant_assignment_1' },
  ]
  contracts: Array<RefRow> = [
    { archived_at: null, id: 'omni_accepted_outcome_contract_1' },
  ]
  orders: Array<RefRow> = [
    { archived_at: null, id: 'software_order_1' },
    { archived_at: null, id: 'software_order_coding_1' },
  ]
  sites: Array<RefRow> = [{ archived_at: null, id: 'site_project_1' }]
  workrooms: Array<WorkroomRow> = []
}

const runtime = {
  makeWorkroomId: () => 'omni_workroom_generated',
  nowIso: () => '2026-06-05T23:40:00.000Z',
}

class OmniWorkroomStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OmniWorkroomStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_workrooms')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.workrooms.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM software_orders')) {
      return Promise.resolve(this.findRef(this.store.orders) as T | null)
    }

    if (this.query.includes('FROM site_projects')) {
      return Promise.resolve(this.findRef(this.store.sites) as T | null)
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      return Promise.resolve(this.findRef(this.store.assignments) as T | null)
    }

    if (this.query.includes('FROM omni_accepted_outcome_contracts')) {
      return Promise.resolve(this.findRef(this.store.contracts) as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO omni_workrooms')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.workrooms.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.workrooms.push({
          accepted_outcome_contract_id: this.values[3] as string | null,
          archived_at: null,
          artifact_refs_json: String(this.values[12]),
          assignment_id: this.values[5] as string | null,
          blocker_refs_json: String(this.values[15]),
          classification_caveat_ref: String(this.values[18]),
          created_at: String(this.values[21]),
          customer_intent_ref: String(this.values[9]),
          data_classification: this
            .values[16] as WorkroomRow['data_classification'],
          email_refs_json: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[20]),
          public_receipt_ref: String(this.values[19]),
          receipt_refs_json: String(this.values[14]),
          site_id: this.values[4] as string | null,
          software_order_id: String(this.values[2]),
          source_refs_json: String(this.values[11]),
          status: this.values[7] as WorkroomRow['status'],
          task_packet_ref: this.values[10] as string | null,
          trust_tier: this.values[17] as WorkroomRow['trust_tier'],
          updated_at: String(this.values[22]),
          visibility: this.values[8] as WorkroomRow['visibility'],
          work_kind: this.values[6] as WorkroomRow['work_kind'],
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

  private findRef(rows: ReadonlyArray<RefRow>): RefRow | null {
    const id = String(this.values[0])

    return (
      rows.find(item => item.id === id && item.archived_at === null) ?? null
    )
  }
}

const omniWorkroomDb = (store: OmniWorkroomStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OmniWorkroomStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const promote = (
  store: OmniWorkroomStore,
  overrides: Partial<Parameters<typeof promoteOmniWorkroom>[1]> = {},
) =>
  Effect.runPromise(
    promoteOmniWorkroom(
      omniWorkroomDb(store),
      {
        acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
        artifactRefs: ['site_version_ref_1'],
        assignmentId: 'adjutant_assignment_1',
        blockerRefs: ['blocker_none_ref'],
        customerIntentRef: 'customer_intent_order_1',
        classificationCaveatRef: 'classification_caveat_reviewed_public',
        dataClassification: 'public',
        emailRefs: ['email_dispatch_ref_1'],
        id: 'omni_workroom_1',
        idempotencyKey: 'omni-workroom:software-order-1',
        receiptRefs: ['receipt_order_status_ref'],
        siteId: 'site_project_1',
        softwareOrderId: 'software_order_1',
        sourceRefs: ['source_card_ref_1'],
        status: 'active',
        taskPacketRef: 'task_packet_ref_1',
        trustTier: 'reviewed',
        visibility: 'customer',
        workKind: 'site',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni workrooms', () => {
  test('promotes Site orders idempotently with split projections', async () => {
    const store = new OmniWorkroomStore()
    const workroom = await promote(store)
    const replay = await promote(store, {
      customerIntentRef: 'customer_intent_changed',
    })
    const customer = customerOmniWorkroomProjection(workroom)
    const publicProjection = publicOmniWorkroomProjection(workroom)
    const operator = operatorOmniWorkroomProjection(workroom)

    expect(replay.customerIntentRef).toBe('customer_intent_order_1')
    expect(workroom).toMatchObject({
      acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
      assignmentId: 'adjutant_assignment_1',
      siteId: 'site_project_1',
      softwareOrderId: 'software_order_1',
      status: 'active',
      workKind: 'site',
    })
    expect(customer).toMatchObject({
      artifactRefs: ['site_version_ref_1'],
      blockerRefs: ['blocker_none_ref'],
      emailRefs: ['email_dispatch_ref_1'],
      receiptRefs: ['receipt_order_status_ref'],
    })
    expect(customer).not.toHaveProperty('sourceRefs')
    expect(customer).not.toHaveProperty('taskPacketRef')
    expect(publicProjection).toEqual({
      classificationCaveatRef: 'classification_caveat_reviewed_public',
      dataClassification: 'public',
      publicReceiptRef:
        'omni_workroom:software_order_1:omni-workroom:software-order-1',
      siteId: 'site_project_1',
      softwareOrderId: 'software_order_1',
      status: 'active',
      trustTier: 'reviewed',
      visibility: 'customer',
      workKind: 'site',
    })
    expect(operator).toMatchObject({
      assignmentId: 'adjutant_assignment_1',
      sourceRefs: ['source_card_ref_1'],
      taskPacketRef: 'task_packet_ref_1',
    })
  })

  test('supports non-Sites coding workrooms without a Site ref', async () => {
    const store = new OmniWorkroomStore()
    const workroom = await promote(store, {
      acceptedOutcomeContractId: undefined,
      artifactRefs: ['pull_request_ref_1'],
      assignmentId: undefined,
      blockerRefs: [],
      customerIntentRef: 'customer_intent_coding_order_1',
      dataClassification: undefined,
      emailRefs: [],
      id: 'omni_workroom_coding_1',
      idempotencyKey: 'omni-workroom:coding-order-1',
      receiptRefs: ['github_write_authority_receipt_1'],
      siteId: undefined,
      softwareOrderId: 'software_order_coding_1',
      sourceRefs: ['repository_source_ref_1'],
      taskPacketRef: 'task_packet_coding_ref_1',
      trustTier: undefined,
      workKind: 'coding',
    })

    expect(workroom).toMatchObject({
      dataClassification: 'customer',
      siteId: null,
      softwareOrderId: 'software_order_coding_1',
      trustTier: 'unverified',
      workKind: 'coding',
    })
    expect(customerOmniWorkroomProjection(workroom)).toMatchObject({
      dataClassification: 'customer',
      trustTier: 'unverified',
    })
    expect(() => publicOmniWorkroomProjection(workroom)).toThrow()
  })

  test('enforces legal, provider, payment, and blocked projection boundaries', async () => {
    const legal = await promote(new OmniWorkroomStore(), {
      dataClassification: 'legal_sensitive',
      idempotencyKey: 'omni-workroom:legal-1',
      siteId: undefined,
      softwareOrderId: 'software_order_coding_1',
      trustTier: 'reviewed',
      workKind: 'legal_sensitive',
    })
    const providerPrivate = await promote(new OmniWorkroomStore(), {
      dataClassification: 'provider_private',
      idempotencyKey: 'omni-workroom:provider-private-1',
      siteId: undefined,
      softwareOrderId: 'software_order_coding_1',
      trustTier: 'reviewed',
      workKind: 'coding',
    })
    const paymentPrivate = await promote(new OmniWorkroomStore(), {
      dataClassification: 'payment_private',
      idempotencyKey: 'omni-workroom:payment-private-1',
      siteId: undefined,
      softwareOrderId: 'software_order_coding_1',
      trustTier: 'reviewed',
      workKind: 'business',
    })
    const blocked = await promote(new OmniWorkroomStore(), {
      dataClassification: 'public',
      idempotencyKey: 'omni-workroom:blocked-public-1',
      trustTier: 'blocked',
    })

    expect(operatorOmniWorkroomProjection(legal)).toMatchObject({
      dataClassification: 'legal_sensitive',
    })
    expect(operatorOmniWorkroomProjection(providerPrivate)).toMatchObject({
      dataClassification: 'provider_private',
    })
    expect(operatorOmniWorkroomProjection(paymentPrivate)).toMatchObject({
      dataClassification: 'payment_private',
    })
    expect(() => customerOmniWorkroomProjection(legal)).toThrow()
    expect(() => customerOmniWorkroomProjection(providerPrivate)).toThrow()
    expect(() => customerOmniWorkroomProjection(paymentPrivate)).toThrow()
    expect(() => publicOmniWorkroomProjection(blocked)).toThrow()
  })

  test('rejects legal-sensitive workrooms with weak classification', async () => {
    await expect(
      promote(new OmniWorkroomStore(), {
        dataClassification: 'public',
        siteId: undefined,
        softwareOrderId: 'software_order_coding_1',
        workKind: 'legal_sensitive',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomValidationError)
  })

  test('requires Site workrooms to include an existing Site ref', async () => {
    await expect(
      promote(new OmniWorkroomStore(), {
        siteId: undefined,
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomValidationError)

    await expect(
      promote(new OmniWorkroomStore(), {
        siteId: 'site_project_missing',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomSiteNotFound)
  })

  test('rejects missing orders and missing optional linked refs', async () => {
    await expect(
      promote(new OmniWorkroomStore(), {
        softwareOrderId: 'software_order_missing',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomOrderNotFound)

    await expect(
      promote(new OmniWorkroomStore(), {
        assignmentId: 'adjutant_assignment_missing',
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomAssignmentNotFound)
  })

  test('rejects raw provider, run log, email, payment, wallet, and customer material', async () => {
    await expect(
      promote(new OmniWorkroomStore(), {
        metadata: { rawEmail: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomValidationError)

    await expect(
      promote(new OmniWorkroomStore(), {
        receiptRefs: ['lnbc1rawinvoice'],
      }),
    ).rejects.toBeInstanceOf(OmniWorkroomValidationError)
  })
})
