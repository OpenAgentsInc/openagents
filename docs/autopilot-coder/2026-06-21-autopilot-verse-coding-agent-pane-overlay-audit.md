# Autopilot Verse Coding Agent Pane Overlay Audit

**Date:** 2026-06-21

**Scope:** What remains to make coding agents run inside the Autopilot Desktop
Verse experience, with a code-mode overlay and pane system rather than leaving
the 3D world.

**Repos inspected:** `openagents`, `three-effect`, local `projects/repos/drei`,
local `projects/repos/react-three-fiber`

**Status:** Audit and implementation plan only. No product promise is closed by
this document.

## 0. Executive Summary

The missing work is no longer "make Autopilot able to run Codex or Claude-like
coding sessions." A large part of that substrate already exists in the current
desktop app and Pylon runtime:

- Pylon can spawn bounded coding sessions, list/cancel sessions, stream public
  event rows, surface decisions, resolve approvals, and retain transcript
  projections.
- Autopilot Desktop already has composer, sessions, swarm, decisions,
  autonomous-loop, session-detail, account picker, worktree picker, and shell
  target paths for Codex and the local Claude Agent adapter.
- The Verse scene is now the default first surface for chat, with live pylons,
  Bitcoin HUD, Tassadar projection, bulletin board, multiplayer/presence
  primitives, payment particles, and a retained Three scene that must not be
  remounted by ordinary UI updates.
- A pure pane manager already exists in Desktop: pane-as-data, cascade
  placement, focus/z-order, dragging, resizing, and a managed pane layer that
  can render existing Autopilot panes above the active surface.

The missing work is the product and runtime seam between those pieces:

1. **An explicit Verse coding mode.** Explore mode should stay clean and give
   pointer/keyboard ownership to the 3D world. Coding mode should add the pane
   overlay, command palette, hotbar/code entry points, agent stream panes,
   approvals, diffs, artifacts, and session controls without navigating away
   from Verse.
2. **A DOM overlay host for `three-effect`.** `three-effect` already has
   projection helpers inspired by Drei `Html`, but the Foldkit custom element
   does not yet expose a first-class DOM overlay/portal host around the canvas.
   That host should be generic, not Autopilot-specific.
3. **Input ownership and focus arbitration.** R3F/Drei solve this by separating
   the canvas event source, DOM portal target, pointer-events policy, and
   per-frame projection/occlusion. Autopilot needs the same boundary so
   mouselook, scroll/zoom, text selection, pane dragging, and editor input do
   not fight each other.
4. **A coding-agent pane vocabulary.** Existing panes can be reused, but the
   Verse overlay needs a tighter set: composer, live agent stream, sessions,
   decisions/approvals, diff/artifacts, terminal/log tail, and swarm. Those
   should use public-safe projections from the Pylon/Bun host, not raw secrets
   or uncontrolled process output.
5. **Regression tests that exercise scene + overlay + coding.** Current tests
   intentionally assert that first-render Verse hides the command palette and
   composer. New tests need to cover the separate coding-mode path and prove
   that pane updates/session streams do not remount or flicker the character
   and do not steal mouselook unless a DOM pane is focused.

The key recommendation: **add the generic overlay/portal primitive to
`three-effect`, but keep pane management and coding-agent semantics in
Autopilot.** `three-effect` should know how to host DOM over a Three canvas and
project world anchors to screen coordinates. It should not know what a Codex
session, approval, workspace, or Autopilot pane is.

## 1. Target Experience

The desired owner experience is:

- Open Autopilot Desktop and land in Verse.
- Walk, look, zoom, inspect pylons, approach bulletin boards, and read world
  information without UI clutter.
- Toggle into coding mode with a shortcut, palette command, or compact HUD
  affordance.
- Keep the Verse scene alive in the background while resizable panes appear
  over it.
- Start a coding task against a selected repo/worktree.
- Watch code-agent activity stream in a pane: plan, tool calls, files changed,
  checks, approvals, errors, and closeout.
- Open sessions, swarm, approvals, diffs, and artifacts as panes without losing
  the current camera/character position.
- Return to explore mode and have the DOM overlay release pointer and keyboard
  ownership back to the scene.

This is not a marketing overlay and not a separate dashboard route. It is a
mode over the same Verse runtime.

## 2. Current Autopilot Desktop Inventory

### 2.1 Verse Host

Relevant files:

- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/ui/model.ts`
- `apps/autopilot-desktop/src/ui/update.ts`
- `apps/autopilot-desktop/src/ui/subscriptions.ts`
- `apps/autopilot-desktop/src/ui/styles.css`

The default model now lands on `pane: "chat"` with `verseEnabled: true`. When
Verse is active, `chatPane` renders the full-screen Three scene through:

```ts
trainingRunView(
  [cls("three-effect-chat-scene")],
  verseSceneVisualization(model),
  onNodeSelected,
  onPresenceZone,
  onLocalPose,
  onWorldItemProximity,
)
```

The scene visualization combines live network/pylon projection, Tassadar
training status, the bulletin board, pylon base, payment particles,
multiplayer avatars, and a third-person character controller. The recent scene
graph work established a critical invariant: **local controller pose is runtime
state, not Foldkit render state.** Ordinary projection changes must not remount
the custom element or reset the character.

The Verse root already renders a DOM layer around the scene for the Pylon sats
HUD, selected-object inspector, bulletin overlay, and Tassadar HUD. The root
also includes `managedPaneLayer(model)`, so the pane manager is technically
available above Verse.

### 2.2 Current Pane System

Relevant files:

- `apps/autopilot-desktop/src/ui/pane-manager.ts`
- `apps/autopilot-desktop/src/ui/nav.ts`
- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/ui/keyboard.ts`
- `apps/autopilot-desktop/src/ui/update.ts`

Desktop already has the important pane primitives:

- `ManagedPaneKind = PaneId`
- `PaneLayerState = { panes, activeId, nextSerial }`
- `PaneLayerAction = open | close | focus | drag-start | drag-move | drag-end |
  close-all`
- deterministic cascade placement
- clamped geometry
- eight resize handles
- z-order/focus management
- a managed layer that renders `paneContent(model, pane.kind)` inside floating
  windows

Navigation already has the anti-clutter grouping:

- Chat
- Code
- Supervise
- Explore
- Settings

The Code group already points to `composer`, `swarm`, `sessions`, and `spawn`.
The palette already has commands like `pane.composer`, `pane.sessions`,
`pane.swarm`, and `pane.decisions` that can open those destinations as managed
panes.

The important limitation is that Verse currently disables this machinery in
normal launch mode. `verseHudDisabled(model)` returns true when Verse is active
and `VITE_CHAT_WORLD_HUD` is not enabled. In that state:

- `OpenedCommandPalette` is ignored.
- `PressedKey` is ignored by the Desktop reducer.
- `ToggleVerse` is ignored.
- The default Verse launch test asserts the command palette, hotbar, composer,
  sessions, swarm, and full UI chrome are absent.

That was the right hardening move for a clean game-world first render, but it
means coding mode must become a separate state rather than a global HUD flag.

### 2.3 Coding Agent Runtime

Relevant files:

- `apps/autopilot-desktop/src/bun/index.ts`
- `apps/autopilot-desktop/src/bun/pylon-control.ts`
- `apps/autopilot-desktop/src/bun/node-state-poll.ts`
- `apps/autopilot-desktop/src/bun/transcript-store.ts`
- `apps/autopilot-desktop/src/ui/update.ts`
- `apps/autopilot-desktop/src/ui/helpers.ts`
- `apps/autopilot-desktop/src/ui/blueprint-chat-runtime.ts`

The Bun host is the authority boundary for local desktop control. It owns the
Pylon bearer token and exposes public-safe commands/projections to the Foldkit
view. It can:

- spawn coding sessions with a workspace path or managed repo ref
- resolve managed worktree refs
- cancel sessions
- resolve approvals
- pause/resume the coordinator
- poll node state
- persist and rehydrate transcript projections
- map shell targets to `codex` and `claude_agent`

The UI already contains:

- composer turn submission and continuation
- account picker for per-session adapter/account selection
- worktree picker for existing path or managed GitHub repo/ref
- sessions list
- session detail timeline and artifacts
- swarm/multi-session projection
- decisions/approvals surface
- shell target tabs for current, local Claude Agent, and Codex
- autonomous loop controls

The runtime gap is not "can the app spawn a coding session?" It can. The gap is
"can the owner do that from inside Verse, while the scene remains stable and
the code stream feels native to the world?"

## 3. Previous Pane Systems Worth Carrying Forward

The launch docs already captured the lineage. The parts worth preserving for
Verse coding mode are:

| System | Reusable idea | Verse implication |
| --- | --- | --- |
| AutoDev HUD/pane store | Pane-as-data state, draggable/resizable coding surfaces, chat/diff panes | Keep panes serializable and reducer-driven. Do not encode pane state in Three scene options. |
| Commander | `Pane` records with `id`, `type`, `title`, `x`, `y`, `width`, `height`, z/focus, hotbar slots, Cmd-K | Current Desktop already recreated most of this. Verse mode should use it rather than inventing a new window manager. |
| v4 desktop predecessor | Dense owner/operator panes for coding work | Mine for component ergonomics, not runtime authority. Current Pylon/Desktop authority is newer. |
| WGPUI HUD crate | Frames, meters, status lights, dot-grid, hotbar, palette, screenshot capture | Use visual language sparingly. Coding mode should feel like a tool surface, not a second dashboard. |
| Current Foldkit nav shell | Grouped nav, palette, typed pane IDs, hidden full UI | Retain anti-clutter. Add a code-mode overlay instead of bringing the whole sidebar into explore mode. |

