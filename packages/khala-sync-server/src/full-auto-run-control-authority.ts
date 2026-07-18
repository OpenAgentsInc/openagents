import {
  FullAutoRunControlIntent as FullAutoRunControlIntentSchema,
  type FullAutoRunControlIntent,
  type FullAutoRunControlIntentDispatchRequest,
  type FullAutoRunControlIntentOutcomeReport,
} from "@openagentsinc/khala-sync"
import { Effect, Schema as S } from "effect"

import type { SyncSql } from "./sql.js"

/**
 * MOB-FA-02 (#8994): server-side durable storage for typed Pause/Resume/Stop
 * control intents dispatched from OpenAgents mobile toward a Desktop-owned
 * FullAutoRun. Sibling to `full-auto-run-projection-authority.ts`'s "simple,
 * owner-scoped, no idempotency/event-log machinery" discipline -- but this
 * table DOES need idempotency (a retried mobile POST after a dropped
 * response must not double-dispatch) and a durable status column (`pending`
 * until Desktop reports back), which the projection table deliberately does
 * not need (it is pure last-write-wins).
 *
 * AUTHORITY STAYS DESKTOP-SIDE. This repository only records the typed
 * request and its eventual outcome; it never applies a Pause/Resume/Stop
 * itself. Desktop's control-intent consumer
 * (`apps/openagents-desktop/src/full-auto-run-control-intent-consumer.ts`)
 * is the only caller that actually transitions a `FullAutoRun`.
 */

const OwnerUserId = S.Trim.check(S.isMinLength(1), S.isMaxLength(160))
const PublicRef = S.Trim.check(S.isMinLength(1), S.isMaxLength(160), S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u))

export class FullAutoRunControlAuthorityError extends S.TaggedErrorClass<FullAutoRunControlAuthorityError>()(
  "FullAutoRunControlAuthorityError",
  {
    kind: S.Literals(["invalid_request", "intent_exists", "intent_not_found", "storage_unavailable"]),
    reason: S.String,
  },
) {}

export type FullAutoRunControlDispatchInput = Readonly<{
  ownerUserId: string
  request: FullAutoRunControlIntentDispatchRequest
}>

export type FullAutoRunControlListInput = Readonly<{ ownerUserId: string }>

export type FullAutoRunControlReportOutcomeInput = Readonly<{
  ownerUserId: string
  outcome: FullAutoRunControlIntentOutcomeReport
}>

export type FullAutoRunControlAuthorityRepositoryShape = Readonly<{
  /** Idempotent: replaying the same `(ownerUserId, idempotencyKey)` or the
   * same `intentId` returns the ALREADY-STORED row rather than erroring or
   * inserting a duplicate -- a dropped-response retry from mobile is safe. */
  dispatch: (
    input: FullAutoRunControlDispatchInput,
  ) => Effect.Effect<FullAutoRunControlIntent, FullAutoRunControlAuthorityError>
  /** Bounded recent-first list for this owner -- both mobile (polling one
   * intent's outcome) and Desktop (pulling every still-pending intent) read
   * through this same method. */
  list: (
    input: FullAutoRunControlListInput,
  ) => Effect.Effect<ReadonlyArray<FullAutoRunControlIntent>, FullAutoRunControlAuthorityError>
  reportOutcome: (
    input: FullAutoRunControlReportOutcomeInput,
  ) => Effect.Effect<FullAutoRunControlIntent, FullAutoRunControlAuthorityError>
}>

type StoredRow = Readonly<{
  intent_id: string
  idempotency_key: string
  run_ref: string
  action: string
  surface: string
  status: string
  applied_at: string | null
  rejection_reason: string | null
  result_lifecycle_state: string | null
  created_at: string
}>

const decodeIntent = S.decodeUnknownSync(FullAutoRunControlIntentSchema)

const rowToIntent = (row: StoredRow): FullAutoRunControlIntent =>
  decodeIntent({
    schema: "full_auto_run.control_intent.v1",
    intentId: row.intent_id,
    idempotencyKey: row.idempotency_key,
    runRef: row.run_ref,
    action: row.action,
    surface: row.surface,
    createdAt: row.created_at,
    status: row.status,
    appliedAt: row.applied_at,
    rejectionReason: row.rejection_reason,
    resultLifecycleState: row.result_lifecycle_state,
  })

const invalidRequest = (reason: string): FullAutoRunControlAuthorityError =>
  new FullAutoRunControlAuthorityError({ kind: "invalid_request", reason })

const storageUnavailable = (cause: unknown): FullAutoRunControlAuthorityError =>
  new FullAutoRunControlAuthorityError({
    kind: "storage_unavailable",
    reason: cause instanceof Error ? cause.message : "full auto run control intent storage unavailable",
  })

const FULL_AUTO_RUN_CONTROL_INTENT_LIST_LIMIT = 200

