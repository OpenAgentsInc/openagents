import { Effect, Schema as S } from 'effect'

import { parseJsonStringArray } from './json-boundary'
import {
  compactRandomId,
  epochMillisToIsoTimestamp,
  isoTimestampAfterIso,
} from './runtime-primitives'

export const BUSINESS_FULFILLMENT_LOOP_AGENT_DEFINITION_REF =
  'agent_definition.v1.business_fulfillment_loop'

export const BUSINESS_FULFILLMENT_LOOP_DEFAULT_LIMIT = 10

export const BusinessServicePromiseState = S.Literals([
  'active',
  'paused',
  'blocked',
  'closed',
])
export type BusinessServicePromiseState =
  typeof BusinessServicePromiseState.Type

export const BusinessServicePromiseCadence = S.Literals(['daily', 'weekly'])
export type BusinessServicePromiseCadence =
  typeof BusinessServicePromiseCadence.Type

export const BusinessServicePromiseRecord = S.Struct({
  acceptedOutcomeContractId: S.NullOr(S.String),
  cadence: BusinessServicePromiseCadence,
  createdAt: S.String,
  crmStateRef: S.String,
  id: S.String,
  lastMotionReceiptRef: S.NullOr(S.String),
  nextMotionDueAt: S.NullOr(S.String),
  promiseRef: S.String,
  sourceRefs: S.Array(S.String),
  stakeholderRefs: S.Array(S.String),
  state: BusinessServicePromiseState,
  updatedAt: S.String,
  workspaceRef: S.String,
})
export type BusinessServicePromiseRecord =
  typeof BusinessServicePromiseRecord.Type

export const BusinessFulfillmentMotionReceipt = S.Struct({
  agentDefinitionRef: S.String,
  approvalGateRef: S.String,
  blockerRefs: S.Array(S.String),
  cadence: BusinessServicePromiseCadence,
  clientCommsDraftRef: S.String,
  clientCommsEmailLedgerRef: S.String,
  createdAt: S.String,
  crmStateRef: S.String,
  customerVisibleWorkroomUpdateRef: S.String,
  forwardMotionRef: S.String,
  id: S.String,
  motionDate: S.String,
  outboundAllowed: S.Boolean,
  promiseId: S.String,
  promiseRef: S.String,
  receiptRef: S.String,
  sourceRefs: S.Array(S.String),
  stakeholderFlagRefs: S.Array(S.String),
  stakeholderRefs: S.Array(S.String),
})
export type BusinessFulfillmentMotionReceipt =
  typeof BusinessFulfillmentMotionReceipt.Type

export type BusinessFulfillmentLoopStore = Readonly<{
  claimMotionReceipt: (
    receipt: BusinessFulfillmentMotionReceipt,
  ) => Promise<Readonly<{ claimed: boolean }>>
  listDuePromises: (
    nowIso: string,
    limit: number,
  ) => Promise<ReadonlyArray<BusinessServicePromiseRecord>>
  markPromiseMotionRecorded: (
    promiseId: string,
    receiptRef: string,
    nextMotionDueAt: string,
    nowIso: string,
  ) => Promise<void>
}>

export type BusinessFulfillmentLoopRuntime = Readonly<{
  makeId: (prefix: string) => string
  nowIso: () => string
}>

export type BusinessFulfillmentLoopResult = Readonly<{
  duePromiseCount: number
  motionReceiptRefs: ReadonlyArray<string>
  skippedDuplicateCount: number
  state: 'completed' | 'skipped'
  workroomUpdateRefs: ReadonlyArray<string>
}>

export class BusinessFulfillmentLoopValidationError extends S.TaggedErrorClass<BusinessFulfillmentLoopValidationError>()(
  'BusinessFulfillmentLoopValidationError',
  { reason: S.String },
) {}

export class BusinessFulfillmentLoopStorageError extends S.TaggedErrorClass<BusinessFulfillmentLoopStorageError>()(
  'BusinessFulfillmentLoopStorageError',
  { reason: S.String },
) {}

