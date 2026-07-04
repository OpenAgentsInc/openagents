import {
  canonicalJson,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { openKhalaSyncStore } from "./sqlite-store.js"
import {
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
} from "./store.js"

const run = <A>(
  effect: Effect.Effect<A, KhalaSyncClientStoreError>,
): A => Effect.runSync(effect)

const runError = <A>(
  effect: Effect.Effect<A, KhalaSyncClientStoreError>,
): KhalaSyncClientStoreError => Effect.runSync(Effect.flip(effect))

const scopeA = SyncScope.make("scope.team.alpha")
const scopeB = SyncScope.make("scope.team.beta")

const upsert = (
  scope: SyncScope,
  version: number,
  entityId: string,
  postImage: unknown,
  entityType = "task",
): ChangelogEntry =>
  new ChangelogEntry({
    scope,
    version: SyncVersion.make(version),
    entityType: EntityType.make(entityType),
    entityId: EntityId.make(entityId),
    op: "upsert",
    postImageJson: canonicalJson(postImage),
    committedAt: "2026-07-04T00:00:00.000Z",
  })

const tombstone = (
  scope: SyncScope,
  version: number,
  entityId: string,
  entityType = "task",
): ChangelogEntry =>
  new ChangelogEntry({
    scope,
    version: SyncVersion.make(version),
    entityType: EntityType.make(entityType),
    entityId: EntityId.make(entityId),
    op: "delete",
    committedAt: "2026-07-04T00:00:00.000Z",
  })

const envelope = (mutationId: number, name = "task.create"): MutationEnvelope =>
  new MutationEnvelope({
    mutationId: MutationId.make(mutationId),
    name: MutatorName.make(name),
    argsJson: canonicalJson({ id: mutationId }),
  })

const snapshotEntity = (
  version: number,
  entityId: string,
  postImage: unknown,
  entityType = "task",
): ConfirmedEntity => ({
  entityType,
  entityId,
  postImageJson: canonicalJson(postImage),
  version: SyncVersion.make(version),
})

const identity = {
  clientId: ClientId.make("client-1"),
  clientGroupId: ClientGroupId.make("group-1"),
  schemaVersion: SyncSchemaVersion.make(1),
}

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

const memoryStore = () => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  return store
}

