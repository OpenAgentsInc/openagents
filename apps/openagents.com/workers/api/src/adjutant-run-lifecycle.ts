import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import {
  type AdjutantSiteArtifactReceipt,
  firstReceiptFromOmniEvents,
} from './adjutant-site-artifact-receipts'
import {
  type RecordAdjutantUsageReceiptInput,
  recordAdjutantUsageReceipt,
} from './adjutant-usage-receipts'
import type { ResendEmailConfig } from './config'
import {
  type AdjutantCustomerNotificationStage,
  type EmailLedgerSendResult,
  type OrderSitesTransactionalEmailKind,
  OrderSitesTransactionalEmailInput,
  buildOrderSitesTransactionalEmailIdempotencyKey,
  sendOrderSitesTransactionalEmailWithLedger,
} from './email'
import type { OmniEventRecord } from './omni-runs'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import { AutopilotSitesService } from './sites'
import {
  collectSiteVisualAssetFindings,
  inferSiteVisualAssetRequirements,
} from './sites-build-validations'
import type { SiteCompatibilityProjectFile } from './sites-compatibility'

type AdjutantRunAssignmentRow = Readonly<{
  assigned_by_user_id: string | null
  commit_sha: string | null
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  site_id: string | null
  software_order_id: string | null
  status: string
  task_spec_path: string | null
  visibility: 'private' | 'team' | 'public'
}>

type SavedSiteVersionPointer = Readonly<{
  build_status: string
  id: string
}>

type SiteRevisionActivationProjectRow = Readonly<{
  active_deployment_id: string | null
  active_version_id: string | null
  id: string
  slug: string
}>

type SiteRevisionDeploymentRow = Readonly<{
  id: string
}>

type SiteRevisionFeedbackRequirementRow = Readonly<{
  body: string
}>

type CustomerNotificationTargetRow = Readonly<{
  active_version_id: string | null
  display_name: string
  order_id: string
  primary_email: string | null
  site_title: string | null
  site_url: string | null
  target_user_id: string
}>

type CustomerNotificationOutcome = Readonly<{
  emailMessageId: string | null
  emailStatus: 'accepted' | 'failed' | 'skipped'
  errorMessage?: string | undefined
  errorName?: string | undefined
  providerMessageId?: string | null | undefined
  skipReason?: string | undefined
}>

type LifecycleStage =
  | 'delivered'
  | 'needs_customer_input'
  | 'queued'
  | 'running'
  | 'unavailable'

type LifecycleNotificationStage =
  | AdjutantCustomerNotificationStage
  | 'autopilot_queued'
  | 'autopilot_running'

type LifecycleMapping = Readonly<{
  assignmentStatus:
    | 'blocked'
    | 'delivered'
    | 'queued'
    | 'review_needed'
    | 'running'
  eventType:
    | 'adjutant.customer_input_needed'
    | 'adjutant.run_delivered'
    | 'adjutant.run_queued'
    | 'adjutant.run_running'
    | 'adjutant.run_unavailable'
  orderStatus:
    | 'agent_queued'
    | 'agent_running'
    | 'delivered'
    | 'needs_customer_input'
    | 'unavailable'
  stage: LifecycleStage
  summary: string
}>

export class AdjutantRunLifecycleStorageError extends S.TaggedErrorClass<AdjutantRunLifecycleStorageError>()(
  'AdjutantRunLifecycleStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

type AdjutantRunLifecycleInput = Readonly<{
  actorUserId: string
  appOrigin?: string | undefined
  artifacts?: R2Bucket | undefined
  emailConfig?: ResendEmailConfig | undefined
  emailFetcher?: typeof fetch | undefined
  events: ReadonlyArray<OmniEventRecord>
  runId: string
  status?: string | undefined
}>

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdjutantRunLifecycleStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdjutantRunLifecycleStorageError({ operation, error }),
  })

const notificationOutcomeFromSendResult = (
  result: EmailLedgerSendResult,
): CustomerNotificationOutcome =>
  result.ok
    ? {
        emailMessageId: result.emailMessageId,
        emailStatus: 'accepted',
        providerMessageId: result.providerMessageId,
      }
    : {
        emailMessageId: result.emailMessageId,
        emailStatus: 'failed',
        errorMessage: result.errorMessage,
        errorName: result.errorName,
      }

const stageFromText = (
  value: string | null | undefined,
): LifecycleStage | undefined => {
  const text = value?.toLowerCase() ?? ''

  if (text === 'queued') {
    return 'queued'
  }

  if (
    text === 'running' ||
    text.includes('started') ||
    text.includes('start') ||
    text.includes('dispatched')
  ) {
    return 'running'
  }

  if (
    text === 'waiting_for_input' ||
    text.includes('input_required') ||
    text.includes('needs_customer_input')
  ) {
    return 'needs_customer_input'
  }

  if (text === 'completed' || text.includes('completed')) {
    return 'delivered'
  }

  if (
    text === 'failed' ||
    text === 'canceled' ||
    text.includes('failed') ||
    text.includes('error')
  ) {
    return 'unavailable'
  }

  return undefined
}

const stageFromEvent = (event: OmniEventRecord): LifecycleStage | undefined =>
  stageFromText(event.status) ?? stageFromText(event.type)

const lifecycleStage = (
  input: AdjutantRunLifecycleInput,
): LifecycleStage | undefined =>
  stageFromText(input.status) ??
  [...input.events]
    .reverse()
    .map(stageFromEvent)
    .find((stage): stage is LifecycleStage => stage !== undefined)

