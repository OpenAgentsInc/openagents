# Verse Custom Keybindings Audit

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-06-22

Scope: current keyboard, mouse, Verse movement, and app shortcut handling in
`openagents` and the linked `three-effect` runtime, with a recommendation for
MMORPG-style custom keybindings.

Status: complete for the initial keybindings sequence. The shared contract,
resolver, three-effect controller bindings, retained Verse runtime wiring,
Settings UI, and packaged custom-binding smoke coverage are implemented.

## Implementation Progress

- 2026-06-22: Issue #5944 added `@openagentsinc/input-bindings` with the
  shared action catalog, default `openagents.input-bindings.v1` profile,
  profile decoding/fallback, display-label helpers, conflict detection, and
  native-reserved binding checks. This is intentionally a pure contract package;
  desktop subscriptions, three-effect controllers, Settings UI, and packaged
  smoke coverage still need to consume it in the later issues.
- 2026-06-22: Issue #5945 added the Drei-inspired
  `createOpenAgentsKeyboardControls` primitive to `@openagentsinc/input-bindings`.
  It maps scoped keyboard events to named actions, tracks held state, supports
  subscriptions, handles repeat dedupe, can update binding maps, and clears held
  state on reset or binding swaps. The primitive is independent of React,
  Zustand, and scene identity so later Verse wiring can consume it without
  forcing Three scene remounts.
- 2026-06-22: Issue #5946 replaced the desktop shortcut forwarding decision
  with profile-driven action resolution. The subscription still emits the
  existing `PressedKey` compatibility message, but forwarding now resolves
  desktop actions from `@openagentsinc/input-bindings` instead of a static raw
  key whitelist. This fixes the real DOM path for Cmd/Ctrl-Shift-V and adds a
  test proving a custom action map can move the command palette shortcut without
  editing subscription code.
- 2026-06-22: Issue #5947 added configurable key maps to the shared
  `three-effect` Verse controllers in `three-effect` commit `4a7d9d3`.
  `createWasdMouseLookController` and `createThreePlayerController` now accept a
  `WasdKeyboardBindingMap`, preserve the current WASD/arrow/shift/space/C
  defaults, allow bindings to be removed or replaced, and drive target cycling
  through action bindings instead of hard-coded `Tab`. OpenAgents now pins both
  desktop and web `@openagentsinc/three-effect` consumers to that commit and the
  desktop type shim exposes the new binding-aware option shape.
- 2026-06-22: Issue #5948 wired the desktop input profile into the retained
  Verse scene. `three-effect` commit `71f89cc` removes movement and target
  binding maps from the retained structural identity, adds in-place controller
  binding updates, and clears held movement state only when a binding map
  actually changes. OpenAgents now projects the active `inputProfile` into
  third-person movement and target-cycling options, records `input.profile`
  diagnostics, and keeps pose/camera restoration stable across profile changes.
  The desktop Verse launch suite now proves an IJKL-style profile updates
  controller bindings without forcing a scene rebuild.
- 2026-06-22: Issue #5949 added the desktop Keybindings Settings editor.
  Desktop now loads and saves the active profile under the separate versioned
  `autopilot-desktop.input-bindings.v1` key, never inside the existing
  preferences blob. Settings renders dense Movement, Camera, Targeting,
  Interaction, HUD, App, Code, and Action Bar sections, supports keyboard-first
  primary/alternate capture, deterministic conflict warnings, and row/category
  /all restore controls. The input contract also now includes initial Code
  overlay actions so the Settings category is not placeholder-only. Unit
  coverage exercises render, capture, conflict display, persistence,
  corruption fallback, and restore behavior.
- 2026-06-22: Issue #5950 extended the packaged desktop Verse smoke harness to
  exercise custom bindings through the real Settings UI. The smoke opens
  Settings, captures `movement.forward` from `W` to `I`, verifies `W` no longer
  moves, verifies `I` moves before and after reload, checks mouselook drag and
  wheel zoom after the rebind, confirms focused composer/terminal text surfaces
  do not move the avatar, resets defaults, and verifies `W` moves again. The
  receipt now includes stage diagnostics, screenshot artifact paths, pose deltas,
  storage/UI probes, black-frame checks, and active-remount checks.

