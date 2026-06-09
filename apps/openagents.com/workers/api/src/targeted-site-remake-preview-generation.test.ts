import { describe, expect, test } from 'vitest'

import {
  TargetedSiteRemakePreviewGenerationValidationError,
  listTargetedSiteRemakePreviewGenerationsByCampaign,
  listTargetedSiteRemakePreviewGenerationsByDomain,
  listTargetedSiteRemakePreviewGenerationsByProspect,
  operatorTargetedSiteRemakePreviewGenerationProjection,
  publicTargetedSiteRemakePreviewGenerationProjection,
  recordTargetedSiteRemakePreviewGeneration,
  type RecordTargetedSiteRemakePreviewGenerationInput,
} from './targeted-site-remake-preview-generation'
import type { TargetedSiteRemakeBriefRecord } from './targeted-site-remake-brief'

type StoredPreviewGeneration = Readonly<{
  archived_at: string | null
  campaign_id: string
  candidate_site_project_ref: string | null
  candidate_site_version_ref: string | null
  completed_at: string | null
  concept_slug: string
  created_at: string
  failure_ref: string | null
  generated_artifact_ref: string | null
  generated_source_ref: string | null
  generation_constraints_json: string
  generation_receipt_ref: string | null
  id: string
  idempotency_key: string
  legal_sensitive: number
  metadata_json: string
  normalized_domain: string
  preview_url: string | null
  prospect_id: string | null
  provider_adapter_run_id: string | null
  quality_audit_id: string
  remake_brief_id: string
  rendered_capture_run_id: string | null
  requested_at: string
  source_authority_pack_ref: string
  state: 'requested' | 'generating' | 'generated' | 'failed' | 'blocked' | 'archived'
  static_capture_run_id: string | null
}>

class PreviewGenerationStore {
  rows: Array<StoredPreviewGeneration> = []
}

class PreviewGenerationStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: PreviewGenerationStore,
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
    if (
      this.query.includes(
        'INSERT OR IGNORE INTO targeted_site_remake_preview_generations',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          candidate_site_project_ref: this.values[16] as string | null,
          candidate_site_version_ref: this.values[17] as string | null,
          completed_at: this.values[24] as string | null,
          concept_slug: String(this.values[12]),
          created_at: String(this.values[25]),
          failure_ref: this.values[19] as string | null,
          generated_artifact_ref: this.values[14] as string | null,
          generated_source_ref: this.values[15] as string | null,
          generation_constraints_json: String(this.values[21]),
          generation_receipt_ref: this.values[18] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          legal_sensitive: Number(this.values[20]),
          metadata_json: String(this.values[22]),
          normalized_domain: String(this.values[4]),
          preview_url: this.values[11] as string | null,
          prospect_id: this.values[3] as string | null,
          provider_adapter_run_id: this.values[9] as string | null,
          quality_audit_id: String(this.values[6]),
          remake_brief_id: String(this.values[5]),
          rendered_capture_run_id: this.values[8] as string | null,
          requested_at: String(this.values[23]),
          source_authority_pack_ref: String(this.values[13]),
          state: this.values[10] as StoredPreviewGeneration['state'],
          static_capture_run_id: this.values[7] as string | null,
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_remake_preview_generations')) {
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

const previewGenerationDb = (store: PreviewGenerationStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new PreviewGenerationStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const approvedBrief = {
  archivedAt: null,
  auditFindingRefs: ['audit_finding_outdated_design'],
  campaignId: 'targeted_site_campaign_texas_energy',
  copiedImageRefs: ['copied_image_ref_original_diagram'],
  copiedTextRefs: ['copied_text_ref_public_thesis'],
  createdAt: '2026-06-05T18:00:00.000Z',
  generationConstraints: {
    conceptOnly: true,
    noFakeCaseResults: true,
    noFakeCredentials: true,
    noFakeReviews: true,
    noLegalAdvice: true,
    noMisleadingEndorsements: true,
    noUnverifiableGuarantees: true,
    notes: ['concept_preview_only'],
  },
  id: 'targeted_site_remake_brief_otec_1',
  idempotencyKey: 'targeted-site-remake-brief:otec:1',
  legalSensitive: false,
  metadata: {},
  normalizedDomain: 'otec.example',
  originalScreenshotRefs: ['rendered_screenshot_ref_1'],
  preparedAt: '2026-06-05T18:05:00.000Z',
  prospectId: 'targeted_site_prospect_otec',
  providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
  qualityAuditId: 'targeted_site_quality_audit_otec_1',
  qualityAuditRecommendation: 'remake_candidate',
  renderedCaptureRunId: 'targeted_site_rendered_capture_otec_1',
  reviewedAt: '2026-06-05T18:10:00.000Z',
  sourceAuthorityPack: {
    cards: [
      {
        allowedUse: 'use_as_visual_reference_only',
        caveats: ['do_not_copy_brand_assets_without_approval'],
        kind: 'original_screenshot',
        publicRef: 'rendered_screenshot_ref_1',
        sourceHash: 'sha256=originalscreenshothash',
      },
    ],
    prohibitedClaims: ['no_endorsement_claims'],
    requiredDisclosures: ['concept_preview_disclosure'],
    sourcePackRef: 'source_pack_otec_1',
  },
  state: 'approved_for_generation',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
} satisfies TargetedSiteRemakeBriefRecord

const baseInput = {
  candidateSiteProjectRef: 'site_project_concept_otec',
  candidateSiteVersionRef: 'site_version_concept_otec_v1',
  completedAt: '2026-06-05T18:15:00.000Z',
  conceptSlug: 'otec-example',
  generatedArtifactRef: 'artifact_concept_otec_v1',
  generatedSourceRef: 'source_archive_concept_otec_v1',
  generationReceiptRef: 'generation_receipt_otec_v1',
  id: 'targeted_site_remake_preview_otec_1',
  idempotencyKey: 'targeted-site-remake-preview:otec:1',
  metadata: { runner: 'adjutant_seed' },
  previewUrl:
    'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
  remakeBrief: approvedBrief,
  state: 'generated',
} satisfies RecordTargetedSiteRemakePreviewGenerationInput

describe('targeted Site remake preview generation', () => {
  test('records idempotent generated previews and lists them by campaign, prospect, and domain', async () => {
    const store = new PreviewGenerationStore()
    const db = previewGenerationDb(store)
    const first = await recordTargetedSiteRemakePreviewGeneration(db, baseInput)
    const replay = await recordTargetedSiteRemakePreviewGeneration(db, {
      ...baseInput,
      id: 'targeted_site_remake_preview_replay',
      state: 'failed',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      candidateSiteVersionRef: 'site_version_concept_otec_v1',
      conceptSlug: 'otec-example',
      generatedArtifactRef: 'artifact_concept_otec_v1',
      generatedSourceRef: 'source_archive_concept_otec_v1',
      normalizedDomain: 'otec.example',
      previewUrl:
        'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
      sourceAuthorityPackRef: 'source_pack_otec_1',
      state: 'generated',
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteRemakePreviewGenerationsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRemakePreviewGenerationsByProspect(
        db,
        'targeted_site_prospect_otec',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteRemakePreviewGenerationsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('redacts public projection and keeps operator projection to refs', async () => {
    const preview = await recordTargetedSiteRemakePreviewGeneration(
      previewGenerationDb(new PreviewGenerationStore()),
      baseInput,
    )
    const publicProjection =
      publicTargetedSiteRemakePreviewGenerationProjection(preview)
    const operatorProjection =
      operatorTargetedSiteRemakePreviewGenerationProjection(preview)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      conceptSlug: 'otec-example',
      normalizedDomain: 'otec.example',
      previewUrl:
        'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
      prospectId: 'targeted_site_prospect_otec',
      requestedAt: preview.requestedAt,
      state: 'generated',
    })
    expect(publicProjection).not.toHaveProperty('generationConstraints')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      candidateSiteVersionRef: 'site_version_concept_otec_v1',
      generatedArtifactRef: 'artifact_concept_otec_v1',
      generatedSourceRef: 'source_archive_concept_otec_v1',
      generationReceiptRef: 'generation_receipt_otec_v1',
      hasMetadata: true,
      remakeBriefId: 'targeted_site_remake_brief_otec_1',
      sourceAuthorityPackRef: 'source_pack_otec_1',
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('requires approved remake brief before generated output', async () => {
    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-remake-preview:not-approved',
          remakeBrief: {
            ...approvedBrief,
            state: 'ready_for_operator_review',
          },
        },
      ),
    ).rejects.toMatchObject({
      reason:
        'generated preview output requires an approved_for_generation remake brief.',
    })

    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          candidateSiteVersionRef: undefined,
          idempotencyKey: 'targeted-site-remake-preview:missing-output',
        },
      ),
    ).rejects.toMatchObject({
      reason:
        'generated preview output requires previewUrl, generatedArtifactRef, generatedSourceRef, and candidateSiteVersionRef.',
    })
  })

  test('validates concept preview URL and blocks target-domain impersonation', async () => {
    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-remake-preview:wrong-host',
          previewUrl: 'https://otec.example/concepts/targeted/otec-example',
        },
      ),
    ).rejects.toMatchObject({
      reason:
        'previewUrl must use the OpenAgents concept preview domain and path.',
    })

    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          conceptSlug: 'Otec Example',
          idempotencyKey: 'targeted-site-remake-preview:bad-slug',
        },
      ),
    ).rejects.toMatchObject({
      reason: 'conceptSlug must be a lowercase public-safe slug.',
    })
  })

  test('rejects private material and unsafe generation constraints', async () => {
    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-remake-preview:unsafe-metadata',
          metadata: { contact: 'ben@example.com' },
        },
      ),
    ).rejects.toBeInstanceOf(
      TargetedSiteRemakePreviewGenerationValidationError,
    )

    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-remake-preview:unsafe-ref',
          generatedArtifactRef: 'provider_payload_raw_1',
        },
      ),
    ).rejects.toBeInstanceOf(
      TargetedSiteRemakePreviewGenerationValidationError,
    )

    await expect(
      recordTargetedSiteRemakePreviewGeneration(
        previewGenerationDb(new PreviewGenerationStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-remake-preview:unsafe-constraints',
          remakeBrief: {
            ...approvedBrief,
            generationConstraints: {
              ...approvedBrief.generationConstraints,
              conceptOnly: false,
            },
          },
        },
      ),
    ).rejects.toMatchObject({
      reason:
        'preview generation requires concept-only and law-firm safety constraints.',
    })
  })
})
