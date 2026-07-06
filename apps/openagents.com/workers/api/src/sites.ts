import {
  containsProviderSecretMaterial,
  redactProviderAccountSecretMaterial,
} from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { artifactsBucketForEnv } from './artifacts-binding'
import { parseJsonRecord } from './json-boundary'
// KS-8.12 (#8323): the AutopilotSitesService layer is the central db seam
// for sites.ts writes — acquire the dual-write mirroring database here so
// every layer-provided caller (operator sites/triage routes, adjutant run
// lifecycle) mirrors scoped writes into Postgres. Passthrough for
// non-scoped statements; degrades to the raw D1 handle when no
// KHALA_SYNC_DB binding exists.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type AutopilotSitesEnv = Readonly<{
  ARTIFACTS?: R2Bucket
  OPENAGENTS_DB: D1Database
}>

export type AutopilotSitesRuntime = Readonly<{
  makeDeploymentId: () => string
  makeDeploymentAttemptId: () => string
  makeEventId: () => string
  makeSiteId: () => string
  makeVersionId: () => string
  nowIso: () => string
}>

export const systemAutopilotSitesRuntime: AutopilotSitesRuntime = {
  makeDeploymentId: () => compactRandomId('site_deployment'),
  makeDeploymentAttemptId: () => compactRandomId('site_deployment_attempt'),
  makeEventId: () => compactRandomId('site_event'),
  makeSiteId: () => compactRandomId('site_project'),
  makeVersionId: () => compactRandomId('site_version'),
  nowIso: currentIsoTimestamp,
}

export const AutopilotSiteStatus = S.Literals([
  'draft',
  'generating',
  'generated',
  'needs_review',
  'approved',
  'archived',
  'disabled',
])
export type AutopilotSiteStatus = typeof AutopilotSiteStatus.Type

export const AutopilotSiteAccessMode = S.Literals([
  'owner_admins',
  'openagents_core',
  'customer_owner',
  'custom_users',
  'public',
])
export type AutopilotSiteAccessMode = typeof AutopilotSiteAccessMode.Type

export const AutopilotSiteVisibility = S.Literals([
  'private',
  'team',
  'public',
])
export type AutopilotSiteVisibility = typeof AutopilotSiteVisibility.Type

export const AutopilotSiteVersionSourceKind = S.Literals([
  'autopilot_generated',
  'github_import',
  'operator_static',
])
export type AutopilotSiteVersionSourceKind =
  typeof AutopilotSiteVersionSourceKind.Type

export const AutopilotSiteVersionBuildStatus = S.Literals([
  'planned',
  'building',
  'build_failed',
  'saved',
  'rejected',
  'superseded',
])
export type AutopilotSiteVersionBuildStatus =
  typeof AutopilotSiteVersionBuildStatus.Type

export const AutopilotSiteDeploymentStatus = S.Literals([
  'queued',
  'deploying',
  'active',
  'failed',
  'disabled',
  'rolled_back',
])
export type AutopilotSiteDeploymentStatus =
  typeof AutopilotSiteDeploymentStatus.Type

export const AutopilotSiteRuntimeKind = S.Literals([
  'omega_static_r2',
  'workers_for_platforms',
])
export type AutopilotSiteRuntimeKind = typeof AutopilotSiteRuntimeKind.Type

export const AutopilotSiteEnvironmentValueKind = S.Literals([
  'plain',
  'secret',
])
export type AutopilotSiteEnvironmentValueKind =
  typeof AutopilotSiteEnvironmentValueKind.Type

export const AutopilotSiteAccessPrincipalKind = S.Literals([
  'user',
  'team',
  'admin',
  'public',
])
export type AutopilotSiteAccessPrincipalKind =
  typeof AutopilotSiteAccessPrincipalKind.Type

export const AutopilotSiteAccessRole = S.Literals(['owner', 'admin', 'viewer'])
export type AutopilotSiteAccessRole = typeof AutopilotSiteAccessRole.Type

export const AutopilotSiteStaticManifestAsset = S.Struct({
  r2Key: S.String,
  cacheControl: S.optionalKey(S.String),
  contentType: S.optionalKey(S.String),
})
export type AutopilotSiteStaticManifestAsset =
  typeof AutopilotSiteStaticManifestAsset.Type

export const AutopilotSiteStaticAssetsManifest = S.Struct({
  assets: S.Record(S.String, AutopilotSiteStaticManifestAsset),
})
export type AutopilotSiteStaticAssetsManifest =
  typeof AutopilotSiteStaticAssetsManifest.Type

export const AutopilotSiteSourceRepository = S.Struct({
  provider: S.Literal('github'),
  owner: S.String,
  name: S.String,
  ref: S.String,
})
export type AutopilotSiteSourceRepository =
  typeof AutopilotSiteSourceRepository.Type

export const AutopilotSiteProject = S.Struct({
  id: S.String,
  softwareOrderId: S.NullOr(S.String),
  ownerUserId: S.String,
  teamId: S.NullOr(S.String),
  projectId: S.NullOr(S.String),
  slug: S.String,
  title: S.String,
  prompt: S.String,
  status: AutopilotSiteStatus,
  accessMode: AutopilotSiteAccessMode,
  visibility: AutopilotSiteVisibility,
  sourceRepository: S.NullOr(AutopilotSiteSourceRepository),
  activeVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  archivedAt: S.NullOr(S.String),
})
export type AutopilotSiteProject = typeof AutopilotSiteProject.Type

export const AutopilotSiteVersion = S.Struct({
  id: S.String,
  siteId: S.String,
  sourceKind: AutopilotSiteVersionSourceKind,
  sourceCommitSha: S.NullOr(S.String),
  sourceArchiveR2Key: S.NullOr(S.String),
  artifactManifestR2Key: S.NullOr(S.String),
  buildLogR2Key: S.NullOr(S.String),
  buildStatus: AutopilotSiteVersionBuildStatus,
  buildCommand: S.NullOr(S.String),
  workerModuleR2Key: S.NullOr(S.String),
  staticAssetsManifest: AutopilotSiteStaticAssetsManifest,
  d1BindingName: S.NullOr(S.String),
  r2BindingName: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  createdByUserId: S.NullOr(S.String),
  createdByRunId: S.NullOr(S.String),
  createdAt: S.String,
  savedAt: S.NullOr(S.String),
  rejectedAt: S.NullOr(S.String),
})
export type AutopilotSiteVersion = typeof AutopilotSiteVersion.Type

export const AutopilotSiteDeployment = S.Struct({
  id: S.String,
  siteId: S.String,
  versionId: S.String,
  slug: S.String,
  url: S.String,
  runtimeKind: AutopilotSiteRuntimeKind,
  runtimeScriptName: S.NullOr(S.String),
  dispatchNamespace: S.NullOr(S.String),
  status: AutopilotSiteDeploymentStatus,
  deployedByUserId: S.NullOr(S.String),
  externalDeploymentId: S.NullOr(S.String),
  startedAt: S.NullOr(S.String),
  activatedAt: S.NullOr(S.String),
  failedAt: S.NullOr(S.String),
  disabledAt: S.NullOr(S.String),
  rolledBackAt: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
})
export type AutopilotSiteDeployment = typeof AutopilotSiteDeployment.Type

export const AutopilotSiteEnvironmentValue = S.Struct({
  id: S.String,
  siteId: S.String,
  key: S.String,
  kind: AutopilotSiteEnvironmentValueKind,
  secretRef: S.NullOr(S.String),
  plainValue: S.NullOr(S.String),
  createdAt: S.String,
  updatedAt: S.String,
  deletedAt: S.NullOr(S.String),
})
export type AutopilotSiteEnvironmentValue =
  typeof AutopilotSiteEnvironmentValue.Type

export const AutopilotSiteAccessGrant = S.Struct({
  id: S.String,
  siteId: S.String,
  principalKind: AutopilotSiteAccessPrincipalKind,
  principalRef: S.String,
  role: AutopilotSiteAccessRole,
  createdAt: S.String,
  revokedAt: S.NullOr(S.String),
})
export type AutopilotSiteAccessGrant = typeof AutopilotSiteAccessGrant.Type

export const AutopilotSiteEvent = S.Struct({
  id: S.String,
  siteId: S.String,
  versionId: S.NullOr(S.String),
  deploymentId: S.NullOr(S.String),
  type: S.String,
  summary: S.String,
  actorUserId: S.NullOr(S.String),
  actorRunId: S.NullOr(S.String),
  createdAt: S.String,
})
export type AutopilotSiteEvent = typeof AutopilotSiteEvent.Type

export const AutopilotSiteLaunchChecklist = S.Struct({
  sourceReviewed: S.Boolean,
  buildReviewed: S.Boolean,
  audienceReviewed: S.Boolean,
  secretsReviewed: S.Boolean,
  urlReviewed: S.Boolean,
})
export type AutopilotSiteLaunchChecklist =
  typeof AutopilotSiteLaunchChecklist.Type

export const CreateAutopilotSiteFromOrderInput = S.Struct({
  softwareOrderId: S.String,
  slug: S.String,
  title: S.String,
  accessMode: S.optionalKey(AutopilotSiteAccessMode),
  actorUserId: S.optionalKey(S.String),
  projectId: S.optionalKey(S.String),
  teamId: S.optionalKey(S.String),
  visibility: S.optionalKey(AutopilotSiteVisibility),
})
export type CreateAutopilotSiteFromOrderInput =
  typeof CreateAutopilotSiteFromOrderInput.Type

