import type { IdentityDb } from './identity-db'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentCredentialLookup,
  type AgentRegistrationRecord,
  type AgentRegistrationStore,
} from './agent-registration'
import { makeOnboardingRoutes } from './onboarding/routes'

type TestSession = Readonly<{ user: Readonly<{ userId: string }> }>
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
type StoredSiteRevision = Readonly<{
  activated_at: string | null
  build_status: string
  created_at: string
  deployment_id: string | null
  deployment_status: string | null
  deployment_url: string | null
  id: string
  metadata_json: string
  saved_at: string | null
  site_id: string
  source_commit_sha: string | null
}>
type StoredSiteFeedback = Readonly<{
  adjutant_adjustment_id: string | null
  adjutant_assignment_id: string | null
  archived_at: string | null
  author_user_id: string
  body: string
  created_at: string
  id: string
  site_deployment_id: string | null
  site_id: string | null
  site_version_id: string | null
  software_order_id: string
  status:
    | 'submitted'
    | 'queued'
    | 'running'
    | 'addressed'
    | 'closed'
    | 'rejected'
  updated_at: string
}>
type StoredFulfillmentArtifact = Readonly<{
  archived_at: string | null
  commit_sha: string | null
  created_at: string
  id: string
  kind:
    | 'pull_request'
    | 'branch'
    | 'commit'
    | 'diff'
    | 'preview'
    | 'notes'
    | 'attachment'
  repository_full_name: string | null
  software_order_id: string
  source_branch: string | null
  status:
    | 'draft'
    | 'customer_review_ready'
    | 'customer_accepted'
    | 'superseded'
    | 'rejected'
  summary: string
  target_branch: string | null
  title: string
  updated_at: string
  url: string | null
  visibility: 'private' | 'team' | 'public'
}>
type StoredAdjutantAssignment = Readonly<{
  archived_at: string | null
  assignment_kind: 'site_generation' | 'site_adjustment'
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  site_id: string | null
  software_order_id: string | null
  status: string
  updated_at: string
  visibility: 'private' | 'team' | 'public'
}>
type StoredAdjutantAdjustment = Readonly<{
  assignment_id: string
  continuation_mode: 'follow_up_turn' | 'new_goal_run' | null
  id: string
  instruction: string
  resulting_version_id: string | null
  site_id: string
  software_order_id: string
  source_run_id: string | null
  status: string
  visibility: 'private' | 'team' | 'public'
}>
type StoredTraceEvent = Readonly<{
  assignment_id?: string
  site_id?: string
  type: string
}>
type StoredReferralAttribution = Readonly<{
  archived_at: string | null
  capture_path: 'human' | 'agent'
  claimed_user_id: string | null
  created_at: string
  expires_at: string
  first_verified_at: string | null
  id: string
  policy_state: 'pending' | 'claimed' | 'expired'
  public_invite_ref: string | null
  public_source_ref: string
  referral_invite_id: string | null
  referral_source_id: string
  target: 'home' | 'order' | 'agent_claim'
  updated_at: string
}>
type StoredUserReferralAttribution = Readonly<{
  archived_at: string | null
  referral_attribution_id: string
  user_id: string
}>
type StoredOrderReferralAttribution = Readonly<{
  archived_at: string | null
  referral_attribution_id: string
  software_order_id: string
  user_id: string
}>

const executionContext = (): ExecutionContext => ({
  passThroughOnException: () => undefined,
  props: undefined,
  waitUntil: () => undefined,
})

type StoredOrder = Record<string, unknown>

class CustomerOrderStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: CustomerOrderDbStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (
      this.query.includes(
        'site_projects.active_version_id AS active_version_id',
      )
    ) {
      const [userId, orderId] = this.values
      const order = this.store.findOrder(userId, orderId)

      if (order === null) {
        return Promise.resolve(null)
      }

      return Promise.resolve({
        active_deployment_id: order.site_active_deployment_id ?? null,
        active_version_id: order.site_active_version_id ?? null,
        order_id: order.id,
        site_id: order.site_id ?? null,
      } as T)
    }

    if (
      this.query.includes('FROM site_revision_feedback') &&
      this.query.includes('adjutant_adjustment_id')
    ) {
      const [
        softwareOrderId,
        authorUserId,
        body,
        siteId,
        ,
        versionId,
        ,
        deploymentId,
      ] = this.values
      const duplicate = this.store.feedback.find(
        feedback =>
          feedback.software_order_id === softwareOrderId &&
          feedback.author_user_id === authorUserId &&
          feedback.body === body &&
          feedback.archived_at === null &&
          (feedback.status === 'submitted' ||
            feedback.status === 'queued' ||
            feedback.status === 'running') &&
          feedback.site_id === (siteId === null ? null : String(siteId)) &&
          feedback.site_version_id ===
            (versionId === null ? null : String(versionId)) &&
          feedback.site_deployment_id ===
            (deploymentId === null ? null : String(deploymentId)),
      )

      return Promise.resolve((duplicate as T | undefined) ?? null)
    }

    if (this.query.includes('FROM adjutant_assignments')) {
      const [softwareOrderId, siteId] = this.values
      const assignment = this.store.assignments.find(
        item =>
          item.archived_at === null &&
          item.status !== 'complete' &&
          item.status !== 'canceled' &&
          (item.software_order_id === softwareOrderId ||
            item.site_id === siteId),
      )

      return Promise.resolve((assignment as T | undefined) ?? null)
    }

    if (this.query.includes('FROM software_orders')) {
      if (this.store.failOrderRead) {
        return Promise.reject(new Error('software order read failed'))
      }

      if (this.query.includes('agent_idempotency_key = ?')) {
        const [userId, idempotencyKey] = this.values
        const order = this.store
          .listOrders()
          .find(
            item =>
              item.user_id === userId &&
              item.agent_idempotency_key === idempotencyKey &&
              item.archived_at !== '1',
          )

        return Promise.resolve(
          order === undefined ? null : ({ id: order.id } as T),
        )
      }

      if (
        this.store.failFeedbackRead &&
        this.query.includes('site_revision_feedback')
      ) {
        return Promise.reject(new Error('feedback table should not be read'))
      }

      const [userId, orderId] = this.values
      const order = this.store.findOrder(
        userId,
        this.query.includes('software_orders.id = ?') ? orderId : null,
      )

      if (order === null) {
        return Promise.resolve(null)
      }

      return Promise.resolve({
        triage_customer_safe_status: null,
        triage_customer_safe_summary: null,
        triage_next_action: null,
        ...order,
      } as T)
    }

    if (this.query.includes('FROM referral_attributions')) {
      const [attributionId] = this.values

      return Promise.resolve(
        (this.store.referralAttributions.find(
          attribution => attribution.id === attributionId,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM user_referral_attributions')) {
      const [userId] = this.values

      return Promise.resolve(
        (this.store.userReferralAttributions.find(
          attribution =>
            attribution.user_id === userId && attribution.archived_at === null,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM order_referral_attributions')) {
      const [orderId] = this.values

      return Promise.resolve(
        (this.store.orderReferralAttributions.find(
          attribution =>
            attribution.software_order_id === orderId &&
            attribution.archived_at === null,
        ) as T | undefined) ?? null,
      )
    }

    if (this.query.includes('FROM users')) {
      return Promise.resolve((this.store.onboarding as T | undefined) ?? null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO software_orders')) {
      const [
        id,
        userId,
        request,
        repositoryProvider,
        repositoryOwner,
        repositoryName,
        repositoryFullName,
        repositoryPrivate,
        repositoryDefaultBranch,
        repositoryHtmlUrl,
        publicWorkAcknowledgedAt,
        dataUseAcknowledgedAt,
        computePaymentAcknowledgedAt,
      ] = this.values
      const hasAgentIdempotencyColumn =
        this.query.includes('agent_idempotency_key')
      const agentIdempotencyKey = hasAgentIdempotencyColumn
        ? this.values[13]
        : null
      const createdAt = hasAgentIdempotencyColumn
        ? this.values[14]
        : this.values[13]
      const updatedAt = hasAgentIdempotencyColumn
        ? this.values[15]
        : this.values[14]

      const order = {
        id,
        user_id: userId,
        status: 'submitted',
        visibility: 'public',
        request,
        repository_provider: repositoryProvider,
        repository_owner: repositoryOwner,
        repository_name: repositoryName,
        repository_full_name: repositoryFullName,
        repository_private: repositoryPrivate,
        repository_default_branch: repositoryDefaultBranch,
        repository_html_url: repositoryHtmlUrl,
        site_id: null,
        site_status: null,
        site_active_url: null,
        site_active_version_id: null,
        site_active_deployment_id: null,
        site_latest_saved_version_id: null,
        site_latest_build_status: null,
        site_feedback_count: 0,
        site_open_feedback_count: 0,
        triage_customer_safe_status: null,
        triage_customer_safe_summary: null,
        triage_next_action: null,
        public_work_acknowledged_at: publicWorkAcknowledgedAt,
        data_use_acknowledged_at: dataUseAcknowledgedAt,
        compute_payment_acknowledged_at: computePaymentAcknowledgedAt,
        provider_account_required: 0,
        free_slice_cents: 5000,
        quote_cents: null,
        current_run_id: null,
        agent_started_at: null,
        agent_idempotency_key: agentIdempotencyKey,
        created_at: createdAt,
        updated_at: updatedAt,
      }
      this.store.order = order
      this.store.orders = [
        order,
        ...this.store.orders.filter(existing => existing.id !== id),
      ]

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO user_referral_attributions')) {
      const [userId, referralAttributionId] = this.values

      if (
        this.store.userReferralAttributions.every(
          attribution => attribution.user_id !== userId,
        )
      ) {
        this.store.userReferralAttributions.push({
          archived_at: null,
          referral_attribution_id: String(referralAttributionId),
          user_id: String(userId),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT OR IGNORE INTO order_referral_attributions')) {
      const [orderId, userId, referralAttributionId] = this.values

      if (
        this.store.orderReferralAttributions.every(
          attribution => attribution.software_order_id !== orderId,
        )
      ) {
        this.store.orderReferralAttributions.push({
          archived_at: null,
          referral_attribution_id: String(referralAttributionId),
          software_order_id: String(orderId),
          user_id: String(userId),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE referral_attributions')) {
      const [userId, firstVerifiedAt, updatedAt, attributionId] = this.values

      this.store.referralAttributions = this.store.referralAttributions.map(
        attribution =>
          attribution.id === attributionId &&
          attribution.policy_state === 'pending'
            ? {
                ...attribution,
                claimed_user_id:
                  attribution.claimed_user_id ?? String(userId),
                first_verified_at:
                  attribution.first_verified_at ?? String(firstVerifiedAt),
                policy_state: 'claimed',
                updated_at: String(updatedAt),
              }
            : attribution,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_revision_feedback')) {
      const [
        id,
        softwareOrderId,
        siteId,
        siteVersionId,
        siteDeploymentId,
        authorUserId,
        body,
        createdAt,
        updatedAt,
      ] = this.values

      this.store.feedback.unshift({
        adjutant_adjustment_id: null,
        adjutant_assignment_id: null,
        archived_at: null,
        author_user_id: String(authorUserId),
        body: String(body),
        created_at: String(createdAt),
        id: String(id),
        site_deployment_id:
          siteDeploymentId === null ? null : String(siteDeploymentId),
        site_id: siteId === null ? null : String(siteId),
        site_version_id: siteVersionId === null ? null : String(siteVersionId),
        software_order_id: String(softwareOrderId),
        status: 'submitted',
        updated_at: String(updatedAt),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_assignments')) {
      const [id, softwareOrderId, siteId, assignedByUserId, objective, , updatedAt] =
        this.values

      this.store.assignments.unshift({
        archived_at: null,
        assignment_kind: 'site_adjustment',
        current_run_id: null,
        goal_id: null,
        id: String(id),
        objective: String(objective),
        site_id: String(siteId),
        software_order_id: String(softwareOrderId),
        status: 'queued',
        updated_at: String(updatedAt),
        visibility: 'public',
      })
      this.store.events.push({
        assignment_id: String(id),
        type: `assigned:${String(assignedByUserId)}`,
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_adjustment_requests')) {
      const [
        id,
        assignmentId,
        softwareOrderId,
        siteId,
        ,
        ,
        instruction,
        continuationMode,
        sourceRunId,
      ] = this.values

      this.store.adjustments.unshift({
        assignment_id: String(assignmentId),
        continuation_mode:
          continuationMode === null
            ? null
            : (String(continuationMode) as 'follow_up_turn' | 'new_goal_run'),
        id: String(id),
        instruction: String(instruction),
        resulting_version_id: null,
        site_id: String(siteId),
        software_order_id: String(softwareOrderId),
        source_run_id: sourceRunId === null ? null : String(sourceRunId),
        status: 'queued',
        visibility: 'public',
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE site_revision_feedback')) {
      const [assignmentId, adjustmentId, updatedAt, feedbackId] = this.values

      this.store.feedback = this.store.feedback.map(feedback =>
        feedback.id === feedbackId
          ? {
              ...feedback,
              adjutant_adjustment_id: String(adjustmentId),
              adjutant_assignment_id: String(assignmentId),
              status: 'queued',
              updated_at: String(updatedAt),
            }
          : feedback,
      )

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('UPDATE software_orders')) {
      const [updatedAt, softwareOrderId] = this.values

      if (this.store.order?.id === softwareOrderId) {
        this.store.order = {
          ...this.store.order,
          status: 'agent_queued',
          updated_at: updatedAt,
        }
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO adjutant_assignment_events')) {
      const [, assignmentId, , , , , eventType] = this.values

      this.store.events.push({
        assignment_id: String(assignmentId),
        type: String(eventType),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    if (this.query.includes('INSERT INTO site_events')) {
      const [, siteId, eventType] = this.values

      this.store.events.push({
        site_id: String(siteId),
        type: String(eventType),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM site_versions\n')) {
      const [softwareOrderId, siteId] = this.values

      return Promise.resolve({
        meta: {} as D1Meta & Record<string, unknown>,
        results: this.store.revisions
          .filter(revision => revision.site_id === siteId)
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .map(revision => {
            const origin = this.store.feedback.find(feedback => {
              const adjustment =
                feedback.adjutant_adjustment_id === null
                  ? undefined
                  : this.store.adjustments.find(
                      item => item.id === feedback.adjutant_adjustment_id,
                    )

              return (
                feedback.software_order_id === softwareOrderId &&
                feedback.archived_at === null &&
                adjustment?.resulting_version_id === revision.id
              )
            })

            return {
              ...revision,
              origin_feedback_body: origin?.body ?? null,
              origin_feedback_created_at: origin?.created_at ?? null,
            }
          }) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM site_revision_feedback')) {
      const [softwareOrderId, authorUserId] = this.values

      return Promise.resolve({
        meta: {} as D1Meta & Record<string, unknown>,
        results: this.store.feedback
          .filter(
            feedback =>
              feedback.software_order_id === softwareOrderId &&
              feedback.author_user_id === authorUserId &&
              feedback.archived_at === null,
          )
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .slice(0, 50) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM order_fulfillment_artifacts')) {
      const [softwareOrderId] = this.values

      return Promise.resolve({
        meta: {} as D1Meta & Record<string, unknown>,
        results: this.store.artifacts
          .filter(
            artifact =>
              artifact.software_order_id === softwareOrderId &&
              artifact.visibility === 'public' &&
              artifact.archived_at === null,
          )
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          ) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM software_orders')) {
      const [userId] = this.values

      return Promise.resolve({
        meta: {} as D1Meta & Record<string, unknown>,
        results: this.store.listOrders()
          .filter(order => order.user_id === userId && order.archived_at !== '1')
          .sort((left, right) =>
            String(right.created_at).localeCompare(String(left.created_at)),
          )
          .map(order => ({
            triage_customer_safe_status: null,
            triage_customer_safe_summary: null,
            triage_next_action: null,
            ...order,
          })) as Array<T>,
        success: true,
      })
    }

    if (this.query.includes('FROM adjutant_usage_receipts')) {
      const [softwareOrderId, limit] = this.values

      return Promise.resolve({
        meta: {} as D1Meta & Record<string, unknown>,
        results: this.store.receipts
          .filter(
            receipt =>
              receipt.software_order_id === softwareOrderId &&
              receipt.visibility === 'public',
          )
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .slice(0, Number(limit ?? 50)) as Array<T>,
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

class CustomerOrderDbStore {
  adjustments: Array<StoredAdjutantAdjustment> = []
  artifacts: Array<StoredFulfillmentArtifact> = []
  assignments: Array<StoredAdjutantAssignment> = []
  events: Array<StoredTraceEvent> = []
  failFeedbackRead = false
  failOrderRead = false
  order: StoredOrder | null = null
  orders: Array<StoredOrder> = []
  feedback: Array<StoredSiteFeedback> = []
  receipts: Array<StoredUsageReceipt> = []
  referralAttributions: Array<StoredReferralAttribution> = []
  orderReferralAttributions: Array<StoredOrderReferralAttribution> = []
  revisions: Array<StoredSiteRevision> = []
  userReferralAttributions: Array<StoredUserReferralAttribution> = []
  onboarding: StoredOrder | null = {
    onboarding_completed_at: '2026-06-04T12:00:00.000Z',
    onboarding_goal: 'Add the public order status page.',
    onboarding_repository_provider: 'github',
    onboarding_repository_owner: 'OpenAgentsInc',
    onboarding_repository_name: 'autopilot-omega',
    onboarding_repository_full_name: 'OpenAgentsInc/autopilot-omega',
    onboarding_repository_private: 0,
    onboarding_repository_default_branch: 'main',
    onboarding_repository_html_url:
      'https://github.com/OpenAgentsInc/autopilot-omega',
  }

  findOrder(userId: unknown, orderId: unknown): StoredOrder | null {
    const userOrders = this.listOrders().filter(
      order => order.user_id === userId && order.archived_at !== '1',
    )

    if (typeof orderId === 'string' && orderId !== '') {
      return userOrders.find(order => order.id === orderId) ?? null
    }

    return (
      userOrders.sort((left, right) =>
        String(right.created_at).localeCompare(String(left.created_at)),
      )[0] ?? null
    )
  }

  listOrders(): ReadonlyArray<StoredOrder> {
    return this.orders.length === 0 && this.order !== null
      ? [this.order]
      : this.orders
  }
}

const emptyUsageSummary = {
  billingMode: 'public_beta_free',
  categories: [],
  totalCreditsChargedCents: 0,
  totalCreditsChargedFormatted: '$0.00',
}

const storedOrder = (overrides: StoredOrder = {}): StoredOrder => ({
  id: 'software_order_test',
  user_id: 'github:1',
  status: 'submitted',
  visibility: 'public',
  request: 'Add the public order status page.',
  repository_provider: null,
  repository_owner: null,
  repository_name: null,
  repository_full_name: null,
  repository_private: null,
  repository_default_branch: null,
  repository_html_url: null,
  site_id: null,
  site_status: null,
  site_active_url: null,
  site_active_version_id: null,
  site_active_deployment_id: null,
  site_latest_saved_version_id: null,
  site_latest_build_status: null,
  site_feedback_count: 0,
  site_open_feedback_count: 0,
  triage_customer_safe_status: null,
  triage_customer_safe_summary: null,
  triage_next_action: null,
  public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
  data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
  compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
  provider_account_required: 0,
  free_slice_cents: 5000,
  quote_cents: null,
  current_run_id: null,
  agent_started_at: null,
  agent_idempotency_key: null,
  created_at: '2026-06-04T12:01:00.000Z',
  updated_at: '2026-06-04T12:01:00.000Z',
  ...overrides,
})

const storedReferralAttribution = (
  overrides: Partial<StoredReferralAttribution> = {},
): StoredReferralAttribution => ({
  archived_at: null,
  capture_path: 'human',
  claimed_user_id: null,
  created_at: '2026-06-04T12:00:00.000Z',
  expires_at: '2026-07-04T12:00:00.000Z',
  first_verified_at: null,
  id: 'referral_attribution_otec',
  policy_state: 'pending',
  public_invite_ref: null,
  public_source_ref: 'site_ref_otec_ben',
  referral_invite_id: null,
  referral_source_id: 'site_referral_source_otec',
  target: 'order',
  updated_at: '2026-06-04T12:00:00.000Z',
  ...overrides,
})

const customerOrderDb = (store: CustomerOrderDbStore): D1Database => ({
  batch: async <T = unknown>(statements: Array<D1PreparedStatement>) => {
    const snapshot = {
      orderReferralAttributions: [...store.orderReferralAttributions],
      referralAttributions: [...store.referralAttributions],
      userReferralAttributions: [...store.userReferralAttributions],
    }

    try {
      const results: Array<D1Result> = []

      for (const statement of statements) {
        results.push(await statement.run())
      }

      return results as Array<D1Result<T>>
    } catch (error) {
      store.orderReferralAttributions = snapshot.orderReferralAttributions
      store.referralAttributions = snapshot.referralAttributions
      store.userReferralAttributions = snapshot.userReferralAttributions

      throw error
    }
  },
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new CustomerOrderStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

class CustomerOrderAgentStore implements AgentRegistrationStore {
  readonly touches: Array<
    Readonly<{ credentialId: string; lastUsedAt: string }>
  > = []

  constructor(private readonly lookup: AgentCredentialLookup | undefined) {}

  createAgentRegistration(_record: AgentRegistrationRecord): Promise<void> {
    return Promise.resolve()
  }

  findAgentByTokenHash(): Promise<AgentCredentialLookup | undefined> {
    return Promise.resolve(this.lookup)
  }

  touchAgentCredential(
    credentialId: string,
    lastUsedAt: string,
  ): Promise<void> {
    this.touches.push({ credentialId, lastUsedAt })

    return Promise.resolve()
  }

  updateAgentDisplayName(): Promise<number> {
    return Promise.resolve(0)
  }
}

const agentStoreForOwner = (
  ownerUserId: string,
  scopes: ReadonlyArray<string> = [
    'customer_orders.read',
    'customer_orders.write',
    'customer_orders.feedback',
  ],
) =>
  new CustomerOrderAgentStore({
    credentialId: 'agent_credential_customer_order_test',
    profileMetadataJson: JSON.stringify({
      customerOrderGrants: [
        {
          expiresAt: null,
          ownerUserId,
          scopes,
          status: 'active',
        },
      ],
    }),
    tokenPrefix: 'oa_agent_customer',
    user: {
      avatarUrl: null,
      createdAt: '2026-06-04T12:00:00.000Z',
      displayName: 'Customer Order API Agent',
      id: 'user_agent_customer_order',
      kind: 'agent',
      primaryEmail: null,
      status: 'active',
      updatedAt: '2026-06-04T12:00:00.000Z',
    },
  })

const makeRoutes = (
  session: TestSession | null,
  agentRegistrationStore?: AgentRegistrationStore,
) =>
  makeOnboardingRoutes({
    appendRefreshedSessionCookies: response => {
      response.headers.set('x-session-refreshed', 'true')

      return response
    },
    requireBrowserSession: () => Promise.resolve(session ?? undefined),
    requireUserBearerSession: () => Promise.resolve(undefined),
    ...(agentRegistrationStore === undefined ? {} : { agentRegistrationStore }),
    customerOrderRuntime: {
      makeAdjutantAdjustmentId: () => 'adjutant_adjustment_test',
      makeAdjutantAssignmentEventId: () =>
        'adjutant_assignment_event_test',
      makeAdjutantAssignmentId: () => 'adjutant_assignment_test',
      makeOrderId: () => 'software_order_test',
      makeSiteFeedbackId: () => 'site_feedback_test',
      makeSiteEventId: () => 'site_event_test',
      nowIso: () => '2026-06-04T12:01:00.000Z',
    },
  })

const identityDbOver = (db: D1Database): IdentityDb => ({
  batch: () => Promise.resolve(),
  query: async (sql, params = []) => {
    const row = await db
      .prepare(sql)
      .bind(...params)
      .first<Record<string, unknown>>()
    return row === null ? [] : [row]
  },
})

const runRoute = (
  session: TestSession | null,
  store: CustomerOrderDbStore,
  path = '/api/customer-orders/active',
  init?: RequestInit,
  agentRegistrationStore?: AgentRegistrationStore,
): Promise<Response> => {
  const route = makeRoutes(
    session,
    agentRegistrationStore,
  ).routeOnboardingRequest(
    new Request(`https://openagents.com${path}`, init),
    {
      // CFG-4 Domain 2 (#8519): the onboarding-source `users` read serves
      // from the Postgres identity handle — backed by the same scripted
      // store here (env test-override slot, same pattern as AUTH_KV).
      IDENTITY_DB: identityDbOver(customerOrderDb(store)),
      OPENAGENTS_DB: customerOrderDb(store),
    },
    executionContext(),
  )

  if (route === undefined) {
    throw new Error('route did not match')
  }

  return Effect.runPromise(route)
}

describe('customer order API routes', () => {
  test('returns unauthorized without a browser session', async () => {
    const response = await runRoute(null, new CustomerOrderDbStore())

    expect(response.status).toBe(401)
    await expect(response.json()).resolves.toEqual({ error: 'unauthorized' })
  })

  test('lets an approved agent token list only the granted owner customer orders', async () => {
    const store = new CustomerOrderDbStore()
    const agentStore = agentStoreForOwner('github:1')
    store.orders = [
      storedOrder({ id: 'software_order_allowed', user_id: 'github:1' }),
      storedOrder({ id: 'software_order_other', user_id: 'github:2' }),
    ]

    const response = await runRoute(
      null,
      store,
      '/api/customer-orders',
      {
        headers: {
          authorization: 'Bearer oa_agent_customer_order_test',
        },
      },
      agentStore,
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBeNull()
    await expect(response.json()).resolves.toMatchObject({
      orders: [expect.objectContaining({ id: 'software_order_allowed' })],
    })
    expect(agentStore.touches).toEqual([
      {
        credentialId: 'agent_credential_customer_order_test',
        lastUsedAt: '2026-06-04T12:01:00.000Z',
      },
    ])
  })

  test('denies an agent token without a customer order grant', async () => {
    const response = await runRoute(
      null,
      new CustomerOrderDbStore(),
      '/api/customer-orders',
      {
        headers: {
          authorization: 'Bearer oa_agent_customer_order_test',
        },
      },
      new CustomerOrderAgentStore({
        credentialId: 'agent_credential_no_grant',
        profileMetadataJson: '{}',
        tokenPrefix: 'oa_agent_customer',
        user: {
          avatarUrl: null,
          createdAt: '2026-06-04T12:00:00.000Z',
          displayName: 'No Grant Agent',
          id: 'user_agent_no_grant',
          kind: 'agent',
          primaryEmail: null,
          status: 'active',
          updatedAt: '2026-06-04T12:00:00.000Z',
        },
      }),
    )

    expect(response.status).toBe(403)
    await expect(response.json()).resolves.toMatchObject({
      error: 'forbidden',
    })
  })

  test('requires idempotency for agent-created customer orders', async () => {
    const response = await runRoute(
      null,
      new CustomerOrderDbStore(),
      '/api/customer-orders',
      {
        body: JSON.stringify({ request: 'Build an agent-requested Site.' }),
        headers: {
          authorization: 'Bearer oa_agent_customer_order_test',
        },
        method: 'POST',
      },
      agentStoreForOwner('github:1'),
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'idempotency_key_required',
      reason: 'Agent customer order creation requires Idempotency-Key.',
    })
  })

  test('creates customer orders idempotently for an approved agent token', async () => {
    const store = new CustomerOrderDbStore()
    const agentStore = agentStoreForOwner('github:1')
    const init = {
      body: JSON.stringify({ request: 'Build an agent-requested Site.' }),
      headers: {
        authorization: 'Bearer oa_agent_customer_order_test',
        'Idempotency-Key': 'agent-order-create-1',
      },
      method: 'POST',
    }

    const first = await runRoute(
      null,
      store,
      '/api/customer-orders',
      init,
      agentStore,
    )
    const second = await runRoute(
      null,
      store,
      '/api/customer-orders',
      init,
      agentStore,
    )

    expect(first.status).toBe(201)
    await expect(first.json()).resolves.toMatchObject({
      idempotent: false,
      order: {
        id: 'software_order_test',
        request: 'Build an agent-requested Site.',
      },
    })
    expect(second.status).toBe(200)
    await expect(second.json()).resolves.toMatchObject({
      idempotent: true,
      order: {
        id: 'software_order_test',
        request: 'Build an agent-requested Site.',
      },
    })
    expect(store.orders).toHaveLength(1)
    expect(store.orders[0]).toMatchObject({
      agent_idempotency_key: 'agent-order-create-1',
      user_id: 'github:1',
    })
  })

  test('creates an active public order from completed onboarding', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new CustomerOrderDbStore(),
    )

    expect(response.status).toBe(200)
    expect(response.headers.get('x-session-refreshed')).toBe('true')
    await expect(response.json()).resolves.toEqual({
      order: {
        id: 'software_order_test',
        status: 'submitted',
        visibility: 'public',
        request: 'Add the public order status page.',
        repository: {
          provider: 'github',
          owner: 'OpenAgentsInc',
          name: 'autopilot-omega',
          fullName: 'OpenAgentsInc/autopilot-omega',
          private: false,
          defaultBranch: 'main',
          htmlUrl: 'https://github.com/OpenAgentsInc/autopilot-omega',
        },
        site: null,
        triage: null,
        adjutant: {
          stage: 'queued',
          orderStatus: 'submitted',
          siteStatus: null,
          activeUrl: null,
          adjustmentStatus: null,
          claimState: {
            caveats: [
              'This claim is planned and should not be read as completed.',
            ],
            description:
              'Intended work or capability that is not yet evidenced.',
            evidenceRefs: ['order:software_order_test'],
            label: 'Planned',
            state: 'planned',
          },
          reviewNeeded: false,
          inputNeeded: false,
          nextAction: 'Autopilot is queued for this order.',
        },
        usageReceipts: [],
        usageSummary: emptyUsageSummary,
        publicWorkAcknowledgedAt: '2026-06-04T12:00:00.000Z',
        dataUseAcknowledgedAt: '2026-06-04T12:00:00.000Z',
        computePaymentAcknowledgedAt: '2026-06-04T12:00:00.000Z',
        providerAccountRequired: false,
        freeSliceCents: 5000,
        quoteCents: null,
        createdAt: '2026-06-04T12:01:00.000Z',
        updatedAt: '2026-06-04T12:01:00.000Z',
      },
    })
  })

  test('consumes pending referral attribution when active order is bootstrapped', async () => {
    const store = new CustomerOrderDbStore()
    store.referralAttributions.push(storedReferralAttribution())

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/active',
      {
        headers: {
          cookie:
            'oa_pending_referral_attribution=referral_attribution_otec',
        },
      },
    )
    const body = await response.text()

    expect(response.status).toBe(200)
    expect(response.headers.get('set-cookie')).toContain(
      'oa_pending_referral_attribution=',
    )
    expect(store.userReferralAttributions).toEqual([
      expect.objectContaining({
        referral_attribution_id: 'referral_attribution_otec',
        user_id: 'github:1',
      }),
    ])
    expect(store.orderReferralAttributions).toEqual([
      expect.objectContaining({
        referral_attribution_id: 'referral_attribution_otec',
        software_order_id: 'software_order_test',
        user_id: 'github:1',
      }),
    ])
    expect(store.referralAttributions[0]).toMatchObject({
      claimed_user_id: 'github:1',
      first_verified_at: '2026-06-04T12:01:00.000Z',
      policy_state: 'claimed',
    })
    expect(body).not.toContain('site_ref_otec_ben')
    expect(body).not.toContain('referral_attribution_otec')
    expect(body).not.toContain('token_hash')
  })

  test('links pending referral attribution when a new customer order is submitted', async () => {
    const store = new CustomerOrderDbStore()
    store.referralAttributions.push(storedReferralAttribution())

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders',
      {
        body: JSON.stringify({ request: 'Build a public Site.' }),
        headers: {
          cookie:
            'oa_pending_referral_attribution=referral_attribution_otec',
        },
        method: 'POST',
      },
    )
    const body = await response.text()

    expect(response.status).toBe(201)
    expect(response.headers.get('set-cookie')).toContain(
      'oa_pending_referral_attribution=',
    )
    expect(store.orderReferralAttributions).toEqual([
      expect.objectContaining({
        referral_attribution_id: 'referral_attribution_otec',
        software_order_id: 'software_order_test',
      }),
    ])
    expect(body).not.toContain('site_ref_otec_ben')
    expect(body).not.toContain('referral_attribution_otec')
  })

  test('lists all customer software workstreams newest first', async () => {
    const store = new CustomerOrderDbStore()
    store.orders = [
      storedOrder({
        id: 'software_order_old',
        request: 'Open a pull request for README cleanup.',
        created_at: '2026-06-04T12:01:00.000Z',
      }),
      storedOrder({
        id: 'software_order_new',
        request: 'Build an OTEC Site.',
        site_id: 'site_project_otec',
        site_status: 'approved',
        site_active_url: 'https://sites.openagents.com/otec',
        created_at: '2026-06-05T12:01:00.000Z',
      }),
      storedOrder({
        id: 'software_order_other_user',
        user_id: 'github:2',
        request: 'Do not include this.',
        created_at: '2026-06-06T12:01:00.000Z',
      }),
    ]

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders',
    )

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      orders: [
        {
          id: 'software_order_new',
          request: 'Build an OTEC Site.',
          site: {
            activeUrl: 'https://sites.openagents.com/otec',
            status: 'approved',
          },
        },
        {
          id: 'software_order_old',
          request: 'Open a pull request for README cleanup.',
          site: null,
        },
      ],
    })
  })

  test('creates a new customer software workstream from request text', async () => {
    const store = new CustomerOrderDbStore()

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders',
      {
        body: JSON.stringify({ request: 'Build a concise docs Site.' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(response.status).toBe(201)
    await expect(response.json()).resolves.toMatchObject({
      order: {
        id: 'software_order_test',
        request: 'Build a concise docs Site.',
        repository: {
          fullName: 'OpenAgentsInc/autopilot-omega',
        },
        status: 'submitted',
      },
    })
    expect(store.orders.map(order => order.request)).toEqual([
      'Build a concise docs Site.',
    ])
  })

  test('rejects empty customer software workstream requests', async () => {
    const response = await runRoute(
      { user: { userId: 'github:1' } },
      new CustomerOrderDbStore(),
      '/api/customer-orders',
      {
        body: JSON.stringify({ request: '   ' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(response.status).toBe(400)
    await expect(response.json()).resolves.toEqual({
      error: 'bad_request',
      reason: 'request is required',
    })
  })

  test('returns storage errors instead of hiding active orders as missing', async () => {
    const store = new CustomerOrderDbStore()
    store.failOrderRead = true

    const response = await runRoute({ user: { userId: 'github:1' } }, store)

    expect(response.status).toBe(500)
    await expect(response.json()).resolves.toEqual({ error: 'storage_error' })
  })

  test('does not require revision feedback storage to render active orders', async () => {
    const store = new CustomerOrderDbStore()
    store.failFeedbackRead = true

    const response = await runRoute({ user: { userId: 'github:1' } }, store)

    expect(response.status).toBe(200)
    const body = (await response.json()) as {
      order: { id: string }
    }
    expect(body.order.id).toBe('software_order_test')
  })

  test('returns only customer-safe site status and active URL', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_site',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_status: 'approved',
      site_active_url: 'https://sites.openagents.com/otec',
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: null,
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const response = await runRoute({ user: { userId: 'github:1' } }, store)

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      order: {
        id: 'software_order_site',
        adjutant: {
          activeUrl: 'https://sites.openagents.com/otec',
          adjustmentStatus: null,
          inputNeeded: false,
          nextAction: 'Open the live Site and send any adjustment request.',
          orderStatus: 'delivered',
          reviewNeeded: false,
          siteStatus: 'approved',
          stage: 'deployed',
        },
        repository: null,
        site: {
          status: 'approved',
          activeUrl: 'https://sites.openagents.com/otec',
        },
      },
    })
  })

  test('lists customer-owned site revisions with active deployment state', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_site',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_id: 'site_project_otec',
      site_status: 'approved',
      site_active_url: 'https://sites.openagents.com/otec',
      site_active_version_id: 'site_version_2',
      site_active_deployment_id: 'site_deployment_2',
      site_latest_saved_version_id: 'site_version_2',
      site_latest_build_status: 'saved',
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_internal',
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }
    store.revisions = [
      {
        activated_at: '2026-06-04T12:05:00.000Z',
        build_status: 'saved',
        created_at: '2026-06-04T12:04:00.000Z',
        deployment_id: 'site_deployment_2',
        deployment_status: 'active',
        deployment_url: 'https://sites.openagents.com/otec',
        id: 'site_version_2',
        metadata_json: JSON.stringify({
          customerReviewState: 'customer_review_ready',
          sha256: 'hash2',
        }),
        saved_at: '2026-06-04T12:04:30.000Z',
        site_id: 'site_project_otec',
        source_commit_sha: 'commit2',
      },
      {
        activated_at: '2026-06-04T12:03:00.000Z',
        build_status: 'saved',
        created_at: '2026-06-04T12:02:00.000Z',
        deployment_id: 'site_deployment_1',
        deployment_status: 'rolled_back',
        deployment_url: 'https://sites.openagents.com/otec',
        id: 'site_version_1',
        metadata_json: JSON.stringify({
          customerPromptSummary: 'Initial public OTEC explainer request.',
          sha256: 'sha256:hash1',
        }),
        saved_at: '2026-06-04T12:02:30.000Z',
        site_id: 'site_project_otec',
        source_commit_sha: 'commit1',
      },
    ]
    store.adjustments = [
      {
        assignment_id: 'adjutant_assignment_test',
        continuation_mode: 'follow_up_turn',
        id: 'adjutant_adjustment_test',
        instruction: 'Add diagrams and make the evidence more credible.',
        resulting_version_id: 'site_version_2',
        site_id: 'site_project_otec',
        software_order_id: 'software_order_site',
        source_run_id: 'agent_run_internal',
        status: 'completed',
        visibility: 'public',
      },
    ]
    store.feedback = [
      {
        adjutant_adjustment_id: 'adjutant_adjustment_test',
        adjutant_assignment_id: 'adjutant_assignment_test',
        archived_at: null,
        author_user_id: 'github:1',
        body: 'Add diagrams and make the evidence more credible.',
        created_at: '2026-06-04T12:03:30.000Z',
        id: 'site_feedback_revision_2',
        site_deployment_id: 'site_deployment_1',
        site_id: 'site_project_otec',
        site_version_id: 'site_version_1',
        software_order_id: 'software_order_site',
        status: 'addressed',
        updated_at: '2026-06-04T12:05:30.000Z',
      },
    ]

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-revisions',
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toEqual({
      revisions: [
        {
          active: true,
          activatedAt: '2026-06-04T12:05:00.000Z',
          buildStatus: 'saved',
          createdAt: '2026-06-04T12:04:00.000Z',
          deploymentId: 'site_deployment_2',
          deploymentStatus: 'active',
          id: 'site_version_2',
          originCreatedAt: '2026-06-04T12:03:30.000Z',
          originSummary: 'Add diagrams and make the evidence more credible.',
          reviewState: 'customer_review_ready',
          savedAt: '2026-06-04T12:04:30.000Z',
          siteId: 'site_project_otec',
          sourceCommitSha: 'commit2',
          sourceHash: 'sha256:hash2',
          url: 'https://sites.openagents.com/otec/versions/site_version_2',
        },
        {
          active: false,
          activatedAt: '2026-06-04T12:03:00.000Z',
          buildStatus: 'saved',
          createdAt: '2026-06-04T12:02:00.000Z',
          deploymentId: 'site_deployment_1',
          deploymentStatus: null,
          id: 'site_version_1',
          originCreatedAt: null,
          originSummary: 'Initial public OTEC explainer request.',
          reviewState: 'internal_draft',
          savedAt: '2026-06-04T12:02:30.000Z',
          siteId: 'site_project_otec',
          sourceCommitSha: 'commit1',
          sourceHash: 'sha256:hash1',
          url: 'https://sites.openagents.com/otec/versions/site_version_1',
        },
      ],
    })
    expect(JSON.stringify(body)).not.toContain('agent_run_internal')
  })

  test('submits and lists customer-owned site feedback', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_site',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_id: 'site_project_otec',
      site_status: 'approved',
      site_active_url: 'https://sites.openagents.com/otec',
      site_active_version_id: 'site_version_2',
      site_active_deployment_id: 'site_deployment_2',
      site_latest_saved_version_id: 'site_version_2',
      site_latest_build_status: 'saved',
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: null,
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const submitResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
      {
        body: JSON.stringify({
          body: 'Please rebuild the hero so it looks credible.',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(submitResponse.status).toBe(201)
    await expect(submitResponse.json()).resolves.toEqual({
      feedback: {
        body: 'Please rebuild the hero so it looks credible.',
        createdAt: '2026-06-04T12:01:00.000Z',
        deploymentId: 'site_deployment_2',
        id: 'site_feedback_test',
        orderId: 'software_order_site',
        siteId: 'site_project_otec',
        status: 'queued',
        updatedAt: '2026-06-04T12:01:00.000Z',
        versionId: 'site_version_2',
      },
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.adjustments).toMatchObject([
      {
        assignment_id: 'adjutant_assignment_test',
        continuation_mode: 'new_goal_run',
        site_id: 'site_project_otec',
        software_order_id: 'software_order_site',
        status: 'queued',
        visibility: 'public',
      },
    ])
    expect(store.feedback[0]).toMatchObject({
      adjutant_adjustment_id: 'adjutant_adjustment_test',
      adjutant_assignment_id: 'adjutant_assignment_test',
      status: 'queued',
    })
    expect(store.events).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'adjutant.customer_feedback_queued',
        }),
      ]),
    )

    const listResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
    )

    expect(listResponse.status).toBe(200)
    await expect(listResponse.json()).resolves.toMatchObject({
      feedback: [
        {
          body: 'Please rebuild the hero so it looks credible.',
          id: 'site_feedback_test',
          status: 'queued',
        },
      ],
    })

    const retryResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
      {
        body: JSON.stringify({
          body: 'Please rebuild the hero so it looks credible.',
        }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(retryResponse.status).toBe(201)
    await expect(retryResponse.json()).resolves.toMatchObject({
      feedback: {
        id: 'site_feedback_test',
        status: 'queued',
      },
    })
    expect(store.adjustments).toHaveLength(1)
  })

  test('queues customer feedback onto an active assignment run', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_site',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_id: 'site_project_otec',
      site_status: 'approved',
      site_active_url: 'https://sites.openagents.com/otec',
      site_active_version_id: 'site_version_2',
      site_active_deployment_id: 'site_deployment_2',
      site_latest_saved_version_id: 'site_version_2',
      site_latest_build_status: 'saved',
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_internal',
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }
    store.assignments = [
      {
        archived_at: null,
        assignment_kind: 'site_generation',
        current_run_id: 'agent_run_active',
        goal_id: 'agent_goal_site',
        id: 'adjutant_assignment_active',
        objective: 'Build the OTEC Site.',
        site_id: 'site_project_otec',
        software_order_id: 'software_order_site',
        status: 'running',
        updated_at: '2026-06-04T12:02:00.000Z',
        visibility: 'public',
      },
    ]

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
      {
        body: JSON.stringify({ body: 'Make the diagram legible on mobile.' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(response.status).toBe(201)
    await expect(response.json()).resolves.toMatchObject({
      feedback: {
        status: 'queued',
      },
    })
    expect(store.assignments).toHaveLength(1)
    expect(store.adjustments).toMatchObject([
      {
        assignment_id: 'adjutant_assignment_active',
        continuation_mode: 'follow_up_turn',
        source_run_id: 'agent_run_active',
        status: 'queued',
      },
    ])
  })

  test('handles feedback and revisions when an owned order has no site yet', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_no_site',
      user_id: 'github:1',
      status: 'submitted',
      visibility: 'public',
      request: 'Build a site.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_id: null,
      site_status: null,
      site_active_url: null,
      site_active_version_id: null,
      site_active_deployment_id: null,
      site_latest_saved_version_id: null,
      site_latest_build_status: null,
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: null,
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const revisionsResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_no_site/site-revisions',
    )

    expect(revisionsResponse.status).toBe(200)
    await expect(revisionsResponse.json()).resolves.toEqual({ revisions: [] })

    const feedbackResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_no_site/site-feedback',
      {
        body: JSON.stringify({ body: 'Use a maritime visual direction.' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(feedbackResponse.status).toBe(201)
    await expect(feedbackResponse.json()).resolves.toMatchObject({
      feedback: {
        deploymentId: null,
        siteId: null,
        versionId: null,
      },
    })
  })

  test('lists customer-owned public fulfillment artifacts for non-Sites orders', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_codebase',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Open a PR against my existing app.',
      repository_provider: 'github',
      repository_owner: 'customer',
      repository_name: 'app',
      repository_full_name: 'customer/app',
      repository_private: 0,
      repository_default_branch: 'main',
      repository_html_url: 'https://github.com/customer/app',
      site_id: null,
      site_status: null,
      site_active_url: null,
      site_active_version_id: null,
      site_active_deployment_id: null,
      site_latest_saved_version_id: null,
      site_latest_build_status: null,
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_private',
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }
    store.artifacts = [
      {
        archived_at: null,
        commit_sha: 'abc123',
        created_at: '2026-06-05T12:00:00.000Z',
        id: 'fulfillment_artifact_pr',
        kind: 'pull_request',
        repository_full_name: 'customer/app',
        software_order_id: 'software_order_codebase',
        source_branch: 'openagents/software-order-codebase',
        status: 'customer_review_ready',
        summary: 'Opened a review-ready pull request for the requested change.',
        target_branch: 'main',
        title: 'Review PR',
        updated_at: '2026-06-05T12:00:00.000Z',
        url: 'https://github.com/customer/app/pull/7',
        visibility: 'public',
      },
      {
        archived_at: null,
        commit_sha: null,
        created_at: '2026-06-05T12:01:00.000Z',
        id: 'fulfillment_artifact_private',
        kind: 'notes',
        repository_full_name: null,
        software_order_id: 'software_order_codebase',
        source_branch: null,
        status: 'customer_review_ready',
        summary: 'Internal runner notes.',
        target_branch: null,
        title: 'Internal notes',
        updated_at: '2026-06-05T12:01:00.000Z',
        url: null,
        visibility: 'team',
      },
      {
        archived_at: null,
        commit_sha: null,
        created_at: '2026-06-05T12:02:00.000Z',
        id: 'fulfillment_artifact_other_order',
        kind: 'diff',
        repository_full_name: 'customer/app',
        software_order_id: 'software_order_other',
        source_branch: null,
        status: 'customer_review_ready',
        summary: 'Other order diff.',
        target_branch: null,
        title: 'Other diff',
        updated_at: '2026-06-05T12:02:00.000Z',
        url: 'https://example.com/private-diff',
        visibility: 'public',
      },
    ]

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_codebase/fulfillment-artifacts',
    )
    const body = await response.json()

    expect(response.status).toBe(200)
    expect(body).toEqual({
      artifacts: [
        {
          commitSha: 'abc123',
          createdAt: '2026-06-05T12:00:00.000Z',
          id: 'fulfillment_artifact_pr',
          kind: 'pull_request',
          orderId: 'software_order_codebase',
          repositoryFullName: 'customer/app',
          sourceBranch: 'openagents/software-order-codebase',
          status: 'customer_review_ready',
          summary:
            'Opened a review-ready pull request for the requested change.',
          targetBranch: 'main',
          title: 'Review PR',
          updatedAt: '2026-06-05T12:00:00.000Z',
          url: 'https://github.com/customer/app/pull/7',
        },
      ],
    })
    expect(JSON.stringify(body)).not.toContain('agent_run_private')

    const otherUserResponse = await runRoute(
      { user: { userId: 'github:2' } },
      store,
      '/api/customer-orders/software_order_codebase/fulfillment-artifacts',
    )

    expect(otherUserResponse.status).toBe(404)
  })

  test('rejects malformed and non-owned site feedback requests', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_site',
      user_id: 'github:1',
      status: 'delivered',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_id: 'site_project_otec',
      site_status: 'approved',
      site_active_url: 'https://sites.openagents.com/otec',
      site_active_version_id: 'site_version_2',
      site_active_deployment_id: 'site_deployment_2',
      site_latest_saved_version_id: 'site_version_2',
      site_latest_build_status: 'saved',
      site_feedback_count: 0,
      site_open_feedback_count: 0,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: null,
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const badResponse = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
      {
        body: JSON.stringify({ body: '   ' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(badResponse.status).toBe(400)

    const otherUserResponse = await runRoute(
      { user: { userId: 'github:2' } },
      store,
      '/api/customer-orders/software_order_site/site-feedback',
      {
        body: JSON.stringify({ body: 'Change the hero.' }),
        headers: { 'content-type': 'application/json' },
        method: 'POST',
      },
    )

    expect(otherUserResponse.status).toBe(404)
    await expect(otherUserResponse.json()).resolves.toEqual({
      error: 'customer_order_not_found',
    })
  })

  test('returns customer-safe triage projection without operator-only fields', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_triage',
      user_id: 'github:1',
      status: 'submitted',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_status: null,
      site_active_url: null,
      triage_customer_safe_status: 'scoping',
      triage_customer_safe_summary:
        'OpenAgents is preparing this website order for the first overnight Sites batch.',
      triage_next_action:
        'Run a compatibility check before preparing the first saved version.',
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_internal',
      agent_started_at: null,
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const response = await runRoute({ user: { userId: 'github:1' } }, store)
    const body = (await response.json()) as {
      order: Record<string, unknown> | null
    }

    expect(response.status).toBe(200)
    expect(body.order).toMatchObject({
      id: 'software_order_triage',
      triage: {
        status: 'scoping',
        summary:
          'OpenAgents is preparing this website order for the first overnight Sites batch.',
        nextAction:
          'Run a compatibility check before preparing the first saved version.',
      },
    })
    expect(body.order).not.toHaveProperty('operatorPriority')
    expect(body.order).not.toHaveProperty('holdReason')
    expect(JSON.stringify(body.order)).not.toContain('agent_run_internal')
  })

  test('surfaces customer-safe adjustment progress on active orders', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_adjusting',
      user_id: 'github:1',
      status: 'agent_running',
      visibility: 'public',
      request: 'Launch a public OTEC explainer.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_status: 'generating',
      site_active_url: 'https://sites.openagents.com/otec',
      latest_adjustment_status: 'running',
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_internal',
      agent_started_at: '2026-06-04T12:01:00.000Z',
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }

    const response = await runRoute({ user: { userId: 'github:1' } }, store)

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toMatchObject({
      order: {
        id: 'software_order_adjusting',
        adjutant: {
          activeUrl: 'https://sites.openagents.com/otec',
          adjustmentStatus: 'running',
          inputNeeded: false,
          nextAction: 'Autopilot is applying the requested Site adjustment.',
          orderStatus: 'agent_running',
          reviewNeeded: false,
          siteStatus: 'generating',
          stage: 'running',
        },
      },
    })
  })

  test('returns customer-owned order detail without raw runner refs', async () => {
    const store = new CustomerOrderDbStore()
    store.order = {
      id: 'software_order_detail',
      user_id: 'github:1',
      status: 'agent_running',
      visibility: 'public',
      request: 'Build a public order page.',
      repository_provider: null,
      repository_owner: null,
      repository_name: null,
      repository_full_name: null,
      repository_private: null,
      repository_default_branch: null,
      repository_html_url: null,
      site_status: 'generating',
      site_active_url: null,
      public_work_acknowledged_at: '2026-06-04T12:00:00.000Z',
      data_use_acknowledged_at: '2026-06-04T12:00:00.000Z',
      compute_payment_acknowledged_at: '2026-06-04T12:00:00.000Z',
      provider_account_required: 0,
      free_slice_cents: 5000,
      quote_cents: null,
      current_run_id: 'agent_run_internal',
      agent_started_at: '2026-06-04T12:01:00.000Z',
      created_at: '2026-06-04T12:01:00.000Z',
      updated_at: '2026-06-04T12:02:00.000Z',
    }
    store.receipts = [
      {
        adjustment_id: null,
        assignment_id: 'adjutant_assignment_public',
        billing_ledger_entry_id: null,
        billing_mode: 'public_beta_free',
        category: 'generation',
        created_at: '2026-06-04T12:03:00.000Z',
        credits_charged_cents: 0,
        currency: 'USD',
        id: 'adjutant_usage_receipt_generation',
        public_receipt_json: JSON.stringify({
          billingNote: 'Public beta Site generation is free.',
        }),
        quantity: 1,
        run_id: 'agent_run_internal',
        site_id: 'site_project_detail',
        software_order_id: 'software_order_detail',
        summary: 'Autopilot Site generation run was queued.',
        team_receipt_json: JSON.stringify({
          runId: 'agent_run_internal',
        }),
        unit: 'run',
        visibility: 'public',
      },
      {
        adjustment_id: null,
        assignment_id: 'adjutant_assignment_private',
        billing_ledger_entry_id: null,
        billing_mode: 'public_beta_free',
        category: 'hosting',
        created_at: '2026-06-04T12:04:00.000Z',
        credits_charged_cents: 0,
        currency: 'USD',
        id: 'adjutant_usage_receipt_private',
        public_receipt_json: '{}',
        quantity: 1,
        run_id: 'agent_run_private',
        site_id: 'site_project_detail',
        software_order_id: 'software_order_detail',
        summary: 'Private receipt.',
        team_receipt_json: '{}',
        unit: 'deployment',
        visibility: 'team',
      },
    ]

    const response = await runRoute(
      { user: { userId: 'github:1' } },
      store,
      '/api/customer-orders/software_order_detail',
    )
    const body = (await response.json()) as {
      order: Record<string, unknown> | null
    }

    expect(response.status).toBe(200)
    expect(body.order).toMatchObject({
      id: 'software_order_detail',
      adjutant: {
        activeUrl: null,
        adjustmentStatus: null,
        inputNeeded: false,
        nextAction: 'Autopilot is building the Site version.',
        orderStatus: 'agent_running',
        reviewNeeded: false,
        siteStatus: 'generating',
        stage: 'running',
      },
      usageReceipts: [
        {
          billingMode: 'public_beta_free',
          category: 'generation',
          creditsChargedCents: 0,
          creditsChargedFormatted: '$0.00',
          details: {
            billingNote: 'Public beta Site generation is free.',
          },
          id: 'adjutant_usage_receipt_generation',
          quantity: 1,
          summary: 'Autopilot Site generation run was queued.',
          unit: 'run',
        },
      ],
      usageSummary: {
        billingMode: 'public_beta_free',
        categories: [
          {
            category: 'generation',
            creditsChargedCents: 0,
            creditsChargedFormatted: '$0.00',
            quantity: 1,
            receiptCount: 1,
            unit: 'run',
          },
        ],
        totalCreditsChargedCents: 0,
        totalCreditsChargedFormatted: '$0.00',
      },
    })
    expect(body.order).not.toHaveProperty('currentRunId')
    expect(body.order).not.toHaveProperty('agentStartedAt')
    expect(JSON.stringify(body.order)).not.toContain('agent_run_internal')

    const otherUserResponse = await runRoute(
      { user: { userId: 'github:2' } },
      store,
      '/api/customer-orders/software_order_detail',
    )

    await expect(otherUserResponse.json()).resolves.toEqual({ order: null })
  })

  test('returns no active order before onboarding is complete', async () => {
    const store = new CustomerOrderDbStore()
    store.onboarding = {
      ...store.onboarding,
      onboarding_completed_at: null,
    }

    const response = await runRoute({ user: { userId: 'github:1' } }, store)

    expect(response.status).toBe(200)
    await expect(response.json()).resolves.toEqual({ order: null })
  })
})
