import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  AutopilotSiteLaunchChecklistRequired,
  AutopilotSiteRuntimeNotDeployable,
  AutopilotSiteSlugUnavailable,
  AutopilotSiteUnsafePayload,
  AutopilotSiteVersionNotDeployable,
  AutopilotSitesService,
} from './sites'

type StoredSoftwareOrder = Readonly<{
  id: string
  user_id: string
  request: string
  repository_provider: 'github' | null
  repository_owner: string | null
  repository_name: string | null
  repository_default_branch: string | null
  archived_at: string | null
}>

type StoredSiteProject = Readonly<{
  access_mode: string
  active_deployment_id: string | null
  active_version_id: string | null
  archived_at: string | null
  created_at: string
  id: string
  owner_user_id: string
  project_id: string | null
  prompt: string
  slug: string
  software_order_id: string | null
  source_repository_name: string | null
  source_repository_owner: string | null
  source_repository_provider: 'github' | null
  source_repository_ref: string | null
  status: string
  team_id: string | null
  title: string
  updated_at: string
  visibility: string
}>

type StoredSiteEvent = Readonly<{
  actor_run_id: string | null
  actor_user_id: string | null
  created_at: string
  deployment_id: string | null
  id: string
  payload_json: string | null
  site_id: string
  summary: string
  type: string
  version_id: string | null
}>

type StoredSiteVersion = Readonly<{
  artifact_manifest_r2_key: string | null
  build_command: string | null
  build_log_r2_key: string | null
  build_status: string
  created_at: string
  created_by_run_id: string | null
  created_by_user_id: string | null
  d1_binding_name: string | null
  id: string
  metadata_json: string
  r2_binding_name: string | null
  rejected_at: string | null
  saved_at: string | null
  site_id: string
  source_archive_r2_key: string | null
  source_commit_sha: string | null
  source_kind: string
  static_assets_manifest_json: string
  worker_module_r2_key: string | null
}>

type StoredSiteDeployment = Readonly<{
  activated_at: string | null
  created_at: string
  deploy_status_marker?: string
  deployed_by_user_id: string | null
  disabled_at: string | null
  dispatch_namespace: string | null
  external_deployment_id: string | null
  failed_at: string | null
  id: string
  rolled_back_at: string | null
  runtime_kind: string
  runtime_script_name: string | null
  site_id: string
  slug: string
  started_at: string | null
  status: string
  updated_at: string
  url: string
  version_id: string
}>

type StoredSiteDeploymentAttempt = Readonly<{
  archived_at: string | null
  created_at: string
  deployment_id: string | null
  dispatch_namespace: string | null
  external_deployment_id: string | null
  health_ref: string | null
  health_status: string
  health_url: string | null
  id: string
  metadata_json: string
  observability_ref: string | null
  rollback_ref: string | null
  runtime_kind: string
  runtime_script_name: string | null
  site_id: string
  status: string
  updated_at: string
  upload_receipt_ref: string | null
  version_id: string
}>

type StoredSiteStorageBinding = Readonly<{
  binding_name: string
  cloudflare_resource_ref: string | null
  created_at: string
  id: string
  kind: string
  scope: string
  site_id: string
  updated_at: string
}>

type StoredSiteEnvironmentValue = Readonly<{
  created_at: string
  deleted_at: string | null
  id: string
  key: string
  kind: string
  plain_value: string | null
  secret_ref: string | null
  site_id: string
  updated_at: string
}>

type StoredSiteAccessGrant = Readonly<{
  created_at: string
  id: string
  principal_kind: string
  principal_ref: string
  revoked_at: string | null
  role: string
  site_id: string
}>

class SitesTestRuntime {
  private deploymentAttemptCounter = 0
  private deploymentCounter = 0
  private eventCounter = 0
  private siteCounter = 0
  private versionCounter = 0

  makeDeploymentId = (): string => {
    this.deploymentCounter += 1

    return `site_deployment_${this.deploymentCounter}`
  }

  makeDeploymentAttemptId = (): string => {
    this.deploymentAttemptCounter += 1

    return `site_deployment_attempt_${this.deploymentAttemptCounter}`
  }

  makeEventId = (): string => {
    this.eventCounter += 1

    return `site_event_${this.eventCounter}`
  }

  makeSiteId = (): string => {
    this.siteCounter += 1

    return `site_project_${this.siteCounter}`
  }

  makeVersionId = (): string => {
    this.versionCounter += 1

    return `site_version_${this.versionCounter}`
  }

  nowIso = (): string => '2026-06-04T21:00:00.000Z'
}

class SitesDbStore {
  accessGrants: Array<StoredSiteAccessGrant> = []
  deploymentAttempts: Array<StoredSiteDeploymentAttempt> = []
  deployments: Array<StoredSiteDeployment> = []
  environmentValues: Array<StoredSiteEnvironmentValue> = []
  events: Array<StoredSiteEvent> = []
  projects: Array<StoredSiteProject> = []
  runtime = new SitesTestRuntime()
  softwareOrders: Array<StoredSoftwareOrder> = [
    {
      id: 'software_order_ben_otec',
      user_id: 'github:ben',
      request:
        'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
      repository_provider: 'github',
      repository_owner: 'bensilone',
      repository_name: 'openagents',
      repository_default_branch: 'main',
      archived_at: null,
    },
  ]
  storageBindings: Array<StoredSiteStorageBinding> = []
  versions: Array<StoredSiteVersion> = []
}

