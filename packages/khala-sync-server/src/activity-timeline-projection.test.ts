import { publicScope, ACTIVITY_TIMELINE_SNAPSHOT_CHANNEL_ID } from "@openagentsinc/khala-sync"
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
  activityTimelinePublicScope,
  projectActivityTimelineSnapshotBestEffort,
  readActivityTimelineSnapshot,
} from "./activity-timeline-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const nowIso = "2026-07-05T00:00:00.000Z"

const sampleEvent = (overrides: Partial<Record<string, unknown>> = {}) => ({
  blockerRefs: [],
  caveatRefs: [],
  cursor: `${nowIso}:forum:event.public.forum_topic.t1`,
  eventRef: "event.public.forum_topic.t1",
  kind: "forum_topic_created",
  refs: ["topic:t1"],
  sourceKind: "forum",
  sourceRefs: ["topic:t1", "route:/api/forum"],
  text: "Public Forum topic created.",
  ts: nowIso,
  ...overrides,
})

const sourceLagEntry = (overrides: Partial<Record<string, unknown>> = {}) => ({
  blockerRefs: [],
  caveatRefs: [],
  lagSeconds: 0,
  latestSourceEventAt: nowIso,
  maxStalenessSeconds: 600,
  observedAt: nowIso,
  sourceKind: "forum",
  sourceRefs: ["route:/api/forum"],
  status: "current",
  ...overrides,
})

const snapshot = (overrides: Partial<Record<string, unknown>> = {}) => ({
  events: [sampleEvent()],
  generatedAt: nowIso,
  sourceLag: [sourceLagEntry()],
  ...overrides,
})

// ---------------------------------------------------------------------------
// Scope helper + fail-soft wrappers (no working database)
// ---------------------------------------------------------------------------

describe("activityTimelinePublicScope", () => {
  test("is scope.public.activity-timeline", () => {
    expect(String(activityTimelinePublicScope())).toBe(
      String(publicScope(ACTIVITY_TIMELINE_SNAPSHOT_CHANNEL_ID)),
    )
    expect(String(activityTimelinePublicScope())).toBe(
      "scope.public.activity-timeline",
    )
  })
})

describe("activity-timeline snapshot projection fail-soft", () => {
  const broken = {
    begin: async () => {
      throw new Error("connection refused: postgres://user:secret@10.0.0.1")
    },
  } as unknown as SyncSql

  test("a broken SQL handle yields a diagnostic, never a throw", async () => {
    const outcome = await projectActivityTimelineSnapshotBestEffort(
      broken,
      snapshot(),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("invalid input yields invalid_input without touching storage", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectActivityTimelineSnapshotBestEffort(
      neverCalled,
      snapshot({ events: [sampleEvent({ kind: "made_up_kind" })] }),
    )
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
  "activity-timeline snapshot projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_activity_timeline")
      await admin.end()
      const url = pg.urlFor("khala_sync_activity_timeline")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("writes then reads back the EXACT post-image (parity)", async () => {
      const outcome = await projectActivityTimelineSnapshotBestEffort(
        s(),
        snapshot(),
      )
      expect(outcome.ok).toBe(true)

      const read = await readActivityTimelineSnapshot(s())
      expect(read).not.toBeNull()
      expect(read?.events).toHaveLength(1)
      expect(read?.events[0]?.eventRef).toBe("event.public.forum_topic.t1")
      expect(read?.sourceLag[0]?.sourceKind).toBe("forum")
    })

    test("re-projecting is idempotent at the read layer (latest upsert wins)", async () => {
      const first = await projectActivityTimelineSnapshotBestEffort(
        s(),
        snapshot({ events: [sampleEvent({ eventRef: "event.a" })] }),
      )
      expect(first.ok).toBe(true)
      const second = await projectActivityTimelineSnapshotBestEffort(
        s(),
        snapshot({
          events: [
            sampleEvent({ eventRef: "event.a" }),
            sampleEvent({
              cursor: `${nowIso}:forum:event.b`,
              eventRef: "event.b",
            }),
          ],
        }),
      )
      expect(second.ok).toBe(true)

      const read = await readActivityTimelineSnapshot(s())
      // The LATEST refresh wins — a stale in-flight refresh never regresses
      // the served snapshot back to fewer events.
      expect(read?.events).toHaveLength(2)
    })

    test("a fresh scope with no snapshot yet reads as null (fail-open signal)", async () => {
      const url2 = pg.urlFor("khala_sync_activity_timeline_empty")
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_activity_timeline_empty")
      await admin.end()
      await runMigrations({ databaseUrl: url2 })
      const emptySql = new SQL({ url: url2, max: 5 })
      try {
        const read = await readActivityTimelineSnapshot(
          emptySql as unknown as SyncSql,
        )
        expect(read).toBeNull()
      } finally {
        await emptySql.end()
      }
    })
  },
)
