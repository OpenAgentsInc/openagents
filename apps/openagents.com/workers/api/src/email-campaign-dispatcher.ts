import type { DripEmailKind } from '@openagentsinc/email-templates'
import { Effect } from 'effect'

import type { ResendEmailConfig } from './config'
// KS-8.11 (#8322): CrmEmailDatabase union — the dispatch cron's claim/skip/
// sent/failed transitions on email_campaign_sends mirror to Postgres
// fail-soft; the suppression/preference gates route through the flag-gated
// seam inside email-campaigns.ts.
import {
  crmEmailAuthorityDb,
  mirrorCrmEmailRows,
  type CrmEmailDatabase,
} from './crm-email-domain-store'
import {
  DripCampaignEmailInput,
  type EmailLedgerSendResult,
  type EmailRuntime,
  sendDripCampaignEmailWithLedger,
  systemEmailRuntime,
} from './email'
import { isEmailSuppressed, readEmailPreferenceAllows } from './email-campaigns'
import {
  type EmailSequenceSendRow,
  type EmailSequenceSendServiceDependencies,
  makeEmailSequenceSendService,
} from './email-sequence-send-service'
import { parseJsonRecord } from './json-boundary'
import { isoTimestampAfterIso } from './runtime-primitives'

export type EmailCampaignDispatcherResult = Readonly<{
  claimed: number
  failed: number
  retried: number
  sent: number
  skipped: number
  suppressed: number
}>

export type EmailCampaignDispatcherOptions = Readonly<{
  appOrigin: string
  fetcher?: typeof fetch | undefined
  limit?: number | undefined
  maxAttempts?: number | undefined
  resend?: ResendEmailConfig | undefined
  runtime?: EmailRuntime | undefined
  sequenceSend?: EmailSequenceSendServiceDependencies | undefined
}>

type DueCampaignSendRow = Readonly<{
  attempt_count: number
  campaign_id: string
  delay_seconds: number
  due_at: string
  email: string
  enrollment_id: string
  id: string
  idempotency_key: string
  lifecycle_kind: string | null
  metadata_json: string
  source_authority_ref: string
  step_id: string
  step_key: string
  template_slug: string
  user_id: string | null
}>

type UserOrderStateRow = Readonly<{ status: string }>

const emptyResult = (): EmailCampaignDispatcherResult => ({
  claimed: 0,
  failed: 0,
  retried: 0,
  sent: 0,
  skipped: 0,
  suppressed: 0,
})

const mergeResult = (
  left: EmailCampaignDispatcherResult,
  right: EmailCampaignDispatcherResult,
): EmailCampaignDispatcherResult => ({
  claimed: left.claimed + right.claimed,
  failed: left.failed + right.failed,
  retried: left.retried + right.retried,
  sent: left.sent + right.sent,
  skipped: left.skipped + right.skipped,
  suppressed: left.suppressed + right.suppressed,
})

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const campaignSendChanges = (result: D1Result): number =>
  typeof (result.meta as { changes?: unknown }).changes === 'number'
    ? ((result.meta as { changes: number }).changes ?? 0)
    : 0

const metadataString = (
  metadata: Record<string, unknown> | null | undefined,
  key: string,
): string | null => {
  const value = metadata?.[key]

  return typeof value === 'string' && value.trim() !== ''
    ? compactText(value, 160)
    : null
}

const dripKindFromRow = (row: DueCampaignSendRow): DripEmailKind | null =>
  row.lifecycle_kind === 'signup_day_0' ||
  row.lifecycle_kind === 'signup_day_1' ||
  row.lifecycle_kind === 'signup_day_2'
    ? row.lifecycle_kind
    : row.template_slug === 'drip.signup_day_0.v1'
      ? 'signup_day_0'
      : row.template_slug === 'drip.signup_day_1.v1'
        ? 'signup_day_1'
        : row.template_slug === 'drip.signup_day_2.v1'
          ? 'signup_day_2'
          : null

const managePreferencesUrl = (appOrigin: string): string =>
  `${appOrigin.replace(/\/+$/, '')}/email/preferences`

