import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const FirstBatchPaymentPolicyMode = S.Literals([
  'public_beta_free',
  'operator_grant',
])
export type FirstBatchPaymentPolicyMode =
  typeof FirstBatchPaymentPolicyMode.Type

export type FirstBatchPaymentPolicy = Readonly<{
  appliedByUserId: string | null
  assignmentId: string | null
  createdAt: string
  customerSafeSummary: string
  id: string
  policyMode: FirstBatchPaymentPolicyMode
  reason: string
  siteId: string | null
  softwareOrderId: string
  updatedAt: string
}>

export type FirstBatchPaymentPolicyGate = Readonly<{
  ok: boolean
  policy: FirstBatchPaymentPolicy | null
  required: boolean
  status: 'not_required' | 'missing' | 'satisfied'
}>

export type FirstBatchPaymentPolicyRuntime = Readonly<{
  makePolicyId: () => string
  nowIso: () => string
}>

export const systemFirstBatchPaymentPolicyRuntime: FirstBatchPaymentPolicyRuntime =
  {
    makePolicyId: () => compactRandomId('first_batch_payment_policy'),
    nowIso: currentIsoTimestamp,
  }

export class FirstBatchPaymentPolicyStorageError extends S.TaggedErrorClass<FirstBatchPaymentPolicyStorageError>()(
  'FirstBatchPaymentPolicyStorageError',
  {
    operation: S.String,
    error: S.Defect,
  },
) {}

export class FirstBatchPaymentPolicyUnsafe extends S.TaggedErrorClass<FirstBatchPaymentPolicyUnsafe>()(
  'FirstBatchPaymentPolicyUnsafe',
  {
    reason: S.String,
  },
) {}

export type FirstBatchPaymentPolicyError =
  | FirstBatchPaymentPolicyStorageError
  | FirstBatchPaymentPolicyUnsafe

type FirstBatchPaymentPolicyRow = Readonly<{
  applied_by_user_id: string | null
  assignment_id: string | null
  created_at: string
  customer_safe_summary: string
  id: string
  policy_mode: FirstBatchPaymentPolicyMode
  reason: string
  site_id: string | null
  software_order_id: string
  updated_at: string
}>

type FirstBatchPolicyRequiredRow = Readonly<{
  id: string
}>

const disallowedNoChargeClaims = [
  /\bpaid\b/i,
  /\bsettled\b/i,
  /\bsettlement\b/i,
  /\bpayout\b/i,
  /\bprovider payout\b/i,
  /\bbitcoin\b/i,
  /\blightning\b/i,
  /\bldk\b/i,
  /\bmdk\b/i,
  /\bl402\b/i,
  /\bwallet\b/i,
  /\binvoice\b/i,
  /\bpayment id\b/i,
]

const d1Effect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, FirstBatchPaymentPolicyStorageError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new FirstBatchPaymentPolicyStorageError({ operation, error }),
  })

const compactRequiredText = (
  field: string,
  value: string,
): Effect.Effect<string, FirstBatchPaymentPolicyUnsafe> =>
  Effect.gen(function* () {
    const text = value.replace(/\s+/g, ' ').trim()

    if (text === '') {
      return yield* new FirstBatchPaymentPolicyUnsafe({
        reason: `${field} is required.`,
      })
    }

    if (containsProviderSecretMaterial(text)) {
      return yield* new FirstBatchPaymentPolicyUnsafe({
        reason: `${field} contains secret-shaped material.`,
      })
    }

    return text.length <= 500 ? text : `${text.slice(0, 497)}...`
  })

const customerSafeNoChargeSummary = (
  value: string,
): Effect.Effect<string, FirstBatchPaymentPolicyUnsafe> =>
  Effect.gen(function* () {
    const text = yield* compactRequiredText('customerSafeSummary', value)
    const disallowed = disallowedNoChargeClaims.find(pattern =>
      pattern.test(text),
    )

    if (disallowed !== undefined) {
      return yield* new FirstBatchPaymentPolicyUnsafe({
        reason:
          'No-payment customer summary must not imply paid, settled, Lightning, MDK, wallet, or provider-payout activity.',
      })
    }

    return text
  })

const rowToPolicy = (
  row: FirstBatchPaymentPolicyRow,
): FirstBatchPaymentPolicy => ({
  appliedByUserId: row.applied_by_user_id,
  assignmentId: row.assignment_id,
  createdAt: row.created_at,
  customerSafeSummary: row.customer_safe_summary,
  id: row.id,
  policyMode: row.policy_mode,
  reason: row.reason,
  siteId: row.site_id,
  softwareOrderId: row.software_order_id,
  updatedAt: row.updated_at,
})

export const readFirstBatchPaymentPolicyForOrder = (
  db: D1Database,
  softwareOrderId: string,
): Effect.Effect<
  FirstBatchPaymentPolicy | null,
  FirstBatchPaymentPolicyStorageError
