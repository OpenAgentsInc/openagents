import { publicScope, TOKENS_SERVED_AGGREGATES_CHANNEL_ID } from "@openagentsinc/khala-sync"
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
  projectTokensServedChannelMixSnapshotBestEffort,
  projectTokensServedDemandMixSnapshotBestEffort,
  projectTokensServedHistorySnapshotBestEffort,
  projectTokensServedModelMixSnapshotBestEffort,
  readTokensServedChannelMixSnapshot,
  readTokensServedDemandMixSnapshot,
  readTokensServedHistorySnapshot,
  readTokensServedModelMixSnapshot,
  tokensServedAggregatesPublicScope,
} from "./tokens-served-mix-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const nowIso = "2026-07-05T00:00:00.000Z"

const modelMixSnapshot = (overrides: Partial<Record<string, unknown>> = {}) => ({
  generatedAt: nowIso,
  groups: [
    { family: "glm", label: "GLM family", pct: 60, reqs: 6, tokens: 600 },
    { family: "other", label: "Other", pct: 40, reqs: 4, tokens: 400 },
  ],
  totalTokens: 1000,
  window: "30d",
  ...overrides,
})

const demandMixSnapshot = (overrides: Partial<Record<string, unknown>> = {}) => ({
  generatedAt: nowIso,
  groups: [
    {
      client: "khala-code",
      kind: "external",
      pct: 100,
      reqs: 10,
      source: "chat",
      tokens: 1000,
    },
  ],
  totalTokens: 1000,
  window: "30d",
  ...overrides,
})

const channelMixSnapshot = (overrides: Partial<Record<string, unknown>> = {}) => ({
  generatedAt: nowIso,
  groups: [
    { channel: "khala_api", label: "Khala API", pct: 100, reqs: 10, tokens: 1000 },
  ],
  totalTokens: 1000,
  window: "30d",
  ...overrides,
})

const historySnapshot = (overrides: Partial<Record<string, unknown>> = {}) => ({
  bucket: "day",
  generatedAt: nowIso,
  series: [{ day: "2026-07-05", tokensServed: 1000 }],
  timezone: "America/Chicago",
  window: "30d",
  ...overrides,
})

// ---------------------------------------------------------------------------
// Scope helper + fail-soft wrappers (no working database)
// ---------------------------------------------------------------------------

describe("tokensServedAggregatesPublicScope", () => {
  test("is scope.public.tokens-served-aggregates", () => {
    expect(String(tokensServedAggregatesPublicScope())).toBe(
      String(publicScope(TOKENS_SERVED_AGGREGATES_CHANNEL_ID)),
    )
    expect(String(tokensServedAggregatesPublicScope())).toBe(
      "scope.public.tokens-served-aggregates",
    )
  })
})

