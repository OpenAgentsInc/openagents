import { Effect, Ref, Scope } from "effect"
import * as Three from "three"
import type { CanvasBackend, FrameTick } from "./backend"
import type { Camera, GeometryRef, MaterialRef, SceneNodeLeaf, Vec3 } from "./scene"
import {
  createSceneNodeReconciler,
  createSceneResourceScope,
  type SceneNodeCatalogue,
  type SceneNodeDescriptor,
  type SceneNodeFactory,
  type SceneNodeReconciler,
  type SceneResourceScope
} from "./scene-node-reconciler"

/**
 * Three.js backend for `@effect-native/render-canvas`.
 *
 * Live path owns:
 *  - a closed scene-node reconciler + catalogue (group/mesh/line/points/label)
 *  - a resource scope bridged to Effect `Scope` disposal
 *  - an optional WebGLRenderer (draw skipped when no GPU/canvas is available)
 *
 * {@link makeThreeCanvasBackend} is the pure adapter over any
 * {@link ThreeSceneGraph} port (recording fake in tests, live graph in apps).
 * The package depends on `three` — not on experimental product wrappers.
 */

/** Serializable scene-node descriptor for the Three.js backend. */
export interface ThreeSceneDescriptor {
  readonly id: string
  readonly kind: string
  readonly props: Record<string, unknown>
  readonly children?: ReadonlyArray<ThreeSceneDescriptor>
}

/**
 * Effect-shaped port over a Three.js scene graph. A concrete implementation
 * owns a scene-node reconciler + camera/renderer; resources are released when
 * the Effect `Scope` closes.
 */
export interface ThreeSceneGraph {
  readonly update: (descriptors: ReadonlyArray<ThreeSceneDescriptor>) => Effect.Effect<void>
  readonly setCamera: (camera: Camera) => Effect.Effect<void>
  readonly setBackground: (color: string | undefined) => Effect.Effect<void>
  readonly render: (tick: FrameTick) => Effect.Effect<void>
  /** Live graph only — root group for inspection / embedding. */
  readonly root?: Three.Object3D
  /** Live graph only — perspective/ortho camera used for WebGL draws. */
  readonly threeCamera?: Three.Camera
  /** Live graph only — WebGL renderer when one was constructed. */
  readonly renderer?: Three.WebGLRenderer | undefined
}

export interface LiveThreeSceneGraphOptions {
  /**
   * Optional canvas / offscreen target for WebGL. When omitted, the graph still
   * reconciles Three.js objects (smoke/tests) but `render` no-ops the draw call.
   */
  readonly canvas?: HTMLCanvasElement | OffscreenCanvas
  /** Pixel ratio for the WebGL renderer (default 1). */
  readonly pixelRatio?: number
  /** Explicit width/height when using OffscreenCanvas without CSS layout. */
  readonly size?: { readonly width: number; readonly height: number }
}

const stripMeta = (leaf: SceneNodeLeaf): Record<string, unknown> => {
  const {
    _tag: _drop,
    key: _key,
    ...rest
  } = leaf as Record<string, unknown> & {
    _tag: string
    key: string
  }
  return rest
}

/** Map a single leaf to a scene-node descriptor (sans children). */
export const toThreeDescriptorProps = (leaf: SceneNodeLeaf): { kind: string; props: Record<string, unknown> } => ({
  kind: leaf._tag.toLowerCase(),
  props: stripMeta(leaf)
})

interface StoredNode {
  readonly id: string
  readonly parentId: string | null
  readonly index: number
  readonly node: SceneNodeLeaf
}

