import { Effect, Schema as S } from 'effect'

// KS-8.12 (#8323): the site-library db seam rides the dual-write mirror —
// passthrough for non-scoped statements, raw D1 when no binding.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  AutopilotSiteAccessMode,
  AutopilotSiteVisibility,
} from './sites'

export const SiteLibraryScope = S.Literals(['mine', 'public', 'recent'])
export type SiteLibraryScope = typeof SiteLibraryScope.Type

export const SiteLibrarySiteStatus = S.Literals([
  'draft',
  'generating',
  'generated',
  'needs_review',
  'approved',
  'archived',
  'disabled',
])
export type SiteLibrarySiteStatus = typeof SiteLibrarySiteStatus.Type

export const SiteLibraryProjection = S.Struct({
  accessMode: AutopilotSiteAccessMode,
  activeDeploymentId: S.NullOr(S.String),
  activeDeploymentStatus: S.NullOr(S.String),
  activeUrl: S.NullOr(S.String),
  activeVersionId: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
  canManage: S.Boolean,
  createdAt: S.String,
  deploymentCount: S.Number,
  id: S.String,
  ownerUserId: S.String,
  slug: S.String,
  softwareOrderId: S.NullOr(S.String),
  status: SiteLibrarySiteStatus,
  title: S.String,
  updatedAt: S.String,
  versionCount: S.Number,
  visibility: AutopilotSiteVisibility,
})
export type SiteLibraryProjection = typeof SiteLibraryProjection.Type

export const SiteLibraryResponse = S.Struct({
  sites: S.Array(SiteLibraryProjection),
})
export type SiteLibraryResponse = typeof SiteLibraryResponse.Type

export const UpdateSiteLibraryAccessInput = S.Struct({
  accessMode: AutopilotSiteAccessMode,
  actorUserId: S.String,
  isAdmin: S.Boolean,
  siteId: S.String,
  visibility: AutopilotSiteVisibility,
})
export type UpdateSiteLibraryAccessInput =
  typeof UpdateSiteLibraryAccessInput.Type

export const MutateSiteLibraryLifecycleInput = S.Struct({
  actorUserId: S.String,
  idempotencyKey: S.String,
  isAdmin: S.Boolean,
  siteId: S.String,
})
export type MutateSiteLibraryLifecycleInput =
  typeof MutateSiteLibraryLifecycleInput.Type

export type SiteLibraryRuntime = Readonly<{
  makeEventId: () => string
  nowIso: () => string
}>

export const systemSiteLibraryRuntime: SiteLibraryRuntime = {
  makeEventId: () => compactRandomId('site_event'),
  nowIso: currentIsoTimestamp,
}

type SiteLibraryRow = Readonly<{
  access_mode: typeof AutopilotSiteAccessMode.Type
  active_deployment_id: string | null
  active_deployment_status: string | null
  active_url: string | null
  active_version_id: string | null
  archived_at: string | null
  created_at: string
  deployment_count: number
  id: string
  owner_user_id: string
  slug: string
  software_order_id: string | null
  status: SiteLibrarySiteStatus
  title: string
  updated_at: string
  version_count: number
  visibility: typeof AutopilotSiteVisibility.Type
}>

export class SiteLibraryStorageError extends S.TaggedErrorClass<SiteLibraryStorageError>()(
  'SiteLibraryStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class SiteLibraryNotFound extends S.TaggedErrorClass<SiteLibraryNotFound>()(
  'SiteLibraryNotFound',
  {
    siteId: S.String,
  },
) {}

export class SiteLibraryForbidden extends S.TaggedErrorClass<SiteLibraryForbidden>()(
  'SiteLibraryForbidden',
  {
    siteId: S.String,
  },
) {}

export class SiteLibraryValidationError extends S.TaggedErrorClass<SiteLibraryValidationError>()(
  'SiteLibraryValidationError',
  {
    reason: S.String,
  },
) {}

export type SiteLibraryError =
  | SiteLibraryForbidden
  | SiteLibraryNotFound
  | SiteLibraryStorageError
  | SiteLibraryValidationError

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteLibraryStorageError> =>
  Effect.tryPromise({
    catch: error => new SiteLibraryStorageError({ error, operation }),
    try: run,
  })

