// KS-8.11 (#8322): read-only inspection accepts the union; reads stay on
// the authoritative D1 (no flag routing needed for operator inspection).
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
} from './crm-email-domain-store'
import {
  containsProviderSecretMaterial,
  redactProviderAccountSecretMaterial,
} from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

export const OperatorEmailInspectionScope = S.Struct({
  siteId: S.optionalKey(S.String),
  softwareOrderId: S.optionalKey(S.String),
})
export type OperatorEmailInspectionScope =
  typeof OperatorEmailInspectionScope.Type

export const OperatorEmailInspectionMessageStatus = S.Literals([
  'reserved',
  'rendered',
  'accepted',
  'failed',
  'draft_recorded',
  'skipped',
  'suppressed',
])
export type OperatorEmailInspectionMessageStatus =
  typeof OperatorEmailInspectionMessageStatus.Type

export type OperatorEmailDeliveryAttempt = Readonly<{
  attemptedAt: string
  completedAt: string | null
  errorMessage: string | null
  errorName: string | null
  id: string
  provider: string
  providerMessageId: string | null
  status: string
}>

export type OperatorEmailInspectionMessage = Readonly<{
  actionSubmissionId: string | null
  createdAt: string
  deliveryAttempts: number
  emailMessageId: string
  errorMessage: string | null
  errorName: string | null
  eventRefs: ReadonlyArray<string>
  idempotencyKey: string
  kind: string
  latestDelivery: OperatorEmailDeliveryAttempt | null
  provider: string | null
  providerMessageId: string | null
  relatedAssignmentIds: ReadonlyArray<string>
  relatedOrderIds: ReadonlyArray<string>
  relatedSiteIds: ReadonlyArray<string>
  safeNextAction: string
  skippedReason: string | null
  sourceAuthorityRef: string
  status: OperatorEmailInspectionMessageStatus
  templateSlug: string
  updatedAt: string
}>

export type OperatorEmailInspectionResult = Readonly<{
  messages: ReadonlyArray<OperatorEmailInspectionMessage>
  scope: OperatorEmailInspectionScope
  summary: Readonly<{
    accepted: number
    failed: number
    messageCount: number
    skipped: number
    suppressed: number
  }>
}>

type EmailMessageRow = Readonly<{
  action_submission_id: string | null
  created_at: string
  error_message: string | null
  error_name: string | null
  id: string
  idempotency_key: string
  kind: string
  provider: string | null
  provider_message_id: string | null
  source_authority_ref: string
  status: 'reserved' | 'rendered' | 'accepted' | 'failed' | 'draft_recorded'
  template_slug: string
  updated_at: string
}>

type EmailDeliveryRow = Readonly<{
  attempted_at: string
  completed_at: string | null
  error_message: string | null
  error_name: string | null
  id: string
  provider: string
  provider_message_id: string | null
  status: string
}>

type EventLinkRow = Readonly<{
  assignment_id: string | null
  event_id: string
  event_type: string
  event_source: 'assignment' | 'site'
  site_id: string | null
  software_order_id: string | null
}>

export class OperatorEmailInspectionStorageError extends S.TaggedErrorClass<OperatorEmailInspectionStorageError>()(
  'OperatorEmailInspectionStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class OperatorEmailInspectionInvalidScope extends S.TaggedErrorClass<OperatorEmailInspectionInvalidScope>()(
  'OperatorEmailInspectionInvalidScope',
  {
    reason: S.String,
  },
) {}

export type OperatorEmailInspectionError =
  | OperatorEmailInspectionInvalidScope
  | OperatorEmailInspectionStorageError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OperatorEmailInspectionStorageError> =>
  Effect.tryPromise({
    catch: error => new OperatorEmailInspectionStorageError({ error, operation }),
    try: run,
  })

const compact = (value: string | null, max = 500): string | null => {
  if (value === null) {
    return null
  }

  const redacted = redactProviderAccountSecretMaterial(value).trim()

  return redacted.length > max ? `${redacted.slice(0, max)}...` : redacted
}

const assertSafeProjection = (
  result: OperatorEmailInspectionResult,
): Effect.Effect<void, OperatorEmailInspectionStorageError> =>
  containsProviderSecretMaterial(JSON.stringify(result))
    ? Effect.fail(
        new OperatorEmailInspectionStorageError({
          error: new Error('Email inspection projection contains secret-shaped material.'),
          operation: 'OperatorEmailInspection.assertSafeProjection',
        }),
      )
    : Effect.void

