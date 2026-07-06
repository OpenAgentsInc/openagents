// KS-8.11 (#8322): CRM/email entry points construct the dual-write seam
// (plain D1 drop-in when KHALA_SYNC_DB / the flags are absent).
import {
  identityDbForEnv,
  readIdentityUserProfiles,
  type IdentityDb,
  type IdentityDbEnv,
} from './identity-db'
import {
  type CrmEmailDatabase,
  crmEmailAuthorityDb,
  makeCrmEmailDatabaseForEnv,
} from './crm-email-domain-store'
import { Effect, Match as M, Schema as S } from 'effect'

import type { ResendEmailConfig } from './config'
import {
  OrderSitesTransactionalEmailInput,
  buildOrderSitesTransactionalEmailIdempotencyKey,
  renderOrderSitesTransactionalEmail,
  sendOrderSitesTransactionalEmailWithLedger,
} from './email'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { readJsonObject } from './json-boundary'
import {
  type OperatorEmailInspectionError,
  OperatorEmailInspectionScope,
  inspectOperatorEmailDelivery,
} from './operator-email-inspection'
// KS-8.12 (#8323): sites writes ride the dual-write mirror seam — the
// mirroring database is a passthrough for non-scoped statements and
// degrades to the raw D1 handle when no KHALA_SYNC_DB binding exists.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  makeSupervisionLongtailMirrorForEnv,
  type SupervisionLongtailMirror,
} from './supervision-longtail-domain-store'

type OperatorEmailInspectionEnv = IdentityDbEnv &
  Readonly<{
    OPENAGENTS_DB: D1Database
  }>
type HttpResponse = globalThis.Response

type OperatorEmailInspectionSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorEmailInspectionRouteDependencies<
  Session extends OperatorEmailInspectionSession,
  Bindings extends OperatorEmailInspectionEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  getAppOrigin?: ((env: Bindings) => string) | undefined
  getResendEmailConfig?:
    | ((env: Bindings) => ResendEmailConfig | undefined)
    | undefined
  emailFetcher?: typeof fetch | undefined
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

class OperatorEmailInspectionUnauthorized extends S.TaggedErrorClass<OperatorEmailInspectionUnauthorized>()(
  'OperatorEmailInspectionUnauthorized',
  {},
) {}

class OperatorEmailInspectionForbidden extends S.TaggedErrorClass<OperatorEmailInspectionForbidden>()(
  'OperatorEmailInspectionForbidden',
  {},
) {}

class OperatorEmailInspectionSessionError extends S.TaggedErrorClass<OperatorEmailInspectionSessionError>()(
  'OperatorEmailInspectionSessionError',
  {
    error: S.Defect,
  },
) {}

class OperatorEmailInspectionBadRequest extends S.TaggedErrorClass<OperatorEmailInspectionBadRequest>()(
  'OperatorEmailInspectionBadRequest',
  {
    reason: S.String,
  },
) {}

type OperatorEmailInspectionRouteError =
  | OperatorEmailInspectionBadRequest
  | OperatorEmailInspectionError
  | OperatorEmailInspectionForbidden
  | OperatorEmailInspectionSessionError
  | OperatorEmailInspectionUnauthorized

const routeErrorResponse = (
  error: OperatorEmailInspectionRouteError,
): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      OperatorEmailInspectionForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      OperatorEmailInspectionBadRequest: ({ reason }) =>
        noStoreJsonResponse({ error: 'bad_request', reason }, { status: 400 }),
      OperatorEmailInspectionInvalidScope: ({ reason }) =>
        noStoreJsonResponse(
          { error: 'email_inspection_invalid_scope', reason },
          { status: 400 },
        ),
      OperatorEmailInspectionSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      OperatorEmailInspectionStorageError: () =>
        noStoreJsonResponse(
          { error: 'email_inspection_storage_error' },
          { status: 500 },
        ),
      OperatorEmailInspectionUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const requireAdminSession = <
  Session extends OperatorEmailInspectionSession,
  Bindings extends OperatorEmailInspectionEnv,
>(
  dependencies: OperatorEmailInspectionRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorEmailInspectionSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken === true) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorEmailInspectionSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorEmailInspectionUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorEmailInspectionForbidden({})
    }

    return session
  })