describe("openKhalaSyncStore / applyConfirmed", () => {
  test("applies upserts + deletes and advances the cursor atomically", () => {
    const store = memoryStore()
    expect(run(store.cursor(scopeA))).toBeNull()

    run(
      store.applyConfirmed(
        scopeA,
        [
          upsert(scopeA, 1, "t1", { title: "one" }),
          upsert(scopeA, 1, "t2", { title: "two" }),
        ],
        SyncVersion.make(1),
      ),
    )
    run(
      store.applyConfirmed(
        scopeA,
        [
          upsert(scopeA, 2, "t1", { title: "one v2" }),
          tombstone(scopeA, 2, "t2"),
        ],
        SyncVersion.make(2),
      ),
    )

    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(2))
    const entities = run(store.readEntities(scopeA))
    expect(entities).toHaveLength(1)
    expect(entities[0]).toEqual({
      entityType: "task",
      entityId: "t1",
      postImageJson: canonicalJson({ title: "one v2" }),
      version: SyncVersion.make(2),
    })
  })

  test("re-applying the same entries + cursor is a no-op end state", () => {
    const store = memoryStore()
    const entries = [
      upsert(scopeA, 3, "t1", { n: 1 }),
      tombstone(scopeA, 3, "t2"),
    ]
    run(store.applyConfirmed(scopeA, entries, SyncVersion.make(3)))
    const before = run(store.readEntities(scopeA))

    // at-least-once redelivery of the identical batch
    run(store.applyConfirmed(scopeA, entries, SyncVersion.make(3)))

    expect(run(store.readEntities(scopeA))).toEqual(before)
    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(3))
  })

  test("skips entries with version <= the stored entity version", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 5, "t1", { latest: true })],
        SyncVersion.make(5),
      ),
    )
    // stale redelivered upsert and stale tombstone must not clobber v5
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 2, "t1", { latest: false }), tombstone(scopeA, 3, "t1")],
        SyncVersion.make(6),
      ),
    )

    const entities = run(store.readEntities(scopeA))
    expect(entities).toHaveLength(1)
    expect(entities[0]!.version).toBe(SyncVersion.make(5))
    expect(entities[0]!.postImageJson).toBe(canonicalJson({ latest: true }))
    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(6))
  })

  test("rejects a cursor lower than the stored cursor, leaving state intact", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 5, "t1", { v: 5 })],
        SyncVersion.make(5),
      ),
    )

    const error = runError(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 3, "t2", { v: 3 })],
        SyncVersion.make(3),
      ),
    )
    expect(error._tag).toBe("KhalaSyncClientStoreError")
    expect(error.reason).toBe("cursor_regression")
    // rolled back: no partial entry, cursor unchanged
    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(5))
    expect(run(store.readEntities(scopeA))).toHaveLength(1)
  })

  test("a failing entry mid-batch rolls back the whole transaction", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 1, "t1", { v: 1 })],
        SyncVersion.make(1),
      ),
    )

    // second entry is an upsert with no post-image → constraint violation
    const broken = new ChangelogEntry({
      scope: scopeA,
      version: SyncVersion.make(2),
      entityType: EntityType.make("task"),
      entityId: EntityId.make("t3"),
      op: "upsert",
      committedAt: "2026-07-04T00:00:00.000Z",
    })
    const error = runError(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 2, "t2", { v: 2 }), broken],
        SyncVersion.make(2),
      ),
    )
    expect(error.reason).toBe("constraint_violation")

    // crash-safety shape: the valid first entry did NOT land, cursor held
    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(1))
    const entities = run(store.readEntities(scopeA))
    expect(entities.map((e) => e.entityId)).toEqual(["t1"])
  })

  test("rejects entries whose scope does not match the applied scope", () => {
    const store = memoryStore()
    const error = runError(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeB, 1, "t1", { v: 1 })],
        SyncVersion.make(1),
      ),
    )
    expect(error.reason).toBe("constraint_violation")
    expect(run(store.cursor(scopeA))).toBeNull()
    expect(run(store.readEntities(scopeA))).toHaveLength(0)
  })

  test("filters readEntities by entity type", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [
          upsert(scopeA, 1, "t1", { kind: "task" }, "task"),
          upsert(scopeA, 1, "r1", { kind: "run" }, "run"),
        ],
        SyncVersion.make(1),
      ),
    )
    expect(run(store.readEntities(scopeA, "run")).map((e) => e.entityId)).toEqual([
      "r1",
    ])
    expect(run(store.readEntities(scopeA))).toHaveLength(2)
  })
})

describe("resetScope (MustRefetch path)", () => {
  test("replaces scope state + cursor in one transaction, other scopes untouched", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 9, "old", { stale: true })],
        SyncVersion.make(9),
      ),
    )
    run(
      store.applyConfirmed(
        scopeB,
        [upsert(scopeB, 4, "keep", { keep: true })],
        SyncVersion.make(4),
      ),
    )

    // snapshot cursor may be lower than the (now-invalid) stored cursor
    run(
      store.resetScope(
        scopeA,
        [snapshotEntity(3, "n1", { fresh: 1 }), snapshotEntity(3, "n2", { fresh: 2 })],
        SyncVersion.make(3),
      ),
    )

    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(3))
    expect(run(store.readEntities(scopeA)).map((e) => e.entityId)).toEqual([
      "n1",
      "n2",
    ])
    // scope B untouched
    expect(run(store.cursor(scopeB))).toBe(SyncVersion.make(4))
    expect(run(store.readEntities(scopeB)).map((e) => e.entityId)).toEqual([
      "keep",
    ])
  })

  test("resets to an empty snapshot", () => {
    const store = memoryStore()
    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 2, "t1", { v: 2 })],
        SyncVersion.make(2),
      ),
    )
    run(store.resetScope(scopeA, [], SyncVersion.make(7)))
    expect(run(store.readEntities(scopeA))).toHaveLength(0)
    expect(run(store.cursor(scopeA))).toBe(SyncVersion.make(7))
  })
})

