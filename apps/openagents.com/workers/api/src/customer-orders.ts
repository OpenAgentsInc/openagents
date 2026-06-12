import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import {
  AdjutantUsageReceiptBillingMode,
  AdjutantUsageReceiptCategory,
  type AdjutantUsageReceiptSummary,
  type CustomerAdjutantUsageReceipt,
  listCustomerAdjutantUsageReceiptsForOrder,
  summarizeAdjutantUsageReceipts,
} from './adjutant-usage-receipts'
import { optionalString, parseJsonRecord } from './json-boundary'
import {
  PublicClaimStateProjection,
  publicClaimStateProjection,
} from './public-claim-state'
import { openAgentsDatabase } from './runtime'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

type CustomerOrderEnv = Readonly<{
  OPENAGENTS_DB: D1Database
}>

export type CustomerOrderRuntime = Readonly<{
  makeAdjutantAdjustmentId: () => string
  makeAdjutantAssignmentEventId: () => string
  makeAdjutantAssignmentId: () => string
  makeOrderId: () => string
  makeSiteFeedbackId: () => string
  makeSiteEventId: () => string
  nowIso: () => string
}>

export const systemCustomerOrderRuntime: CustomerOrderRuntime = {
  makeAdjutantAdjustmentId: () => compactRandomId('adjutant_adjustment'),
  makeAdjutantAssignmentEventId: () =>
    compactRandomId('adjutant_assignment_event'),
  makeAdjutantAssignmentId: () => compactRandomId('adjutant_assignment'),
  makeOrderId: () => compactRandomId('software_order'),
  makeSiteFeedbackId: () => compactRandomId('site_feedback'),
  makeSiteEventId: () => compactRandomId('site_event'),
  nowIso: currentIsoTimestamp,
}

export const CustomerOrderStatus = S.Literals([
  'submitted',
  'scoping',
  'free_slice_ready',
  'quote_ready',
  'agent_queued',
  'agent_running',
  'delivered',
  'needs_customer_input',
  'declined',
  'unavailable',
])
export type CustomerOrderStatus = typeof CustomerOrderStatus.Type

export const CustomerOrderRepository = S.Struct({
  provider: S.Literal('github'),
  owner: S.String,
  name: S.String,
  fullName: S.String,
  private: S.Boolean,
  defaultBranch: S.String,
  htmlUrl: S.String,
})
export type CustomerOrderRepository = typeof CustomerOrderRepository.Type

export const CustomerOrderSite = S.Struct({
  id: S.String,
  status: S.String,
  activeUrl: S.NullOr(S.String),
  activeVersionId: S.NullOr(S.String),
  activeDeploymentId: S.NullOr(S.String),
  latestSavedVersionId: S.NullOr(S.String),
  latestBuildStatus: S.NullOr(S.String),
  feedbackCount: S.Number,
  openFeedbackCount: S.Number,
})
export type CustomerOrderSite = typeof CustomerOrderSite.Type

export const CustomerSiteRevisionReviewState = S.Literals([
  'runtime_verified',
  'internal_draft',
  'customer_review_ready',
  'customer_accepted',
])
export type CustomerSiteRevisionReviewState =
  typeof CustomerSiteRevisionReviewState.Type

export const CustomerSiteRevision = S.Struct({
  id: S.String,
  siteId: S.String,
  buildStatus: S.String,
  deploymentId: S.NullOr(S.String),
  deploymentStatus: S.NullOr(S.String),
  url: S.NullOr(S.String),
  active: S.Boolean,
  sourceCommitSha: S.NullOr(S.String),
  sourceHash: S.NullOr(S.String),
  reviewState: CustomerSiteRevisionReviewState,
  originSummary: S.NullOr(S.String),
  originCreatedAt: S.NullOr(S.String),
  createdAt: S.String,
  savedAt: S.NullOr(S.String),
  activatedAt: S.NullOr(S.String),
})
export type CustomerSiteRevision = typeof CustomerSiteRevision.Type

export const CustomerSiteFeedbackStatus = S.Literals([
  'submitted',
  'queued',
  'running',
  'addressed',
  'closed',
  'rejected',
])
export type CustomerSiteFeedbackStatus = typeof CustomerSiteFeedbackStatus.Type