The recurring lesson is that panes should stay as data and actions, not as
implicit DOM state scattered through the view tree.

## 4. R3F / Drei `Html` Lessons

Inspected references:

- `projects/repos/drei/docs/misc/html.mdx`
- `projects/repos/drei/src/web/Html.tsx`
- `projects/repos/react-three-fiber/packages/fiber/src/web/Canvas.tsx`
- `projects/repos/react-three-fiber/packages/fiber/src/web/events.ts`
- `three-effect/packages/core/src/htmlOverlayPrimitives.ts`
- `three-effect/packages/foldkit/src/index.ts`

Drei `Html` is the closest pattern for what we want, but it is narrower than a
window manager. It provides:

- a DOM element mounted next to the Three canvas, not inside WebGL
- screen-space projection from a Three object/world point
- fullscreen and centered modes
- distance scaling
- z-index mapping
- optional occlusion
- `pointerEvents` control
- optional portal target
- a per-frame update loop that moves the DOM element without rebuilding the
  scene

R3F also has an important event lesson. The canvas can connect pointer events
to a configurable event source. When the event source is outside the canvas,
the canvas can set `pointer-events: none` so DOM overlays and scene events do
not trample each other. The event manager owns pointer capture, propagation,
and stop-propagation semantics.

`three-effect` already has the mathematical half:

- `projectWorldToScreen`
- `htmlDistanceScale`
- `htmlOverlayZIndex`
- `isWorldPointOccluded`
- `htmlOverlayStyle`

What it does not yet expose through Foldkit is the host half:

- a stable overlay root/slot/portal target associated with the custom element
- a way to mount fullscreen DOM above the canvas while the retained scene stays
  untouched
- a way to mount world-anchored DOM overlays from app descriptors without
  forcing a scene remount
- a typed event-ownership policy between canvas and overlay

That is the piece that belongs in `three-effect`.

## 5. Ownership Recommendation

### 5.1 Put In `three-effect`

Add generic DOM overlay host primitives:

- A Foldkit custom-element overlay slot or sibling overlay root.
- A core descriptor for screen-space/fullscreen DOM overlays and world-anchored
  DOM overlays.
- Projection style helpers wired into the retained render loop.
- Pointer-events policy primitives, for example `canvas`, `overlay`, and
  `passthrough`.
- Optional occlusion/distance/z-index calculation for world labels/cards.
- Tests proving overlay descriptor changes do not change the Three
  visualization structural signature and do not remount the renderer.

This should be positioned as a `Drei Html`-style bridge for Foldkit/Effect, not
as React emulation and not as Autopilot business logic.

### 5.2 Keep In Autopilot Desktop Or `autopilot-ui`

Keep these out of `three-effect`:

- pane reducer and pane actions
- command palette commands
- hotbar slots
- Codex/Claude/Pylon session state
- approvals
- account/worktree selection
- artifacts/diffs
- Autopilot visual copy and product modes

If shared reuse is needed across Desktop, Web, or Mobile, extract those to
`@openagentsinc/autopilot-ui` or a new Autopilot-specific package, not
`three-effect`.

### 5.3 Keep In Bun/Pylon

Keep authority in the host/runtime:

- bearer tokens
- provider credentials
- workspace path access
- managed worktree materialization
- raw subprocess output
- session spawn/cancel/approval commands
- transcript persistence
- redaction and public-safe projection

The Verse overlay should consume projected state and emit typed intents. It
should not hold secrets or run code.

## 6. Remaining Work

### 6.1 Product State: Verse Explore Mode vs Verse Code Mode

Add a real state field, not just `VITE_CHAT_WORLD_HUD`, for example:

```ts
verseMode: "explore" | "code"
```

Explore mode:

- no command palette by default
- no pane windows unless explicitly pinned by product policy
- pointer/keyboard go to the scene
- escape/click behavior remains local to scene interactions

Code mode:

- command palette available
- hotbar/code dock available
- managed panes available
- focusable text inputs and scrollable panes own input while focused
- scene remains visible and live behind panes
- returning to explore mode closes or hides transient coding panes according to
  policy, without resetting camera/character pose

This can replace the broad `verseHudDisabled` gate with a more precise input
and chrome policy:

```ts
verseChromePolicy(model) =
  explore -> sceneOnly
  code    -> overlayEnabled
```

### 6.2 Overlay Host

Implement a first app-local pass if needed, but the durable home should be
`three-effect`.

The primitive should support two families:

1. **Fullscreen overlay layer:** for panes, palette, hotbar, HUD, inspectors,
   and coding stream.
2. **World-anchored overlay layer:** for selected pylon info, bulletin details,
   tooltips, agent labels, and future in-world code terminals.

The fullscreen overlay must be outside the Three visualization options so pane
state changes cannot remount the scene. World-anchored overlays can use
projected positions but should still be DOM descriptors or app state, not
large scene structural changes.

