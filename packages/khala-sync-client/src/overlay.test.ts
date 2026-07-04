import {
  canonicalJson,
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationId,
  MutatorName,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  type ClientMutator,
  createOverlay,
  type KhalaSyncOverlay,
  KhalaSyncOverlayError,
  type OverlayEffect,
  type OverlayError,
  type OverlayView,
} from "./overlay.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

const run = <A>(effect: Effect.Effect<A, OverlayError>): A =>
  Effect.runSync(effect)

const runError = <A>(effect: Effect.Effect<A, OverlayError>): OverlayError =>
  Effect.runSync(Effect.flip(effect))

const scopeA = SyncScope.make("scope.team.alpha")
const scopeB = SyncScope.make("scope.team.beta")

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

const memoryStore = () => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  return store
}

// -- test mutators -----------------------------------------------------------

interface SetArgs {
  readonly scope: string
  readonly id: string
  readonly value: number
}

const setTask: ClientMutator<SetArgs> = {
  name: MutatorName.make("task.set"),
  apply: (args) => [
    {
      kind: "upsert",
      scope: SyncScope.make(args.scope),
      entityType: "task",
      entityId: args.id,
      postImageJson: canonicalJson({ value: args.value }),
    },
  ],
}

interface IncArgs {
  readonly scope: string
  readonly id: string
  readonly by: number
}

/** Replay-sensitive: reads the current view — rebase changes its output. */
const incrementTask: ClientMutator<IncArgs> = {
  name: MutatorName.make("task.increment"),
  apply: (args, view) => {
    const scope = SyncScope.make(args.scope)
    const current = view.get(scope, "task", args.id)
    const base =
      current === undefined
        ? 0
        : (JSON.parse(current) as { value: number }).value
    return [
      {
        kind: "upsert",
        scope,
        entityType: "task",
        entityId: args.id,
        postImageJson: canonicalJson({ value: base + args.by }),
      },
    ]
  },
}

interface RemoveArgs {
  readonly scope: string
  readonly id: string
}

const removeTask: ClientMutator<RemoveArgs> = {
  name: MutatorName.make("task.remove"),
  apply: (args) => [
    {
      kind: "delete",
      scope: SyncScope.make(args.scope),
      entityType: "task",
      entityId: args.id,
    },
  ],
}

/**
 * Replay-sensitive pair write: bumps BOTH `<id>-a` and `<id>-b` to
 * (current of `<id>-a`) + by. Every consistent view keeps the pair equal —
 * a half-rebased view would show them diverged.
 */
const incrementPair: ClientMutator<IncArgs> = {
  name: MutatorName.make("task.incrementPair"),
  apply: (args, view) => {
    const scope = SyncScope.make(args.scope)
    const current = view.get(scope, "task", `${args.id}-a`)
    const base =
      current === undefined
        ? 0
        : (JSON.parse(current) as { value: number }).value
    const effect = (id: string): OverlayEffect => ({
      kind: "upsert",
      scope,
      entityType: "task",
      entityId: id,
      postImageJson: canonicalJson({ value: base + args.by }),
    })
    return [effect(`${args.id}-a`), effect(`${args.id}-b`)]
  },
}

const allMutators = [setTask, incrementTask, removeTask, incrementPair]

const confirmedUpsert = (
  scope: SyncScope,
  version: number,
  entityId: string,
  value: number,
): ChangelogEntry =>
  new ChangelogEntry({
    scope,
    version: SyncVersion.make(version),
    entityType: EntityType.make("task"),
    entityId: EntityId.make(entityId),
    op: "upsert",
    postImageJson: canonicalJson({ value }),
    committedAt: "2026-07-04T00:00:00.000Z",
  })

const values = (view: OverlayView): Record<string, number> =>
  Object.fromEntries(
    view
      .list("task")
      .map((e) => [e.entityId, (JSON.parse(e.postImageJson) as { value: number }).value]),
  )

const makeOverlay = (
  store = memoryStore(),
): { store: ReturnType<typeof memoryStore>; overlay: KhalaSyncOverlay } => ({
  store,
  overlay: run(createOverlay(store, allMutators)),
})

// ---------------------------------------------------------------------------

