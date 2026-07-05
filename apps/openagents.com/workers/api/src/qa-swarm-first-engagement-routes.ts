import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { notFound } from '@openagentsinc/sync-worker'
import { Data, Effect, Schema as S } from 'effect'

import {
  BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
} from './business-commitment-ledger'
import { readBusinessSignupRequest } from './business-signup-routes'
import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import { parseJsonUnknown } from './json-boundary'
import { makePrefilledWorkspaceService } from './prefilled-workspace'
import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import {
  firstDollarEvidenceBundleRef,
  recordRevenueEventProvenance,
} from './revenue-event-provenance'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'
import type { SupervisionLongtailMirror } from './supervision-longtail-domain-store'

type HttpResponse = globalThis.Response

export const QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT =
  '/api/operator/qa-swarm/first-engagements' as const
export const QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT =
  '/api/public/qa-swarm/first-engagements' as const
export const QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_RECEIPT_PATH =
  '/api/public/qa-swarm/first-engagements/:receiptRef' as const
export const QA_SWARM_FIRST_ENGAGEMENT_TABLE =
  'qa_swarm_first_engagements' as const
export const QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF =
  'deliverable.qa_swarm.swarm_audit.report.v1' as const

export const QaSwarmFirstEngagementPaymentPath = S.Literals([
  'operator_sales_deposit_invoice',
  'checkout_kickoff_receipt',
])
export type QaSwarmFirstEngagementPaymentPath =
  typeof QaSwarmFirstEngagementPaymentPath.Type

export const QaSwarmFirstEngagementIntakeRequest = S.Struct({
  schemaVersion: S.Literal(
    'openagents.qa_swarm.first_engagement_intake.v1',
  ),
  packageKind: S.Literal('swarm_audit'),
  paymentPath: QaSwarmFirstEngagementPaymentPath,
  businessSignupRequestId: S.String,
  userId: S.String,
  committedAmountCents: S.Number,
  intakeReceiptRef: S.String,
  targetAdapterReviewRef: S.String,
  packageContractRef: S.String,
  firstReportDueAt: S.String,
  checkoutKickoffReceiptRef: S.optionalKey(S.String),
  depositInvoiceReceiptRef: S.optionalKey(S.String),
  idempotencyKey: S.optionalKey(S.String),
})
export type QaSwarmFirstEngagementIntakeRequest =
  typeof QaSwarmFirstEngagementIntakeRequest.Type

export const PublicQaSwarmFirstEngagementReceipt = S.Struct({
  schemaVersion: S.Literal(
    'openagents.qa_swarm.first_engagement_receipt.v1',
  ),
  product: S.Literal('qa-swarm'),
  packageKind: S.Literal('swarm_audit'),
  promiseIds: S.Array(S.String),
  receiptRef: S.String,
  receiptUrl: S.String,
  generatedAt: S.String,
  recordedAt: S.String,
  purchase: S.Struct({
    operatorAssisted: S.Literal(true),
    selfServe: S.Literal(false),
    paymentPath: QaSwarmFirstEngagementPaymentPath,
    intakeReceiptRef: S.String,
    checkoutOrDepositReceiptRef: S.String,
    committedAmountCents: S.Number,
    paymentEvidenceRecorded: S.Literal(true),
    rawPaymentMaterialIncluded: S.Literal(false),
    firstPaidDeliveryReceipt: S.Literal(false),
    settlementMovedMoney: S.Literal(false),
  }),
  provision: S.Struct({
    workspaceRef: S.String,
    servicePromiseContractRef: S.String,
    servicePromiseState: S.Literal('active'),
    deliverableContractRef: S.String,
  }),
  commitment: S.Struct({
    commitmentRef: S.String,
    weeklyReviewRef: S.Literal(BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF),
    promisedObjectRef: S.Literal(QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF),
    dueState: S.Literal('due'),
    firstReportDueAt: S.String,
  }),
  publicSafety: S.Struct({
    noCustomerIdentity: S.Literal(true),
    noRawPaymentMaterial: S.Literal(true),
    noRawInvoice: S.Literal(true),
    noTargetCredentials: S.Literal(true),
    noRawRunnerLogs: S.Literal(true),
    noProviderPayloads: S.Literal(true),
  }),
  evidenceRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  staleness: PublicProjectionStalenessContract,
})
export type PublicQaSwarmFirstEngagementReceipt =
  typeof PublicQaSwarmFirstEngagementReceipt.Type

export const PublicQaSwarmFirstEngagementEnvelope = S.Struct({
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  receipt: PublicQaSwarmFirstEngagementReceipt,
})
export type PublicQaSwarmFirstEngagementEnvelope =
  typeof PublicQaSwarmFirstEngagementEnvelope.Type

export const OperatorQaSwarmFirstEngagementIntakeEnvelope = S.Struct({
  ok: S.Literal(true),
  idempotent: S.Boolean,
  generatedAt: S.String,
  staleness: PublicProjectionStalenessContract,
  receipt: PublicQaSwarmFirstEngagementReceipt,
})
export type OperatorQaSwarmFirstEngagementIntakeEnvelope =
  typeof OperatorQaSwarmFirstEngagementIntakeEnvelope.Type