## Goal

The Verse should support a proper MMO-style keybinding system:

- users can rebind movement, camera, target cycling, interaction, HUD, code
  overlay, and future action-bar commands;
- defaults keep working exactly as today;
- text editing, terminal panes, command palettes, native edit accelerators, and
  pointer/camera control do not fight each other;
- changes do not remount the Three scene, reset player pose, flicker the
  character, or steal input from the canvas;
- tests cover real DOM forwarding, held-key state, custom movement mappings,
  editable focus, and packaged desktop smoke paths.

## Current State

There are two separate input systems today.

### Desktop UI Shortcut Layer

Implementation homes:

- `apps/autopilot-desktop/src/ui/subscriptions.ts`
- `apps/autopilot-desktop/src/ui/keyboard.ts`
- `apps/autopilot-desktop/src/ui/update.ts`
- `apps/autopilot-desktop/src/ui/nav.ts`
- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/ui/preferences.ts`
- `apps/autopilot-desktop/src/bun/application-menu.ts`

The desktop app has a pure shortcut interpreter in `keyboard.ts`. The
subscription layer listens for `keydown`, filters a small fixed set of keys, and
emits one `PressedKey` message. `update.ts` interprets that against model state
and redispatches real messages such as `OpenedCommandPalette`,
`ClosedCommandPalette`, `NavigatedTo`, `RanPaletteCommand`, `SubmittedShell`,
and `ToggleVerse`.

Current app-level shortcut behavior:

- `Cmd/Ctrl-K`: command palette.
- Palette open: `Escape`, `ArrowUp`, `ArrowDown`, `Enter`.
- `Cmd/Ctrl-Enter`: submit chat/composer turn.
- Bare `j` / `k`: move between panes in the current group.
- `Cmd/Ctrl-Shift-V`: intended Verse toggle in code mode.
- Shell input has local `Shift-Tab` and `Enter` handlers in `view.ts`.
- Native macOS/WebKit edit commands are handled through
  `src/bun/application-menu.ts`: copy, paste, cut, select all, undo, redo, quit,
  hide, minimize, and related system roles.

Current persistence:

- `preferences.ts` stores theme, default adapter/lane, notification-panel
  visibility, and gateway fallback under
  `autopilot-desktop.preferences.v2`.
- `apps/autopilot-desktop/src/ui/input-profile-preferences.ts` stores the active
  keybinding profile separately under `autopilot-desktop.input-bindings.v1`.
- `@openagentsinc/input-bindings` owns the action catalog, default profile,
  conflict model, native-reserved checks, formatting helpers, profile fallback,
  and named keyboard-control primitive.

Resolved subscription gap:

- `interpretKey` knows about `Cmd/Ctrl-Shift-V`, and tests call `PressedKey`
  directly, but `subscriptions.ts` currently forwards only a fixed
  `KEYBOARD_KEYS` set that does not include `v`. That means a direct reducer
  test can pass while the real DOM subscription path does not forward the key.
  Custom keybindings will make this class of bug more likely unless we replace
  the static "known keys" gate with profile-driven resolution.
- Issue #5946 replaced that static gate with profile-driven resolution and a
  real subscription test proving a changed key reaches the reducer path.

### Verse And Three.js Input Layer

Implementation homes:

- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/types/three-effect-core.d.ts`
- `/Users/christopherdavid/work/three-effect/packages/core/src/playerControllerPrimitives.ts`
- `/Users/christopherdavid/work/three-effect/packages/core/src/trainingRun.ts`
- `/Users/christopherdavid/work/three-effect/packages/foldkit/src/index.ts`

Desktop builds the Verse scene options in `view.ts`. The current live Verse
uses:

- `cameraMode: "perspective_walk"`
- `controller: "third_person_character"`
- `thirdPersonController.character.walkSpeed = 3.8`
- `thirdPersonController.character.runSpeed = 6.7`
- `jumpHeight = 4.9`
- `gravity = -13.5`