export const CustomerSiteFeedback = S.Struct({
  id: S.String,
  orderId: S.String,
  siteId: S.NullOr(S.String),
  versionId: S.NullOr(S.String),
  deploymentId: S.NullOr(S.String),
  body: S.String,
  status: CustomerSiteFeedbackStatus,
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerSiteFeedback = typeof CustomerSiteFeedback.Type

export const SubmitCustomerSiteFeedbackRequest = S.Struct({
  body: S.String,
})
export type SubmitCustomerSiteFeedbackRequest =
  typeof SubmitCustomerSiteFeedbackRequest.Type

export const CustomerOrderTriageProjection = S.Struct({
  status: S.String,
  summary: S.String,
  nextAction: S.String,
})
export type CustomerOrderTriageProjection =
  typeof CustomerOrderTriageProjection.Type

export const CustomerOrderAdjutantStage = S.Literals([
  'queued',
  'running',
  'reviewing',
  'deployed',
  'waiting_for_input',
  'unavailable',
])
export type CustomerOrderAdjutantStage = typeof CustomerOrderAdjutantStage.Type

export const CustomerOrderAdjutantProgress = S.Struct({
  stage: CustomerOrderAdjutantStage,
  orderStatus: CustomerOrderStatus,
  siteStatus: S.NullOr(S.String),
  activeUrl: S.NullOr(S.String),
  adjustmentStatus: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
  reviewNeeded: S.Boolean,
  inputNeeded: S.Boolean,
  nextAction: S.String,
})
export type CustomerOrderAdjutantProgress =
  typeof CustomerOrderAdjutantProgress.Type

export const CustomerOrderUsageReceipt = S.Struct({
  id: S.String,
  category: AdjutantUsageReceiptCategory,
  summary: S.String,
  quantity: S.Number,
  unit: S.String,
  billingMode: AdjutantUsageReceiptBillingMode,
  creditsChargedCents: S.Number,
  creditsChargedFormatted: S.String,
  details: S.Record(S.String, S.Unknown),
  createdAt: S.String,
})
export type CustomerOrderUsageReceipt = typeof CustomerOrderUsageReceipt.Type

export const CustomerOrderUsageReceiptCategoryTotal = S.Struct({
  category: AdjutantUsageReceiptCategory,
  quantity: S.Number,
  unit: S.NullOr(S.String),
  receiptCount: S.Number,
  creditsChargedCents: S.Number,
  creditsChargedFormatted: S.String,
})
export type CustomerOrderUsageReceiptCategoryTotal =
  typeof CustomerOrderUsageReceiptCategoryTotal.Type

export const CustomerOrderUsageSummary = S.Struct({
  billingMode: AdjutantUsageReceiptBillingMode,
  totalCreditsChargedCents: S.Number,
  totalCreditsChargedFormatted: S.String,
  categories: S.Array(CustomerOrderUsageReceiptCategoryTotal),
})
export type CustomerOrderUsageSummary = typeof CustomerOrderUsageSummary.Type

export const CustomerOrder = S.Struct({
  id: S.String,
  status: CustomerOrderStatus,
  visibility: S.Literal('public'),
  request: S.String,
  repository: S.NullOr(CustomerOrderRepository),
  site: S.NullOr(CustomerOrderSite),
  triage: S.NullOr(CustomerOrderTriageProjection),
  adjutant: CustomerOrderAdjutantProgress,
  usageReceipts: S.Array(CustomerOrderUsageReceipt),
  usageSummary: CustomerOrderUsageSummary,
  publicWorkAcknowledgedAt: S.String,
  dataUseAcknowledgedAt: S.String,
  computePaymentAcknowledgedAt: S.String,
  providerAccountRequired: S.Boolean,
  freeSliceCents: S.Number,
  quoteCents: S.NullOr(S.Number),
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerOrder = typeof CustomerOrder.Type

export const CustomerOrderResponse = S.Struct({
  order: S.NullOr(CustomerOrder),
})
export type CustomerOrderResponse = typeof CustomerOrderResponse.Type

export const CustomerOrdersResponse = S.Struct({
  orders: S.Array(CustomerOrder),
})
export type CustomerOrdersResponse = typeof CustomerOrdersResponse.Type

export const CreateCustomerOrderRequest = S.Struct({
  request: S.String,
})
export type CreateCustomerOrderRequest =
  typeof CreateCustomerOrderRequest.Type

export const CustomerFulfillmentArtifactKind = S.Literals([
  'pull_request',
  'branch',
  'commit',
  'diff',
  'preview',
  'notes',
  'attachment',
])
export type CustomerFulfillmentArtifactKind =
  typeof CustomerFulfillmentArtifactKind.Type

export const CustomerFulfillmentArtifactStatus = S.Literals([
  'draft',
  'customer_review_ready',
  'customer_accepted',
  'superseded',
  'rejected',
])
export type CustomerFulfillmentArtifactStatus =
  typeof CustomerFulfillmentArtifactStatus.Type

export const CustomerFulfillmentArtifact = S.Struct({
  id: S.String,
  orderId: S.String,
  kind: CustomerFulfillmentArtifactKind,
  title: S.String,
  summary: S.String,
  url: S.NullOr(S.String),
  repositoryFullName: S.NullOr(S.String),
  sourceBranch: S.NullOr(S.String),
  targetBranch: S.NullOr(S.String),
  commitSha: S.NullOr(S.String),
  status: CustomerFulfillmentArtifactStatus,
  createdAt: S.String,
  updatedAt: S.String,
})
export type CustomerFulfillmentArtifact =
  typeof CustomerFulfillmentArtifact.Type

export const CustomerFulfillmentArtifactsResponse = S.Struct({
  artifacts: S.Array(CustomerFulfillmentArtifact),
})
export type CustomerFulfillmentArtifactsResponse =
  typeof CustomerFulfillmentArtifactsResponse.Type

type OrderRow = Readonly<{
  id: string
  status: CustomerOrderStatus
  visibility: 'public'
  request: string
  repository_provider: 'github' | null
  repository_owner: string | null
  repository_name: string | null
  repository_full_name: string | null
  repository_private: number | null
  repository_default_branch: string | null
  repository_html_url: string | null
  site_id: string | null
  site_status: string | null
  site_active_url: string | null
  site_active_version_id: string | null
  site_active_deployment_id: string | null
  site_latest_saved_version_id: string | null
  site_latest_build_status: string | null
  site_feedback_count: number | null
  site_open_feedback_count: number | null
  triage_customer_safe_status: string | null
  triage_customer_safe_summary: string | null
  triage_next_action: string | null
  public_work_acknowledged_at: string
  data_use_acknowledged_at: string
  compute_payment_acknowledged_at: string
  provider_account_required: number
  free_slice_cents: number
  quote_cents: number | null
  latest_adjustment_status: string | null
  created_at: string
  updated_at: string
}>

type CustomerOrderSiteContextRow = Readonly<{
  order_id: string
  site_id: string | null
  active_version_id: string | null
  active_deployment_id: string | null
}>

type CustomerOrderAdjutantAssignmentRow = Readonly<{
  current_run_id: string | null
  goal_id: string | null
  id: string
  objective: string
  site_id: string | null
  software_order_id: string | null
  status: string
  visibility: 'private' | 'team' | 'public'
}>

type SiteRevisionRow = Readonly<{
  id: string
  site_id: string
  build_status: string
  deployment_id: string | null
  deployment_status: string | null
  deployment_url: string | null
  source_commit_sha: string | null
  metadata_json: string
  origin_feedback_body: string | null
  origin_feedback_created_at: string | null
  created_at: string
  saved_at: string | null
  activated_at: string | null
}>

type SiteFeedbackRow = Readonly<{
  id: string
  software_order_id: string
  site_id: string | null
  site_version_id: string | null
  site_deployment_id: string | null
  body: string
  status: CustomerSiteFeedbackStatus
  created_at: string
  updated_at: string
}>

type FulfillmentArtifactRow = Readonly<{
  id: string
  software_order_id: string
  kind: CustomerFulfillmentArtifactKind
  title: string
  summary: string
  url: string | null
  repository_full_name: string | null
  source_branch: string | null
  target_branch: string | null
  commit_sha: string | null
  status: CustomerFulfillmentArtifactStatus
  created_at: string
  updated_at: string
}>

type ExistingSiteFeedbackRow = SiteFeedbackRow &
  Readonly<{
    adjutant_adjustment_id: string | null
  }>

type OnboardingOrderSourceRow = Readonly<{
  onboarding_completed_at: string | null
  onboarding_goal: string | null
  onboarding_repository_provider: 'github' | null
  onboarding_repository_owner: string | null
  onboarding_repository_name: string | null
  onboarding_repository_full_name: string | null
  onboarding_repository_private: number | null
  onboarding_repository_default_branch: string | null
  onboarding_repository_html_url: string | null
}>

export class CustomerOrderStorageError extends S.TaggedErrorClass<CustomerOrderStorageError>()(
  'CustomerOrderStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, CustomerOrderStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error => new CustomerOrderStorageError({ operation, error }),
  })

const repositoryFromOrderRow = (
  row: OrderRow,
): CustomerOrderRepository | null => {
  if (
    row.repository_provider !== 'github' ||
    row.repository_owner === null ||
    row.repository_name === null ||
    row.repository_full_name === null
  ) {
    return null
  }

  return {
    provider: 'github',
    owner: row.repository_owner,
    name: row.repository_name,
    fullName: row.repository_full_name,
    private: row.repository_private === 1,
    defaultBranch: row.repository_default_branch ?? 'main',
    htmlUrl:
      row.repository_html_url ??
      `https://github.com/${row.repository_full_name}`,
  }
}

const triageFromOrderRow = (
  row: OrderRow,
): CustomerOrderTriageProjection | null => {
  if (
    row.triage_customer_safe_status === null ||
    row.triage_customer_safe_summary === null ||
    row.triage_next_action === null
  ) {
    return null
  }

  return {
    status: row.triage_customer_safe_status,
    summary: row.triage_customer_safe_summary,
    nextAction: row.triage_next_action,
  }
}

const adjutantProgressFromOrderRow = (
  row: OrderRow,
): CustomerOrderAdjutantProgress => {
  const siteStatus = row.site_status
  const activeUrl = row.site_active_url
  const adjustmentStatus = row.latest_adjustment_status ?? null
  const claimState = (
    stage: CustomerOrderAdjutantStage,
  ): PublicClaimStateProjection =>
    publicClaimStateProjection({
      desiredState:
        stage === 'deployed'
          ? 'verified'
          : stage === 'queued'
            ? 'planned'
            : 'measured',
      evidenceRefs: [
        `order:${row.id}`,
        ...(activeUrl === null ? [] : [activeUrl]),
        ...(siteStatus === null ? [] : [`site_status:${siteStatus}`]),
      ],
      kind: stage === 'deployed' ? 'site_url' : 'deployment',
    })

  if (row.status === 'needs_customer_input') {
    const stage = 'waiting_for_input'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: true,
      nextAction: 'Reply with the details OpenAgents requested.',
    }
  }

  if (adjustmentStatus === 'requested' || adjustmentStatus === 'queued') {
    const stage = 'queued'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'Autopilot is queued to apply the requested Site adjustment.',
    }
  }

  if (adjustmentStatus === 'running') {
    const stage = 'running'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'Autopilot is applying the requested Site adjustment.',
    }
  }

  if (adjustmentStatus === 'review_needed') {
    const stage = 'reviewing'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: true,
      inputNeeded: false,
      nextAction: 'OpenAgents is reviewing the adjusted Site version.',
    }
  }

  if (activeUrl !== null) {
    const stage = 'deployed'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'Open the live Site and send any adjustment request.',
    }
  }

  if (row.status === 'agent_running') {
    const stage = 'running'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'Autopilot is building the Site version.',
    }
  }

  if (row.status === 'agent_queued' || row.status === 'submitted') {
    const stage = 'queued'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'Autopilot is queued for this order.',
    }
  }

  if (row.status === 'declined' || row.status === 'unavailable') {
    const stage = 'unavailable'

    return {
      stage,
      orderStatus: row.status,
      siteStatus,
      activeUrl,
      adjustmentStatus,
      claimState: claimState(stage),
      reviewNeeded: false,
      inputNeeded: false,
      nextAction: 'OpenAgents cannot continue this order right now.',
    }
  }

  const stage = 'reviewing'

  return {
    stage,
    orderStatus: row.status,
    siteStatus,
    activeUrl,
    adjustmentStatus,
    claimState: claimState(stage),
    reviewNeeded: true,
    inputNeeded: false,
    nextAction: 'OpenAgents is reviewing the generated Site before release.',
  }
}

