import type { AuthKvStore } from './auth/auth-kv'
import { Effect } from 'effect'
import { describe, expect, test, vi } from 'vitest'

import type { OpenAgentsWorkerConfigEnv } from './config'
import { makeOperatorAdjutantRoutes } from './operator-adjutant-routes'

type TestEnv = OpenAgentsWorkerConfigEnv &
  Readonly<{
    ADJUTANT_ENRICHMENT_QUEUE: Queue
    AUTH_KV?: AuthKvStore | undefined
    OPENAGENTS_DB: D1Database
  }>

type TestSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type StoredSoftwareOrder = Readonly<{
  agent_started_at: string | null
  archived_at: string | null
  created_at: string
  current_run_id: string | null
  id: string
  repository_default_branch: string | null
  repository_full_name: string | null
  repository_html_url: string | null
  repository_name: string | null
  repository_owner: string | null
  repository_private: number | null
  repository_provider: 'github' | null
  request: string
  status: string
  updated_at: string
  visibility: 'public'
}>

type StoredSiteProject = Readonly<{
  access_mode: string
  active_deployment_id: string | null
  active_version_id: string | null
  archived_at: string | null
  id: string
  software_order_id: string | null
  source_repository_name: string | null
  source_repository_owner: string | null
  source_repository_provider: 'github' | null
  source_repository_ref: string | null
  slug: string
  status: string
  title: string
  visibility: string
}>

type StoredSiteVersion = Readonly<{
  build_command: string | null
  build_status: string
  created_at: string
  created_by_run_id: string | null
  id: string
  rejected_at: string | null
  saved_at: string | null
  site_id: string
  source_commit_sha: string | null
  source_kind: string
  worker_module_r2_key: string | null
}>

type StoredSiteDeployment = Readonly<{
  activated_at: string | null
  disabled_at: string | null
  external_deployment_id: string | null
  id: string
  rolled_back_at: string | null
  runtime_kind: string
  site_id: string
  status: string
  updated_at: string
  url: string
  version_id: string
}>

type StoredAgentRun = Readonly<{
  archived_at: string | null
  backend: string
  created_at: string
  event_cursor: number
  external_run_id: string | null
  id: string
  runtime: string
  status: string
  updated_at: string
}>