const lifecycleMapping = (stage: LifecycleStage): LifecycleMapping =>
  stage === 'queued'
    ? {
        assignmentStatus: 'queued',
        eventType: 'adjutant.run_queued',
        orderStatus: 'agent_queued',
        stage,
        summary: 'Autopilot run is queued.',
      }
    : stage === 'running'
      ? {
          assignmentStatus: 'running',
          eventType: 'adjutant.run_running',
          orderStatus: 'agent_running',
          stage,
          summary: 'Autopilot run is in progress.',
        }
      : stage === 'needs_customer_input'
        ? {
            assignmentStatus: 'review_needed',
            eventType: 'adjutant.customer_input_needed',
            orderStatus: 'needs_customer_input',
            stage,
            summary: 'Autopilot needs customer input.',
          }
        : stage === 'delivered'
          ? {
              assignmentStatus: 'delivered',
              eventType: 'adjutant.run_delivered',
              orderStatus: 'delivered',
              stage,
              summary: 'Autopilot delivered the run result.',
            }
          : {
              assignmentStatus: 'blocked',
              eventType: 'adjutant.run_unavailable',
              orderStatus: 'unavailable',
              stage,
              summary: 'Autopilot run is unavailable.',
            }

const notificationStageForLifecycleStage = (
  stage: LifecycleStage,
): LifecycleNotificationStage =>
  stage === 'queued'
    ? 'autopilot_queued'
    : stage === 'running'
      ? 'autopilot_running'
      : stage === 'delivered'
        ? 'review_ready'
        : stage === 'needs_customer_input'
          ? 'input_needed'
          : 'unavailable'

const notificationKindForLifecycleStage = (
  stage: LifecycleStage,
): OrderSitesTransactionalEmailKind =>
  stage === 'queued'
    ? 'autopilot_queued'
    : stage === 'running'
      ? 'autopilot_running'
      : stage === 'needs_customer_input'
        ? 'customer_input_needed'
        : stage === 'delivered'
          ? 'review_ready'
          : 'unavailable_declined'

const customerSafeStatusForLifecycleStage = (stage: LifecycleStage): string =>
  stage === 'queued'
    ? 'queued'
    : stage === 'running'
      ? 'running'
      : stage === 'needs_customer_input'
        ? 'waiting for customer input'
        : stage === 'delivered'
          ? 'ready for review'
          : 'unavailable'

const customerNextActionForLifecycleStage = (stage: LifecycleStage): string =>
  stage === 'queued'
    ? 'OpenAgents will start the next Autopilot work step when the run is ready.'
    : stage === 'running'
      ? 'OpenAgents will send another update when the work is ready for review or needs input.'
      : stage === 'needs_customer_input'
        ? 'Reply with the requested details so OpenAgents can continue.'
        : stage === 'delivered'
          ? 'Review the result and reply with approval or requested changes.'
          : 'Review the reason and reply if the scope should change.'

const customerSafeReasonForLifecycleStage = (
  stage: LifecycleStage,
): string | null =>
  stage === 'unavailable'
    ? 'The current Autopilot run cannot continue with the available information.'
    : null

const notificationEventType = (
  stage: LifecycleNotificationStage,
): string => `adjutant.notification.${stage}`

const latestEvent = (
  events: ReadonlyArray<OmniEventRecord>,
): OmniEventRecord | undefined => events.at(-1)

const lifecyclePayload = (
  input: AdjutantRunLifecycleInput,
  mapping: LifecycleMapping,
  assignment: AdjutantRunAssignmentRow,
): Record<string, unknown> => {
  const event = latestEvent(input.events)

  return {
    assignmentId: assignment.id,
    commitSha: assignment.commit_sha,
    eventCount: input.events.length,
    latestEvent:
      event === undefined
        ? null
        : {
            sequence: event.sequence,
            source: event.source,
            status: event.status,
            type: event.type,
          },
    runId: input.runId,
    runStatus: input.status ?? null,
    stage: mapping.stage,
    taskSpecPath: assignment.task_spec_path,
  }
}

const payloadJson = (
  payload: Record<string, unknown>,
): Effect.Effect<string, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    const json = yield* Effect.try({
      catch: error =>
        new AdjutantRunLifecycleStorageError({
          error,
          operation: 'adjutantRunLifecycle.payload',
        }),
      try: () => JSON.stringify(payload),
    })

    if (json.length > 4096) {
      return yield* new AdjutantRunLifecycleStorageError({
        error: 'Lifecycle payload is too large.',
        operation: 'adjutantRunLifecycle.payload',
      })
    }

    if (containsProviderSecretMaterial(json)) {
      return yield* new AdjutantRunLifecycleStorageError({
        error: 'Lifecycle payload contains secret-shaped material.',
        operation: 'adjutantRunLifecycle.payload',
      })
    }

    return json
  })

const readAssignmentByRunId = (
  db: D1Database,
  runId: string,
): Effect.Effect<
  AdjutantRunAssignmentRow | null,
  AdjutantRunLifecycleStorageError
> =>
  d1Effect('adjutantRunLifecycle.assignment.readByRunId', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                assigned_by_user_id,
                status,
                visibility,
                task_spec_path,
                commit_sha,
                objective
           FROM adjutant_assignments
          WHERE current_run_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(runId)
      .first<AdjutantRunAssignmentRow>(),
  )

const readCustomerNotificationTarget = (
  db: D1Database,
  assignment: AdjutantRunAssignmentRow,
): Effect.Effect<
  CustomerNotificationTargetRow | null,
  AdjutantRunLifecycleStorageError
