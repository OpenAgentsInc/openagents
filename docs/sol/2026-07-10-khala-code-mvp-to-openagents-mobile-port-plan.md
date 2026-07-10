# Khala Code MVP → OpenAgents mobile Effect Native port plan

- Date: 2026-07-10
- Status: active P0 capability ledger under Master Roadmap Revision 25
- Primary issues: #8597, #8547, #8636, #8566, #8638
- Destination: `apps/openagents-mobile`
- Source app: `clients/khala-mobile` (deprecated/frozen extraction source)
- Runtime authority: Khala Sync, Pylon/Fleet, OpenAgents sandbox/workrooms, and
  Agent Computers

## Executive decision

OpenAgents mobile must absorb the useful Khala Code MVP capability set,
including useful coding through remote containers/workrooms. Mobile is no
longer scoped to supervision plus Desktop handoff. It is a compact, phone-native
coding and fleet client that can complete a repository-bound agent task without
Desktop while preserving the same thread, workroom, run, authority, and receipt
truth when the user does hand off.

“Port” means move behavior, typed contracts, test vectors, and proven native/
release knowledge into Effect Native and shared services. It does **not** mean
importing the legacy React Native/NativeWind component tree, preserving its app-
local authority, or continuing to ship `clients/khala-mobile`.

## Non-negotiable authority boundary

```text
OpenAgents mobile intent
        |
        v
Khala Sync / policy / workroom API
        |
        v
owner-scoped remote workroom or Agent Computer
        |
        v
typed progress + exact post-image + verification + receipt
```

The phone may render and control remote files, diffs, terminal sessions,
previews, artifacts, branches, and pull requests. It never receives raw local
device filesystem/process authority, durable provider credentials, unbounded
network/port access, or permission to bypass brokered Git writeback. A local
cache or timed-out request is never completion truth.

The remote contract must cover:

- create, resume, stop, destroy/reclaim, TTL, and snapshot identity;
- stable owner/repository/thread/workroom/run refs;
- isolated workspace and provider-account homes;
- brokered GitHub/provider grants with bounded redemption and replay defense;
- file list/read/write and exact pre/post-image identity;
- bounded run/spawn/PTY with reconnect and teardown;
- managed preview ports and explicit network policy;
- progress, approvals, verification, artifacts, usage, and terminal outcomes;
- safe branch/PR writeback with no force push and explicit failure recovery.

The existing feasibility and isolation contracts are the starting evidence:

- [`../khala-code/2026-07-06-agent-computers-strategy.md`](../khala-code/2026-07-06-agent-computers-strategy.md)
- [`../khala-code/2026-07-06-agent-computer-isolation-posture.md`](../khala-code/2026-07-06-agent-computer-isolation-posture.md)
- [`../khala-code/2026-07-04-ai-sdk-harness-fork-sandbox-feasibility-audit.md`](../khala-code/2026-07-04-ai-sdk-harness-fork-sandbox-feasibility-audit.md)
- `packages/ai-sdk-sandbox-openagents`

## Exhaustive capability disposition

Every row is part of the migration ledger. A coding agent may split a row into
smaller issue leaves, but may not silently drop it.

