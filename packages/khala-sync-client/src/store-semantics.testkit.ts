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
  SyncVersionWatermark,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  type ConfirmedEntity,
  KhalaSyncClientStoreError,
  type KhalaSyncLocalStore,
} from "./store.js"

/**
 * The shared {@link KhalaSyncLocalStore} SEMANTICS suite (KS-5.1/KS-5.4):
 * one parameterized set of contract tests that every store adapter must
 * pass — the `bun:sqlite` desktop store, the driver-agnostic SQL core,
 * and the web proxy → RPC → storage-worker → core pipeline.
 *
 * Covers: idempotent applyConfirmed under at-least-once redelivery,
 * skip-stale entity versions, cursor monotonicity (`cursor_regression`),
 * whole-batch rollback, scope isolation, resetScope (including the
 * watermark-0 "scope start" cursor clear), the FIFO mutation queue with
 * gap rejection and burned ids, server-ahead acks, and identity meta.
 *
 * Not a `.test.ts` file — imported by concrete adapter test files.
 */

export interface StoreHarness {
  readonly store: KhalaSyncLocalStore
  readonly cleanup?: () => void | Promise<void>
}

export type MakeStore = () => StoreHarness | Promise<StoreHarness>

const run = <A>(
  effect: Effect.Effect<A, KhalaSyncClientStoreError>,
): Promise<A> => Effect.runPromise(effect)

const runError = <A>(
  effect: Effect.Effect<A, KhalaSyncClientStoreError>,
): Promise<KhalaSyncClientStoreError> => Effect.runPromise(Effect.flip(effect))

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

/**
 * Register the shared semantics suite under `describe(suiteName)` against
 * stores built by `makeStore` (a fresh, empty store per test).
 */
