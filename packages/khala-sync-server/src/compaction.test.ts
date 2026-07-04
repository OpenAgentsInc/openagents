import {
  type BootstrapResponse,
  EntityId,
  EntityType,
  type LogPage,
  publicScope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import { CAPTURE_CHECKPOINTS_TABLE, compactAll, compactScope } from "./compaction.js"
import { KhalaSyncCursorBehindRetainedWindowError } from "./errors.js"
import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import { bootstrap, logPage } from "./read-service.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(240_000)

const entityType = EntityType.make("thing")
const eid = (n: number | string) => EntityId.make(`thing-${n}`)

let scopeCounter = 0
const freshScope = (): SyncScope => publicScope(`compaction-test-${++scopeCounter}`)

/** Deterministic PRNG for the property-ish interleaving loop. */
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

describe.skipIf(!hasLocalPostgres())("compaction against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_compaction")
    await admin.end()
    const url = pg.urlFor("khala_sync_compaction")
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    expect(result.applied).toContain("0003_khala_sync_changelog_entity_index.sql")
    sql = new SQL({ url, max: 10 })
    // The capture lane (KS-4) lands concurrently; create its checkpoint
    // table (compatible minimal shape, no-op if a migration already made
    // it) so this suite exercises the checkpoint guard for real. Tests that
    // are NOT about the guard mark their scope fully pushed.
    await sql.unsafe(`
      CREATE TABLE IF NOT EXISTS ${CAPTURE_CHECKPOINTS_TABLE} (
        scope text PRIMARY KEY,
        pushed_through_version bigint NOT NULL DEFAULT 0
      )
    `)
  })

  afterAll(async () => {
    await sql?.end()
    await pg?.stop()
  })

  // -------------------------------------------------------------------------
  // Seed + client helpers
  // -------------------------------------------------------------------------

  const upsert = async (
    scope: SyncScope,
    id: string,
    postImage: unknown,
  ): Promise<number> => {
    const entry = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({ scope, entityType, entityId: eid(id), op: "upsert", postImage }),
    )
    return Number(entry.version)
  }

  const tombstone = async (scope: SyncScope, id: string): Promise<number> => {
    const entry = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({ scope, entityType, entityId: eid(id), op: "delete" }),
    )
    return Number(entry.version)
  }

  /** One transaction touching several entities → ONE version group. */
  const upsertGroup = async (
    scope: SyncScope,
    ids: ReadonlyArray<string>,
    tag: string,
  ): Promise<number> => {
    let version = 0
    await withSyncTransaction(sql, async (writer) => {
      for (const id of ids) {
        const entry = await writer.appendChange({
          scope,
          entityType,
          entityId: eid(id),
          op: "upsert",
          postImage: { id, tag },
        })
        version = Number(entry.version)
      }
    })
    return version
  }

  /** Capture checkpoint: mark `scope` pushed through `version`. */
  const markPushed = async (scope: SyncScope, version: number): Promise<void> => {
    await sql`
      INSERT INTO khala_sync_capture_checkpoints (scope, pushed_through_version)
      VALUES (${scope}, ${version})
      ON CONFLICT (scope) DO UPDATE SET
        pushed_through_version = EXCLUDED.pushed_through_version
    `
  }

  interface ScopeCounters {
    lastVersion: number
    retainedFromVersion: number
  }

  const scopeCounters = async (scope: SyncScope): Promise<ScopeCounters> => {
    const rows: Array<{ last_version: string; retained_from_version: string }> = await sql`
      SELECT last_version, retained_from_version
        FROM khala_sync_scopes WHERE scope = ${scope}
    `
    expect(rows.length).toBe(1)
    return {
      lastVersion: Number(rows[0]!.last_version),
      retainedFromVersion: Number(rows[0]!.retained_from_version),
    }
  }

  const changelogVersions = async (scope: SyncScope): Promise<Array<number>> => {
    const rows: Array<{ version: string }> = await sql`
      SELECT version FROM khala_sync_changelog
       WHERE scope = ${scope} ORDER BY version, entity_type, entity_id
    `
    return rows.map((r) => Number(r.version))
  }

  const entityKey = (type: string, id: string): string => `${type} ${id}`

  const applyBootstrapPage = (store: Map<string, string>, page: BootstrapResponse): void => {
    for (const e of page.entities) {
      store.set(entityKey(String(e.entityType), String(e.entityId)), e.postImageJson)
    }
  }

  const applyLogPage = (store: Map<string, string>, page: LogPage): void => {
    for (const entry of page.entries) {
      const key = entityKey(String(entry.entityType), String(entry.entityId))
      if (entry.op === "upsert") {
        store.set(key, entry.postImageJson as string)
      } else {
        store.delete(key)
      }
    }
  }

  /** Fresh full sync: bootstrap all pages, then stitch the log to head. */
  const fullSync = async (scope: SyncScope): Promise<Map<string, string>> => {
    const store = new Map<string, string>()
    let pageToken: string | undefined
    let cursor = 0
    for (;;) {
      const page = await bootstrap(sql, { scope, pageSize: 3, pageToken })
      applyBootstrapPage(store, page)
      if (page.nextPageToken === undefined) {
        cursor = Number(page.cursor)
        break
      }
      pageToken = page.nextPageToken
    }
    for (;;) {
      const page = await logPage(sql, { scope, afterVersion: cursor, limit: 4 })
      applyLogPage(store, page)
      if (page.upToDate) return store
      cursor = Number(page.nextCursor)
    }
  }

  // -------------------------------------------------------------------------
  // Entry-count bound
  // -------------------------------------------------------------------------

  test("entry-count compaction keeps exactly the newest N version groups and advances the watermark", async () => {
    const scope = freshScope()
    // One entity updated 10× — every old version is superseded, so nothing
    // needs preserving behind the watermark.
    for (let i = 1; i <= 10; i++) await upsert(scope, "x", { i })
    await markPushed(scope, 10)

    const result = await compactScope(sql, { scope, maxRetainedEntries: 3 })
    expect(result.advanced).toBe(true)
    expect(result.newRetainedFromVersion).toBe(8) // 10 - 3 + 1
    expect(result.boundedBy).toBe("entry_count")
    expect(result.deletedRows).toBe(7)
    expect(result.preservedSnapshotRows).toBe(0)

    expect(await changelogVersions(scope)).toEqual([8, 9, 10])
    const counters = await scopeCounters(scope)
    expect(counters.retainedFromVersion).toBe(8)
    expect(counters.lastVersion).toBe(10)

    // Idempotent rerun: nothing further to do.
    const again = await compactScope(sql, { scope, maxRetainedEntries: 3 })
    expect(again.advanced).toBe(false)
    expect(again.deletedRows).toBe(0)
    expect(again.newRetainedFromVersion).toBe(8)
  })

  test("live entities' latest upsert rows are preserved behind the watermark (bootstrap stays whole)", async () => {
    const scope = freshScope()
    await upsert(scope, "a", { v: "a1" }) // v1 — a's latest, must survive
    await upsert(scope, "b", { v: "b1" }) // v2 — b's latest, must survive
    for (let i = 3; i <= 10; i++) await upsert(scope, "c", { v: `c${i}` })
    await markPushed(scope, 10)

    const result = await compactScope(sql, { scope, maxRetainedEntries: 2 })
    expect(result.newRetainedFromVersion).toBe(9)
    // c@3..8 superseded and deleted; a@1, b@2 preserved snapshot residue.
    expect(result.deletedRows).toBe(6)
    expect(result.preservedSnapshotRows).toBe(2)
    expect(await changelogVersions(scope)).toEqual([1, 2, 9, 10])

    const store = await fullSync(scope)
    expect(store.size).toBe(3)
    expect(store.get(entityKey("thing", "thing-a"))).toContain("a1")
    expect(store.get(entityKey("thing", "thing-b"))).toContain("b1")
    expect(store.get(entityKey("thing", "thing-c"))).toContain("c10")
  })

  test("tombstone GC: tombstones and their superseded upserts vanish behind the watermark", async () => {
    const scope = freshScope()
    await upsert(scope, "x", { v: 1 }) // v1
    await tombstone(scope, "x") // v2 — x is gone
    await upsert(scope, "y", { v: 3 }) // v3
    await upsert(scope, "y", { v: 4 }) // v4
    await markPushed(scope, 4)

    const result = await compactScope(sql, { scope, maxRetainedEntries: 1 })
    expect(result.newRetainedFromVersion).toBe(4)
    // x@1 superseded, x@2 tombstone, y@3 superseded — all GC'd.
    expect(result.deletedRows).toBe(3)
    expect(await changelogVersions(scope)).toEqual([4])

    const store = await fullSync(scope)
    expect([...store.keys()]).toEqual([entityKey("thing", "thing-y")])
  })

  // -------------------------------------------------------------------------
  // Age bound
  // -------------------------------------------------------------------------

  test("age bound holds the watermark back for entries younger than maxRetainedAgeMs", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 5; i++) await upsert(scope, "x", { i })
    await markPushed(scope, 5)
    // Backdate versions 1-2 to two hours ago; 3-5 stay fresh.
    await sql`
      UPDATE khala_sync_changelog
         SET committed_at = now() - interval '2 hours'
       WHERE scope = ${scope} AND version <= 2
    `

    const held = await compactScope(sql, {
      scope,
      maxRetainedEntries: 1, // entry-count alone would allow watermark 5
      maxRetainedAgeMs: 60 * 60 * 1000, // keep everything younger than 1h
    })
    expect(held.boundedBy).toBe("age")
    expect(held.ageCandidate).toBe(3)
    expect(held.newRetainedFromVersion).toBe(3)
    expect(held.deletedRows).toBe(2)
    expect(await changelogVersions(scope)).toEqual([3, 4, 5])

    // Once everything is old enough, entry count is the binding bound again.
    await sql`
      UPDATE khala_sync_changelog
         SET committed_at = now() - interval '2 hours'
       WHERE scope = ${scope}
    `
    const released = await compactScope(sql, {
      scope,
      maxRetainedEntries: 1,
      maxRetainedAgeMs: 60 * 60 * 1000,
    })
    expect(released.boundedBy).toBe("entry_count")
    expect(released.newRetainedFromVersion).toBe(5)
    expect(await changelogVersions(scope)).toEqual([5])
  })

  // -------------------------------------------------------------------------
  // Capture-checkpoint bound
  // -------------------------------------------------------------------------

  test("capture-checkpoint bound never advances the watermark past pushed_through_version + 1", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 10; i++) await upsert(scope, "x", { i })
    await markPushed(scope, 4) // capture has only pushed v1..v4

    const held = await compactScope(sql, { scope, maxRetainedEntries: 2 })
    expect(held.boundedBy).toBe("capture_checkpoint")
    expect(held.captureCheckpointCandidate).toBe(5)
    expect(held.newRetainedFromVersion).toBe(5)
    expect(await changelogVersions(scope)).toEqual([5, 6, 7, 8, 9, 10])

    // Capture catches up → the entry-count window applies fully.
    await markPushed(scope, 10)
    const released = await compactScope(sql, { scope, maxRetainedEntries: 2 })
    expect(released.boundedBy).toBe("entry_count")
    expect(released.newRetainedFromVersion).toBe(9)
    expect(await changelogVersions(scope)).toEqual([9, 10])
  })

  test("a scope with NO checkpoint row fails closed (no compaction at all)", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 6; i++) await upsert(scope, "x", { i })
    // Deliberately NO markPushed: capture has recorded nothing.

    const result = await compactScope(sql, { scope, maxRetainedEntries: 1 })
    expect(result.advanced).toBe(false)
    expect(result.boundedBy).toBe("capture_checkpoint")
    expect(result.captureCheckpointCandidate).toBe(1)
    expect(result.deletedRows).toBe(0)
    expect((await scopeCounters(scope)).retainedFromVersion).toBe(1)
    expect(await changelogVersions(scope)).toEqual([1, 2, 3, 4, 5, 6])
  })

  // -------------------------------------------------------------------------
  // Whole version groups
  // -------------------------------------------------------------------------

  test("compaction never splits a version group at or above the watermark", async () => {
    const scope = freshScope()
    // Three transactions, three entities each → version groups of 3 rows.
    await upsertGroup(scope, ["p", "q", "r"], "g1") // v1
    await upsertGroup(scope, ["p", "q", "r"], "g2") // v2
    await upsertGroup(scope, ["p", "q", "r"], "g3") // v3
    await markPushed(scope, 3)

    const result = await compactScope(sql, { scope, maxRetainedEntries: 2 })
    expect(result.newRetainedFromVersion).toBe(2)
    expect(result.deletedRows).toBe(3) // ALL of group v1, nothing else

    // Groups v2 and v3 are fully intact (3 rows each), group v1 fully gone.
    const versions = await changelogVersions(scope)
    expect(versions).toEqual([2, 2, 2, 3, 3, 3])

    // The whole retained window is servable from its edge.
    const page = await logPage(sql, { scope, afterVersion: 1, limit: 10 })
    expect(page.entries.length).toBe(6)
    expect(page.upToDate).toBe(true)
  })

  // -------------------------------------------------------------------------
  // MustRefetch chain + seam integrity post-compaction
  // -------------------------------------------------------------------------

  test("after compaction a stale cursor raises cursor_behind_retained_window while a fresh bootstrap converges", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 8; i++) await upsert(scope, `e${i % 3}`, { i })
    await tombstone(scope, "e0") // v9
    for (let i = 10; i <= 12; i++) await upsert(scope, "e1", { i })
    await markPushed(scope, 12)

    // A bootstrap page token minted BEFORE compaction (snapshot cursor 12
    // with more pages pending would embed 12; force paging with pageSize 1).
    const preCompactionFirstPage = await bootstrap(sql, { scope, pageSize: 1 })
    expect(preCompactionFirstPage.nextPageToken).toBeDefined()

    // More writes so the pre-compaction snapshot falls behind the window.
    for (let i = 13; i <= 20; i++) await upsert(scope, "e2", { i })
    await markPushed(scope, 20)

    const result = await compactScope(sql, { scope, maxRetainedEntries: 3 })
    expect(result.newRetainedFromVersion).toBe(18)

    // Invariant 6, log path: a cursor behind the window is REFUSED (the
    // wire layer maps this to MustRefetch(cursor_behind_retained_window);
    // the hub DO returns 410 Gone with the same code) — never a partial log.
    const stale = logPage(sql, { scope, afterVersion: 5, limit: 10 })
    await expect(stale).rejects.toBeInstanceOf(KhalaSyncCursorBehindRetainedWindowError)
    await expect(stale).rejects.toMatchObject({
      code: "cursor_behind_retained_window",
      scope: String(scope),
      retainedFromVersion: 18,
    })

    // Boundary exactness: resuming after watermark-1 (needs versions >= 18
    // only) is servable; one earlier is not.
    const edge = await logPage(sql, { scope, afterVersion: 17, limit: 10 })
    expect(edge.upToDate).toBe(true)
    await expect(
      logPage(sql, { scope, afterVersion: 16, limit: 10 }),
    ).rejects.toBeInstanceOf(KhalaSyncCursorBehindRetainedWindowError)

    // Invariant 6, bootstrap path: the pre-compaction page token's snapshot
    // (12) is behind the window → every further page fails closed.
    await expect(
      bootstrap(sql, { scope, pageSize: 1, pageToken: preCompactionFirstPage.nextPageToken }),
    ).rejects.toBeInstanceOf(KhalaSyncCursorBehindRetainedWindowError)

    // The recovery path converges: fresh bootstrap + stitch equals head
    // state (e0 tombstoned away, e1@12, e2@20 — preserved snapshot rows and
    // retained log stitched exactly).
    const store = await fullSync(scope)
    expect([...store.keys()].sort()).toEqual([
      entityKey("thing", "thing-e1"),
      entityKey("thing", "thing-e2"),
    ])
    expect(store.get(entityKey("thing", "thing-e1"))).toContain("12")
    expect(store.get(entityKey("thing", "thing-e2"))).toContain("20")
  })

  // -------------------------------------------------------------------------
  // compactAll
  // -------------------------------------------------------------------------

  test("compactAll discovers only scopes whose window can move, isolates per-scope failures, and reports a summary", async () => {
    const compactable = freshScope()
    const alreadyCompact = freshScope()
    const poisoned = freshScope()

    for (let i = 1; i <= 6; i++) await upsert(compactable, "x", { i })
    await markPushed(compactable, 6)
    await upsert(alreadyCompact, "x", { v: 1 }) // 1 version — window can't move
    await markPushed(alreadyCompact, 1)
    for (let i = 1; i <= 6; i++) await upsert(poisoned, "x", { i })
    await markPushed(poisoned, 6)

    // Real fault injection: refuse DELETEs for the poisoned scope so its
    // compaction transaction aborts AFTER the watermark update.
    await sql.unsafe(`
      CREATE OR REPLACE FUNCTION khala_test_poison_delete() RETURNS trigger AS $$
      BEGIN
        IF OLD.scope = '${String(poisoned)}' THEN
          RAISE EXCEPTION 'poisoned scope refuses deletes';
        END IF;
        RETURN OLD;
      END;
      $$ LANGUAGE plpgsql;
      CREATE TRIGGER khala_test_poison_delete_trigger
        BEFORE DELETE ON khala_sync_changelog
        FOR EACH ROW EXECUTE FUNCTION khala_test_poison_delete();
    `)
    try {
      const summary = await compactAll(sql, { maxRetainedEntries: 2 })

      const scopes = summary.results.map((r) => String(r.scope))
      expect(scopes).toContain(String(compactable))
      expect(scopes).not.toContain(String(alreadyCompact)) // never discovered
      expect(summary.failures.map((f) => f.scope)).toContain(String(poisoned))

      const ok = summary.results.find((r) => String(r.scope) === String(compactable))!
      expect(ok.advanced).toBe(true)
      expect(ok.newRetainedFromVersion).toBe(5)
      expect(await changelogVersions(compactable)).toEqual([5, 6])

      // The poisoned scope's transaction rolled back ATOMICALLY: watermark
      // untouched, rows untouched (watermark advance + delete are one txn).
      expect((await scopeCounters(poisoned)).retainedFromVersion).toBe(1)
      expect(await changelogVersions(poisoned)).toEqual([1, 2, 3, 4, 5, 6])
    } finally {
      await sql.unsafe(`
        DROP TRIGGER IF EXISTS khala_test_poison_delete_trigger ON khala_sync_changelog;
        DROP FUNCTION IF EXISTS khala_test_poison_delete();
      `)
    }
  })

  test("compactAll dry-run reports the identical plan without writing anything", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 7; i++) await upsert(scope, "x", { i })
    await markPushed(scope, 7)

    const dry = await compactAll(sql, { maxRetainedEntries: 3, dryRun: true })
    const plan = dry.results.find((r) => String(r.scope) === String(scope))!
    expect(dry.dryRun).toBe(true)
    expect(plan.advanced).toBe(true)
    expect(plan.newRetainedFromVersion).toBe(5)
    expect(plan.deletedRows).toBe(4)
    expect(plan.preservedSnapshotRows).toBe(0)

    // Nothing changed.
    expect((await scopeCounters(scope)).retainedFromVersion).toBe(1)
    expect(await changelogVersions(scope)).toEqual([1, 2, 3, 4, 5, 6, 7])

    // The real run matches the dry-run plan exactly.
    const real = await compactScope(sql, { scope, maxRetainedEntries: 3 })
    expect(real.newRetainedFromVersion).toBe(plan.newRetainedFromVersion)
    expect(real.deletedRows).toBe(plan.deletedRows)
    expect(real.preservedSnapshotRows).toBe(plan.preservedSnapshotRows)
  })

  // -------------------------------------------------------------------------
  // CLI
  // -------------------------------------------------------------------------

  test("scripts/compact.ts --dry-run prints a per-scope plan and writes nothing", async () => {
    const scope = freshScope()
    for (let i = 1; i <= 5; i++) await upsert(scope, "x", { i })
    await markPushed(scope, 5)

    const script = new URL("../scripts/compact.ts", import.meta.url).pathname
    const proc = Bun.spawnSync(
      ["bun", script, "--dry-run", "--max-retained-entries", "2"],
      {
        env: {
          ...process.env,
          KHALA_SYNC_DATABASE_URL: pg.urlFor("khala_sync_compaction"),
        },
        stdout: "pipe",
        stderr: "pipe",
      },
    )
    const stdout = proc.stdout.toString()
    expect(proc.exitCode).toBe(0)
    expect(stdout).toContain(String(scope))
    expect(stdout).toContain("retained_from 1 -> 4")
    expect(stdout).toContain("dry run:")

    expect((await scopeCounters(scope)).retainedFromVersion).toBe(1)
    expect((await changelogVersions(scope)).length).toBe(5)
  })

  // -------------------------------------------------------------------------
  // Property-ish loop: constraint + window invariants under interleavings
  // -------------------------------------------------------------------------

  test("random write/compact interleavings never violate the retention constraint and always converge (seed 8289)", async () => {
    const scope = freshScope()
    const rand = mulberry32(8289)
    const ids = ["a", "b", "c", "d", "e", "f", "g", "h"]
    /** Client-model of head state: entity id → live post-image tag. */
    const model = new Map<string, number>()
    let lastVersion = 0
    let op = 0

    for (let step = 0; step < 120; step++) {
      const roll = rand()
      if (roll < 0.55 || lastVersion === 0) {
        // Write: upsert or tombstone a random entity.
        const id = ids[Math.floor(rand() * ids.length)]!
        if (rand() < 0.8 || !model.has(id)) {
          op++
          lastVersion = await upsert(scope, id, { id, op })
          model.set(id, op)
        } else {
          lastVersion = await tombstone(scope, id)
          model.delete(id)
        }
        // Capture keeps up most of the time; sometimes it lags.
        if (rand() < 0.8) await markPushed(scope, lastVersion)
      } else {
        const result = await compactScope(sql, {
          scope,
          maxRetainedEntries: 1 + Math.floor(rand() * 6),
          ...(rand() < 0.3 ? { maxRetainedAgeMs: Math.floor(rand() * 50) } : {}),
        })
        // Constraint (khala_sync_scopes_retention) can never be violated —
        // Postgres would have thrown; assert the invariant shape anyway.
        const counters = await scopeCounters(scope)
        expect(counters.retainedFromVersion).toBeGreaterThanOrEqual(1)
        expect(counters.retainedFromVersion).toBeLessThanOrEqual(counters.lastVersion + 1)
        expect(result.newRetainedFromVersion).toBe(counters.retainedFromVersion)
        expect(result.newRetainedFromVersion).toBeGreaterThanOrEqual(
          result.previousRetainedFromVersion,
        )

        // Rows behind the watermark are always upserts (tombstones are
        // GC'd at every advance), at most ONE per entity, and any row
        // superseding one lives AT/ABOVE the watermark (a write that
        // landed after the advance; the next advance cleans the residue,
        // and bootstrap's latest-per-entity pick stays correct meanwhile).
        const behind: Array<{
          op: string
          entity_id: string
          version: string
          newer_below_watermark: boolean
        }> = await sql`
          SELECT c.op, c.entity_id, c.version,
                 EXISTS (
                   SELECT 1 FROM khala_sync_changelog n
                    WHERE n.scope = c.scope
                      AND n.entity_type = c.entity_type
                      AND n.entity_id = c.entity_id
                      AND n.version > c.version
                      AND n.version < ${counters.retainedFromVersion}
                 ) AS newer_below_watermark
            FROM khala_sync_changelog c
           WHERE c.scope = ${scope}
             AND c.version < ${counters.retainedFromVersion}
        `
        const behindEntities = new Set<string>()
        for (const row of behind) {
          expect(row.op).toBe("upsert")
          expect(row.newer_below_watermark).toBe(false)
          expect(behindEntities.has(row.entity_id)).toBe(false)
          behindEntities.add(row.entity_id)
        }

        // The window edge is always servable.
        const edge = await logPage(sql, {
          scope,
          afterVersion: counters.retainedFromVersion - 1,
          limit: 1000,
        })
        expect(edge.upToDate).toBe(true)
      }
    }

    // Final convergence: a fresh client (bootstrap + stitch) sees exactly
    // the model's head state, post any interleaved compactions.
    const store = await fullSync(scope)
    expect(store.size).toBe(model.size)
    for (const [id, tag] of model) {
      const image = store.get(entityKey("thing", `thing-${id}`))
      expect(image).toBeDefined()
      expect(JSON.parse(image!)).toEqual({ id, op: tag })
    }
  })
})