const scopeFromUrl = (url: URL) => {
  const siteId = url.searchParams.get('siteId') ?? undefined
  const softwareOrderId = url.searchParams.get('softwareOrderId') ?? undefined
  const parsed = S.decodeUnknownSync(OperatorEmailInspectionScope)({
    ...(siteId === undefined ? {} : { siteId }),
    ...(softwareOrderId === undefined ? {} : { softwareOrderId }),
  })

  return parsed
}

const ReviewReadySmokeRequest = S.Struct({
  artifactId: S.optionalKey(S.String),
  dryRun: S.optionalKey(S.Boolean),
  siteId: S.optionalKey(S.String),
  softwareOrderId: S.optionalKey(S.String),
})

type ReviewReadySmokeRequest = typeof ReviewReadySmokeRequest.Type

type ReviewReadySmokeTargetRow = Readonly<{
  active_deployment_id: string | null
  active_version_id: string | null
  deployment_url: string | null
  display_name: string | null
  primary_email: string | null
  site_id: string
  site_title: string
  software_order_id: string | null
  target_user_id: string | null
  version_id: string | null
}>

type ReviewReadySmokeAssignmentRow = Readonly<{
  current_run_id: string | null
  goal_id: string | null
  id: string
  software_order_id: string | null
  visibility: 'private' | 'team' | 'public'
}>

type ReviewReadyArtifactSmokeTargetRow = Readonly<{
  artifact_id: string
  artifact_title: string
  artifact_url: string | null
  assignment_current_run_id: string | null
  assignment_goal_id: string | null
  assignment_id: string
  assignment_visibility: 'private' | 'team' | 'public'
  display_name: string | null
  kind: string
  primary_email: string | null
  software_order_id: string
  target_user_id: string | null
}>

const decodeReviewReadySmokeRequest = (
  request: Request,
): Effect.Effect<ReviewReadySmokeRequest, OperatorEmailInspectionBadRequest> =>
  Effect.tryPromise({
    catch: error =>
      new OperatorEmailInspectionBadRequest({
        reason: error instanceof Error ? error.message : 'invalid json',
      }),
    try: () => readJsonObject(request),
  }).pipe(
    Effect.flatMap(payload =>
      S.decodeUnknownEffect(ReviewReadySmokeRequest)(payload).pipe(
        Effect.mapError(
          error =>
            new OperatorEmailInspectionBadRequest({ reason: String(error) }),
        ),
      ),
    ),
    Effect.flatMap(payload =>
      payload.artifactId === undefined &&
      payload.siteId === undefined &&
      payload.softwareOrderId === undefined
        ? Effect.fail(
            new OperatorEmailInspectionBadRequest({
              reason: 'artifactId, siteId, or softwareOrderId is required',
            }),
          )
        : Effect.succeed(payload),
    ),
  )

// CFG-4 Domain 2 (#8519): the customer `users` row reads from the Postgres
// identity handle; the old `LEFT JOIN users ... deleted_at IS NULL`
// semantics hold (a missing/deleted user leaves the target fields null).
const enrichSmokeTargetUser = async <
  Row extends Readonly<{ order_user_id: string | null }>,
>(
  identityDb: IdentityDb,
  row: Row,
): Promise<
  Omit<Row, 'order_user_id'> &
    Readonly<{
      target_user_id: string | null
      display_name: string | null
      primary_email: string | null
    }>
> => {
  const { order_user_id, ...rest } = row
  const profile =
    order_user_id === null
      ? undefined
      : (await readIdentityUserProfiles(identityDb, [order_user_id])).get(
          order_user_id,
        )
  const live =
    profile === undefined || profile.deletedAt !== null ? undefined : profile
  return {
    ...rest,
    display_name: live?.displayName ?? null,
    primary_email: live?.primaryEmail ?? null,
    target_user_id: live?.userId ?? null,
  }
}

const readReviewReadyArtifactSmokeTarget = (
  db: CrmEmailDatabase,
  identityDb: IdentityDb,
  artifactId: string,
): Effect.Effect<
  ReviewReadyArtifactSmokeTargetRow | null,
  OperatorEmailInspectionRouteError
