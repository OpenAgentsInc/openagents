import {
  type BootstrapResponse,
  canonicalJson,
  type ChangelogEntry,
  type CvrPullResponse,
  EntityId,
  EntityType,
  publicScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { compactScope } from "./compaction.js"
import {
  CVR_RETAINED_VERSIONS,
  cvrPull,
  isKhalaSyncCvrEnabled,
  KhalaSyncCvrRowSetTooLargeError,
} from "./cvr-service.js"
import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import { bootstrap } from "./read-service.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(240_000)

/**
 * KS-7.2 (#8306) equivalence tests against REAL local Postgres: for
 * randomized scope histories (upserts / deletes / permission-set changes /
 * compaction / client live-drift), the flagged CVR diff-pull path must
 * leave a client in an end state BYTE-EQUAL to the unflagged full
 * re-bootstrap path — including the revocation case, where a row-set
 * shrink arrives as dels that retract state without a full reset.
 */

const entityType = EntityType.make("thing")
const eid = (n: number | string) => EntityId.make(`thing-${n}`)
const key = (id: string): string => `${entityType}/${eid(id)}`

let scopeCounter = 0
const freshScope = (): SyncScope => publicScope(`cvr-test-${++scopeCounter}`)

/** Deterministic PRNG (mulberry32) so failures replay exactly. */
const mulberry32 = (seed: number) => {
  let a = seed >>> 0
  return (): number => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

describe("flag parsing", () => {
  test("only the literal '1' enables the CVR surface", () => {
    expect(isKhalaSyncCvrEnabled("1")).toBe(true)
    expect(isKhalaSyncCvrEnabled(undefined)).toBe(false)
    expect(isKhalaSyncCvrEnabled("")).toBe(false)
    expect(isKhalaSyncCvrEnabled("0")).toBe(false)
    expect(isKhalaSyncCvrEnabled("true")).toBe(false)
    expect(isKhalaSyncCvrEnabled("on")).toBe(false)
  })
})

describe.skipIf(!hasLocalPostgres())("cvr service against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_cvr")
    await admin.end()
    const url = pg.urlFor("khala_sync_cvr")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0007_khala_sync_cvrs.sql")
    sql = new SQL({ url, max: 10 })
  })

  afterAll(async () => {
    await sql?.end()
    await pg?.stop()
  })

  // -------------------------------------------------------------------------
  // Server-side seeding helpers
  // -------------------------------------------------------------------------

  /** One committed transaction upserting one entity → its changelog entry. */
  const upsert = async (
    scope: SyncScope,
    id: string,
    postImage: unknown,
  ): Promise<ChangelogEntry> =>
    withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: eid(id),
        op: "upsert",
        postImage,
      }),
    )

  /** One committed transaction tombstoning one entity → its entry. */
  const tombstone = async (
    scope: SyncScope,
    id: string,
  ): Promise<ChangelogEntry> =>
    withSyncTransaction(sql, (writer) =>
      writer.appendChange({ scope, entityType, entityId: eid(id), op: "delete" }),
    )

  /**
   * Compact the scope down to the newest version group (the recovery
   * scenario: tombstones + superseded rows behind the window are GONE).
   * The capture checkpoint is advanced first so the checkpoint bound does
   * not hold the watermark (see ./compaction candidate 3).
   */
  const compactToLatest = async (scope: SyncScope): Promise<void> => {
    const rows: Array<{ last_version: string | number | bigint }> = await sql`
      SELECT last_version FROM khala_sync_scopes WHERE scope = ${scope}
    `
    const lastVersion = Number(rows[0]?.last_version ?? 0)
    await sql`
      INSERT INTO khala_sync_capture_checkpoints (scope, pushed_through_version)
      VALUES (${scope}, ${lastVersion})
      ON CONFLICT (scope) DO UPDATE
        SET pushed_through_version = EXCLUDED.pushed_through_version
    `
    await compactScope(sql, { scope, maxRetainedEntries: 1 })
  }

  // -------------------------------------------------------------------------
  // The two client paths under comparison
  // -------------------------------------------------------------------------

  type Visible = (entityType: string, entityId: string) => boolean
  const allVisible: Visible = () => true

  /**
   * UNFLAGGED reference path: a full re-bootstrap (drain every page), with
   * the visibility predicate applied the same way the authorized set
   * applies it. Returns entity key → canonical post-image bytes.
   */
  const bootstrapEndState = async (
    scope: SyncScope,
    visible: Visible = allVisible,
  ): Promise<Map<string, string>> => {
    const state = new Map<string, string>()
    let pageToken: string | undefined
    for (;;) {
      const page: BootstrapResponse = await bootstrap(sql, {
        scope,
        pageSize: 3, // small pages: exercise the token chain
        pageToken,
      })
      for (const entity of page.entities) {
        if (!visible(String(entity.entityType), String(entity.entityId))) continue
        state.set(
          `${entity.entityType}/${entity.entityId}`,
          entity.postImageJson,
        )
      }
      if (page.nextPageToken === undefined) return state
      pageToken = page.nextPageToken
    }
  }

  /**
   * FLAGGED path: a simulated client store driven exactly like the session
   * drives the real one — reset-mode pulls replace scope state (rows at
   * version = pull cursor, the resetScope semantics), diff-mode pulls
   * apply dels then puts (puts at version = pull cursor, the synthesized-
   * entry semantics), and live log entries land at their real versions.
   */
  class CvrClient {
    readonly rows = new Map<string, { postImageJson: string; version: number }>()
    cvr: { version: number; cursor: number } | null = null

    constructor(readonly clientGroupId: string) {}

    applyLive(entry: ChangelogEntry): void {
      const k = `${entry.entityType}/${entry.entityId}`
      if (entry.op === "delete") this.rows.delete(k)
      else {
        this.rows.set(k, {
          postImageJson: entry.postImageJson as string,
          version: Number(entry.version),
        })
      }
    }

    /** Rows applied after the last pull's snapshot (the drift set). */
    drift(): Array<{ entityType: string; entityId: string; version: number }> {
      if (this.cvr === null) return []
      const cursor = this.cvr.cursor
      const drift: Array<{ entityType: string; entityId: string; version: number }> =
        []
      for (const [k, row] of this.rows) {
        if (row.version <= cursor) continue
        const slash = k.indexOf("/")
        drift.push({
          entityType: k.slice(0, slash),
          entityId: k.slice(slash + 1),
          version: row.version,
        })
      }
      return drift
    }

    async pull(scope: SyncScope, visible: Visible = allVisible): Promise<CvrPullResponse> {
      const response = await cvrPull(sql, {
        scope,
        clientGroupId: this.clientGroupId,
        cvrVersion: this.cvr?.version ?? null,
        drift: this.drift(),
        isEntityVisible: visible,
      })
      if (response.mode === "reset") this.rows.clear()
      for (const del of response.dels) {
        this.rows.delete(`${del.entityType}/${del.entityId}`)
      }
      for (const put of response.puts) {
        this.rows.set(`${put.entityType}/${put.entityId}`, {
          postImageJson: put.postImageJson,
          version: Number(response.cursor),
        })
      }
      this.cvr = { version: Number(response.cvrVersion), cursor: Number(response.cursor) }
      return response
    }

    images(): Record<string, string> {
      const out: Record<string, string> = {}
      for (const [k, row] of this.rows) out[k] = row.postImageJson
      return out
    }
  }

  const asObject = (state: Map<string, string>): Record<string, string> =>
    Object.fromEntries(state)

  let clientCounter = 0
  const freshClient = (): CvrClient => new CvrClient(`cg-cvr-${++clientCounter}`)

  // -------------------------------------------------------------------------
  // Tests
  // -------------------------------------------------------------------------

  test("first pull (no CVR) is reset mode and byte-equal to a full bootstrap", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { id: "a", n: 1 })
    await upsert(scope, "b", { id: "b", nested: { deep: [1, 2, 3] } })
    await upsert(scope, "a", { id: "a", n: 2 })
    await tombstone(scope, "b")
    await upsert(scope, "c", { id: "c" })

    const client = freshClient()
    const response = await client.pull(scope)
    expect(response.mode).toBe("reset")
    expect(response.dels).toEqual([])
    expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))
    // The canonical bytes really are byte-equal to what the writer stored.
    expect(client.rows.get(key("a"))?.postImageJson).toBe(
      canonicalJson({ id: "a", n: 2 }),
    )
  })

  test("randomized histories: diff pull ≡ full re-bootstrap (byte-equal), with deletes, drift, and compaction", async () => {
    const random = mulberry32(0x8306)
    const ids = ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"]
    for (let iteration = 0; iteration < 12; iteration++) {
      const scope = freshScope()
      const client = freshClient()
      let revision = 0

      const randomOps = async (count: number): Promise<Array<ChangelogEntry>> => {
        const entries: Array<ChangelogEntry> = []
        for (let i = 0; i < count; i++) {
          const id = ids[Math.floor(random() * ids.length)]!
          if (random() < 0.3) {
            entries.push(await tombstone(scope, id))
          } else {
            revision += 1
            entries.push(await upsert(scope, id, { id, revision, iteration }))
          }
        }
        return entries
      }

      // Phase 1: history → first (reset) pull.
      await randomOps(12)
      const first = await client.pull(scope)
      expect(first.mode).toBe("reset")
      expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))

      // Phase 2: more history; the client applies a random PREFIX live
      // (the hybrid drift Replicache does not have), then the rest happens
      // while it is "offline", and compaction prunes tombstones/superseded
      // rows — the client's cursor is now unrecoverable via the log.
      const phase2 = await randomOps(14)
      const liveCount = Math.floor(random() * (phase2.length + 1))
      for (const entry of phase2.slice(0, liveCount)) client.applyLive(entry)
      await compactToLatest(scope)

      // Recovery: diff pull (never a full reset) must land byte-equal.
      const second = await client.pull(scope)
      expect(second.mode).toBe("diff")
      expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))

      // Phase 3: once more, without drift, to exercise CVR chaining.
      await randomOps(8)
      const third = await client.pull(scope)
      expect(third.mode).toBe("diff")
      expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))
    }
  })

  test("regression: a row acquired LIVE after the last CVR, then deleted and compacted away, is retracted via drift", async () => {
    const scope = freshScope()
    const client = freshClient()
    await upsert(scope, "base", { id: "base" })
    await client.pull(scope) // CVR v1 — does NOT contain "w"

    // "w" is born after the pull; the client applies it live.
    const wEntry = await upsert(scope, "w", { id: "w" })
    client.applyLive(wEntry)
    expect(client.rows.has(key("w"))).toBe(true)

    // "w" dies while the client is offline; the tombstone is compacted.
    await tombstone(scope, "w")
    await upsert(scope, "base", { id: "base", rev: 2 })
    await compactToLatest(scope)

    // A naive CVR diff (base = stored CVR only) would never del "w" —
    // it is in neither the CVR nor the current set. Drift closes the hole.
    const response = await client.pull(scope)
    expect(response.mode).toBe("diff")
    expect(
      response.dels.map((d) => `${d.entityType}/${d.entityId}`),
    ).toContain(key("w"))
    expect(client.rows.has(key("w"))).toBe(false)
    expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))
  })

  test("permission-set shrink: rows leaving the authorized set arrive as dels — no reset, state equals the filtered re-bootstrap", async () => {
    const scope = freshScope()
    const client = freshClient()
    await upsert(scope, "keep-1", { id: "keep-1" })
    await upsert(scope, "keep-2", { id: "keep-2" })
    await upsert(scope, "secret-1", { id: "secret-1" })
    await upsert(scope, "secret-2", { id: "secret-2" })

    await client.pull(scope, allVisible)
    expect(Object.keys(client.images()).sort()).toEqual([
      key("keep-1"),
      key("keep-2"),
      key("secret-1"),
      key("secret-2"),
    ])

    // Permission fanout: the caller loses row-level access to secret-*.
    const restricted: Visible = (_type, entityId) =>
      !String(entityId).includes("secret")
    const response = await client.pull(scope, restricted)
    expect(response.mode).toBe("diff") // structural retraction, NOT a reset
    expect(
      response.dels.map((d) => `${d.entityType}/${d.entityId}`).sort(),
    ).toEqual([key("secret-1"), key("secret-2")])
    // keep-* rows were untouched: no redundant puts for unchanged rows.
    expect(response.puts).toEqual([])
    expect(client.images()).toEqual(
      asObject(await bootstrapEndState(scope, restricted)),
    )
  })

  test("full revocation: the row set shrinks to nothing — dels retract everything, end state equals the cleared unflagged state", async () => {
    const scope = freshScope()
    const client = freshClient()
    await upsert(scope, "a", { id: "a" })
    await upsert(scope, "b", { id: "b" })
    await client.pull(scope)
    expect(Object.keys(client.images())).toHaveLength(2)

    const none: Visible = () => false
    const response = await client.pull(scope, none)
    expect(response.mode).toBe("diff")
    expect(response.puts).toEqual([])
    expect(response.dels).toHaveLength(2)
    // Byte-equal to the unflagged path's end state after revocation
    // (clear scope-local state — SPEC §7 invariant 7): empty.
    expect(client.images()).toEqual({})
  })

  test("unknown / pruned cvrVersion degrades to a reset-mode pull (never a silent under-delivery of dels)", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { id: "a" })
    const client = freshClient()
    await client.pull(scope)
    // Client claims a CVR the server no longer has.
    client.cvr = { version: 999, cursor: 0 }
    const response = await client.pull(scope)
    expect(response.mode).toBe("reset")
    expect(client.images()).toEqual(asObject(await bootstrapEndState(scope)))
  })

  test("CVR retention: old versions are pruned per (clientGroup, scope)", async () => {
    const scope = freshScope()
    const client = freshClient()
    for (let i = 0; i < CVR_RETAINED_VERSIONS + 3; i++) {
      await upsert(scope, "a", { id: "a", i })
      await client.pull(scope)
    }
    const rows: Array<{ n: string | number | bigint }> = await sql`
      SELECT count(*) AS n FROM khala_sync_cvrs
       WHERE client_group_id = ${client.clientGroupId} AND scope = ${scope}
    `
    expect(Number(rows[0]!.n)).toBeLessThanOrEqual(CVR_RETAINED_VERSIONS)
  })

  test("row-set cap: an oversized scope refuses with the typed error (fall back to paged bootstrap)", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { id: "a" })
    await upsert(scope, "b", { id: "b" })
    await upsert(scope, "c", { id: "c" })
    expect(
      cvrPull(sql, {
        scope,
        clientGroupId: "cg-cap",
        cvrVersion: null,
        maxRowSet: 2,
      }),
    ).rejects.toBeInstanceOf(KhalaSyncCvrRowSetTooLargeError)
  })

  test("empty scope: reset pull at watermark 0 with no puts", async () => {
    const scope = freshScope()
    const client = freshClient()
    const response = await client.pull(scope)
    expect(response.mode).toBe("reset")
    expect(response.puts).toEqual([])
    expect(Number(response.cursor)).toBe(0)
    expect(client.images()).toEqual({})
  })
})
