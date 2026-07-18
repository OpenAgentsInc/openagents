import { Effect } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  FullAutoRunControlAuthorityError,
  makeFullAutoRunControlAuthority,
} from "./full-auto-run-control-authority.js"
import type { SyncSql } from "./sql.js"

const timestamp = "2026-07-18T02:00:00.000Z"

/** A minimal in-memory fake of the tagged-template `SyncSql` handle keyed by
 * `intent_id`, mirroring `full-auto-run-projection-authority.test.ts`'s
 * fake-SQL discipline so this module's SQL-shaping/decode paths are
 * exercised without a live Postgres connection. */
const makeFakeSql = (): SyncSql => {
  const rows = new Map<string, Record<string, unknown> & { owner_user_id: string }>()
  const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const text = strings.join("?")
    if (text.includes("INSERT INTO")) {
      // `surface` ('mobile') and `status` ('pending') are literal SQL text
      // in the real query, not placeholders -- only 6 values are bound.
      const [intentId, ownerUserId, idempotencyKey, runRef, action, createdAt] = values
      rows.set(String(intentId), {
        intent_id: intentId as string, owner_user_id: ownerUserId as string,
        idempotency_key: idempotencyKey, run_ref: runRef, action, surface: "mobile",
        status: "pending", applied_at: null, rejection_reason: null, result_lifecycle_state: null,
        created_at: createdAt,
      })
      return []
    }
    if (text.includes("UPDATE")) {
      const [status, appliedAt, rejectionReason, resultLifecycleState, ownerUserId, intentId] = values
      const row = rows.get(String(intentId))
      if (row !== undefined && row.owner_user_id === ownerUserId) {
        Object.assign(row, {
          status, applied_at: appliedAt, rejection_reason: rejectionReason,
          result_lifecycle_state: resultLifecycleState,
        })
      }
      return []
    }
    if (text.includes("SELECT")) {
      if (text.includes(" OR ")) {
        // dispatch's existing-row lookup: WHERE owner_user_id = ? AND
        // (intent_id = ? OR idempotency_key = ?).
        const [ownerUserId, intentId, idempotencyKey] = values
        const row = [...rows.values()].find(candidate =>
          candidate.owner_user_id === ownerUserId
          && (candidate.intent_id === intentId || candidate.idempotency_key === idempotencyKey))
        return row === undefined ? [] : [row]
      }
      if (text.includes("AND intent_id")) {
        // reportOutcome's single-intent lookup: WHERE owner_user_id = ? AND
        // intent_id = ?. (Distinct from the plain owner-scoped `list` query
        // below, whose SELECT column list also happens to mention
        // "intent_id" as a column name.)
        const [ownerUserId, intentId] = values
        const row = rows.get(String(intentId))
        return row !== undefined && row.owner_user_id === ownerUserId ? [row] : []
      }
      // list: WHERE owner_user_id = ? ORDER BY created_at ASC.
      const [ownerUserId] = values
      return [...rows.values()].filter(row => row.owner_user_id === ownerUserId)
        .sort((a, b) => String(a.created_at).localeCompare(String(b.created_at)))
    }
    throw new Error(`unexpected query: ${text}`)
  }) as unknown as SyncSql
  return Object.assign(sql, {
    begin: async <A>(fn: (tx: SyncSql) => Promise<A>) => fn(sql),
  }) as SyncSql
}