export const makeFullAutoRunControlAuthority = (
  input: Readonly<{ sql: SyncSql; now?: () => Date }>,
): FullAutoRunControlAuthorityRepositoryShape => {
  const now = input.now ?? (() => new Date())

  const dispatch: FullAutoRunControlAuthorityRepositoryShape["dispatch"] = raw =>
    Effect.gen(function* () {
      const ownerUserId = yield* Effect.try({
        try: () => S.decodeUnknownSync(OwnerUserId)(raw.ownerUserId),
        catch: () => invalidRequest("invalid ownerUserId"),
      })
      const request = yield* Effect.try({
        try: () => ({
          intentId: S.decodeUnknownSync(PublicRef)(raw.request.intentId),
          idempotencyKey: S.decodeUnknownSync(PublicRef)(raw.request.idempotencyKey),
          runRef: S.decodeUnknownSync(PublicRef)(raw.request.runRef),
          action: raw.request.action,
        }),
        catch: () => invalidRequest("invalid control intent dispatch request"),
      })

      const existing = yield* Effect.tryPromise({
        try: () => input.sql`
          SELECT intent_id, idempotency_key, run_ref, action, surface, status,
                 applied_at, rejection_reason, result_lifecycle_state, created_at
          FROM desktop_full_auto_run_control_intents
          WHERE owner_user_id = ${ownerUserId}
            AND (intent_id = ${request.intentId} OR idempotency_key = ${request.idempotencyKey})
          LIMIT 1
        ` as Promise<ReadonlyArray<StoredRow>>,
        catch: storageUnavailable,
      })
      const existingRow = existing[0]
      if (existingRow !== undefined) return rowToIntent(existingRow)

      const createdAt = now().toISOString()
      yield* Effect.tryPromise({
        try: () => input.sql`
          INSERT INTO desktop_full_auto_run_control_intents (
            intent_id, owner_user_id, idempotency_key, run_ref, action, surface,
            status, created_at
          ) VALUES (
            ${request.intentId}, ${ownerUserId}, ${request.idempotencyKey}, ${request.runRef},
            ${request.action}, 'mobile', 'pending', ${createdAt}
          )
        `,
        catch: storageUnavailable,
      })
      return rowToIntent({
        intent_id: request.intentId,
        idempotency_key: request.idempotencyKey,
        run_ref: request.runRef,
        action: request.action,
        surface: "mobile",
        status: "pending",
        applied_at: null,
        rejection_reason: null,
        result_lifecycle_state: null,
        created_at: createdAt,
      })
    })

  const list: FullAutoRunControlAuthorityRepositoryShape["list"] = raw =>
    Effect.gen(function* () {
      const ownerUserId = yield* Effect.try({
        try: () => S.decodeUnknownSync(OwnerUserId)(raw.ownerUserId),
        catch: () => invalidRequest("invalid ownerUserId"),
      })
      const rows = yield* Effect.tryPromise({
        try: () => input.sql`
          SELECT intent_id, idempotency_key, run_ref, action, surface, status,
                 applied_at, rejection_reason, result_lifecycle_state, created_at
          FROM desktop_full_auto_run_control_intents
          WHERE owner_user_id = ${ownerUserId}
          ORDER BY created_at ASC
          LIMIT ${FULL_AUTO_RUN_CONTROL_INTENT_LIST_LIMIT}
        ` as Promise<ReadonlyArray<StoredRow>>,
        catch: storageUnavailable,
      })
      return rows.map(rowToIntent)
    })

  const reportOutcome: FullAutoRunControlAuthorityRepositoryShape["reportOutcome"] = raw =>
    Effect.gen(function* () {
      const ownerUserId = yield* Effect.try({
        try: () => S.decodeUnknownSync(OwnerUserId)(raw.ownerUserId),
        catch: () => invalidRequest("invalid ownerUserId"),
      })
      const intentId = yield* Effect.try({
        try: () => S.decodeUnknownSync(PublicRef)(raw.outcome.intentId),
        catch: () => invalidRequest("invalid intentId"),
      })
      const existing = yield* Effect.tryPromise({
        try: () => input.sql`
          SELECT intent_id, idempotency_key, run_ref, action, surface, status,
                 applied_at, rejection_reason, result_lifecycle_state, created_at
          FROM desktop_full_auto_run_control_intents
          WHERE owner_user_id = ${ownerUserId} AND intent_id = ${intentId}
          LIMIT 1
        ` as Promise<ReadonlyArray<StoredRow>>,
        catch: storageUnavailable,
      })
      const existingRow = existing[0]
      if (existingRow === undefined) {
        return yield* new FullAutoRunControlAuthorityError({ kind: "intent_not_found", reason: "no such control intent for this owner" })
      }
      // Idempotent terminal report: an already-terminal intent is returned
      // as-is (Desktop's outcome POST can safely retry after a dropped
      // response without a second write racing the first).
      if (existingRow.status !== "pending") return rowToIntent(existingRow)

      const appliedAt = raw.outcome.status === "applied" ? now().toISOString() : null
      const rejectionReason = raw.outcome.status === "rejected" ? (raw.outcome.rejectionReason ?? "storage_unavailable") : null
      const resultLifecycleState = raw.outcome.resultLifecycleState ?? null
      yield* Effect.tryPromise({
        try: () => input.sql`
          UPDATE desktop_full_auto_run_control_intents
          SET status = ${raw.outcome.status},
              applied_at = ${appliedAt},
              rejection_reason = ${rejectionReason},
              result_lifecycle_state = ${resultLifecycleState}
          WHERE owner_user_id = ${ownerUserId} AND intent_id = ${intentId}
        `,
        catch: storageUnavailable,
      })
      return rowToIntent({
        ...existingRow,
        status: raw.outcome.status,
        applied_at: appliedAt,
        rejection_reason: rejectionReason,
        result_lifecycle_state: resultLifecycleState,
      })
    })

  return { dispatch, list, reportOutcome }
}