> =>
  assignment.software_order_id === null
    ? Effect.succeed(null)
    : d1Effect('adjutantRunLifecycle.notificationTarget.read', () =>
        db
          .prepare(
            `SELECT software_orders.id AS order_id,
                    users.id AS target_user_id,
                    users.display_name,
                    users.primary_email,
                    site_projects.active_version_id,
                    site_projects.title AS site_title,
                    active_deployments.url AS site_url
               FROM software_orders
               INNER JOIN users
                  ON users.id = software_orders.user_id
                 AND users.deleted_at IS NULL
               LEFT JOIN site_projects
                  ON site_projects.id = ?
                 AND site_projects.archived_at IS NULL
               LEFT JOIN site_deployments AS active_deployments
                  ON active_deployments.id = site_projects.active_deployment_id
                 AND active_deployments.status = 'active'
              WHERE software_orders.id = ?
                AND software_orders.archived_at IS NULL
              LIMIT 1`,
          )
          .bind(assignment.site_id, assignment.software_order_id)
          .first<CustomerNotificationTargetRow>(),
      )

const siteRevisionUrl = (
  siteUrl: string | null,
  versionId: string | null,
): string | null =>
  siteUrl === null || versionId === null
    ? null
    : `${siteUrl.replace(/\/+$/, '')}/versions/${encodeURIComponent(versionId)}`

const sendCustomerNotification = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  stage: LifecycleStage,
): Effect.Effect<
  CustomerNotificationOutcome,
  AdjutantRunLifecycleStorageError
> =>
  Effect.gen(function* () {
    const target = yield* readCustomerNotificationTarget(db, assignment)

    if (target === null) {
      return {
        emailMessageId: null,
        emailStatus: 'skipped',
        skipReason: 'missing_order_notification_target',
      }
    }

    const email = target.primary_email?.trim()

    if (email === undefined || email === '') {
      return {
        emailMessageId: null,
        emailStatus: 'skipped',
        skipReason: 'missing_customer_email',
      }
    }

    if (input.emailConfig === undefined) {
      return {
        emailMessageId: null,
        emailStatus: 'skipped',
        skipReason: 'email_config_missing',
      }
    }

    const notificationInputWithoutKey = new OrderSitesTransactionalEmailInput({
      appOrigin: input.appOrigin ?? 'https://openagents.com',
      assignmentId: assignment.id,
      artifactLabel: null,
      artifactUrl: null,
      customerSafeStatus: customerSafeStatusForLifecycleStage(stage),
      displayName: target.display_name,
      eventRef: input.runId,
      lifecycleKind: notificationKindForLifecycleStage(stage),
      nextAction: customerNextActionForLifecycleStage(stage),
      orderId: target.order_id,
      revisionUrl: siteRevisionUrl(target.site_url, target.active_version_id),
      safeReason: customerSafeReasonForLifecycleStage(stage),
      ...(assignment.site_id === null ? {} : { siteId: assignment.site_id }),
      siteTitle: target.site_title,
      siteUrl: target.site_url,
      sourceAuthorityRefs: [
        'docs/2026-06-05-adjutant-sites-supervisor-audit.md#16',
      ],
      targetRefs: [
        assignment.id,
        input.runId,
        target.order_id,
        ...(assignment.site_id === null ? [] : [assignment.site_id]),
      ],
      to: email,
    })
    const notificationInput = new OrderSitesTransactionalEmailInput({
      ...notificationInputWithoutKey,
      idempotencyKey: buildOrderSitesTransactionalEmailIdempotencyKey(
        notificationInputWithoutKey,
      ),
    })

    const result = yield* sendOrderSitesTransactionalEmailWithLedger(
      db,
      input.emailConfig,
      notificationInput,
      {
        actorUserId: input.actorUserId,
        metadata: {
          assignmentId: assignment.id,
          eventSource: 'adjutant_run_lifecycle',
          lifecycleKind: notificationInput.lifecycleKind,
          runId: input.runId,
          softwareOrderId: assignment.software_order_id,
          stage: notificationStageForLifecycleStage(stage),
        },
        sourceAuthorityRef:
          'system.order_sites_lifecycle_email.v1',
        targetUserId: target.target_user_id,
      },
      input.emailFetcher,
    ).pipe(
      Effect.mapError(
        error =>
          new AdjutantRunLifecycleStorageError({
            error,
            operation: 'adjutantRunLifecycle.customerNotification.send',
          }),
      ),
    )

    return notificationOutcomeFromSendResult(result)
  })

const readSiteVersionForRun = (
  db: D1Database,
  input: Readonly<{ runId: string; siteId: string }>,
): Effect.Effect<
  SavedSiteVersionPointer | null,
  AdjutantRunLifecycleStorageError
> =>
  d1Effect('adjutantRunLifecycle.siteVersion.readForRun', () =>
    db
      .prepare(
        `SELECT id,
                build_status
           FROM site_versions
          WHERE site_id = ?
            AND created_by_run_id = ?
          LIMIT 1`,
      )
      .bind(input.siteId, input.runId)
      .first<SavedSiteVersionPointer>(),
  )

const lifecycleEventExists = (
  db: D1Database,
  table: 'adjutant_assignment_events' | 'site_events',
  input: Readonly<{
    assignmentId: string
    eventType: string
    runId: string
    siteId: string | null
  }>,
): Effect.Effect<boolean, AdjutantRunLifecycleStorageError> =>
  table === 'adjutant_assignment_events'
    ? d1Effect('adjutantRunLifecycle.assignmentEvent.exists', () =>
        db
          .prepare(
            `SELECT id
               FROM adjutant_assignment_events
              WHERE assignment_id = ?
                AND run_id = ?
                AND event_type = ?
              LIMIT 1`,
          )
          .bind(input.assignmentId, input.runId, input.eventType)
          .first<Readonly<{ id: string }>>(),
      ).pipe(Effect.map(row => row !== null))
    : input.siteId === null
      ? Effect.succeed(true)
      : d1Effect('adjutantRunLifecycle.siteEvent.exists', () =>
          db
            .prepare(
              `SELECT id
                 FROM site_events
                WHERE site_id = ?
                  AND actor_run_id = ?
                  AND type = ?
                LIMIT 1`,
            )
            .bind(input.siteId, input.runId, input.eventType)
            .first<Readonly<{ id: string }>>(),
        ).pipe(Effect.map(row => row !== null))

