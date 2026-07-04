import {
  type ChangelogEntry,
  canonicalJson,
  MutationEnvelope,
  MutationId,
  type MutatorName,
  type SyncScope,
  type SyncVersion,
} from "@openagentsinc/khala-sync"
import { Effect } from "effect"
import type {
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"

/**
 * In-memory optimistic overlay + rebase engine (KS-5.2; SPEC §2.4, §6,
 * §7 invariant 2).
 *
 * The overlay is the ONLY home of optimistic effects. The durable local
 * store (KS-5.1) holds server-confirmed state exclusively; this module
 * writes to it only through the confirmed paths (`applyConfirmed`,
 * `enqueueMutation` — the mutation *intent*, never its effects — and
 * `ackMutations`).
 *
 * Rebase model (Replicache/Zero/Linear): on every confirmed delta the
 * overlay rewinds to confirmed state, re-applies the still-unconfirmed
 * pending mutations in `mutationId` order by re-running their pure
 * `apply` against the new confirmed view, and reveals the result
 * atomically — readers observe either the pre-rebase or the post-rebase
 * view, never a partial one (the reveal is a single snapshot-reference
 * swap). Mutators must be replay-safe; server outcome wins.
 */

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

export type KhalaSyncOverlayErrorReason =
  /** A mutator name was not registered with this overlay. */
  | "unknown_mutator"
  /** Two registered mutators share one name. */
  | "duplicate_mutator"
  /** Mutation args could not be canonicalized / parsed. */
  | "invalid_args"
  /** A mutator's pure `apply` threw (at optimistic apply or on rebase). */
  | "mutator_failure"

/** Typed overlay error; `message` names the violated rule, never row values. */
export class KhalaSyncOverlayError extends Error {
  readonly _tag = "KhalaSyncOverlayError"
  override readonly name = "KhalaSyncOverlayError"
  constructor(
    readonly reason: KhalaSyncOverlayErrorReason,
    message: string,
    options?: { readonly cause?: unknown },
  ) {
    super(message, options)
  }
}

export type OverlayError = KhalaSyncOverlayError | KhalaSyncClientStoreError

// ---------------------------------------------------------------------------
// Mutator + view contracts
// ---------------------------------------------------------------------------

/** One entity as seen through the overlay (confirmed + optimistic). */
export interface OverlayEntity {
  readonly entityType: string
  readonly entityId: string
  readonly postImageJson: string
}

/**
 * Scoped read surface over confirmed + optimistic state. `get` returns the
 * entity's post-image (canonical JSON) or `undefined`; `list` returns the
 * scope's entities of one type ordered by entityId (matching the store's
 * ordering).
 */
export interface OverlayView {
  readonly get: (entityType: string, entityId: string) => string | undefined
  readonly list: (entityType: string) => ReadonlyArray<OverlayEntity>
}

/**
 * The read surface handed to a mutator's `apply`: scope-qualified reads
 * over the working state at that point of the fold (confirmed state plus
 * the effects of all earlier pending mutations). Reads of scopes the
 * overlay has never tracked (never `read`, never confirmed, never written
 * by an effect) return `undefined`/empty.
 */
export interface OverlayReadView {
  readonly get: (
    scope: SyncScope,
    entityType: string,
    entityId: string,
  ) => string | undefined
  readonly list: (
    scope: SyncScope,
    entityType: string,
  ) => ReadonlyArray<OverlayEntity>
}

/**
 * One optimistic effect of a mutation. `postImageJson` MUST be canonical
 * JSON (`canonicalJson`) so optimistic and confirmed post-images compare
 * byte-wise.
 */
export type OverlayEffect =
  | {
      readonly kind: "upsert"
      readonly scope: SyncScope
      readonly entityType: string
      readonly entityId: string
      readonly postImageJson: string
    }
  | {
      readonly kind: "delete"
      readonly scope: SyncScope
      readonly entityType: string
      readonly entityId: string
    }

/**
 * The client-side implementation of a named mutator: a pure function from
 * (args, view) to overlay effects. Re-executed on every rebase against the
 * then-current confirmed view, so it must be replay-safe and side-effect
 * free. Args round-trip through canonical JSON — `apply` always receives
 * the parsed round-trip, at first apply and at every replay alike.
 */
export interface ClientMutator<Args = unknown> {
  readonly name: MutatorName
  readonly apply: (
    args: Args,
    view: OverlayReadView,
  ) => ReadonlyArray<OverlayEffect>
}

// ---------------------------------------------------------------------------
// Overlay interface
// ---------------------------------------------------------------------------

export interface KhalaSyncOverlay {
  /**
   * Scoped live view over confirmed + optimistic state. Tracks the scope
   * (loads its confirmed base from the store on first read). The returned
   * view is live: it always reflects the latest atomically-revealed
   * snapshot.
   */
  readonly read: (
    scope: SyncScope,
  ) => Effect.Effect<OverlayView, OverlayError>
  /**
   * Optimistic write: run the mutator's pure `apply` against the current
   * overlay view, enqueue the mutation intent on the store's durable FIFO
   * queue, apply the effects to the in-memory overlay, notify. The durable
   * entities/cursors tables are NEVER touched (SPEC §7 invariant 2).
   * Returns the assigned mutationId.
   */
  readonly mutate: <Args>(
    mutator: ClientMutator<Args>,
    args: Args,
  ) => Effect.Effect<MutationId, OverlayError>
  /**
   * Confirmed delta from the server: apply entries + cursor to the durable
   * store, then rebase — rewind to confirmed, re-apply still-pending
   * mutations in mutationId order, reveal atomically. If a replay fails,
   * the durable store keeps the confirmed delta (it already committed) and
   * the overlay keeps its previous consistent snapshot; the error is
   * surfaced typed.
   */
  readonly onConfirmed: (
    scope: SyncScope,
    entries: ReadonlyArray<ChangelogEntry>,
    cursor: SyncVersion,
  ) => Effect.Effect<void, OverlayError>
  /**
   * Server ack through `throughMutationId` (rejections ack too): drop the
   * acked mutations from the durable queue and from the overlay (their
   * optimistic contributions vanish; confirmed outcomes arrive/arrived via
   * the changelog), rebuild, reveal atomically.
   */
  readonly onAck: (
    throughMutationId: MutationId,
  ) => Effect.Effect<void, OverlayError>
  /** Still-unconfirmed queued mutations, ascending mutationId. */
  readonly pending: () => ReadonlyArray<MutationEnvelope>
  /**
   * Change notifications: `listener(scope)` fires after a snapshot reveal
   * for each scope whose view content changed. Returns unsubscribe.
   */
  readonly subscribe: (
    listener: (scope: SyncScope) => void,
  ) => () => void
}

// ---------------------------------------------------------------------------
// Internal snapshot model
// ---------------------------------------------------------------------------

/** entityId → postImageJson */
type TypeMap = Map<string, string>
/** entityType → TypeMap (never contains empty TypeMaps) */
type ScopeMap = Map<string, TypeMap>
type Snapshots = Map<SyncScope, ScopeMap>

const cloneScopeMap = (scopeMap: ScopeMap): ScopeMap => {
  const clone: ScopeMap = new Map()
  for (const [entityType, typeMap] of scopeMap) {
    clone.set(entityType, new Map(typeMap))
  }
  return clone
}

const applyEffectInPlace = (scopeMap: ScopeMap, effect: OverlayEffect): void => {
  if (effect.kind === "upsert") {
    let typeMap = scopeMap.get(effect.entityType)
    if (typeMap === undefined) {
      typeMap = new Map()
      scopeMap.set(effect.entityType, typeMap)
    }
    typeMap.set(effect.entityId, effect.postImageJson)
    return
  }
  const typeMap = scopeMap.get(effect.entityType)
  if (typeMap === undefined) return
  typeMap.delete(effect.entityId)
  if (typeMap.size === 0) scopeMap.delete(effect.entityType)
}

const scopeMapsEqual = (
  a: ScopeMap | undefined,
  b: ScopeMap | undefined,
): boolean => {
  const left = a ?? new Map<string, TypeMap>()
  const right = b ?? new Map<string, TypeMap>()
  if (left.size !== right.size) return false
  for (const [entityType, typeMap] of left) {
    const other = right.get(entityType)
    if (other === undefined || other.size !== typeMap.size) return false
    for (const [entityId, postImageJson] of typeMap) {
      if (other.get(entityId) !== postImageJson) return false
    }
  }
  return true
}

const listOf = (
  scopeMap: ScopeMap | undefined,
  entityType: string,
): ReadonlyArray<OverlayEntity> => {
  const typeMap = scopeMap?.get(entityType)
  if (typeMap === undefined) return []
  return [...typeMap.entries()]
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([entityId, postImageJson]) => ({ entityType, entityId, postImageJson }))
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/**
 * Create the optimistic overlay over a local store. Loads any pending
 * mutations that survived restart from the durable queue and rebuilds
 * their optimistic effects (re-running each mutator's pure `apply`).
 *
 * `mutators` is the replay registry: every mutator ever passed to
 * {@link KhalaSyncOverlay.mutate} must be registered here so a queued
 * mutation can be re-applied on rebase and across restarts.
 */
export const createOverlay = (
  store: KhalaSyncLocalStore,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- registry over heterogeneous arg types
  mutators: ReadonlyArray<ClientMutator<any>>,
): Effect.Effect<KhalaSyncOverlay, OverlayError> =>
  Effect.gen(function* () {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const registry = new Map<MutatorName, ClientMutator<any>>()
    for (const mutator of mutators) {
      if (registry.has(mutator.name)) {
        return yield* Effect.fail(
          new KhalaSyncOverlayError(
            "duplicate_mutator",
            `mutator "${mutator.name}" is registered twice`,
          ),
        )
      }
      registry.set(mutator.name, mutator)
    }

    /** The revealed snapshot. Reveals replace this reference atomically. */
    let snapshots: Snapshots = new Map()
    /** Scopes whose confirmed base participates in rebuilds. */
    const tracked = new Set<SyncScope>()
    let pendingList: Array<MutationEnvelope> = [
      ...(yield* store.pendingMutations()),
    ]
    let lastId: number = (yield* store.lastMutationId()) ?? 0
    const listeners = new Set<(scope: SyncScope) => void>()

    const notify = (scopes: Iterable<SyncScope>): void => {
      for (const scope of scopes) {
        for (const listener of [...listeners]) listener(scope)
      }
    }

    const loadConfirmed = (
      scope: SyncScope,
    ): Effect.Effect<ScopeMap, KhalaSyncClientStoreError> =>
      Effect.map(store.readEntities(scope), (entities) => {
        const scopeMap: ScopeMap = new Map()
        for (const entity of entities) {
          let typeMap = scopeMap.get(entity.entityType)
          if (typeMap === undefined) {
            typeMap = new Map()
            scopeMap.set(entity.entityType, typeMap)
          }
          typeMap.set(entity.entityId, entity.postImageJson)
        }
        return scopeMap
      })

    const parseArgs = (
      envelope: MutationEnvelope,
    ): Effect.Effect<unknown, KhalaSyncOverlayError> =>
      Effect.try({
        try: () => JSON.parse(envelope.argsJson) as unknown,
        catch: (cause) =>
          new KhalaSyncOverlayError(
            "invalid_args",
            `argsJson of mutation ${envelope.mutationId} is not valid JSON`,
            { cause },
          ),
      })

    const runMutator = (
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      mutator: ClientMutator<any>,
      args: unknown,
      view: OverlayReadView,
      mutationId: number,
    ): Effect.Effect<ReadonlyArray<OverlayEffect>, KhalaSyncOverlayError> =>
      Effect.try({
        try: () => mutator.apply(args, view),
        catch: (cause) =>
          new KhalaSyncOverlayError(
            "mutator_failure",
            `mutator "${mutator.name}" threw while applying mutation ${mutationId}`,
            { cause },
          ),
      })

    const readViewOver = (source: Snapshots): OverlayReadView => ({
      get: (scope, entityType, entityId) =>
        source.get(scope)?.get(entityType)?.get(entityId),
      list: (scope, entityType) => listOf(source.get(scope), entityType),
    })

    /** Load `scope` into `working` (confirmed base) if absent; track it. */
    const ensureScope = (
      working: Snapshots,
      scope: SyncScope,
    ): Effect.Effect<ScopeMap, KhalaSyncClientStoreError> =>
      Effect.gen(function* () {
        const existing = working.get(scope)
        if (existing !== undefined) return existing
        const base = yield* loadConfirmed(scope)
        working.set(scope, base)
        tracked.add(scope)
        return base
      })

    /** One rebase fold over freshly-loaded confirmed state. */
    const rebuildPass = (): Effect.Effect<Snapshots, OverlayError> =>
      Effect.gen(function* () {
        const working: Snapshots = new Map()
        for (const scope of tracked) {
          working.set(scope, yield* loadConfirmed(scope))
        }
        const view = readViewOver(working)
        for (const envelope of pendingList) {
          const mutator = registry.get(envelope.name)
          if (mutator === undefined) {
            return yield* Effect.fail(
              new KhalaSyncOverlayError(
                "unknown_mutator",
                `pending mutation ${envelope.mutationId} references unregistered mutator "${envelope.name}"`,
              ),
            )
          }
          const args = yield* parseArgs(envelope)
          const effects = yield* runMutator(
            mutator,
            args,
            view,
            envelope.mutationId,
          )
          for (const effect of effects) {
            const scopeMap = yield* ensureScope(working, effect.scope)
            applyEffectInPlace(scopeMap, effect)
          }
        }
        return working
      })

    /**
     * The rebase core: rebuild the whole overlay off to the side — confirmed
     * base for every tracked scope, then each pending mutation's pure apply
     * in mutationId order — and reveal via one reference swap. Runs the fold
     * to a fixpoint over the tracked-scope set: when a pass discovers a
     * scope through an effect (e.g. restart replay before any `read`), the
     * fold re-runs with that scope's confirmed base preloaded, so replays
     * read the same confirmed state a live session would. On failure the
     * previous snapshot stays revealed (never partial). Returns the scopes
     * whose visible content changed.
     */
    const rebuild = (): Effect.Effect<Set<SyncScope>, OverlayError> =>
      Effect.gen(function* () {
        let trackedBefore = tracked.size
        let working = yield* rebuildPass()
        // tracked only grows; each extra pass adds ≥1 scope → bounded.
        while (tracked.size > trackedBefore) {
          trackedBefore = tracked.size
          working = yield* rebuildPass()
        }
        const changed = new Set<SyncScope>()
        for (const scope of new Set([...snapshots.keys(), ...working.keys()])) {
          if (!scopeMapsEqual(snapshots.get(scope), working.get(scope))) {
            changed.add(scope)
          }
        }
        snapshots = working // atomic reveal
        return changed
      })

    const scopedView = (scope: SyncScope): OverlayView => ({
      get: (entityType, entityId) =>
        snapshots.get(scope)?.get(entityType)?.get(entityId),
      list: (entityType) => listOf(snapshots.get(scope), entityType),
    })

    const read = (
      scope: SyncScope,
    ): Effect.Effect<OverlayView, OverlayError> =>
      Effect.gen(function* () {
        tracked.add(scope)
        if (!snapshots.has(scope)) {
          // Untracked ⇒ no pending effects ever touched it (effects track
          // their scopes eagerly) ⇒ base = confirmed only.
          const base = yield* loadConfirmed(scope)
          const next = new Map(snapshots)
          next.set(scope, base)
          snapshots = next
        }
        return scopedView(scope)
      })

    const mutate = <Args>(
      mutator: ClientMutator<Args>,
      args: Args,
    ): Effect.Effect<MutationId, OverlayError> =>
      Effect.gen(function* () {
        if (registry.get(mutator.name) !== mutator) {
          return yield* Effect.fail(
            new KhalaSyncOverlayError(
              "unknown_mutator",
              `mutator "${mutator.name}" is not registered with this overlay`,
            ),
          )
        }
        const argsJson = yield* Effect.try({
          try: () => canonicalJson(args),
          catch: (cause) =>
            new KhalaSyncOverlayError(
              "invalid_args",
              `args of mutator "${mutator.name}" are not canonical-JSON representable`,
              { cause },
            ),
        })
        const mutationId = MutationId.make(lastId + 1)
        const envelope = new MutationEnvelope({
          mutationId,
          name: mutator.name,
          argsJson,
        })
        // apply receives the canonical round-trip, exactly as every replay will
        const parsed = JSON.parse(argsJson) as unknown
        const effects = yield* runMutator(
          mutator,
          parsed,
          readViewOver(snapshots),
          mutationId,
        )
        // Durable intent only — never the effects (SPEC §7 invariant 2).
        yield* store.enqueueMutation(envelope)
        lastId = mutationId
        pendingList.push(envelope)
        // Copy-on-write apply of the effects; reveal via one swap.
        const next = new Map(snapshots)
        const touched = new Set<SyncScope>()
        for (const effect of effects) {
          tracked.add(effect.scope)
          if (!touched.has(effect.scope)) {
            const current = next.get(effect.scope)
            next.set(
              effect.scope,
              current !== undefined
                ? cloneScopeMap(current)
                : yield* loadConfirmed(effect.scope),
            )
            touched.add(effect.scope)
          }
          applyEffectInPlace(next.get(effect.scope)!, effect)
        }
        const changed = new Set<SyncScope>()
        for (const scope of touched) {
          if (!scopeMapsEqual(snapshots.get(scope), next.get(scope))) {
            changed.add(scope)
          }
        }
        snapshots = next // atomic reveal
        notify(changed)
        return mutationId
      })

    const onConfirmed = (
      scope: SyncScope,
      entries: ReadonlyArray<ChangelogEntry>,
      cursor: SyncVersion,
    ): Effect.Effect<void, OverlayError> =>
      Effect.gen(function* () {
        yield* store.applyConfirmed(scope, entries, cursor)
        tracked.add(scope)
        notify(yield* rebuild())
      })

    const onAck = (
      throughMutationId: MutationId,
    ): Effect.Effect<void, OverlayError> =>
      Effect.gen(function* () {
        yield* store.ackMutations(throughMutationId)
        const remaining = pendingList.filter(
          (m) => m.mutationId > throughMutationId,
        )
        const dropped = remaining.length !== pendingList.length
        pendingList = remaining
        if (throughMutationId > lastId) lastId = throughMutationId
        if (dropped) notify(yield* rebuild())
      })

    // Restart path: rebuild optimistic effects of any queued survivors.
    if (pendingList.length > 0) {
      yield* rebuild()
    }

    return {
      read,
      mutate,
      onConfirmed,
      onAck,
      pending: () => [...pendingList],
      subscribe: (listener) => {
        listeners.add(listener)
        return () => {
          listeners.delete(listener)
        }
      },
    } satisfies KhalaSyncOverlay
  })