describe("createOverlay / mutate", () => {
  test("optimistic effects are visible in the view but never in the durable store (invariant 2)", () => {
    const { store, overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))

    const id1 = run(overlay.mutate(setTask, { scope: scopeA, id: "t1", value: 1 }))
    const id2 = run(
      overlay.mutate(incrementTask, { scope: scopeA, id: "t1", by: 2 }),
    )
    expect(Number(id1)).toBe(1)
    expect(Number(id2)).toBe(2)

    expect(values(view)).toEqual({ t1: 3 })
    expect(view.get("task", "t1")).toBe(canonicalJson({ value: 3 }))
    // durable store: intent queued, entities/cursors untouched
    expect(run(store.readEntities(scopeA))).toHaveLength(0)
    expect(run(store.cursor(scopeA))).toBeNull()
    expect(run(store.pendingMutations()).map((m) => Number(m.mutationId))).toEqual([
      1, 2,
    ])
    expect(overlay.pending().map((m) => Number(m.mutationId))).toEqual([1, 2])
  })

  test("delete effects shadow confirmed entities", () => {
    const { overlay } = makeOverlay()
    run(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "t1", 7)],
        SyncVersion.make(1),
      ),
    )
    const view = run(overlay.read(scopeA))
    expect(values(view)).toEqual({ t1: 7 })

    run(overlay.mutate(removeTask, { scope: scopeA, id: "t1" }))
    expect(values(view)).toEqual({})
    expect(view.get("task", "t1")).toBeUndefined()
  })

  test("rejects an unregistered mutator, typed", () => {
    const { overlay } = makeOverlay()
    const rogue: ClientMutator<SetArgs> = { ...setTask }
    const error = runError(
      overlay.mutate(rogue, { scope: scopeA, id: "t1", value: 1 }),
    )
    expect(error).toBeInstanceOf(KhalaSyncOverlayError)
    expect((error as KhalaSyncOverlayError).reason).toBe("unknown_mutator")
  })

  test("rejects non-canonicalizable args typed; nothing is queued or applied", () => {
    const { store, overlay } = makeOverlay()
    const error = runError(
      overlay.mutate(setTask, {
        scope: scopeA,
        id: "t1",
        value: Number.POSITIVE_INFINITY,
      }),
    )
    expect((error as KhalaSyncOverlayError).reason).toBe("invalid_args")
    expect(run(store.pendingMutations())).toHaveLength(0)
    expect(values(run(overlay.read(scopeA)))).toEqual({})
  })

  test("a throwing mutator fails typed without queueing", () => {
    const store = memoryStore()
    const boom: ClientMutator<Record<string, never>> = {
      name: MutatorName.make("task.boom"),
      apply: () => {
        throw new Error("boom")
      },
    }
    const overlay = run(createOverlay(store, [boom]))
    const error = runError(overlay.mutate(boom, {}))
    expect((error as KhalaSyncOverlayError).reason).toBe("mutator_failure")
    expect(run(store.pendingMutations())).toHaveLength(0)
  })

  test("duplicate mutator registration fails typed", () => {
    const store = memoryStore()
    const error = runError(createOverlay(store, [setTask, setTask]))
    expect((error as KhalaSyncOverlayError).reason).toBe("duplicate_mutator")
  })
})

describe("rebase on onConfirmed", () => {
  test("replay against changed confirmed state produces different effects (replay-safety)", () => {
    const { overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))

    // optimistic increment over empty confirmed state: 0 + 1 = 1
    run(overlay.mutate(incrementTask, { scope: scopeA, id: "t1", by: 1 }))
    expect(values(view)).toEqual({ t1: 1 })

    // foreign confirmed write lands t1 = 10 → rebase replays the pending
    // increment against the NEW confirmed view: 10 + 1 = 11
    run(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "t1", 10)],
        SyncVersion.make(1),
      ),
    )
    expect(values(view)).toEqual({ t1: 11 })
  })

  test("confirm-then-ack: own mutation confirmed while pending, then acked", () => {
    const { store, overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))

    run(overlay.mutate(setTask, { scope: scopeA, id: "t1", value: 5 }))
    expect(values(view)).toEqual({ t1: 5 })

    // server confirms the mutation's outcome before the ack frame arrives;
    // the still-pending mutation replays on top (same value — set is stable)
    run(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "t1", 5)],
        SyncVersion.make(1),
      ),
    )
    expect(values(view)).toEqual({ t1: 5 })
    expect(run(store.readEntities(scopeA))).toHaveLength(1)

    run(overlay.onAck(MutationId.make(1)))
    expect(values(view)).toEqual({ t1: 5 })
    expect(overlay.pending()).toHaveLength(0)
    expect(run(store.pendingMutations())).toHaveLength(0)
  })

  test("ack-then-confirm: contribution drops on ack, returns as confirmed state", () => {
    const { store, overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))

    run(overlay.mutate(setTask, { scope: scopeA, id: "t1", value: 5 }))
    expect(values(view)).toEqual({ t1: 5 })

    // ack first: the optimistic contribution vanishes (server outcome wins;
    // its confirmed entry is still in flight)
    run(overlay.onAck(MutationId.make(1)))
    expect(values(view)).toEqual({})
    expect(overlay.pending()).toHaveLength(0)

    // the confirmed delta lands: state returns, now server-confirmed
    run(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "t1", 5)],
        SyncVersion.make(1),
      ),
    )
    expect(values(view)).toEqual({ t1: 5 })
    expect(run(store.readEntities(scopeA))).toHaveLength(1)
  })

  test("redelivered confirm batch changes nothing and notifies nothing", () => {
    const { overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))
    const entries = [confirmedUpsert(scopeA, 1, "t1", 7)]
    run(overlay.onConfirmed(scopeA, entries, SyncVersion.make(1)))

    const notified: Array<string> = []
    cleanups.push(overlay.subscribe((scope) => notified.push(scope)))
    run(overlay.onConfirmed(scopeA, entries, SyncVersion.make(1)))
    expect(values(view)).toEqual({ t1: 7 })
    expect(notified).toEqual([])
  })
})