`three-effect` owns the real input listeners and controller state.

The library has reusable primitives:

- `WasdAction`: `forward`, `backward`, `left`, `right`, `rise`, `fall`,
  `sprint`.
- `defaultWasdKeyboardState`.
- `keyCodeToWasdAction(code)`.
- `setWasdKeyState(state, code, pressed)`.
- `createWasdMouseLookController`.
- `createThreePlayerController`.
- `keyboardTargeting` for Tab-based target cycling.

Default movement bindings:

- `KeyW` / `ArrowUp`: forward.
- `KeyS` / `ArrowDown`: backward.
- `KeyA` / `ArrowLeft`: left.
- `KeyD` / `ArrowRight`: right.
- `ShiftLeft` / `ShiftRight`: sprint.
- `Space`: rise or jump.
- `KeyC`: fall.
- Mouse drag: third-person orbit.
- Wheel: third-person zoom.
- Pointer lock plus mouse deltas: first-person mouselook mode.
- `Tab` / `Shift-Tab`: keyboard target cycling inside `trainingRun.ts`.

The shared `three-effect` API now accepts movement and target binding maps,
updates those bindings in place, and keeps the retained scene identity stable
when only the active input profile changes.

## Useful Reference: Drei KeyboardControls

`projects/repos/drei/src/web/KeyboardControls.tsx` is the best local reference.
It accepts a `map` of named actions to keys, builds a state store, tracks
keydown/keyup, supports `event.key` or `event.code`, and exposes both
subscription and get-state APIs.

The part worth copying conceptually is not React or Zustand. It is:

- named actions, not ad hoc keys;
- a map from actions to one or more keys;
- state that says whether each action is pressed;
- an optional change callback;
- a scoped event source.

The OpenAgents version should use our Effect/Foldkit style and work without
React, but it should have the same named-action model.

## Current Tests

Desktop coverage:

- `apps/autopilot-desktop/tests/nav-shell.test.ts` covers the pure shortcut
  interpreter and the static subscription forward decision.
- `apps/autopilot-desktop/tests/verse-toggle.test.ts` covers direct
  `PressedKey` behavior in explore/code modes.
- `apps/autopilot-desktop/tests/terminal-log-pane.test.ts` checks that editable
  terminal focus blocks app shortcuts.
- `apps/autopilot-desktop/scripts/verse-launch-smoke.ts` exercises packaged
  movement, mouselook drag, wheel zoom, nonblank frames, code overlay input, and
  custom keybinding capture/reload/reset through Settings.
- `apps/autopilot-desktop/tests/preferences.test.ts` covers keybinding Settings
  render, capture, conflict display, persistence, restore, and corruption
  fallback.
- `apps/autopilot-desktop/tests/verse-launch-checklist.test.ts` covers input
  profile projection and binding changes without retained scene rebuilds.
- `packages/input-bindings/src/index.test.ts` covers profile schema/defaults,
  partial/corrupt fallback, labels, conflict detection, native-reserved checks,
  action resolution, and the named keyboard-control primitive.

Three-effect coverage:

- `packages/core/src/index.test.ts` covers default and custom WASD binding maps,
  desired movement vector math, third-person movement, sprinting backward with
  `S`, target cycling, and controller option resolution.
- `trainingRun.ts` has retained update behavior for scene data and in-place
  binding changes, so profile updates do not remount the scene.

Remaining coverage opportunities:

- Mouse and wheel capture remain future work; mouse/wheel bindings can be shown
  and modeled, but Settings capture is keyboard-first today.
- Import/export JSON, multi-profile management, and action-bar macro capture are
  still product extensions beyond this first keybindings sequence.

### 2026-06-22 Hotbar Follow-up

The Verse hotbar is re-enabled as the first visible consumer of
`action_bar.slot_1` through `action_bar.slot_10`. It reads labels from the
shared input profile, shows the default number-key bindings in the world HUD,
and routes slot 1 (`Digit1` by default) to a fresh coder-session overlay using
the synced OpenAI icon catalog. The important architectural point is that this
is not a parallel shortcut table: the hotbar, Settings keybinding UI, DOM
keyboard subscription, and reducer path all resolve through
`@openagentsinc/input-bindings`.

