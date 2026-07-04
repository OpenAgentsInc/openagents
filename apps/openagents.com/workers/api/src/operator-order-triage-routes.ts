import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  type AdjutantAssignment,
  type AdjutantAssignmentError,
  AdjutantAssignmentService,
} from './adjutant-assignments'
import { CustomerOrderStatus } from './customer-orders'
import {
  type FirstBatchPaymentPolicy,
  type FirstBatchPaymentPolicyError,
  FirstBatchPaymentPolicyMode,
  readFirstBatchPaymentGate,
  systemFirstBatchPaymentPolicyRuntime,
  upsertFirstBatchPaymentPolicy,
} from './first-batch-payment-policies'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonRecord } from './json-boundary'
// KS-8.12 (#8323): sites writes ride the dual-write mirror seam — the
// mirroring database is a passthrough for non-scoped statements and
// degrades to the raw D1 handle when no KHALA_SYNC_DB binding exists.
import { sitesContentDatabaseForEnv as openAgentsDatabase } from './sites-content-store'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import {
  type AutopilotSiteError,
  type AutopilotSiteProject,
  AutopilotSitesService,
} from './sites'

type OperatorOrderTriageEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>
type HttpResponse = globalThis.Response

type OperatorOrderTriageSession = Readonly<{
  user: Readonly<{
    email: string
    userId: string
  }>
}>

type OperatorOrderTriageRouteDependencies<
  Session extends OperatorOrderTriageSession,
  Bindings extends OperatorOrderTriageEnv,
> = Readonly<{
  appendRefreshedSessionCookies: (
    response: HttpResponse,
    session: Session,
  ) => HttpResponse
  isOpenAgentsAdminEmail: (email: string) => boolean
  requireAdminApiToken?: (
    request: Request,
    env: Bindings,
  ) => Promise<boolean>
  requireBrowserSession: (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) => Promise<Session | undefined>
}>

export const OrderTriageClassification = S.Literals([
  'runnable_site',
  'runnable_general_autopilot',
  'needs_clarification',
  'smoke_or_test',
  'legal_sensitive_policy_review',
  'unavailable_or_declined',
])
export type OrderTriageClassification = typeof OrderTriageClassification.Type

export class UpsertOrderTriageRequest extends S.Class<UpsertOrderTriageRequest>(
  'UpsertOrderTriageRequest',
)({
  classification: OrderTriageClassification,
  operatorPriority: S.Number,
  firstBatchEligible: S.Boolean,
  holdReason: S.optionalKey(S.NullOr(S.String)),
  nextAction: S.String,
  customerSafeStatus: S.String,
  customerSafeSummary: S.String,
  orderStatus: S.optionalKey(CustomerOrderStatus),
}) {}

export class FirstBatchAssignmentRequest extends S.Class<FirstBatchAssignmentRequest>(
  'FirstBatchAssignmentRequest',
)({
  dryRun: S.optionalKey(S.Boolean),
  limit: S.optionalKey(S.Number),
  softwareOrderIds: S.optionalKey(S.Array(S.String)),
}) {}

export class PrepareOrderFulfillmentRequest extends S.Class<PrepareOrderFulfillmentRequest>(
  'PrepareOrderFulfillmentRequest',
)({
  dryRun: S.optionalKey(S.Boolean),
}) {}

export class ApplyFirstBatchPaymentPolicyRequest extends S.Class<ApplyFirstBatchPaymentPolicyRequest>(
  'ApplyFirstBatchPaymentPolicyRequest',
)({
  customerSafeSummary: S.optionalKey(S.String),
  policyMode: S.optionalKey(FirstBatchPaymentPolicyMode),
  reason: S.optionalKey(S.String),
  softwareOrderIds: S.Array(S.String),
}) {}

export type OperatorOrderTriageOrder = Readonly<{
  id: string
  userId: string
  userDisplayName: string | null
  userEmail: string | null
  status: CustomerOrderStatus
  visibility: string
  request: string
  repositoryFullName: string | null
  currentRunId: string | null
  siteProjectId: string | null
  siteTitle: string | null
  siteSlug: string | null
  siteStatus: string | null
  latestAssignmentId: string | null
  latestAssignmentStatus: string | null
  latestAssignmentKind: string | null
  createdAt: string
  updatedAt: string
}>

export type OperatorOrderTriageRecord = Readonly<{
  id: string
  softwareOrderId: string
  classification: OrderTriageClassification
  operatorPriority: number
  firstBatchEligible: boolean
  overnightLaunchEligible: boolean
  holdReason: string | null
  nextAction: string
  customerSafeStatus: string
  customerSafeSummary: string
  reviewerUserId: string | null
  reviewedAt: string | null
  createdAt: string
  updatedAt: string
  order: OperatorOrderTriageOrder
}>

type OrderTriageRow = Readonly<{
  id: string
  software_order_id: string
  classification: OrderTriageClassification
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
  order_id: string
  order_user_id: string
  user_display_name: string | null
  user_email: string | null
  order_status: CustomerOrderStatus
  order_visibility: string
  order_request: string
  repository_full_name: string | null
  current_run_id: string | null
  site_project_id: string | null
  site_title: string | null
  site_slug: string | null
  site_status: string | null
  latest_assignment_id: string | null
  latest_assignment_status: string | null
  latest_assignment_kind: string | null
  order_created_at: string
  order_updated_at: string
}>

type OrderExistsRow = Readonly<{
  id: string
}>

export type OrderTriageRuntime = Readonly<{
  makeEventId: () => string
  makeRecordId: () => string
  nowIso: () => string
}>

export const systemOrderTriageRuntime: OrderTriageRuntime = {
  makeEventId: () => compactRandomId('order_triage_event'),
  makeRecordId: () => compactRandomId('order_triage'),
  nowIso: currentIsoTimestamp,
}

export class OrderTriageStorageError extends S.TaggedErrorClass<OrderTriageStorageError>()(
  'OrderTriageStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class OrderTriageSoftwareOrderNotFound extends S.TaggedErrorClass<OrderTriageSoftwareOrderNotFound>()(
  'OrderTriageSoftwareOrderNotFound',
  {
    softwareOrderId: S.String,
  },
) {}

class OperatorOrderTriageUnauthorized extends S.TaggedErrorClass<OperatorOrderTriageUnauthorized>()(
  'OperatorOrderTriageUnauthorized',
  {},
) {}

class OperatorOrderTriageForbidden extends S.TaggedErrorClass<OperatorOrderTriageForbidden>()(
  'OperatorOrderTriageForbidden',
  {},
) {}

class OperatorOrderTriageBadRequest extends S.TaggedErrorClass<OperatorOrderTriageBadRequest>()(
  'OperatorOrderTriageBadRequest',
  {
    reason: S.String,
  },
) {}

class OperatorOrderTriageSessionError extends S.TaggedErrorClass<OperatorOrderTriageSessionError>()(
  'OperatorOrderTriageSessionError',
  {
    error: S.Defect,
  },
) {}

type OperatorOrderTriageRouteError =
  | AdjutantAssignmentError
  | AutopilotSiteError
  | FirstBatchPaymentPolicyError
  | OperatorOrderTriageBadRequest
  | OperatorOrderTriageForbidden
  | OperatorOrderTriageSessionError
  | OperatorOrderTriageUnauthorized
  | OrderTriageSoftwareOrderNotFound
  | OrderTriageStorageError

type OrderTriageError =
  | OrderTriageSoftwareOrderNotFound
  | OrderTriageStorageError

type FirstBatchAssignmentDecision =
  | 'already_assigned'
  | 'created_assignment'
  | 'held'
  | 'would_create_assignment'

type FirstBatchAssignmentResult = Readonly<{
  assignment: AdjutantAssignment | null
  assignmentId: string | null
  classification: OrderTriageClassification
  customerSafeStatus: string
  customerSafeSummary: string
  decision: FirstBatchAssignmentDecision
  dryRun: boolean
  firstBatchEligible: boolean
  holdReason: string | null
  nextAction: string
  orderStatus: CustomerOrderStatus
  receiptId: string | null
  site: AutopilotSiteProject | null
  siteId: string | null
  softwareOrderId: string
  summary: string
}>

type FirstBatchAssignmentSummary = Readonly<{
  alreadyAssigned: number
  created: number
  dryRun: boolean
  held: number
  total: number
  wouldCreate: number
}>

type FirstBatchPaymentPolicyApplyResult = Readonly<{
  assignmentId: string | null
  classification: OrderTriageClassification
  firstBatchEligible: boolean
  overnightLaunchEligible: boolean
  paymentPolicy: FirstBatchPaymentPolicy
  siteId: string | null
  softwareOrderId: string
}>

type FirstBatchMonitorState =
  | 'blocked'
  | 'deployed'
  | 'delivered'
  | 'failed'
  | 'held'
  | 'not_yet_assigned'
  | 'preflight_ready'
  | 'queued'
  | 'review_ready'
  | 'running'
  | 'waiting_for_input'

type MonitorAssignmentRow = Readonly<{
  agent_id: string
  assigned_by_user_id: string | null
  assignment_kind: string
  blocked_at: string | null
  commit_sha: string | null
  completed_at: string | null
  created_at: string
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  site_id: string | null
  software_order_id: string | null
  status: string
  task_spec_path: string | null
  updated_at: string
  visibility: string
}>

type MonitorLeaseRow = Readonly<{
  account_health: string | null
  account_label: string | null
  account_status: string | null
  cooldown_until: string | null
  expires_at: string
  failure_class: string | null
  last_touched_at: string | null
  lease_ref: string
  low_credit_flag: number | null
  provider_account_ref: string
  reauth_required_reason: string | null
  requested_action: string
  started_at: string
  status: string
}>

type MonitorFailoverRow = Readonly<{
  account_state_action: string
  cooldown_until: string | null
  created_at: string
  customer_safe_status: string
  customer_safe_summary: string | null
  failure_class: string
  id: string
  next_lease_ref: string | null
  next_provider_account_ref: string | null
  operator_summary: string
  outcome: string
  policy_version: string
  previous_lease_ref: string | null
  previous_provider_account_ref: string | null
}>

