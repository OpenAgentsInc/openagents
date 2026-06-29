import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import { EmailAddress, ResendEmailSender, WorkerSecret } from './config'
import {
  type EmailServiceShape,
  makeEmailService,
} from './email'
import {
  TargetedSiteRemakeOutreachEmailValidationError,
  dispatchTargetedSiteRemakeOutreachEmail,
  operatorTargetedSiteRemakeOutreachEmailDispatchProjection,
  publicTargetedSiteRemakeOutreachEmailDispatchProjection,
  type DispatchTargetedSiteRemakeOutreachEmailInput,
} from './targeted-site-remake-outreach-email'
import type { TargetedSiteOperatorReviewEventRecord } from './targeted-site-operator-review'
import type { TargetedSiteRemakePreviewGenerationRecord } from './targeted-site-remake-preview-generation'

type StoredOutreachDispatch = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  dispatch_state: 'accepted' | 'failed' | 'blocked' | 'skipped'
  dispatched_at: string
  email_message_id: string | null
  error_message: string | null
  error_name: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  normalized_domain: string
  operator_review_event_id: string
  preview_generation_id: string
  prospect_id: string | null
  recipient_ref: string
  suppression_state: 'unknown' | 'clear' | 'suppressed' | 'manual_review'
  template_slug: string
}>

class OutreachDispatchStore {
  rows: Array<StoredOutreachDispatch> = []
}

class OutreachDispatchStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OutreachDispatchStore,
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
        'INSERT OR IGNORE INTO targeted_site_remake_outreach_email_dispatches',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (this.store.rows.every(row => row.idempotency_key !== idempotencyKey)) {
        this.store.rows.push({
          archived_at: null,
          campaign_id: String(this.values[2]),
          created_at: String(this.values[16]),
          dispatch_state: this.values[11] as StoredOutreachDispatch['dispatch_state'],
          dispatched_at: String(this.values[15]),
          email_message_id: this.values[7] as string | null,
          error_message: this.values[13] as string | null,
          error_name: this.values[12] as string | null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[14]),
          normalized_domain: String(this.values[4]),
          operator_review_event_id: String(this.values[6]),
          preview_generation_id: String(this.values[5]),
          prospect_id: this.values[3] as string | null,
          recipient_ref: String(this.values[8]),
          suppression_state:
            this.values[10] as StoredOutreachDispatch['suppression_state'],
          template_slug: String(this.values[9]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({
      results: [] as ReadonlyArray<T>,
      success: true,
    } as D1Result<T>)
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

const outreachDispatchDb = (store: OutreachDispatchStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OutreachDispatchStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const resendConfig = () => ({
  apiKey: Redacted.make(WorkerSecret.make('re_test')),
  fromEmail: ResendEmailSender.make('OpenAgents <chris+sites@openagents.com>'),
  replyToEmail: EmailAddress.make('chris+sites@openagents.com'),
})

const preview = {
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

const operatorReview = {
  archivedAt: null,
  campaignId: 'targeted_site_campaign_texas_energy',
  createdAt: '2026-06-05T18:30:00.000Z',
  decidedAt: '2026-06-05T18:30:00.000Z',
  decision: 'approve_outreach',
  evidenceRefs: ['preview_review_receipt_otec_v1'],
  id: 'targeted_site_operator_review_otec_1',
  idempotencyKey: 'targeted-site-operator-review:otec:approve-outreach:1',
  meetingCtaRef: 'meeting_cta_calendly_otec',
  metadata: {},
  nextState: 'outreach_approved',
  normalizedDomain: 'otec.example',
  operatorActorUserId: 'operator_chris',
  operatorNoteRef: 'operator_note_otec_ready',
  outreachDraftRef: 'outreach_draft_otec_v1',
  previousState: 'preview_approved',
  previewGenerationId: 'targeted_site_remake_preview_otec_1',
  prospectId: 'targeted_site_prospect_otec',
  remakeBriefId: 'targeted_site_remake_brief_otec_1',
  suppressionState: 'clear',
} satisfies TargetedSiteOperatorReviewEventRecord

const baseInput = {
  appOrigin: 'https://openagents.com',
  conceptDisclosure:
    'This is an OpenAgents concept preview, not a site operated or endorsed by your organization.',
  displayName: 'Alex Customer',
  id: 'targeted_site_outreach_email_otec_1',
  idempotencyKey:
    'targeted_remake_outreach:targeted_site_remake_preview_otec_1:operator_review_otec_1',
  meetingUrl: 'https://openagents.com/meet/otec-review',
  metadata: { templateVersion: 'targeted_remake.outreach.v1' },
  operatorReview,
  postalAddress: 'OpenAgents, 548 Market St, San Francisco, CA 94104',
  preferencesUrl: 'https://openagents.com/email/preferences',
  preview,
  recipientEmail: 'alex.customer@example.com',
  recipientRef: 'targeted_site_recipient_otec_primary',
  senderContact: 'chris+sites@openagents.com',
  senderName: 'Chris at OpenAgents',
  targetName: 'OTEC Floating Datacenter',
  unsubscribeUrl: 'https://openagents.com/email/unsubscribe/targeted',
  valueProposition:
    'The preview focuses on clearer positioning, stronger calls to action, and a cleaner technical story.',
} satisfies DispatchTargetedSiteRemakeOutreachEmailInput

const fakeEmailService = (
  calls: Array<unknown>,
  ok = true,
): EmailServiceShape => ({
  ...makeEmailService(),
  sendTargetedRemakeOutreachEmailWithLedger: (
    _db,
    _config,
    input,
    context,
  ) => {
    calls.push({ context, input })

    return Effect.succeed(
      ok
        ? {
            emailMessageId: 'email_msg_targeted_1',
            ok: true as const,
            providerMessageId: 'email_resend_targeted_1',
          }
        : {
            emailMessageId: 'email_msg_targeted_1',
            errorMessage: 'provider rejected',
            errorName: 'provider_rejected',
            ok: false as const,
          },
    )
  },
})

describe('targeted remake outreach email dispatch', () => {
  test('dispatches through EmailService and records an accepted ledger row', async () => {
    const store = new OutreachDispatchStore()
    const calls: Array<unknown> = []
    const record = await Effect.runPromise(
      dispatchTargetedSiteRemakeOutreachEmail(
        outreachDispatchDb(store),
        resendConfig(),
        fakeEmailService(calls),
        baseInput,
        fetch,
        {
          nowIso: () => '2026-06-05T19:30:00.000Z',
          randomId: prefix => `${prefix}_fixed`,
        },
      ),
    )

    expect(calls).toHaveLength(1)
    expect(record).toMatchObject({
      campaignId: 'targeted_site_campaign_texas_energy',
      dispatchState: 'accepted',
      emailMessageId: 'email_msg_targeted_1',
      normalizedDomain: 'otec.example',
      operatorReviewEventId: 'targeted_site_operator_review_otec_1',
      previewGenerationId: 'targeted_site_remake_preview_otec_1',
      recipientRef: 'targeted_site_recipient_otec_primary',
      suppressionState: 'clear',
      templateSlug: 'targeted_remake.outreach.v1',
    })
    expect(store.rows).toHaveLength(1)
  })

  test('is idempotent and does not call EmailService on replay', async () => {
    const store = new OutreachDispatchStore()
    const calls: Array<unknown> = []
    const db = outreachDispatchDb(store)

    const first = await Effect.runPromise(
      dispatchTargetedSiteRemakeOutreachEmail(
        db,
        resendConfig(),
        fakeEmailService(calls),
        baseInput,
      ),
    )
    const replay = await Effect.runPromise(
      dispatchTargetedSiteRemakeOutreachEmail(
        db,
        resendConfig(),
        fakeEmailService(calls),
        { ...baseInput, id: 'targeted_site_outreach_email_replay' },
      ),
    )

    expect(replay).toEqual(first)
    expect(calls).toHaveLength(1)
    expect(store.rows).toHaveLength(1)
  })

  test('records failed EmailService results without raw provider payloads', async () => {
    const record = await Effect.runPromise(
      dispatchTargetedSiteRemakeOutreachEmail(
        outreachDispatchDb(new OutreachDispatchStore()),
        resendConfig(),
        fakeEmailService([], false),
        {
          ...baseInput,
          idempotencyKey: 'targeted_remake_outreach:failed',
        },
      ),
    )

    expect(record).toMatchObject({
      dispatchState: 'failed',
      errorMessage: 'provider rejected',
      errorName: 'provider_rejected',
    })
  })

  test('redacts public projection and keeps operator projection to refs', async () => {
    const record = await Effect.runPromise(
      dispatchTargetedSiteRemakeOutreachEmail(
        outreachDispatchDb(new OutreachDispatchStore()),
        resendConfig(),
        fakeEmailService([]),
        baseInput,
      ),
    )

    expect(publicTargetedSiteRemakeOutreachEmailDispatchProjection(record))
      .toEqual({
        campaignId: 'targeted_site_campaign_texas_energy',
        dispatchState: 'accepted',
        normalizedDomain: 'otec.example',
        previewGenerationId: 'targeted_site_remake_preview_otec_1',
      })
    expect(operatorTargetedSiteRemakeOutreachEmailDispatchProjection(record))
      .toMatchObject({
        emailMessageId: 'email_msg_targeted_1',
        hasMetadata: true,
        operatorReviewEventId: 'targeted_site_operator_review_otec_1',
        recipientRef: 'targeted_site_recipient_otec_primary',
      })
  })

  test('requires approval, clear suppression, and safe content', async () => {
    await expect(
      Effect.runPromise(
        dispatchTargetedSiteRemakeOutreachEmail(
          outreachDispatchDb(new OutreachDispatchStore()),
          resendConfig(),
          fakeEmailService([]),
          {
            ...baseInput,
            idempotencyKey: 'targeted_remake_outreach:not-approved',
            operatorReview: {
              ...operatorReview,
              nextState: 'preview_approved',
            },
          },
        ),
      ),
    ).rejects.toMatchObject({
      reason: 'targeted remake outreach requires an approved outreach review event.',
    })

    await expect(
      Effect.runPromise(
        dispatchTargetedSiteRemakeOutreachEmail(
          outreachDispatchDb(new OutreachDispatchStore()),
          resendConfig(),
          fakeEmailService([]),
          {
            ...baseInput,
            idempotencyKey: 'targeted_remake_outreach:suppressed',
            operatorReview: {
              ...operatorReview,
              suppressionState: 'suppressed',
            },
          },
        ),
      ),
    ).rejects.toMatchObject({
      reason: 'targeted remake outreach requires clear suppression state.',
    })

    await expect(
      Effect.runPromise(
        dispatchTargetedSiteRemakeOutreachEmail(
          outreachDispatchDb(new OutreachDispatchStore()),
          resendConfig(),
          fakeEmailService([]),
          {
            ...baseInput,
            idempotencyKey: 'targeted_remake_outreach:unsafe',
            valueProposition: 'provider_payload_raw_1 with captcha bypass',
          },
        ),
      ),
    ).rejects.toBeInstanceOf(TargetedSiteRemakeOutreachEmailValidationError)
  })
})
