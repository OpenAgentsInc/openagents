# OpenAgents compared with T3 Code: current implementation gaps

> [!NOTE]
> **Status (2026-11-22): Historical 2026-11-14 snapshot — superseded.**
> This comparison was pinned to OpenAgents `097e11c86d` (iOS build **1085**) against T3 Code `0.1.9` (`f313d0a6cb`). Nearly all OpenAgents gaps described below have changed materially: iOS and Desktop moved from UIKit/JavaScriptCore to **compiled Metal/Vulkan GPU** (`apps/openagents-ios/`, `apps/openagents-desktop/`); the desktop op 14/15/16 host-contract mismatch is completely gone; Desktop & iOS gained resident `coder-host`, Tailscale zero-config pairing, per-chat worktrees, Inky + Psionic Metal 4 / CUDA, Antigravity 1.5+ core parity, interactive permission/question cards, follow-up queue/steering, and the 29-flag `--minimal` build.
>
> **Read instead:** [`docs/t3/README.md`](../../t3/README.md) (corpus index), [`docs/t3/2026-11-22-t3code-borrowing-theory.md`](../../t3/2026-11-22-t3code-borrowing-theory.md) (live disposition & next priorities), and [`docs/t3/reference.md`](../../t3/reference.md) (`t3code` `18850a5eeb`).