const boundedLimit = (limit: number | undefined): number => {
  if (limit === undefined || !Number.isSafeInteger(limit)) {
    return 50
  }

  return Math.min(Math.max(limit, 1), 100)
}

const canManage = (
  row: Pick<SiteLibraryRow, 'owner_user_id'>,
  actorUserId: string,
  isAdmin: boolean,
): boolean => isAdmin || row.owner_user_id === actorUserId

const projectionFromRow = (
  row: SiteLibraryRow,
  actorUserId: string,
  isAdmin: boolean,
): SiteLibraryProjection => ({
  accessMode: row.access_mode,
  activeDeploymentId: row.active_deployment_id,
  activeDeploymentStatus: row.active_deployment_status,
  activeUrl: row.active_url,
  activeVersionId: row.active_version_id,
  archivedAt: row.archived_at,
  canManage: canManage(row, actorUserId, isAdmin),
  createdAt: row.created_at,
  deploymentCount: row.deployment_count,
  id: row.id,
  ownerUserId: row.owner_user_id,
  slug: row.slug,
  softwareOrderId: row.software_order_id,
  status: row.status,
  title: row.title,
  updatedAt: row.updated_at,
  versionCount: row.version_count,
  visibility: row.visibility,
})

const siteLibrarySelect = `site_projects.id,
       site_projects.software_order_id,
       site_projects.owner_user_id,
       site_projects.slug,
       site_projects.title,
       site_projects.status,
       site_projects.access_mode,
       site_projects.visibility,
       site_projects.active_version_id,
       site_projects.active_deployment_id,
       site_projects.created_at,
       site_projects.updated_at,
       site_projects.archived_at,
       active_deployments.url AS active_url,
       active_deployments.status AS active_deployment_status,
       (
         SELECT COUNT(*)
           FROM site_versions
          WHERE site_versions.site_id = site_projects.id
       ) AS version_count,
       (
         SELECT COUNT(*)
           FROM site_deployments
          WHERE site_deployments.site_id = site_projects.id
       ) AS deployment_count`

const siteLibraryJoins = `LEFT JOIN site_deployments AS active_deployments
         ON active_deployments.id = site_projects.active_deployment_id
        AND active_deployments.site_id = site_projects.id`