| Khala Code MVP idea | Evidence/source | Effect Native destination | Disposition and acceptance |
| --- | --- | --- | --- |
| GitHub/OpenAuth PKCE sign-in | Legacy mobile auth/session flow | Shared owner/org/device session service and mobile sign-in | **Port.** Restore/revoke safely; secrets remain in approved secure storage/host services. |
| Secure session persistence and recovery | Legacy SecureStore/keychain behavior | Effect Native session state plus RN secure-storage host capability | **Port.** Cold start, expiry, revocation, partial storage failure, and sign-out are tested. |
| Device pairing and delegation safety | `khala-mobile-pairing*`, `security/delegation-prompt.ts` | Shared device registration/pairing and bounded delegation confirmation | **Port.** Pairing is owner-scoped, expires/revokes, and cannot widen repository/account/workroom grants. |
| First-run onboarding and straight-line first task | `onboarding-*`, `qa/straight-line-e2e.ts` | Effect Native onboarding from sign-in → repository → thread → workroom → receipt | **Port and rewrite.** The shortest real path creates useful work; no demo fixture is presented as completion. |
| Local-first thread catalog and cursor recovery | `khala-sync-mobile.ts`, SQLite persistence | Shared Khala Sync conversation projection/cache | **Replace with stronger contract.** Offline cache/cursor remains; server projection and tombstones are authoritative. |
| Create/open/rename/archive thread | Legacy thread flows | Shared typed conversation intents | **Port.** Desktop and mobile observe identical refs, versions, and outcomes. |
| Repository list, search, and picker | `khala-mobile-repos-api.ts`, `khala-mobile-repo-search-core.ts` | Effect Native repository picker and repository authority service | **Port.** Search is accessible, paged, cancellable, and honest about loading/empty/error/offline state. |
| Thread ↔ repository binding | `khala-thread-repo-binding-core.ts` | Canonical repository/thread/workroom binding | **Port.** A turn cannot target an ambiguous repository or stale binding. |
| Rich runtime transcript | `khala-runtime-transcript-core.ts` | Shared streamed turn/event model and mobile timeline | **Port.** Reasoning, text, tool, usage, status, file-change, verification, and writeback events survive reconnect without transcript inference. |
| Composer and context | `chat-composer.tsx`, `khala-runtime-compose-core.ts` | Effect Native composer, attachment/context chips, typed submit | **Port behavior, redesign UI.** Queue/steer/interrupt/retry/follow-up states are explicit and accessible. |
| Copy, copy Markdown, quote, swipe actions | Legacy transcript action cores/components | Effect Native mobile message actions | **Port where useful.** Actions never mutate authority-bearing runtime state implicitly. |
| Codex/Claude connected accounts, readiness, quota | Account API/core modules | Shared account/capacity projection and settings | **Port.** Named account refs only; unavailable/quota/reauth states are visible. |
| Model and target preference | Model preference modules | Shared model/runtime/execution-target selector | **Port.** Provider and `owner_local | managed_remote | auto` selection/fallback remain typed and visible. |
| Cross-agent handoff | `khala-cross-agent-handoff-core.ts` | Typed worker/agent handoff intent | **Port.** Preserve repository/thread/workroom continuity and record the handoff outcome. |
| Fleet peek and attention state | `khala-fleet-collections-core.ts`, `fleet-peek-core.ts` | Mobile activity/fleet home and attention queue | **Port and deepen.** Use canonical Fleet projections, never a private mobile model. |
| Steer/approve/pause/resume/stop | Legacy runtime controls plus Fleet intents | Shared action catalog and mobile controls | **Port.** Durable command IDs/outcomes, lost-ack reconciliation, confirmation where destructive. |
| Push registration and deep links | `khala-sync-push-core.ts`, `use-khala-sync-push.ts` | Shared push projection plus RN notification/deep-link host | **Port.** Notification opens the exact owner-scoped thread/workroom/run/approval and handles revoked/stale targets safely. |
| Connectivity and recoverable problem model | `status/khala-code-connectivity*`, `network/mobile-problem.ts` | Shared Sync/workroom connectivity phases and actionable errors | **Port.** Distinguish offline, reconnecting, stale, refetch, denied, workroom-expired, provider unavailable, and failed. |
| Connectivity, crash diagnostics, debug beacon | Legacy diagnostics/devtools | Public-safe Effect Native diagnostics and release telemetry | **Port.** No credentials, raw prompts, repository content, or private paths in evidence. |
| Owned OTA/update behavior | Mobile OTA runbook and new app updater | `apps/openagents-mobile` owned OTA/release contract | **Port the discipline.** No EAS and no silent legacy feed/build identity reuse. |
| Credits balance/history | Legacy credits API/core | Bounded account/economics projection | **Defer unless needed for workroom admission.** If shown, distinguish model subscription, compute, and payment truth. |
| Demo Minerals/IAP pricing | Demo and StoreKit planning | None in current P0 | **Paused.** Do not port presentation/demo economics into the active app. |
| Native push-to-talk/STT | `modules/khala-push-to-talk-stt` | Future bounded host capability | **Paused.** Native speech proof was not an accepted reliable MVP dependency. |
| Apple Foundation Models/local model routing | Legacy experiments | Future typed runtime target | **Paused.** Must re-enter through the same explicit target/evidence contract. |
| Sarah/persona/avatar/video | Legacy/compatibility routes | Compatibility adapter only | **Paused.** Not default navigation, product authority, or acceptance scope. |
| Architecture cruiser, unit/mount tests | Legacy QA scripts/tests | New app and shared package gates | **Port test intent.** Rewrite against Effect Native; never import the old app package. |
| Storybook, Maestro, visual baselines | Khala QA harness and receipts | Effect Native stories, physical-device journeys, visual/accessibility gates | **Port useful scenarios.** Rebaseline the new product; old pixels are evidence, not a design mandate. |
| iOS/Android local build and nightly gates | Mobile testing/OTA runbooks | Owned OpenAgents mobile release lane | **Port and strengthen.** Both platforms install, recover, update, and pass physical-device flows. |

## Required remote coding surfaces

The phone UI uses progressive, compact navigation rather than scaling Desktop
columns. The minimum repository-bound workspace exposes:

1. **Thread** — streamed agent work, composer, context, plan, controls, and
   approvals.
2. **Files** — searchable tree, bounded read/edit/save, selected ranges, dirty/
   conflict state, and exact file identity.
