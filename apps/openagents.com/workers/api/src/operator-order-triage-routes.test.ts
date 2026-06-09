import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { makeOperatorOrderTriageRoutes } from './operator-order-triage-routes'

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type StoredSoftwareOrder = Readonly<{
  id: string
  user_id: string
  status:
    | 'submitted'
    | 'scoping'
    | 'free_slice_ready'
    | 'quote_ready'
    | 'agent_queued'
    | 'agent_running'
    | 'delivered'
    | 'needs_customer_input'
    | 'declined'
    | 'unavailable'
  visibility: 'public'
  request: string
  repository_full_name: string | null
  current_run_id: string | null
  created_at: string
  updated_at: string
  archived_at: string | null
}>

type StoredUser = Readonly<{
  id: string
  display_name: string
  primary_email: string | null
  deleted_at: string | null
}>

type StoredSiteProject = Readonly<{
  id: string
  access_mode?: string
  active_deployment_id?: string | null
  active_version_id?: string | null
  created_at?: string
  owner_user_id?: string
  project_id?: string | null
  prompt?: string
  software_order_id: string
  source_repository_name?: string | null
  source_repository_owner?: string | null
  source_repository_provider?: string | null
  source_repository_ref?: string | null
  title: string
  slug: string
  status: string
  team_id?: string | null
  updated_at?: string
  visibility?: string
  archived_at: string | null
}>

type StoredAssignment = Readonly<{
  id: string
  software_order_id: string
  agent_id?: string
  assignment_kind: string
  assigned_by_user_id?: string | null
  blocked_at?: string | null
  commit_sha?: string | null
  completed_at?: string | null
  current_run_id?: string | null
  goal_id?: string | null
  objective?: string
  project_id?: string | null
  site_id?: string | null
  status: string
  task_spec_path?: string | null
  team_id?: string | null
  created_at: string
  updated_at?: string
  visibility?: string
  archived_at: string | null
}>

type StoredAssignmentEvent = Readonly<{
  actor_user_id: string | null
  assignment_id: string
  created_at: string
  event_type: string
  goal_id: string | null
  id: string
  payload_json: string | null
  run_id: string | null
  site_id: string | null
  software_order_id: string | null
  summary: string
  visibility: string
}>

type StoredAgentGoal = Readonly<{
  agent_id: string
  archived_at: string | null
  blocked_at: string | null
  completed_at: string | null
  created_at: string
  current_run_id: string | null
  id: string
  objective: string
  paused_at: string | null
  project_id: string | null
  status: string
  team_id: string | null
  time_used_seconds: number
  token_budget: number | null
  tokens_used: number
  updated_at: string
  user_id: string | null
  visibility: string
}>

type StoredTriageEvent = Readonly<{
  actor_user_id: string | null
  assignment_id: string | null
  created_at: string
  event_type: string
  id: string
  payload_json: string | null
  site_id: string | null
  software_order_id: string
  summary: string
  triage_record_id: string
  visibility: string
}>

type StoredAgentRun = Readonly<{
  completed_at: string | null
  created_at: string
  failed_at: string | null
  id: string
  provider_account_ref: string | null
  repository_owner: string
  repository_ref: string
  repository_repo: string
  started_at: string | null
  status: string
  updated_at: string
}>

type StoredAgentRunEvent = Readonly<{
  created_at: string
  run_id: string
  sequence: number
  status: string | null
  summary: string
  type: string
}>

type StoredProviderLease = Readonly<{
  account_health: string | null
  account_label: string | null
  account_status: string | null
  assignment_id: string | null
  cooldown_until: string | null
  expires_at: string
  failure_class: string | null
  last_touched_at: string | null
  lease_ref: string
  low_credit_flag: number | null
  order_id: string | null
  provider_account_ref: string
  requested_action: string
  run_id: string | null
  started_at: string
  status: string
  reauth_required_reason: string | null
}>

type StoredFailoverReceipt = Readonly<{
  account_state_action: string
  assignment_id: string | null
  cooldown_until: string | null
  created_at: string
  customer_safe_status: string
  customer_safe_summary: string | null
  failure_class: string
  id: string
  next_lease_ref: string | null
  next_provider_account_ref: string | null
  operator_summary: string
  order_id: string | null
  outcome: string
  policy_version: string
  previous_lease_ref: string | null
  previous_provider_account_ref: string | null
  run_id: string | null
}>

type StoredPaymentPolicy = Readonly<{
  applied_by_user_id: string | null
  archived_at: string | null
  assignment_id: string | null
  created_at: string
  customer_safe_summary: string
  id: string
  policy_mode: 'public_beta_free' | 'operator_grant'
  reason: string
  site_id: string | null
  software_order_id: string
  updated_at: string
}>

type StoredSiteBuilderSession = Readonly<{
  archived_at: string | null
  created_at: string
  id: string
  order_id: string | null
  site_id: string | null
  status: string
  updated_at: string
}>

type StoredSiteBuilderArtifact = Readonly<{
  archived_at: string | null
  artifact_ref: string
  created_at: string
  id: string
  metadata_json: string
  session_id: string
}>

type StoredTriageRecord = Readonly<{
  id: string
  software_order_id: string
  classification:
    | 'runnable_site'
    | 'runnable_general_autopilot'
    | 'needs_clarification'
    | 'smoke_or_test'
    | 'legal_sensitive_policy_review'
    | 'unavailable_or_declined'
  operator_priority: number
  first_batch_eligible: number
  hold_reason: string | null
  next_action: string
  customer_safe_status: string
  customer_safe_summary: string
  reviewer_user_id: string | null
  reviewed_at: string | null
  created_at: string
  updated_at: string
  archived_at: string | null
}>

type TriageRow = Record<string, unknown>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

