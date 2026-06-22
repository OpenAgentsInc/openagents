# WoC Input, Camera, Targeting, and Mobile

Date: 2026-06-22
Scope: `src/game/` (input, keybinds, camera, click-move, pointer-pick, interactions, mobile), `src/render/camera_collision.ts`.

The Verse already has a WASD + mouselook third-person controller, tab-target, and
jump/sprint (per the Tassadar controller plan and #5943 keybindings work). This is the
subsystem with the most direct overlap, so the focus here is on the specific pure modules
worth lifting, not the gameplay.

## Keybind registry and remapping

`src/game/keybinds.ts` (~243 lines, pure, no DOM) is the part to adopt and the part that
most directly overlaps our **custom keybindings effort (#5943, audit
`2026-06-22-verse-custom-keybindings-audit.md`)**.

- **Registry pattern.** A central `BIND_ACTIONS` array declares each action's `id`,
  `label`, `category`, `kind` (`held` vs `edge`), and up to two default key codes. All ~25
  actions live in one place.
- **Bidirectional map.** `actionForCode()` and `codesForAction()`; one code per action with
  WoW-style mutual eviction on rebind (`bind()`), an `allowShared` exception for overlapping
  cases (Attack-Move shares the Turn-Left key), and a reserved `Escape`.
- **Persistence.** Loads from `localStorage` (`woc_keybinds`) with fallback to defaults,
  stored as `{ actionId: [primary, secondary] }`.
- **Capture.** `captureNextKey()` powers the rebind UI.

Dispatch lives in `src/game/input.ts` (~670 lines): `onKeyDown` resolves the action via
`actionForCode()`, routes `held` actions into a polled `keys` Set (read each frame in
`readMoveInput()`) and `edge` actions into `dispatchEdge()`, which splits into ability-bar
slots (`cb.onAbility(slot)`) vs UI keys (`cb.onUiKey(category)`). A `canUseGameKeys()` gate
suppresses game input when a modal or the chat composer is focused: a simple, effective
**input-context** mechanism.

### Relevance to us

**Adopt the `Keybinds` + `BIND_ACTIONS` registry directly into the #5943 lane.** It is
pure, persisted, remappable, and already has the category/kind taxonomy and the
held-vs-edge split our keybinding audit calls for. The `canUseGameKeys()` modal/chat gate
is the cheap version of the "input contexts" that audit wants; adopt it as the baseline and
layer richer contexts on top. The `tests/keybinds.test.ts` pattern (localStorage stub)
shows how to test ours.

## Third-person camera

State lives on the input layer: `camYaw` (orbit), `camPitch` (clamped ~-0.4..1.35),
`camDist` (clamped 3..22 yd).

- **Two modes.** Classic (WASD turns the player; left-drag orbits; right-drag mouselooks
  with pointer lock) and a Mouse-Camera mode (WASD is camera-relative, drag orbits, no
  keyboard turn). Drag activates only after 18 px or 140 ms so a click does not orbit;
  pointer lock is requested lazily to avoid permission-banner spam.
- **Follow/settle** (`camera_follow.ts`, ~107 lines, pure, tested). When not in mouselook
  and the player is moving, the camera eases back behind the player, capped at
  `MAX_AUTO_YAW_SPEED` (3.6 rad/s). Click-to-move gets a gentler cubic-eased curve. Under
  player control (mouselook), auto-follow is bypassed and the player faces the camera.
- **Collision** (`src/render/camera_collision.ts`, ~67 lines, pure). Hard limit (raycast
  hit) + soft limit (breathing room) + FOV compensation so the unexpected pull-in does not
  pop; eases in faster than it eases out (`stepCameraOcclusion`).

### Relevance to us

**Adopt `camera_follow.ts` and `camera_collision.ts` as pure math into the
`three-effect` controller.** We are mouselook-first, so we skip orbit-on-left-drag and the
Mouse-Camera mode, but the auto-settle-behind logic and occlusion easing are exactly the
"feel" upgrades a fresh controller lacks, and they are framework-free. These belong in
`three-effect` per our "build UI with three-effect first" rule.

## Targeting and pointer picking

- **Tab cycle.** Edge action `target` (Tab) -> `world.tabTarget()`; friendly variants on H
  / J. We already have tab-target.
- **Pointer pick** (`src/game/pointer_pick.ts`, ~34 lines, pure). `clickPickFromMouseGesture`
  decides whether a mouse-up is a target click vs a camera drag: same button down/up,
  released on canvas or pointer-locked, duration <= 280 ms, right-click drag <= 18 px. This
  solves the classic "my target click turned into a camera orbit" bug.
- **Interaction dispatch** (`src/game/interactions.ts`, ~127 lines, pure, tested via injected
  `PickInteractionWorld` / `PickInteractionHud` interfaces). Left/right-click on an entity
  routes to target / loot / quest-dialog / pick-up / auto-attack with a range check and a
  "too far" error toast.

### Relevance to us

**Adopt `pointer_pick.ts` wholesale** the moment the Verse has click-to-select on world
entities (Pylons, run cores, assignment markers): it is tiny, pure, and removes a whole
class of input-feel bugs. **Adopt the injected-interface pattern from `interactions.ts`**
so our F-to-interact (inspect a Pylon, open a run board, read a proof) is unit-testable
without a live world. The range-check + error-toast UX is a good default for "you are too
far from that Pylon."

## Click-to-move

`src/game/click_move.ts` (~105 lines, pure, tested) plus integration in `main.ts`. Pure
`clickMoveStep(player, target, stop)` returns `{ facing, forward, arrived }`;
`clickMoveShouldWalk(facing, bearing)` only walks within a +/-60 deg cone (turn in place
otherwise, to avoid orbiting at close range); `manualMovementOverrides()` cancels on any
WASD. The main-loop integration adds A* path following, stuck detection/reroute, and a
latency-aware stop-distance expansion for online play.

### Relevance to us

**Low priority / optional.** We are WASD-first and do not need click-to-move for core
navigation. If we later add accessibility movement or point-and-click traversal of a large
Verse, the pure module ports cleanly; the turning-cone trick and latency stop-distance are
the non-obvious bits to keep.

## Mobile / touch controls

`src/game/mobile_controls.ts` (~676 lines): floating left move-stick (8-way via
`mapJoystickVector` with deadzone, origin clamped to screen), fixed right camera-stick
(yaw/pitch rates), tap-to-action buttons with haptics, two-finger pinch zoom
(`pinchZoomDelta`), single-finger swipe-look with a 6 px deadzone, double-tap recenter, and
long-press-vs-tap (chat peek vs composer). Settings expose deadzone, look speed, invert.

### Relevance to us

**Low priority.** The mobile operator surface today is the Expo
`AutopilotRemoteControl` app, not a touch Verse client. If the Verse ever ships to touch,
the pure helpers (`mapJoystickVector`, `clampJoystickOrigin`, `pinchZoomDelta`) and the
long-press-vs-tap and double-tap-recenter patterns are the reusable parts. Until then,
note it and move on.

## Net for the adaptation plan

High priority: **keybind registry (folds into #5943)**, **camera follow + collision**,
**pointer-pick**. Medium: **interaction dispatch pattern** (when F-to-interact lands).
Low/optional: **click-to-move**, **mobile controls**. Everything here is already DOM-light
and Three-light, so the natural home is `three-effect` (controller/camera/input primitives)
with the desktop consuming them, consistent with our existing game-direction docs.