const listDueCampaignSends = (
  db: CrmEmailDatabase,
  now: string,
  limit: number,
): Effect.Effect<ReadonlyArray<DueCampaignSendRow>, never> =>
  Effect.tryPromise(async () => {
    const result = await crmEmailAuthorityDb(db)
      .prepare(
        `SELECT sends.id,
                  sends.campaign_id,
                  sends.step_id,
                  sends.enrollment_id,
                  sends.user_id,
                  sends.email,
                  sends.due_at,
                  sends.idempotency_key,
                  sends.source_authority_ref,
                  sends.metadata_json,
                  sends.attempt_count,
                  steps.step_key,
                  steps.delay_seconds,
                  steps.template_slug,
                  steps.lifecycle_kind
             FROM email_campaign_sends AS sends
             JOIN email_campaign_steps AS steps
               ON steps.id = sends.step_id
            WHERE sends.status = 'scheduled'
              AND sends.due_at <= ?
              AND (sends.next_attempt_at IS NULL OR sends.next_attempt_at <= ?)
            ORDER BY sends.due_at ASC
            LIMIT ?`,
      )
      .bind(now, now, Math.max(1, Math.min(limit, 100)))
      .all<DueCampaignSendRow>()

    return result.results
  }).pipe(Effect.catch(() => Effect.succeed([])))

const claimCampaignSend = (
  db: CrmEmailDatabase,
  sendId: string,
  now: string,
): Effect.Effect<boolean, never> =>
  Effect.tryPromise(async () => {
    const result = await crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE email_campaign_sends
              SET status = 'claimed',
                  attempt_count = attempt_count + 1,
                  claimed_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND status = 'scheduled'`,
      )
      .bind(now, now, sendId)
      .run()

    await mirrorCrmEmailRows(db, 'email_campaign_sends', 'id', [sendId])

    return campaignSendChanges(result) > 0
  }).pipe(Effect.catch(() => Effect.succeed(false)))

const readUserOrderState = (
  db: CrmEmailDatabase,
  userId: string | null,
): Effect.Effect<'active' | 'delivered' | 'none', never> => {
  if (userId === null) {
    return Effect.succeed('none')
  }

  return Effect.tryPromise(async () => {
    const row = await crmEmailAuthorityDb(db)
      .prepare(
        `SELECT status
             FROM software_orders
            WHERE user_id = ?
              AND archived_at IS NULL
              AND status IN (
                'submitted',
                'scoping',
                'free_slice_ready',
                'quote_ready',
                'agent_queued',
                'agent_running',
                'needs_customer_input',
                'delivered'
              )
            ORDER BY CASE
              WHEN status IN (
                'submitted',
                'scoping',
                'free_slice_ready',
                'quote_ready',
                'agent_queued',
                'agent_running',
                'needs_customer_input'
              ) THEN 0
              ELSE 1
            END ASC,
            updated_at DESC
            LIMIT 1`,
      )
      .bind(userId)
      .first<UserOrderStateRow>()

    if (row === null) {
      return 'none'
    }

    return row.status === 'delivered' ? 'delivered' : 'active'
  }).pipe(Effect.catch(() => Effect.succeed('none' as const)))
}

const markCampaignSendSkipped = (
  db: CrmEmailDatabase,
  row: DueCampaignSendRow,
  now: string,
  errorName: string,
  errorMessage: string,
  status: 'skipped' | 'suppressed',
): Effect.Effect<void, never> =>
  Effect.tryPromise(async () => {
    await crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE email_campaign_sends
              SET status = ?,
                  error_name = ?,
                  error_message = ?,
                  skipped_at = ?,
                  updated_at = ?
            WHERE id = ?`,
      )
      .bind(
        status,
        compactText(errorName, 120),
        compactText(errorMessage, 500),
        now,
        now,
        row.id,
      )
      .run()

    await mirrorCrmEmailRows(db, 'email_campaign_sends', 'id', [row.id])
  }).pipe(Effect.catch(() => Effect.void))

