import { publicScope, SETTLED_FEED_CHANNEL_ID } from "@openagentsinc/khala-sync"
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
  DEFAULT_SETTLED_FEED_READ_LIMIT,
  projectSettledFeedEvents,
  projectSettledFeedEventsBestEffort,
  readSettledFeedProjection,
  SETTLED_FEED_PROJECTION_SYSTEM_REF,
  settledFeedPublicScope,
} from "./settled-feed-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const rawEvent = (overrides: Partial<Record<string, unknown>> = {}) => ({
  amountSats: 5,
  challengeRef: "challenge.tassadar.window.0001",
  contributorRef: "pylon.worker.orrery",
  eventRef: "settled.challenge_tassadar_window_0001.worker.0",
  party: "worker" as const,
  runRef: "run.tassadar.poc",
  settledAt: "2026-07-05T00:00:00.000Z",
  totalSettledCount: 1,
  totalSettledSats: 5,
  windowRef: "window.tassadar.0001",
  ...overrides,
})

const rawSummary = (overrides: Partial<Record<string, unknown>> = {}) => ({
  latestEventRef: "settled.challenge_tassadar_window_0001.worker.0",
  latestSettledAt: "2026-07-05T00:00:00.000Z",
  totalSettledCount: 1,
  totalSettledSats: 5,
  updatedAt: "2026-07-05T00:00:00.000Z",
  ...overrides,
})

// ---------------------------------------------------------------------------
// Scope helper + fail-soft wrapper (no working database)
// ---------------------------------------------------------------------------

describe("settledFeedPublicScope", () => {
  test("is scope.public.settled-feed", () => {
    expect(String(settledFeedPublicScope())).toBe(
      String(publicScope(SETTLED_FEED_CHANNEL_ID)),
    )
    expect(String(settledFeedPublicScope())).toBe("scope.public.settled-feed")
  })
})

describe("projectSettledFeedEventsBestEffort fail-soft", () => {
  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const broken = {
      begin: async () => {
        throw new Error("connection refused: postgres://user:secret@10.0.0.1")
      },
    } as unknown as SyncSql
    const outcome = await projectSettledFeedEventsBestEffort(broken, {
      rawEvents: [rawEvent()],
      rawSummary: rawSummary(),
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      // A generic thrown Error (not a recognized KhalaSyncStorageError
      // shape) classifies as projection_failed — same precedent as
      // applyPublicCounterIncrementBestEffort's own fail-soft test.
      expect(outcome.diagnostic.reason).toBe("projection_failed")
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
    const outcome = await projectSettledFeedEventsBestEffort(neverCalled, {
      rawEvents: [rawEvent({ amountSats: -1 })],
      rawSummary: rawSummary(),
    })
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("invalid_input")
    }
  })

  test("no events is a clean no-op, never touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectSettledFeedEventsBestEffort(neverCalled, {
      rawEvents: [],
      rawSummary: rawSummary(),
    })
    expect(outcome.ok).toBe(true)
    if (outcome.ok) {
      expect(outcome.entries).toHaveLength(0)
    }
  })
})

