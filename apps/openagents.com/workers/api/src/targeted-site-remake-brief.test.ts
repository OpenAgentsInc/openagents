import { describe, expect, test } from 'vitest'

import {
  TargetedSiteRemakeBriefValidationError,
  listTargetedSiteRemakeBriefsByCampaign,
  listTargetedSiteRemakeBriefsByDomain,
  listTargetedSiteRemakeBriefsByProspect,
  operatorTargetedSiteRemakeBriefProjection,
  publicTargetedSiteRemakeBriefProjection,
  recordTargetedSiteRemakeBrief,
  type RecordTargetedSiteRemakeBriefInput,
} from './targeted-site-remake-brief'
import type { TargetedSiteQualityAuditRecord } from './targeted-site-quality-audit'

type StoredRemakeBrief = Readonly<{
  archived_at: string | null
  audit_finding_refs_json: string
  campaign_id: string
  copied_image_refs_json: string
  copied_text_refs_json: string
  created_at: string
  generation_constraints_json: string
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  original_screenshot_refs_json: string
  prepared_at: string
  prospect_id: string | null
  provider_adapter_run_id: string | null
  quality_audit_id: string
  rendered_capture_run_id: string | null
  reviewed_at: string | null
  source_authority_pack_json: string
  state:
    | 'draft'
    | 'ready_for_operator_review'
    | 'approved_for_generation'
    | 'rejected'
    | 'blocked'
    | 'archived'
  static_capture_run_id: string | null
}>

class RemakeBriefStore {
  rows: Array<StoredRemakeBrief> = []
}

class RemakeBriefStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: RemakeBriefStore,
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
    if (this.query.includes('INSERT OR IGNORE INTO targeted_site_remake_briefs')) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          audit_finding_refs_json: String(this.values[12]),
          campaign_id: String(this.values[2]),
          copied_image_refs_json: String(this.values[15]),
          copied_text_refs_json: String(this.values[14]),
          created_at: String(this.values[20]),
          generation_constraints_json: String(this.values[16]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[10]),
          metadata_json: String(this.values[17]),
          normalized_domain: String(this.values[4]),
          original_screenshot_refs_json: String(this.values[13]),
          prepared_at: String(this.values[18]),
          prospect_id: this.values[3] as string | null,
          provider_adapter_run_id: this.values[8] as string | null,
          quality_audit_id: String(this.values[5]),
          rendered_capture_run_id: this.values[7] as string | null,
          reviewed_at: this.values[19] as string | null,
          source_authority_pack_json: String(this.values[11]),
          state: this.values[9] as StoredRemakeBrief['state'],
          static_capture_run_id: this.values[6] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_remake_briefs')) {
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

