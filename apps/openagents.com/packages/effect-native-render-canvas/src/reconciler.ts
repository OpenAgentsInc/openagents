import type { Camera, CanvasScene, SceneNode, SceneNodeLeaf } from "./scene"
import { childrenOf, toLeaf } from "./scene"

/**
 * Reconciler: a pure, inspectable diff of two typed scene trees into a minimal
 * ordered list of {@link SceneOp}s. The ops are applied imperatively to a
 * backend scene graph elsewhere (see `backend.ts`). Keeping the diff pure makes
 * every frame's mutation set snapshot-testable without a GPU.
 */

export type SceneOp =
  | { readonly _tag: "SetCamera"; readonly camera: Camera }
  | { readonly _tag: "SetBackground"; readonly color: string | undefined }
  | {
      readonly _tag: "CreateNode"
      readonly id: string
      readonly parentId: string | null
      readonly index: number
      readonly node: SceneNodeLeaf
    }
  | { readonly _tag: "UpdateNode"; readonly id: string; readonly node: SceneNodeLeaf }
  | { readonly _tag: "MoveNode"; readonly id: string; readonly parentId: string | null; readonly index: number }
  | { readonly _tag: "RemoveNode"; readonly id: string }

export class DuplicateNodeKeyError extends Error {
  readonly _tag = "DuplicateNodeKeyError"
  constructor(readonly key: string) {
    super(`Duplicate scene node key: ${key}. Scene node keys must be unique across the whole tree.`)
    this.name = "DuplicateNodeKeyError"
  }
}

interface FlatEntry {
  readonly id: string
  readonly parentId: string | null
  readonly index: number
  readonly depth: number
  readonly leaf: SceneNodeLeaf
}

/** Flatten a scene tree into a key-indexed map with parent/index/depth metadata. */
export const flattenScene = (scene: CanvasScene): ReadonlyMap<string, FlatEntry> => {
  const out = new Map<string, FlatEntry>()
  const walk = (nodes: ReadonlyArray<SceneNode>, parentId: string | null, depth: number): void => {
    nodes.forEach((node, index) => {
      if (out.has(node.key)) {
        throw new DuplicateNodeKeyError(node.key)
      }
      out.set(node.key, { id: node.key, parentId, index, depth, leaf: toLeaf(node) })
      walk(childrenOf(node), node.key, depth + 1)
    })
  }
  walk(scene.children, null, 0)
  return out
}

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null

/** Structural equality over the bounded, JSON-shaped descriptor values. */
export const deepEqual = (a: unknown, b: unknown): boolean => {
  if (a === b) return true
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false
    for (let i = 0; i < a.length; i++) {
      if (!deepEqual(a[i], b[i])) return false
    }
    return true
  }
  if (isObject(a) && isObject(b)) {
    const aKeys = Object.keys(a)
    const bKeys = Object.keys(b)
    if (aKeys.length !== bKeys.length) return false
    for (const key of aKeys) {
      if (!Object.prototype.hasOwnProperty.call(b, key)) return false
      if (!deepEqual(a[key], b[key])) return false
    }
    return true
  }
  return false
}

const cameraChanged = (prev: Camera | undefined, next: Camera): boolean => prev === undefined || !deepEqual(prev, next)

const backgroundChanged = (prev: CanvasScene | undefined, next: CanvasScene): boolean =>
  prev === undefined ? next.background !== undefined : prev.background !== next.background

/**
 * Diff a previous scene against the next scene, producing a minimal ordered op
 * list. Order: camera, background, removals (deepest first), creations
 * (shallowest first), moves, updates. Passing `undefined` as `prev` produces a
 * full mount of `next`.
 */
export const diffScene = (prev: CanvasScene | undefined, next: CanvasScene): ReadonlyArray<SceneOp> => {
  const ops: Array<SceneOp> = []

  if (cameraChanged(prev?.camera, next.camera)) {
    ops.push({ _tag: "SetCamera", camera: next.camera })
  }
  if (backgroundChanged(prev, next)) {
    ops.push({ _tag: "SetBackground", color: next.background })
  }

  const prevMap = prev === undefined ? new Map<string, FlatEntry>() : flattenScene(prev)
  const nextMap = flattenScene(next)

  const removals: Array<FlatEntry> = []
  for (const [id, entry] of prevMap) {
    if (!nextMap.has(id)) removals.push(entry)
  }
  removals.sort((a, b) => b.depth - a.depth).forEach((entry) => ops.push({ _tag: "RemoveNode", id: entry.id }))

  const creations: Array<FlatEntry> = []
  const moves: Array<SceneOp> = []
  const updates: Array<SceneOp> = []
  for (const [id, entry] of nextMap) {
    const prevEntry = prevMap.get(id)
    if (prevEntry === undefined) {
      creations.push(entry)
      continue
    }
    if (prevEntry.parentId !== entry.parentId || prevEntry.index !== entry.index) {
      moves.push({ _tag: "MoveNode", id, parentId: entry.parentId, index: entry.index })
    }
    if (!deepEqual(prevEntry.leaf, entry.leaf)) {
      updates.push({ _tag: "UpdateNode", id, node: entry.leaf })
    }
  }

  creations
    .sort((a, b) => a.depth - b.depth || a.index - b.index)
    .forEach((entry) =>
      ops.push({
        _tag: "CreateNode",
        id: entry.id,
        parentId: entry.parentId,
        index: entry.index,
        node: entry.leaf
      })
    )

  for (const op of moves) ops.push(op)
  for (const op of updates) ops.push(op)

  return ops
}
