# Three.js Integration with Effuse Components

Speculation on how Three.js can work with Effuse's Effect-native component system.

**Note:** Effuse currently uses the term "component" throughout the codebase, but it's in the process of being refactored to use "component" instead. This document uses "component" to reflect the intended terminology.

## The Challenge

Effuse components use `innerHTML` replacement for rendering, which means:
- Every state change triggers a full DOM replacement
- Canvas elements and WebGL contexts would be destroyed on re-render
- Three.js objects (Scene, Camera, Renderer) aren't serializable
- Animation loops need to run independently of state updates

## Integration Patterns

### Pattern 1: Canvas as Persistent Element (Recommended)

**Concept:** Render the canvas element once, never re-render it. Manage Three.js objects outside the reactive state system.

```typescript
import { Effect, Stream, pipe } from "effect"
import { html } from "../effuse/index.js"
import type { Component } from "../effuse/component/types.js"
import * as THREE from "three"

interface ThreeSceneState {
  // Only serializable state
  cameraPosition: { x: number; y: number; z: number }
  rotationSpeed: number
  backgroundColor: string
  // Three.js objects stored separately (not in state)
}

type ThreeSceneEvent =
  | { type: "setRotationSpeed"; speed: number }
  | { type: "setCameraPosition"; x: number; y: number; z: number }

export const ThreeSceneComponent: Component<ThreeSceneState, ThreeSceneEvent> = {
  id: "three-scene",

  initialState: () => ({
    cameraPosition: { x: 0, y: 0, z: 5 },
    rotationSpeed: 0.01,
    backgroundColor: "#000000",
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get

      // Render canvas element (only once, never re-rendered)
      return html`
        <div class="three-container">
          <canvas
            id="${ctx.container.id}-three-canvas"
            width="800"
            height="600"
            style="display: block; width: 100%; height: 100%;"
          ></canvas>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Initialize Three.js scene AFTER canvas is rendered
      const canvas = yield* ctx.dom.queryId<HTMLCanvasElement>(
        `${ctx.container.id}-three-canvas`
      )

      // Create Three.js objects (stored in closure, not state)
      const scene = new THREE.Scene()
      const camera = new THREE.PerspectiveCamera(
        75,
        canvas.width / canvas.height,
        0.1,
        1000
      )
      const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
      renderer.setSize(canvas.width, canvas.height)

      // Create objects
      const geometry = new THREE.BoxGeometry(1, 1, 1)
      const material = new THREE.MeshStandardMaterial({ color: 0x00ff00 })
      const cube = new THREE.Mesh(geometry, material)
      scene.add(cube)

      // Store Three.js objects in a WeakMap or closure
      // (not in StateCell - they're not serializable)
      const threeObjects = { scene, camera, renderer, cube }

      // Initial camera position from state
      const initialState = yield* ctx.state.get
      camera.position.set(
        initialState.cameraPosition.x,
        initialState.cameraPosition.y,
        initialState.cameraPosition.z
      )

      // Animation loop (runs independently)
      let animationId: number | null = null
      const animate = () => {
        animationId = requestAnimationFrame(animate)

        // Read current state (reactive)
        const currentState = ctx.state.get // This is an Effect, need to run it
        // For now, use a ref or closure variable

        cube.rotation.x += initialState.rotationSpeed
        cube.rotation.y += initialState.rotationSpeed
        renderer.render(scene, camera)
      }
      animate()

      // Subscribe to state changes to update Three.js
      yield* pipe(
        ctx.state.changes,
        Stream.tap((state) =>
          Effect.sync(() => {
            // Update Three.js objects based on state
            camera.position.set(
              state.cameraPosition.x,
              state.cameraPosition.y,
              state.cameraPosition.z
            )
            // Update rotation speed (stored in closure)
            // Update background color
            scene.background = new THREE.Color(state.backgroundColor)
          })
        ),
        Stream.runDrain,
        Effect.forkScoped
      )

      // Cleanup on unmount
      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          if (animationId !== null) {
            cancelAnimationFrame(animationId)
          }
          renderer.dispose()
          geometry.dispose()
          material.dispose()
        })
      )
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "setRotationSpeed":
          yield* ctx.state.update((s) => ({
            ...s,
            rotationSpeed: event.speed,
          }))
          break
        case "setCameraPosition":
          yield* ctx.state.update((s) => ({
            ...s,
            cameraPosition: { x: event.x, y: event.y, z: event.z },
          }))
          break
      }
    }),
}
```

**Key Points:**
- Canvas rendered once, never re-rendered
- Three.js objects stored in closure/WeakMap (not StateCell)
- State changes update Three.js objects directly (not via re-render)
- Animation loop runs independently
- Cleanup handled via Effect finalizers

### Pattern 2: ThreeService (More Structured)

**Concept:** Create a `ThreeService` similar to `DomService` that manages Three.js scenes.

```typescript
// src/effuse/services/three.ts
export interface ThreeService {
  createScene: (
    canvas: HTMLCanvasElement,
    options?: SceneOptions
  ) => Effect.Effect<ThreeScene, ThreeError>