export type QaSwarmFirstEngagementRecord = Readonly<{
  receiptRef: string
  packageKind: 'swarm_audit'
  paymentPath: QaSwarmFirstEngagementPaymentPath
  businessSignupRequestId: string
  userId: string
  committedAmountCents: number
  intakeReceiptRef: string
  checkoutOrDepositReceiptRef: string
  targetAdapterReviewRef: string
  packageContractRef: string
  workspaceId: string
  servicePromiseContractId: string
  commitmentRef: string
  firstReportDueAt: string
  recordedAt: string
}>

export type QaSwarmFirstEngagementDraft =
  QaSwarmFirstEngagementRecord &
    Readonly<{
      idempotencyKey: string
    }>

export type QaSwarmFirstEngagementStore = Readonly<{
  recordFirstEngagement: (
    draft: QaSwarmFirstEngagementDraft,
  ) => Effect.Effect<
    Readonly<{
      record: QaSwarmFirstEngagementRecord
      idempotent: boolean
    }>,
    QaSwarmFirstEngagementStoreUnavailable
  >
  readFirstEngagement: (
    receiptRef: string,
  ) => Effect.Effect<
    QaSwarmFirstEngagementRecord | null,
    QaSwarmFirstEngagementStoreUnavailable
  >
}>

export type QaSwarmFirstEngagementRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

type RouteInput = Readonly<{
  OPENAGENTS_DB?: D1Database
  // KS-8.17 (#8361): optional read-back mirror for the
  // omni_accepted_outcome_contracts row this route's service-promise write
  // creates. Undefined is a safe no-op (no Postgres binding / dual-write
  // off, or the caller has not threaded a mirror through yet).
  mirror?: SupervisionLongtailMirror | undefined
  nowIso?: (() => string) | undefined
  store?: QaSwarmFirstEngagementStore | undefined
}>

type OperatorRouteInput = RouteInput &
  Readonly<{
    requireAdminApiToken: (request: Request) => Promise<boolean>
  }>

type SqlRow = Readonly<{
  receipt_ref: unknown
  package_kind: unknown
  payment_path: unknown
  business_signup_request_id: unknown
  user_id: unknown
  committed_amount_cents: unknown
  intake_receipt_ref: unknown
  checkout_or_deposit_receipt_ref: unknown
  target_adapter_review_ref: unknown
  package_contract_ref: unknown
  workspace_id: unknown
  service_promise_contract_id: unknown
  commitment_ref: unknown
  first_report_due_at: unknown
  recorded_at: unknown
}>

export class QaSwarmFirstEngagementStoreUnavailable extends Data.TaggedError(
  'QaSwarmFirstEngagementStoreUnavailable',
)<{ readonly reason: string }> {}

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const integerValue = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isSafeInteger(value) ? value : undefined

const isPaymentPath = (
  value: string | undefined,
): value is QaSwarmFirstEngagementPaymentPath =>
  value === 'operator_sales_deposit_invoice' ||
  value === 'checkout_kickoff_receipt'

export const qaSwarmFirstEngagementRecordFromSql = (
  row: SqlRow,
): QaSwarmFirstEngagementRecord | null => {
  const receiptRef = stringValue(row.receipt_ref)
  const packageKind = stringValue(row.package_kind)
  const paymentPath = stringValue(row.payment_path)
  const businessSignupRequestId = stringValue(row.business_signup_request_id)
  const userId = stringValue(row.user_id)
  const committedAmountCents = integerValue(row.committed_amount_cents)
  const intakeReceiptRef = stringValue(row.intake_receipt_ref)
  const checkoutOrDepositReceiptRef = stringValue(
    row.checkout_or_deposit_receipt_ref,
  )
  const targetAdapterReviewRef = stringValue(row.target_adapter_review_ref)
  const packageContractRef = stringValue(row.package_contract_ref)
  const workspaceId = stringValue(row.workspace_id)
  const servicePromiseContractId = stringValue(
    row.service_promise_contract_id,
  )
  const commitmentRef = stringValue(row.commitment_ref)
  const firstReportDueAt = stringValue(row.first_report_due_at)
  const recordedAt = stringValue(row.recorded_at)

  if (
    receiptRef === undefined ||
    packageKind !== 'swarm_audit' ||
    !isPaymentPath(paymentPath) ||
    businessSignupRequestId === undefined ||
    userId === undefined ||
    committedAmountCents === undefined ||
    intakeReceiptRef === undefined ||
    checkoutOrDepositReceiptRef === undefined ||
    targetAdapterReviewRef === undefined ||
    packageContractRef === undefined ||
    workspaceId === undefined ||
    servicePromiseContractId === undefined ||
    commitmentRef === undefined ||
    firstReportDueAt === undefined ||
    recordedAt === undefined
  ) {
    return null
  }

  return {
    receiptRef,
    packageKind: 'swarm_audit',
    paymentPath,
    businessSignupRequestId,
    userId,
    committedAmountCents,
    intakeReceiptRef,
    checkoutOrDepositReceiptRef,
    targetAdapterReviewRef,
    packageContractRef,
    workspaceId,
    servicePromiseContractId,
    commitmentRef,
    firstReportDueAt,
    recordedAt,
  }
}

