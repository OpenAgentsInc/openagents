import { Schema as S } from 'effect'

import {
  makePrefilledWorkspaceService,
  type PrefilledWorkspaceRecord,
} from './prefilled-workspace'
import { readBusinessSignupRequest } from './business-signup-routes'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export type BusinessCheckoutKickoffInput = Readonly<{
  checkoutSessionId: string
  creditGrantCents: number
  setupFeeCents: number
  signupId: string
  totalAmountCents: number
  userId: string
}>

export type BusinessCheckoutKickoffRecord = Readonly<{
  checkoutSessionId: string
  creditGrantCents: number
  publicReceiptRef: string
  servicePromiseContractId: string
  setupFeeCents: number
  signupId: string
  totalAmountCents: number
  userId: string
  workspaceId: string
}>

type KickoffRow = Readonly<{
  checkout_session_id: string
  business_signup_request_id: string
  user_id: string
  total_amount_cents: number
  setup_fee_cents: number
  credit_grant_cents: number
  workspace_id: string
  service_promise_contract_id: string
  public_receipt_ref: string
}>

export class BusinessCheckoutKickoffError extends S.TaggedErrorClass<BusinessCheckoutKickoffError>()(
  'BusinessCheckoutKickoffError',
  { reason: S.String },
) {}

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/

const safeRef = (field: string, value: string): string => {
  const ref = value.trim()

  if (!SAFE_REF_PATTERN.test(ref)) {
    throw new BusinessCheckoutKickoffError({
      reason: `${field} must be an opaque public-safe ref.`,
    })
  }

  return ref
}

const nonNegativeInt = (field: string, value: number): number => {
  const next = Math.trunc(value)

  if (!Number.isFinite(next) || next < 0) {
    throw new BusinessCheckoutKickoffError({
      reason: `${field} must be a non-negative integer.`,
    })
  }

  return next
}

const rowToRecord = (row: KickoffRow): BusinessCheckoutKickoffRecord => ({
  checkoutSessionId: row.checkout_session_id,
  creditGrantCents: row.credit_grant_cents,
  publicReceiptRef: row.public_receipt_ref,
  servicePromiseContractId: row.service_promise_contract_id,
  setupFeeCents: row.setup_fee_cents,
  signupId: row.business_signup_request_id,
  totalAmountCents: row.total_amount_cents,
  userId: row.user_id,
  workspaceId: row.workspace_id,
})

const readKickoff = async (
  db: D1Database,
  checkoutSessionId: string,
): Promise<BusinessCheckoutKickoffRecord | null> => {
  const row = await db
    .prepare(
      `SELECT checkout_session_id,
              business_signup_request_id,
              user_id,
              total_amount_cents,
              setup_fee_cents,
              credit_grant_cents,
              workspace_id,
              service_promise_contract_id,
              public_receipt_ref
         FROM business_checkout_kickoffs
        WHERE checkout_session_id = ?
        LIMIT 1`,
    )
    .bind(checkoutSessionId)
    .first<KickoffRow>()

  return row === null ? null : rowToRecord(row)
}

const createWorkspace = async (
  db: D1Database,
  input: BusinessCheckoutKickoffInput,
): Promise<PrefilledWorkspaceRecord> => {
  const signup = await readBusinessSignupRequest(db, input.signupId)

  if (signup === undefined) {
    throw new BusinessCheckoutKickoffError({
      reason: 'business signup request not found.',
    })
  }

  return makePrefilledWorkspaceService(db, {
    makeId: prefix => `${prefix}_${compactRandomId('business')}`,
    nowIso: currentIsoTimestamp,
  }).createWorkspace({
    holderRef: `business_signup:${input.signupId}`,
    holderUserId: input.userId,
    introReceipt: {
      summary: 'Business workspace provisioned from a settled checkout.',
      publicSourceRefs: [
        `business_signup:${input.signupId}`,
        `receipt.billing.stripe_checkout.${input.checkoutSessionId}`,
      ],
    },
    projectName: 'Business service workspace',
    seededMemory: [
      {
        label: 'Signup receipt',
        publicSourceRef: `business_signup:${input.signupId}`,
        value: 'Business intake receipt recorded.',
      },
      {
        label: 'Checkout receipt',
        publicSourceRef: `receipt.billing.stripe_checkout.${input.checkoutSessionId}`,
        value: 'Settled checkout recorded.',
      },
    ],
    starterWorkflows: [
      {
        description: 'Prepare the first operator-reviewed business deliverable.',
        outcomeKind: 'business_deliverable',
        status: 'queued',
        title: 'First deliverable',
      },
    ],
    status: 'active',
  })
}