export const MarkAutopilotSiteActiveDeploymentInput = S.Struct({
  siteId: S.String,
  versionId: S.String,
  deploymentId: S.String,
  actorUserId: S.optionalKey(S.String),
})
export type MarkAutopilotSiteActiveDeploymentInput =
  typeof MarkAutopilotSiteActiveDeploymentInput.Type

export const RecordAutopilotSiteEventInput = S.Struct({
  siteId: S.String,
  type: S.String,
  summary: S.String,
  actorUserId: S.optionalKey(S.String),
  actorRunId: S.optionalKey(S.String),
  deploymentId: S.optionalKey(S.String),
  payload: S.optionalKey(S.Unknown),
  versionId: S.optionalKey(S.String),
})
export type RecordAutopilotSiteEventInput =
  typeof RecordAutopilotSiteEventInput.Type

export const UpdateAutopilotSiteAccessInput = S.Struct({
  siteId: S.String,
  accessMode: AutopilotSiteAccessMode,
  visibility: AutopilotSiteVisibility,
  actorUserId: S.optionalKey(S.String),
  launchChecklist: S.optionalKey(AutopilotSiteLaunchChecklist),
})
export type UpdateAutopilotSiteAccessInput =
  typeof UpdateAutopilotSiteAccessInput.Type

export const SaveAutopilotSiteVersionInput = S.Struct({
  siteId: S.String,
  sourceKind: AutopilotSiteVersionSourceKind,
  buildStatus: S.Literals(['build_failed', 'saved']),
  staticAssetsManifest: AutopilotSiteStaticAssetsManifest,
  actorRunId: S.optionalKey(S.String),
  actorUserId: S.optionalKey(S.String),
  buildCommand: S.optionalKey(S.String),
  buildLogText: S.optionalKey(S.String),
  d1BindingName: S.optionalKey(S.String),
  metadata: S.optionalKey(S.Record(S.String, S.Unknown)),
  r2BindingName: S.optionalKey(S.String),
  sourceArchiveText: S.optionalKey(S.String),
  sourceCommitSha: S.optionalKey(S.String),
  workerModuleText: S.optionalKey(S.String),
  workerModuleR2Key: S.optionalKey(S.String),
})
export type SaveAutopilotSiteVersionInput =
  typeof SaveAutopilotSiteVersionInput.Type

export const DeployAutopilotSiteVersionInput = S.Struct({
  siteId: S.String,
  versionId: S.String,
  actorUserId: S.optionalKey(S.String),
  dispatchNamespace: S.optionalKey(S.String),
  externalDeploymentId: S.optionalKey(S.String),
  healthCheck: S.optionalKey(
    S.Struct({
      status: S.Literals(['passed', 'failed']),
      checkedAt: S.optionalKey(S.String),
      healthRef: S.optionalKey(S.String),
      summary: S.optionalKey(S.String),
      url: S.optionalKey(S.String),
    }),
  ),
  launchChecklist: S.optionalKey(AutopilotSiteLaunchChecklist),
  observabilityRef: S.optionalKey(S.String),
  rollbackRef: S.optionalKey(S.String),
  runtimeKind: S.optionalKey(AutopilotSiteRuntimeKind),
  runtimeScriptName: S.optionalKey(S.String),
  tags: S.optionalKey(S.Array(S.String)),
  uploadReceiptRef: S.optionalKey(S.String),
})
export type DeployAutopilotSiteVersionInput =
  typeof DeployAutopilotSiteVersionInput.Type

export const UpsertAutopilotSiteEnvironmentValueInput = S.Struct({
  siteId: S.String,
  key: S.String,
  kind: AutopilotSiteEnvironmentValueKind,
  actorUserId: S.optionalKey(S.String),
  plainValue: S.optionalKey(S.String),
  secretRef: S.optionalKey(S.String),
})
export type UpsertAutopilotSiteEnvironmentValueInput =
  typeof UpsertAutopilotSiteEnvironmentValueInput.Type

export const GrantAutopilotSiteAccessInput = S.Struct({
  siteId: S.String,
  principalKind: AutopilotSiteAccessPrincipalKind,
  principalRef: S.String,
  role: AutopilotSiteAccessRole,
  actorUserId: S.optionalKey(S.String),
})
export type GrantAutopilotSiteAccessInput =
  typeof GrantAutopilotSiteAccessInput.Type

export const DisableAutopilotSiteDeploymentInput = S.Struct({
  siteId: S.String,
  deploymentId: S.String,
  actorUserId: S.optionalKey(S.String),
})
export type DisableAutopilotSiteDeploymentInput =
  typeof DisableAutopilotSiteDeploymentInput.Type

export const RollbackAutopilotSiteDeploymentInput = S.Struct({
  siteId: S.String,
  deploymentId: S.String,
  actorUserId: S.optionalKey(S.String),
})
export type RollbackAutopilotSiteDeploymentInput =
  typeof RollbackAutopilotSiteDeploymentInput.Type

export const RequestAutopilotSiteGenerationInput = S.Struct({
  siteId: S.String,
  actorRunId: S.optionalKey(S.String),
  actorUserId: S.optionalKey(S.String),
  operatorNotes: S.optionalKey(S.String),
})
export type RequestAutopilotSiteGenerationInput =
  typeof RequestAutopilotSiteGenerationInput.Type

export const AutopilotSiteGenerationPacket = S.Struct({
  siteId: S.String,
  softwareOrderId: S.NullOr(S.String),
  slug: S.String,
  title: S.String,
  publicUrl: S.String,
  objective: S.String,
  generationGoal: S.String,
  outputContract: S.Array(S.String),
  preflightChecklist: S.Array(S.String),
  sourceRepository: S.NullOr(AutopilotSiteSourceRepository),
  operatorNotes: S.NullOr(S.String),
  createdAt: S.String,
})
export type AutopilotSiteGenerationPacket =
  typeof AutopilotSiteGenerationPacket.Type

type AutopilotSiteProjectRow = Readonly<{
  id: string
  software_order_id: string | null
  owner_user_id: string
  team_id: string | null
  project_id: string | null
  slug: string
  title: string
  prompt: string
  status: AutopilotSiteStatus
  access_mode: AutopilotSiteAccessMode
  visibility: AutopilotSiteVisibility
  source_repository_provider: 'github' | null
  source_repository_owner: string | null
  source_repository_name: string | null
  source_repository_ref: string | null
  active_version_id: string | null
  active_deployment_id: string | null
  created_at: string
  updated_at: string
  archived_at: string | null
}>

type SoftwareOrderSiteSourceRow = Readonly<{
  id: string
  user_id: string
  request: string
  repository_provider: 'github' | null
  repository_owner: string | null
  repository_name: string | null
  repository_default_branch: string | null
  archived_at: string | null
}>

type AutopilotSiteVersionRow = Readonly<{
  id: string
  site_id: string
  source_kind: AutopilotSiteVersionSourceKind
  source_commit_sha: string | null
  source_archive_r2_key: string | null
  artifact_manifest_r2_key: string | null
  build_log_r2_key: string | null
  build_status: AutopilotSiteVersionBuildStatus
  build_command: string | null
  worker_module_r2_key: string | null
  static_assets_manifest_json: string
  d1_binding_name: string | null
  r2_binding_name: string | null
  metadata_json: string
  created_by_user_id: string | null
  created_by_run_id: string | null
  created_at: string
  saved_at: string | null
  rejected_at: string | null
}>

type AutopilotSiteDeploymentRow = Readonly<{
  id: string
  site_id: string
  version_id: string
  slug: string
  url: string
  runtime_kind: AutopilotSiteRuntimeKind
  runtime_script_name: string | null
  dispatch_namespace: string | null
  status: AutopilotSiteDeploymentStatus
  deployed_by_user_id: string | null
  external_deployment_id: string | null
  started_at: string | null
  activated_at: string | null
  failed_at: string | null
  disabled_at: string | null
  rolled_back_at: string | null
  created_at: string
  updated_at: string
}>

type AutopilotSiteEventRow = Readonly<{
  id: string
  site_id: string
  version_id: string | null
  deployment_id: string | null
  type: string
  summary: string
  actor_user_id: string | null
  actor_run_id: string | null
  created_at: string
}>

export class AutopilotSiteStorageError extends S.TaggedErrorClass<AutopilotSiteStorageError>()(
  'AutopilotSiteStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class AutopilotSiteSoftwareOrderNotFound extends S.TaggedErrorClass<AutopilotSiteSoftwareOrderNotFound>()(
  'AutopilotSiteSoftwareOrderNotFound',
  {
    softwareOrderId: S.String,
  },
) {}

export class AutopilotSiteProjectNotFound extends S.TaggedErrorClass<AutopilotSiteProjectNotFound>()(
  'AutopilotSiteProjectNotFound',
  {
    siteId: S.String,
  },
) {}

export class AutopilotSiteVersionNotFound extends S.TaggedErrorClass<AutopilotSiteVersionNotFound>()(
  'AutopilotSiteVersionNotFound',
  {
    siteId: S.String,
    versionId: S.String,
  },
) {}