type MonitorRunRow = Readonly<{
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

type MonitorEventRow = Readonly<{
  created_at: string
  sequence?: number
  status: string | null
  summary: string
  type: string
}>

type FirstBatchMonitorItem = Readonly<{
  actionCommands: ReadonlyArray<string>
  activeLease: Readonly<{
    accountHealth: string | null
    accountLabel: string | null
    accountStatus: string | null
    cooldownUntil: string | null
    expiresAt: string
    failureClass: string | null
    lastTouchedAt: string | null
    leaseRef: string
    lowCredit: boolean
    providerAccountRef: string
    reauthRequiredReason: string | null
    requestedAction: string
    startedAt: string
    status: string
  }> | null
  assignment: Readonly<{
    agentId: string
    assignmentKind: string
    blockedAt: string | null
    commitSha: string | null
    currentRunId: string | null
    goalId: string | null
    id: string
    status: string
    taskSpecPath: string | null
    updatedAt: string
  }> | null
  callbackLagSeconds: number | null
  callbackStatus: 'no_run' | 'no_callback' | 'fresh' | 'stale'
  customerSafe: Readonly<{
    orderStatus: CustomerOrderStatus
    status: string
    summary: string
  }>
  currentBlocker: string | null
  latestFailover: Readonly<{
    accountStateAction: string
    cooldownUntil: string | null
    createdAt: string
    customerSafeStatus: string
    customerSafeSummary: string | null
    failureClass: string
    id: string
    nextLeaseRef: string | null
    nextProviderAccountRef: string | null
    operatorSummary: string
    outcome: string
    policyVersion: string
    previousLeaseRef: string | null
    previousProviderAccountRef: string | null
  }> | null
  latestRunnerEvent: Readonly<{
    createdAt: string
    status: string | null
    summary: string
    type: string
  }> | null
  latestTriageEvent: Readonly<{
    createdAt: string
    eventType: string
    summary: string
  }> | null
  nextAction: string
  order: Readonly<{
    createdAt: string
    id: string
    repositoryFullName: string | null
    title: string
    updatedAt: string
  }>
  paymentPolicy: Readonly<{
    appliedByUserId: string | null
    customerSafeSummary: string | null
    id: string | null
    mode: FirstBatchPaymentPolicy['policyMode'] | null
    reason: string | null
    required: boolean
    status: 'not_required' | 'missing' | 'satisfied'
    updatedAt: string | null
  }>
  run: Readonly<{
    completedAt: string | null
    failedAt: string | null
    id: string
    providerAccountRef: string | null
    repositoryFullName: string
    repositoryRef: string
    startedAt: string | null
    status: string
    updatedAt: string
  }> | null
  site: Readonly<{
    id: string
    slug: string
    status: string
    title: string
  }> | null
  state: FirstBatchMonitorState
  triage: Readonly<{
    classification: OrderTriageClassification
    firstBatchEligible: boolean
    holdReason: string | null
    nextAction: string
    overnightLaunchEligible: boolean
  }>
}>

type FoldoverInventorySourceKind =
  | 'adjutant_assignment'
  | 'site_builder_artifact'
  | 'site_project'
  | 'software_order'

type FoldoverInventoryState =
  | 'delivered'
  | 'pending'
  | 'running'
  | 'stale'

type FoldoverInventoryPrivacyState =
  | 'private_only'
  | 'public_safe'

type FoldoverInventoryItem = Readonly<{
  artifactRef: string | null
  assignmentId: string | null
  foldableIntoAutopilot: boolean
  id: string
  orderId: string | null
  privacyState: FoldoverInventoryPrivacyState
  reasonRefs: ReadonlyArray<string>
  siteId: string | null
  sourceKind: FoldoverInventorySourceKind
  state: FoldoverInventoryState
  status: string
  updatedAt: string
}>

type FoldoverInventorySummary = Readonly<{
  byPrivacyState: Readonly<Record<FoldoverInventoryPrivacyState, number>>
  bySourceKind: Readonly<Record<FoldoverInventorySourceKind, number>>
  byState: Readonly<Record<FoldoverInventoryState, number>>
  foldable: number
  privateOnly: number
  publicSafe: number
  total: number
}>

type FoldoverInventoryReport = Readonly<{
  dryRun: true
  generatedAt: string
  items: ReadonlyArray<FoldoverInventoryItem>
  mutatesRecords: false
  summary: FoldoverInventorySummary
}>

type FoldoverSoftwareOrderRow = Readonly<{
  archived_at: string | null
  created_at: string
  current_run_id: string | null
  id: string
  repository_private: number | null
  status: string
  updated_at: string
  visibility: string
}>

type FoldoverAssignmentRow = Readonly<{
  archived_at: string | null
  completed_at: string | null
  current_run_id: string | null
  id: string
  site_id: string | null
  software_order_id: string | null
  status: string
  updated_at: string
  visibility: string
}>

type FoldoverSiteRow = Readonly<{
  active_deployment_id: string | null
  active_version_id: string | null
  archived_at: string | null
  id: string
  software_order_id: string | null
  status: string
  updated_at: string
  visibility: string
}>

type FoldoverArtifactRow = Readonly<{
  archived_at: string | null
  artifact_ref: string
  created_at: string
  id: string
  metadata_json: string
  order_id: string | null
  session_id: string
  session_status: string
  site_id: string | null
}>

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, OrderTriageStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new OrderTriageStorageError({ operation, error }),
  })

const numericLimit = (url: URL): number => {
  const parsed = Number(url.searchParams.get('limit') ?? '100')

  if (!Number.isFinite(parsed)) {
    return 100
  }

  return Math.min(Math.max(Math.trunc(parsed), 1), 250)
}

const isRunnableClassification = (
  classification: OrderTriageClassification,
): boolean =>
  classification === 'runnable_site' ||
  classification === 'runnable_general_autopilot'

const isActiveAssignmentStatus = (status: string | null): boolean =>
  status !== null && status !== 'complete' && status !== 'canceled'

const orderStatusForHeldClassification = (
  classification: OrderTriageClassification,
): CustomerOrderStatus =>
  classification === 'needs_clarification'
    ? 'needs_customer_input'
    : classification === 'unavailable_or_declined'
      ? 'unavailable'
      : 'scoping'

const titleFromRecord = (record: OperatorOrderTriageRecord): string => {
  const display = record.order.userDisplayName?.trim()

  if (display !== undefined && display !== '') {
    return display
  }

  const repo = record.order.repositoryFullName?.split('/').at(-1)?.trim()

  if (repo !== undefined && repo !== '') {
    return repo.replace(/[-_]+/g, ' ')
  }

  return `Site for ${record.softwareOrderId}`
}

const slugFromRecord = (record: OperatorOrderTriageRecord): string => {
  const base = titleFromRecord(record)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48)
    .replace(/-+$/g, '')

  return `${base === '' ? 'site' : base}-${record.softwareOrderId
    .replace(/^software_order_/, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 24)}`
}

const objectiveFromRecord = (
  record: OperatorOrderTriageRecord,
  site: AutopilotSiteProject | null,
): string =>
  record.classification === 'runnable_site'
    ? `Prepare the first-batch Autopilot Site assignment for ${site?.title ?? titleFromRecord(record)} from software order ${record.softwareOrderId}. Generate a task packet and preflight plan, but do not deploy without explicit review.`
    : `Prepare first-batch Autopilot fulfillment for software order ${record.softwareOrderId}. Generate a task packet and preflight plan, but do not launch without explicit review.`

const statusForCreatedAssignment: CustomerOrderStatus = 'agent_queued'

const firstBatchEligibleRecords = (
  records: ReadonlyArray<OperatorOrderTriageRecord>,
  input: FirstBatchAssignmentRequest,
): ReadonlyArray<OperatorOrderTriageRecord> => {
  const requestedIds = new Set(input.softwareOrderIds ?? [])
  const requestedOnly = requestedIds.size > 0
  const limit =
    input.limit === undefined || !Number.isFinite(input.limit)
      ? 100
      : Math.max(1, Math.min(250, Math.trunc(input.limit)))

  return records
    .filter(record => !requestedOnly || requestedIds.has(record.softwareOrderId))
    .filter(record => record.firstBatchEligible || requestedOnly)
    .slice(0, limit)
}

const secondsBetweenIso = (
  earlier: string | null,
  later: string,
): number | null => {
  if (earlier === null) {
    return null
  }

  const earlierTime = Date.parse(earlier)
  const laterTime = Date.parse(later)

  if (!Number.isFinite(earlierTime) || !Number.isFinite(laterTime)) {
    return null
  }

  return Math.max(0, Math.trunc((laterTime - earlierTime) / 1000))
}

const staleCallbackThresholdSeconds = 30 * 60

const monitorState = (
  record: OperatorOrderTriageRecord,
  assignment: MonitorAssignmentRow | null,
  run: MonitorRunRow | null,
  latestFailover: MonitorFailoverRow | null,
  callbackLagSeconds: number | null,
): FirstBatchMonitorState => {
  if (!record.firstBatchEligible || !isRunnableClassification(record.classification)) {
    return record.classification === 'needs_clarification'
      ? 'waiting_for_input'
      : 'held'
  }

  if (assignment === null) {
    return 'not_yet_assigned'
  }

  if (latestFailover?.outcome === 'blocked') {
    return 'blocked'
  }

  if (run?.status === 'failed' || assignment.status === 'blocked') {
    return 'failed'
  }

  if (
    callbackLagSeconds !== null &&
    callbackLagSeconds > staleCallbackThresholdSeconds &&
    run !== null &&
    (run.status === 'queued' || run.status === 'running')
  ) {
    return 'blocked'
  }

  switch (assignment.status) {
    case 'draft':
    case 'preflight_pending':
      return 'preflight_ready'
    case 'queued':
      return 'queued'
    case 'running':
      return 'running'
    case 'review_needed':
      return 'review_ready'
    case 'deployed':
      return 'deployed'
    case 'delivered':
    case 'complete':
      return 'delivered'
    case 'canceled':
      return 'failed'
    default:
      return run?.status === 'waiting_for_input'
        ? 'waiting_for_input'
        : run?.status === 'running'
          ? 'running'
          : run?.status === 'queued'
            ? 'queued'
            : 'preflight_ready'
  }
}

