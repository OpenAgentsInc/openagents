import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OmniEvidenceBundleValidationError,
  OmniEvidenceBundleWorkroomNotFound,
  createOmniEvidenceBundle,
  customerOmniEvidenceBundleProjection,
  operatorOmniEvidenceBundleProjection,
  publicOmniEvidenceBundleProjection,
  type OmniEvidenceBundleEntry,
} from './omni-evidence-bundles'

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

type BundleRow = Readonly<{
  archived_at: string | null
  created_at: string
  entries_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  public_receipt_ref: string
  source_authority_caveat_ref: string | null
  status:
    | 'draft'
    | 'ready'
    | 'redaction_required'
    | 'superseded'
    | 'archived'
  summary_ref: string
  updated_at: string
  work_kind: WorkroomRow['work_kind']
  workroom_id: string
}>

class EvidenceBundleStore {
  bundles: Array<BundleRow> = []
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
  makeBundleId: () => 'omni_evidence_bundle_generated',
  nowIso: () => '2026-06-06T00:58:00.000Z',
}

class EvidenceBundleStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: EvidenceBundleStore,
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
    if (this.query.includes('INSERT OR IGNORE INTO omni_evidence_bundles')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.bundles.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.bundles.push({
          archived_at: null,
          created_at: String(this.values[11]),
          entries_json: String(this.values[8]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[5]),
          metadata_json: String(this.values[10]),
          public_receipt_ref: String(this.values[9]),
          source_authority_caveat_ref: this.values[7] as string | null,
          status: this.values[4] as BundleRow['status'],
          summary_ref: String(this.values[6]),
          updated_at: String(this.values[12]),
          work_kind: this.values[3] as BundleRow['work_kind'],
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

const evidenceBundleDb = (store: EvidenceBundleStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new EvidenceBundleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const siteEntries: ReadonlyArray<OmniEvidenceBundleEntry> = [
  {
    caveatRef: null,
    entryKind: 'exa_source_card',
    publicSafe: true,
    redactionState: 'redacted',
    ref: 'exa_source_card_otec_1',
    required: true,
    sourceAuthority: 'public_web',
    summaryRef: 'source_card_summary_otec_1',
    visibility: 'customer',
  },
  {
    caveatRef: null,
    entryKind: 'research_brief',
    publicSafe: true,
    redactionState: 'redacted',
    ref: 'research_brief_otec_1',
    required: true,
    sourceAuthority: 'operator_reviewed',
    summaryRef: 'research_brief_summary_otec_1',
    visibility: 'public',
  },
  {
    caveatRef: null,
    entryKind: 'build_log',
    publicSafe: false,
    redactionState: 'private_only',
    ref: 'site_build_log_otec_1',
    required: true,
    sourceAuthority: 'system_receipt',
    summaryRef: 'build_log_summary_otec_1',
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
    summaryRef: 'deployment_summary_otec_2',
    visibility: 'public',
  },
]

const create = (
  store: EvidenceBundleStore,
  overrides: Partial<Parameters<typeof createOmniEvidenceBundle>[1]> = {},
) =>
  Effect.runPromise(
    createOmniEvidenceBundle(
      evidenceBundleDb(store),
      {
        entries: siteEntries,
        id: 'omni_evidence_bundle_1',
        idempotencyKey: 'omni-evidence-bundle:site-1',
        metadata: { generator: 'omni_evidence_bundle_test' },
        status: 'ready',
        summaryRef: 'evidence_summary_otec_revision_2',
        workKind: 'site',
        workroomId: 'omni_workroom_site_1',
        ...overrides,
      },
      runtime,
    ),
  )

describe('Omni evidence bundles', () => {
  test('creates idempotent Site evidence bundles with split projections', async () => {
    const store = new EvidenceBundleStore()
    const bundle = await create(store)
    const replay = await create(store, {
      summaryRef: 'different_summary_ref',
    })
    const publicProjection = publicOmniEvidenceBundleProjection(bundle)
    const customer = customerOmniEvidenceBundleProjection(bundle)
    const operator = operatorOmniEvidenceBundleProjection(bundle)

    expect(replay.summaryRef).toBe('evidence_summary_otec_revision_2')
    expect(publicProjection.entries.map(entry => entry.entryKind)).toEqual([
      'research_brief',
      'deployment_url',
    ])
    expect(customer.entries.map(entry => entry.entryKind)).toEqual([
      'exa_source_card',
      'research_brief',
      'deployment_url',
    ])
    expect(operator.entries.map(entry => entry.entryKind)).toContain(
      'build_log',
    )
    expect(operator.metadata).toEqual({
      generator: 'omni_evidence_bundle_test',
    })
  })

  test('supports PR-style coding evidence without Site artifacts', async () => {
    const store = new EvidenceBundleStore()
    const bundle = await create(store, {
      entries: [
        {
          caveatRef: 'source_authority_customer_repo_access',
          entryKind: 'source_commit',
          publicSafe: true,
          redactionState: 'redacted',
          ref: 'commit_abc123',
          required: true,
          sourceAuthority: 'github',
          summaryRef: 'commit_summary_abc123',
          visibility: 'customer',
        },
        {
          caveatRef: 'source_authority_customer_repo_access',
          entryKind: 'diff',
          publicSafe: true,
          redactionState: 'redacted',
          ref: 'diff_pr_42',
          required: true,
          sourceAuthority: 'github',
          summaryRef: 'diff_summary_pr_42',
          visibility: 'customer',
        },
        {
          caveatRef: null,
          entryKind: 'email_receipt',
          publicSafe: true,
          redactionState: 'redacted',
          ref: 'email_message_review_ready_42',
          required: true,
          sourceAuthority: 'system_receipt',
          summaryRef: 'email_summary_review_ready_42',
          visibility: 'customer',
        },
      ],
      id: 'omni_evidence_bundle_coding_1',
      idempotencyKey: 'omni-evidence-bundle:coding-1',
      summaryRef: 'evidence_summary_coding_pr_42',
      workKind: 'coding',
      workroomId: 'omni_workroom_coding_1',
    })

    expect(bundle.workKind).toBe('coding')
    expect(customerOmniEvidenceBundleProjection(bundle).entries).toHaveLength(3)
  })

  test('requires legal-sensitive evidence to carry caveats and redaction reports', async () => {
    const store = new EvidenceBundleStore()

    await expect(
      create(store, {
        entries: [
          {
            caveatRef: null,
            entryKind: 'research_brief',
            publicSafe: true,
            redactionState: 'redacted',
            ref: 'legal_research_brief_1',
            required: true,
            sourceAuthority: 'operator_reviewed',
            summaryRef: 'legal_research_summary_1',
            visibility: 'customer',
          },
        ],
        idempotencyKey: 'omni-evidence-bundle:legal-missing-redaction',
        legalSensitive: true,
        workKind: 'legal_sensitive',
        workroomId: 'omni_workroom_legal_1',
      }),
    ).rejects.toBeInstanceOf(OmniEvidenceBundleValidationError)

    const bundle = await create(store, {
      entries: [
        {
          caveatRef: 'legal_review_caveat_1',
          entryKind: 'research_brief',
          publicSafe: true,
          redactionState: 'redacted',
          ref: 'legal_research_brief_1',
          required: true,
          sourceAuthority: 'operator_reviewed',
          summaryRef: 'legal_research_summary_1',
          visibility: 'customer',
        },
        {
          caveatRef: 'legal_review_caveat_1',
          entryKind: 'redaction_report',
          publicSafe: true,
          redactionState: 'redacted',
          ref: 'legal_redaction_report_1',
          required: true,
          sourceAuthority: 'system_receipt',
          summaryRef: 'legal_redaction_summary_1',
          visibility: 'customer',
        },
      ],
      idempotencyKey: 'omni-evidence-bundle:legal-ok',
      legalSensitive: true,
      sourceAuthorityCaveatRef: 'legal_source_authority_caveat_1',
      workKind: 'legal_sensitive',
      workroomId: 'omni_workroom_legal_1',
    })

    expect(bundle.legalSensitive).toBe(true)
    expect(customerOmniEvidenceBundleProjection(bundle).entries).toHaveLength(2)
  })

  test('rejects missing workrooms, work kind mismatch, and unsafe material', async () => {
    await expect(
      create(new EvidenceBundleStore(), {
        workroomId: 'omni_workroom_missing',
      }),
    ).rejects.toBeInstanceOf(OmniEvidenceBundleWorkroomNotFound)

    await expect(
      create(new EvidenceBundleStore(), {
        workKind: 'coding',
      }),
    ).rejects.toBeInstanceOf(OmniEvidenceBundleValidationError)

    await expect(
      create(new EvidenceBundleStore(), {
        metadata: { rawEmail: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(OmniEvidenceBundleValidationError)

    await expect(
      create(new EvidenceBundleStore(), {
        entries: [
          {
            caveatRef: null,
            entryKind: 'exa_source_card',
            publicSafe: true,
            redactionState: 'redacted',
            ref: 'lnbc1unsafeinvoice',
            required: true,
            sourceAuthority: 'public_web',
            summaryRef: 'source_card_summary_otec_1',
            visibility: 'customer',
          },
        ],
      }),
    ).rejects.toBeInstanceOf(OmniEvidenceBundleValidationError)
  })
})