const readSql = `
  SELECT
    receipt_ref,
    package_kind,
    payment_path,
    business_signup_request_id,
    user_id,
    committed_amount_cents,
    intake_receipt_ref,
    checkout_or_deposit_receipt_ref,
    target_adapter_review_ref,
    package_contract_ref,
    workspace_id,
    service_promise_contract_id,
    commitment_ref,
    first_report_due_at,
    recorded_at
  FROM qa_swarm_first_engagements
`

const recordFromDbRow = (
  row: SqlRow | null | undefined,
): QaSwarmFirstEngagementRecord | null =>
  row === null || row === undefined
    ? null
    : qaSwarmFirstEngagementRecordFromSql(row)

const runtimeDefaults: QaSwarmFirstEngagementRuntime = {
  makeId: compactRandomId,
  nowIso: currentIsoTimestamp,
}

const readByIdempotencyKey = async (
  db: D1Database,
  idempotencyKey: string,
): Promise<QaSwarmFirstEngagementRecord | null> => {
  const row = await db
    .prepare(`${readSql} WHERE idempotency_key = ? LIMIT 1`)
    .bind(idempotencyKey)
    .first<SqlRow>()

  return recordFromDbRow(row)
}

const createWorkspace = async (
  db: D1Database,
  draft: QaSwarmFirstEngagementDraft,
  runtime: QaSwarmFirstEngagementRuntime,
) => {
  const signup = await readBusinessSignupRequest(
    db,
    draft.businessSignupRequestId,
  )

  if (signup === undefined) {
    throw new QaSwarmFirstEngagementStoreUnavailable({
      reason: 'business_signup_request_not_found',
    })
  }

  return makePrefilledWorkspaceService(db, runtime).createWorkspace({
    holderRef: `business_signup:${draft.businessSignupRequestId}`,
    holderUserId: draft.userId,
    introReceipt: {
      summary:
        'QA Swarm Audit workspace provisioned from operator-assisted first engagement intake.',
      publicSourceRefs: [
        draft.intakeReceiptRef,
        draft.checkoutOrDepositReceiptRef,
        draft.targetAdapterReviewRef,
        draft.packageContractRef,
      ],
    },
    projectName: 'QA Swarm Audit workspace',
    seededMemory: [
      {
        label: 'Package',
        publicSourceRef: draft.packageContractRef,
        value: 'Swarm Audit is the committed deliverable contract.',
      },
      {
        label: 'Intake receipt',
        publicSourceRef: draft.intakeReceiptRef,
        value: 'Operator-assisted QA Swarm intake receipt recorded.',
      },
      {
        label: 'Payment evidence',
        publicSourceRef: draft.checkoutOrDepositReceiptRef,
        value:
          draft.paymentPath === 'checkout_kickoff_receipt'
            ? 'Checkout kickoff receipt recorded.'
            : 'Deposit invoice receipt recorded.',
      },
    ],
    starterWorkflows: [
      {
        description:
          'Review target adapter, credential boundaries, and redaction requirements.',
        outcomeKind: 'qa_swarm_target_adapter_review',
        status: 'queued',
        title: 'Target adapter review',
      },
      {
        description:
          'Prepare the seed scenarios, monkey window, explorer lane, and perf baseline.',
        outcomeKind: 'qa_swarm_swarm_audit_plan',
        status: 'queued',
        title: 'Swarm Audit plan',
      },
      {
        description:
          'Publish the first public-safe report and regression-pack closeout refs.',
        outcomeKind: 'qa_swarm_public_safe_report',
        status: 'queued',
        title: 'First report closeout',
      },
    ],
    status: 'active',
  })
}