const orderFromRow = (
  row: OrderRow,
  usageReceipts: ReadonlyArray<CustomerAdjutantUsageReceipt>,
  usageSummary: AdjutantUsageReceiptSummary,
): CustomerOrder => ({
  id: row.id,
  status: row.status,
  visibility: 'public',
  request: row.request,
  repository: repositoryFromOrderRow(row),
  site:
    row.site_status === null
      ? null
      : {
          id: row.site_id ?? '',
          status: row.site_status,
          activeUrl: row.site_active_url,
          activeVersionId: row.site_active_version_id ?? null,
          activeDeploymentId: row.site_active_deployment_id ?? null,
          latestSavedVersionId: row.site_latest_saved_version_id ?? null,
          latestBuildStatus: row.site_latest_build_status ?? null,
          feedbackCount: row.site_feedback_count ?? 0,
          openFeedbackCount: row.site_open_feedback_count ?? 0,
        },
  triage: triageFromOrderRow(row),
  adjutant: adjutantProgressFromOrderRow(row),
  usageReceipts,
  usageSummary,
  publicWorkAcknowledgedAt: row.public_work_acknowledged_at,
  dataUseAcknowledgedAt: row.data_use_acknowledged_at,
  computePaymentAcknowledgedAt: row.compute_payment_acknowledged_at,
  providerAccountRequired: row.provider_account_required === 1,
  freeSliceCents: row.free_slice_cents,
  quoteCents: row.quote_cents,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const sourceHashFromVersionMetadata = (metadataJson: string): string | null => {
  const sha256 = optionalString(parseJsonRecord(metadataJson)?.sha256)

  if (sha256 === undefined) {
    return null
  }

  return sha256.startsWith('sha256:') ? sha256 : `sha256:${sha256}`
}

const compactCustomerSafeText = (
  value: string | null | undefined,
  maxLength: number,
): string | null => {
  if (value === null || value === undefined) {
    return null
  }

  const normalized = value.replace(/\s+/g, ' ').trim()

  if (normalized === '' || containsProviderSecretMaterial(normalized)) {
    return null
  }

  return normalized.length <= maxLength
    ? normalized
    : `${normalized.slice(0, Math.max(0, maxLength - 1)).trimEnd()}...`
}

const originSummaryFromVersionMetadata = (
  metadataJson: string,
): string | null => {
  const metadata = parseJsonRecord(metadataJson)

  if (metadata === null || metadata === undefined) {
    return null
  }

  return (
    compactCustomerSafeText(optionalString(metadata.originSummary), 220) ??
    compactCustomerSafeText(optionalString(metadata.customerPromptSummary), 220) ??
    compactCustomerSafeText(optionalString(metadata.feedbackSummary), 220)
  )
}

const reviewStateFromVersionRow = (
  row: SiteRevisionRow,
  activeVersionId: string | null,
): CustomerSiteRevisionReviewState => {
  const state = optionalString(
    parseJsonRecord(row.metadata_json)?.customerReviewState,
  )

  if (
    state === 'runtime_verified' ||
    state === 'internal_draft' ||
    state === 'customer_review_ready' ||
    state === 'customer_accepted'
  ) {
    return state
  }

  return row.id === activeVersionId && row.deployment_status === 'active'
    ? 'runtime_verified'
    : 'internal_draft'
}

const revisionFromRow = (
  row: SiteRevisionRow,
  activeVersionId: string | null,
): CustomerSiteRevision => {
  const active = row.id === activeVersionId
  const deploymentStatus = active ? row.deployment_status : null
  const deploymentUrl =
    row.deployment_url === null ? null : new URL(row.deployment_url)
  const deploymentSlug =
    deploymentUrl?.pathname
      .split('/')
      .filter(segment => segment !== '')[0] ?? null
  const url =
    deploymentUrl === null || deploymentSlug === null
      ? null
      : `${deploymentUrl.origin}/${deploymentSlug}/versions/${row.id}`

  return {
    id: row.id,
    siteId: row.site_id,
    buildStatus: row.build_status,
    deploymentId: row.deployment_id,
    deploymentStatus,
    url,
    active,
    sourceCommitSha: row.source_commit_sha,
    sourceHash: sourceHashFromVersionMetadata(row.metadata_json),
    reviewState: reviewStateFromVersionRow(row, activeVersionId),
    originSummary:
      compactCustomerSafeText(row.origin_feedback_body, 220) ??
      originSummaryFromVersionMetadata(row.metadata_json),
    originCreatedAt: row.origin_feedback_created_at,
    createdAt: row.created_at,
    savedAt: row.saved_at,
    activatedAt: row.activated_at,
  }
}

const feedbackFromRow = (row: SiteFeedbackRow): CustomerSiteFeedback => ({
  id: row.id,
  orderId: row.software_order_id,
  siteId: row.site_id,
  versionId: row.site_version_id,
  deploymentId: row.site_deployment_id,
  body: row.body,
  status: row.status,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const artifactFromRow = (
  row: FulfillmentArtifactRow,
): CustomerFulfillmentArtifact => ({
  id: row.id,
  orderId: row.software_order_id,
  kind: row.kind,
  title: row.title,
  summary: row.summary,
  url: row.url,
  repositoryFullName: row.repository_full_name,
  sourceBranch: row.source_branch,
  targetBranch: row.target_branch,
  commitSha: row.commit_sha,
  status: row.status,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

const customerFeedbackAdjustmentInstruction = (
  feedback: Readonly<{
    body: string
    id: string
    orderId: string
    siteId: string
    versionId: string | null
  }>,
): string =>
  [
    `Customer Site revision feedback ${feedback.id}`,
    `Software order ID: ${feedback.orderId}`,
    `Site ID: ${feedback.siteId}`,
    `Revision ID: ${feedback.versionId ?? 'none'}`,
    '',
    'Apply this customer-requested Site adjustment through the existing Sites version lifecycle.',
    'Save the adjusted output as a new Site version for review before deployment.',
    '',
    'Customer feedback:',
    feedback.body,
  ].join('\n')

const readCustomerUsageReceipts = (
  db: D1Database,
  orderId: string,
): Effect.Effect<
  ReadonlyArray<CustomerAdjutantUsageReceipt>,
  CustomerOrderStorageError
> =>
  listCustomerAdjutantUsageReceiptsForOrder(db, orderId, 50).pipe(
    Effect.mapError(
      error =>
        new CustomerOrderStorageError({
          error,
          operation: 'customerOrders.usageReceipts.read',
        }),
    ),
  )

const orderFromRowWithUsage = (
  db: D1Database,
  row: OrderRow,
): Effect.Effect<CustomerOrder, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const usageReceipts = yield* readCustomerUsageReceipts(db, row.id)

    return orderFromRow(
      row,
      usageReceipts,
      summarizeAdjutantUsageReceipts(usageReceipts),
    )
  })

const readActiveOrderRow = (
  db: D1Database,
  userId: string,
): Effect.Effect<OrderRow | null, CustomerOrderStorageError> =>
  d1Effect('customerOrders.active.read', () =>
    db
      .prepare(
        `SELECT software_orders.id AS id,
                software_orders.status AS status,
                software_orders.visibility AS visibility,
                software_orders.request AS request,
                software_orders.repository_provider AS repository_provider,
                software_orders.repository_owner AS repository_owner,
                software_orders.repository_name AS repository_name,
                software_orders.repository_full_name AS repository_full_name,
                software_orders.repository_private AS repository_private,
                software_orders.repository_default_branch AS repository_default_branch,
                software_orders.repository_html_url AS repository_html_url,
                site_projects.id AS site_id,
                site_projects.status AS site_status,
                site_projects.active_version_id AS site_active_version_id,
                site_projects.active_deployment_id AS site_active_deployment_id,
                active_site_deployments.url AS site_active_url,
                (SELECT version.id
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                    AND version.build_status = 'saved'
                    AND version.rejected_at IS NULL
                  ORDER BY version.saved_at DESC, version.created_at DESC
                  LIMIT 1) AS site_latest_saved_version_id,
                (SELECT version.build_status
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                  ORDER BY version.created_at DESC
                  LIMIT 1) AS site_latest_build_status,
                0 AS site_feedback_count,
                0 AS site_open_feedback_count,
                active_order_triage.customer_safe_status AS triage_customer_safe_status,
                active_order_triage.customer_safe_summary AS triage_customer_safe_summary,
                active_order_triage.next_action AS triage_next_action,
                software_orders.public_work_acknowledged_at AS public_work_acknowledged_at,
                software_orders.data_use_acknowledged_at AS data_use_acknowledged_at,
                software_orders.compute_payment_acknowledged_at AS compute_payment_acknowledged_at,
                software_orders.provider_account_required AS provider_account_required,
                software_orders.free_slice_cents AS free_slice_cents,
                software_orders.quote_cents AS quote_cents,
                (SELECT adjustment.status
                   FROM adjutant_adjustment_requests AS adjustment
                  WHERE adjustment.software_order_id = software_orders.id
                    AND adjustment.archived_at IS NULL
                    AND adjustment.visibility = 'public'
                  ORDER BY adjustment.created_at DESC
                  LIMIT 1) AS latest_adjustment_status,
                software_orders.created_at AS created_at,
                software_orders.updated_at AS updated_at
           FROM software_orders
           LEFT JOIN site_projects
             ON site_projects.software_order_id = software_orders.id
            AND site_projects.archived_at IS NULL
           LEFT JOIN site_deployments AS active_site_deployments
             ON active_site_deployments.id = site_projects.active_deployment_id
            AND active_site_deployments.status = 'active'
           LEFT JOIN order_triage_records AS active_order_triage
             ON active_order_triage.software_order_id = software_orders.id
            AND active_order_triage.archived_at IS NULL
          WHERE software_orders.user_id = ?
            AND software_orders.archived_at IS NULL
          ORDER BY software_orders.created_at DESC
          LIMIT 1`,
      )
      .bind(userId)
      .first<OrderRow>(),
  )

const readOrderRowById = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<OrderRow | null, CustomerOrderStorageError> =>
  d1Effect('customerOrders.detail.read', () =>
    db
      .prepare(
        `SELECT software_orders.id AS id,
                software_orders.status AS status,
                software_orders.visibility AS visibility,
                software_orders.request AS request,
                software_orders.repository_provider AS repository_provider,
                software_orders.repository_owner AS repository_owner,
                software_orders.repository_name AS repository_name,
                software_orders.repository_full_name AS repository_full_name,
                software_orders.repository_private AS repository_private,
                software_orders.repository_default_branch AS repository_default_branch,
                software_orders.repository_html_url AS repository_html_url,
                site_projects.id AS site_id,
                site_projects.status AS site_status,
                site_projects.active_version_id AS site_active_version_id,
                site_projects.active_deployment_id AS site_active_deployment_id,
                active_site_deployments.url AS site_active_url,
                (SELECT version.id
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                    AND version.build_status = 'saved'
                    AND version.rejected_at IS NULL
                  ORDER BY version.saved_at DESC, version.created_at DESC
                  LIMIT 1) AS site_latest_saved_version_id,
                (SELECT version.build_status
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                  ORDER BY version.created_at DESC
                  LIMIT 1) AS site_latest_build_status,
                0 AS site_feedback_count,
                0 AS site_open_feedback_count,
                active_order_triage.customer_safe_status AS triage_customer_safe_status,
                active_order_triage.customer_safe_summary AS triage_customer_safe_summary,
                active_order_triage.next_action AS triage_next_action,
                software_orders.public_work_acknowledged_at AS public_work_acknowledged_at,
                software_orders.data_use_acknowledged_at AS data_use_acknowledged_at,
                software_orders.compute_payment_acknowledged_at AS compute_payment_acknowledged_at,
                software_orders.provider_account_required AS provider_account_required,
                software_orders.free_slice_cents AS free_slice_cents,
                software_orders.quote_cents AS quote_cents,
                (SELECT adjustment.status
                   FROM adjutant_adjustment_requests AS adjustment
                  WHERE adjustment.software_order_id = software_orders.id
                    AND adjustment.archived_at IS NULL
                    AND adjustment.visibility = 'public'
                  ORDER BY adjustment.created_at DESC
                  LIMIT 1) AS latest_adjustment_status,
                software_orders.created_at AS created_at,
                software_orders.updated_at AS updated_at
           FROM software_orders
           LEFT JOIN site_projects
             ON site_projects.software_order_id = software_orders.id
            AND site_projects.archived_at IS NULL
           LEFT JOIN site_deployments AS active_site_deployments
             ON active_site_deployments.id = site_projects.active_deployment_id
            AND active_site_deployments.status = 'active'
           LEFT JOIN order_triage_records AS active_order_triage
             ON active_order_triage.software_order_id = software_orders.id
            AND active_order_triage.archived_at IS NULL
          WHERE software_orders.user_id = ?
            AND software_orders.id = ?
            AND software_orders.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(userId, orderId)
      .first<OrderRow>(),
  )

const listOrderRows = (
  db: D1Database,
  userId: string,
): Effect.Effect<ReadonlyArray<OrderRow>, CustomerOrderStorageError> =>
  d1Effect('customerOrders.list.read', () =>
    db
      .prepare(
        `SELECT software_orders.id AS id,
                software_orders.status AS status,
                software_orders.visibility AS visibility,
                software_orders.request AS request,
                software_orders.repository_provider AS repository_provider,
                software_orders.repository_owner AS repository_owner,
                software_orders.repository_name AS repository_name,
                software_orders.repository_full_name AS repository_full_name,
                software_orders.repository_private AS repository_private,
                software_orders.repository_default_branch AS repository_default_branch,
                software_orders.repository_html_url AS repository_html_url,
                site_projects.id AS site_id,
                site_projects.status AS site_status,
                site_projects.active_version_id AS site_active_version_id,
                site_projects.active_deployment_id AS site_active_deployment_id,
                active_site_deployments.url AS site_active_url,
                (SELECT version.id
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                    AND version.build_status = 'saved'
                    AND version.rejected_at IS NULL
                  ORDER BY version.saved_at DESC, version.created_at DESC
                  LIMIT 1) AS site_latest_saved_version_id,
                (SELECT version.build_status
                   FROM site_versions AS version
                  WHERE version.site_id = site_projects.id
                  ORDER BY version.created_at DESC
                  LIMIT 1) AS site_latest_build_status,
                0 AS site_feedback_count,
                0 AS site_open_feedback_count,
                active_order_triage.customer_safe_status AS triage_customer_safe_status,
                active_order_triage.customer_safe_summary AS triage_customer_safe_summary,
                active_order_triage.next_action AS triage_next_action,
                software_orders.public_work_acknowledged_at AS public_work_acknowledged_at,
                software_orders.data_use_acknowledged_at AS data_use_acknowledged_at,
                software_orders.compute_payment_acknowledged_at AS compute_payment_acknowledged_at,
                software_orders.provider_account_required AS provider_account_required,
                software_orders.free_slice_cents AS free_slice_cents,
                software_orders.quote_cents AS quote_cents,
                (SELECT adjustment.status
                   FROM adjutant_adjustment_requests AS adjustment
                  WHERE adjustment.software_order_id = software_orders.id
                    AND adjustment.archived_at IS NULL
                    AND adjustment.visibility = 'public'
                  ORDER BY adjustment.created_at DESC
                  LIMIT 1) AS latest_adjustment_status,
                software_orders.created_at AS created_at,
                software_orders.updated_at AS updated_at
           FROM software_orders
           LEFT JOIN site_projects
             ON site_projects.software_order_id = software_orders.id
            AND site_projects.archived_at IS NULL
           LEFT JOIN site_deployments AS active_site_deployments
             ON active_site_deployments.id = site_projects.active_deployment_id
            AND active_site_deployments.status = 'active'
           LEFT JOIN order_triage_records AS active_order_triage
             ON active_order_triage.software_order_id = software_orders.id
            AND active_order_triage.archived_at IS NULL
          WHERE software_orders.user_id = ?
            AND software_orders.archived_at IS NULL
          ORDER BY software_orders.created_at DESC
          LIMIT 50`,
      )
      .bind(userId)
      .all<OrderRow>()
      .then(result => result.results ?? []),
  )

const readOnboardingOrderSource = (
  db: D1Database,
  userId: string,
): Effect.Effect<OnboardingOrderSourceRow | null, CustomerOrderStorageError> =>
  d1Effect('customerOrders.onboardingSource.read', () =>
    db
      .prepare(
        `SELECT onboarding_completed_at,
                onboarding_goal,
                onboarding_repository_provider,
                onboarding_repository_owner,
                onboarding_repository_name,
                onboarding_repository_full_name,
                onboarding_repository_private,
                onboarding_repository_default_branch,
                onboarding_repository_html_url
           FROM users
          WHERE id = ?
            AND kind = 'human'
            AND deleted_at IS NULL
          LIMIT 1`,
      )
      .bind(userId)
      .first<OnboardingOrderSourceRow>(),
  )

const repositoryFromOnboardingSource = (
  row: OnboardingOrderSourceRow,
): CustomerOrderRepository | null => {
  if (
    row.onboarding_repository_provider !== 'github' ||
    row.onboarding_repository_owner === null ||
    row.onboarding_repository_name === null
  ) {
    return null
  }

  const fullName =
    row.onboarding_repository_full_name ??
    `${row.onboarding_repository_owner}/${row.onboarding_repository_name}`

  return {
    provider: 'github',
    owner: row.onboarding_repository_owner,
    name: row.onboarding_repository_name,
    fullName,
    private: row.onboarding_repository_private === 1,
    defaultBranch: row.onboarding_repository_default_branch ?? 'main',
    htmlUrl:
      row.onboarding_repository_html_url ?? `https://github.com/${fullName}`,
  }
}

const insertOrderFromOnboarding = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  userId: string,
  source: OnboardingOrderSourceRow,
): Effect.Effect<CustomerOrder | null, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const request = source.onboarding_goal?.trim() ?? ''

    if (source.onboarding_completed_at === null || request === '') {
      return null
    }

    const id = runtime.makeOrderId()
    const now = runtime.nowIso()
    const repository = repositoryFromOnboardingSource(source)

    yield* d1Effect('customerOrders.active.insertFromOnboarding', () =>
      db
        .prepare(
          `INSERT INTO software_orders
             (id,
              user_id,
              status,
              visibility,
              request,
              repository_provider,
              repository_owner,
              repository_name,
              repository_full_name,
              repository_private,
              repository_default_branch,
              repository_html_url,
              public_work_acknowledged_at,
              data_use_acknowledged_at,
              compute_payment_acknowledged_at,
              provider_account_required,
              free_slice_cents,
              quote_cents,
              current_run_id,
              agent_started_at,
              created_at,
              updated_at)
           VALUES (?, ?, 'submitted', 'public', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 5000, NULL, NULL, NULL, ?, ?)`,
        )
        .bind(
          id,
          userId,
          request,
          repository?.provider ?? null,
          repository?.owner ?? null,
          repository?.name ?? null,
          repository?.fullName ?? null,
          repository === null ? null : repository.private ? 1 : 0,
          repository?.defaultBranch ?? null,
          repository?.htmlUrl ?? null,
          source.onboarding_completed_at,
          source.onboarding_completed_at,
          source.onboarding_completed_at,
          now,
          now,
        )
        .run(),
    )

    const row = yield* readActiveOrderRow(db, userId)

    return row === null ? null : yield* orderFromRowWithUsage(db, row)
  })

const insertOrderFromRequest = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  userId: string,
  request: string,
  agentIdempotencyKey?: string,
): Effect.Effect<CustomerOrder, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const id = runtime.makeOrderId()
    const now = runtime.nowIso()
    const source = yield* readOnboardingOrderSource(db, userId)
    const repository =
      source === null ? null : repositoryFromOnboardingSource(source)
    const acknowledgedAt = source?.onboarding_completed_at ?? now

    yield* d1Effect('customerOrders.create.insert', () =>
      db
        .prepare(
          `INSERT INTO software_orders
             (id,
              user_id,
              status,
              visibility,
              request,
              repository_provider,
              repository_owner,
              repository_name,
              repository_full_name,
              repository_private,
              repository_default_branch,
              repository_html_url,
              public_work_acknowledged_at,
              data_use_acknowledged_at,
              compute_payment_acknowledged_at,
              provider_account_required,
              free_slice_cents,
              quote_cents,
              current_run_id,
              agent_started_at,
              agent_idempotency_key,
              created_at,
              updated_at)
           VALUES (?, ?, 'submitted', 'public', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 5000, NULL, NULL, NULL, ?, ?, ?)`,
        )
        .bind(
          id,
          userId,
          request,
          repository?.provider ?? null,
          repository?.owner ?? null,
          repository?.name ?? null,
          repository?.fullName ?? null,
          repository === null ? null : repository.private ? 1 : 0,
          repository?.defaultBranch ?? null,
          repository?.htmlUrl ?? null,
          acknowledgedAt,
          acknowledgedAt,
          acknowledgedAt,
          agentIdempotencyKey ?? null,
          now,
          now,
        )
        .run(),
    )

    const row = yield* readOrderRowById(db, userId, id)

    if (row === null) {
      return yield* new CustomerOrderStorageError({
        error: new Error('created order was not readable'),
        operation: 'customerOrders.create.read',
      })
    }

    return yield* orderFromRowWithUsage(db, row)
  })