export class AutopilotSiteDeploymentNotFound extends S.TaggedErrorClass<AutopilotSiteDeploymentNotFound>()(
  'AutopilotSiteDeploymentNotFound',
  {
    deploymentId: S.String,
    siteId: S.String,
  },
) {}

export class AutopilotSiteVersionNotDeployable extends S.TaggedErrorClass<AutopilotSiteVersionNotDeployable>()(
  'AutopilotSiteVersionNotDeployable',
  {
    buildStatus: AutopilotSiteVersionBuildStatus,
    versionId: S.String,
  },
) {}

export class AutopilotSiteRuntimeNotDeployable extends S.TaggedErrorClass<AutopilotSiteRuntimeNotDeployable>()(
  'AutopilotSiteRuntimeNotDeployable',
  {
    reason: S.String,
    siteId: S.String,
    versionId: S.String,
  },
) {}

export class AutopilotSiteLaunchChecklistRequired extends S.TaggedErrorClass<AutopilotSiteLaunchChecklistRequired>()(
  'AutopilotSiteLaunchChecklistRequired',
  {
    reason: S.String,
    siteId: S.String,
  },
) {}

export class AutopilotSiteSlugUnavailable extends S.TaggedErrorClass<AutopilotSiteSlugUnavailable>()(
  'AutopilotSiteSlugUnavailable',
  {
    slug: S.String,
  },
) {}

export class AutopilotSiteUnsafePayload extends S.TaggedErrorClass<AutopilotSiteUnsafePayload>()(
  'AutopilotSiteUnsafePayload',
  {
    reason: S.String,
  },
) {}

export type AutopilotSiteError =
  | AutopilotSiteDeploymentNotFound
  | AutopilotSiteLaunchChecklistRequired
  | AutopilotSiteProjectNotFound
  | AutopilotSiteRuntimeNotDeployable
  | AutopilotSiteSlugUnavailable
  | AutopilotSiteSoftwareOrderNotFound
  | AutopilotSiteStorageError
  | AutopilotSiteUnsafePayload
  | AutopilotSiteVersionNotDeployable
  | AutopilotSiteVersionNotFound

const SITE_EVENT_PAYLOAD_LIMIT_BYTES = 20_000
const SITE_BUILD_LOG_LIMIT_BYTES = 100_000

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AutopilotSiteStorageError> =>
  Effect.tryPromise({
    catch: error => new AutopilotSiteStorageError({ error, operation }),
    try: run,
  })

const r2Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, AutopilotSiteStorageError> =>
  Effect.tryPromise({
    catch: error => new AutopilotSiteStorageError({ error, operation }),
    try: run,
  })

const sourceRepositoryFromProjectRow = (
  row: AutopilotSiteProjectRow,
): AutopilotSiteSourceRepository | null => {
  if (
    row.source_repository_provider !== 'github' ||
    row.source_repository_owner === null ||
    row.source_repository_name === null
  ) {
    return null
  }

  return {
    provider: 'github',
    owner: row.source_repository_owner,
    name: row.source_repository_name,
    ref: row.source_repository_ref ?? 'main',
  }
}

const sourceRepositoryFromOrderRow = (
  row: SoftwareOrderSiteSourceRow,
): AutopilotSiteSourceRepository | null => {
  if (
    row.repository_provider !== 'github' ||
    row.repository_owner === null ||
    row.repository_name === null
  ) {
    return null
  }

  return {
    provider: 'github',
    owner: row.repository_owner,
    name: row.repository_name,
    ref: row.repository_default_branch ?? 'main',
  }
}

const projectFromRow = (
  row: AutopilotSiteProjectRow,
): AutopilotSiteProject => ({
  id: row.id,
  softwareOrderId: row.software_order_id,
  ownerUserId: row.owner_user_id,
  teamId: row.team_id,
  projectId: row.project_id,
  slug: row.slug,
  title: row.title,
  prompt: row.prompt,
  status: row.status,
  accessMode: row.access_mode,
  visibility: row.visibility,
  sourceRepository: sourceRepositoryFromProjectRow(row),
  activeVersionId: row.active_version_id,
  activeDeploymentId: row.active_deployment_id,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  archivedAt: row.archived_at,
})

const staticAssetsManifestFromJson = (
  value: string,
): AutopilotSiteStaticAssetsManifest =>
  S.decodeUnknownSync(AutopilotSiteStaticAssetsManifest)(
    parseJsonRecord(value) ?? { assets: {} },
  )

const versionFromRow = (row: AutopilotSiteVersionRow): AutopilotSiteVersion => ({
  id: row.id,
  siteId: row.site_id,
  sourceKind: row.source_kind,
  sourceCommitSha: row.source_commit_sha,
  sourceArchiveR2Key: row.source_archive_r2_key,
  artifactManifestR2Key: row.artifact_manifest_r2_key,
  buildLogR2Key: row.build_log_r2_key,
  buildStatus: row.build_status,
  buildCommand: row.build_command,
  workerModuleR2Key: row.worker_module_r2_key,
  staticAssetsManifest: staticAssetsManifestFromJson(
    row.static_assets_manifest_json,
  ),
  d1BindingName: row.d1_binding_name,
  r2BindingName: row.r2_binding_name,
  metadata: parseJsonRecord(row.metadata_json) ?? {},
  createdByUserId: row.created_by_user_id,
  createdByRunId: row.created_by_run_id,
  createdAt: row.created_at,
  savedAt: row.saved_at,
  rejectedAt: row.rejected_at,
})