  updateScene: (
    sceneId: string,
    updater: (scene: ThreeScene) => void
  ) => Effect.Effect<void, ThreeError>

  disposeScene: (sceneId: string) => Effect.Effect<void, never>
}

export interface ThreeScene {
  id: string
  scene: THREE.Scene
  camera: THREE.Camera
  renderer: THREE.WebGLRenderer
  objects: Map<string, THREE.Object3D>
}

// Usage in component
export const ThreeSceneComponent: Component<State, Event, ThreeServiceTag> = {
  // ...
  setupEvents: (ctx) =>
    Effect.gen(function* () {
      const three = yield* ThreeServiceTag
      const canvas = yield* ctx.dom.queryId<HTMLCanvasElement>("canvas")

      const scene = yield* three.createScene(canvas)

      // Subscribe to state changes
      yield* pipe(
        ctx.state.changes,
        Stream.tap((state) =>
          three.updateScene(scene.id, (s) => {
            // Update scene based on state
            s.camera.position.set(state.x, state.y, state.z)
          })
        ),
        Stream.runDrain,
        Effect.forkScoped
      )

      // Cleanup
      yield* Effect.addFinalizer(() => three.disposeScene(scene.id))
    }),
}
```

**Benefits:**
- Centralized Three.js management
- Type-safe scene operations
- Automatic cleanup
- Testable (mockable service)

### Pattern 3: Hybrid - Canvas Container + Direct Updates

**Concept:** Render a container div, mount canvas via direct DOM manipulation, update Three.js based on state.

```typescript
render: (ctx) =>
  Effect.gen(function* () {
    // Render container (can be re-rendered safely)
    return html`
      <div class="three-scene-container" data-scene-id="${ctx.container.id}">
        <!-- Canvas will be created in setupEvents -->
      </div>
    `
  }),

setupEvents: (ctx) =>
  Effect.gen(function* () {
    const container = yield* ctx.dom.query(".three-scene-container")

    // Create canvas directly (not via render)
    const canvas = document.createElement("canvas")
    canvas.width = 800
    canvas.height = 600
    container.appendChild(canvas)

    // Initialize Three.js...
    // Update based on state changes...
  }),
