import {
  EntityId,
  EntityType,
  publicScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import {
  captureConfigFromEnv,
  runCapturePass,
  startCaptureDaemon,
  type CaptureConfig,
  runCaptureOnce,
} from "./capture.js"
import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

// ---------------------------------------------------------------------------
// Fake hub: a Bun.serve replica of the KhalaSyncHubDO /append contract
// (scope match, ascending version groups, density with the window edge,
// idempotent replay dedupe by version, 409 gap) behind the Worker's
// internal-route shape (?scope= query + admin bearer).
// ---------------------------------------------------------------------------

interface EncodedEntry {
  readonly scope: string
  readonly version: number
  readonly entityType: string
  readonly entityId: string
  readonly op: string
  readonly postImageJson?: string
  readonly committedAt: string
}

interface FakeHubScopeState {
  lastVersion: number
  /** version:type:id → entry (accepted, deduped by version). */
  entries: Map<string, EncodedEntry>
  /** Fresh (non-duplicate) entries of each successful append, in order. */
  batches: Array<Array<EncodedEntry>>
  /** Total entries dropped as idempotent replays across all appends. */
  duplicatesDropped: number
}

interface FakeHub {
  readonly appendUrl: string
  readonly token: string
  readonly scopes: Map<string, FakeHubScopeState>
  readonly failing: Set<string>
  readonly state: (scope: string) => FakeHubScopeState
  readonly stop: () => void
}

const makeFakeHub = (): FakeHub => {
  const token = "test-admin-token"
  const scopes = new Map<string, FakeHubScopeState>()
  const failing = new Set<string>()

  const state = (scope: string): FakeHubScopeState => {
    let existing = scopes.get(scope)
    if (existing === undefined) {
      existing = {
        lastVersion: 0,
        entries: new Map(),
        batches: [],
        duplicatesDropped: 0,
      }
      scopes.set(scope, existing)
    }
    return existing
  }

  const server = Bun.serve({
    port: 0,
    fetch: async (request) => {
      const url = new URL(request.url)
      if (request.method !== "POST") {
        return Response.json({ error: "method_not_allowed" }, { status: 405 })
      }
      if (request.headers.get("authorization") !== `Bearer ${token}`) {
        return Response.json({ error: "unauthorized" }, { status: 401 })
      }
      const scope = url.searchParams.get("scope")
      const body = (await request.json()) as {
        scope?: string
        entries?: Array<EncodedEntry>
      }
      if (
        scope === null ||
        body.scope !== scope ||
        !Array.isArray(body.entries) ||
        body.entries.length === 0 ||
        body.entries.some((e) => e.scope !== scope)
      ) {
        return Response.json(
          { error: "khala_sync_hub_append_invalid" },
          { status: 400 },
        )
      }
      if (failing.has(scope)) {
        return Response.json({ error: "injected_failure" }, { status: 500 })
      }

      for (let i = 1; i < body.entries.length; i++) {
        if (body.entries[i]!.version < body.entries[i - 1]!.version) {
          return Response.json(
            { error: "khala_sync_hub_append_invalid" },
            { status: 400 },
          )
        }
      }

      const hub = state(scope)
      const fresh = body.entries.filter((e) => e.version > hub.lastVersion)
      if (fresh.length === 0) {
        hub.duplicatesDropped += body.entries.length
        return Response.json({
          ok: true,
          appended: 0,
          duplicates: body.entries.length,
          lastVersion: hub.lastVersion,
        })
      }
      const versions = [...new Set(fresh.map((e) => e.version))]
      if (hub.lastVersion > 0 && versions[0]! !== hub.lastVersion + 1) {
        return Response.json(
          {
            error: "khala_sync_hub_version_gap",
            expectedFirstVersion: hub.lastVersion + 1,
            receivedFirstVersion: versions[0],
          },
          { status: 409 },
        )
      }
      for (let i = 1; i < versions.length; i++) {
        if (versions[i]! !== versions[i - 1]! + 1) {
          return Response.json(
            {
              error: "khala_sync_hub_version_gap",
              expectedFirstVersion: versions[i - 1]! + 1,
              receivedFirstVersion: versions[i],
            },
            { status: 409 },
          )
        }
      }

      for (const entry of fresh) {
        hub.entries.set(
          `${entry.version}:${entry.entityType}:${entry.entityId}`,
          entry,
        )
      }
      hub.lastVersion = versions[versions.length - 1]!
      hub.batches.push(fresh)
      hub.duplicatesDropped += body.entries.length - fresh.length
      return Response.json({
        ok: true,
        appended: fresh.length,
        duplicates: body.entries.length - fresh.length,
        lastVersion: hub.lastVersion,
      })
    },
  })

  return {
    appendUrl: `http://127.0.0.1:${server.port}/api/internal/khala-sync/hub/append`,
    token,
    scopes,
    failing,
    state,
    stop: () => server.stop(true),
  }
}

const waitUntil = async (
  predicate: () => boolean,
  timeoutMs: number,
  intervalMs = 50,
): Promise<boolean> => {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (predicate()) return true
    await new Promise((resolve) => setTimeout(resolve, intervalMs))
  }
  return predicate()
}