### 6.3 Input Ownership

Define an explicit focus/input contract:

- Scene owns WASD, shift sprint, mouse look, wheel/zoom, and pointer lock when
  no overlay element is active.
- A focused pane owns typing, selection, copy/paste, scrolling, drag handles,
  and resize handles.
- Palette owns arrows/enter/escape while open.
- Textareas and inputs must set `inEditable` and must not leak Cmd-Enter except
  through intended submit actions.
- Pane drag/resize should capture pointer until pointerup.
- The canvas must not show text-selection cursors behind the character because
  an invisible DOM label is crossing the event layer.

Tests should model this directly rather than relying on manual screenshots.

### 6.4 Coding Pane Vocabulary

The first code-mode pane set should be:

- **Composer:** objective input, repo/worktree picker, account/adapter picker,
  submit/continue/cancel.
- **Agent Stream:** chronological stream of plan/tool/file/check/approval
  events for the active session, optimized for watching code come through.
- **Sessions:** current sessions grouped by running/blocked/done, with open
  detail/cancel actions.
- **Decisions:** pending approvals with compact approve/deny controls.
- **Diff/Artifacts:** files changed, parsed change set, receipts, external
  session refs, verification lines.
- **Swarm:** multi-session grid for parallel runs.
- **Terminal/Log Tail:** optional owner-only diagnostic pane for safe projected
  logs, not raw secrets.

The existing full-page panes can provide the first implementation. The audit
recommendation is to progressively split them into smaller overlay-friendly
widgets where density or focus demands it.

### 6.5 Live Agent Event Stream

Desktop currently has strong polling and transcript persistence. It also has a
Pylon control contract that supports per-session event retrieval. For a coding
overlay, the experience should move toward a live stream:

- keep the current poller as fallback and state repair
- add cursor-resumable per-session event streaming into the Bun host
- de-dupe events against the transcript store
- project stable event rows to the UI
- keep raw terminal/provider output behind the host boundary

This matters for "code coming through" because two-second polling is adequate
for status cards but feels sluggish for an agent stream pane.

### 6.6 Worktree And Repo Defaults

Composer already supports existing path and managed GitHub repo/ref. In Verse
coding mode, the default should be explicit and visible:

- selected workspace path or managed repo/ref
- base ref
- account/adapter
- danger/approval policy summary
- whether the session can write, run shell, create PRs, or only propose

Do not infer repo intent from vague world selection alone. A selected pylon can
suggest a default worker/node, but the coding target must remain explicit.

### 6.7 Claude/Codex Naming

The runtime may keep adapter names like `codex` and `claude_agent`. User-facing
copy should be careful:

- "Codex" is acceptable for the Codex lane.
- Prefer "Claude Agent" or "local Claude" over "Claude Code" unless the
  product/legal decision explicitly allows that label in the current surface.
- Do not imply OpenAgents is redistributing another vendor's app. It is routing
  to an adapter/account controlled by the owner.

### 6.8 Scene Stability

The recent Verse work created the right invariant:

- Local pose stays in the Three runtime.
- Foldkit model updates can observe/cache/publish pose but must not feed it
  back as structural visualization input.
- Projection updates can update material scene data but should not recreate the
  custom element.
- Pane state, palette state, session event tails, and typing must be outside
  the `TrainingRunVisualizationOptions` structural signature.

Coding mode must preserve this. A streaming agent pane that receives events
several times per second must not cause character flicker, black frames,
pointer reset, camera reset, or animation pause.

## 7. Proposed Architecture

```text
Autopilot Desktop window
  app-shell-verse
    three-effect Foldkit custom element
      retained Three scene
      character controller runtime state
      scene graph / world items / pylon visuals
    three-effect DOM overlay host
      world-anchored overlays
        selected pylon card
        bulletin detail anchor
        optional labels/tooltips
      fullscreen overlay
        explore HUD
        code-mode pane layer
        command palette
        hotbar / code dock
        toasts / diagnostics
  Bun host boundary
    Pylon control token
    session spawn/cancel/approval commands
    transcript store
    redaction/public-safe projections
  Pylon node
    Codex adapter
    Claude Agent adapter
    worktree/session executor
    approvals
    wallet/session state
```

Rules:

- Scene data flows down as stable visualization options.
- Scene pose/proximity flows up as events, not as remount-triggering model
  structure.
- Overlay pane state flows through the Autopilot pane reducer.
- Runtime authority flows only through the Bun host.
- Raw credentials and raw subprocess streams never enter the Foldkit view.

## 8. Tests Needed

### 8.1 Pure Unit Tests

Add or extend tests for:

- `verseChromePolicy` or equivalent mode derivation.
- Code mode permits palette/panes while explore mode keeps them hidden.
- Pane reducer open/focus/drag/resize/close behavior is stable and clamped.
- Keyboard interpreter respects focused editable elements.
- Palette shortcuts work in code mode and are ignored in explore mode unless
  intentionally allowed.
