import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type OmniHandoffInput,
  type OmniHandoffRuntime,
  customerOmniHandoffProjection,
  runOmniWorkroomHandoff,
} from './omni-handoff'

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
    { archived_at: null, id: 'omni_workroom_coding_1', work_kind: 'coding' },
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

let counter = 0
const runtime: OmniHandoffRuntime = {
  evidenceRuntime: {
    makeBundleId: () => `omni_evidence_bundle_${(counter += 1)}`,
    nowIso: () => '2026-06-14T00:00:00.000Z',
  },
  proofRuntime: {
    makeProofBundleId: () => `omni_public_proof_bundle_${(counter += 1)}`,
    nowIso: () => '2026-06-14T00:00:00.000Z',
  },
}

// A workroom with mixed evidence: a private build log + raw email receipt
// (must stay internal), and public-safe deployment URL, source commit, and a
// public receipt + redaction report (eligible for the customer handoff).
const handoffInput = (
  overrides: Partial<OmniHandoffInput> = {},
): OmniHandoffInput => ({
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
        entryKind: 'email_receipt',
        publicSafe: false,
        redactionState: 'private_only',
        ref: 'internal_email_receipt_1',
        required: false,
        sourceAuthority: 'system_receipt',
        summaryRef: 'email_receipt_summary_1',
        visibility: 'private',
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
        entryKind: 'source_commit',
        publicSafe: true,
        redactionState: 'not_needed',
        ref: 'source_commit_abc123',
        required: true,
        sourceAuthority: 'github',
        summaryRef: 'source_commit_summary_1',
        visibility: 'public',
      },
      {
        caveatRef: null,
        entryKind: 'receipt',
        publicSafe: true,
        redactionState: 'not_needed',
        ref: 'public_receipt_1',
        required: true,
        sourceAuthority: 'system_receipt',
        summaryRef: 'receipt_summary_1',
        visibility: 'public',
      },
    ],
    idempotencyKey: 'omni-handoff-evidence:site-1',
    summaryRef: 'evidence_summary_handoff_1',
    ...overrides.evidence,
  },
  proof: {
    acceptanceStateRef: 'acceptance_state_accepted_1',
    economicsCaveatRef: 'economics_caveat_estimate_only_1',
    privacyCaveatRef: 'privacy_caveat_redacted_1',
    reviewStateRef: 'review_state_recorded_1',
    ...overrides.proof,
  },
  proofIdempotencyKey: 'omni-handoff-proof:site-1',
  workroom: {
    state: 'completed',
    workKind: 'site',
    workroomId: 'omni_workroom_site_1',
    ...overrides.workroom,
  },
})

describe('Omni workroom handoff orchestration', () => {
  test('chains evidence bundle into a redacted public proof bundle', async () => {
    const store = new HandoffStore()
    const result = await Effect.runPromise(
      runOmniWorkroomHandoff(handoffDb(store), handoffInput(), runtime),
    )

    // Evidence bundle persisted with the full entry set.
    expect(store.evidence).toHaveLength(1)
    expect(result.evidenceBundle.entries).toHaveLength(5)

    // Proof bundle persisted and derived from public-safe public entries only.
    expect(store.proofs).toHaveLength(1)
    expect(result.proofBundle.sourceRefs).toEqual(['source_commit_abc123'])
    expect(result.proofBundle.artifactRefs).toEqual([
      'https://sites.openagents.com/otec/revisions/2',
    ])
    expect(result.proofBundle.receiptRefs).toEqual(['public_receipt_1'])
    expect(result.proofBundle.noSettlementImplication).toBe(true)
  })

  test('redaction enforced: private/team entries never reach the proof bundle', async () => {
    const store = new HandoffStore()
    const result = await Effect.runPromise(
      runOmniWorkroomHandoff(handoffDb(store), handoffInput(), runtime),
    )

    const proofJson = JSON.stringify(result.publicProofProjection)
    expect(proofJson).not.toContain('internal_build_log_1')
    expect(proofJson).not.toContain('internal_email_receipt_1')

    // The customer projection exposes the customer-safe evidence view, never
    // the private team/build-log entry.
    const projection = customerOmniHandoffProjection(result)
    const evidenceJson = JSON.stringify(projection.evidence)
    expect(evidenceJson).not.toContain('internal_build_log_1')
    expect(evidenceJson).not.toContain('internal_email_receipt_1')
    expect(projection.evidence).not.toHaveProperty('metadata')
  })

  test('rejects a workroom that is not completed or accepted', async () => {
    const store = new HandoffStore()
    const result = await Effect.runPromiseExit(
      runOmniWorkroomHandoff(
        handoffDb(store),
        handoffInput({
          // @ts-expect-error exercising the runtime guard with a bad state
          workroom: { state: 'in_progress', workKind: 'site', workroomId: 'omni_workroom_site_1' },
        }),
        runtime,
      ),
    )

    expect(result._tag).toBe('Failure')
    expect(store.evidence).toHaveLength(0)
    expect(store.proofs).toHaveLength(0)
  })

  test('rejects settlement/payout material smuggled in via evidence refs', async () => {
    const store = new HandoffStore()
    const result = await Effect.runPromiseExit(
      runOmniWorkroomHandoff(
        handoffDb(store),
        handoffInput({
          evidence: {
            entries: [
              {
                caveatRef: null,
                entryKind: 'receipt',
                publicSafe: true,
                redactionState: 'not_needed',
                ref: 'payout_settlement_receipt_1',
                required: true,
                sourceAuthority: 'system_receipt',
                summaryRef: 'receipt_summary_1',
                visibility: 'public',
              },
            ],
            idempotencyKey: 'omni-handoff-evidence:unsafe',
            summaryRef: 'evidence_summary_handoff_unsafe',
          },
        }),
        runtime,
      ),
    )

    // The proof bundle service rejects payout/settlement refs as a defect.
    expect(result._tag).toBe('Failure')
    expect(store.proofs).toHaveLength(0)
  })

  test('is idempotent on repeated handoff for the same workroom', async () => {
    const store = new HandoffStore()
    await Effect.runPromise(
      runOmniWorkroomHandoff(handoffDb(store), handoffInput(), runtime),
    )
    await Effect.runPromise(
      runOmniWorkroomHandoff(handoffDb(store), handoffInput(), runtime),
    )

    expect(store.evidence).toHaveLength(1)
    expect(store.proofs).toHaveLength(1)
  })
})