const readOrderByAgentIdempotencyKey = (
  db: D1Database,
  userId: string,
  idempotencyKey: string,
): Effect.Effect<CustomerOrder | null, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const row = yield* d1Effect('customerOrders.agentIdempotency.read', () =>
      db
        .prepare(
          `SELECT id
             FROM software_orders
            WHERE user_id = ?
              AND agent_idempotency_key = ?
              AND archived_at IS NULL
            ORDER BY created_at DESC
            LIMIT 1`,
        )
        .bind(userId, idempotencyKey)
        .first<{ id: string }>(),
    )

    return row === null ? null : yield* readOrderById(db, userId, row.id)
  })

const readOrCreateActiveOrder = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  userId: string,
): Effect.Effect<CustomerOrder | null, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const active = yield* readActiveOrderRow(db, userId)

    if (active !== null) {
      return yield* orderFromRowWithUsage(db, active)
    }

    const source = yield* readOnboardingOrderSource(db, userId)

    return source === null
      ? null
      : yield* insertOrderFromOnboarding(db, runtime, userId, source)
  })

const readOrderById = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<CustomerOrder | null, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const row = yield* readOrderRowById(db, userId, orderId)

    return row === null ? null : yield* orderFromRowWithUsage(db, row)
  })

const listOrders = (
  db: D1Database,
  userId: string,
): Effect.Effect<ReadonlyArray<CustomerOrder>, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const rows = yield* listOrderRows(db, userId)

    return yield* Effect.all(rows.map(row => orderFromRowWithUsage(db, row)))
  })