const createServicePromise = async (
  db: D1Database,
  draft: QaSwarmFirstEngagementDraft,
  mirror?: SupervisionLongtailMirror | undefined,
): Promise<{ id: string }> => {
  const idempotencyKey = `qa_swarm_first_engagement:${draft.idempotencyKey}`
  const existing = await db
    .prepare(
      `SELECT id
         FROM omni_accepted_outcome_contracts
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<{ id: string }>()

  if (existing !== null) {
    return existing
  }

  const now = currentIsoTimestamp()
  const id = compactRandomId('omni_accepted_outcome_contract')
  const workspaceRef = `workspace:${draft.workspaceId}`
  const servicePromiseRef = `omni_accepted_outcome:qa_swarm:${idempotencyKey}`
  const expectedArtifacts = [
    {
      artifactKind: 'qa_swarm_report',
      deliverableRef: QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF,
      publicSafe: true,
      required: true,
    },
    {
      artifactKind: 'qa_swarm_regression_pack',
      deliverableRef: 'deliverable.qa_swarm.swarm_audit.regression_pack.v1',
      publicSafe: true,
      required: true,
    },
    {
      artifactKind: 'qa_swarm_trace_coverage_video_refs',
      deliverableRef: 'deliverable.qa_swarm.swarm_audit.evidence_refs.v1',
      publicSafe: true,
      required: true,
    },
  ]
  const closeoutRequirements = [
    {
      required: true,
      requirementKind: 'operator_review',
      sourceRef: 'gate.qa_swarm.operator_assisted_review.v1',
    },
    {
      required: true,
      requirementKind: 'public_safe_report_ready',
      sourceRef: QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF,
    },
    {
      required: true,
      requirementKind: 'target_adapter_review',
      sourceRef: draft.targetAdapterReviewRef,
    },
  ]
  const slaTerms = [
    {
      termKind: 'first_report_due_at',
      dueAt: draft.firstReportDueAt,
      sourceRef: draft.packageContractRef,
    },
  ]

  await db
    .prepare(
      `INSERT OR IGNORE INTO omni_accepted_outcome_contracts
         (id,
          idempotency_key,
          work_kind,
          subject_ref,
          customer_ref,
          expected_artifacts_json,
          review_policy,
          acceptance_state,
          proof_policy,
          economic_state,
          closeout_requirements_json,
          legal_sensitive,
          public_receipt_ref,
          metadata_json,
          committed_deliverables_json,
          service_promise_state,
          sla_terms_json,
          fulfillment_receipts_json,
          created_at,
          updated_at,
          archived_at)
       VALUES (?, ?, 'business', ?, ?, ?, 'operator_review', 'draft',
               'customer_safe_summary', 'paid_required', ?, 0, ?, ?, ?,
               'active', ?, '[]', ?, ?, NULL)`,
    )
    .bind(
      id,
      idempotencyKey,
      workspaceRef,
      `business_signup:${draft.businessSignupRequestId}`,
      JSON.stringify(expectedArtifacts),
      JSON.stringify(closeoutRequirements),
      servicePromiseRef,
      JSON.stringify({
        firstEngagementReceiptRef: draft.receiptRef,
        intakeReceiptRef: draft.intakeReceiptRef,
        packageKind: draft.packageKind,
        paymentPath: draft.paymentPath,
        signupRef: `business_signup:${draft.businessSignupRequestId}`,
        workspaceRef,
      }),
      JSON.stringify(expectedArtifacts),
      JSON.stringify(slaTerms),
      now,
      now,
    )
    .run()

  if (mirror !== undefined) {
    await mirror.mirrorRowsByKey('omni_accepted_outcome_contracts', [[id]])
  }

  const inserted = await db
    .prepare(
      `SELECT id
         FROM omni_accepted_outcome_contracts
        WHERE idempotency_key = ?
          AND archived_at IS NULL
        LIMIT 1`,
    )
    .bind(idempotencyKey)
    .first<{ id: string }>()

  if (inserted === null) {
    throw new QaSwarmFirstEngagementStoreUnavailable({
      reason: 'service_promise_contract_not_persisted',
    })
  }

  return inserted
}

const createCommitmentLedgerRow = async (
  db: D1Database,
  draft: QaSwarmFirstEngagementDraft,
): Promise<void> => {
  const id = compactRandomId('business_commitment_qa_swarm')
  const now = currentIsoTimestamp()

  await db
    .prepare(
      `INSERT OR IGNORE INTO business_commitment_ledger (
        id,
        commitment_ref,
        engagement_ref,
        owner_ref,
        vertical_ref,
        promised_object_ref,
        commitment_kind,
        due_state,
        due_at,
        shipped_at,
        weekly_review_ref,
        source_refs_json,
        blocker_refs_json,
        evidence_refs_json,
        created_at,
        updated_at
      ) VALUES (?, ?, ?, 'owner.business.ops', 'vertical.qa_swarm',
                ?, 'deliverable', 'due', ?, NULL, ?, ?, '[]', ?, ?, ?)`,
    )
    .bind(
      id,
      draft.commitmentRef,
      `qa_swarm.first_engagement:${draft.receiptRef}`,
      QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF,
      draft.firstReportDueAt,
      BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
      JSON.stringify([
        'docs/fable/2026-07-02-qa-swarm-product-plan.md#3-the-packages',
        'docs/transcripts/246.md',
        `route:${QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT}`,
      ]),
      JSON.stringify([
        draft.receiptRef,
        draft.intakeReceiptRef,
        draft.checkoutOrDepositReceiptRef,
        `omni_accepted_outcome:${draft.servicePromiseContractId}`,
      ]),
      now,
      now,
    )
    .run()
}

const recordQaSwarmRevenueEventProvenance = async (
  db: D1Database,
  record: QaSwarmFirstEngagementRecord,
): Promise<void> => {
  const eventRef = `revenue_event.qa_swarm.first_engagement.${receiptSuffix(
    record.receiptRef,
  )}`
  await recordRevenueEventProvenance(db, {
    amountCents: record.committedAmountCents,
    amountSats: null,
    caveatRefs: [
      'caveat.revenue.qa_swarm.operator_assisted_payment_evidence_only',
      'caveat.revenue.qa_swarm.first_paid_delivery_not_claimed',
      'caveat.revenue.first_dollar.owner_signoff_required_for_public_claim',
    ],
    demandProvenance: 'external',
    eventRef,
    evidenceBundleRef: firstDollarEvidenceBundleRef('qa_swarm', eventRef),
    idempotencyKey: `revenue-event:qa-swarm-first-engagement:${record.receiptRef}`,
    ledgerRowRef: record.receiptRef,
    ledgerTable: 'qa_swarm_first_engagements',
    paymentState: 'payment_evidence_recorded',
    productRef: 'qa_swarm',
    publicEvidenceRefs: [
      record.receiptRef,
      `route:${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT}/${record.receiptRef}`,
      'promise:qa_swarm.service_packages.v1',
    ],
    receiptRef: record.receiptRef,
    recordedAt: record.recordedAt,
    revenueSurfaceRef: 'qa_swarm.swarm_audit_first_engagement',
    sourceRefs: [
      `route:${QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT}`,
      `route:${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_RECEIPT_PATH}`,
      `table:${QA_SWARM_FIRST_ENGAGEMENT_TABLE}`,
      'table:business_commitment_ledger',
    ],
  })
}

export const makeD1QaSwarmFirstEngagementStore = (
  db: D1Database | undefined,
  runtime: QaSwarmFirstEngagementRuntime = runtimeDefaults,
  mirror?: SupervisionLongtailMirror | undefined,
): QaSwarmFirstEngagementStore => ({
  recordFirstEngagement: draft =>
    Effect.tryPromise({
      try: async () => {
        if (db === undefined) {
          throw new QaSwarmFirstEngagementStoreUnavailable({
            reason: 'OPENAGENTS_DB missing',
          })
        }

        const existing = await readByIdempotencyKey(db, draft.idempotencyKey)
        if (existing !== null) {
          await recordQaSwarmRevenueEventProvenance(db, existing)
          return { record: existing, idempotent: true }
        }

        const workspace = await createWorkspace(db, draft, runtime)
        const withWorkspace = { ...draft, workspaceId: workspace.id }
        const contract = await createServicePromise(db, withWorkspace, mirror)
        const fullDraft = {
          ...withWorkspace,
          servicePromiseContractId: contract.id,
        }
        await createCommitmentLedgerRow(db, fullDraft)

        await db
          .prepare(
            `INSERT INTO qa_swarm_first_engagements (
              receipt_ref,
              idempotency_key,
              package_kind,
              payment_path,
              business_signup_request_id,
              user_id,
              committed_amount_cents,
              intake_receipt_ref,
              checkout_or_deposit_receipt_ref,
              target_adapter_review_ref,
              package_contract_ref,
              workspace_id,
              service_promise_contract_id,
              commitment_ref,
              first_report_due_at,
              recorded_at,
              created_at,
              updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            fullDraft.receiptRef,
            fullDraft.idempotencyKey,
            fullDraft.packageKind,
            fullDraft.paymentPath,
            fullDraft.businessSignupRequestId,
            fullDraft.userId,
            fullDraft.committedAmountCents,
            fullDraft.intakeReceiptRef,
            fullDraft.checkoutOrDepositReceiptRef,
            fullDraft.targetAdapterReviewRef,
            fullDraft.packageContractRef,
            fullDraft.workspaceId,
            fullDraft.servicePromiseContractId,
            fullDraft.commitmentRef,
            fullDraft.firstReportDueAt,
            fullDraft.recordedAt,
            fullDraft.recordedAt,
            fullDraft.recordedAt,
          )
          .run()

        const inserted = await readByIdempotencyKey(db, fullDraft.idempotencyKey)
        if (inserted === null) {
          throw new QaSwarmFirstEngagementStoreUnavailable({
            reason: 'first_engagement_not_persisted',
          })
        }

        await recordQaSwarmRevenueEventProvenance(db, inserted)

        return { record: inserted, idempotent: false }
      },
      catch: error =>
        error instanceof QaSwarmFirstEngagementStoreUnavailable
          ? error
          : new QaSwarmFirstEngagementStoreUnavailable({
              reason: error instanceof Error ? error.message : String(error),
            }),
    }),
  readFirstEngagement: receiptRef =>
    Effect.tryPromise({
      try: async () => {
        if (db === undefined) {
          throw new QaSwarmFirstEngagementStoreUnavailable({
            reason: 'OPENAGENTS_DB missing',
          })
        }
        const row = await db
          .prepare(`${readSql} WHERE receipt_ref = ? LIMIT 1`)
          .bind(receiptRef)
          .first<SqlRow>()
        return recordFromDbRow(row)
      },
      catch: error =>
        error instanceof QaSwarmFirstEngagementStoreUnavailable
          ? error
          : new QaSwarmFirstEngagementStoreUnavailable({
              reason: error instanceof Error ? error.message : String(error),
            }),
    }),
})