## Product Requirements

The keybinding system should feel closer to an MMO than a small app shortcut
list.

Required categories:

- Movement: forward, backward, strafe left, strafe right, sprint, jump, walk
  modifier or autorun later.
- Camera: orbit drag, zoom, reset camera, toggle camera mode later.
- Targeting: target next, target previous, target nearest pylon, target nearest
  avatar, clear target.
- Interaction: interact/select, open selected info card, open bulletin, open
  pylon actions, tip selected pylon, tip selected forum post.
- HUD and mode: toggle code overlay, toggle diagnostics, toggle Tassadar HUD,
  toggle sats HUD later if needed.
- App commands: command palette, submit chat/composer, pane navigation, open
  settings, focus chat input.
- Future action bar: numbered slots or chorded actions for agent commands,
  pylon operations, forum actions, proofs, and approvals.

Users need:

- one primary binding and optional alternate bindings per action;
- keyboard and mouse bindings;
- held actions and press/toggle actions;
- conflict warnings;
- per-category reset to defaults;
- full profile reset;
- import/export JSON eventually;
- clear labels in Settings;
- profile changes that apply immediately.

## Recommended Architecture

### 1. Create A Shared Input Contract

Add a small shared contract before adding UI:

- preferred home: `three-effect` for low-level input primitives, with a desktop
  shim importing types into `openagents`;
- possible later package: `packages/input-bindings` if web, desktop, mobile, and
  MCP all need the same schema outside Three.js.

Core types:

```ts
type InputContext =
  | "global"
  | "text_entry"
  | "command_palette"
  | "verse_explore"
  | "verse_code_overlay"
  | "verse_pointer_locked"
  | "managed_pane"
  | "terminal"

type InputActionKind = "press" | "hold" | "toggle" | "axis"

type InputBinding =
  | { type: "keyboard_code"; code: string; modifiers?: Modifiers }
  | { type: "keyboard_key"; key: string; modifiers?: Modifiers }
  | { type: "mouse_button"; button: number; modifiers?: Modifiers }
  | { type: "wheel"; direction: "up" | "down"; modifiers?: Modifiers }

type InputActionSpec = {
  id: string
  title: string
  category: string
  kind: InputActionKind
  contexts: readonly InputContext[]
  defaultBindings: readonly InputBinding[]
  reserved?: boolean
  editable?: boolean
}

type InputProfile = {
  schemaVersion: "openagents.input-bindings.v1"
  profileId: string
  bindings: Record<string, readonly InputBinding[]>
}
```

Use `event.code` for movement defaults because it means physical WASD works
across keyboard layouts. Use `event.key` only where the semantic character is
the point, such as command palette letters or text-like chords.

### 2. Resolve Context Before Resolving Keys

Custom keybindings need a context stack, not one global map.

Recommended priority:

1. Native OS/AppKit reserved commands.
2. Text entry and terminal editing.
3. Command palette.
4. Active modal or rebind-capture UI.
5. Verse pointer-locked controls.
6. Verse explore controls.
7. Verse code overlay panes.
8. Global desktop app commands.

The resolver should return an action event, not a raw key event:

```ts
type ResolvedInputEvent =
  | { kind: "none" }
  | { kind: "blocked"; reason: string }
  | { kind: "pressed"; actionId: string }
  | { kind: "released"; actionId: string }
```

`PressedKey` can become a compatibility bridge, but new code should prefer
`InputActionChanged({ actionId, pressed, source })` or separate
`PressedInputAction` / `ReleasedInputAction` messages.

### 3. Replace Static Subscription Filtering

The current subscription layer decides what to forward by a fixed set of key
strings. That cannot support custom mappings.

Instead:

- subscription receives all `keydown` and `keyup` events;
- it asks the input resolver whether the event matches a binding in the active
  context;
- it forwards only resolved action events;
- it calls `preventDefault` based on the action spec and context;
- it never blocks native edit shortcuts in editable contexts;
- it dedupes repeated keydown for hold actions.

