# OpenCode Desktop Harvest For Autopilot Verse Coding Overlay

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


**Date:** 2026-06-21

**Scope:** Follow-on audit to
`2026-06-21-autopilot-verse-coding-agent-pane-overlay-audit.md`, focused on
what OpenAgents can harvest from the local OpenCode desktop reference app for
Autopilot Desktop Verse coding mode.

**Reference inspected:** read-only local reference repo
`projects/repos/opencode`, especially:

- `packages/desktop`
- `packages/app`
- `packages/ui`

**Status:** Audit and implementation guidance only. No OpenCode source is
vendored or copied by this document.

## 0. Executive Summary

The previous Verse coding overlay audit established the target:

- keep Verse as the first surface
- add an explicit code mode over the 3D world
- use a DOM overlay host from `three-effect`
- keep Autopilot pane/coding state out of `three-effect`
- prove the scene does not flicker, remount, or lose input while code streams

OpenCode gives us a useful reference for the coding half of that plan. The
desktop app itself is an Electron host, which Autopilot should not copy
directly. The harvest is in its boundaries and product primitives:

1. **Typed desktop authority boundary.** OpenCode keeps native authority in the
   main process and exposes a typed `window.api` through preload. Autopilot has
   the equivalent Bun/Pylon boundary already; the harvest is the rigor:
   narrow methods, subscriptions with cleanup, file-picker authorization
   tokens, debug-log export, and native window/zoom/deep-link control kept out
   of the renderer.
2. **Sidecar readiness as UI state.** OpenCode starts a local sidecar server,
   waits for explicit readiness, then renders the app with a known local
   server connection. Autopilot should make local Pylon/host readiness a
   first-class Verse code-mode resource instead of smearing "unknown" or
   "offline" through unrelated panes.
3. **Scoped command registry.** OpenCode has a dynamic command registry with
   keybind parsing, palette metadata, editable-target filtering, a suspension
   counter, duplicate guards, and persisted command catalog metadata.
   Autopilot's current palette is static and nav-driven. Verse code mode would
   benefit from a scoped command registry, especially for session-specific
   commands and pane-local commands.
4. **Dock stack plus panes.** OpenCode's session surface is not only windows.
   It has a bottom composer region plus permission, question, follow-up,
   revert, and todo docks. That is a better model for Verse code mode than
   "floating panes only": use a compact dock stack for active prompts and
   approvals, and use panes for sessions, stream, diffs, terminal, and swarm.
5. **Timeline row algebra.** OpenCode projects session messages into stable
   tagged timeline rows, reuses rows by key/equality, and separates projection
   from rendering. This maps directly to the proposed Autopilot Agent Stream
   pane and helps avoid high-frequency stream updates causing excessive DOM
   churn over the 3D scene.
6. **Review, terminal, and scroll/focus restoration.** OpenCode has patterns
   for resizable terminal panels, file/review panels, file tabs, scroll state,
   delayed focus repair, and scroll restoration that stops when the user
   interacts. Those patterns are highly relevant to code panes layered above
   Verse.

The strongest product refinement from OpenCode is this: **Verse coding mode
should be a dock-and-pane workspace, not just a pile of floating windows.**
The dock carries the live prompt/permission/question/follow-up loop; panes
carry durable inspection surfaces.

## 1. What Was Inspected

### 1.1 Desktop Host

OpenCode desktop files inspected:

- `packages/desktop/src/main/index.ts`
- `packages/desktop/src/main/ipc.ts`
- `packages/desktop/src/main/server.ts`
- `packages/desktop/src/main/sidecar.ts`
- `packages/desktop/src/main/windows.ts`
- `packages/desktop/src/preload/index.ts`
- `packages/desktop/src/preload/types.ts`
- `packages/desktop/src/renderer/index.tsx`

Key observations:

- The renderer process should only call `window.api` from preload.
- Native and authority operations stay behind IPC:
  - sidecar lifecycle
  - initialization readiness
  - deep links
  - default server URL
  - persistent store
  - directory/file/save pickers
  - tokenized file reads after picker authorization
  - clipboard image reads
  - notifications
  - window focus/show
  - relaunch
  - zoom and pinch-zoom settings
  - titlebar theme
  - updater
  - debug-log export
  - fatal renderer error recording