> =>
  Effect.tryPromise({
    catch: error =>
      new OperatorEmailInspectionBadRequest({
        reason:
          error instanceof Error ? error.message : 'artifact target read failed',
      }),
    try: () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `SELECT order_fulfillment_artifacts.id AS artifact_id,
                  order_fulfillment_artifacts.title AS artifact_title,
                  order_fulfillment_artifacts.url AS artifact_url,
                  order_fulfillment_artifacts.kind AS kind,
                  software_orders.id AS software_order_id,
                  software_orders.user_id AS order_user_id,
                  adjutant_assignments.id AS assignment_id,
                  adjutant_assignments.goal_id AS assignment_goal_id,
                  adjutant_assignments.current_run_id AS assignment_current_run_id,
                  adjutant_assignments.visibility AS assignment_visibility
             FROM order_fulfillment_artifacts
             JOIN software_orders
               ON software_orders.id = order_fulfillment_artifacts.software_order_id
              AND software_orders.archived_at IS NULL
             JOIN adjutant_assignments
               ON adjutant_assignments.id = (
                    SELECT id
                      FROM adjutant_assignments AS assignment
                     WHERE assignment.software_order_id = software_orders.id
                       AND assignment.archived_at IS NULL
                     ORDER BY assignment.updated_at DESC
                     LIMIT 1
                  )
            WHERE order_fulfillment_artifacts.id = ?
              AND order_fulfillment_artifacts.archived_at IS NULL
              AND order_fulfillment_artifacts.visibility = 'public'
              AND order_fulfillment_artifacts.status = 'customer_review_ready'
            LIMIT 1`,
        )
        .bind(artifactId)
        .first<
          Omit<ReviewReadyArtifactSmokeTargetRow, 'target_user_id' | 'display_name' | 'primary_email'> &
            Readonly<{ order_user_id: string | null }>
        >()
        .then(row =>
          row === null
            ? null
            : enrichSmokeTargetUser(identityDb, row),
        ),
  })

const readReviewReadySmokeTarget = (
  db: CrmEmailDatabase,
  identityDb: IdentityDb,
  input: ReviewReadySmokeRequest,
): Effect.Effect<
  ReviewReadySmokeTargetRow | null,
  OperatorEmailInspectionRouteError
> =>
  Effect.tryPromise({
    catch: error =>
      new OperatorEmailInspectionBadRequest({
        reason: error instanceof Error ? error.message : 'target read failed',
      }),
    try: () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `SELECT site_projects.id AS site_id,
                  site_projects.title AS site_title,
                  site_projects.software_order_id AS software_order_id,
                  site_projects.active_version_id AS active_version_id,
                  site_projects.active_deployment_id AS active_deployment_id,
                  site_versions.id AS version_id,
                  site_deployments.url AS deployment_url,
                  software_orders.user_id AS order_user_id
             FROM site_projects
             LEFT JOIN site_versions
               ON site_versions.id = site_projects.active_version_id
             LEFT JOIN site_deployments
               ON site_deployments.id = site_projects.active_deployment_id
              AND site_deployments.status = 'active'
             LEFT JOIN software_orders
               ON software_orders.id = site_projects.software_order_id
              AND software_orders.archived_at IS NULL
            WHERE site_projects.archived_at IS NULL
              AND (? IS NULL OR site_projects.id = ?)
              AND (? IS NULL OR site_projects.software_order_id = ?)
            ORDER BY site_projects.updated_at DESC
            LIMIT 1`,
        )
        .bind(
          input.siteId ?? null,
          input.siteId ?? null,
          input.softwareOrderId ?? null,
          input.softwareOrderId ?? null,
        )
        .first<
          Omit<ReviewReadySmokeTargetRow, 'target_user_id' | 'display_name' | 'primary_email'> &
            Readonly<{ order_user_id: string | null }>
        >()
        .then(row =>
          row === null
            ? null
            : enrichSmokeTargetUser(identityDb, row),
        ),
  })

const readReviewReadySmokeAssignment = (
  db: CrmEmailDatabase,
  siteId: string,
): Effect.Effect<
  ReviewReadySmokeAssignmentRow | null,
  OperatorEmailInspectionRouteError
> =>
  Effect.tryPromise({
    catch: error =>
      new OperatorEmailInspectionBadRequest({
        reason: error instanceof Error ? error.message : 'assignment read failed',
      }),
    try: () =>
      crmEmailAuthorityDb(db)
        .prepare(
          `SELECT id,
                  software_order_id,
                  goal_id,
                  current_run_id,
                  visibility
             FROM adjutant_assignments
            WHERE site_id = ?
              AND archived_at IS NULL
            ORDER BY updated_at DESC
            LIMIT 1`,
        )
        .bind(siteId)
        .first<ReviewReadySmokeAssignmentRow>(),
  })

