import { Effect, Layer, Match as M, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { arrayFromUnknown, parseJsonUnknown } from './json-boundary'
import { openAgentsDatabase } from './runtime'

type AdminOverviewEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type AdminOverviewSession = Readonly<{
  user: Readonly<{
    email: string
  }>
}>

type AdminOverviewRouteDependencies<
  Session extends AdminOverviewSession,
  Bindings extends AdminOverviewEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

export type AdminOverviewUser = Readonly<{
  userId: string
  kind: 'human' | 'agent'
  displayName: string
  email: string | null
  githubUsername: string | null
  status: string
  onboardingStep: string
  onboardingCompletedAt: string | null
  softwareOrderCount: number
  createdAt: string
  updatedAt: string
}>

export type AdminOverviewSoftwareOrder = Readonly<{
  id: string
  userId: string
  userDisplayName: string | null
  userEmail: string | null
  status: string
  visibility: string
  request: string
  repositoryFullName: string | null
  currentRunId: string | null
  siteProjectId: string | null
  siteTitle: string | null
  siteSlug: string | null
  siteStatus: string | null
  siteAccessMode: string | null
  siteVisibility: string | null
  siteActiveVersionId: string | null
  siteActiveDeploymentId: string | null
  siteActiveUrl: string | null
  siteVersionCount: number
  siteLatestVersionId: string | null
  siteLatestVersionStatus: string | null
  siteLatestVersionSourceKind: string | null
  siteLatestVersionCreatedAt: string | null
  siteDeploymentCount: number
  siteLatestDeploymentId: string | null
  siteLatestDeploymentStatus: string | null
  siteLatestDeploymentRuntimeKind: string | null
  siteLatestDeploymentUpdatedAt: string | null
  siteStorageBindingCount: number
  siteStorageBindingSummary: string | null
  siteEnvironmentValueCount: number
  siteEnvironmentKeySummary: string | null
  siteAccessGrantCount: number
  siteLatestEventType: string | null
  siteLatestEventSummary: string | null
  siteLatestEventCreatedAt: string | null
  siteLatestCompatibilityId: string | null
  siteLatestCompatibilityStatus: string | null
  siteLatestCompatibilityCustomerSafeStatus: string | null
  siteLatestCompatibilityCustomerSafeNextAction: string | null
  siteLatestCompatibilityBlockerCount: number
  siteLatestCompatibilityWarningCount: number
  siteLatestCompatibilityCreatedAt: string | null
  siteLatestBuildValidationId: string | null
  siteLatestBuildValidationStatus: string | null
  siteLatestBuildValidationSourceHash: string | null
  siteLatestBuildValidationCustomerSafeStatus: string | null
  siteLatestBuildValidationCustomerSafeNextAction: string | null
  siteLatestBuildValidationBlockerCount: number
  siteLatestBuildValidationWarningCount: number
  siteLatestBuildValidationCreatedAt: string | null
  createdAt: string
  updatedAt: string
  archivedAt: string | null
}>

export type AdminOverview = Readonly<{
  users: ReadonlyArray<AdminOverviewUser>
  softwareOrders: ReadonlyArray<AdminOverviewSoftwareOrder>
}>

type AdminUserRow = Readonly<{
  user_id: string
  kind: 'human' | 'agent'
  display_name: string
  primary_email: string | null
  github_username: string | null
  status: string
  onboarding_step: string
  onboarding_completed_at: string | null
  software_order_count: number | null
  created_at: string
  updated_at: string
}>

type AdminSoftwareOrderRow = Readonly<{
  id: string
  user_id: string
  user_display_name: string | null
  user_email: string | null
  status: string
  visibility: string
  request: string
  repository_full_name: string | null
  current_run_id: string | null
  site_project_id: string | null
  site_title: string | null
  site_slug: string | null
  site_status: string | null
  site_access_mode: string | null
  site_visibility: string | null
  site_active_version_id: string | null
  site_active_deployment_id: string | null
  site_active_url: string | null
  site_version_count: number | null
  site_latest_version_id: string | null
  site_latest_version_status: string | null
  site_latest_version_source_kind: string | null
  site_latest_version_created_at: string | null
  site_deployment_count: number | null
  site_latest_deployment_id: string | null
  site_latest_deployment_status: string | null
  site_latest_deployment_runtime_kind: string | null
  site_latest_deployment_updated_at: string | null
  site_storage_binding_count: number | null
  site_storage_binding_summary: string | null
  site_environment_value_count: number | null
  site_environment_key_summary: string | null
  site_access_grant_count: number | null
  site_latest_event_type: string | null
  site_latest_event_summary: string | null
  site_latest_event_created_at: string | null
  site_latest_compatibility_id: string | null
  site_latest_compatibility_status: string | null
  site_latest_compatibility_customer_safe_status: string | null
  site_latest_compatibility_customer_safe_next_action: string | null
  site_latest_compatibility_blockers_json: string | null
  site_latest_compatibility_warnings_json: string | null
  site_latest_compatibility_created_at: string | null
  site_latest_build_validation_id: string | null
  site_latest_build_validation_status: string | null
  site_latest_build_validation_source_hash: string | null
  site_latest_build_validation_customer_safe_status: string | null
  site_latest_build_validation_customer_safe_next_action: string | null
  site_latest_build_validation_blockers_json: string | null
  site_latest_build_validation_warnings_json: string | null
  site_latest_build_validation_created_at: string | null
  created_at: string
  updated_at: string
  archived_at: string | null
}>

export class AdminOverviewStorageError extends S.TaggedErrorClass<AdminOverviewStorageError>()(
  'AdminOverviewStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

class AdminOverviewUnauthorized extends S.TaggedErrorClass<AdminOverviewUnauthorized>()(
  'AdminOverviewUnauthorized',
  {},
) {}

class AdminOverviewForbidden extends S.TaggedErrorClass<AdminOverviewForbidden>()(
  'AdminOverviewForbidden',
  {},
) {}

class AdminOverviewSessionError extends S.TaggedErrorClass<AdminOverviewSessionError>()(
  'AdminOverviewSessionError',
  {
    error: S.Defect,
  },
) {}

type AdminOverviewRouteError =
  | AdminOverviewForbidden
  | AdminOverviewSessionError
  | AdminOverviewStorageError
  | AdminOverviewUnauthorized

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AdminOverviewStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new AdminOverviewStorageError({ operation, error }),
  })

