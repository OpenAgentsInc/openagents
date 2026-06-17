# Tassadar WASD + Mouselook Controller Plan

Date: 2026-06-17
Status: Implemented and verified.
Goal: make `/tassadar` navigable with a first-person WASD + mouselook controller,
starting as a 2.5D walkable version of the current run scene and moving toward a
proper 3D MMO world.

## Implementation Status

- `three-effect#1` is implemented in
  `OpenAgentsInc/three-effect@0e6d5a9`: `playerControllerPrimitives.ts` now
  owns the reusable WASD + pointer-lock mouselook controller primitive.
- `three-effect#2` is implemented in
  `OpenAgentsInc/three-effect@7914994`: `trainingRunView` now supports
  `cameraMode: "perspective_walk"`, `controller: "wasd_mouselook"`, a 2.5D
  ground-plane view, and center-reticle raycasting while pointer lock is active.
- `OpenAgentsInc/three-effect@d4a5ca4` fixes the first browser-smoke gap:
  entity text labels now stay camera-facing during perspective walk/mouselook by
  using billboarded text-label handles and updating `faceCamera(camera)` each
  frame.
- `openagents#5219` is implemented in this change: `/tassadar` pins the updated
  `@openagentsinc/three-effect`, passes the perspective/controller options into
  the live training-run element, renders a small `Enter run` affordance before
  the Three scene mounts, and exposes pointer-lock state through
  `data-pointer-lock`.
- `openagents#5220` verification: local browser smoke confirmed `/tassadar`
  enters pointer lock, supports WASD movement, and releases with Escape. The
  smoke also found the camera-facing label bug above; the app now pins the
  fixed `three-effect` commit. A follow-up local browser smoke loaded the route
  with the live public summary stubbed into the local app, confirmed the
  `Enter run` control and training scene host render, dispatched a settlement
  selection, and verified the linked proof drawer appears.
- `openagents#5221` is the final tracker closeout for the completed issue
  sequence.
- `OpenAgentsInc/three-effect@2c7b2ad` is the current camera-control audit fix:
  the WASD controller now owns pointer-lock mouse deltas, uses
  `document.pointerLockElement` as well as the Three control flag when deciding
  whether mouselook is active, and emits throttled
  `[three-effect:wasd_mouselook]` console samples for lock/unlock/mousemove.
- `/tassadar` pins that commit and enables controller debug logging for the
  live scene, so a locked cursor with no camera rotation can be diagnosed from
  the browser console by checking whether mousemove samples arrive and whether
  their `movementX` / `movementY` values are non-zero.
- `OpenAgentsInc/three-effect@435afe0` fixes the re-entry failure observed in
  Chrome after Escape/unlock: the scene now requests pointer lock from a real
  `click` gesture rather than `pointerdown`, suppresses Three's default
  `pointerlockerror` console dump, catches rejected pointer-lock promises, and
  emits `lock_error` diagnostics through `[three-effect:wasd_mouselook]`.
- `OpenAgentsInc/three-effect@d547c0e` hardens the still-failing movement path:
  while pointer lock is active anywhere in the scene document, the controller
  now accepts `mousemove`, `pointermove`, and `pointerrawupdate` deltas and logs
  movement snapshots with `console.warn` instead of `console.info`.
- `/tassadar` now passes the same page-owned debug callback through both the
  initial Worker-summary render and the SpacetimeDB update render. The callback
  writes `data-mouselook-*` attributes on `<oa-tassadar-run>` and emits
  `[tassadar:mouselook]` warnings, so a locked cursor with no camera motion can
  be diagnosed by checking whether the browser is emitting non-zero deltas and
  whether those deltas were applied.

## References Read

- `projects/repos/drei/src/core/PointerLockControls.tsx`
- `projects/repos/drei/src/core/FirstPersonControls.tsx`
- `projects/repos/drei/src/core/FlyControls.tsx`
- `projects/repos/drei/src/web/KeyboardControls.tsx`
- `projects/repos/drei/.storybook/stories/PointerLockControls.stories.tsx`
- `projects/repos/drei/.storybook/stories/KeyboardControls.stories.tsx`
- `projects/repos/react-three-fiber/docs/API/hooks.mdx`
- `projects/repos/react-three-fiber/docs/API/events.mdx`
- `projects/repos/Quick_3D_MMORPG/client/src/player-input.js`
- `projects/repos/Quick_3D_MMORPG/client/src/player-entity.js`
- `projects/repos/Quick_3D_MMORPG/client/src/third-person-camera.js`
- `/Users/christopherdavid/work/three-effect/packages/core/src/extraControlsPrimitives.ts`
- `/Users/christopherdavid/work/three-effect/packages/core/src/trainingRun.ts`
- `apps/openagents.com/apps/web/src/page/run.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts`