describe("pending-mutation queue", () => {
  test("enqueues FIFO and requires mutationId = last + 1", () => {
    const store = memoryStore()
    run(store.enqueueMutation(envelope(1)))
    run(store.enqueueMutation(envelope(2)))
    run(store.enqueueMutation(envelope(3)))

    const pending = run(store.pendingMutations())
    expect(pending.map((m) => Number(m.mutationId))).toEqual([1, 2, 3])
    expect(pending[0]).toBeInstanceOf(MutationEnvelope)
    expect(pending[0]!.argsJson).toBe(canonicalJson({ id: 1 }))

    // gap
    expect(runError(store.enqueueMutation(envelope(5))).reason).toBe(
      "mutation_id_gap",
    )
    // duplicate / out-of-order
    expect(runError(store.enqueueMutation(envelope(3))).reason).toBe(
      "mutation_id_gap",
    )
    // failed enqueues left the queue untouched
    expect(run(store.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([
      1, 2, 3,
    ])
  })

  test("ackMutations drops rows through the id; next enqueue continues the sequence", () => {
    const store = memoryStore()
    for (const id of [1, 2, 3]) run(store.enqueueMutation(envelope(id)))

    run(store.ackMutations(MutationId.make(2)))
    expect(run(store.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([3])

    // acked ids stay burned: 1 is not reusable, next must be 4
    expect(runError(store.enqueueMutation(envelope(1))).reason).toBe(
      "mutation_id_gap",
    )
    run(store.enqueueMutation(envelope(4)))
    expect(run(store.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([3, 4])
  })

  test("acking past the local counter advances it (server-ahead ack)", () => {
    const store = memoryStore()
    run(store.enqueueMutation(envelope(1)))
    run(store.ackMutations(MutationId.make(10)))
    expect(run(store.pendingMutations())).toHaveLength(0)
    expect(runError(store.enqueueMutation(envelope(2))).reason).toBe(
      "mutation_id_gap",
    )
    run(store.enqueueMutation(envelope(11)))
    expect(run(store.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([11])
  })

  test("ackMutations on an empty queue is a no-op", () => {
    const store = memoryStore()
    run(store.ackMutations(MutationId.make(1)))
    expect(run(store.pendingMutations())).toHaveLength(0)
  })
})

describe("client identity (meta)", () => {
  test("round-trips and stays null until set", () => {
    const store = memoryStore()
    expect(run(store.identity())).toBeNull()
    run(store.setIdentity(identity))
    expect(run(store.identity())).toEqual(identity)
    // idempotent for equal values
    run(store.setIdentity(identity))
    expect(run(store.identity())).toEqual(identity)
  })

  test("rejects a conflicting identity rewrite", () => {
    const store = memoryStore()
    run(store.setIdentity(identity))
    const error = runError(
      store.setIdentity({ ...identity, clientId: ClientId.make("client-2") }),
    )
    expect(error.reason).toBe("constraint_violation")
    expect(run(store.identity())).toEqual(identity)
  })
})

describe("file-backed store", () => {
  test("WAL journaling is on; state persists across close/reopen", () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-sync-store-"))
    cleanups.push(() => rmSync(dir, { recursive: true, force: true }))
    const path = join(dir, "store.sqlite")

    const store = openKhalaSyncStore(path)
    const raw = new Database(path)
    expect(
      (raw.query("PRAGMA journal_mode").get() as { journal_mode: string })
        .journal_mode,
    ).toBe("wal")
    raw.close()

    run(
      store.applyConfirmed(
        scopeA,
        [upsert(scopeA, 2, "t1", { durable: true }), tombstone(scopeA, 2, "t2")],
        SyncVersion.make(2),
      ),
    )
    run(store.enqueueMutation(envelope(1)))
    run(store.enqueueMutation(envelope(2)))
    run(store.ackMutations(MutationId.make(1)))
    run(store.setIdentity(identity))
    run(store.close())

    const reopened = openKhalaSyncStore(path)
    cleanups.push(() => Effect.runSync(Effect.ignore(reopened.close())))
    expect(run(reopened.cursor(scopeA))).toBe(SyncVersion.make(2))
    expect(run(reopened.readEntities(scopeA))).toEqual([
      {
        entityType: "task",
        entityId: "t1",
        postImageJson: canonicalJson({ durable: true }),
        version: SyncVersion.make(2),
      },
    ])
    expect(run(reopened.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([2])
    expect(run(reopened.identity())).toEqual(identity)
    // counter survived: next enqueue is 3, not 1
    expect(runError(reopened.enqueueMutation(envelope(1))).reason).toBe(
      "mutation_id_gap",
    )
    run(reopened.enqueueMutation(envelope(3)))
  })

  test("operations after close fail with a typed storage error", () => {
    const store = openKhalaSyncStore(":memory:")
    run(store.close())
    const error = runError(store.cursor(scopeA))
    expect(error._tag).toBe("KhalaSyncClientStoreError")
    expect(error.reason).toBe("storage_failure")
  })
})
