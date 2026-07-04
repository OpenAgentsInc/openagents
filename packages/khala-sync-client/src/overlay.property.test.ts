import {
  canonicalJson,
  ChangelogEntry,
  EntityId,
  EntityType,
  MutationEnvelope,
  MutationId,
  MutatorName,
  SyncScope,
  SyncVersion,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"
import { Effect } from "effect"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  type ClientMutator,
  createOverlay,
  type OverlayEffect,
  type OverlayError,
  type OverlayReadView,
} from "./overlay.js"
import { openKhalaSyncStore } from "./sqlite-store.js"

/**
 * Model-based property tests for the optimistic overlay + rebase engine
 * (KS-5.2 acceptance; SPEC §8 "Rebase correctness").
 *
 * A seeded random interleaving of client mutates, server processing,
 * confirmed-delta deliveries (own mutations + foreign writes, including
 * at-least-once redeliveries), and acks is driven against a reference
 * model. Asserted:
 *
 *  (a) convergence — once everything is processed, delivered, and acked,
 *      overlay view == durable confirmed view == server state, with zero
 *      optimistic residue;
 *  (b) durability boundary — at EVERY step the durable SQLite tables
 *      (inspected directly through a second connection) contain exactly
 *      the confirmed fold of delivered entries — never optimistic effects
 *      (SPEC §7 invariant 2);
 *  (c) determinism — the same operation sequence yields the same view
 *      after every step.
 */

// ---------------------------------------------------------------------------
// Seeded PRNG (mulberry32)
// ---------------------------------------------------------------------------