const promiseIds = [
  'qa_swarm.service_packages.v1',
  'qa_swarm.product_surface.v1',
] as const

const publicSafety = {
  noCustomerIdentity: true,
  noRawPaymentMaterial: true,
  noRawInvoice: true,
  noTargetCredentials: true,
  noRawRunnerLogs: true,
  noProviderPayloads: true,
} as const

const evidenceRefs = [
  'issue:OpenAgentsInc/openagents#8252',
  'promise:qa_swarm.service_packages.v1',
  'promise:qa_swarm.product_surface.v1',
  'docs/fable/2026-07-02-qa-swarm-product-plan.md',
  'docs/transcripts/246.md',
] as const

const caveatRefs = [
  'caveat.qa_swarm_first_engagement.operator_assisted_only',
  'caveat.qa_swarm_first_engagement.not_self_serve_hosted_run',
  'caveat.qa_swarm_first_engagement.no_first_paid_delivery_receipt_claim',
  'caveat.qa_swarm_first_engagement.no_settlement_or_payout_authority',
] as const

const sourceRefs = [
  `table:${QA_SWARM_FIRST_ENGAGEMENT_TABLE}`,
  'table:prefilled_workspaces',
  'table:omni_accepted_outcome_contracts',
  'table:business_commitment_ledger',
  `route:${QA_SWARM_FIRST_ENGAGEMENT_OPERATOR_ENDPOINT}`,
  `route:${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_RECEIPT_PATH}`,
] as const