describe("tokens-served mix/history projection fail-soft", () => {
  const broken = {
    begin: async () => {
      throw new Error("connection refused: postgres://user:secret@10.0.0.1")
    },
  } as unknown as SyncSql

  test("a broken SQL handle yields a diagnostic, never a throw (model-mix)", async () => {
    const outcome = await projectTokensServedModelMixSnapshotBestEffort(
      broken,
      modelMixSnapshot(),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("projection_failed")
      expect(outcome.diagnostic.messageSafe).not.toContain("secret")
      expect(outcome.diagnostic.messageSafe).not.toContain("10.0.0.1")
    }
  })

  test("invalid input yields invalid_input without touching storage (demand-mix)", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectTokensServedDemandMixSnapshotBestEffort(
      neverCalled,
      demandMixSnapshot({ groups: [{ kind: "made_up" }] }),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("invalid_input")
    }
  })

  test("invalid input yields invalid_input without touching storage (channel-mix)", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectTokensServedChannelMixSnapshotBestEffort(
      neverCalled,
      channelMixSnapshot({ totalTokens: -1 }),
    )
    expect(outcome.ok).toBe(false)
    if (!outcome.ok) {
      expect(outcome.diagnostic.reason).toBe("invalid_input")
    }
  })

  test("invalid input yields invalid_input without touching storage (history)", async () => {
    const neverCalled = {
      begin: async () => {
        throw new Error("must not be reached")
      },
    } as unknown as SyncSql
    const outcome = await projectTokensServedHistorySnapshotBestEffort(
      neverCalled,
      historySnapshot({ series: [{ day: "not-a-day", tokensServed: 1 }] }),
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
  "tokens-served mix/history projection against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    const s = () => sql as unknown as SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_tokens_served_mix")
      await admin.end()
      const url = pg.urlFor("khala_sync_tokens_served_mix")
      await runMigrations({ databaseUrl: url })
      sql = new SQL({ url, max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("model-mix: writes then reads back the EXACT post-image (parity)", async () => {
      const snapshot = modelMixSnapshot({ window: "7d" })
      const outcome = await projectTokensServedModelMixSnapshotBestEffort(
        s(),
        snapshot,
      )
      expect(outcome.ok).toBe(true)

      const read = await readTokensServedModelMixSnapshot(s(), "7d")
      expect(read).not.toBeNull()
      expect(read?.totalTokens).toBe(1000)
      expect(read?.groups).toHaveLength(2)
      expect(read?.groups.find(g => g.family === "glm")?.tokens).toBe(600)
    })

    test("demand-mix: writes then reads back the EXACT post-image", async () => {
      const snapshot = demandMixSnapshot({ window: "all" })
      const outcome = await projectTokensServedDemandMixSnapshotBestEffort(
        s(),
        snapshot,
      )
      expect(outcome.ok).toBe(true)

      const read = await readTokensServedDemandMixSnapshot(s(), "all")
      expect(read).not.toBeNull()
      expect(read?.groups[0]?.source).toBe("chat")
      expect(read?.groups[0]?.kind).toBe("external")
    })

    test("channel-mix: writes then reads back the EXACT post-image", async () => {
      const snapshot = channelMixSnapshot({ window: "today" })
      const outcome = await projectTokensServedChannelMixSnapshotBestEffort(
        s(),
        snapshot,
      )
      expect(outcome.ok).toBe(true)

      const read = await readTokensServedChannelMixSnapshot(s(), "today")
      expect(read).not.toBeNull()
      expect(read?.groups[0]?.channel).toBe("khala_api")
    })

    test("history: writes then reads back the EXACT post-image, keyed by window+timezone", async () => {
      const snapshot = historySnapshot({ window: "30d" })
      const outcome = await projectTokensServedHistorySnapshotBestEffort(
        s(),
        snapshot,
      )
      expect(outcome.ok).toBe(true)

      const read = await readTokensServedHistorySnapshot(
        s(),
        "30d",
        "America/Chicago",
      )
      expect(read).not.toBeNull()
      expect(read?.series).toHaveLength(1)
      expect(read?.series[0]?.day).toBe("2026-07-05")

      // A different timezone key never resolves to this snapshot.
      const missByTimezone = await readTokensServedHistorySnapshot(
        s(),
        "30d",
        "UTC",
      )
      expect(missByTimezone).toBeNull()
    })

    test("re-projecting the SAME window is idempotent at the read layer (upsert wins)", async () => {
      const first = await projectTokensServedModelMixSnapshotBestEffort(
        s(),
        modelMixSnapshot({ totalTokens: 1000, window: "all" }),
      )
      expect(first.ok).toBe(true)
      const second = await projectTokensServedModelMixSnapshotBestEffort(
        s(),
        modelMixSnapshot({ totalTokens: 2000, window: "all" }),
      )
      expect(second.ok).toBe(true)

      const read = await readTokensServedModelMixSnapshot(s(), "all")
      // The LATEST refresh wins — a stale in-flight refresh never regresses
      // the served snapshot back to an older total.
      expect(read?.totalTokens).toBe(2000)
    })

    test("an unprojected window/timezone reads as null (fail-open signal)", async () => {
      const read = await readTokensServedModelMixSnapshot(s(), "today")
      expect(read).toBeNull()
    })
  },
)
