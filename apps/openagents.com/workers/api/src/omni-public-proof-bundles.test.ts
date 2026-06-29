import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniPublicProofBundleValidationError,
  OmniPublicProofBundleWorkroomNotFound,
  createOmniPublicProofBundle,
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

type ProofBundleRow = Readonly<{
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

class ProofBundleStore {
  bundles: Array<ProofBundleRow> = []
  workrooms: Array<WorkroomRow> = [
    { archived_at: null, id: 'omni_workroom_site_1', work_kind: 'site' },
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
    {
      archived_at: null,
      id: 'omni_workroom_legal_1',
      work_kind: 'legal_sensitive',
    },
  ]
}

const runtime = {
  makeProofBundleId: () => 'omni_public_proof_bundle_generated',
  nowIso: () => '2026-06-06T02:20:00.000Z',
}

class ProofBundleStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ProofBundleStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.bundles.find(
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
    if (this.query.includes('INSERT OR IGNORE INTO omni_public_proof_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.bundles.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.bundles.push({
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
          status: this.values[4] as ProofBundleRow['status'],
          updated_at: String(this.values[18]),
          work_kind: this.values[3] as ProofBundleRow['work_kind'],
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

const proofBundleDb = (store: ProofBundleStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ProofBundleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const create = (
  store: ProofBundleStore,
  overrides: Partial<Parameters<typeof createOmniPublicProofBundle>[1]> = {},
) =>
  Effect.runPromise(
    createOmniPublicProofBundle(
      proofBundleDb(store),
      {
        acceptanceStateRef: 'acceptance_state_revision_requested',
        artifactRefs: ['site_version_ref_2'],
        economicsCaveatRef: 'economics_caveat_internal_no_claim',
        id: 'omni_public_proof_bundle_1',
        idempotencyKey: 'omni-proof:site-1',
        privacyCaveatRef: 'privacy_caveat_public_safe',
        receiptRefs: ['receipt_site_revision_requested_1'],
        reviewStateRef: 'review_state_customer_visible',
        sourceRefs: ['research_brief_otec_1'],
        status: 'ready',
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni public proof bundles', () => {
  test('creates idempotent public-safe Site proof bundles', async () => {
    const store = new ProofBundleStore()
    const bundle = await create(store)
    const replay = await create(store, {
      artifactRefs: ['changed_artifact_ref'],
    })
    const publicProjection = publicOmniProofBundleProjection(bundle)
    const operator = operatorOmniProofBundleProjection(bundle)

    expect(replay.artifactRefs).toEqual(['site_version_ref_2'])
    expect(publicProjection).toEqual({
      acceptanceStateRef: 'acceptance_state_revision_requested',
      artifactRefs: ['site_version_ref_2'],
      economicsCaveatRef: 'economics_caveat_internal_no_claim',
      legalCaveatRef: null,
      noSettlementImplication: true,
      privacyCaveatRef: 'privacy_caveat_public_safe',
      publicReceiptRef: 'omni_public_proof_bundle:omni_workroom_site_1:omni-proof:site-1',
      receiptRefs: ['receipt_site_revision_requested_1'],
      reviewStateRef: 'review_state_customer_visible',
      sourceRefs: ['research_brief_otec_1'],
      status: 'ready',
      workKind: 'site',
      workroomId: 'omni_workroom_site_1',
    })
    expect(operator.metadata).toEqual({})
  })

  test('supports coding proof bundles', async () => {
    const bundle = await create(new ProofBundleStore(), {
      artifactRefs: ['pull_request_ref_42'],
      id: 'omni_public_proof_bundle_coding_1',
      idempotencyKey: 'omni-proof:coding-1',
      sourceRefs: ['source_commit_ref_42'],
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    })

    expect(bundle.workKind).toBe('coding')
    expect(bundle.artifactRefs).toEqual(['pull_request_ref_42'])
  })

  test('requires legal caveats for legal-sensitive proof bundles', async () => {
    await expect(
      create(new ProofBundleStore(), {
        idempotencyKey: 'omni-proof:legal-missing-caveat',
        legalSensitive: true,
        workKind: 'legal_sensitive',
        workroomId: 'omni_workroom_legal_1',
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleValidationError)

    await expect(
      create(new ProofBundleStore(), {
        id: 'omni_public_proof_bundle_legal_1',
        idempotencyKey: 'omni-proof:legal-ok',
        legalCaveatRef: 'legal_caveat_attorney_review_required',
        legalSensitive: true,
        workKind: 'legal_sensitive',
        workroomId: 'omni_workroom_legal_1',
      }),
    ).resolves.toMatchObject({
      legalCaveatRef: 'legal_caveat_attorney_review_required',
      legalSensitive: true,
    })
  })

  test('rejects missing workrooms, mismatches, private material, and settlement claims', async () => {
    await expect(
      create(new ProofBundleStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleWorkroomNotFound)

    await expect(
      create(new ProofBundleStore(), {
        workKind: 'coding',
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleValidationError)

    await expect(
      create(new ProofBundleStore(), {
        sourceRefs: ['raw_run_log_private'],
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleValidationError)

    await expect(
      create(new ProofBundleStore(), {
        metadata: { customerEmail: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleValidationError)

    await expect(
      create(new ProofBundleStore(), {
        receiptRefs: ['settlement_receipt_paid_out_1'],
      }),
    ).rejects.toBeInstanceOf(OmniPublicProofBundleValidationError)
  })
})