## Current State

`/tassadar` is already a full-bleed page shell around the self-fetching
`<oa-tassadar-run>` element. That wrapper fetches
`/api/public/tassadar-run-summary`, maps it through
`tassadarRunVisualizationOptions`, then mounts the Foldkit
`<oa-training-run>` element from `@openagentsinc/three-effect`.

The underlying `trainingRunView` currently renders with an orthographic camera:

- camera: `OrthographicCamera(-5, 5, 3, -3, 0.1, 100)`;
- camera position: `(0, 0, 10)`;
- scene layout: mostly XY coordinates with shallow Z layering;
- interaction: raycast against circle hit targets from pointer coordinates;
- motion: mostly decorative pulses/lines from the existing visualization.

That means the current scene is 3D technically, but experienced as a 2D diagram.
The first controller pass should not rewrite the run data model. It should make
the existing scene walkable in a perspective/2.5D mode.

## Reference Lessons

### Drei

Drei splits the problem into reusable controls:

- `PointerLockControls` owns browser pointer lock and mouse-look.
- `FirstPersonControls` and `FlyControls` provide camera update loops.
- `KeyboardControls` maps raw keys into named action state, then a frame loop
  reads that state and applies velocity.

The important parts to port are not React-specific. The useful primitives are:

- selector-based "click to play" locking;
- lock/unlock callbacks;
- event cleanup on unmount;
- centered raycasting while pointer lock is active;
- named key state instead of ad hoc keydown logic;
- frame-loop update with `delta` seconds.

### React Three Fiber

R3F reinforces two implementation rules:

- per-frame controller work should be tiny and avoid reactive state writes;
- pointer/raycast event computation needs to be explicit when the camera moves
  or pointer lock changes how the cursor is interpreted.

For `three-effect`, that means the controller should expose an `update(delta)`
method and the scene should call it inside its existing animation loop. Selection
under pointer lock should use a center reticle ray, not stale browser pointer
coordinates.

### Quick_3D_MMORPG

The sample MMORPG uses:

- a dedicated input component with `forward/backward/left/right/shift` keys;
- velocity, acceleration, and deceleration rather than direct position jumps;
- movement relative to the controlled object's quaternion;
- terrain height sampling;
- collision checks against nearby entities;
- a camera component that lerps toward an ideal offset/look-at.

For OpenAgents, do not copy that code directly. The useful pattern is the
separation between input state, movement integration, terrain/collision queries,
and camera update. P0 can skip animation state machines and full collision, but
the API should leave room for them.

## three-effect Primitive To Add

Add a new core primitive in `three-effect`:

`packages/core/src/playerControllerPrimitives.ts`

Suggested export:

```ts
export type WasdMouseLookControllerOptions = Readonly<{
  enabled?: boolean
  lockSelector?: string
  inputTarget?: HTMLElement | Window
  initialPosition?: readonly [number, number, number]
  eyeHeight?: number
  movementSpeed?: number
  sprintMultiplier?: number
  acceleration?: number
  damping?: number
  pitchMin?: number
  pitchMax?: number
  bounds?: Readonly<{
    minX: number
    maxX: number
    minZ: number
    maxZ: number
  }>
  groundHeightAt?: (x: number, z: number) => number
  onLockChange?: (locked: boolean) => void
}>

export type WasdMouseLookControllerHandle = Readonly<{
  controls: PointerLockControls
  update: (delta: number) => Effect.Effect<void>
  lock: Effect.Effect<void>
  unlock: Effect.Effect<void>
  isLocked: Effect.Effect<boolean>
  getPosition: Effect.Effect<Three.Vector3>
  setPosition: (position: Three.Vector3) => Effect.Effect<void>
  dispose: Effect.Effect<void>
}>
```

Implementation shape:

- use the existing `createPointerLockControls` primitive for mouse-look;
- keep a named key map internally: `forward`, `backward`, `left`, `right`,
  `sprint`, `rise`, `fall` for future fly/ghost mode;
- listen to `event.code` (`KeyW`, `KeyA`, `KeyS`, `KeyD`, `ShiftLeft`,
  `ShiftRight`, `Space`, `KeyC`);
- ignore keydown/keyup when the target is an input, textarea, select, or
  contenteditable element;
- compute movement from the camera yaw projected onto the XZ plane;
- keep Y fixed at `groundHeightAt(x, z) + eyeHeight` for P0;
- clamp to bounds;
- apply acceleration/damping so movement feels like a controller, not a cursor;
- clean up all DOM listeners and pointer-lock listeners in `dispose`.