const deploymentFromRow = (
  row: AutopilotSiteDeploymentRow,
): AutopilotSiteDeployment => ({
  id: row.id,
  siteId: row.site_id,
  versionId: row.version_id,
  slug: row.slug,
  url: row.url,
  runtimeKind: row.runtime_kind,
  runtimeScriptName: row.runtime_script_name,
  dispatchNamespace: row.dispatch_namespace,
  status: row.status,
  deployedByUserId: row.deployed_by_user_id,
  externalDeploymentId: row.external_deployment_id,
  startedAt: row.started_at,
  activatedAt: row.activated_at,
  failedAt: row.failed_at,
  disabledAt: row.disabled_at,
  rolledBackAt: row.rolled_back_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const eventFromRow = (row: AutopilotSiteEventRow): AutopilotSiteEvent => ({
  id: row.id,
  siteId: row.site_id,
  versionId: row.version_id,
  deploymentId: row.deployment_id,
  type: row.type,
  summary: row.summary,
  actorUserId: row.actor_user_id,
  actorRunId: row.actor_run_id,
  createdAt: row.created_at,
})

const projectSelectColumns = `id,
       software_order_id,
       owner_user_id,
       team_id,
       project_id,
       slug,
       title,
       prompt,
       status,
       access_mode,
       visibility,
       source_repository_provider,
       source_repository_owner,
       source_repository_name,
       source_repository_ref,
       active_version_id,
       active_deployment_id,
       created_at,
       updated_at,
       archived_at`

const versionSelectColumns = `id,
       site_id,
       source_kind,
       source_commit_sha,
       source_archive_r2_key,
       artifact_manifest_r2_key,
       build_log_r2_key,
       build_status,
       build_command,
       worker_module_r2_key,
       static_assets_manifest_json,
       d1_binding_name,
       r2_binding_name,
       metadata_json,
       created_by_user_id,
       created_by_run_id,
       created_at,
       saved_at,
       rejected_at`

const deploymentSelectColumns = `id,
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
       updated_at`

const readProjectByOrder = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<AutopilotSiteProject | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.projects.readByOrder', () =>
    db
      .prepare(
        `SELECT ${projectSelectColumns}
           FROM site_projects
          WHERE software_order_id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<AutopilotSiteProjectRow>(),
  ).pipe(Effect.map(row => (row === null ? null : projectFromRow(row))))

const readProjectBySlug = (
  db: D1Database,
  slug: string,
): Effect.Effect<AutopilotSiteProject | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.projects.readBySlug', () =>
    db
      .prepare(
        `SELECT ${projectSelectColumns}
           FROM site_projects
          WHERE slug = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(slug)
      .first<AutopilotSiteProjectRow>(),
  ).pipe(Effect.map(row => (row === null ? null : projectFromRow(row))))

const readProjectById = (
  db: D1Database,
  siteId: string,
): Effect.Effect<AutopilotSiteProject | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.projects.readById', () =>
    db
      .prepare(
        `SELECT ${projectSelectColumns}
           FROM site_projects
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(siteId)
      .first<AutopilotSiteProjectRow>(),
  ).pipe(Effect.map(row => (row === null ? null : projectFromRow(row))))

const readVersionById = (
  db: D1Database,
  siteId: string,
  versionId: string,
): Effect.Effect<AutopilotSiteVersion | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.versions.readById', () =>
    db
      .prepare(
        `SELECT ${versionSelectColumns}
           FROM site_versions
          WHERE id = ?
            AND site_id = ?
          LIMIT 1`,
      )
      .bind(versionId, siteId)
      .first<AutopilotSiteVersionRow>(),
  ).pipe(Effect.map(row => (row === null ? null : versionFromRow(row))))

const readDeploymentById = (
  db: D1Database,
  siteId: string,
  deploymentId: string,
): Effect.Effect<AutopilotSiteDeployment | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.deployments.readById', () =>
    db
      .prepare(
        `SELECT ${deploymentSelectColumns}
           FROM site_deployments
          WHERE id = ?
            AND site_id = ?
          LIMIT 1`,
      )
      .bind(deploymentId, siteId)
      .first<AutopilotSiteDeploymentRow>(),
  ).pipe(Effect.map(row => (row === null ? null : deploymentFromRow(row))))

const listProjects = (
  db: D1Database,
  limit: number,
): Effect.Effect<ReadonlyArray<AutopilotSiteProject>, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.projects.list', () =>
    db
      .prepare(
        `SELECT ${projectSelectColumns}
           FROM site_projects
          WHERE archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<AutopilotSiteProjectRow>(),
  ).pipe(Effect.map(result => result.results.map(projectFromRow)))

const readSoftwareOrder = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<SoftwareOrderSiteSourceRow | null, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.softwareOrders.read', () =>
    db
      .prepare(
        `SELECT id,
                user_id,
                request,
                repository_provider,
                repository_owner,
                repository_name,
                repository_default_branch,
                archived_at
           FROM software_orders
          WHERE id = ?
            AND archived_at IS NULL
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<SoftwareOrderSiteSourceRow>(),
  )

const listEvents = (
  db: D1Database,
  siteId: string,
  limit: number,
): Effect.Effect<ReadonlyArray<AutopilotSiteEvent>, AutopilotSiteStorageError> =>
  d1Effect('autopilotSites.events.list', () =>
    db
      .prepare(
        `SELECT id,
                site_id,
                version_id,
                deployment_id,
                type,
                summary,
                actor_user_id,
                actor_run_id,
                created_at
           FROM site_events
          WHERE site_id = ?
          ORDER BY created_at DESC
          LIMIT ?`,
      )
      .bind(siteId, limit)
      .all<AutopilotSiteEventRow>(),
  ).pipe(Effect.map(result => result.results.map(eventFromRow)))

const eventPayloadJson = (
  payload: unknown | undefined,
): Effect.Effect<string | null, AutopilotSiteUnsafePayload> =>
  Effect.gen(function* () {
    if (payload === undefined) {
      return null
    }

    const json = yield* Effect.try({
      catch: error =>
        new AutopilotSiteUnsafePayload({
          reason:
            error instanceof Error ? error.message : 'invalid event payload',
        }),
      try: () => JSON.stringify(payload),
    })

    if (json.length > SITE_EVENT_PAYLOAD_LIMIT_BYTES) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site event payload is too large.',
      })
    }

    if (containsProviderSecretMaterial(json)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site event payload contains secret-shaped material.',
      })
    }

    return json
  })

const storageJson = (
  value: unknown,
): Effect.Effect<string, AutopilotSiteUnsafePayload> =>
  eventPayloadJson(value).pipe(Effect.map(json => json ?? '{}'))

const truncatedBuildLog = (value: string): string =>
  value.length <= SITE_BUILD_LOG_LIMIT_BYTES
    ? value
    : `${value.slice(0, SITE_BUILD_LOG_LIMIT_BYTES)}\n[TRUNCATED]`

const redactedBuildLog = (value: string | undefined): string | null =>
  value === undefined
    ? null
    : truncatedBuildLog(redactProviderAccountSecretMaterial(value))

const rejectSecretText = (
  value: string | undefined,
  reason: string,
): Effect.Effect<string | null, AutopilotSiteUnsafePayload> =>
  Effect.gen(function* () {
    if (value === undefined) {
      return null
    }

    if (containsProviderSecretMaterial(value)) {
      return yield* new AutopilotSiteUnsafePayload({ reason })
    }

    return value
  })

const requiredArtifactsBucket = (
  artifacts: R2Bucket | undefined,
): Effect.Effect<R2Bucket, AutopilotSiteStorageError> =>
  artifacts === undefined
    ? Effect.fail(
        new AutopilotSiteStorageError({
          error: 'ARTIFACTS binding is required for Sites version storage.',
          operation: 'autopilotSites.artifacts.require',
        }),
      )
    : Effect.succeed(artifacts)

const putOptionalArtifactText = (
  artifacts: R2Bucket,
  key: string,
  value: string | null,
): Effect.Effect<string | null, AutopilotSiteStorageError> =>
  value === null
    ? Effect.succeed(null)
    : r2Effect('autopilotSites.artifacts.put', () =>
        artifacts.put(key, value).then(() => key),
      )

const putRequiredArtifactText = (
  artifacts: R2Bucket,
  key: string,
  value: string,
): Effect.Effect<string, AutopilotSiteStorageError> =>
  r2Effect('autopilotSites.artifacts.put', () =>
    artifacts.put(key, value).then(() => key),
  )

const storageBindingId = (
  siteId: string,
  kind: 'd1' | 'r2',
  bindingName: string,
): string => `site_storage_binding:${siteId}:${kind}:${bindingName}`

const upsertStorageBinding = (
  db: D1Database,
  siteId: string,
  kind: 'd1' | 'r2',
  bindingName: string | null,
  now: string,
): Effect.Effect<void, AutopilotSiteStorageError> =>
  bindingName === null
    ? Effect.void
    : d1Effect('autopilotSites.storageBindings.upsert', () =>
        db
          .prepare(
            `INSERT INTO site_storage_bindings
               (id,
                site_id,
                kind,
                binding_name,
                cloudflare_resource_ref,
                scope,
                created_at,
                updated_at)
             VALUES (?, ?, ?, ?, NULL, 'shared_prefix', ?, ?)
             ON CONFLICT(site_id, kind, binding_name)
             DO UPDATE SET updated_at = excluded.updated_at`,
          )
          .bind(
            storageBindingId(siteId, kind, bindingName),
            siteId,
            kind,
            bindingName,
            now,
            now,
          )
          .run()
          .then(() => undefined),
      )

const launchChecklistComplete = (
  checklist: AutopilotSiteLaunchChecklist | undefined,
): boolean =>
  checklist?.sourceReviewed === true &&
  checklist.buildReviewed === true &&
  checklist.audienceReviewed === true &&
  checklist.secretsReviewed === true &&
  checklist.urlReviewed === true

const requirePublicLaunchChecklist = (
  project: AutopilotSiteProject,
  checklist: AutopilotSiteLaunchChecklist | undefined,
  reason: string,
): Effect.Effect<void, AutopilotSiteLaunchChecklistRequired> =>
  project.accessMode === 'public' || project.visibility === 'public'
    ? launchChecklistComplete(checklist)
      ? Effect.void
      : Effect.fail(
          new AutopilotSiteLaunchChecklistRequired({
            reason,
            siteId: project.id,
          }),
        )
    : Effect.void

const environmentValueId = (siteId: string, key: string): string =>
  `site_environment_value:${siteId}:${key}`

const accessGrantId = (
  siteId: string,
  principalKind: AutopilotSiteAccessPrincipalKind,
  principalRef: string,
  role: AutopilotSiteAccessRole,
): string => `site_access_grant:${siteId}:${principalKind}:${principalRef}:${role}`

const nonEmptyOptionalInput = (value: string | undefined): string | null => {
  const trimmed = value?.trim()

  return trimmed === undefined || trimmed === '' ? null : trimmed
}

const recordSiteEvent = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: RecordAutopilotSiteEventInput,
): Effect.Effect<
  void,
  AutopilotSiteStorageError | AutopilotSiteUnsafePayload
> =>
  Effect.gen(function* () {
    const payloadJson = yield* eventPayloadJson(input.payload)
    const now = runtime.nowIso()

    yield* d1Effect('autopilotSites.events.insert', () =>
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
          runtime.makeEventId(),
          input.siteId,
          input.versionId ?? null,
          input.deploymentId ?? null,
          input.type,
          input.summary,
          input.actorUserId ?? null,
          input.actorRunId ?? null,
          payloadJson,
          now,
        )
        .run(),
    )
  })

const insertProjectFromOrder = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: CreateAutopilotSiteFromOrderInput,
  order: SoftwareOrderSiteSourceRow,
): Effect.Effect<
  AutopilotSiteProject,
  AutopilotSiteStorageError | AutopilotSiteUnsafePayload
> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const id = runtime.makeSiteId()
    const sourceRepository = sourceRepositoryFromOrderRow(order)
    const project: AutopilotSiteProject = {
      id,
      softwareOrderId: order.id,
      ownerUserId: order.user_id,
      teamId: input.teamId ?? null,
      projectId: input.projectId ?? null,
      slug: input.slug,
      title: input.title,
      prompt: order.request,
      status: 'draft',
      accessMode: input.accessMode ?? 'public',
      visibility: input.visibility ?? 'public',
      sourceRepository,
      activeVersionId: null,
      activeDeploymentId: null,
      createdAt: now,
      updatedAt: now,
      archivedAt: null,
    }

    yield* d1Effect('autopilotSites.projects.insertFromOrder', () =>
      db
        .prepare(
          `INSERT INTO site_projects
             (id,
              software_order_id,
              owner_user_id,
              team_id,
              project_id,
              slug,
              title,
              prompt,
              status,
              access_mode,
              visibility,
              source_repository_provider,
              source_repository_owner,
              source_repository_name,
              source_repository_ref,
              active_version_id,
              active_deployment_id,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL)`,
        )
        .bind(
          project.id,
          project.softwareOrderId,
          project.ownerUserId,
          project.teamId,
          project.projectId,
          project.slug,
          project.title,
          project.prompt,
          project.accessMode,
          project.visibility,
          sourceRepository?.provider ?? null,
          sourceRepository?.owner ?? null,
          sourceRepository?.name ?? null,
          sourceRepository?.ref ?? null,
          project.createdAt,
          project.updatedAt,
        )
        .run(),
    )

    const projectCreatedEvent: RecordAutopilotSiteEventInput =
      input.actorUserId === undefined
        ? {
            siteId: project.id,
            type: 'site_project.created',
            summary: `Created Autopilot Site ${project.slug} from software order ${order.id}.`,
            payload: {
              accessMode: project.accessMode,
              softwareOrderId: order.id,
              slug: project.slug,
              visibility: project.visibility,
            },
          }
        : {
            siteId: project.id,
            type: 'site_project.created',
            summary: `Created Autopilot Site ${project.slug} from software order ${order.id}.`,
            actorUserId: input.actorUserId,
            payload: {
              accessMode: project.accessMode,
              softwareOrderId: order.id,
              slug: project.slug,
              visibility: project.visibility,
            },
          }

    yield* recordSiteEvent(db, runtime, projectCreatedEvent)

    return project
  })

const createProjectFromSoftwareOrder = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: CreateAutopilotSiteFromOrderInput,
): Effect.Effect<AutopilotSiteProject, AutopilotSiteError> =>
  Effect.gen(function* () {
    const existingForOrder = yield* readProjectByOrder(
      db,
      input.softwareOrderId,
    )

    if (existingForOrder !== null) {
      return existingForOrder
    }

    const existingForSlug = yield* readProjectBySlug(db, input.slug)

    if (existingForSlug !== null) {
      return yield* new AutopilotSiteSlugUnavailable({ slug: input.slug })
    }

    const order = yield* readSoftwareOrder(db, input.softwareOrderId)

    if (order === null) {
      return yield* new AutopilotSiteSoftwareOrderNotFound({
        softwareOrderId: input.softwareOrderId,
      })
    }

    return yield* insertProjectFromOrder(db, runtime, input, order)
  })

const markActiveDeployment = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: MarkAutopilotSiteActiveDeploymentInput,
): Effect.Effect<void, AutopilotSiteError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()

    yield* d1Effect('autopilotSites.projects.markActiveDeployment', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET active_version_id = ?,
                  active_deployment_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.versionId, input.deploymentId, now, input.siteId)
        .run(),
    )

    const deploymentActivatedEvent: RecordAutopilotSiteEventInput =
      input.actorUserId === undefined
        ? {
            siteId: input.siteId,
            type: 'site_deployment.activated',
            summary: `Activated deployment ${input.deploymentId}.`,
            deploymentId: input.deploymentId,
            versionId: input.versionId,
            payload: {
              deploymentId: input.deploymentId,
              versionId: input.versionId,
            },
          }
        : {
            siteId: input.siteId,
            type: 'site_deployment.activated',
            summary: `Activated deployment ${input.deploymentId}.`,
            actorUserId: input.actorUserId,
            deploymentId: input.deploymentId,
            versionId: input.versionId,
            payload: {
              deploymentId: input.deploymentId,
              versionId: input.versionId,
            },
          }

    yield* recordSiteEvent(db, runtime, deploymentActivatedEvent)
  })

const updateProjectAccess = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: UpdateAutopilotSiteAccessInput,
): Effect.Effect<AutopilotSiteProject, AutopilotSiteError> =>
  Effect.gen(function* () {
    const existing = yield* readProjectById(db, input.siteId)

    if (existing === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    if (input.accessMode === 'public' || input.visibility === 'public') {
      yield* requirePublicLaunchChecklist(
        {
          ...existing,
          accessMode: input.accessMode,
          visibility: input.visibility,
        },
        input.launchChecklist,
        'Public access changes require the launch checklist.',
      )
    }

    const now = runtime.nowIso()

    yield* d1Effect('autopilotSites.projects.updateAccess', () =>
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
        .run(),
    )

    const accessUpdatedEvent: RecordAutopilotSiteEventInput =
      input.actorUserId === undefined
        ? {
            siteId: input.siteId,
            type: 'site_project.access_updated',
            summary: `Updated site access to ${input.accessMode}.`,
            payload: {
              accessMode: input.accessMode,
              visibility: input.visibility,
            },
          }
        : {
            siteId: input.siteId,
            type: 'site_project.access_updated',
            summary: `Updated site access to ${input.accessMode}.`,
            actorUserId: input.actorUserId,
            payload: {
              accessMode: input.accessMode,
              visibility: input.visibility,
            },
          }

    yield* recordSiteEvent(db, runtime, accessUpdatedEvent)

    const updated = yield* readProjectById(db, input.siteId)

    if (updated === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    return updated
  })

const upsertEnvironmentValue = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: UpsertAutopilotSiteEnvironmentValueInput,
): Effect.Effect<AutopilotSiteEnvironmentValue, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const key = input.key.trim()

    if (key === '') {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site environment key is required.',
      })
    }

    const plainValue =
      input.kind === 'plain' ? nonEmptyOptionalInput(input.plainValue) : null
    const secretRef =
      input.kind === 'secret' ? nonEmptyOptionalInput(input.secretRef) : null

    if (input.kind === 'secret' && secretRef === null) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Secret environment values require a secret reference.',
      })
    }

    if (input.kind === 'secret' && input.plainValue !== undefined) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Secret environment values may not include plain values.',
      })
    }

    if (plainValue !== null && containsProviderSecretMaterial(plainValue)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Plain environment value contains secret-shaped material.',
      })
    }

    if (secretRef !== null && containsProviderSecretMaterial(secretRef)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Secret reference contains secret-shaped material.',
      })
    }

    const now = runtime.nowIso()
    const value: AutopilotSiteEnvironmentValue = {
      id: environmentValueId(input.siteId, key),
      siteId: input.siteId,
      key,
      kind: input.kind,
      secretRef,
      plainValue,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    }

    yield* d1Effect('autopilotSites.environmentValues.upsert', () =>
      db
        .prepare(
          `INSERT INTO site_environment_values
             (id,
              site_id,
              key,
              kind,
              secret_ref,
              plain_value,
              created_at,
              updated_at,
              deleted_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, NULL)
           ON CONFLICT(site_id, key)
           WHERE deleted_at IS NULL
           DO UPDATE SET kind = excluded.kind,
                         secret_ref = excluded.secret_ref,
                         plain_value = excluded.plain_value,
                         updated_at = excluded.updated_at`,
        )
        .bind(
          value.id,
          value.siteId,
          value.key,
          value.kind,
          value.secretRef,
          value.plainValue,
          value.createdAt,
          value.updatedAt,
        )
        .run(),
    )

    if (project.activeDeploymentId !== null) {
      yield* d1Effect('autopilotSites.projects.markEnvironmentNeedsReview', () =>
        db
          .prepare(
            `UPDATE site_projects
                SET status = 'needs_review',
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(now, input.siteId)
          .run(),
      )
    }

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      type: 'site_environment.updated',
      summary: `Updated Site environment value ${key}.`,
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        key,
        kind: input.kind,
        requiresRedeploy: project.activeDeploymentId !== null,
        secretRef: secretRef === null ? null : '[SECRET_REF]',
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    return value
  })