const createServicePromise = (
  db: D1Database,
  input: BusinessCheckoutKickoffInput,
  workspace: PrefilledWorkspaceRecord,
): Promise<{ id: string }> =>
  Promise.resolve().then(async () => {
    const idempotencyKey = `business_checkout:${input.checkoutSessionId}`
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
    const receiptRef = `receipt.billing.stripe_checkout.${input.checkoutSessionId}`
    const workspaceRef = `workspace:${workspace.id}`

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
            created_at,
            updated_at,
            archived_at)
         VALUES (?, ?, 'business', ?, ?, ?, 'operator_review', 'draft',
                 'customer_safe_summary', 'credits_required', ?, 0, ?, ?, ?, ?, NULL)`,
      )
      .bind(
        id,
        idempotencyKey,
        workspaceRef,
        `business_signup:${input.signupId}`,
        JSON.stringify([
          {
            artifactKind: 'operator_receipt',
            publicSafe: false,
            required: true,
            sourceRef: receiptRef,
          },
        ]),
        JSON.stringify([
          {
            required: true,
            requirementKind: 'operator_review',
            sourceRef: 'gate.business.service_promise.operator_review.v1',
          },
          {
            required: true,
            requirementKind: 'proof_bundle_ready',
            sourceRef: receiptRef,
          },
        ]),
        `omni_accepted_outcome:business:${idempotencyKey}`,
        JSON.stringify({
          creditGrantCents: input.creditGrantCents,
          setupFeeCents: input.setupFeeCents,
          signupRef: `business_signup:${input.signupId}`,
          stripeCheckoutRef: input.checkoutSessionId,
          workspaceRef,
        }),
        now,
        now,
      )
      .run()

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
      throw new BusinessCheckoutKickoffError({
        reason: 'service promise contract was not persisted.',
      })
    }

    return inserted
  })

export const provisionBusinessCheckoutKickoff = async (
  db: D1Database,
  rawInput: BusinessCheckoutKickoffInput,
): Promise<BusinessCheckoutKickoffRecord> => {
  const input: BusinessCheckoutKickoffInput = {
    checkoutSessionId: safeRef(
      'checkoutSessionId',
      rawInput.checkoutSessionId,
    ),
    creditGrantCents: nonNegativeInt(
      'creditGrantCents',
      rawInput.creditGrantCents,
    ),
    setupFeeCents: nonNegativeInt('setupFeeCents', rawInput.setupFeeCents),
    signupId: safeRef('signupId', rawInput.signupId),
    totalAmountCents: nonNegativeInt(
      'totalAmountCents',
      rawInput.totalAmountCents,
    ),
    userId: safeRef('userId', rawInput.userId),
  }

  if (input.setupFeeCents + input.creditGrantCents !== input.totalAmountCents) {
    throw new BusinessCheckoutKickoffError({
      reason: 'setup fee plus credit grant must equal the checkout total.',
    })
  }

  const existing = await readKickoff(db, input.checkoutSessionId)

  if (existing !== null) {
    return existing
  }

  const workspace = await createWorkspace(db, input)
  const contract = await createServicePromise(db, input, workspace)
  const now = currentIsoTimestamp()
  const publicReceiptRef = `receipt.business.checkout_kickoff.${input.checkoutSessionId}`

  await db
    .prepare(
      `INSERT OR IGNORE INTO business_checkout_kickoffs
        (checkout_session_id,
         business_signup_request_id,
         user_id,
         total_amount_cents,
         setup_fee_cents,
         credit_grant_cents,
         workspace_id,
         service_promise_contract_id,
         public_receipt_ref,
         created_at,
         updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      input.checkoutSessionId,
      input.signupId,
      input.userId,
      input.totalAmountCents,
      input.setupFeeCents,
      input.creditGrantCents,
      workspace.id,
      contract.id,
      publicReceiptRef,
      now,
      now,
    )
    .run()

  const inserted = await readKickoff(db, input.checkoutSessionId)

  if (inserted === null) {
    throw new BusinessCheckoutKickoffError({
      reason: 'business checkout kickoff was not persisted.',
    })
  }

  return inserted
}
