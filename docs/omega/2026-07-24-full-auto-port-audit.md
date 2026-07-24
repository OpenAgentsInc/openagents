# Full Auto port into Omega — audit

- Date: 2026-07-24
- Class: current-status and port roadmap
- Owner: OpenAgents
- Scope: port of Full Auto idea and Desktop implementation into Omega
- Packet target: `OMEGA-OA-05`
- OpenAgents source: `567998471df28c1d83b24aa7c1b45fc6b98ed213`
- Omega source: `9e585569cb21f4cf93aef70f0c1fb1d0501b64b5`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`

## 1. Question

How can OpenAgents port the Full Auto product idea and the current Desktop
implementation into Omega, the Zed-based Desktop application?

This audit answers that question.
It does not start implementation.
It does not admit a release.
It does not change ProductSpec or AssuranceSpec authority.

## 2. Result

Full Auto can move into Omega with low rewrite risk for its durable core.
Most of the run engine is already transport-agnostic TypeScript and Effect.
Omega must host that core in a supervised Node service.
Omega must not copy Electron, React, or IPC patterns into GPUI.

The correct port sequence is:

1. Finish the shared runtime seam (`OMEGA-OA-01`).
2. Finish one real agent turn path (`OMEGA-OA-02`).
3. Finish conversation controls (`OMEGA-OA-03`).
4. Finish identity, Sync, and mobile continuity (`OMEGA-OA-04`).
5. Port Full Auto as `OMEGA-OA-05`.

Do not start Full Auto before those four packets.
Full Auto needs a durable action layer, a bound workspace, and Sync continuity.

The port must keep these product laws:

- Full Auto is a run, not a composer option.
- The limit is eight active runs.
- Each thread holds one active lease.
- Provider text cannot close a run.
- Only typed outcomes can close a run.
- Own-capacity routing stays fail-closed.
- Mobile can send control intents.
- Desktop remains the sole executor.

The largest rewrite is the GPUI launcher and run monitor.
The largest reuse is `full-auto-run-actions.ts` and the loopback control
surface.
The largest risk is a second durable authority in GPUI state.

## 3. Sources

The audit used these sources on 2026-07-24:

- All files in `docs/omega/`.
- [Accepted Omega plan](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md).
- [Omega roadmap](./ROADMAP.md) packet `OMEGA-OA-05`.
- Transcript [25X Full Auto](../transcripts/25X-fullauto.md).
- [Full Auto ProductSpec](../../specs/desktop/full-auto.product-spec.md).
- [Full Auto AssuranceSpec](../../specs/desktop/full-auto.assurance-spec.md).
- [2026-07-17 implementation audit](../fable/2026-07-17-full-auto-implementation-audit.md).
- [2026-07-20 verifiable-mode design](../fable/2026-07-20-full-auto-first-verifiable-mode.md).
- [2026-07-21 status audit](../fable/2026-07-21-full-auto-status-audit.md).
- [Desktop Full Auto deep dive](../sol/2026-07-16-openagents-desktop-full-auto-deep-dive.md).
- [One-click concurrent runs](../sol/2026-07-18-full-auto-one-click-concurrent-runs-implementation.md).
- OpenAgents Desktop modules under `apps/openagents-desktop/src/full-auto-*`.
- Omega crates under `/Users/christopherdavid/work/omega/crates/`, especially
  `agent`, `agent_ui`, `agent_servers`, `acp_thread`, `app_identity`, and
  `omega_identity`.

## 4. Product intent

### 4.1 Owner need from transcript 25X

The owner needs agents that continue while the owner is away.
The owner needs many personal accounts and providers in one system.
The owner needs smart rotation when one account exhausts its budget.
The owner needs a start action that means leave the keyboard.

The composer toggle failed that need.
It looked like a message option.
It mixed chat steering with unattended work.
The owner asked for a dedicated Full Auto mode with play, pause, and stop.

### 4.2 Current product contract

Full Auto is a durable, restart-safe, autonomous continuation loop.
The owner states one objective and one done condition.
The host keeps dispatching admitted turns against one granted workspace.
The loop stops only on stop, typed failure, typed completion, or a turn cap.

The current Desktop surface is a dedicated launcher and a read-only run view.
Retire the legacy composer toggle.
Ordinary chat must not start Full Auto authority.

### 4.3 Verifiable-mode role

Full Auto is also the first verifiable mode.
Readiness must gate the run.
Express capacity as typed truth.
Apple FM stays advisory only.
Provider self-report cannot prove success.
Reports and receipts must close the evidence chain.

## 5. Current Desktop implementation

### 5.1 Topology

Full Auto lives in OpenAgents Desktop.
It is a main-process feature with about sixty `full-auto-*` modules.
There is no Rust or Swift Full Auto core today.

| Concern | Primary module |
| --- | --- |
| Run state machine | `full-auto-run-registry.ts` |
| Per-thread lease and profile | `full-auto-registry.ts` |
| Dispatch decision | `full-auto-reconcile.ts` |
| Transport-agnostic actions | `full-auto-run-actions.ts` |
| Liveness and stall class | `full-auto-liveness.ts` |
| Routing and readiness | `full-auto-routing.ts`, `full-auto-readiness.ts` |
| Capacity ledger | `full-auto-capacity.ts` |
| Lane policies | `full-auto-lane.ts` |
| Provider handoff | `full-auto-provider-handoff.ts` |
| Mission packet | `full-auto-mission.ts` |
| Verification and completion | `full-auto-verification.ts`, `full-auto-completion.ts` |
| Reports and receipts | `full-auto-run-report.ts` |
| Control HTTP surface | `full-auto-control-server.ts` |
| Electron IPC transport | `full-auto-run-ipc-contract.ts` |
| Mobile projection | `full-auto-run-projection-publisher.ts` |
| Mobile control intents | `full-auto-run-control-intent-consumer.ts` |
| Launcher and run UI | `renderer/react-full-auto-surface.tsx` |
| UI state helpers | `renderer/full-auto-workspace.ts` |
| RLM recall | `full-auto-recall.ts` |

### 5.2 Hard product numbers

| Rule | Value |
| --- | --- |
| Active run limit | 8 |
| Default continuation cap | 20 |
| Consecutive failure limit | 5 |
| Reconcile fan-out | 8 |
| Terminal states | `completed`, `failed`, `stopped`, `cap_reached` |
| Active states | `running`, `pausing`, `paused`, `retrying`, `stalled` |

### 5.3 Action lanes

Current action lanes are:

- `codex-local`
- `claude-local`
- `acp:grok-cli`
- `acp:cursor-agent`
- `harness:goose`
- `harness:opencode`
- `harness:pi`

Apple FM is advisory only.
It is not an action lane.

### 5.4 Authority map

Full Auto may:

- dispatch an admitted turn on an owner account
- continue after a settled turn inside the bound workspace
- rotate on typed capacity or provider failures
- pause, resume, stop, and retry
- emit private reports and public-safe receipts
- notify the owner with redacted title and state

Full Auto may not:

- grant new commit, merge, or push authority
- leave the workspace bound at start
- use third-party or pooled capacity
- trigger a rate-limit reset as a strategy
- treat provider prose as completion proof
- put objective or transcript text into public receipts or notifications

Full Auto is not Pylon and not FleetRun.
Those systems remain separate.
Khala Sync carries only the redacted projection and mobile control intents.

### 5.5 Electron-only seams

These parts cannot move as-is:

- Electron main and renderer split
- `ipcMain` and preload channels
- React DOM launcher and CSS
- Electron `userData` path resolution
- Electron notification permission bridge
- Electron package and forge release path
- the large orchestration glue in `main.ts`

These parts can move into a supervised Node service with small adapters:

- registries and legal transitions
- reconcile and liveness decisions
- routing, readiness, and capacity
- run actions
- reports and receipts
- control OpenAPI surface
- Sync projection and control-intent consumer
- mission, verification, completion, and recall logic

## 6. Omega substrate today

### 6.1 What Omega already has

Omega is a tracked Zed fork with GPUI.
It already has:

- ACP process supervision in `crates/agent_servers`
- ACP thread models in `crates/acp_thread`
- multi-thread agent panel UI in `crates/agent_ui`
- native agent storage and tool policy in `crates/agent`
- worktree trust, Git, terminal, and project truth in Rust
- Omega app identity in `crates/app_identity`
- sovereign identity custody work in `crates/omega_identity`

The Threads Sidebar already hosts concurrent sessions.
That is not a Full Auto run scheduler.
It is a useful UI pattern for a later concurrent run monitor.

### 6.2 What Omega lacks

Omega does not have:

- a durable Full Auto run ledger
- an eight-run scheduler with one lease per thread
- typed own-capacity routing across accounts
- Khala Sync conversation and Full Auto projection wiring
- restart reconciliation for many leased runs
- mobile continuity for Full Auto controls
- a dedicated objective launcher and read-only run view
- OpenAgents receipts as run closure authority

### 6.3 Design laws that bound the port

From the Omega roadmap:

1. Zed owns editor, project, buffer, language, terminal, and worktree truth.
2. OpenAgents owns work, agent, policy, receipt, and run truth.
3. Khala Sync owns shared conversation and timeline truth.
4. A UI projection must not become a second durable authority.
5. External agents keep their own configuration and credential custody.
6. Rust does not gain authority only because it is native code.

## 7. Recommended architecture

### 7.1 Process split

```text
Omega GPUI shell
  owns: panes, commands, project refs, native notifications
  projects: Full Auto run state
  sends: typed start, pause, resume, stop, retry, handoff

