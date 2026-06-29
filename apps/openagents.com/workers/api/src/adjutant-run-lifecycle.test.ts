import { Effect, Redacted } from 'effect'
import { describe, expect, test } from 'vitest'

import { applyAdjutantRunLifecycleEvents } from './adjutant-run-lifecycle'
import { EmailAddress, ResendEmailSender, WorkerSecret } from './config'
import type { OmniEventRecord } from './omni-runs'

type StoredAssignment = Readonly<{
  assigned_by_user_id: string | null
  archived_at: string | null
  blocked_at: string | null
  commit_sha: string | null
  completed_at: string | null
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  site_id: string | null
  software_order_id: string | null
  status: string
  task_spec_path: string | null
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredOrder = Readonly<{
  agent_started_at: string | null
  archived_at: string | null
  current_run_id: string | null
  id: string
  status: string
  updated_at: string
  user_id: string
}>

type StoredUser = Readonly<{
  deleted_at: string | null
  display_name: string
  id: string
  primary_email: string | null
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

type StoredAdjutantEvent = Readonly<{
  actor_user_id: string | null
  assignment_id: string
  created_at: string
  email_message_id: string | null
  event_type: string
  id: string
  payload_json: string | null
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredAdjutantAdjustment = Readonly<{
  archived_at: string | null
  assignment_id: string
  completed_at: string | null
  continuation_run_id: string | null
  created_at: string
  id: string
  resulting_version_id: string | null
  source_run_id: string | null
  status: string
  updated_at: string
}>

type StoredSiteEvent = Readonly<{
  actor_run_id: string | null
  actor_user_id: string | null
  created_at: string
  deployment_id: string | null
  email_message_id: string | null
  id: string
  payload_json: string | null
  site_id: string
  summary: string
  type: string
  version_id: string | null
}>

type StoredSiteDeployment = Readonly<{
  activated_at: string | null
  id: string
  rolled_back_at: string | null
  runtime_kind: string
  site_id: string
  slug: string
  status: string
  updated_at: string
  url: string
  version_id: string
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

type StoredUsageReceipt = Readonly<{
  adjustment_id: string | null
  assignment_id: string
  billing_ledger_entry_id: string | null
  billing_mode: 'public_beta_free' | 'paid_credits'
  category: 'generation' | 'build' | 'hosting' | 'storage' | 'adjustment'
  created_at: string
  credits_charged_cents: number
  currency: string
  id: string
  idempotency_key: string
  public_receipt_json: string
  quantity: number
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  team_receipt_json: string
  unit: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredEmailMessage = Readonly<{
  id: string
  idempotency_key: string
  provider_message_id: string | null
  status: string
}>

type StoredEmailDelivery = Readonly<{
  message_id: string
  provider_message_id: string | null
  status: string
}>

type StoredSiteFeedback = Readonly<{
  adjutant_adjustment_id: string | null
  archived_at: string | null
  body: string
  created_at: string
  id: string
  status: string
  updated_at: string
}>

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: false,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 1,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeResult = <T>(results: Array<T> = []): D1Result<T> => ({
  meta: d1Meta(),
  results,
  success: true,
})

const lifecycleResendConfig = () => ({
  apiKey: Redacted.make(WorkerSecret.make('re_adjutant_test')),
  fromEmail: ResendEmailSender.make('OpenAgents <sites@openagents.com>'),
  replyToEmail: EmailAddress.make('support@openagents.com'),
})

const event = (
  input: Partial<OmniEventRecord> & Pick<OmniEventRecord, 'sequence' | 'type'>,
): OmniEventRecord => ({
  artifactRefs: input.artifactRefs ?? [],
  createdAt: input.createdAt ?? '2026-06-05T00:00:00.000Z',
  externalEventId: input.externalEventId ?? null,
  id: input.id ?? `runner_event_${input.sequence}`,
  parentId: input.parentId ?? 'agent_run_adjutant',
  payloadJson: input.payloadJson ?? null,
  sequence: input.sequence,
  source: input.source ?? 'runner',
  status: input.status ?? null,
  summary: input.summary ?? 'Runner event.',
  type: input.type,
})

class LifecycleStore {
  adjustments: Array<StoredAdjutantAdjustment> = []
  assignments: Array<StoredAssignment> = [
    {
      archived_at: null,
      assigned_by_user_id: 'github:operator',
      blocked_at: null,
      commit_sha: '2e8a3875',
      completed_at: null,
      current_run_id: 'agent_run_adjutant',
      goal_id: 'agent_goal_adjutant',
      id: 'adjutant_assignment_1',
      objective:
        'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
      status: 'queued',
      task_spec_path: 'docs/autopilot-tasks/adjutant-otec.md',
      updated_at: '2026-06-05T00:00:00.000Z',
      visibility: 'public',
    },
  ]
  assignmentEvents: Array<StoredAdjutantEvent> = []
  orders: Array<StoredOrder> = [
    {
      agent_started_at: null,
      archived_at: null,
      current_run_id: 'agent_run_adjutant',
      id: 'software_order_otec',
      status: 'agent_queued',
      updated_at: '2026-06-05T00:00:00.000Z',
      user_id: 'github:customer',
    },
  ]
  users: Array<StoredUser> = [
    {
      deleted_at: null,
      display_name: 'Alex Customer',
      id: 'github:customer',
      primary_email: 'alex.customer@example.com',
    },
  ]
  emailDeliveries: Array<StoredEmailDelivery> = []
  emailMessagesByKey = new Map<string, StoredEmailMessage>()
  feedback: Array<StoredSiteFeedback> = []
  deployments: Array<StoredSiteDeployment> = []
  siteProjects: Array<StoredSiteProject> = [
    {
      access_mode: 'public',
      active_deployment_id: null,
      active_version_id: null,
      archived_at: null,
      created_at: '2026-06-05T00:00:00.000Z',
      id: 'site_project_otec',
      owner_user_id: 'github:operator',
      project_id: 'project_adjutant',
      prompt: 'Build the OTEC floating datacenter Site.',
      slug: 'otec',
      software_order_id: 'software_order_otec',
      source_repository_name: 'autopilot-omega',
      source_repository_owner: 'OpenAgentsInc',
      source_repository_provider: 'github',
      source_repository_ref: 'main',
      status: 'draft',
      team_id: 'team_openagents_core',
      title: 'OTEC Floating Datacenter',
      updated_at: '2026-06-05T00:00:00.000Z',
      visibility: 'public',
    },
  ]
  siteEvents: Array<StoredSiteEvent> = []
  storageBindings: Array<StoredSiteStorageBinding> = []
  usageReceipts: Array<StoredUsageReceipt> = []
  versions: Array<StoredSiteVersion> = []
}

class LifecycleArtifactsBucket {
  objects = new Map<string, string>()

  put(key: string, value: string): Promise<R2Object> {
    this.objects.set(key, value)

    return Promise.resolve({ key } as R2Object)
  }
}

class LifecycleStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: LifecycleStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [idempotencyKey] = this.values
      const row = this.store.usageReceipts.find(
        receipt => receipt.idempotency_key === idempotencyKey,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      const [runId] = this.values
      const row = this.store.assignments.find(
        assignment =>
          assignment.current_run_id === runId &&
          assignment.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM software_orders') &&
      this.query.includes('INNER JOIN users')
    ) {
      const [_siteId, orderId] = this.values
      const order = this.store.orders.find(
        item => item.id === orderId && item.archived_at === null,
      )
      const user = this.store.users.find(
        item => item.id === order?.user_id && item.deleted_at === null,
      )
      const site = this.store.siteProjects.find(
        item => item.id === _siteId && item.archived_at === null,
      )

      if (order === undefined || user === undefined) {
        return Promise.resolve(null)
      }

      return Promise.resolve({
        display_name: user.display_name,
        order_id: order.id,
        primary_email: user.primary_email,
        site_title: site?.title ?? null,
        site_url: null,
        target_user_id: user.id,
      } as T)
    }

    if (this.query.includes('FROM adjutant_assignment_events')) {
      const [assignmentId, runId, eventType] = this.values
      const row = this.store.assignmentEvents.find(
        event =>
          event.assignment_id === assignmentId &&
          event.run_id === runId &&
          event.event_type === eventType,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM email_messages')) {
      const [idempotencyKey] = this.values
      const row = this.store.emailMessagesByKey.get(String(idempotencyKey))

      if (row === undefined) {
        return Promise.resolve(null)
      }

      return Promise.resolve({
        created_at: '2026-06-05T00:00:00.000Z',
        error_message: null,
        error_name: null,
        id: row.id,
        idempotency_key: row.idempotency_key,
        kind: 'operator_notification',
        provider: row.provider_message_id === null ? null : 'resend',
        provider_message_id: row.provider_message_id,
        status: row.status,
        updated_at: '2026-06-05T00:00:00.000Z',
      } as T)
    }

    if (this.query.includes('FROM site_versions')) {
      const [siteId, runId] = this.values
      const row = this.store.versions.find(
        version =>
          version.site_id === siteId && version.created_by_run_id === runId,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_deployments')) {
      const [siteId, versionId] = this.values
      const row = this.store.deployments.find(
        deployment =>
          deployment.site_id === siteId && deployment.version_id === versionId,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_projects')) {
      const [siteId] = this.values
      const row = this.store.siteProjects.find(
        project => project.id === siteId && project.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_events')) {
      const [siteId, runId, eventType] = this.values
      const row = this.store.siteEvents.find(
        event =>
          event.site_id === siteId &&
          event.actor_run_id === runId &&
          event.type === eventType,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    return Promise.resolve(null)
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('UPDATE adjutant_adjustment_requests')) {
      const [
        status,
        resultingVersionId,
        updatedAt,
        failedStatus,
        completedAt,
        assignmentId,
        continuationRunId,
        sourceRunId,
      ] = this.values
      const candidate = [...this.store.adjustments]
        .filter(
          adjustment =>
            adjustment.assignment_id === assignmentId &&
            adjustment.archived_at === null &&
            ['requested', 'queued', 'running'].includes(adjustment.status) &&
            (adjustment.continuation_run_id === continuationRunId ||
              adjustment.source_run_id === sourceRunId),
        )
        .sort((left, right) => right.created_at.localeCompare(left.created_at))
        .at(0)

      this.store.adjustments = this.store.adjustments.map(adjustment =>
        candidate !== undefined && adjustment.id === candidate.id
          ? {
              ...adjustment,
              completed_at:
                failedStatus === 'failed'
                  ? (adjustment.completed_at ??
                    (typeof completedAt === 'string' ? completedAt : null))
                  : adjustment.completed_at,
              resulting_version_id:
                typeof resultingVersionId === 'string'
                  ? resultingVersionId
                  : null,
              status: String(status),
              updated_at: String(updatedAt),
            }
          : adjustment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE adjutant_assignments')) {
      const [
        status,
        updatedAt,
        blockedStatus,
        blockedAt,
        completedStatus,
        completedAt,
        assignmentId,
      ] = this.values

      this.store.assignments = this.store.assignments.map(assignment =>
        assignment.id === assignmentId
          ? {
              ...assignment,
              blocked_at:
                blockedStatus === 'blocked'
                  ? (assignment.blocked_at ??
                    (typeof blockedAt === 'string' ? blockedAt : null))
                  : assignment.blocked_at,
              completed_at:
                completedStatus === 'delivered'
                  ? (assignment.completed_at ??
                    (typeof completedAt === 'string' ? completedAt : null))
                  : assignment.completed_at,
              status: String(status),
              updated_at: String(updatedAt),
            }
          : assignment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE site_versions')) {
      const [versionId, siteId] = this.values

      this.store.versions = this.store.versions.map(version =>
        version.id === versionId && version.site_id === siteId
          ? {
              ...version,
              metadata_json: JSON.stringify({
                ...JSON.parse(version.metadata_json),
                customerAccepted: false,
                customerReviewState: 'customer_review_ready',
                runtimeActivationPolicy: 'latest_successful_revision',
              }),
            }
          : version,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("status = 'rolled_back'")
    ) {
      const [rolledBackAt, updatedAt, deploymentId, siteId] = this.values

      this.store.deployments = this.store.deployments.map(deployment =>
        deployment.id === deploymentId && deployment.site_id === siteId
          ? {
              ...deployment,
              rolled_back_at: String(rolledBackAt),
              status: 'rolled_back',
              updated_at: String(updatedAt),
            }
          : deployment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE site_deployments') &&
      this.query.includes("status = 'active'")
    ) {
      const [activatedAt, updatedAt, deploymentId, siteId] = this.values

      this.store.deployments = this.store.deployments.map(deployment =>
        deployment.id === deploymentId && deployment.site_id === siteId
          ? {
              ...deployment,
              activated_at:
                deployment.activated_at ?? String(activatedAt),
              status: 'active',
              updated_at: String(updatedAt),
            }
          : deployment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO site_deployments')) {
      const [
        id,
        siteId,
        versionId,
        slug,
        url,
        runtimeKind,
        _runtimeScriptName,
        _dispatchNamespace,
        _deployedByUserId,
        _startedAt,
        activatedAt,
        _createdAt,
        updatedAt,
      ] = this.values

      this.store.deployments.push({
        activated_at: String(activatedAt),
        id: String(id),
        rolled_back_at: null,
        runtime_kind: String(runtimeKind),
        site_id: String(siteId),
        slug: String(slug),
        status: 'active',
        updated_at: String(updatedAt),
        url: String(url),
        version_id: String(versionId),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE site_projects') &&
      this.query.includes('active_version_id')
    ) {
      const [versionId, deploymentId, updatedAt, siteId] = this.values

      this.store.siteProjects = this.store.siteProjects.map(project =>
        project.id === siteId
          ? {
              ...project,
              active_deployment_id: String(deploymentId),
              active_version_id: String(versionId),
              status: 'needs_review',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE site_projects')) {
      const [updatedAt, siteId] = this.values

      this.store.siteProjects = this.store.siteProjects.map(project =>
        project.id === siteId
          ? {
              ...project,
              status: 'needs_review',
              updated_at: String(updatedAt),
            }
          : project,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE site_revision_feedback')) {
      const [updatedAt, _assignmentId, versionId, runId] = this.values
      const adjustment = this.store.adjustments.find(
        item =>
          item.resulting_version_id === versionId &&
          (item.continuation_run_id === runId || item.source_run_id === runId),
      )

      this.store.feedback = this.store.feedback.map(feedback =>
        adjustment !== undefined &&
        feedback.adjutant_adjustment_id === adjustment.id &&
        (feedback.status === 'submitted' ||
          feedback.status === 'queued' ||
          feedback.status === 'running')
          ? {
              ...feedback,
              status: 'addressed',
              updated_at: String(updatedAt),
            }
          : feedback,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE software_orders')) {
      const [runId, status, runningStatus, agentStartedAt, updatedAt, orderId] =
        this.values

      this.store.orders = this.store.orders.map(order =>
        order.id === orderId
          ? {
              ...order,
              agent_started_at:
                runningStatus === 'agent_running'
                  ? (order.agent_started_at ??
                    (typeof agentStartedAt === 'string'
                      ? agentStartedAt
                      : null))
                  : order.agent_started_at,
              current_run_id: typeof runId === 'string' ? runId : null,
              status: String(status),
              updated_at: String(updatedAt),
            }
          : order,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_events')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        _goalId,
        runId,
        eventType,
        visibility,
        summary,
        actorUserId,
        payloadJson,
        emailMessageId,
        createdAt,
      ] = this.values

      this.store.assignmentEvents.push({
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        email_message_id:
          typeof emailMessageId === 'string' ? emailMessageId : null,
        event_type: String(eventType),
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        run_id: typeof runId === 'string' ? runId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        summary: String(summary),
        visibility: visibility as StoredAdjutantEvent['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO email_messages')) {
      const [id, kind, _actorUserId, _targetUserId, toEmail, fromEmail] =
        this.values
      const idempotencyKey = this.values[12]

      void kind
      void toEmail
      void fromEmail

      if (
        typeof idempotencyKey === 'string' &&
        !this.store.emailMessagesByKey.has(idempotencyKey)
      ) {
        this.store.emailMessagesByKey.set(idempotencyKey, {
          id: String(id),
          idempotency_key: idempotencyKey,
          provider_message_id: null,
          status: 'rendered',
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'rendered'")
    ) {
      const idempotencyKey = this.values.at(-1)
      const current =
        typeof idempotencyKey === 'string'
          ? this.store.emailMessagesByKey.get(idempotencyKey)
          : undefined

      if (current !== undefined && current.status !== 'accepted') {
        this.store.emailMessagesByKey.set(current.idempotency_key, {
          ...current,
          provider_message_id: null,
          status: 'rendered',
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'accepted'")
    ) {
      const [provider, providerMessageId, _updatedAt, messageId] = this.values
      const current = [...this.store.emailMessagesByKey.values()].find(
        message => message.id === messageId,
      )

      void provider

      if (current !== undefined) {
        this.store.emailMessagesByKey.set(current.idempotency_key, {
          ...current,
          provider_message_id:
            typeof providerMessageId === 'string' ? providerMessageId : null,
          status: 'accepted',
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('UPDATE email_messages') &&
      this.query.includes("status = 'failed'")
    ) {
      const messageId = this.values.at(-1)
      const current = [...this.store.emailMessagesByKey.values()].find(
        message => message.id === messageId,
      )

      if (current !== undefined) {
        this.store.emailMessagesByKey.set(current.idempotency_key, {
          ...current,
          provider_message_id: null,
          status: 'failed',
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO email_deliveries')) {
      const [
        _id,
        messageId,
        _provider,
        providerMessageId,
        _idempotencyKey,
        status,
      ] = this.values

      this.store.emailDeliveries.push({
        message_id: String(messageId),
        provider_message_id:
          typeof providerMessageId === 'string' ? providerMessageId : null,
        status: String(status),
      })

      return Promise.resolve(makeResult<T>())
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

      this.store.versions.push({
        artifact_manifest_r2_key:
          artifactManifestR2Key === null ? null : String(artifactManifestR2Key),
        build_command: buildCommand === null ? null : String(buildCommand),
        build_log_r2_key: buildLogR2Key === null ? null : String(buildLogR2Key),
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
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO site_storage_bindings')) {
      const [id, siteId, kind, bindingName, createdAt, updatedAt] = this.values

      this.store.storageBindings.push({
        binding_name: String(bindingName),
        cloudflare_resource_ref: null,
        created_at: String(createdAt),
        id: String(id),
        kind: String(kind),
        scope: 'shared_prefix',
        site_id: String(siteId),
        updated_at: String(updatedAt),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT OR IGNORE INTO adjutant_usage_receipts')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        adjustmentId,
        runId,
        category,
        visibility,
        billingMode,
        summary,
        quantity,
        unit,
        creditsChargedCents,
        currency,
        billingLedgerEntryId,
        publicReceiptJson,
        teamReceiptJson,
        idempotencyKey,
        createdAt,
      ] = this.values

      if (
        typeof idempotencyKey === 'string' &&
        !this.store.usageReceipts.some(
          receipt => receipt.idempotency_key === idempotencyKey,
        )
      ) {
        this.store.usageReceipts.push({
          adjustment_id: typeof adjustmentId === 'string' ? adjustmentId : null,
          assignment_id: String(assignmentId),
          billing_ledger_entry_id:
            typeof billingLedgerEntryId === 'string'
              ? billingLedgerEntryId
              : null,
          billing_mode:
            billingMode === 'paid_credits'
              ? 'paid_credits'
              : 'public_beta_free',
          category: category as StoredUsageReceipt['category'],
          created_at: String(createdAt),
          credits_charged_cents: Number(creditsChargedCents ?? 0),
          currency: String(currency),
          id: String(id),
          idempotency_key: idempotencyKey,
          public_receipt_json: String(publicReceiptJson),
          quantity: Number(quantity ?? 0),
          run_id: typeof runId === 'string' ? runId : null,
          site_id: typeof siteId === 'string' ? siteId : null,
          software_order_id:
            typeof softwareOrderId === 'string' ? softwareOrderId : null,
          summary: String(summary),
          team_receipt_json: String(teamReceiptJson),
          unit: String(unit),
          visibility: visibility as StoredUsageReceipt['visibility'],
        })
      }

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO site_events')) {
      const [
        id,
        siteId,
        third,
        fourth,
        fifth,
        sixth,
        seventh,
        eighth,
        ninth,
        tenth,
      ] = this.values
      const eventType = this.values.length === 10 ? fifth : third
      const summary = this.values.length === 10 ? sixth : fourth
      const actorUserId = this.values.length === 10 ? seventh : fifth
      const actorRunId = this.values.length === 10 ? eighth : sixth
      const payloadJson = this.values.length === 10 ? ninth : seventh
      const emailMessageId =
        this.values.length === 9
          ? eighth
          : this.values.length === 10
            ? null
            : null
      const createdAt =
        this.values.length === 9
          ? ninth
          : this.values.length === 10
            ? tenth
            : eighth

      this.store.siteEvents.push({
        actor_run_id: typeof actorRunId === 'string' ? actorRunId : null,
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        created_at: String(createdAt),
        deployment_id:
          this.values.length === 10 && typeof fourth === 'string'
            ? fourth
            : null,
        email_message_id:
          typeof emailMessageId === 'string' ? emailMessageId : null,
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        site_id: String(siteId),
        summary: String(summary),
        type: String(eventType),
        version_id:
          this.values.length === 10 && typeof third === 'string' ? third : null,
      })

      return Promise.resolve(makeResult<T>())
    }

    return Promise.resolve(makeResult<T>())
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_revision_feedback')) {
      const [assignmentId] = this.values
      const adjustmentIds = new Set(
        this.store.adjustments
          .filter(
            adjustment =>
              adjustment.assignment_id === assignmentId &&
              adjustment.archived_at === null,
          )
          .map(adjustment => adjustment.id),
      )
      const results = this.store.feedback
        .filter(
          feedback =>
            feedback.adjutant_adjustment_id !== null &&
            adjustmentIds.has(feedback.adjutant_adjustment_id) &&
            feedback.archived_at === null &&
            ['submitted', 'queued', 'running'].includes(feedback.status),
        )
        .sort((left, right) => left.created_at.localeCompare(right.created_at))
        .map(feedback => ({ body: feedback.body }))

      return Promise.resolve(makeResult(results as Array<T>))
    }

    return Promise.resolve(makeResult<T>())
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const lifecycleDb = (store: LifecycleStore): D1Database => ({
  batch: async <T = unknown>(): Promise<Array<D1Result<T>>> => [],
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new LifecycleStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const applyLifecycle = (
  store: LifecycleStore,
  input: Parameters<typeof applyAdjutantRunLifecycleEvents>[1],
): Promise<void> =>
  Effect.runPromise(applyAdjutantRunLifecycleEvents(lifecycleDb(store), input))

describe('Adjutant run lifecycle mapping', () => {
  test('maps running callbacks into assignment, order, and Site lifecycle', async () => {
    const store = new LifecycleStore()

    await applyLifecycle(store, {
      actorUserId: 'openagents:runner-ingest',
      events: [
        event({ sequence: 2, status: 'running', type: 'runner.started' }),
      ],
      runId: 'agent_run_adjutant',
      status: 'running',
    })

    expect(store.assignments[0]).toMatchObject({
      status: 'running',
    })
    expect(store.orders[0]).toMatchObject({
      current_run_id: 'agent_run_adjutant',
      status: 'agent_running',
    })
    expect(store.assignmentEvents).toEqual([
      expect.objectContaining({
        event_type: 'adjutant.run_running',
        run_id: 'agent_run_adjutant',
      }),
      expect.objectContaining({
        email_message_id: null,
        event_type: 'adjutant.notification.autopilot_running',
        run_id: 'agent_run_adjutant',
      }),
    ])
    expect(store.siteEvents).toEqual([
      expect.objectContaining({
        actor_run_id: 'agent_run_adjutant',
        type: 'adjutant.run_running',
      }),
      expect.objectContaining({
        actor_run_id: 'agent_run_adjutant',
        email_message_id: null,
        type: 'adjutant.notification.autopilot_running',
      }),
    ])
    expect(store.siteEvents[0]?.payload_json).not.toContain('Runner event.')
    expect(JSON.parse(store.assignmentEvents[1]?.payload_json ?? '{}')).toEqual(
      expect.objectContaining({
        emailStatus: 'skipped',
        lifecycleKind: 'autopilot_running',
        skipReason: 'email_config_missing',
        stage: 'autopilot_running',
      }),
    )
  })

  test('keeps lifecycle records idempotent across callback retry', async () => {
    const store = new LifecycleStore()
    const input = {
      actorUserId: 'openagents:runner-ingest',
      events: [
        event({ sequence: 3, status: 'completed', type: 'runner.completed' }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    } as const

    await applyLifecycle(store, input)
    await applyLifecycle(store, input)

    expect(store.assignments[0]).toMatchObject({
      completed_at: expect.any(String),
      status: 'delivered',
    })
    expect(store.orders[0]).toMatchObject({
      status: 'delivered',
    })
    expect(store.assignmentEvents).toHaveLength(2)
    expect(store.siteEvents).toHaveLength(2)
    expect(store.assignmentEvents[0]).toMatchObject({
      event_type: 'adjutant.run_delivered',
    })
    expect(store.assignmentEvents[1]).toMatchObject({
      email_message_id: null,
      event_type: 'adjutant.notification.review_ready',
    })
    expect(JSON.parse(store.assignmentEvents[1]?.payload_json ?? '{}')).toEqual(
      expect.objectContaining({
        emailStatus: 'skipped',
        skipReason: 'email_config_missing',
        stage: 'review_ready',
      }),
    )
  })

  test('sends review-ready customer notifications through the email ledger once', async () => {
    const store = new LifecycleStore()
    let fetchCount = 0
    const fetcher: typeof fetch = async (input, init) => {
      fetchCount += 1
      const request =
        input instanceof Request ? input : new Request(input, init)

      expect(request.headers.get('Idempotency-Key')).toBe(
        'order_sites_email:review_ready:software_order_otec:adjutant_assignment_1:site_project_otec:agent_run_adjutant',
      )
      await expect(request.json()).resolves.toEqual(
        expect.objectContaining({
          subject: 'OTEC Floating Datacenter is ready for review',
          to: ['alex.customer@example.com'],
        }),
      )

      return new Response(JSON.stringify({ id: 'email_adjutant_test' }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })
    }
    const input = {
      actorUserId: 'openagents:runner-ingest',
      appOrigin: 'https://openagents.com',
      emailConfig: lifecycleResendConfig(),
      emailFetcher: fetcher,
      events: [
        event({ sequence: 3, status: 'completed', type: 'runner.completed' }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    } as const

    await applyLifecycle(store, input)
    await applyLifecycle(store, input)

    const message = Array.from(store.emailMessagesByKey.values())[0]
    const notification = store.assignmentEvents.find(
      item => item.event_type === 'adjutant.notification.review_ready',
    )

    expect(fetchCount).toBe(1)
    expect(message).toMatchObject({
      provider_message_id: 'email_adjutant_test',
      status: 'accepted',
    })
    expect(store.emailDeliveries).toEqual([
      expect.objectContaining({
        message_id: message?.id,
        provider_message_id: 'email_adjutant_test',
        status: 'accepted',
      }),
    ])
    expect(notification).toMatchObject({
      email_message_id: message?.id,
    })
    expect(JSON.parse(notification?.payload_json ?? '{}')).toEqual(
      expect.objectContaining({
        emailMessageId: message?.id,
        emailStatus: 'accepted',
        providerMessageId: 'email_adjutant_test',
        stage: 'review_ready',
      }),
    )
    expect(notification?.payload_json).not.toContain('re_adjutant_test')
  })

  test('records missing customer email as a skipped lifecycle notification without rolling back state', async () => {
    const store = new LifecycleStore()
    store.users = store.users.map(user => ({
      ...user,
      primary_email: null,
    }))

    await applyLifecycle(store, {
      actorUserId: 'openagents:runner-ingest',
      emailConfig: lifecycleResendConfig(),
      events: [
        event({
          sequence: 4,
          status: 'waiting_for_input',
          type: 'runner.input_required',
        }),
      ],
      runId: 'agent_run_adjutant',
      status: 'waiting_for_input',
    })

    const notification = store.assignmentEvents.find(
      item => item.event_type === 'adjutant.notification.input_needed',
    )

    expect(store.assignments[0]).toMatchObject({
      status: 'review_needed',
    })
    expect(store.orders[0]).toMatchObject({
      status: 'needs_customer_input',
    })
    expect(store.emailMessagesByKey.size).toBe(0)
    expect(notification).toMatchObject({
      email_message_id: null,
    })
    expect(JSON.parse(notification?.payload_json ?? '{}')).toEqual(
      expect.objectContaining({
        emailStatus: 'skipped',
        lifecycleKind: 'customer_input_needed',
        skipReason: 'missing_customer_email',
        stage: 'input_needed',
      }),
    )
  })

  test('records redacted provider failures without rolling back lifecycle state', async () => {
    const store = new LifecycleStore()
    const fetcher: typeof fetch = async () =>
      new Response(
        JSON.stringify({
          message:
            'Provider rejected request with Bearer provider-secret-token-123456.',
          name: 'validation_error',
        }),
        {
          headers: { 'content-type': 'application/json' },
          status: 422,
        },
      )

    await applyLifecycle(store, {
      actorUserId: 'openagents:runner-ingest',
      appOrigin: 'https://openagents.com',
      emailConfig: lifecycleResendConfig(),
      emailFetcher: fetcher,
      events: [
        event({ sequence: 3, status: 'completed', type: 'runner.completed' }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    })

    const message = Array.from(store.emailMessagesByKey.values())[0]
    const notification = store.assignmentEvents.find(
      item => item.event_type === 'adjutant.notification.review_ready',
    )
    const payload = JSON.parse(notification?.payload_json ?? '{}')

    expect(store.assignments[0]).toMatchObject({
      completed_at: expect.any(String),
      status: 'delivered',
    })
    expect(store.orders[0]).toMatchObject({
      status: 'delivered',
    })
    expect(message).toMatchObject({
      status: 'failed',
    })
    expect(payload).toEqual(
      expect.objectContaining({
        emailMessageId: message?.id,
        emailStatus: 'failed',
        errorMessage: 'Provider rejected request with Bearer [REDACTED]',
        lifecycleKind: 'review_ready',
        stage: 'review_ready',
      }),
    )
    expect(notification?.payload_json).not.toContain('provider-secret-token')
  })

  test('saves a completed Site artifact receipt as one generated Site version', async () => {
    const store = new LifecycleStore()
    const artifacts = new LifecycleArtifactsBucket()
    store.adjustments = [
      {
        archived_at: null,
        assignment_id: 'adjutant_assignment_1',
        completed_at: null,
        continuation_run_id: 'agent_run_adjutant',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'adjutant_adjustment_1',
        resulting_version_id: null,
        source_run_id: 'agent_run_adjutant',
        status: 'running',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]
    store.feedback = [
      {
        adjutant_adjustment_id: 'adjutant_adjustment_1',
        archived_at: null,
        body: 'Tighten the evidence copy.',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'site_feedback_1',
        status: 'queued',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]
    const receipt = {
      schemaVersion: 'openagents.adjutant.site_artifact_receipt.v1',
      buildCommand: 'bun run build',
      buildLogText:
        'Build succeeded with GitHub token gho_abcdefghijklmnopqrstuvwxyz.',
      buildStatus: 'saved',
      d1BindingName: 'SITE_DB',
      metadata: {
        renderer: 'adjutant',
      },
      r2BindingName: 'SITE_ASSETS',
      siteId: 'site_project_otec',
      sourceArchiveText: '<html><body>OTEC</body></html>',
      sourceCommitSha: '9298552f',
      staticAssetsManifest: {
        assets: {
          '/index.html': {
            contentType: 'text/html',
            r2Key: 'sites/site_project_otec/assets/index.html',
          },
        },
      },
      workerModuleText:
        'export default { fetch() { return new Response("ok") } }',
    } as const
    const input = {
      actorUserId: 'openagents:runner-ingest',
      artifacts: artifacts as unknown as R2Bucket,
      events: [
        event({
          payloadJson: JSON.stringify({
            adjutantSiteArtifactReceipt: receipt,
          }),
          sequence: 6,
          status: 'completed',
          type: 'runner.completed',
        }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    } as const

    await applyLifecycle(store, input)
    await applyLifecycle(store, input)

    expect(store.versions).toHaveLength(1)
    expect(store.versions[0]).toMatchObject({
      build_command: 'bun run build',
      build_status: 'saved',
      created_by_run_id: 'agent_run_adjutant',
      d1_binding_name: 'SITE_DB',
      r2_binding_name: 'SITE_ASSETS',
      source_commit_sha: '9298552f',
      source_kind: 'autopilot_generated',
    })
    expect(store.versions[0]?.worker_module_r2_key).toEqual(
      expect.stringContaining('/worker.mjs'),
    )
    expect(store.adjustments).toEqual([
      expect.objectContaining({
        id: 'adjutant_adjustment_1',
        resulting_version_id: store.versions[0]?.id,
        status: 'review_needed',
      }),
    ])
    expect(store.siteProjects[0]).toMatchObject({
      active_deployment_id: expect.any(String),
      active_version_id: store.versions[0]?.id,
      status: 'needs_review',
    })
    expect(store.deployments).toEqual([
      expect.objectContaining({
        activated_at: expect.any(String),
        runtime_kind: 'workers_for_platforms',
        slug: 'otec',
        status: 'active',
        url: 'https://sites.openagents.com/otec',
        version_id: store.versions[0]?.id,
      }),
    ])
    expect(JSON.parse(store.versions[0]?.metadata_json ?? '{}')).toEqual(
      expect.objectContaining({
        customerAccepted: false,
        customerReviewState: 'customer_review_ready',
        runtimeActivationPolicy: 'latest_successful_revision',
      }),
    )
    expect(store.feedback).toEqual([
      expect.objectContaining({
        id: 'site_feedback_1',
        status: 'addressed',
      }),
    ])
    expect(
      artifacts.objects.get(store.versions[0]?.worker_module_r2_key ?? ''),
    ).toBe('export default { fetch() { return new Response("ok") } }')
    expect(
      artifacts.objects.get(store.versions[0]?.build_log_r2_key ?? ''),
    ).not.toContain('gho_abcdefghijklmnopqrstuvwxyz')
    expect(store.siteEvents.map(siteEvent => siteEvent.type)).toEqual([
      'adjutant.run_delivered',
      'adjutant.notification.review_ready',
      'site_version.saved',
      'site_deployment.activated',
    ])
    expect(store.storageBindings).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ binding_name: 'SITE_DB', kind: 'd1' }),
        expect.objectContaining({ binding_name: 'SITE_ASSETS', kind: 'r2' }),
      ]),
    )
    expect(store.usageReceipts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          billing_mode: 'public_beta_free',
          category: 'build',
          credits_charged_cents: 0,
          quantity: 1,
          unit: 'build',
        }),
        expect.objectContaining({
          billing_mode: 'public_beta_free',
          category: 'storage',
          credits_charged_cents: 0,
          quantity: 5,
          unit: 'artifact',
        }),
      ]),
    )
    expect(store.usageReceipts).toHaveLength(2)
  })

  test('blocks customer review activation when requested images are missing', async () => {
    const store = new LifecycleStore()
    const artifacts = new LifecycleArtifactsBucket()
    store.assignments = [
      {
        ...store.assignments[0],
        objective: 'Revise the OTEC Site and add images of ocean infrastructure.',
      } as StoredAssignment,
    ]
    store.adjustments = [
      {
        archived_at: null,
        assignment_id: 'adjutant_assignment_1',
        completed_at: null,
        continuation_run_id: 'agent_run_adjutant',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'adjutant_adjustment_1',
        resulting_version_id: null,
        source_run_id: 'agent_run_adjutant',
        status: 'running',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]
    store.feedback = [
      {
        adjutant_adjustment_id: 'adjutant_adjustment_1',
        archived_at: null,
        body: 'Add images that make the OTEC and SWAC concept visually credible.',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'site_feedback_1',
        status: 'queued',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]

    await expect(
      applyLifecycle(store, {
        actorUserId: 'openagents:runner-ingest',
        artifacts: artifacts as unknown as R2Bucket,
        events: [
          event({
            payloadJson: JSON.stringify({
              adjutantSiteArtifactReceipt: {
                schemaVersion: 'openagents.adjutant.site_artifact_receipt.v1',
                buildStatus: 'saved',
                siteId: 'site_project_otec',
                sourceArchiveText:
                  '<html><body><section class="diagram"></section></body></html>',
                staticAssetsManifest: {
                  assets: {
                    '/index.html': {
                      contentType: 'text/html',
                      r2Key: 'sites/site_project_otec/latest/index.html',
                    },
                  },
                },
              },
            }),
            sequence: 7,
            status: 'completed',
            type: 'runner.completed',
          }),
        ],
        runId: 'agent_run_adjutant',
        status: 'completed',
      }),
    ).rejects.toMatchObject({
      operation: 'adjutantRunLifecycle.siteActivation.visualAssets',
    })

    expect(store.versions).toHaveLength(1)
    expect(store.deployments).toEqual([])
    expect(store.siteProjects[0]).toMatchObject({
      active_deployment_id: null,
      active_version_id: null,
      status: 'draft',
    })
    expect(JSON.parse(store.versions[0]?.metadata_json ?? '{}')).not.toEqual(
      expect.objectContaining({
        customerReviewState: 'customer_review_ready',
      }),
    )
    expect(store.feedback[0]).toMatchObject({ status: 'queued' })
  })

  test('allows customer review activation when requested image assets are present', async () => {
    const store = new LifecycleStore()
    const artifacts = new LifecycleArtifactsBucket()
    store.assignments = [
      {
        ...store.assignments[0],
        objective: 'Revise the OTEC Site and add images of ocean infrastructure.',
      } as StoredAssignment,
    ]
    store.adjustments = [
      {
        archived_at: null,
        assignment_id: 'adjutant_assignment_1',
        completed_at: null,
        continuation_run_id: 'agent_run_adjutant',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'adjutant_adjustment_1',
        resulting_version_id: null,
        source_run_id: 'agent_run_adjutant',
        status: 'running',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]

    await applyLifecycle(store, {
      actorUserId: 'openagents:runner-ingest',
      artifacts: artifacts as unknown as R2Bucket,
      events: [
        event({
          payloadJson: JSON.stringify({
            adjutantSiteArtifactReceipt: {
              schemaVersion: 'openagents.adjutant.site_artifact_receipt.v1',
              buildStatus: 'saved',
              siteId: 'site_project_otec',
              sourceArchiveText:
                '<html><body><img src="/assets/ocean-platform.png" alt="Ocean platform"></body></html>',
              staticAssetsManifest: {
                assets: {
                  '/assets/ocean-platform.png': {
                    contentType: 'image/png',
                    r2Key: 'sites/site_project_otec/latest/ocean-platform.png',
                  },
                  '/index.html': {
                    contentType: 'text/html',
                    r2Key: 'sites/site_project_otec/latest/index.html',
                  },
                },
              },
            },
          }),
          sequence: 7,
          status: 'completed',
          type: 'runner.completed',
        }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    })

    expect(store.deployments).toEqual([
      expect.objectContaining({
        status: 'active',
        version_id: store.versions[0]?.id,
      }),
    ])
    expect(JSON.parse(store.versions[0]?.metadata_json ?? '{}')).toEqual(
      expect.objectContaining({
        customerReviewState: 'customer_review_ready',
      }),
    )
  })

  test('activates latest revision and preserves prior deployment history', async () => {
    const store = new LifecycleStore()
    const artifacts = new LifecycleArtifactsBucket()
    store.siteProjects = [
      {
        ...store.siteProjects[0],
        active_deployment_id: 'site_deployment_previous',
        active_version_id: 'site_version_previous',
        status: 'approved',
      } as StoredSiteProject,
    ]
    store.versions = [
      {
        artifact_manifest_r2_key: 'sites/site_project_otec/previous/manifest.json',
        build_command: 'bun run build',
        build_log_r2_key: null,
        build_status: 'saved',
        created_at: '2026-06-05T00:00:00.000Z',
        created_by_run_id: 'agent_run_previous',
        created_by_user_id: 'openagents:runner-ingest',
        d1_binding_name: null,
        id: 'site_version_previous',
        metadata_json: JSON.stringify({
          customerReviewState: 'runtime_verified',
        }),
        r2_binding_name: null,
        rejected_at: null,
        saved_at: '2026-06-05T00:00:00.000Z',
        site_id: 'site_project_otec',
        source_archive_r2_key: null,
        source_commit_sha: 'previous',
        source_kind: 'autopilot_generated',
        static_assets_manifest_json: '{}',
        worker_module_r2_key: null,
      },
    ]
    store.deployments = [
      {
        activated_at: '2026-06-05T00:00:00.000Z',
        id: 'site_deployment_previous',
        rolled_back_at: null,
        runtime_kind: 'omega_static_r2',
        site_id: 'site_project_otec',
        slug: 'otec',
        status: 'active',
        updated_at: '2026-06-05T00:00:00.000Z',
        url: 'https://sites.openagents.com/otec',
        version_id: 'site_version_previous',
      },
    ]
    store.adjustments = [
      {
        archived_at: null,
        assignment_id: 'adjutant_assignment_1',
        completed_at: null,
        continuation_run_id: 'agent_run_adjutant',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'adjutant_adjustment_1',
        resulting_version_id: null,
        source_run_id: 'agent_run_previous',
        status: 'queued',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]
    store.feedback = [
      {
        adjutant_adjustment_id: 'adjutant_adjustment_1',
        archived_at: null,
        body: 'Tighten the evidence copy.',
        created_at: '2026-06-05T00:01:00.000Z',
        id: 'site_feedback_1',
        status: 'running',
        updated_at: '2026-06-05T00:01:00.000Z',
      },
    ]
    const receipt = {
      schemaVersion: 'openagents.adjutant.site_artifact_receipt.v1',
      buildStatus: 'saved',
      siteId: 'site_project_otec',
      sourceCommitSha: 'latest',
      staticAssetsManifest: {
        assets: {
          '/index.html': {
            contentType: 'text/html',
            r2Key: 'sites/site_project_otec/latest/index.html',
          },
        },
      },
    } as const

    await applyLifecycle(store, {
      actorUserId: 'openagents:runner-ingest',
      artifacts: artifacts as unknown as R2Bucket,
      events: [
        event({
          payloadJson: JSON.stringify({
            adjutantSiteArtifactReceipt: receipt,
          }),
          sequence: 7,
          status: 'completed',
          type: 'runner.completed',
        }),
      ],
      runId: 'agent_run_adjutant',
      status: 'completed',
    })

    const latestVersion = store.versions.find(
      version => version.id !== 'site_version_previous',
    )
    const latestDeployment = store.deployments.find(
      deployment => deployment.version_id === latestVersion?.id,
    )

    expect(latestVersion).toMatchObject({
      build_status: 'saved',
      source_commit_sha: 'latest',
    })
    expect(store.siteProjects[0]).toMatchObject({
      active_deployment_id: latestDeployment?.id,
      active_version_id: latestVersion?.id,
      status: 'needs_review',
    })
    expect(store.deployments).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'site_deployment_previous',
          rolled_back_at: expect.any(String),
          status: 'rolled_back',
          version_id: 'site_version_previous',
        }),
        expect.objectContaining({
          id: latestDeployment?.id,
          status: 'active',
          url: 'https://sites.openagents.com/otec',
          version_id: latestVersion?.id,
        }),
      ]),
    )
    expect(JSON.parse(latestVersion?.metadata_json ?? '{}')).toEqual(
      expect.objectContaining({
        customerAccepted: false,
        customerReviewState: 'customer_review_ready',
      }),
    )
    expect(store.feedback[0]).toMatchObject({
      status: 'addressed',
    })
    expect(store.siteEvents.map(item => item.type)).toEqual(
      expect.arrayContaining([
        'site_deployment.superseded',
        'site_deployment.activated',
      ]),
    )
    expect(JSON.stringify(store.siteEvents)).not.toContain('customerAccepted":true')
  })

  test('maps waiting and failed callbacks to customer-visible order states', async () => {
    const waitingStore = new LifecycleStore()
    const failedStore = new LifecycleStore()

    await applyLifecycle(waitingStore, {
      actorUserId: 'openagents:runner-ingest',
      events: [
        event({
          sequence: 4,
          status: 'waiting_for_input',
          type: 'runner.input_required',
        }),
      ],
      runId: 'agent_run_adjutant',
      status: 'waiting_for_input',
    })
    await applyLifecycle(failedStore, {
      actorUserId: 'openagents:runner-ingest',
      events: [event({ sequence: 5, status: 'failed', type: 'runner.failed' })],
      runId: 'agent_run_adjutant',
      status: 'failed',
    })

    expect(waitingStore.assignments[0]).toMatchObject({
      status: 'review_needed',
    })
    expect(waitingStore.orders[0]).toMatchObject({
      status: 'needs_customer_input',
    })
    expect(waitingStore.siteEvents[0]).toMatchObject({
      type: 'adjutant.customer_input_needed',
    })
    expect(waitingStore.siteEvents.map(item => item.type)).toEqual([
      'adjutant.customer_input_needed',
      'adjutant.notification.input_needed',
    ])
    expect(failedStore.assignments[0]).toMatchObject({
      blocked_at: expect.any(String),
      status: 'blocked',
    })
    expect(failedStore.orders[0]).toMatchObject({
      status: 'unavailable',
    })
    expect(failedStore.siteEvents[0]).toMatchObject({
      type: 'adjutant.run_unavailable',
    })
    expect(failedStore.siteEvents.map(item => item.type)).toEqual([
      'adjutant.run_unavailable',
      'adjutant.notification.unavailable',
    ])
  })
})