export const publicQaSwarmFirstEngagementReceipt = (
  record: QaSwarmFirstEngagementRecord,
  generatedAt: string,
): PublicQaSwarmFirstEngagementReceipt => ({
  schemaVersion: 'openagents.qa_swarm.first_engagement_receipt.v1',
  product: 'qa-swarm',
  packageKind: 'swarm_audit',
  promiseIds: [...promiseIds],
  receiptRef: record.receiptRef,
  receiptUrl: `${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT}/${encodeURIComponent(
    record.receiptRef,
  )}`,
  generatedAt,
  recordedAt: record.recordedAt,
  purchase: {
    operatorAssisted: true,
    selfServe: false,
    paymentPath: record.paymentPath,
    intakeReceiptRef: record.intakeReceiptRef,
    checkoutOrDepositReceiptRef: record.checkoutOrDepositReceiptRef,
    committedAmountCents: record.committedAmountCents,
    paymentEvidenceRecorded: true,
    rawPaymentMaterialIncluded: false,
    firstPaidDeliveryReceipt: false,
    settlementMovedMoney: false,
  },
  provision: {
    workspaceRef: `workspace:${record.workspaceId}`,
    servicePromiseContractRef: `omni_accepted_outcome:${record.servicePromiseContractId}`,
    servicePromiseState: 'active',
    deliverableContractRef: record.packageContractRef,
  },
  commitment: {
    commitmentRef: record.commitmentRef,
    weeklyReviewRef: BUSINESS_COMMITMENT_WEEKLY_REVIEW_REF,
    promisedObjectRef: QA_SWARM_SWARM_AUDIT_DELIVERABLE_REF,
    dueState: 'due',
    firstReportDueAt: record.firstReportDueAt,
  },
  publicSafety,
  evidenceRefs: [...evidenceRefs],
  caveatRefs: [...caveatRefs],
  sourceRefs: [...sourceRefs],
  staleness: liveAtReadStaleness([
    QA_SWARM_FIRST_ENGAGEMENT_TABLE,
    'prefilled_workspaces',
    'omni_accepted_outcome_contracts',
    'business_commitment_ledger',
  ]),
})

const boundedIdempotencyKey = (
  value: string | undefined,
): string | undefined => {
  const trimmed = value?.trim()
  return trimmed !== undefined &&
    trimmed !== '' &&
    /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(trimmed)
    ? trimmed
    : undefined
}

const makeReceiptRef = (): string =>
  compactRandomId('receipt.qa_swarm.first_engagement').replace(
    'receipt.qa_swarm.first_engagement_',
    'receipt.qa_swarm.first_engagement.',
  )

const receiptSuffix = (receiptRef: string): string => {
  const parts = receiptRef.split('.')
  return parts[parts.length - 1] ?? 'unknown'
}

const makeCommitmentRef = (receiptRef: string): string =>
  `business.commitment.qa_swarm.swarm_audit.${receiptSuffix(receiptRef)}`

const privateMaterialKeyPattern =
  /(^|_|\b)(authorization|bearer|cwd|destination|home|log|mnemonic|path|paymentHash|preimage|prompt|raw[A-Za-z0-9_]*|secret|token|wallet)(_|$|\b)/i