type StoredAdjutantAssignment = Readonly<{
  agent_id: string
  archived_at: string | null
  assigned_by_user_id: string | null
  assignment_kind:
    | 'site_generation'
    | 'site_adjustment'
    | 'site_review'
    | 'site_deployment'
    | 'general_order_fulfillment'
  blocked_at: string | null
  commit_sha: string | null
  completed_at: string | null
  created_at: string
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  project_id: string | null
  site_id: string | null
  software_order_id: string | null
  status:
    | 'draft'
    | 'preflight_pending'
    | 'blocked'
    | 'queued'
    | 'running'
    | 'review_needed'
    | 'deployed'
    | 'delivered'
    | 'complete'
    | 'canceled'
  task_spec_path: string | null
  team_id: string | null
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredAdjutantAssignmentEvent = Readonly<{
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
  visibility: 'private' | 'team' | 'public'
}>

type StoredAdjutantAdjustment = Readonly<{
  archived_at: string | null
  assignment_id: string
  completed_at: string | null
  continuation_mode: 'follow_up_turn' | 'new_goal_run' | null
  continuation_run_id: string | null
  created_at: string
  goal_id: string | null
  id: string
  instruction: string
  requested_by_user_id: string | null
  resulting_version_id: string | null
  site_id: string
  software_order_id: string | null
  source_run_id: string | null
  status:
    | 'requested'
    | 'queued'
    | 'running'
    | 'review_needed'
    | 'completed'
    | 'rejected'
    | 'canceled'
    | 'failed'
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>

type StoredResearchBrief = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  claims_needing_review_json: string
  created_at: string
  created_by_user_id: string | null
  enrichment_run_id: string | null
  grounded_facts_json: string
  id: string
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  source_cards_json: string
  status: 'draft' | 'needs_review' | 'approved' | 'rejected' | 'stale'
  suggested_sections_json: string
  summary: string
  unknowns_json: string
  updated_at: string
}>

type StoredExaEnrichmentRun = Readonly<{
  approved_source_count: number
  archived_at: string | null
  assignment_id: string
  cache_hit_count: number
  completed_at: string | null
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  id: string
  plan_id: string
  request_budget: number
  request_count: number
  site_id: string | null
  software_order_id: string | null
  source_count: number
  started_at: string | null
  status: string
  subject: string
  updated_at: string
}>

type StoredExaEnrichmentBudgetEvent = Readonly<{
  assignment_id: string
  created_at: string
  day_key: string
  id: string
  reason: string
  request_units: number
  run_id: string | null
}>

type StoredExaEnrichmentCacheEntry = Readonly<{
  archived_at: string | null
  cache_key: string
  cost_dollars: number | null
  created_at: string
  expires_at: string
  freshness_max_age_hours: number
  id: string
  result_count: number
  results_json: string
  search_type: string
  source_category: string
}>

type StoredExaEnrichmentQuery = Readonly<{
  assignment_id: string
  cost_dollars: number | null
  created_at: string
  error_code: string | null
  error_summary: string | null
  freshness_max_age_hours: number
  id: string
  latency_ms: number | null
  query_hash: string
  query_text: string
  result_count: number
  run_id: string
  search_type: string
  source_category: string
  status: string
  updated_at: string
}>

type StoredExaEnrichmentSource = Readonly<{
  approved_at: string | null
  assignment_id: string
  created_at: string
  domain: string
  exa_request_id: string | null
  highlight_text: string | null
  id: string
  public_safe: number
  published_date: string | null
  query_id: string | null
  rejected_at: string | null
  rejected_reason: string | null
  review_status: string
  run_id: string
  search_type: string | null
  selected_text_hash: string | null
  site_id: string | null
  software_order_id: string | null
  source_category: string
  title: string
  updated_at: string
  url: string
}>

type StoredAdjutantPublicSourceRef = Readonly<{
  approved_at: string | null
  archived_at: string | null
  assignment_id: string
  created_at: string
  id: string
  kind: string
  label: string | null
  normalized_domain: string
  proposed_by_user_id: string | null
  public_safe: number
  rejected_at: string | null
  review_reason: string | null
  reviewed_by_user_id: string | null
  site_id: string | null
  software_order_id: string | null
  status: string
  updated_at: string
  url: string
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

type StoredOrderTriageRecord = Readonly<{
  archived_at: string | null
  classification: 'runnable_site' | 'runnable_general_autopilot' | string
  first_batch_eligible: number
  id: string
  software_order_id: string
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

type StoredResearchPolicy = Readonly<{
  actor_user_id: string | null
  archived_at: string | null
  assignment_id: string
  created_at: string
  customer_safe_summary: string
  policy_mode:
    | 'research_required'
    | 'research_optional'
    | 'research_not_applicable'
    | 'research_bypassed_by_operator'
  reason: string
  source_authority_ref: string | null
  updated_at: string
}>

type StoredEnrichmentJob = Readonly<{
  archived_at: string | null
  assignment_id: string
  completed_at: string | null
  created_at: string
  enrichment_run_id: string | null
  error_code: string | null
  error_summary: string | null
  id: string
  refresh: number
  requested_by_user_id: string | null
  request_json: string | null
  started_at: string | null
  status: 'queued' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'canceled'
  trigger_kind: 'research_required' | 'operator_requested' | 'operator_refresh'
  updated_at: string
}>

type StoredTaskPacketFreshness = Readonly<{
  actor_user_id: string | null
  archived_at: string | null
  assignment_id: string
  commit_sha: string | null
  created_at: string
  customer_safe_summary: string | null
  kept_at: string | null
  operator_keep_reason: string | null
  research_brief_approved_at: string | null
  research_brief_id: string | null
  source_card_count: number
  stale_at: string | null
  status: 'current' | 'stale' | 'kept_current'
  task_spec_path: string
  updated_at: string
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
  status:
    | 'active'
    | 'paused'
    | 'blocked'
    | 'usage_limited'
    | 'budget_limited'
    | 'complete'
  team_id: string | null
  time_used_seconds: number
  token_budget: number | null
  tokens_used: number
  updated_at: string
  user_id: string | null
  visibility: 'private' | 'team' | 'public'
}>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

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

const addFirstBatchPaymentPolicy = (
  store: OperatorAdjutantDbStore,
  assignmentId: string,
): void => {
  store.paymentPolicies.push({
    applied_by_user_id: 'github:operator',
    archived_at: null,
    assignment_id: assignmentId,
    created_at: '2026-06-05T00:00:00.000Z',
    customer_safe_summary:
      'This first-batch OpenAgents run is covered by a public beta free slice.',
    id: `first_batch_payment_policy_${assignmentId}`,
    policy_mode: 'public_beta_free',
    reason: 'Public beta free slice for first submitted-order batch.',
    site_id: 'site_project_otec',
    software_order_id: 'software_order_otec',
    updated_at: '2026-06-05T00:00:00.000Z',
  })
}

class OperatorAdjutantDbStore {
  private adjustmentCounter = 0
  private assignmentCounter = 0
  private eventCounter = 0

  adjustments: Array<StoredAdjutantAdjustment> = []
  assignments: Array<StoredAdjutantAssignment> = []
  budgetEvents: Array<StoredExaEnrichmentBudgetEvent> = []
  cacheEntries: Array<StoredExaEnrichmentCacheEntry> = []
  enrichmentQueries: Array<StoredExaEnrichmentQuery> = []
  enrichmentRuns: Array<StoredExaEnrichmentRun> = []
  enrichmentSources: Array<StoredExaEnrichmentSource> = []
  publicSourceRefs: Array<StoredAdjutantPublicSourceRef> = []
  researchBriefs: Array<StoredResearchBrief> = []
  deployments: Array<StoredSiteDeployment> = [
    {
      activated_at: '2026-06-05T00:00:00.000Z',
      disabled_at: null,
      external_deployment_id: null,
      id: 'site_deployment_otec',
      rolled_back_at: null,
      runtime_kind: 'workers_for_platforms',
      site_id: 'site_project_otec',
      status: 'active',
      updated_at: '2026-06-05T00:00:00.000Z',
      url: 'https://sites.openagents.com/otec',
      version_id: 'site_version_otec',
    },
  ]
  events: Array<StoredAdjutantAssignmentEvent> = []
  goals: Array<StoredAgentGoal> = []
  runs: Array<StoredAgentRun> = [
    {
      archived_at: null,
      backend: 'shc_vm',
      created_at: '2026-06-05T00:00:00.000Z',
      event_cursor: 7,
      external_run_id: 'external_adjutant_run_1',
      id: 'agent_run_adjutant_1',
      runtime: 'codex',
      status: 'completed',
      updated_at: '2026-06-05T00:10:00.000Z',
    },
  ]
  siteEvents: Array<StoredSiteEvent> = []
  usageReceipts: Array<StoredUsageReceipt> = []
  paymentPolicies: Array<StoredPaymentPolicy> = []
  researchPolicies: Array<StoredResearchPolicy> = []
  enrichmentJobs: Array<StoredEnrichmentJob> = []
  taskPacketFreshness: Array<StoredTaskPacketFreshness> = []
  queueMessages: Array<unknown> = []
  triageRecords: Array<StoredOrderTriageRecord> = [
    {
      archived_at: null,
      classification: 'runnable_site',
      first_batch_eligible: 1,
      id: 'triage_otec',
      software_order_id: 'software_order_otec',
    },
  ]
  softwareOrders: Array<StoredSoftwareOrder> = [
    {
      agent_started_at: null,
      archived_at: null,
      created_at: '2026-06-05T00:00:00.000Z',
      current_run_id: null,
      id: 'software_order_otec',
      repository_default_branch: 'main',
      repository_full_name: 'OpenAgentsInc/autopilot-omega',
      repository_html_url: 'https://github.com/OpenAgentsInc/autopilot-omega',
      repository_name: 'autopilot-omega',
      repository_owner: 'OpenAgentsInc',
      repository_private: 0,
      repository_provider: 'github',
      request: 'Build the OTEC floating datacenter Site.',
      status: 'submitted',
      updated_at: '2026-06-05T00:00:00.000Z',
      visibility: 'public',
    },
  ]
  sites: Array<StoredSiteProject> = [
    {
      access_mode: 'public',
      active_deployment_id: 'site_deployment_otec',
      active_version_id: 'site_version_otec',
      archived_at: null,
      id: 'site_project_otec',
      software_order_id: 'software_order_otec',
      source_repository_name: 'autopilot-omega',
      source_repository_owner: 'OpenAgentsInc',
      source_repository_provider: 'github',
      source_repository_ref: 'main',
      slug: 'otec',
      status: 'approved',
      title: 'OTEC Floating Datacenter',
      visibility: 'public',
    },
  ]
  versions: Array<StoredSiteVersion> = [
    {
      build_command: 'bun run build',
      build_status: 'saved',
      created_at: '2026-06-05T00:08:00.000Z',
      created_by_run_id: 'agent_run_adjutant_1',
      id: 'site_version_otec',
      rejected_at: null,
      saved_at: '2026-06-05T00:08:00.000Z',
      site_id: 'site_project_otec',
      source_commit_sha: 'fa1fdfbb',
      source_kind: 'autopilot_generated',
      worker_module_r2_key:
        'sites/site_project_otec/versions/site_version_otec/worker.mjs',
    },
  ]

  makeAssignmentId(): string {
    this.assignmentCounter += 1

    return `adjutant_assignment_${this.assignmentCounter}`
  }

  makeAdjustmentId(): string {
    this.adjustmentCounter += 1

    return `adjutant_adjustment_${this.adjustmentCounter}`
  }

  makeEventId(): string {
    this.eventCounter += 1

    return `adjutant_assignment_event_${this.eventCounter}`
  }
}

class OperatorAdjutantStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: OperatorAdjutantDbStore,
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

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM order_triage_records')) {
      const [softwareOrderId] = this.values
      const row =
        this.store.triageRecords.find(
          record =>
            record.software_order_id === softwareOrderId &&
            record.first_batch_eligible === 1 &&
            (record.classification === 'runnable_site' ||
              record.classification === 'runnable_general_autopilot') &&
            record.archived_at === null,
        ) ?? null

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [idempotencyKey] = this.values
      const row = this.store.usageReceipts.find(
        receipt => receipt.idempotency_key === idempotencyKey,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM software_orders')) {
      const [softwareOrderId] = this.values
      const row = this.store.softwareOrders.find(
        order => order.id === softwareOrderId && order.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM site_projects')) {
      const [siteId] = this.values
      const row = this.store.sites.find(
        site => site.id === siteId && site.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM agent_runs')) {
      const [runId] = this.values
      const row = this.store.runs.find(
        run => run.id === runId && run.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM agent_goals') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [goalId] = this.values
      const row = this.store.goals.find(goal => goal.id === goalId)

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM agent_goals')) {
      const [agentId, userId, teamId, projectId] = this.values
      const row = this.store.goals.find(
        goal =>
          goal.agent_id === agentId &&
          (goal.user_id ?? '') === (userId ?? '') &&
          (goal.team_id ?? '') === (teamId ?? '') &&
          (goal.project_id ?? '') === (projectId ?? '') &&
          goal.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [assignmentId] = this.values
      const row = this.store.assignments.find(
        assignment =>
          assignment.id === assignmentId && assignment.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_assignment_research_policies')) {
      const [assignmentId] = this.values
      const row =
        this.store.researchPolicies.find(
          policy =>
            policy.assignment_id === assignmentId &&
            policy.archived_at === null,
        ) ?? null

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_enrichment_jobs')) {
      const [assignmentOrJobId] = this.values
      const rows = this.store.enrichmentJobs
        .filter(job => job.archived_at === null)
        .filter(job =>
          this.query.includes('WHERE id = ?')
            ? job.id === assignmentOrJobId
            : job.assignment_id === assignmentOrJobId,
        )
        .filter(job =>
          this.query.includes("status IN ('queued', 'running')")
            ? job.status === 'queued' || job.status === 'running'
            : true,
        )
        .sort((left, right) => right.updated_at.localeCompare(left.updated_at))
      const row = rows[0] ?? null

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_task_packet_freshness')) {
      const [assignmentId] = this.values
      const row =
        this.store.taskPacketFreshness.find(
          record =>
            record.assignment_id === assignmentId &&
            record.archived_at === null,
        ) ?? null

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM adjutant_adjustment_requests') &&
      this.query.includes('WHERE id = ?')
    ) {
      const [adjustmentId] = this.values
      const row = this.store.adjustments.find(
        adjustment =>
          adjustment.id === adjustmentId && adjustment.archived_at === null,
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_research_briefs')) {
      const [assignmentId, requestedStatus] = this.values
      const row = this.store.researchBriefs.find(
        brief =>
          brief.assignment_id === assignmentId &&
          brief.archived_at === null &&
          (requestedStatus === null || brief.status === requestedStatus),
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM exa_enrichment_budget_events')) {
      const [assignmentId, dayKey] = this.values
      const events = this.store.budgetEvents.filter(
        event => event.day_key === dayKey,
      )
      const assignmentUnits = events
        .filter(event => event.assignment_id === assignmentId)
        .reduce((total, event) => total + event.request_units, 0)
      const dayUnits = events.reduce(
        (total, event) => total + event.request_units,
        0,
      )

      return Promise.resolve({
        assignment_units: assignmentUnits,
        day_units: dayUnits,
      } as T)
    }

    if (this.query.includes('FROM exa_enrichment_cache_entries')) {
      const [cacheKey, freshnessMaxAgeHours, nowIso] = this.values
      const row =
        this.store.cacheEntries
          .filter(
            entry =>
              entry.cache_key === cacheKey &&
              entry.freshness_max_age_hours === Number(freshnessMaxAgeHours) &&
              entry.expires_at > String(nowIso) &&
              entry.archived_at === null,
          )
          .sort((left, right) =>
            right.expires_at.localeCompare(left.expires_at),
          )[0] ?? null

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (this.query.includes('FROM exa_enrichment_runs')) {
      const [assignmentId] = this.values
      const row = this.store.enrichmentRuns
        .filter(
          run => run.assignment_id === assignmentId && run.archived_at === null,
        )
        .sort((left, right) =>
          right.created_at.localeCompare(left.created_at),
        )[0]

      return Promise.resolve((row as T | undefined) ?? null)
    }

    if (
      this.query.includes('FROM adjutant_assignments') &&
      this.query.includes("status NOT IN ('complete', 'canceled')")
    ) {
      const [softwareOrderId, siteId] = this.values
      const row = this.store.assignments.find(
        assignment =>
          assignment.archived_at === null &&
          assignment.status !== 'complete' &&
          assignment.status !== 'canceled' &&
          ((softwareOrderId !== null &&
            assignment.software_order_id === softwareOrderId) ||
            (siteId !== null && assignment.site_id === siteId)),
      )

      return Promise.resolve((row as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
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
        assignment_kind:
          assignmentKind as StoredAdjutantAssignment['assignment_kind'],
        blocked_at: null,
        commit_sha: typeof commitSha === 'string' ? commitSha : null,
        completed_at: null,
        created_at: String(createdAt),
        current_run_id: typeof currentRunId === 'string' ? currentRunId : null,
        goal_id: typeof goalId === 'string' ? goalId : null,
        id:
          String(id) === 'adjutant_assignment_static'
            ? this.store.makeAssignmentId()
            : String(id),
        objective: String(objective),
        project_id: typeof projectId === 'string' ? projectId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        status: status as StoredAdjutantAssignment['status'],
        task_spec_path: typeof taskSpecPath === 'string' ? taskSpecPath : null,
        team_id: typeof teamId === 'string' ? teamId : null,
        updated_at: String(updatedAt),
        visibility: visibility as StoredAdjutantAssignment['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_adjustment_requests')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        goalId,
        requestedByUserId,
        instruction,
        sourceRunId,
        visibility,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.adjustments.push({
        archived_at: null,
        assignment_id: String(assignmentId),
        completed_at: null,
        continuation_mode: null,
        continuation_run_id: null,
        created_at: String(createdAt),
        goal_id: typeof goalId === 'string' ? goalId : null,
        id:
          String(id) === 'adjutant_adjustment_static'
            ? this.store.makeAdjustmentId()
            : String(id),
        instruction: String(instruction),
        requested_by_user_id:
          typeof requestedByUserId === 'string' ? requestedByUserId : null,
        resulting_version_id: null,
        site_id: String(siteId),
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        source_run_id: typeof sourceRunId === 'string' ? sourceRunId : null,
        status: 'requested',
        updated_at: String(updatedAt),
        visibility: visibility as StoredAdjutantAdjustment['visibility'],
      })

      return Promise.resolve(makeResult<T>())
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

      this.store.events.push({
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        event_type: String(eventType),
        goal_id: typeof goalId === 'string' ? goalId : null,
        id:
          String(id) === 'adjutant_assignment_event_static'
            ? this.store.makeEventId()
            : String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        run_id: typeof runId === 'string' ? runId : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        summary: String(summary),
        visibility: visibility as StoredAdjutantAssignmentEvent['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    if (
      this.query.includes('INSERT INTO adjutant_assignment_research_policies')
    ) {
      const [
        assignmentId,
        policyMode,
        reason,
        customerSafeSummary,
        actorUserId,
        sourceAuthorityRef,
        createdAt,
        updatedAt,
      ] = this.values
      const existing = this.store.researchPolicies.find(
        policy => policy.assignment_id === assignmentId,
      )
      const next: StoredResearchPolicy = {
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        archived_at: null,
        assignment_id: String(assignmentId),
        created_at:
          existing?.created_at ??
          (typeof createdAt === 'string' ? createdAt : ''),
        customer_safe_summary: String(customerSafeSummary),
        policy_mode: policyMode as StoredResearchPolicy['policy_mode'],
        reason: String(reason),
        source_authority_ref:
          typeof sourceAuthorityRef === 'string' ? sourceAuthorityRef : null,
        updated_at: String(updatedAt),
      }

      this.store.researchPolicies =
        existing === undefined
          ? [...this.store.researchPolicies, next]
          : this.store.researchPolicies.map(policy =>
              policy.assignment_id === assignmentId ? next : policy,
            )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_enrichment_jobs')) {
      const [
        id,
        assignmentId,
        enrichmentRunId,
        triggerKind,
        refresh,
        requestedByUserId,
        requestJson,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.enrichmentJobs.push({
        archived_at: null,
        assignment_id: String(assignmentId),
        completed_at: null,
        created_at: String(createdAt),
        enrichment_run_id:
          typeof enrichmentRunId === 'string' ? enrichmentRunId : null,
        error_code: null,
        error_summary: null,
        id: String(id),
        refresh: Number(refresh),
        requested_by_user_id:
          typeof requestedByUserId === 'string' ? requestedByUserId : null,
        request_json: typeof requestJson === 'string' ? requestJson : null,
        started_at: null,
        status: 'queued',
        trigger_kind: triggerKind as StoredEnrichmentJob['trigger_kind'],
        updated_at: String(updatedAt),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_runs')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        planId,
        subject,
        status,
        requestBudget,
        startedAt,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.enrichmentRuns.push({
        approved_source_count: 0,
        archived_at: null,
        assignment_id: String(assignmentId),
        cache_hit_count: 0,
        completed_at: null,
        cost_dollars: null,
        created_at: String(createdAt),
        error_code: null,
        error_summary: null,
        id: String(id),
        plan_id: String(planId),
        request_budget: Number(requestBudget),
        request_count: 0,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        source_count: 0,
        started_at: typeof startedAt === 'string' ? startedAt : null,
        status: String(status),
        subject: String(subject),
        updated_at: String(updatedAt),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_budget_events')) {
      const [id, assignmentId, runId, dayKey, requestUnits, reason, createdAt] =
        this.values

      this.store.budgetEvents.push({
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        day_key: String(dayKey),
        id: String(id),
        reason: String(reason),
        request_units: Number(requestUnits),
        run_id: typeof runId === 'string' ? runId : null,
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE exa_enrichment_runs')) {
      if (this.query.includes('request_count = request_count + 1')) {
        const [cacheHitIncrement, costDollars, updatedAt, runId] = this.values

        this.store.enrichmentRuns = this.store.enrichmentRuns.map(run =>
          run.id === runId
            ? {
                ...run,
                cache_hit_count:
                  run.cache_hit_count + Number(cacheHitIncrement ?? 0),
                cost_dollars:
                  (run.cost_dollars ?? 0) + Number(costDollars ?? 0),
                request_count: run.request_count + 1,
                updated_at: String(updatedAt),
              }
            : run,
        )

        return Promise.resolve(makeResult<T>())
      }

      if (this.query.includes('source_count = (')) {
        const [updatedAt, runId] = this.values
        const sourceCount = this.store.enrichmentSources.filter(
          source => source.run_id === runId,
        ).length
        const approvedSourceCount = this.store.enrichmentSources.filter(
          source =>
            source.run_id === runId &&
            source.public_safe === 1 &&
            (source.review_status === 'approved' ||
              source.review_status === 'public_safe'),
        ).length

        this.store.enrichmentRuns = this.store.enrichmentRuns.map(run =>
          run.id === runId
            ? {
                ...run,
                approved_source_count: approvedSourceCount,
                source_count: sourceCount,
                updated_at: String(updatedAt),
              }
            : run,
        )

        return Promise.resolve(makeResult<T>())
      }

      const [status, errorCode, errorSummary, completedAt, updatedAt, runId] =
        this.values

      this.store.enrichmentRuns = this.store.enrichmentRuns.map(run =>
        run.id === runId
          ? {
              ...run,
              completed_at:
                typeof completedAt === 'string' ? completedAt : null,
              error_code: typeof errorCode === 'string' ? errorCode : null,
              error_summary:
                typeof errorSummary === 'string' ? errorSummary : null,
              status: String(status),
              updated_at: String(updatedAt),
            }
          : run,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_enrichments')) {
      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_queries')) {
      const [
        id,
        runId,
        assignmentId,
        queryHash,
        queryText,
        sourceCategory,
        searchType,
        freshnessMaxAgeHours,
        status,
        resultCount,
        latencyMs,
        costDollars,
        errorCode,
        errorSummary,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.enrichmentQueries.push({
        assignment_id: String(assignmentId),
        cost_dollars: typeof costDollars === 'number' ? costDollars : null,
        created_at: String(createdAt),
        error_code: typeof errorCode === 'string' ? errorCode : null,
        error_summary: typeof errorSummary === 'string' ? errorSummary : null,
        freshness_max_age_hours: Number(freshnessMaxAgeHours),
        id: String(id),
        latency_ms: typeof latencyMs === 'number' ? latencyMs : null,
        query_hash: String(queryHash),
        query_text: String(queryText),
        result_count: Number(resultCount),
        run_id: String(runId),
        search_type: String(searchType),
        source_category: String(sourceCategory),
        status: String(status),
        updated_at: String(updatedAt),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_sources')) {
      const [
        id,
        runId,
        queryId,
        assignmentId,
        softwareOrderId,
        siteId,
        sourceCategory,
        reviewStatus,
        title,
        url,
        domain,
        publishedDate,
        highlightText,
        selectedTextHash,
        exaRequestId,
        searchType,
        publicSafe,
        rejectedReason,
        approvedAt,
        rejectedAt,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.enrichmentSources.push({
        approved_at: typeof approvedAt === 'string' ? approvedAt : null,
        assignment_id: String(assignmentId),
        created_at: String(createdAt),
        domain: String(domain),
        exa_request_id: typeof exaRequestId === 'string' ? exaRequestId : null,
        highlight_text:
          typeof highlightText === 'string' ? highlightText : null,
        id: String(id),
        public_safe: Number(publicSafe),
        published_date:
          typeof publishedDate === 'string' ? publishedDate : null,
        query_id: typeof queryId === 'string' ? queryId : null,
        rejected_at: typeof rejectedAt === 'string' ? rejectedAt : null,
        rejected_reason:
          typeof rejectedReason === 'string' ? rejectedReason : null,
        review_status: String(reviewStatus),
        run_id: String(runId),
        search_type: typeof searchType === 'string' ? searchType : null,
        selected_text_hash:
          typeof selectedTextHash === 'string' ? selectedTextHash : null,
        site_id: typeof siteId === 'string' ? siteId : null,
        software_order_id:
          typeof softwareOrderId === 'string' ? softwareOrderId : null,
        source_category: String(sourceCategory),
        title: String(title),
        updated_at: String(updatedAt),
        url: String(url),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE exa_enrichment_cache_entries')) {
      const [archivedAt, cacheKey] = this.values

      this.store.cacheEntries = this.store.cacheEntries.map(entry =>
        entry.cache_key === cacheKey && entry.archived_at === null
          ? { ...entry, archived_at: String(archivedAt) }
          : entry,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_cache_entries')) {
      const [
        id,
        cacheKey,
        sourceCategory,
        searchType,
        freshnessMaxAgeHours,
        resultsJson,
        resultCount,
        costDollars,
        createdAt,
        expiresAt,
      ] = this.values

      this.store.cacheEntries.push({
        archived_at: null,
        cache_key: String(cacheKey),
        cost_dollars: typeof costDollars === 'number' ? costDollars : null,
        created_at: String(createdAt),
        expires_at: String(expiresAt),
        freshness_max_age_hours: Number(freshnessMaxAgeHours),
        id: String(id),
        result_count: Number(resultCount),
        results_json: String(resultsJson),
        search_type: String(searchType),
        source_category: String(sourceCategory),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO exa_enrichment_metric_events')) {
      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_research_briefs')) {
      const [
        id,
        assignmentId,
        enrichmentRunId,
        status,
        summary,
        groundedFactsJson,
        suggestedSectionsJson,
        unknownsJson,
        claimsNeedingReviewJson,
        sourceCardsJson,
        createdByUserId,
        approvedAt,
        rejectedAt,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.researchBriefs.push({
        approved_at: typeof approvedAt === 'string' ? approvedAt : null,
        archived_at: null,
        assignment_id: String(assignmentId),
        claims_needing_review_json: String(claimsNeedingReviewJson),
        created_at: String(createdAt),
        created_by_user_id:
          typeof createdByUserId === 'string' ? createdByUserId : null,
        enrichment_run_id:
          typeof enrichmentRunId === 'string' ? enrichmentRunId : null,
        grounded_facts_json: String(groundedFactsJson),
        id: String(id),
        rejected_at: typeof rejectedAt === 'string' ? rejectedAt : null,
        review_reason: null,
        reviewed_by_user_id: null,
        source_cards_json: String(sourceCardsJson),
        status: status as StoredResearchBrief['status'],
        suggested_sections_json: String(suggestedSectionsJson),
        summary: String(summary),
        unknowns_json: String(unknownsJson),
        updated_at: String(updatedAt),
      })

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('INSERT INTO adjutant_task_packet_freshness')) {
      const assignmentId = String(this.values[0])
      const taskSpecPath = String(this.values[1])
      const commitSha =
        typeof this.values[2] === 'string' ? this.values[2] : null
      const existing = this.store.taskPacketFreshness.find(
        record => record.assignment_id === assignmentId,
      )
      const status = this.query.includes("'kept_current'")
        ? 'kept_current'
        : this.query.includes("'stale'")
          ? 'stale'
          : 'current'
      const now = String(this.values[this.values.length - 1])
      const next: StoredTaskPacketFreshness = {
        actor_user_id:
          status === 'kept_current' ? String(this.values[8]) : null,
        archived_at: null,
        assignment_id: assignmentId,
        commit_sha: commitSha,
        created_at: existing?.created_at ?? now,
        customer_safe_summary:
          status === 'kept_current' ? String(this.values[7]) : null,
        kept_at: status === 'kept_current' ? now : null,
        operator_keep_reason:
          status === 'kept_current' ? String(this.values[6]) : null,
        research_brief_approved_at:
          typeof this.values[4] === 'string' ? this.values[4] : null,
        research_brief_id:
          typeof this.values[3] === 'string' ? this.values[3] : null,
        source_card_count: Number(this.values[5] ?? 0),
        stale_at: status === 'stale' ? now : null,
        status,
        task_spec_path: taskSpecPath,
        updated_at: now,
      }

      this.store.taskPacketFreshness =
        existing === undefined
          ? [...this.store.taskPacketFreshness, next]
          : this.store.taskPacketFreshness.map(record =>
              record.assignment_id === assignmentId ? next : record,
            )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE adjutant_adjustment_requests')) {
      const [
        status,
        continuationMode,
        continuationRunId,
        resultingVersionId,
        updatedAt,
        completedAt,
        adjustmentId,
      ] = this.values

      this.store.adjustments = this.store.adjustments.map(adjustment =>
        adjustment.id === adjustmentId
          ? {
              ...adjustment,
              completed_at:
                typeof completedAt === 'string' ? completedAt : null,
              continuation_mode:
                continuationMode === 'follow_up_turn' ||
                continuationMode === 'new_goal_run'
                  ? continuationMode
                  : null,
              continuation_run_id:
                typeof continuationRunId === 'string'
                  ? continuationRunId
                  : null,
              resulting_version_id:
                typeof resultingVersionId === 'string'
                  ? resultingVersionId
                  : null,
              status: status as StoredAdjutantAdjustment['status'],
              updated_at: String(updatedAt),
            }
          : adjustment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE adjutant_assignments')) {
      const [
        goalId,
        currentRunId,
        status,
        taskSpecPath,
        commitSha,
        objective,
        updatedAt,
        completedAt,
        blockedAt,
        assignmentId,
      ] = this.values

      this.store.assignments = this.store.assignments.map(assignment =>
        assignment.id === assignmentId
          ? {
              ...assignment,
              blocked_at: typeof blockedAt === 'string' ? blockedAt : null,
              commit_sha: typeof commitSha === 'string' ? commitSha : null,
              completed_at:
                typeof completedAt === 'string' ? completedAt : null,
              current_run_id:
                typeof currentRunId === 'string' ? currentRunId : null,
              goal_id: typeof goalId === 'string' ? goalId : null,
              objective: String(objective),
              status: status as StoredAdjutantAssignment['status'],
              task_spec_path:
                typeof taskSpecPath === 'string' ? taskSpecPath : null,
              updated_at: String(updatedAt),
            }
          : assignment,
      )

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE site_projects')) {
      const [status, updatedAt, siteId] = this.values

      this.store.sites = this.store.sites.map(site =>
        site.id === siteId
          ? {
              ...site,
              status: String(status),
            }
          : site,
      )
      void updatedAt

      return Promise.resolve(makeResult<T>())
    }

    if (this.query.includes('UPDATE software_orders')) {
      const [runId, status, agentStartedAt, updatedAt, softwareOrderId] =
        this.values

      this.store.softwareOrders = this.store.softwareOrders.map(order =>
        order.id === softwareOrderId
          ? {
              ...order,
              agent_started_at:
                order.agent_started_at ??
                (typeof agentStartedAt === 'string' ? agentStartedAt : null),
              current_run_id: typeof runId === 'string' ? runId : null,
              status: String(status),
              updated_at: String(updatedAt),
            }
          : order,
      )

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
        eventType,
        summary,
        actorUserId,
        actorRunId,
        payloadJson,
        createdAt,
      ] = this.values

      this.store.siteEvents.push({
        actor_run_id: typeof actorRunId === 'string' ? actorRunId : null,
        actor_user_id: typeof actorUserId === 'string' ? actorUserId : null,
        created_at: String(createdAt),
        deployment_id: null,
        id: String(id),
        payload_json: typeof payloadJson === 'string' ? payloadJson : null,
        site_id: String(siteId),
        summary: String(summary),
        type: String(eventType),
        version_id: null,
      })

      return Promise.resolve(makeResult<T>())
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

      return Promise.resolve(makeResult<T>())
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
        visibility: visibility as StoredAgentGoal['visibility'],
      })

      return Promise.resolve(makeResult<T>())
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [assignmentId, limit] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.usageReceipts
            .filter(receipt => receipt.assignment_id === assignmentId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            )
            .slice(0, Number(limit ?? 50)) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM site_versions')) {
      const [siteId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.versions
            .filter(version => version.site_id === siteId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            )
            .slice(0, 20) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM site_deployments')) {
      const [siteId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.deployments
            .filter(deployment => deployment.site_id === siteId)
            .sort((left, right) =>
              right.updated_at.localeCompare(left.updated_at),
            )
            .slice(0, 20) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM adjutant_assignment_events')) {
      const [assignmentId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.events
            .filter(event => event.assignment_id === assignmentId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            )
            .slice(0, 20) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM site_events')) {
      const [siteId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.siteEvents
            .filter(event => event.site_id === siteId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            )
            .slice(0, 20) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM adjutant_adjustment_requests')) {
      const [assignmentId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.adjustments
            .filter(
              adjustment =>
                adjustment.assignment_id === assignmentId &&
                adjustment.archived_at === null,
            )
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            )
            .slice(0, 20) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM exa_enrichment_queries')) {
      const [runId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.enrichmentQueries
            .filter(query => query.run_id === runId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            ) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM exa_enrichment_sources')) {
      const [assignmentId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.enrichmentSources
            .filter(source => source.assignment_id === assignmentId)
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            ) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM adjutant_public_source_refs')) {
      const [assignmentId] = this.values

      return Promise.resolve(
        makeResult<T>(
          this.store.publicSourceRefs
            .filter(
              sourceRef =>
                sourceRef.assignment_id === assignmentId &&
                sourceRef.archived_at === null,
            )
            .sort((left, right) =>
              right.created_at.localeCompare(left.created_at),
            ) as Array<T>,
        ),
      )
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      return Promise.resolve(
        makeResult<T>(
          this.store.assignments
            .filter(assignment => assignment.archived_at === null)
            .slice(0, Number(this.values[0] ?? 100)) as Array<T>,
        ),
      )
    }

    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

const operatorAdjutantDb = (store: OperatorAdjutantDbStore): D1Database => ({
  batch: async <T = unknown>(
    statements: D1PreparedStatement[],
  ): Promise<Array<D1Result<T>>> =>
    Promise.all(statements.map(statement => statement.run<T>())),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new OperatorAdjutantStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

type TestAutopilotPreflight = Parameters<
  typeof makeOperatorAdjutantRoutes<TestSession, TestEnv>
>[0]['buildOperatorAutopilotPreflightPayload']
type TestTaskPacketRefValidator = Parameters<
  typeof makeOperatorAdjutantRoutes<TestSession, TestEnv>
>[0]['validateAdjutantTaskPacketRef']
type TestAutopilotLaunch = Parameters<
  typeof makeOperatorAdjutantRoutes<TestSession, TestEnv>
>[0]['launchUserAutopilotMission']
type TestAutopilotContinuation = Parameters<
  typeof makeOperatorAdjutantRoutes<TestSession, TestEnv>
>[0]['continueUserAutopilotRun']

const defaultAutopilotPreflight: TestAutopilotPreflight = async () => ({
  checks: [
    {
      message: 'Required Autopilot tables are present.',
      name: 'database_migrations',
      status: 'ok',
    },
    {
      message: 'Selected team project and agent metadata are ready.',
      name: 'team_project_agent',
      status: 'ok',
    },
    {
      message: 'A connected healthy ChatGPT/Codex account is available.',
      name: 'provider_account',
      status: 'ok',
    },
    {
      message: 'GitHub writeback is connected with repo/workflow scopes.',
      name: 'github_write',
      status: 'ok',
    },
    {
      message: 'SHC control API is reachable.',
      name: 'shc_control',
      status: 'ok',
    },
    {
      message: 'Runner callback URL and token reference are available.',
      name: 'runner_callback',
      status: 'ok',
    },
    {
      details: {
        automaticFailover: {
          effective: false,
          requested: false,
        },
        lanes: {
          cloudflare_container_backup: {
            eligibleForWorkload: true,
            ready: false,
          },
          gcloud_reference: {
            ready: false,
          },
          shc_primary: {
            ready: true,
          },
        },
        policy: 'shc_primary_only',
        workloadTrust: 'low',
      },
      message: 'SHC primary runner is ready; backup lanes are not enabled.',
      name: 'runner_backends',
      status: 'ok',
    },
  ],
  nextSafeAction: 'Create or launch the next Autopilot run.',
  status: 'ok',
  targetUser: null,
})
const defaultTaskPacketRefValidator: TestTaskPacketRefValidator = async () =>
  true
const defaultAutopilotLaunch: TestAutopilotLaunch = async () => ({
  launch: {
    payload: {
      mission: {
        status: 'queued',
      },
      run: {
        status: 'queued',
      },
    },
    runId: 'agent_run_default',
  },
  ok: true,
})
const defaultAutopilotContinuation: TestAutopilotContinuation = async () => ({
  continuation: {
    goalId: 'agent_goal_adjutant',
    mode: 'follow_up_turn',
    payload: {
      accepted: true,
      ingestedEvents: 0,
      status: 'running',
    },
    runId: 'agent_run_adjutant_1',
  },
  ok: true,
})

const makeRoutes = (
  session: TestSession | null,
  autopilotPreflight: TestAutopilotPreflight = defaultAutopilotPreflight,
  validateAdjutantTaskPacketRef: TestTaskPacketRefValidator = defaultTaskPacketRefValidator,
  launchUserAutopilotMission: TestAutopilotLaunch = defaultAutopilotLaunch,
  continueUserAutopilotRun: TestAutopilotContinuation = defaultAutopilotContinuation,
  hasAdminApiToken = false,
) =>
  makeOperatorAdjutantRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    buildOperatorAutopilotPreflightPayload: autopilotPreflight,
    continueUserAutopilotRun,
    isOpenAgentsAdminEmail: email => email === 'chris@openagents.com',
    launchUserAutopilotMission,
    requireAdminApiToken: () => Promise.resolve(hasAdminApiToken),
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
    validateAdjutantTaskPacketRef,
  })

const runRoute = (
  session: TestSession | null,
  store: OperatorAdjutantDbStore,
  request: Request,
  autopilotPreflight?: TestAutopilotPreflight,
  validateAdjutantTaskPacketRef?: TestTaskPacketRefValidator,
  launchUserAutopilotMission?: TestAutopilotLaunch,
  continueUserAutopilotRun?: TestAutopilotContinuation,
  hasAdminApiToken?: boolean,
  envOverrides: Partial<TestEnv> = {},
): Promise<Response> => {
  const route = makeRoutes(
    session,
    autopilotPreflight,
    validateAdjutantTaskPacketRef,
    launchUserAutopilotMission,
    continueUserAutopilotRun,
    hasAdminApiToken,
  ).routeOperatorAdjutantRequest(
    request,
    {
      ADJUTANT_ENRICHMENT_QUEUE: {
        send: (value: unknown) => {
          store.queueMessages.push(value)

          return Promise.resolve()
        },
      } as unknown as Queue,
      GITHUB_CLIENT_ID: 'github-client-id',
      GITHUB_CLIENT_SECRET: 'github-client-secret',
      OPENAGENTS_APP_URL: 'https://openagents.com',
      OPENAGENTS_DB: operatorAdjutantDb(store),
      OPENAUTH_CLIENT_ID: 'openauth-client-id',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      ...envOverrides,
    },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

const runRouteWithEnv = (
  session: TestSession | null,
  store: OperatorAdjutantDbStore,
  request: Request,
  envOverrides: Partial<TestEnv>,
): Promise<Response> => {
  const route = makeRoutes(session).routeOperatorAdjutantRequest(
    request,
    {
      ADJUTANT_ENRICHMENT_QUEUE: {
        send: (value: unknown) => {
          store.queueMessages.push(value)

          return Promise.resolve()
        },
      } as unknown as Queue,
      GITHUB_CLIENT_ID: 'github-client-id',
      GITHUB_CLIENT_SECRET: 'github-client-secret',
      OPENAGENTS_APP_URL: 'https://openagents.com',
      OPENAGENTS_DB: operatorAdjutantDb(store),
      OPENAUTH_CLIENT_ID: 'openauth-client-id',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      ...envOverrides,
    },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

const runRouteWithEnvAndLaunch = (
  session: TestSession | null,
  store: OperatorAdjutantDbStore,
  request: Request,
  envOverrides: Partial<OpenAgentsWorkerConfigEnv>,
  launchUserAutopilotMission: TestAutopilotLaunch,
): Promise<Response> => {
  const route = makeRoutes(
    session,
    defaultAutopilotPreflight,
    defaultTaskPacketRefValidator,
    launchUserAutopilotMission,
  ).routeOperatorAdjutantRequest(
    request,
    {
      ADJUTANT_ENRICHMENT_QUEUE: {
        send: (value: unknown) => {
          store.queueMessages.push(value)

          return Promise.resolve()
        },
      } as unknown as Queue,
      GITHUB_CLIENT_ID: 'github-client-id',
      GITHUB_CLIENT_SECRET: 'github-client-secret',
      OPENAGENTS_APP_URL: 'https://openagents.com',
      OPENAGENTS_DB: operatorAdjutantDb(store),
      OPENAUTH_CLIENT_ID: 'openauth-client-id',
      OPENAUTH_ISSUER_URL: 'https://auth.openagents.com',
      ...envOverrides,
    },
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

const assignOrderRequest = (body: Record<string, unknown> = {}) =>
  new Request(
    'https://openagents.com/api/operator/adjutant/orders/software_order_otec/assign',
    {
      body: JSON.stringify(body),
      method: 'POST',
    },
  )

const completeLaunchChecklist = {
  audienceReviewed: true,
  buildReviewed: true,
  secretsReviewed: true,
  sourceReviewed: true,
  urlReviewed: true,
}

const createDeploymentAssignment = async (
  store: OperatorAdjutantDbStore,
): Promise<string> => {
  await runRoute(
    adminSession,
    store,
    new Request(
      'https://openagents.com/api/operator/adjutant/sites/site_project_otec/assign',
      {
        body: JSON.stringify({
          assignmentKind: 'site_deployment',
          commitSha: '707c0302',
          objective: 'Deploy the reviewed OTEC Site.',
          taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          visibility: 'public',
        }),
        method: 'POST',
      },
    ),
  )

  const assignmentId = store.assignments[0]?.id

  if (assignmentId === undefined) {
    throw new Error('assignment was not created')
  }

  return assignmentId
}

const createGenerationAssignment = async (
  store: OperatorAdjutantDbStore,
): Promise<string> => {
  await runRoute(
    adminSession,
    store,
    new Request(
      'https://openagents.com/api/operator/adjutant/sites/site_project_otec/assign',
      {
        body: JSON.stringify({
          assignmentKind: 'site_generation',
          commitSha: '707c0302',
          objective: 'Generate the OTEC Site.',
          taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          visibility: 'public',
        }),
        method: 'POST',
      },
    ),
  )

  const assignmentId = store.assignments[0]?.id

  if (assignmentId === undefined) {
    throw new Error('assignment was not created')
  }

  return assignmentId
}

const attachApprovedOtecResearch = (
  store: OperatorAdjutantDbStore,
  assignmentId: string,
): void => {
  const assignment = store.assignments.find(
    candidate => candidate.id === assignmentId,
  )

  if (assignment === undefined) {
    throw new Error('expected assignment')
  }

  store.enrichmentRuns.push({
    approved_source_count: 2,
    archived_at: null,
    assignment_id: assignment.id,
    cache_hit_count: 1,
    completed_at: '2026-06-05T00:05:00.000Z',
    cost_dollars: 0.04,
    created_at: '2026-06-05T00:01:00.000Z',
    error_code: null,
    error_summary: null,
    id: 'exa_enrichment_run_otec',
    plan_id: 'exa_plan_otec',
    request_budget: 3,
    request_count: 3,
    site_id: assignment.site_id,
    software_order_id: assignment.software_order_id,
    source_count: 2,
    started_at: '2026-06-05T00:01:00.000Z',
    status: 'approved',
    subject:
      'Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.',
    updated_at: '2026-06-05T00:06:00.000Z',
  })
  store.researchBriefs.push({
    approved_at: '2026-06-05T00:06:00.000Z',
    archived_at: null,
    assignment_id: assignment.id,
    claims_needing_review_json: JSON.stringify([
      'Gigawatt-scale language should be presented as ambition unless the customer provides deployment specifics.',
    ]),
    created_at: '2026-06-05T00:05:00.000Z',
    created_by_user_id: 'github:operator',
    enrichment_run_id: 'exa_enrichment_run_otec',
    grounded_facts_json: JSON.stringify([
      'OTEC uses warm surface water and cold deep seawater temperature differences to generate power.',
      'SWAC uses cold deep seawater for thermal management and can be explained alongside OTEC.',
    ]),
    id: 'adjutant_research_brief_otec',
    rejected_at: null,
    review_reason: 'Approved for OTEC launch smoke.',
    reviewed_by_user_id: 'github:operator',
    source_cards_json: JSON.stringify([
      {
        domain: 'example.com',
        highlightText: 'Ocean thermal energy conversion context.',
        id: 'exa_enrichment_source_otec',
        title: 'OTEC overview',
        url: 'https://example.com/otec',
      },
      {
        domain: 'example.com',
        highlightText: 'Seawater air conditioning context.',
        id: 'exa_enrichment_source_swac',
        title: 'SWAC overview',
        url: 'https://example.com/swac',
      },
    ]),
    status: 'approved',
    suggested_sections_json: JSON.stringify([
      'Open with the floating datacenter concept, then explain OTEC power and SWAC cooling as the infrastructure thesis.',
      'Include a public-source-backed section for unknowns and operator-reviewed claims.',
    ]),
    summary:
      'Approved public evidence supports using OTEC and SWAC as the grounding context for the floating datacenter Site.',
    unknowns_json: JSON.stringify([
      'Customer-specific deployment location and engineering constraints remain unknown.',
    ]),
    updated_at: '2026-06-05T00:06:00.000Z',
  })
}

describe('operator Adjutant assignment API routes', () => {
  test('rejects unsupported methods before session work', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignGetResponse = await runRoute(
      null,
      store,
      new Request(
        'https://openagents.com/api/operator/adjutant/orders/software_order_otec/assign',
        { method: 'GET' },
      ),
    )
    const listPostResponse = await runRoute(
      adminSession,
      store,
      new Request('https://openagents.com/api/operator/adjutant/assignments', {
        method: 'POST',
      }),
    )
    const researchPatchResponse = await runRoute(
      null,
      store,
      new Request(
        'https://openagents.com/api/operator/adjutant/assignments/assignment_otec/research-policy',
        { method: 'PATCH' },
      ),
    )

    expect(assignGetResponse.status).toBe(405)
    expect(assignGetResponse.headers.get('allow')).toBe('POST')
    await expect(assignGetResponse.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
    expect(listPostResponse.status).toBe(405)
    expect(listPostResponse.headers.get('allow')).toBe('GET')
    await expect(listPostResponse.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
    expect(researchPatchResponse.status).toBe(405)
    expect(researchPatchResponse.headers.get('allow')).toBe('GET, POST')
    await expect(researchPatchResponse.json()).resolves.toEqual({
      error: 'method_not_allowed',
    })
  })

  test('returns unauthorized without a browser session', async () => {
    const response = await runRoute(
      null,
      new OperatorAdjutantDbStore(),
      new Request('https://openagents.com/api/operator/adjutant/assignments'),
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
      new OperatorAdjutantDbStore(),
      new Request('https://openagents.com/api/operator/adjutant/assignments'),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toEqual({ error: 'forbidden' })
  })

  test('accepts an admin API bearer token as a CLI operator session', async () => {
    const store = new OperatorAdjutantDbStore()
    const response = await runRoute(
      null,
      store,
      new Request('https://openagents.com/api/operator/adjutant/assignments'),
      undefined,
      undefined,
      undefined,
      undefined,
      true,
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    const body = (await response.json()) as {
      assignments: ReadonlyArray<unknown>
    }
    expect(Array.isArray(body.assignments)).toBe(true)
  })

  test('assigns a software order to Adjutant and records a safe event', async () => {
    const store = new OperatorAdjutantDbStore()
    const response = await runRoute(
      adminSession,
      store,
      assignOrderRequest({
        objective: 'Generate the OTEC Site.',
        visibility: 'public',
      }),
    )

    expect(response.status).toBe(201)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        agentId: 'agent_adjutant',
        assignedByUserId: 'github:operator',
        assignmentKind: 'site_generation',
        goalId: expect.any(String),
        objective: 'Generate the OTEC Site.',
        projectId: 'project_adjutant',
        siteId: null,
        softwareOrderId: 'software_order_otec',
        status: 'draft',
        teamId: 'team_openagents_core',
        visibility: 'public',
      }),
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.events).toEqual([
      expect.objectContaining({
        actor_user_id: 'github:operator',
        assignment_id: store.assignments[0]?.id,
        event_type: 'adjutant.assignment_created',
        goal_id: store.goals[0]?.id,
        payload_json: JSON.stringify({
          assignmentKind: 'site_generation',
          status: 'draft',
          visibility: 'public',
        }),
        software_order_id: 'software_order_otec',
        summary: 'Autopilot assignment created.',
        visibility: 'public',
      }),
    ])
    expect(store.assignments[0]?.goal_id).toBe(store.goals[0]?.id)
    expect(store.goals).toEqual([
      expect.objectContaining({
        agent_id: 'agent_adjutant',
        project_id: 'project_adjutant',
        team_id: 'team_openagents_core',
        visibility: 'public',
      }),
    ])
  })

  test('assigns an existing Site to Adjutant and infers the order', async () => {
    const store = new OperatorAdjutantDbStore()
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        'https://openagents.com/api/operator/adjutant/sites/site_project_otec/assign',
        {
          body: JSON.stringify({
            assignmentKind: 'site_adjustment',
            objective: 'Adjust the OTEC Site hero.',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(201)
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        assignmentKind: 'site_adjustment',
        goalId: expect.any(String),
        siteId: 'site_project_otec',
        softwareOrderId: 'software_order_otec',
      }),
    })
    expect(store.events[0]).toMatchObject({
      event_type: 'adjutant.assignment_created',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
    })
  })

  test('lists and reads assignments', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id

    const listResponse = await runRoute(
      adminSession,
      store,
      new Request('https://openagents.com/api/operator/adjutant/assignments'),
    )
    const readResponse = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}`,
      ),
    )

    expect(listResponse.status).toBe(200)
    await expect(listResponse.json()).resolves.toEqual({
      assignments: [
        expect.objectContaining({
          goalId: store.goals[0]?.id,
          id: assignmentId,
          softwareOrderId: 'software_order_otec',
        }),
      ],
    })
    expect(readResponse.status).toBe(200)
    await expect(readResponse.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        goalId: store.goals[0]?.id,
        id: assignmentId,
        softwareOrderId: 'software_order_otec',
      }),
      review: expect.objectContaining({
        deployments: [],
        goal: expect.objectContaining({
          id: store.goals[0]?.id,
        }),
        nextAction:
          'Generate the tracked task packet, run preflight, then launch Autopilot.',
        order: expect.objectContaining({
          id: 'software_order_otec',
          repositoryFullName: 'OpenAgentsInc/autopilot-omega',
        }),
        site: null,
        versions: [],
      }),
    })
  })

  test('reads and stores assignment research policy overrides', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id

    const readDefault = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/research-policy`,
      ),
    )

    expect(readDefault.status).toBe(200)
    await expect(readDefault.json()).resolves.toEqual({
      policy: expect.objectContaining({
        customerSafeStatus: 'research_required',
        defaultMode: 'research_required',
        effectiveMode: 'research_required',
        reason: null,
        source: 'default_assignment_kind',
      }),
    })

    const setBypass = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/research-policy`,
        {
          body: JSON.stringify({
            customerSafeSummary:
              'The operator approved this assignment using customer-provided public context.',
            policyMode: 'research_bypassed_by_operator',
            reason:
              'The order already included enough public source context for this first pass.',
            sourceAuthorityRef: 'order:software_order_otec',
          }),
          method: 'POST',
        },
      ),
    )

    expect(setBypass.status).toBe(200)
    await expect(setBypass.json()).resolves.toEqual({
      policy: expect.objectContaining({
        actorUserId: 'github:operator',
        customerSafeStatus: 'research_bypassed',
        defaultMode: 'research_required',
        effectiveMode: 'research_bypassed_by_operator',
        reason:
          'The order already included enough public source context for this first pass.',
        source: 'operator_override',
        sourceAuthorityRef: 'order:software_order_otec',
      }),
    })
    expect(store.researchPolicies).toHaveLength(1)
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.research_policy_set',
        payload_json: JSON.stringify({
          customerSafeStatus: 'research_bypassed',
          effectiveMode: 'research_bypassed_by_operator',
          sourceAuthorityRef: 'order:software_order_otec',
        }),
      }),
    )

    const readAssignment = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}`,
      ),
    )

    expect(readAssignment.status).toBe(200)
    await expect(readAssignment.json()).resolves.toEqual(
      expect.objectContaining({
        review: expect.objectContaining({
          researchPolicy: expect.objectContaining({
            customerSafeStatus: 'research_bypassed',
            effectiveMode: 'research_bypassed_by_operator',
          }),
        }),
      }),
    )
  })

  test('rejects invalid research policy overrides', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/research-policy`,
        {
          body: JSON.stringify({
            customerSafeSummary: 'Operator approved this assignment.',
            policyMode: 'research_bypassed_by_operator',
            reason: '   ',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'research_policy_validation_error',
      reason: 'reason is required.',
    })
    expect(store.researchPolicies).toEqual([])
  })

  test('requires an operator session for enrichment endpoints', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id

    const endpoints = [
      `/api/operator/adjutant/assignments/${assignmentId}/enrichment/plan`,
      `/api/operator/adjutant/assignments/${assignmentId}/enrichment/enqueue`,
      `/api/operator/adjutant/assignments/${assignmentId}/enrichment/run`,
      `/api/operator/adjutant/assignments/${assignmentId}/enrichment/source-cards/exa_source_1/review`,
      `/api/operator/adjutant/assignments/${assignmentId}/enrichment/briefs/adjutant_research_brief_1/review`,
    ]
    const responses = await Promise.all(
      endpoints.map(path =>
        runRoute(
          null,
          store,
          new Request(`https://openagents.com${path}`, {
            body: JSON.stringify({ reviewStatus: 'rejected', status: 'stale' }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          }),
        ),
      ),
    )

    expect(responses.map(response => response.status)).toEqual([
      401, 401, 401, 401, 401,
    ])
  })

  test('plans enrichment from explicit assignment context', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/plan`,
        {
          body: JSON.stringify({ numResults: 3 }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      exaConfigured: false,
      plan: expect.objectContaining({
        assignmentId,
        expectedSourceCategories: expect.arrayContaining([
          'topic_web',
          'repository',
        ]),
        searchTasks: expect.arrayContaining([
          expect.objectContaining({
            sourceCategory: 'topic_web',
          }),
          expect.objectContaining({
            sourceCategory: 'repository',
          }),
        ]),
      }),
    })
  })

  test('enqueues enrichment jobs and returns duplicate active job state', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id

    const first = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/enqueue`,
        {
          body: JSON.stringify({
            numResults: 2,
            operatorNotes:
              'Queue public-source research for the assignment only.',
            requestBudget: 2,
          }),
          method: 'POST',
        },
      ),
    )

    expect(first.status).toBe(202)
    const firstBody = (await first.json()) as {
      duplicate: boolean
      enrichment: { latestJob: { id: string; status: string } }
      job: { enrichmentRunId: string; id: string; status: string }
    }
    expect(firstBody.duplicate).toBe(false)
    expect(firstBody.job).toMatchObject({
      enrichmentRunId: expect.any(String),
      status: 'queued',
    })
    expect(firstBody.enrichment.latestJob).toMatchObject({
      id: firstBody.job.id,
      status: 'queued',
    })
    expect(store.enrichmentJobs).toHaveLength(1)
    expect(store.enrichmentRuns).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        id: firstBody.job.enrichmentRunId,
        status: 'queued',
      }),
    )
    expect(store.queueMessages).toEqual([
      expect.objectContaining({
        assignmentId,
        jobId: firstBody.job.id,
        schemaVersion: 'openagents.adjutant_enrichment_job.v1',
      }),
    ])

    const second = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/enqueue`,
        {
          body: JSON.stringify({ requestBudget: 2 }),
          method: 'POST',
        },
      ),
    )

    expect(second.status).toBe(200)
    await expect(second.json()).resolves.toEqual(
      expect.objectContaining({
        duplicate: true,
        job: expect.objectContaining({
          id: firstBody.job.id,
          status: 'queued',
        }),
      }),
    )
    expect(store.enrichmentJobs).toHaveLength(1)
    expect(store.queueMessages).toHaveLength(1)
  })

  test('reports unconfigured Exa before running enrichment', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/run`,
        {
          body: '{}',
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        configured: false,
        error: 'exa_unconfigured',
      }),
    )
    expect(store.enrichmentRuns).toEqual([])
  })

  test('blocks configured enrichment when the assignment Exa budget is exhausted', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id
    const response = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/run`,
        {
          body: JSON.stringify({ requestBudget: 4 }),
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
        EXA_ASSIGNMENT_REQUEST_BUDGET: '1',
        EXA_DAILY_REQUEST_BUDGET: '10',
      },
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        error: 'exa_budget_exhausted',
        scope: 'assignment',
      }),
    )
    expect(store.enrichmentRuns).toEqual([])
  })

  test('bounds long Exa highlights before storing source cards', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignmentId = store.assignments[0]?.id
    const originalFetch = globalThis.fetch
    const providerHighlight = 'Public provider highlight. '.repeat(120)
    globalThis.fetch = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            costDollars: {
              search: { neural: 0.007 },
              total: 0.007,
            },
            requestId: 'exa_req_large_highlight',
            results: [
              {
                highlights: [providerHighlight],
                id: 'https://example.com/large-highlight',
                score: 0.91,
                title: 'Large highlight source',
                url: 'https://example.com/large-highlight',
              },
            ],
            searchType: 'auto',
          }),
          { headers: { 'content-type': 'application/json' }, status: 200 },
        ),
    ) as typeof fetch

    try {
      const response = await runRouteWithEnv(
        adminSession,
        store,
        new Request(
          `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/enrichment/run`,
          {
            body: JSON.stringify({ numResults: 1, requestBudget: 1 }),
            headers: { 'content-type': 'application/json' },
            method: 'POST',
          },
        ),
        {
          EXA_API_KEY: 'exa-test-secret',
        },
      )

      const payload = await response.json()
      expect(response.status).toBe(202)
      expect(payload).toEqual(
        expect.objectContaining({
          runId: expect.any(String),
        }),
      )
    } finally {
      globalThis.fetch = originalFetch
    }

    expect(store.enrichmentRuns[0]).toEqual(
      expect.objectContaining({
        source_count: 1,
        status: 'needs_review',
      }),
    )
    expect(store.enrichmentSources).toHaveLength(1)
    expect(store.enrichmentSources[0]?.highlight_text).toHaveLength(1200)
    expect(store.enrichmentSources[0]?.selected_text_hash).toHaveLength(64)
    expect(JSON.stringify(store.enrichmentSources)).not.toContain(
      providerHighlight,
    )
  })

  test('blocks duplicate active Exa enrichment runs before launching provider work', async () => {
    const store = new OperatorAdjutantDbStore()
    await runRoute(adminSession, store, assignOrderRequest())
    const assignment = store.assignments[0]

    if (assignment === undefined) {
      throw new Error('expected assignment')
    }

    store.enrichmentRuns.push({
      approved_source_count: 0,
      archived_at: null,
      assignment_id: assignment.id,
      cache_hit_count: 0,
      completed_at: null,
      cost_dollars: null,
      created_at: '2026-06-05T00:00:00.000Z',
      error_code: null,
      error_summary: null,
      id: 'exa_enrichment_run_active',
      plan_id: 'exa_plan_active',
      request_budget: 2,
      request_count: 0,
      site_id: assignment.site_id,
      software_order_id: assignment.software_order_id,
      source_count: 0,
      started_at: '2026-06-05T00:00:00.000Z',
      status: 'running',
      subject: assignment.objective,
      updated_at: '2026-06-05T00:00:00.000Z',
    })

    const response = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignment.id}/enrichment/run`,
        {
          body: '{}',
          headers: { 'content-type': 'application/json' },
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        error: 'adjutant_enrichment_already_running',
      }),
    )
    expect(store.enrichmentRuns).toHaveLength(1)
  })

  test('returns review data for a Site-linked Adjutant assignment', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    store.assignments = store.assignments.map(assignment =>
      assignment.id === assignmentId
        ? {
            ...assignment,
            current_run_id: 'agent_run_adjutant_1',
            status: 'delivered',
          }
        : assignment,
    )
    store.events.push({
      actor_user_id: 'github:operator',
      assignment_id: assignmentId,
      created_at: '2026-06-05T00:09:00.000Z',
      event_type: 'adjutant.run_delivered',
      goal_id: store.assignments[0]?.goal_id ?? null,
      id: 'adjutant_assignment_event_review',
      payload_json: null,
      run_id: 'agent_run_adjutant_1',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
      summary: 'Autopilot delivered the run result.',
      visibility: 'public',
    })
    store.siteEvents.push({
      actor_run_id: 'agent_run_adjutant_1',
      actor_user_id: 'github:operator',
      created_at: '2026-06-05T00:08:00.000Z',
      deployment_id: null,
      id: 'site_event_version_saved',
      payload_json: null,
      site_id: 'site_project_otec',
      summary: 'Saved version site_version_otec.',
      type: 'site_version.saved',
      version_id: 'site_version_otec',
    })
    store.siteEvents.push({
      actor_run_id: null,
      actor_user_id: 'github:operator',
      created_at: '2026-06-05T00:07:00.000Z',
      deployment_id: 'site_deployment_old',
      id: 'site_event_rollback',
      payload_json: null,
      site_id: 'site_project_otec',
      summary: 'Rolled back deployment site_deployment_old.',
      type: 'site_deployment.rolled_back',
      version_id: 'site_version_old',
    })
    store.usageReceipts.push(
      {
        adjustment_id: null,
        assignment_id: assignmentId,
        billing_ledger_entry_id: null,
        billing_mode: 'public_beta_free',
        category: 'generation',
        created_at: '2026-06-05T00:06:00.000Z',
        credits_charged_cents: 0,
        currency: 'USD',
        id: 'adjutant_usage_generation',
        idempotency_key: 'adjutant_usage:review:generation',
        public_receipt_json: '{}',
        quantity: 1,
        run_id: 'agent_run_adjutant_1',
        site_id: 'site_project_otec',
        software_order_id: 'software_order_otec',
        summary: 'Autopilot Site generation run was queued.',
        team_receipt_json: '{}',
        unit: 'run',
        visibility: 'public',
      },
      {
        adjustment_id: null,
        assignment_id: assignmentId,
        billing_ledger_entry_id: null,
        billing_mode: 'public_beta_free',
        category: 'hosting',
        created_at: '2026-06-05T00:10:00.000Z',
        credits_charged_cents: 0,
        currency: 'USD',
        id: 'adjutant_usage_hosting',
        idempotency_key: 'adjutant_usage:review:hosting',
        public_receipt_json: '{}',
        quantity: 1,
        run_id: 'agent_run_adjutant_1',
        site_id: 'site_project_otec',
        software_order_id: 'software_order_otec',
        summary: 'Autopilot activated public Site hosting.',
        team_receipt_json: '{}',
        unit: 'deployment',
        visibility: 'public',
      },
    )

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}`,
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        currentRunId: 'agent_run_adjutant_1',
        id: assignmentId,
        siteId: 'site_project_otec',
      }),
      review: expect.objectContaining({
        assignmentEvents: expect.arrayContaining([
          expect.objectContaining({
            type: 'adjutant.run_delivered',
          }),
          expect.objectContaining({
            type: 'adjutant.assignment_created',
          }),
        ]),
        currentRun: expect.objectContaining({
          eventCursor: 7,
          id: 'agent_run_adjutant_1',
          status: 'completed',
        }),
        deployments: [
          expect.objectContaining({
            id: 'site_deployment_otec',
            status: 'active',
            url: 'https://sites.openagents.com/otec',
          }),
        ],
        nextAction:
          'Monitor the active deployment or disable/rollback if the release is unsafe.',
        site: expect.objectContaining({
          accessMode: 'public',
          activeDeploymentId: 'site_deployment_otec',
          id: 'site_project_otec',
          visibility: 'public',
        }),
        siteEvents: expect.arrayContaining([
          expect.objectContaining({ type: 'site_version.saved' }),
          expect.objectContaining({ type: 'site_deployment.rolled_back' }),
        ]),
        usageReceipts: expect.arrayContaining([
          expect.objectContaining({
            category: 'generation',
            creditsChargedFormatted: '$0.00',
            unit: 'run',
          }),
          expect.objectContaining({
            category: 'hosting',
            creditsChargedFormatted: '$0.00',
            unit: 'deployment',
          }),
        ]),
        usageSummary: expect.objectContaining({
          billingMode: 'public_beta_free',
          categories: expect.arrayContaining([
            expect.objectContaining({ category: 'generation', quantity: 1 }),
            expect.objectContaining({ category: 'hosting', quantity: 1 }),
          ]),
          totalCreditsChargedCents: 0,
        }),
        versions: [
          expect.objectContaining({
            buildStatus: 'saved',
            createdByRunId: 'agent_run_adjutant_1',
            id: 'site_version_otec',
            sourceKind: 'autopilot_generated',
          }),
        ],
      }),
    })
  })

  test('generates a safe task packet and records its pushed commit ref', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const validatedRefs: Array<Parameters<TestTaskPacketRefValidator>[0]> = []
    store.assignments = store.assignments.map(assignment => ({
      ...assignment,
      commit_sha: null,
      task_spec_path: null,
    }))
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'a0badf52',
            operatorNotes: 'Focus on the public OTEC Site release.',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          }),
          method: 'POST',
        },
      ),
      undefined,
      async input => {
        validatedRefs.push(input)

        return true
      },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        commitSha: 'a0badf52',
        id: assignmentId,
        taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
      }),
      packet: {
        commitSha: 'a0badf52',
        markdown: expect.stringContaining(
          'targetUrl: https://sites.openagents.com/otec',
        ),
        path: 'docs/autopilot-tasks/adjutant-otec.md',
      },
      taskPacketFreshness: expect.objectContaining({
        researchBriefId: null,
        status: 'current',
        taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
      }),
    })
    expect(validatedRefs).toEqual([
      {
        commitSha: 'a0badf52',
        path: 'docs/autopilot-tasks/adjutant-otec.md',
        repositoryName: 'autopilot-omega',
        repositoryOwner: 'OpenAgentsInc',
      },
    ])
    const assignment = store.assignments[0]

    expect(assignment?.commit_sha).toBe('a0badf52')
    expect(assignment?.task_spec_path).toBe(
      'docs/autopilot-tasks/adjutant-otec.md',
    )
  })

  test('validates task packet refs with the operator GitHub identity token when present', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const validatedRefs: Array<Parameters<TestTaskPacketRefValidator>[0]> = []
    store.assignments = store.assignments.map(assignment => ({
      ...assignment,
      commit_sha: null,
      task_spec_path: null,
    }))

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'a0badf52',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          }),
          method: 'POST',
        },
      ),
      undefined,
      async input => {
        validatedRefs.push(input)

        return true
      },
      undefined,
      undefined,
      undefined,
      {
        AUTH_KV: {
          get: ((key: string) =>
            Promise.resolve(
              key === 'github-identity:token:github:operator'
                ? 'github-identity-token'
                : null,
            )) as AuthKvStore['get'],
          put: () => Promise.resolve(),
          delete: () => Promise.resolve(),
          listPrefix: () => Promise.resolve([]),
        },
      },
    )

    expect(response.status).toBe(200)
    expect(validatedRefs).toEqual([
      {
        commitSha: 'a0badf52',
        githubAccessToken: 'github-identity-token',
        path: 'docs/autopilot-tasks/adjutant-otec.md',
        repositoryName: 'autopilot-omega',
        repositoryOwner: 'OpenAgentsInc',
      },
    ])
  })

  test('generates the canonical OTEC task packet with approved research brief context', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    attachApprovedOtecResearch(store, assignmentId)
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'a0badf52',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    const payload = (await response.json()) as {
      packet: { markdown: string; path: string }
    }

    expect(payload.packet.path).toBe('docs/autopilot-tasks/adjutant-otec.md')
    expect(payload.packet.markdown).toContain('## Approved Research Brief')
    expect(payload.packet.markdown).toContain(
      '- researchBriefId: adjutant_research_brief_otec',
    )
    expect(payload.packet.markdown).toContain(
      'OTEC uses warm surface water and cold deep seawater temperature differences',
    )
    expect(payload.packet.markdown).toContain(
      'SWAC overview: https://example.com/swac',
    )
    expect(JSON.stringify(payload)).not.toContain('exa-test-secret')
  })

  test('keeps a stale task packet current with an operator reason', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    attachApprovedOtecResearch(store, assignmentId)

    const keep = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet/keep-current`,
        {
          body: JSON.stringify({
            customerSafeSummary:
              'The current task packet already includes the approved public research context needed for this pass.',
            reason:
              'Operator reviewed the approved brief and confirmed the current packet is sufficient.',
          }),
          method: 'POST',
        },
      ),
    )

    expect(keep.status).toBe(200)
    await expect(keep.json()).resolves.toEqual({
      taskPacketFreshness: expect.objectContaining({
        latestApprovedResearchBriefId: 'adjutant_research_brief_otec',
        researchBriefId: 'adjutant_research_brief_otec',
        status: 'kept_current',
        taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
      }),
    })
    expect(store.taskPacketFreshness).toContainEqual(
      expect.objectContaining({
        actor_user_id: 'github:operator',
        assignment_id: assignmentId,
        status: 'kept_current',
      }),
    )
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.task_packet_kept_current',
      }),
    )
  })

  test('rejects task packets missing from the pushed commit ref', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    store.assignments = store.assignments.map(assignment => ({
      ...assignment,
      commit_sha: null,
      task_spec_path: null,
    }))
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'a0badf52',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-missing.md',
          }),
          method: 'POST',
        },
      ),
      undefined,
      async () => false,
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      commitSha: 'a0badf52',
      error: 'task_packet_ref_missing',
      path: 'docs/autopilot-tasks/adjutant-missing.md',
      reason: 'Task packet was not found at the pushed commit SHA.',
    })
    expect(store.assignments[0]?.commit_sha).toBeNull()
    expect(store.assignments[0]?.task_spec_path).toBeNull()
  })

  test('rejects invalid task packet paths and commit SHAs', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const invalidPath = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'a0badf52',
            taskSpecPath: '../secrets.md',
          }),
          method: 'POST',
        },
      ),
    )
    const invalidSha = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: 'not-a-sha',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          }),
          method: 'POST',
        },
      ),
    )

    expect(invalidPath.status).toBe(400)
    await expect(invalidPath.json()).resolves.toEqual({
      error: 'task_packet_validation_error',
      reason:
        'Task packet path must be a Markdown file directly under docs/autopilot-tasks/.',
    })
    expect(invalidSha.status).toBe(400)
    await expect(invalidSha.json()).resolves.toEqual({
      error: 'task_packet_validation_error',
      reason: 'Task packet commit SHA must be a 7 to 40 character hex SHA.',
    })
  })

  test('launches an Adjutant assignment through Omni and records order and Site refs', async () => {
    const store = new OperatorAdjutantDbStore()
    const softwareOrder = store.softwareOrders[0]
    const site = store.sites[0]

    if (softwareOrder === undefined) {
      throw new Error('Expected base software order fixture.')
    }

    if (site === undefined) {
      throw new Error('Expected base Site fixture.')
    }

    store.softwareOrders[0] = {
      ...softwareOrder,
      repository_default_branch: 'chore/translate-frontend-english',
    }
    store.sites[0] = {
      ...site,
      source_repository_ref: 'chore/translate-frontend-english',
    }
    const assignmentId = await createDeploymentAssignment(store)
    store.paymentPolicies.push({
      applied_by_user_id: 'github:operator',
      archived_at: null,
      assignment_id: assignmentId,
      created_at: '2026-06-05T00:00:00.000Z',
      customer_safe_summary:
        'This first-batch OpenAgents run is covered by a public beta free slice.',
      id: 'first_batch_payment_policy_otec',
      policy_mode: 'public_beta_free',
      reason: 'Public beta free slice for first submitted-order batch.',
      site_id: 'site_project_otec',
      software_order_id: 'software_order_otec',
      updated_at: '2026-06-05T00:00:00.000Z',
    })
    const launchInputs: Array<Parameters<TestAutopilotLaunch>[2]> = []
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async (_env, _ctx, input) => {
        launchInputs.push(input)

        return {
          launch: {
            payload: {
              mission: {
                status: 'queued',
              },
              run: {
                status: 'queued',
              },
            },
            runId: 'agent_run_adjutant_1',
          },
          ok: true,
        }
      },
    )

    expect(response.status).toBe(202)
    await expect(response.json()).resolves.toEqual({
      accepted: true,
      assignment: expect.objectContaining({
        currentRunId: 'agent_run_adjutant_1',
        id: assignmentId,
        status: 'queued',
      }),
      launch: {
        mission: {
          status: 'queued',
        },
        run: {
          status: 'queued',
        },
      },
      preflight: expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({
            name: 'research_brief',
            status: 'warning',
          }),
          expect.objectContaining({
            name: 'research_review',
            status: 'warning',
          }),
        ]),
        status: 'warning',
      }),
      runId: 'agent_run_adjutant_1',
    })
    expect(launchInputs).toEqual([
      {
        selector: expect.objectContaining({
          baseRef: 'chore/translate-frontend-english',
          branchName: `adjutant/${assignmentId}`,
          dispatchGoal: expect.stringContaining(
            `Autopilot assignment ${assignmentId}`,
          ),
          openPullRequest: true,
          repository:
            'OpenAgentsInc/autopilot-omega@chore/translate-frontend-english',
          repositoryRef: 'chore/translate-frontend-english',
        }),
        userId: 'github:operator',
      },
    ])
    expect(String(launchInputs[0]?.selector.dispatchGoal)).toContain(
      'Task packet: docs/autopilot-tasks/adjutant-otec.md',
    )
    expect(String(launchInputs[0]?.selector.dispatchGoal)).toContain(
      'Research brief ID: none',
    )
    expect(String(launchInputs[0]?.selector.pullRequestBody)).toContain(
      'Specification: linked in the assignment metadata',
    )
    expect(String(launchInputs[0]?.selector.pullRequestBody)).not.toContain(
      'autopilot-tasks',
    )
    expect(String(launchInputs[0]?.selector.pullRequestBody)).not.toContain(
      'sk-',
    )
    expect(store.softwareOrders[0]).toMatchObject({
      current_run_id: 'agent_run_adjutant_1',
      status: 'agent_queued',
    })
    expect(store.siteEvents).toEqual([
      expect.objectContaining({
        actor_run_id: 'agent_run_adjutant_1',
        actor_user_id: 'github:operator',
        site_id: 'site_project_otec',
        type: 'adjutant.run_queued',
      }),
    ])
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.run_queued',
        run_id: 'agent_run_adjutant_1',
      }),
    )
  })

  test('blocks first-batch launch until an explicit no-payment policy exists', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const launches: Array<Parameters<TestAutopilotLaunch>[2]> = []
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async (_env, _ctx, input) => {
        launches.push(input)

        return {
          launch: {
            payload: {},
            runId: 'agent_run_should_not_start',
          },
          ok: true,
        }
      },
    )
    const body = (await response.json()) as Record<string, unknown>

    expect(response.status).toBe(409)
    expect(body).toMatchObject({
      error: 'adjutant_launch_blocked',
      status: 'blocked',
    })
    expect(JSON.stringify(body)).toContain('first_batch_payment_policy')
    expect(launches).toHaveLength(0)
    expect(store.usageReceipts).toHaveLength(0)
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.launch_blocked',
      }),
    )
  })

  test('records zero-charge first-batch policy receipt for generation launch', async () => {
    const store = new OperatorAdjutantDbStore()
    const create = await runRoute(
      adminSession,
      store,
      assignOrderRequest({
        objective: 'Generate the OTEC Site.',
        visibility: 'public',
      }),
    )
    expect(create.status).toBe(201)

    const assignmentId = store.assignments[0]?.id

    if (assignmentId === undefined) {
      throw new Error('expected assignment')
    }

    store.assignments = store.assignments.map(assignment =>
      assignment.id === assignmentId
        ? {
            ...assignment,
            commit_sha: '707c0302',
            status: 'preflight_pending',
            task_spec_path: 'docs/autopilot-tasks/adjutant-otec.md',
          }
        : assignment,
    )
    attachApprovedOtecResearch(store, assignmentId)
    addFirstBatchPaymentPolicy(store, assignmentId)

    const launch = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async () => ({
        launch: {
          payload: {
            mission: { status: 'queued' },
            run: { status: 'queued' },
          },
          runId: 'agent_run_generation_1',
        },
        ok: true,
      }),
    )

    expect(launch.status).toBe(202)
    expect(store.usageReceipts).toEqual([
      expect.objectContaining({
        billing_ledger_entry_id: null,
        billing_mode: 'public_beta_free',
        credits_charged_cents: 0,
        software_order_id: 'software_order_otec',
      }),
    ])
    expect(
      JSON.parse(store.usageReceipts[0]?.team_receipt_json ?? '{}'),
    ).toMatchObject({
      firstBatchPaymentPolicyId: `first_batch_payment_policy_${assignmentId}`,
      firstBatchPaymentPolicyMode: 'public_beta_free',
    })
    expect(JSON.stringify(store.usageReceipts)).not.toContain('Lightning')
    expect(JSON.stringify(store.usageReceipts)).not.toContain('settled')
  })

  test('continues an active Adjutant run for a Site adjustment', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const goalCount = store.goals.length
    const continuations: Array<Parameters<TestAutopilotContinuation>[2]> = []
    const launches: Array<Parameters<TestAutopilotLaunch>[2]> = []
    store.runs = store.runs.map(run =>
      run.id === 'agent_run_adjutant_1'
        ? {
            ...run,
            status: 'running',
          }
        : run,
    )
    store.assignments = store.assignments.map(assignment =>
      assignment.id === assignmentId
        ? {
            ...assignment,
            current_run_id: 'agent_run_adjutant_1',
            status: 'running',
          }
        : assignment,
    )

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/adjustments`,
        {
          body: JSON.stringify({
            instruction: 'Make the hero headline clearer for public visitors.',
          }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async (_env, _ctx, input) => {
        launches.push(input)

        return defaultAutopilotLaunch(_env, _ctx, input)
      },
      async (_env, _ctx, input) => {
        continuations.push(input)

        return {
          continuation: {
            goalId: store.assignments[0]?.goal_id ?? null,
            mode: 'follow_up_turn',
            payload: {
              accepted: true,
              ingestedEvents: 1,
              status: 'running',
            },
            runId: input.runId,
          },
          ok: true,
        }
      },
    )

    expect(response.status).toBe(202)
    await expect(response.json()).resolves.toEqual({
      accepted: true,
      adjustment: expect.objectContaining({
        assignmentId,
        continuationMode: 'follow_up_turn',
        continuationRunId: 'agent_run_adjutant_1',
        siteId: 'site_project_otec',
        sourceRunId: 'agent_run_adjutant_1',
        status: 'running',
      }),
      assignment: expect.objectContaining({
        currentRunId: 'agent_run_adjutant_1',
        id: assignmentId,
        status: 'running',
      }),
      continuation: expect.objectContaining({
        mode: 'follow_up_turn',
        runId: 'agent_run_adjutant_1',
      }),
      runId: 'agent_run_adjutant_1',
    })
    expect(continuations).toEqual([
      {
        prompt: expect.stringContaining(
          'Make the hero headline clearer for public visitors.',
        ),
        runId: 'agent_run_adjutant_1',
        userId: 'github:operator',
      },
    ])
    expect(launches).toEqual([])
    expect(store.goals).toHaveLength(goalCount)
    expect(store.softwareOrders[0]).toMatchObject({
      current_run_id: 'agent_run_adjutant_1',
      status: 'agent_running',
    })
    expect(store.sites[0]).toMatchObject({
      status: 'generating',
    })
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.adjustment_requested',
        run_id: 'agent_run_adjutant_1',
      }),
    )
    expect(store.siteEvents).toContainEqual(
      expect.objectContaining({
        actor_run_id: 'agent_run_adjutant_1',
        site_id: 'site_project_otec',
        type: 'adjutant.adjustment_running',
      }),
    )

    const reviewResponse = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}`,
      ),
    )

    await expect(reviewResponse.json()).resolves.toEqual({
      assignment: expect.objectContaining({ id: assignmentId }),
      review: expect.objectContaining({
        adjustments: [
          expect.objectContaining({
            continuationMode: 'follow_up_turn',
            status: 'running',
          }),
        ],
        nextAction:
          'Wait for Autopilot to save the adjusted Site version for review.',
      }),
    })
  })

  test('launches a new run for adjustments after the current run completed', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const goalId = store.assignments[0]?.goal_id
    const goalCount = store.goals.length
    const launchInputs: Array<Parameters<TestAutopilotLaunch>[2]> = []
    let continuationCalled = false
    store.assignments = store.assignments.map(assignment =>
      assignment.id === assignmentId
        ? {
            ...assignment,
            current_run_id: 'agent_run_adjutant_1',
            status: 'delivered',
          }
        : assignment,
    )

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/adjustments`,
        {
          body: JSON.stringify({
            instruction: 'Add a section for operating cost assumptions.',
          }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async (_env, _ctx, input) => {
        launchInputs.push(input)

        return {
          launch: {
            payload: {
              mission: {
                status: 'queued',
              },
              run: {
                status: 'queued',
              },
            },
            runId: 'agent_run_adjustment_2',
          },
          ok: true,
        }
      },
      async () => {
        continuationCalled = true

        return {
          continuation: {
            goalId: goalId ?? null,
            mode: 'follow_up_turn',
            payload: {
              accepted: true,
              ingestedEvents: 0,
              status: 'running',
            },
            runId: 'agent_run_adjutant_1',
          },
          ok: true,
        }
      },
    )

    expect(response.status).toBe(202)
    await expect(response.json()).resolves.toEqual({
      accepted: true,
      adjustment: expect.objectContaining({
        assignmentId,
        continuationMode: 'new_goal_run',
        continuationRunId: 'agent_run_adjustment_2',
        goalId,
        sourceRunId: 'agent_run_adjutant_1',
        status: 'queued',
      }),
      assignment: expect.objectContaining({
        currentRunId: 'agent_run_adjustment_2',
        id: assignmentId,
        status: 'queued',
      }),
      launch: {
        mission: {
          status: 'queued',
        },
        run: {
          status: 'queued',
        },
      },
      runId: 'agent_run_adjustment_2',
    })
    expect(continuationCalled).toBe(false)
    expect(store.goals).toHaveLength(goalCount)
    expect(launchInputs).toEqual([
      {
        selector: expect.objectContaining({
          branchName: expect.stringContaining(
            `adjutant/${assignmentId}/adjustment-`,
          ),
          dispatchGoal: expect.stringContaining(
            'Add a section for operating cost assumptions.',
          ),
          goalId,
          repository: 'OpenAgentsInc/autopilot-omega@main',
        }),
        userId: 'github:operator',
      },
    ])
    expect(store.softwareOrders[0]).toMatchObject({
      current_run_id: 'agent_run_adjustment_2',
      status: 'agent_queued',
    })
    expect(store.sites[0]).toMatchObject({
      status: 'generating',
    })
  })

  test('blocks launch when Adjutant preflight has blockers', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const blockedPreflight: TestAutopilotPreflight = async () => ({
      checks: [
        {
          message: 'Reconnect the target provider account.',
          name: 'provider_account',
          status: 'blocked',
        },
      ],
      nextSafeAction: 'Resolve provider_account.',
      status: 'blocked',
      targetUser: null,
    })
    let launchCalled = false
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      blockedPreflight,
      undefined,
      async () => {
        launchCalled = true

        return {
          launch: {
            payload: {},
            runId: 'agent_run_should_not_launch',
          },
          ok: true,
        }
      },
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        error: 'adjutant_launch_blocked',
        nextSafeAction:
          'Resolve provider_account: Reconnect the target provider account.',
        status: 'blocked',
      }),
    )
    expect(launchCalled).toBe(false)
    expect(store.assignments[0]?.status).toBe('blocked')
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.launch_blocked',
      }),
    )
  })

  test('records typed pre-dispatch blocker responses from Omni launch', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    addFirstBatchPaymentPolicy(store, assignmentId)
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async () => ({
        ok: false,
        response: new Response(
          JSON.stringify({ error: 'github_write_connection_required' }),
          {
            headers: { 'content-type': 'application/json' },
            status: 409,
          },
        ),
      }),
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'github_write_connection_required',
    })
    expect(store.assignments[0]?.status).toBe('blocked')
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.launch_blocked',
      }),
    )
  })

  test('records Adjutant dispatch failures after a run is created', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    addFirstBatchPaymentPolicy(store, assignmentId)
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      undefined,
      undefined,
      async () => ({
        launch: {
          payload: {
            mission: {
              status: 'failed',
            },
            run: {
              status: 'failed',
            },
          },
          runId: 'agent_run_dispatch_failed',
        },
        ok: true,
      }),
    )

    expect(response.status).toBe(202)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        assignment: expect.objectContaining({
          currentRunId: 'agent_run_dispatch_failed',
          status: 'blocked',
        }),
        runId: 'agent_run_dispatch_failed',
      }),
    )
    expect(store.softwareOrders[0]).toMatchObject({
      current_run_id: 'agent_run_dispatch_failed',
      status: 'unavailable',
    })
    expect(store.siteEvents).toEqual([
      expect.objectContaining({
        actor_run_id: 'agent_run_dispatch_failed',
        type: 'adjutant.dispatch_failed',
      }),
    ])
    expect(store.events).toContainEqual(
      expect.objectContaining({
        event_type: 'adjutant.dispatch_failed',
        run_id: 'agent_run_dispatch_failed',
      }),
    )
  })

  test('preflights a public Site deployment with completed checks', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        id: assignmentId,
        status: 'preflight_pending',
      }),
      autopilotPreflight: expect.objectContaining({
        status: 'ok',
      }),
      checks: expect.arrayContaining([
        expect.objectContaining({ name: 'provider_account', status: 'ok' }),
        expect.objectContaining({ name: 'github_write', status: 'ok' }),
        expect.objectContaining({ name: 'shc_control', status: 'ok' }),
        expect.objectContaining({ name: 'runner_callback', status: 'ok' }),
        expect.objectContaining({ name: 'runner_backends', status: 'ok' }),
        expect.objectContaining({
          name: 'sites_launch_checklist',
          status: 'ok',
        }),
        expect.objectContaining({ name: 'task_packet', status: 'ok' }),
        expect.objectContaining({ name: 'commit_sha', status: 'ok' }),
        expect.objectContaining({ name: 'research_brief', status: 'warning' }),
        expect.objectContaining({ name: 'research_review', status: 'warning' }),
      ]),
      nextSafeAction: 'Create or launch the next Autopilot run.',
      status: 'warning',
    })
    expect(store.assignments[0]?.status).toBe('preflight_pending')
  })

  test('blocks research-required preflight when no approved research exists', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    const response = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )

    expect(response.status).toBe(200)
    const payload = await response.json()

    expect(payload).toEqual(
      expect.objectContaining({
        assignment: expect.objectContaining({
          id: assignmentId,
          status: 'blocked',
        }),
        checks: expect.arrayContaining([
          expect.objectContaining({
            name: 'research_policy',
            status: 'ok',
          }),
          expect.objectContaining({
            details: expect.objectContaining({
              nextAction: expect.stringContaining('Queue Exa enrichment'),
            }),
            name: 'research_required_gate',
            status: 'blocked',
          }),
        ]),
        nextSafeAction: expect.stringContaining('research_required_gate'),
        status: 'blocked',
      }),
    )
    expect(JSON.stringify(payload)).not.toContain('exa-test-secret')
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.preflight_blocked',
      }),
    )
  })

  test('blocks launch while required research job is queued', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    addFirstBatchPaymentPolicy(store, assignmentId)
    store.enrichmentJobs.push({
      archived_at: null,
      assignment_id: assignmentId,
      completed_at: null,
      created_at: '2026-06-05T00:00:00.000Z',
      enrichment_run_id: 'exa_enrichment_run_queued',
      error_code: null,
      error_summary: null,
      id: 'adjutant_enrichment_job_queued',
      refresh: 0,
      requested_by_user_id: 'github:operator',
      request_json: '{}',
      started_at: null,
      status: 'queued',
      trigger_kind: 'research_required',
      updated_at: '2026-06-05T00:00:00.000Z',
    })
    let launched = false
    const response = await runRouteWithEnvAndLaunch(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
      async () => {
        launched = true

        return {
          launch: { payload: {}, runId: 'agent_run_should_not_launch' },
          ok: true,
        }
      },
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({
            details: expect.objectContaining({
              enrichmentJobId: 'adjutant_enrichment_job_queued',
              enrichmentJobStatus: 'queued',
              nextAction: expect.stringContaining(
                'Wait for the enrichment job',
              ),
            }),
            name: 'research_required_gate',
            status: 'blocked',
          }),
        ]),
        error: 'adjutant_launch_blocked',
        status: 'blocked',
      }),
    )
    expect(launched).toBe(false)
    expect(store.assignments[0]?.current_run_id).toBeNull()
  })

  test('clears a failed current run so an assignment can be retried', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    store.runs = store.runs.map(run =>
      run.id === 'agent_run_adjutant_1' ? { ...run, status: 'failed' } : run,
    )
    store.assignments = store.assignments.map(assignment => ({
      ...assignment,
      current_run_id: 'agent_run_adjutant_1',
      status: 'blocked',
    }))

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/current-run/clear`,
        {
          body: JSON.stringify({
            reason:
              'Runner failed before producing artifacts; clear for retry with another provider account.',
            runId: 'agent_run_adjutant_1',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({
      assignment: expect.objectContaining({
        currentRunId: null,
        id: assignmentId,
        status: 'preflight_pending',
      }),
      clearedRun: {
        id: 'agent_run_adjutant_1',
        status: 'failed',
      },
    })
    expect(store.assignments[0]?.current_run_id).toBeNull()
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.current_run_cleared',
      }),
    )
  })

  test('does not clear an active current run', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    store.runs = store.runs.map(run =>
      run.id === 'agent_run_adjutant_1' ? { ...run, status: 'running' } : run,
    )
    store.assignments = store.assignments.map(assignment => ({
      ...assignment,
      current_run_id: 'agent_run_adjutant_1',
      status: 'running',
    }))

    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/current-run/clear`,
        {
          body: JSON.stringify({
            reason: 'Attempt to clear active run should fail.',
            runId: 'agent_run_adjutant_1',
          }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(409)
    await expect(response.json()).resolves.toEqual({
      error: 'current_run_not_terminal',
      reason:
        'Only completed, failed, or canceled current runs can be cleared.',
    })
    expect(store.assignments[0]?.current_run_id).toBe('agent_run_adjutant_1')
  })

  test('blocks required research when the latest enrichment job failed', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    store.enrichmentJobs.push({
      archived_at: null,
      assignment_id: assignmentId,
      completed_at: '2026-06-05T00:03:00.000Z',
      created_at: '2026-06-05T00:00:00.000Z',
      enrichment_run_id: 'exa_enrichment_run_failed',
      error_code: 'provider_timeout',
      error_summary: 'redacted provider timeout',
      id: 'adjutant_enrichment_job_failed',
      refresh: 1,
      requested_by_user_id: 'github:operator',
      request_json: '{}',
      started_at: '2026-06-05T00:01:00.000Z',
      status: 'failed',
      trigger_kind: 'research_required',
      updated_at: '2026-06-05T00:03:00.000Z',
    })
    const response = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({
            details: expect.objectContaining({
              enrichmentJobId: 'adjutant_enrichment_job_failed',
              enrichmentJobStatus: 'failed',
              redactedReason: 'provider_timeout',
            }),
            name: 'research_required_gate',
            status: 'blocked',
          }),
        ]),
        status: 'blocked',
      }),
    )
  })

  test('allows required research preflight after approved current research', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    attachApprovedOtecResearch(store, assignmentId)
    await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/task-packet`,
        {
          body: JSON.stringify({
            commitSha: '707c0302',
            taskSpecPath: 'docs/autopilot-tasks/adjutant-otec.md',
          }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )
    const response = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({
            name: 'research_required_gate',
            status: 'ok',
          }),
          expect.objectContaining({
            name: 'task_packet_freshness',
            status: 'ok',
          }),
        ]),
        status: 'ok',
      }),
    )
    expect(store.events).toContainEqual(
      expect.objectContaining({
        assignment_id: assignmentId,
        event_type: 'adjutant.preflight_ready',
      }),
    )
  })

  test('allows required research preflight with explicit operator bypass', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createGenerationAssignment(store)
    store.researchPolicies.push({
      actor_user_id: 'github:operator',
      archived_at: null,
      assignment_id: assignmentId,
      created_at: '2026-06-05T00:00:00.000Z',
      customer_safe_summary:
        'The operator approved this assignment using customer-provided public context.',
      policy_mode: 'research_bypassed_by_operator',
      reason:
        'The order already included enough public source context for this first pass.',
      source_authority_ref: 'order:software_order_otec',
      updated_at: '2026-06-05T00:00:00.000Z',
    })
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({
            details: expect.objectContaining({
              actorUserId: 'github:operator',
              updatedAt: '2026-06-05T00:00:00.000Z',
            }),
            name: 'research_required_gate',
            status: 'ok',
          }),
          expect.objectContaining({
            name: 'research_policy',
            status: 'ok',
          }),
        ]),
        status: 'warning',
      }),
    )
  })

  test('preflights and launches canonical OTEC assignment with approved enrichment metadata', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    addFirstBatchPaymentPolicy(store, assignmentId)
    attachApprovedOtecResearch(store, assignmentId)
    const preflight = await runRouteWithEnv(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
    )

    expect(preflight.status).toBe(200)
    await expect(preflight.json()).resolves.toEqual(
      expect.objectContaining({
        checks: expect.arrayContaining([
          expect.objectContaining({ name: 'exa_enrichment', status: 'ok' }),
          expect.objectContaining({ name: 'research_brief', status: 'ok' }),
          expect.objectContaining({ name: 'research_review', status: 'ok' }),
          expect.objectContaining({
            name: 'task_packet_freshness',
            status: 'warning',
          }),
        ]),
        status: 'warning',
      }),
    )

    const launchInputs: Array<Parameters<TestAutopilotLaunch>[2]> = []
    const launch = await runRouteWithEnvAndLaunch(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/launch`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      {
        EXA_API_KEY: 'exa-test-secret',
      },
      async (_env, _ctx, input) => {
        launchInputs.push(input)

        return {
          launch: {
            payload: {
              mission: {
                status: 'queued',
              },
              run: {
                status: 'queued',
              },
            },
            runId: 'agent_run_adjutant_otec',
          },
          ok: true,
        }
      },
    )

    expect(launch.status).toBe(202)
    await expect(launch.json()).resolves.toEqual(
      expect.objectContaining({
        preflight: expect.objectContaining({
          checks: expect.arrayContaining([
            expect.objectContaining({ name: 'exa_enrichment', status: 'ok' }),
            expect.objectContaining({ name: 'research_brief', status: 'ok' }),
            expect.objectContaining({ name: 'research_review', status: 'ok' }),
            expect.objectContaining({
              name: 'task_packet_freshness',
              status: 'warning',
            }),
          ]),
          status: 'warning',
        }),
        runId: 'agent_run_adjutant_otec',
      }),
    )
    expect(launchInputs).toHaveLength(1)
    expect(launchInputs[0]?.selector).toEqual(
      expect.objectContaining({
        researchBrief: expect.objectContaining({
          id: 'adjutant_research_brief_otec',
          sourceCount: 2,
          status: 'approved',
        }),
        researchBriefId: 'adjutant_research_brief_otec',
      }),
    )
    expect(String(launchInputs[0]?.selector.dispatchGoal)).toContain(
      'Research brief ID: adjutant_research_brief_otec',
    )
    expect(String(launchInputs[0]?.selector.pullRequestBody)).toContain(
      'Research brief: adjutant_research_brief_otec',
    )
    expect(String(launchInputs[0]?.selector.pullRequestBody)).not.toContain(
      'sk-',
    )
    expect(JSON.stringify(launchInputs)).not.toContain('exa-test-secret')
    expect(store.events).toContainEqual(
      expect.objectContaining({
        event_type: 'adjutant.run_queued',
        run_id: 'agent_run_adjutant_otec',
      }),
    )
  })

  test('inherits blocking Autopilot operator checks before launch', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const blockedPreflight: TestAutopilotPreflight = async () => ({
      checks: [
        {
          message: 'Reconnect the target provider account.',
          name: 'provider_account',
          status: 'blocked',
        },
      ],
      nextSafeAction: 'Resolve provider_account.',
      status: 'blocked',
      targetUser: null,
    })
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({ launchChecklist: completeLaunchChecklist }),
          method: 'POST',
        },
      ),
      blockedPreflight,
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        assignment: expect.objectContaining({
          id: assignmentId,
          status: 'blocked',
        }),
        nextSafeAction:
          'Resolve provider_account: Reconnect the target provider account.',
        status: 'blocked',
      }),
    )
    expect(store.assignments[0]?.status).toBe('blocked')
  })

  test('blocks public deployment preflight without the Sites launch checklist', async () => {
    const store = new OperatorAdjutantDbStore()
    const assignmentId = await createDeploymentAssignment(store)
    const response = await runRoute(
      adminSession,
      store,
      new Request(
        `https://openagents.com/api/operator/adjutant/assignments/${assignmentId}/preflight`,
        {
          body: JSON.stringify({}),
          method: 'POST',
        },
      ),
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual(
      expect.objectContaining({
        assignment: expect.objectContaining({
          id: assignmentId,
          status: 'blocked',
        }),
        checks: expect.arrayContaining([
          expect.objectContaining({
            name: 'sites_launch_checklist',
            status: 'blocked',
          }),
        ]),
        nextSafeAction:
          'Resolve sites_launch_checklist: Public Site deployment work requires the Sites launch checklist.',
        status: 'blocked',
      }),
    )
  })

  test('returns missing order and Site as typed errors', async () => {
    const missingOrderResponse = await runRoute(
      adminSession,
      new OperatorAdjutantDbStore(),
      new Request(
        'https://openagents.com/api/operator/adjutant/orders/software_order_missing/assign',
        { body: JSON.stringify({}), method: 'POST' },
      ),
    )
    const missingSiteResponse = await runRoute(
      adminSession,
      new OperatorAdjutantDbStore(),
      new Request(
        'https://openagents.com/api/operator/adjutant/sites/site_project_missing/assign',
        { body: JSON.stringify({}), method: 'POST' },
      ),
    )

    expect(missingOrderResponse.status).toBe(404)
    await expect(missingOrderResponse.json()).resolves.toEqual({
      error: 'software_order_not_found',
      softwareOrderId: 'software_order_missing',
    })
    expect(missingSiteResponse.status).toBe(404)
    await expect(missingSiteResponse.json()).resolves.toEqual({
      error: 'site_not_found',
      siteId: 'site_project_missing',
    })
  })

  test('returns duplicate, invalid visibility, and unsafe payload errors', async () => {
    const duplicateStore = new OperatorAdjutantDbStore()
    await runRoute(adminSession, duplicateStore, assignOrderRequest())
    const duplicateResponse = await runRoute(
      adminSession,
      duplicateStore,
      assignOrderRequest({ objective: 'Try again.' }),
    )
    const invalidVisibilityResponse = await runRoute(
      adminSession,
      new OperatorAdjutantDbStore(),
      assignOrderRequest({ visibility: 'customers' }),
    )
    const unsafeResponse = await runRoute(
      adminSession,
      new OperatorAdjutantDbStore(),
      assignOrderRequest({
        objective: 'Generate with OPENAI_API_KEY=sk-test-secret.',
      }),
    )

    expect(duplicateResponse.status).toBe(409)
    await expect(duplicateResponse.json()).resolves.toEqual({
      assignmentId: duplicateStore.assignments[0]?.id,
      error: 'active_assignment_exists',
      siteId: null,
      softwareOrderId: 'software_order_otec',
    })
    expect(invalidVisibilityResponse.status).toBe(400)
    await expect(invalidVisibilityResponse.json()).resolves.toEqual({
      error: 'invalid_visibility',
    })
    expect(unsafeResponse.status).toBe(400)
    await expect(unsafeResponse.json()).resolves.toEqual({
      error: 'unsafe_assignment_payload',
      reason: 'Autopilot assignment payload contains secret-shaped material.',
    })
  })
})