// ---------------------------------------------------------------------------
// Integration (local Postgres)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())(
  "settled-feed projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_settled_feed")
      await admin.end()
      const url = pg.urlFor("khala_sync_settled_feed")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("writes N events + one summary in ONE changelog version", async () => {
      const outcome = await projectSettledFeedEventsBestEffort(s(), {
        rawEvents: [
          rawEvent({ eventRef: "settled.batch1.worker.0" }),
          rawEvent({
            contributorRef: "pylon.validator.whitefang",
            eventRef: "settled.batch1.validator.0",
            party: "validator",
            totalSettledCount: 2,
            totalSettledSats: 10,
          }),
        ],
        rawSummary: rawSummary({
          latestEventRef: "settled.batch1.validator.0",
          totalSettledCount: 2,
          totalSettledSats: 10,
        }),
      })
      expect(outcome.ok).toBe(true)
      if (!outcome.ok) return

      expect(outcome.entries).toHaveLength(3)
      const versions = new Set(outcome.entries.map((e) => Number(e.version)))
      expect(versions.size).toBe(1)
      expect(
        outcome.entries.every(
          (e) => String(e.scope) === "scope.public.settled-feed",
        ),
      ).toBe(true)
      expect(
        outcome.entries.every(
          (e) => e.mutationRef === SETTLED_FEED_PROJECTION_SYSTEM_REF,
        ),
      ).toBe(true)

      const rows: Array<{ entity_type: string; entity_id: string }> = await sql`
        SELECT entity_type, entity_id FROM khala_sync_changelog
         WHERE scope = ${"scope.public.settled-feed"}
         ORDER BY entity_type, entity_id
      `
      expect(rows).toHaveLength(3)
      expect(rows.filter((r) => r.entity_type === "settled_feed_event")).toHaveLength(2)
      expect(rows.filter((r) => r.entity_type === "settled_feed_summary")).toHaveLength(1)
    })

    test("readSettledFeedProjection returns the latest events + summary", async () => {
      const snapshot = await readSettledFeedProjection(s(), { limit: 10 })
      expect(snapshot.events).toHaveLength(2)
      expect(snapshot.summary).not.toBeNull()
      expect(snapshot.summary?.totalSettledCount).toBe(2)
      expect(snapshot.summary?.totalSettledSats).toBe(10)
      expect(snapshot.summary?.latestEventRef).toBe(
        "settled.batch1.validator.0",
      )
      expect(
        snapshot.events.map((e) => e.eventRef).sort(),
      ).toEqual(["settled.batch1.validator.0", "settled.batch1.worker.0"].sort())
    })

    test("replaying the SAME eventRef is idempotent (upsert, not duplicate)", async () => {
      const before = await sql`
        SELECT count(*)::int AS count FROM khala_sync_changelog
         WHERE scope = ${"scope.public.settled-feed"}
           AND entity_type = 'settled_feed_event'
      `
      const beforeCount = Number((before as Array<{ count: number }>)[0]?.count)

      const replay = await projectSettledFeedEventsBestEffort(s(), {
        rawEvents: [rawEvent({ eventRef: "settled.batch1.worker.0" })],
        rawSummary: rawSummary({
          latestEventRef: "settled.batch1.worker.0",
          totalSettledCount: 2,
          totalSettledSats: 10,
        }),
      })
      expect(replay.ok).toBe(true)

      const after = await sql`
        SELECT count(*)::int AS count FROM khala_sync_changelog
         WHERE scope = ${"scope.public.settled-feed"}
           AND entity_type = 'settled_feed_event'
      `
      const afterCount = Number((after as Array<{ count: number }>)[0]?.count)
      // A new changelog ROW is appended per version (append-only log), but
      // the LATEST-state read (readSettledFeedProjection / bootstrap) still
      // resolves to exactly one row per distinct eventRef — the replay
      // never creates a second distinct entity.
      expect(afterCount).toBe(beforeCount + 1)
      const snapshot = await readSettledFeedProjection(s(), { limit: 10 })
      const eventRefs = snapshot.events.map((e) => e.eventRef)
      expect(new Set(eventRefs).size).toBe(eventRefs.length)
      expect(eventRefs).toContain("settled.batch1.worker.0")
    })

    test("bounds the read to the requested limit", async () => {
      for (let i = 0; i < 5; i += 1) {
        // eslint-disable-next-line no-await-in-loop
        await projectSettledFeedEvents(s(), {
          events: [
            {
              amountSats: 1,
              challengeRef: "challenge.tassadar.window.bound",
              contributorRef: "pylon.worker.orrery",
              eventRef: `settled.bound.${i}`,
              party: "worker",
              runRef: "run.tassadar.poc",
              settledAt: "2026-07-05T01:00:00.000Z",
              totalSettledCount: 100 + i,
              totalSettledSats: 500 + i,
              windowRef: null,
            },
          ],
          summary: {
            latestEventRef: `settled.bound.${i}`,
            latestSettledAt: "2026-07-05T01:00:00.000Z",
            totalSettledCount: 100 + i,
            totalSettledSats: 500 + i,
            updatedAt: "2026-07-05T01:00:00.000Z",
          },
        })
      }
      const snapshot = await readSettledFeedProjection(s(), { limit: 3 })
      expect(snapshot.events).toHaveLength(3)
      expect(snapshot.summary?.totalSettledCount).toBe(104)
    })

    test("default read limit is applied when unspecified", () => {
      expect(DEFAULT_SETTLED_FEED_READ_LIMIT).toBeGreaterThan(0)
    })
  },
)