/** Build a descriptor tree from a flat stored-node set. */
export const buildThreeDescriptors = (stored: ReadonlyArray<StoredNode>): ReadonlyArray<ThreeSceneDescriptor> => {
  const byParent = new Map<string | null, Array<StoredNode>>()
  for (const entry of stored) {
    const bucket = byParent.get(entry.parentId)
    if (bucket === undefined) byParent.set(entry.parentId, [entry])
    else bucket.push(entry)
  }
  const build = (parentId: string | null): ReadonlyArray<ThreeSceneDescriptor> => {
    const bucket = byParent.get(parentId)
    if (bucket === undefined) return []
    return [...bucket]
      .sort((a, b) => a.index - b.index)
      .map((entry) => {
        const { kind, props } = toThreeDescriptorProps(entry.node)
        const children = build(entry.id)
        return children.length === 0 ? { id: entry.id, kind, props } : { id: entry.id, kind, props, children }
      })
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

/**
 * Build a {@link CanvasBackend} that reconciles our typed scene ops into a
 * descriptor tree and pushes it to the injected graph port on each frame. Camera/background changes are forwarded immediately.
 */
export const makeThreeCanvasBackend = (graph: ThreeSceneGraph): Effect.Effect<CanvasBackend, never, Scope.Scope> =>
  Effect.gen(function* () {
    const nodesRef = yield* Ref.make<ReadonlyArray<StoredNode>>([])
    const dirtyRef = yield* Ref.make(false)

    const markDirty = Ref.set(dirtyRef, true)

    const backend: CanvasBackend = {
      setCamera: (camera) => graph.setCamera(camera),
      setBackground: (color) => graph.setBackground(color),
      createNode: ({ id, index, node, parentId }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => [
            ...nodes.filter((entry) => entry.id !== id),
            { id, parentId, index, node }
          ])
          yield* markDirty
        }),
      updateNode: ({ id, node }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => nodes.map((entry) => (entry.id === id ? { ...entry, node } : entry)))
          yield* markDirty
        }),
      moveNode: ({ id, index, parentId }) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) =>
            nodes.map((entry) => (entry.id === id ? { ...entry, parentId, index } : entry))
          )
          yield* markDirty
        }),
      removeNode: (id) =>
        Effect.gen(function* () {
          yield* Ref.update(nodesRef, (nodes) => {
            const doomed = descendantIds(nodes, id)
            return nodes.filter((entry) => !doomed.has(entry.id))
          })
          yield* markDirty
        }),
      renderFrame: (tick) =>
        Effect.gen(function* () {
          const dirty = yield* Ref.get(dirtyRef)
          if (dirty) {
            const stored = yield* Ref.get(nodesRef)
            yield* graph.update(buildThreeDescriptors(stored))
            yield* Ref.set(dirtyRef, false)
          }
          yield* graph.render(tick)
        })
    }

    return backend
  })

// ---------------------------------------------------------------------------
// Live Three.js scene graph
// ---------------------------------------------------------------------------

const applyTransform = (
  object: Three.Object3D,
  props: {
    readonly position?: Vec3
    readonly rotation?: Vec3
    readonly scale?: Vec3
    readonly visible?: boolean
  }
): void => {
  if (props.position !== undefined) object.position.set(...props.position)
  if (props.rotation !== undefined) object.rotation.set(...props.rotation)
  if (props.scale !== undefined) object.scale.set(...props.scale)
  if (props.visible !== undefined) object.visible = props.visible
}

const geometryFromRef = (geometry: GeometryRef): Three.BufferGeometry => {
  switch (geometry._tag) {
    case "Box":
      return new Three.BoxGeometry(geometry.width, geometry.height, geometry.depth)
    case "Sphere":
      return new Three.SphereGeometry(geometry.radius, geometry.segments ?? 16, geometry.segments ?? 16)
    case "Plane":
      return new Three.PlaneGeometry(geometry.width, geometry.height)
  }
}

const materialFromRef = (material: MaterialRef): Three.Material => {
  switch (material._tag) {
    case "Basic":
      return new Three.MeshBasicMaterial({
        color: material.color,
        opacity: material.opacity ?? 1,
        transparent: (material.opacity ?? 1) < 1,
        wireframe: material.wireframe ?? false
      })
    case "Standard":
      return new Three.MeshStandardMaterial({
        color: material.color,
        opacity: material.opacity ?? 1,
        transparent: (material.opacity ?? 1) < 1,
        metalness: material.metalness ?? 0,
        roughness: material.roughness ?? 1,
        emissive: material.emissive ?? "#000000"
      })
  }
}

type MeshState = { geometry: Three.BufferGeometry; material: Three.Material }
type LineState = { geometry: Three.BufferGeometry; material: Three.LineBasicMaterial }
type PointsState = { geometry: Three.BufferGeometry; material: Three.PointsMaterial }
type LabelState = { material: Three.SpriteMaterial; texture: Three.CanvasTexture }

