import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorSitesRoutes } from './operator-sites-routes'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

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

type StoredSiteCompatibilityCheck = Readonly<{
  blockers_json: string
  build_command: string | null
  checked_by_user_id: string | null
  confidence: string
  created_at: string
  customer_safe_next_action: string
  customer_safe_status: string
  env_keys_json: string
  evidence_refs_json: string
  findings_json: string
  id: string
  needs_d1: number
  needs_public_auth: number
  needs_r2: number
  needs_workspace_auth: number
  output_kind: string
  output_path: string | null
  package_manager: string | null
  site_id: string
  source_kind: string
  source_repository_json: string | null
  status: string
  warnings_json: string
  worker_module_path: string | null
}>

type StoredSiteBuildValidation = Readonly<{
  blockers_json: string
  bounded_logs_json: string
  build_command: string | null
  compatibility_check_id: string | null
  created_at: string
  customer_safe_next_action: string
  customer_safe_status: string
  evidence_refs_json: string
  findings_json: string
  id: string
  log_line_count: number
  log_truncated: number
  manifest_json: string
  output_kind: string
  output_path: string | null
  package_manager: string | null
  requested_build_command: string | null
  site_id: string
  source_commit_sha: string | null
  source_hash: string
  source_kind: string
  source_repository_json: string | null
  status: string
  validated_by_user_id: string | null
  warnings_json: string
  worker_module_path: string | null
}>

type StoredSiteSourceExport = Readonly<{
  actor_user_id: string | null
  approved_by_user_id: string | null
  archived_at: string | null
  artifact_manifest_r2_key: string | null
  created_at: string
  destination_branch: string | null
  destination_owner: string | null
  destination_provider: string
  destination_pull_request_url: string | null
  destination_repository: string | null
  destination_url: string | null
  export_kind: string
  id: string
  idempotency_key: string
  receipt_json: string
  secret_scan_ref: string | null
  secret_scan_status: string
  site_id: string
  source_archive_r2_key: string | null
  source_artifact_ref: string | null
  status: string
  token_expires_at: string | null
  token_hash: string | null
  token_ref: string | null
  updated_at: string
  version_id: string
  worker_module_r2_key: string | null
}>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

class OperatorSitesDbStore {
  buildValidations: Array<StoredSiteBuildValidation> = []
  compatibilityChecks: Array<StoredSiteCompatibilityCheck> = []
  deploymentAttempts: Array<Record<string, unknown>> = []
  deployments: Array<StoredSiteDeployment> = []
  events: Array<StoredSiteEvent> = []
  projects: Array<StoredSiteProject> = []
  provisioningPlans: Array<Record<string, unknown>> = []
  sourceExports: Array<StoredSiteSourceExport> = []
  softwareOrders: Array<StoredSoftwareOrder> = [
    {
      archived_at: null,
      id: 'software_order_ben_otec',
      repository_default_branch: 'main',
      repository_name: 'openagents',
      repository_owner: 'bensilone',
      repository_provider: 'github',
      request:
        'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
      user_id: 'github:ben',
    },
    {
      archived_at: null,
      id: 'software_order_testing',
      repository_default_branch: 'main',
      repository_name: 'autopilot-omega',
      repository_owner: 'OpenAgentsInc',
      repository_provider: 'github',
      request: 'Testing surface for Autopilot Sites.',
      user_id: 'github:chris',
    },
  ]
  versions: Array<StoredSiteVersion> = []
}

class OperatorSitesStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OperatorSitesDbStore,
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

    if (this.query.includes('FROM site_provisioning_plans')) {
      const idempotencyKey = String(this.values[0])
      const plan =
        this.store.provisioningPlans.find(
          item =>
            item.idempotency_key === idempotencyKey && item.archived_at === null,
        ) ?? null

      return Promise.resolve((plan as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_source_exports')) {
      const idempotencyKey = String(this.values[0])
      const sourceExport =
        this.store.sourceExports.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve((sourceExport as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_compatibility_checks')) {
      const siteId = String(this.values[0])
      const receipt =
        this.store.compatibilityChecks.find(item => item.site_id === siteId) ??
        null

      return Promise.resolve((receipt as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_build_validations')) {
      const siteId = String(this.values[0])
      const receipt =
        this.store.buildValidations.find(item => item.site_id === siteId) ??
        null

      return Promise.resolve((receipt as T | undefined) ?? null)
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
      const eventType = this.query.includes('site_build_validation.checked')
        ? 'site_build_validation.checked'
        : 'site_compatibility.checked'
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
      ] =
        this.values.length === 6
          ? [
              this.values[0],
              this.values[1],
              null,
              null,
              eventType,
              this.values[2],
              this.values[3],
              null,
              this.values[4],
              this.values[5],
            ]
          : this.values

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

    if (this.query.includes('INSERT INTO site_build_validations')) {
      const [
        id,
        siteId,
        compatibilityCheckId,
        sourceKind,
        sourceRepositoryJson,
        sourceCommitSha,
        sourceHash,
        status,
        packageManager,
        requestedBuildCommand,
        buildCommand,
        outputKind,
        outputPath,
        workerModulePath,
        manifestJson,
        boundedLogsJson,
        logLineCount,
        logTruncated,
        findingsJson,
        blockersJson,
        warningsJson,
        evidenceRefsJson,
        customerSafeStatus,
        customerSafeNextAction,
        validatedByUserId,
        createdAt,
      ] = this.values

      this.store.buildValidations.unshift({
        blockers_json: String(blockersJson),
        bounded_logs_json: String(boundedLogsJson),
        build_command: buildCommand === null ? null : String(buildCommand),
        compatibility_check_id:
          compatibilityCheckId === null ? null : String(compatibilityCheckId),
        created_at: String(createdAt),
        customer_safe_next_action: String(customerSafeNextAction),
        customer_safe_status: String(customerSafeStatus),
        evidence_refs_json: String(evidenceRefsJson),
        findings_json: String(findingsJson),
        id: String(id),
        log_line_count: Number(logLineCount),
        log_truncated: Number(logTruncated),
        manifest_json: String(manifestJson),
        output_kind: String(outputKind),
        output_path: outputPath === null ? null : String(outputPath),
        package_manager: packageManager === null ? null : String(packageManager),
        requested_build_command:
          requestedBuildCommand === null ? null : String(requestedBuildCommand),
        site_id: String(siteId),
        source_commit_sha:
          sourceCommitSha === null ? null : String(sourceCommitSha),
        source_hash: String(sourceHash),
        source_kind: String(sourceKind),
        source_repository_json:
          sourceRepositoryJson === null ? null : String(sourceRepositoryJson),
        status: String(status),
        validated_by_user_id:
          validatedByUserId === null ? null : String(validatedByUserId),
        warnings_json: String(warningsJson),
        worker_module_path:
          workerModulePath === null ? null : String(workerModulePath),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_compatibility_checks')) {
      const [
        id,
        siteId,
        sourceKind,
        sourceRepositoryJson,
        status,
        confidence,
        packageManager,
        buildCommand,
        outputKind,
        outputPath,
        workerModulePath,
        needsD1,
        needsR2,
        needsWorkspaceAuth,
        needsPublicAuth,
        envKeysJson,
        findingsJson,
        blockersJson,
        warningsJson,
        evidenceRefsJson,
        customerSafeStatus,
        customerSafeNextAction,
        checkedByUserId,
        createdAt,
      ] = this.values

      this.store.compatibilityChecks.unshift({
        blockers_json: String(blockersJson),
        build_command: buildCommand === null ? null : String(buildCommand),
        checked_by_user_id:
          checkedByUserId === null ? null : String(checkedByUserId),
        confidence: String(confidence),
        created_at: String(createdAt),
        customer_safe_next_action: String(customerSafeNextAction),
        customer_safe_status: String(customerSafeStatus),
        env_keys_json: String(envKeysJson),
        evidence_refs_json: String(evidenceRefsJson),
        findings_json: String(findingsJson),
        id: String(id),
        needs_d1: Number(needsD1),
        needs_public_auth: Number(needsPublicAuth),
        needs_r2: Number(needsR2),
        needs_workspace_auth: Number(needsWorkspaceAuth),
        output_kind: String(outputKind),
        output_path: outputPath === null ? null : String(outputPath),
        package_manager: packageManager === null ? null : String(packageManager),
        site_id: String(siteId),
        source_kind: String(sourceKind),
        source_repository_json:
          sourceRepositoryJson === null ? null : String(sourceRepositoryJson),
        status: String(status),
        warnings_json: String(warningsJson),
        worker_module_path:
          workerModulePath === null ? null : String(workerModulePath),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("SET status = 'rolled_back'")
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
      this.store.deploymentAttempts.push({
        archived_at: null,
        created_at: String(this.values[16]),
        deployment_id: this.values[3] as string | null,
        dispatch_namespace: this.values[6] as string | null,
        external_deployment_id: this.values[7] as string | null,
        health_ref: this.values[12] as string | null,
        health_status: String(this.values[10]),
        health_url: this.values[11] as string | null,
        id: String(this.values[0]),
        metadata_json: String(this.values[15]),
        observability_ref: this.values[14] as string | null,
        rollback_ref: this.values[13] as string | null,
        runtime_kind: String(this.values[4]),
        runtime_script_name: this.values[5] as string | null,
        site_id: String(this.values[1]),
        status: String(this.values[8]),
        updated_at: String(this.values[17]),
        upload_receipt_ref: this.values[9] as string | null,
        version_id: String(this.values[2]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO site_provisioning_plans')) {
      if (
        this.store.provisioningPlans.some(
          item => item.idempotency_key === this.values[1],
        )
      ) {
        return Promise.resolve({ success: true } as D1Result<T>)
      }

      this.store.provisioningPlans.push({
        archived_at: null,
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        receipt_json: String(this.values[7]),
        requested_by_user_id: this.values[4] as string | null,
        resource_manifest_json: String(this.values[6]),
        reviewed_at: this.values[9] as string | null,
        reviewed_by_user_id: this.values[5] as string | null,
        site_id: String(this.values[2]),
        status: String(this.values[3]),
        updated_at: String(this.values[10]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_source_exports')) {
      this.store.sourceExports.push({
        actor_user_id: this.values[6] as string | null,
        approved_by_user_id: this.values[7] as string | null,
        archived_at: null,
        artifact_manifest_r2_key: this.values[15] as string | null,
        created_at: String(this.values[24]),
        destination_branch: this.values[11] as string | null,
        destination_owner: this.values[9] as string | null,
        destination_provider: String(this.values[8]),
        destination_pull_request_url: this.values[12] as string | null,
        destination_repository: this.values[10] as string | null,
        destination_url: this.values[13] as string | null,
        export_kind: String(this.values[5]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        receipt_json: String(this.values[23]),
        secret_scan_ref: this.values[22] as string | null,
        secret_scan_status: String(this.values[21]),
        site_id: String(this.values[2]),
        source_archive_r2_key: this.values[14] as string | null,
        source_artifact_ref: this.values[17] as string | null,
        status: String(this.values[4]),
        token_expires_at: this.values[20] as string | null,
        token_hash: this.values[19] as string | null,
        token_ref: this.values[18] as string | null,
        updated_at: String(this.values[25]),
        version_id: String(this.values[3]),
        worker_module_r2_key: this.values[16] as string | null,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes('active_version_id = ?')
    ) {
      const [versionId, deploymentId, updatedAt, siteId] = this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              active_deployment_id: String(deploymentId),
              active_version_id: String(versionId),
              status: 'approved',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes('SET access_mode = ?')
    ) {
      const [accessMode, visibility, updatedAt, siteId] = this.values

      this.store.projects = this.store.projects.map(project =>
        project.id === String(siteId)
          ? {
              ...project,
              access_mode: String(accessMode),
              updated_at: String(updatedAt),
              visibility: String(visibility),
            }
          : project,
      )

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

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_projects')) {
      const limit = Number(this.values[0])
      const results = this.store.projects
        .filter(project => project.archived_at === null)
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
        .slice(0, limit)
      const rows: ReadonlyArray<unknown> = results
      const result: D1Result<T> = {
        results: rows.map(row => row as T),
        success: true,
      } as D1Result<T>

      return Promise.resolve(result)
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

const operatorSitesDb = (store: OperatorSitesDbStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OperatorSitesStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const makeRoutes = (session: TestSession | null) =>
  makeOperatorSitesRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  })

const runRoute = (
  session: TestSession | null,
  store: OperatorSitesDbStore,
  request: Request,
): Promise<Response> => {
  const route = makeRoutes(session).routeOperatorSitesRequest(
    request,
    { OPENAGENTS_DB: operatorSitesDb(store) },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

const adminSession: TestSession = {
  user: {
    email: 'chris@openagents.com',
    userId: 'github:operator',
  },
}

const createOtecSiteRequest = () =>
  new Request('https://openagents.com/api/operator/sites', {
    body: JSON.stringify({
      softwareOrderId: 'software_order_ben_otec',
      slug: 'otec',
      title: 'OTEC Floating Datacenter',
    }),
    method: 'POST',
  })

describe('operator Sites API routes', () => {
  test('returns unauthorized without a browser session', async () => {
    const response = await runRoute(
      null,
      new OperatorSitesDbStore(),
      new Request('https://openagents.com/api/operator/sites'),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('returns forbidden for non-admin browser sessions', async () => {
    const response = await runRoute(
      {
        user: {
          email: 'ben@example.com',
          userId: 'github:ben',
        },
      },
      new OperatorSitesDbStore(),
      new Request('https://openagents.com/api/operator/sites'),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('creates an Autopilot Site from a software order', async () => {
    const store = new OperatorSitesDbStore()
    const response = await runRoute(
      adminSession,
      store,
      createOtecSiteRequest(),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    await expect(response.json()).resolves.toEqual({
      site: {
        accessMode: 'public',
        activeDeploymentId: null,
        activeVersionId: null,
        archivedAt: null,
        createdAt: expect.any(String),
        id: expect.stringMatching(/^site_project_/),
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
        updatedAt: expect.any(String),
        visibility: 'public',
      },
    })
    expect(store.projects).toHaveLength(1)
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
    ])
  })

  test('lists and reads operator Sites', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const listResponse = await runRoute(
      adminSession,
      store,
      new Request('https://openagents.com/api/operator/sites'),
    )
    const detailResponse = await runRoute(
      adminSession,
      store,
      new Request(`https://openagents.com/api/operator/sites/${siteId}`),
    )

    expect(listResponse.status).toBe(200)
    await expect(listResponse.json()).resolves.toEqual({
      sites: [
        expect.objectContaining({
          id: siteId,
          slug: 'otec',
        }),
      ],
    })
    expect(detailResponse.status).toBe(200)
    await expect(detailResponse.json()).resolves.toEqual({
      site: expect.objectContaining({
        id: siteId,
        slug: 'otec',
      }),
    })
  })

  test('checks and reads Site compatibility without leaking env values', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const check = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/compatibility/check`,
        {
          body: JSON.stringify({
            files: [
              {
                path: 'package.json',
                text: JSON.stringify({
                  dependencies: { vite: '^6.0.0' },
                  packageManager: 'pnpm@9.0.0',
                  scripts: { build: 'vite build' },
                }),
              },
              { path: 'vite.config.ts', text: 'export default {}' },
              { path: '.env.example', text: 'OPENAI_API_KEY=sk-test-value\n' },
            ],
          }),
          method: 'POST',
        },
      ),
    )

    expect(check.status).toBe(201)
    const checkBody = (await check.json()) as {
      compatibility: { id: string }
    }

    expect(checkBody).toEqual(
      expect.objectContaining({
        compatibility: expect.objectContaining({
          buildCommand: 'pnpm build',
          envKeys: ['OPENAI_API_KEY'],
          outputKind: 'static',
          outputPath: 'dist',
          packageManager: 'pnpm',
          siteId,
          status: 'ready',
        }),
      }),
    )
    expect(JSON.stringify(checkBody)).not.toContain('sk-test-value')
    expect(store.compatibilityChecks).toHaveLength(1)
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_compatibility.checked',
    ])

    const read = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/compatibility`,
      ),
    )

    expect(read.status).toBe(200)
    await expect(read.json()).resolves.toEqual(
      expect.objectContaining({
        compatibility: expect.objectContaining({
          id: checkBody.compatibility.id,
          status: 'ready',
        }),
      }),
    )
  })

  test('returns missing Site for compatibility checks on unknown projects', async () => {
    const response = await runRoute(
      adminSession,
      new OperatorSitesDbStore(),
      new Request(
        'https://openagents.com/api/operator/sites/site_project_missing/compatibility/check',
        {
          body: JSON.stringify({ files: [] }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(404)
    await expect(response.json()).resolves.toEqual({
      error: 'site_not_found',
      siteId: 'site_project_missing',
    })
  })

  test('requires an admin session for compatibility reads', async () => {
    const response = await runRoute(
      null,
      new OperatorSitesDbStore(),
      new Request(
        'https://openagents.com/api/operator/sites/site_project_1/compatibility',
      ),
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('validates and reads latest Site build receipts', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/build-validations`,
        {
          body: JSON.stringify({
            compatibility: {
              buildCommand: 'pnpm build',
              compatibilityCheckId: 'site_compatibility_check_ready',
              outputKind: 'static',
              outputPath: 'dist',
              packageManager: 'pnpm',
              status: 'ready',
            },
            files: [
              {
                path: 'package.json',
                text: JSON.stringify({
                  dependencies: { vite: '^6.0.0' },
                  packageManager: 'pnpm@9.0.0',
                  scripts: { build: 'vite build' },
                }),
              },
              { path: 'vite.config.ts', text: 'export default {}' },
              { path: 'src/main.ts', text: 'console.log(import.meta.env.PUBLIC_URL)' },
            ],
            sourceCommitSha: 'abc1234',
            sourceKind: 'github_import',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(201)
    const body = (await response.json()) as {
      buildValidation: { id: string; sourceHash: string }
    }

    expect(body).toEqual(
      expect.objectContaining({
        buildValidation: expect.objectContaining({
          buildCommand: 'pnpm build',
          compatibilityCheckId: 'site_compatibility_check_ready',
          manifest: expect.objectContaining({
            entrypoints: ['dist'],
            envKeys: ['PUBLIC_URL'],
          }),
          outputKind: 'static',
          packageManager: 'pnpm',
          siteId,
          sourceCommitSha: 'abc1234',
          sourceHash: expect.stringMatching(/^[a-f0-9]{64}$/),
          status: 'passed',
        }),
      }),
    )
    expect(store.buildValidations).toHaveLength(1)
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_build_validation.checked',
    ])

    const read = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/build-validations/latest`,
      ),
    )

    expect(read.status).toBe(200)
    await expect(read.json()).resolves.toEqual(
      expect.objectContaining({
        buildValidation: expect.objectContaining({
          id: body.buildValidation.id,
          sourceHash: body.buildValidation.sourceHash,
          status: 'passed',
        }),
      }),
    )
  })

  test('rejects unsafe Site build validation payloads', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/build-validations`,
        {
          body: JSON.stringify({
            files: [
              { path: 'index.html', text: '<main></main>' },
              { path: '.env', text: 'OPENAI_API_KEY=sk-test-secret-value\n' },
            ],
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'unsafe_site_build_validation_payload',
      reason: 'Site build validation input contains secret-shaped material.',
    })
  })

  test('updates operator Site access policy', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/access`,
        {
          body: JSON.stringify({
            accessMode: 'owner_admins',
            visibility: 'private',
          }),
          method: 'PATCH',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      site: expect.objectContaining({
        accessMode: 'owner_admins',
        id: siteId,
        visibility: 'private',
      }),
    })
    expect(store.events.map(event => event.type)).toEqual([
      'site_project.created',
      'site_project.access_updated',
    ])
  })

  test('prepares a generation packet through the operator API', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/generate`,
        {
          body: JSON.stringify({
            actorRunId: 'agent_run_otec',
            operatorNotes: 'Prepare OTEC launch.',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(202)
    await expect(response.json()).resolves.toEqual({
      generation: expect.objectContaining({
        publicUrl: 'https://sites.openagents.com/otec',
        siteId,
        slug: 'otec',
        softwareOrderId: 'software_order_ben_otec',
      }),
    })
    expect(store.projects[0]?.status).toBe('generating')
    expect(store.events.at(-1)).toMatchObject({
      actor_run_id: 'agent_run_otec',
      type: 'site_generation.requested',
    })
  })

  test('records reviewed source export receipts without exposing token hashes', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id ?? ''
    const versionId = 'site_version_export'
    store.versions = [
      {
        artifact_manifest_r2_key: 'sites/otec/export/manifest.json',
        build_command: 'bun run build',
        build_log_r2_key: null,
        build_status: 'saved',
        created_at: '2026-06-05T00:00:00.000Z',
        created_by_run_id: null,
        created_by_user_id: 'github:operator',
        d1_binding_name: null,
        id: versionId,
        metadata_json: '{}',
        r2_binding_name: null,
        rejected_at: null,
        saved_at: '2026-06-05T00:00:00.000Z',
        site_id: siteId,
        source_archive_r2_key: 'sites/otec/export/source.tar.zst',
        source_commit_sha: null,
        source_kind: 'autopilot_generated',
        static_assets_manifest_json: '{"assets":{}}',
        worker_module_r2_key: null,
      },
    ]

    const exportBody = JSON.stringify({
      approve: true,
      destination: {
        branch: 'openagents/otec-site',
        owner: 'OpenAgentsInc',
        provider: 'github',
        pullRequestUrl: 'https://github.com/OpenAgentsInc/otec/pull/1',
        repository: 'otec',
      },
      exportKind: 'github_pull_request',
      expiresInSeconds: 3600,
      receipt: { reviewRef: 'operator-review:otec-export' },
      secretScan: {
        scannerRef: 'secret-scan:otec-export',
        status: 'passed',
      },
    })
    const request = new Request(
      `https://openagents.com/api/operator/sites/${siteId}/versions/${versionId}/source-exports`,
      {
        body: exportBody,
        headers: {
          'idempotency-key': 'operator-source-export:otec',
        },
        method: 'POST',
      },
    )

    const first = await runRoute(adminSession, store, request)
    const second = await runRoute(
      adminSession,
      store,
      new Request(request.url, {
        body: exportBody,
        headers: {
          'idempotency-key': 'operator-source-export:otec',
        },
        method: 'POST',
      }),
    )

    expect(first.status).toBe(201)
    await expect(first.json()).resolves.toEqual({
      sourceExport: expect.objectContaining({
        approvedByUserId: 'github:operator',
        destination: expect.objectContaining({
          owner: 'OpenAgentsInc',
          provider: 'github',
          repository: 'otec',
        }),
        exportKind: 'github_pull_request',
        sourceArchiveR2Key: 'sites/otec/export/source.tar.zst',
        status: 'approved',
        tokenExpiresAt: expect.any(String),
        tokenRef: expect.stringMatching(/^site_source_export_token_/),
      }),
    })
    expect(second.status).toBe(201)
    expect(store.sourceExports).toHaveLength(1)
    expect(store.sourceExports[0]?.token_hash).toMatch(
      /^site_source_export_token_hash_/,
    )
  })

  test('rejects source exports without idempotency or passed secret scans', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())
    const siteId = store.projects[0]?.id ?? ''
    const versionId = 'site_version_export'
    store.versions = [
      {
        artifact_manifest_r2_key: 'sites/otec/export/manifest.json',
        build_command: 'bun run build',
        build_log_r2_key: null,
        build_status: 'saved',
        created_at: '2026-06-05T00:00:00.000Z',
        created_by_run_id: null,
        created_by_user_id: 'github:operator',
        d1_binding_name: null,
        id: versionId,
        metadata_json: '{}',
        r2_binding_name: null,
        rejected_at: null,
        saved_at: '2026-06-05T00:00:00.000Z',
        site_id: siteId,
        source_archive_r2_key: 'sites/otec/export/source.tar.zst',
        source_commit_sha: null,
        source_kind: 'autopilot_generated',
        static_assets_manifest_json: '{"assets":{}}',
        worker_module_r2_key: null,
      },
    ]
    const body = {
      destination: { provider: 'download' },
      exportKind: 'download_token',
      secretScan: { status: 'failed' },
    }

    const missingKey = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/versions/${versionId}/source-exports`,
        {
          body: JSON.stringify(body),
          method: 'POST',
        },
      ),
    )
    const failedScan = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/sites/${siteId}/versions/${versionId}/source-exports`,
        {
          body: JSON.stringify(body),
          headers: {
            'idempotency-key': 'operator-source-export:failed-scan',
          },
          method: 'POST',
        },
      ),
    )

    expect(missingKey.status).toBe(400)
    await expect(missingKey.json()).resolves.toEqual({
      error: 'bad_request',
      reason: 'idempotency key is required',
    })
    expect(failedScan.status).toBe(400)
    await expect(failedScan.json()).resolves.toEqual({
      error: 'site_source_export_validation_error',
      reason: 'source export requires a passed secret scan.',
    })
  })

  test('returns order and slug conflicts through operator route errors', async () => {
    const store = new OperatorSitesDbStore()
    await runRoute(adminSession, store, createOtecSiteRequest())

    const missingOrderResponse = await runRoute(
      adminSession,
      store,
      new Request('https://openagents.com/api/operator/sites', {
        body: JSON.stringify({
          softwareOrderId: 'software_order_missing',
          slug: 'missing',
          title: 'Missing',
        }),
        method: 'POST',
      }),
    )
    const slugConflictResponse = await runRoute(
      adminSession,
      store,
      new Request('https://openagents.com/api/operator/sites', {
        body: JSON.stringify({
          softwareOrderId: 'software_order_testing',
          slug: 'otec',
          title: 'Duplicate OTEC',
        }),
        method: 'POST',
      }),
    )

    expect(missingOrderResponse.status).toBe(404)
    await expect(missingOrderResponse.json()).resolves.toEqual({
      error: 'software_order_not_found',
      softwareOrderId: 'software_order_missing',
    })
    expect(slugConflictResponse.status).toBe(409)
    await expect(slugConflictResponse.json()).resolves.toEqual({
      error: 'site_slug_unavailable',
      slug: 'otec',
    })
  })

  test('returns generic bad request errors without echoing request values', async () => {
    const response = await runRoute(
      adminSession,
      new OperatorSitesDbStore(),
      new Request('https://openagents.com/api/operator/sites', {
        body: JSON.stringify({
          accessMode: 'sk-abcdefghijklmnopqrstuvwxyz',
          softwareOrderId: 'software_order_ben_otec',
          slug: 'otec',
          title: 'OTEC Floating Datacenter',
        }),
        method: 'POST',
      }),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'bad_request',
      reason: 'invalid request body',
    })
  })

  test('matches version save routes through typed request decoding', async () => {
    const saveResponse = await runRoute(
      adminSession,
      new OperatorSitesDbStore(),
      new Request(
        'https://openagents.com/api/operator/sites/site_project_1/versions',
        {
          body: JSON.stringify({
            buildStatus: 'saved',
            sourceKind: 'operator_static',
          }),
          method: 'POST',
        },
      ),
    )

    expect(saveResponse.status).toBe(400)
    await expect(saveResponse.json()).resolves.toEqual({
      error: 'bad_request',
      reason: 'invalid request body',
    })
  })
})