const containsPrivateMaterialKey = (value: unknown): boolean => {
  if (Array.isArray(value)) {
    return value.some(item => containsPrivateMaterialKey(item))
  }
  if (typeof value !== 'object' || value === null) {
    return false
  }
  for (const [key, child] of Object.entries(value)) {
    if (privateMaterialKeyPattern.test(key)) return true
    if (containsPrivateMaterialKey(child)) return true
  }
  return false
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,300}$/
const unsafeRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|checkout[_-]?(raw|url)|cookie|customer[_-]?(email|name|phone|prompt|record|value)|email[_-]?(address|body|html|raw|text)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(raw|payload|body|url)|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|preimage|proof|raw|secret|payload)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(customer|key|repo|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(auth|customer|fixture|invoice|log|package|payment|payload|prompt|provider|receipt|runner|run[_-]?log|schema|source|target|trace|usage|webhook)|secret|seed[_-]?phrase|sk-[a-z0-9]|spark[_-]?(address|invoice|request|secret)|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const refIsSafe = (ref: string): boolean =>
  safeRefPattern.test(ref) &&
  !containsProviderSecretMaterial(ref) &&
  !unsafeRefPattern.test(ref) &&
  !rawTimestampPattern.test(ref)

const validateRef = (
  label: string,
  value: string,
): string | { error: string } => {
  const trimmed = value.trim()
  return trimmed !== '' && refIsSafe(trimmed)
    ? trimmed
    : { error: `${label}_unsafe` }
}

const decodeRequest = (
  value: unknown,
): QaSwarmFirstEngagementIntakeRequest | undefined => {
  try {
    return S.decodeUnknownSync(QaSwarmFirstEngagementIntakeRequest)(value)
  } catch {
    return undefined
  }
}

const readBody = (request: Request) =>
  Effect.promise(() => request.text().catch(() => ''))

type NormalizedBody = Readonly<{
  packageKind: 'swarm_audit'
  paymentPath: QaSwarmFirstEngagementPaymentPath
  businessSignupRequestId: string
  userId: string
  committedAmountCents: number
  intakeReceiptRef: string
  checkoutOrDepositReceiptRef: string
  targetAdapterReviewRef: string
  packageContractRef: string
  firstReportDueAt: string
}>

const isoTimestampPattern = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/

const validateDueAt = (
  value: string,
): string | { error: string } => {
  const trimmed = value.trim()
  return isoTimestampPattern.test(trimmed)
    ? trimmed
    : { error: 'first_report_due_at_must_be_iso' }
}

const normalizeBody = (
  body: QaSwarmFirstEngagementIntakeRequest,
): NormalizedBody | { error: string } => {
  if (
    !Number.isSafeInteger(body.committedAmountCents) ||
    body.committedAmountCents < 100_000 ||
    body.committedAmountCents > 500_000
  ) {
    return { error: 'swarm_audit_amount_must_match_public_band' }
  }

  const relevantPaymentReceiptRef =
    body.paymentPath === 'checkout_kickoff_receipt'
      ? body.checkoutKickoffReceiptRef
      : body.depositInvoiceReceiptRef

  if (relevantPaymentReceiptRef === undefined) {
    return {
      error:
        body.paymentPath === 'checkout_kickoff_receipt'
          ? 'checkout_kickoff_receipt_ref_required'
          : 'deposit_invoice_receipt_ref_required',
    }
  }

  const refFields = [
    ['business_signup_request_id', body.businessSignupRequestId],
    ['user_id', body.userId],
    ['intake_receipt_ref', body.intakeReceiptRef],
    ['checkout_or_deposit_receipt_ref', relevantPaymentReceiptRef],
    ['target_adapter_review_ref', body.targetAdapterReviewRef],
    ['package_contract_ref', body.packageContractRef],
  ] as const
  const normalized = new Map<string, string>()

  for (const [label, value] of refFields) {
    const ref = validateRef(label, value)
    if (typeof ref !== 'string') {
      return ref
    }
    normalized.set(label, ref)
  }

  const firstReportDueAt = validateDueAt(body.firstReportDueAt)
  if (typeof firstReportDueAt !== 'string') {
    return firstReportDueAt
  }

  return {
    packageKind: 'swarm_audit',
    paymentPath: body.paymentPath,
    businessSignupRequestId: normalized.get('business_signup_request_id')!,
    userId: normalized.get('user_id')!,
    committedAmountCents: body.committedAmountCents,
    intakeReceiptRef: normalized.get('intake_receipt_ref')!,
    checkoutOrDepositReceiptRef: normalized.get(
      'checkout_or_deposit_receipt_ref',
    )!,
    targetAdapterReviewRef: normalized.get('target_adapter_review_ref')!,
    packageContractRef: normalized.get('package_contract_ref')!,
    firstReportDueAt,
  }
}

const receiptRefFromPath = (pathname: string): string | null => {
  const prefix = `${QA_SWARM_FIRST_ENGAGEMENT_PUBLIC_ENDPOINT}/`
  return pathname.startsWith(prefix) && pathname.length > prefix.length
    ? decodeURIComponent(pathname.slice(prefix.length))
    : null
}

const receiptRefPattern =
  /^receipt\.qa_swarm\.first_engagement\.[A-Za-z0-9_-]+$/

export const handleOperatorQaSwarmFirstEngagementsApi = (
  request: Request,
  input: OperatorRouteInput,
): Effect.Effect<HttpResponse> =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const authorized = yield* Effect.promise(() =>
      input.requireAdminApiToken(request).catch(() => false),
    )

    if (!authorized) {
      return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
    }

    const text = yield* readBody(request)
    const parsed = yield* Effect.try({
      try: () => parseJsonUnknown(text),
      catch: () => undefined,
    }).pipe(Effect.catch(() => Effect.void))

    if (parsed === undefined) {
      return noStoreJsonResponse({ error: 'invalid_json' }, { status: 400 })
    }

    if (containsPrivateMaterialKey(parsed)) {
      return noStoreJsonResponse(
        { error: 'private_material_not_allowed' },
        { status: 400 },
      )
    }

    const body = decodeRequest(parsed)
    if (body === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_request_schema' },
        { status: 400 },
      )
    }

    const normalized = normalizeBody(body)
    if ('error' in normalized) {
      return noStoreJsonResponse(
        { error: 'invalid_public_safe_evidence', reason: normalized.error },
        { status: 400 },
      )
    }

    const clientKey = boundedIdempotencyKey(body.idempotencyKey)
    if (body.idempotencyKey !== undefined && clientKey === undefined) {
      return noStoreJsonResponse(
        { error: 'invalid_idempotency_key' },
        { status: 400 },
      )
    }

    const nowIso = input.nowIso ?? currentIsoTimestamp
    const recordedAt = nowIso()
    const receiptRef = makeReceiptRef()
    const idempotencyKey = `qa-swarm-first-engagement:${
      clientKey ??
      [
        normalized.businessSignupRequestId,
        normalized.paymentPath,
        normalized.checkoutOrDepositReceiptRef,
      ].join(':')
    }`
    const store =
      input.store ??
      makeD1QaSwarmFirstEngagementStore(
        input.OPENAGENTS_DB,
        undefined,
        input.mirror,
      )
    const recorded = yield* store
      .recordFirstEngagement({
        ...normalized,
        commitmentRef: makeCommitmentRef(receiptRef),
        idempotencyKey,
        receiptRef,
        recordedAt,
        servicePromiseContractId: '',
        workspaceId: '',
      })
      .pipe(
        Effect.catch(() =>
          Effect.succeed<
            Readonly<{
              record: QaSwarmFirstEngagementRecord
              idempotent: boolean
            }> | null
          >(null),
        ),
      )

    if (recorded === null) {
      return noStoreJsonResponse(
        { error: 'qa_swarm_first_engagement_receipt_unavailable' },
        { status: 503 },
      )
    }

    const generatedAt = nowIso()
    const receipt = publicQaSwarmFirstEngagementReceipt(
      recorded.record,
      generatedAt,
    )

    return noStoreJsonResponse(
      {
        ok: true,
        idempotent: recorded.idempotent,
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies OperatorQaSwarmFirstEngagementIntakeEnvelope,
      { status: recorded.idempotent ? 200 : 201 },
    )
  })

