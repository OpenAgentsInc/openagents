import {
  canonicalJson,
  decodeChangelogEntry,
  EntityId,
  EntityType,
  encodeChangelogEntry,
  publicScope,
  type SyncScope,
  type SyncVersion,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import { afterAll, beforeAll, describe, expect, setDefaultTimeout, test } from "bun:test"
import { KhalaSyncStorageError, storageErrorFromUnknown } from "./errors.js"
import { runMigrations } from "./migrate.js"
import {
  type AppendChangeInput,
  changelogEntryFromRow,
  type ChangelogRow,
  withSyncTransaction,
} from "./outbox-writer.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const entityType = EntityType.make("thing")
const entityId = (n: number) => EntityId.make(`thing-${n}`)

let scopeCounter = 0
const freshScope = (): SyncScope => publicScope(`outbox-test-${++scopeCounter}`)

// ---------------------------------------------------------------------------
// Pure: storage error mapping (no database needed)
// ---------------------------------------------------------------------------

describe("storageErrorFromUnknown", () => {
  const pgError = (errno: string, constraint?: string) =>
    Object.assign(new Error("server error"), {
      code: "ERR_POSTGRES_SERVER_ERROR",
      errno,
      ...(constraint === undefined ? {} : { constraint }),
    })

  test("maps SQLSTATE 23xxx to constraint_violation with the constraint name", () => {
    const mapped = storageErrorFromUnknown(
      pgError("23514", "khala_sync_changelog_post_image_shape"),
    )
    expect(mapped?.reason).toBe("constraint_violation")
    expect(mapped?.messageSafe).toContain("23514")
    expect(mapped?.messageSafe).toContain("khala_sync_changelog_post_image_shape")
  })

  test("maps serialization/deadlock/lock SQLSTATEs to transaction_conflict", () => {
    for (const errno of ["40001", "40P01", "55P03"]) {
      expect(storageErrorFromUnknown(pgError(errno))?.reason).toBe(
        "transaction_conflict",
      )
    }
  })

  test("maps connection-class failures to connection_failed", () => {
    expect(storageErrorFromUnknown(pgError("08006"))?.reason).toBe(
      "connection_failed",
    )
    const closed = Object.assign(new Error("Connection closed"), {
      code: "ERR_POSTGRES_CONNECTION_CLOSED",
    })
    expect(storageErrorFromUnknown(closed)?.reason).toBe("connection_failed")
  })

  test("maps resource/system SQLSTATEs to unavailable", () => {
    expect(storageErrorFromUnknown(pgError("53300"))?.reason).toBe("unavailable")
    expect(storageErrorFromUnknown(pgError("57P01"))?.reason).toBe("unavailable")
  })

  test("returns existing KhalaSyncStorageError unchanged", () => {
    const err = new KhalaSyncStorageError("unavailable", "x")
    expect(storageErrorFromUnknown(err)).toBe(err)
  })

  test("returns null for non-SQL errors (caller domain errors pass through)", () => {
    expect(storageErrorFromUnknown(new Error("domain"))).toBeNull()
    expect(storageErrorFromUnknown("string")).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Type-level: delete ⟺ no post-image
// ---------------------------------------------------------------------------

// @ts-expect-error deletes are tombstones — a post-image is a type error
const _tombstoneCannotCarryPostImage: AppendChangeInput = {
  scope: publicScope("type-test"),
  entityType,
  entityId: entityId(0),
  op: "delete",
  postImage: { nope: true },
}
void _tombstoneCannotCarryPostImage

// ---------------------------------------------------------------------------
// Integration: real local Postgres (initdb + pg_ctl throwaway instance)
// ---------------------------------------------------------------------------

describe.skipIf(!hasLocalPostgres())("outbox writer against local Postgres", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = new SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_outbox")
    await admin.end()
    const url = pg.urlFor("khala_sync_outbox")
    // Apply the real migration set with the KS-0.3 runner.
    const result = await runMigrations({ databaseUrl: url })
    expect(result.applied).toContain("0001_khala_sync_core.sql")
    // max: 10 so concurrent-writer tests hold several transactions open.
    sql = new SQL({ url, max: 10 })
  })

  afterAll(async () => {
    await sql?.end()
    await pg?.stop()
  })

  const changelogRows = (scope: SyncScope): Promise<Array<ChangelogRow>> =>
    sql`
      SELECT scope, version, entity_type, entity_id, op,
             post_image_json, mutation_ref, committed_at
        FROM khala_sync_changelog
       WHERE scope = ${scope}
       ORDER BY version, entity_type, entity_id
    ` as unknown as Promise<Array<ChangelogRow>>

  const lastVersion = async (scope: SyncScope): Promise<number> => {
    const rows = await sql`
      SELECT last_version FROM khala_sync_scopes WHERE scope = ${scope}
    `
    return rows.length === 0 ? 0 : Number(rows[0].last_version)
  }

  test("sequential transactions get dense monotonic versions per scope", async () => {
    const scope = freshScope()
    const versions: Array<SyncVersion> = []
    for (let i = 1; i <= 5; i++) {
      const entry = await withSyncTransaction(sql, (writer) =>
        writer.appendChange({
          scope,
          entityType,
          entityId: entityId(i),
          op: "upsert",
          postImage: { n: i },
        }),
      )
      versions.push(entry.version)
    }
    expect(versions.map(Number)).toEqual([1, 2, 3, 4, 5])
    expect(await lastVersion(scope)).toBe(5)
    expect((await changelogRows(scope)).length).toBe(5)
  })

  test("one transaction reuses ONE version per scope across appends", async () => {
    const scope = freshScope()
    const [a, b, allocated] = await withSyncTransaction(sql, async (writer) => {
      const first = await writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { n: 1 },
      })
      const second = await writer.appendChange({
        scope,
        entityType,
        entityId: entityId(2),
        op: "upsert",
        postImage: { n: 2 },
      })
      return [first, second, await writer.allocateVersion(scope)] as const
    })
    expect(Number(a.version)).toBe(1)
    expect(Number(b.version)).toBe(1)
    expect(Number(allocated)).toBe(1)
    expect(await lastVersion(scope)).toBe(1)
    expect((await changelogRows(scope)).length).toBe(2)
  })

  test("re-appending the same entity in one transaction keeps one row, last write wins", async () => {
    const scope = freshScope()
    await withSyncTransaction(sql, async (writer) => {
      await writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { state: "first" },
      })
      await writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "delete",
      })
    })
    const rows = await changelogRows(scope)
    expect(rows.length).toBe(1)
    expect(rows[0]?.op).toBe("delete")
    expect(rows[0]?.post_image_json).toBeNull()
  })

  test("CONCURRENT writers on one scope serialize into dense versions with no gaps or duplicates", async () => {
    const scope = freshScope()
    const writers = 8
    const versions = await Promise.all(
      Array.from({ length: writers }, (_, i) =>
        withSyncTransaction(sql, async (writer) => {
          const entry = await writer.appendChange({
            scope,
            entityType,
            entityId: entityId(i),
            op: "upsert",
            postImage: { writer: i },
          })
          // Hold the transaction (and the scope-counter row lock) open so
          // writers genuinely overlap.
          await Bun.sleep(25)
          return Number(entry.version)
        }),
      ),
    )
    const sorted = [...versions].sort((x, y) => x - y)
    expect(sorted).toEqual(Array.from({ length: writers }, (_, i) => i + 1))
    expect(new Set(versions).size).toBe(writers)
    const rows = await changelogRows(scope)
    expect(rows.length).toBe(writers)
    expect(rows.map((r) => Number(r.version)).sort((x, y) => x - y)).toEqual(sorted)
    expect(await lastVersion(scope)).toBe(writers)
  })

  test("rollback discards the allocated version — the next commit stays dense", async () => {
    const scope = freshScope()
    const first = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { n: 1 },
      }),
    )
    expect(Number(first.version)).toBe(1)

    class DomainRollback extends Error {}
    let seenVersionInAborted = 0
    await expect(
      withSyncTransaction(sql, async (writer) => {
        const entry = await writer.appendChange({
          scope,
          entityType,
          entityId: entityId(2),
          op: "upsert",
          postImage: { n: 2 },
        })
        seenVersionInAborted = Number(entry.version)
        // Business write in the same transaction, also rolled back.
        await writer.sql`
          INSERT INTO khala_sync_client_state (client_group_id, user_id, schema_version)
          VALUES ('rollback-test-group', 'user-1', 1)
        `
        throw new DomainRollback("intentional rollback")
      }),
    ).rejects.toBeInstanceOf(DomainRollback)
    expect(seenVersionInAborted).toBe(2)

    // Counter rolled back with the transaction: no gap for the next commit.
    expect(await lastVersion(scope)).toBe(1)
    const next = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: entityId(3),
        op: "upsert",
        postImage: { n: 3 },
      }),
    )
    expect(Number(next.version)).toBe(2)

    const rows = await changelogRows(scope)
    expect(rows.map((r) => Number(r.version))).toEqual([1, 2])
    expect(rows.map((r) => r.entity_id)).toEqual(["thing-1", "thing-3"])
    const state = await sql`
      SELECT count(*)::int AS c FROM khala_sync_client_state
      WHERE client_group_id = 'rollback-test-group'
    `
    expect(state[0].c).toBe(0)
  })

  test("multiple scopes in ONE transaction get independent versions", async () => {
    const scopeA = freshScope()
    const scopeB = freshScope()
    // Advance scopeA to version 2 first so the counters visibly differ.
    await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope: scopeA,
        entityType,
        entityId: entityId(0),
        op: "upsert",
        postImage: { seed: true },
      }),
    )
    const [a, b] = await withSyncTransaction(sql, async (writer) => {
      const inA = await writer.appendChange({
        scope: scopeA,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { in: "a" },
      })
      const inB = await writer.appendChange({
        scope: scopeB,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { in: "b" },
      })
      return [inA, inB] as const
    })
    expect(Number(a.version)).toBe(2)
    expect(Number(b.version)).toBe(1)
    expect(await lastVersion(scopeA)).toBe(2)
    expect(await lastVersion(scopeB)).toBe(1)
  })

  test("tombstone rule: the writer rejects a delete carrying a post-image before any SQL", async () => {
    const scope = freshScope()
    await expect(
      withSyncTransaction(sql, (writer) =>
        writer.appendChange({
          scope,
          entityType,
          entityId: entityId(1),
          op: "delete",
          postImage: { must: "not exist" },
        } as unknown as AppendChangeInput),
      ),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
    expect(await lastVersion(scope)).toBe(0)
  })

  test("tombstone rule: an upsert without a post-image is rejected", async () => {
    const scope = freshScope()
    await expect(
      withSyncTransaction(sql, (writer) =>
        writer.appendChange({
          scope,
          entityType,
          entityId: entityId(1),
          op: "upsert",
          postImage: undefined,
        } as unknown as AppendChangeInput),
      ),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
  })

  test("tombstone rule: the DB CHECK constraint backstops raw inserts, mapped to constraint_violation", async () => {
    const scope = freshScope()
    await expect(
      withSyncTransaction(sql, async (writer) => {
        const version = await writer.allocateVersion(scope)
        await writer.sql`
          INSERT INTO khala_sync_changelog
            (scope, version, entity_type, entity_id, op, post_image_json)
          VALUES (${scope}, ${version}, ${entityType}, ${entityId(1)},
                  'delete', '{"sneaky":true}'::jsonb)
        `
      }),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
    // The whole transaction rolled back — counter included.
    expect(await lastVersion(scope)).toBe(0)
  })

  test("non-canonical-JSON post-images are rejected as constraint_violation", async () => {
    const scope = freshScope()
    await expect(
      withSyncTransaction(sql, (writer) =>
        writer.appendChange({
          scope,
          entityType,
          entityId: entityId(1),
          op: "upsert",
          postImage: { bad: () => "function" },
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "KhalaSyncStorageError",
      reason: "constraint_violation",
    })
  })

  test("changelog rows round-trip through the khala-sync ChangelogEntry codec", async () => {
    const scope = freshScope()
    const postImage = { z: "last", a: [1, 2, { nested: true }], m: -0, s: "héllo" }
    const written = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage,
        mutationRef: "mutation:test:1",
      }),
    )
    const deleted = await withSyncTransaction(sql, (writer) =>
      writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "delete",
      }),
    )

    // Post-images are canonical JSON: sorted keys, -0 normalized, no spaces.
    expect(written.postImageJson).toBe(canonicalJson(postImage))
    expect(written.postImageJson).toBe(
      '{"a":[1,2,{"nested":true}],"m":0,"s":"héllo","z":"last"}',
    )
    expect(deleted.postImageJson).toBeUndefined()
    expect(deleted.mutationRef).toBeUndefined()

    const rows = await changelogRows(scope)
    expect(rows.length).toBe(2)
    const fromRows = rows.map(changelogEntryFromRow)

    // Row-decoded entries match what appendChange returned, byte for byte
    // through the wire codec.
    expect(fromRows.map((e) => encodeChangelogEntry(e))).toEqual(
      [written, deleted].map((e) => encodeChangelogEntry(e)),
    )
    // And the codec round-trips: encode → decode → encode is stable.
    for (const entry of fromRows) {
      const encoded = encodeChangelogEntry(entry)
      expect(encodeChangelogEntry(decodeChangelogEntry(encoded))).toEqual(encoded)
      expect(entry.scope).toBe(scope)
      expect(Number.isInteger(entry.version)).toBe(true)
      expect(new Date(entry.committedAt).toISOString()).toBe(entry.committedAt)
    }
  })

  test("withSyncTransaction returns the callback's value on commit", async () => {
    const scope = freshScope()
    const out = await withSyncTransaction(sql, async (writer) => {
      await writer.appendChange({
        scope,
        entityType,
        entityId: entityId(1),
        op: "upsert",
        postImage: { ok: true },
      })
      return "business-result"
    })
    expect(out).toBe("business-result")
  })
})