export const describeKhalaSyncStoreSemantics = (
  suiteName: string,
  makeStore: MakeStore,
): void => {
  describe(suiteName, () => {
    const cleanups: Array<() => void | Promise<void>> = []
    afterEach(async () => {
      while (cleanups.length > 0) await cleanups.pop()!()
    })

    const open = async (): Promise<KhalaSyncLocalStore> => {
      const harness = await makeStore()
      if (harness.cleanup !== undefined) cleanups.push(harness.cleanup)
      return harness.store
    }

    describe("applyConfirmed", () => {
      test("applies upserts + deletes and advances the cursor atomically", async () => {
        const store = await open()
        expect(await run(store.cursor(scopeA))).toBeNull()

        await run(
          store.applyConfirmed(
            scopeA,
            [
              upsert(scopeA, 1, "t1", { title: "one" }),
              upsert(scopeA, 1, "t2", { title: "two" }),
            ],
            SyncVersion.make(1),
          ),
        )
        await run(
          store.applyConfirmed(
            scopeA,
            [
              upsert(scopeA, 2, "t1", { title: "one v2" }),
              tombstone(scopeA, 2, "t2"),
            ],
            SyncVersion.make(2),
          ),
        )

        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(2))
        const entities = await run(store.readEntities(scopeA))
        expect(entities).toHaveLength(1)
        expect(entities[0]).toEqual({
          entityType: "task",
          entityId: "t1",
          postImageJson: canonicalJson({ title: "one v2" }),
          version: SyncVersion.make(2),
        })
      })

      test("re-applying the same entries + cursor is a no-op end state", async () => {
        const store = await open()
        const entries = [
          upsert(scopeA, 3, "t1", { n: 1 }),
          tombstone(scopeA, 3, "t2"),
        ]
        await run(store.applyConfirmed(scopeA, entries, SyncVersion.make(3)))
        const before = await run(store.readEntities(scopeA))

        // at-least-once redelivery of the identical batch
        await run(store.applyConfirmed(scopeA, entries, SyncVersion.make(3)))

        expect(await run(store.readEntities(scopeA))).toEqual(before)
        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(3))
      })

      test("skips entries with version <= the stored entity version", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 5, "t1", { latest: true })],
            SyncVersion.make(5),
          ),
        )
        // stale redelivered upsert and stale tombstone must not clobber v5
        await run(
          store.applyConfirmed(
            scopeA,
            [
              upsert(scopeA, 2, "t1", { latest: false }),
              tombstone(scopeA, 3, "t1"),
            ],
            SyncVersion.make(6),
          ),
        )

        const entities = await run(store.readEntities(scopeA))
        expect(entities).toHaveLength(1)
        expect(entities[0]!.version).toBe(SyncVersion.make(5))
        expect(entities[0]!.postImageJson).toBe(canonicalJson({ latest: true }))
        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(6))
      })

      test("rejects a cursor lower than the stored cursor, leaving state intact", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 5, "t1", { v: 5 })],
            SyncVersion.make(5),
          ),
        )

        const error = await runError(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 3, "t2", { v: 3 })],
            SyncVersion.make(3),
          ),
        )
        expect(error._tag).toBe("KhalaSyncClientStoreError")
        expect(error.reason).toBe("cursor_regression")
        // rolled back: no partial entry, cursor unchanged
        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(5))
        expect(await run(store.readEntities(scopeA))).toHaveLength(1)
      })

      test("a failing entry mid-batch rolls back the whole transaction", async () => {
        const store = await open()
        await run(
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
        const error = await runError(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 2, "t2", { v: 2 }), broken],
            SyncVersion.make(2),
          ),
        )
        expect(error.reason).toBe("constraint_violation")

        // crash-safety shape: the valid first entry did NOT land, cursor held
        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(1))
        const entities = await run(store.readEntities(scopeA))
        expect(entities.map((e) => e.entityId)).toEqual(["t1"])
      })

      test("rejects entries whose scope does not match the applied scope", async () => {
        const store = await open()
        const error = await runError(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeB, 1, "t1", { v: 1 })],
            SyncVersion.make(1),
          ),
        )
        expect(error.reason).toBe("constraint_violation")
        expect(await run(store.cursor(scopeA))).toBeNull()
        expect(await run(store.readEntities(scopeA))).toHaveLength(0)
      })

      test("filters readEntities by entity type", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [
              upsert(scopeA, 1, "t1", { kind: "task" }, "task"),
              upsert(scopeA, 1, "r1", { kind: "run" }, "run"),
            ],
            SyncVersion.make(1),
          ),
        )
        expect(
          (await run(store.readEntities(scopeA, "run"))).map((e) => e.entityId),
        ).toEqual(["r1"])
        expect(await run(store.readEntities(scopeA))).toHaveLength(2)
      })
    })

    describe("resetScope (MustRefetch path)", () => {
      test("replaces scope state + cursor in one transaction, other scopes untouched", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 9, "old", { stale: true })],
            SyncVersion.make(9),
          ),
        )
        await run(
          store.applyConfirmed(
            scopeB,
            [upsert(scopeB, 4, "keep", { keep: true })],
            SyncVersion.make(4),
          ),
        )

        // snapshot cursor may be lower than the (now-invalid) stored cursor
        await run(
          store.resetScope(
            scopeA,
            [
              snapshotEntity(3, "n1", { fresh: 1 }),
              snapshotEntity(3, "n2", { fresh: 2 }),
            ],
            SyncVersion.make(3),
          ),
        )

        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(3))
        expect(
          (await run(store.readEntities(scopeA))).map((e) => e.entityId),
        ).toEqual(["n1", "n2"])
        // scope B untouched
        expect(await run(store.cursor(scopeB))).toBe(SyncVersion.make(4))
        expect(
          (await run(store.readEntities(scopeB))).map((e) => e.entityId),
        ).toEqual(["keep"])
      })

      test("resets to an empty snapshot", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 2, "t1", { v: 2 })],
            SyncVersion.make(2),
          ),
        )
        await run(store.resetScope(scopeA, [], SyncVersion.make(7)))
        expect(await run(store.readEntities(scopeA))).toHaveLength(0)
        expect(await run(store.cursor(scopeA))).toBe(SyncVersion.make(7))
      })

      test("watermark 0 clears the cursor back to never-synced", async () => {
        const store = await open()
        await run(
          store.applyConfirmed(
            scopeA,
            [upsert(scopeA, 2, "t1", { v: 2 })],
            SyncVersion.make(2),
          ),
        )
        await run(store.resetScope(scopeA, [], SyncVersionWatermark.make(0)))
        expect(await run(store.readEntities(scopeA))).toHaveLength(0)
        // cleared, never stored as 0
        expect(await run(store.cursor(scopeA))).toBeNull()
      })
    })

    describe("pending-mutation queue", () => {
      test("enqueues FIFO and requires mutationId = last + 1", async () => {
        const store = await open()
        await run(store.enqueueMutation(envelope(1)))
        await run(store.enqueueMutation(envelope(2)))
        await run(store.enqueueMutation(envelope(3)))

        const pending = await run(store.pendingMutations())
        expect(pending.map((m) => Number(m.mutationId))).toEqual([1, 2, 3])
        expect(pending[0]).toBeInstanceOf(MutationEnvelope)
        expect(pending[0]!.argsJson).toBe(canonicalJson({ id: 1 }))

        // gap
        expect((await runError(store.enqueueMutation(envelope(5)))).reason).toBe(
          "mutation_id_gap",
        )
        // duplicate / out-of-order
        expect((await runError(store.enqueueMutation(envelope(3)))).reason).toBe(
          "mutation_id_gap",
        )
        // failed enqueues left the queue untouched
        expect(
          (await run(store.pendingMutations())).map((m) => Number(m.mutationId)),
        ).toEqual([1, 2, 3])
      })

      test("ackMutations drops rows through the id; next enqueue continues the sequence", async () => {
        const store = await open()
        for (const id of [1, 2, 3]) await run(store.enqueueMutation(envelope(id)))

        await run(store.ackMutations(MutationId.make(2)))
        expect(
          (await run(store.pendingMutations())).map((m) => Number(m.mutationId)),
        ).toEqual([3])

        // acked ids stay burned: 1 is not reusable, next must be 4
        expect((await runError(store.enqueueMutation(envelope(1)))).reason).toBe(
          "mutation_id_gap",
        )
        await run(store.enqueueMutation(envelope(4)))
        expect(
          (await run(store.pendingMutations())).map((m) => Number(m.mutationId)),
        ).toEqual([3, 4])
      })

      test("acking past the local counter advances it (server-ahead ack)", async () => {
        const store = await open()
        await run(store.enqueueMutation(envelope(1)))
        await run(store.ackMutations(MutationId.make(10)))
        expect(await run(store.pendingMutations())).toHaveLength(0)
        expect((await runError(store.enqueueMutation(envelope(2)))).reason).toBe(
          "mutation_id_gap",
        )
        await run(store.enqueueMutation(envelope(11)))
        expect(
          (await run(store.pendingMutations())).map((m) => Number(m.mutationId)),
        ).toEqual([11])
      })

      test("ackMutations on an empty queue is a no-op", async () => {
        const store = await open()
        await run(store.ackMutations(MutationId.make(1)))
        expect(await run(store.pendingMutations())).toHaveLength(0)
      })

      test("lastMutationId is null before the first enqueue and tracks acks", async () => {
        const store = await open()
        expect(await run(store.lastMutationId())).toBeNull()
        await run(store.enqueueMutation(envelope(1)))
        expect(await run(store.lastMutationId())).toBe(MutationId.make(1))
        await run(store.ackMutations(MutationId.make(4)))
        expect(await run(store.lastMutationId())).toBe(MutationId.make(4))
      })
    })

    describe("client identity (meta)", () => {
      test("round-trips and stays null until set", async () => {
        const store = await open()
        expect(await run(store.identity())).toBeNull()
        await run(store.setIdentity(identity))
        expect(await run(store.identity())).toEqual(identity)
        // idempotent for equal values
        await run(store.setIdentity(identity))
        expect(await run(store.identity())).toEqual(identity)
      })

      test("rejects a conflicting identity rewrite", async () => {
        const store = await open()
        await run(store.setIdentity(identity))
        const error = await runError(
          store.setIdentity({ ...identity, clientId: ClientId.make("client-2") }),
        )
        expect(error.reason).toBe("constraint_violation")
        expect(await run(store.identity())).toEqual(identity)
      })
    })
  })
}
