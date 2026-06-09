import { describe, expect, test } from 'vitest'

import {
  TargetedSiteQualityAuditValidationError,
  evaluateTargetedSiteQualityAudit,
  listTargetedSiteQualityAuditsByCampaign,
  listTargetedSiteQualityAuditsByDomain,
  listTargetedSiteQualityAuditsByProspect,
  operatorTargetedSiteQualityAuditProjection,
  publicTargetedSiteQualityAuditProjection,
  recordTargetedSiteQualityAudit,
  type RecordTargetedSiteQualityAuditInput,
} from './targeted-site-quality-audit'

type StoredQualityAudit = Readonly<{
  archived_at: string | null
  audited_at: string
  campaign_id: string
  created_at: string
  dimensions_json: string
  evidence_refs_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  overall_score: number
  prospect_id: string | null
  provider_adapter_run_id: string | null
  recommendation: 'skip' | 'monitor' | 'remake_candidate' | 'manual_review' | 'blocked'
  rendered_capture_run_id: string | null
  state: 'draft' | 'ready' | 'manual_review' | 'blocked' | 'archived'
  static_capture_run_id: string | null
}>

class QualityAuditStore {
  rows: Array<StoredQualityAudit> = []
}

class QualityAuditStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: QualityAuditStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.rows.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO targeted_site_quality_audits')) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          audited_at: String(this.values[15]),
          campaign_id: String(this.values[2]),
          created_at: String(this.values[16]),
          dimensions_json: String(this.values[12]),
          evidence_refs_json: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[11]),
          metadata_json: String(this.values[14]),
          normalized_domain: String(this.values[4]),
          overall_score: Number(this.values[10]),
          prospect_id: this.values[3] as string | null,
          provider_adapter_run_id: this.values[7] as string | null,
          recommendation: this.values[9] as StoredQualityAudit['recommendation'],
          rendered_capture_run_id: this.values[6] as string | null,
          state: this.values[8] as StoredQualityAudit['state'],
          static_capture_run_id: this.values[5] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_quality_audits')) {
      const value = String(this.values[0])
      const key = this.query.includes('campaign_id = ?')
        ? 'campaign_id'
        : this.query.includes('prospect_id = ?')
          ? 'prospect_id'
          : 'normalized_domain'
      const limit = Number(this.values[1] ?? 100)
      const rows = this.store.rows
        .filter(row => row.archived_at === null && row[key] === value)
        .slice(0, limit)

      return Promise.resolve({
        results: rows as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
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

const qualityAuditDb = (store: QualityAuditStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new QualityAuditStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const weakSiteDimensions = {
  accessibility: 55,
  contentQuality: 60,
  ctaClarity: 40,
  designAge: 35,
  imageQuality: 45,
  informationArchitecture: 50,
  legalSensitiveClaims: 0,
  localSeoMetadata: 45,
  mobileResponsiveRisk: 50,
  performanceRisk: 65,
  staleBrokenMixedContent: 55,
  trustSignals: 50,
}

const baseInput = {
  campaignId: 'targeted_site_campaign_texas_energy',
  dimensions: weakSiteDimensions,
  evidenceRefs: ['static_capture_ref_1', 'rendered_capture_ref_1'],
  id: 'targeted_site_quality_audit_otec_1',
  idempotencyKey: 'targeted-site-quality-audit:otec:1',
  metadata: { scorer: 'operator_seed' },
  normalizedDomain: 'otec.example',
  prospectId: 'targeted_site_prospect_otec',
  providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
  renderedCaptureRunId: 'targeted_site_rendered_capture_otec_1',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
} satisfies RecordTargetedSiteQualityAuditInput

describe('targeted Site quality audits', () => {
  test('evaluates bounded score and recommendation states', () => {
    expect(
      evaluateTargetedSiteQualityAudit({
        dimensions: {
          accessibility: -10,
          contentQuality: 110,
          ctaClarity: 60,
        },
      }),
    ).toMatchObject({
      dimensions: {
        accessibility: 0,
        contentQuality: 100,
        ctaClarity: 60,
      },
    })

    expect(evaluateTargetedSiteQualityAudit({ dimensions: weakSiteDimensions }))
      .toMatchObject({
        recommendation: 'remake_candidate',
        state: 'ready',
      })

    expect(
      evaluateTargetedSiteQualityAudit({
        dimensions: { ...weakSiteDimensions, legalSensitiveClaims: 30 },
      }),
    ).toMatchObject({
      legalSensitive: true,
      recommendation: 'manual_review',
      state: 'manual_review',
    })

    expect(
      evaluateTargetedSiteQualityAudit({
        blocked: true,
        dimensions: weakSiteDimensions,
      }),
    ).toMatchObject({
      recommendation: 'blocked',
      state: 'blocked',
    })
  })

  test('records idempotent quality audits and lists them by campaign, prospect, and domain', async () => {
    const store = new QualityAuditStore()
    const db = qualityAuditDb(store)
    const first = await recordTargetedSiteQualityAudit(db, baseInput)
    const replay = await recordTargetedSiteQualityAudit(db, {
      ...baseInput,
      dimensions: { accessibility: 100 },
      id: 'targeted_site_quality_audit_replay',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      evidenceRefs: ['static_capture_ref_1', 'rendered_capture_ref_1'],
      legalSensitive: false,
      normalizedDomain: 'otec.example',
      recommendation: 'remake_candidate',
      state: 'ready',
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteQualityAuditsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteQualityAuditsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteQualityAuditsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('redacts public projection and keeps operator projection evidence-only', async () => {
    const audit = await recordTargetedSiteQualityAudit(
      qualityAuditDb(new QualityAuditStore()),
      baseInput,
    )
    const publicProjection = publicTargetedSiteQualityAuditProjection(audit)
    const operatorProjection = operatorTargetedSiteQualityAuditProjection(audit)

    expect(publicProjection).toEqual({
      auditedAt: audit.auditedAt,
      campaignId: 'targeted_site_campaign_texas_energy',
      evidenceCount: 2,
      legalSensitive: false,
      normalizedDomain: 'otec.example',
      overallScore: audit.overallScore,
      prospectId: 'targeted_site_prospect_otec',
      recommendation: 'remake_candidate',
      state: 'ready',
    })
    expect(publicProjection).not.toHaveProperty('evidenceRefs')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      evidenceRefs: ['static_capture_ref_1', 'rendered_capture_ref_1'],
      hasMetadata: true,
      providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('requires evidence and rejects private capture/provider/contact/payment material', async () => {
    await expect(
      recordTargetedSiteQualityAudit(qualityAuditDb(new QualityAuditStore()), {
        ...baseInput,
        evidenceRefs: [],
        idempotencyKey: 'targeted-site-quality-audit:no-evidence',
      }),
    ).rejects.toMatchObject({
      reason: 'at least one evidence ref is required for quality audits.',
    })

    await expect(
      recordTargetedSiteQualityAudit(qualityAuditDb(new QualityAuditStore()), {
        ...baseInput,
        evidenceRefs: ['provider_payload_raw_1'],
        idempotencyKey: 'targeted-site-quality-audit:unsafe-evidence',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteQualityAuditValidationError)

    await expect(
      recordTargetedSiteQualityAudit(qualityAuditDb(new QualityAuditStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-quality-audit:unsafe-metadata',
        metadata: { contact: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteQualityAuditValidationError)

    await expect(
      recordTargetedSiteQualityAudit(qualityAuditDb(new QualityAuditStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-quality-audit:unsafe-payment',
        staticCaptureRunId: 'lnbc_private_invoice',
      }),
    ).rejects.toBeInstanceOf(TargetedSiteQualityAuditValidationError)
  })
})