- The desktop app creates a sandboxed BrowserWindow with:
  - `contextIsolation: true`
  - `nodeIntegration: false`
  - `sandbox: true`
  - a custom `oc://renderer` protocol
  - path traversal checks for packaged renderer assets
  - custom document policy headers
- The sidecar server is started in a utility process, sends an explicit
  `ready` message, is health-checked on loopback, and is stopped on quit or
  relaunch.
- Proxy bypass and system certificates are explicit startup concerns.
- Deep links are queued until the renderer exists.
- Window unresponsive recovery can export logs, relaunch, or quit.

### 1.2 Shared App Session UI

OpenCode app files inspected:

- `packages/app/src/context/command.tsx`
- `packages/app/src/pages/session/use-session-commands.tsx`
- `packages/app/src/components/prompt-input.tsx`
- `packages/app/src/pages/session/composer/session-composer-region.tsx`
- `packages/app/src/pages/session/composer/session-permission-dock.tsx`
- `packages/app/src/pages/session/composer/session-question-dock.tsx`
- `packages/app/src/pages/session/composer/session-followup-dock.tsx`
- `packages/app/src/context/permission.tsx`
- `packages/app/src/context/permission-auto-respond.ts`
- `packages/app/src/pages/session/timeline/projection.ts`
- `packages/app/src/pages/session/timeline/rows.ts`
- `packages/app/src/pages/session/timeline/model.ts`
- `packages/app/src/pages/session/session-side-panel.tsx`
- `packages/app/src/pages/session/terminal-panel.tsx`
- `packages/app/src/pages/session/review-tab.tsx`
- `packages/app/src/pages/session/file-tabs.tsx`
- `packages/app/src/components/debug-bar.tsx`

Key observations:

- The command system supports:
  - dynamic registration
  - keyed replacement
  - persisted command catalog metadata
  - suggested commands
  - categories
  - palette shortcut
  - custom keybinds
  - editable-target filtering
  - a small allowlist of shortcuts that can work while typing
  - suspension count for temporary command disable
- The prompt input supports:
  - dock and new-session variants
  - prompt history split between normal and shell
  - project/worktree selection
  - agent/model selection
  - attachments and image paste
  - slash popover
  - context items
  - queue/abort/submit hooks
  - cursor-aware scroll repair
- The session composer region stacks:
  - question dock
  - permission dock
  - follow-up dock
  - revert dock
  - todo dock
  - prompt input
- The permission system supports:
  - deny
  - allow once
  - allow always
  - auto-accept scoped to session lineage and directory
  - duplicate-response suppression with TTL
- The timeline projection turns raw message/part state into stable rows:
  - `TurnGap`
  - `CommentStrip`
  - `UserMessage`
  - `TurnDivider`
  - `AssistantPart`
  - `Thinking`
  - `DiffSummary`
  - `Error`
  - `Retry`
- The timeline reuses row objects when keys/equality match, which matters for
  high-frequency streams.
- The terminal panel clamps height to the viewport, supports resize/collapse,
  auto-creates a terminal when opened, re-focuses with frame/timer retries, and
  blurs when closed.
- Review/file tabs persist scroll state, restore after render, and stop
  fighting the user after direct interaction.
- DebugBar tracks FPS, frame gaps, jank, long tasks, navigation duration,
  memory, CLS, INP, and event delay.

## 2. Harvest Matrix