// ---------------------------------------------------------------------------
// Config unit tests (no Postgres required)
// ---------------------------------------------------------------------------

describe("captureConfigFromEnv", () => {
  test("builds a config from the documented variables", () => {
    const config = captureConfigFromEnv({
      KHALA_SYNC_DATABASE_URL: "postgres://u@h:5432/db",
      KHALA_SYNC_HUB_APPEND_URL: "https://openagents.com/api/internal/khala-sync/hub/append",
      OPENAGENTS_ADMIN_API_TOKEN: "tok",
      KHALA_SYNC_CAPTURE_POLL_INTERVAL_MS: "2500",
      KHALA_SYNC_CAPTURE_BATCH_VERSIONS: "50",
    })
    expect(config.databaseUrl).toBe("postgres://u@h:5432/db")
    expect(config.pollIntervalMs).toBe(2500)
    expect(config.batchVersions).toBe(50)
  })

  test("names every missing variable without echoing values", () => {
    expect(() => captureConfigFromEnv({})).toThrow(
      "KHALA_SYNC_DATABASE_URL, KHALA_SYNC_HUB_APPEND_URL, OPENAGENTS_ADMIN_API_TOKEN",
    )
  })
})

// ---------------------------------------------------------------------------
// Integration: capture against real local Postgres + a fake hub
// ---------------------------------------------------------------------------

const entityType = EntityType.make("thing")
let scopeCounter = 0
const freshScope = (): SyncScope => publicScope(`capture-test-${++scopeCounter}`)