const disposeMaterial = (material: Three.Material): void => {
  material.dispose()
}

/** Factories use `state: unknown` for the catalogue's untyped state slot. */
const makeGroupFactory = (): SceneNodeFactory => ({
  create: (descriptor) => {
    const object = new Three.Group()
    object.name = descriptor.id
    applyTransform(
      object,
      descriptor.props as {
        position?: Vec3
        rotation?: Vec3
        scale?: Vec3
        visible?: boolean
      }
    )
    return { object, childRoot: object }
  },
  update: (runtime, descriptor) => {
    applyTransform(
      runtime.object,
      descriptor.props as {
        position?: Vec3
        rotation?: Vec3
        scale?: Vec3
        visible?: boolean
      }
    )
  }
})

const makeMeshFactory = (): SceneNodeFactory => ({
  create: (descriptor, scope) => {
    const props = descriptor.props as {
      geometry: GeometryRef
      material: MaterialRef
      position?: Vec3
      rotation?: Vec3
      scale?: Vec3
      visible?: boolean
    }
    const geometry = geometryFromRef(props.geometry)
    const material = materialFromRef(props.material)
    const object = new Three.Mesh(geometry, material)
    object.name = descriptor.id
    applyTransform(object, props)
    scope.add(() => {
      geometry.dispose()
      disposeMaterial(material)
    })
    return { object, state: { geometry, material } satisfies MeshState }
  },
  update: (runtime, descriptor) => {
    const props = descriptor.props as {
      geometry: GeometryRef
      material: MaterialRef
      position?: Vec3
      rotation?: Vec3
      scale?: Vec3
      visible?: boolean
    }
    const prev = runtime.descriptor.props as { geometry: GeometryRef; material: MaterialRef }
    if (prev.geometry._tag !== props.geometry._tag || prev.material._tag !== props.material._tag) {
      return false
    }
    applyTransform(runtime.object, props)
    const state = runtime.state as MeshState | undefined
    if (state !== undefined && "color" in state.material) {
      const mat = state.material as Three.MeshBasicMaterial | Three.MeshStandardMaterial
      mat.color.set(props.material.color)
    }
    return true
  }
})

const makeLineFactory = (): SceneNodeFactory => ({
  create: (descriptor, scope) => {
    const props = descriptor.props as {
      points: ReadonlyArray<Vec3>
      color: string
      opacity?: number
      position?: Vec3
      visible?: boolean
    }
    const geometry = new Three.BufferGeometry().setFromPoints(props.points.map((p) => new Three.Vector3(...p)))
    const material = new Three.LineBasicMaterial({
      color: props.color,
      opacity: props.opacity ?? 1,
      transparent: (props.opacity ?? 1) < 1
    })
    const object = new Three.Line(geometry, material)
    object.name = descriptor.id
    applyTransform(object, props)
    scope.add(() => {
      geometry.dispose()
      material.dispose()
    })
    return { object, state: { geometry, material } satisfies LineState }
  },
  update: (runtime, descriptor) => {
    const props = descriptor.props as {
      points: ReadonlyArray<Vec3>
      color: string
      opacity?: number
      position?: Vec3
      visible?: boolean
    }
    applyTransform(runtime.object, props)
    const state = runtime.state as LineState | undefined
    if (state !== undefined) {
      state.material.color.set(props.color)
      state.material.opacity = props.opacity ?? 1
      state.geometry.setFromPoints(props.points.map((p) => new Three.Vector3(...p)))
    }
  }
})