export const handlePublicQaSwarmFirstEngagementRead = (
  request: Request,
  input: RouteInput & Readonly<{ receiptRef: string }>,
): Effect.Effect<HttpResponse> => {
  if (request.method !== 'GET') {
    return Effect.succeed(methodNotAllowed(['GET']))
  }
  if (!receiptRefPattern.test(input.receiptRef)) {
    return Effect.succeed(notFound())
  }

  const nowIso = input.nowIso ?? currentIsoTimestamp
  const store =
    input.store ?? makeD1QaSwarmFirstEngagementStore(input.OPENAGENTS_DB)

  return store.readFirstEngagement(input.receiptRef).pipe(
    Effect.map(record => {
      if (record === null) {
        return notFound()
      }

      const generatedAt = nowIso()
      const receipt = publicQaSwarmFirstEngagementReceipt(record, generatedAt)

      return noStoreJsonResponse({
        generatedAt,
        staleness: receipt.staleness,
        receipt,
      } satisfies PublicQaSwarmFirstEngagementEnvelope)
    }),
    Effect.catch(() =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'qa_swarm_first_engagement_receipt_unavailable' },
          { status: 503 },
        ),
      ),
    ),
  )
}

export const makeQaSwarmFirstEngagementRoutes = <Bindings>(
  dependencies: Readonly<{
    makeStore: (env: Bindings) => QaSwarmFirstEngagementStore
    nowIso: () => string
  }>,
) => ({
  routePublicQaSwarmFirstEngagementRequest: (
    request: Request,
    env: Bindings,
  ): Effect.Effect<HttpResponse> | undefined => {
    const receiptRef = receiptRefFromPath(new URL(request.url).pathname)
    return receiptRef === null
      ? undefined
      : handlePublicQaSwarmFirstEngagementRead(request, {
          receiptRef,
          store: dependencies.makeStore(env),
          nowIso: dependencies.nowIso,
        })
  },
})