This belongs in `three-effect` because every future game/HUD surface will need
the same input primitive: Tassadar, the MMO world client, Forge factory flythrough,
and any cockpit scenes.

## Training Run Scene Change

Add a camera/controller option to `TrainingRunVisualizationOptions`:

```ts
cameraMode?: "orthographic_map" | "perspective_walk"
controller?: "none" | "wasd_mouselook"
walkController?: WasdMouseLookControllerOptions
```

Default stays exactly as today:

- `cameraMode: "orthographic_map"`;
- `controller: "none"`.

For `perspective_walk`, `trainingRunView` should:

1. Use a `PerspectiveCamera`.
2. Place the camera at a safe starting point, e.g. `(0, 1.65, 7)`.
3. Convert the current XY diagram into a 2.5D ground plane by putting run
   entities on XZ and using Y for height.
4. Keep labels billboarded toward the camera.
5. Keep node hit targets raycastable.
6. Add a faint floor/grid so movement has spatial reference.
7. Use center-reticle selection while pointer lock is active.

Do not remove the orthographic mode. It is the fallback and the safer test
baseline.

## `/tassadar` Integration

`apps/openagents.com/apps/web/src/scene/tassadarRunElement.ts` should enable the
new mode by passing visualization options to `<oa-training-run>`:

```ts
run.visualization = {
  ...tassadarRunVisualizationOptions(summary),
  cameraMode: "perspective_walk",
  controller: "wasd_mouselook",
  walkController: {
    lockSelector: "[data-tassadar-enter-world]",
    movementSpeed: 4.5,
    sprintMultiplier: 1.8,
    eyeHeight: 1.65,
    bounds: { minX: -8, maxX: 8, minZ: -8, maxZ: 8 },
  },
}
```

The wrapper should add a small overlay button:

- label: `Enter run`;
- attribute: `data-tassadar-enter-world`;
- hidden or dimmed once locked;
- helper text: `WASD move / mouse look / Esc release`.

The overlay must not become another dashboard. It is a pointer-lock affordance
only. Existing status and selected-proof panels remain regular HTML overlays and
should be usable when pointer lock is released.

## Selection Rules

When pointer lock is inactive:

- keep current pointer-based raycasting.

When pointer lock is active:

- raycast from the center of the screen;
- show a subtle center reticle;
- click selects the entity under the reticle;
- never make hovering or looking at a thing imply work is happening.

Selection still routes through `node-selected` and the existing proof-link logic.

## 2.5D First, 3D Next

P0 should be deliberately 2.5D:

- camera can walk/fly around;
- nodes are placed on a ground plane;
- rings, labels, and proof objects gain depth;
- no physics engine;
- no avatar mesh;
- no multiplayer presence yet;
- no invented motion.

P1 can add:

- collision bounds around large entities;
- a nav floor/terrain height callback;
- inspectable 3D proof portals;
- optional third-person camera/ghost avatar;
- camera presets: map view, walk view, selected-entity focus.

P2 can add:

- SpacetimeDB-backed player presence;
- multiplayer operator ghosts;
- region streaming;
- persisted user location;
- proper avatar/controller animation if the MMO client needs it.

## Testing Plan

`three-effect`:

- unit-test key-state transitions and input cleanup;
- unit-test movement vector math for yaw-relative WASD;
- unit-test bounds clamping;
- add a small mount/dispose test for pointer-lock controller construction where
  browser APIs can be stubbed.

`openagents.com`:

- test that `/tassadar` passes `cameraMode: "perspective_walk"` and
  `controller: "wasd_mouselook"` into the custom element;
- test that the enter-world overlay renders;
- preserve existing tests for loading/error/selection/proof-link behavior.

Browser smoke:

- done: run `/tassadar` locally;
- done: click `Enter run`;
- done: verify pointer lock enters;
- done: move with W/A/S/D;
- done: verify Esc unlocks;
- follow-up fixed: text labels over entity nodes did not face the camera;
- done: verify proof selection still works after the label-fix pin;
- done: verify no fake flow or unbacked motion was added.

## Implementation Order

1. Done: add `playerControllerPrimitives.ts` in `three-effect`.
2. Done: export it from `packages/core/src/index.ts`.
3. Done: add `cameraMode` / `controller` options to `trainingRun.ts`.
4. Done: implement perspective-walk rendering as an additive mode.
5. Done: add center-reticle raycasting for locked pointer mode.
6. Done: consume the new options in `tassadarRunElement.ts`.
7. Done: add the tiny `/tassadar` enter-world overlay.
8. Done: run the full deployment-facing checks and final proof-selection smoke
   after the label-fix pin.

This keeps the reusable controller in `three-effect` while letting the
OpenAgents page decide when the controller is enabled for the live run.