const mulberry32 = (seed: number): (() => number) => {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const pick = <T>(rand: () => number, items: ReadonlyArray<T>): T =>
  items[Math.floor(rand() * items.length)]!

const int = (rand: () => number, maxExclusive: number): number =>
  Math.floor(rand() * maxExclusive)

// ---------------------------------------------------------------------------
// Shared mutators (client and server run the SAME pure apply — the server
// model is authoritative because it executes against server state)
// ---------------------------------------------------------------------------

interface SetArgs {
  readonly scope: string
  readonly id: string
  readonly value: number
}
interface IncArgs {
  readonly scope: string
  readonly id: string
  readonly by: number
}
interface RemoveArgs {
  readonly scope: string
  readonly id: string
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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const mutators: ReadonlyArray<ClientMutator<any>> = [
  setTask,
  incrementTask,
  removeTask,
]
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const registry = new Map<string, ClientMutator<any>>(
  mutators.map((m) => [m.name, m]),
)

// ---------------------------------------------------------------------------
// Server reference model
// ---------------------------------------------------------------------------

/** scope → entityId → postImageJson (single entity type "task"). */
type StateMap = Map<string, Map<string, string>>

const viewOver = (state: StateMap): OverlayReadView => ({
  get: (scope, entityType, entityId) =>
    entityType === "task" ? state.get(scope)?.get(entityId) : undefined,
  list: (scope, entityType) =>
    entityType === "task"
      ? [...(state.get(scope) ?? new Map<string, string>())]
          .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
          .map(([entityId, postImageJson]) => ({
            entityType,
            entityId,
            postImageJson,
          }))
      : [],
})

const applyEffects = (
  state: StateMap,
  effects: ReadonlyArray<OverlayEffect>,
): void => {
  for (const effect of effects) {
    let scopeState = state.get(effect.scope)
    if (scopeState === undefined) {
      scopeState = new Map()
      state.set(effect.scope, scopeState)
    }
    if (effect.kind === "upsert") scopeState.set(effect.entityId, effect.postImageJson)
    else scopeState.delete(effect.entityId)
  }
}

class ServerModel {
  readonly state: StateMap = new Map()
  readonly versions = new Map<string, number>()
  readonly logs = new Map<string, Array<ChangelogEntry>>()
  readonly pushQueue: Array<MutationEnvelope> = []
  lastProcessedMutationId = 0

  private appendEntries(effects: ReadonlyArray<OverlayEffect>): void {
    // one version per scope per transaction (SPEC §2.3)
    const versionOf = new Map<string, number>()
    for (const effect of effects) {
      let version = versionOf.get(effect.scope)
      if (version === undefined) {
        version = (this.versions.get(effect.scope) ?? 0) + 1
        this.versions.set(effect.scope, version)
        versionOf.set(effect.scope, version)
      }
      let log = this.logs.get(effect.scope)
      if (log === undefined) {
        log = []
        this.logs.set(effect.scope, log)
      }
      log.push(
        new ChangelogEntry({
          scope: SyncScope.make(effect.scope),
          version: SyncVersion.make(version),
          entityType: EntityType.make("task"),
          entityId: EntityId.make(effect.entityId),
          op: effect.kind === "upsert" ? "upsert" : "delete",
          ...(effect.kind === "upsert"
            ? { postImageJson: effect.postImageJson }
            : {}),
          committedAt: "2026-07-04T00:00:00.000Z",
        }),
      )
    }
    applyEffects(this.state, effects)
  }

  /** Execute the next pushed client mutation, server-authoritatively. */
  processNext(): void {
    const envelope = this.pushQueue.shift()
    if (envelope === undefined) return
    const mutator = registry.get(envelope.name)!
    const effects = mutator.apply(
      JSON.parse(envelope.argsJson),
      viewOver(this.state),
    )
    this.appendEntries(effects)
    this.lastProcessedMutationId = envelope.mutationId
  }

  /** A write from another client / system writer. */
  foreignWrite(scope: string, id: string, value: number | null): void {
    this.appendEntries([
      value === null
        ? { kind: "delete", scope: SyncScope.make(scope), entityType: "task", entityId: id }
        : {
            kind: "upsert",
            scope: SyncScope.make(scope),
            entityType: "task",
            entityId: id,
            postImageJson: canonicalJson({ value }),
          },
    ])
  }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

const run = <A>(effect: Effect.Effect<A, OverlayError>): A =>
  Effect.runSync(effect)

const scopePool = ["scope.team.alpha", "scope.team.beta"]
const idPool = ["t1", "t2", "t3"]

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

interface ConfirmedRecord {
  readonly postImageJson: string
  readonly version: number
}

/** One full interleaved session for a seed; returns per-step view fingerprints. */
const runSession = (seed: number, steps: number, dbPath: string): Array<string> => {
  const rand = mulberry32(seed)
  const store = openKhalaSyncStore(dbPath)
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const raw = new Database(dbPath, { readonly: true })
  cleanups.push(() => raw.close())
  const overlay = run(createOverlay(store, mutators))
  const server = new ServerModel()

  // reference confirmed fold (what the durable store must equal at all times)
  const confirmed = new Map<string, Map<string, ConfirmedRecord>>()
  const delivered = new Map<string, number>() // scope → delivered version watermark
  const created: Array<MutationEnvelope> = []
  let ackedThrough = 0
  const fingerprints: Array<string> = []

  const views = scopePool.map((scope) => ({
    scope,
    view: run(overlay.read(SyncScope.make(scope))),
  }))

  const foldDelivered = (entries: ReadonlyArray<ChangelogEntry>): void => {
    for (const entry of entries) {
      let scopeState = confirmed.get(entry.scope)
      if (scopeState === undefined) {
        scopeState = new Map()
        confirmed.set(entry.scope, scopeState)
      }
      const existing = scopeState.get(entry.entityId)
      if (existing !== undefined && existing.version >= entry.version) continue
      if (entry.op === "upsert") {
        scopeState.set(entry.entityId, {
          postImageJson: entry.postImageJson!,
          version: entry.version,
        })
      } else {
        scopeState.delete(entry.entityId)
      }
    }
  }

  const deliverUpTo = (scope: string, upper: number): void => {
    const from = delivered.get(scope) ?? 0
    if (upper <= from) return
    const entries = (server.logs.get(scope) ?? []).filter(
      (e) => e.version > from && e.version <= upper,
    )
    run(
      overlay.onConfirmed(SyncScope.make(scope), entries, SyncVersion.make(upper)),
    )
    foldDelivered(entries)
    delivered.set(scope, upper)
  }

  // -- per-step invariant checks --------------------------------------------

  const checkDurableIsConfirmedOnly = (): void => {
    // (b) the durable tables hold exactly the delivered confirmed fold —
    // inspected directly via a second SQLite connection
    const rows = raw
      .query<
        {
          scope: string
          entity_id: string
          post_image_json: string
          version: number
        },
        []
      >(
        `SELECT scope, entity_id, post_image_json, version
         FROM entities ORDER BY scope, entity_type, entity_id`,
      )
      .all()
    const expectedRows = [...confirmed.entries()]
      .flatMap(([scope, scopeState]) =>
        [...scopeState.entries()].map(([entityId, record]) => ({
          scope,
          entity_id: entityId,
          post_image_json: record.postImageJson,
          version: record.version,
        })),
      )
      .sort((a, b) =>
        a.scope === b.scope
          ? a.entity_id < b.entity_id
            ? -1
            : a.entity_id > b.entity_id
              ? 1
              : 0
          : a.scope < b.scope
            ? -1
            : 1,
      )
    expect(rows).toEqual(expectedRows)

    const cursorRows = raw
      .query<{ scope: string; version: number }, []>(
        "SELECT scope, version FROM cursors ORDER BY scope",
      )
      .all()
    const expectedCursors = [...delivered.entries()]
      .map(([scope, version]) => ({ scope, version }))
      .sort((a, b) => (a.scope < b.scope ? -1 : 1))
    expect(cursorRows).toEqual(expectedCursors)

    const pendingRows = raw
      .query<{ mutation_id: number }, []>(
        "SELECT mutation_id FROM pending_mutations ORDER BY mutation_id",
      )
      .all()
      .map((r) => r.mutation_id)
    const expectedPending = created
      .filter((m) => m.mutationId > ackedThrough)
      .map((m) => Number(m.mutationId))
    expect(pendingRows).toEqual(expectedPending)
  }

  const checkOverlayMatchesReference = (): void => {
    // reference optimistic view: confirmed fold ⊕ pending replay in order
    const referenceState: StateMap = new Map()
    for (const [scope, scopeState] of confirmed) {
      referenceState.set(
        scope,
        new Map(
          [...scopeState.entries()].map(([id, record]) => [
            id,
            record.postImageJson,
          ]),
        ),
      )
    }
    const referenceView = viewOver(referenceState)
    for (const envelope of created) {
      if (envelope.mutationId <= ackedThrough) continue
      const mutator = registry.get(envelope.name)!
      applyEffects(
        referenceState,
        mutator.apply(JSON.parse(envelope.argsJson), referenceView),
      )
    }
    for (const { scope, view } of views) {
      expect(view.list("task")).toEqual(
        referenceView.list(SyncScope.make(scope), "task"),
      )
    }
  }

  const fingerprint = (): string =>
    JSON.stringify(views.map(({ view }) => view.list("task")))

  // -- the random interleaving ------------------------------------------------

  for (let step = 0; step < steps; step++) {
    const roll = rand()
    if (roll < 0.35) {
      // client mutate
      const scope = pick(rand, scopePool)
      const id = pick(rand, idPool)
      const which = rand()
      let mutator: ClientMutator<unknown>
      let args: unknown
      if (which < 0.45) {
        mutator = setTask as ClientMutator<unknown>
        args = { scope, id, value: int(rand, 10) }
      } else if (which < 0.85) {
        mutator = incrementTask as ClientMutator<unknown>
        args = { scope, id, by: 1 + int(rand, 3) }
      } else {
        mutator = removeTask as ClientMutator<unknown>
        args = { scope, id }
      }
      const mutationId = run(overlay.mutate(mutator, args))
      const envelope = new MutationEnvelope({
        mutationId,
        name: mutator.name,
        argsJson: canonicalJson(args),
      })
      created.push(envelope)
      server.pushQueue.push(envelope) // simulated push
    } else if (roll < 0.55) {
      server.processNext()
    } else if (roll < 0.78) {
      // deliver a confirmed batch (1-2 versions) for a random scope
      const scope = pick(rand, scopePool)
      const from = delivered.get(scope) ?? 0
      const last = server.versions.get(scope) ?? 0
      deliverUpTo(scope, Math.min(from + 1 + int(rand, 2), last))
    } else if (roll < 0.84) {
      // at-least-once redelivery of an already-delivered tail (equal cursor)
      const scope = pick(rand, scopePool)
      const upTo = delivered.get(scope) ?? 0
      if (upTo > 0) {
        const lower = Math.max(1, upTo - 1)
        const entries = (server.logs.get(scope) ?? []).filter(
          (e) => e.version >= lower && e.version <= upTo,
        )
        run(
          overlay.onConfirmed(
            SyncScope.make(scope),
            entries,
            SyncVersion.make(upTo),
          ),
        )
      }
    } else if (roll < 0.92) {
      // deliver the ack watermark
      if (server.lastProcessedMutationId > ackedThrough) {
        run(overlay.onAck(MutationId.make(server.lastProcessedMutationId)))
        ackedThrough = server.lastProcessedMutationId
      }
    } else {
      // foreign write (another client / system writer)
      const scope = pick(rand, scopePool)
      const id = pick(rand, idPool)
      server.foreignWrite(scope, id, rand() < 0.8 ? int(rand, 100) : null)
    }

    checkDurableIsConfirmedOnly()
    checkOverlayMatchesReference()
    fingerprints.push(fingerprint())
  }

  // -- drain: process, deliver, ack everything --------------------------------

  while (server.pushQueue.length > 0) server.processNext()
  for (const scope of scopePool) {
    deliverUpTo(scope, server.versions.get(scope) ?? 0)
  }
  if (server.lastProcessedMutationId > ackedThrough) {
    run(overlay.onAck(MutationId.make(server.lastProcessedMutationId)))
    ackedThrough = server.lastProcessedMutationId
  }

  // (a) convergence with zero optimistic residue
  expect(overlay.pending()).toHaveLength(0)
  checkDurableIsConfirmedOnly()
  for (const { scope, view } of views) {
    const serverView = viewOver(server.state)
    // overlay view == server state
    expect(view.list("task")).toEqual(
      serverView.list(SyncScope.make(scope), "task"),
    )
    // durable confirmed view == server state
    const stored = run(store.readEntities(SyncScope.make(scope))).map(
      (entity) => ({
        entityType: entity.entityType,
        entityId: entity.entityId,
        postImageJson: entity.postImageJson,
      }),
    )
    expect(stored).toEqual([...serverView.list(SyncScope.make(scope), "task")])
  }
  fingerprints.push(fingerprint())
  return fingerprints
}

// ---------------------------------------------------------------------------

describe("overlay rebase — model-based properties (SPEC §8)", () => {
  const SEEDS = 50
  const STEPS = 40

  test(`random interleavings converge with zero optimistic residue and a confirmed-only durable store (${SEEDS} seeds)`, () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-sync-overlay-prop-"))
    cleanups.push(() => rmSync(dir, { recursive: true, force: true }))
    for (let seed = 1; seed <= SEEDS; seed++) {
      runSession(seed, STEPS, join(dir, `seed-${seed}.sqlite`))
    }
  })

  test("rebase determinism: the same sequence yields the same view after every step", () => {
    const dir = mkdtempSync(join(tmpdir(), "khala-sync-overlay-det-"))
    cleanups.push(() => rmSync(dir, { recursive: true, force: true }))
    for (const seed of [3, 17, 29, 41]) {
      const first = runSession(seed, STEPS, join(dir, `a-${seed}.sqlite`))
      const second = runSession(seed, STEPS, join(dir, `b-${seed}.sqlite`))
      expect(second).toEqual(first)
    }
  })
})