describe.skipIf(!hasLocalPostgres())("capture against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL
  let databaseUrl: string
  let hub: FakeHub

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_capture")
    await admin.end()
    databaseUrl = pg.urlFor("khala_sync_capture")
    // Migration 0002 applies via the runner alongside 0001.
    const result = await runMigrations({ databaseUrl })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    expect(result.applied).toContain("0002_khala_sync_capture.sql")
    sql = new SQL({ url: databaseUrl, max: 10 })
    hub = makeFakeHub()
  })

  afterAll(async () => {
    hub?.stop()
    await sql?.end()
    await pg?.stop()
  })

  const baseConfig = (): CaptureConfig => ({
    databaseUrl,
    hubAppendUrl: hub.appendUrl,
    adminToken: hub.token,
    pushRetryBackoffMs: 1,
  })

  /** One committed transaction upserting one entity → its version. */
  const upsert = async (
    scope: SyncScope,
    id: string,
    postImage: unknown,
  ): Promise<number> => {
    const entry = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: EntityId.make(id),
        op: "upsert",
        postImage,
      }),
    )
    return Number(entry.version)
  }

  /** One committed transaction touching several entities → one version group. */
  const upsertGroup = async (
    scope: SyncScope,
    ids: ReadonlyArray<string>,
  ): Promise<number> => {
    let version = 0
    await withSyncTransaction(sql, async (writer) => {
      for (const id of ids) {
        const entry = await writer.appendChange({
          scope,
          entityType,
          entityId: EntityId.make(id),
          op: "upsert",
          postImage: { id },
        })
        version = Number(entry.version)
      }
    })
    return version
  }

  const checkpointOf = async (scope: SyncScope): Promise<number> => {
    const rows: Array<{ pushed_through_version: string | number | bigint }> =
      await sql`
        SELECT pushed_through_version
          FROM khala_sync_capture_checkpoints
         WHERE scope = ${scope}
      `
    return rows.length === 0 ? 0 : Number(rows[0]!.pushed_through_version)
  }

  const changelogCount = async (scope: SyncScope): Promise<number> => {
    const rows: Array<{ n: string | number | bigint }> = await sql`
      SELECT count(*) AS n FROM khala_sync_changelog WHERE scope = ${scope}
    `
    return Number(rows[0]!.n)
  }

  // -------------------------------------------------------------------------

  test("a --once pass pushes new rows ordered to the hub and advances checkpoints", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { id: "a", n: 1 })
    await upsert(scope, "b", { id: "b", n: 2 })
    await upsertGroup(scope, ["c", "d"])

    const first = await runCapturePass(sql, baseConfig())
    const scopeResult = first.scopes.find((s) => s.scope === scope)!
    expect(first.failedScopes).toBe(0)
    expect(scopeResult.upToDate).toBe(true)
    expect(scopeResult.entriesPushed).toBe(4)
    expect(scopeResult.pushedThroughVersion).toBe(3)
    expect(await checkpointOf(scope)).toBe(3)

    const state = hub.state(scope)
    expect(state.lastVersion).toBe(3)
    expect(state.entries.size).toBe(4)
    // Ordered: versions non-decreasing across the concatenated batches.
    const pushedVersions = state.batches.flat().map((e) => e.version)
    expect(pushedVersions).toEqual([...pushedVersions].sort((a, b) => a - b))
    // Post-images made it through the codec.
    expect(
      state.entries.get(`1:thing:a`)?.postImageJson,
    ).toBe(JSON.stringify({ id: "a", n: 1 }))

    // An immediately repeated pass finds nothing pending and pushes nothing.
    const batchesBefore = state.batches.length
    const second = await runCapturePass(sql, baseConfig())
    expect(second.scopes.find((s) => s.scope === scope)).toBeUndefined()
    expect(state.batches.length).toBe(batchesBefore)
  })

  test("restart resumes from the durable checkpoint with no hub duplicates", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { id: "a" })
    await upsert(scope, "b", { id: "b" })
    await runCapturePass(sql, baseConfig())
    expect(await checkpointOf(scope)).toBe(2)

    // New rows land while "the worker is down".
    await upsert(scope, "c", { id: "c" })
    await upsertGroup(scope, ["d", "e"])

    // "Restart": a fresh SQL pool resuming purely from the checkpoint table.
    const restarted = new SQL({ url: databaseUrl, max: 2 })
    try {
      const pass = await runCapturePass(restarted, baseConfig())
      const scopeResult = pass.scopes.find((s) => s.scope === scope)!
      expect(scopeResult.startedAfterVersion).toBe(2)
      expect(scopeResult.entriesPushed).toBe(3)
    } finally {
      await restarted.end()
    }

    const state = hub.state(scope)
    expect(await checkpointOf(scope)).toBe(4)
    expect(state.lastVersion).toBe(4)
    // The hub saw every changelog row exactly once — no replays reached it.
    expect(state.entries.size).toBe(await changelogCount(scope))
    expect(state.duplicatesDropped).toBe(0)
  })

  test("hub 409 version gap heals by re-pushing from the hub's expectation", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 8; i++) {
      await upsert(scope, `e${i}`, { i })
    }
    await runCapturePass(sql, baseConfig())
    expect(await checkpointOf(scope)).toBe(8)

    // Simulate hub divergence (e.g. restored from an older snapshot): the
    // hub only holds versions 1..2 while the checkpoint says 8.
    const state = hub.state(scope)
    state.lastVersion = 2
    for (const key of [...state.entries.keys()]) {
      if (Number(key.split(":")[0]) > 2) state.entries.delete(key)
    }

    await upsert(scope, "e9", { i: 9 })
    await upsert(scope, "e10", { i: 10 })

    const pass = await runCapturePass(sql, baseConfig())
    const scopeResult = pass.scopes.find((s) => s.scope === scope)!
    expect(scopeResult.error).toBeUndefined()
    expect(scopeResult.upToDate).toBe(true)

    // The hub healed to a dense 1..10 window and the checkpoint advanced.
    expect(state.lastVersion).toBe(10)
    expect(state.entries.size).toBe(10)
    expect(await checkpointOf(scope)).toBe(10)
  })

  test("a failing scope is isolated: others advance, its checkpoint stays, later pass heals", async () => {
    const failingScope = freshScope()
    const healthyScope = freshScope()
    await upsert(failingScope, "a", { id: "a" })
    await upsert(healthyScope, "b", { id: "b" })

    hub.failing.add(failingScope)
    const config = { ...baseConfig(), maxPushAttempts: 2 }
    const pass = await runCapturePass(sql, config)
    expect(pass.failedScopes).toBe(1)
    const failed = pass.scopes.find((s) => s.scope === failingScope)!
    expect(failed.error).toContain("http 500")
    expect(failed.pushedThroughVersion).toBe(0)
    expect(await checkpointOf(failingScope)).toBe(0)

    const healthy = pass.scopes.find((s) => s.scope === healthyScope)!
    expect(healthy.error).toBeUndefined()
    expect(await checkpointOf(healthyScope)).toBe(1)

    // The hub recovers → the next pass drains the failed scope.
    hub.failing.delete(failingScope)
    const healPass = await runCapturePass(sql, config)
    expect(healPass.failedScopes).toBe(0)
    expect(await checkpointOf(failingScope)).toBe(1)
    expect(hub.state(failingScope).lastVersion).toBe(1)
  })

  test("version groups are never split across pushes", async () => {
    const scope = freshScope()
    await upsert(scope, "solo1", { id: "solo1" })
    await upsertGroup(scope, ["g1", "g2", "g3"]) // one version, three entities
    await upsert(scope, "solo2", { id: "solo2" })
    await upsertGroup(scope, ["h1", "h2"])

    // batchVersions = 1: the smallest possible pages — a splitting bug
    // would push part of a group, and the hub's version dedupe would then
    // silently drop the remainder of that group forever.
    const pass = await runCapturePass(sql, { ...baseConfig(), batchVersions: 1 })
    const scopeResult = pass.scopes.find((s) => s.scope === scope)!
    expect(scopeResult.error).toBeUndefined()
    expect(scopeResult.batchesPushed).toBe(4)

    const state = hub.state(scope)
    // Nothing lost: every changelog row arrived.
    expect(state.entries.size).toBe(await changelogCount(scope))
    // No version appears in more than one pushed batch, and each batch
    // carries whole version groups (all entities of each version together).
    const seenVersions = new Set<number>()
    for (const batch of state.batches) {
      for (const version of new Set(batch.map((e) => e.version))) {
        expect(seenVersions.has(version)).toBe(false)
        seenVersions.add(version)
      }
    }
    const groupBatch = state.batches.find((b) => b.some((e) => e.entityId === "g1"))!
    expect(groupBatch.map((e) => e.entityId).sort()).toEqual(["g1", "g2", "g3"])
  })

  test("daemon: NOTIFY wakes the loop and pushes promptly (poll fallback idle)", async () => {
    const scope = freshScope()
    await upsert(scope, "pre", { id: "pre" })

    // Poll interval far beyond the assertion window: only the LISTEN wake
    // can deliver the post-start insert in time.
    const daemon = startCaptureDaemon({
      ...baseConfig(),
      pollIntervalMs: 120_000,
    })
    try {
      // Initial resume-from-checkpoints pass drains the seed row.
      expect(
        await waitUntil(() => hub.state(scope).lastVersion === 1, 10_000),
      ).toBe(true)
      await daemon.listenerReady

      const version = await upsert(scope, "live", { id: "live" })
      expect(version).toBe(2)
      expect(
        await waitUntil(() => hub.state(scope).lastVersion === 2, 10_000),
      ).toBe(true)
      expect(await checkpointOf(scope)).toBe(2)
    } finally {
      await daemon.stop()
    }
  })

  test("daemon: stop() shuts down cleanly and stops pushing", async () => {
    const scope = freshScope()
    const daemon = startCaptureDaemon({ ...baseConfig(), pollIntervalMs: 200 })
    await daemon.listenerReady
    await daemon.stop()

    await upsert(scope, "after-stop", { id: "after-stop" })
    await new Promise((resolve) => setTimeout(resolve, 600))
    expect(hub.scopes.get(scope)?.lastVersion ?? 0).toBe(0)
  })

  test("mirror hub receives every acknowledged batch; a failing mirror never gates the checkpoint (CFG-5)", async () => {
    const mirror = makeFakeHub()
    try {
      const scope = freshScope()
      await upsert(scope, "m1", { id: "m1" })
      await upsert(scope, "m2", { id: "m2" })

      const config: CaptureConfig = {
        ...baseConfig(),
        mirrorAppendUrl: mirror.appendUrl,
        mirrorToken: mirror.token,
      }
      const first = await runCaptureOnce(config)
      const firstScope = first.scopes.find((r) => r.scope === scope)!
      expect(firstScope.upToDate).toBe(true)
      expect(await checkpointOf(scope)).toBe(2)
      // The mirror saw the same acknowledged batch.
      expect(mirror.state(scope).lastVersion).toBe(2)

      // Break ONLY the mirror: the primary still acks, the checkpoint
      // still advances, the pass reports no error.
      mirror.failing.add(scope)
      await upsert(scope, "m3", { id: "m3" })
      const second = await runCaptureOnce(config)
      const secondScope = second.scopes.find((r) => r.scope === scope)!
      expect(secondScope.upToDate).toBe(true)
      expect(secondScope.error).toBeUndefined()
      expect(await checkpointOf(scope)).toBe(3)
      expect(hub.state(scope).lastVersion).toBe(3)
      expect(mirror.state(scope).lastVersion).toBe(2)
    } finally {
      mirror.stop()
    }
  })
})