- Session event rows become agent-stream rows without leaking raw secrets.

### 8.2 Foldkit View Tests

Add serialized view tests for:

- first-render Verse explore mode still has no full code chrome
- toggling code mode adds a code dock/pane affordance
- opening composer as a pane keeps `app-shell-verse` and the scene element in
  the tree
- opening sessions/decisions/diff panes does not remove the Verse scene
- selected pylon/bulletin overlays can coexist with managed panes

### 8.3 Browser/Desktop Smoke

Add a reusable smoke that launches the app and:

1. Waits for the Verse scene to render nonblank.
2. Samples scene logs and fails on remount/flicker/reset while idle.
3. Toggles code mode.
4. Opens Composer as a pane.
5. Types an objective into the pane.
6. Starts a local stub or loopback coding session.
7. Waits for stream rows showing plan/tool/file/check events.
8. Drags/resizes a pane.
9. Moves the character and rotates/zooms the camera while the pane remains open
   but unfocused.
10. Focuses the pane textarea and proves WASD/mouselook no longer leaks to the
    scene while typing.
11. Blurs/closes the pane and proves scene control returns.
12. Asserts no black frames, no character flicker, no `oa-training-run` remount
    loop, and no reset to the starting pose.

This smoke should reuse the existing Verse scene logging conventions:

- `[verse-scene]`
- `__OA_VERSE_SCENE_LOGS`
- `__OA_DUMP_VERSE_SCENE_LOGS()`

It should fail loudly on the category of bug that recently caused repeated
black frames, character flicker, and mouselook loss.

### 8.4 Runtime Proofs

Keep existing proofs and extend them into the overlay path:

- `proof:composer`
- `proof:account-picker`
- `proof:swarm`
- `proof:transcript`
- `proof:shell-control`
- `smoke:verse-launch`
- `test:verse-launch`

New proof name suggestion:

```text
proof:verse-coding-overlay
```

The proof should exercise the real Desktop update/view path, not only helper
functions.

## 9. Sequenced Implementation Plan

### Phase 1: Product State And Discoverability

- Add `verseMode: "explore" | "code"` or equivalent.
- Add a compact code-mode affordance that is visible enough to discover but
  does not turn explore mode back into a dashboard.
- Replace broad `verseHudDisabled` checks with a policy that allows coding
  chrome only in code mode.
- Add tests that preserve clean first-render explore mode.

### Phase 2: Overlay Host

- Add an app-local overlay host if needed for speed.
- Then add the generic DOM overlay/portal primitive to `three-effect`.
- Ensure overlay changes do not affect the Three visualization structural key.
- Add `three-effect` tests around overlay style/projection/remount behavior.

### Phase 3: Managed Panes In Verse Code Mode

- Reuse `PaneLayer` over Verse in code mode.
- Open Composer, Sessions, Decisions, Swarm, and Session Detail as panes.
- Add hotbar/palette commands scoped to code mode.
- Ensure pane close/hide behavior does not navigate away from Verse.

### Phase 4: Agent Stream Pane

- Add a dedicated live agent stream projection for the selected coding session.
- Parse existing event phases into compact row types:
  `plan`, `tool`, `file`, `check`, `approval`, `error`, `done`.
- Link rows to session detail/diff/artifacts.
- Keep raw event details behind public-safe parsing/redaction.

### Phase 5: Input Arbitration

- Implement explicit canvas/overlay focus ownership.
- Add pointer-capture behavior for pane drag/resize.
- Ensure world labels and hidden text cannot intercept mouselook.
- Ensure focused code panes can select text and type normally.

### Phase 6: Integration Smoke

- Add the reusable Verse coding overlay smoke described in section 8.3.
- Run it against dev and packaged builds before claiming the feature ready.
- Make the smoke capture a screenshot and scene log artifact for debugging.

### Phase 7: Streaming And Remote Expansion

- Add low-latency per-session event streaming behind the Bun host.
- Keep polling as repair/fallback.
- Later unify local, SHC, and Google placement into the same overlay session
  model, but do not block local Verse coding mode on cloud placement.

## 10. Open Questions

- Should code mode be a persistent preference, or should every app launch start
  in explore mode?
- Should closing the last code pane automatically return to explore mode?
- Should a selected pylon imply a default execution node/account, or only
  provide context in the composer?
- Should the shell target UI survive as a separate pane, or should Codex/Claude
  selection be folded into the composer/account picker?
- Should world-anchored code panes exist, or should all coding surfaces remain
  screen-space DOM panes for readability?
- Should the first overlay smoke use a stub control server, a local Pylon node,
  or both?

## 11. Sequential Codex-First Issue Ladder

These are doc-local issue keys, ordered so the UI becomes more usable at every
step while preserving the Verse scene invariant. They are ready to become live
GitHub issues when implementation starts.

