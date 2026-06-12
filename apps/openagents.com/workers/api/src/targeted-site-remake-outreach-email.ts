import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import type { ResendEmailConfig } from './config'
import {
  type EmailServiceError,
  type EmailRuntime,
  type EmailServiceShape,
  TargetedRemakeOutreachEmailInput,
  systemEmailRuntime,
} from './email'
import { parseJsonRecord } from './json-boundary'
import type { TargetedSiteOperatorReviewEventRecord } from './targeted-site-operator-review'
import type { TargetedSiteRemakePreviewGenerationRecord } from './targeted-site-remake-preview-generation'

export const TargetedSiteRemakeOutreachEmailDispatchState = S.Literals([
  'accepted',
  'failed',
  'blocked',
  'skipped',
])
export type TargetedSiteRemakeOutreachEmailDispatchState =
  typeof TargetedSiteRemakeOutreachEmailDispatchState.Type

export const TargetedSiteRemakeOutreachEmailDispatchRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  campaignId: S.String,
  createdAt: S.String,
  dispatchState: TargetedSiteRemakeOutreachEmailDispatchState,
  dispatchedAt: S.String,
  emailMessageId: S.NullOr(S.String),
  errorMessage: S.NullOr(S.String),
  errorName: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  normalizedDomain: S.String,
  operatorReviewEventId: S.String,
  previewGenerationId: S.String,
  prospectId: S.NullOr(S.String),
  recipientRef: S.String,
  suppressionState: S.Literals(['unknown', 'clear', 'suppressed', 'manual_review']),
  templateSlug: S.String,
})
export type TargetedSiteRemakeOutreachEmailDispatchRecord =
  typeof TargetedSiteRemakeOutreachEmailDispatchRecord.Type

export const PublicTargetedSiteRemakeOutreachEmailDispatchProjection = S.Struct({
  campaignId: S.String,
  dispatchState: TargetedSiteRemakeOutreachEmailDispatchState,
  normalizedDomain: S.String,
  previewGenerationId: S.String,
})
export type PublicTargetedSiteRemakeOutreachEmailDispatchProjection =
  typeof PublicTargetedSiteRemakeOutreachEmailDispatchProjection.Type

export const OperatorTargetedSiteRemakeOutreachEmailDispatchProjection = S.Struct({
  campaignId: S.String,
  dispatchState: TargetedSiteRemakeOutreachEmailDispatchState,
  dispatchedAt: S.String,
  emailMessageId: S.NullOr(S.String),
  errorMessage: S.NullOr(S.String),
  errorName: S.NullOr(S.String),
  hasMetadata: S.Boolean,
  normalizedDomain: S.String,
  operatorReviewEventId: S.String,
  previewGenerationId: S.String,
  prospectId: S.NullOr(S.String),
  recipientRef: S.String,
  suppressionState: S.Literals(['unknown', 'clear', 'suppressed', 'manual_review']),
  templateSlug: S.String,
})
export type OperatorTargetedSiteRemakeOutreachEmailDispatchProjection =
  typeof OperatorTargetedSiteRemakeOutreachEmailDispatchProjection.Type

export type DispatchTargetedSiteRemakeOutreachEmailInput = Readonly<{
  appOrigin: string
  conceptDisclosure: string
  displayName: string
  id?: string | undefined
  idempotencyKey: string
  meetingUrl: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  operatorReview: TargetedSiteOperatorReviewEventRecord
  postalAddress: string
  preferencesUrl: string
  preview: TargetedSiteRemakePreviewGenerationRecord
  recipientEmail: string
  recipientRef: string
  senderContact: string
  senderName: string
  targetName: string
  unsubscribeUrl: string
  valueProposition: string
}>