const wildcard = (value: string | undefined): string =>
  `%:${(value ?? '').replaceAll('%', '\\%').replaceAll('_', '\\_')}:%`

const readMessages = (
  db: CrmEmailDatabase,
  scope: OperatorEmailInspectionScope,
): Effect.Effect<ReadonlyArray<EmailMessageRow>, OperatorEmailInspectionError> =>
  d1Effect('OperatorEmailInspection.messages.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT DISTINCT email_messages.id,
                email_messages.kind,
                email_messages.status,
                email_messages.provider,
                email_messages.provider_message_id,
                email_messages.error_name,
                email_messages.error_message,
                email_messages.idempotency_key,
                email_messages.template_slug,
                email_messages.source_authority_ref,
                email_messages.action_submission_id,
                email_messages.created_at,
                email_messages.updated_at
           FROM email_messages
          WHERE email_messages.id IN (
                  SELECT site_events.email_message_id
                    FROM site_events
                   WHERE site_events.email_message_id IS NOT NULL
                     AND (? IS NULL OR site_events.site_id = ?)
                )
             OR email_messages.id IN (
                  SELECT adjutant_assignment_events.email_message_id
                    FROM adjutant_assignment_events
                   WHERE adjutant_assignment_events.email_message_id IS NOT NULL
                     AND (? IS NULL OR adjutant_assignment_events.site_id = ?)
                     AND (? IS NULL OR adjutant_assignment_events.software_order_id = ?)
                )
             OR (? IS NOT NULL AND email_messages.idempotency_key LIKE ? ESCAPE '\\')
             OR (? IS NOT NULL AND email_messages.idempotency_key LIKE ? ESCAPE '\\')
          ORDER BY email_messages.updated_at DESC
          LIMIT 100`,
      )
      .bind(
        scope.siteId ?? null,
        scope.siteId ?? null,
        scope.siteId ?? null,
        scope.siteId ?? null,
        scope.softwareOrderId ?? null,
        scope.softwareOrderId ?? null,
        scope.softwareOrderId ?? null,
        wildcard(scope.softwareOrderId),
        scope.siteId ?? null,
        wildcard(scope.siteId),
      )
      .all<EmailMessageRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

const readDeliveries = (
  db: CrmEmailDatabase,
  messageId: string,
): Effect.Effect<ReadonlyArray<EmailDeliveryRow>, OperatorEmailInspectionError> =>
  d1Effect('OperatorEmailInspection.deliveries.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT id,
                provider,
                provider_message_id,
                status,
                error_name,
                error_message,
                attempted_at,
                completed_at
           FROM email_deliveries
          WHERE message_id = ?
          ORDER BY attempted_at DESC
          LIMIT 20`,
      )
      .bind(messageId)
      .all<EmailDeliveryRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

const readEventLinks = (
  db: CrmEmailDatabase,
  messageId: string,
): Effect.Effect<ReadonlyArray<EventLinkRow>, OperatorEmailInspectionError> =>
  d1Effect('OperatorEmailInspection.eventLinks.read', () =>
    crmEmailAuthorityDb(db)
      .prepare(
        `SELECT 'site' AS event_source,
                site_events.id AS event_id,
                site_events.type AS event_type,
                NULL AS assignment_id,
                site_events.site_id AS site_id,
                site_projects.software_order_id AS software_order_id
           FROM site_events
           LEFT JOIN site_projects
             ON site_projects.id = site_events.site_id
            AND site_projects.archived_at IS NULL
          WHERE site_events.email_message_id = ?
          UNION ALL
         SELECT 'assignment' AS event_source,
                adjutant_assignment_events.id AS event_id,
                adjutant_assignment_events.event_type AS event_type,
                adjutant_assignment_events.assignment_id AS assignment_id,
                adjutant_assignment_events.site_id AS site_id,
                adjutant_assignment_events.software_order_id AS software_order_id
           FROM adjutant_assignment_events
          WHERE adjutant_assignment_events.email_message_id = ?`,
      )
      .bind(messageId, messageId)
      .all<EventLinkRow>(),
  ).pipe(Effect.map(result => result.results ?? []))

