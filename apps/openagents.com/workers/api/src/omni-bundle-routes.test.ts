import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OmniBundleRoutesDependencies,
  makeOmniBundleRoutes,
} from './omni-bundle-routes'
import {
  type OmniEvidenceBundleRecord,
  customerOmniEvidenceBundleProjection,
  operatorOmniEvidenceBundleProjection,
} from './omni-evidence-bundles'
import {
  type OmniPublicProofBundleRecord,
  operatorOmniProofBundleProjection,
  publicOmniProofBundleProjection,
} from './omni-public-proof-bundles'

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

type EvidenceRow = Readonly<{
  archived_at: string | null
  created_at: string
  entries_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  public_receipt_ref: string
  source_authority_caveat_ref: string | null
  status: 'draft' | 'ready' | 'redaction_required' | 'superseded' | 'archived'
  summary_ref: string
  updated_at: string
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

type ProofRow = Readonly<{
  acceptance_state_ref: string
  archived_at: string | null
  artifact_refs_json: string
  created_at: string
  economics_caveat_ref: string
  id: string
  idempotency_key: string
  legal_caveat_ref: string | null
  legal_sensitive: number
  metadata_json: string
  no_settlement_implication: number
  privacy_caveat_ref: string
  public_receipt_ref: string
  receipt_refs_json: string
  review_state_ref: string
  source_refs_json: string
  status: 'draft' | 'ready' | 'blocked' | 'superseded' | 'archived'
  updated_at: string
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

class BundleStore {
  evidence: Array<EvidenceRow> = []
  proofs: Array<ProofRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
  ]
}

class BundleStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: BundleStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_evidence_bundles')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.evidence.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.proofs.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
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
    if (this.query.includes('INSERT OR IGNORE INTO omni_evidence_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (this.store.evidence.every(item => item.idempotency_key !== idempotencyKey)) {
        this.store.evidence.push({
          archived_at: null,
          created_at: String(this.values[11]),
          entries_json: String(this.values[8]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[5]),
          metadata_json: String(this.values[10]),
          public_receipt_ref: String(this.values[9]),
          source_authority_caveat_ref: this.values[7] as string | null,
          status: this.values[4] as EvidenceRow['status'],
          summary_ref: String(this.values[6]),
          updated_at: String(this.values[12]),
          work_kind: this.values[3] as EvidenceRow['work_kind'],
          workroom_id: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (this.store.proofs.every(item => item.idempotency_key !== idempotencyKey)) {
        this.store.proofs.push({
          acceptance_state_ref: String(this.values[10]),
          archived_at: null,
          artifact_refs_json: String(this.values[7]),
          created_at: String(this.values[17]),
          economics_caveat_ref: String(this.values[11]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_caveat_ref: this.values[12] as string | null,
          legal_sensitive: Number(this.values[5]),
          metadata_json: String(this.values[16]),
          no_settlement_implication: Number(this.values[15]),
          privacy_caveat_ref: String(this.values[13]),
          public_receipt_ref: String(this.values[14]),
          receipt_refs_json: String(this.values[8]),
          review_state_ref: String(this.values[9]),
          source_refs_json: String(this.values[6]),
          status: this.values[4] as ProofRow['status'],
          updated_at: String(this.values[18]),
          work_kind: this.values[3] as ProofRow['work_kind'],
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
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const bundleDb = (store: BundleStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new BundleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

type TestEnv = Readonly<{ store: BundleStore }>

const evidenceReader = (
  store: BundleStore,
): ((db: D1Database, id: string) => Promise<OmniEvidenceBundleRecord | null>) =>
  (_db, id) => {
    const row = store.evidence.find(item => item.id === id && item.archived_at === null)

    if (row === undefined) {
      return Promise.resolve(null)
    }

    return Promise.resolve({
      archivedAt: row.archived_at,
      createdAt: row.created_at,
      entries: JSON.parse(row.entries_json),
      id: row.id,
      idempotencyKey: row.idempotency_key,
      legalSensitive: row.legal_sensitive === 1,
      metadata: JSON.parse(row.metadata_json),
      publicReceiptRef: row.public_receipt_ref,
      sourceAuthorityCaveatRef: row.source_authority_caveat_ref,
      status: row.status,
      summaryRef: row.summary_ref,
      updatedAt: row.updated_at,
      workKind: row.work_kind,
      workroomId: row.workroom_id,
    })
  }

const proofReader = (
  store: BundleStore,
): ((db: D1Database, id: string) => Promise<OmniPublicProofBundleRecord | null>) =>
  (_db, id) => {
    const row = store.proofs.find(item => item.id === id && item.archived_at === null)

    if (row === undefined) {
      return Promise.resolve(null)
    }

    return Promise.resolve({
      acceptanceStateRef: row.acceptance_state_ref,
      archivedAt: row.archived_at,
      artifactRefs: JSON.parse(row.artifact_refs_json),
      createdAt: row.created_at,
      economicsCaveatRef: row.economics_caveat_ref,
      id: row.id,
      idempotencyKey: row.idempotency_key,
      legalCaveatRef: row.legal_caveat_ref,
      legalSensitive: row.legal_sensitive === 1,
      metadata: JSON.parse(row.metadata_json),
      noSettlementImplication: row.no_settlement_implication === 1,
      privacyCaveatRef: row.privacy_caveat_ref,
      publicReceiptRef: row.public_receipt_ref,
      receiptRefs: JSON.parse(row.receipt_refs_json),
      reviewStateRef: row.review_state_ref,
      sourceRefs: JSON.parse(row.source_refs_json),
      status: row.status,
      updatedAt: row.updated_at,
      workKind: row.work_kind,
      workroomId: row.workroom_id,
    })
  }

const makeRoutes = (
  store: BundleStore,
  operatorAllowed: boolean,
): ReturnType<typeof makeOmniBundleRoutes<TestEnv>> => {
  const dependencies: OmniBundleRoutesDependencies<TestEnv> = {
    db: () => bundleDb(store),
    readEvidenceBundle: evidenceReader(store),
    readProofBundle: proofReader(store),
    requireOperator: () => Promise.resolve(operatorAllowed),
  }

  return makeOmniBundleRoutes<TestEnv>(dependencies)
}

const ctx = {} as ExecutionContext

const run = (
  effect: ReturnType<
    ReturnType<typeof makeRoutes>['routeOmniBundleRequest']
  >,
): Promise<Response> => {
  if (effect === undefined) {
    throw new Error('route returned undefined')
  }

  return Effect.runPromise(effect)
}

const evidenceBody = {
  entries: [
    {
      caveatRef: null,
      entryKind: 'build_log',
      publicSafe: false,
      redactionState: 'private_only',
      ref: 'site_build_log_1',
      required: true,
      sourceAuthority: 'system_receipt',
      summaryRef: 'build_log_summary_1',
      visibility: 'team',
    },
    {
      caveatRef: null,
      entryKind: 'deployment_url',
      publicSafe: true,
      redactionState: 'not_needed',
      ref: 'https://sites.openagents.com/otec/revisions/2',
      required: true,
      sourceAuthority: 'system_receipt',
      summaryRef: 'deployment_summary_2',
      visibility: 'public',
    },
    {
      caveatRef: null,
      entryKind: 'exa_source_card',
      publicSafe: true,
      redactionState: 'redacted',
      ref: 'exa_source_card_1',
      required: true,
      sourceAuthority: 'public_web',
      summaryRef: 'source_card_summary_1',
      visibility: 'customer',
    },
  ],
  id: 'omni_evidence_bundle_1',
  idempotencyKey: 'omni-evidence-bundle:site-1',
  status: 'ready',
  summaryRef: 'evidence_summary_revision_2',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
}

const proofBody = {
  acceptanceStateRef: 'acceptance_state_ready_1',
  artifactRefs: ['artifact_deploy_url_1'],
  economicsCaveatRef: 'economics_caveat_estimate_only_1',
  id: 'omni_public_proof_bundle_1',
  idempotencyKey: 'omni-public-proof-bundle:site-1',
  privacyCaveatRef: 'privacy_caveat_redacted_1',
  receiptRefs: ['receipt_public_1'],
  reviewStateRef: 'review_state_recorded_1',
  sourceRefs: ['source_card_1'],
  status: 'ready',
  workKind: 'site',
  workroomId: 'omni_workroom_site_1',
}

const jsonPost = (path: string, body: unknown): Request =>
  new Request(`https://openagents.com${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const get = (path: string): Request =>
  new Request(`https://openagents.com${path}`, { method: 'GET' })

describe('Omni bundle routes', () => {
  test('operator creates an evidence bundle, then reads operator vs customer-safe projections', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    const createResponse = await run(
      routes.routeOmniBundleRequest(
        jsonPost('/api/omni/evidence-bundles', evidenceBody),
        { store },
        ctx,
      ),
    )
    expect(createResponse.status).toBe(201)
    const created = (await createResponse.json()) as {
      bundle: ReturnType<typeof operatorOmniEvidenceBundleProjection>
    }
    expect(created.bundle.id).toBe('omni_evidence_bundle_1')

    const customerResponse = await run(
      routes.routeOmniBundleRequest(
        get('/api/omni/evidence-bundles/omni_evidence_bundle_1'),
        { store },
        ctx,
      ),
    )
    expect(customerResponse.status).toBe(200)
    const customer = (await customerResponse.json()) as {
      bundle: ReturnType<typeof customerOmniEvidenceBundleProjection>
      view: string
    }
    expect(customer.view).toBe('customer')
    expect(customer.bundle.entries.map(entry => entry.entryKind)).toEqual([
      'deployment_url',
      'exa_source_card',
    ])
    expect(JSON.stringify(customer.bundle)).not.toContain('build_log')
    expect(customer.bundle).not.toHaveProperty('metadata')

    const operatorResponse = await run(
      routes.routeOmniBundleRequest(
        get('/api/omni/evidence-bundles/omni_evidence_bundle_1?view=operator'),
        { store },
        ctx,
      ),
    )
    expect(operatorResponse.status).toBe(200)
    const operator = (await operatorResponse.json()) as {
      bundle: ReturnType<typeof operatorOmniEvidenceBundleProjection>
      view: string
    }
    expect(operator.view).toBe('operator')
    expect(operator.bundle.entries.map(entry => entry.entryKind)).toContain(
      'build_log',
    )
  })

  test('non-operator cannot create an evidence bundle or read operator projection', async () => {
    const store = new BundleStore()
    const blocked = makeRoutes(store, false)

    const createResponse = await run(
      blocked.routeOmniBundleRequest(
        jsonPost('/api/omni/evidence-bundles', evidenceBody),
        { store },
        ctx,
      ),
    )
    expect(createResponse.status).toBe(401)
    expect(store.evidence).toHaveLength(0)

    const allowed = makeRoutes(store, true)
    await run(
      allowed.routeOmniBundleRequest(
        jsonPost('/api/omni/evidence-bundles', evidenceBody),
        { store },
        ctx,
      ),
    )

    const operatorViewBlocked = await run(
      blocked.routeOmniBundleRequest(
        get('/api/omni/evidence-bundles/omni_evidence_bundle_1?view=operator'),
        { store },
        ctx,
      ),
    )
    expect(operatorViewBlocked.status).toBe(401)
  })

  test('operator creates a public proof bundle, public read omits operator-only material', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    const createResponse = await run(
      routes.routeOmniBundleRequest(
        jsonPost('/api/omni/public-proof-bundles', proofBody),
        { store },
        ctx,
      ),
    )
    expect(createResponse.status).toBe(201)
    const created = (await createResponse.json()) as {
      bundle: ReturnType<typeof operatorOmniProofBundleProjection>
    }
    expect(created.bundle.noSettlementImplication).toBe(true)

    const publicResponse = await run(
      routes.routeOmniBundleRequest(
        get('/api/omni/public-proof-bundles/omni_public_proof_bundle_1'),
        { store },
        ctx,
      ),
    )
    expect(publicResponse.status).toBe(200)
    const publicJson = (await publicResponse.json()) as {
      bundle: ReturnType<typeof publicOmniProofBundleProjection>
      view: string
    }
    expect(publicJson.view).toBe('public')
    expect(publicJson.bundle.noSettlementImplication).toBe(true)
    expect(publicJson.bundle).not.toHaveProperty('idempotencyKey')
    expect(publicJson.bundle).not.toHaveProperty('metadata')
  })

  test('renders a shareable public handoff page from the public proof projection', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    await run(
      routes.routeOmniBundleRequest(
        jsonPost('/api/omni/public-proof-bundles', {
          ...proofBody,
          artifactRefs: ['artifact_deploy_url_1', 'artifact_redacted_diff_1'],
          legalCaveatRef: 'legal_caveat_not_advice_1',
          metadata: {
            operatorOnly: 'do_not_render',
          },
          receiptRefs: ['receipt_public_1', 'receipt_review_gate_1'],
          sourceRefs: ['source_card_1', 'source_commit_1'],
        }),
        { store },
        ctx,
      ),
    )

    const response = await run(
      routes.routeOmniBundleRequest(
        get('/handoff/omni_public_proof_bundle_1'),
        { store },
        ctx,
      ),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toContain('text/html')

    const html = await response.text()
    expect(html).toContain('OpenAgents public handoff')
    expect(html).toContain('Redacted deliverables')
    expect(html).toContain('artifact_deploy_url_1')
    expect(html).toContain('artifact_redacted_diff_1')
    expect(html).toContain('receipt_review_gate_1')
    expect(html).toContain('source_commit_1')
    expect(html).toContain('legal_caveat_not_advice_1')
    expect(html).toContain('/api/omni/public-proof-bundles/omni_public_proof_bundle_1')
    expect(html).not.toContain('do_not_render')
    expect(html).not.toContain('idempotencyKey')
  })

  test('rejects work-kind-mismatched proof bundles and unknown bundle ids', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    const mismatch = await run(
      routes.routeOmniBundleRequest(
        jsonPost('/api/omni/public-proof-bundles', {
          ...proofBody,
          workKind: 'coding',
        }),
        { store },
        ctx,
      ),
    )
    expect(mismatch.status).toBe(400)

    const missing = await run(
      routes.routeOmniBundleRequest(
        get('/api/omni/evidence-bundles/does_not_exist'),
        { store },
        ctx,
      ),
    )
    expect(missing.status).toBe(404)
  })

  test('rejects proof bundles carrying settlement or payout material', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniBundleRequest(
        jsonPost('/api/omni/public-proof-bundles', {
          ...proofBody,
          idempotencyKey: 'omni-public-proof-bundle:unsafe-source',
          sourceRefs: ['payout_settlement_ref'],
        }),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(400)
    expect(store.proofs).toHaveLength(0)
  })

  test('returns 405 for unsupported methods on bundle collections', async () => {
    const store = new BundleStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniBundleRequest(
        get('/api/omni/evidence-bundles'),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(405)
  })
})