const grantAccess = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: GrantAutopilotSiteAccessInput,
): Effect.Effect<AutopilotSiteAccessGrant, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const principalRef = input.principalRef.trim()

    if (principalRef === '') {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site access grant principal reference is required.',
      })
    }

    if (containsProviderSecretMaterial(principalRef)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site access grant principal reference contains secret-shaped material.',
      })
    }

    const now = runtime.nowIso()
    const grant: AutopilotSiteAccessGrant = {
      id: accessGrantId(input.siteId, input.principalKind, principalRef, input.role),
      siteId: input.siteId,
      principalKind: input.principalKind,
      principalRef,
      role: input.role,
      createdAt: now,
      revokedAt: null,
    }

    yield* d1Effect('autopilotSites.accessGrants.insert', () =>
      db
        .prepare(
          `INSERT INTO site_access_grants
             (id,
              site_id,
              principal_kind,
              principal_ref,
              role,
              created_at,
              revoked_at)
           VALUES (?, ?, ?, ?, ?, ?, NULL)
           ON CONFLICT(site_id, principal_kind, principal_ref, role)
           WHERE revoked_at IS NULL
           DO UPDATE SET revoked_at = NULL`,
        )
        .bind(
          grant.id,
          grant.siteId,
          grant.principalKind,
          grant.principalRef,
          grant.role,
          grant.createdAt,
        )
        .run(),
    )

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      type: 'site_access.granted',
      summary: `Granted ${input.role} access to ${input.principalKind}.`,
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        principalKind: input.principalKind,
        principalRef,
        role: input.role,
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    return grant
  })