```

## State Management Strategy

**Critical Decision:** What goes in StateCell vs. what stays outside?

### ✅ In StateCell (Serializable)
- Camera position/orientation
- Object positions/rotations (as numbers)
- Material colors (as hex strings)
- Animation speeds
- Scene settings (fog, background color)

### ❌ Outside StateCell (Not Serializable)
- `THREE.Scene` objects
- `THREE.Camera` objects
- `THREE.Renderer` objects
- `THREE.Mesh`, `THREE.Geometry`, `THREE.Material` objects
- WebGL contexts
- Animation frame IDs

### Storage Options

1. **Closure Variables** (Simple)
   ```typescript
   setupEvents: (ctx) => {
     const scene = new THREE.Scene() // In closure
     // ...
   }
   ```

2. **WeakMap** (Multiple Scenes)
   ```typescript
   const sceneMap = new WeakMap<Element, ThreeScene>()
   sceneMap.set(ctx.container, { scene, camera, renderer })
   ```

3. **ThreeService Registry** (Structured)
   ```typescript
   const scenes = new Map<string, ThreeScene>()
   ```

## Animation Loop Integration

**Challenge:** Animation loops need to run continuously, but state updates are reactive.

**Solution:** Run animation loop independently, read state reactively:

```typescript
setupEvents: (ctx) =>
  Effect.gen(function* () {
    // ... create Three.js objects ...

    // Animation loop (independent of state updates)
    let animationId: number | null = null
    const animate = () => {
      animationId = requestAnimationFrame(animate)

      // Read current state (synchronous, from closure variable)
      // Or use a ref that's updated by state.changes stream

      cube.rotation.x += rotationSpeedRef.current
      renderer.render(scene, camera)
    }
    animate()

    // Update rotation speed when state changes
    yield* pipe(
      ctx.state.changes,
      Stream.tap((state) =>
        Effect.sync(() => {
          rotationSpeedRef.current = state.rotationSpeed
        })
      ),
      Stream.runDrain,
      Effect.forkScoped
    )

    // Cleanup
    yield* Effect.addFinalizer(() =>
      Effect.sync(() => {
        if (animationId !== null) cancelAnimationFrame(animationId)
      })
    )
  }),
```

## Re-render Protection

**Problem:** Effuse re-renders on state changes, which would destroy the canvas.

**Solutions:**

1. **Never Re-render Canvas Container** (Recommended)
   - Render canvas once in `setupEvents`
   - Use direct DOM manipulation for updates
   - State changes update Three.js objects, not DOM

2. **Conditional Rendering**
   ```typescript
   render: (ctx) =>
     Effect.gen(function* () {
       const state = yield* ctx.state.get

       // Only render UI controls, not canvas
       return html`
         <div class="three-component">
           <canvas id="canvas" data-persist="true"></canvas>
           <div class="controls">
             <input type="range" data-action="setSpeed" />
           </div>
         </div>
       `
     }),
   ```
   - Mark canvas with `data-persist="true"`
   - Mount system skips re-rendering persistent elements

3. **Custom Render Strategy**
   - Extend `DomService` with `renderPreserving` method
   - Only updates non-persistent elements

## Example: Full Three.js Component

```typescript
import { Effect, Stream, pipe, Ref } from "effect"
import { html } from "../effuse/index.js"
import type { Component } from "../effuse/component/types.js"
import * as THREE from "three"

interface CubeSceneState {
  rotationSpeed: number
  color: string
  cameraZ: number
}

type CubeSceneEvent =
  | { type: "setSpeed"; speed: number }
  | { type: "setColor"; color: string }
  | { type: "setCameraZ"; z: number }