const makePointsFactory = (): SceneNodeFactory => ({
  create: (descriptor, scope) => {
    const props = descriptor.props as {
      positions: ReadonlyArray<Vec3>
      size: number
      color: string
      opacity?: number
      visible?: boolean
    }
    const geometry = new Three.BufferGeometry().setFromPoints(props.positions.map((p) => new Three.Vector3(...p)))
    const material = new Three.PointsMaterial({
      color: props.color,
      size: props.size,
      opacity: props.opacity ?? 1,
      transparent: (props.opacity ?? 1) < 1
    })
    const object = new Three.Points(geometry, material)
    object.name = descriptor.id
    if (props.visible !== undefined) object.visible = props.visible
    scope.add(() => {
      geometry.dispose()
      material.dispose()
    })
    return { object, state: { geometry, material } satisfies PointsState }
  },
  update: (runtime, descriptor) => {
    const props = descriptor.props as {
      positions: ReadonlyArray<Vec3>
      size: number
      color: string
      opacity?: number
      visible?: boolean
    }
    if (props.visible !== undefined) runtime.object.visible = props.visible
    const state = runtime.state as PointsState | undefined
    if (state !== undefined) {
      state.material.color.set(props.color)
      state.material.size = props.size
      state.material.opacity = props.opacity ?? 1
      state.geometry.setFromPoints(props.positions.map((p) => new Three.Vector3(...p)))
    }
  }
})

const makeLabelTexture = (text: string, color: string, fontSize: number): Three.CanvasTexture => {
  const canvas =
    typeof globalThis.document !== "undefined"
      ? globalThis.document.createElement("canvas")
      : // Node tests without DOM: 1×1 placeholder still yields a texture.
        ({
          width: 1,
          height: 1,
          getContext: () => null
        } as unknown as HTMLCanvasElement)

  if ("getContext" in canvas && typeof (canvas as HTMLCanvasElement).getContext === "function") {
    const el = canvas as HTMLCanvasElement
    el.width = Math.max(64, Math.ceil(fontSize * Math.max(text.length, 1) * 0.7))
    el.height = Math.max(32, Math.ceil(fontSize * 1.6))
    const ctx = el.getContext("2d")
    if (ctx) {
      ctx.clearRect(0, 0, el.width, el.height)
      ctx.fillStyle = color
      ctx.font = `${fontSize}px sans-serif`
      ctx.textBaseline = "middle"
      ctx.fillText(text, 4, el.height / 2)
    }
  }
  const texture = new Three.CanvasTexture(canvas as HTMLCanvasElement)
  texture.needsUpdate = true
  return texture
}

const makeLabelFactory = (): SceneNodeFactory => ({
  create: (descriptor, scope) => {
    const props = descriptor.props as {
      text: string
      color: string
      fontSize: number
      position?: Vec3
      visible?: boolean
    }
    const texture = makeLabelTexture(props.text, props.color, props.fontSize)
    const material = new Three.SpriteMaterial({ map: texture, transparent: true })
    const object = new Three.Sprite(material)
    object.name = descriptor.id
    const scale = Math.max(props.fontSize / 24, 0.25)
    object.scale.set(scale * 2, scale, 1)
    if (props.position !== undefined) object.position.set(...props.position)
    if (props.visible !== undefined) object.visible = props.visible
    scope.add(() => {
      texture.dispose()
      material.dispose()
    })
    return { object, state: { material, texture } satisfies LabelState }
  },
  update: (runtime, descriptor) => {
    const props = descriptor.props as {
      text: string
      color: string
      fontSize: number
      position?: Vec3
      visible?: boolean
    }
    const prev = runtime.descriptor.props as { text: string; color: string; fontSize: number }
    if (prev.text !== props.text || prev.color !== props.color || prev.fontSize !== props.fontSize) {
      return false
    }
    if (props.position !== undefined) runtime.object.position.set(...props.position)
    if (props.visible !== undefined) runtime.object.visible = props.visible
    return true
  }
})

/** Closed catalogue mapping our scene kinds onto Three.js factories. */
export const makeEffectNativeSceneCatalogue = (): SceneNodeCatalogue => ({
  group: makeGroupFactory(),
  mesh: makeMeshFactory(),
  line: makeLineFactory(),
  points: makePointsFactory(),
  label: makeLabelFactory()
})

const applyCameraToThree = (threeCamera: Three.Camera, camera: Camera): void => {
  threeCamera.position.set(...camera.position)
  threeCamera.lookAt(new Three.Vector3(...camera.target))
  if (camera._tag === "Perspective" && threeCamera instanceof Three.PerspectiveCamera) {
    threeCamera.fov = camera.fov
    threeCamera.near = camera.near
    threeCamera.far = camera.far
    threeCamera.updateProjectionMatrix()
  }
  if (camera._tag === "Orthographic" && threeCamera instanceof Three.OrthographicCamera) {
    const f = camera.frustum
    threeCamera.left = -f
    threeCamera.right = f
    threeCamera.top = f
    threeCamera.bottom = -f
    threeCamera.near = camera.near
    threeCamera.far = camera.far
    threeCamera.updateProjectionMatrix()
  }
}

