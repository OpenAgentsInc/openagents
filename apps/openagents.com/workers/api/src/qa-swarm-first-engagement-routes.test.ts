import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { insertBusinessSignupRequest } from './business-signup-routes'
import {
  QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT,
  QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT,
  handleOperatorQaSwarmFirstEngagementsApi,
  handlePublicQaSwarmFirstEngagementRead,
  makeD1QaSwarmFirstEngagementStore,
  qaSwarmFirstEngagementRecordFromSql,
} from './qa-swarm-first-engagement-routes'

const nowIso = '2026-07-04T18:00:00.000Z'

type Row = Record<string, unknown>

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

  async all<T = Row>(): Promise<D1Result<T>> {
    const results = this.db
      .prepare(this.sql)
      .all(...(this.bound as never[])) as Array<T>

    return { results, success: true } as D1Result<T>
  }

  async run(): Promise<D1Result> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true } as D1Result
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  batch<T = unknown>(
    statements: ReadonlyArray<D1PreparedStatement>,
  ): Promise<Array<D1Result<T>>> {
    return Promise.all(statements.map(statement => statement.run<T>()))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.resolve(new ArrayBuffer(0))
  }

  exec(sql: string): Promise<D1ExecResult> {
    this.db.exec(sql)
    return Promise.resolve({ count: 0, duration: 0 })
  }

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const migration = (name: string): string =>
  readFileSync(join(__dirname, '..', 'migrations', name), 'utf8')

const makeDb = (): D1Database => {
  const db = new DatabaseSync(':memory:')
  db.exec(`
    CREATE TABLE users (id TEXT PRIMARY KEY);
    CREATE TABLE teams (
      id TEXT PRIMARY KEY,
      status TEXT NOT NULL DEFAULT 'active',
      archived_at TEXT
    );
    CREATE TABLE team_memberships (
      team_id TEXT NOT NULL,
      user_id TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'active'
    );
    CREATE TABLE team_projects (
      id TEXT PRIMARY KEY,
      team_id TEXT NOT NULL,
      status TEXT NOT NULL DEFAULT 'active',
      archived_at TEXT
    );
  `)
  for (const name of [
    '0091_omni_accepted_outcome_contracts.sql',
    '0190_prefilled_workspaces.sql',
    '0191_business_signup_requests.sql',
    '0192_prefilled_workspace_invite_engagement.sql',
    '0195_private_prefilled_workspace_access.sql',
    '0216_business_signup_referral_attribution.sql',
    '0271_business_signup_fulfillment.sql',
    '0273_accepted_outcome_service_promises.sql',
    '0278_business_commitment_ledger.sql',
    '0292_qa_swarm_first_engagements.sql',
  ]) {
    db.exec(migration(name))
  }

  return new SqliteD1(db) as unknown as D1Database
}

const createSignup = async (db: D1Database): Promise<string> => {
  await db
    .prepare("INSERT INTO users (id) VALUES ('github:qa_buyer_1')")
    .run()

  const signup = await insertBusinessSignupRequest(
    db,
    {
      businessName: 'QA buyer',
      contactEmail: 'buyer@example.com',
      helpWith: 'Need a Swarm Audit.',
      phone: '',
      referralCode: null,
      requestSlackChannel: false,
      sourceAttribution: null,
      website: 'https://example.com',
    },
    {
      expiresAtFromNow: () => '2026-08-04T00:00:00.000Z',
      makeId: prefix => `${prefix}_qa_001`,
      nowIso: () => '2026-07-04T00:00:00.000Z',
    },
  )

  return signup.id
}

const validBody = (
  signupId: string,
  idempotencyKey = 'qa-swarm-first-engagement-test-1',
): Record<string, unknown> => ({
  schemaVersion: 'openagents.qa_swarm.first_engagement_intake.v1',
  packageKind: 'swarm_audit',
  paymentPath: 'operator_sales_deposit_invoice',
  businessSignupRequestId: signupId,
  userId: 'github:qa_buyer_1',
  committedAmountCents: 300_000,
  intakeReceiptRef: 'receipt.business.intake.qa_swarm.example_001',
  depositInvoiceReceiptRef:
    'receipt.qa_swarm.deposit_invoice.operator_sales.example_001',
  targetAdapterReviewRef: 'review.qa_swarm.target_adapter.example_001',
  packageContractRef: 'contract.qa_swarm.swarm_audit.operator_assisted.v1',
  firstReportDueAt: '2026-07-11T18:00:00.000Z',
  idempotencyKey,
})