const updateAssignmentStatus = (
  db: D1Database,
  assignment: AdjutantRunAssignmentRow,
  mapping: LifecycleMapping,
  now: string,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  d1Effect('adjutantRunLifecycle.assignment.updateStatus', () =>
    db
      .prepare(
        `UPDATE adjutant_assignments
            SET status = ?,
                updated_at = ?,
                blocked_at = CASE WHEN ? = 'blocked' THEN COALESCE(blocked_at, ?) ELSE blocked_at END,
                completed_at = CASE WHEN ? = 'delivered' THEN COALESCE(completed_at, ?) ELSE completed_at END
          WHERE id = ?
            AND archived_at IS NULL`,
      )
      .bind(
        mapping.assignmentStatus,
        now,
        mapping.assignmentStatus,
        now,
        mapping.assignmentStatus,
        now,
        assignment.id,
      )
      .run(),
  ).pipe(Effect.asVoid)

const updateSoftwareOrderStatus = (
  db: D1Database,
  assignment: AdjutantRunAssignmentRow,
  mapping: LifecycleMapping,
  now: string,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  assignment.software_order_id === null
    ? Effect.void
    : d1Effect('adjutantRunLifecycle.softwareOrder.updateStatus', () =>
        db
          .prepare(
            `UPDATE software_orders
                SET current_run_id = ?,
                    status = ?,
                    agent_started_at = CASE WHEN ? = 'agent_running' THEN COALESCE(agent_started_at, ?) ELSE agent_started_at END,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(
            assignment.current_run_id,
            mapping.orderStatus,
            mapping.orderStatus,
            now,
            now,
            assignment.software_order_id,
          )
          .run(),
      ).pipe(Effect.asVoid)

const recordAssignmentLifecycleEvent = (
  db: D1Database,
  input: Readonly<{
    actorUserId: string
    assignment: AdjutantRunAssignmentRow
    emailMessageId?: string | null | undefined
    eventType: string
    payload: string
    runId: string
    summary: string
    now: string
  }>,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  d1Effect('adjutantRunLifecycle.assignmentEvent.insert', () =>
    db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            email_message_id,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('adjutant_assignment_event'),
        input.assignment.id,
        input.assignment.software_order_id,
        input.assignment.site_id,
        input.assignment.goal_id,
        input.runId,
        input.eventType,
        input.assignment.visibility,
        input.summary,
        input.actorUserId,
        input.payload,
        input.emailMessageId ?? null,
        input.now,
      )
      .run(),
  ).pipe(Effect.asVoid)

const recordSiteLifecycleEvent = (
  db: D1Database,
  input: Readonly<{
    actorUserId: string
    assignment: AdjutantRunAssignmentRow
    emailMessageId?: string | null | undefined
    eventType: string
    payload: string
    runId: string
    summary: string
    now: string
  }>,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  input.assignment.site_id === null
    ? Effect.void
    : d1Effect('adjutantRunLifecycle.siteEvent.insert', () =>
        db
          .prepare(
            `INSERT INTO site_events
               (id,
                site_id,
                version_id,
                deployment_id,
                type,
                summary,
                actor_user_id,
                actor_run_id,
                payload_json,
                email_message_id,
                created_at)
             VALUES (?, ?, NULL, NULL, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            compactRandomId('site_event'),
            input.assignment.site_id,
            input.eventType,
            input.summary,
            input.actorUserId,
            input.runId,
            input.payload,
            input.emailMessageId ?? null,
            input.now,
          )
          .run(),
      ).pipe(Effect.asVoid)

const recordCustomerNotificationEvents = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  stage: LifecycleStage,
  outcome: CustomerNotificationOutcome,
  now: string,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    const notificationStage = notificationStageForLifecycleStage(stage)
    const lifecycleKind = notificationKindForLifecycleStage(stage)
    const eventType = notificationEventType(notificationStage)
    const payload = yield* payloadJson({
      assignmentId: assignment.id,
      errorMessage: outcome.errorMessage ?? null,
      errorName: outcome.errorName ?? null,
      emailMessageId: outcome.emailMessageId,
      emailStatus: outcome.emailStatus,
      lifecycleKind,
      providerMessageId: outcome.providerMessageId ?? null,
      runId: input.runId,
      skipReason: outcome.skipReason ?? null,
      softwareOrderId: assignment.software_order_id,
      stage: notificationStage,
    })
    const summary =
      outcome.emailStatus === 'accepted'
        ? `Autopilot customer ${notificationStage} email notification was accepted.`
        : outcome.emailStatus === 'failed'
          ? `Autopilot customer ${notificationStage} email notification failed.`
          : `Autopilot customer ${notificationStage} email notification is needed.`

    const assignmentEventExists = yield* lifecycleEventExists(
      db,
      'adjutant_assignment_events',
      {
        assignmentId: assignment.id,
        eventType,
        runId: input.runId,
        siteId: assignment.site_id,
      },
    )

    if (!assignmentEventExists) {
      yield* recordAssignmentLifecycleEvent(db, {
        actorUserId: input.actorUserId,
        assignment,
        emailMessageId: outcome.emailMessageId,
        eventType,
        now,
        payload,
        runId: input.runId,
        summary,
      })
    }

    const siteEventExists = yield* lifecycleEventExists(db, 'site_events', {
      assignmentId: assignment.id,
      eventType,
      runId: input.runId,
      siteId: assignment.site_id,
    })

    if (!siteEventExists) {
      yield* recordSiteLifecycleEvent(db, {
        actorUserId: input.actorUserId,
        assignment,
        emailMessageId: outcome.emailMessageId,
        eventType,
        now,
        payload,
        runId: input.runId,
        summary,
      })
    }
  })

const saveReceiptVersion = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  receipt: AdjutantSiteArtifactReceipt,
): Effect.Effect<SavedSiteVersionPointer, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    if (assignment.site_id === null || receipt.siteId !== assignment.site_id) {
      return yield* new AdjutantRunLifecycleStorageError({
        error: 'Receipt Site ID does not match the Autopilot assignment Site.',
        operation: 'adjutantRunLifecycle.siteVersion.receipt',
      })
    }

    const existingVersion = yield* readSiteVersionForRun(db, {
      runId: input.runId,
      siteId: receipt.siteId,
    })

    if (existingVersion !== null) {
      return existingVersion
    }

    const sites = yield* AutopilotSitesService

    const version = yield* sites.saveVersion({
      actorRunId: input.runId,
      actorUserId: input.actorUserId,
      buildStatus: receipt.buildStatus,
      siteId: receipt.siteId,
      sourceKind: 'autopilot_generated',
      staticAssetsManifest: receipt.staticAssetsManifest,
      ...(receipt.buildCommand === undefined
        ? {}
        : { buildCommand: receipt.buildCommand }),
      ...(receipt.buildLogText === undefined
        ? {}
        : { buildLogText: receipt.buildLogText }),
      ...(receipt.d1BindingName === undefined
        ? {}
        : { d1BindingName: receipt.d1BindingName }),
      ...(receipt.metadata === undefined ? {} : { metadata: receipt.metadata }),
      ...(receipt.r2BindingName === undefined
        ? {}
        : { r2BindingName: receipt.r2BindingName }),
      ...(receipt.sourceArchiveText === undefined
        ? {}
        : { sourceArchiveText: receipt.sourceArchiveText }),
      ...(receipt.sourceCommitSha === undefined
        ? {}
        : { sourceCommitSha: receipt.sourceCommitSha }),
      ...(receipt.workerModuleR2Key === undefined
        ? {}
        : { workerModuleR2Key: receipt.workerModuleR2Key }),
      ...(receipt.workerModuleText === undefined
        ? {}
        : { workerModuleText: receipt.workerModuleText }),
    })

    return {
      build_status: version.buildStatus,
      id: version.id,
    }
  }).pipe(
    Effect.provide(
      AutopilotSitesService.layer({
        OPENAGENTS_DB: db,
        ...(input.artifacts === undefined
          ? {}
          : { ARTIFACTS: input.artifacts }),
      }),
    ),
    Effect.mapError(
      error =>
        new AdjutantRunLifecycleStorageError({
          error,
          operation: 'adjutantRunLifecycle.siteVersion.save',
        }),
    ),
  )

