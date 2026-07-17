import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  FullAutoRunProjectionAuthorityError,
  makeFullAutoRunProjectionRepository,
} from "./full-auto-run-projection-authority.js"
import type { SyncSql } from "./sql.js"

const timestamp = "2026-07-17T21:00:00.000Z"
const run = {
  runRef: "run.full-auto.abc123.def456",
  threadRef: "thread.abc123",
  objective: "Ship the mobile live-run projection.",
  doneCondition: "The new endpoint round-trips a projection end to end.",
  lifecycleState: "running" as const,
  workspaceLabel: "openagents",
  startedAt: timestamp,
  updatedAt: timestamp,
  lastTransition: { actor: "control_api" as const, at: timestamp },
}

/** A minimal in-memory fake of the tagged-template `SyncSql` handle, keyed
 * by owner (mirroring the real table's `owner_user_id` primary key), so this
 * module's SQL-shaping and decode paths are exercised without a live
 * Postgres connection. */
const makeFakeSql = (): SyncSql => {
  const rows = new Map<string, Record<string, unknown>>()
  const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const text = strings.join("?")
    if (text.includes("DELETE FROM")) {
      const [ownerUserId] = values
      rows.delete(String(ownerUserId))
      return []
    }
    if (text.includes("INSERT INTO")) {
      const [
        ownerUserId, runRef, threadRef, objective, doneCondition,
        lifecycleState, workspaceLabel, startedAt, updatedAt,
        lastTransitionActor, lastTransitionAt,
      ] = values
      rows.set(String(ownerUserId), {
        run_ref: runRef, thread_ref: threadRef, objective, done_condition: doneCondition,
        lifecycle_state: lifecycleState, workspace_label: workspaceLabel, started_at: startedAt,
        updated_at: updatedAt, last_transition_actor: lastTransitionActor, last_transition_at: lastTransitionAt,
      })
      return []
    }
    if (text.includes("SELECT")) {
      const [ownerUserId] = values
      const row = rows.get(String(ownerUserId))
      return row === undefined ? [] : [row]
    }
    throw new Error(`unexpected query: ${text}`)
  }) as unknown as SyncSql
  return Object.assign(sql, {
    begin: async <A>(fn: (tx: SyncSql) => Promise<A>) => fn(sql),
  }) as SyncSql
}

describe("makeFullAutoRunProjectionRepository", () => {
  test("publish then observe round-trips the run for its owner", async () => {
    const repository = makeFullAutoRunProjectionRepository({ sql: makeFakeSql(), now: () => new Date(timestamp) })
    await Effect.runPromise(repository.publish({ ownerUserId: "owner-a", run }))
    const result = await Effect.runPromise(repository.observe({ ownerUserId: "owner-a" }))
    expect(result.projection.run).toMatchObject({ runRef: run.runRef, lifecycleState: "running" })
  })

  test("observe returns a null run for an owner who never published", async () => {
    const repository = makeFullAutoRunProjectionRepository({ sql: makeFakeSql(), now: () => new Date(timestamp) })
    const result = await Effect.runPromise(repository.observe({ ownerUserId: "owner-never-published" }))
    expect(result.projection.run).toBeNull()
  })

  test("cross-owner isolation: publishing for owner A never leaks to owner B's observe", async () => {
    const sql = makeFakeSql()
    const repository = makeFullAutoRunProjectionRepository({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(repository.publish({ ownerUserId: "owner-a", run }))
    const observedByOwnerB = await Effect.runPromise(repository.observe({ ownerUserId: "owner-b" }))
    expect(observedByOwnerB.projection.run).toBeNull()
    const observedByOwnerA = await Effect.runPromise(repository.observe({ ownerUserId: "owner-a" }))
    expect(observedByOwnerA.projection.run?.runRef).toBe(run.runRef)
  })

  test("publishing null clears a previously published run", async () => {
    const sql = makeFakeSql()
    const repository = makeFullAutoRunProjectionRepository({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(repository.publish({ ownerUserId: "owner-a", run }))
    await Effect.runPromise(repository.publish({ ownerUserId: "owner-a", run: null }))
    const result = await Effect.runPromise(repository.observe({ ownerUserId: "owner-a" }))
    expect(result.projection.run).toBeNull()
  })

  test("rejects an invalid run projection with invalid_request", async () => {
    const repository = makeFullAutoRunProjectionRepository({ sql: makeFakeSql(), now: () => new Date(timestamp) })
    const outcome = await Effect.runPromise(
      repository.publish({ ownerUserId: "owner-a", run: { ...run, lifecycleState: "bogus" as never } }).pipe(
        Effect.match({
          onFailure: error => ({ kind: "failure" as const, error }),
          onSuccess: value => ({ kind: "success" as const, value }),
        }),
      ),
    )
    expect(outcome.kind).toBe("failure")
    if (outcome.kind === "failure") {
      expect(outcome.error).toBeInstanceOf(FullAutoRunProjectionAuthorityError)
      expect(outcome.error.kind).toBe("invalid_request")
    }
  })

  test("wraps a thrown SQL failure as storage_unavailable", async () => {
    const failingSql = (async () => {
      throw new Error("connection refused")
    }) as unknown as SyncSql
    const repository = makeFullAutoRunProjectionRepository({
      sql: Object.assign(failingSql, { begin: async <A>(fn: (tx: SyncSql) => Promise<A>) => fn(failingSql) }) as SyncSql,
      now: () => new Date(timestamp),
    })
    const outcome = await Effect.runPromise(
      repository.observe({ ownerUserId: "owner-a" }).pipe(
        Effect.match({
          onFailure: error => ({ kind: "failure" as const, error }),
          onSuccess: value => ({ kind: "success" as const, value }),
        }),
      ),
    )
    expect(outcome.kind).toBe("failure")
    if (outcome.kind === "failure") expect(outcome.error.kind).toBe("storage_unavailable")
  })
})
