import { publicScope, TOKENS_SERVED_COUNTER_ID } from "@openagentsinc/khala-sync"
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
  applyPublicCounterIncrement,
  applyPublicCounterIncrementBestEffort,
  PUBLIC_COUNTER_PROJECTION_SYSTEM_REF,
  PUBLIC_COUNTER_REPAIR_SYSTEM_REF,
  PublicCounterNotInitializedError,
  readPublicCounter,
  repairPublicCounter,
} from "./public-counter-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fail-soft wrapper (no working database: must return a diagnostic)
// ---------------------------------------------------------------------------

describe("applyPublicCounterIncrementBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await applyPublicCounterIncrementBestEffort(broken, {
      counterId: TOKENS_SERVED_COUNTER_ID,
      delta: 100,
      idempotencyKey: "evt-broken-1",
      observedAt: "2026-07-04T15:20:11.412Z",
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      // Never echoes connection strings / raw driver messages.
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("invalid input yields a typed diagnostic without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await applyPublicCounterIncrementBestEffort(neverCalled, {
      counterId: TOKENS_SERVED_COUNTER_ID,
      delta: 0,
      idempotencyKey: "evt-zero",
      observedAt: "2026-07-04T15:20:11.412Z",
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
  "public counter projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_public_counters")
      await admin.end()
      const url = pg.urlFor("khala_sync_public_counters")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0006_khala_sync_public_counters.sql")
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

    test("increments refuse before the backfill (counter_not_initialized) and roll the guard back", async () => {
      expect(await readPublicCounter(s(), "pre-backfill")).toBeNull()
      await expect(
        applyPublicCounterIncrement(s(), {
          counterId: "pre-backfill",
          delta: 10,
          idempotencyKey: "evt-pre-1",
          observedAt,
        }),
      ).rejects.toThrow(PublicCounterNotInitializedError)
      // The guard row rolled back with the refused increment: after the
      // backfill, the SAME event applies (it was covered by neither).
      const guardRows: Array<{ idempotency_key: string }> = await sql`
        SELECT idempotency_key FROM khala_sync_counter_applied
         WHERE counter_id = ${"pre-backfill"}
      `
      expect(guardRows.length).toBe(0)
      const bestEffort = await applyPublicCounterIncrementBestEffort(s(), {
        counterId: "pre-backfill",
        delta: 10,
        idempotencyKey: "evt-pre-1",
        observedAt,
      })
      expect(bestEffort.ok).toBe(false)
      if (!bestEffort.ok) {
        expect(bestEffort.diagnostic.reason).toBe("counter_not_initialized")
      }
    })

    test("backfill repair creates the row, records the audit, appends the post-image", async () => {
      const repaired = await repairPublicCounter(s(), {
        auditNote: "first bring-up backfill from exact D1 SUM (test)",
        counterId: TOKENS_SERVED_COUNTER_ID,
        exactTotal: 1_000_000,
        source: "backfill",
      })
      expect(repaired.previousTotal).toBeNull()
      expect(repaired.counter.total).toBe(1_000_000)
      expect(String(repaired.entry.scope)).toBe(
        String(publicScope(TOKENS_SERVED_COUNTER_ID)),
      )
      expect(String(repaired.entry.entityType)).toBe("public_counter")
      expect(String(repaired.entry.entityId)).toBe(TOKENS_SERVED_COUNTER_ID)
      expect(repaired.entry.mutationRef).toBe(PUBLIC_COUNTER_REPAIR_SYSTEM_REF)

      const audits: Array<{
        previous_total: string | number | null
        new_total: string | number
        source: string
        audit_note: string
      }> = await sql`
        SELECT previous_total, new_total, source, audit_note
          FROM khala_sync_public_counter_repairs
         WHERE counter_id = ${TOKENS_SERVED_COUNTER_ID}
         ORDER BY id
      `
      expect(audits.length).toBe(1)
      expect(audits[0]?.previous_total).toBeNull()
      expect(Number(audits[0]?.new_total)).toBe(1_000_000)
      expect(audits[0]?.source).toBe("backfill")
      expect(audits[0]?.audit_note).toContain("backfill")
    })

    test("exact-once under replay: the same idempotency key increments once", async () => {
      const first = await applyPublicCounterIncrement(s(), {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta: 250,
        idempotencyKey: "evt-replay-1",
        observedAt,
      })
      expect(first.applied).toBe(true)
      if (first.applied) {
        expect(first.counter.total).toBe(1_000_250)
        expect(first.counter.lastEventAt).toBe(observedAt)
        expect(first.entry.mutationRef).toBe(
          PUBLIC_COUNTER_PROJECTION_SYSTEM_REF,
        )
        const postImage = JSON.parse(first.entry.postImageJson ?? "{}") as {
          counterId?: string
          total?: number
          lastEventAt?: string
        }
        expect(postImage.counterId).toBe(TOKENS_SERVED_COUNTER_ID)
        expect(postImage.total).toBe(1_000_250)
        expect(postImage.lastEventAt).toBe(observedAt)
      }

      // Replay: same source event key — applies NOTHING.
      const replay = await applyPublicCounterIncrement(s(), {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta: 250,
        idempotencyKey: "evt-replay-1",
        observedAt,
      })
      expect(replay.applied).toBe(false)
      const counter = await readPublicCounter(s(), TOKENS_SERVED_COUNTER_ID)
      expect(counter?.total).toBe(1_000_250)
    })

    test("changelog entries land in scope.public.tokens-served with dense versions", async () => {
      const scope = publicScope(TOKENS_SERVED_COUNTER_ID)
      await applyPublicCounterIncrement(s(), {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta: 5,
        idempotencyKey: "evt-dense-1",
        observedAt: laterObservedAt,
      })
      await applyPublicCounterIncrement(s(), {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta: 7,
        idempotencyKey: "evt-dense-2",
        observedAt: laterObservedAt,
      })
      const rows: Array<{ version: string | number; entity_type: string }> =
        await sql`
          SELECT version, entity_type FROM khala_sync_changelog
           WHERE scope = ${scope}
           ORDER BY version
        `
      // backfill + evt-replay-1 + evt-dense-1 + evt-dense-2 = 4 entries.
      expect(rows.length).toBe(4)
      expect(rows.map((row) => Number(row.version))).toEqual([1, 2, 3, 4])
      expect(new Set(rows.map((row) => row.entity_type))).toEqual(
        new Set(["public_counter"]),
      )
      const counter = await readPublicCounter(s(), TOKENS_SERVED_COUNTER_ID)
      expect(counter?.total).toBe(1_000_262)
      expect(counter?.lastEventAt).toBe(laterObservedAt)
    })

    test("lastEventAt never moves backwards on out-of-order observedAt", async () => {
      const earlier = "2026-07-04T10:00:00.000Z"
      const applied = await applyPublicCounterIncrement(s(), {
        counterId: TOKENS_SERVED_COUNTER_ID,
        delta: 1,
        idempotencyKey: "evt-out-of-order",
        observedAt: earlier,
      })
      expect(applied.applied).toBe(true)
      if (applied.applied) {
        expect(applied.counter.lastEventAt).toBe(laterObservedAt)
      }
    })

    test("reconcile-repair on seeded drift realigns and audits (never silent)", async () => {
      // Seed drift: pretend the exact source SUM says the counter is behind
      // (e.g. an unhooked ingest path or a lost best-effort increment).
      const before = await readPublicCounter(s(), TOKENS_SERVED_COUNTER_ID)
      expect(before).not.toBeNull()
      const exactTotal = (before?.total ?? 0) + 12_345

      const repaired = await repairPublicCounter(s(), {
        auditNote: "reconcile detected drift of 12345 (test)",
        counterId: TOKENS_SERVED_COUNTER_ID,
        exactTotal,
        source: "reconcile_repair",
      })
      expect(repaired.previousTotal).toBe(before?.total ?? -1)
      expect(repaired.counter.total).toBe(exactTotal)

      const audits: Array<{ source: string }> = await sql`
        SELECT source FROM khala_sync_public_counter_repairs
         WHERE counter_id = ${TOKENS_SERVED_COUNTER_ID}
         ORDER BY id
      `
      expect(audits.map((row) => row.source)).toEqual([
        "backfill",
        "reconcile_repair",
      ])

      const after = await readPublicCounter(s(), TOKENS_SERVED_COUNTER_ID)
      expect(after?.total).toBe(exactTotal)
    })

    test("repair refuses an empty audit note", async () => {
      await expect(
        repairPublicCounter(s(), {
          auditNote: "   ",
          counterId: TOKENS_SERVED_COUNTER_ID,
          exactTotal: 1,
          source: "reconcile_repair",
        }),
      ).rejects.toThrow(/audit note/)
    })
  },
)