| OpenCode Reference | Useful Pattern | Autopilot Verse Use |
| --- | --- | --- |
| `desktop/src/preload/*`, `desktop/src/main/ipc.ts` | typed renderer API with subscriptions and cleanup | Keep Verse code mode renderer pure. Add narrow host APIs for code-overlay diagnostics, file picks, log export, deep links, and readiness. |
| `desktop/src/main/server.ts`, `sidecar.ts` | sidecar ready message plus health check before app use | Make Pylon/Bun readiness explicit in the code overlay. Do not let "unknown" leak into unrelated UI. |
| `desktop/src/main/windows.ts` | sandboxed window, custom app protocol, path traversal guard, unresponsive recovery | Autopilot should keep its Electrobun surface similarly narrow and add code-overlay recovery/export hooks. |
| `renderer/index.tsx` | platform provider maps native APIs to app capabilities | Add a `VerseCodingPlatform` or equivalent adapter for host capabilities consumed by panes. |
| `context/command.tsx` | dynamic scoped command registry with keybind parsing and editable target guard | Replace or augment static `nav.ts` palette commands for code mode with scoped session/pane commands. |
| `use-session-commands.tsx` | category-specific session commands | Code mode palette categories should include Session, Pane, Approval, Diff, Terminal, Agent, and View. |
| `prompt-input.tsx` | prompt editor with history, attachments, slash/context, agent/model/project selectors | Build the Verse code-mode composer dock from existing Autopilot composer plus these interaction ideas. |
| `session-composer-region.tsx` | stack prompt with permission/question/follow-up/revert/todo docks | Add a Verse code dock stack; do not make every active interruption a separate floating pane. |
| `session-permission-dock.tsx` | deny/allow once/allow always action triad with pattern detail | Shape Autopilot approval prompts with a compact triad and scoped "always" semantics. |
| `permission-auto-respond.ts` | auto-accept scoped by directory and session lineage | Adapt to Pylon session/workspace lineage so "always" is explicit and bounded. |
| `timeline/projection.ts`, `timeline/rows.ts` | stable tagged row projection with object reuse | Implement `agent-stream-projection.ts` for code events over Verse. |
| `timeline/model.ts` | history sync and load-older without scroll churn | Agent Stream pane should page older events while streaming new ones. |
| `session-side-panel.tsx`, `review-tab.tsx`, `file-tabs.tsx` | file/review tabs, diff style, scroll restoration, user-interaction guard | Build Diff/Artifacts pane with durable scroll and selection. |
| `terminal-panel.tsx` | terminal panel focus repair, height clamp, handoff titles | Build Terminal/Log pane without stealing mouselook except when focused. |
| `debug-bar.tsx` | performance HUD with frame/jank/INP signals | Add optional owner diagnostics for Verse coding overlay smokes and bug reports. |

## 3. What Not To Harvest

OpenCode is useful, but the wrong harvest would make Autopilot worse.

Do not copy:

- Electron as a platform decision. Autopilot Desktop is Electrobun/Foldkit and
  already has its own packaging/release path.
- Solid components directly into Foldkit. Port concepts and state machines, not
  JSX.
- A full IDE sidebar into Verse explore mode. The owner asked for a mode over
  Verse, not a dashboard taking over first render.
- OpenCode's local server authentication model verbatim. Pylon/Bun already has
  the relevant authority model and bearer/control-token boundary.
- WSL management as a P0. It is useful later for Windows parity, but it should
  not block local Mac Verse coding mode.
- The full file tree as the first code-mode pane. Start with active diffs,
  artifacts, and selected files.
- Global auto-accept defaults. Any "always" action must stay scoped to
  workspace/session lineage and must be visible.

## 4. How This Refines The Verse Overlay Audit

The prior audit said "managed panes in code mode." OpenCode suggests a more
precise product split:

```text
Verse explore mode
  3D scene owns input
  minimal world HUD

Verse code mode
  3D scene remains live
  bottom/edge dock stack
    composer
    permission prompt
    question prompt
    follow-up suggestions
    revert/todo prompts
  floating pane layer
    agent stream
    sessions
    decisions
    diffs/artifacts
    terminal/log
    swarm
  command palette
    scoped commands from active session/panes
```

This means the first implementation should not open the Composer as just
another large floating pane. The first implementation should probably:

1. Add code mode.
2. Show a compact code dock at the bottom or lower left.
3. Open Agent Stream as the primary floating pane.
4. Let Decisions, Diff/Artifacts, Terminal/Log, Sessions, and Swarm open as
   panes.

OpenCode also suggests that **Agent Stream needs a projection module before it
needs fancy UI.** The projection should convert Pylon session events into stable
rows with stable keys. Suggested Autopilot row tags:

- `TurnGap`
- `UserObjective`
- `AssistantText`
- `Planning`
- `ToolCall`
- `FileChange`
- `CheckRun`
- `ApprovalRequested`
- `ApprovalResolved`
- `DiffSummary`
- `Artifact`
- `Error`
- `Done`

Row projection should live outside the view and outside `three-effect`.

## 5. Concrete Autopilot Changes Suggested By OpenCode

### 5.1 Add A Scoped Command Registry For Code Mode

Autopilot currently has static palette commands in `nav.ts` and a pure
keyboard interpreter in `keyboard.ts`. That is good for the launch shell.
Code mode needs scoped commands:

- session-specific commands
- pane-specific commands
- active approval commands
- selected diff/artifact commands
- terminal/log commands
- command metadata usable by hotbar/tooltips
- editable-target filtering that lets text areas type normally
- a suspension mechanism while a modal/picker owns input