type TargetedSiteRemakeOutreachEmailDispatchRow = Readonly<{
  archived_at: string | null
  campaign_id: string
  created_at: string
  dispatch_state: TargetedSiteRemakeOutreachEmailDispatchState
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

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_DOMAIN_PATTERN =
  /^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z][a-z0-9-]{1,62}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)/i

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: `${field} must be a public-safe ref without private capture, provider, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeDomain = (value: string): void => {
  if (!SAFE_DOMAIN_PATTERN.test(value) || !textIsSafe(value)) {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: 'normalizedDomain must be a public-safe normalized domain.',
    })
  }
}

const assertSafeText = (field: string, value: string): void => {
  if (value.trim() === '' || !textIsSafe(value)) {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: `${field} must be public-safe text.`,
    })
  }
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): void => {
  if (metadata === undefined) {
    return
  }

  const json = JSON.stringify(metadata)

  if (containsProviderSecretMaterial(json) || PROHIBITED_TEXT_PATTERN.test(json)) {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason:
        'metadata must not contain private capture, provider, payment, wallet, or bypass material.',
    })
  }
}

export class TargetedSiteRemakeOutreachEmailValidationError extends S.TaggedErrorClass<TargetedSiteRemakeOutreachEmailValidationError>()(
  'TargetedSiteRemakeOutreachEmailValidationError',
  {
    reason: S.String,
  },
) {}

export class TargetedSiteRemakeOutreachEmailStorageError extends S.TaggedErrorClass<TargetedSiteRemakeOutreachEmailStorageError>()(
  'TargetedSiteRemakeOutreachEmailStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const assertValidInput = (
  input: DispatchTargetedSiteRemakeOutreachEmailInput,
): void => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('campaignId', input.preview.campaignId)
  assertSafeRef('prospectId', input.preview.prospectId ?? undefined)
  assertSafeRef('previewGenerationId', input.preview.id)
  assertSafeRef('operatorReviewEventId', input.operatorReview.id)
  assertSafeRef('recipientRef', input.recipientRef)
  assertSafeDomain(input.preview.normalizedDomain)
  assertSafeText('conceptDisclosure', input.conceptDisclosure)
  assertSafeText('targetName', input.targetName)
  assertSafeText('valueProposition', input.valueProposition)
  assertSafeMetadata(input.metadata)

  if (
    input.operatorReview.previewGenerationId !== input.preview.id ||
    input.operatorReview.nextState !== 'outreach_approved' ||
    input.operatorReview.decision !== 'approve_outreach'
  ) {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: 'targeted remake outreach requires an approved outreach review event.',
    })
  }

  if (input.operatorReview.suppressionState !== 'clear') {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: 'targeted remake outreach requires clear suppression state.',
    })
  }

  if (input.preview.previewUrl === null || input.preview.state !== 'generated') {
    throw new TargetedSiteRemakeOutreachEmailValidationError({
      reason: 'targeted remake outreach requires a generated preview URL.',
    })
  }
}

const dispatchRecordFromRow = (
  row: TargetedSiteRemakeOutreachEmailDispatchRow,
): TargetedSiteRemakeOutreachEmailDispatchRecord => ({
  archivedAt: row.archived_at,
  campaignId: row.campaign_id,
  createdAt: row.created_at,
  dispatchState: row.dispatch_state,
  dispatchedAt: row.dispatched_at,
  emailMessageId: row.email_message_id,
  errorMessage: row.error_message,
  errorName: row.error_name,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  normalizedDomain: row.normalized_domain,
  operatorReviewEventId: row.operator_review_event_id,
  previewGenerationId: row.preview_generation_id,
  prospectId: row.prospect_id,
  recipientRef: row.recipient_ref,
  suppressionState: row.suppression_state,
  templateSlug: row.template_slug,
})

const readByIdempotencyKey = (
  db: D1Database,
  idempotencyKey: string,
): Effect.Effect<
  TargetedSiteRemakeOutreachEmailDispatchRecord | null,
  TargetedSiteRemakeOutreachEmailStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT *
             FROM targeted_site_remake_outreach_email_dispatches
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<TargetedSiteRemakeOutreachEmailDispatchRow>()

      return row === null ? null : dispatchRecordFromRow(row)
    },
    catch: error =>
      new TargetedSiteRemakeOutreachEmailStorageError({
        operation: 'readByIdempotencyKey',
        reason: error instanceof Error ? error.message : String(error),
      }),
  })

const recordDispatch = (
  db: D1Database,
  input: DispatchTargetedSiteRemakeOutreachEmailInput,
  result: Readonly<{
    emailMessageId: string
    errorMessage?: string | undefined
    errorName?: string | undefined
    ok: boolean
  }>,
  runtime: EmailRuntime,
): Effect.Effect<
  TargetedSiteRemakeOutreachEmailDispatchRecord,
  TargetedSiteRemakeOutreachEmailStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const now = runtime.nowIso()
      const id = input.id ?? runtime.randomId('targeted_site_outreach_email')
      const dispatchState = result.ok ? 'accepted' : 'failed'

      await db
        .prepare(
          `INSERT OR IGNORE INTO targeted_site_remake_outreach_email_dispatches (
             id,
             idempotency_key,
             campaign_id,
             prospect_id,
             normalized_domain,
             preview_generation_id,
             operator_review_event_id,
             email_message_id,
             recipient_ref,
             template_slug,
             suppression_state,
             dispatch_state,
             error_name,
             error_message,
             metadata_json,
             dispatched_at,
             created_at,
             archived_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          id,
          input.idempotencyKey,
          input.preview.campaignId,
          input.preview.prospectId,
          input.preview.normalizedDomain,
          input.preview.id,
          input.operatorReview.id,
          result.emailMessageId,
          input.recipientRef,
          'targeted_remake.outreach.v1',
          input.operatorReview.suppressionState,
          dispatchState,
          result.errorName ?? null,
          result.errorMessage ?? null,
          JSON.stringify(input.metadata ?? {}),
          now,
          now,
        )
        .run()

      const record = await db
        .prepare(
          `SELECT *
             FROM targeted_site_remake_outreach_email_dispatches
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(input.idempotencyKey)
        .first<TargetedSiteRemakeOutreachEmailDispatchRow>()

      return record === null ? null : dispatchRecordFromRow(record)
    },
    catch: error =>
      new TargetedSiteRemakeOutreachEmailStorageError({
        operation: 'recordDispatch',
        reason: error instanceof Error ? error.message : String(error),
      }),
  }).pipe(
    Effect.flatMap(record =>
      record === null
        ? Effect.fail(
            new TargetedSiteRemakeOutreachEmailStorageError({
              operation: 'recordDispatch.readByIdempotencyKey',
              reason: 'targeted remake outreach dispatch record was not readable.',
            }),
          )
        : Effect.succeed(record),
    ),
  )

export const publicTargetedSiteRemakeOutreachEmailDispatchProjection = (
  record: TargetedSiteRemakeOutreachEmailDispatchRecord,
): PublicTargetedSiteRemakeOutreachEmailDispatchProjection => ({
  campaignId: record.campaignId,
  dispatchState: record.dispatchState,
  normalizedDomain: record.normalizedDomain,
  previewGenerationId: record.previewGenerationId,
})

export const operatorTargetedSiteRemakeOutreachEmailDispatchProjection = (
  record: TargetedSiteRemakeOutreachEmailDispatchRecord,
): OperatorTargetedSiteRemakeOutreachEmailDispatchProjection => ({
  campaignId: record.campaignId,
  dispatchState: record.dispatchState,
  dispatchedAt: record.dispatchedAt,
  emailMessageId: record.emailMessageId,
  errorMessage: record.errorMessage,
  errorName: record.errorName,
  hasMetadata: Object.keys(record.metadata).length > 0,
  normalizedDomain: record.normalizedDomain,
  operatorReviewEventId: record.operatorReviewEventId,
  previewGenerationId: record.previewGenerationId,
  prospectId: record.prospectId,
  recipientRef: record.recipientRef,
  suppressionState: record.suppressionState,
  templateSlug: record.templateSlug,
})

export const dispatchTargetedSiteRemakeOutreachEmail = (
  db: D1Database,
  config: ResendEmailConfig,
  emailService: EmailServiceShape,
  input: DispatchTargetedSiteRemakeOutreachEmailInput,
  fetcher: typeof fetch = fetch,
  runtime: EmailRuntime = systemEmailRuntime,
): Effect.Effect<
  TargetedSiteRemakeOutreachEmailDispatchRecord,
  | TargetedSiteRemakeOutreachEmailStorageError
  | TargetedSiteRemakeOutreachEmailValidationError
  | EmailServiceError
> =>
  Effect.gen(function* () {
    yield* Effect.try({
      try: () => assertValidInput(input),
      catch: error =>
        error instanceof TargetedSiteRemakeOutreachEmailValidationError
          ? error
          : new TargetedSiteRemakeOutreachEmailValidationError({
              reason: error instanceof Error ? error.message : String(error),
            }),
    })
    const existing = yield* readByIdempotencyKey(db, input.idempotencyKey)

    if (existing !== null) {
      return existing
    }

    const result =
      yield* emailService.sendTargetedRemakeOutreachEmailWithLedger(
        db,
        config,
        new TargetedRemakeOutreachEmailInput({
          appOrigin: input.appOrigin,
          campaignId: input.preview.campaignId,
          conceptDisclosure: input.conceptDisclosure,
          displayName: input.displayName,
          idempotencyKey: input.idempotencyKey,
          meetingUrl: input.meetingUrl,
          postalAddress: input.postalAddress,
          preferencesUrl: input.preferencesUrl,
          previewGenerationId: input.preview.id,
          previewUrl: input.preview.previewUrl ?? '',
          prospectId: input.preview.prospectId,
          senderContact: input.senderContact,
          senderName: input.senderName,
          targetDomain: input.preview.normalizedDomain,
          targetName: input.targetName,
          to: input.recipientEmail,
          unsubscribeUrl: input.unsubscribeUrl,
          valueProposition: input.valueProposition,
        }),
        {
          actorUserId: input.operatorReview.operatorActorUserId,
          metadata: {
            ...(input.metadata ?? {}),
            operatorReviewEventId: input.operatorReview.id,
          },
          sourceAuthorityRef: input.preview.sourceAuthorityPackRef,
        },
        fetcher,
        runtime,
      )

    return yield* recordDispatch(db, input, result, runtime)
  })