const saveVersion = (
  db: D1Database,
  artifacts: R2Bucket | undefined,
  runtime: AutopilotSitesRuntime,
  input: SaveAutopilotSiteVersionInput,
): Effect.Effect<AutopilotSiteVersion, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const bucket = yield* requiredArtifactsBucket(artifacts)
    const now = runtime.nowIso()
    const versionId = runtime.makeVersionId()
    const artifactPrefix = `sites/${input.siteId}/versions/${versionId}`
    const sourceArchiveText = yield* rejectSecretText(
      input.sourceArchiveText,
      'Site source archive contains secret-shaped material.',
    )
    const buildLogText = redactedBuildLog(input.buildLogText)
    const workerModuleText = yield* rejectSecretText(
      input.workerModuleText,
      'Site worker module contains secret-shaped material.',
    )
    const staticAssetsManifestJson = yield* storageJson(
      input.staticAssetsManifest,
    )
    const metadataJson = yield* storageJson(input.metadata ?? {})

    if (containsProviderSecretMaterial(staticAssetsManifestJson)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site artifact manifest contains secret-shaped material.',
      })
    }

    const sourceArchiveR2Key = yield* putOptionalArtifactText(
      bucket,
      `${artifactPrefix}/source.txt`,
      sourceArchiveText,
    )
    const buildLogR2Key = yield* putOptionalArtifactText(
      bucket,
      `${artifactPrefix}/build.log`,
      buildLogText,
    )
    const storedWorkerModuleR2Key = yield* putOptionalArtifactText(
      bucket,
      `${artifactPrefix}/worker.mjs`,
      workerModuleText,
    )
    const artifactManifestR2Key = yield* putRequiredArtifactText(
      bucket,
      `${artifactPrefix}/static-assets-manifest.json`,
      staticAssetsManifestJson,
    )
    const savedAt = input.buildStatus === 'saved' ? now : null
    const rejectedAt = input.buildStatus === 'build_failed' ? now : null
    const version: AutopilotSiteVersion = {
      id: versionId,
      siteId: input.siteId,
      sourceKind: input.sourceKind,
      sourceCommitSha: input.sourceCommitSha ?? null,
      sourceArchiveR2Key,
      artifactManifestR2Key,
      buildLogR2Key,
      buildStatus: input.buildStatus,
      buildCommand: input.buildCommand ?? null,
      workerModuleR2Key: input.workerModuleR2Key ?? storedWorkerModuleR2Key,
      staticAssetsManifest: input.staticAssetsManifest,
      d1BindingName: input.d1BindingName ?? null,
      r2BindingName: input.r2BindingName ?? null,
      metadata: input.metadata ?? {},
      createdByUserId: input.actorUserId ?? null,
      createdByRunId: input.actorRunId ?? null,
      createdAt: now,
      savedAt,
      rejectedAt,
    }

    yield* d1Effect('autopilotSites.versions.insert', () =>
      db
        .prepare(
          `INSERT INTO site_versions
             (id,
              site_id,
              source_kind,
              source_commit_sha,
              source_archive_r2_key,
              artifact_manifest_r2_key,
              build_log_r2_key,
              build_status,
              build_command,
              worker_module_r2_key,
              static_assets_manifest_json,
              d1_binding_name,
              r2_binding_name,
              metadata_json,
              created_by_user_id,
              created_by_run_id,
              created_at,
              saved_at,
              rejected_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          version.id,
          version.siteId,
          version.sourceKind,
          version.sourceCommitSha,
          version.sourceArchiveR2Key,
          version.artifactManifestR2Key,
          version.buildLogR2Key,
          version.buildStatus,
          version.buildCommand,
          version.workerModuleR2Key,
          staticAssetsManifestJson,
          version.d1BindingName,
          version.r2BindingName,
          metadataJson,
          version.createdByUserId,
          version.createdByRunId,
          version.createdAt,
          version.savedAt,
          version.rejectedAt,
        )
        .run(),
    )

    yield* upsertStorageBinding(db, input.siteId, 'd1', version.d1BindingName, now)
    yield* upsertStorageBinding(db, input.siteId, 'r2', version.r2BindingName, now)

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      versionId,
      type:
        input.buildStatus === 'saved'
          ? 'site_version.saved'
          : 'site_version.build_failed',
      summary:
        input.buildStatus === 'saved'
          ? `Saved Site version ${versionId}.`
          : `Recorded failed Site version ${versionId}.`,
      ...(input.actorRunId === undefined ? {} : { actorRunId: input.actorRunId }),
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        artifactManifestR2Key,
        buildLogR2Key,
        buildStatus: input.buildStatus,
        sourceArchiveR2Key,
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    return version
  })

const activeDeploymentUrl = (slug: string): string =>
  `https://sites.openagents.com/${slug}`

const generationOutputContract = (
  project: AutopilotSiteProject,
): ReadonlyArray<string> => [
  `Create a static Autopilot Site for ${project.title}.`,
  `Target public URL: ${activeDeploymentUrl(project.slug)}.`,
  'Produce reviewable HTML, CSS, JavaScript, and asset manifest entries for the Sites save-version API.',
  'Keep the first viewport focused on the customer subject and leave the next section partially visible.',
  'Use only information from the order prompt, source repository, and operator-provided context.',
  'Do not include private customer data, credentials, OAuth state, checkout state, or deployment/result query strings.',
]

const generationPreflightChecklist: ReadonlyArray<string> = [
  'Run the operator Autopilot preflight before dispatch.',
  'Confirm required provider/repository access is connected before launch.',
  'Use callback retry and continuation for interrupted runs.',
  'Save generated artifacts through the Sites version lifecycle before deploy.',
  'Deploy only after operator review approves the saved version.',
]

const generationGoal = (
  project: AutopilotSiteProject,
  operatorNotes: string | undefined,
): string =>
  [
    `Generate the first public Autopilot Site for ${project.title}.`,
    '',
    `siteId: ${project.id}`,
    `softwareOrderId: ${project.softwareOrderId ?? 'none'}`,
    `slug: ${project.slug}`,
    `targetUrl: ${activeDeploymentUrl(project.slug)}`,
    project.sourceRepository === null
      ? 'sourceRepository: none'
      : `sourceRepository: ${project.sourceRepository.owner}/${project.sourceRepository.name}@${project.sourceRepository.ref}`,
    '',
    'Customer order prompt:',
    project.prompt,
    '',
    'Output requirements:',
    ...generationOutputContract(project).map(item => `- ${item}`),
    ...(operatorNotes === undefined
      ? []
      : ['', 'Operator notes:', operatorNotes]),
  ].join('\n')

const generationPacket = (
  project: AutopilotSiteProject,
  now: string,
  operatorNotes: string | undefined,
): AutopilotSiteGenerationPacket => ({
  siteId: project.id,
  softwareOrderId: project.softwareOrderId,
  slug: project.slug,
  title: project.title,
  publicUrl: activeDeploymentUrl(project.slug),
  objective: `Generate ${project.title} for ${activeDeploymentUrl(project.slug)}.`,
  generationGoal: generationGoal(project, operatorNotes),
  outputContract: generationOutputContract(project),
  preflightChecklist: generationPreflightChecklist,
  sourceRepository: project.sourceRepository,
  operatorNotes: operatorNotes ?? null,
  createdAt: now,
})

type DeploymentRuntimeMetadata = Readonly<{
  runtimeKind: AutopilotSiteRuntimeKind
  runtimeScriptName: string | null
  dispatchNamespace: string | null
  externalDeploymentId: string | null
}>

type DeploymentAttemptStatus =
  | 'activated'
  | 'health_failed'
  | 'health_missing'

type DeploymentAttemptInput = Readonly<{
  deploymentId: string | null
  metadata: DeploymentRuntimeMetadata
  status: DeploymentAttemptStatus
  siteId: string
  versionId: string
  input: DeployAutopilotSiteVersionInput
}>

const deploymentRuntimeMetadata = (
  input: DeployAutopilotSiteVersionInput,
  version: AutopilotSiteVersion,
): Effect.Effect<DeploymentRuntimeMetadata, AutopilotSiteRuntimeNotDeployable> =>
  Effect.gen(function* () {
    const runtimeKind = input.runtimeKind ?? 'omega_static_r2'

    if (runtimeKind === 'omega_static_r2') {
      return {
        runtimeKind,
        runtimeScriptName: null,
        dispatchNamespace: null,
        externalDeploymentId: null,
      }
    }

    const runtimeScriptName = nonEmptyOptionalInput(input.runtimeScriptName)
    const dispatchNamespace = nonEmptyOptionalInput(input.dispatchNamespace)

    if (version.workerModuleR2Key === null) {
      return yield* new AutopilotSiteRuntimeNotDeployable({
        reason:
          'Workers for Platforms deployments require a saved worker module artifact.',
        siteId: input.siteId,
        versionId: input.versionId,
      })
    }

    if (runtimeScriptName === null || dispatchNamespace === null) {
      return yield* new AutopilotSiteRuntimeNotDeployable({
        reason:
          'Workers for Platforms deployments require runtime script and dispatch namespace metadata.',
        siteId: input.siteId,
        versionId: input.versionId,
      })
    }

    return {
      runtimeKind,
      runtimeScriptName,
      dispatchNamespace,
      externalDeploymentId:
        nonEmptyOptionalInput(input.externalDeploymentId) ?? null,
    }
  })

const wfpHealthStatus = (
  input: DeployAutopilotSiteVersionInput,
  metadata: DeploymentRuntimeMetadata,
): DeploymentAttemptStatus | 'passed' => {
  if (metadata.runtimeKind !== 'workers_for_platforms') {
    return 'passed'
  }

  if (input.healthCheck === undefined) {
    return 'health_missing'
  }

  return input.healthCheck.status === 'passed' ? 'passed' : 'health_failed'
}

const recordDeploymentAttempt = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  attempt: DeploymentAttemptInput,
  now: string,
): Effect.Effect<void, AutopilotSiteStorageError | AutopilotSiteUnsafePayload> =>
  Effect.gen(function* () {
    const metadataJson = yield* storageJson({
      healthCheckedAt: attempt.input.healthCheck?.checkedAt ?? null,
      healthSummary: attempt.input.healthCheck?.summary ?? null,
      tags: attempt.input.tags ?? [],
    })

    if (containsProviderSecretMaterial(metadataJson)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site deployment attempt metadata contains secret-shaped material.',
      })
    }

    yield* d1Effect('autopilotSites.deploymentAttempts.insert', () =>
      db
        .prepare(
          `INSERT INTO site_deployment_attempts
             (id,
              site_id,
              version_id,
              deployment_id,
              runtime_kind,
              runtime_script_name,
              dispatch_namespace,
              external_deployment_id,
              status,
              upload_receipt_ref,
              health_status,
              health_url,
              health_ref,
              rollback_ref,
              observability_ref,
              metadata_json,
              created_at,
              updated_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
        )
        .bind(
          runtime.makeDeploymentAttemptId(),
          attempt.siteId,
          attempt.versionId,
          attempt.deploymentId,
          attempt.metadata.runtimeKind,
          attempt.metadata.runtimeScriptName,
          attempt.metadata.dispatchNamespace,
          attempt.metadata.externalDeploymentId,
          attempt.status,
          nonEmptyOptionalInput(attempt.input.uploadReceiptRef),
          attempt.input.healthCheck?.status ?? 'not_recorded',
          nonEmptyOptionalInput(attempt.input.healthCheck?.url),
          nonEmptyOptionalInput(attempt.input.healthCheck?.healthRef),
          nonEmptyOptionalInput(attempt.input.rollbackRef),
          nonEmptyOptionalInput(attempt.input.observabilityRef),
          metadataJson,
          now,
          now,
        )
        .run(),
    )
  })

const deployVersion = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: DeployAutopilotSiteVersionInput,
): Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const version = yield* readVersionById(db, input.siteId, input.versionId)

    if (version === null) {
      return yield* new AutopilotSiteVersionNotFound({
        siteId: input.siteId,
        versionId: input.versionId,
      })
    }

    if (version.buildStatus !== 'saved') {
      return yield* new AutopilotSiteVersionNotDeployable({
        buildStatus: version.buildStatus,
        versionId: input.versionId,
      })
    }

    const now = runtime.nowIso()
    yield* requirePublicLaunchChecklist(
      project,
      input.launchChecklist,
      'Public deployments require the launch checklist.',
    )
    const runtimeMetadata = yield* deploymentRuntimeMetadata(input, version)
    const deploymentId = runtime.makeDeploymentId()
    const healthStatus = wfpHealthStatus(input, runtimeMetadata)

    if (healthStatus !== 'passed') {
      yield* recordDeploymentAttempt(
        db,
        runtime,
        {
          deploymentId: null,
          input,
          metadata: runtimeMetadata,
          siteId: input.siteId,
          status: healthStatus,
          versionId: input.versionId,
        },
        now,
      )

      return yield* new AutopilotSiteRuntimeNotDeployable({
        reason:
          healthStatus === 'health_missing'
            ? 'Workers for Platforms deployments require a passed health check before activation.'
            : 'Workers for Platforms deployment health check failed.',
        siteId: input.siteId,
        versionId: input.versionId,
      })
    }

    const previousDeploymentId = project.activeDeploymentId
    const deployment: AutopilotSiteDeployment = {
      id: deploymentId,
      siteId: input.siteId,
      versionId: input.versionId,
      slug: project.slug,
      url: activeDeploymentUrl(project.slug),
      runtimeKind: runtimeMetadata.runtimeKind,
      runtimeScriptName: runtimeMetadata.runtimeScriptName,
      dispatchNamespace: runtimeMetadata.dispatchNamespace,
      status: 'active',
      deployedByUserId: input.actorUserId ?? null,
      externalDeploymentId: runtimeMetadata.externalDeploymentId,
      startedAt: now,
      activatedAt: now,
      failedAt: null,
      disabledAt: null,
      rolledBackAt: null,
      createdAt: now,
      updatedAt: now,
    }

    yield* recordDeploymentAttempt(
      db,
      runtime,
      {
        deploymentId: deployment.id,
        input,
        metadata: runtimeMetadata,
        siteId: input.siteId,
        status: 'activated',
        versionId: input.versionId,
      },
      now,
    )

    yield* d1Effect('autopilotSites.deployments.rollbackPreviousActive', () =>
      db
        .prepare(
          `UPDATE site_deployments
              SET status = 'rolled_back',
                  rolled_back_at = ?,
                  updated_at = ?
            WHERE site_id = ?
              AND status = 'active'`,
        )
        .bind(now, now, input.siteId)
        .run(),
    )

    yield* d1Effect('autopilotSites.deployments.insertActive', () =>
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
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?, ?, NULL, NULL, NULL, ?, ?)`,
        )
        .bind(
          deployment.id,
          deployment.siteId,
          deployment.versionId,
          deployment.slug,
          deployment.url,
          deployment.runtimeKind,
          deployment.runtimeScriptName,
          deployment.dispatchNamespace,
          deployment.deployedByUserId,
          deployment.externalDeploymentId,
          deployment.startedAt,
          deployment.activatedAt,
          deployment.createdAt,
          deployment.updatedAt,
        )
        .run(),
    )

    yield* d1Effect('autopilotSites.projects.markDeployedVersion', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET active_version_id = ?,
                  active_deployment_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(input.versionId, deployment.id, now, input.siteId)
        .run(),
    )

    const deploymentActivatedEvent: RecordAutopilotSiteEventInput =
      input.actorUserId === undefined
        ? {
            siteId: input.siteId,
            versionId: input.versionId,
            deploymentId: deployment.id,
            type: 'site_deployment.activated',
            summary: `Activated deployment ${deployment.id}.`,
            payload: {
              dispatchNamespace: deployment.dispatchNamespace,
              deploymentId: deployment.id,
              externalDeploymentId: deployment.externalDeploymentId,
              healthRef: input.healthCheck?.healthRef ?? null,
              healthStatus: input.healthCheck?.status ?? 'not_required',
              observabilityRef: input.observabilityRef ?? null,
              previousDeploymentId,
              rollbackRef: input.rollbackRef ?? null,
              runtimeKind: deployment.runtimeKind,
              runtimeScriptName: deployment.runtimeScriptName,
              tags: input.tags ?? [],
              uploadReceiptRef: input.uploadReceiptRef ?? null,
              url: deployment.url,
              versionId: input.versionId,
            },
          }
        : {
            siteId: input.siteId,
            versionId: input.versionId,
            deploymentId: deployment.id,
            type: 'site_deployment.activated',
            summary: `Activated deployment ${deployment.id}.`,
            actorUserId: input.actorUserId,
            payload: {
              dispatchNamespace: deployment.dispatchNamespace,
              deploymentId: deployment.id,
              externalDeploymentId: deployment.externalDeploymentId,
              healthRef: input.healthCheck?.healthRef ?? null,
              healthStatus: input.healthCheck?.status ?? 'not_required',
              observabilityRef: input.observabilityRef ?? null,
              previousDeploymentId,
              rollbackRef: input.rollbackRef ?? null,
              runtimeKind: deployment.runtimeKind,
              runtimeScriptName: deployment.runtimeScriptName,
              tags: input.tags ?? [],
              uploadReceiptRef: input.uploadReceiptRef ?? null,
              url: deployment.url,
              versionId: input.versionId,
            },
          }

    yield* recordSiteEvent(db, runtime, deploymentActivatedEvent)

    return deployment
  })