Rust supervisor
  owns: process life, health, restart, framed protocol
  starts: packaged Node 24 omega-effectd

omega-effectd (Node + Effect)
  owns: Full Auto registries, leases, reconcile, liveness
  owns: routing, capacity, reports, receipts
  owns: Sync projection and mobile intent apply
  dispatches: admitted provider turns through existing harness adapters

ACP / provider workers
  own: provider homes, credentials, sessions
  never: Full Auto run authority
```

### 7.2 Why this split

`full-auto-run-actions.ts` already has three callers:

- Desktop UI through Electron IPC
- loopback control API
- mobile control-intent consumer

Omega GPUI becomes a fourth caller.
It must call the same action functions.
It must not invent a parallel lifecycle.

The accepted Omega plan already selects a packaged Node service for product
control-plane work.
Full Auto is exactly that class of work.

### 7.3 Protocol requirements

The Rust and Node protocol must carry:

- schema and service versions
- capability negotiation
- request and event IDs
- stable `runRef` and `threadRef`
- generation fencing
- cancellation
- bounded queues and backpressure
- gap and overload results
- health and restart state
- redacted diagnostics

The first transport can reuse the existing loopback Full Auto control contract.
A later packet can replace HTTP with the generated framed protocol.
Do not keep two write authorities during that cutover.

### 7.4 Storage and identity

Use the Omega data root, not a Zed root and not the Electron Desktop root.
Keep Full Auto durable files under an Omega-owned path such as
`full-auto/runs.json` and `full-auto/run-reports.json`.
Keep mode `0600` and atomic rename behavior.
Do not migrate Electron Full Auto state into the first Omega packet.
Migration belongs to Phase 3 cutover work.

## 8. Port map

### 8.1 Port as Effect service code

| Desktop module | Omega home | Notes |
| --- | --- | --- |
| `full-auto-run-registry.ts` | `omega-effectd` | Keep legal transition graph |
| `full-auto-registry.ts` | `omega-effectd` | Keep lease and profile |
| `full-auto-reconcile.ts` | `omega-effectd` | Keep single decision function |
| `full-auto-run-actions.ts` | `omega-effectd` | Keep as sole mutation API |
| `full-auto-liveness.ts` | `omega-effectd` | Keep stall classes |
| `full-auto-routing.ts` and readiness | `omega-effectd` | Keep fail-closed admission |
| `full-auto-capacity.ts` | `omega-effectd` | Keep eight-run ledger |
| `full-auto-run-report.ts` | `omega-effectd` | Keep report and receipt split |
| control server and contract | `omega-effectd` | Keep OpenAPI until framed protocol |
| Sync publisher and intent consumer | `omega-effectd` | Keep Desktop as sole executor |
| mission, verification, completion | `omega-effectd` | Keep typed completion gate |
| `full-auto-recall.ts` | `omega-effectd` | Port as candidate recall only |

### 8.2 Rewrite in GPUI

| Desktop surface | Omega target | Notes |
| --- | --- | --- |
| `react-full-auto-surface.tsx` | new GPUI Full Auto panes | Dedicated launcher and read-only run view |
| `full-auto-workspace.ts` | shared projection helpers | Keep validation and labels if possible |
| Electron broadcast and timers | Rust event fan-out | Project Node events into GPUI entities |
| OS notification bridge | Omega native notifications | Keep redaction rules |

### 8.3 Do not port

| Item | Reason |
| --- | --- |
| Electron IPC channel names | Host-specific |
| React and CSS presentation | Wrong renderer |
| Monaco or xterm paths | Not Full Auto truth |
| Electron packaging | Omega has its own release path |
| composer toggle UX | Retired and forbidden |
| Pylon or FleetRun coupling | Not present and not required |
| Buzz workflow runtime | Roadmap rejects incomplete Buzz runtime |
| MemoHarness FA-AC-69..76 | Designed-only, out of first port |

## 9. Ordered port packets

These packets specialize `OMEGA-OA-05`.
They start only after `OMEGA-OA-01` through `OMEGA-OA-04` pass.

### OMEGA-FA-00: freeze the Omega Full Auto contract

Work:

- Admit the ProductSpec and AssuranceSpec delta for Omega.
- Freeze the eight-run limit and one-lease-per-thread rule.
- Freeze the ten-state lifecycle and legal transitions.
- Freeze the redaction map for receipts, notifications, and Sync.
- Freeze the non-overridable guardrails.
- Name the first lane set for Omega.
- Settle whether initiative and MemoHarness stay deferred.

Exit:

- No packet can invent a second lifecycle.
- Owner and assurance accept the freeze.

Falsifier:

- A GPUI view or ACP panel becomes run authority.

### OMEGA-FA-01: extract Full Auto into omega-effectd

Work:

- Package the portable Full Auto modules as a released service artifact.
- Inject Omega data paths.
- Keep `full-auto-run-actions.ts` as the only mutation API.
- Prove unit and integration coverage outside Electron.
- Prove deterministic shutdown with zero surviving child processes.

Exit:

- Electron Desktop and Omega can consume the same service bytes.
- Digests and versions are immutable.

Falsifier:

- Omega needs a relative monorepo path or unpublished workspace edge.

### OMEGA-FA-02: Rust supervisor and protocol

Work:

- Register `omega-effectd` as a managed supervised process.
- Reuse the ACP supervision pattern, not the user agent registry.
- Expose Full Auto commands and events on the private protocol.
- Fence stale generations and restart recovery.
- Keep redacted diagnostics only.

Exit:

- Rust can start, health-check, restart, and stop the service.
- Crash recovery restores durable run truth from disk and Sync.

Falsifier:

- GPUI local state can rewrite a durable run after restart.

### OMEGA-FA-03: GPUI launcher and concurrent monitor

Work:

- Add a dedicated Full Auto entry near new session.
- Build the one-objective launcher with collapsed advanced controls.
- Build the concurrent run monitor for up to eight active runs.
- Show a read-only conversation and turn transcript.
- Expose pause, resume, stop, and retry.
- Do not add an ordinary composer while a run is active.

Exit:

- The owner can start one run and leave the keyboard.
- The UI never looks like a chat modifier.

Falsifier:

- A composer toggle or ambient preference starts Full Auto.

### OMEGA-FA-04: routing, capacity, guardrails, and liveness

Work:

- Port readiness-gated routing.
- Port the capacity ledger.
- Port non-overridable and owner guardrails.
- Port stall classes and recovery actions.
- Wire attention notifications with redacted content.
- Keep own-capacity-only dispatch.

Exit:

- Typed rotation works on admitted lanes.
- Stall shows one owner action.
- Provider prose cannot close a run.

Falsifier:

- A silent cache eviction or missing thread ends a run without a stall class.

### OMEGA-FA-05: reports, receipts, Sync, and mobile

Work:

- Port private reports and public-safe receipts.
- Publish redacted live projections through Khala Sync.
- Apply mobile pause, resume, and stop intents with typed outcomes.
- Keep Desktop or Omega as the sole executor.

Exit:

- Mobile can supervise without local execution authority.
- Receipts carry digests and counts, not transcript text.

Falsifier:

- Mobile writes durable run state directly.

### OMEGA-FA-06: native code loop join

Work:

- Bind each run to a Zed project and worktree ref.
- Use native Git, diagnostics, and buffer truth for review.
- Keep OpenAgents admission for completion.
- Refuse stale edits and workspace mismatch.

Exit:

- A completed run points to exact native evidence.
- OMEGA-OA-06 can extend this join without a second run store.

Falsifier:

- Full Auto mutates buffers outside Zed project truth.

### OMEGA-FA-07: proof and owner journey

Work:

- Replay the 2026-07-17 thread-pressure incident shape.
- Run the owner-real acceptance matrix on Omega.
- Prove restart, pause, resume, stop, retry, and cap.
- Prove one cross-provider handoff with sidebar evidence.
- Prove offline and Sync gap behavior.
- Record independent assurance against the exact candidate.

Exit:

- `OMEGA-OA-05` can close.
- Electron remains rollback until primary cutover.

Falsifier:

- Fixture-only proof without an installed Omega journey.

## 10. Dependency order

```text
OMEGA-OA-01 shared runtime seam
  -> OMEGA-OA-02 one agent front door
  -> OMEGA-OA-03 conversations and controls
  -> OMEGA-OA-04 identity, Sync, mobile
  -> OMEGA-FA-00 contract freeze
  -> OMEGA-FA-01 service extract
  -> OMEGA-FA-02 Rust supervisor
  -> OMEGA-FA-03 GPUI launcher and monitor
  -> OMEGA-FA-04 routing and liveness
  -> OMEGA-FA-05 reports and mobile
  -> OMEGA-FA-06 native code join
  -> OMEGA-FA-07 proof