const remakeBriefDb = (store: RemakeBriefStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new RemakeBriefStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const qualityAudit = {
  archivedAt: null,
  auditedAt: '2026-06-05T18:00:00.000Z',
  campaignId: 'targeted_site_campaign_texas_energy',
  createdAt: '2026-06-05T18:00:00.000Z',
  dimensions: {
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
  },
  evidenceRefs: ['static_capture_ref_1', 'rendered_capture_ref_1'],
  id: 'targeted_site_quality_audit_otec_1',
  idempotencyKey: 'targeted-site-quality-audit:otec:1',
  legalSensitive: false,
  metadata: {},
  normalizedDomain: 'otec.example',
  overallScore: 50,
  prospectId: 'targeted_site_prospect_otec',
  providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
  recommendation: 'remake_candidate',
  renderedCaptureRunId: 'targeted_site_rendered_capture_otec_1',
  state: 'ready',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
} satisfies TargetedSiteQualityAuditRecord

const sourceAuthorityPack = {
  cards: [
    {
      allowedUse: 'use_as_visual_reference_only',
      caveats: ['do_not_copy_brand_assets_without_approval'],
      kind: 'original_screenshot',
      publicRef: 'rendered_screenshot_ref_1',
      sourceHash: 'sha256=originalscreenshothash',
    },
    {
      allowedUse: 'summarize_public_business_fact',
      caveats: ['verify_before_outreach'],
      kind: 'public_business_fact',
      publicRef: 'source_card_otec_public_fact',
      sourceHash: 'sha256=businessfacthash',
    },
  ],
  prohibitedClaims: ['no_endorsement_claims', 'no_customer_operated_claims'],
  requiredDisclosures: ['concept_preview_disclosure'],
  sourcePackRef: 'source_pack_otec_1',
} satisfies RecordTargetedSiteRemakeBriefInput['sourceAuthorityPack']

const baseInput = {
  auditFindingRefs: ['audit_finding_outdated_design', 'audit_finding_weak_cta'],
  copiedImageRefs: ['copied_image_ref_original_diagram'],
  copiedTextRefs: ['copied_text_ref_public_thesis'],
  id: 'targeted_site_remake_brief_otec_1',
  idempotencyKey: 'targeted-site-remake-brief:otec:1',
  metadata: { preparer: 'operator_seed' },
  originalScreenshotRefs: ['rendered_screenshot_ref_1'],
  qualityAudit,
  sourceAuthorityPack,
} satisfies RecordTargetedSiteRemakeBriefInput

describe('targeted Site remake briefs', () => {
  test('records idempotent remake briefs and lists them by campaign, prospect, and domain', async () => {
    const store = new RemakeBriefStore()
    const db = remakeBriefDb(store)
    const first = await recordTargetedSiteRemakeBrief(db, baseInput)
    const replay = await recordTargetedSiteRemakeBrief(db, {
      ...baseInput,
      auditFindingRefs: ['audit_finding_replay'],
      id: 'targeted_site_remake_brief_replay',
    })

    expect(first).toMatchObject({
      auditFindingRefs: [
        'audit_finding_outdated_design',
        'audit_finding_weak_cta',
      ],
      campaignId: 'targeted_site_campaign_texas_energy',
      legalSensitive: false,
      normalizedDomain: 'otec.example',
      qualityAuditId: 'targeted_site_quality_audit_otec_1',
      state: 'draft',
    })
    expect(first.generationConstraints).toMatchObject({
      conceptOnly: true,
      noFakeCaseResults: true,
      noFakeCredentials: true,
      noFakeReviews: true,
      noLegalAdvice: true,
      noMisleadingEndorsements: true,
      noUnverifiableGuarantees: true,
    })
    expect(replay).toEqual({
      ...first,
      qualityAuditRecommendation: null,
    })
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteRemakeBriefsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRemakeBriefsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRemakeBriefsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('redacts public projection and keeps operator projection source-authority only', async () => {
    const brief = await recordTargetedSiteRemakeBrief(
      remakeBriefDb(new RemakeBriefStore()),
      baseInput,
    )
    const publicProjection = publicTargetedSiteRemakeBriefProjection(brief)
    const operatorProjection = operatorTargetedSiteRemakeBriefProjection(brief)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      normalizedDomain: 'otec.example',
      preparedAt: brief.preparedAt,
      prospectId: 'targeted_site_prospect_otec',
      sourceAuthorityCardCount: 2,
      state: 'draft',
    })
    expect(publicProjection).not.toHaveProperty('sourceAuthorityPack')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      copiedImageRefs: ['copied_image_ref_original_diagram'],
      copiedTextRefs: ['copied_text_ref_public_thesis'],
      hasMetadata: true,
      originalScreenshotRefs: ['rendered_screenshot_ref_1'],
      providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
      sourceAuthorityPack,
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('requires audit findings, screenshots, source cards, and enabled safety constraints', async () => {
    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        auditFindingRefs: [],
        idempotencyKey: 'targeted-site-remake-brief:no-audits',
      }),
    ).rejects.toMatchObject({
      reason: 'remake briefs require at least one audit finding ref.',
    })

    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:no-screenshot',
        originalScreenshotRefs: [],
      }),
    ).rejects.toMatchObject({
      reason: 'remake briefs require at least one original screenshot ref.',
    })

    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:no-source-cards',
        sourceAuthorityPack: { ...sourceAuthorityPack, cards: [] },
      }),
    ).rejects.toMatchObject({
      reason: 'source authority pack requires at least one source card.',
    })

    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        generationConstraints: { noFakeReviews: false },
        idempotencyKey: 'targeted-site-remake-brief:unsafe-constraints',
      }),
    ).rejects.toMatchObject({
      reason:
        'generation constraints must keep concept-only and law-firm safety controls enabled.',
    })
  })

  test('rejects private capture/provider/contact/payment material and blocked audits', async () => {
    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:unsafe-source',
        sourceAuthorityPack: {
          ...sourceAuthorityPack,
          cards: [
            {
              allowedUse: 'use_as_visual_reference_only',
              caveats: ['do_not_copy_brand_assets_without_approval'],
              kind: 'original_screenshot',
              publicRef: 'provider_payload_raw_1',
              sourceHash: 'sha256=originalscreenshothash',
            },
          ],
        },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteRemakeBriefValidationError)

    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:unsafe-metadata',
        metadata: { contact: 'ben@example.com' },
      }),
    ).rejects.toBeInstanceOf(TargetedSiteRemakeBriefValidationError)

    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:blocked-audit',
        qualityAudit: {
          ...qualityAudit,
          recommendation: 'blocked',
          state: 'blocked',
        },
      }),
    ).rejects.toMatchObject({
      reason: 'blocked quality audits cannot produce remake briefs.',
    })
  })

  test('blocks legal-sensitive source packs that authorize claims we cannot make', async () => {
    await expect(
      recordTargetedSiteRemakeBrief(remakeBriefDb(new RemakeBriefStore()), {
        ...baseInput,
        idempotencyKey: 'targeted-site-remake-brief:legal-sensitive',
        legalSensitive: true,
        sourceAuthorityPack: {
          ...sourceAuthorityPack,
          cards: [
            {
              allowedUse: 'generate five star client reviews',
              caveats: ['do_not_copy_brand_assets_without_approval'],
              kind: 'original_screenshot',
              publicRef: 'rendered_screenshot_ref_1',
              sourceHash: 'sha256=originalscreenshothash',
            },
          ],
        },
      }),
    ).rejects.toMatchObject({
      reason:
        'legal-sensitive remake briefs may not authorize fake reviews, credentials, case results, legal advice, guarantees, or misleading endorsement claims.',
    })
  })
})