const disableDeployment = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: DisableAutopilotSiteDeploymentInput,
): Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const deployment = yield* readDeploymentById(
      db,
      input.siteId,
      input.deploymentId,
    )

    if (deployment === null) {
      return yield* new AutopilotSiteDeploymentNotFound({
        deploymentId: input.deploymentId,
        siteId: input.siteId,
      })
    }

    const now = runtime.nowIso()

    yield* d1Effect('autopilotSites.deployments.disable', () =>
      db
        .prepare(
          `UPDATE site_deployments
              SET status = 'disabled',
                  disabled_at = ?,
                  updated_at = ?
            WHERE id = ?
              AND site_id = ?`,
        )
        .bind(now, now, input.deploymentId, input.siteId)
        .run(),
    )

    if (project.activeDeploymentId === input.deploymentId) {
      yield* d1Effect('autopilotSites.projects.markDisabled', () =>
        db
          .prepare(
            `UPDATE site_projects
                SET status = 'disabled',
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(now, input.siteId)
          .run(),
      )
    }

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      deploymentId: input.deploymentId,
      versionId: deployment.versionId,
      type: 'site_deployment.disabled',
      summary: `Disabled deployment ${input.deploymentId}.`,
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        deploymentId: input.deploymentId,
        wasActive: project.activeDeploymentId === input.deploymentId,
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    const updated = yield* readDeploymentById(db, input.siteId, input.deploymentId)

    if (updated === null) {
      return yield* new AutopilotSiteDeploymentNotFound({
        deploymentId: input.deploymentId,
        siteId: input.siteId,
      })
    }

    return updated
  })

const rollbackDeployment = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: RollbackAutopilotSiteDeploymentInput,
): Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const target = yield* readDeploymentById(db, input.siteId, input.deploymentId)

    if (target === null) {
      return yield* new AutopilotSiteDeploymentNotFound({
        deploymentId: input.deploymentId,
        siteId: input.siteId,
      })
    }

    const now = runtime.nowIso()
    const previousDeploymentId = project.activeDeploymentId

    yield* d1Effect('autopilotSites.deployments.rollbackCurrentActive', () =>
      db
        .prepare(
          `UPDATE site_deployments
              SET status = 'rolled_back',
                  rolled_back_at = ?,
                  updated_at = ?
            WHERE site_id = ?
              AND status = 'active'
              AND id <> ?`,
        )
        .bind(now, now, input.siteId, input.deploymentId)
        .run(),
    )

    yield* d1Effect('autopilotSites.deployments.activateRollbackTarget', () =>
      db
        .prepare(
          `UPDATE site_deployments
              SET status = 'active',
                  activated_at = ?,
                  disabled_at = NULL,
                  rolled_back_at = NULL,
                  updated_at = ?
            WHERE id = ?
              AND site_id = ?`,
        )
        .bind(now, now, input.deploymentId, input.siteId)
        .run(),
    )

    yield* d1Effect('autopilotSites.projects.rollbackActiveDeployment', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET status = 'approved',
                  active_version_id = ?,
                  active_deployment_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(target.versionId, input.deploymentId, now, input.siteId)
        .run(),
    )

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      deploymentId: input.deploymentId,
      versionId: target.versionId,
      type: 'site_deployment.rollback_activated',
      summary: `Rolled back to deployment ${input.deploymentId}.`,
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        previousDeploymentId,
        targetDeploymentId: input.deploymentId,
        targetVersionId: target.versionId,
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    const updated = yield* readDeploymentById(db, input.siteId, input.deploymentId)

    if (updated === null) {
      return yield* new AutopilotSiteDeploymentNotFound({
        deploymentId: input.deploymentId,
        siteId: input.siteId,
      })
    }

    return updated
  })

const requestGeneration = (
  db: D1Database,
  runtime: AutopilotSitesRuntime,
  input: RequestAutopilotSiteGenerationInput,
): Effect.Effect<AutopilotSiteGenerationPacket, AutopilotSiteError> =>
  Effect.gen(function* () {
    const project = yield* readProjectById(db, input.siteId)

    if (project === null) {
      return yield* new AutopilotSiteProjectNotFound({ siteId: input.siteId })
    }

    const now = runtime.nowIso()
    const packet = generationPacket(project, now, input.operatorNotes)
    const packetJson = JSON.stringify(packet)

    if (containsProviderSecretMaterial(packetJson)) {
      return yield* new AutopilotSiteUnsafePayload({
        reason: 'Site generation packet contains secret-shaped material.',
      })
    }

    yield* d1Effect('autopilotSites.projects.markGenerating', () =>
      db
        .prepare(
          `UPDATE site_projects
              SET status = 'generating',
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(now, input.siteId)
        .run(),
    )

    const eventInput: RecordAutopilotSiteEventInput = {
      siteId: input.siteId,
      type: 'site_generation.requested',
      summary: `Requested Site generation for ${project.slug}.`,
      ...(input.actorRunId === undefined
        ? {}
        : { actorRunId: input.actorRunId }),
      ...(input.actorUserId === undefined
        ? {}
        : { actorUserId: input.actorUserId }),
      payload: {
        generationGoal: packet.generationGoal,
        publicUrl: packet.publicUrl,
        slug: packet.slug,
        softwareOrderId: packet.softwareOrderId,
      },
    }

    yield* recordSiteEvent(db, runtime, eventInput)

    return packet
  })

