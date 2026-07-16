import { Effect } from "effect"
import type { SceneOp } from "./reconciler"
import type { Camera, SceneNodeLeaf } from "./scene"

/** A single frame tick produced by the frame clock and threaded to the backend. */
export interface FrameTick {
  readonly frame: number
  readonly time: number
  readonly delta: number
}

/**
 * The backend scene-graph interface. The reconciler produces {@link SceneOp}s;
 * a backend applies them imperatively to whatever it owns (a headless record,
 * a Three.js scene graph, a raw WebGL context, ...). GPU/geometry/material
 * resource lifetimes belong to the backend and are released on `Scope` exit
 * (see the headless and Three.js backends).
 */
export interface CanvasBackend {
  readonly setCamera: (camera: Camera) => Effect.Effect<void>
  readonly setBackground: (color: string | undefined) => Effect.Effect<void>
  readonly createNode: (input: {
    readonly id: string
    readonly parentId: string | null
    readonly index: number
    readonly node: SceneNodeLeaf
  }) => Effect.Effect<void>
  readonly updateNode: (input: { readonly id: string; readonly node: SceneNodeLeaf }) => Effect.Effect<void>
  readonly moveNode: (input: {
    readonly id: string
    readonly parentId: string | null
    readonly index: number
  }) => Effect.Effect<void>
  readonly removeNode: (id: string) => Effect.Effect<void>
  readonly renderFrame: (tick: FrameTick) => Effect.Effect<void>
}

/** Apply a single reconciled op to a backend. */
export const applyOp = (backend: CanvasBackend, op: SceneOp): Effect.Effect<void> => {
  switch (op._tag) {
    case "SetCamera":
      return backend.setCamera(op.camera)
    case "SetBackground":
      return backend.setBackground(op.color)
    case "CreateNode":
      return backend.createNode({ id: op.id, parentId: op.parentId, index: op.index, node: op.node })
    case "UpdateNode":
      return backend.updateNode({ id: op.id, node: op.node })
    case "MoveNode":
      return backend.moveNode({ id: op.id, parentId: op.parentId, index: op.index })
    case "RemoveNode":
      return backend.removeNode(op.id)
  }
}

/** Apply an ordered list of reconciled ops to a backend, in order. */
export const applyOps = (backend: CanvasBackend, ops: ReadonlyArray<SceneOp>): Effect.Effect<void> =>
  Effect.forEach(ops, (op) => applyOp(backend, op), { discard: true })
