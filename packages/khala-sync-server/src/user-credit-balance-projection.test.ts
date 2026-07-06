import { personalScope } from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  applyUserCreditBalanceDelta,
  applyUserCreditBalanceDeltaBestEffort,
  readUserCreditBalance,
  repairUserCreditBalance,
  UserCreditBalanceNotInitializedError,
  USER_CREDIT_BALANCE_PROJECTION_SYSTEM_REF,
  USER_CREDIT_BALANCE_REPAIR_SYSTEM_REF,
} from "./user-credit-balance-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fail-soft wrapper (no working database: must return a diagnostic)
// ---------------------------------------------------------------------------

describe("applyUserCreditBalanceDeltaBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await applyUserCreditBalanceDeltaBestEffort(broken, {
      deltaUsdCents: 1000,
      idempotencyKey: "evt-broken-1",
      observedAt: "2026-07-04T15:20:11.412Z",
      userId: "user-1",
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      // Never echoes connection strings / raw driver messages.
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("invalid input (zero delta) yields a typed diagnostic without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await applyUserCreditBalanceDeltaBestEffort(neverCalled, {
      deltaUsdCents: 0,
      idempotencyKey: "evt-zero",
      observedAt: "2026-07-04T15:20:11.412Z",
      userId: "user-1",
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("invalid_input")
    }
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "user credit balance projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_user_credit_balances")
      await admin.end()
      const url = pg.urlFor("khala_sync_user_credit_balances")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain(
        "0038_khala_sync_user_credit_balances.sql",
      )
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    const observedAt = "2026-07-04T15:20:11.412Z"
    const laterObservedAt = "2026-07-04T16:00:00.000Z"
    const userId = "user-credit-1"

    test("deltas refuse before the backfill (credit_balance_not_initialized) and roll the guard back", async () => {
      expect(await readUserCreditBalance(s(), "pre-backfill-user")).toBeNull()
      await expect(
        applyUserCreditBalanceDelta(s(), {
          deltaUsdCents: 500,
          idempotencyKey: "evt-pre-1",
          observedAt,
          userId: "pre-backfill-user",
        }),
      ).rejects.toThrow(UserCreditBalanceNotInitializedError)
      // The guard row rolled back with the refused delta: after the
      // backfill, the SAME event applies (it was covered by neither).
      const guardRows: Array<{ idempotency_key: string }> = await sql`
        SELECT idempotency_key FROM khala_sync_user_credit_balance_applied
         WHERE user_id = ${"pre-backfill-user"}
      `
      expect(guardRows.length).toBe(0)
      const bestEffort = await applyUserCreditBalanceDeltaBestEffort(s(), {
        deltaUsdCents: 500,
        idempotencyKey: "evt-pre-1",
        observedAt,
        userId: "pre-backfill-user",
      })
      expect(bestEffort.ok).toBe(false)
      if (!bestEffort.ok) {
        expect(bestEffort.diagnostic.reason).toBe(
          "credit_balance_not_initialized",
        )
      }
    })

    test("backfill repair creates the row, records the audit, appends the post-image", async () => {
      const repaired = await repairUserCreditBalance(s(), {
        auditNote: "first bring-up backfill from exact D1 agent_balances (test)",
        exactBalanceUsdCents: 1_000,
        source: "backfill",
        userId,
      })
      expect(repaired.previousBalanceUsdCents).toBeNull()
      expect(repaired.balance.balanceUsdCents).toBe(1_000)
      expect(String(repaired.entry.scope)).toBe(String(personalScope(userId)))
      expect(String(repaired.entry.entityType)).toBe("credit_balance")
      expect(String(repaired.entry.entityId)).toBe(userId)
      expect(repaired.entry.mutationRef).toBe(
        USER_CREDIT_BALANCE_REPAIR_SYSTEM_REF,
      )

      const audits: Array<{
        previous_balance: string | number | null
        new_balance: string | number
        source: string
        audit_note: string
      }> = await sql`
        SELECT previous_balance, new_balance, source, audit_note
          FROM khala_sync_user_credit_balance_repairs
         WHERE user_id = ${userId}
         ORDER BY id
      `
      expect(audits.length).toBe(1)
      expect(audits[0]?.previous_balance).toBeNull()
      expect(Number(audits[0]?.new_balance)).toBe(1_000)
      expect(audits[0]?.source).toBe("backfill")
      expect(audits[0]?.audit_note).toContain("backfill")
    })

    test("exact-once under replay: the same idempotency key applies once", async () => {
      const first = await applyUserCreditBalanceDelta(s(), {
        deltaUsdCents: 250,
        idempotencyKey: "evt-replay-1",
        observedAt,
        userId,
      })
      expect(first.applied).toBe(true)
      if (first.applied) {
        expect(first.balance.balanceUsdCents).toBe(1_250)
        expect(first.balance.lastEventAt).toBe(observedAt)
        expect(first.entry.mutationRef).toBe(
          USER_CREDIT_BALANCE_PROJECTION_SYSTEM_REF,
        )
        const postImage = JSON.parse(first.entry.postImageJson ?? "{}") as {
          userId?: string
          balanceUsdCents?: number
          lastEventAt?: string
        }
        expect(postImage.userId).toBe(userId)
        expect(postImage.balanceUsdCents).toBe(1_250)
        expect(postImage.lastEventAt).toBe(observedAt)
      }

      // Replay: same source event key — applies NOTHING.
      const replay = await applyUserCreditBalanceDelta(s(), {
        deltaUsdCents: 250,
        idempotencyKey: "evt-replay-1",
        observedAt,
        userId,
      })
      expect(replay.applied).toBe(false)
      const balance = await readUserCreditBalance(s(), userId)
      expect(balance?.balanceUsdCents).toBe(1_250)
    })

    test("a charge (negative delta) decreases the projected balance", async () => {
      const charged = await applyUserCreditBalanceDelta(s(), {
        deltaUsdCents: -300,
        idempotencyKey: "evt-charge-1",
        observedAt: laterObservedAt,
        userId,
      })
      expect(charged.applied).toBe(true)
      if (charged.applied) {
        expect(charged.balance.balanceUsdCents).toBe(950)
      }
    })

    test("changelog entries land in scope.user.<userId> with dense versions", async () => {
      const scope = personalScope(userId)
      const rows: Array<{ version: string | number; entity_type: string }> =
        await sql`
          SELECT version, entity_type FROM khala_sync_changelog
           WHERE scope = ${scope}
           ORDER BY version
        `
      // backfill + evt-replay-1 + evt-charge-1 = 3 entries.
      expect(rows.length).toBe(3)
      expect(rows.map((row) => Number(row.version))).toEqual([1, 2, 3])
      expect(new Set(rows.map((row) => row.entity_type))).toEqual(
        new Set(["credit_balance"]),
      )
    })

    test("a delta that would drive the projection negative is refused, never silently floored", async () => {
      await expect(
        applyUserCreditBalanceDelta(s(), {
          deltaUsdCents: -1_000_000,
          idempotencyKey: "evt-overdraw-1",
          observedAt: laterObservedAt,
          userId,
        }),
      ).rejects.toThrow()
      // The guard row rolled back with the refused delta.
      const guardRows: Array<{ idempotency_key: string }> = await sql`
        SELECT idempotency_key FROM khala_sync_user_credit_balance_applied
         WHERE user_id = ${userId} AND idempotency_key = ${"evt-overdraw-1"}
      `
      expect(guardRows.length).toBe(0)
      const balance = await readUserCreditBalance(s(), userId)
      expect(balance?.balanceUsdCents).toBe(950)
    })

    test("one user's deltas never touch another user's row", async () => {
      const otherUserId = "user-credit-2"
      await repairUserCreditBalance(s(), {
        auditNote: "backfill for second user (test)",
        exactBalanceUsdCents: 5_000,
        source: "backfill",
        userId: otherUserId,
      })
      await applyUserCreditBalanceDelta(s(), {
        deltaUsdCents: -100,
        idempotencyKey: "evt-other-user-1",
        observedAt: laterObservedAt,
        userId: otherUserId,
      })
      expect((await readUserCreditBalance(s(), otherUserId))?.balanceUsdCents).toBe(
        4_900,
      )
      expect((await readUserCreditBalance(s(), userId))?.balanceUsdCents).toBe(950)
    })

    test("reconcile-repair on seeded drift realigns and audits (never silent)", async () => {
      const before = await readUserCreditBalance(s(), userId)
      expect(before).not.toBeNull()
      const exactBalanceUsdCents = (before?.balanceUsdCents ?? 0) + 12_345

      const repaired = await repairUserCreditBalance(s(), {
        auditNote: "reconcile detected drift of 12345 cents (test)",
        exactBalanceUsdCents,
        source: "reconcile_repair",
        userId,
      })
      expect(repaired.previousBalanceUsdCents).toBe(before?.balanceUsdCents ?? -1)
      expect(repaired.balance.balanceUsdCents).toBe(exactBalanceUsdCents)

      const audits: Array<{ source: string }> = await sql`
        SELECT source FROM khala_sync_user_credit_balance_repairs
         WHERE user_id = ${userId}
         ORDER BY id
      `
      expect(audits.map((row) => row.source)).toEqual([
        "backfill",
        "reconcile_repair",
      ])

      const after = await readUserCreditBalance(s(), userId)
      expect(after?.balanceUsdCents).toBe(exactBalanceUsdCents)
    })

    test("repair refuses an empty audit note", async () => {
      await expect(
        repairUserCreditBalance(s(), {
          auditNote: "   ",
          exactBalanceUsdCents: 1,
          source: "reconcile_repair",
          userId,
        }),
      ).rejects.toThrow(/audit note/)
    })
  },
)