const readActiveSite = (
  db: D1Database,
  siteId: string,
): Effect.Effect<SiteLibraryRow | null, SiteLibraryStorageError> =>
  d1Effect('siteLibrary.readActiveSite', () =>
    db
      .prepare(
        `SELECT ${siteLibrarySelect}
           FROM site_projects
           ${siteLibraryJoins}
          WHERE site_projects.id = ?
            AND site_projects.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<SiteLibraryRow>(),
  )

const authorizedSite = (
  row: SiteLibraryRow | null,
  input: Readonly<{ actorUserId: string; isAdmin: boolean; siteId: string }>,
): Effect.Effect<SiteLibraryRow, SiteLibraryForbidden | SiteLibraryNotFound> => {
  if (row === null) {
    return Effect.fail(new SiteLibraryNotFound({ siteId: input.siteId }))
  }

  return canManage(row, input.actorUserId, input.isAdmin)
    ? Effect.succeed(row)
    : Effect.fail(new SiteLibraryForbidden({ siteId: input.siteId }))
}

export const listSiteLibrary = (
  db: D1Database,
  input: Readonly<{
    actorUserId: string
    isAdmin: boolean
    limit?: number | undefined
    scope: SiteLibraryScope
  }>,
): Effect.Effect<SiteLibraryResponse, SiteLibraryStorageError> => {
  const limit = boundedLimit(input.limit)

  if (input.scope === 'public') {
    return d1Effect('siteLibrary.listPublic', () =>
      db
        .prepare(
          `SELECT ${siteLibrarySelect}
             FROM site_projects
             ${siteLibraryJoins}
            WHERE site_projects.archived_at IS NULL
              AND site_projects.status != 'disabled'
              AND site_projects.access_mode = 'public'
              AND site_projects.visibility = 'public'
            ORDER BY site_projects.updated_at DESC
            LIMIT ?`,
        )
        .bind(limit)
        .all<SiteLibraryRow>(),
    ).pipe(
      Effect.map(result => ({
        sites: result.results.map(row =>
          projectionFromRow(row, input.actorUserId, input.isAdmin),
        ),
      })),
    )
  }

  if (input.scope === 'recent' && input.isAdmin) {
    return d1Effect('siteLibrary.listRecentAdmin', () =>
      db
        .prepare(
          `SELECT ${siteLibrarySelect}
             FROM site_projects
             ${siteLibraryJoins}
            WHERE site_projects.archived_at IS NULL
              AND site_projects.status != 'disabled'
            ORDER BY site_projects.updated_at DESC
            LIMIT ?`,
        )
        .bind(limit)
        .all<SiteLibraryRow>(),
    ).pipe(
      Effect.map(result => ({
        sites: result.results.map(row =>
          projectionFromRow(row, input.actorUserId, input.isAdmin),
        ),
      })),
    )
  }

  return d1Effect('siteLibrary.listMine', () =>
    db
      .prepare(
        `SELECT ${siteLibrarySelect}
           FROM site_projects
           ${siteLibraryJoins}
          WHERE site_projects.owner_user_id = ?
            AND site_projects.archived_at IS NULL
            AND site_projects.status != 'disabled'
          ORDER BY site_projects.updated_at DESC
          LIMIT ?`,
      )
      .bind(input.actorUserId, limit)
      .all<SiteLibraryRow>(),
  ).pipe(
    Effect.map(result => ({
      sites: result.results.map(row =>
        projectionFromRow(row, input.actorUserId, input.isAdmin),
      ),
    })),
  )
}

const recordLifecycleEvent = (
  db: D1Database,
  runtime: SiteLibraryRuntime,
  input: Readonly<{
    actorUserId: string
    idempotencyKey: string | null
    siteId: string
    summary: string
    type: string
  }>,
): Effect.Effect<void, SiteLibraryStorageError> =>
  d1Effect('siteLibrary.events.insert', () =>
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
         VALUES (?, ?, NULL, NULL, ?, ?, ?, NULL, ?, ?)`,
      )
      .bind(
        runtime.makeEventId(),
        input.siteId,
        input.type,
        input.summary,
        input.actorUserId,
        input.idempotencyKey === null
          ? null
          : JSON.stringify({ idempotencyKey: input.idempotencyKey }),
        runtime.nowIso(),
      )
      .run()
      .then(() => undefined),
  )

export const updateSiteLibraryAccess = (
  db: D1Database,
  runtime: SiteLibraryRuntime,
  input: UpdateSiteLibraryAccessInput,
): Effect.Effect<SiteLibraryProjection, SiteLibraryError> =>
  Effect.gen(function* () {
    const existing = yield* authorizedSite(
      yield* readActiveSite(db, input.siteId),
      input,
    )

    if (
      !input.isAdmin &&
      (input.accessMode === 'public' || input.visibility === 'public') &&
      (existing.access_mode !== 'public' || existing.visibility !== 'public')
    ) {
      return yield* new SiteLibraryValidationError({
        reason: 'Making a hidden Site public again requires operator review.',
      })
    }

    const now = runtime.nowIso()

    yield* d1Effect('siteLibrary.updateAccess', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET access_mode = ?,
                  visibility = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.accessMode, input.visibility, now, input.siteId)
        .run()
        .then(() => undefined),
    )

    yield* recordLifecycleEvent(db, runtime, {
      actorUserId: input.actorUserId,
      idempotencyKey: null,
      siteId: input.siteId,
      summary: `Updated Site library visibility to ${input.accessMode} / ${input.visibility}.`,
      type: 'site_library.visibility_updated',
    })

    const updated = yield* readActiveSite(db, input.siteId)

    return yield* authorizedSite(updated, input).pipe(
      Effect.map(row =>
        projectionFromRow(row, input.actorUserId, input.isAdmin),
      ),
    )
  })