const currentBlocker = (
  record: OperatorOrderTriageRecord,
  state: FirstBatchMonitorState,
  paymentPolicy: Readonly<{ required: boolean; status: string }>,
  lease: MonitorLeaseRow | null,
  failover: MonitorFailoverRow | null,
  callbackLagSeconds: number | null,
  run: MonitorRunRow | null,
): string | null => {
  if (state === 'held' || state === 'waiting_for_input') {
    return record.holdReason ?? record.nextAction
  }

  if (paymentPolicy.required && paymentPolicy.status === 'missing') {
    return 'First-batch no-payment policy has not been applied.'
  }

  if (failover?.outcome === 'blocked') {
    return failover.operator_summary
  }

  if (lease?.low_credit_flag === 1) {
    return 'Selected provider account is marked low-credit.'
  }

  if (lease?.reauth_required_reason !== null && lease?.reauth_required_reason !== undefined) {
    return `Selected provider account requires reauth: ${lease.reauth_required_reason}.`
  }

  if (lease?.cooldown_until !== null && lease?.cooldown_until !== undefined) {
    return `Selected provider account is cooling down until ${lease.cooldown_until}.`
  }

  if (
    callbackLagSeconds !== null &&
    callbackLagSeconds > staleCallbackThresholdSeconds &&
    run !== null &&
    (run.status === 'queued' || run.status === 'running')
  ) {
    return `Runner callback is stale by ${callbackLagSeconds} seconds.`
  }

  if (run?.status === 'failed') {
    return 'Current runner failed.'
  }

  return null
}

const actionCommands = (
  record: OperatorOrderTriageRecord,
  assignment: MonitorAssignmentRow | null,
): ReadonlyArray<string> => {
  const commands = [
    `curl -sS https://openagents.com/api/operator/orders/triage?limit=100`,
    `curl -sS https://openagents.com/api/operator/orders/triage/first-batch/monitor?limit=100`,
  ]

  if (assignment !== null) {
    commands.push(
      `curl -sS https://openagents.com/api/operator/adjutant/assignments/${assignment.id}`,
      `node scripts/provider-chatgpt-device-login.mjs failover-history --assignmentId ${assignment.id} --email chris@openagents.com`,
    )
  }

  if (record.order.latestAssignmentId === null && record.overnightLaunchEligible) {
    commands.push(
      `curl -sS -X POST https://openagents.com/api/operator/orders/triage/first-batch/assign -H 'content-type: application/json' --data '{"softwareOrderIds":["${record.softwareOrderId}"]}'`,
    )
  }

  commands.push(
    'node scripts/provider-chatgpt-device-login.mjs dashboard --email chris@openagents.com',
    'node scripts/provider-chatgpt-device-login.mjs explain-lease --email chris@openagents.com',
  )

  return commands
}

const payloadJson = (
  payload: unknown,
): Effect.Effect<string, OrderTriageStorageError> =>
  Effect.try({
    try: () => JSON.stringify(payload),
    catch: error =>
      new OrderTriageStorageError({
        operation: 'orderTriage.event.payload.encode',
        error,
      }),
  })

const recordTriageEvent = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  record: OperatorOrderTriageRecord,
  input: Readonly<{
    actorUserId: string
    assignmentId?: string | null | undefined
    eventType: string
    payload: unknown
    siteId?: string | null | undefined
    summary: string
  }>,
): Effect.Effect<string, OrderTriageStorageError> =>
  Effect.gen(function* () {
    const id = runtime.makeEventId()
    const now = runtime.nowIso()
    const encoded = yield* payloadJson(input.payload)

    yield* d1Effect('orderTriage.events.insert', () =>
      db
        .prepare(
          `INSERT INTO order_triage_events
             (id,
              triage_record_id,
              software_order_id,
              site_id,
              assignment_id,
              event_type,
              visibility,
              summary,
              actor_user_id,
              payload_json,
              created_at)
           VALUES (?, ?, ?, ?, ?, ?, 'team', ?, ?, ?, ?)`,
        )
        .bind(
          id,
          record.id,
          record.softwareOrderId,
          input.siteId ?? null,
          input.assignmentId ?? null,
          input.eventType,
          input.summary,
          input.actorUserId,
          encoded,
          now,
        )
        .run(),
    )

    return id
  })

const readMonitorAssignment = (
  db: D1Database,
  record: OperatorOrderTriageRecord,
): Effect.Effect<MonitorAssignmentRow | null, OrderTriageStorageError> =>
  d1Effect('orderTriage.monitor.assignment.read', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                agent_id,
                assigned_by_user_id,
                assignment_kind,
                status,
                visibility,
                task_spec_path,
                commit_sha,
                objective,
                created_at,
                updated_at,
                completed_at,
                blocked_at
           FROM adjutant_assignments
          WHERE archived_at IS NULL
            AND (
              software_order_id = ?
              OR (? IS NOT NULL AND site_id = ?)
            )
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(
        record.softwareOrderId,
        record.order.siteProjectId,
        record.order.siteProjectId,
      )
      .first<MonitorAssignmentRow>(),
  )

const readMonitorRun = (
  db: D1Database,
  runId: string | null,
): Effect.Effect<MonitorRunRow | null, OrderTriageStorageError> =>
  runId === null
    ? Effect.succeed(null)
    : d1Effect('orderTriage.monitor.run.read', () =>
        db
          .prepare(
            `SELECT id,
                    provider_account_ref,
                    repository_owner,
                    repository_repo,
                    repository_ref,
                    status,
                    created_at,
                    updated_at,
                    started_at,
                    completed_at,
                    failed_at
               FROM agent_runs
              WHERE id = ?
              LIMIT 1`,
          )
          .bind(runId)
          .first<MonitorRunRow>(),
      )

const readMonitorLatestRunEvent = (
  db: D1Database,
  runId: string | null,
): Effect.Effect<MonitorEventRow | null, OrderTriageStorageError> =>
  runId === null
    ? Effect.succeed(null)
    : d1Effect('orderTriage.monitor.runEvent.read', () =>
        db
          .prepare(
            `SELECT type,
                    summary,
                    status,
                    created_at
               FROM agent_run_events
              WHERE run_id = ?
              ORDER BY sequence DESC
              LIMIT 1`,
          )
          .bind(runId)
          .first<MonitorEventRow>(),
      )

const readMonitorLatestTriageEvent = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<MonitorEventRow | null, OrderTriageStorageError> =>
  d1Effect('orderTriage.monitor.triageEvent.read', () =>
    db
      .prepare(
        `SELECT event_type AS type,
                summary,
                NULL AS status,
                created_at
           FROM order_triage_events
          WHERE software_order_id = ?
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<MonitorEventRow>(),
  )

const readMonitorLease = (
  db: D1Database,
  input: Readonly<{
    assignmentId: string | null
    orderId: string
    runId: string | null
  }>,
): Effect.Effect<MonitorLeaseRow | null, OrderTriageStorageError> =>
  d1Effect('orderTriage.monitor.lease.read', () =>
    db
      .prepare(
        `SELECT lease.lease_ref,
                lease.provider_account_ref,
                lease.requested_action,
                lease.status,
                lease.started_at,
                lease.expires_at,
                lease.last_touched_at,
                lease.failure_class,
                account.account_label,
                account.status AS account_status,
                account.health AS account_health,
                account.low_credit_flag,
                account.cooldown_until,
                account.reauth_required_reason
           FROM provider_account_leases lease
           LEFT JOIN provider_accounts account
             ON account.id = lease.provider_account_id
          WHERE lease.status = 'active'
            AND (
              lease.order_id = ?
              OR (? IS NOT NULL AND lease.assignment_id = ?)
              OR (? IS NOT NULL AND lease.run_id = ?)
            )
          ORDER BY lease.started_at DESC
          LIMIT 1`,
      )
      .bind(
        input.orderId,
        input.assignmentId,
        input.assignmentId,
        input.runId,
        input.runId,
      )
      .first<MonitorLeaseRow>(),
  )

const readMonitorFailover = (
  db: D1Database,
  input: Readonly<{
    assignmentId: string | null
    orderId: string
    runId: string | null
  }>,
): Effect.Effect<MonitorFailoverRow | null, OrderTriageStorageError> =>
  d1Effect('orderTriage.monitor.failover.read', () =>
    db
      .prepare(
        `SELECT id,
                previous_lease_ref,
                previous_provider_account_ref,
                next_lease_ref,
                next_provider_account_ref,
                failure_class,
                account_state_action,
                outcome,
                customer_safe_status,
                policy_version,
                cooldown_until,
                operator_summary,
                customer_safe_summary,
                created_at
           FROM provider_account_failover_receipts
          WHERE order_id = ?
             OR (? IS NOT NULL AND assignment_id = ?)
             OR (? IS NOT NULL AND run_id = ?)
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(
        input.orderId,
        input.assignmentId,
        input.assignmentId,
        input.runId,
        input.runId,
      )
      .first<MonitorFailoverRow>(),
  )

const rowToRecord = (row: OrderTriageRow): OperatorOrderTriageRecord => ({
  id: row.id,
  softwareOrderId: row.software_order_id,
  classification: row.classification,
  operatorPriority: row.operator_priority,
  firstBatchEligible: row.first_batch_eligible === 1,
  overnightLaunchEligible:
    row.first_batch_eligible === 1 &&
    isRunnableClassification(row.classification),
  holdReason: row.hold_reason,
  nextAction: row.next_action,
  customerSafeStatus: row.customer_safe_status,
  customerSafeSummary: row.customer_safe_summary,
  reviewerUserId: row.reviewer_user_id,
  reviewedAt: row.reviewed_at,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
  order: {
    id: row.order_id,
    userId: row.order_user_id,
    userDisplayName: row.user_display_name,
    userEmail: row.user_email,
    status: row.order_status,
    visibility: row.order_visibility,
    request: row.order_request,
    repositoryFullName: row.repository_full_name,
    currentRunId: row.current_run_id,
    siteProjectId: row.site_project_id,
    siteTitle: row.site_title,
    siteSlug: row.site_slug,
    siteStatus: row.site_status,
    latestAssignmentId: row.latest_assignment_id,
    latestAssignmentStatus: row.latest_assignment_status,
    latestAssignmentKind: row.latest_assignment_kind,
    createdAt: row.order_created_at,
    updatedAt: row.order_updated_at,
  },
})

const triageSelectSql = `
SELECT order_triage_records.id AS id,
       order_triage_records.software_order_id AS software_order_id,
       order_triage_records.classification AS classification,
       order_triage_records.operator_priority AS operator_priority,
       order_triage_records.first_batch_eligible AS first_batch_eligible,
       order_triage_records.hold_reason AS hold_reason,
       order_triage_records.next_action AS next_action,
       order_triage_records.customer_safe_status AS customer_safe_status,
       order_triage_records.customer_safe_summary AS customer_safe_summary,
       order_triage_records.reviewer_user_id AS reviewer_user_id,
       order_triage_records.reviewed_at AS reviewed_at,
       order_triage_records.created_at AS created_at,
       order_triage_records.updated_at AS updated_at,
       software_orders.id AS order_id,
       software_orders.user_id AS order_user_id,
       users.display_name AS user_display_name,
       users.primary_email AS user_email,
       software_orders.status AS order_status,
       software_orders.visibility AS order_visibility,
       software_orders.request AS order_request,
       software_orders.repository_full_name AS repository_full_name,
       software_orders.current_run_id AS current_run_id,
       site_projects.id AS site_project_id,
       site_projects.title AS site_title,
       site_projects.slug AS site_slug,
       site_projects.status AS site_status,
       (SELECT assignment.id
          FROM adjutant_assignments AS assignment
         WHERE assignment.software_order_id = software_orders.id
           AND assignment.archived_at IS NULL
         ORDER BY assignment.created_at DESC
         LIMIT 1) AS latest_assignment_id,
       (SELECT assignment.status
          FROM adjutant_assignments AS assignment
         WHERE assignment.software_order_id = software_orders.id
           AND assignment.archived_at IS NULL
         ORDER BY assignment.created_at DESC
         LIMIT 1) AS latest_assignment_status,
       (SELECT assignment.assignment_kind
          FROM adjutant_assignments AS assignment
         WHERE assignment.software_order_id = software_orders.id
           AND assignment.archived_at IS NULL
         ORDER BY assignment.created_at DESC
         LIMIT 1) AS latest_assignment_kind,
       software_orders.created_at AS order_created_at,
       software_orders.updated_at AS order_updated_at
  FROM order_triage_records
  JOIN software_orders
    ON software_orders.id = order_triage_records.software_order_id
   AND software_orders.archived_at IS NULL
  LEFT JOIN users
    ON users.id = software_orders.user_id
   AND users.deleted_at IS NULL
  LEFT JOIN site_projects
    ON site_projects.software_order_id = software_orders.id
   AND site_projects.archived_at IS NULL
 WHERE order_triage_records.archived_at IS NULL`

const readQueue = (
  db: D1Database,
  limit: number,
): Effect.Effect<
  ReadonlyArray<OperatorOrderTriageRecord>,
  OrderTriageStorageError
> =>
  d1Effect('orderTriage.queue.list', () =>
    db
      .prepare(
        `${triageSelectSql}
          ORDER BY order_triage_records.first_batch_eligible DESC,
                   order_triage_records.operator_priority ASC,
                   order_triage_records.updated_at DESC
          LIMIT ?`,
      )
      .bind(limit)
      .all<OrderTriageRow>(),
  ).pipe(Effect.map(result => result.results.map(rowToRecord)))

const readRecordByOrderId = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<OperatorOrderTriageRecord | null, OrderTriageStorageError> =>
  d1Effect('orderTriage.record.read', () =>
    db
      .prepare(
        `${triageSelectSql}
          AND order_triage_records.software_order_id = ?
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<OrderTriageRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToRecord(row))))

const requireTriageRecordByOrderId = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<OperatorOrderTriageRecord, OrderTriageError> =>
  Effect.gen(function* () {
    const record = yield* readRecordByOrderId(db, softwareOrderId)

    if (record === null) {
      return yield* new OrderTriageSoftwareOrderNotFound({ softwareOrderId })
    }

    return record
  })

const requireSoftwareOrder = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<void, OrderTriageError> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('orderTriage.softwareOrder.read', () =>
      db
        .prepare(
          `SELECT id
             FROM software_orders
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(softwareOrderId)
        .first<OrderExistsRow>(),
    )

    if (row === null) {
      return yield* new OrderTriageSoftwareOrderNotFound({ softwareOrderId })
    }
  })