```

Parallel work is safe only when paths do not collide:

- FA-00 can draft while OA-03 and OA-04 finish.
- FA-01 can extract modules while the team designs GPUI fixtures.
- Brand and RC packets stay independent of Full Auto.

Do not parallelize two writers for:

- run registry schema
- legal transition graph
- control protocol
- Sync projection schema

## 11. Risks

### 11.1 Second durable authority

GPUI entity state can become a false run store.
Prevent this with a projection-only UI model.
All mutations go through `full-auto-run-actions` equivalents.

### 11.2 Thread-cache eviction class

The 2026-07-17 overnight failure came from eviction of an active thread.
Omega must protect active Full Auto threads from ordinary session cache policy.
A missing thread must become a typed stall, never a silent stop.

### 11.3 Ordinary chat inheritance

Issue `#9159` showed ordinary chat forcing Full Auto behavior.
Omega must keep a hard split between chat turns and Full Auto runs.
A delegate or ACP turn is not Full Auto unless a `FullAutoRun` started it.

### 11.4 Sandbox gap for external agents

Zed sandboxing applies to the native Zed agent only.
External ACP agents are not sandboxed by that path.
Full Auto guardrails must live in `omega-effectd` and OpenAgents policy.
Do not assume Seatbelt or Bubblewrap covers provider workers.