const reviewReadySmokePayload = (input: {
  dryRun: boolean
  emailMessageId: string | null
  emailStatus: 'accepted' | 'failed' | 'skipped'
  providerMessageId: string | null
  siteId: string
  siteUrl: string | null
  skipReason: string | null
  softwareOrderId: string | null
  versionId: string | null
}) =>
  JSON.stringify({
    dryRun: input.dryRun,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: input.skipReason,
    softwareOrderId: input.softwareOrderId,
    stage: 'review_ready',
    versionId: input.versionId,
  })

const reviewReadyArtifactSmokePayload = (input: {
  artifactId: string
  artifactUrl: string | null
  dryRun: boolean
  emailMessageId: string | null
  emailStatus: 'accepted' | 'failed' | 'skipped'
  providerMessageId: string | null
  skipReason: string | null
  softwareOrderId: string
}) =>
  JSON.stringify({
    artifactId: input.artifactId,
    artifactUrl: input.artifactUrl,
    dryRun: input.dryRun,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId,
    skipReason: input.skipReason,
    softwareOrderId: input.softwareOrderId,
    stage: 'review_ready',
  })

const siteRevisionUrl = (
  siteUrl: string | null,
  versionId: string | null,
): string | null =>
  siteUrl === null || versionId === null
    ? null
    : `${siteUrl.replace(/\/+$/, '')}/versions/${encodeURIComponent(versionId)}`

const recordReviewReadySmokeEvents = (
  db: CrmEmailDatabase,
  input: Readonly<{
    actorRunId: string | null
    actorUserId: string
    assignment: ReviewReadySmokeAssignmentRow | null
    emailMessageId: string | null
    payload: string
    siteId: string
    summary: string
    versionId: string | null
  }>,
  mirror?: SupervisionLongtailMirror,
) =>
  Effect.gen(function* () {
    const now = currentIsoTimestamp()
    const assignment = input.assignment

    if (assignment !== null) {
      const eventId = compactRandomId('adjutant_assignment_event')
      yield* Effect.tryPromise({
        catch: error =>
          new OperatorEmailInspectionBadRequest({
            reason:
              error instanceof Error ? error.message : 'assignment event failed',
          }),
        try: () =>
          crmEmailAuthorityDb(db)
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
               VALUES (?, ?, ?, ?, ?, ?, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
            )
            .bind(
              eventId,
              assignment.id,
              assignment.software_order_id,
              input.siteId,
              assignment.goal_id,
              assignment.current_run_id,
              assignment.visibility,
              input.summary,
              input.actorUserId,
              input.payload,
              input.emailMessageId,
              now,
            )
            .run(),
      })
      yield* Effect.promise(
        () =>
          mirror?.mirrorRowsByKey('adjutant_assignment_events', [[eventId]]) ??
          Promise.resolve(),
      )
    }

    yield* Effect.tryPromise({
      catch: error =>
        new OperatorEmailInspectionBadRequest({
          reason: error instanceof Error ? error.message : 'site event failed',
        }),
      try: () =>
        crmEmailAuthorityDb(db)
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
             VALUES (?, ?, ?, NULL, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            compactRandomId('site_event'),
            input.siteId,
            input.versionId,
            input.summary,
            input.actorUserId,
            input.actorRunId,
            input.payload,
            input.emailMessageId,
            now,
          )
          .run(),
    })
  })

const recordReviewReadyArtifactSmokeEvent = (
  db: CrmEmailDatabase,
  input: Readonly<{
    actorUserId: string
    emailMessageId: string | null
    payload: string
    summary: string
    target: ReviewReadyArtifactSmokeTargetRow
  }>,
  mirror?: SupervisionLongtailMirror,
) => {
  const eventId = compactRandomId('adjutant_assignment_event')
  return Effect.tryPromise({
    catch: error =>
      new OperatorEmailInspectionBadRequest({
        reason:
          error instanceof Error ? error.message : 'artifact event failed',
      }),
    try: () =>
      crmEmailAuthorityDb(db)
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
           VALUES (?, ?, ?, NULL, ?, ?, 'adjutant.notification.review_ready_artifact', ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          eventId,
          input.target.assignment_id,
          input.target.software_order_id,
          input.target.assignment_goal_id,
          input.target.assignment_current_run_id,
          input.target.assignment_visibility,
          input.summary,
          input.actorUserId,
          input.payload,
          input.emailMessageId,
          currentIsoTimestamp(),
        )
        .run(),
  }).pipe(
    Effect.tap(() =>
      Effect.promise(
        () =>
          mirror?.mirrorRowsByKey('adjutant_assignment_events', [[eventId]]) ??
          Promise.resolve(),
      ),
    ),
  )
}