const updateOrderStatus = (
  db: D1Database,
  softwareOrderId: string,
  status: CustomerOrderStatus | undefined,
  now: string,
): Effect.Effect<void, OrderTriageStorageError> =>
  status === undefined
    ? Effect.void
    : d1Effect('orderTriage.softwareOrder.status.update', () =>
        db
          .prepare(
            `UPDATE software_orders
                SET status = ?,
                    updated_at = ?
              WHERE id = ?
                AND archived_at IS NULL`,
          )
          .bind(status, now, softwareOrderId)
          .run(),
      ).pipe(Effect.asVoid)

const upsertTriageRecord = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  softwareOrderId: string,
  actorUserId: string,
  input: UpsertOrderTriageRequest,
): Effect.Effect<OperatorOrderTriageRecord, OrderTriageError> =>
  Effect.gen(function* () {
    yield* requireSoftwareOrder(db, softwareOrderId)

    const now = runtime.nowIso()
    const holdReason = input.holdReason ?? null

    yield* updateOrderStatus(db, softwareOrderId, input.orderStatus, now)

    const updateResult = yield* d1Effect('orderTriage.record.update', () =>
      db
        .prepare(
          `UPDATE order_triage_records
              SET classification = ?,
                  operator_priority = ?,
                  first_batch_eligible = ?,
                  hold_reason = ?,
                  next_action = ?,
                  customer_safe_status = ?,
                  customer_safe_summary = ?,
                  reviewer_user_id = ?,
                  reviewed_at = ?,
                  updated_at = ?
            WHERE software_order_id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          input.classification,
          input.operatorPriority,
          input.firstBatchEligible ? 1 : 0,
          holdReason,
          input.nextAction,
          input.customerSafeStatus,
          input.customerSafeSummary,
          actorUserId,
          now,
          now,
          softwareOrderId,
        )
        .run(),
    )

    if (Number(updateResult.meta?.changes ?? 0) === 0) {
      yield* d1Effect('orderTriage.record.insert', () =>
        db
          .prepare(
            `INSERT INTO order_triage_records
               (id,
                software_order_id,
                classification,
                operator_priority,
                first_batch_eligible,
                hold_reason,
                next_action,
                customer_safe_status,
                customer_safe_summary,
                reviewer_user_id,
                reviewed_at,
                created_at,
                updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            runtime.makeRecordId(),
            softwareOrderId,
            input.classification,
            input.operatorPriority,
            input.firstBatchEligible ? 1 : 0,
            holdReason,
            input.nextAction,
            input.customerSafeStatus,
            input.customerSafeSummary,
            actorUserId,
            now,
            now,
            now,
          )
          .run(),
      )
    }

    return yield* requireTriageRecordByOrderId(db, softwareOrderId)
  })

const summarizeFirstBatch = (
  dryRun: boolean,
  results: ReadonlyArray<FirstBatchAssignmentResult>,
): FirstBatchAssignmentSummary => ({
  alreadyAssigned: results.filter(
    result => result.decision === 'already_assigned',
  ).length,
  created: results.filter(result => result.decision === 'created_assignment')
    .length,
  dryRun,
  held: results.filter(result => result.decision === 'held').length,
  total: results.length,
  wouldCreate: results.filter(
    result => result.decision === 'would_create_assignment',
  ).length,
})

const heldResult = (
  record: OperatorOrderTriageRecord,
  dryRun: boolean,
  receiptId: string | null,
  orderStatus: CustomerOrderStatus,
): FirstBatchAssignmentResult => ({
  assignment: null,
  assignmentId: null,
  classification: record.classification,
  customerSafeStatus: record.customerSafeStatus,
  customerSafeSummary: record.customerSafeSummary,
  decision: 'held',
  dryRun,
  firstBatchEligible: record.firstBatchEligible,
  holdReason:
    record.holdReason ??
    (record.firstBatchEligible
      ? 'Triage classification is not launchable.'
      : 'Order is not first-batch eligible.'),
  nextAction: record.nextAction,
  orderStatus,
  receiptId,
  site: null,
  siteId: record.order.siteProjectId,
  softwareOrderId: record.softwareOrderId,
  summary: `Held software order ${record.softwareOrderId}: ${record.customerSafeStatus}.`,
})

const alreadyAssignedResult = (
  record: OperatorOrderTriageRecord,
  dryRun: boolean,
  receiptId: string | null,
): FirstBatchAssignmentResult => ({
  assignment: null,
  assignmentId: record.order.latestAssignmentId,
  classification: record.classification,
  customerSafeStatus: record.customerSafeStatus,
  customerSafeSummary: record.customerSafeSummary,
  decision: 'already_assigned',
  dryRun,
  firstBatchEligible: record.firstBatchEligible,
  holdReason: null,
  nextAction: 'Continue the existing active Autopilot assignment.',
  orderStatus: record.order.status,
  receiptId,
  site: null,
  siteId: record.order.siteProjectId,
  softwareOrderId: record.softwareOrderId,
  summary: `Software order ${record.softwareOrderId} already has active assignment ${record.order.latestAssignmentId}.`,
})

const createFirstBatchAssignment = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  record: OperatorOrderTriageRecord,
  actorUserId: string,
  dryRun: boolean,
  mode: 'first_batch' | 'order_fulfillment' = 'first_batch',
): Effect.Effect<
  FirstBatchAssignmentResult,
  | AdjutantAssignmentError
  | AutopilotSiteError
  | OrderTriageStorageError,
  AdjutantAssignmentService | AutopilotSitesService