export const CubeSceneComponent: Component<CubeSceneState, CubeSceneEvent> = {
  id: "cube-scene",

  initialState: () => ({
    rotationSpeed: 0.01,
    color: "#00ff00",
    cameraZ: 5,
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      // Render container with canvas (canvas created once)
      return html`
        <div class="three-cube-scene">
          <canvas
            id="${ctx.container.id}-canvas"
            width="800"
            height="600"
            style="display: block; width: 100%; height: 100%;"
          ></canvas>
          <div class="controls">
            <input
              type="range"
              min="0"
              max="0.1"
              step="0.001"
              value="${(yield* ctx.state.get).rotationSpeed}"
              data-action="setSpeed"
            />
          </div>
        </div>
      `
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      // Get canvas
      const canvas = yield* ctx.dom.queryId<HTMLCanvasElement>(
        `${ctx.container.id}-canvas`
      )

      // Create Three.js scene
      const scene = new THREE.Scene()
      const camera = new THREE.PerspectiveCamera(
        75,
        canvas.width / canvas.height,
        0.1,
        1000
      )
      const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
      renderer.setSize(canvas.width, canvas.height)

      // Create cube
      const geometry = new THREE.BoxGeometry(1, 1, 1)
      const material = new THREE.MeshStandardMaterial({ color: 0x00ff00 })
      const cube = new THREE.Mesh(geometry, material)
      scene.add(cube)

      // Add lighting
      const light = new THREE.DirectionalLight(0xffffff, 1)
      light.position.set(5, 5, 5)
      scene.add(light)
      scene.add(new THREE.AmbientLight(0x404040))

      // Store mutable refs for animation loop
      const rotationSpeedRef = yield* Ref.make(0.01)
      const colorRef = yield* Ref.make("#00ff00")
      const cameraZRef = yield* Ref.make(5)

      // Initial setup from state
      const initialState = yield* ctx.state.get
      yield* Ref.set(rotationSpeedRef, initialState.rotationSpeed)
      yield* Ref.set(colorRef, initialState.color)
      yield* Ref.set(cameraZRef, initialState.cameraZ)
      camera.position.z = initialState.cameraZ

      // Animation loop
      let animationId: number | null = null
      const animate = () => {
        animationId = requestAnimationFrame(animate)

        // Read from refs (synchronous)
        const speed = Ref.unsafeMake(rotationSpeedRef).get // Need to run Effect
        // Actually, use a closure variable instead:
        // let currentSpeed = initialState.rotationSpeed

        cube.rotation.x += currentSpeed
        cube.rotation.y += currentSpeed
        renderer.render(scene, camera)
      }
      animate()

      // Update refs when state changes
      yield* pipe(
        ctx.state.changes,
        Stream.tap((state) =>
          Effect.gen(function* () {
            yield* Ref.set(rotationSpeedRef, state.rotationSpeed)
            yield* Ref.set(colorRef, state.color)
            yield* Ref.set(cameraZRef, state.cameraZ)

            // Update Three.js objects
            camera.position.z = state.cameraZ
            material.color.setHex(parseInt(state.color.replace("#", "0x")))
          })
        ),
        Stream.runDrain,
        Effect.forkScoped
      )

      // Event delegation for controls
      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action]",
        "input",
        (e, target) => {
          const action = (target as HTMLElement).dataset.action
          if (action === "setSpeed") {
            const value = parseFloat((target as HTMLInputElement).value)
            Effect.runFork(ctx.emit({ type: "setSpeed", speed: value }))
          }
        }
      )

      // Cleanup
      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          if (animationId !== null) {
            cancelAnimationFrame(animationId)
          }
          renderer.dispose()
          geometry.dispose()
          material.dispose()
        })
      )
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      switch (event.type) {
        case "setSpeed":
          yield* ctx.state.update((s) => ({
            ...s,
            rotationSpeed: event.speed,
          }))
          break
        case "setColor":
          yield* ctx.state.update((s) => ({ ...s, color: event.color }))
          break
        case "setCameraZ":
          yield* ctx.state.update((s) => ({ ...s, cameraZ: event.z }))
          break
      }
    }),
}
```

## Key Insights

1. **Canvas is Persistent** - Render once, never re-render
2. **State is Reactive, Three.js is Direct** - State changes update Three.js objects directly
3. **Animation Loop is Independent** - Runs separately from state updates
4. **Cleanup via Effect Finalizers** - Proper resource management
5. **Type Safety** - Full TypeScript support for both Effuse and Three.js

## Benefits of This Approach

- ✅ Full TypeScript support
- ✅ Reactive state management (Effuse)
- ✅ Powerful 3D rendering (Three.js)
- ✅ Proper cleanup (Effect finalizers)
- ✅ Testable (mock Three.js in tests)
- ✅ No bundling (ESM modules)

## Potential Extensions

1. **ThreeService** - Centralized scene management
2. **Three.js Hooks** - Reusable patterns for common Three.js operations
3. **Scene Serialization** - Save/load scene state (serializable parts only)
4. **Multi-Scene Support** - Multiple Three.js scenes in one component
5. **WebGL Context Sharing** - Optimize multiple canvases