const readCustomerOrderSiteContext = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<
  CustomerOrderSiteContextRow | null,
  CustomerOrderStorageError
> =>
  d1Effect('customerOrders.siteContext.read', () =>
    db
      .prepare(
        `SELECT software_orders.id AS order_id,
                site_projects.id AS site_id,
                site_projects.active_version_id AS active_version_id,
                site_projects.active_deployment_id AS active_deployment_id
           FROM software_orders
           LEFT JOIN site_projects
             ON site_projects.software_order_id = software_orders.id
            AND site_projects.archived_at IS NULL
          WHERE software_orders.user_id = ?
            AND software_orders.id = ?
            AND software_orders.archived_at IS NULL
          LIMIT 1`,
      )
      .bind(userId, orderId)
      .first<CustomerOrderSiteContextRow>(),
  )

const listFulfillmentArtifacts = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<
  ReadonlyArray<CustomerFulfillmentArtifact> | null,
  CustomerOrderStorageError
> =>
  Effect.gen(function* () {
    const context = yield* readCustomerOrderSiteContext(db, userId, orderId)

    if (context === null) {
      return null
    }

    const rows = yield* d1Effect('customerOrders.fulfillmentArtifacts.list', () =>
      db
        .prepare(
          `SELECT id,
                  software_order_id,
                  kind,
                  title,
                  summary,
                  url,
                  repository_full_name,
                  source_branch,
                  target_branch,
                  commit_sha,
                  status,
                  created_at,
                  updated_at
             FROM order_fulfillment_artifacts
            WHERE software_order_id = ?
              AND visibility = 'public'
              AND archived_at IS NULL
            ORDER BY CASE status
                       WHEN 'customer_review_ready' THEN 0
                       WHEN 'draft' THEN 1
                       WHEN 'customer_accepted' THEN 2
                       WHEN 'superseded' THEN 3
                       ELSE 4
                     END,
                     created_at DESC
            LIMIT 50`,
        )
        .bind(orderId)
        .all<FulfillmentArtifactRow>(),
    )

    return (rows.results ?? []).map(artifactFromRow)
  })