The ladder starts with Codex because the source already has the strongest local
account and execution substrate:

- `apps/autopilot-desktop/src/bun/account-management.ts` edits the node-local
  `dev.accounts` config for `codex` and `claude_agent` entries.
- `apps/autopilot-desktop/tests/account-management.test.ts` proves add/list,
  duplicate rejection, priority ordering, removal, `SelectedComposerAccount`,
  and `ClickedComposerSpawn` threading the selected Codex `accountRef`.
- Bun exposes `listManagedAccounts`, while the node-state account projection
  carries `accountRef`, `accountRefHash`, provider, selector, readiness, and
  priority-safe display material.
- Pylon's Codex composer and control-session path accept `accountRef`, resolve
  the account registry, and project only `accountRefHash` into public session
  evidence.
- `packages/autopilot-control-protocol/src/spawn-request-validate.test.ts`
  already accepts trimmed `accountRef` for session spawn requests.

### VCODE-01 - Add Verse Code Mode State And First-Render Tests

Status: implemented in #5918 on 2026-06-21. The Desktop model now has explicit
`verseMode: "explore" | "code"` state. Explore remains the clean first-render
Verse surface; code mode opts into the existing command palette and managed
pane layer without changing the retained Three visualization or local restore
pose.

Build:

- Add an explicit `verseMode: "explore" | "code"` state, or equivalent typed
  model field.
- Keep launch/default Verse in explore mode with no command palette, panes,
  hotbar, or text overlays intercepting scene input.
- Permit code-mode palette and pane commands only after an intentional toggle.

Acceptance:

- Existing first-render Verse tests stay green.
- A new reducer/view test proves toggling code mode does not remount the
  Three custom element and does not reset the local character pose.
- Pointer, wheel, and keyboard scene controls remain owned by the canvas in
  explore mode.

### VCODE-02 - Surface Codex Account Inventory In Code Mode

Status: implemented in #5919 on 2026-06-21. Verse code mode now refreshes the
existing managed-account projection and renders a compact Codex account
inventory overlay. Rows combine managed `dev.accounts` refs with live node
readiness, selector, priority, selection state, and short `accountRefHash`
tails; explore mode stays clean and full hashes/blocker details remain out of
the default UI.

Build:

- Reuse the existing managed-account projection rather than adding a new
  account protocol.
- In code mode, show multiple Codex accounts with short labels, readiness,
  priority, selector type, and redacted `accountRefHash`.
- Hide local homes, raw provider payloads, emails, tokens, and full hashes from
  the main UI.

Acceptance:

- With two `dev.accounts` Codex entries, the Verse account UI displays both.
- The selected account is visible before a spawn.
- Full account refs and hashes appear only in an explicit detail/diagnostic
  surface.

### VCODE-03 - Add Codex Account Management Pane

Status: implemented in #5920 on 2026-06-21. Autopilot Desktop now has a
dedicated `accounts` pane that reuses the existing `dev.accounts`
add/remove/priority surface inside the managed pane layer. Verse code mode adds
a compact Manage action from the Codex inventory, opens the Accounts pane
without leaving Verse, refreshes managed accounts and gateway readiness on pane
open, keeps Codex rows first while leaving Claude Agent visible, and adds UI
blockers for invalid refs, missing refs/homes, duplicate refs, and missing
managed homes.

Build:

- Move the existing add/remove/priority UI into a Verse code-mode pane or dock.
- Support multiple Codex homes first; keep Claude Agent entries visible but
  secondary until the Codex path is green.
- Add readiness refresh and mutation status without navigating away from Verse.

Acceptance:

- The current `dev.accounts` CRUD tests gain a Verse-pane wrapper test.
- Adding, removing, and reprioritizing Codex accounts updates the code-mode
  account inventory without restarting the app.
- Invalid refs, missing homes, and duplicate refs produce typed UI blockers.

### VCODE-04 - Wire The Codex Account Picker Into The Verse Composer

Status: implemented in #5921 on 2026-06-21. Entering Verse code mode now
defaults the Composer runtime to Codex. The Composer spawn form renders an
inline run context with runtime, selected account, repo/worktree target, and
verify-command count. `ClickedComposerSpawn` continues to use the existing
`session.spawn` command contract, but now blocks clearly when a selected Codex
account is absent or blocked in the live node account projection, and adapter
changes clear stale selected account refs so Apple FM / non-Codex paths do not
receive Codex account refs.

Build:

- Make the code-mode composer default to adapter `codex`.
- Require the current Codex account selection to be shown inline with the
  objective, repo/worktree target, and verification plan.
- Thread the selected `accountRef` through `ClickedComposerSpawn` and
  `session.spawn` using the existing command contract.

Acceptance:

- A two-account test proves spawning under account A and account B sends the
  correct `accountRef`.
- If the selected account becomes unavailable, the composer blocks clearly
  instead of silently falling back to the default home.
- Apple FM and future non-Codex lanes do not receive Codex `accountRef`.

