# P0 TRACK: OpenAgents Desktop workbench, Sync, and runtime gateway

- Issue: #8574
- Program parent: #8566
- Destination: `apps/openagents-desktop`
- Status: active P0 under Master Revision 105 / R0–R7 / D0–D6
- Dispatch: no; current/next/open language below is pinned issue-source prose
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Architecture:
  [`../2026-07-10-openagents-desktop-product-architecture.md`](../2026-07-10-openagents-desktop-product-architecture.md)
- Capability audit:
  [`../2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](../2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)
- Current work and ownership: live issue comments plus
  [`../CLAIM_PROTOCOL.md`](../CLAIM_PROTOCOL.md)
- Local coding cutover graph:
  [`../2026-07-11-openagents-coding-cutover-issue-plan.md`](../2026-07-11-openagents-coding-cutover-issue-plan.md)
- Parallel persistent-audio track:
  [`../../voice/2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md`](../../voice/2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md)
  (#8733; leaves #8734–#8741)
- Persistent-audio process split:
  [`../../voice/2026-07-12-effect-vs-rust-audio-architecture-decision.md`](../../voice/2026-07-12-effect-vs-rust-audio-architecture-decision.md)
- Closed P0 historical conversation/subagent slice:
  [`desktop-codex-subagent-history.md`](./desktop-codex-subagent-history.md)
  (#8674)
The dated cutover graph preserves acceptance history, not the immediate queue.
CUT-01 through CUT-26 are closed. #8707 is the only remaining ordinary
local-coding cutover gate; refresh its live evidence before selecting work.

## Outcome

Build a practical OpenCode-parity coding workbench and server-authoritative
Fleet cockpit on Effect Native with a hardened Electron host. Direct typed
software controls are primary. Sarah is not a required relationship or
steering surface; a future assistant may consume the same typed action registry
under the same policy and approval boundaries.

#8733 is now that bounded P1-parallel audio consumer. It may extend the
host/renderer/media boundary only through its AUDIO-1 contract freeze and must
reuse the existing Desktop command/outcome authority. It is not a D0–D6/CUT-27
completion dependency.

This is not a rename or in-place conversion of
`clients/khala-code-desktop`. The Electrobun application remains a frozen
behavior/service/release extraction source until successor and migration proof.

## Current truthful baseline

- CUT-01 through CUT-26 (#8681–#8706) are closed. Together they represent the
  D0–D6 implementation baseline: one Effect Native tree, hardened Electron and
  explicit Effect lifecycle boundaries, authenticated Sync, projects/sessions,
  streamed and historical agent topology, composer/context, files/editor/Git/
  bounded terminal, commands/keybindings, named runtime/account/model/MCP and
  permission/settings surfaces, authoritative Fleet controls, accessibility,
  diagnostics/recovery, signed distribution, rollback, and legacy desktop
  release lockout.
- The prerequisite conversation, fault, service-topology, and simultaneous-
  provider proofs #8640, #8676, #8677, and #8678 are closed. They remain
  prerequisites and do not substitute for the counted CUT-27 product journey.
- CUT-26 supplies the signed/notarized/stapled RC5 installed-artifact,
  production update, rollback/downgrade-refusal, reinstall, diagnostics, and
  hardened Electron receipt.
- #8707 remains open. Its exact residual is a successful installed task through
  one named entitled Claude account, literal per-counted-Codex-and-Claude-task
  physical-iOS/Android-emulator continuation and interruption convergence, one
  consolidated public-safe cutover bundle, and the bounded default-surface
  declaration. No historical “next CUT-05” or broad “not yet claimed” list is
  an implementation queue.
- #8733 and AUDIO-1 through AUDIO-7 are closed; AUDIO-8 #8741 remains a
  separately tracked P1-parallel owner-microphone/evidence gate. It is not a
  D0–D6, CUT-27, #8574, or #8566 completion dependency.

After #8707 closes, check every Exit row below against its final bundle and
the closed CUT receipts. A missing row becomes a bounded defect/acceptance
leaf; it is not waived by this current-state reconciliation. Portable remote
workrooms, graph-wide host movement, managed-provider breadth, any-host mobile
control, and portable voice remain in PORT-03 through PORT-08 and are not
claimed by closing this ordinary local Desktop track.

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
  completeness accounting, and three-pane inspector; closed #8675 accepts that
  UX in real Electron, then #8676 launches and attaches the real live runtime.
  A bounded follow-on must prove the same causal child card, complete topology,
  lifecycle/latest durable activity, independent transcript, and replay-
  deduplication contract live; historical projection is not sufficient.
- **D2:** projects/sessions/routes/tabs/search/archive, command registry/palette,
  conflict-safe keybindings, menu, deep links, single instance, and restore.
  Pointer and keyboard supervision invoke the same typed action at scale.
  #8678's topology, enforcement, replaceability, disposal, correlation, and
  built-host acceptance are complete through CUT-03/CUT-04; preserve that
  architecture freeze while implementing the remaining D2/D3/D4 leaves.
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
