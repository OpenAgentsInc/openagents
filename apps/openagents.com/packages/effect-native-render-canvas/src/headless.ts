import { Effect, Ref, Scope } from "effect"
import type { CanvasBackend, FrameTick } from "./backend"
import type { SceneOp } from "./reconciler"
import type { Camera, SceneNodeLeaf } from "./scene"

/**
 * A headless canvas backend that records the reconciled scene graph and the
 * stream of applied ops + frame ticks, so scenes are snapshot-testable without
 * a GPU. This mirrors the headless DOM/RN test renderers in the UI catalog and
 * is the primary conformance surface for canvas scenes.
 *
 * The backend registers a `Scope` finalizer that flips `disposed` to `true`,
 * proving that resources are released deterministically on scope exit.
 */

interface StoredNode {
  readonly id: string
  readonly parentId: string | null
  readonly index: number
  readonly node: SceneNodeLeaf
}

/** A rebuilt node in the recorded scene tree, with resolved children. */
export interface HeadlessNode {
  readonly id: string
  readonly node: SceneNodeLeaf
  readonly children: ReadonlyArray<HeadlessNode>
}

export interface HeadlessCanvasSnapshot {
  readonly camera: Camera | undefined
  readonly background: string | undefined
  readonly nodes: ReadonlyArray<HeadlessNode>
  readonly frames: number
  readonly disposed: boolean
}

export interface HeadlessCanvasBackend {
  readonly backend: CanvasBackend
  /** The ordered log of every op applied to this backend. */
  readonly ops: Effect.Effect<ReadonlyArray<SceneOp>>
  /** Every frame tick handed to `renderFrame`. */
  readonly frameTicks: Effect.Effect<ReadonlyArray<FrameTick>>
  /** The current reconciled scene graph state. */
  readonly snapshot: Effect.Effect<HeadlessCanvasSnapshot>
  readonly isDisposed: Effect.Effect<boolean>
}

const buildTree = (stored: ReadonlyArray<StoredNode>): ReadonlyArray<HeadlessNode> => {
  const byParent = new Map<string | null, Array<StoredNode>>()
  for (const entry of stored) {
    const bucket = byParent.get(entry.parentId)
    if (bucket === undefined) byParent.set(entry.parentId, [entry])
    else bucket.push(entry)
  }
  const build = (parentId: string | null): ReadonlyArray<HeadlessNode> => {
    const bucket = byParent.get(parentId)
    if (bucket === undefined) return []
    return [...bucket]
      .sort((a, b) => a.index - b.index)
      .map((entry) => ({ id: entry.id, node: entry.node, children: build(entry.id) }))
  }
  return build(null)
}

const descendantIds = (stored: ReadonlyArray<StoredNode>, rootId: string): ReadonlySet<string> => {
  const ids = new Set<string>([rootId])
  let grew = true
  while (grew) {
    grew = false
    for (const entry of stored) {
      if (entry.parentId !== null && ids.has(entry.parentId) && !ids.has(entry.id)) {
        ids.add(entry.id)
        grew = true
      }
    }
  }
  return ids
}

export const makeHeadlessCanvasBackend = (): Effect.Effect<HeadlessCanvasBackend, never, Scope.Scope> =>
  Effect.gen(function* () {
    const cameraRef = yield* Ref.make<Camera | undefined>(undefined)
    const backgroundRef = yield* Ref.make<string | undefined>(undefined)
    const nodesRef = yield* Ref.make<ReadonlyArray<StoredNode>>([])
    const opsRef = yield* Ref.make<ReadonlyArray<SceneOp>>([])
    const ticksRef = yield* Ref.make<ReadonlyArray<FrameTick>>([])
    const disposedRef = yield* Ref.make(false)

    yield* Effect.addFinalizer(() => Ref.set(disposedRef, true))

    const record = (op: SceneOp): Effect.Effect<void> => Ref.update(opsRef, (log) => [...log, op])

    const backend: CanvasBackend = {
      setCamera: (camera) =>
        Effect.gen(function* () {
          yield* Ref.set(cameraRef, camera)
          yield* record({ _tag: "SetCamera", camera })
        }),
      setBackground: (color) =>
        Effect.gen(function* () {
          yield* Ref.set(backgroundRef, color)
          yield* record({ _tag: "SetBackground", color })
        }),
      createNode: ({ id, index, node, parentId }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => [
            ...nodes.filter((entry) => entry.id !== id),
            { id, parentId, index, node }
          ])
          yield* record({ _tag: "CreateNode", id, parentId, index, node })
        }),
      updateNode: ({ id, node }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => nodes.map((entry) => (entry.id === id ? { ...entry, node } : entry)))
          yield* record({ _tag: "UpdateNode", id, node })
        }),
      moveNode: ({ id, index, parentId }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) =>
            nodes.map((entry) => (entry.id === id ? { ...entry, parentId, index } : entry))
          )
          yield* record({ _tag: "MoveNode", id, parentId, index })
        }),
      removeNode: (id) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => {
            const doomed = descendantIds(nodes, id)
            return nodes.filter((entry) => !doomed.has(entry.id))
          })
          yield* record({ _tag: "RemoveNode", id })
        }),
      renderFrame: (tick) => Ref.update(ticksRef, (ticks) => [...ticks, tick])
    }

    return {
      backend,
      ops: Ref.get(opsRef),
      frameTicks: Ref.get(ticksRef),
      isDisposed: Ref.get(disposedRef),
      snapshot: Effect.gen(function* () {
        const stored = yield* Ref.get(nodesRef)
        return {
          camera: yield* Ref.get(cameraRef),
          background: yield* Ref.get(backgroundRef),
          nodes: buildTree(stored),
          frames: (yield* Ref.get(ticksRef)).length,
          disposed: yield* Ref.get(disposedRef)
        }
      })
    }
  })