class OperatorOrderTriageDbStore {
  orders: Array<StoredSoftwareOrder> = [
    {
      id: 'software_order_site',
      user_id: 'user_1',
      status: 'submitted',
      visibility: 'public',
      request: 'Build the Chefgroep website.',
      repository_full_name: 'OnlineChefGroep/chefgroep.nl',
      current_run_id: null,
      created_at: '2026-06-04T10:00:00.000Z',
      updated_at: '2026-06-04T10:00:00.000Z',
      archived_at: null,
    },
    {
      id: 'software_order_smoke',
      user_id: 'user_2',
      status: 'submitted',
      visibility: 'public',
      request: 'Smoke test this pipeline.',
      repository_full_name: 'OpenAgentsInc/autopilot-omega',
      current_run_id: 'agent_run_internal',
      created_at: '2026-06-04T11:00:00.000Z',
      updated_at: '2026-06-04T11:00:00.000Z',
      archived_at: null,
    },
    {
      id: 'software_order_fresh_site',
      user_id: 'user_4',
      status: 'submitted',
      visibility: 'public',
      request: 'Build the Ben OTEC website.',
      repository_full_name: 'ben/otec-site',
      current_run_id: null,
      created_at: '2026-06-04T09:00:00.000Z',
      updated_at: '2026-06-04T09:00:00.000Z',
      archived_at: null,
    },
    {
      id: 'software_order_legal',
      user_id: 'user_3',
      status: 'submitted',
      visibility: 'public',
      request: 'Legal-sensitive request.',
      repository_full_name: null,
      current_run_id: null,
      created_at: '2026-06-04T12:00:00.000Z',
      updated_at: '2026-06-04T12:00:00.000Z',
      archived_at: null,
    },
  ]
  users: Array<StoredUser> = [
    {
      id: 'user_1',
      display_name: 'Chef Groep',
      primary_email: 'chef@example.com',
      deleted_at: null,
    },
    {
      id: 'user_2',
      display_name: 'Omega',
      primary_email: 'omega@example.com',
      deleted_at: null,
    },
    {
      id: 'user_3',
      display_name: 'Legal Customer',
      primary_email: 'legal@example.com',
      deleted_at: null,
    },
    {
      id: 'user_4',
      display_name: 'Ben OTEC',
      primary_email: 'ben@example.com',
      deleted_at: null,
    },
  ]
  sites: Array<StoredSiteProject> = [
    {
      id: 'site_project_1',
      software_order_id: 'software_order_site',
      title: 'Chefgroep',
      slug: 'chefgroep',
      status: 'draft',
      archived_at: null,
    },
  ]
  assignments: Array<StoredAssignment> = [
    {
      id: 'assignment_1',
      software_order_id: 'software_order_site',
      assignment_kind: 'site_generation',
      status: 'queued',
      created_at: '2026-06-04T10:30:00.000Z',
      archived_at: null,
    },
  ]
  assignmentEvents: Array<StoredAssignmentEvent> = []
  failoverReceipts: Array<StoredFailoverReceipt> = []
  goals: Array<StoredAgentGoal> = []
  leases: Array<StoredProviderLease> = []
  paymentPolicies: Array<StoredPaymentPolicy> = []
  runEvents: Array<StoredAgentRunEvent> = []
  runs: Array<StoredAgentRun> = []
  siteBuilderArtifacts: Array<StoredSiteBuilderArtifact> = [
    {
      archived_at: null,
      artifact_ref: 'artifact.site_builder.public_manifest',
      created_at: '2026-06-04T10:35:00.000Z',
      id: 'site_builder_artifact_public',
      metadata_json: JSON.stringify({ publicSafe: true }),
      session_id: 'site_builder_session_1',
    },
  ]
  siteBuilderSessions: Array<StoredSiteBuilderSession> = [
    {
      archived_at: null,
      created_at: '2026-06-04T10:31:00.000Z',
      id: 'site_builder_session_1',
      order_id: 'software_order_site',
      site_id: 'site_project_1',
      status: 'generated',
      updated_at: '2026-06-04T10:35:00.000Z',
    },
  ]
  triageRecords: Array<StoredTriageRecord> = [
    {
      id: 'triage_fresh_site',
      software_order_id: 'software_order_fresh_site',
      classification: 'runnable_site',
      operator_priority: 5,
      first_batch_eligible: 1,
      hold_reason: null,
      next_action: 'Create a Site assignment.',
      customer_safe_status: 'scoping',
      customer_safe_summary: 'Preparing this website order.',
      reviewer_user_id: null,
      reviewed_at: null,
      created_at: '2026-06-04T09:01:00.000Z',
      updated_at: '2026-06-04T09:01:00.000Z',
      archived_at: null,
    },
    {
      id: 'triage_site',
      software_order_id: 'software_order_site',
      classification: 'runnable_site',
      operator_priority: 10,
      first_batch_eligible: 1,
      hold_reason: null,
      next_action: 'Run compatibility check.',
      customer_safe_status: 'scoping',
      customer_safe_summary: 'Preparing this website order.',
      reviewer_user_id: null,
      reviewed_at: null,
      created_at: '2026-06-04T10:01:00.000Z',
      updated_at: '2026-06-04T10:01:00.000Z',
      archived_at: null,
    },
    {
      id: 'triage_smoke',
      software_order_id: 'software_order_smoke',
      classification: 'smoke_or_test',
      operator_priority: 200,
      first_batch_eligible: 0,
      hold_reason: 'Pipeline smoke test.',
      next_action: 'Hold until promoted.',
      customer_safe_status: 'held',
      customer_safe_summary:
        'This request is held while OpenAgents reviews it.',
      reviewer_user_id: null,
      reviewed_at: null,
      created_at: '2026-06-04T11:01:00.000Z',
      updated_at: '2026-06-04T11:01:00.000Z',
      archived_at: null,
    },
    {
      id: 'triage_legal',
      software_order_id: 'software_order_legal',
      classification: 'legal_sensitive_policy_review',
      operator_priority: 500,
      first_batch_eligible: 1,
      hold_reason: 'Legal-sensitive request.',
      next_action: 'Hold for policy review.',
      customer_safe_status: 'policy_review',
      customer_safe_summary: 'This request needs human review.',
      reviewer_user_id: null,
      reviewed_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:01:00.000Z',
      archived_at: null,
    },
  ]
  triageEvents: Array<StoredTriageEvent> = []
}

const meta = (changes = 0): D1Meta & Record<string, unknown> =>
  ({ changes }) as D1Meta & Record<string, unknown>

const orderById = (
  store: OperatorOrderTriageDbStore,
  softwareOrderId: string,
): StoredSoftwareOrder | undefined =>
  store.orders.find(
    order => order.id === softwareOrderId && order.archived_at === null,
  )

const latestAssignment = (
  store: OperatorOrderTriageDbStore,
  softwareOrderId: string,
): StoredAssignment | undefined =>
  store.assignments
    .filter(
      assignment =>
        assignment.software_order_id === softwareOrderId &&
        assignment.archived_at === null,
    )
    .sort((left, right) => right.created_at.localeCompare(left.created_at))
    .at(0)

