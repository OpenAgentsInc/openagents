import { Effect, Layer, Option, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { artifactsBucketForEnv } from './artifacts-binding'
import { optionalString, parseJsonRecord } from './json-boundary'
import { openAgentsDatabase } from './runtime'

type SiteRuntimeEnv = Readonly<{
  // Optional since #8516 (account-level R2 disabled); resolved through
  // `artifactsBucketForEnv`, which rejects per-call when absent.
  ARTIFACTS?: R2Bucket | undefined
  OPENAGENTS_DB: D1Database
}>

export type SiteRuntimeStaticAsset = Readonly<{
  _tag: 'static'
  assetPath: string
  cacheControl: string | null
  contentType: string | null
  deploymentId: string
  r2Key: string
  siteId: string
  slug: string
  versionId: string
}>

export type SiteRuntimeWorkerDeployment = Readonly<{
  _tag: 'worker'
  deploymentId: string
  dispatchNamespace: string
  d1BindingName: string | null
  externalDeploymentId: string | null
  r2BindingName: string | null
  runtimeScriptName: string
  siteId: string
  slug: string
  versionId: string
  workerModuleR2Key: string
}>

export type SiteRuntimeTarget =
  SiteRuntimeStaticAsset | SiteRuntimeWorkerDeployment

type SiteRuntimeDeploymentRow = Readonly<{
  site_id: string
  slug: string
  site_status: string
  access_mode: string
  visibility: string
  active_deployment_id: string | null
  deployment_id: string | null
  deployment_status: string | null
  runtime_kind: string | null
  runtime_script_name: string | null
  dispatch_namespace: string | null
  external_deployment_id: string | null
  version_id: string | null
  build_status: string | null
  worker_module_r2_key: string | null
  static_assets_manifest_json: string | null
  d1_binding_name: string | null
  r2_binding_name: string | null
}>

const StaticManifestAssetObject = S.Struct({
  r2Key: S.String,
  cacheControl: S.optionalKey(S.String),
  contentType: S.optionalKey(S.String),
})

const StaticManifestAsset = S.Union([S.String, StaticManifestAssetObject])

const StaticAssetsManifest = S.Struct({
  assets: S.Record(S.String, StaticManifestAsset),
})

type StaticManifestAsset = typeof StaticManifestAsset.Type

const decodeStaticAssetsManifest = S.decodeUnknownOption(StaticAssetsManifest)

export class SiteRuntimeStorageError extends S.TaggedErrorClass<SiteRuntimeStorageError>()(
  'SiteRuntimeStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteRuntimeStorageError> =>
  Effect.tryPromise({
    catch: error => new SiteRuntimeStorageError({ error, operation }),
    try: run,
  })

const r2Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, SiteRuntimeStorageError> =>
  Effect.tryPromise({
    catch: error => new SiteRuntimeStorageError({ error, operation }),
    try: run,
  })

const readRuntimeDeployment = (
  db: D1Database,
  slug: string,
): Effect.Effect<SiteRuntimeDeploymentRow | null, SiteRuntimeStorageError> =>
  d1Effect('siteRuntime.deployments.readActiveBySlug', () =>
    db
      .prepare(
        `SELECT site_projects.id AS site_id,
                site_projects.slug,
                site_projects.status AS site_status,
                site_projects.access_mode,
                site_projects.visibility,
                site_projects.active_deployment_id,
                site_deployments.id AS deployment_id,
                site_deployments.status AS deployment_status,
                site_deployments.runtime_kind,
                site_deployments.runtime_script_name,
                site_deployments.dispatch_namespace,
                site_deployments.external_deployment_id,
                site_versions.id AS version_id,
                site_versions.build_status,
                site_versions.worker_module_r2_key,
                site_versions.static_assets_manifest_json,
                site_versions.d1_binding_name,
                site_versions.r2_binding_name
           FROM site_projects
           LEFT JOIN site_deployments
             ON site_deployments.id = site_projects.active_deployment_id
            AND site_deployments.site_id = site_projects.id
           LEFT JOIN site_versions
             ON site_versions.id = site_deployments.version_id
            AND site_versions.site_id = site_projects.id
          WHERE site_projects.slug = ?
            AND site_projects.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(slug)
      .first<SiteRuntimeDeploymentRow>(),
  )

const readRuntimeVersion = (
  db: D1Database,
  slug: string,
  versionId: string,
): Effect.Effect<SiteRuntimeDeploymentRow | null, SiteRuntimeStorageError> =>
  d1Effect('siteRuntime.deployments.readVersionBySlug', () =>
    db
      .prepare(
        `SELECT site_projects.id AS site_id,
                site_projects.slug,
                site_projects.status AS site_status,
                site_projects.access_mode,
                site_projects.visibility,
                site_projects.active_deployment_id,
                site_deployments.id AS deployment_id,
                site_deployments.status AS deployment_status,
                site_deployments.runtime_kind,
                site_deployments.runtime_script_name,
                site_deployments.dispatch_namespace,
                site_deployments.external_deployment_id,
                site_versions.id AS version_id,
                site_versions.build_status,
                site_versions.worker_module_r2_key,
                site_versions.static_assets_manifest_json,
                site_versions.d1_binding_name,
                site_versions.r2_binding_name
           FROM site_projects
           JOIN site_versions
             ON site_versions.site_id = site_projects.id
            AND site_versions.id = ?
           LEFT JOIN site_deployments
             ON site_deployments.id = (
                  SELECT deployment.id
                    FROM site_deployments AS deployment
                   WHERE deployment.site_id = site_projects.id
                     AND deployment.version_id = site_versions.id
                   ORDER BY deployment.updated_at DESC,
                            deployment.created_at DESC
                   LIMIT 1
                )
          WHERE site_projects.slug = ?
            AND site_projects.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(versionId, slug)
      .first<SiteRuntimeDeploymentRow>(),
  )

const readRuntimeDeploymentForTeam = (
  db: D1Database,
  teamId: string,
): Effect.Effect<SiteRuntimeDeploymentRow | null, SiteRuntimeStorageError> =>
  d1Effect('siteRuntime.deployments.readActiveByTeam', () =>
    db
      .prepare(
        `SELECT site_projects.id AS site_id,
                site_projects.slug,
                site_projects.status AS site_status,
                site_projects.access_mode,
                site_projects.visibility,
                site_projects.active_deployment_id,
                site_deployments.id AS deployment_id,
                site_deployments.status AS deployment_status,
                site_deployments.runtime_kind,
                site_deployments.runtime_script_name,
                site_deployments.dispatch_namespace,
                site_deployments.external_deployment_id,
                site_versions.id AS version_id,
                site_versions.build_status,
                site_versions.worker_module_r2_key,
                site_versions.static_assets_manifest_json,
                site_versions.d1_binding_name,
                site_versions.r2_binding_name
           FROM site_projects
           JOIN site_deployments
             ON site_deployments.id = site_projects.active_deployment_id
            AND site_deployments.site_id = site_projects.id
           JOIN site_versions
             ON site_versions.id = site_deployments.version_id
            AND site_versions.site_id = site_projects.id
          WHERE site_projects.team_id = ?
            AND site_projects.archived_at IS NULL
            AND site_projects.status NOT IN ('disabled', 'archived')
            AND site_projects.access_mode = 'public'
            AND site_projects.visibility = 'public'
            AND site_deployments.status = 'active'
            AND site_versions.build_status = 'saved'
          ORDER BY site_projects.updated_at DESC,
                   site_projects.created_at DESC
          LIMIT 1`,
      )
      .bind(teamId)
      .first<SiteRuntimeDeploymentRow>(),
  )

const isPublicActiveRuntimeRow = (row: SiteRuntimeDeploymentRow): boolean =>
  row.site_status !== 'disabled' &&
  row.site_status !== 'archived' &&
  row.access_mode === 'public' &&
  row.visibility === 'public' &&
  row.active_deployment_id !== null &&
  row.deployment_id === row.active_deployment_id &&
  row.deployment_status === 'active' &&
  row.build_status === 'saved'

const isPublicSavedVersionRuntimeRow = (
  row: SiteRuntimeDeploymentRow,
): boolean =>
  row.site_status !== 'disabled' &&
  row.site_status !== 'archived' &&
  row.access_mode === 'public' &&
  row.visibility === 'public' &&
  row.version_id !== null &&
  row.build_status === 'saved'

const isPublicActiveStaticRuntimeRow = (
  row: SiteRuntimeDeploymentRow,
): boolean =>
  isPublicActiveRuntimeRow(row) && row.runtime_kind === 'omega_static_r2'

const workerDeploymentFromRow = (
  row: SiteRuntimeDeploymentRow,
  requireActive: boolean = true,
): SiteRuntimeWorkerDeployment | null => {
  if (
    !(requireActive
      ? isPublicActiveRuntimeRow(row)
      : isPublicSavedVersionRuntimeRow(row)) ||
    row.runtime_kind !== 'workers_for_platforms' ||
    row.deployment_status === 'disabled' ||
    row.deployment_id === null ||
    row.dispatch_namespace === null ||
    row.runtime_script_name === null ||
    row.version_id === null ||
    row.worker_module_r2_key === null
  ) {
    return null
  }

  return {
    _tag: 'worker',
    deploymentId: row.deployment_id,
    dispatchNamespace: row.dispatch_namespace,
    d1BindingName: row.d1_binding_name,
    externalDeploymentId: row.external_deployment_id,
    r2BindingName: row.r2_binding_name,
    runtimeScriptName: row.runtime_script_name,
    siteId: row.site_id,
    slug: row.slug,
    versionId: row.version_id,
    workerModuleR2Key: row.worker_module_r2_key,
  }
}

const isPublicVersionStaticRuntimeRow = (
  row: SiteRuntimeDeploymentRow,
): boolean =>
  isPublicSavedVersionRuntimeRow(row) &&
  (row.runtime_kind === 'omega_static_r2' ||
    row.static_assets_manifest_json !== null)

const assetFromManifestEntry = (
  row: SiteRuntimeDeploymentRow,
  assetPath: string,
  entry: StaticManifestAsset,
): SiteRuntimeStaticAsset | null => {
  if (row.deployment_id === null || row.version_id === null) {
    return null
  }

  if (typeof entry === 'string') {
    return {
      _tag: 'static',
      assetPath,
      cacheControl: null,
      contentType: null,
      deploymentId: row.deployment_id,
      r2Key: entry,
      siteId: row.site_id,
      slug: row.slug,
      versionId: row.version_id,
    }
  }

  return {
    _tag: 'static',
    assetPath,
    cacheControl: optionalString(entry.cacheControl) ?? null,
    contentType: optionalString(entry.contentType) ?? null,
    deploymentId: row.deployment_id,
    r2Key: entry.r2Key,
    siteId: row.site_id,
    slug: row.slug,
    versionId: row.version_id,
  }
}

const resolveManifestAsset = (
  row: SiteRuntimeDeploymentRow,
  candidatePaths: ReadonlyArray<string>,
): SiteRuntimeStaticAsset | null => {
  const decoded = Option.getOrUndefined(
    decodeStaticAssetsManifest(
      parseJsonRecord(row.static_assets_manifest_json),
    ),
  )

  if (decoded === undefined) {
    return null
  }

  const candidatePath = candidatePaths.find(
    assetPath => decoded.assets[assetPath] !== undefined,
  )

  if (candidatePath === undefined) {
    return null
  }

  const entry = decoded.assets[candidatePath]

  return entry === undefined
    ? null
    : assetFromManifestEntry(row, candidatePath, entry)
}

const resolveStaticAsset = (
  db: D1Database,
  slug: string,
  candidatePaths: ReadonlyArray<string>,
): Effect.Effect<SiteRuntimeStaticAsset | null, SiteRuntimeStorageError> =>
  Effect.gen(function* () {
    const row = yield* readRuntimeDeployment(db, slug)

    if (row === null || !isPublicActiveStaticRuntimeRow(row)) {
      return null
    }

    return resolveManifestAsset(row, candidatePaths)
  })

const resolveRuntimeTarget = (
  db: D1Database,
  slug: string,
  candidatePaths: ReadonlyArray<string>,
): Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError> =>
  Effect.gen(function* () {
    const row = yield* readRuntimeDeployment(db, slug)

    if (row === null) {
      return null
    }

    const workerDeployment = workerDeploymentFromRow(row)

    if (workerDeployment !== null) {
      return workerDeployment
    }

    if (!isPublicActiveStaticRuntimeRow(row)) {
      return null
    }

    return resolveManifestAsset(row, candidatePaths)
  })

const resolveRuntimeTargetForTeam = (
  db: D1Database,
  teamId: string,
  candidatePaths: ReadonlyArray<string>,
): Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError> =>
  Effect.gen(function* () {
    const row = yield* readRuntimeDeploymentForTeam(db, teamId)

    if (row === null) {
      return null
    }

    const workerDeployment = workerDeploymentFromRow(row)

    if (workerDeployment !== null) {
      return workerDeployment
    }

    if (!isPublicActiveStaticRuntimeRow(row)) {
      return null
    }

    return resolveManifestAsset(row, candidatePaths)
  })

const resolveVersionRuntimeTarget = (
  db: D1Database,
  slug: string,
  versionId: string,
  candidatePaths: ReadonlyArray<string>,
): Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError> =>
  Effect.gen(function* () {
    const row = yield* readRuntimeVersion(db, slug, versionId)

    if (row === null) {
      return null
    }

    const workerDeployment = workerDeploymentFromRow(row, false)

    if (workerDeployment !== null) {
      return workerDeployment
    }

    if (!isPublicVersionStaticRuntimeRow(row)) {
      return null
    }

    return resolveManifestAsset(row, candidatePaths)
  })

const readArtifactObject = (
  artifacts: R2Bucket,
  asset: SiteRuntimeStaticAsset,
): Effect.Effect<R2ObjectBody | null, SiteRuntimeStorageError> =>
  r2Effect('siteRuntime.artifacts.get', () => artifacts.get(asset.r2Key))

export class SiteRuntimeService extends Context.Service<
  SiteRuntimeService,
  {
    readonly readArtifactObject: (
      asset: SiteRuntimeStaticAsset,
    ) => Effect.Effect<R2ObjectBody | null, SiteRuntimeStorageError>
    readonly resolveStaticAsset: (
      slug: string,
      candidatePaths: ReadonlyArray<string>,
    ) => Effect.Effect<SiteRuntimeStaticAsset | null, SiteRuntimeStorageError>
    readonly resolveRuntimeTarget: (
      slug: string,
      candidatePaths: ReadonlyArray<string>,
    ) => Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError>
    readonly resolveRuntimeTargetForTeam: (
      teamId: string,
      candidatePaths: ReadonlyArray<string>,
    ) => Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError>
    readonly resolveVersionRuntimeTarget: (
      slug: string,
      versionId: string,
      candidatePaths: ReadonlyArray<string>,
    ) => Effect.Effect<SiteRuntimeTarget | null, SiteRuntimeStorageError>
  }
>()('@openagentsinc/autopilot-omega/SiteRuntimeService') {
  static readonly layer = (env: SiteRuntimeEnv) =>
    Layer.succeed(SiteRuntimeService, {
      readArtifactObject: Effect.fn('SiteRuntimeService.readArtifactObject')(
        asset => readArtifactObject(artifactsBucketForEnv(env), asset),
      ),
      resolveStaticAsset: Effect.fn('SiteRuntimeService.resolveStaticAsset')(
        (slug, candidatePaths) =>
          resolveStaticAsset(openAgentsDatabase(env), slug, candidatePaths),
      ),
      resolveRuntimeTarget: Effect.fn(
        'SiteRuntimeService.resolveRuntimeTarget',
      )((slug, candidatePaths) =>
        resolveRuntimeTarget(openAgentsDatabase(env), slug, candidatePaths),
      ),
      resolveRuntimeTargetForTeam: Effect.fn(
        'SiteRuntimeService.resolveRuntimeTargetForTeam',
      )((teamId, candidatePaths) =>
        resolveRuntimeTargetForTeam(
          openAgentsDatabase(env),
          teamId,
          candidatePaths,
        ),
      ),
      resolveVersionRuntimeTarget: Effect.fn(
        'SiteRuntimeService.resolveVersionRuntimeTarget',
      )((slug, versionId, candidatePaths) =>
        resolveVersionRuntimeTarget(
          openAgentsDatabase(env),
          slug,
          versionId,
          candidatePaths,
        ),
      ),
    })
}