const listSiteRevisions = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<
  ReadonlyArray<CustomerSiteRevision> | null,
  CustomerOrderStorageError
> =>
  Effect.gen(function* () {
    const context = yield* readCustomerOrderSiteContext(db, userId, orderId)

    if (context === null) {
      return null
    }

    if (context.site_id === null) {
      return []
    }

    const rows = yield* d1Effect('customerOrders.siteRevisions.list', () =>
      db
        .prepare(
          `SELECT site_versions.id AS id,
                  site_versions.site_id AS site_id,
                  site_versions.build_status AS build_status,
                  site_versions.source_commit_sha AS source_commit_sha,
                  site_versions.metadata_json AS metadata_json,
                  origin_feedback.body AS origin_feedback_body,
                  origin_feedback.created_at AS origin_feedback_created_at,
                  site_versions.created_at AS created_at,
                  site_versions.saved_at AS saved_at,
                  latest_deployment.id AS deployment_id,
                  latest_deployment.status AS deployment_status,
                  latest_deployment.url AS deployment_url,
                  latest_deployment.activated_at AS activated_at
             FROM site_versions
             LEFT JOIN site_deployments AS latest_deployment
               ON latest_deployment.id = (
                    SELECT deployment.id
                      FROM site_deployments AS deployment
                     WHERE deployment.version_id = site_versions.id
                     ORDER BY CASE deployment.status
                                WHEN 'active' THEN 0
                                WHEN 'deploying' THEN 1
                                WHEN 'queued' THEN 2
                                ELSE 3
                              END,
                              deployment.updated_at DESC,
                              deployment.created_at DESC
                     LIMIT 1
                  )
             LEFT JOIN site_revision_feedback AS origin_feedback
               ON origin_feedback.id = (
                    SELECT feedback.id
                      FROM site_revision_feedback AS feedback
                      LEFT JOIN adjutant_adjustment_requests AS adjustment
                        ON adjustment.id = feedback.adjutant_adjustment_id
                       AND adjustment.archived_at IS NULL
                     WHERE feedback.software_order_id = ?
                       AND feedback.visibility = 'public'
                       AND feedback.archived_at IS NULL
                       AND adjustment.resulting_version_id = site_versions.id
                     ORDER BY feedback.created_at DESC
                     LIMIT 1
                  )
            WHERE site_versions.site_id = ?
            ORDER BY site_versions.created_at DESC
            LIMIT 25`,
        )
        .bind(context.order_id, context.site_id)
        .all<SiteRevisionRow>(),
    )

    return (rows.results ?? []).map(row =>
      revisionFromRow(row, context.active_version_id),
    )
  })

const listSiteFeedback = (
  db: D1Database,
  userId: string,
  orderId: string,
): Effect.Effect<
  ReadonlyArray<CustomerSiteFeedback> | null,
  CustomerOrderStorageError
> =>
  Effect.gen(function* () {
    const context = yield* readCustomerOrderSiteContext(db, userId, orderId)

    if (context === null) {
      return null
    }

    const rows = yield* d1Effect('customerOrders.siteFeedback.list', () =>
      db
        .prepare(
          `SELECT id,
                  software_order_id,
                  site_id,
                  site_version_id,
                  site_deployment_id,
                  body,
                  status,
                  created_at,
                  updated_at
             FROM site_revision_feedback
            WHERE software_order_id = ?
              AND author_user_id = ?
              AND archived_at IS NULL
            ORDER BY created_at DESC
            LIMIT 50`,
        )
        .bind(orderId, userId)
        .all<SiteFeedbackRow>(),
    )

    return (rows.results ?? []).map(feedbackFromRow)
  })