const operatorRequest = (method: string, body?: unknown): Request => {
  const init =
    body === undefined
      ? { method }
      : {
          method,
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        }

  return new Request(
    `https://openagents.com${QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT}`,
    init,
  )
}

const publicRequest = (receiptRef: string, method = 'GET'): Request =>
  new Request(
    `https://openagents.com${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT}/${encodeURIComponent(
      receiptRef,
    )}`,
    { method },
  )

const makeStore = (db: D1Database) =>
  makeD1QaSwarmFirstEngagementStore(db, {
    makeId: prefix => `${prefix}_qa_001`,
    nowIso: () => nowIso,
  })

describe('QA Swarm first engagement routes', () => {
  test('records an admin-gated Swarm Audit engagement and provisions BF-2/BF-9.1 rows', async () => {
    const db = makeDb()
    const signupId = await createSignup(db)
    const response = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', validBody(signupId)),
        {
          nowIso: () => nowIso,
          store: makeStore(db),
          requireAdminApiToken: async () => true,
        },
      ),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toContain('no-store')

    const body = (await response.json()) as {
      ok: boolean
      idempotent: boolean
      generatedAt: string
      receipt: {
        receiptRef: string
        promiseIds: ReadonlyArray<string>
        purchase: Record<string, unknown>
        provision: Record<string, unknown>
        commitment: Record<string, unknown>
        publicSafety: Record<string, boolean>
        staleness: { composition: string; maxStalenessSeconds: number }
      }
    }

    expect(body.ok).toBe(true)
    expect(body.idempotent).toBe(false)
    expect(body.generatedAt).toBe(nowIso)
    expect(body.receipt.promiseIds).toEqual([
      'qa_swarm.service_packages.v1',
      'qa_swarm.product_surface.v1',
    ])
    expect(body.receipt.receiptRef).toMatch(
      /^receipt\.qa_swarm\.first_engagement\./,
    )
    expect(body.receipt.purchase).toMatchObject({
      operatorAssisted: true,
      selfServe: false,
      paymentPath: 'operator_sales_deposit_invoice',
      committedAmountCents: 300_000,
      paymentEvidenceRecorded: true,
      rawPaymentMaterialIncluded: false,
      firstPaidDeliveryReceipt: false,
      settlementMovedMoney: false,
    })
    expect(body.receipt.provision).toMatchObject({
      servicePromiseState: 'active',
      deliverableContractRef:
        'contract.qa_swarm.swarm_audit.operator_assisted.v1',
    })
    expect(body.receipt.commitment).toMatchObject({
      promisedObjectRef: 'deliverable.qa_swarm.swarm_audit.report.v1',
      dueState: 'due',
      firstReportDueAt: '2026-07-11T18:00:00.000Z',
    })
    expect(body.receipt.publicSafety).toMatchObject({
      noCustomerIdentity: true,
      noRawPaymentMaterial: true,
      noRawInvoice: true,
      noRawRunnerLogs: true,
    })
    expect(body.receipt.staleness).toMatchObject({
      composition: 'live_at_read',
      maxStalenessSeconds: 0,
    })

    const counts = {
      workspaces: await db
        .prepare('SELECT COUNT(*) AS count FROM prefilled_workspaces')
        .first<{ count: number }>(),
      contracts: await db
        .prepare('SELECT COUNT(*) AS count FROM omni_accepted_outcome_contracts')
        .first<{ count: number }>(),
      commitments: await db
        .prepare(
          `SELECT COUNT(*) AS count
             FROM business_commitment_ledger
            WHERE commitment_ref LIKE 'business.commitment.qa_swarm.swarm_audit.%'`,
        )
        .first<{ count: number }>(),
      engagements: await db
        .prepare('SELECT COUNT(*) AS count FROM qa_swarm_first_engagements')
        .first<{ count: number }>(),
    }
    expect(counts.workspaces?.count).toBe(1)
    expect(counts.contracts?.count).toBe(1)
    expect(counts.commitments?.count).toBe(1)
    expect(counts.engagements?.count).toBe(1)

    const workspace = await db
      .prepare(
        `SELECT project_name, status, intro_receipt_json
           FROM prefilled_workspaces
          LIMIT 1`,
      )
      .first<{
        intro_receipt_json: string
        project_name: string
        status: string
      }>()
    expect(workspace).toMatchObject({
      project_name: 'QA Swarm Audit workspace',
      status: 'active',
    })
    expect(workspace?.intro_receipt_json).toContain(
      'receipt.qa_swarm.deposit_invoice.operator_sales.example_001',
    )

    const contract = await db
      .prepare(
        `SELECT service_promise_state,
                committed_deliverables_json,
                metadata_json
           FROM omni_accepted_outcome_contracts
          LIMIT 1`,
      )
      .first<{
        committed_deliverables_json: string
        metadata_json: string
        service_promise_state: string
      }>()
    expect(contract?.service_promise_state).toBe('active')
    expect(contract?.committed_deliverables_json).toContain(
      'deliverable.qa_swarm.swarm_audit.report.v1',
    )
    expect(contract?.metadata_json).toContain(body.receipt.receiptRef)
  })

  test('idempotently replays and publicly dereferences without leaking private buyer or payment material', async () => {
    const db = makeDb()
    const signupId = await createSignup(db)
    const store = makeStore(db)

    const first = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', validBody(signupId)),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    const second = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', validBody(signupId)),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )

    const firstBody = (await first.json()) as {
      receipt: { receiptRef: string }
    }
    const secondBody = (await second.json()) as {
      idempotent: boolean
      receipt: { receiptRef: string }
    }

    expect(second.status).toBe(200)
    expect(secondBody.idempotent).toBe(true)
    expect(secondBody.receipt.receiptRef).toBe(firstBody.receipt.receiptRef)

    const workspaceCount = await db
      .prepare('SELECT COUNT(*) AS count FROM prefilled_workspaces')
      .first<{ count: number }>()
    expect(workspaceCount?.count).toBe(1)

    const read = await Effect.runPromise(
      handlePublicQaSwarmFirstEngagementRead(
        publicRequest(firstBody.receipt.receiptRef),
        {
          receiptRef: firstBody.receipt.receiptRef,
          nowIso: () => nowIso,
          store,
        },
      ),
    )

    expect(read.status).toBe(200)
    const text = await read.text()
    const body = JSON.parse(text) as {
      receipt: {
        purchase: {
          firstPaidDeliveryReceipt: boolean
          settlementMovedMoney: boolean
        }
      }
    }
    expect(body.receipt.purchase.firstPaidDeliveryReceipt).toBe(false)
    expect(body.receipt.purchase.settlementMovedMoney).toBe(false)
    expect(text).not.toContain('buyer@example.com')
    expect(text).not.toContain('QA buyer')
    expect(text).not.toContain('lnbc')
    expect(text).not.toContain('payment_hash')
    expect(text).not.toContain('preimage')
    expect(text).not.toContain('rawPaymentPayload')
  })

  test('requires admin authorization and expected methods', async () => {
    const db = makeDb()
    const signupId = await createSignup(db)

    const unauthorized = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', validBody(signupId)),
        {
          nowIso: () => nowIso,
          store: makeStore(db),
          requireAdminApiToken: async () => false,
        },
      ),
    )
    expect(unauthorized.status).toBe(401)

    const wrongOperatorMethod = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(operatorRequest('GET'), {
        nowIso: () => nowIso,
        store: makeStore(db),
        requireAdminApiToken: async () => true,
      }),
    )
    expect(wrongOperatorMethod.status).toBe(405)

    const wrongPublicMethod = await Effect.runPromise(
      handlePublicQaSwarmFirstEngagementRead(
        publicRequest('receipt.qa_swarm.first_engagement.any', 'POST'),
        {
          receiptRef: 'receipt.qa_swarm.first_engagement.any',
          nowIso: () => nowIso,
          store: makeStore(db),
        },
      ),
    )
    expect(wrongPublicMethod.status).toBe(405)
  })

  test('rejects unsafe refs, missing payment evidence, impossible bands, and private-material keys', async () => {
    const db = makeDb()
    const signupId = await createSignup(db)
    const store = makeStore(db)

    const unsafe = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', {
          ...validBody(signupId),
          depositInvoiceReceiptRef: 'lnbc1rawinvoice',
        }),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    expect(unsafe.status).toBe(400)
    await expect(unsafe.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
    })

    const impossible = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', {
          ...validBody(signupId, 'qa-swarm-first-engagement-test-2'),
          committedAmountCents: 999_999,
        }),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    expect(impossible.status).toBe(400)
    await expect(impossible.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
      reason: 'swarm_audit_amount_must_match_public_band',
    })

    const missingCheckout = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', {
          ...validBody(signupId, 'qa-swarm-first-engagement-test-3'),
          paymentPath: 'checkout_kickoff_receipt',
          depositInvoiceReceiptRef: undefined,
        }),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    expect(missingCheckout.status).toBe(400)
    await expect(missingCheckout.json()).resolves.toMatchObject({
      error: 'invalid_public_safe_evidence',
      reason: 'checkout_kickoff_receipt_ref_required',
    })

    const privateKey = await Effect.runPromise(
      handleOperatorQaSwarmFirstEngagementsApi(
        operatorRequest('POST', {
          ...validBody(signupId, 'qa-swarm-first-engagement-test-4'),
          rawInvoicePayload: 'private',
        }),
        {
          nowIso: () => nowIso,
          store,
          requireAdminApiToken: async () => true,
        },
      ),
    )
    expect(privateKey.status).toBe(400)
    await expect(privateKey.json()).resolves.toMatchObject({
      error: 'private_material_not_allowed',
    })
  })

  test('SQL row normalization rejects unsafe shape drift', () => {
    expect(
      qaSwarmFirstEngagementRecordFromSql({
        receipt_ref: 'receipt.qa_swarm.first_engagement.abc',
        package_kind: 'swarm_audit',
        payment_path: 'operator_sales_deposit_invoice',
        business_signup_request_id: 'business_signup_qa_001',
        user_id: 'github:qa_buyer_1',
        committed_amount_cents: 300_000,
        intake_receipt_ref: 'receipt.business.intake.qa_swarm.example_001',
        checkout_or_deposit_receipt_ref:
          'receipt.qa_swarm.deposit_invoice.operator_sales.example_001',
        target_adapter_review_ref: 'review.qa_swarm.target_adapter.example_001',
        package_contract_ref: 'contract.qa_swarm.swarm_audit.v1',
        workspace_id: 'workspace_qa_001',
        service_promise_contract_id: 'contract_qa_001',
        commitment_ref: 'business.commitment.qa_swarm.swarm_audit.abc',
        first_report_due_at: '2026-07-11T18:00:00.000Z',
        recorded_at: nowIso,
      }),
    ).toMatchObject({
      packageKind: 'swarm_audit',
      committedAmountCents: 300_000,
    })

    expect(
      qaSwarmFirstEngagementRecordFromSql({
        receipt_ref: 'receipt.qa_swarm.first_engagement.bad',
        package_kind: 'hosted_run',
        payment_path: 'operator_sales_deposit_invoice',
        business_signup_request_id: 'business_signup_qa_001',
        user_id: 'github:qa_buyer_1',
        committed_amount_cents: 300_000,
        intake_receipt_ref: 'receipt.business.intake.qa_swarm.example_001',
        checkout_or_deposit_receipt_ref:
          'receipt.qa_swarm.deposit_invoice.operator_sales.example_001',
        target_adapter_review_ref: 'review.qa_swarm.target_adapter.example_001',
        package_contract_ref: 'contract.qa_swarm.swarm_audit.v1',
        workspace_id: 'workspace_qa_001',
        service_promise_contract_id: 'contract_qa_001',
        commitment_ref: 'business.commitment.qa_swarm.swarm_audit.bad',
        first_report_due_at: '2026-07-11T18:00:00.000Z',
        recorded_at: nowIso,
      }),
    ).toBeNull()
  })
})
