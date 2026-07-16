/**
 * Scene-node reconciler for the Three.js canvas backend.
 *
 * Owns keyed create/update/dispose of `three.Object3D` trees with a nested
 * resource scope. This is part of `@effect-native/render-canvas` (not a
 * dependency on any product-side Three wrapper). Historical lineage: the
 * keyed-descriptor reconcile pattern is common to scene graphs; implementation
 * here is owned and versioned with Effect Native.
 */
import * as Three from "three"

export type SceneResourceFinalizer = () => void

export type SceneResourceScope = Readonly<{
  add: (finalizer: SceneResourceFinalizer) => SceneResourceFinalizer
  child: () => SceneResourceScope
  dispose: () => void
  disposed: () => boolean
  size: () => number
}>

export const createSceneResourceScope = (): SceneResourceScope => {
  let closed = false
  const finalizers: SceneResourceFinalizer[] = []

  const add = (finalizer: SceneResourceFinalizer): SceneResourceFinalizer => {
    if (closed) {
      finalizer()
      return () => undefined
    }

    let active = true
    const wrapped = () => {
      if (!active) return
      active = false
      finalizer()
    }
    finalizers.push(wrapped)

    return () => {
      if (!active) return
      active = false
      const index = finalizers.indexOf(wrapped)
      if (index >= 0) finalizers.splice(index, 1)
    }
  }

  const dispose = () => {
    if (closed) return
    closed = true
    const errors: unknown[] = []
    for (const finalizer of finalizers.splice(0).reverse()) {
      try {
        finalizer()
      } catch (error) {
        errors.push(error)
      }
    }
    if (errors.length > 0) {
      throw new AggregateError(errors, "Scene resource scope disposal failed")
    }
  }

  const child = (): SceneResourceScope => {
    const scope = createSceneResourceScope()
    add(scope.dispose)
    return scope
  }

  return {
    add,
    child,
    dispose,
    disposed: () => closed,
    size: () => finalizers.length
  }
}

export type SceneNodeDescriptor<TKind extends string = string, TProps = unknown> = Readonly<{
  id: string
  kind: TKind
  props: TProps
  children?: readonly SceneNodeDescriptor[]
}>

export type SceneNodeInstance<TState = unknown> = Readonly<{
  object: Three.Object3D
  childRoot?: Three.Object3D
  state?: TState
  dispose?: () => void
}>

export type SceneNodeRuntime<TState = unknown> = Readonly<{
  id: string
  kind: string
  object: Three.Object3D
  childRoot: Three.Object3D
  descriptor: SceneNodeDescriptor
  scope: SceneResourceScope
  state: TState | undefined
}>

export type SceneNodeFactory<TState = unknown> = Readonly<{
  create: (descriptor: SceneNodeDescriptor, scope: SceneResourceScope) => SceneNodeInstance<TState>
  update?: (runtime: SceneNodeRuntime<TState>, descriptor: SceneNodeDescriptor) => boolean | void
}>

export type SceneNodeCatalogue = Readonly<Record<string, SceneNodeFactory>>

export type SceneNodeReconciler = Readonly<{
  update: (descriptors: readonly SceneNodeDescriptor[]) => void
  dispose: () => void
  entries: () => readonly SceneNodeRuntime[]
  get: (id: string) => SceneNodeRuntime | undefined
}>

type MutableSceneNodeRuntime = {
  id: string
  kind: string
  object: Three.Object3D
  childRoot: Three.Object3D
  children: Map<string, MutableSceneNodeRuntime>
  descriptor: SceneNodeDescriptor
  scope: SceneResourceScope
  state: unknown
}

export type SceneNodeReconcilerOptions = Readonly<{
  root: Three.Object3D
  catalogue: SceneNodeCatalogue
  scope?: SceneResourceScope
}>