const markCampaignSendSent = (
  db: CrmEmailDatabase,
  row: DueCampaignSendRow,
  now: string,
  result: EmailLedgerSendResult & { ok: true },
): Effect.Effect<void, never> =>
  Effect.tryPromise(async () => {
    await crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE email_campaign_sends
              SET status = 'sent',
                  email_message_id = ?,
                  provider_event_id = ?,
                  error_name = NULL,
                  error_message = NULL,
                  sent_at = ?,
                  updated_at = ?
            WHERE id = ?`,
      )
      .bind(result.emailMessageId, result.providerMessageId, now, now, row.id)
      .run()

    await mirrorCrmEmailRows(db, 'email_campaign_sends', 'id', [row.id])
  }).pipe(Effect.catch(() => Effect.void))

const markCampaignSendFailedOrRetry = (
  db: CrmEmailDatabase,
  row: DueCampaignSendRow,
  now: string,
  attemptCount: number,
  maxAttempts: number,
  result: EmailLedgerSendResult & { ok: false },
): Effect.Effect<'failed' | 'retried', never> =>
  Effect.tryPromise(async () => {
    const errorName = compactText(result.errorName ?? 'email_send_failed', 120)
    const errorMessage = compactText(result.errorMessage, 500)

    if (attemptCount < maxAttempts) {
      const retryAt = isoTimestampAfterIso(now, 300_000 * attemptCount)

      await crmEmailAuthorityDb(db)
        .prepare(
          `UPDATE email_campaign_sends
                SET status = 'scheduled',
                    email_message_id = ?,
                    due_at = ?,
                    next_attempt_at = ?,
                    error_name = ?,
                    error_message = ?,
                    claimed_at = NULL,
                    updated_at = ?
              WHERE id = ?`,
        )
        .bind(
          result.emailMessageId,
          retryAt,
          retryAt,
          errorName,
          errorMessage,
          now,
          row.id,
        )
        .run()

      await mirrorCrmEmailRows(db, 'email_campaign_sends', 'id', [row.id])

      return 'retried'
    }

    await crmEmailAuthorityDb(db)
      .prepare(
        `UPDATE email_campaign_sends
              SET status = 'failed',
                  email_message_id = ?,
                  error_name = ?,
                  error_message = ?,
                  failed_at = ?,
                  updated_at = ?
            WHERE id = ?`,
      )
      .bind(result.emailMessageId, errorName, errorMessage, now, now, row.id)
      .run()

    await mirrorCrmEmailRows(db, 'email_campaign_sends', 'id', [row.id])

    return 'failed'
  }).pipe(Effect.catch(() => Effect.succeed('failed' as const)))

type ClaimedDispatchOptions = Readonly<{
  appOrigin: string
  fetcher?: typeof fetch | undefined
  maxAttempts: number
  resend?: ResendEmailConfig | undefined
  runtime: EmailRuntime
  sequenceSend?: EmailSequenceSendServiceDependencies | undefined
}>

const dueCampaignSendRowToSequenceRow = (
  row: DueCampaignSendRow,
  metadata: Record<string, unknown> | null | undefined,
): EmailSequenceSendRow => ({
  campaignId: row.campaign_id,
  displayName: metadataString(metadata, 'displayName'),
  email: row.email,
  enrollmentId: row.enrollment_id,
  idempotencyKey: row.idempotency_key,
  sendId: row.id,
  sourceAuthorityRef: row.source_authority_ref,
  stepId: row.step_id,
  stepKey: row.step_key,
  templateSlug: row.template_slug,
  userId: row.user_id,
})

const dispatchClaimedCampaignSend = (
  db: CrmEmailDatabase,
  row: DueCampaignSendRow,
  input: ClaimedDispatchOptions,
): Effect.Effect<EmailCampaignDispatcherResult, never> =>
  Effect.gen(function* () {
    const now = input.runtime.nowIso()
    const attemptCount = row.attempt_count + 1

    if (yield* Effect.promise(() => isEmailSuppressed(db, row.email, 'drip'))) {
      yield* markCampaignSendSkipped(
        db,
        row,
        now,
        'drip_suppressed',
        'Recipient has an active drip suppression.',
        'suppressed',
      )

      return { ...emptyResult(), claimed: 1, suppressed: 1 }
    }

    if (
      !(yield* Effect.promise(() =>
        readEmailPreferenceAllows(db, row.email, 'drip'),
      ))
    ) {
      yield* markCampaignSendSkipped(
        db,
        row,
        now,
        'drip_preference_disabled',
        'Recipient disabled drip emails.',
        'suppressed',
      )

      return { ...emptyResult(), claimed: 1, suppressed: 1 }
    }

    const orderState = yield* readUserOrderState(db, row.user_id)

    if (orderState === 'active' || orderState === 'delivered') {
      yield* markCampaignSendSkipped(
        db,
        row,
        now,
        orderState === 'active' ? 'active_order' : 'delivered_order',
        orderState === 'active'
          ? 'Recipient already has active requested work.'
          : 'Recipient already has delivered requested work.',
        'skipped',
      )

      return { ...emptyResult(), claimed: 1, skipped: 1 }
    }

    if (input.resend === undefined) {
      yield* markCampaignSendSkipped(
        db,
        row,
        now,
        'email_config_missing',
        'Resend email configuration is not set.',
        'skipped',
      )

      return { ...emptyResult(), claimed: 1, skipped: 1 }
    }

    const kind = dripKindFromRow(row)

    const metadata = parseJsonRecord(row.metadata_json)

    if (kind === null && input.sequenceSend !== undefined) {
      const sequenceSend = input.sequenceSend
      const outcome = yield*
        makeEmailSequenceSendService(
          sequenceSend,
        ).dispatchSequenceSend(dueCampaignSendRowToSequenceRow(row, metadata))

      if (outcome.kind === 'dry_run') {
        yield* markCampaignSendSkipped(
          db,
          row,
          now,
          'email_sequence_send_disabled',
          'Email sequence send service is disabled.',
          'skipped',
        )

        return { ...emptyResult(), claimed: 1, skipped: 1 }
      }

      if (outcome.kind === 'sent') {
        yield* markCampaignSendSent(db, row, now, outcome.result)

        return { ...emptyResult(), claimed: 1, sent: 1 }
      }

      const failedState = yield* markCampaignSendFailedOrRetry(
        db,
        row,
        now,
        attemptCount,
        input.maxAttempts,
        outcome.result,
      )

      return failedState === 'retried'
        ? { ...emptyResult(), claimed: 1, retried: 1 }
        : { ...emptyResult(), claimed: 1, failed: 1 }
    }

    if (kind === null) {
      yield* markCampaignSendSkipped(
        db,
        row,
        now,
        'unsupported_campaign_template',
        'Campaign send does not reference a supported drip template.',
        'skipped',
      )

      return { ...emptyResult(), claimed: 1, skipped: 1 }
    }

    const result = yield* sendDripCampaignEmailWithLedger(
      db,
      input.resend,
      new DripCampaignEmailInput({
        appOrigin: input.appOrigin,
        displayName: metadataString(metadata, 'displayName') ?? 'there',
        idempotencyKey: row.idempotency_key,
        kind,
        managePreferencesUrl: managePreferencesUrl(input.appOrigin),
        to: row.email,
      }),
      {
        metadata: {
          campaignId: row.campaign_id,
          enrollmentId: row.enrollment_id,
          stepId: row.step_id,
          stepKey: row.step_key,
        },
        sourceAuthorityRef: row.source_authority_ref,
        targetUserId: row.user_id ?? undefined,
      },
      input.fetcher,
      input.runtime,
    ).pipe(
      Effect.catch(error =>
        Effect.succeed({
          emailMessageId: row.id,
          errorMessage: error.message,
          errorName: error.operation,
          ok: false as const,
        }),
      ),
    )

    if (result.ok) {
      yield* markCampaignSendSent(db, row, now, result)

      return { ...emptyResult(), claimed: 1, sent: 1 }
    }

    const failedState = yield* markCampaignSendFailedOrRetry(
      db,
      row,
      now,
      attemptCount,
      input.maxAttempts,
      result,
    )

    return failedState === 'retried'
      ? { ...emptyResult(), claimed: 1, retried: 1 }
      : { ...emptyResult(), claimed: 1, failed: 1 }
  })

export const dispatchDueEmailCampaignSends = (
  db: CrmEmailDatabase,
  options: EmailCampaignDispatcherOptions,
): Effect.Effect<EmailCampaignDispatcherResult, never> =>
  Effect.gen(function* () {
    const runtime = options.runtime ?? systemEmailRuntime
    const now = runtime.nowIso()
    const due = yield* listDueCampaignSends(db, now, options.limit ?? 25)
    const input = {
      appOrigin: options.appOrigin,
      fetcher: options.fetcher,
      maxAttempts: options.maxAttempts ?? 3,
      resend: options.resend,
      runtime,
      sequenceSend: options.sequenceSend,
    }

    const results = yield* Effect.forEach(
      due,
      row =>
        Effect.gen(function* () {
          const claimed = yield* claimCampaignSend(db, row.id, now)

          return claimed
            ? yield* dispatchClaimedCampaignSend(db, row, input)
            : emptyResult()
        }),
      { concurrency: 1 },
    )

    return results.reduce(mergeResult, emptyResult())
  }).pipe(Effect.withSpan('EmailCampaignDispatcher.dispatchDue'))