const projectedStatus = (
  row: EmailMessageRow,
): OperatorEmailInspectionMessageStatus => {
  if (row.error_name === 'email_config_missing') {
    return 'skipped'
  }

  if (row.error_name === 'email_suppressed') {
    return 'suppressed'
  }

  return row.status
}

const skippedReason = (row: EmailMessageRow): string | null =>
  row.error_name === 'email_config_missing' ||
  row.error_name === 'email_suppressed'
    ? row.error_name
    : null

const safeNextAction = (
  status: OperatorEmailInspectionMessageStatus,
): string => {
  switch (status) {
    case 'accepted':
      return 'No operator action is needed for this email.'
    case 'failed':
      return 'Inspect the redacted error and retry only after fixing the provider or template issue.'
    case 'skipped':
      return 'Confirm the skipped reason is intentional, then resend if the configuration or target is fixed.'
    case 'suppressed':
      return 'Do not resend unless suppression policy is changed by an operator.'
    case 'draft_recorded':
      return 'Review the recorded draft before sending.'
    case 'rendered':
    case 'reserved':
      return 'Email is reserved/rendered but not accepted by a provider yet.'
  }
}

const projectionFromRows = (
  row: EmailMessageRow,
  deliveries: ReadonlyArray<EmailDeliveryRow>,
  links: ReadonlyArray<EventLinkRow>,
): OperatorEmailInspectionMessage => {
  const status = projectedStatus(row)
  const latest = deliveries[0]
  const eventRefs = links.map(
    link => `${link.event_source}:${link.event_type}:${link.event_id}`,
  )
  const relatedAssignmentIds = [
    ...new Set(
      links
        .map(link => link.assignment_id)
        .filter((value): value is string => value !== null),
    ),
  ].sort()
  const relatedOrderIds = [
    ...new Set(
      links
        .map(link => link.software_order_id)
        .filter((value): value is string => value !== null),
    ),
  ].sort()
  const relatedSiteIds = [
    ...new Set(
      links
        .map(link => link.site_id)
        .filter((value): value is string => value !== null),
    ),
  ].sort()

  return {
    actionSubmissionId: row.action_submission_id,
    createdAt: row.created_at,
    deliveryAttempts: deliveries.length,
    emailMessageId: row.id,
    errorMessage: compact(row.error_message),
    errorName: compact(row.error_name, 120),
    eventRefs,
    idempotencyKey: row.idempotency_key,
    kind: row.kind,
    latestDelivery:
      latest === undefined
        ? null
        : {
            attemptedAt: latest.attempted_at,
            completedAt: latest.completed_at,
            errorMessage: compact(latest.error_message),
            errorName: compact(latest.error_name, 120),
            id: latest.id,
            provider: latest.provider,
            providerMessageId: latest.provider_message_id,
            status: latest.status,
          },
    provider: row.provider,
    providerMessageId: row.provider_message_id,
    relatedAssignmentIds,
    relatedOrderIds,
    relatedSiteIds,
    safeNextAction: safeNextAction(status),
    skippedReason: skippedReason(row),
    sourceAuthorityRef: row.source_authority_ref,
    status,
    templateSlug: row.template_slug,
    updatedAt: row.updated_at,
  }
}

export const inspectOperatorEmailDelivery = (
  db: CrmEmailDatabase,
  scope: OperatorEmailInspectionScope,
): Effect.Effect<OperatorEmailInspectionResult, OperatorEmailInspectionError> =>
  Effect.gen(function* () {
    if (scope.siteId === undefined && scope.softwareOrderId === undefined) {
      return yield* new OperatorEmailInspectionInvalidScope({
        reason: 'siteId or softwareOrderId is required.',
      })
    }

    const rows = yield* readMessages(db, scope)
    const messages = yield* Effect.forEach(rows, row =>
      Effect.gen(function* () {
        const deliveries = yield* readDeliveries(db, row.id)
        const links = yield* readEventLinks(db, row.id)

        return projectionFromRows(row, deliveries, links)
      }),
    )
    const result: OperatorEmailInspectionResult = {
      messages,
      scope,
      summary: {
        accepted: messages.filter(message => message.status === 'accepted').length,
        failed: messages.filter(message => message.status === 'failed').length,
        messageCount: messages.length,
        skipped: messages.filter(message => message.status === 'skipped').length,
        suppressed: messages.filter(message => message.status === 'suppressed').length,
      },
    }

    yield* assertSafeProjection(result)

    return result
  })