describe("reveal atomicity", () => {
  test("a subscriber observes either the pre- or post-rebase view, never partial", () => {
    const { overlay } = makeOverlay()
    const view = run(overlay.read(scopeA))

    // pending pair-write keeps p-a and p-b equal in every consistent view
    run(overlay.mutate(incrementPair, { scope: scopeA, id: "p", by: 1 }))
    expect(values(view)).toEqual({ "p-a": 1, "p-b": 1 })
    const observed: Array<Record<string, number>> = []
    cleanups.push(
      overlay.subscribe(() => {
        observed.push(values(view))
      }),
    )

    // confirmed batch moves both entities; rebase replays the pair-write
    run(
      overlay.onConfirmed(
        scopeA,
        [
          confirmedUpsert(scopeA, 1, "p-a", 9),
          confirmedUpsert(scopeA, 1, "p-b", 9),
        ],
        SyncVersion.make(1),
      ),
    )

    expect(observed.length).toBeGreaterThan(0)
    for (const snapshot of observed) {
      // never a half-rebased view: the pair never diverges
      expect(snapshot["p-a"]).toBe(snapshot["p-b"]!)
    }
    // post state: pending pair-write replayed over the confirmed 9s
    expect(values(view)).toEqual({ "p-a": 10, "p-b": 10 })
  })

  test("a failing replay keeps the previous consistent snapshot revealed", () => {
    const store = memoryStore()
    // throws only when the confirmed marker entity exists — i.e. only on
    // the rebase replay, not on the first optimistic apply
    const fragile: ClientMutator<SetArgs> = {
      name: MutatorName.make("task.fragile"),
      apply: (args, view) => {
        if (view.get(scopeA, "task", "marker") !== undefined) {
          throw new Error("replay explosion")
        }
        return setTask.apply(args, view)
      },
    }
    const overlay = run(createOverlay(store, [fragile]))
    const view = run(overlay.read(scopeA))
    run(overlay.mutate(fragile, { scope: scopeA, id: "t1", value: 5 }))
    expect(values(view)).toEqual({ t1: 5 })

    const error = runError(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "marker", 1)],
        SyncVersion.make(1),
      ),
    )
    expect((error as KhalaSyncOverlayError).reason).toBe("mutator_failure")
    // durable store took the confirmed delta (it committed first) …
    expect(run(store.readEntities(scopeA)).map((e) => e.entityId)).toEqual([
      "marker",
    ])
    // … but the revealed overlay snapshot is still the previous consistent
    // one — not a partial rebase
    expect(values(view)).toEqual({ t1: 5 })
  })
})

describe("subscriptions", () => {
  test("notifies changed scopes on mutate/confirm; unsubscribe stops delivery", () => {
    const { overlay } = makeOverlay()
    run(overlay.read(scopeA))
    run(overlay.read(scopeB))

    const notified: Array<string> = []
    const unsubscribe = overlay.subscribe((scope) => notified.push(scope))
    run(overlay.mutate(setTask, { scope: scopeA, id: "t1", value: 1 }))
    expect(notified).toEqual([scopeA])

    run(
      overlay.onConfirmed(
        scopeB,
        [confirmedUpsert(scopeB, 1, "b1", 2)],
        SyncVersion.make(1),
      ),
    )
    expect(notified).toEqual([scopeA, scopeB])

    unsubscribe()
    run(overlay.mutate(setTask, { scope: scopeA, id: "t2", value: 2 }))
    expect(notified).toEqual([scopeA, scopeB])
  })
})

describe("restart", () => {
  test("pending mutations that survived restart rebuild their optimistic view", () => {
    const dirStore = memoryStore()
    const overlay = run(createOverlay(dirStore, allMutators))
    run(overlay.read(scopeA))
    run(
      overlay.onConfirmed(
        scopeA,
        [confirmedUpsert(scopeA, 1, "t1", 10)],
        SyncVersion.make(1),
      ),
    )
    run(overlay.mutate(incrementTask, { scope: scopeA, id: "t1", by: 1 }))
    run(overlay.mutate(setTask, { scope: scopeA, id: "t2", value: 2 }))

    // "restart": a fresh overlay over the same store rebuilds the queue's
    // optimistic effects from the durable intent alone
    const reborn = run(createOverlay(dirStore, allMutators))
    const view = run(reborn.read(scopeA))
    expect(values(view)).toEqual({ t1: 11, t2: 2 })
    expect(reborn.pending().map((m) => Number(m.mutationId))).toEqual([1, 2])
    // ids continue after the survivors
    const next = run(reborn.mutate(setTask, { scope: scopeA, id: "t3", value: 3 }))
    expect(Number(next)).toBe(3)
  })

  test("a queued mutation whose mutator is not registered fails rebuild typed", () => {
    const store = memoryStore()
    const overlay = run(createOverlay(store, allMutators))
    run(overlay.mutate(setTask, { scope: scopeA, id: "t1", value: 1 }))

    const error = runError(createOverlay(store, [incrementTask]))
    expect((error as KhalaSyncOverlayError).reason).toBe("unknown_mutator")
  })
})