3. **Changes** — typed Git status and diff, comments/approval, verification,
   revert where policy allows, and safe branch/PR writeback.
4. **Terminal** — explicit remote workroom session, bounded command/process
   state, reconnect/teardown, and clear exit/unknown state.
5. **Preview** — host-managed port discovery, authenticated preview gateway,
   loading/error/expired state, and no arbitrary public port exposure.
6. **Artifacts/receipts** — outputs, tests, usage truth, writeback target,
   provenance, and terminal closeout.

Activity, repositories, threads, and fleet attention are primary navigation.
Workspace modes are contextual navigation inside a selected thread/workroom.
Controls must remain reachable and accessible on small screens; deep work may
handoff to Desktop, but handoff is an option rather than the only route.

## Ordered implementation waves

| Wave | Scope | Exit before advancing |
| --- | --- | --- |
| M0 — freeze and inventory | Lock destination/source boundary; capability manifest; identity/icon/release locks; migrate useful legacy test cases into this ledger | Every legacy MVP idea has a disposition and source/destination owner |
| M1 — identity, Sync, repositories | Auth/session, secure recovery, device registration, thread catalog, repository picker/binding, offline/cursor states | Same authenticated repo-bound thread appears on Desktop/mobile after restart/reconnect |
| M2 — authoritative turns | Rich transcript, composer/context, queue/steer/interrupt/retry, account/model readiness, notifications | One real turn and its exact outcomes survive background/reconnect without duplicate submit |
| M3 — workroom lifecycle | #8547/#8636 target policy, create/resume/stop/reclaim, grants, isolation, snapshots/TTL, progress projection | Phone starts and resumes one real owner-scoped remote workroom with honest isolation rung |
| M4 — files, changes, writeback | Tree/read/edit, exact diff, artifacts, verification, branch/PR writeback | Useful repository change completes with safe refs, exact post-image, verification, and receipt |
| M5 — terminal and preview | Bounded PTY/run/spawn, managed ports, preview gateway, reconnect/teardown | Command and preview work on physical iOS/Android; expiration/failure is explicit and reclaim is proven |
| M6 — fleet and release hardening | Fleet/attention/approvals, push/deep links, diagnostics, accessibility, architecture/tests/stories/Maestro/visual gates | Mobile manages a mixed run, survives fault matrix, and passes owned iOS/Android release gates |
| M7 — cross-device dogfood | Mobile-originated task, Desktop continuation, offline/update/restart faults, legacy retirement | One signed owner-accepted receipt; no state fork, unsafe grant, false success, or shipping legacy path |

M1/M2 may run alongside Desktop R1/R2 work. M3's shared grants, lifecycle,
workroom schema, and target-routing contracts require senior review and
serialized claims. M4/M5 phone surfaces can use fixtures after their typed
contract freezes, but R6/R7 remain blocked until they exercise a real workroom.

## Issue ownership and triage

- **#8597 APP-MOBILE** owns mobile information architecture, Effect Native
  porting, repository/thread UX, remote coding surfaces, push, physical-device
  QA, and retirement of the old shipping path.
- **#8547 FC-CLOUD-1** owns the first real brokered Codex Agent Computer/
  workroom turn, isolation/grants, lifecycle, reclaim, and compute receipt.
- **#8636 FC-4** owns typed owner-local/managed-remote target policy, visible
  fallback, and one claim registry across targets.
- **#8566 APP-1** owns the shared Effect Native/Khala Sync cross-client exit.
- **#8638/#8640** own Fleet substrate and the real mixed-account proof consumed
  by both clients; they do not own mobile IA.

#8547 and #8636 are P0 because they are prerequisites for the mobile coding
MVP. Advanced elastic placement/provider breadth remains follow-on. Live issue
bodies, labels, and claims must be reconciled to this split before dispatch.

## Acceptance receipt

The fold-in is complete only when a physical phone can:

1. sign in, choose an authorized repository, and create/resume a repo-bound
   thread;
2. select an explicit account/model/remote target and start an isolated real
   workroom;
3. stream the authoritative agent turn, inspect the plan and tool/file events,
   steer or approve, and survive background/reconnect;
4. inspect/edit files, review the exact diff, run a bounded command, open a
   managed preview, and inspect artifacts/verification;
5. write back through a safe branch/PR path with no force push and one durable
   receipt;
6. supervise the associated FleetRun and receive a push/deep link to the exact
   attention item;
7. continue the same thread/workroom/run on Desktop without duplicate work or
   forked identity/state; and
8. survive restart, offline/lost acknowledgement, update, token revocation,
   workroom expiration, and reclaim with explicit converged or failed-closed
   state.

Unit tests or a fixture-only container do not close this acceptance. Report the
code-landed, fixture-proven, deployed, live-proven, owner-accepted, and closed
rungs separately.