const readonlyRuntime = (runtime: MutableSceneNodeRuntime): SceneNodeRuntime => ({
  id: runtime.id,
  kind: runtime.kind,
  object: runtime.object,
  childRoot: runtime.childRoot,
  descriptor: runtime.descriptor,
  scope: runtime.scope,
  state: runtime.state
})

const disposeRuntime = (runtime: MutableSceneNodeRuntime): void => {
  runtime.scope.dispose()
}

export const createSceneNodeReconciler = (options: SceneNodeReconcilerOptions): SceneNodeReconciler => {
  const rootScope = options.scope ?? createSceneResourceScope()
  const runtimes = new Map<string, MutableSceneNodeRuntime>()

  const createRuntime = (
    descriptor: SceneNodeDescriptor,
    parent: Three.Object3D,
    parentScope: SceneResourceScope
  ): MutableSceneNodeRuntime => {
    const factory = options.catalogue[descriptor.kind]
    if (factory === undefined) {
      throw new Error(`No scene node factory registered for ${descriptor.kind}`)
    }

    const scope = parentScope.child()
    const instance = factory.create(descriptor, scope)
    const runtime: MutableSceneNodeRuntime = {
      id: descriptor.id,
      kind: descriptor.kind,
      object: instance.object,
      childRoot: instance.childRoot ?? instance.object,
      children: new Map(),
      descriptor,
      scope,
      state: instance.state
    }

    parent.add(runtime.object)
    scope.add(() => runtime.object.removeFromParent())
    if (instance.dispose !== undefined) {
      scope.add(instance.dispose)
    }

    reconcileChildren(runtime.childRoot, runtime.scope, runtime.children, descriptor.children ?? [])
    return runtime
  }

  const updateRuntime = (runtime: MutableSceneNodeRuntime, descriptor: SceneNodeDescriptor): boolean => {
    const factory = options.catalogue[descriptor.kind]
    if (factory === undefined) {
      throw new Error(`No scene node factory registered for ${descriptor.kind}`)
    }

    const keepRuntime = factory.update?.(readonlyRuntime(runtime), descriptor) ?? true
    if (keepRuntime === false) return false
    runtime.descriptor = descriptor
    reconcileChildren(runtime.childRoot, runtime.scope, runtime.children, descriptor.children ?? [])
    return true
  }

  function reconcileChildren(
    parent: Three.Object3D,
    parentScope: SceneResourceScope,
    children: Map<string, MutableSceneNodeRuntime>,
    descriptors: readonly SceneNodeDescriptor[]
  ): void {
    const activeIds = new Set(descriptors.map((descriptor) => descriptor.id))
    for (const [id, runtime] of [...children.entries()]) {
      if (!activeIds.has(id)) {
        children.delete(id)
        disposeRuntime(runtime)
      }
    }

    for (const descriptor of descriptors) {
      const existing = children.get(descriptor.id)
      if (existing === undefined || existing.kind !== descriptor.kind) {
        if (existing !== undefined) disposeRuntime(existing)
        const runtime = createRuntime(descriptor, parent, parentScope)
        children.set(descriptor.id, runtime)
        continue
      }

      if (!updateRuntime(existing, descriptor)) {
        children.delete(descriptor.id)
        disposeRuntime(existing)
        const runtime = createRuntime(descriptor, parent, parentScope)
        children.set(descriptor.id, runtime)
      }
    }

    for (const descriptor of descriptors) {
      const runtime = children.get(descriptor.id)
      if (runtime !== undefined) parent.add(runtime.object)
    }
  }

  return {
    update: (descriptors) => reconcileChildren(options.root, rootScope, runtimes, descriptors),
    dispose: () => {
      for (const runtime of [...runtimes.values()].reverse()) {
        disposeRuntime(runtime)
      }
      runtimes.clear()
      if (options.scope === undefined) rootScope.dispose()
    },
    entries: () => [...runtimes.values()].map(readonlyRuntime),
    get: (id) => {
      const runtime = runtimes.get(id)
      return runtime === undefined ? undefined : readonlyRuntime(runtime)
    }
  }
}
