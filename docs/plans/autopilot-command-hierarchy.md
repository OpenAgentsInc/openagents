# Autopilot Command Hierarchy

Status: draft for Tauri shell implementation

Owner repo: `openagents`

Primary surface: `apps/autopilot`

Related docs:

- `docs/MVP.md`
- `docs/PANES.md`
- `docs/ENGINEERING_GRAPHICS_UI.md`
- `docs/codex/AUTOPILOT_TAURI_CONTROL.md`
- `docs/pylon/autopilot-test-matrix.md`

## Summary

Autopilot needs a real command hierarchy. Keep Command-K as a search and
acceleration surface over the command hierarchy. Do not make it the primary
information architecture.

The app should expose a stable menu tree, pane-local controls, and a searchable
Command-K palette generated from the same action registry. New features should
enter the product through that registry and through an explicit subsystem path.

## Problem

The current Tauri shell has a command dialog with grouped actions for Pylon,
Proof, and Theme. It also has a text command entry that switches views by loose
keyword matching. This is useful for the prototype. It does not scale.

The current shape asks Command-K to do three jobs at once:

- navigation
- mutation
- discovery

This turns the palette into a flat list as features are added. The operator has
to scan unrelated commands and infer structure from labels. That conflicts with
Autopilot's engineering graphics direction, which calls for explicit labels,
authority paths, state, evidence, and exact failure modes.

## Product Direction

Autopilot should use a menu hierarchy as the command source of truth.
Command-K should search that hierarchy.

The command system should make each action's subsystem, authority path, effect,
and safety class visible. A mutating command should never look like a vague
search result. It should carry enough context for an operator to understand
what subsystem will change and what evidence will update afterward.

## Interaction Model

Autopilot should use three command surfaces.

### 1. Top menu hierarchy

The menu hierarchy is the canonical product structure. It should be stable,
learnable, and organized by subsystem nouns.

Initial hierarchy:

```text
Autopilot
  Settings
  Check Runtime Status
  Quit

View
  Command Console
  Pylon
  Proof Flow
  Activity
  Logs
  Artifacts

Pylon
  Show Status
  Refresh
  Start Serve
  Stop Serve
  Restart Serve
  Provider Mode
    Online
    Offline
    Pause
    Resume
  Open Logs

Proof
  Show Flow
  Run Lane
    CS336 A1
    Stale Recovery
    Replacement Attempt
  Doctor Namespace
  Stop Namespace
  Reset Namespace
  Open Artifacts

Help
  Diagnostics
  Control Plane Status
```

The first implementation may render this as in-app dropdown menus if native
Tauri menu integration is not ready. The model should still be a real command
tree, not hardcoded JSX in each menu component.

### 2. Pane-local controls

Pane-local controls remain. If the operator is viewing Pylon, Pylon actions
belong in that panel. If the operator is viewing Proof Flow, proof actions
belong in that panel.

This is deliberate duplication. The menu tree provides global structure.
Pane-local controls provide direct manipulation of the visible object.

### 3. Command-K locator

Command-K should search the same action registry used by menus and pane-local
controls.

Results should display breadcrumbs:

```text
Pylon > Provider Mode > Online
Proof > Run Lane > CS336 A1
View > Proof Flow
Help > Diagnostics
```

Command-K should prefer active-pane actions first, then global actions, then
other subsystem actions. It should show disabled reasons instead of hiding
commands when the disabled state teaches the operator what is missing.

## Engineering Graphics Requirements

The command hierarchy must follow `docs/ENGINEERING_GRAPHICS_UI.md`.

Requirements:

- Use dense labels and stable grouping.
- Show exact command labels, subsystem paths, and disabled reasons.
- Classify commands by kind: view, safe, mutating, destructive.
- Show authority path for mutating commands.
- Show expected effect for mutating commands.
- Connect completed commands to evidence: status rows, event tape entries,
  last action, last error, timestamps, run reports, or artifacts.
- Avoid decorative motion or oversized empty search UI.
- Keep visual hierarchy tight and operational.

## Action Registry

`apps/autopilot` should define one action registry and render all command
surfaces from it.

Initial TypeScript shape:

```ts
type ActionKind = "view" | "safe" | "mutating" | "destructive";
type ActionScope = "global" | "active-view" | "selection";
type ActionAuthority =
  | "local-tauri"
  | "pylon"
  | "proof"
  | "wallet"
  | "network";

type AutopilotAction = {
  id: string;
  label: string;
  menuPath: string[];
  paletteKeywords: string[];
  shortcut?: string;
  scope: ActionScope;
  kind: ActionKind;
  authority: ActionAuthority;
  effect: string;
  evidence: string[];
  disabledReason?: string;
  run: () => Promise<void>;
};
```