> =>
  Effect.gen(function* () {
    const requiresFirstBatchEligibility = mode === 'first_batch'

    if (
      (requiresFirstBatchEligibility && !record.firstBatchEligible) ||
      !isRunnableClassification(record.classification)
    ) {
      const orderStatus = orderStatusForHeldClassification(record.classification)
      yield* updateOrderStatus(db, record.softwareOrderId, orderStatus, runtime.nowIso())
      const receiptId = dryRun
        ? null
        : yield* recordTriageEvent(db, runtime, record, {
            actorUserId,
            eventType:
              mode === 'first_batch'
                ? 'order_triage.first_batch_held'
                : 'order_fulfillment.prepare_held',
            payload: {
              classification: record.classification,
              customerSafeStatus: record.customerSafeStatus,
              firstBatchEligible: record.firstBatchEligible,
              holdReason: record.holdReason,
              nextAction: record.nextAction,
              prepareMode: mode,
              softwareOrderId: record.softwareOrderId,
            },
            summary:
              mode === 'first_batch'
                ? `First-batch held: ${record.customerSafeStatus}.`
                : `Order fulfillment held: ${record.customerSafeStatus}.`,
          })

      return heldResult(record, dryRun, receiptId, orderStatus)
    }

    if (isActiveAssignmentStatus(record.order.latestAssignmentStatus)) {
      const receiptId = dryRun
        ? null
        : yield* recordTriageEvent(db, runtime, record, {
            actorUserId,
            assignmentId: record.order.latestAssignmentId,
            eventType:
              mode === 'first_batch'
                ? 'order_triage.first_batch_already_assigned'
                : 'order_fulfillment.prepare_already_assigned',
            payload: {
              assignmentId: record.order.latestAssignmentId,
              assignmentKind: record.order.latestAssignmentKind,
              classification: record.classification,
              prepareMode: mode,
              softwareOrderId: record.softwareOrderId,
            },
            siteId: record.order.siteProjectId,
            summary:
              mode === 'first_batch'
                ? 'First-batch assignment skipped because an active assignment already exists.'
                : 'Order fulfillment prepare skipped because an active assignment already exists.',
          })

      return alreadyAssignedResult(record, dryRun, receiptId)
    }

    const site =
      record.classification === 'runnable_site'
        ? yield* (yield* AutopilotSitesService).createProjectFromSoftwareOrder({
            accessMode: 'customer_owner',
            actorUserId,
            softwareOrderId: record.softwareOrderId,
            slug: record.order.siteSlug ?? slugFromRecord(record),
            title: record.order.siteTitle ?? titleFromRecord(record),
            visibility: 'team',
          })
        : null

    if (dryRun) {
      return {
        assignment: null,
        assignmentId: null,
        classification: record.classification,
        customerSafeStatus: 'queued',
        customerSafeSummary:
          record.classification === 'runnable_site'
            ? 'OpenAgents is ready to create a website assignment for this order.'
            : 'OpenAgents is ready to create an Autopilot assignment for this order.',
        decision: 'would_create_assignment' as const,
        dryRun,
        firstBatchEligible: record.firstBatchEligible,
        holdReason: null,
        nextAction: 'Create task packet and run operator preflight.',
        orderStatus: record.order.status,
        receiptId: null,
        site,
        siteId: site?.id ?? record.order.siteProjectId,
        softwareOrderId: record.softwareOrderId,
        summary: `Would create first-batch assignment for software order ${record.softwareOrderId}.`,
      }
    }

    yield* updateOrderStatus(
      db,
      record.softwareOrderId,
      statusForCreatedAssignment,
      runtime.nowIso(),
    )

    const assignment = yield* (yield* AdjutantAssignmentService).createAssignment(
      {
        assignedByUserId: actorUserId,
        assignmentKind:
          record.classification === 'runnable_site'
            ? 'site_generation'
            : 'general_order_fulfillment',
        objective: objectiveFromRecord(record, site),
        softwareOrderId: record.softwareOrderId,
        status: 'preflight_pending',
        visibility: 'team',
        ...(site?.id === undefined ? {} : { siteId: site.id }),
      },
    )

    yield* (yield* AdjutantAssignmentService).recordEvent({
      actorUserId,
      assignmentId: assignment.id,
      eventType:
        mode === 'first_batch'
          ? 'adjutant.first_batch_assignment_prepared'
          : 'adjutant.order_fulfillment_prepared',
      payload: {
        classification: record.classification,
        firstBatchEligible: record.firstBatchEligible,
        nextAction: 'Create task packet and run operator preflight.',
        orderTriageRecordId: record.id,
        prepareMode: mode,
        siteId: site?.id ?? null,
        softwareOrderId: record.softwareOrderId,
      },
      summary:
        mode === 'first_batch'
          ? 'First-batch assignment prepared from typed order triage. No launch was started.'
          : 'Order fulfillment prepared from typed order triage. No launch was started.',
    })

    const receiptId = yield* recordTriageEvent(db, runtime, record, {
      actorUserId,
      assignmentId: assignment.id,
      eventType:
        mode === 'first_batch'
          ? 'order_triage.first_batch_assignment_created'
          : 'order_fulfillment.prepare_assignment_created',
      payload: {
        assignmentId: assignment.id,
        assignmentKind: assignment.assignmentKind,
        classification: record.classification,
        firstBatchEligible: record.firstBatchEligible,
        nextAction: 'Create task packet and run operator preflight.',
        prepareMode: mode,
        siteId: site?.id ?? null,
        softwareOrderId: record.softwareOrderId,
      },
      siteId: site?.id ?? null,
      summary:
        mode === 'first_batch'
          ? 'First-batch assignment created from typed order triage. No launch was started.'
          : 'Order fulfillment assignment created from typed order triage. No launch was started.',
    })

    return {
      assignment,
      assignmentId: assignment.id,
      classification: record.classification,
      customerSafeStatus: 'queued',
      customerSafeSummary:
        record.classification === 'runnable_site'
          ? 'OpenAgents queued this website order for Autopilot preflight.'
          : 'OpenAgents queued this request for Autopilot preflight.',
      decision: 'created_assignment' as const,
      dryRun,
      firstBatchEligible: record.firstBatchEligible,
      holdReason: null,
      nextAction: 'Create task packet and run operator preflight.',
      orderStatus: statusForCreatedAssignment,
      receiptId,
      site,
      siteId: site?.id ?? null,
      softwareOrderId: record.softwareOrderId,
      summary: `Created first-batch assignment ${assignment.id} for software order ${record.softwareOrderId}.`,
    }
  }).pipe(
    Effect.catchTag('AdjutantAssignmentActiveExists', error =>
      Effect.gen(function* () {
        const receiptId = dryRun
          ? null
          : yield* recordTriageEvent(db, runtime, record, {
              actorUserId,
              assignmentId: error.assignmentId,
              eventType: 'order_triage.first_batch_already_assigned',
              payload: {
              assignmentId: error.assignmentId,
              classification: record.classification,
              prepareMode: mode,
              siteId: error.siteId,
              softwareOrderId: error.softwareOrderId,
            },
            siteId: error.siteId,
            summary:
              mode === 'first_batch'
                ? 'First-batch assignment skipped because the assignment service found an active assignment.'
                : 'Order fulfillment prepare skipped because the assignment service found an active assignment.',
          })

        return alreadyAssignedResult(
          {
            ...record,
            order: {
              ...record.order,
              latestAssignmentId: error.assignmentId,
              latestAssignmentStatus: 'preflight_pending',
            },
          },
          dryRun,
          receiptId,
        )
      }),
    ),
  )

const assignFirstBatch = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  actorUserId: string,
  input: FirstBatchAssignmentRequest,
): Effect.Effect<
  Readonly<{
    results: ReadonlyArray<FirstBatchAssignmentResult>
    summary: FirstBatchAssignmentSummary
  }>,
  | AdjutantAssignmentError
  | AutopilotSiteError
  | OrderTriageStorageError,
  AdjutantAssignmentService | AutopilotSitesService
> =>
  Effect.gen(function* () {
    const dryRun = input.dryRun ?? false
    const queue = yield* readQueue(db, input.limit ?? 100)
    const records = firstBatchEligibleRecords(queue, input)
    const results: Array<FirstBatchAssignmentResult> = []

    for (const record of records) {
      results.push(
        yield* createFirstBatchAssignment(
          db,
          runtime,
          record,
          actorUserId,
          dryRun,
        ),
      )
    }

    return {
      results,
      summary: summarizeFirstBatch(dryRun, results),
    }
  })

const prepareOrderFulfillment = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  actorUserId: string,
  softwareOrderId: string,
  input: PrepareOrderFulfillmentRequest,
): Effect.Effect<
  FirstBatchAssignmentResult,
  | AdjutantAssignmentError
  | AutopilotSiteError
  | OrderTriageSoftwareOrderNotFound
  | OrderTriageStorageError,
  AdjutantAssignmentService | AutopilotSitesService
> =>
  Effect.gen(function* () {
    const record = yield* requireTriageRecordByOrderId(db, softwareOrderId)

    return yield* createFirstBatchAssignment(
      db,
      runtime,
      record,
      actorUserId,
      input.dryRun ?? false,
      'order_fulfillment',
    )
  })

const defaultNoPaymentPolicyReason =
  'First submitted-order batch is covered by the OpenAgents public beta free-slice policy.'

const defaultNoPaymentCustomerSummary =
  'This first-batch OpenAgents run is covered by a public beta free slice. No customer charge is being recorded for this launch.'

const applyFirstBatchPaymentPolicies = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  actorUserId: string,
  input: ApplyFirstBatchPaymentPolicyRequest,
): Effect.Effect<
  ReadonlyArray<FirstBatchPaymentPolicyApplyResult>,
  | FirstBatchPaymentPolicyError
  | OperatorOrderTriageBadRequest
  | OrderTriageStorageError
> =>
  Effect.gen(function* () {
    const uniqueOrderIds = [...new Set(input.softwareOrderIds)]

    if (uniqueOrderIds.length === 0) {
      return yield* new OperatorOrderTriageBadRequest({
        reason: 'At least one softwareOrderId is required.',
      })
    }

    const results: Array<FirstBatchPaymentPolicyApplyResult> = []

    for (const softwareOrderId of uniqueOrderIds) {
      const record = yield* readRecordByOrderId(db, softwareOrderId)

      if (record === null) {
        return yield* new OperatorOrderTriageBadRequest({
          reason: `No first-batch triage record exists for ${softwareOrderId}.`,
        })
      }

      if (!record.firstBatchEligible) {
        return yield* new OperatorOrderTriageBadRequest({
          reason: `${softwareOrderId} is not first-batch eligible.`,
        })
      }

      const assignment = yield* readMonitorAssignment(db, record)
      const policy = yield* upsertFirstBatchPaymentPolicy(
        db,
        systemFirstBatchPaymentPolicyRuntime,
        {
          appliedByUserId: actorUserId,
          assignmentId: assignment?.id ?? record.order.latestAssignmentId,
          customerSafeSummary:
            input.customerSafeSummary ?? defaultNoPaymentCustomerSummary,
          policyMode: input.policyMode ?? 'public_beta_free',
          reason: input.reason ?? defaultNoPaymentPolicyReason,
          siteId: assignment?.site_id ?? record.order.siteProjectId,
          softwareOrderId,
        },
      )

      yield* recordTriageEvent(db, runtime, record, {
        actorUserId,
        assignmentId: policy.assignmentId,
        eventType: 'order_triage.first_batch_payment_policy_applied',
        payload: {
          customerSafeSummary: policy.customerSafeSummary,
          policyId: policy.id,
          policyMode: policy.policyMode,
          softwareOrderId,
        },
        siteId: policy.siteId,
        summary:
          'First-batch no-payment policy applied for public beta fulfillment.',
      })

      results.push({
        assignmentId: policy.assignmentId,
        classification: record.classification,
        firstBatchEligible: record.firstBatchEligible,
        overnightLaunchEligible: record.overnightLaunchEligible,
        paymentPolicy: policy,
        siteId: policy.siteId,
        softwareOrderId,
      })
    }

    return results
  })

