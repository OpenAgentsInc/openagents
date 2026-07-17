import {
  FullAutoRunClientProjection as FullAutoRunClientProjectionSchema,
  FullAutoRunClientRunProjection as FullAutoRunClientRunProjectionSchema,
  type FullAutoRunClientProjection,
  type FullAutoRunClientRunProjection,
} from "@openagentsinc/khala-sync"
import { Effect, Schema as S } from "effect"

import type { SyncSql } from "./sql.js"

/**
 * FA-RUN-05 (#8981): server-side storage for the FullAutoRun mobile
 * projection. Deliberately NOT modeled on `fleet-run-authority.ts`'s
 * append-only, Sync-changelog-integrated authority ledger -- this is a
 * simple "last write wins" LIVE PROJECTION, one row per owner, queried
 * directly by the authenticated route (`full-auto-run-routes.ts`) the same
 * way `/api/fleet-runs` queries `fleet-run-authority.ts`'s repository, minus
 * the idempotency/event-log machinery that domain doesn't need here.
 *
 * Isolation: every query is `WHERE owner_user_id = $1` against the
 * SERVER-authenticated caller's own id (never a client-supplied field), and
 * the primary key IS `owner_user_id` -- there is no `run_ref`-keyed lookup
 * path that could return another owner's row even by accident.
 */

const OwnerUserId = S.Trim.check(S.isMinLength(1), S.isMaxLength(160))

export class FullAutoRunProjectionAuthorityError extends S.TaggedErrorClass<FullAutoRunProjectionAuthorityError>()(
  "FullAutoRunProjectionAuthorityError",
  {
    kind: S.Literals(["invalid_request", "storage_unavailable"]),
    reason: S.String,
  },
) {}

export type FullAutoRunProjectionPublishInput = Readonly<{
  ownerUserId: string
  /** `null` clears the owner's projection (no active run to report). */
  run: FullAutoRunClientRunProjection | null
}>

export type FullAutoRunProjectionObserveInput = Readonly<{
  ownerUserId: string
}>

export type FullAutoRunProjectionResult = Readonly<{
  projection: FullAutoRunClientProjection
}>

export type FullAutoRunProjectionAuthorityRepositoryShape = Readonly<{
  publish: (
    input: FullAutoRunProjectionPublishInput,
  ) => Effect.Effect<FullAutoRunProjectionResult, FullAutoRunProjectionAuthorityError>
  observe: (
    input: FullAutoRunProjectionObserveInput,
  ) => Effect.Effect<FullAutoRunProjectionResult, FullAutoRunProjectionAuthorityError>
}>

type StoredRow = Readonly<{
  run_ref: string
  thread_ref: string | null
  objective: string
  done_condition: string
  lifecycle_state: string
  workspace_label: string | null
  started_at: string | null
  updated_at: string
  last_transition_actor: string
  last_transition_at: string
}>

const rowToRunProjection = (row: StoredRow): FullAutoRunClientRunProjection =>
  S.decodeUnknownSync(FullAutoRunClientRunProjectionSchema)({
    runRef: row.run_ref,
    threadRef: row.thread_ref,
    objective: row.objective,
    doneCondition: row.done_condition,
    lifecycleState: row.lifecycle_state,
    workspaceLabel: row.workspace_label,
    startedAt: row.started_at,
    updatedAt: row.updated_at,
    lastTransition: { actor: row.last_transition_actor, at: row.last_transition_at },
  })

const envelope = (
  run: FullAutoRunClientRunProjection | null,
  generatedAt: string,
): FullAutoRunClientProjection =>
  S.decodeUnknownSync(FullAutoRunClientProjectionSchema)({
    schema: "full_auto_run.mobile_projection.v1",
    privateMaterialExcluded: true,
    generatedAt,
    run,
  })

const invalidRequest = (reason: string): FullAutoRunProjectionAuthorityError =>
  new FullAutoRunProjectionAuthorityError({ kind: "invalid_request", reason })