This also fixes the direct-test-vs-real-DOM risk visible in the Verse toggle
path.

### 4. Make Three-Effect Controllers Accept A Binding Map

`three-effect` should not hard-code `keyCodeToWasdAction` as the only path.
Keep it as the default profile, but make both controllers accept:

```ts
type WasdKeyboardBindingMap = Partial<
  Record<WasdAction, readonly InputBinding[]>
>
```

or a resolved lower-level form:

```ts
type WasdKeyboardBindingMap = Partial<Record<WasdAction, readonly string[]>>
```

The controller should resolve keydown/keyup through the binding map and mutate
the same `WasdKeyboardState`. Backward compatibility should be exact when no
map is supplied.

For the current third-person Verse controller, wire:

- movement bindings into `createThreePlayerController`;
- mouselook/orbit drag remains pointer-based first;
- wheel zoom remains pointer-based first;
- Tab target cycling should become a configurable `target.next` action;
- selected-object interaction should become a configurable `interact` action.

### 5. Keep Binding Profiles Out Of Scene Identity

The previous flicker/reset problems came from treating non-structural updates
as scene rebuild triggers. Keybinding changes must not change the scene
material identity.

Recommended pattern:

- the custom element accepts an input profile separately from the structural
  scene options, or `three-effect` retains controller handles and updates their
  binding maps in place;
- binding changes update the live input resolver and controller state;
- no renderer, camera, avatar, or scene graph remount happens;
- held state is cleared when a binding changes to avoid "stuck key" movement.

Acceptance criterion:

- changing `forward` from `KeyW` to `KeyI` causes `KeyW` to stop moving and
  `KeyI` to move without a `verse-host.remount`, black frame, pose reset, or
  animation stop.

### 6. Add A Settings UI Built Around Actions

Settings should stop being only a static shortcut list. It should have a real
"Keybindings" section with dense MMO-style groups.

Recommended UI:

- category list: Movement, Camera, Targeting, Interaction, HUD, App, Code.
- rows: action name, primary binding, alternate binding, context, conflict
  status, reset button.
- rebind flow: click a binding cell, capture next key/mouse input, show
  conflict, confirm or cancel.
- restore defaults per row/category/profile.
- search/filter by action name or key.

Persistence:

- local-only first, refs-only and secret-free, using a new storage key such as
  `autopilot-desktop.input-bindings.v1`.
- do not mix keybindings into `autopilot-desktop.preferences.v2`; keep them
  versioned separately because this shape will grow quickly.
- later user sync can be added only after conflict and device/layout semantics
  are stable.

### 7. Conflict Policy

Not every duplicate is bad. It depends on context.

Hard conflicts:

- same key + modifiers + same context + two editable actions;
- movement hold action colliding with another hold action in `verse_explore`;
- action-bar slot colliding with text-entry shortcuts in a context where text
  is active;
- rebinding native destructive app commands such as quit unless explicitly
  allowed later.

Allowed:

- same key in disjoint contexts;
- `Escape` for pointer unlock and modal dismissal if priority decides the
  owner;
- `Enter` for command palette run and shell submit in separate contexts;
- movement keys inside text fields, because text-entry context wins and the
  movement action is suppressed.

Reserved by default:

- `Cmd/Ctrl-Q`, `Cmd/Ctrl-H`, `Cmd/Ctrl-M`, native edit commands in editable
  contexts, browser/devtools-reserved chords, and the active rebind-capture
  cancel key.

### 8. Multiplayer And Authority

Keybindings are local presentation/control preferences. They should not change
network authority.

Rules:

- bindings do not affect SpacetimeDB validation;
- only the resulting bounded pose or semantic intent is published;
- no remote user can infer private profile details unless we deliberately expose
  them later;
- payment/tipping actions still route through existing approval/receipt policy;
- action-bar macros cannot become a bypass around payment, deployment, admin,
  or workspace-write authority.

## Implementation Sequence

### Phase 1: Contract And Defaults