const readDuplicateOpenFeedback = (
  db: D1Database,
  input: Readonly<{
    body: string
    deploymentId: string | null
    orderId: string
    siteId: string | null
    userId: string
    versionId: string | null
  }>,
): Effect.Effect<ExistingSiteFeedbackRow | null, CustomerOrderStorageError> =>
  d1Effect('customerOrders.siteFeedback.duplicate.read', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                site_version_id,
                site_deployment_id,
                body,
                status,
                adjutant_adjustment_id,
                created_at,
                updated_at
           FROM site_revision_feedback
          WHERE software_order_id = ?
            AND author_user_id = ?
            AND body = ?
            AND status IN ('submitted', 'queued', 'running')
            AND archived_at IS NULL
            AND ((site_id IS NULL AND ? IS NULL) OR site_id = ?)
            AND ((site_version_id IS NULL AND ? IS NULL) OR site_version_id = ?)
            AND ((site_deployment_id IS NULL AND ? IS NULL) OR site_deployment_id = ?)
          ORDER BY created_at DESC
          LIMIT 1`,
      )
      .bind(
        input.orderId,
        input.userId,
        input.body,
        input.siteId,
        input.siteId,
        input.versionId,
        input.versionId,
        input.deploymentId,
        input.deploymentId,
      )
      .first<ExistingSiteFeedbackRow>(),
  )

const readActiveCustomerAdjutantAssignment = (
  db: D1Database,
  input: Readonly<{ orderId: string; siteId: string }>,
): Effect.Effect<
  CustomerOrderAdjutantAssignmentRow | null,
  CustomerOrderStorageError
> =>
  d1Effect('customerOrders.adjutantAssignment.active.read', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                site_id,
                goal_id,
                current_run_id,
                objective,
                status,
                visibility
           FROM adjutant_assignments
          WHERE archived_at IS NULL
            AND status NOT IN ('complete', 'canceled')
            AND (
              (software_order_id IS NOT NULL AND software_order_id = ?)
              OR
              (site_id IS NOT NULL AND site_id = ?)
            )
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(input.orderId, input.siteId)
      .first<CustomerOrderAdjutantAssignmentRow>(),
  )

const createCustomerFeedbackAssignment = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  input: Readonly<{
    feedbackBody: string
    orderId: string
    siteId: string
    userId: string
  }>,
): Effect.Effect<CustomerOrderAdjutantAssignmentRow, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const now = runtime.nowIso()
    const id = runtime.makeAdjutantAssignmentId()
    const objective = [
      'Apply customer Site feedback for software order',
      input.orderId,
      '',
      input.feedbackBody,
    ].join(' ')

    yield* d1Effect('customerOrders.adjutantAssignment.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_assignments
             (id,
              software_order_id,
              site_id,
              goal_id,
              current_run_id,
              team_id,
              project_id,
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
              blocked_at,
              archived_at)
           VALUES (?, ?, ?, NULL, NULL, 'team_openagents_core', 'project_adjutant', 'agent_adjutant', ?, 'site_adjustment', 'queued', 'public', NULL, NULL, ?, ?, ?, NULL, NULL, NULL)`,
        )
        .bind(id, input.orderId, input.siteId, input.userId, objective, now, now)
        .run(),
    )

    const assignment = yield* readActiveCustomerAdjutantAssignment(db, {
      orderId: input.orderId,
      siteId: input.siteId,
    })

    if (assignment !== null) {
      return assignment
    }

    return {
      current_run_id: null,
      goal_id: null,
      id,
      objective,
      site_id: input.siteId,
      software_order_id: input.orderId,
      status: 'queued',
      visibility: 'public',
    }
  })

const recordCustomerAssignmentEvent = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  input: Readonly<{
    assignment: CustomerOrderAdjutantAssignmentRow
    eventType: string
    feedbackId: string
    summary: string
    userId: string
  }>,
): Effect.Effect<void, CustomerOrderStorageError> =>
  d1Effect('customerOrders.adjutantAssignmentEvent.insert', () =>
    db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        runtime.makeAdjutantAssignmentEventId(),
        input.assignment.id,
        input.assignment.software_order_id,
        input.assignment.site_id,
        input.assignment.goal_id,
        input.assignment.current_run_id,
        input.eventType,
        input.assignment.visibility,
        input.summary,
        input.userId,
        JSON.stringify({ feedbackId: input.feedbackId }),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const recordCustomerSiteEvent = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  input: Readonly<{
    adjustmentId?: string
    assignmentId: string
    feedbackId: string
    siteId: string
    summary: string
    type: string
    userId: string
  }>,
): Effect.Effect<void, CustomerOrderStorageError> =>
  d1Effect('customerOrders.siteEvent.insert', () =>
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
        runtime.makeSiteEventId(),
        input.siteId,
        input.type,
        input.summary,
        input.userId,
        JSON.stringify({
          adjustmentId: input.adjustmentId,
          assignmentId: input.assignmentId,
          feedbackId: input.feedbackId,
        }),
        runtime.nowIso(),
      )
      .run(),
  ).pipe(Effect.asVoid)