const tryCreateWebGlRenderer = (options: LiveThreeSceneGraphOptions | undefined): Three.WebGLRenderer | undefined => {
  try {
    const canvas = options?.canvas
    const renderer = new Three.WebGLRenderer({
      canvas: canvas as HTMLCanvasElement | undefined,
      antialias: true,
      alpha: true,
      // Headless / missing GPU: allow construction to throw; we catch below.
      failIfMajorPerformanceCaveat: false
    })
    const width = options?.size?.width ?? 64
    const height = options?.size?.height ?? 64
    renderer.setSize(width, height, false)
    renderer.setPixelRatio(options?.pixelRatio ?? 1)
    return renderer
  } catch {
    return undefined
  }
}

/**
 * Construct a live {@link ThreeSceneGraph} over Three.js with a scene-node
 * reconciler; geometry/material lifetimes ride a resource scope bridged to
 * Effect `Scope` disposal.
 *
 * Works without a GPU: the reconciler still builds a real Three.js object
 * tree. When WebGL is available (or a canvas is supplied), `render` draws the
 * frame; otherwise it is a no-op after the scene graph update.
 */
export const makeLiveThreeSceneGraph = (
  options?: LiveThreeSceneGraphOptions
): Effect.Effect<ThreeSceneGraph, never, Scope.Scope> =>
  Effect.gen(function* () {
    const root = new Three.Group()
    root.name = "effect-native-canvas-root"
    const scene = new Three.Scene()
    scene.add(root)

    const sceneScope: SceneResourceScope = createSceneResourceScope()
    const catalogue = makeEffectNativeSceneCatalogue()
    const reconciler: SceneNodeReconciler = createSceneNodeReconciler({
      root,
      catalogue,
      scope: sceneScope
    })

    const cameraHolder: { current: Three.Camera } = {
      current: new Three.PerspectiveCamera(60, 1, 0.1, 1000)
    }
    cameraHolder.current.position.set(0, 0, 5)

    const renderer = tryCreateWebGlRenderer(options)
    if (renderer !== undefined) {
      sceneScope.add(() => renderer.dispose())
    }

    // Bridge resource scope → Effect Scope finalizer.
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        reconciler.dispose()
        sceneScope.dispose()
        scene.clear()
      })
    )

    const graph: ThreeSceneGraph = {
      root,
      get threeCamera() {
        return cameraHolder.current
      },
      renderer,
      update: (descriptors) =>
        Effect.sync(() => {
          reconciler.update(descriptors as ReadonlyArray<SceneNodeDescriptor>)
        }),
      setCamera: (camera) =>
        Effect.sync(() => {
          if (camera._tag === "Perspective" && !(cameraHolder.current instanceof Three.PerspectiveCamera)) {
            cameraHolder.current = new Three.PerspectiveCamera(camera.fov, 1, camera.near, camera.far)
          } else if (camera._tag === "Orthographic" && !(cameraHolder.current instanceof Three.OrthographicCamera)) {
            const f = camera.frustum
            cameraHolder.current = new Three.OrthographicCamera(-f, f, f, -f, camera.near, camera.far)
          }
          applyCameraToThree(cameraHolder.current, camera)
        }),
      setBackground: (color) =>
        Effect.sync(() => {
          scene.background = color === undefined ? null : new Three.Color(color)
        }),
      render: (_tick) =>
        Effect.sync(() => {
          if (renderer === undefined) return
          renderer.render(scene, cameraHolder.current)
        })
    }

    return graph
  })

/**
 * Convenience: live Three.js graph + canvas backend adapter on one Scope.
 */
export const makeLiveThreeCanvasBackend = (
  options?: LiveThreeSceneGraphOptions
): Effect.Effect<CanvasBackend, never, Scope.Scope> =>
  Effect.gen(function* () {
    const graph = yield* makeLiveThreeSceneGraph(options)
    return yield* makeThreeCanvasBackend(graph)
  })