> =>
  d1Effect('firstBatchPaymentPolicies.order.read', () =>
    db
      .prepare(
        `SELECT id,
                software_order_id,
                assignment_id,
                site_id,
                policy_mode,
                applied_by_user_id,
                reason,
                customer_safe_summary,
                created_at,
                updated_at
           FROM first_batch_payment_policies
          WHERE software_order_id = ?
            AND archived_at IS NULL
          ORDER BY updated_at DESC
          LIMIT 1`,
      )
      .bind(softwareOrderId)
      .first<FirstBatchPaymentPolicyRow>(),
  ).pipe(Effect.map(row => (row === null ? null : rowToPolicy(row))))

export const isFirstBatchPaymentPolicyRequired = (
  db: D1Database,
  softwareOrderId: string | null,
): Effect.Effect<boolean, FirstBatchPaymentPolicyStorageError> =>
  softwareOrderId === null
    ? Effect.succeed(false)
    : d1Effect('firstBatchPaymentPolicies.required.read', () =>
        db
          .prepare(
            `SELECT id
               FROM order_triage_records
              WHERE software_order_id = ?
                AND first_batch_eligible = 1
                AND classification IN ('runnable_site', 'runnable_general_autopilot')
                AND archived_at IS NULL
              LIMIT 1`,
          )
          .bind(softwareOrderId)
          .first<FirstBatchPolicyRequiredRow>(),
      ).pipe(Effect.map(row => row !== null))

export const readFirstBatchPaymentGate = (
  db: D1Database,
  softwareOrderId: string | null,
): Effect.Effect<FirstBatchPaymentPolicyGate, FirstBatchPaymentPolicyStorageError> =>
  Effect.gen(function* () {
    const required = yield* isFirstBatchPaymentPolicyRequired(
      db,
      softwareOrderId,
    )

    if (!required || softwareOrderId === null) {
      return {
        ok: true,
        policy: null,
        required: false,
        status: 'not_required' as const,
      }
    }

    const policy = yield* readFirstBatchPaymentPolicyForOrder(
      db,
      softwareOrderId,
    )

    return {
      ok: policy !== null,
      policy,
      required: true,
      status: policy === null ? 'missing' : 'satisfied',
    }
  })

export const upsertFirstBatchPaymentPolicy = (
  db: D1Database,
  runtime: FirstBatchPaymentPolicyRuntime,
  input: Readonly<{
    appliedByUserId: string
    assignmentId?: string | null | undefined
    customerSafeSummary: string
    policyMode: FirstBatchPaymentPolicyMode
    reason: string
    siteId?: string | null | undefined
    softwareOrderId: string
  }>,
): Effect.Effect<FirstBatchPaymentPolicy, FirstBatchPaymentPolicyError> =>
  Effect.gen(function* () {
    const reason = yield* compactRequiredText('reason', input.reason)
    const customerSafeSummary = yield* customerSafeNoChargeSummary(
      input.customerSafeSummary,
    )
    const now = runtime.nowIso()
    const assignmentId = input.assignmentId ?? null
    const siteId = input.siteId ?? null

    const update = yield* d1Effect('firstBatchPaymentPolicies.update', () =>
      db
        .prepare(
          `UPDATE first_batch_payment_policies
              SET assignment_id = ?,
                  site_id = ?,
                  policy_mode = ?,
                  applied_by_user_id = ?,
                  reason = ?,
                  customer_safe_summary = ?,
                  updated_at = ?
            WHERE software_order_id = ?
              AND archived_at IS NULL`,
        )
        .bind(
          assignmentId,
          siteId,
          input.policyMode,
          input.appliedByUserId,
          reason,
          customerSafeSummary,
          now,
          input.softwareOrderId,
        )
        .run(),
    )

    if (Number(update.meta?.changes ?? 0) === 0) {
      yield* d1Effect('firstBatchPaymentPolicies.insert', () =>
        db
          .prepare(
            `INSERT INTO first_batch_payment_policies
               (id,
                software_order_id,
                assignment_id,
                site_id,
                policy_mode,
                applied_by_user_id,
                reason,
                customer_safe_summary,
                created_at,
                updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          )
          .bind(
            runtime.makePolicyId(),
            input.softwareOrderId,
            assignmentId,
            siteId,
            input.policyMode,
            input.appliedByUserId,
            reason,
            customerSafeSummary,
            now,
            now,
          )
          .run(),
      )
    }

    const policy = yield* readFirstBatchPaymentPolicyForOrder(
      db,
      input.softwareOrderId,
    )

    if (policy === null) {
      return yield* new FirstBatchPaymentPolicyStorageError({
        error: 'First-batch payment policy insert did not return a row.',
        operation: 'firstBatchPaymentPolicies.upsert.readback',
      })
    }

    return policy
  })