const markSiteNeedsReviewForAdjustment = (
  db: D1Database,
  input: Readonly<{ siteId: string | null; updatedAt: string }>,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  input.siteId === null
    ? Effect.void
    : d1Effect('adjutantRunLifecycle.site.adjustmentNeedsReview', () =>
        db
          .prepare(
            `UPDATE site_projects
                SET status = 'needs_review',
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(input.updatedAt, input.siteId)
          .run(),
      ).pipe(Effect.asVoid)

const readActivationProject = (
  db: D1Database,
  siteId: string,
): Effect.Effect<
  SiteRevisionActivationProjectRow | null,
  AdjutantRunLifecycleStorageError
> =>
  d1Effect('adjutantRunLifecycle.siteActivation.project.read', () =>
    db
      .prepare(
        `SELECT id,
                slug,
                active_version_id,
                active_deployment_id
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<SiteRevisionActivationProjectRow>(),
  )

const readDeploymentForVersion = (
  db: D1Database,
  input: Readonly<{ siteId: string; versionId: string }>,
): Effect.Effect<
  SiteRevisionDeploymentRow | null,
  AdjutantRunLifecycleStorageError
> =>
  d1Effect('adjutantRunLifecycle.siteActivation.deployment.readForVersion', () =>
    db
      .prepare(
        `SELECT id
           FROM site_deployments
          WHERE site_id = ?
            AND version_id = ?
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(input.siteId, input.versionId)
      .first<SiteRevisionDeploymentRow>(),
  )

const readOpenFeedbackBodiesForAssignment = (
  db: D1Database,
  assignmentId: string,
): Effect.Effect<
  ReadonlyArray<string>,
  AdjutantRunLifecycleStorageError
> =>
  d1Effect('adjutantRunLifecycle.siteActivation.feedback.requirements', () =>
    db
      .prepare(
        `SELECT site_revision_feedback.body
           FROM site_revision_feedback
           JOIN adjutant_adjustment_requests
             ON adjutant_adjustment_requests.id = site_revision_feedback.adjutant_adjustment_id
          WHERE adjutant_adjustment_requests.assignment_id = ?
            AND adjutant_adjustment_requests.archived_at IS NULL
            AND site_revision_feedback.archived_at IS NULL
            AND site_revision_feedback.status IN ('submitted', 'queued', 'running')
          ORDER BY site_revision_feedback.created_at ASC
          LIMIT 10`,
      )
      .bind(assignmentId)
      .all<SiteRevisionFeedbackRequirementRow>(),
  ).pipe(Effect.map(result => result.results.map(row => row.body)))

const receiptProjectFiles = (
  receipt: AdjutantSiteArtifactReceipt,
): ReadonlyArray<typeof SiteCompatibilityProjectFile.Type> => [
  ...(receipt.sourceArchiveText === undefined
    ? []
    : [{ path: 'source-archive.html', text: receipt.sourceArchiveText }]),
  ...(receipt.workerModuleText === undefined
    ? []
    : [{ path: 'worker.mjs', text: receipt.workerModuleText }]),
]

const assertVisualAssetRequirementsForReceipt = (
  db: D1Database,
  assignment: AdjutantRunAssignmentRow,
  receipt: AdjutantSiteArtifactReceipt,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    const feedbackBodies = yield* readOpenFeedbackBodiesForAssignment(
      db,
      assignment.id,
    )
    if (
      typeof receipt.metadata?.visualAssetWaiverReason === 'string' &&
      receipt.metadata.visualAssetWaiverReason.trim() !== ''
    ) {
      return
    }

    const requirements = inferSiteVisualAssetRequirements([
      { source: 'customer_request', text: assignment.objective },
      ...feedbackBodies.map(body => ({
        source: 'customer_request' as const,
        text: body,
      })),
    ])

    if (requirements.length === 0) {
      return
    }

    const files = new Map(
      receiptProjectFiles(receipt).map(file => [file.path, file.text] as const),
    )
    const findings = collectSiteVisualAssetFindings(
      files,
      requirements,
      Object.keys(receipt.staticAssetsManifest.assets),
    )
    const missing = findings.find(
      finding => finding.code === 'missing_required_visual_asset',
    )

    if (missing !== undefined) {
      return yield* new AdjutantRunLifecycleStorageError({
        error: missing.message,
        operation: 'adjutantRunLifecycle.siteActivation.visualAssets',
      })
    }
  })

const recordSiteActivationEvent = (
  db: D1Database,
  input: Readonly<{
    actorUserId: string
    deploymentId: string | null
    eventType: string
    payload: string
    runId: string
    siteId: string
    summary: string
    versionId: string | null
    now: string
  }>,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  d1Effect('adjutantRunLifecycle.siteActivation.event.insert', () =>
    db
      .prepare(
        `INSERT INTO site_events
           (id,
            site_id,
            version_id,
            deployment_id,
            type,
            summary,
            actor_user_id,
            actor_run_id,
            payload_json,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('site_event'),
        input.siteId,
        input.versionId,
        input.deploymentId,
        input.eventType,
        input.summary,
        input.actorUserId,
        input.runId,
        input.payload,
        input.now,
      )
      .run(),
  ).pipe(Effect.asVoid)

const markFeedbackAddressedForVersion = (
  db: D1Database,
  input: Readonly<{
    assignmentId: string
    runId: string
    updatedAt: string
    versionId: string
  }>,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  d1Effect('adjutantRunLifecycle.siteActivation.feedback.address', () =>
    db
      .prepare(
        `UPDATE site_revision_feedback
            SET status = 'addressed',
                updated_at = ?
          WHERE adjutant_adjustment_id = (
            SELECT id
              FROM adjutant_adjustment_requests
             WHERE assignment_id = ?
               AND resulting_version_id = ?
               AND archived_at IS NULL
               AND (continuation_run_id = ? OR source_run_id = ?)
             ORDER BY updated_at DESC
             LIMIT 1
          )
            AND status IN ('submitted', 'queued', 'running')`,
      )
      .bind(
        input.updatedAt,
        input.assignmentId,
        input.versionId,
        input.runId,
        input.runId,
      )
      .run(),
  ).pipe(Effect.asVoid)

const activateSavedRevisionForStableSlug = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  receipt: AdjutantSiteArtifactReceipt,
  version: SavedSiteVersionPointer,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    if (
      assignment.site_id === null ||
      version.build_status !== 'saved' ||
      receipt.buildStatus !== 'saved'
    ) {
      return
    }

    const project = yield* readActivationProject(db, assignment.site_id)

    if (project === null) {
      return yield* new AdjutantRunLifecycleStorageError({
        error: `Site project ${assignment.site_id} was not found.`,
        operation: 'adjutantRunLifecycle.siteActivation.project',
      })
    }

    const existingDeployment = yield* readDeploymentForVersion(db, {
      siteId: assignment.site_id,
      versionId: version.id,
    })
    const now = currentIsoTimestamp()
    const deploymentId =
      existingDeployment?.id ?? compactRandomId('site_deployment')
    const previousDeploymentId =
      project.active_deployment_id === deploymentId
        ? null
        : project.active_deployment_id
    const runtimeKind =
      receipt.workerModuleR2Key !== undefined ||
      receipt.workerModuleText !== undefined
        ? 'workers_for_platforms'
        : 'omega_static_r2'
    const url = `https://sites.openagents.com/${project.slug}`

    yield* d1Effect('adjutantRunLifecycle.siteActivation.version.metadata', () =>
      db
        .prepare(
          `UPDATE site_versions
              SET metadata_json = json_set(
                    COALESCE(NULLIF(metadata_json, ''), '{}'),
                    '$.customerReviewState',
                    'customer_review_ready',
                    '$.runtimeActivationPolicy',
                    'latest_successful_revision',
                    '$.customerAccepted',
                    json('false')
                  )
            WHERE id = ?
              AND site_id = ?`,
        )
        .bind(version.id, assignment.site_id)
        .run(),
    )

    if (previousDeploymentId !== null) {
      yield* d1Effect('adjutantRunLifecycle.siteActivation.previous.rollback', () =>
        db
          .prepare(
            `UPDATE site_deployments
                SET status = 'rolled_back',
                    rolled_back_at = ?,
                    updated_at = ?
              WHERE id = ?
                AND site_id = ?
                AND status = 'active'`,
          )
          .bind(now, now, previousDeploymentId, assignment.site_id)
          .run(),
      )
      const supersededPayload = yield* payloadJson({
        nextDeploymentId: deploymentId,
        nextVersionId: version.id,
        previousDeploymentId,
        policy: 'latest_successful_revision',
      })
      const supersededEventExists = yield* lifecycleEventExists(db, 'site_events', {
        assignmentId: assignment.id,
        eventType: 'site_deployment.superseded',
        runId: input.runId,
        siteId: assignment.site_id,
      })

      if (!supersededEventExists) {
        yield* recordSiteActivationEvent(db, {
          actorUserId: input.actorUserId,
          deploymentId: previousDeploymentId,
          eventType: 'site_deployment.superseded',
          now,
          payload: supersededPayload,
          runId: input.runId,
          siteId: assignment.site_id,
          summary: `Superseded deployment ${previousDeploymentId} with latest revision ${version.id}.`,
          versionId: project.active_version_id,
        })
      }
    }

    if (existingDeployment === null) {
      yield* d1Effect('adjutantRunLifecycle.siteActivation.deployment.insert', () =>
        db
          .prepare(
            `INSERT INTO site_deployments
               (id,
                site_id,
                version_id,
                slug,
                url,
                runtime_kind,
                runtime_script_name,
                dispatch_namespace,
                status,
                deployed_by_user_id,
                external_deployment_id,
                started_at,
                activated_at,
                failed_at,
                disabled_at,
                rolled_back_at,
                created_at,
                updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, NULL, ?, ?, NULL, NULL, NULL, ?, ?)`,
          )
          .bind(
            deploymentId,
            assignment.site_id,
            version.id,
            project.slug,
            url,
            runtimeKind,
            runtimeKind === 'workers_for_platforms'
              ? `site-${assignment.site_id}-${version.id}`
              : null,
            runtimeKind === 'workers_for_platforms'
              ? 'openagents_sites'
              : null,
            input.actorUserId,
            now,
            now,
            now,
            now,
          )
          .run(),
      )
    } else {
      yield* d1Effect('adjutantRunLifecycle.siteActivation.deployment.activate', () =>
        db
          .prepare(
            `UPDATE site_deployments
                SET status = 'active',
                    activated_at = COALESCE(activated_at, ?),
                    updated_at = ?
              WHERE id = ?
                AND site_id = ?`,
          )
          .bind(now, now, deploymentId, assignment.site_id)
          .run(),
      )
    }

    yield* d1Effect('adjutantRunLifecycle.siteActivation.project.update', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET status = 'needs_review',
                  active_version_id = ?,
                  active_deployment_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(version.id, deploymentId, now, assignment.site_id)
        .run(),
    )

    const activationPayload = yield* payloadJson({
      acceptedByCustomer: false,
      deploymentId,
      policy: 'latest_successful_revision',
      previousDeploymentId,
      reviewState: 'customer_review_ready',
      url,
      versionId: version.id,
    })
    const activationEventExists = yield* lifecycleEventExists(db, 'site_events', {
      assignmentId: assignment.id,
      eventType: 'site_deployment.activated',
      runId: input.runId,
      siteId: assignment.site_id,
    })

    if (!activationEventExists) {
      yield* recordSiteActivationEvent(db, {
        actorUserId: input.actorUserId,
        deploymentId,
        eventType: 'site_deployment.activated',
        now,
        payload: activationPayload,
        runId: input.runId,
        siteId: assignment.site_id,
        summary: `Activated latest Site revision ${version.id} at ${url}.`,
        versionId: version.id,
      })
    }
    yield* markFeedbackAddressedForVersion(db, {
      assignmentId: assignment.id,
      runId: input.runId,
      updatedAt: now,
      versionId: version.id,
    })
  })

const updateAdjustmentForSavedVersion = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  version: SavedSiteVersionPointer,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> => {
  const now = currentIsoTimestamp()
  const adjustmentStatus =
    version.build_status === 'saved' ? 'review_needed' : 'failed'

  return Effect.gen(function* () {
    yield* d1Effect('adjutantRunLifecycle.adjustment.updateForVersion', () =>
      db
        .prepare(
          `UPDATE adjutant_adjustment_requests
              SET status = ?,
                  resulting_version_id = ?,
                  updated_at = ?,
                  completed_at = CASE WHEN ? = 'failed' THEN COALESCE(completed_at, ?) ELSE completed_at END
            WHERE id = (
              SELECT id
                FROM adjutant_adjustment_requests
               WHERE assignment_id = ?
                 AND archived_at IS NULL
                 AND status IN ('requested', 'queued', 'running')
                 AND (continuation_run_id = ? OR source_run_id = ?)
               ORDER BY created_at DESC
               LIMIT 1
            )`,
        )
        .bind(
          adjustmentStatus,
          version.id,
          now,
          adjustmentStatus,
          now,
          assignment.id,
          input.runId,
          input.runId,
        )
        .run(),
    )

    if (version.build_status === 'saved') {
      yield* markSiteNeedsReviewForAdjustment(db, {
        siteId: assignment.site_id,
        updatedAt: now,
      })
    }
  })
}

const storageArtifactQuantity = (
  receipt: AdjutantSiteArtifactReceipt,
): number => {
  const staticAssetCount = Object.keys(
    receipt.staticAssetsManifest.assets,
  ).length
  const storedArtifactCount = [
    receipt.sourceArchiveText,
    receipt.buildLogText,
    receipt.workerModuleText,
    receipt.workerModuleR2Key,
  ].filter(value => value !== undefined && value !== '').length

  return Math.max(1, staticAssetCount + storedArtifactCount + 1)
}

const recordUsageReceipt = (
  db: D1Database,
  input: RecordAdjutantUsageReceiptInput,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  recordAdjutantUsageReceipt(db, input).pipe(
    Effect.asVoid,
    Effect.mapError(
      error =>
        new AdjutantRunLifecycleStorageError({
          error,
          operation: 'adjutantRunLifecycle.usageReceipt.record',
        }),
    ),
  )

const recordArtifactUsageReceipts = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
  assignment: AdjutantRunAssignmentRow,
  receipt: AdjutantSiteArtifactReceipt,
  version: SavedSiteVersionPointer,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    const buildStatus =
      receipt.buildStatus === 'saved' ? 'saved' : 'build_failed'
    const staticAssetCount = Object.keys(
      receipt.staticAssetsManifest.assets,
    ).length
    const artifactQuantity = storageArtifactQuantity(receipt)

    yield* recordUsageReceipt(db, {
      assignmentId: assignment.id,
      billingMode: 'public_beta_free',
      category: 'build',
      idempotencyKey: [
        'adjutant_usage',
        assignment.id,
        input.runId,
        version.id,
        'build',
      ].join(':'),
      publicDetails: {
        billingNote: 'Public beta Site builds are free.',
        buildStatus,
      },
      quantity: 1,
      runId: input.runId,
      siteId: assignment.site_id,
      softwareOrderId: assignment.software_order_id,
      summary:
        receipt.buildStatus === 'saved'
          ? 'Autopilot saved a generated Site build for review.'
          : 'Autopilot recorded a failed generated Site build.',
      teamDetails: {
        buildCommand: receipt.buildCommand ?? null,
        buildStatus,
        sourceCommitSha: receipt.sourceCommitSha ?? null,
        versionId: version.id,
      },
      unit: 'build',
      visibility: assignment.visibility,
    })

    yield* recordUsageReceipt(db, {
      assignmentId: assignment.id,
      billingMode: 'public_beta_free',
      category: 'storage',
      idempotencyKey: [
        'adjutant_usage',
        assignment.id,
        input.runId,
        version.id,
        'storage',
      ].join(':'),
      publicDetails: {
        artifactCount: artifactQuantity,
        billingNote: 'Public beta Site artifact storage is free.',
      },
      quantity: artifactQuantity,
      runId: input.runId,
      siteId: assignment.site_id,
      softwareOrderId: assignment.software_order_id,
      summary: 'Autopilot stored Site artifacts for review.',
      teamDetails: {
        artifactCount: artifactQuantity,
        hasD1Binding: receipt.d1BindingName !== undefined,
        hasR2Binding: receipt.r2BindingName !== undefined,
        staticAssetCount,
        versionId: version.id,
      },
      unit: 'artifact',
      visibility: assignment.visibility,
    })
  })

export const applyAdjutantRunLifecycleEvents = (
  db: D1Database,
  input: AdjutantRunLifecycleInput,
): Effect.Effect<void, AdjutantRunLifecycleStorageError> =>
  Effect.gen(function* () {
    const stage = lifecycleStage(input)
    const receipt = yield* firstReceiptFromOmniEvents(input.events).pipe(
      Effect.mapError(
        error =>
          new AdjutantRunLifecycleStorageError({
            error,
            operation: 'adjutantRunLifecycle.receipt.decode',
          }),
      ),
    )

    if (stage === undefined && receipt === undefined) {
      return
    }

    const assignment = yield* readAssignmentByRunId(db, input.runId)

    if (assignment === null) {
      return
    }

    if (stage !== undefined) {
      const mapping = lifecycleMapping(stage)
      const now = currentIsoTimestamp()
      const payload = yield* payloadJson(
        lifecyclePayload(input, mapping, assignment),
      )

      yield* updateAssignmentStatus(db, assignment, mapping, now)
      yield* updateSoftwareOrderStatus(db, assignment, mapping, now)

      const assignmentEventExists = yield* lifecycleEventExists(
        db,
        'adjutant_assignment_events',
        {
          assignmentId: assignment.id,
          eventType: mapping.eventType,
          runId: input.runId,
          siteId: assignment.site_id,
        },
      )

      if (!assignmentEventExists) {
        yield* recordAssignmentLifecycleEvent(db, {
          actorUserId: input.actorUserId,
          assignment,
          eventType: mapping.eventType,
          now,
          payload,
          runId: input.runId,
          summary: mapping.summary,
        })
      }

      const siteEventExists = yield* lifecycleEventExists(db, 'site_events', {
        assignmentId: assignment.id,
        eventType: mapping.eventType,
        runId: input.runId,
        siteId: assignment.site_id,
      })

      if (!siteEventExists) {
        yield* recordSiteLifecycleEvent(db, {
          actorUserId: input.actorUserId,
          assignment,
          eventType: mapping.eventType,
          now,
          payload,
          runId: input.runId,
          summary: mapping.summary,
        })
      }

      const outcome = yield* sendCustomerNotification(db, input, assignment, stage)

      yield* recordCustomerNotificationEvents(
        db,
        input,
        assignment,
        stage,
        outcome,
        now,
      )
    }

    if (receipt !== undefined) {
      const version = yield* saveReceiptVersion(db, input, assignment, receipt)

      yield* assertVisualAssetRequirementsForReceipt(db, assignment, receipt)
      yield* updateAdjustmentForSavedVersion(db, input, assignment, version)
      yield* activateSavedRevisionForStableSlug(
        db,
        input,
        assignment,
        receipt,
        version,
      )
      yield* recordArtifactUsageReceipts(
        db,
        input,
        assignment,
        receipt,
        version,
      )
    }
  })