### VCODE-05 - Add A Scoped Code-Mode Command Registry

Status: implemented in #5922 on 2026-06-21. The command registry now carries
typed command scopes, generated keybinding metadata, and a dedicated Verse
code-mode command set for panes, accounts, sessions, approvals, diffs, and
diagnostics. Verse code mode uses that scoped set instead of global navigation
commands, explore mode still has no code commands, and focused editable fields
now suspend command shortcuts so Composer/editor/terminal typing cannot trigger
palette, submit, pane movement, or Verse-toggle commands.

Build:

- Split code-mode commands from global navigation commands.
- Add command scopes for sessions, panes, accounts, approvals, diffs, and
  diagnostics.
- Implement editable-target guards and temporary command suspension while a
  pane, modal, picker, or terminal owns focus.

Acceptance:

- Typing in composer/editor/terminal fields never triggers WASD, mouselook, or
  command shortcuts.
- Code-mode commands are unavailable in explore mode.
- Command labels and keybinds are generated from typed registry data.

### VCODE-06 - Add The Codex-First Code Dock Stack

Status: implemented in #5923 on 2026-06-21. Verse code mode now renders a
compact Codex dock as screen-space DOM over the retained scene. The dock offers
the first-turn composer, active-session summary, follow-up prompt, compact
permission approval/deny controls, and durable-inspector shortcuts into
Composer, Sessions, Decisions, and Diff/Artifacts panes. The dock is hidden in
explore mode, does not render as a managed pane by itself, keeps full inspection
surfaces in the pane layer, and its CSS makes dock chrome pointer-pass-through
while only focused/interactive controls opt into pointer events. Tests cover
mode gating, active-session/permission rendering, pass-through pointer policy,
and hiding code mode without changing open panes, active session refs, or Verse
restore pose.

Build:

- Add a compact code dock over Verse for composer, permission prompt,
  follow-up prompt, and active-session controls.
- Keep durable inspection surfaces as panes, not dock-only widgets.
- Ensure the dock is screen-space DOM hosted by the overlay layer, not a
  Three scene child.

Acceptance:

- The dock appears only in code mode.
- Permission/follow-up prompts do not steal scene input unless focused.
- Closing or hiding the dock preserves active sessions and character pose.

### VCODE-07 - Add The Codex Agent Stream Projection

Build:

- Project existing Pylon/Codex events into stable row types:
  `objective`, `plan`, `tool`, `file`, `check`, `approval`, `error`, and
  `done`.
- Key rows by stable session/event refs so streaming updates reuse unchanged
  DOM nodes.
- Carry `sessionRef`, adapter, short account label, and redacted
  `accountRefHash`.

Acceptance:

- Real or fixture Codex event samples render as compact stream rows.
- Replaying the same event batch does not churn row identity.
- Raw prompts, local paths, secrets, and provider payloads are redacted from
  the projection by default.

### VCODE-08 - Add Session List And Detail Panes Synced To Codex Accounts

Build:

- Make Sessions filterable by account, adapter, workspace, and status.
- Open selected session details in a pane without leaving Verse.
- Preserve selected session and scroll state across stream/poll updates.

Acceptance:

- Starting a Codex session under a selected account makes it appear in the
  filtered session list.
- Session detail links back to Agent Stream, Decisions, Diff/Artifacts, and
  Terminal/Log panes.
- Account filters use short labels by default and detail panels for hashes.

### VCODE-09 - Add Decisions And Approval UI

Build:

- Build a code-mode Decisions pane/dock surface for pending approvals.
- Use explicit actions: reject, allow once, and scoped always.
- Display scope before any persistent approval is enabled: session,
  workspace, command class, account, and expiration where available.

Acceptance:

- Approval actions call the existing decision/approval control surface.
- A persistent approval cannot be created without visible scope.
- Public assignment paths and market/provider lanes remain blocked from
  local supervised danger modes.

### VCODE-10 - Add Diff And Artifact Panes

Build:

- Add a Diff/Artifacts pane for changed files, patch summaries, check refs,
  receipt refs, screenshots, and retained proof links.
- Use public-safe projections for paths and output digests.
- Add scroll restoration and selected-file preservation.

Acceptance:

- A Codex session with changed files opens the correct diff/artifact rows.
- Raw local absolute paths and full private logs are not shown unless the
  surface is explicitly local-only.
- Re-rendering stream updates does not reset scroll position.

### VCODE-11 - Add Terminal And Log Pane Hardening

Build:

- Add a projected Terminal/Log pane for controlled process output and session
  logs.
- Keep raw secrets, raw env, wallet material, and provider payloads out of the
  default log projection.
- Make focus ownership explicit for terminal text selection, copy, scroll, and
  key handling.

Acceptance:

- Terminal focus blocks scene controls; blur/close returns control to Verse.
- Hidden terminal panes are inert and cannot intercept mouselook.
- Logs show digest refs or redacted excerpts where raw output is unsafe.