const routeErrorResponse = (error: AdminOverviewRouteError): HttpResponse =>
  M.value(error).pipe(
    M.tags({
      AdminOverviewForbidden: () =>
        noStoreJsonResponse({ error: 'forbidden' }, { status: 403 }),
      AdminOverviewSessionError: () =>
        noStoreJsonResponse({ error: 'session_error' }, { status: 500 }),
      AdminOverviewStorageError: () =>
        noStoreJsonResponse({ error: 'storage_error' }, { status: 500 }),
      AdminOverviewUnauthorized: () =>
        noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 }),
    }),
    M.exhaustive,
  )

const userFromRow = (row: AdminUserRow): AdminOverviewUser => ({
  userId: row.user_id,
  kind: row.kind,
  displayName: row.display_name,
  email: row.primary_email,
  githubUsername: row.github_username,
  status: row.status,
  onboardingStep: row.onboarding_step,
  onboardingCompletedAt: row.onboarding_completed_at,
  softwareOrderCount: row.software_order_count ?? 0,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const jsonArrayCount = (value: string | null): number => {
  if (value === null || value.trim() === '') {
    return 0
  }

  try {
    return arrayFromUnknown(parseJsonUnknown(value))?.length ?? 0
  } catch {
    return 0
  }
}

const softwareOrderFromRow = (
  row: AdminSoftwareOrderRow,
): AdminOverviewSoftwareOrder => ({
  id: row.id,
  userId: row.user_id,
  userDisplayName: row.user_display_name,
  userEmail: row.user_email,
  status: row.status,
  visibility: row.visibility,
  request: row.request,
  repositoryFullName: row.repository_full_name,
  currentRunId: row.current_run_id,
  siteProjectId: row.site_project_id,
  siteTitle: row.site_title,
  siteSlug: row.site_slug,
  siteStatus: row.site_status,
  siteAccessMode: row.site_access_mode,
  siteVisibility: row.site_visibility,
  siteActiveVersionId: row.site_active_version_id,
  siteActiveDeploymentId: row.site_active_deployment_id,
  siteActiveUrl: row.site_active_url,
  siteVersionCount: row.site_version_count ?? 0,
  siteLatestVersionId: row.site_latest_version_id,
  siteLatestVersionStatus: row.site_latest_version_status,
  siteLatestVersionSourceKind: row.site_latest_version_source_kind,
  siteLatestVersionCreatedAt: row.site_latest_version_created_at,
  siteDeploymentCount: row.site_deployment_count ?? 0,
  siteLatestDeploymentId: row.site_latest_deployment_id,
  siteLatestDeploymentStatus: row.site_latest_deployment_status,
  siteLatestDeploymentRuntimeKind: row.site_latest_deployment_runtime_kind,
  siteLatestDeploymentUpdatedAt: row.site_latest_deployment_updated_at,
  siteStorageBindingCount: row.site_storage_binding_count ?? 0,
  siteStorageBindingSummary: row.site_storage_binding_summary,
  siteEnvironmentValueCount: row.site_environment_value_count ?? 0,
  siteEnvironmentKeySummary: row.site_environment_key_summary,
  siteAccessGrantCount: row.site_access_grant_count ?? 0,
  siteLatestEventType: row.site_latest_event_type,
  siteLatestEventSummary: row.site_latest_event_summary,
  siteLatestEventCreatedAt: row.site_latest_event_created_at,
  siteLatestCompatibilityId: row.site_latest_compatibility_id,
  siteLatestCompatibilityStatus: row.site_latest_compatibility_status,
  siteLatestCompatibilityCustomerSafeStatus:
    row.site_latest_compatibility_customer_safe_status,
  siteLatestCompatibilityCustomerSafeNextAction:
    row.site_latest_compatibility_customer_safe_next_action,
  siteLatestCompatibilityBlockerCount: jsonArrayCount(
    row.site_latest_compatibility_blockers_json,
  ),
  siteLatestCompatibilityWarningCount: jsonArrayCount(
    row.site_latest_compatibility_warnings_json,
  ),
  siteLatestCompatibilityCreatedAt: row.site_latest_compatibility_created_at,
  siteLatestBuildValidationId: row.site_latest_build_validation_id,
  siteLatestBuildValidationStatus: row.site_latest_build_validation_status,
  siteLatestBuildValidationSourceHash:
    row.site_latest_build_validation_source_hash,
  siteLatestBuildValidationCustomerSafeStatus:
    row.site_latest_build_validation_customer_safe_status,
  siteLatestBuildValidationCustomerSafeNextAction:
    row.site_latest_build_validation_customer_safe_next_action,
  siteLatestBuildValidationBlockerCount: jsonArrayCount(
    row.site_latest_build_validation_blockers_json,
  ),
  siteLatestBuildValidationWarningCount: jsonArrayCount(
    row.site_latest_build_validation_warnings_json,
  ),
  siteLatestBuildValidationCreatedAt:
    row.site_latest_build_validation_created_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  archivedAt: row.archived_at,
})

const readAdminOverview = (
  db: D1Database,
): Effect.Effect<AdminOverview, AdminOverviewStorageError> =>
  Effect.gen(function* () {
    const users = yield* d1Effect('adminOverview.users.list', () =>
      db
        .prepare(
          `SELECT users.id AS user_id,
                  users.kind,
                  users.display_name,
                  users.primary_email,
                  MAX(auth_identities.provider_username) AS github_username,
                  users.status,
                  users.onboarding_step,
                  users.onboarding_completed_at,
                  COUNT(DISTINCT software_orders.id) AS software_order_count,
                  users.created_at,
                  users.updated_at
             FROM users
             LEFT JOIN auth_identities
               ON auth_identities.user_id = users.id
              AND auth_identities.provider = 'github'
              AND auth_identities.deleted_at IS NULL
             LEFT JOIN software_orders
               ON software_orders.user_id = users.id
            WHERE users.deleted_at IS NULL
            GROUP BY users.id,
                     users.kind,
                     users.display_name,
                     users.primary_email,
                     users.status,
                     users.onboarding_step,
                     users.onboarding_completed_at,
                     users.created_at,
                     users.updated_at
            ORDER BY users.created_at DESC`,
        )
        .all<AdminUserRow>(),
    )
    const softwareOrders = yield* d1Effect(
      'adminOverview.softwareOrders.list',
      () =>
        db
          .prepare(
            `SELECT software_orders.id,
                    software_orders.user_id,
                    users.display_name AS user_display_name,
                    users.primary_email AS user_email,
                    software_orders.status,
                    software_orders.visibility,
                    software_orders.request,
                    software_orders.repository_full_name,
                    software_orders.current_run_id,
                    site_projects.id AS site_project_id,
                    site_projects.title AS site_title,
                    site_projects.slug AS site_slug,
                    site_projects.status AS site_status,
                    site_projects.access_mode AS site_access_mode,
                    site_projects.visibility AS site_visibility,
                    site_projects.active_version_id AS site_active_version_id,
                    site_projects.active_deployment_id AS site_active_deployment_id,
                    active_deployments.url AS site_active_url,
                    (
                      SELECT COUNT(*)
                        FROM site_versions
                       WHERE site_versions.site_id = site_projects.id
                    ) AS site_version_count,
                    latest_versions.id AS site_latest_version_id,
                    latest_versions.build_status AS site_latest_version_status,
                    latest_versions.source_kind AS site_latest_version_source_kind,
                    latest_versions.created_at AS site_latest_version_created_at,
                    (
                      SELECT COUNT(*)
                        FROM site_deployments
                       WHERE site_deployments.site_id = site_projects.id
                    ) AS site_deployment_count,
                    latest_deployments.id AS site_latest_deployment_id,
                    latest_deployments.status AS site_latest_deployment_status,
                    latest_deployments.runtime_kind AS site_latest_deployment_runtime_kind,
                    latest_deployments.updated_at AS site_latest_deployment_updated_at,
                    (
                      SELECT COUNT(*)
                        FROM site_storage_bindings
                       WHERE site_storage_bindings.site_id = site_projects.id
                    ) AS site_storage_binding_count,
                    (
                      SELECT GROUP_CONCAT(kind || ':' || binding_name, ', ')
                        FROM site_storage_bindings
                       WHERE site_storage_bindings.site_id = site_projects.id
                    ) AS site_storage_binding_summary,
                    (
                      SELECT COUNT(*)
                        FROM site_environment_values
                       WHERE site_environment_values.site_id = site_projects.id
                         AND site_environment_values.deleted_at IS NULL
                    ) AS site_environment_value_count,
                    (
                      SELECT GROUP_CONCAT(key || ':' || kind, ', ')
                        FROM site_environment_values
                       WHERE site_environment_values.site_id = site_projects.id
                         AND site_environment_values.deleted_at IS NULL
                    ) AS site_environment_key_summary,
                    (
                      SELECT COUNT(*)
                        FROM site_access_grants
                       WHERE site_access_grants.site_id = site_projects.id
                         AND site_access_grants.revoked_at IS NULL
                    ) AS site_access_grant_count,
                    latest_events.type AS site_latest_event_type,
                    latest_events.summary AS site_latest_event_summary,
                    latest_events.created_at AS site_latest_event_created_at,
                    latest_compatibility.id AS site_latest_compatibility_id,
                    latest_compatibility.status AS site_latest_compatibility_status,
                    latest_compatibility.customer_safe_status AS site_latest_compatibility_customer_safe_status,
                    latest_compatibility.customer_safe_next_action AS site_latest_compatibility_customer_safe_next_action,
                    latest_compatibility.blockers_json AS site_latest_compatibility_blockers_json,
                    latest_compatibility.warnings_json AS site_latest_compatibility_warnings_json,
                    latest_compatibility.created_at AS site_latest_compatibility_created_at,
                    latest_build_validations.id AS site_latest_build_validation_id,
                    latest_build_validations.status AS site_latest_build_validation_status,
                    latest_build_validations.source_hash AS site_latest_build_validation_source_hash,
                    latest_build_validations.customer_safe_status AS site_latest_build_validation_customer_safe_status,
                    latest_build_validations.customer_safe_next_action AS site_latest_build_validation_customer_safe_next_action,
                    latest_build_validations.blockers_json AS site_latest_build_validation_blockers_json,
                    latest_build_validations.warnings_json AS site_latest_build_validation_warnings_json,
                    latest_build_validations.created_at AS site_latest_build_validation_created_at,
                    software_orders.created_at,
                    software_orders.updated_at,
                    software_orders.archived_at
               FROM software_orders
               LEFT JOIN users
                 ON users.id = software_orders.user_id
               LEFT JOIN site_projects
                 ON site_projects.software_order_id = software_orders.id
                AND site_projects.archived_at IS NULL
               LEFT JOIN site_deployments AS active_deployments
                 ON active_deployments.id = site_projects.active_deployment_id
               LEFT JOIN site_versions AS latest_versions
                 ON latest_versions.id = (
                    SELECT site_versions.id
                      FROM site_versions
                     WHERE site_versions.site_id = site_projects.id
                     ORDER BY site_versions.created_at DESC
                     LIMIT 1
                 )
               LEFT JOIN site_deployments AS latest_deployments
                 ON latest_deployments.id = (
                    SELECT site_deployments.id
                      FROM site_deployments
                     WHERE site_deployments.site_id = site_projects.id
                     ORDER BY site_deployments.created_at DESC
                     LIMIT 1
                 )
               LEFT JOIN site_events AS latest_events
                 ON latest_events.id = (
                    SELECT site_events.id
                      FROM site_events
                     WHERE site_events.site_id = site_projects.id
                     ORDER BY site_events.created_at DESC
                     LIMIT 1
                 )
               LEFT JOIN site_compatibility_checks AS latest_compatibility
                 ON latest_compatibility.id = (
                    SELECT site_compatibility_checks.id
                      FROM site_compatibility_checks
                     WHERE site_compatibility_checks.site_id = site_projects.id
                       AND site_compatibility_checks.archived_at IS NULL
                     ORDER BY site_compatibility_checks.created_at DESC
                     LIMIT 1
                 )
               LEFT JOIN site_build_validations AS latest_build_validations
                 ON latest_build_validations.id = (
                    SELECT site_build_validations.id
                      FROM site_build_validations
                     WHERE site_build_validations.site_id = site_projects.id
                       AND site_build_validations.archived_at IS NULL
                     ORDER BY site_build_validations.created_at DESC
                     LIMIT 1
                 )
              ORDER BY software_orders.created_at DESC`,
          )
          .all<AdminSoftwareOrderRow>(),
    )

    return {
      users: users.results.map(userFromRow),
      softwareOrders: softwareOrders.results.map(softwareOrderFromRow),
    }
  })

export class AdminOverviewStore extends Context.Service<
  AdminOverviewStore,
  {
    readonly readOverview: Effect.Effect<
      AdminOverview,
      AdminOverviewStorageError
    >
  }
>()('@openagentsinc/autopilot-omega/AdminOverviewStore') {
  static readonly layer = (env: AdminOverviewEnv) =>
    Layer.succeed(AdminOverviewStore, {
      readOverview: readAdminOverview(openAgentsDatabase(env)),
    })
}

const requireAdminSession = <
  Session extends AdminOverviewSession,
  Bindings extends AdminOverviewEnv,
>(
  dependencies: AdminOverviewRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const session = yield* Effect.tryPromise({
      try: () => dependencies.requireBrowserSession(request, env, ctx),
      catch: error => new AdminOverviewSessionError({ error }),
    })

    if (session === undefined) {
      return yield* new AdminOverviewUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new AdminOverviewForbidden({})
    }

    return session
  })

const runRoute = (
  env: AdminOverviewEnv,
  effect: Effect.Effect<
    HttpResponse,
    AdminOverviewRouteError,
    AdminOverviewStore
  >,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.provide(AdminOverviewStore.layer(env)),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeAdminOverviewHandlers = <
  Session extends AdminOverviewSession,
  Bindings extends AdminOverviewEnv,
>(
  dependencies: AdminOverviewRouteDependencies<Session, Bindings>,
) => ({
  handleAdminOverviewApi: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => {
    if (request.method !== 'GET') {
      return Effect.succeed(methodNotAllowed(['GET']))
    }

    return runRoute(
      env,
      Effect.gen(function* () {
        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const store = yield* AdminOverviewStore
        const overview = yield* store.readOverview

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(overview),
          session,
        )
      }),
    )
  },
})