const monitorItem = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  record: OperatorOrderTriageRecord,
): Effect.Effect<FirstBatchMonitorItem, OrderTriageStorageError> =>
  Effect.gen(function* () {
    const assignment = yield* readMonitorAssignment(db, record)
    const runId = assignment?.current_run_id ?? record.order.currentRunId
    const [run, latestRunnerEvent, latestTriageEvent, activeLease, latestFailover] =
      yield* Effect.all(
        [
          readMonitorRun(db, runId),
          readMonitorLatestRunEvent(db, runId),
          readMonitorLatestTriageEvent(db, record.softwareOrderId),
          readMonitorLease(db, {
            assignmentId: assignment?.id ?? record.order.latestAssignmentId,
            orderId: record.softwareOrderId,
            runId,
          }),
          readMonitorFailover(db, {
            assignmentId: assignment?.id ?? record.order.latestAssignmentId,
            orderId: record.softwareOrderId,
            runId,
          }),
        ] as const,
        { concurrency: 5 },
      )
    const now = runtime.nowIso()
    const callbackBasis = latestRunnerEvent?.created_at ?? run?.updated_at ?? null
    const callbackLagSeconds = runId === null ? null : secondsBetweenIso(callbackBasis, now)
    const callbackStatus =
      runId === null
        ? 'no_run'
        : latestRunnerEvent === null
          ? 'no_callback'
          : callbackLagSeconds !== null &&
              callbackLagSeconds > staleCallbackThresholdSeconds
            ? 'stale'
            : 'fresh'
    const state = monitorState(
      record,
      assignment,
      run,
      latestFailover,
      callbackLagSeconds,
    )
    const paymentGate = yield* readFirstBatchPaymentGate(
      db,
      record.softwareOrderId,
    ).pipe(
      Effect.mapError(
        error =>
          new OrderTriageStorageError({
            error,
            operation: 'orderTriage.monitor.paymentPolicy.read',
          }),
      ),
    )
    const blocker = currentBlocker(
      record,
      state,
      paymentGate,
      activeLease,
      latestFailover,
      callbackLagSeconds,
      run,
    )

    return {
      actionCommands: actionCommands(record, assignment),
      activeLease:
        activeLease === null
          ? null
          : {
              accountHealth: activeLease.account_health,
              accountLabel: activeLease.account_label,
              accountStatus: activeLease.account_status,
              cooldownUntil: activeLease.cooldown_until,
              expiresAt: activeLease.expires_at,
              failureClass: activeLease.failure_class,
              lastTouchedAt: activeLease.last_touched_at,
              leaseRef: activeLease.lease_ref,
              lowCredit: activeLease.low_credit_flag === 1,
              providerAccountRef: activeLease.provider_account_ref,
              reauthRequiredReason: activeLease.reauth_required_reason,
              requestedAction: activeLease.requested_action,
              startedAt: activeLease.started_at,
              status: activeLease.status,
            },
      assignment:
        assignment === null
          ? null
          : {
              agentId: assignment.agent_id,
              assignmentKind: assignment.assignment_kind,
              blockedAt: assignment.blocked_at,
              commitSha: assignment.commit_sha,
              currentRunId: assignment.current_run_id,
              goalId: assignment.goal_id,
              id: assignment.id,
              status: assignment.status,
              taskSpecPath: assignment.task_spec_path,
              updatedAt: assignment.updated_at,
            },
      callbackLagSeconds,
      callbackStatus,
      currentBlocker: blocker,
      customerSafe: {
        orderStatus: record.order.status,
        status: record.customerSafeStatus,
        summary: record.customerSafeSummary,
      },
      latestFailover:
        latestFailover === null
          ? null
          : {
              accountStateAction: latestFailover.account_state_action,
              cooldownUntil: latestFailover.cooldown_until,
              createdAt: latestFailover.created_at,
              customerSafeStatus: latestFailover.customer_safe_status,
              customerSafeSummary: latestFailover.customer_safe_summary,
              failureClass: latestFailover.failure_class,
              id: latestFailover.id,
              nextLeaseRef: latestFailover.next_lease_ref,
              nextProviderAccountRef: latestFailover.next_provider_account_ref,
              operatorSummary: latestFailover.operator_summary,
              outcome: latestFailover.outcome,
              policyVersion: latestFailover.policy_version,
              previousLeaseRef: latestFailover.previous_lease_ref,
              previousProviderAccountRef:
                latestFailover.previous_provider_account_ref,
            },
      latestRunnerEvent:
        latestRunnerEvent === null
          ? null
          : {
              createdAt: latestRunnerEvent.created_at,
              status: latestRunnerEvent.status,
              summary: latestRunnerEvent.summary,
              type: latestRunnerEvent.type,
            },
      latestTriageEvent:
        latestTriageEvent === null
          ? null
          : {
              createdAt: latestTriageEvent.created_at,
              eventType: latestTriageEvent.type,
              summary: latestTriageEvent.summary,
            },
      nextAction: blocker ?? record.nextAction,
      order: {
        createdAt: record.order.createdAt,
        id: record.softwareOrderId,
        repositoryFullName: record.order.repositoryFullName,
        title: titleFromRecord(record),
        updatedAt: record.order.updatedAt,
      },
      paymentPolicy: {
        appliedByUserId: paymentGate.policy?.appliedByUserId ?? null,
        customerSafeSummary: paymentGate.policy?.customerSafeSummary ?? null,
        id: paymentGate.policy?.id ?? null,
        mode: paymentGate.policy?.policyMode ?? null,
        reason: paymentGate.policy?.reason ?? null,
        required: paymentGate.required,
        status: paymentGate.status,
        updatedAt: paymentGate.policy?.updatedAt ?? null,
      },
      run:
        run === null
          ? null
          : {
              completedAt: run.completed_at,
              failedAt: run.failed_at,
              id: run.id,
              providerAccountRef: run.provider_account_ref,
              repositoryFullName: `${run.repository_owner}/${run.repository_repo}`,
              repositoryRef: run.repository_ref,
              startedAt: run.started_at,
              status: run.status,
              updatedAt: run.updated_at,
            },
      site:
        record.order.siteProjectId === null
          ? null
          : {
              id: record.order.siteProjectId,
              slug: record.order.siteSlug ?? '',
              status: record.order.siteStatus ?? 'unknown',
              title: record.order.siteTitle ?? titleFromRecord(record),
            },
      state,
      triage: {
        classification: record.classification,
        firstBatchEligible: record.firstBatchEligible,
        holdReason: record.holdReason,
        nextAction: record.nextAction,
        overnightLaunchEligible: record.overnightLaunchEligible,
      },
    }
  })

const monitorFirstBatch = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  limit: number,
): Effect.Effect<
  Readonly<{
    monitor: ReadonlyArray<FirstBatchMonitorItem>
    summary: Readonly<Record<FirstBatchMonitorState, number> & { total: number }>
  }>,
  OrderTriageStorageError
> =>
  Effect.gen(function* () {
    const queue = yield* readQueue(db, limit)
    const records = queue.filter(record => record.firstBatchEligible)
    const monitor = yield* Effect.all(
      records.map(record => monitorItem(db, runtime, record)),
      { concurrency: 5 },
    )
    const base: Record<FirstBatchMonitorState, number> = {
      blocked: 0,
      deployed: 0,
      delivered: 0,
      failed: 0,
      held: 0,
      not_yet_assigned: 0,
      preflight_ready: 0,
      queued: 0,
      review_ready: 0,
      running: 0,
      waiting_for_input: 0,
    }

    for (const item of monitor) {
      base[item.state] += 1
    }

    return { monitor, summary: { ...base, total: monitor.length } }
  })

const staleFoldoverThresholdSeconds = 24 * 60 * 60

const foldoverState = (
  status: string,
  updatedAt: string,
  nowIso: string,
): FoldoverInventoryState => {
  const activeState =
    status === 'delivered' ||
    status === 'complete' ||
    status === 'approved' ||
    status === 'generated' ||
    status === 'saved'
      ? 'delivered'
      : status === 'agent_running' ||
          status === 'running' ||
          status === 'queued' ||
          status === 'generating'
        ? 'running'
        : 'pending'

  if (activeState !== 'delivered') {
    const ageSeconds = secondsBetweenIso(updatedAt, nowIso)

    if (ageSeconds !== null && ageSeconds > staleFoldoverThresholdSeconds) {
      return 'stale'
    }
  }

  return activeState
}

const safeMetadataFlag = (metadataJson: string, key: string): boolean => {
  const parsed = parseJsonRecord(metadataJson)

  return parsed?.[key] === true
}

const foldableForState = (state: FoldoverInventoryState): boolean =>
  state === 'pending' || state === 'running' || state === 'stale'

const sourceReasonRef = (
  sourceKind: FoldoverInventorySourceKind,
  state: FoldoverInventoryState,
  privacyState: FoldoverInventoryPrivacyState,
): ReadonlyArray<string> => [
  `foldover.source.${sourceKind}`,
  `foldover.state.${state}`,
  `foldover.privacy.${privacyState}`,
]

const readFoldoverSoftwareOrders = (
  db: D1Database,
  limit: number,
): Effect.Effect<ReadonlyArray<FoldoverSoftwareOrderRow>, OrderTriageStorageError> =>
  d1Effect('orderTriage.foldover.softwareOrders', () =>
    db.prepare(
      `SELECT id,
              status,
              visibility,
              repository_private,
              current_run_id,
              created_at,
              updated_at,
              archived_at
       FROM software_orders
       WHERE archived_at IS NULL
       ORDER BY updated_at DESC
       LIMIT ?`,
    ).bind(limit).all<FoldoverSoftwareOrderRow>()
  ).pipe(Effect.map(result => result.results ?? []))