export type AutopilotSitesOperations = Readonly<{
  createProjectFromSoftwareOrder: (
    input: CreateAutopilotSiteFromOrderInput,
  ) => Effect.Effect<AutopilotSiteProject, AutopilotSiteError>
  markActiveDeployment: (
    input: MarkAutopilotSiteActiveDeploymentInput,
  ) => Effect.Effect<void, AutopilotSiteError>
  listProjects: (
    limit: number,
  ) => Effect.Effect<
    ReadonlyArray<AutopilotSiteProject>,
    AutopilotSiteStorageError
  >
  readProjectById: (
    siteId: string,
  ) => Effect.Effect<AutopilotSiteProject | null, AutopilotSiteStorageError>
  readProjectBySlug: (
    slug: string,
  ) => Effect.Effect<AutopilotSiteProject | null, AutopilotSiteStorageError>
  recordEvent: (
    input: RecordAutopilotSiteEventInput,
  ) => Effect.Effect<
    void,
    AutopilotSiteStorageError | AutopilotSiteUnsafePayload
  >
  updateProjectAccess: (
    input: UpdateAutopilotSiteAccessInput,
  ) => Effect.Effect<AutopilotSiteProject, AutopilotSiteError>
  saveVersion: (
    input: SaveAutopilotSiteVersionInput,
  ) => Effect.Effect<AutopilotSiteVersion, AutopilotSiteError>
  deployVersion: (
    input: DeployAutopilotSiteVersionInput,
  ) => Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError>
  disableDeployment: (
    input: DisableAutopilotSiteDeploymentInput,
  ) => Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError>
  rollbackDeployment: (
    input: RollbackAutopilotSiteDeploymentInput,
  ) => Effect.Effect<AutopilotSiteDeployment, AutopilotSiteError>
  upsertEnvironmentValue: (
    input: UpsertAutopilotSiteEnvironmentValueInput,
  ) => Effect.Effect<AutopilotSiteEnvironmentValue, AutopilotSiteError>
  grantAccess: (
    input: GrantAutopilotSiteAccessInput,
  ) => Effect.Effect<AutopilotSiteAccessGrant, AutopilotSiteError>
  listEvents: (
    siteId: string,
    limit: number,
  ) => Effect.Effect<
    ReadonlyArray<AutopilotSiteEvent>,
    AutopilotSiteStorageError
  >
  requestGeneration: (
    input: RequestAutopilotSiteGenerationInput,
  ) => Effect.Effect<AutopilotSiteGenerationPacket, AutopilotSiteError>
}>

export class AutopilotSitesService extends Context.Service<
  AutopilotSitesService,
  AutopilotSitesOperations
>()('@openagentsinc/autopilot-omega/AutopilotSitesService') {
  static readonly fromBindings = (
    db: D1Database,
    artifacts: R2Bucket | undefined,
    runtime: AutopilotSitesRuntime = systemAutopilotSitesRuntime,
  ): AutopilotSitesOperations => ({
    createProjectFromSoftwareOrder: Effect.fn(
      'AutopilotSitesService.createProjectFromSoftwareOrder',
    )(input => createProjectFromSoftwareOrder(db, runtime, input)),
    markActiveDeployment: Effect.fn(
      'AutopilotSitesService.markActiveDeployment',
    )(input => markActiveDeployment(db, runtime, input)),
    listProjects: Effect.fn('AutopilotSitesService.listProjects')(limit =>
      listProjects(db, limit),
    ),
    readProjectById: Effect.fn('AutopilotSitesService.readProjectById')(
      siteId => readProjectById(db, siteId),
    ),
    readProjectBySlug: Effect.fn(
      'AutopilotSitesService.readProjectBySlug',
    )(slug => readProjectBySlug(db, slug)),
    recordEvent: Effect.fn('AutopilotSitesService.recordEvent')(input =>
      recordSiteEvent(db, runtime, input),
    ),
    updateProjectAccess: Effect.fn(
      'AutopilotSitesService.updateProjectAccess',
    )(input => updateProjectAccess(db, runtime, input)),
    saveVersion: Effect.fn('AutopilotSitesService.saveVersion')(input =>
      saveVersion(db, artifacts, runtime, input),
    ),
    deployVersion: Effect.fn('AutopilotSitesService.deployVersion')(input =>
      deployVersion(db, runtime, input),
    ),
    disableDeployment: Effect.fn(
      'AutopilotSitesService.disableDeployment',
    )(input => disableDeployment(db, runtime, input)),
    rollbackDeployment: Effect.fn(
      'AutopilotSitesService.rollbackDeployment',
    )(input => rollbackDeployment(db, runtime, input)),
    upsertEnvironmentValue: Effect.fn(
      'AutopilotSitesService.upsertEnvironmentValue',
    )(input => upsertEnvironmentValue(db, runtime, input)),
    grantAccess: Effect.fn('AutopilotSitesService.grantAccess')(input =>
      grantAccess(db, runtime, input),
    ),
    listEvents: Effect.fn('AutopilotSitesService.listEvents')(
      (siteId, limit) => listEvents(db, siteId, limit),
    ),
    requestGeneration: Effect.fn(
      'AutopilotSitesService.requestGeneration',
    )(input => requestGeneration(db, runtime, input)),
  })

  static readonly layer = (
    env: AutopilotSitesEnv,
    runtime: AutopilotSitesRuntime = systemAutopilotSitesRuntime,
  ) => {
    const db = openAgentsDatabase(env)

    return Layer.succeed(
      AutopilotSitesService,
      // CFG-8 (#8523): resolve the artifacts bucket (GCS adapter when
      // configured) instead of reading the removed R2 binding slot.
      AutopilotSitesService.fromBindings(db, artifactsBucketForEnv(env), runtime),
    )
  }
}
