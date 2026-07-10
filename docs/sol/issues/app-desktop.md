# P0 TRACK: OpenAgents Desktop workbench, Sync, and runtime gateway

- Issue: #8574
- Program parent: #8566
- Destination: `apps/openagents-desktop`
- Status: active P0 under R0–R7/D0–D6
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Architecture:
  [`../2026-07-10-openagents-desktop-product-architecture.md`](../2026-07-10-openagents-desktop-product-architecture.md)
- Capability audit:
  [`../2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](../2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)
- Bounded leaves:
  [`../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)

## Outcome

Build a practical OpenCode-parity coding workbench and server-authoritative
Fleet cockpit on Effect Native with a hardened Electron host. Direct typed
software controls are primary. Sarah is not a required relationship or
steering surface; a future assistant may consume the same typed action registry
under the same policy and approval boundaries.

This is not a rename or in-place conversion of
`clients/khala-code-desktop`. The Electrobun application remains a frozen
behavior/service/release extraction source until successor and migration proof.

## Current truthful baseline

The greenfield app now has:

- hardened Electron sandbox/isolation/navigation/permission boundaries;
- a minimal Effect Native conversation workspace;
- local bounded thread persistence, recent read-only Codex history, and
  host-held gateway completion with honest configuration failure;
- project-root selection, bounded root listing/read/edit/save with conflict and
  atomic-write checks, plus typed read-only Git status/diff;
- a closed command registry and palette;
- a local diagnostic Fleet brief that explicitly is not a FleetRun;
- Codex readiness and isolated Pylon device-auth Settings.

The normal Desktop `verify` gate is green: typecheck, contract/e2e tests,
bundle, and real-Electron fixture smoke. The current architecture receipt is
`f49a66b4aa`; this is still fixture/development proof, not a signed/live-product
claim.

Not yet claimed: authoritative Sync threads, complete streamed session state,
full workbench, visible authoritative Fleet cockpit, signed distribution, live
owner account success, or legacy retirement.

## Required product shape

1. **Work and activity:** persistent conversations/sessions, context, requests,
   approvals, outcomes, and next actions.
2. **Coding workbench:** projects/sessions, streamed agent timeline, rich
   composer/context, files/editor, Git review, bounded terminal, commands/
   keybindings, providers/models/MCP/permissions, settings, and diagnostics.
3. **Fleet cockpit:** run/work/attempt/account/worker state, approvals, command
   outcomes, proof, usage/economics, receipts, and closeout from current
   Pylon/Khala Sync authority.

Conversation remains a quiet default. Workbench/Fleet depth opens intentionally;
it does not become fabricated permanent status chrome.

## D0–D6 scope

- **D0:** truthful green baseline, capability manifest, isolated smoke state,
  and removal or completion of dormant/fake affordances.
- **D1:** authenticated Khala Sync session/conversation continuity with streamed
  reasoning/tools/plan/questions/permissions/errors/usage, interrupt/resume,
  rich composer, history, context, and mobile continuation.
- **D2:** projects/sessions/routes/tabs/search/archive, command registry/palette,
  conflict-safe keybindings, menu, deep links, single instance, and restore.
- **D3:** recursive files, grants, watcher/search/cache, edit/save/conflict,
  typed Git diff/review, selected context, and workspace-bounded PTY.
- **D4:** sign-in, provider/model/runtime catalog, MCP state/auth, permissions,
  settings, accessibility, notifications, diagnostics, and recovery.
- **D5:** authoritative Fleet cockpit and shared Desktop/mobile command outcomes.
- **D6:** identity freeze, fuses, packaging, signing/notarization, updates,
  rollback, crash/load recovery, and clean-machine proof.

## Non-negotiable Electron boundary

- `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
  `webviewTag: false`, `webSecurity: true`, restrictive CSP, deny-by-default
  permissions/navigation/window-open, and allowlisted external protocols.
- Every preload method is fixed, sender/frame-origin validated, schema-decoded,
  bounded, and least-authority.
- No raw `ipcRenderer`, MessagePort, Node/Electron built-ins, generic
  filesystem/process/shell authority, credentials, or raw private events enter
  the renderer.
- Monaco/editor and terminal depth use typed foreign-host nodes and approved
  host services.

## Shared action/Sync law

- Stable command IDs back direct manipulation and mobile/possible future
  automation.
- Policy determines immediate execution versus approval; no model prose or
  pixel state authorizes work.
- Khala Sync carries durable refs/versions/outcomes. Cursor/focus/selection
  stays local unless a typed continuity requirement says otherwise.
- Pending, rejected, failed, unavailable, reconnecting, stale, must-refetch, and
  unknown-pending-reconcile are explicit.

## Exit

The everyday OpenCode workflow completes through the hardened app: open a
project/session, stream agent work, edit/save, review a diff, use bounded
terminal/context, configure the real runtime, inspect/control a real FleetRun,
and resume after restart/reconnect/mobile handoff. Every control converges to
one durable outcome/receipt. The packaged app is installable, updateable, and
recoverable, and deprecated Electrobun product/release paths cannot ship.