1. Add an input action catalog with stable IDs and categories.
2. Add default keybinding profile matching current behavior.
3. Add schema/decoder for persisted local profile.
4. Add conflict detection and display-label helpers.
5. Add tests for defaults, decoding, corruption fallback, conflict detection,
   and native-reserved behavior.

No runtime behavior should change in this phase.

### Phase 2: Desktop Resolver

1. Replace `keyboardForwardDecision` with profile-driven resolution.
2. Add action messages for press/release.
3. Keep existing `interpretKey` as an adapter or migrate it to action IDs.
4. Fix the real DOM path for `Cmd/Ctrl-Shift-V`.
5. Preserve editable focus behavior and native edit shortcuts.
6. Add tests that exercise the subscription/resolver path, not only direct
   reducer messages.

### Phase 3: Three-Effect Controller Binding Maps

1. Add `keyBindings` or `inputBindings` to `WasdMouseLookControllerOptions` and
   `ThreePlayerControllerOptions`.
2. Keep hard-coded current defaults as `defaultWasdBindings`.
3. Resolve keydown/keyup through the binding map.
4. Add custom-map tests: for example, `KeyI` forward, `KeyK` backward, `KeyJ`
   left, `KeyL` right.
5. Add tests proving old WASD defaults are byte-for-byte behavior compatible.
6. Add tests proving sprint works with custom sprint bindings and with backward
   movement.

### Phase 4: Retained Runtime Wiring

1. Pass the resolved input profile from Desktop into the retained Verse runtime.
2. Update controller bindings in place, not through scene rebuild.
3. Clear held state after profile changes.
4. Add diagnostics for binding profile version, active context, and last
   resolved action.
5. Add a retained-update test proving profile changes do not remount, reset
   pose, or black-frame.

### Phase 5: Settings UI

1. Add the Keybindings settings section.
2. Add rebind capture, conflict display, cancel, restore row, restore category,
   restore all.
3. Persist profile locally under a separate versioned key.
4. Add tests for render, capture, conflict, persistence, restore, and corruption
   fallback.

### Phase 6: Browser/Desktop Smokes

1. Launch packaged desktop.
2. Open Keybindings.
3. Rebind forward from `W` to another key.
4. Return to Verse.
5. Verify old key no longer moves.
6. Verify new key moves.
7. Verify mouselook drag and wheel zoom still work.
8. Verify typing in chat/composer/terminal does not move the avatar.
9. Reload and verify the profile persists.
10. Reset defaults and verify WASD works again.

Completion note: Issues #5944 through #5950 implemented the six phases above.
The remaining items are follow-on product extensions, not blockers for the
initial MMO-style keyboard customization path.

## Recommended First Issues

1. Add shared input action catalog and default binding profile.
2. Replace static desktop keyboard forwarding with profile-driven action
   resolution.
3. Add configurable binding maps to three-effect third-person and WASD
   controllers.
4. Wire Desktop Verse profile into retained controller state without remounting.
5. Add Settings Keybindings UI with local persistence and conflict detection.
6. Add packaged smoke for rebind, movement, text editing, mouselook, and reset.

## Acceptance Criteria For "Ready"

- User can rebind movement keys and immediately move with the new bindings.
- Existing WASD/arrows/Shift/Space defaults still work.
- Rebinding does not remount the Verse scene or reset position.
- Editable fields never leak typed letters into movement.
- Native edit shortcuts keep working.
- Settings displays the real active bindings, not stale documentation.
- Conflict warnings are deterministic and tested.
- Tests cover real subscription forwarding, controller held state, persistence,
  and packaged desktop smoke behavior.

## Summary Recommendation

Build this as an input action system, not as a list of shortcut exceptions.

The durable shape is:

1. stable action IDs;
2. context-aware binding profiles;
3. one resolver for keydown/keyup/mouse events;
4. controllers that consume action state rather than hard-coded key codes;
5. Settings UI that edits the same profile the runtime uses;
6. retained scene updates so input changes do not touch renderer ownership.

That gets us MMO-style custom controls without reopening the class of bugs
where a projection refresh, overlay, or text element steals camera/movement
from the Verse.