const queueCustomerSiteFeedbackAdjustment = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  input: Readonly<{
    body: string
    deploymentId: string | null
    feedbackId: string
    orderId: string
    siteId: string | null
    userId: string
    versionId: string | null
  }>,
): Effect.Effect<CustomerSiteFeedbackStatus, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    if (
      input.siteId === null ||
      containsProviderSecretMaterial(input.body)
    ) {
      return 'submitted' as const
    }

    const activeAssignment = yield* readActiveCustomerAdjutantAssignment(db, {
      orderId: input.orderId,
      siteId: input.siteId,
    })
    const assignment =
      activeAssignment ??
      (yield* createCustomerFeedbackAssignment(db, runtime, {
        feedbackBody: input.body,
        orderId: input.orderId,
        siteId: input.siteId,
        userId: input.userId,
      }))
    const now = runtime.nowIso()
    const adjustmentId = runtime.makeAdjutantAdjustmentId()
    const instruction = customerFeedbackAdjustmentInstruction({
      body: input.body,
      id: input.feedbackId,
      orderId: input.orderId,
      siteId: input.siteId,
      versionId: input.versionId,
    })
    const continuationMode =
      assignment.current_run_id === null ? 'new_goal_run' : 'follow_up_turn'

    yield* d1Effect('customerOrders.adjutantAdjustment.insert', () =>
      db
        .prepare(
          `INSERT INTO adjutant_adjustment_requests
             (id,
              assignment_id,
              software_order_id,
              site_id,
              goal_id,
              requested_by_user_id,
              instruction,
              status,
              continuation_mode,
              source_run_id,
              continuation_run_id,
              resulting_version_id,
              visibility,
              created_at,
              updated_at,
              completed_at,
              archived_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'queued', ?, ?, NULL, NULL, 'public', ?, ?, NULL, NULL)`,
        )
        .bind(
          adjustmentId,
          assignment.id,
          input.orderId,
          input.siteId,
          assignment.goal_id,
          input.userId,
          instruction,
          continuationMode,
          assignment.current_run_id,
          now,
          now,
        )
        .run(),
    )

    yield* d1Effect('customerOrders.siteFeedback.queue.update', () =>
      db
        .prepare(
          `UPDATE site_revision_feedback
              SET status = 'queued',
                  adjutant_assignment_id = ?,
                  adjutant_adjustment_id = ?,
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(assignment.id, adjustmentId, now, input.feedbackId)
        .run(),
    )
    yield* d1Effect('customerOrders.softwareOrder.feedbackQueue.update', () =>
      db
        .prepare(
          `UPDATE software_orders
              SET status = 'agent_queued',
                  updated_at = ?
            WHERE id = ?
              AND archived_at IS NULL`,
        )
        .bind(now, input.orderId)
        .run(),
    )
    yield* recordCustomerAssignmentEvent(db, runtime, {
      assignment,
      eventType: 'adjutant.customer_feedback_queued',
      feedbackId: input.feedbackId,
      summary: 'Customer Site feedback was queued for Autopilot adjustment.',
      userId: input.userId,
    })
    yield* recordCustomerSiteEvent(db, runtime, {
      adjustmentId,
      assignmentId: assignment.id,
      feedbackId: input.feedbackId,
      siteId: input.siteId,
      summary: 'Customer Site feedback was queued for Autopilot adjustment.',
      type: 'adjutant.customer_feedback_queued',
      userId: input.userId,
    })

    return 'queued' as const
  })

const submitSiteFeedback = (
  db: D1Database,
  runtime: CustomerOrderRuntime,
  userId: string,
  orderId: string,
  body: string,
): Effect.Effect<CustomerSiteFeedback | null, CustomerOrderStorageError> =>
  Effect.gen(function* () {
    const trimmedBody = body.trim()

    if (trimmedBody === '' || trimmedBody.length > 4000) {
      return null
    }

    const context = yield* readCustomerOrderSiteContext(db, userId, orderId)

    if (context === null) {
      return null
    }

    const duplicate = yield* readDuplicateOpenFeedback(db, {
      body: trimmedBody,
      deploymentId: context.active_deployment_id,
      orderId: context.order_id,
      siteId: context.site_id,
      userId,
      versionId: context.active_version_id,
    })

    if (duplicate !== null) {
      return feedbackFromRow(duplicate)
    }

    const now = runtime.nowIso()
    const id = runtime.makeSiteFeedbackId()

    yield* d1Effect('customerOrders.siteFeedback.insert', () =>
      db
        .prepare(
          `INSERT INTO site_revision_feedback
             (id,
              software_order_id,
              site_id,
              site_version_id,
              site_deployment_id,
              author_user_id,
              body,
              status,
              source,
              visibility,
              created_at,
              updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'submitted', 'customer_order_ui', 'public', ?, ?)`,
        )
        .bind(
          id,
          context.order_id,
          context.site_id,
          context.active_version_id,
          context.active_deployment_id,
          userId,
          trimmedBody,
          now,
          now,
        )
        .run(),
    )
    const status = yield* queueCustomerSiteFeedbackAdjustment(db, runtime, {
      body: trimmedBody,
      deploymentId: context.active_deployment_id,
      feedbackId: id,
      orderId: context.order_id,
      siteId: context.site_id,
      userId,
      versionId: context.active_version_id,
    })

    return {
      id,
      orderId: context.order_id,
      siteId: context.site_id,
      versionId: context.active_version_id,
      deploymentId: context.active_deployment_id,
      body: trimmedBody,
      status,
      createdAt: now,
      updatedAt: now,
    }
  })

export class CustomerOrderStore extends Context.Service<
  CustomerOrderStore,
  {
    readonly readOrderById: (
      userId: string,
      orderId: string,
    ) => Effect.Effect<CustomerOrder | null, CustomerOrderStorageError>
    readonly listOrders: (
      userId: string,
    ) => Effect.Effect<ReadonlyArray<CustomerOrder>, CustomerOrderStorageError>
    readonly readOrCreateActiveOrder: (
      userId: string,
    ) => Effect.Effect<CustomerOrder | null, CustomerOrderStorageError>
    readonly createOrder: (
      userId: string,
      request: string,
      agentIdempotencyKey?: string,
    ) => Effect.Effect<CustomerOrder, CustomerOrderStorageError>
    readonly readOrderByAgentIdempotencyKey: (
      userId: string,
      idempotencyKey: string,
    ) => Effect.Effect<CustomerOrder | null, CustomerOrderStorageError>
    readonly listSiteFeedback: (
      userId: string,
      orderId: string,
    ) => Effect.Effect<
      ReadonlyArray<CustomerSiteFeedback> | null,
      CustomerOrderStorageError
    >
    readonly listFulfillmentArtifacts: (
      userId: string,
      orderId: string,
    ) => Effect.Effect<
      ReadonlyArray<CustomerFulfillmentArtifact> | null,
      CustomerOrderStorageError
    >
    readonly listSiteRevisions: (
      userId: string,
      orderId: string,
    ) => Effect.Effect<
      ReadonlyArray<CustomerSiteRevision> | null,
      CustomerOrderStorageError
    >
    readonly submitSiteFeedback: (
      userId: string,
      orderId: string,
      body: string,
    ) => Effect.Effect<CustomerSiteFeedback | null, CustomerOrderStorageError>
  }
>()('@openagentsinc/autopilot-omega/CustomerOrderStore') {
  static readonly layer = (
    env: CustomerOrderEnv,
    runtime: CustomerOrderRuntime = systemCustomerOrderRuntime,
  ) =>
    Layer.succeed(CustomerOrderStore, {
      readOrderById: Effect.fn('CustomerOrderStore.readOrderById')(
        (userId, orderId) =>
          readOrderById(openAgentsDatabase(env), userId, orderId),
      ),
      listOrders: Effect.fn('CustomerOrderStore.listOrders')(userId =>
        listOrders(openAgentsDatabase(env), userId),
      ),
      readOrCreateActiveOrder: Effect.fn(
        'CustomerOrderStore.readOrCreateActiveOrder',
      )(userId =>
        readOrCreateActiveOrder(openAgentsDatabase(env), runtime, userId),
      ),
      createOrder: Effect.fn('CustomerOrderStore.createOrder')(
        (userId, request, agentIdempotencyKey) =>
          insertOrderFromRequest(
            openAgentsDatabase(env),
            runtime,
            userId,
            request,
            agentIdempotencyKey,
          ),
      ),
      readOrderByAgentIdempotencyKey: Effect.fn(
        'CustomerOrderStore.readOrderByAgentIdempotencyKey',
      )((userId, idempotencyKey) =>
        readOrderByAgentIdempotencyKey(
          openAgentsDatabase(env),
          userId,
          idempotencyKey,
        ),
      ),
      listSiteFeedback: Effect.fn('CustomerOrderStore.listSiteFeedback')(
        (userId, orderId) =>
          listSiteFeedback(openAgentsDatabase(env), userId, orderId),
      ),
      listFulfillmentArtifacts: Effect.fn(
        'CustomerOrderStore.listFulfillmentArtifacts',
      )((userId, orderId) =>
        listFulfillmentArtifacts(openAgentsDatabase(env), userId, orderId),
      ),
      listSiteRevisions: Effect.fn('CustomerOrderStore.listSiteRevisions')(
        (userId, orderId) =>
          listSiteRevisions(openAgentsDatabase(env), userId, orderId),
      ),
      submitSiteFeedback: Effect.fn('CustomerOrderStore.submitSiteFeedback')(
        (userId, orderId, body) =>
          submitSiteFeedback(
            openAgentsDatabase(env),
            runtime,
            userId,
            orderId,
            body,
          ),
      ),
    })
}