This can be implemented in the Autopilot reducer style rather than copying
OpenCode's Solid context.

### 5.2 Add A Code Dock Stack

Implement a dock stack that is separate from floating panes:

- Composer dock
- Permission dock
- Question dock
- Follow-up dock
- Revert/todo dock later

The dock stack should be part of Verse code mode only. It must have explicit
pointer ownership and must not be present in clean first-render explore mode.

### 5.3 Add Agent Stream Projection

Create an Autopilot projection module that takes:

- `SessionSummary`
- public-safe `SessionEvent[]`
- artifacts/diff summaries
- pending approvals
- active composer state

and returns stable row objects. The view renders those rows in a floating
Agent Stream pane.

Rules:

- No raw secrets.
- No raw unbounded logs.
- Stable row keys.
- Preserve row object identity when content is unchanged.
- Keep row projection independent from Foldkit view code.
- Keep row projection outside `TrainingRunVisualizationOptions`.

### 5.4 Harden Approval Semantics

OpenCode's permission dock has a clean triad:

- deny
- allow once
- allow always

Autopilot should adapt this to Pylon approvals:

- Reject
- Allow this request
- Allow matching requests for this session/workspace

Any broader auto-approval must show its scope. Do not hide it inside settings.

### 5.5 Add Host Readiness And Diagnostics To Code Mode

OpenCode treats sidecar readiness and unresponsive-window recovery as real
desktop UX. Autopilot should add a code-mode host diagnostics pane or compact
status row:

- Pylon host ready/offline/starting
- active server URL or local node identity, without raw tokens
- session event stream connected/reconnecting/fallback poll
- last transcript persistence status
- export desktop/Pylon logs
- copy public-safe diagnostic bundle
- record scene diagnostics around code-mode failures

This pairs directly with the recent Verse flicker/mouselook work: when
something breaks, the operator should have logs and scene diagnostics without
guessing.

### 5.6 Add Scroll And Focus Restoration Rules

Borrow OpenCode's behavior, not its code:

- restore scroll after render
- stop restoring when the user scrolls, wheels, presses a key, or touches
- clamp terminal/log panes to viewport
- retry focus after opening terminal/editor panes
- blur closed panes so keyboard ownership returns to the scene
- never let hidden panes remain focusable or pointer-active

This is a direct guard against Verse mouselook and keyboard regressions.

## 6. Updated Implementation Sequence

The OpenCode harvest suggests this order:

1. **Verse code mode state**
   - Add explore/code mode.
   - Preserve clean first render.
2. **Code dock stack**
   - Composer dock first.
   - Permission dock second.
   - Question/follow-up docks later.
3. **Scoped command registry**
   - Keep static nav commands.
   - Add code-mode scoped commands.
   - Add editable-target and modal suspension tests.
4. **Agent Stream projection**
   - Build row algebra and tests.
   - Render simple pane first.
5. **Diff/Artifacts and Terminal/Log panes**
   - Add scroll/focus restoration.
   - Keep authority in Bun/Pylon.
6. **Host readiness and diagnostics**
   - Add code-mode status/diagnostics.
   - Add export diagnostic bundle.
7. **`three-effect` overlay host**
   - Keep generic DOM overlay host in `three-effect`.
   - Use it for fullscreen overlay and world-anchored info.
8. **Reusable smoke**
   - Launch Verse.
   - Toggle code mode.
   - Use dock.
   - Stream stub session rows.
   - Open diff/terminal panes.
   - Verify scene movement/mouselook/zoom/focus.
   - Verify no remount/flicker/reset.

This order gives the owner a usable coding overlay sooner, while still moving
toward the cleaner `three-effect` DOM overlay host boundary.

## 7. Test Ideas To Add From OpenCode

OpenCode has many small tests around keybind parsing, timeline projection,
terminal panels, file tabs, directory pickers, updater state, and WSL runtime.
For Autopilot Verse coding mode, add equivalent focused tests:

- Command registry:
  - parses Mac and non-Mac keybinds
  - ignores normal typing in editable fields
  - permits specific editable shortcuts
  - suspends while modal/picker owns focus
- Code dock:
  - hidden in explore mode
  - visible in code mode
  - permission prompt buttons emit correct typed messages
  - "always" shows scope