export type BusinessFulfillmentLoopError =
  | BusinessFulfillmentLoopStorageError
  | BusinessFulfillmentLoopValidationError

const businessFulfillmentLoopErrorFromUnknown = (
  error: unknown,
): BusinessFulfillmentLoopError =>
  error instanceof BusinessFulfillmentLoopValidationError
    ? error
    : new BusinessFulfillmentLoopStorageError({
        reason: error instanceof Error ? error.message : String(error),
      })

const tryLoopPromise = <A>(
  promise: () => Promise<A>,
): Effect.Effect<A, BusinessFulfillmentLoopError> =>
  Effect.tryPromise({
    catch: businessFulfillmentLoopErrorFromUnknown,
    try: promise,
  })

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=#-]{0,220}$/
const UNSAFE_TEXT_PATTERN =
  /\b(raw[_ -]?crm|raw[_ -]?email|email[_ -]?body|client[_ -]?name|client[_ -]?email|customer[_ -]?name|customer[_ -]?email|contact[_ -]?email|provider[_ -]?payload|access_token|refresh_token|private_key|wallet_secret|payment_preimage|webhook_secret|xprv|mnemonic)\b|@/i

const assertSafeRef = (field: string, value: string): void => {
  if (!SAFE_REF_PATTERN.test(value) || UNSAFE_TEXT_PATTERN.test(value)) {
    throw new BusinessFulfillmentLoopValidationError({
      reason: `${field} must be an opaque public-safe ref`,
    })
  }
}

const assertSafeRefs = (
  field: string,
  values: ReadonlyArray<string>,
): void => {
  values.forEach(value => assertSafeRef(field, value))
}

const promiseFromRow = (
  row: Readonly<Record<string, unknown>>,
): BusinessServicePromiseRecord => ({
  acceptedOutcomeContractId:
    typeof row.accepted_outcome_contract_id === 'string'
      ? row.accepted_outcome_contract_id
      : null,
  createdAt: String(row.created_at),
  crmStateRef: String(row.crm_state_ref),
  id: String(row.id),
  lastMotionReceiptRef:
    typeof row.last_motion_receipt_ref === 'string'
      ? row.last_motion_receipt_ref
      : null,
  nextMotionDueAt:
    typeof row.next_motion_due_at === 'string' ? row.next_motion_due_at : null,
  promiseRef: String(row.promise_ref),
  sourceRefs: parseJsonStringArray(String(row.source_refs_json ?? '[]')),
  stakeholderRefs: parseJsonStringArray(
    String(row.stakeholder_refs_json ?? '[]'),
  ),
  cadence: S.decodeUnknownSync(BusinessServicePromiseCadence)(row.cadence),
  state: S.decodeUnknownSync(BusinessServicePromiseState)(row.state),
  updatedAt: String(row.updated_at),
  workspaceRef: String(row.workspace_ref),
})