export const makeOperatorEmailInspectionRoutes = <
  Session extends OperatorEmailInspectionSession,
  Bindings extends OperatorEmailInspectionEnv,
>(
  dependencies: OperatorEmailInspectionRouteDependencies<Session, Bindings>,
) => ({
  routeOperatorEmailInspectionRequest: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ): Effect.Effect<HttpResponse> | undefined => {
    const url = new URL(request.url)

    if (url.pathname === '/api/operator/email-deliveries/review-ready-smoke') {
      return Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeReviewReadySmokeRequest(request)
        const db = makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) })
        const supervisionMirror = makeSupervisionLongtailMirrorForEnv(env)

        if (body.artifactId !== undefined) {
          const target = yield* readReviewReadyArtifactSmokeTarget(
            db,
            identityDbForEnv(env),
            body.artifactId,
          )

          if (target === null) {
            return noStoreJsonResponse(
              { error: 'artifact_email_target_not_found' },
              { status: 404 },
            )
          }

          if (target.target_user_id === null) {
            return noStoreJsonResponse(
              { error: 'artifact_email_target_missing_order' },
              { status: 409 },
            )
          }

          const email = target.primary_email?.trim()

          if (email === undefined || email === '') {
            return noStoreJsonResponse(
              { error: 'artifact_email_target_missing_email' },
              { status: 409 },
            )
          }

          const appOrigin =
            dependencies.getAppOrigin?.(env) ?? 'https://openagents.com'
          const artifactLabel =
            target.kind === 'pull_request'
              ? 'Review pull request'
              : target.kind === 'diff'
                ? 'Review diff'
                : 'Review artifact'
          const emailInputWithoutKey = new OrderSitesTransactionalEmailInput({
            appOrigin,
            assignmentId: target.assignment_id,
            artifactLabel,
            artifactUrl: target.artifact_url,
            customerSafeStatus: 'Ready for review',
            displayName: target.display_name ?? email,
            eventRef: target.artifact_id,
            lifecycleKind: 'review_ready',
            nextAction:
              'Open your order status page, review the latest deliverable, and send any follow-up comment.',
            orderId: target.software_order_id,
            revisionUrl: null,
            safeReason: null,
            siteTitle: target.artifact_title,
            siteUrl: null,
            sourceAuthorityRefs: [
              'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
              'github:OpenAgentsInc/autopilot-omega#152',
            ],
            targetRefs: [
              target.software_order_id,
              target.artifact_id,
              target.assignment_id,
            ],
            to: email,
          })
          const emailInput = new OrderSitesTransactionalEmailInput({
            ...emailInputWithoutKey,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                emailInputWithoutKey,
              ),
          })
          const resend = dependencies.getResendEmailConfig?.(env)

          if (body.dryRun === true) {
            const rendered =
              resend === undefined
                ? null
                : yield* renderOrderSitesTransactionalEmail(
                    resend,
                    emailInput,
                  ).pipe(
                    Effect.mapError(
                      error =>
                        new OperatorEmailInspectionBadRequest({
                          reason: error.message,
                        }),
                    ),
                  )

            return dependencies.appendRefreshedSessionCookies(
              noStoreJsonResponse({
                smoke: {
                  artifactId: target.artifact_id,
                  dryRun: true,
                  emailStatus: 'skipped',
                  idempotencyKey: emailInput.idempotencyKey,
                  rendered:
                    rendered === null
                      ? null
                      : {
                          subject: rendered.subject,
                          templateSlug: rendered.templateSlug,
                        },
                  skipReason:
                    resend === undefined ? 'email_config_missing' : 'dry_run',
                  softwareOrderId: target.software_order_id,
                },
              }),
              session,
            )
          }

          if (resend === undefined) {
            return dependencies.appendRefreshedSessionCookies(
              noStoreJsonResponse({
                smoke: {
                  artifactId: target.artifact_id,
                  dryRun: false,
                  emailStatus: 'skipped',
                  idempotencyKey: emailInput.idempotencyKey,
                  skipReason: 'email_config_missing',
                  softwareOrderId: target.software_order_id,
                },
              }),
              session,
            )
          }

          const result = yield* sendOrderSitesTransactionalEmailWithLedger(
            db,
            resend,
            emailInput,
            {
              actorUserId: session.user.userId,
              metadata: {
                artifactId: target.artifact_id,
                assignmentId: target.assignment_id,
                eventSource: 'operator_review_ready_artifact_email_smoke',
                lifecycleKind: 'review_ready',
                softwareOrderId: target.software_order_id,
              },
              sourceAuthorityRef:
                'system.review_ready_artifact_notification_reconciler.v1',
              targetUserId: target.target_user_id,
            },
            dependencies.emailFetcher,
          ).pipe(
            Effect.mapError(
              error =>
                new OperatorEmailInspectionBadRequest({
                  reason: error.message,
                }),
            ),
          )
          const emailStatus = result.ok ? 'accepted' : 'failed'
          const payload = reviewReadyArtifactSmokePayload({
            artifactId: target.artifact_id,
            artifactUrl: target.artifact_url,
            dryRun: false,
            emailMessageId: result.emailMessageId,
            emailStatus,
            providerMessageId: result.ok ? result.providerMessageId : null,
            skipReason: result.ok ? null : result.errorMessage,
            softwareOrderId: target.software_order_id,
          })
          const summary =
            result.ok
              ? 'Autopilot customer artifact review-ready email notification was accepted.'
              : 'Autopilot customer artifact review-ready email notification failed.'

          yield* recordReviewReadyArtifactSmokeEvent(
            db,
            {
              actorUserId: session.user.userId,
              emailMessageId: result.emailMessageId,
              payload,
              summary,
              target,
            },
            supervisionMirror,
          )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({
              smoke: {
                artifactId: target.artifact_id,
                dryRun: false,
                emailMessageId: result.emailMessageId,
                emailStatus,
                idempotencyKey: emailInput.idempotencyKey,
                providerMessageId: result.ok ? result.providerMessageId : null,
                skipReason: result.ok ? null : result.errorMessage,
                softwareOrderId: target.software_order_id,
              },
            }),
            session,
          )
        }

        const target = yield* readReviewReadySmokeTarget(
          db,
          identityDbForEnv(env),
          body,
        )

        if (target === null) {
          return noStoreJsonResponse(
            { error: 'site_revision_email_target_not_found' },
            { status: 404 },
          )
        }

        if (
          target.software_order_id === null ||
          target.target_user_id === null
        ) {
          return noStoreJsonResponse(
            { error: 'site_revision_email_target_missing_order' },
            { status: 409 },
          )
        }

        const email = target.primary_email?.trim()

        if (email === undefined || email === '') {
          return noStoreJsonResponse(
            { error: 'site_revision_email_target_missing_email' },
            { status: 409 },
          )
        }

        const assignment = yield* readReviewReadySmokeAssignment(
          db,
          target.site_id,
        )
        const appOrigin = dependencies.getAppOrigin?.(env) ?? 'https://openagents.com'
        const emailInputWithoutKey = new OrderSitesTransactionalEmailInput({
          appOrigin,
          ...(assignment === null ? {} : { assignmentId: assignment.id }),
          artifactLabel: null,
          artifactUrl: null,
          customerSafeStatus: 'Ready for review',
          displayName: target.display_name ?? email,
          eventRef:
            target.version_id ??
            target.active_deployment_id ??
            target.site_id,
          lifecycleKind: 'review_ready',
          nextAction:
            'Open your order status page, review the latest Site revision, and send any follow-up comment.',
          orderId: target.software_order_id,
          revisionUrl: siteRevisionUrl(target.deployment_url, target.version_id),
          safeReason: null,
          siteId: target.site_id,
          siteTitle: target.site_title,
          siteUrl: target.deployment_url,
          sourceAuthorityRefs: [
            'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
            'github:OpenAgentsInc/autopilot-omega#148',
          ],
          targetRefs: [
            target.software_order_id,
            target.site_id,
            ...(target.version_id === null ? [] : [target.version_id]),
            ...(target.active_deployment_id === null
              ? []
              : [target.active_deployment_id]),
            ...(assignment === null ? [] : [assignment.id]),
          ],
          to: email,
        })
        const emailInput = new OrderSitesTransactionalEmailInput({
          ...emailInputWithoutKey,
          idempotencyKey:
            buildOrderSitesTransactionalEmailIdempotencyKey(
              emailInputWithoutKey,
            ),
        })
        const resend = dependencies.getResendEmailConfig?.(env)

        if (body.dryRun === true) {
          const rendered =
            resend === undefined
              ? null
              : yield* renderOrderSitesTransactionalEmail(
                  resend,
                  emailInput,
                ).pipe(
                  Effect.mapError(
                    error =>
                      new OperatorEmailInspectionBadRequest({
                        reason: error.message,
                      }),
                  ),
                )

          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({
              smoke: {
                dryRun: true,
                emailStatus: 'skipped',
                idempotencyKey: emailInput.idempotencyKey,
                rendered:
                  rendered === null
                    ? null
                    : {
                        subject: rendered.subject,
                        templateSlug: rendered.templateSlug,
                      },
                skipReason:
                  resend === undefined ? 'email_config_missing' : 'dry_run',
                siteId: target.site_id,
                softwareOrderId: target.software_order_id,
                versionId: target.version_id,
              },
            }),
            session,
          )
        }

        if (resend === undefined) {
          return dependencies.appendRefreshedSessionCookies(
            noStoreJsonResponse({
              smoke: {
                dryRun: false,
                emailStatus: 'skipped',
                idempotencyKey: emailInput.idempotencyKey,
                skipReason: 'email_config_missing',
                siteId: target.site_id,
                softwareOrderId: target.software_order_id,
                versionId: target.version_id,
              },
            }),
            session,
          )
        }

        const result = yield* sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          emailInput,
          {
            actorUserId: session.user.userId,
            metadata: {
              assignmentId: assignment?.id ?? null,
              eventSource: 'operator_review_ready_email_smoke',
              lifecycleKind: 'review_ready',
              siteId: target.site_id,
              softwareOrderId: target.software_order_id,
              versionId: target.version_id,
            },
            sourceAuthorityRef:
              'system.order_sites_lifecycle_email.v1',
            targetUserId: target.target_user_id,
          },
          dependencies.emailFetcher,
        ).pipe(
          Effect.mapError(
            error =>
              new OperatorEmailInspectionBadRequest({ reason: error.message }),
          ),
        )
        const emailStatus = result.ok ? 'accepted' : 'failed'
        const payload = reviewReadySmokePayload({
          dryRun: false,
          emailMessageId: result.emailMessageId,
          emailStatus,
          providerMessageId: result.ok ? result.providerMessageId : null,
          siteId: target.site_id,
          siteUrl: target.deployment_url,
          skipReason: result.ok ? null : result.errorMessage,
          softwareOrderId: target.software_order_id,
          versionId: target.version_id,
        })
        const summary =
          result.ok
            ? 'Autopilot customer review-ready email notification was accepted.'
            : 'Autopilot customer review-ready email notification failed.'

        yield* recordReviewReadySmokeEvents(
          db,
          {
            actorRunId: assignment?.current_run_id ?? null,
            actorUserId: session.user.userId,
            assignment,
            emailMessageId: result.emailMessageId,
            payload,
            siteId: target.site_id,
            summary,
            versionId: target.version_id,
          },
          supervisionMirror,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({
            smoke: {
              dryRun: false,
              emailMessageId: result.emailMessageId,
              emailStatus,
              idempotencyKey: emailInput.idempotencyKey,
              providerMessageId: result.ok ? result.providerMessageId : null,
              siteId: target.site_id,
              softwareOrderId: target.software_order_id,
              versionId: target.version_id,
            },
          }),
          session,
        )
      }).pipe(
        Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
      )
    }

    if (url.pathname !== '/api/operator/email-deliveries') {
      return undefined
    }

    return Effect.gen(function* () {
      if (request.method !== 'GET') {
        return methodNotAllowed(['GET'])
      }

      const session = yield* requireAdminSession(
        dependencies,
        request,
        env,
        ctx,
      )
      const inspection = yield* inspectOperatorEmailDelivery(
        makeCrmEmailDatabaseForEnv(env, { d1: openAgentsDatabase(env) }),
        scopeFromUrl(url),
      )

      return dependencies.appendRefreshedSessionCookies(
        noStoreJsonResponse({ inspection }),
        session,
      )
    }).pipe(
      Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
    )
  },
})