export const archiveSiteLibrarySite = (
  db: D1Database,
  runtime: SiteLibraryRuntime,
  input: MutateSiteLibraryLifecycleInput,
): Effect.Effect<SiteLibraryProjection, SiteLibraryError> =>
  Effect.gen(function* () {
    const existing = yield* authorizedSite(
      yield* readActiveSite(db, input.siteId),
      input,
    )
    const now = runtime.nowIso()

    yield* d1Effect('siteLibrary.archiveSite', () =>
      db
        .batch([
          db
            .prepare(
              `UPDATE site_deployments
                  SET status = 'disabled',
                      disabled_at = ?,
                      updated_at = ?
                WHERE site_id = ?
                  AND status = 'active'`,
            )
            .bind(now, now, input.siteId),
          db
            .prepare(
              `UPDATE site_builder_sessions
                  SET status = 'archived',
                      archived_at = ?,
                      updated_at = ?
                WHERE site_id = ?
                  AND archived_at IS NULL`,
            )
            .bind(now, now, input.siteId),
          db
            .prepare(
              `UPDATE site_projects
                  SET status = 'archived',
                      access_mode = 'owner_admins',
                      visibility = 'private',
                      active_version_id = NULL,
                      active_deployment_id = NULL,
                      updated_at = ?,
                      archived_at = ?
                WHERE id = ?
                  AND archived_at IS NULL`,
            )
            .bind(now, now, input.siteId),
        ])
        .then(() => undefined),
    )

    yield* recordLifecycleEvent(db, runtime, {
      actorUserId: input.actorUserId,
      idempotencyKey: input.idempotencyKey,
      siteId: input.siteId,
      summary: `Archived Site ${existing.slug}.`,
      type: 'site_library.archived',
    })

    return {
      ...projectionFromRow(existing, input.actorUserId, input.isAdmin),
      accessMode: 'owner_admins',
      activeDeploymentId: null,
      activeDeploymentStatus: null,
      activeUrl: null,
      activeVersionId: null,
      archivedAt: now,
      status: 'archived',
      updatedAt: now,
      visibility: 'private',
    }
  })

export const deleteSiteLibrarySite = (
  db: D1Database,
  runtime: SiteLibraryRuntime,
  input: MutateSiteLibraryLifecycleInput,
): Effect.Effect<SiteLibraryProjection, SiteLibraryError> =>
  Effect.gen(function* () {
    const existing = yield* authorizedSite(
      yield* readActiveSite(db, input.siteId),
      input,
    )
    const now = runtime.nowIso()

    yield* d1Effect('siteLibrary.deleteSite', () =>
      db
        .batch([
          db
            .prepare(
              `UPDATE site_deployments
                  SET status = 'disabled',
                      disabled_at = ?,
                      updated_at = ?
                WHERE site_id = ?
                  AND status = 'active'`,
            )
            .bind(now, now, input.siteId),
          db
            .prepare(
              `UPDATE site_builder_sessions
                  SET status = 'archived',
                      archived_at = ?,
                      updated_at = ?
                WHERE site_id = ?
                  AND archived_at IS NULL`,
            )
            .bind(now, now, input.siteId),
          db
            .prepare(
              `UPDATE site_projects
                  SET status = 'disabled',
                      access_mode = 'owner_admins',
                      visibility = 'private',
                      active_version_id = NULL,
                      active_deployment_id = NULL,
                      updated_at = ?,
                      archived_at = ?
                WHERE id = ?
                  AND archived_at IS NULL`,
            )
            .bind(now, now, input.siteId),
        ])
        .then(() => undefined),
    )

    yield* recordLifecycleEvent(db, runtime, {
      actorUserId: input.actorUserId,
      idempotencyKey: input.idempotencyKey,
      siteId: input.siteId,
      summary: `Disabled Site ${existing.slug}.`,
      type: 'site_library.deleted',
    })

    return {
      ...projectionFromRow(existing, input.actorUserId, input.isAdmin),
      accessMode: 'owner_admins',
      activeDeploymentId: null,
      activeDeploymentStatus: null,
      activeUrl: null,
      activeVersionId: null,
      archivedAt: now,
      status: 'disabled',
      updatedAt: now,
      visibility: 'private',
    }
  })

export const siteIsVisibleForBuilderSession = (
  db: D1Database,
  siteId: string | null,
): Effect.Effect<boolean, SiteLibraryStorageError> =>
  siteId === null
    ? Effect.succeed(true)
    : d1Effect('siteLibrary.builderSessionSiteVisible', () =>
        db
          .prepare(
            `SELECT id
               FROM site_projects
              WHERE id = ?
                AND archived_at IS NULL
                AND status != 'disabled'
              LIMIT 1`,
          )
          .bind(siteId)
          .first<{ id: string }>(),
      ).pipe(Effect.map(row => row !== null))

export const siteLibraryDatabase = (env: { OPENAGENTS_DB: D1Database }) =>
  openAgentsDatabase(env)