describe("makeFullAutoRunControlAuthority", () => {
  test("dispatch records a pending intent", async () => {
    const authority = makeFullAutoRunControlAuthority({ sql: makeFakeSql(), now: () => new Date(timestamp) })
    const intent = await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "pause" },
    }))
    expect(intent).toMatchObject({ status: "pending", action: "pause", runRef: "run.full-auto.abc" })
  })

  test("dispatch is idempotent by idempotencyKey: a retried POST returns the same stored row, never a duplicate", async () => {
    const authority = makeFullAutoRunControlAuthority({ sql: makeFakeSql(), now: () => new Date(timestamp) })
    const first = await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "pause" },
    }))
    const retried = await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      // Same idempotencyKey, a different client-generated intentId (a naive
      // retry might mint a new one) -- must still resolve to the SAME row.
      request: { intentId: "intent.mobile.2", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "pause" },
    }))
    expect(retried).toEqual(first)
    const listed = await Effect.runPromise(authority.list({ ownerUserId: "owner-a" }))
    expect(listed).toHaveLength(1)
  })

  test("list is owner-scoped: owner B never sees owner A's intents", async () => {
    const sql = makeFakeSql()
    const authority = makeFullAutoRunControlAuthority({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "stop" },
    }))
    const listedByB = await Effect.runPromise(authority.list({ ownerUserId: "owner-b" }))
    expect(listedByB).toHaveLength(0)
    const listedByA = await Effect.runPromise(authority.list({ ownerUserId: "owner-a" }))
    expect(listedByA).toHaveLength(1)
  })

  test("reportOutcome transitions a pending intent to applied with a resultLifecycleState", async () => {
    const sql = makeFakeSql()
    const authority = makeFullAutoRunControlAuthority({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "resume" },
    }))
    const outcome = await Effect.runPromise(authority.reportOutcome({
      ownerUserId: "owner-a",
      outcome: { intentId: "intent.mobile.1", status: "applied", resultLifecycleState: "running" },
    }))
    expect(outcome).toMatchObject({ status: "applied", resultLifecycleState: "running" })
    expect(outcome.appliedAt).not.toBeNull()
  })

  test("reportOutcome transitions a pending intent to rejected with a typed reason", async () => {
    const sql = makeFakeSql()
    const authority = makeFullAutoRunControlAuthority({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "resume" },
    }))
    const outcome = await Effect.runPromise(authority.reportOutcome({
      ownerUserId: "owner-a",
      outcome: { intentId: "intent.mobile.1", status: "rejected", rejectionReason: "illegal_transition" },
    }))
    expect(outcome).toMatchObject({ status: "rejected", rejectionReason: "illegal_transition" })
  })

  test("reportOutcome never silently no-ops on a wrong owner: it fails with intent_not_found", async () => {
    const sql = makeFakeSql()
    const authority = makeFullAutoRunControlAuthority({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "resume" },
    }))
    const result = await Effect.runPromise(
      authority.reportOutcome({
        ownerUserId: "owner-b",
        outcome: { intentId: "intent.mobile.1", status: "applied" },
      }).pipe(Effect.match({
        onFailure: error => ({ kind: "failure" as const, error }),
        onSuccess: value => ({ kind: "success" as const, value }),
      })),
    )
    expect(result.kind).toBe("failure")
    if (result.kind === "failure") {
      expect(result.error).toBeInstanceOf(FullAutoRunControlAuthorityError)
      expect(result.error.kind).toBe("intent_not_found")
    }
  })

  test("reportOutcome is idempotent for an already-terminal intent: a second report never re-writes it", async () => {
    const sql = makeFakeSql()
    const authority = makeFullAutoRunControlAuthority({ sql, now: () => new Date(timestamp) })
    await Effect.runPromise(authority.dispatch({
      ownerUserId: "owner-a",
      request: { intentId: "intent.mobile.1", idempotencyKey: "idem.mobile.1", runRef: "run.full-auto.abc", action: "resume" },
    }))
    const first = await Effect.runPromise(authority.reportOutcome({
      ownerUserId: "owner-a",
      outcome: { intentId: "intent.mobile.1", status: "applied", resultLifecycleState: "running" },
    }))
    const second = await Effect.runPromise(authority.reportOutcome({
      ownerUserId: "owner-a",
      outcome: { intentId: "intent.mobile.1", status: "rejected", rejectionReason: "run_not_found" },
    }))
    expect(second).toEqual(first)
  })

  test("wraps a thrown SQL failure as storage_unavailable", async () => {
    const failingSql = (async () => {
      throw new Error("connection refused")
    }) as unknown as SyncSql
    const authority = makeFullAutoRunControlAuthority({
      sql: Object.assign(failingSql, { begin: async <A>(fn: (tx: SyncSql) => Promise<A>) => fn(failingSql) }) as SyncSql,
      now: () => new Date(timestamp),
    })
    const outcome = await Effect.runPromise(
      authority.list({ ownerUserId: "owner-a" }).pipe(
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