const readFoldoverAssignments = (
  db: D1Database,
  limit: number,
): Effect.Effect<ReadonlyArray<FoldoverAssignmentRow>, OrderTriageStorageError> =>
  d1Effect('orderTriage.foldover.assignments', () =>
    db.prepare(
      `SELECT id,
              software_order_id,
              site_id,
              current_run_id,
              status,
              visibility,
              updated_at,
              completed_at,
              archived_at
       FROM adjutant_assignments
       WHERE archived_at IS NULL
       ORDER BY updated_at DESC
       LIMIT ?`,
    ).bind(limit).all<FoldoverAssignmentRow>()
  ).pipe(Effect.map(result => result.results ?? []))

const readFoldoverSites = (
  db: D1Database,
  limit: number,
): Effect.Effect<ReadonlyArray<FoldoverSiteRow>, OrderTriageStorageError> =>
  d1Effect('orderTriage.foldover.sites', () =>
    db.prepare(
      `SELECT id,
              software_order_id,
              status,
              visibility,
              active_version_id,
              active_deployment_id,
              updated_at,
              archived_at
       FROM site_projects
       WHERE archived_at IS NULL
       ORDER BY updated_at DESC
       LIMIT ?`,
    ).bind(limit).all<FoldoverSiteRow>()
  ).pipe(Effect.map(result => result.results ?? []))

const readFoldoverArtifacts = (
  db: D1Database,
  limit: number,
): Effect.Effect<ReadonlyArray<FoldoverArtifactRow>, OrderTriageStorageError> =>
  d1Effect('orderTriage.foldover.artifacts', () =>
    db.prepare(
      `SELECT a.id,
              a.artifact_ref,
              a.metadata_json,
              a.created_at,
              a.archived_at,
              s.id AS session_id,
              s.site_id,
              s.order_id,
              s.status AS session_status
       FROM site_builder_artifacts a
       INNER JOIN site_builder_sessions s
          ON s.id = a.session_id
       WHERE a.archived_at IS NULL
         AND s.archived_at IS NULL
       ORDER BY a.created_at DESC
       LIMIT ?`,
    ).bind(limit).all<FoldoverArtifactRow>()
  ).pipe(Effect.map(result => result.results ?? []))

const foldoverInventorySummary = (
  items: ReadonlyArray<FoldoverInventoryItem>,
): FoldoverInventorySummary => {
  const byPrivacyState: Record<FoldoverInventoryPrivacyState, number> = {
    private_only: 0,
    public_safe: 0,
  }
  const bySourceKind: Record<FoldoverInventorySourceKind, number> = {
    adjutant_assignment: 0,
    site_builder_artifact: 0,
    site_project: 0,
    software_order: 0,
  }
  const byState: Record<FoldoverInventoryState, number> = {
    delivered: 0,
    pending: 0,
    running: 0,
    stale: 0,
  }
  let foldable = 0

  for (const item of items) {
    byPrivacyState[item.privacyState] += 1
    bySourceKind[item.sourceKind] += 1
    byState[item.state] += 1
    foldable += item.foldableIntoAutopilot ? 1 : 0
  }

  return {
    byPrivacyState,
    bySourceKind,
    byState,
    foldable,
    privateOnly: byPrivacyState.private_only,
    publicSafe: byPrivacyState.public_safe,
    total: items.length,
  }
}

const foldoverInventory = (
  db: D1Database,
  runtime: OrderTriageRuntime,
  limit: number,
): Effect.Effect<FoldoverInventoryReport, OrderTriageStorageError> =>
  Effect.gen(function* () {
    const nowIso = runtime.nowIso()
    const [
      softwareOrders,
      assignments,
      sites,
      artifacts,
    ] = yield* Effect.all([
      readFoldoverSoftwareOrders(db, limit),
      readFoldoverAssignments(db, limit),
      readFoldoverSites(db, limit),
      readFoldoverArtifacts(db, limit),
    ])

    const softwareOrderItems = softwareOrders.map(row => {
      const privacyState = row.visibility === 'public' && row.repository_private !== 1
        ? 'public_safe'
        : 'private_only'
      const state = foldoverState(row.status, row.updated_at, nowIso)

      return {
        artifactRef: null,
        assignmentId: null,
        foldableIntoAutopilot: foldableForState(state),
        id: row.id,
        orderId: row.id,
        privacyState,
        reasonRefs: sourceReasonRef('software_order', state, privacyState),
        siteId: null,
        sourceKind: 'software_order',
        state,
        status: row.status,
        updatedAt: row.updated_at,
      } satisfies FoldoverInventoryItem
    })
    const assignmentItems = assignments.map(row => {
      const privacyState = row.visibility === 'public'
        ? 'public_safe'
        : 'private_only'
      const state = foldoverState(row.status, row.updated_at, nowIso)

      return {
        artifactRef: null,
        assignmentId: row.id,
        foldableIntoAutopilot: foldableForState(state),
        id: row.id,
        orderId: row.software_order_id,
        privacyState,
        reasonRefs: sourceReasonRef('adjutant_assignment', state, privacyState),
        siteId: row.site_id,
        sourceKind: 'adjutant_assignment',
        state,
        status: row.status,
        updatedAt: row.updated_at,
      } satisfies FoldoverInventoryItem
    })
    const siteItems = sites.map(row => {
      const privacyState = row.visibility === 'public'
        ? 'public_safe'
        : 'private_only'
      const state = foldoverState(row.status, row.updated_at, nowIso)

      return {
        artifactRef: row.active_version_id,
        assignmentId: null,
        foldableIntoAutopilot: foldableForState(state),
        id: row.id,
        orderId: row.software_order_id,
        privacyState,
        reasonRefs: sourceReasonRef('site_project', state, privacyState),
        siteId: row.id,
        sourceKind: 'site_project',
        state,
        status: row.status,
        updatedAt: row.updated_at,
      } satisfies FoldoverInventoryItem
    })
    const artifactItems = artifacts.map(row => {
      const privacyState = safeMetadataFlag(row.metadata_json, 'publicSafe')
        ? 'public_safe'
        : 'private_only'
      const state = foldoverState(row.session_status, row.created_at, nowIso)

      return {
        artifactRef: row.artifact_ref,
        assignmentId: null,
        foldableIntoAutopilot: foldableForState(state),
        id: row.id,
        orderId: row.order_id,
        privacyState,
        reasonRefs: sourceReasonRef('site_builder_artifact', state, privacyState),
        siteId: row.site_id,
        sourceKind: 'site_builder_artifact',
        state,
        status: row.session_status,
        updatedAt: row.created_at,
      } satisfies FoldoverInventoryItem
    })
    const items = [
      ...softwareOrderItems,
      ...assignmentItems,
      ...siteItems,
      ...artifactItems,
    ].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))

    return {
      dryRun: true,
      generatedAt: nowIso,
      items,
      mutatesRecords: false,
      summary: foldoverInventorySummary(items),
    }
  })

export class OrderTriageService extends Context.Service<
  OrderTriageService,
  {
    readonly assignFirstBatch: (
      actorUserId: string,
      input: FirstBatchAssignmentRequest,
    ) => Effect.Effect<
      Readonly<{
        results: ReadonlyArray<FirstBatchAssignmentResult>
        summary: FirstBatchAssignmentSummary
      }>,
      | AdjutantAssignmentError
      | AutopilotSiteError
      | OrderTriageStorageError,
      AdjutantAssignmentService | AutopilotSitesService
    >
    readonly applyFirstBatchPaymentPolicies: (
      actorUserId: string,
      input: ApplyFirstBatchPaymentPolicyRequest,
    ) => Effect.Effect<
      ReadonlyArray<FirstBatchPaymentPolicyApplyResult>,
      | FirstBatchPaymentPolicyError
      | OperatorOrderTriageBadRequest
      | OrderTriageStorageError
    >
    readonly foldoverInventory: (
      limit: number,
    ) => Effect.Effect<FoldoverInventoryReport, OrderTriageStorageError>
    readonly monitorFirstBatch: (
      limit: number,
    ) => Effect.Effect<
      Readonly<{
        monitor: ReadonlyArray<FirstBatchMonitorItem>
        summary: Readonly<Record<FirstBatchMonitorState, number> & { total: number }>
      }>,
      OrderTriageStorageError
    >
    readonly prepareOrderFulfillment: (
      actorUserId: string,
      softwareOrderId: string,
      input: PrepareOrderFulfillmentRequest,
    ) => Effect.Effect<
      FirstBatchAssignmentResult,
      | AdjutantAssignmentError
      | AutopilotSiteError
      | OrderTriageSoftwareOrderNotFound
      | OrderTriageStorageError,
      AdjutantAssignmentService | AutopilotSitesService
    >
    readonly listQueue: (
      limit: number,
    ) => Effect.Effect<
      ReadonlyArray<OperatorOrderTriageRecord>,
      OrderTriageStorageError
    >
    readonly upsertRecord: (
      softwareOrderId: string,
      actorUserId: string,
      input: UpsertOrderTriageRequest,
    ) => Effect.Effect<OperatorOrderTriageRecord, OrderTriageError>
  }
>()('@openagentsinc/autopilot-omega/OrderTriageService') {
  static readonly layer = (
    env: OperatorOrderTriageEnv,
    runtime: OrderTriageRuntime = systemOrderTriageRuntime,
  ) =>
    Layer.succeed(OrderTriageService, {
      assignFirstBatch: Effect.fn('OrderTriageService.assignFirstBatch')(
        (actorUserId, input) =>
          assignFirstBatch(
            openAgentsDatabase(env),
            runtime,
            actorUserId,
            input,
          ),
      ),
      applyFirstBatchPaymentPolicies: Effect.fn(
        'OrderTriageService.applyFirstBatchPaymentPolicies',
      )((actorUserId, input) =>
        applyFirstBatchPaymentPolicies(
          openAgentsDatabase(env),
          runtime,
          actorUserId,
          input,
        ),
      ),
      listQueue: Effect.fn('OrderTriageService.listQueue')(limit =>
        readQueue(openAgentsDatabase(env), limit),
      ),
      foldoverInventory: Effect.fn(
        'OrderTriageService.foldoverInventory',
      )(limit =>
        foldoverInventory(openAgentsDatabase(env), runtime, limit)
      ),
      monitorFirstBatch: Effect.fn('OrderTriageService.monitorFirstBatch')(
        limit =>
          monitorFirstBatch(openAgentsDatabase(env), runtime, limit),
      ),
      prepareOrderFulfillment: Effect.fn(
        'OrderTriageService.prepareOrderFulfillment',
      )((actorUserId, softwareOrderId, input) =>
        prepareOrderFulfillment(
          openAgentsDatabase(env),
          runtime,
          actorUserId,
          softwareOrderId,
          input,
        ),
      ),
      upsertRecord: Effect.fn('OrderTriageService.upsertRecord')(
        (softwareOrderId, actorUserId, input) =>
          upsertTriageRecord(
            openAgentsDatabase(env),
            runtime,
            softwareOrderId,
            actorUserId,
            input,
          ),
      ),
    })
}