### 11.5 Two-layer registry risk

Desktop still has a legacy per-thread registry and a run registry.
Both release residuals in the 2026-07-21 audit were desynchronization bugs.
The Omega extract should keep one settle function or collapse the layers with
differential proof.
Do not invent a third layer in Rust.

### 11.6 Provider handoff proof debt

Handoff plumbing exists.
Real same-thread cross-provider proof remains a first-class acceptance gate.
Omega must not claim handoff until that matrix passes on Omega.

### 11.7 Initiative and RLM scope

RLM recall is in the roadmap text as attention and recall.
Initiative and MemoHarness change stop behavior and are less proven.
Keep recall as cited candidates only.
Defer initiative and MemoHarness unless the owner expands `OMEGA-OA-05`.

## 12. Acceptance criteria

### 12.1 Roadmap criteria for OMEGA-OA-05

Preserve:

- eight active runs
- one active lease for each thread

Port:

- default objective launcher
- advanced mission controls
- concurrent run monitor
- pause, resume, stop, and retry
- routing policy and capacity truth
- guardrails and liveness
- restart reconciliation
- reports and receipts
- attention and RLM recall

Require:

- provider text does not determine success
- only typed outcomes close a run

### 12.2 Stronger gates from Desktop truth

Also require:

1. Dedicated launcher and read-only run view.
   No composer toggle.