export const makeD1BusinessFulfillmentLoopStore = (
  db: D1Database,
): BusinessFulfillmentLoopStore => ({
  claimMotionReceipt: async receipt => {
    const result = await db
      .prepare(
        `INSERT OR IGNORE INTO business_fulfillment_motion_receipts (
          id,
          promise_id,
          promise_ref,
          motion_date,
          receipt_ref,
          agent_definition_ref,
          cadence,
          crm_state_ref,
          stakeholder_refs_json,
          stakeholder_flag_refs_json,
          forward_motion_ref,
          client_comms_draft_ref,
          client_comms_email_ledger_ref,
          customer_visible_workroom_update_ref,
          approval_gate_ref,
          outbound_allowed,
          blocker_refs_json,
          source_refs_json,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        receipt.id,
        receipt.promiseId,
        receipt.promiseRef,
        receipt.motionDate,
        receipt.receiptRef,
        receipt.agentDefinitionRef,
        receipt.cadence,
        receipt.crmStateRef,
        JSON.stringify(receipt.stakeholderRefs),
        JSON.stringify(receipt.stakeholderFlagRefs),
        receipt.forwardMotionRef,
        receipt.clientCommsDraftRef,
        receipt.clientCommsEmailLedgerRef,
        receipt.customerVisibleWorkroomUpdateRef,
        receipt.approvalGateRef,
        receipt.outboundAllowed ? 1 : 0,
        JSON.stringify(receipt.blockerRefs),
        JSON.stringify(receipt.sourceRefs),
        receipt.createdAt,
      )
      .run()

    return { claimed: (result.meta.changes ?? 0) > 0 }
  },
  listDuePromises: async (nowIso, limit) => {
    const rows = await db
      .prepare(
        `SELECT *
           FROM business_service_promises
          WHERE state = 'active'
            AND cadence IN ('daily', 'weekly')
            AND (next_motion_due_at IS NULL OR next_motion_due_at <= ?)
          ORDER BY COALESCE(next_motion_due_at, created_at) ASC, updated_at ASC
          LIMIT ?`,
      )
      .bind(nowIso, limit)
      .all<Record<string, unknown>>()

    return (rows.results ?? []).map(promiseFromRow)
  },
  markPromiseMotionRecorded: async (
    promiseId,
    receiptRef,
    nextMotionDueAt,
    nowIso,
  ) => {
    await db
      .prepare(
        `UPDATE business_service_promises
            SET last_motion_receipt_ref = ?,
                next_motion_due_at = ?,
                updated_at = ?
          WHERE id = ?`,
      )
      .bind(receiptRef, nextMotionDueAt, nowIso, promiseId)
      .run()
  },
})

const refSuffix = (value: string): string =>
  value.toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '')

const cadenceIntervalMs = (cadence: BusinessServicePromiseCadence): number =>
  cadence === 'weekly' ? 7 * 24 * 60 * 60 * 1000 : 24 * 60 * 60 * 1000

const nextDueAtForCadence = (
  cadence: BusinessServicePromiseCadence,
  nowIso: string,
): string => isoTimestampAfterIso(nowIso, cadenceIntervalMs(cadence))

const validatePromise = (promise: BusinessServicePromiseRecord): void => {
  assertSafeRef('promiseRef', promise.promiseRef)
  assertSafeRef('crmStateRef', promise.crmStateRef)
  assertSafeRef('workspaceRef', promise.workspaceRef)
  if (promise.acceptedOutcomeContractId !== null) {
    assertSafeRef('acceptedOutcomeContractId', promise.acceptedOutcomeContractId)
  }
  assertSafeRefs('stakeholderRefs', promise.stakeholderRefs)
  assertSafeRefs('sourceRefs', promise.sourceRefs)
}

export const buildBusinessFulfillmentMotionReceipt = (
  promise: BusinessServicePromiseRecord,
  runtime: BusinessFulfillmentLoopRuntime,
): BusinessFulfillmentMotionReceipt => {
  validatePromise(promise)

  const nowIso = runtime.nowIso()
  const motionDate = nowIso.slice(0, 10)
  const suffix = refSuffix(`${promise.promiseRef}.${motionDate}`)
  const sourceRefs = [
    ...promise.sourceRefs,
    'docs/fable/ROADMAP_BIZ.md#BF-5.1',
    'docs/fable/ROADMAP_BIZ.md#BF-5.3',
    'docs/fable/2026-07-02-business-fulfillment-engine-meditations.md#fulfillment-agents',
  ]

  return {
    agentDefinitionRef: BUSINESS_FULFILLMENT_LOOP_AGENT_DEFINITION_REF,
    approvalGateRef: `approval_gate.business_fulfillment.client_comms.${suffix}`,
    blockerRefs: [],
    cadence: promise.cadence,
    clientCommsDraftRef: `draft.business_fulfillment.client_comms.${suffix}`,
    clientCommsEmailLedgerRef: `email_campaign_send.business_fulfillment.client_comms.${suffix}`,
    createdAt: nowIso,
    crmStateRef: promise.crmStateRef,
    customerVisibleWorkroomUpdateRef: `workroom_update.business_fulfillment.customer_visible.${suffix}`,
    forwardMotionRef: `motion.business_fulfillment.${promise.cadence}.${suffix}`,
    id: runtime.makeId('business_fulfillment_motion'),
    motionDate,
    outboundAllowed: false,
    promiseId: promise.id,
    promiseRef: promise.promiseRef,
    receiptRef: `receipt.business_fulfillment.${promise.cadence}_motion.${suffix}`,
    sourceRefs,
    stakeholderFlagRefs: promise.stakeholderRefs.map(
      stakeholderRef =>
        `stakeholder_flag.business_fulfillment.${refSuffix(stakeholderRef)}.${motionDate.replaceAll('-', '')}`,
    ),
    stakeholderRefs: promise.stakeholderRefs,
  }
}

export const runBusinessFulfillmentLoop = (
  input: Readonly<{
    limit?: number | undefined
    runtime: BusinessFulfillmentLoopRuntime
    store: BusinessFulfillmentLoopStore
  }>,
): Effect.Effect<BusinessFulfillmentLoopResult, BusinessFulfillmentLoopError> =>
  Effect.gen(function* () {
    const nowIso = input.runtime.nowIso()
    const duePromises = yield* tryLoopPromise(() =>
      input.store.listDuePromises(
        nowIso,
        input.limit ?? BUSINESS_FULFILLMENT_LOOP_DEFAULT_LIMIT,
      ),
    )
    const outcomes = yield* Effect.forEach(
      duePromises,
      promise =>
        Effect.gen(function* () {
          const receipt = yield* Effect.try({
            catch: businessFulfillmentLoopErrorFromUnknown,
            try: () =>
              buildBusinessFulfillmentMotionReceipt(promise, input.runtime),
          })
          const claim = yield* tryLoopPromise(() =>
            input.store.claimMotionReceipt(receipt),
          )

          if (!claim.claimed) {
            return {
              maybeReceiptRef: null,
              maybeWorkroomUpdateRef: null,
              skippedDuplicateCount: 1,
            }
          }

          yield* tryLoopPromise(() =>
            input.store.markPromiseMotionRecorded(
              promise.id,
              receipt.receiptRef,
              nextDueAtForCadence(promise.cadence, nowIso),
              nowIso,
            ),
          )

          return {
            maybeReceiptRef: receipt.receiptRef,
            maybeWorkroomUpdateRef: receipt.customerVisibleWorkroomUpdateRef,
            skippedDuplicateCount: 0,
          }
        }),
      { concurrency: 1 },
    )

    const motionReceiptRefs = outcomes.flatMap(outcome =>
      outcome.maybeReceiptRef === null ? [] : [outcome.maybeReceiptRef],
    )
    const skippedDuplicateCount = outcomes.reduce(
      (sum, outcome) => sum + outcome.skippedDuplicateCount,
      0,
    )
    const workroomUpdateRefs = outcomes.flatMap(outcome =>
      outcome.maybeWorkroomUpdateRef === undefined ||
      outcome.maybeWorkroomUpdateRef === null
        ? []
        : [outcome.maybeWorkroomUpdateRef],
    )

    return {
      duePromiseCount: duePromises.length,
      motionReceiptRefs,
      skippedDuplicateCount,
      state: duePromises.length === 0 ? 'skipped' : 'completed',
      workroomUpdateRefs,
    }
  })

export const runBusinessFulfillmentLoopScheduled = (
  db: D1Database,
  scheduledTimeMs: number,
): Effect.Effect<BusinessFulfillmentLoopResult, never> =>
  runBusinessFulfillmentLoop({
    runtime: {
      makeId: compactRandomId,
      nowIso: () => epochMillisToIsoTimestamp(scheduledTimeMs),
    },
    store: makeD1BusinessFulfillmentLoopStore(db),
  }).pipe(
    Effect.catch(() =>
      Effect.succeed({
        duePromiseCount: 0,
        motionReceiptRefs: [],
        skippedDuplicateCount: 0,
        state: 'skipped' as const,
        workroomUpdateRefs: [],
      }),
    ),
  )