- Agent stream projection:
  - stable row keys
  - object reuse when rows are unchanged
  - raw event details redacted
  - diff/check/approval rows rendered from real Pylon event samples
- Focus and input:
  - focused text input blocks WASD/mouselook
  - blurred/closed pane returns control to scene
  - hidden panes are inert
- Scroll restoration:
  - pane restores scroll after re-render
  - restoration stops after user scroll/keyboard/pointer interaction
- Packaged smoke:
  - no black frame
  - no character flicker
  - no remounts during code stream updates
  - no text cursor over canvas unless a text field is focused

## 8. OpenCode-Aligned Sync For The Codex Ladder

The primary implementation ladder now lives in
`2026-06-21-autopilot-verse-coding-agent-pane-overlay-audit.md` as
`VCODE-01` through `VCODE-16`. This companion audit should be read as the
OpenCode reference map for that ladder, not as a separate backlog.

| Ladder step | OpenCode harvest | Sync point |
| --- | --- | --- |
| `VCODE-01` Verse code mode | App-shell/runtime separation | Toggle code mode without remounting the scene or leaking commands into explore mode. |
| `VCODE-02` Codex account inventory | Desktop server readiness surfaces | Show multiple Codex accounts from existing `dev.accounts`/node-state projection, with concise labels and redacted refs. |
| `VCODE-03` Codex account management pane | Platform API + desktop settings patterns | Make account CRUD a normal code-mode pane while Bun/Pylon keep account-home authority. |
| `VCODE-04` Codex composer picker | Composer target selection | Keep account, adapter, repo/worktree, and objective visible before spawn. |
| `VCODE-05` command registry | `context/command.tsx` scoped commands | Use dynamic scopes, editable-target guards, and suspension while focused panes own input. |
| `VCODE-06` code dock stack | Composer/permission/question/follow-up docks | Put short-lived interaction prompts in a dock; keep durable inspection in panes. |
| `VCODE-07` Agent Stream projection | Timeline row algebra | Build stable tagged rows for objective, plan, tool, file, check, approval, error, and done events. |
| `VCODE-08` sessions/detail panes | Session navigation and side panels | Preserve selection and scroll across stream updates. |
| `VCODE-09` approvals | Permission dialog | Use reject, once, scoped-always with visible scope before persistence. |
| `VCODE-10` diff/artifacts | Review/file panels | Keep diffs durable, scroll-restored, and redacted by default. |
| `VCODE-11` terminal/log | Terminal panel focus behavior | Make terminal focus explicit so it cannot steal mouselook while hidden. |
| `VCODE-12` sync loop | Local server as authority | Sync accounts, sessions, event rows, transcripts, readiness, and fallback polling in one typed model. |
| `VCODE-13` multi-Codex routing | Workspace/server target defaults | Make explicit account selection outrank last-used, priority, and default home. |
| `VCODE-14` diagnostics | Debug bar and desktop diagnostics | Surface Pylon/Bun/Codex readiness plus scene/input/remount counters. |
| `VCODE-15` integration smoke | Desktop app smoke/debug affordances | Exercise dock, panes, stream, account picker, mouselook, zoom, focus, and no remount/flicker. |
| `VCODE-16` non-Codex extension | Provider-neutral session concepts | Extend the proven Codex contract to Claude Agent/Fable after Codex is smooth. |

Two OpenCode lessons are especially important for the multi-account Codex
start:

- Treat account selection as desktop-host state projected into the UI. The UI
  can add, select, and prioritize refs, but Bun/Pylon remain responsible for
  resolving homes and spawning processes.
- Treat the timeline as a stable projection. Multi-account Codex sessions will
  stream concurrently, so rows must key by session/event refs and carry only
  short labels plus redacted account identity in normal views.

## 9. Bottom Line

OpenCode should not be treated as a codebase to copy. It should be treated as a
reference for proven interaction shapes:

- local desktop host authority behind a typed bridge
- sidecar/server readiness surfaced as real app state
- dynamic scoped commands
- a prompt/permission/follow-up dock stack
- stable timeline row projection
- durable diff/terminal scroll and focus restoration
- desktop diagnostics when something hangs

Those pieces fit the previous Verse overlay audit cleanly. Together they point
to a stronger design: **Autopilot Verse code mode should combine a compact dock
stack with floating panes, backed by stable public-safe projections, while
`three-effect` supplies only the generic DOM overlay host.**