2. Legal transition immunity tests for the three non-overridable guardrails.
3. Redaction by explicit field lists.
   No object spread into public receipts.
4. Replay of the 2026-07-17 eviction incident shape.
5. Owner-real multi-turn unattended run on Omega.
6. One visible Codex to Claude or equivalent handoff proof.
7. Packaged restart reconciliation with durable reports intact.
8. Mobile pause, resume, and stop intents with typed outcomes.
9. No ordinary chat path can set Full Auto authority.
10. Independent assurance on the exact Omega candidate.

## 13. Current status relative to the port

| Area | Desktop | Omega |
| --- | --- | --- |
| Product idea | Stable | Accepted in roadmap |
| Run engine | Implemented | Absent |
| Dedicated UX | Implemented | Absent |
| Shared runtime seam | Electron-local | Required first |
| Identity and Sync | Present | In earlier Omega packets |
| Brand and package RC | Not the Full Auto gate | Blocks useful dogfood installs |
| Release claim | Still gated by Desktop release work | Not started |

Omega brand and RC work can continue in parallel.
They do not replace Full Auto prerequisites.
A branded editor without `OMEGA-OA-01` through `OMEGA-OA-04` cannot host Full
Auto honestly.

## 14. Non-goals

This audit does not:

- start `OMEGA-OA-05` implementation
- move Full Auto into Rust as the first authority cutover
- port Electron UI or IPC
- create an Omega-only cloud thread store
- couple Full Auto to Pylon or FleetRun
- claim Desktop release readiness
- claim Omega feature parity
- admit initiative or MemoHarness into the first Omega port
- migrate Electron Full Auto user state in the first packet

## 15. Recommended next actions

1. Keep identity, brand, and RC packets on their current track.
2. Complete `OMEGA-OA-01` before any Full Auto UI work.
3. Create `OMEGA-FA-00` as the contract freeze after Sync identity is ready.
4. Extract the portable Full Auto modules into a released `omega-effectd`
   artifact.
5. Build the GPUI launcher only after the service action API is live.
6. Treat the 2026-07-17 incident replay as a mandatory falsifier.
7. Keep Electron Full Auto as the dogfood and rollback surface until Omega
   proof passes.

## 16. Completion rule

This audit is complete as a planning record when it is on `main` and indexed
from `docs/omega/README.md`.

`OMEGA-OA-05` is complete only when the FA-00 through FA-07 packets pass with
code, tests, packaged journeys, and independent assurance.
A document is not completion.