Example action:

```ts
{
  id: "pylon.provider.online",
  label: "Online",
  menuPath: ["Pylon", "Provider Mode", "Online"],
  paletteKeywords: ["provider", "mode", "earn", "online"],
  scope: "global",
  kind: "mutating",
  authority: "pylon",
  effect: "Requests provider online mode through local Tauri IPC.",
  evidence: ["provider state", "last action", "last error", "updated"],
  run: async () => setProviderMode("online"),
}
```

The registry should be the source for:

- top app menus or in-app dropdown menus
- Command-K
- pane-local action rows
- context menus
- programmatic control documentation
- UI tests and smoke tests

## Command-K Behavior

Command-K should render one row per action. Rows should include:

- command label
- breadcrumb path
- optional shortcut
- command kind
- authority badge for mutating commands
- disabled reason when disabled

Command-K should not own command taxonomy. It should not contain commands that
do not exist in the registry. It should not use loose text parsing as the main
execution model.

The current `CommandEntry` text box can remain as a command console only if it
maps typed commands to registered actions through exact IDs, aliases, or a
documented parser. It should not silently switch panes through broad keyword
matches.

## Menu Rendering Requirements

The first menu implementation should support:

- top-level menus
- nested submenus
- separators
- disabled items with reasons
- keyboard shortcut labels
- destructive styling for destructive commands
- shared execution path with Command-K

The initial Tauri prototype can use the existing `dropdown-menu.tsx` and
`navigation-menu.tsx` primitives. Native Tauri menu integration can follow
after the action registry is stable.

## Safety Classes

Command visual treatment should reflect action kind.

| Kind | Meaning | UI behavior |
| --- | --- | --- |
| `view` | Switches view or opens a read-only surface | No confirmation |
| `safe` | Refreshes, opens logs, opens artifacts, or diagnoses | No confirmation |
| `mutating` | Changes runtime state | Show authority and effect |
| `destructive` | Resets, deletes, kills, or clears state | Require confirmation or explicit guarded UI |

Current examples:

- `View > Pylon`: `view`
- `Pylon > Refresh`: `safe`
- `Pylon > Start Serve`: `mutating`
- `Proof > Reset Namespace`: `destructive`

## Tauri Control Relationship

The visual command registry and the programmatic Tauri control surface should
stay aligned.

When a UI action invokes a Tauri command that affects runtime state, the same
behavior should remain testable through `autopilotctl-tauri` and the smoke
scripts named in `docs/codex/AUTOPILOT_TAURI_CONTROL.md`.

The registry does not replace the control plane. It gives the operator-facing
UI a clear action model and gives tests a stable inventory to assert against.

## Migration Plan

1. Add an action registry module in `apps/autopilot/src`.
2. Move current Pylon, Proof, Theme, and View actions out of `App.tsx` into the
   registry.
3. Render Command-K from the registry with breadcrumb rows and disabled reasons.
4. Add a hierarchical menu bar or in-app top menu from the same registry.
5. Update pane-local controls to dispatch through registered actions where it
   improves consistency.
6. Replace broad keyword matching in `CommandEntry` with exact action IDs,
   aliases, or remove the console from the default first screen.
7. Add tests that verify the menu tree, Command-K rows, and smoke-control
   commands stay aligned.
8. Update `docs/PANES.md`, `docs/MVP.md`, and the Tauri control docs after the
   implementation lands.

## Implementation Issues

Create these GitHub issues in `OpenAgentsInc/openagents`:

1. Define `apps/autopilot` action registry and command metadata model.
2. Render Command-K from the action registry with breadcrumb rows.
3. Add hierarchical Tauri top menus or in-app dropdown menus from the registry.
4. Route Pylon and Proof pane-local controls through registered actions.
5. Replace loose command-entry keyword routing with exact actions or remove it
   from the default first screen.
6. Add tests and docs gates for command hierarchy, Command-K, menus, and Tauri
   control alignment.

## Acceptance Criteria

- `apps/autopilot` has one typed action registry for command metadata and
  execution.
- Menus and Command-K render from that registry.
- Command-K rows show breadcrumb paths.
- Mutating commands expose authority and effect.
- Disabled commands show reasons.
- Destructive commands are guarded.
- The initial Pylon and Proof commands are covered by the registry.
- Existing Tauri control smoke still passes.
- `docs/PANES.md` no longer describes the command palette as a flat product
  architecture for the Tauri shell.