### VCODE-12 - Add The Codex Account And Session Sync Loop

Build:

- Sync managed accounts, node state, session list, event stream, transcript
  store, and quota/readiness projections into one typed code-mode model.
- Use streaming where available and polling as repair/fallback.
- De-dupe account/session updates by stable refs.

Acceptance:

- Adding/removing a Codex account updates account picker, sessions filter, and
  diagnostics without app reload.
- A running Codex session updates Agent Stream, Sessions, Decisions, and
  Diff/Artifacts in one model tick.
- Sync failures appear as repairable diagnostics, not stale "ready" UI.

### VCODE-13 - Add Multi-Codex Account Routing Defaults

Build:

- Define routing precedence: explicit selected account, last-used account for
  the workspace, priority ordering, then default home only when allowed.
- Show the chosen route before spawn.
- Add an override command for "run this same task with another Codex account".

Acceptance:

- No silent fallback occurs when an explicit account is blocked.
- Last-used and priority routes are deterministic and test-covered.
- Route decisions store only redacted account refs/hashes in session evidence.

### VCODE-14 - Add Code-Mode Host Readiness And Diagnostics

Build:

- Add a diagnostics pane for Pylon node readiness, Bun bridge readiness, Codex
  account readiness, stream-vs-poll status, transcript persistence, and scene
  remount/flicker counters.
- Make exports public-safe by default.

Acceptance:

- A user can see why Codex is unavailable for a given account.
- Diagnostics include enough scene/input counters to debug black frames,
  character remounts, stuck streams, and lost mouselook.
- Exported diagnostics omit secrets, full account refs, raw local paths, and
  provider payloads.

### VCODE-15 - Add A Reusable Verse Coding Integration Smoke

Build:

- Launch Verse, verify first render, toggle code mode, pick a Codex account,
  open composer, spawn a stub or controlled Codex session, stream rows, open
  approvals/diff/terminal panes, move/rotate/zoom, and capture screenshots.
- Assert no scene remount, no black frame, no character flicker, and no input
  theft outside focused DOM panes.

Acceptance:

- The smoke runs against the dev app and packaged Desktop app.
- It produces logs and screenshots useful for debugging failures.
- It becomes the gate before claiming Verse coding mode is ready.

### VCODE-16 - Extend To Claude Agent And Fable After Codex Is Green

Build:

- Reuse the same account, session, stream, approval, diff, and diagnostics
  shapes for Claude Agent/Fable.
- Keep Codex as the reference implementation until the whole code-mode loop is
  smooth.

Acceptance:

- Claude Agent can use the shared UI contract without weakening Codex tests.
- Fable review is visibly a review/planning lane unless configured as an
  implementation adapter.
- Non-Codex provider claims stay scoped to implemented, tested behavior.

## 12. Bottom Line

Autopilot already has most of the coding-agent runtime and most of the pane
system. Verse already has the 3D runtime. The next meaningful product step is
to bind them with a deliberate overlay architecture:

- **`three-effect` gets the generic DOM overlay/Html-like host.**
- **Autopilot gets Verse code mode and coding-agent panes.**
- **Bun/Pylon keep execution authority and public-safe projections.**
- **Tests prove the scene stays alive while code streams through the overlay.**

That path gives the owner the experience they are asking for: walk around in
Verse, switch into coding mode, watch agents work in panes, and switch back
without the world blinking, resetting, or losing input.

## 13. OpenCode Desktop Companion Audit Addendum

The companion audit
`2026-06-21-opencode-desktop-harvest-for-verse-coding-overlay-audit.md`
refines this plan with a concrete reference from `projects/repos/opencode`.

The main update is that Verse code mode should be a **dock-and-pane**
workspace:

- A code dock stack should own the active composer, permission prompt,
  question prompt, follow-up suggestions, and later revert/todo prompts.
- Floating panes should own durable inspection surfaces: Agent Stream,
  Sessions, Decisions, Diff/Artifacts, Terminal/Log, and Swarm.
- The Agent Stream pane should be backed by a stable tagged row projection,
  similar to OpenCode's timeline row algebra, so streaming updates do not
  churn the DOM over the Three scene.
- Code-mode commands should evolve from only static nav commands toward a
  scoped command registry with editable-target filtering and temporary
  suspension while a modal, picker, or focused pane owns input.
- Approval UI should use an explicit reject/once/scoped-always triad, with
  the scope visible before any auto-approval is enabled.
- Host readiness and diagnostics should become part of code mode: Pylon/Bun
  readiness, stream-vs-poll state, transcript persistence state, scene logs,
  and public-safe diagnostic export.

This does not change the ownership recommendation above. `three-effect` should
still get only the generic DOM overlay/Html-like host. Autopilot should own
the dock stack, pane manager, command registry, coding-agent projections, and
Pylon/Bun authority boundary.
