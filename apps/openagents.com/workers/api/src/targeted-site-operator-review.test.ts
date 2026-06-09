import { describe, expect, test } from 'vitest'

import {
  TargetedSiteOperatorReviewValidationError,
  buildTargetedSiteOperatorReviewViewModel,
  listTargetedSiteOperatorReviewsByCampaign,
  listTargetedSiteOperatorReviewsByDomain,
  listTargetedSiteOperatorReviewsByPreview,
  operatorTargetedSiteOperatorReviewProjection,
  publicTargetedSiteOperatorReviewProjection,
  recordTargetedSiteOperatorReviewEvent,
  type RecordTargetedSiteOperatorReviewEventInput,
} from './targeted-site-operator-review'
import type { TargetedSiteRemakeBriefRecord } from './targeted-site-remake-brief'
import type { TargetedSiteRemakePreviewGenerationRecord } from './targeted-site-remake-preview-generation'

type StoredOperatorReview = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  decided_at: string
  decision:
    | 'approve_preview'
    | 'reject_preview'
    | 'request_regeneration'
    | 'skip_target'
    | 'approve_outreach'
    | 'block_target'
    | 'archive'
  evidence_refs_json: string
  id: string
  idempotency_key: string
  meeting_cta_ref: string | null
  metadata_json: string
  next_state:
    | 'preview_approved'
    | 'preview_rejected'
    | 'regeneration_requested'
    | 'target_skipped'
    | 'outreach_approved'
    | 'target_blocked'
    | 'archived'
  normalized_domain: string
  operator_actor_user_id: string
  operator_note_ref: string | null
  outreach_draft_ref: string | null
  previous_state: string
  preview_generation_id: string
  prospect_id: string | null
  remake_brief_id: string
  suppression_state: 'unknown' | 'clear' | 'suppressed' | 'manual_review'
}>

class OperatorReviewStore {
  rows: Array<StoredOperatorReview> = []
}

class OperatorReviewStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OperatorReviewStore,
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
        'INSERT OR IGNORE INTO targeted_site_operator_review_events',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          created_at: String(this.values[18]),
          decided_at: String(this.values[17]),
          decision: this.values[7] as StoredOperatorReview['decision'],
          evidence_refs_json: String(this.values[15]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          meeting_cta_ref: this.values[13] as string | null,
          metadata_json: String(this.values[16]),
          next_state: this.values[9] as StoredOperatorReview['next_state'],
          normalized_domain: String(this.values[4]),
          operator_actor_user_id: String(this.values[10]),
          operator_note_ref: this.values[11] as string | null,
          outreach_draft_ref: this.values[12] as string | null,
          previous_state: String(this.values[8]),
          preview_generation_id: String(this.values[6]),
          prospect_id: this.values[3] as string | null,
          remake_brief_id: String(this.values[5]),
          suppression_state:
            this.values[14] as StoredOperatorReview['suppression_state'],
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM targeted_site_operator_review_events')) {
      const value = String(this.values[0])
      const key = this.query.includes('campaign_id = ?')
        ? 'campaign_id'
        : this.query.includes('preview_generation_id = ?')
          ? 'preview_generation_id'
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

const operatorReviewDb = (store: OperatorReviewStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OperatorReviewStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const generationConstraints = {
  conceptOnly: true,
  noFakeCaseResults: true,
  noFakeCredentials: true,
  noFakeReviews: true,
  noLegalAdvice: true,
  noMisleadingEndorsements: true,
  noUnverifiableGuarantees: true,
  notes: ['concept_preview_only'],
}

const remakeBrief = {
  archivedAt: null,
  auditFindingRefs: ['audit_finding_outdated_design'],
  campaignId: 'targeted_site_campaign_texas_energy',
  copiedImageRefs: [],
  copiedTextRefs: [],
  createdAt: '2026-06-05T18:00:00.000Z',
  generationConstraints,
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

const generatedPreview = {
  archivedAt: null,
  campaignId: 'targeted_site_campaign_texas_energy',
  candidateSiteProjectRef: 'site_project_concept_otec',
  candidateSiteVersionRef: 'site_version_concept_otec_v1',
  completedAt: '2026-06-05T18:15:00.000Z',
  conceptSlug: 'otec-example',
  createdAt: '2026-06-05T18:11:00.000Z',
  failureRef: null,
  generatedArtifactRef: 'artifact_concept_otec_v1',
  generatedSourceRef: 'source_archive_concept_otec_v1',
  generationConstraints,
  generationReceiptRef: 'generation_receipt_otec_v1',
  id: 'targeted_site_remake_preview_otec_1',
  idempotencyKey: 'targeted-site-remake-preview:otec:1',
  legalSensitive: false,
  metadata: {},
  normalizedDomain: 'otec.example',
  previewUrl:
    'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
  prospectId: 'targeted_site_prospect_otec',
  providerAdapterRunId: 'targeted_site_provider_adapter_otec_1',
  qualityAuditId: 'targeted_site_quality_audit_otec_1',
  remakeBriefId: 'targeted_site_remake_brief_otec_1',
  renderedCaptureRunId: 'targeted_site_rendered_capture_otec_1',
  requestedAt: '2026-06-05T18:12:00.000Z',
  sourceAuthorityPackRef: 'source_pack_otec_1',
  state: 'generated',
  staticCaptureRunId: 'targeted_site_static_capture_otec_1',
} satisfies TargetedSiteRemakePreviewGenerationRecord

const baseInput = {
  decision: 'approve_outreach',
  evidenceRefs: ['preview_review_receipt_otec_v1'],
  id: 'targeted_site_operator_review_otec_1',
  idempotencyKey: 'targeted-site-operator-review:otec:approve-outreach:1',
  meetingCtaRef: 'meeting_cta_calendly_otec',
  metadata: { reviewer: 'operator_seed' },
  operatorActorUserId: 'operator_chris',
  operatorNoteRef: 'operator_note_otec_ready',
  outreachDraftRef: 'outreach_draft_otec_v1',
  previousState: 'preview_approved',
  preview: generatedPreview,
  suppressionState: 'clear',
} satisfies RecordTargetedSiteOperatorReviewEventInput

describe('targeted Site operator review', () => {
  test('builds a UI-ready view model with enabled and disabled actions', () => {
    const viewModel = buildTargetedSiteOperatorReviewViewModel({
      auditOverallScore: 50.4,
      capturePolicyState: 'allowed',
      meetingCtaRef: 'meeting_cta_calendly_otec',
      outreachDraftRef: 'outreach_draft_otec_v1',
      preview: generatedPreview,
      remakeBrief,
      suppressionState: 'clear',
    })

    expect(viewModel).toMatchObject({
      auditScoreLabel: '50 / 100',
      domain: 'otec.example',
      meetingCtaReady: true,
      preparedAtLabel: '2026-06-05 18:12 UTC',
      previewState: 'generated',
      previewUrl:
        'https://sites.openagents.com/concepts/targeted_site_campaign_texas_energy/otec-example',
      remakeBriefState: 'approved_for_generation',
      sourceAuthorityCardCount: 1,
      suppressionState: 'clear',
    })
    expect(viewModel.preparedAtLabel).not.toEqual('2026-06-05T18:12:00.000Z')
    expect(viewModel.preparedAtLabel).not.toContain('.000Z')
    expect(
      viewModel.actionAvailability.find(
        action => action.decision === 'approve_outreach',
      ),
    ).toMatchObject({ enabled: true, reason: null })

    const blocked = buildTargetedSiteOperatorReviewViewModel({
      auditOverallScore: 50,
      capturePolicyState: 'allowed',
      preview: generatedPreview,
      remakeBrief,
      suppressionState: 'suppressed',
    })

    expect(
      blocked.actionAvailability.find(
        action => action.decision === 'approve_outreach',
      ),
    ).toMatchObject({
      enabled: false,
      reason: 'Outreach draft ref is required.',
    })
  })

  test('records idempotent operator decisions and lists by campaign, preview, and domain', async () => {
    const store = new OperatorReviewStore()
    const db = operatorReviewDb(store)
    const first = await recordTargetedSiteOperatorReviewEvent(db, baseInput)
    const replay = await recordTargetedSiteOperatorReviewEvent(db, {
      ...baseInput,
      decision: 'archive',
      id: 'targeted_site_operator_review_replay',
    })

    expect(first).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      decision: 'approve_outreach',
      evidenceRefs: ['preview_review_receipt_otec_v1'],
      meetingCtaRef: 'meeting_cta_calendly_otec',
      nextState: 'outreach_approved',
      normalizedDomain: 'otec.example',
      outreachDraftRef: 'outreach_draft_otec_v1',
      previewGenerationId: 'targeted_site_remake_preview_otec_1',
      suppressionState: 'clear',
    })
    expect(replay).toEqual(first)
    expect(store.rows).toHaveLength(1)
    await expect(
      listTargetedSiteOperatorReviewsByCampaign(
        db,
        'targeted_site_campaign_texas_energy',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteOperatorReviewsByPreview(
        db,
        'targeted_site_remake_preview_otec_1',
      ),
    ).resolves.toHaveLength(1)
    await expect(
      listTargetedSiteOperatorReviewsByDomain(db, 'otec.example'),
    ).resolves.toHaveLength(1)
  })

  test('redacts public projection and keeps operator projection to refs', async () => {
    const event = await recordTargetedSiteOperatorReviewEvent(
      operatorReviewDb(new OperatorReviewStore()),
      baseInput,
    )
    const publicProjection = publicTargetedSiteOperatorReviewProjection(event)
    const operatorProjection = operatorTargetedSiteOperatorReviewProjection(event)

    expect(publicProjection).toEqual({
      campaignId: 'targeted_site_campaign_texas_energy',
      normalizedDomain: 'otec.example',
      previewGenerationId: 'targeted_site_remake_preview_otec_1',
      state: 'outreach_approved',
    })
    expect(publicProjection).not.toHaveProperty('operatorActorUserId')
    expect(publicProjection).not.toHaveProperty('metadata')
    expect(operatorProjection).toMatchObject({
      decision: 'approve_outreach',
      evidenceRefs: ['preview_review_receipt_otec_v1'],
      hasMetadata: true,
      meetingCtaRef: 'meeting_cta_calendly_otec',
      operatorActorUserId: 'operator_chris',
      outreachDraftRef: 'outreach_draft_otec_v1',
    })
    expect(operatorProjection).not.toHaveProperty('metadata')
  })

  test('blocks disabled outreach approval, missing evidence, and private material', async () => {
    await expect(
      recordTargetedSiteOperatorReviewEvent(
        operatorReviewDb(new OperatorReviewStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-operator-review:no-draft',
          outreachDraftRef: undefined,
        },
      ),
    ).rejects.toMatchObject({
      reason: 'Outreach draft ref is required.',
    })

    await expect(
      recordTargetedSiteOperatorReviewEvent(
        operatorReviewDb(new OperatorReviewStore()),
        {
          ...baseInput,
          evidenceRefs: [],
          idempotencyKey: 'targeted-site-operator-review:no-evidence',
        },
      ),
    ).rejects.toMatchObject({
      reason: 'operator review decisions require at least one evidence ref.',
    })

    await expect(
      recordTargetedSiteOperatorReviewEvent(
        operatorReviewDb(new OperatorReviewStore()),
        {
          ...baseInput,
          idempotencyKey: 'targeted-site-operator-review:unsafe-metadata',
          metadata: { contact: 'ben@example.com' },
        },
      ),
    ).rejects.toBeInstanceOf(TargetedSiteOperatorReviewValidationError)

    await expect(
      recordTargetedSiteOperatorReviewEvent(
        operatorReviewDb(new OperatorReviewStore()),
        {
          ...baseInput,
          evidenceRefs: ['provider_payload_raw_1'],
          idempotencyKey: 'targeted-site-operator-review:unsafe-ref',
        },
      ),
    ).rejects.toBeInstanceOf(TargetedSiteOperatorReviewValidationError)
  })
})
