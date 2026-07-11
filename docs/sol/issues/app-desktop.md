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
- Closed P0 historical conversation/subagent slice:
  [`desktop-codex-subagent-history.md`](./desktop-codex-subagent-history.md)
  (#8674)
- Immediate bounded queue: #8675 trace-workspace acceptance, #8676 real
  streamed Desktop→mobile conversation, #8677 fault convergence, and #8678
  Effect scope topology.

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
- an Effect Native conversation workspace with a loss-accounted owner-local
  Codex history catalog/page capability through Runtime Gateway v4: active and
  archived rollouts, real parent/depth agent graph, source-order typed items,
  explicit redaction/gaps, paging/windowing, and a three-pane Agents/Item
  inspector without uploading local history;
- project-root selection, bounded root listing/read/edit/save with conflict and
  atomic-write checks, plus typed read-only Git status/diff;
- a closed command registry and palette;
- a local diagnostic Fleet brief that explicitly is not a FleetRun;
- Codex readiness and isolated Pylon device-auth Settings.
- host-owned Khala Sync SQLite persistence and an Electron `safeStorage`
  native-session vault with private atomic ciphertext, encryption-backend
  refusal, invalid-record purge, and a tokenless Runtime Gateway phase.
- recovered-session validation through the existing native-session boundary,
  rotation-before-ready persistence, denial/owner-mismatch purge, transient
  retention, and bounded verified/unavailable gateway state.
- a distinct `openagents-desktop` public-client issuer policy for exact RFC
  8252 literal-loopback GitHub code + S256 entry; Desktop does not claim the
  mobile custom scheme.
- host-composed loopback listener/browser/exchange/server-verification and
  fail-closed dual-revocation sign-out behind argument-free Runtime Gateway
  commands with bounded outcomes.
- visible Effect Native Settings session phase and typed sign-in/sign-out
  intents over argument-free Runtime Gateway commands, with honest disabled
  in-flight state and no credential/callback projection.
- visible authoritative Sync conversations (#8670), the matching mobile
  continuation (#8671), and a shared confirmed provider-neutral timeline reader
  (#8672); #8673 landed its schema-bounded Runtime Gateway v3 timeline query at
  `bf4037e923` without claiming visible UI.
- #8674 closed at `c83f5faac9` after 138 Desktop tests, build, packaged Electron
  smoke, Effect Native accessibility tests, a 100+ MiB/100-child/100,000-item
  scale corpus, and a structure-only real nested-history receipt with zero
  unsupported gaps.

The normal Desktop `verify` gate is green: typecheck, contract/e2e tests,
bundle, and real-Electron fixture smoke. #8674 is code-landed and host-smoke
proven, but #8675 still owns the owner-visible predictable trace-workspace
receipt needed for the next recorded demo.

Not yet claimed: physical Desktop auth acceptance, a provider-launched live
stream attached to the confirmed thread/timeline, physical mobile continuation,
full workbench, visible authoritative Fleet cockpit, signed distribution, or
legacy retirement. #8676 owns the next real vertical slice; #8677 owns its
fault acceptance.

## Required product shape

1. **Work and activity:** persistent conversations/sessions, context, requests,
   approvals, outcomes, next actions, and lossless historical parent/subagent
   activity.
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
  rich composer, history, context, and mobile continuation. Closed #8673/#8674
  supply the confirmed timeline plus provider-native historical graph,
  completeness accounting, and three-pane inspector; #8675 accepts that UX in
  real Electron, then #8676 launches and attaches the real live runtime.
- **D2:** projects/sessions/routes/tabs/search/archive, command registry/palette,
  conflict-safe keybindings, menu, deep links, single instance, and restore.
  #8678 freezes the process/WorkContext/run/request/foreign-host Effect scope
  law before broader D3/D4 services land.
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
- Every visible milestone has an executable UX promise and real-Electron smoke;
  a fixture or screenshot alone cannot satisfy product acceptance.

## Exit

The everyday OpenCode workflow completes through the hardened app: open a
project/session, stream agent work, edit/save, review a diff, use bounded
terminal/context, configure the real runtime, inspect/control a real FleetRun,
and resume after restart/reconnect/mobile handoff. Every control converges to
one durable outcome/receipt. The packaged app is installable, updateable, and
recoverable, and deprecated Electrobun product/release paths cannot ship.
