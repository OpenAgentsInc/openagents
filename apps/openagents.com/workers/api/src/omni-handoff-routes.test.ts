import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OmniHandoffRoutesDependencies,
  makeOmniHandoffRoutes,
} from './omni-handoff-routes'

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

type EvidenceRow = Readonly<Record<string, unknown>>
type ProofRow = Readonly<Record<string, unknown>>

class HandoffStore {
  evidence: Array<EvidenceRow> = []
  proofs: Array<ProofRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
  ]
}

class HandoffStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: HandoffStore,
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
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.proofs.find(
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
    if (this.query.includes('INSERT OR IGNORE INTO omni_evidence_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.evidence.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
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
          status: this.values[4],
          summary_ref: String(this.values[6]),
          updated_at: String(this.values[12]),
          work_kind: this.values[3],
          workroom_id: String(this.values[2]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.proofs.every(item => item.idempotency_key !== idempotencyKey)
      ) {
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
          status: this.values[4],
          updated_at: String(this.values[18]),
          work_kind: this.values[3],
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

const handoffDb = (store: HandoffStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new HandoffStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

type TestEnv = Readonly<{ store: HandoffStore }>

const makeRoutes = (
  store: HandoffStore,
  operatorAllowed: boolean,
): ReturnType<typeof makeOmniHandoffRoutes<TestEnv>> => {
  const dependencies: OmniHandoffRoutesDependencies<TestEnv> = {
    db: () => handoffDb(store),
    requireOperator: () => Promise.resolve(operatorAllowed),
  }

  return makeOmniHandoffRoutes<TestEnv>(dependencies)
}

const ctx = {} as ExecutionContext

const run = (
  effect: ReturnType<
    ReturnType<typeof makeRoutes>['routeOmniHandoffRequest']
  >,
): Promise<Response> => {
  if (effect === undefined) {
    throw new Error('route returned undefined')
  }

  return Effect.runPromise(effect)
}

const handoffBody = {
  evidence: {
    entries: [
      {
        caveatRef: null,
        entryKind: 'build_log',
        publicSafe: false,
        redactionState: 'private_only',
        ref: 'internal_build_log_1',
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
    ],
    idempotencyKey: 'omni-handoff-evidence:site-1',
    summaryRef: 'evidence_summary_handoff_1',
  },
  proof: {
    acceptanceStateRef: 'acceptance_state_accepted_1',
    economicsCaveatRef: 'economics_caveat_estimate_only_1',
    privacyCaveatRef: 'privacy_caveat_redacted_1',
    reviewStateRef: 'review_state_recorded_1',
  },
  proofIdempotencyKey: 'omni-handoff-proof:site-1',
  workKind: 'site',
  workroomState: 'completed',
}

const jsonPost = (path: string, body: unknown): Request =>
  new Request(`https://openagents.com${path}`, {
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const rawPost = (path: string, body: string): Request =>
  new Request(`https://openagents.com${path}`, {
    body,
    headers: { 'content-type': 'application/json' },
    method: 'POST',
  })

const get = (path: string): Request =>
  new Request(`https://openagents.com${path}`, { method: 'GET' })

const HANDOFF_PATH = '/api/omni/workrooms/omni_workroom_site_1/handoff'

describe('Omni handoff routes', () => {
  test('operator posts a handoff, chaining evidence into a redacted proof bundle', async () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniHandoffRequest(
        jsonPost(HANDOFF_PATH, handoffBody),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(201)

    const json = (await response.json()) as {
      handoff: {
        evidence: Record<string, unknown>
        proof: {
          artifactRefs: ReadonlyArray<string>
          noSettlementImplication: boolean
          sourceRefs: ReadonlyArray<string>
        }
      }
      proofBundleId: string
    }

    expect(store.evidence).toHaveLength(1)
    expect(store.proofs).toHaveLength(1)
    expect(json.proofBundleId).not.toBe('')
    expect(json.handoff.proof.artifactRefs).toEqual([
      'https://sites.openagents.com/otec/revisions/2',
    ])
    expect(json.handoff.proof.noSettlementImplication).toBe(true)
    // Private team build log must not leak through the client-facing handoff.
    expect(JSON.stringify(json.handoff)).not.toContain('internal_build_log_1')
  })

  test('operator gate: non-operator is rejected with 401 and writes nothing', async () => {
    const store = new HandoffStore()
    const blocked = makeRoutes(store, false)

    const response = await run(
      blocked.routeOmniHandoffRequest(
        jsonPost(HANDOFF_PATH, handoffBody),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(401)
    expect(store.evidence).toHaveLength(0)
    expect(store.proofs).toHaveLength(0)
  })

  test('rejects malformed JSON through the Effect boundary', async () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniHandoffRequest(
        rawPost(HANDOFF_PATH, '{"evidence":'),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'omni_handoff_request_error',
      reason: 'Malformed JSON request body.',
    })
    expect(store.evidence).toHaveLength(0)
    expect(store.proofs).toHaveLength(0)
  })

  test('rejects schema-invalid handoff bodies through the Effect boundary', async () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniHandoffRequest(
        jsonPost(HANDOFF_PATH, { evidence: {} }),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'omni_handoff_request_error',
      reason: 'Omni handoff request did not match the expected schema.',
    })
    expect(store.evidence).toHaveLength(0)
    expect(store.proofs).toHaveLength(0)
  })

  test('returns 405 for non-POST methods on the handoff path', async () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniHandoffRequest(get(HANDOFF_PATH), { store }, ctx),
    )
    expect(response.status).toBe(405)
  })

  test('returns undefined for unrelated paths', () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    expect(
      routes.routeOmniHandoffRequest(get('/api/omni/other'), { store }, ctx),
    ).toBeUndefined()
  })

  test('rejects a handoff for an unknown workroom with 404', async () => {
    const store = new HandoffStore()
    const routes = makeRoutes(store, true)

    const response = await run(
      routes.routeOmniHandoffRequest(
        jsonPost('/api/omni/workrooms/does_not_exist/handoff', handoffBody),
        { store },
        ctx,
      ),
    )
    expect(response.status).toBe(404)
  })
})