const routeErrorResponse = (
  error: OperatorOrderTriageRouteError,
): HttpResponse => {
  switch (error._tag) {
    case 'AdjutantAssignmentActiveExists':
      return noStoreJsonResponse(
        {
          assignmentId: error.assignmentId,
          error: 'active_assignment_exists',
          siteId: error.siteId,
          softwareOrderId: error.softwareOrderId,
        },
        { status: 409 },
      )
    case 'AdjutantAssignmentSiteNotFound':
    case 'AutopilotSiteProjectNotFound':
      return noStoreJsonResponse(
        { error: 'site_not_found', siteId: error.siteId },
        { status: 404 },
      )
    case 'AdjutantAssignmentSoftwareOrderNotFound':
    case 'AutopilotSiteSoftwareOrderNotFound':
    case 'OrderTriageSoftwareOrderNotFound':
      return noStoreJsonResponse(
        { error: 'software_order_not_found', softwareOrderId: error.softwareOrderId },
        { status: 404 },
      )
    case 'AdjutantAssignmentUnsafePayload':
    case 'AutopilotSiteUnsafePayload':
      return noStoreJsonResponse(
        { error: 'unsafe_payload_rejected' },
        { status: 400 },
      )
    case 'AdjutantAssignmentValidationError':
      return noStoreJsonResponse(
        { error: 'assignment_validation_error', reason: error.reason },
        { status: 400 },
      )
    case 'AutopilotSiteSlugUnavailable':
      return noStoreJsonResponse(
        { error: 'site_slug_unavailable', slug: error.slug },
        { status: 409 },
      )
    case 'FirstBatchPaymentPolicyUnsafe':
      return noStoreJsonResponse(
        { error: 'payment_policy_unsafe', reason: error.reason },
        { status: 400 },
      )
    case 'OperatorOrderTriageBadRequest':
      return noStoreJsonResponse(
        { error: 'bad_request', reason: error.reason },
        { status: 400 },
      )
    case 'OperatorOrderTriageForbidden':
      return noStoreJsonResponse({ error: 'forbidden' }, { status: 403 })
    case 'OperatorOrderTriageSessionError':
      return noStoreJsonResponse({ error: 'session_error' }, { status: 500 })
    case 'OperatorOrderTriageUnauthorized':
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    case 'AdjutantAssignmentStorageError':
    case 'AutopilotSiteStorageError':
    case 'FirstBatchPaymentPolicyStorageError':
    case 'OrderTriageStorageError':
      return noStoreJsonResponse({ error: 'storage_error' }, { status: 500 })
    default:
      return noStoreJsonResponse({ error: 'operator_triage_error' }, { status: 500 })
  }
}

const decodeJsonBody = <Schema extends S.Top>(
  request: Request,
  schema: Schema,
) =>
  Effect.gen(function* () {
    const payload = yield* Effect.tryPromise({
      catch: error =>
        new OperatorOrderTriageBadRequest({
          reason: error instanceof Error ? error.message : 'invalid json',
        }),
      try: () => request.json(),
    })

    return yield* S.decodeUnknownEffect(schema)(payload)
  }).pipe(
    Effect.mapError(error =>
      error instanceof OperatorOrderTriageBadRequest
        ? error
        : new OperatorOrderTriageBadRequest({
            reason: 'invalid request body',
          }),
    ),
  )

const requireAdminSession = <
  Session extends OperatorOrderTriageSession,
  Bindings extends OperatorOrderTriageEnv,
>(
  dependencies: OperatorOrderTriageRouteDependencies<Session, Bindings>,
  request: Request,
  env: Bindings,
  ctx: ExecutionContext,
) =>
  Effect.gen(function* () {
    const requireAdminApiToken = dependencies.requireAdminApiToken

    if (requireAdminApiToken !== undefined) {
      const hasAdminApiToken = yield* Effect.tryPromise({
        catch: error => new OperatorOrderTriageSessionError({ error }),
        try: () => requireAdminApiToken(request, env),
      })

      if (hasAdminApiToken === true) {
        return {
          user: {
            email: 'chris@openagents.com',
            userId: 'github:14167547',
          },
        } as Session
      }
    }

    const session = yield* Effect.tryPromise({
      catch: error => new OperatorOrderTriageSessionError({ error }),
      try: () => dependencies.requireBrowserSession(request, env, ctx),
    })

    if (session === undefined) {
      return yield* new OperatorOrderTriageUnauthorized({})
    }

    if (!dependencies.isOpenAgentsAdminEmail(session.user.email)) {
      return yield* new OperatorOrderTriageForbidden({})
    }

    return session
  })

const runRoute = (
  env: OperatorOrderTriageEnv,
  effect: Effect.Effect<
    HttpResponse,
    OperatorOrderTriageRouteError,
    | AdjutantAssignmentService
    | AutopilotSitesService
    | OrderTriageService
  >,
): Effect.Effect<HttpResponse> =>
  effect.pipe(
    Effect.provide(
      Layer.mergeAll(
        AdjutantAssignmentService.layer(env),
        AutopilotSitesService.layer(env),
        OrderTriageService.layer(env),
      ),
    ),
    Effect.catch(error => Effect.succeed(routeErrorResponse(error))),
  )

export const makeOperatorOrderTriageRoutes = <
  Session extends OperatorOrderTriageSession,
  Bindings extends OperatorOrderTriageEnv,
>(
  dependencies: OperatorOrderTriageRouteDependencies<Session, Bindings>,
) => {
  const listQueue = (request: Request, env: Bindings, ctx: ExecutionContext) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const triage = yield* OrderTriageService
        const queue = yield* triage.listQueue(
          numericLimit(new URL(request.url)),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ queue }),
          session,
        )
      }),
    )

  const upsertRecord = (
    softwareOrderId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'PATCH' && request.method !== 'PUT') {
          return methodNotAllowed(['PATCH', 'PUT'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(request, UpsertOrderTriageRequest)
        const triage = yield* OrderTriageService
        const record = yield* triage.upsertRecord(
          softwareOrderId,
          session.user.userId,
          body,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ record }),
          session,
        )
      }),
    )

  const assignFirstBatchRoute = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(request, FirstBatchAssignmentRequest)
        const triage = yield* OrderTriageService
        const result = yield* triage.assignFirstBatch(
          session.user.userId,
          body,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(result, { status: body.dryRun === true ? 200 : 201 }),
          session,
        )
      }),
    )

  const prepareOrderFulfillmentRoute = (
    softwareOrderId: string,
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          PrepareOrderFulfillmentRequest,
        )
        const triage = yield* OrderTriageService
        const result = yield* triage.prepareOrderFulfillment(
          session.user.userId,
          softwareOrderId,
          body,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ result }, {
            status: result.decision === 'created_assignment' ? 201 : 200,
          }),
          session,
        )
      }),
    )

  const applyFirstBatchPaymentPolicyRoute = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return methodNotAllowed(['POST'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const body = yield* decodeJsonBody(
          request,
          ApplyFirstBatchPaymentPolicyRequest,
        )
        const triage = yield* OrderTriageService
        const policies = yield* triage.applyFirstBatchPaymentPolicies(
          session.user.userId,
          body,
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ policies }, { status: 201 }),
          session,
        )
      }),
    )

  const monitorFirstBatchRoute = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const triage = yield* OrderTriageService
        const result = yield* triage.monitorFirstBatch(
          numericLimit(new URL(request.url)),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse(result),
          session,
        )
      }),
    )

  const foldoverInventoryRoute = (
    request: Request,
    env: Bindings,
    ctx: ExecutionContext,
  ) =>
    runRoute(
      env,
      Effect.gen(function* () {
        if (request.method !== 'GET') {
          return methodNotAllowed(['GET'])
        }

        const session = yield* requireAdminSession(
          dependencies,
          request,
          env,
          ctx,
        )
        const triage = yield* OrderTriageService
        const result = yield* triage.foldoverInventory(
          numericLimit(new URL(request.url)),
        )

        return dependencies.appendRefreshedSessionCookies(
          noStoreJsonResponse({ inventory: result }),
          session,
        )
      }),
    )

  return {
    routeOperatorOrderTriageRequest: (
      request: Request,
      env: Bindings,
      ctx: ExecutionContext,
    ): Effect.Effect<HttpResponse> | undefined => {
      const url = new URL(request.url)

      if (url.pathname === '/api/operator/orders/triage') {
        return listQueue(request, env, ctx)
      }

      if (url.pathname === '/api/operator/orders/triage/first-batch/assign') {
        return assignFirstBatchRoute(request, env, ctx)
      }

      if (
        url.pathname ===
        '/api/operator/orders/triage/first-batch/payment-policy'
      ) {
        return applyFirstBatchPaymentPolicyRoute(request, env, ctx)
      }

      if (
        url.pathname ===
        '/api/operator/orders/triage/autopilot-foldover-inventory'
      ) {
        return foldoverInventoryRoute(request, env, ctx)
      }

      if (
        url.pathname === '/api/operator/orders/triage/first-batch/monitor'
      ) {
        return monitorFirstBatchRoute(request, env, ctx)
      }

      const updateMatch = /^\/api\/operator\/orders\/([^/]+)\/triage$/.exec(
        url.pathname,
      )

      if (updateMatch !== null) {
        return upsertRecord(updateMatch[1] ?? '', request, env, ctx)
      }

      const prepareMatch =
        /^\/api\/operator\/orders\/([^/]+)\/fulfillment\/prepare$/.exec(
          url.pathname,
        )

      if (prepareMatch !== null) {
        return prepareOrderFulfillmentRoute(
          prepareMatch[1] ?? '',
          request,
          env,
          ctx,
        )
      }

      return undefined
    },
  }
}