class SitesArtifactsBucket {
  objects = new Map<string, string>()

  put(key: string, value: string): Promise<R2Object> {
    this.objects.set(key, value)

    return Promise.resolve({ key } as R2Object)
  }
}

class SitesStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: SitesDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (
      this.query.includes('FROM site_projects') &&
      this.query.includes('software_order_id = ?')
    ) {
      const softwareOrderId = String(this.values[0])
      const project = this.store.projects.find(
        item =>
          item.software_order_id === softwareOrderId &&
          item.archived_at === null,
      )

      return Promise.resolve((project as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM site_projects') &&
      this.query.includes('slug = ?')
    ) {
      const slug = String(this.values[0])
      const project = this.store.projects.find(
        item => item.slug === slug && item.archived_at === null,
      )

      return Promise.resolve((project as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM site_projects') &&
      this.query.includes('id = ?')
    ) {
      const siteId = String(this.values[0])
      const project = this.store.projects.find(
        item => item.id === siteId && item.archived_at === null,
      )

      return Promise.resolve((project as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_versions')) {
      const versionId = String(this.values[0])
      const siteId = String(this.values[1])
      const version = this.store.versions.find(
        item => item.id === versionId && item.site_id === siteId,
      )

      return Promise.resolve((version as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_deployments')) {
      const deploymentId = String(this.values[0])
      const siteId = String(this.values[1])
      const deployment = this.store.deployments.find(
        item => item.id === deploymentId && item.site_id === siteId,
      )

      return Promise.resolve((deployment as T | undefined) ?? null)
    }

    if (this.query.includes('FROM software_orders')) {
      const softwareOrderId = String(this.values[0])
      const order = this.store.softwareOrders.find(
        item => item.id === softwareOrderId && item.archived_at === null,
      )

      return Promise.resolve((order as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO site_projects')) {
      const [
        id,
        softwareOrderId,
        ownerUserId,
        teamId,
        projectId,
        slug,
        title,
        prompt,
        accessMode,
        visibility,
        sourceRepositoryProvider,
        sourceRepositoryOwner,
        sourceRepositoryName,
        sourceRepositoryRef,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.projects = [
        ...this.store.projects,
        {
          access_mode: String(accessMode),
          active_deployment_id: null,
          active_version_id: null,
          archived_at: null,
          created_at: String(createdAt),
          id: String(id),
          owner_user_id: String(ownerUserId),
          project_id: projectId === null ? null : String(projectId),
          prompt: String(prompt),
          slug: String(slug),
          software_order_id:
            softwareOrderId === null ? null : String(softwareOrderId),
          source_repository_name:
            sourceRepositoryName === null ? null : String(sourceRepositoryName),
          source_repository_owner:
            sourceRepositoryOwner === null
              ? null
              : String(sourceRepositoryOwner),
          source_repository_provider:
            sourceRepositoryProvider === null ? null : 'github',
          source_repository_ref:
            sourceRepositoryRef === null ? null : String(sourceRepositoryRef),
          status: 'draft',
          team_id: teamId === null ? null : String(teamId),
          title: String(title),
          updated_at: String(updatedAt),
          visibility: String(visibility),
        },
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      const [
        id,
        siteId,
        versionId,
        deploymentId,
        type,
        summary,
        actorUserId,
        actorRunId,
        payloadJson,
        createdAt,
      ] = this.values

      this.store.events = [
        ...this.store.events,
        {
          actor_run_id: actorRunId === null ? null : String(actorRunId),
          actor_user_id: actorUserId === null ? null : String(actorUserId),
          created_at: String(createdAt),
          deployment_id: deploymentId === null ? null : String(deploymentId),
          id: String(id),
          payload_json: payloadJson === null ? null : String(payloadJson),
          site_id: String(siteId),
          summary: String(summary),
          type: String(type),
          version_id: versionId === null ? null : String(versionId),
        },
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_versions')) {
      const [
        id,
        siteId,
        sourceKind,
        sourceCommitSha,
        sourceArchiveR2Key,
        artifactManifestR2Key,
        buildLogR2Key,
        buildStatus,
        buildCommand,
        workerModuleR2Key,
        staticAssetsManifestJson,
        d1BindingName,
        r2BindingName,
        metadataJson,
        createdByUserId,
        createdByRunId,
        createdAt,
        savedAt,
        rejectedAt,
      ] = this.values

      this.store.versions = [
        ...this.store.versions,
        {
          artifact_manifest_r2_key:
            artifactManifestR2Key === null
              ? null
              : String(artifactManifestR2Key),
          build_command: buildCommand === null ? null : String(buildCommand),
          build_log_r2_key:
            buildLogR2Key === null ? null : String(buildLogR2Key),
          build_status: String(buildStatus),
          created_at: String(createdAt),
          created_by_run_id:
            createdByRunId === null ? null : String(createdByRunId),
          created_by_user_id:
            createdByUserId === null ? null : String(createdByUserId),
          d1_binding_name: d1BindingName === null ? null : String(d1BindingName),
          id: String(id),
          metadata_json: String(metadataJson),
          r2_binding_name: r2BindingName === null ? null : String(r2BindingName),
          rejected_at: rejectedAt === null ? null : String(rejectedAt),
          saved_at: savedAt === null ? null : String(savedAt),
          site_id: String(siteId),
          source_archive_r2_key:
            sourceArchiveR2Key === null ? null : String(sourceArchiveR2Key),
          source_commit_sha:
            sourceCommitSha === null ? null : String(sourceCommitSha),
          source_kind: String(sourceKind),
          static_assets_manifest_json: String(staticAssetsManifestJson),
          worker_module_r2_key:
            workerModuleR2Key === null ? null : String(workerModuleR2Key),
        },
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_storage_bindings')) {
      const [
        id,
        siteId,
        kind,
        bindingName,
        createdAt,
        updatedAt,
      ] = this.values
      const nextBinding: StoredSiteStorageBinding = {
        binding_name: String(bindingName),
        cloudflare_resource_ref: null,
        created_at: String(createdAt),
        id: String(id),
        kind: String(kind),
        scope: 'shared_prefix',
        site_id: String(siteId),
        updated_at: String(updatedAt),
      }

      this.store.storageBindings = [
        ...this.store.storageBindings.filter(
          binding =>
            !(
              binding.site_id === nextBinding.site_id &&
              binding.kind === nextBinding.kind &&
              binding.binding_name === nextBinding.binding_name
            ),
        ),
        nextBinding,
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("status = 'rolled_back'")
    ) {
      const [rolledBackAt, updatedAt, siteId] = this.values

      this.store.deployments = this.store.deployments.map(deployment =>
        deployment.site_id === String(siteId) && deployment.status === 'active'
          ? {
              ...deployment,
              rolled_back_at: String(rolledBackAt),
              status: 'rolled_back',
              updated_at: String(updatedAt),
            }
          : deployment,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("status = 'disabled'")
    ) {
      const [disabledAt, updatedAt, deploymentId, siteId] = this.values

      this.store.deployments = this.store.deployments.map(deployment =>
        deployment.id === String(deploymentId) &&
        deployment.site_id === String(siteId)
          ? {
              ...deployment,
              disabled_at: String(disabledAt),
              status: 'disabled',
              updated_at: String(updatedAt),
            }
          : deployment,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("status = 'active'")
    ) {
      const [activatedAt, updatedAt, deploymentId, siteId] = this.values

      this.store.deployments = this.store.deployments.map(deployment =>
        deployment.id === String(deploymentId) &&
        deployment.site_id === String(siteId)
          ? {
              ...deployment,
              activated_at: String(activatedAt),
              disabled_at: null,
              rolled_back_at: null,
              status: 'active',
              updated_at: String(updatedAt),
            }
          : deployment,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_deployments')) {
      const [
        id,
        siteId,
        versionId,
        slug,
        url,
        runtimeKind,
        runtimeScriptName,
        dispatchNamespace,
        deployedByUserId,
        externalDeploymentId,
        startedAt,
        activatedAt,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.deployments = [
        ...this.store.deployments,
        {
          activated_at: activatedAt === null ? null : String(activatedAt),
          created_at: String(createdAt),
          deployed_by_user_id:
            deployedByUserId === null ? null : String(deployedByUserId),
          disabled_at: null,
          dispatch_namespace:
            dispatchNamespace === null ? null : String(dispatchNamespace),
          external_deployment_id:
            externalDeploymentId === null ? null : String(externalDeploymentId),
          failed_at: null,
          id: String(id),
          rolled_back_at: null,
          runtime_kind: String(runtimeKind),
          runtime_script_name:
            runtimeScriptName === null ? null : String(runtimeScriptName),
          site_id: String(siteId),
          slug: String(slug),
          started_at: startedAt === null ? null : String(startedAt),
          status: 'active',
          updated_at: String(updatedAt),
          url: String(url),
          version_id: String(versionId),
        },
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_deployment_attempts')) {
      const [
        id,
        siteId,
        versionId,
        deploymentId,
        runtimeKind,
        runtimeScriptName,
        dispatchNamespace,
        externalDeploymentId,
        status,
        uploadReceiptRef,
        healthStatus,
        healthUrl,
        healthRef,
        rollbackRef,
        observabilityRef,
        metadataJson,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.deploymentAttempts = [
        ...this.store.deploymentAttempts,
        {
          archived_at: null,
          created_at: String(createdAt),
          deployment_id: deploymentId === null ? null : String(deploymentId),
          dispatch_namespace:
            dispatchNamespace === null ? null : String(dispatchNamespace),
          external_deployment_id:
            externalDeploymentId === null ? null : String(externalDeploymentId),
          health_ref: healthRef === null ? null : String(healthRef),
          health_status: String(healthStatus),
          health_url: healthUrl === null ? null : String(healthUrl),
          id: String(id),
          metadata_json: String(metadataJson),
          observability_ref:
            observabilityRef === null ? null : String(observabilityRef),
          rollback_ref: rollbackRef === null ? null : String(rollbackRef),
          runtime_kind: String(runtimeKind),
          runtime_script_name:
            runtimeScriptName === null ? null : String(runtimeScriptName),
          site_id: String(siteId),
          status: String(status),
          updated_at: String(updatedAt),
          upload_receipt_ref:
            uploadReceiptRef === null ? null : String(uploadReceiptRef),
          version_id: String(versionId),
        },
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_environment_values')) {
      const [
        id,
        siteId,
        key,
        kind,
        secretRef,
        plainValue,
        createdAt,
        updatedAt,
      ] = this.values
      const nextValue: StoredSiteEnvironmentValue = {
        created_at: String(createdAt),
        deleted_at: null,
        id: String(id),
        key: String(key),
        kind: String(kind),
        plain_value: plainValue === null ? null : String(plainValue),
        secret_ref: secretRef === null ? null : String(secretRef),
        site_id: String(siteId),
        updated_at: String(updatedAt),
      }

      this.store.environmentValues = [
        ...this.store.environmentValues.filter(
          value =>
            !(value.site_id === nextValue.site_id && value.key === nextValue.key),
        ),
        nextValue,
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_access_grants')) {
      const [id, siteId, principalKind, principalRef, role, createdAt] =
        this.values
      const nextGrant: StoredSiteAccessGrant = {
        created_at: String(createdAt),
        id: String(id),
        principal_kind: String(principalKind),
        principal_ref: String(principalRef),
        revoked_at: null,
        role: String(role),
        site_id: String(siteId),
      }

      this.store.accessGrants = [
        ...this.store.accessGrants.filter(
          grant =>
            !(
              grant.site_id === nextGrant.site_id &&
              grant.principal_kind === nextGrant.principal_kind &&
              grant.principal_ref === nextGrant.principal_ref &&
              grant.role === nextGrant.role
            ),
        ),
        nextGrant,
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes("SET status = 'generating'")
    ) {
      const [updatedAt, siteId] = this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              status: 'generating',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes("SET status = 'needs_review'")
    ) {
      const [updatedAt, siteId] = this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              status: 'needs_review',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes("SET status = 'disabled'")
    ) {
      const [updatedAt, siteId] = this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              status: 'disabled',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes("SET status = 'approved'")
    ) {
      const [activeVersionId, activeDeploymentId, updatedAt, siteId] =
        this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              active_deployment_id: String(activeDeploymentId),
              active_version_id: String(activeVersionId),
              status: 'approved',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE site_projects')) {
      const [activeVersionId, activeDeploymentId, updatedAt, siteId] =
        this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              active_deployment_id: String(activeDeploymentId),
              active_version_id: String(activeVersionId),
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_events')) {
      const siteId = String(this.values[0])
      const limit = Number(this.values[1])
      const rows = this.store.events
        .filter(event => event.site_id === siteId)
        .sort((left, right) => right.created_at.localeCompare(left.created_at))
        .slice(0, limit)

      return Promise.resolve({
        results: rows as unknown as ReadonlyArray<T>,
        success: true,
      } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(): Promise<[string[], ...T[]] | T[]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

const sitesDb = (store: SitesDbStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new SitesStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runSites = <A>(
  store: SitesDbStore,
  effect: Effect.Effect<A, unknown, AutopilotSitesService>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(
        AutopilotSitesService.layer(
          { OPENAGENTS_DB: sitesDb(store) },
          store.runtime,
        ),
      ),
    ),
  )

const runSitesWithArtifacts = <A>(
  store: SitesDbStore,
  artifacts: SitesArtifactsBucket,
  effect: Effect.Effect<A, unknown, AutopilotSitesService>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(
        AutopilotSitesService.layer(
          {
            ARTIFACTS: artifacts as unknown as R2Bucket,
            OPENAGENTS_DB: sitesDb(store),
          },
          store.runtime,
        ),
      ),
    ),
  )

const createOtecSite = (store: SitesDbStore) =>
  runSites(
    store,
    Effect.gen(function* () {
      const sites = yield* AutopilotSitesService

      return yield* sites.createProjectFromSoftwareOrder({
        actorUserId: 'github:operator',
        slug: 'otec',
        softwareOrderId: 'software_order_ben_otec',
        title: 'OTEC Floating Datacenter',
      })
    }),
  )

const staticManifest = {
  assets: {
    'index.html': {
      cacheControl: 'public, max-age=120',
      contentType: 'text/html; charset=utf-8',
      r2Key: 'sites/otec/deployments/site_deployment_1/index.html',
    },
  },
}

const launchChecklist = {
  audienceReviewed: true,
  buildReviewed: true,
  secretsReviewed: true,
  sourceReviewed: true,
  urlReviewed: true,
}

const saveOtecVersion = (
  store: SitesDbStore,
  artifacts: SitesArtifactsBucket,
  buildStatus: 'build_failed' | 'saved' = 'saved',
) =>
  runSitesWithArtifacts(
    store,
    artifacts,
    Effect.gen(function* () {
      const sites = yield* AutopilotSitesService

      return yield* sites.saveVersion({
        actorUserId: 'github:operator',
        buildCommand: 'bun run build',
        buildLogText:
          'Build succeeded with GitHub token gho_abcdefghijklmnopqrstuvwxyz.',
        buildStatus,
        metadata: { reviewed: buildStatus === 'saved' },
        siteId: 'site_project_1',
        sourceArchiveText: 'export default "ok"',
        sourceKind: 'operator_static',
        staticAssetsManifest: staticManifest,
      })
    }),
  )

describe('Autopilot Sites service', () => {
  test('creates a public Site project from the Ben OTEC software order', async () => {
    const store = new SitesDbStore()
    const project = await createOtecSite(store)

    expect(project).toEqual({
      accessMode: 'public',
      activeDeploymentId: null,
      activeVersionId: null,
      archivedAt: null,
      createdAt: '2026-06-04T21:00:00.000Z',
      id: 'site_project_1',
      ownerUserId: 'github:ben',
      projectId: null,
      prompt:
        'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
      slug: 'otec',
      softwareOrderId: 'software_order_ben_otec',
      sourceRepository: {
        name: 'openagents',
        owner: 'bensilone',
        provider: 'github',
        ref: 'main',
      },
      status: 'draft',
      teamId: null,
      title: 'OTEC Floating Datacenter',
      updatedAt: '2026-06-04T21:00:00.000Z',
      visibility: 'public',
    })
    expect(store.projects).toHaveLength(1)
    expect(store.events).toHaveLength(1)
    expect(store.events[0]?.type).toBe('site_project.created')
    expect(store.events[0]?.payload_json).toBe(
      '{"accessMode":"public","softwareOrderId":"software_order_ben_otec","slug":"otec","visibility":"public"}',
    )
  })

  test('returns the existing Site when the software order is already linked', async () => {
    const store = new SitesDbStore()
    const project = await createOtecSite(store)
    const secondProject = await createOtecSite(store)

    expect(secondProject).toEqual(project)
    expect(store.projects).toHaveLength(1)
    expect(store.events).toHaveLength(1)
  })

  test('rejects an unavailable active slug for a different order', async () => {
    const store = new SitesDbStore()
    await createOtecSite(store)
    store.softwareOrders = [
      ...store.softwareOrders,
      {
        archived_at: null,
        id: 'software_order_other',
        repository_default_branch: 'main',
        repository_name: 'autopilot-omega',
        repository_owner: 'OpenAgentsInc',
        repository_provider: 'github',
        request: 'Testing',
        user_id: 'github:chris',
      },
    ]

    await expect(
      runSites(
        store,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.createProjectFromSoftwareOrder({
            slug: 'otec',
            softwareOrderId: 'software_order_other',
            title: 'Duplicate OTEC',
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteSlugUnavailable)
  })

  test('updates the active version and deployment references', async () => {
    const store = new SitesDbStore()
    const project = await createOtecSite(store)

    await runSites(
      store,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        yield* sites.markActiveDeployment({
          actorUserId: 'github:operator',
          deploymentId: 'site_deployment_1',
          siteId: project.id,
          versionId: 'site_version_1',
        })

        return yield* sites.readProjectBySlug('otec')
      }),
    )

    expect(store.projects[0]?.active_version_id).toBe('site_version_1')
    expect(store.projects[0]?.active_deployment_id).toBe('site_deployment_1')
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_deployment.activated',
    ])
  })

  test('rejects secret-shaped event payloads before D1 persistence', async () => {
    const store = new SitesDbStore()
    const project = await createOtecSite(store)

    await expect(
      runSites(
        store,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.recordEvent({
            payload: { token: 'sk-abcdefghijklmnopqrstuvwxyz' },
            siteId: project.id,
            summary: 'Unsafe payload test.',
            type: 'site_event.unsafe_test',
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteUnsafePayload)
    expect(store.events).toHaveLength(1)
  })

  test('saves a reviewable Site version with durable redacted R2 refs', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)

    const version = await saveOtecVersion(store, artifacts)

    expect(version).toMatchObject({
      artifactManifestR2Key:
        'sites/site_project_1/versions/site_version_1/static-assets-manifest.json',
      buildLogR2Key: 'sites/site_project_1/versions/site_version_1/build.log',
      buildStatus: 'saved',
      id: 'site_version_1',
      savedAt: '2026-06-04T21:00:00.000Z',
      siteId: 'site_project_1',
      sourceArchiveR2Key:
        'sites/site_project_1/versions/site_version_1/source.txt',
    })
    expect(
      artifacts.objects.get(
        'sites/site_project_1/versions/site_version_1/build.log',
      ),
    ).toContain('gho_[REDACTED]')
    expect(
      artifacts.objects.get(
        'sites/site_project_1/versions/site_version_1/build.log',
      ),
    ).not.toContain('gho_abcdefghijklmnopqrstuvwxyz')
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_version.saved',
    ])
  })

  test('records declared D1 and R2 binding metadata when saving versions', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)

    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.saveVersion({
          buildStatus: 'saved',
          d1BindingName: 'SITE_DB',
          r2BindingName: 'SITE_ASSETS',
          siteId: 'site_project_1',
          sourceKind: 'autopilot_generated',
          staticAssetsManifest: staticManifest,
          workerModuleR2Key:
            'sites/site_project_1/versions/site_version_1/worker.mjs',
        })
      }),
    )

    expect(store.storageBindings).toEqual([
      {
        binding_name: 'SITE_DB',
        cloudflare_resource_ref: null,
        created_at: '2026-06-04T21:00:00.000Z',
        id: 'site_storage_binding:site_project_1:d1:SITE_DB',
        kind: 'd1',
        scope: 'shared_prefix',
        site_id: 'site_project_1',
        updated_at: '2026-06-04T21:00:00.000Z',
      },
      {
        binding_name: 'SITE_ASSETS',
        cloudflare_resource_ref: null,
        created_at: '2026-06-04T21:00:00.000Z',
        id: 'site_storage_binding:site_project_1:r2:SITE_ASSETS',
        kind: 'r2',
        scope: 'shared_prefix',
        site_id: 'site_project_1',
        updated_at: '2026-06-04T21:00:00.000Z',
      },
    ])
  })

  test('stores generated worker module text when saving versions', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.saveVersion({
          buildStatus: 'saved',
          siteId: 'site_project_1',
          sourceKind: 'autopilot_generated',
          staticAssetsManifest: staticManifest,
          workerModuleText:
            'export default { fetch() { return new Response("ok") } }',
        })
      }),
    )

    expect(version.workerModuleR2Key).toBe(
      'sites/site_project_1/versions/site_version_1/worker.mjs',
    )
    expect(artifacts.objects.get(version.workerModuleR2Key ?? '')).toBe(
      'export default { fetch() { return new Response("ok") } }',
    )
  })

  test('rejects secret-shaped source archives before storing version artifacts', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.saveVersion({
            buildStatus: 'saved',
            siteId: 'site_project_1',
            sourceArchiveText: 'token=sk-abcdefghijklmnopqrstuvwxyz',
            sourceKind: 'operator_static',
            staticAssetsManifest: staticManifest,
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteUnsafePayload)
    expect(store.versions).toHaveLength(0)
    expect(artifacts.objects.size).toBe(0)
  })

  test('rejects secret-shaped worker module text before storing version artifacts', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.saveVersion({
            buildStatus: 'saved',
            siteId: 'site_project_1',
            sourceKind: 'autopilot_generated',
            staticAssetsManifest: staticManifest,
            workerModuleText:
              'export default { token: "sk-abcdefghijklmnopqrstuvwxyz" }',
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteUnsafePayload)
    expect(store.versions).toHaveLength(0)
    expect(artifacts.objects.size).toBe(0)
  })

  test('does not deploy a failed build version', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts, 'build_failed')

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.deployVersion({
            actorUserId: 'github:operator',
            siteId: 'site_project_1',
            versionId: version.id,
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteVersionNotDeployable)
    expect(store.deployments).toHaveLength(0)
  })

  test('deploys a saved version as the active Site deployment', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts)

    const deployment = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          actorUserId: 'github:operator',
          launchChecklist,
          siteId: 'site_project_1',
          versionId: version.id,
        })
      }),
    )

    expect(deployment).toMatchObject({
      id: 'site_deployment_1',
      siteId: 'site_project_1',
      status: 'active',
      url: 'https://sites.openagents.com/otec',
      versionId: 'site_version_1',
    })
    expect(store.projects[0]?.active_deployment_id).toBe('site_deployment_1')
    expect(store.projects[0]?.active_version_id).toBe('site_version_1')
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_version.saved',
      'site_deployment.activated',
    ])
  })

  test('requires a completed launch checklist for public deployments', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts)

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.deployVersion({
            siteId: 'site_project_1',
            versionId: version.id,
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteLaunchChecklistRequired)
    expect(store.deployments).toHaveLength(0)
  })

  test('stores secret environment refs only and marks active Sites for redeploy', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts)
    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: version.id,
        })
      }),
    )

    const value = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.upsertEnvironmentValue({
          actorUserId: 'github:operator',
          key: 'OPENAI_API_KEY',
          kind: 'secret',
          secretRef: 'cf-secret:sites/otec/openai-api-key',
          siteId: 'site_project_1',
        })
      }),
    )

    expect(value).toMatchObject({
      key: 'OPENAI_API_KEY',
      kind: 'secret',
      plainValue: null,
      secretRef: 'cf-secret:sites/otec/openai-api-key',
    })
    expect(store.environmentValues[0]).toMatchObject({
      key: 'OPENAI_API_KEY',
      kind: 'secret',
      plain_value: null,
      secret_ref: 'cf-secret:sites/otec/openai-api-key',
    })
    expect(store.projects[0]?.status).toBe('needs_review')
    expect(store.events.at(-1)?.payload_json).toBe(
      '{"key":"OPENAI_API_KEY","kind":"secret","requiresRedeploy":true,"secretRef":"[SECRET_REF]"}',
    )
  })

  test('rejects secret-shaped plain environment values', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.upsertEnvironmentValue({
            key: 'TOKEN',
            kind: 'plain',
            plainValue: 'sk-abcdefghijklmnopqrstuvwxyz',
            siteId: 'site_project_1',
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteUnsafePayload)
    expect(store.environmentValues).toHaveLength(0)
  })

  test('records custom access grants and exposes redacted audit events', async () => {
    const store = new SitesDbStore()
    await createOtecSite(store)

    const grant = await runSites(
      store,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.grantAccess({
          actorUserId: 'github:operator',
          principalKind: 'user',
          principalRef: 'github:ben',
          role: 'viewer',
          siteId: 'site_project_1',
        })
      }),
    )
    const events = await runSites(
      store,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.listEvents('site_project_1', 10)
      }),
    )

    expect(grant).toMatchObject({
      principalKind: 'user',
      principalRef: 'github:ben',
      role: 'viewer',
    })
    expect(store.accessGrants).toHaveLength(1)
    expect(events.find(event => event.type === 'site_access.granted')).toMatchObject({
      summary: 'Granted viewer access to user.',
      type: 'site_access.granted',
    })
    expect('payloadJson' in events[0]!).toBe(false)
  })

  test('deploys a saved Worker module with dispatch runtime metadata', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.saveVersion({
          buildStatus: 'saved',
          d1BindingName: 'SITE_DB',
          r2BindingName: 'SITE_ASSETS',
          siteId: 'site_project_1',
          sourceKind: 'autopilot_generated',
          staticAssetsManifest: staticManifest,
          workerModuleR2Key:
            'sites/site_project_1/versions/site_version_1/worker.mjs',
        })
      }),
    )

    const deployment = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          dispatchNamespace: 'openagents-sites-production',
          externalDeploymentId: 'cf-deployment-otec',
          healthCheck: {
            healthRef: 'health:otec:passed',
            status: 'passed',
            url: 'https://sites.openagents.com/otec/__health',
          },
          launchChecklist,
          observabilityRef: 'observability:workers:otec',
          rollbackRef: 'rollback:site_deployment_previous',
          runtimeKind: 'workers_for_platforms',
          runtimeScriptName: 'site-worker-otec',
          siteId: 'site_project_1',
          tags: ['site:site_project_1', 'version:site_version_1'],
          uploadReceiptRef: 'cf-upload:site-worker-otec',
          versionId: version.id,
        })
      }),
    )

    expect(deployment).toMatchObject({
      dispatchNamespace: 'openagents-sites-production',
      externalDeploymentId: 'cf-deployment-otec',
      runtimeKind: 'workers_for_platforms',
      runtimeScriptName: 'site-worker-otec',
      status: 'active',
    })
    expect(store.deployments[0]).toMatchObject({
      dispatch_namespace: 'openagents-sites-production',
      external_deployment_id: 'cf-deployment-otec',
      runtime_kind: 'workers_for_platforms',
      runtime_script_name: 'site-worker-otec',
      version_id: 'site_version_1',
    })
    expect(store.deploymentAttempts[0]).toMatchObject({
      deployment_id: 'site_deployment_1',
      dispatch_namespace: 'openagents-sites-production',
      health_ref: 'health:otec:passed',
      health_status: 'passed',
      observability_ref: 'observability:workers:otec',
      rollback_ref: 'rollback:site_deployment_previous',
      runtime_kind: 'workers_for_platforms',
      runtime_script_name: 'site-worker-otec',
      status: 'activated',
      upload_receipt_ref: 'cf-upload:site-worker-otec',
      version_id: 'site_version_1',
    })
    expect(JSON.parse(store.deploymentAttempts[0]?.metadata_json ?? '{}')).toEqual({
      healthCheckedAt: null,
      healthSummary: null,
      tags: ['site:site_project_1', 'version:site_version_1'],
    })
    expect(store.projects[0]?.active_deployment_id).toBe('site_deployment_1')
    expect(store.projects[0]?.active_version_id).toBe('site_version_1')
  })

  test('does not activate Workers for Platforms deployments with failed health checks', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.saveVersion({
          buildStatus: 'saved',
          siteId: 'site_project_1',
          sourceKind: 'autopilot_generated',
          staticAssetsManifest: staticManifest,
          workerModuleR2Key:
            'sites/site_project_1/versions/site_version_1/worker.mjs',
        })
      }),
    )

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.deployVersion({
            dispatchNamespace: 'openagents-sites-production',
            externalDeploymentId: 'cf-deployment-otec',
            healthCheck: {
              healthRef: 'health:otec:failed',
              status: 'failed',
              summary: 'Preview returned 500.',
              url: 'https://sites.openagents.com/otec/__health',
            },
            launchChecklist,
            runtimeKind: 'workers_for_platforms',
            runtimeScriptName: 'site-worker-otec',
            siteId: 'site_project_1',
            uploadReceiptRef: 'cf-upload:site-worker-otec',
            versionId: version.id,
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteRuntimeNotDeployable)

    expect(store.deployments).toHaveLength(0)
    expect(store.projects[0]?.active_deployment_id).toBeNull()
    expect(store.projects[0]?.active_version_id).toBeNull()
    expect(store.deploymentAttempts[0]).toMatchObject({
      deployment_id: null,
      health_ref: 'health:otec:failed',
      health_status: 'failed',
      status: 'health_failed',
      upload_receipt_ref: 'cf-upload:site-worker-otec',
    })
  })

  test('rejects Workers for Platforms deploys without a Worker module artifact', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts)

    await expect(
      runSitesWithArtifacts(
        store,
        artifacts,
        Effect.gen(function* () {
          const sites = yield* AutopilotSitesService

          return yield* sites.deployVersion({
            dispatchNamespace: 'openagents-sites-production',
            launchChecklist,
            runtimeKind: 'workers_for_platforms',
            runtimeScriptName: 'site-worker-otec',
            siteId: 'site_project_1',
            versionId: version.id,
          })
        }),
      ),
    ).rejects.toBeInstanceOf(AutopilotSiteRuntimeNotDeployable)
    expect(store.deployments).toHaveLength(0)
  })

  test('switches active deployments and records rollback metadata', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const firstVersion = await saveOtecVersion(store, artifacts)
    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: firstVersion.id,
        })
      }),
    )
    const secondVersion = await saveOtecVersion(store, artifacts)

    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: secondVersion.id,
        })
      }),
    )

    expect(store.deployments.map(deployment => deployment.status)).toEqual([
      'rolled_back',
      'active',
    ])
    expect(store.deployments[0]?.rolled_back_at).toBe(
      '2026-06-04T21:00:00.000Z',
    )
    expect(store.projects[0]?.active_deployment_id).toBe('site_deployment_2')
    expect(store.events.at(-1)?.payload_json).toBe(
      '{"dispatchNamespace":null,"deploymentId":"site_deployment_2","externalDeploymentId":null,"healthRef":null,"healthStatus":"not_required","observabilityRef":null,"previousDeploymentId":"site_deployment_1","rollbackRef":null,"runtimeKind":"omega_static_r2","runtimeScriptName":null,"tags":[],"uploadReceiptRef":null,"url":"https://sites.openagents.com/otec","versionId":"site_version_2"}',
    )
  })

  test('disables an active deployment and records the receipt', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const version = await saveOtecVersion(store, artifacts)
    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: version.id,
        })
      }),
    )

    const deployment = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.disableDeployment({
          actorUserId: 'github:operator',
          deploymentId: 'site_deployment_1',
          siteId: 'site_project_1',
        })
      }),
    )

    expect(deployment.status).toBe('disabled')
    expect(deployment.disabledAt).toBe('2026-06-04T21:00:00.000Z')
    expect(store.projects[0]?.status).toBe('disabled')
    expect(store.events.at(-1)).toMatchObject({
      deployment_id: 'site_deployment_1',
      type: 'site_deployment.disabled',
      version_id: 'site_version_1',
    })
  })

  test('rolls back to a previous deployment and records the receipt', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    const firstVersion = await saveOtecVersion(store, artifacts)
    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: firstVersion.id,
        })
      }),
    )
    const secondVersion = await saveOtecVersion(store, artifacts)
    await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.deployVersion({
          launchChecklist,
          siteId: 'site_project_1',
          versionId: secondVersion.id,
        })
      }),
    )

    const deployment = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.rollbackDeployment({
          actorUserId: 'github:operator',
          deploymentId: 'site_deployment_1',
          siteId: 'site_project_1',
        })
      }),
    )

    expect(deployment.status).toBe('active')
    expect(store.deployments.map(item => item.status)).toEqual([
      'active',
      'rolled_back',
    ])
    expect(store.projects[0]).toMatchObject({
      active_deployment_id: 'site_deployment_1',
      active_version_id: 'site_version_1',
      status: 'approved',
    })
    expect(store.events.at(-1)?.payload_json).toBe(
      '{"previousDeploymentId":"site_deployment_2","targetDeploymentId":"site_deployment_1","targetVersionId":"site_version_1"}',
    )
  })

  test('prepares an OTEC generation packet and links the requested run', async () => {
    const store = new SitesDbStore()
    await createOtecSite(store)

    const packet = await runSites(
      store,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.requestGeneration({
          actorRunId: 'agent_run_otec',
          actorUserId: 'github:operator',
          operatorNotes: 'Prepare the June 5 OTEC launch artifact.',
          siteId: 'site_project_1',
        })
      }),
    )

    expect(packet).toMatchObject({
      publicUrl: 'https://sites.openagents.com/otec',
      siteId: 'site_project_1',
      slug: 'otec',
      softwareOrderId: 'software_order_ben_otec',
    })
    expect(packet.generationGoal).toContain(
      'Website for ocean based, OTEC powered, SWAC cooled',
    )
    expect(packet.preflightChecklist).toContain(
      'Run the operator Autopilot preflight before dispatch.',
    )
    expect(store.projects[0]?.status).toBe('generating')
    expect(store.events.at(-1)).toMatchObject({
      actor_run_id: 'agent_run_otec',
      actor_user_id: 'github:operator',
      type: 'site_generation.requested',
    })
  })

  test('saves generated work through the normal version lifecycle', async () => {
    const store = new SitesDbStore()
    const artifacts = new SitesArtifactsBucket()
    await createOtecSite(store)
    await runSites(
      store,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.requestGeneration({
          siteId: 'site_project_1',
        })
      }),
    )

    const version = await runSitesWithArtifacts(
      store,
      artifacts,
      Effect.gen(function* () {
        const sites = yield* AutopilotSitesService

        return yield* sites.saveVersion({
          buildStatus: 'saved',
          siteId: 'site_project_1',
          sourceKind: 'autopilot_generated',
          staticAssetsManifest: staticManifest,
        })
      }),
    )

    expect(version.sourceKind).toBe('autopilot_generated')
    expect(version.buildStatus).toBe('saved')
  })
})