2026-11-14. OpenAgents source baseline: `097e11c86d39254390fbbb1316aa5728a074f04e`.
The latest recorded iOS release at that baseline is **1085**, with native
Projects. T3 reference: the clean local checkout at
`~/work/projects/repos/t3code`, pinned to
[`f313d0a6cb843551fb60e57a454a71e71a3a8c04`](https://github.com/pingdotgg/t3code/commit/f313d0a6cb843551fb60e57a454a71e71a3a8c04)
(server/desktop package version `0.1.9`). This compares those sources, not an
uninspected newer T3 release.

## Assessment

OpenAgents now has much of the initial phone experience that the
[earlier implementation proposal](07-openagents-path.md) called for: a drawer,
native composer, shared Markdown, collapsed tool output, compact conversation
refresh state, Claude chats on connected computers, model/effort controls,
Claude account login/usage, and native content Projects. Recommending that we
start those features from scratch would misdescribe the current app.

The substantial remaining gap is the coding workbench beneath that UI. T3 has
a common durable orchestration model, persistent client delivery, coordinated
reconnection, rich provider interactions, and workspace tools. OpenAgents has
a deliberately bounded first environment runtime alongside several other
conversation systems. It can control Claude, but it does not yet provide the
same continuity, recovery or workspace coverage.

The recommended direction is to extend the existing Coax modules and versioned
protocol, retaining our account/host authorization. Adopting React Native,
Electron, Effect or T3's entire wire model is unnecessary to close these gaps.

## Evidence and terminology

**Present** below means a concrete source path exists in the current app;
**partial** means the user flow or lifecycle is narrower; **library only**
means reusable code exists without a complete mobile environment integration;
**planned** means a specification is not an implemented guarantee.

The [1085 release record][release] records TestFlight availability, 558 Coax
checks, eight native/UI checks and a separate fresh-install check. Those are
previously recorded results, not checks rerun for this analysis, and they do
not certify every desktop or two-computer scenario. T3 source and tests were
read; no reference scripts, installers, tests or agents were executed. No
credential files were opened. A test's presence is evidence of intended
coverage, not a passing result from this review.

This document supplements the [connection audit](README.md). Its OpenAgents
baseline is newer than the `6fe29cc703` baseline in that audit's source map.

## Architecture and ownership

| Concern | T3 at the pinned revision | OpenAgents now | Consequence |
| --- | --- | --- | --- |
| App implementation | Expo/React Native mobile, React web, Electron desktop; shared client runtime | Coax application/state modules; UIKit and AppKit/JavaScriptCore hosts; generated C/GTK on Linux; Coax Native Web | Keep shared product decisions in Coax; platform effects still need individual qualification. |
| Execution owner | Environment server owns providers, workspaces and orchestration | `coder host serve` owns managed Claude processes; app connects through account/host discovery and the authorized HOST route | The phone already controls an agent on a computer; it need not run a CLI locally. |
| Durable state | SQLite events, projections, command receipts and effect outbox | Host receipts and saved managed JSONL history; separate server-saved account chats and project state | Durable pieces exist, but there is no equivalent common orchestration/recovery layer. |
| Connection owner | Shared per-environment supervisor, RPC session and cached projections | Bounded HTTP/HOST request state, polling and explicit receipt/status recovery | Transport recovery and projection freshness are less developed than T3's connection runtime. |
| Project identity | Workspace/repository, branch and worktree context | Content Projects built from documents, drafts, publication and membership | Native Projects are implemented; repository/worktree management is a separate gap. |

Sources: [T3 topology](01-apps-and-connections.md), [orchestration](02-orchestration.md),
[connection runtime][t3-connection], OpenAgents [runtime contract][runtime],
[native Projects contract][projects] and [desktop host][desktop].

The app currently presents several different execution and storage families:

| Conversation family | Authority and execution | Current boundary |
| --- | --- | --- |
| Managed environment Claude chat | Chosen computer, its CLI profile, generation/revision and host receipts | Owned process; send, stop and permission responses are supported. |
| Observed native CLI history | Chosen computer's bounded history discovery | Read-only. Opening it does not take over an existing terminal process. |
| Account-saved conversation, including public-project association | Account chat API and account-owned saved history; inference through the server route | Project association is versioned; it does not move execution onto a connected computer. |
| Private member-project conversation | Existing member project service and its shared agent state | Project-specific message/stop/undo/agreement semantics. |
| World editing conversation | Native world UI, account chat requests and typed world plans | Playable Metal world and editor are present; computer-owned Claude preview/artifact binding remains separate work. |

A unified drawer must retain these distinctions. A row needs an unambiguous
execution owner, control capability and recovery state; visually merging lists
must not silently change where a message runs.

## Capability gap matrix

Priorities: **P0** protects the current phone/computer workflow; **P1** completes
the coding workbench; **P2** expands product reach. These are recommendations,
not estimates or new release promises.

| Area | T3 implementation evidence | OpenAgents status and exact gap | Priority |
| --- | --- | --- | --- |
| Drawer and history | Searchable, grouped thread navigation; pending tasks, pin/archive/rename, working/settled/snoozed shelves in [mobile sidebar][t3-sidebar] | **Present/partial:** Chat, Projects and Worlds navigation, computer groups and title/computer search. No equivalent full lifecycle, attention shelves, persistent archive/pin workflow or content search. Current shared navigation has 512 rows, 96 visible and 16 in-memory draft slots. | P1 |
| Composer | Native editor integration, context items, file/image attachments, retained attachment leases and upload queue in [ComposerEditor][t3-composer] | **Present/partial:** compact/expanded native text composer; the environment app also offers model/effort controls. Plus offers New chat/Clear draft; dictation delegates to the keyboard. Project document upload is present, but is not attachment delivery into a computer-owned Claude turn. | P1 |
| Markdown and tools | Typed turn items, activity and tool presentation; plans and pending user-input cards | **Present/partial:** shared Markdown and collapsed tool rows with literal expanded output. Runtime items carry role/text/tool name/completion, not structured file changes, tool arguments/results, plans, subagents or question forms. Styling alone cannot recover missing semantics. | P1 |
| Claude control | SDK adapter, streaming, tools, permission callbacks and provider session lifecycle | **Present/partial:** stream-json Coax SDK, managed chats, stop, boolean allow/deny, model and effort. The library handles more SDK frames than the app exposes. Rich elicitation, typed questions, plan workflows and broader session operations remain gaps. | P1 |
| Codex and other agents | Registered drivers include Claude and Codex, with provider-specific capability differences in [driver registry][t3-drivers] | **Library only for mobile Codex:** Coax has an app-server client and other Coder consumers. This environment host imports Claude, not a selectable Codex runtime. No phone Codex login/usage/session path is qualified. Other T3 providers are expansion, not an immediate prerequisite. | P1 / P2 |
| Multiple accounts and usage | Provider instances, scoped Claude homes, usage readers and maintenance capability snapshots | **Present for Claude:** Settings → Usage, isolated host profiles, login and account selection; existing managed chats stay pinned. No unified provider-instance/capability catalog or Codex account surface. Usage must retain its observed/stale/error meaning. | P1 |
| Delivery after disconnect | Persistent mobile outbox; durable server receipts; [effect outbox][t3-outbox] separates acceptance from provider effects | **Partial:** host records normalized command intent before effects and retries use the same identity. App request/draft state is not an equivalent durable offline outbox surviving app termination. Unknown outcomes require recovery, not a second send. | P0 |
| Reconnection and cached reading | [Supervisor][t3-connection] separates transport, authorization and projection freshness; coordinated cursor/snapshot recovery and steady sidebar rows | **Partial:** polling/status recovery exists. No equivalent persistent coherent projection/cursor store or complete per-computer lifecycle supervisor. A fresh socket or HTTP response alone does not prove a complete history. | P0 |
| Host restart and continuation | Event/projection recovery, receipts and provider reconciliation in [V2 orchestration][t3-orchestration] | **Partial:** four managed slots; restart recovers up to eight saved histories as observe-only/unknown. Full process reconciliation and safe native continuation are explicitly not promised. | P0 |
| Repository/worktree context | Project/worktree lifecycle, branch state and worktree setup; checkpoint capture/diff/rollback services | **Missing from this app workflow:** one configured workspace per host profile, with CLI-level setup. Content Projects do not create worktrees or establish checkout ownership. | P1 |
| Files, diffs and terminal | Mobile filesystem queries, native diff review module and thread terminal route | **Partial building blocks elsewhere:** native project file picker, agent tools and realm editor exist. No comparable authorized repository browser, live checkout diff review or interactive environment PTY in the mobile app. Observed terminal history is not a PTY. | P1 |
| Rewind and safe undo | Checkpoint rollback checks isolation and overlapping thread workspaces in [restore safety][t3-restore] | **Not equivalent:** project Undo and world revision/undo rules are domain-specific. There is no general managed-Claude conversation rewind plus isolated checkout restore. | P1 |
| Desktop and enrollment | Desktop backend lifecycle and mobile direct/remote connection management | **Partial:** native Mac/Linux client sources and owner-computer setup exist. Shared screens do not establish effect parity. Document effects are missing/mismatched in the desktop hosts; self-service enrollment/workspace setup and Windows qualification remain unfinished. | P0 / P2 |
| Operations and compatibility | Provider probes, version/maintenance advice, diagnostics and replay recovery fixtures | **Partial:** protocol checks, policy mutation gates, release records and bounded errors exist. No equivalent end-user provider diagnostic/capability/upgrade experience across all connected computers. | P1 |

OpenAgents evidence for the matrix: [navigation][navigation],
[composer][composer], [environment app][environment-app], [runtime schema][schema],
[host service][host], [Claude library parity][claude], [Codex library parity][codex]
and [provider accounts contract][accounts].

## The gaps that matter most

### 1. Reliable ownership and recovery before more surface polish

Keep the existing command identity and generation/revision guards. Extend them
into a durable client pending-command record, a coherent persisted read model,
and an explicit per-computer connection state machine. Account authorization,
host reachability, provider readiness and timeline freshness are distinct states.

T3's receipt/outbox design is useful because it separates accepted intent from
execution. It does not justify claiming exactly-once arbitrary external effects.
Likewise, a saved OpenAgents receipt is not proof that a provider turn finished.
On restart, reconcile the native session/process before permitting continuation;
retain unknown/observe-only where evidence is insufficient.

The current 6,000-byte timeline pages, 32 MiB history/discovery limits, 128-session
discovery cap and bounded recovery rows are explicit safeguards in the
[runtime contract][runtime]. Preserve completeness flags and paging rather than
calling a truncated or inaccessible list empty. Raise a bound only with a
measured reason and revised qualification.

### 2. Typed transcript and interaction data

The current app wire is narrower than both the provider libraries and the
desired UI. Add versioned structured items for tool lifecycle, file changes,
plans, requests for input, subagent references and attachments as they are
implemented. Stable identities must survive streaming, pagination and retries.
Do not parse a Bash output string to reconstruct an authoritative diff or
approval. Unknown item kinds need an inert fallback.

Provider version coverage also differs. The [Coax Claude parity record][claude]
tracks SDK `0.3.296`/CLI `2.1.312`, while the inspected T3 lockfile resolves
SDK `0.3.321`; our [Codex parity record][codex] tracks CLI `0.163.0` and lists
unsupported steering, questions, elicitation and subagent operations. These
are source/qualification pins, not the versions necessarily installed on each
computer. Check actual host capabilities and fixtures before widening the app
contract; matching a dependency number alone would not establish parity.

Extend `components/` and the shared native adapters; preserve the one Markdown
parser and edit buffer. Compact tool rows are already implemented. The next
step is conveying useful summaries and typed details, with stable expansion
and scroll position while new events arrive. T3's [pending question card][t3-question]
and [plan card][t3-plan] illustrate interactions that boolean allow/deny cannot express.

### 3. A workspace model alongside content Projects

Define separate references for a content Project, an environment, an admitted
workspace/repository and a checkout/worktree. Relate them explicitly when needed.
Do not reinterpret a published project identifier as a filesystem path.

Then add typed capabilities for file listing/reading, bounded diff inspection,
attachments and terminal sessions. Each needs path containment, lifecycle,
cancellation, limits and authorization. A generic remotely supplied shell
command is not a replacement for that contract. For rewind, follow the safety
principle visible in T3: restoring a checkout shared with another chat can
destroy unrelated edits, so isolation must be checked again at restore time.

### 4. Platform parity has a concrete host-contract problem

The [desktop program][desktop-program] loads the same iOS Coax application.
However, iOS [NativeHost][native-host] maps operation 14 to `documents.pick`,
15 to upload and 16 to release. Mac [EnvironmentWindow][mac-host] and Linux
[services][linux-host] still treat operation 14 as local environment RPC and
provide no matching 15/16 document effects. World-open support is also absent
from those service dispatchers. Sharing the application tree therefore cannot
be reported as working Projects/worlds on desktop.

Before desktop feature claims, make effect capabilities explicit and reconcile
these mappings. Add host-contract checks so a newly shared screen cannot call
an effect with an older meaning. This is an observed source mismatch, not a
desktop failure reproduced during this review.

## Recommended implementation sequence and acceptance

| Order | Change | Acceptance evidence |
| --- | --- | --- |
| 1 | Qualify the current phone → Mac/CoderOS Claude path; reconcile desktop effect contracts | Both computers remain separately identifiable. Test revoked grants, host unavailable, CLI signed out and incomplete discovery. Project picking invokes a picker on supported hosts and an explicit unavailable capability elsewhere. |
| 2 | Persistent app delivery/read state and host restart reconciliation | Lose the response after host acceptance, kill/reopen the app, and reconnect: query the same command; no duplicate turn. Restart the host during a tool/permission wait: recover ownership safely or show unknown, never silently resume another process. Account switch cannot expose the prior account's cached transcript or pending send. |
| 3 | Versioned rich items and interactions using shared components | A streaming Read/Edit/Bash run stays compact; expanded raw text is inert; paging preserves identity/scroll. A multi-question request can be answered once; stale answers fail. Plan and diff rendering have typed sources. |
| 4 | Codex as one complete additional environment provider | Discover, authenticate, choose account/model/effort, start/resume, stream tools, approve/deny, interrupt and recover on both computers from the phone. Publish actual capabilities; unsupported elicitation/steering is explicit. |
| 5 | Workspace/checkout model, attachments, file/diff/terminal views and safe restore | Phone can choose an admitted checkout, attach a file, inspect the resulting diff and open a terminal. Escape/symlink attempts fail. Restoring a shared checkout is refused without losing conversation history. |
| 6 | Bind computer-owned Claude edits to world authoring | Splice Bots/current Everglade edit chat names its computer and workspace; a fenced artifact/revision becomes a preview; stale edits fail; publication/undo follows world authority rather than transcript text. |
| 7 | Broaden navigation, providers and platform qualification | Persist pin/archive/search state; measure large histories, keyboard/accessibility and foreground/background behavior. Qualify Mac/Linux separately; Windows and additional agents remain explicit later milestones. |

Wire work starts in `proto/` and the protocol specification, with a new version
for a changed published contract, registry entry and one codec. Product state
and admission rules belong in shared Coax; policy units use the repository's
authority and mutation gates. Swift/C remain adapters for installed effects.
Compatible logic/presentation can ship through signed Updates; a new native
effect, entitlement or renderer requires a binary release. T3 feature presence
does not remove those Coax Native compatibility constraints.

## Deliberate differences to retain

- OpenAgents account discovery and device/grant/epoch authorization are not
  T3 direct-server pairing. Preserve narrower host authority and credential
  locality while improving connection UX.
- Observed terminal chats remain read-only until native continuation is
  explicitly qualified. Do not make a row appear controllable by creating a
  different managed chat without telling the user.
- Claude account profiles remain isolated; changing the default affects new
  chats and does not rewrite a live chat's account. Usage observations are
  not a quota reservation or proof of future inference availability.
- Content Projects, Bitcoin services and streamed playable worlds are
  OpenAgents product areas, not missing T3 replicas. A world renderer being
  installed does not prove the computer-owned editing/preview protocol works.
- Repository checks and zero-survivor policy tests are valuable evidence for
  their units; they do not establish end-to-end T3 parity or phone performance.

## Source map

For detailed T3 provider paths and boundaries, use the existing
[pinned source map](08-source-map.md). The additional source links here are
pinned to the same revision. The [Zeron comparison](../zeron/2026-11-14-current-gap-analysis.md)
examines local-first sync, queues and native rendering in more depth.

[release]: ../../verification/2026-11-14-native-projects-1085.json
[runtime]: ../../protocol/environment-runtime-v1alpha1.md
[projects]: ../../protocol/native-projects.md
[accounts]: ../../protocol/provider-accounts-v1.md
[desktop]: ../../../apps/openagents-desktop/README.md
[desktop-program]: ../../../apps/openagents-desktop/native/program.coax
[mac-host]: ../../../apps/openagents-desktop/native/EnvironmentWindow.swift
[linux-host]: ../../../apps/openagents-desktop/native/linux-services.coax
[native-host]: ../../../apps/shared/native/NativeHost.swift
[navigation]: ../../../apps/shared/app-navigation.coax
[composer]: ../../../apps/shared/native/NativeComposer.swift
[environment-app]: ../../../apps/shared/environment-app.coax
[schema]: ../../../proto/openagents/environment/access/v1alpha1/runtime.proto
[host]: ../../../coder/tui/host-environment.coax
[claude]: ../../../coax/lib/agent/claude/PARITY.md
[codex]: ../../../coax/lib/agent/codex/PARITY.md
[t3-connection]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/docs/internals/connection-runtime.md
[t3-sidebar]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/features/threads/ThreadNavigationSidebar.tsx
[t3-composer]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/components/ComposerEditor.tsx
[t3-drivers]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/provider/builtInDrivers.ts
[t3-outbox]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/EffectOutbox.ts
[t3-orchestration]: https://github.com/pingdotgg/t3code/tree/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2
[t3-restore]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/server/src/orchestration-v2/CheckpointRestoreSafety.ts
[t3-question]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/features/threads/PendingUserInputCard.tsx
[t3-plan]: https://github.com/pingdotgg/t3code/blob/f313d0a6cb843551fb60e57a454a71e71a3a8c04/apps/mobile/src/features/threads/ProposedPlanCard.tsx