const rowForTriage = (
  store: OperatorOrderTriageDbStore,
  triage: StoredTriageRecord,
): TriageRow | null => {
  const order = orderById(store, triage.software_order_id)

  if (order === undefined || triage.archived_at !== null) {
    return null
  }

  const user =
    store.users.find(
      candidate =>
        candidate.id === order.user_id && candidate.deleted_at === null,
    ) ?? null
  const site =
    store.sites.find(
      candidate =>
        candidate.software_order_id === order.id &&
        candidate.archived_at === null,
    ) ?? null
  const assignment = latestAssignment(store, order.id) ?? null

  return {
    id: triage.id,
    software_order_id: triage.software_order_id,
    classification: triage.classification,
    operator_priority: triage.operator_priority,
    first_batch_eligible: triage.first_batch_eligible,
    hold_reason: triage.hold_reason,
    next_action: triage.next_action,
    customer_safe_status: triage.customer_safe_status,
    customer_safe_summary: triage.customer_safe_summary,
    reviewer_user_id: triage.reviewer_user_id,
    reviewed_at: triage.reviewed_at,
    created_at: triage.created_at,
    updated_at: triage.updated_at,
    order_id: order.id,
    order_user_id: order.user_id,
    user_display_name: user?.display_name ?? null,
    user_email: user?.primary_email ?? null,
    order_status: order.status,
    order_visibility: order.visibility,
    order_request: order.request,
    repository_full_name: order.repository_full_name,
    current_run_id: order.current_run_id,
    site_project_id: site?.id ?? null,
    site_title: site?.title ?? null,
    site_slug: site?.slug ?? null,
    site_status: site?.status ?? null,
    latest_assignment_id: assignment?.id ?? null,
    latest_assignment_status: assignment?.status ?? null,
    latest_assignment_kind: assignment?.assignment_kind ?? null,
    order_created_at: order.created_at,
    order_updated_at: order.updated_at,
  }
}

class OperatorOrderTriageStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OperatorOrderTriageDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM first_batch_payment_policies')) {
      const [softwareOrderId] = this.values
      const row =
        this.store.paymentPolicies
          .filter(
            policy =>
              policy.software_order_id === softwareOrderId &&
              policy.archived_at === null,
          )
          .sort((left, right) =>
            right.updated_at.localeCompare(left.updated_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM order_triage_records')) {
      const [softwareOrderId] = this.values
      const row =
        storeRows(this.store)
          .filter(candidate => candidate.software_order_id === softwareOrderId)
          .at(0) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM site_projects') &&
      this.query.includes('software_order_id = ?')
    ) {
      const [softwareOrderId] = this.values
      const row =
        this.store.sites.find(
          site =>
            site.software_order_id === softwareOrderId &&
            site.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM site_projects') &&
      this.query.includes('slug = ?')
    ) {
      const [slug] = this.values
      const row =
        this.store.sites.find(
          site => site.slug === slug && site.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM site_projects')) {
      const [siteId] = this.values
      const row =
        this.store.sites.find(
          site => site.id === siteId && site.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes("status NOT IN ('complete', 'canceled')")
    ) {
      const [softwareOrderId, siteId] = this.values
      const row =
        this.store.assignments.find(
          assignment =>
            assignment.archived_at === null &&
            assignment.status !== 'complete' &&
            assignment.status !== 'canceled' &&
            ((softwareOrderId !== null &&
              assignment.software_order_id === softwareOrderId) ||
              (siteId !== null && assignment.site_id === siteId)),
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      const [softwareOrderId, siteId] = this.values
      const row =
        this.store.assignments
          .filter(
            assignment =>
              assignment.archived_at === null &&
              (assignment.software_order_id === softwareOrderId ||
                (siteId !== null && assignment.site_id === siteId)),
          )
          .sort((left, right) =>
            String(right.updated_at ?? right.created_at).localeCompare(
              String(left.updated_at ?? left.created_at),
            ),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [assignmentId] = this.values
      const row =
        this.store.assignments.find(
          assignment =>
            assignment.id === assignmentId && assignment.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (
      this.query.includes('FROM agent_goals') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [goalId] = this.values
      const row =
        this.store.goals.find(
          goal => goal.id === goalId && goal.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM agent_goals')) {
      const [agentId, userId, teamId, projectId] = this.values
      const row =
        this.store.goals.find(
          goal =>
            goal.agent_id === agentId &&
            (goal.user_id ?? '') === (userId ?? '') &&
            (goal.team_id ?? '') === (teamId ?? '') &&
            (goal.project_id ?? '') === (projectId ?? '') &&
            goal.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM software_orders')) {
      const [softwareOrderId] = this.values
      const row = orderById(this.store, String(softwareOrderId))

      return Promise.resolve((row ?? null) as T | null)
    }

    if (this.query.includes('FROM agent_runs')) {
      const [runId] = this.values
      const row =
        this.store.runs.find(run => run.id === runId) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM agent_run_events')) {
      const [runId] = this.values
      const row =
        this.store.runEvents
          .filter(event => event.run_id === runId)
          .sort((left, right) => right.sequence - left.sequence)[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM order_triage_events')) {
      const [softwareOrderId] = this.values
      const row =
        this.store.triageEvents
          .filter(event => event.software_order_id === softwareOrderId)
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .map(event => ({
            created_at: event.created_at,
            status: null,
            summary: event.summary,
            type: event.event_type,
          }))[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM provider_account_leases')) {
      const [orderId, assignmentId, , runId] = this.values
      const row =
        this.store.leases
          .filter(
            lease =>
              lease.status === 'active' &&
              (lease.order_id === orderId ||
                (assignmentId !== null && lease.assignment_id === assignmentId) ||
                (runId !== null && lease.run_id === runId)),
          )
          .sort((left, right) =>
            right.started_at.localeCompare(left.started_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('FROM provider_account_failover_receipts')) {
      const [orderId, assignmentId, , runId] = this.values
      const row =
        this.store.failoverReceipts
          .filter(
            receipt =>
              receipt.order_id === orderId ||
              (assignmentId !== null && receipt.assignment_id === assignmentId) ||
              (runId !== null && receipt.run_id === runId),
          )
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )[0] ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('UPDATE software_orders')) {
      const [status, updatedAt, softwareOrderId] = this.values
      const index = this.store.orders.findIndex(
        order => order.id === softwareOrderId && order.archived_at === null,
      )

      const order = this.store.orders[index]

      if (order !== undefined) {
        this.store.orders[index] = {
          ...order,
          status: status as StoredSoftwareOrder['status'],
          updated_at: String(updatedAt),
        }
      }

      return Promise.resolve({
        meta: meta(order === undefined ? 0 : 1),
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('UPDATE first_batch_payment_policies')) {
      const [
        assignmentId,
        siteId,
        policyMode,
        appliedByUserId,
        reason,
        customerSafeSummary,
        updatedAt,
        softwareOrderId,
      ] = this.values
      const index = this.store.paymentPolicies.findIndex(
        policy =>
          policy.software_order_id === softwareOrderId &&
          policy.archived_at === null,
      )
      const policy = this.store.paymentPolicies[index]

      if (policy !== undefined) {
        this.store.paymentPolicies[index] = {
          ...policy,
          applied_by_user_id:
            typeof appliedByUserId === 'string' ? appliedByUserId : null,
          assignment_id:
            typeof assignmentId === 'string' ? assignmentId : null,
          customer_safe_summary: String(customerSafeSummary),
          policy_mode:
            policyMode === 'operator_grant'
              ? 'operator_grant'
              : 'public_beta_free',
          reason: String(reason),
          site_id: typeof siteId === 'string' ? siteId : null,
          updated_at: String(updatedAt),
        }
      }

      return Promise.resolve({
        meta: meta(policy === undefined ? 0 : 1),
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO first_batch_payment_policies')) {
      const [
        id,
        softwareOrderId,
        assignmentId,
        siteId,
        policyMode,
        appliedByUserId,
        reason,
        customerSafeSummary,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.paymentPolicies.push({
        applied_by_user_id:
          typeof appliedByUserId === 'string' ? appliedByUserId : null,
        archived_at: null,
        assignment_id: typeof assignmentId === 'string' ? assignmentId : null,
        created_at: String(createdAt),
        customer_safe_summary: String(customerSafeSummary),
        id: String(id),
        policy_mode:
          policyMode === 'operator_grant'
            ? 'operator_grant'
            : 'public_beta_free',
        reason: String(reason),
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id: String(softwareOrderId),
        updated_at: String(updatedAt),
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE order_triage_records')) {
      const [
        classification,
        operatorPriority,
        firstBatchEligible,
        holdReason,
        nextAction,
        customerSafeStatus,
        customerSafeSummary,
        reviewerUserId,
        reviewedAt,
        updatedAt,
        softwareOrderId,
      ] = this.values
      const index = this.store.triageRecords.findIndex(
        record =>
          record.software_order_id === softwareOrderId &&
          record.archived_at === null,
      )

      const record = this.store.triageRecords[index]

      if (record !== undefined) {
        this.store.triageRecords[index] = {
          ...record,
          classification:
            classification as StoredTriageRecord['classification'],
          operator_priority: Number(operatorPriority),
          first_batch_eligible: Number(firstBatchEligible),
          hold_reason: holdReason === null ? null : String(holdReason),
          next_action: String(nextAction),
          customer_safe_status: String(customerSafeStatus),
          customer_safe_summary: String(customerSafeSummary),
          reviewer_user_id:
            reviewerUserId === null ? null : String(reviewerUserId),
          reviewed_at: reviewedAt === null ? null : String(reviewedAt),
          updated_at: String(updatedAt),
        }
      }

      return Promise.resolve({
        meta: meta(record === undefined ? 0 : 1),
        success: true,
      } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO order_triage_records')) {
      const [
        id,
        softwareOrderId,
        classification,
        operatorPriority,
        firstBatchEligible,
        holdReason,
        nextAction,
        customerSafeStatus,
        customerSafeSummary,
        reviewerUserId,
        reviewedAt,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.triageRecords.push({
        id: String(id),
        software_order_id: String(softwareOrderId),
        classification: classification as StoredTriageRecord['classification'],
        operator_priority: Number(operatorPriority),
        first_batch_eligible: Number(firstBatchEligible),
        hold_reason: holdReason === null ? null : String(holdReason),
        next_action: String(nextAction),
        customer_safe_status: String(customerSafeStatus),
        customer_safe_summary: String(customerSafeSummary),
        reviewer_user_id:
          reviewerUserId === null ? null : String(reviewerUserId),
        reviewed_at: reviewedAt === null ? null : String(reviewedAt),
        created_at: String(createdAt),
        updated_at: String(updatedAt),
        archived_at: null,
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE agent_goals')) {
      const [archivedAt, updatedAt, agentId, userId, teamId, projectId] =
        this.values

      this.store.goals = this.store.goals.map(goal =>
        goal.agent_id === agentId &&
        (goal.user_id ?? '') === (userId ?? '') &&
        (goal.team_id ?? '') === (teamId ?? '') &&
        (goal.project_id ?? '') === (projectId ?? '') &&
        goal.archived_at === null
          ? {
              ...goal,
              archived_at: String(archivedAt),
              updated_at: String(updatedAt),
            }
          : goal,
      )

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO agent_goals')) {
      const [
        id,
        agentId,
        userId,
        teamId,
        projectId,
        objective,
        visibility,
        tokenBudget,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.goals.push({
        agent_id: String(agentId),
        archived_at: null,
        blocked_at: null,
        completed_at: null,
        created_at: String(createdAt),
        current_run_id: null,
        id: String(id),
        objective: String(objective),
        paused_at: null,
        project_id: typeof projectId === 'string' ? projectId : null,
        status: 'active',
        team_id: typeof teamId === 'string' ? teamId : null,
        time_used_seconds: 0,
        token_budget: typeof tokenBudget === 'number' ? tokenBudget : null,
        tokens_used: 0,
        updated_at: String(updatedAt),
        user_id: typeof userId === 'string' ? userId : null,
        visibility: String(visibility),
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

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
        sourceProvider,
        sourceOwner,
        sourceName,
        sourceRef,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.sites.push({
        access_mode: String(accessMode),
        active_deployment_id: null,
        active_version_id: null,
        archived_at: null,
        created_at: String(createdAt),
        id: String(id),
        owner_user_id: String(ownerUserId),
        project_id: typeof projectId === 'string' ? projectId : null,
        prompt: String(prompt),
        slug: String(slug),
        software_order_id: String(softwareOrderId),
        source_repository_name:
          typeof sourceName === 'string' ? sourceName : null,
        source_repository_owner:
          typeof sourceOwner === 'string' ? sourceOwner : null,
        source_repository_provider:
          typeof sourceProvider === 'string' ? sourceProvider : null,
        source_repository_ref: typeof sourceRef === 'string' ? sourceRef : null,
        status: 'draft',
        team_id: typeof teamId === 'string' ? teamId : null,
        title: String(title),
        updated_at: String(updatedAt),
        visibility: String(visibility),
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_assignments')) {
      const [
        id,
        softwareOrderId,
        siteId,
        goalId,
        currentRunId,
        teamId,
        projectId,
        agentId,
        assignedByUserId,
        assignmentKind,
        status,
        visibility,
        taskSpecPath,
        commitSha,
        objective,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.assignments.push({
        agent_id: String(agentId),
        archived_at: null,
        assigned_by_user_id:
          typeof assignedByUserId === 'string' ? assignedByUserId : null,
        assignment_kind: String(assignmentKind),
        blocked_at: null,
        commit_sha: typeof commitSha === 'string' ? commitSha : null,
        completed_at: null,
        created_at: String(createdAt),
        current_run_id: typeof currentRunId === 'string' ? currentRunId : null,
        goal_id: typeof goalId === 'string' ? goalId : null,
        id: String(id),
        objective: String(objective),
        project_id: typeof projectId === 'string' ? projectId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : '',
        status: String(status),
        task_spec_path: typeof taskSpecPath === 'string' ? taskSpecPath : null,
        team_id: typeof teamId === 'string' ? teamId : null,
        updated_at: String(updatedAt),
        visibility: String(visibility),
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_events')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        goalId,
        runId,
        eventType,
        visibility,
        summary,
        actorUserId,
        payloadJson,
        createdAt,
      ] = this.values

      this.store.assignmentEvents.push({
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        event_type: String(eventType),
        goal_id: typeof goalId === 'string' ? goalId : null,
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        run_id: typeof runId === 'string' ? runId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        summary: String(summary),
        visibility: String(visibility),
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO order_triage_events')) {
      const [
        id,
        triageRecordId,
        softwareOrderId,
        siteId,
        assignmentId,
        eventType,
        summary,
        actorUserId,
        payloadJson,
        createdAt,
      ] = this.values

      this.store.triageEvents.push({
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        assignment_id:
          typeof assignmentId === 'string' ? assignmentId : null,
        created_at: String(createdAt),
        event_type: String(eventType),
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id: String(softwareOrderId),
        summary: String(summary),
        triage_record_id: String(triageRecordId),
        visibility: 'team',
      })

      return Promise.resolve({ meta: meta(1), success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM order_triage_records')) {
      const [limit] = this.values

      return Promise.resolve({
        meta: meta(),
        results: storeRows(this.store)
          .sort(
            (left, right) =>
              Number(right.first_batch_eligible) -
                Number(left.first_batch_eligible) ||
              Number(left.operator_priority) -
                Number(right.operator_priority) ||
              String(right.updated_at).localeCompare(String(left.updated_at)),
          )
          .slice(0, Number(limit ?? 100)) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM software_orders')) {
      const [limit] = this.values

      return Promise.resolve({
        meta: meta(),
        results: this.store.orders
          .filter(order => order.archived_at === null)
          .sort((left, right) =>
            right.updated_at.localeCompare(left.updated_at),
          )
          .slice(0, Number(limit ?? 100))
          .map(order => ({
            archived_at: order.archived_at,
            created_at: order.created_at,
            current_run_id: order.current_run_id,
            id: order.id,
            repository_private: null,
            status: order.status,
            updated_at: order.updated_at,
            visibility: order.visibility,
          })) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      const [limit] = this.values

      return Promise.resolve({
        meta: meta(),
        results: this.store.assignments
          .filter(assignment => assignment.archived_at === null)
          .sort((left, right) =>
            String(right.updated_at ?? right.created_at).localeCompare(
              String(left.updated_at ?? left.created_at),
            ),
          )
          .slice(0, Number(limit ?? 100))
          .map(assignment => ({
            archived_at: assignment.archived_at,
            completed_at: assignment.completed_at ?? null,
            current_run_id: assignment.current_run_id ?? null,
            id: assignment.id,
            site_id: assignment.site_id ?? null,
            software_order_id: assignment.software_order_id,
            status: assignment.status,
            updated_at: assignment.updated_at ?? assignment.created_at,
            visibility: assignment.visibility ?? 'public',
          })) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM site_projects')) {
      const [limit] = this.values

      return Promise.resolve({
        meta: meta(),
        results: this.store.sites
          .filter(site => site.archived_at === null)
          .sort((left, right) =>
            String(right.updated_at ?? right.created_at ?? '').localeCompare(
              String(left.updated_at ?? left.created_at ?? ''),
            ),
          )
          .slice(0, Number(limit ?? 100))
          .map(site => ({
            active_deployment_id: site.active_deployment_id ?? null,
            active_version_id: site.active_version_id ?? null,
            archived_at: site.archived_at,
            id: site.id,
            software_order_id: site.software_order_id,
            status: site.status,
            updated_at: site.updated_at ?? '2026-06-04T10:00:00.000Z',
            visibility: site.visibility ?? 'public',
          })) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM site_builder_artifacts')) {
      const [limit] = this.values

      return Promise.resolve({
        meta: meta(),
        results: this.store.siteBuilderArtifacts
          .flatMap(artifact => {
            const session = this.store.siteBuilderSessions.find(
              candidate =>
                candidate.id === artifact.session_id &&
                candidate.archived_at === null,
            )

            return artifact.archived_at !== null || session === undefined
              ? []
              : [{
                  archived_at: artifact.archived_at,
                  artifact_ref: artifact.artifact_ref,
                  created_at: artifact.created_at,
                  id: artifact.id,
                  metadata_json: artifact.metadata_json,
                  order_id: session.order_id,
                  session_id: session.id,
                  session_status: session.status,
                  site_id: session.site_id,
                }]
          })
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .slice(0, Number(limit ?? 100)) as Array<T>,
        success: true,
      })
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

const storeRows = (store: OperatorOrderTriageDbStore): Array<TriageRow> =>
  store.triageRecords
    .map(record => rowForTriage(store, record))
    .filter((row): row is TriageRow => row !== null)

const db = (store: OperatorOrderTriageDbStore): D1Database => ({
  batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
    Promise.all(statements.map(statement => statement.run())) as Promise<
      Array<D1Result<T>>
    >,
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OperatorOrderTriageStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const makeRoutes = (
  session: TestSession | null,
  hasAdminApiToken = false,
) =>
  makeOperatorOrderTriageRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    isOpenAgentsAdminEmail: email => email === 'admin@openagents.com',
    requireAdminApiToken: () => Promise.resolve(hasAdminApiToken),
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
  })

const runRoute = (
  store: OperatorOrderTriageDbStore,
  path: string,
  init: RequestInit = {},
  session: TestSession | null = {
    user: { email: 'admin@openagents.com', userId: 'admin_user' },
  },
  hasAdminApiToken = false,
): Promise<Response> => {
  const route = makeRoutes(
    session,
    hasAdminApiToken,
  ).routeOperatorOrderTriageRequest(
    new Request(`https://openagents.com${path}`, init),
    { OPENAGENTS_DB: db(store) },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

describe('operator order triage routes', () => {
  test('requires an admin browser session', async () => {
    const response = await runRoute(
      new OperatorOrderTriageDbStore(),
      '/api/operator/orders/triage',
      {},
      null,
    )

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('rejects non-admin sessions', async () => {
    const response = await runRoute(
      new OperatorOrderTriageDbStore(),
      '/api/operator/orders/triage',
      {},
      {
        user: { email: 'customer@example.com', userId: 'user_1' },
      },
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('lists the operator priority queue without provider account mechanics', async () => {
    const response = await runRoute(
      new OperatorOrderTriageDbStore(),
      '/api/operator/orders/triage',
    )
    const body = (await response.json()) as {
      queue: Array<Record<string, unknown>>
    }

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    expect(body.queue.map(record => record.softwareOrderId)).toEqual([
      'software_order_fresh_site',
      'software_order_site',
      'software_order_legal',
      'software_order_smoke',
    ])
    expect(body.queue[1]).toMatchObject({
      classification: 'runnable_site',
      firstBatchEligible: true,
      overnightLaunchEligible: true,
      order: {
        repositoryFullName: 'OnlineChefGroep/chefgroep.nl',
        siteProjectId: 'site_project_1',
        latestAssignmentId: 'assignment_1',
      },
    })
    expect(body.queue[2]).toMatchObject({
      classification: 'legal_sensitive_policy_review',
      firstBatchEligible: true,
      overnightLaunchEligible: false,
    })
    expect(JSON.stringify(body)).not.toContain('provider_account')
    expect(JSON.stringify(body)).not.toContain('auth_grant')
  })

  test('reports current queue foldover inventory without mutating records', async () => {
    const store = new OperatorOrderTriageDbStore()
    const before = {
      artifacts: store.siteBuilderArtifacts.length,
      assignments: store.assignments.length,
      orders: store.orders.length,
      sites: store.sites.length,
    }
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/autopilot-foldover-inventory?limit=25',
    )
    const body = (await response.json()) as {
      inventory: Readonly<{
        dryRun: boolean
        items: ReadonlyArray<Record<string, unknown>>
        mutatesRecords: boolean
        summary: Record<string, unknown>
      }>
    }

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    expect(body.inventory).toMatchObject({
      dryRun: true,
      mutatesRecords: false,
      summary: {
        byPrivacyState: {
          private_only: 0,
          public_safe: 7,
        },
        bySourceKind: {
          adjutant_assignment: 1,
          site_builder_artifact: 1,
          site_project: 1,
          software_order: 4,
        },
        byState: {
          delivered: 1,
          pending: 0,
          running: 0,
          stale: 6,
        },
        foldable: 6,
        total: 7,
      },
    })
    expect(body.inventory.items).toContainEqual(
      expect.objectContaining({
        artifactRef: 'artifact.site_builder.public_manifest',
        foldableIntoAutopilot: false,
        privacyState: 'public_safe',
        reasonRefs: [
          'foldover.source.site_builder_artifact',
          'foldover.state.delivered',
          'foldover.privacy.public_safe',
        ],
        sourceKind: 'site_builder_artifact',
        state: 'delivered',
      }),
    )
    expect(body.inventory.items).toContainEqual(
      expect.objectContaining({
        assignmentId: 'assignment_1',
        foldableIntoAutopilot: true,
        sourceKind: 'adjutant_assignment',
        state: 'stale',
      }),
    )
    expect({
      artifacts: store.siteBuilderArtifacts.length,
      assignments: store.assignments.length,
      orders: store.orders.length,
      sites: store.sites.length,
    }).toEqual(before)
  })

  test('updates triage and can move the customer-safe order status', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/software_order_site/triage',
      {
        body: JSON.stringify({
          classification: 'runnable_site',
          operatorPriority: 5,
          firstBatchEligible: true,
          holdReason: null,
          nextAction: 'Create a Site assignment.',
          customerSafeStatus: 'scoping',
          customerSafeSummary: 'OpenAgents is preparing this website order.',
          orderStatus: 'scoping',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'PATCH',
      },
    )
    const body = (await response.json()) as {
      record: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(body.record).toMatchObject({
      softwareOrderId: 'software_order_site',
      operatorPriority: 5,
      nextAction: 'Create a Site assignment.',
      customerSafeStatus: 'scoping',
      order: {
        status: 'scoping',
      },
    })
  })

  test('dry-runs first-batch assignment creation without mutating assignments', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/assign',
      {
        body: JSON.stringify({
          dryRun: true,
          softwareOrderIds: ['software_order_fresh_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      results: Array<Record<string, unknown>>
      summary: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(body.summary).toMatchObject({
      dryRun: true,
      total: 1,
      wouldCreate: 1,
      created: 0,
    })
    expect(body.results[0]).toMatchObject({
      decision: 'would_create_assignment',
      softwareOrderId: 'software_order_fresh_site',
      site: expect.objectContaining({
        slug: 'ben-otec-fresh-site',
        title: 'Ben OTEC',
      }),
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.triageEvents).toHaveLength(0)
    expect(JSON.stringify(body)).not.toContain('provider_account')
    expect(JSON.stringify(body)).not.toContain('auth_grant')
  })

  test('creates first-batch Site assignments and durable safe receipts', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/assign',
      {
        body: JSON.stringify({
          softwareOrderIds: ['software_order_fresh_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      results: Array<Record<string, unknown>>
      summary: Record<string, unknown>
    }

    expect(response.status).toBe(201)
    expect(body.summary).toMatchObject({
      created: 1,
      dryRun: false,
      total: 1,
    })
    expect(body.results[0]).toMatchObject({
      customerSafeStatus: 'queued',
      decision: 'created_assignment',
      orderStatus: 'agent_queued',
      softwareOrderId: 'software_order_fresh_site',
    })

    const assignment = store.assignments.find(
      item => item.software_order_id === 'software_order_fresh_site',
    )
    const site = store.sites.find(
      item => item.software_order_id === 'software_order_fresh_site',
    )

    expect(site).toMatchObject({
      access_mode: 'customer_owner',
      slug: 'ben-otec-fresh-site',
      visibility: 'team',
    })
    expect(assignment).toMatchObject({
      assigned_by_user_id: 'admin_user',
      assignment_kind: 'site_generation',
      site_id: site?.id,
      status: 'preflight_pending',
      visibility: 'team',
    })
    expect(store.goals).toHaveLength(1)
    expect(store.assignmentEvents.map(event => event.event_type)).toContain(
      'adjutant.first_batch_assignment_prepared',
    )
    expect(store.triageEvents).toEqual([
      expect.objectContaining({
        assignment_id: assignment?.id,
        event_type: 'order_triage.first_batch_assignment_created',
        site_id: site?.id,
        software_order_id: 'software_order_fresh_site',
        visibility: 'team',
      }),
    ])
    expect(orderById(store, 'software_order_fresh_site')?.status).toBe(
      'agent_queued',
    )
    expect(JSON.stringify(body)).not.toContain('provider_account')
    expect(JSON.stringify(store.triageEvents)).not.toContain('auth_grant')
  })

  test('prepares a non-first-batch runnable Site order for fulfillment', async () => {
    const store = new OperatorOrderTriageDbStore()
    store.orders.push({
      id: 'software_order_later_site',
      user_id: 'user_4',
      status: 'submitted',
      visibility: 'public',
      request: 'Build the later OTEC campaign site.',
      repository_full_name: 'ben/later-otec-site',
      current_run_id: null,
      created_at: '2026-06-04T13:00:00.000Z',
      updated_at: '2026-06-04T13:00:00.000Z',
      archived_at: null,
    })
    store.triageRecords.push({
      id: 'triage_later_site',
      software_order_id: 'software_order_later_site',
      classification: 'runnable_site',
      operator_priority: 50,
      first_batch_eligible: 0,
      hold_reason: null,
      next_action: 'Prepare order fulfillment.',
      customer_safe_status: 'scoping',
      customer_safe_summary: 'OpenAgents is preparing this website order.',
      reviewer_user_id: null,
      reviewed_at: null,
      created_at: '2026-06-04T13:01:00.000Z',
      updated_at: '2026-06-04T13:01:00.000Z',
      archived_at: null,
    })

    const response = await runRoute(
      store,
      '/api/operator/orders/software_order_later_site/fulfillment/prepare',
      {
        body: JSON.stringify({}),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      result: Record<string, unknown>
    }
    const assignment = store.assignments.find(
      item => item.software_order_id === 'software_order_later_site',
    )
    const site = store.sites.find(
      item => item.software_order_id === 'software_order_later_site',
    )

    expect(response.status).toBe(201)
    expect(body.result).toMatchObject({
      decision: 'created_assignment',
      firstBatchEligible: false,
      orderStatus: 'agent_queued',
      softwareOrderId: 'software_order_later_site',
      siteId: site?.id,
    })
    expect(site).toMatchObject({
      access_mode: 'customer_owner',
      visibility: 'team',
    })
    expect(assignment).toMatchObject({
      assignment_kind: 'site_generation',
      site_id: site?.id,
      status: 'preflight_pending',
    })
    expect(store.assignmentEvents.map(event => event.event_type)).toContain(
      'adjutant.order_fulfillment_prepared',
    )
    expect(store.triageEvents).toEqual([
      expect.objectContaining({
        assignment_id: assignment?.id,
        event_type: 'order_fulfillment.prepare_assignment_created',
        site_id: site?.id,
        software_order_id: 'software_order_later_site',
      }),
    ])
    expect(JSON.stringify(body)).not.toContain('provider_account')
    expect(JSON.stringify(store.triageEvents)).not.toContain('auth_grant')
  })

  test('preparing fulfillment reuses active assignments and holds non-runnable orders', async () => {
    const store = new OperatorOrderTriageDbStore()
    const existing = await runRoute(
      store,
      '/api/operator/orders/software_order_site/fulfillment/prepare',
      {
        body: JSON.stringify({}),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const existingBody = (await existing.json()) as {
      result: Record<string, unknown>
    }
    const held = await runRoute(
      store,
      '/api/operator/orders/software_order_smoke/fulfillment/prepare',
      {
        body: JSON.stringify({}),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const heldBody = (await held.json()) as {
      result: Record<string, unknown>
    }

    expect(existing.status).toBe(200)
    expect(existingBody.result).toMatchObject({
      assignmentId: 'assignment_1',
      decision: 'already_assigned',
      softwareOrderId: 'software_order_site',
    })
    expect(held.status).toBe(200)
    expect(heldBody.result).toMatchObject({
      decision: 'held',
      holdReason: 'Pipeline smoke test.',
      orderStatus: 'scoping',
      softwareOrderId: 'software_order_smoke',
    })
    expect(store.triageEvents.map(event => event.event_type)).toEqual([
      'order_fulfillment.prepare_already_assigned',
      'order_fulfillment.prepare_held',
    ])
  })

  test('applies first-batch no-payment policy idempotently', async () => {
    const store = new OperatorOrderTriageDbStore()
    const first = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/payment-policy',
      {
        body: JSON.stringify({
          softwareOrderIds: ['software_order_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const firstBody = (await first.json()) as {
      policies: Array<Record<string, unknown>>
    }

    expect(first.status).toBe(201)
    expect(firstBody.policies[0]).toMatchObject({
      assignmentId: 'assignment_1',
      paymentPolicy: {
        appliedByUserId: 'admin_user',
        policyMode: 'public_beta_free',
        softwareOrderId: 'software_order_site',
      },
      siteId: 'site_project_1',
      softwareOrderId: 'software_order_site',
    })
    expect(store.paymentPolicies).toHaveLength(1)

    const second = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/payment-policy',
      {
        body: JSON.stringify({
          policyMode: 'operator_grant',
          reason: 'Operator grant for first overnight batch.',
          customerSafeSummary:
            'This first-batch run is covered by an OpenAgents operator grant.',
          softwareOrderIds: ['software_order_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const secondBody = (await second.json()) as {
      policies: Array<Record<string, unknown>>
    }

    expect(second.status).toBe(201)
    expect(store.paymentPolicies).toHaveLength(1)
    expect(secondBody.policies[0]).toMatchObject({
      paymentPolicy: {
        policyMode: 'operator_grant',
        reason: 'Operator grant for first overnight batch.',
      },
    })
    expect(store.triageEvents.map(event => event.event_type)).toEqual([
      'order_triage.first_batch_payment_policy_applied',
      'order_triage.first_batch_payment_policy_applied',
    ])
    expect(JSON.stringify(secondBody)).not.toContain('settled')
    expect(JSON.stringify(secondBody)).not.toContain('provider payout')
  })

  test('applies first-batch no-payment policy with admin API token', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/payment-policy',
      {
        body: JSON.stringify({
          softwareOrderIds: ['software_order_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
      null,
      true,
    )
    const body = (await response.json()) as {
      policies: Array<Record<string, unknown>>
    }

    expect(response.status).toBe(201)
    expect(body.policies[0]).toMatchObject({
      paymentPolicy: {
        appliedByUserId: 'github:14167547',
        policyMode: 'public_beta_free',
        softwareOrderId: 'software_order_site',
      },
    })
  })

  test('rejects no-payment policy copy that implies settlement', async () => {
    const response = await runRoute(
      new OperatorOrderTriageDbStore(),
      '/api/operator/orders/triage/first-batch/payment-policy',
      {
        body: JSON.stringify({
          customerSafeSummary:
            'This launch has been settled over Lightning for the customer.',
          softwareOrderIds: ['software_order_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(400)
    expect(body).toMatchObject({ error: 'payment_policy_unsafe' })
  })

  test('keeps held first-batch records out of assignment creation', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/assign',
      {
        body: JSON.stringify({
          softwareOrderIds: ['software_order_legal'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      results: Array<Record<string, unknown>>
      summary: Record<string, unknown>
    }

    expect(response.status).toBe(201)
    expect(body.summary).toMatchObject({ held: 1, total: 1 })
    expect(body.results[0]).toMatchObject({
      decision: 'held',
      holdReason: 'Legal-sensitive request.',
      softwareOrderId: 'software_order_legal',
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.triageEvents).toEqual([
      expect.objectContaining({
        assignment_id: null,
        event_type: 'order_triage.first_batch_held',
        software_order_id: 'software_order_legal',
      }),
    ])
  })

  test('treats duplicate active assignment attempts as already assigned', async () => {
    const store = new OperatorOrderTriageDbStore()
    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/assign',
      {
        body: JSON.stringify({
          softwareOrderIds: ['software_order_site'],
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )
    const body = (await response.json()) as {
      results: Array<Record<string, unknown>>
      summary: Record<string, unknown>
    }

    expect(response.status).toBe(201)
    expect(body.summary).toMatchObject({
      alreadyAssigned: 1,
      total: 1,
    })
    expect(body.results[0]).toMatchObject({
      assignmentId: 'assignment_1',
      decision: 'already_assigned',
      softwareOrderId: 'software_order_site',
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.triageEvents).toEqual([
      expect.objectContaining({
        assignment_id: 'assignment_1',
        event_type: 'order_triage.first_batch_already_assigned',
        software_order_id: 'software_order_site',
      }),
    ])
  })

  test('monitors first-batch orders from one redacted operator surface', async () => {
    const store = new OperatorOrderTriageDbStore()
    store.assignments.push({
      agent_id: 'agent_adjutant',
      archived_at: null,
      assigned_by_user_id: 'admin_user',
      assignment_kind: 'site_generation',
      blocked_at: null,
      commit_sha: 'abc123',
      completed_at: null,
      created_at: '2026-06-05T04:00:00.000Z',
      current_run_id: 'agent_run_stale',
      goal_id: 'agent_goal_stale',
      id: 'assignment_stale',
      objective: 'Build the fresh site.',
      site_id: null,
      software_order_id: 'software_order_fresh_site',
      status: 'running',
      task_spec_path: 'docs/autopilot-tasks/fresh.md',
      team_id: 'team_openagents_core',
      updated_at: '2026-06-05T04:00:00.000Z',
      visibility: 'team',
    })
    store.runs.push({
      completed_at: null,
      created_at: '2026-06-05T04:00:00.000Z',
      failed_at: null,
      id: 'agent_run_stale',
      provider_account_ref: 'provider-account_ref_1',
      repository_owner: 'ben',
      repository_ref: 'main',
      repository_repo: 'otec-site',
      started_at: '2026-06-05T04:00:00.000Z',
      status: 'running',
      updated_at: '2026-06-05T04:00:00.000Z',
    })
    store.runEvents.push({
      created_at: '2026-06-05T04:00:00.000Z',
      run_id: 'agent_run_stale',
      sequence: 1,
      status: 'running',
      summary: 'Runner started.',
      type: 'runner.started',
    })
    store.leases.push({
      account_health: 'healthy',
      account_label: 'account 2',
      account_status: 'connected',
      assignment_id: 'assignment_stale',
      cooldown_until: null,
      expires_at: '2026-06-05T06:00:00.000Z',
      failure_class: null,
      last_touched_at: '2026-06-05T04:10:00.000Z',
      lease_ref: 'provider-account-lease_ref_1',
      low_credit_flag: 0,
      order_id: 'software_order_fresh_site',
      provider_account_ref: 'provider-account_ref_1',
      requested_action: 'customer_order_fulfillment',
      reauth_required_reason: null,
      run_id: 'agent_run_stale',
      started_at: '2026-06-05T04:00:00.000Z',
      status: 'active',
    })
    store.failoverReceipts.push({
      account_state_action: 'cooldown',
      assignment_id: 'assignment_stale',
      cooldown_until: '2026-06-05T06:00:00.000Z',
      created_at: '2026-06-05T04:20:00.000Z',
      customer_safe_status: 'running',
      customer_safe_summary: 'OpenAgents is retrying the order.',
      failure_class: 'rate_limited',
      id: 'provider_account_failover_receipt_1',
      next_lease_ref: 'provider-account-lease_ref_1',
      next_provider_account_ref: 'provider-account_ref_1',
      operator_summary: 'Provider account failover retrying after rate_limited.',
      order_id: 'software_order_fresh_site',
      outcome: 'retrying',
      policy_version: 'provider-account-lease-policy:v1',
      previous_lease_ref: 'provider-account-lease_ref_failed',
      previous_provider_account_ref: 'provider-account_ref_failed',
      run_id: 'agent_run_stale',
    })
    store.paymentPolicies.push({
      applied_by_user_id: 'admin_user',
      archived_at: null,
      assignment_id: 'assignment_stale',
      created_at: '2026-06-05T04:00:00.000Z',
      customer_safe_summary:
        'This first-batch OpenAgents run is covered by a public beta free slice.',
      id: 'first_batch_payment_policy_1',
      policy_mode: 'public_beta_free',
      reason: 'Public beta free slice.',
      site_id: null,
      software_order_id: 'software_order_fresh_site',
      updated_at: '2026-06-05T04:00:00.000Z',
    })

    const response = await runRoute(
      store,
      '/api/operator/orders/triage/first-batch/monitor',
    )
    const body = (await response.json()) as {
      monitor: Array<Record<string, unknown>>
      summary: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(body.summary.total).toBe(3)
    expect(body.summary.blocked).toBe(1)
    expect(body.summary.queued).toBe(1)
    expect(body.summary.held).toBe(1)

    const stale = body.monitor.find(
      item =>
        (item.order as { id: string }).id === 'software_order_fresh_site',
    ) as Record<string, unknown>

    expect(stale).toMatchObject({
      activeLease: {
        leaseRef: 'provider-account-lease_ref_1',
        providerAccountRef: 'provider-account_ref_1',
      },
      assignment: {
        id: 'assignment_stale',
        taskSpecPath: 'docs/autopilot-tasks/fresh.md',
      },
      callbackStatus: 'stale',
      latestFailover: {
        failureClass: 'rate_limited',
        outcome: 'retrying',
      },
      paymentPolicy: {
        id: 'first_batch_payment_policy_1',
        required: true,
        status: 'satisfied',
      },
      state: 'blocked',
    })
    expect(stale.currentBlocker).toContain('Runner callback is stale')

    const assigned = body.monitor.find(
      item => (item.order as { id: string }).id === 'software_order_site',
    )
    expect(assigned).toMatchObject({
      assignment: { id: 'assignment_1' },
      currentBlocker: 'First-batch no-payment policy has not been applied.',
      paymentPolicy: {
        required: true,
        status: 'missing',
      },
      state: 'queued',
    })

    const held = body.monitor.find(
      item => (item.order as { id: string }).id === 'software_order_legal',
    )
    expect(held).toMatchObject({
      currentBlocker: 'Legal-sensitive request.',
      state: 'held',
    })

    const serialized = JSON.stringify(body)
    expect(serialized).not.toContain('auth_grant')
    expect(serialized).not.toContain('secret_ref')
    expect(serialized).not.toContain('callback_token')
    expect(serialized).not.toContain('raw_provider')
  })

  test('does not make smoke orders launch eligible without an override path', async () => {
    const response = await runRoute(
      new OperatorOrderTriageDbStore(),
      '/api/operator/orders/software_order_smoke/triage',
      {
        body: JSON.stringify({
          classification: 'smoke_or_test',
          operatorPriority: 1,
          firstBatchEligible: true,
          holdReason: 'Smoke order.',
          nextAction: 'Hold until explicitly promoted.',
          customerSafeStatus: 'held',
          customerSafeSummary:
            'This request is held while OpenAgents reviews it.',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'PUT',
      },
    )
    const body = (await response.json()) as {
      record: Record<string, unknown>
    }

    expect(response.status).toBe(200)
    expect(body.record).toMatchObject({
      classification: 'smoke_or_test',
      firstBatchEligible: true,
      overnightLaunchEligible: false,
    })
  })
})