const storageUnavailable = (cause: unknown): FullAutoRunProjectionAuthorityError =>
  new FullAutoRunProjectionAuthorityError({
    kind: "storage_unavailable",
    reason: cause instanceof Error ? cause.message : "full auto run projection storage unavailable",
  })

export const makeFullAutoRunProjectionRepository = (
  input: Readonly<{ sql: SyncSql; now?: () => Date }>,
): FullAutoRunProjectionAuthorityRepositoryShape => {
  const now = input.now ?? (() => new Date())

  const publish: FullAutoRunProjectionAuthorityRepositoryShape["publish"] = raw =>
    Effect.gen(function* () {
      const ownerUserId = yield* Effect.try({
        try: () => S.decodeUnknownSync(OwnerUserId)(raw.ownerUserId),
        catch: () => invalidRequest("invalid ownerUserId"),
      })
      const publishedAt = now().toISOString()

      if (raw.run === null) {
        yield* Effect.tryPromise({
          try: () => input.sql`
            DELETE FROM desktop_full_auto_run_projections
            WHERE owner_user_id = ${ownerUserId}
          `,
          catch: storageUnavailable,
        })
        return { projection: envelope(null, publishedAt) }
      }

      const run = yield* Effect.try({
        try: () => S.decodeUnknownSync(FullAutoRunClientRunProjectionSchema)(raw.run, { onExcessProperty: "error" }),
        catch: () => invalidRequest("invalid run projection"),
      })

      yield* Effect.tryPromise({
        try: () => input.sql`
          INSERT INTO desktop_full_auto_run_projections (
            owner_user_id, run_ref, thread_ref, objective, done_condition,
            lifecycle_state, workspace_label, started_at, updated_at,
            last_transition_actor, last_transition_at, published_at
          ) VALUES (
            ${ownerUserId}, ${run.runRef}, ${run.threadRef}, ${run.objective}, ${run.doneCondition},
            ${run.lifecycleState}, ${run.workspaceLabel}, ${run.startedAt}, ${run.updatedAt},
            ${run.lastTransition.actor}, ${run.lastTransition.at}, ${publishedAt}
          )
          ON CONFLICT (owner_user_id) DO UPDATE SET
            run_ref = EXCLUDED.run_ref,
            thread_ref = EXCLUDED.thread_ref,
            objective = EXCLUDED.objective,
            done_condition = EXCLUDED.done_condition,
            lifecycle_state = EXCLUDED.lifecycle_state,
            workspace_label = EXCLUDED.workspace_label,
            started_at = EXCLUDED.started_at,
            updated_at = EXCLUDED.updated_at,
            last_transition_actor = EXCLUDED.last_transition_actor,
            last_transition_at = EXCLUDED.last_transition_at,
            published_at = EXCLUDED.published_at
        `,
        catch: storageUnavailable,
      })
      return { projection: envelope(run, publishedAt) }
    })

  const observe: FullAutoRunProjectionAuthorityRepositoryShape["observe"] = raw =>
    Effect.gen(function* () {
      const ownerUserId = yield* Effect.try({
        try: () => S.decodeUnknownSync(OwnerUserId)(raw.ownerUserId),
        catch: () => invalidRequest("invalid ownerUserId"),
      })
      const rows = yield* Effect.tryPromise({
        try: () => input.sql`
          SELECT run_ref, thread_ref, objective, done_condition, lifecycle_state,
                 workspace_label, started_at, updated_at, last_transition_actor, last_transition_at
          FROM desktop_full_auto_run_projections
          WHERE owner_user_id = ${ownerUserId}
          LIMIT 1
        ` as Promise<ReadonlyArray<StoredRow>>,
        catch: storageUnavailable,
      })
      const row = rows[0]
      const run = row === undefined ? null : rowToRunProjection(row)
      return { projection: envelope(run, now().toISOString()) }
    })

  return { publish, observe }
}
