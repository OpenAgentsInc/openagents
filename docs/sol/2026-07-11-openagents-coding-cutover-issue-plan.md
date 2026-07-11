# OpenAgents coding cutover issue plan

- Status: active issue graph
- Date: 2026-07-11
- Program parent: [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566)
- Desktop track: [#8574](https://github.com/OpenAgentsInc/openagents/issues/8574)
- Mobile track: [#8597](https://github.com/OpenAgentsInc/openagents/issues/8597)
- Ordering authority: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)

## Outcome and boundary

This graph is the bounded path for moving ordinary day-to-day Codex and Claude
Code work into the OpenAgents Desktop app while preserving authenticated mobile
continuation and supervision. Application-owned UI, state, actions, and
lifecycle stay in Effect Native; Electron, React Native, and platform APIs are
host/lowering boundaries rather than alternate application architectures.

The graph deliberately does **not** make remote workrooms, cloud host moves,
Daytona, elastic placement, or voice prerequisites for the local coding
cutover. Those remain later Revision 31 outcomes under #8547, #8636, and the
remote-first pathway. Finishing this graph can close the ordinary local-coding
scope of #8574 and its bounded reliability parents; it does not manufacture
proof for those later remote outcomes or automatically close #8566/#8597.

## Dispatch rules

- Work in numeric order when an issue names a dependency.
- Issues in the same parallel lane may run concurrently only when their claimed
  paths do not overlap.
- A checked box means the linked GitHub issue is closed with its stated receipt,
  not merely that code exists.
- Every leaf closes with tests, public-safe evidence, updated source docs, an
  issue summary, and a commit pushed to `main`.
- Parent issues close only after every child and parent-only live receipt is
  accepted. A deterministic harness never substitutes for a required physical
  device, built host, named account, or provider run.

## Ordered issue graph

| Order | Key | GitHub issue | Lane | Depends on |
|---:|---|---|---|---|
| 1 | CUT-01 | [#8681](https://github.com/OpenAgentsInc/openagents/issues/8681) — closed `bab737f565` | R0 / mobile architecture | — |
| 2 | CUT-02 | [#8682](https://github.com/OpenAgentsInc/openagents/issues/8682) — closed `fa4d6489d3` | R0 / verification | — |
| 3 | CUT-03 | [#8683](https://github.com/OpenAgentsInc/openagents/issues/8683) — closed `4d875dcb4b` | D2 / topology | CUT-02 |
| 4 | CUT-04 | [#8684](https://github.com/OpenAgentsInc/openagents/issues/8684) — closed `6ee87714d0` | D2 / topology | CUT-03 |
| 5 | CUT-05 | [#8685](https://github.com/OpenAgentsInc/openagents/issues/8685) — closed `509fb27ea1` | FC5 / Claude runtime | CUT-02 |
| 6 | CUT-06 | [#8686](https://github.com/OpenAgentsInc/openagents/issues/8686) — closed; accepted live receipt | FC5 / runtime authority | CUT-05 |
| 7 | CUT-07 | [#8687](https://github.com/OpenAgentsInc/openagents/issues/8687) — closed; deterministic matrix receipt | R4 / command convergence | CUT-02 |
| 8 | CUT-08 | [#8688](https://github.com/OpenAgentsInc/openagents/issues/8688) — closed; deterministic matrix receipt | R4 / event convergence | CUT-07 |
| 9 | CUT-09 | [#8689](https://github.com/OpenAgentsInc/openagents/issues/8689) — deterministic matrix landed; physical receipt owner-deferred, still required | R4 / lifecycle convergence | CUT-08 |
| 10 | CUT-10 | [#8690](https://github.com/OpenAgentsInc/openagents/issues/8690) — Desktop/mobile no-poll subscription path landed; physical receipt pending | D1 / Runtime Gateway | CUT-09 deterministic matrix; physical receipt remains open by owner exception |
| 11 | CUT-11 | [#8691](https://github.com/OpenAgentsInc/openagents/issues/8691) — schema/Sync/root + real Claude child binding + Gateway v8 reconnect implemented; named Claude/Codex source traces captured; Codex transport convergence + named confirmed reconnect pending | D1 / agent graph | CUT-06, CUT-10 deterministic no-poll path |
| 12 | CUT-12 | [#8692](https://github.com/OpenAgentsInc/openagents/issues/8692) — shared presentation model + mobile and Desktop thread agent stacks landed; physical iOS/Android receipts pending | D1 / agent UX | CUT-01, CUT-11 graph contract/delivery |
| 13 | CUT-13 | [#8693](https://github.com/OpenAgentsInc/openagents/issues/8693) — canonical shared catalog + bounded restart resolver + owner-scoped server projector landed; client/Desktop persistence and built-host receipt pending | D2 / project-session contract | CUT-04, CUT-10 |
| 14 | CUT-14 | [#8694](https://github.com/OpenAgentsInc/openagents/issues/8694) | M1 / mobile binding | CUT-01, CUT-13 |
| 15 | CUT-15 | [#8695](https://github.com/OpenAgentsInc/openagents/issues/8695) | D2 / commands | CUT-13 |
| 16 | CUT-16 | [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696) | D1-D2 / interaction | CUT-12, CUT-15 |
| 17 | CUT-17 | [#8697](https://github.com/OpenAgentsInc/openagents/issues/8697) | D3 / workspace | CUT-13 |
| 18 | CUT-18 | [#8698](https://github.com/OpenAgentsInc/openagents/issues/8698) | D3 / editor | CUT-17 |
| 19 | CUT-19 | [#8699](https://github.com/OpenAgentsInc/openagents/issues/8699) | D3 / Git | CUT-18 |
| 20 | CUT-20 | [#8700](https://github.com/OpenAgentsInc/openagents/issues/8700) | D3 / terminal-preview | CUT-17 |
| 21 | CUT-21 | [#8701](https://github.com/OpenAgentsInc/openagents/issues/8701) | D4 / providers | CUT-06, CUT-13 |
| 22 | CUT-22 | [#8702](https://github.com/OpenAgentsInc/openagents/issues/8702) | D4 / continuity | CUT-21 |
| 23 | CUT-23 | [#8703](https://github.com/OpenAgentsInc/openagents/issues/8703) | D4 / extensions-policy | CUT-16, CUT-21 |
| 24 | CUT-24 | [#8704](https://github.com/OpenAgentsInc/openagents/issues/8704) | D4-D5 / operability | CUT-12, CUT-23 |
| 25 | CUT-25 | [#8705](https://github.com/OpenAgentsInc/openagents/issues/8705) | D5 / Fleet | CUT-12, CUT-14, CUT-24 |
| 26 | CUT-26 | [#8706](https://github.com/OpenAgentsInc/openagents/issues/8706) | D6 / distribution | CUT-24 |
| 27 | CUT-27 | [#8707](https://github.com/OpenAgentsInc/openagents/issues/8707) | R7 / cutover proof | CUT-19-CUT-26 |

Parallel-safe waves after the R0 baseline are: CUT-03 with CUT-05 and CUT-07;
CUT-12 with CUT-13 after their prerequisites; CUT-17 with CUT-21; and CUT-19
with CUT-20 after the shared workspace/editor contracts settle.

---

<!-- issue:CUT-01 -->
## CUT-01 — Enforce Effect Native surface authority and remove the mobile composer island

Parents: #8566, #8597  
Depends on: none  
Parallel-safe lane: R0 mobile architecture; do not overlap mobile composer or
Effect Native renderer/lowering paths.

### Outcome

All shipped Desktop/mobile application UI, state, actions, and lifecycle are
owned by Effect Native contracts. Remove the app-local
`openagents-liquid-glass` SwiftUI composer module and direct application use of
`@expo/ui` after the equivalent owned renderer/lowering exists upstream.

### Completion criteria

- The demand register and import-boundary oracle describe the actual tree and
  fail on app-local React/SwiftUI/application-state islands.
- The mobile composer retains the current text, new-chat, send/clear,
  accessibility, keyboard, and deterministic behavior through Effect Native
  primitives. Rich attachments and runtime cancel controls remain CUT-16
  #8696 rather than being smuggled into this architecture leaf.
- Native/Electron host code is limited to documented renderer, capability, and
  lifecycle lowering boundaries.
- Desktop and mobile typecheck, focused tests, and boundary scans pass.

Verification: include negative fixtures proving forbidden imports/modules fail,
an iOS native prebuild/build, and iOS-26 simulator pixel evidence. Physical
iOS/Android composer acceptance remains the final installed-product rung in
CUT-27 #8707 so this architecture leaf stays agent-completable.
Non-goals: eliminating native host code or waiting for remote workrooms.  
Close rule: merge to `main`, update the demand register, attach receipts, and
close with the commit hash.

---

<!-- issue:CUT-02 -->
## CUT-02 — Restore a truthful green Desktop verification baseline

Parents: #8566, #8574  
Depends on: none  
Parallel-safe lane: R0 tests; scope to verification fixtures and the production
contract they expose.

### Outcome

`apps/openagents-desktop` has one reliable package verification command with no
known ignored failures.

### Completion criteria

- Fix the two `workspace-service.test.ts` Git status and secret-shaped diff
  failures without weakening secret redaction or filesystem policy.
- Package typecheck and the complete Desktop test sweep pass from a clean tree.
- The canonical verification command is documented and CI invokes the same
  contract.
- A negative secret-shaped diff fixture remains rejected/redacted.

Verification: clean-tree command transcript and CI receipt.  
Non-goals: unrelated feature work.  
Close rule: merge the bounded fix and close with command results and commit.

---

<!-- issue:CUT-03 -->
## CUT-03 — Make the Desktop topology oracle source-coupled and deny ambient authority

Parents: #8566, #8574, #8678  
Depends on: CUT-02  
Parallel-safe lane: D2 architecture; topology manifest/oracle only.

### Outcome

The topology check derives evidence from production service construction and
rejects forbidden scope, authority, cache, network, filesystem, process, and
secret edges instead of accepting a hand-maintained mirror.

### Completion criteria

- Every Desktop service/layer is mapped to an owning scope from real source
  construction or a mechanically checked declaration adjacent to it.
- Negative fixtures fail for renderer ambient filesystem/process/network/secret
  access and for session/project services installed at wider scopes.
- Cache and freshness declarations are tied to the implementing service.
- The full topology oracle runs in the Desktop verification command.

Verification: mutation/negative-fixture receipt plus clean full sweep.  
Non-goals: UI redesign.  
Close rule: child closes only after source coupling is demonstrated; #8678
remains open for CUT-04.

---

<!-- issue:CUT-04 -->
## CUT-04 — Prove Desktop service replaceability, lifecycle disposal, and correlation

Parents: #8566, #8574, #8678  
Depends on: CUT-03  
Parallel-safe lane: D2 architecture acceptance; service lifecycle and host
proof paths.

### Outcome

Desktop services can be substituted in tests, dispose exactly once at their
owning scope, and preserve structured correlation through the built host.

### Completion criteria

- Representative runtime, workspace, sync, and account services have
  substitute layers used by production-shaped tests.
- session/project/window/app teardown proves exactly-once interruption and
  disposal with no surviving watchers, PTYs, streams, or secret handles.
- Structured operation/session/run/correlation refs traverse renderer, IPC,
  main process, Runtime Gateway, Sync, and logs without secret leakage.
- A built Electron smoke executes the topology, substitution, disposal, and
  correlation acceptance path.

Verification: lifecycle leak test and public-safe built-host receipt.  
Non-goals: packaging/notarization, owned by CUT-26.  
Close rule: close this leaf and then #8678 only when all residual acceptance is
green.

---

<!-- issue:CUT-05 -->
## CUT-05 — Repair Claude owner-local permission mode without widening public authority

Parents: #8566, #8640  
Depends on: CUT-02  
Parallel-safe lane: FC5 Claude runtime; local executor policy only.

### Outcome

An explicitly owner-authorized local Claude account can complete ordinary
coding tasks, while remote/public callers cannot inherit bypass permissions.

### Completion criteria

- Owner-local authorization is typed, explicit, revocable, and scoped to one
  local target/run rather than inferred from caller strings.
- Claude launches receive the intended permission mode only through that
  authority; public and remote paths retain the restrictive default.
- Positive owner-local and negative public/remote tests cover launch, restart,
  cancellation, and audit receipt behavior.
- No secret or permission mode is serialized into public projections.

Verification: deterministic policy tests and one named-account local Claude
run.  
Non-goals: remote Claude hosts.  
Close rule: merge and attach a redacted run receipt; #8640 stays open for
CUT-06 and parent acceptance.

Closed on 2026-07-11 at `509fb27ea1`. The process-opaque scoped authority,
negative public/bridge/org-cloud/replay coverage, cancellation path, refs-only
audit projection, real `claude-pylon-3` accepted run, and full deploy gate are
recorded in
[`CUT-05 Claude owner-local permission receipt`](../pylon/2026-07-11-cut-05-claude-owner-local-permission-receipt.md).

---

<!-- issue:CUT-06 -->
## CUT-06 — Close supervisor-scope leaks and verifier/publication ordering races

Parents: #8566, #8640  
Depends on: CUT-05  
Parallel-safe lane: FC5 runtime authority; supervisor and receipt publication.

### Outcome

Codex and Claude supervisors do not outlive their run scope, and acceptance is
published only after the verifier has committed the matching terminal result.

### Completion criteria

- Each supervisor/fiber/process is acquired and interrupted in the owning run
  scope across success, failure, cancellation, crash, and restart.
- Publication cannot expose accepted/terminal state before verifier outcome and
  correlation refs are durable.
- Race tests cover simultaneous Codex+Claude runs, delayed verification,
  cancellation, restart, and stale completion.
- The accepted #8640 parent receipt uses named accounts and the production
  supervisor path.

Verification: race/leak suite plus one public-safe simultaneous runtime receipt.  
Non-goals: Fleet UI.  
Close rule: close #8640 only after this leaf and its live parent receipt pass.

Implementation landed on 2026-07-11 at `d98abda795`. Scope-owned cancellation,
loop join before supervisor-guard release, concurrent-restart fencing, delayed
verification, rejected-verifier restart, stale/late lifecycle, and Codex/Claude
cancellation oracles are green. The full Pylon and deploy sweeps are green. The
deterministic evidence is recorded in the
[`CUT-06 Fleet supervisor ordering receipt`](../pylon/2026-07-11-cut-06-fleet-supervisor-ordering-receipt.md).
The required simultaneous named-account production receipt is accepted under
run `fleet_run.sarah.666432631ce5e88a47a5`; CUT-06 and #8640 are closed.

---

<!-- issue:CUT-07 -->
## CUT-07 — Converge conversation commands across lost ACKs, duplicates, and offline expiry

Parents: #8566, #8677  
Depends on: CUT-02  
Parallel-safe lane: R4 command convergence; command ledger and Sync adapter.

### Outcome

Desktop/mobile conversation commands are idempotent and converge when an ACK
is lost, a command is retried, or an offline command expires.

### Completion criteria

- Stable command IDs and terminal results survive retry/reconnect/restart.
- Duplicate delivery never repeats provider side effects.
- Offline expiry has a typed visible terminal result and cannot later execute.
- Tests inject lost ACK, duplicates before/after commit, delayed reconnect, and
  expiry on both Desktop and mobile adapters.

Verification: deterministic matrix with durable ledger assertions.  
Non-goals: cursor/event gaps, owned by CUT-08.  
Close rule: close with matrix results; keep #8677 open.

Status: accepted 2026-07-11. Lost ACK before/after apply, concurrent and
post-commit duplicates, semantic retry, same-ID conflict, delayed reconnect,
server-clock expiry, Desktop/mobile terminal visibility, and local-store
restart are green. Receipt:
[`2026-07-11-cut-07-command-convergence-receipt.md`](./2026-07-11-cut-07-command-convergence-receipt.md).

---

<!-- issue:CUT-08 -->
## CUT-08 — Converge event ordering, cursor gaps, refetch, and store compatibility

Parents: #8566, #8677  
Depends on: CUT-07  
Parallel-safe lane: R4 event convergence; projection/cursor/store code.

### Outcome

Confirmed conversation timelines converge under duplicate, reordered, missing,
and version-skewed events without fabricating provider history.

### Completion criteria

- Cursor and revision contracts detect gaps and trigger bounded authoritative
  refetch/replay.
- Duplicate/reordered events converge to one stable confirmed timeline.
- Store migration/compatibility covers supported previous schema versions and
  fails closed for unsupported ones with recovery guidance.
- Desktop and mobile produce equivalent results for the same injected trace.

Verification: shared cross-client fault corpus and migration receipts.  
Non-goals: process restart/finalization, owned by CUT-09.  
Close rule: merge with corpus evidence; keep #8677 open.

Status: accepted 2026-07-11. Dense live/log version validation, durable-cursor
replay, existing MustRefetch/CVR replacement, reordered/duplicate projection,
supported legacy migration, future-version refusal, and matching Desktop/mobile
trace results are green. Receipt:
[`2026-07-11-cut-08-event-store-convergence-receipt.md`](./2026-07-11-cut-08-event-store-convergence-receipt.md).

---

<!-- issue:CUT-09 -->
## CUT-09 — Converge restart, stale generation, revocation, and interrupted finalization

Parents: #8566, #8677  
Depends on: CUT-08  
Parallel-safe lane: R4 lifecycle faults; runtime/Sync restart paths.

### Outcome

Desktop and mobile recover one truthful terminal state after host restart,
stale-generation messages, authorization revocation, or interrupted provider
finalization.

### Completion criteria

- Stale generation/revision messages cannot mutate the current session.
- Revocation interrupts active authority and leaves a durable visible outcome.
- Restart during stream/finalization reconciles from durable provider/Runtime
  Gateway state without duplicate assistant output.
- Shared deterministic tests pass and a real network-disconnect/restart receipt
  crosses built Desktop and physical mobile.

Verification: matrix plus redacted live receipt.  
Non-goals: remote host move.  
Close rule: close this leaf and #8677 only after every fault row is evidenced.

Status: deterministic implementation accepted locally on 2026-07-11; live
close gate still open. Delayed Sync responses are generation-fenced, runtime
provider events require the exact durable next sequence and valid turn state,
revocation retracts both native stores, and stale hosted worker generations
settle once as interrupted without replaying inference/output. The built
Desktop Runtime Gateway v7 smoke passes. The paired physical iPhone is offline
in Tailnet and Xcode discovery, so the mandatory built-Desktop/physical-mobile
network-gap receipt has not been claimed. See
[`2026-07-11-cut-09-lifecycle-convergence-receipt.md`](./2026-07-11-cut-09-lifecycle-convergence-receipt.md).

---

<!-- issue:CUT-10 -->
## CUT-10 — Replace confirmed-timeline polling with synchronized Runtime Gateway live events

Parents: #8566, #8574, #8597  
Depends on: CUT-09  
Parallel-safe lane: D1 Runtime Gateway; transport/subscription paths.

### Outcome

Desktop and mobile subscribe to one cursor-aware Runtime Gateway event stream
instead of polling the confirmed timeline every 100 ms.

### Completion criteria

- A typed subscribe/resume protocol carries ordered provisional/confirmed/
  interrupted events with cursor and correlation refs.
- Reconnect resumes or performs bounded authoritative refetch on a proven gap.
- Backpressure, cancellation, slow consumers, host restart, and unsubscribe
  dispose cleanly.
- No production conversation surface uses interval polling for live state.

Verification: transport fault tests, latency/backpressure metrics, and built
Desktop/physical mobile continuation receipt.  
Non-goals: cloud host portability.  
Close rule: merge with no-polling boundary check and receipts.

### 2026-07-11 active tranche

The owner explicitly deferred CUT-09's physical receipt while the paired phone
records video and authorized the next non-device work. The first CUT-10 tranche
therefore lands the shared cursor/generation envelope, bounded authoritative
refetch, slow-consumer coalescing/metrics, exact disposal, mobile no-poll
adapter, and a file-disjoint bounded Desktop host subscription registry.
After #8712 released the landed Gateway wire, CUT-10 composed the bounded host
registry and typed subscribe/resume/unsubscribe protocol while preserving its
optional harness lane. After the chat-UI landing satisfied its handoff
condition, Desktop now consumes the fenced adapter across append and terminal
confirmation, closes with one exact unsubscribe, and enforces a source oracle
against recurring timeline polling. Only the owner-deferred physical receipt
remains. See
[`2026-07-11-cut-10-live-event-convergence-receipt.md`](./2026-07-11-cut-10-live-event-convergence-receipt.md).

---

<!-- issue:CUT-11 -->
## CUT-11 — Define and emit the canonical live Codex/Claude agent graph

Parents: #8566, #8574, #8597  
Depends on: CUT-06, CUT-10  
Parallel-safe lane: D1 agent graph; shared schema and provider adapters.

### Outcome

One live schema represents parent/subagent/worktree/tool relationships and
status transitions for Codex and Claude, distinct from historical import.

### Completion criteria

- Stable node/edge IDs, parentage, runtime/provider refs, status, timestamps,
  attention, terminal reason, and loss-accounted unknown states are typed.
- Codex and Claude adapters emit equivalent graph semantics through Runtime
  Gateway; unsupported provider facts remain explicit unknowns.
- Reconnect/replay preserves graph identity and rejects stale transitions.
- Synthetic scale and named-account live traces validate both providers.

Verification: schema/property tests and redacted provider traces.  
Non-goals: graph presentation, owned by CUT-12.  
Close rule: merge schema/adapters/docs and close with trace refs.

### 2026-07-11 shared-contract tranche

With CUT-10's no-poll renderer consumer landed, CUT-11 advances through the
disjoint shared contract. The registered
`openagents.live_agent_graph.v1` schema types stable nodes/edges, explicit
unknown facts, parent/worktree/tool/attention/terminal state, attachment
generation, and activity cursor. Its deterministic reducer accepts exact replay
and refuses stale/gapped generations, identity/cursor/timestamp regression,
terminal reopening, missing/mismatched parents, orphan tools, and cycles. It
includes typed Codex app-server and Claude Agent SDK observation adapters with
equivalent status/tool semantics and explicit provider omission, a validated
Khala Sync full-post-image entity under the canonical thread scope, and a named
transactional server changelog writer. Session and thread identities remain
distinct. The existing runtime start/control/event transaction now appends
Codex and Claude root graphs atomically, with provider identity unknown until
observed and terminal retry fenced by a new attachment generation. It does not
claim provider child topology or named-account traces. A confirmed-only client
read model now bounds graph post-images from the exact live thread scope and
Runtime Gateway protocol v8 emits them, with matching graph refs, through the
existing cursor-aware subscription. Deterministic resume and authoritative-
refetch reconnect are green; non-live cached graphs remain hidden. It does not
claim a named-account reconnect trace. Real Claude Agent SDK task lifecycle
messages now map into body-free `agent.child.*` events and stable child nodes /
parent edges inside the same graph transaction. The installed Codex SDK 0.139.0
public union does not expose a typed child event. Redacted named-account probes
now prove real Claude child lifecycle and the current Codex app-server's typed
`subAgentActivity` child source. They also prove the remaining gap: the bundled
Codex binary fails before a frame and the current
`codex exec --experimental-json` encoder drops that child record. Codex live
topology therefore remains explicitly unsupported in Pylon instead of being
inferred from tool names or historical rows. Per the Pylon streamlining audit,
the next fix converges this source through one conversation service rather than
adding another provider sidecar.
See
[`2026-07-11-cut-11-live-agent-graph-receipt.md`](./2026-07-11-cut-11-live-agent-graph-receipt.md).

---

<!-- issue:CUT-12 -->
## CUT-12 — Ship equivalent Desktop and mobile live-agent supervision UI

Parents: #8566, #8574, #8597  
Depends on: CUT-01, CUT-11  
Parallel-safe lane: D1 agent UX; Desktop/mobile graph presentation.

### Outcome

Both clients show the live canonical agent graph and let the operator inspect
and focus active work without confusing it with historical traces.

### Completion criteria

- Parent/subagent hierarchy, status, worktree/session, current tool/action,
  elapsed time, terminal reason, and attention are visible and accessible.
- Pointer, keyboard, screen-reader, and mobile tap navigation select the same
  typed focus/inspect actions.
- Rapid focus changes, reconnect, large graphs, missing facts, and terminal
  transitions remain deterministic.
- Historical imported graphs are labeled and cannot issue live controls.

Verification: Effect Native contract tests, synthetic scale, built Desktop,
physical iOS, and Android receipts.  
Non-goals: remote-host movement.  
Close rule: merge both clients and attach equivalent receipts.

### 2026-07-11 mobile + Desktop presentation tranche

The shared client now projects canonical graph post-images into deterministic
hierarchy rows with explicit provider/runtime/session/worktree/tool/attention/
terminal facts, live-versus-historical authority, stable selection fallback,
and a named large-graph remainder. Khala Mobile reads the confirmed graph from
the exact thread scope and renders an accessible, inline agent stack above the
transcript. Attention auto-opens the hierarchy; tap selection reveals details;
historical controls fail closed; and the mobile surface caps at 40 rendered
rows while naming the hidden count. OpenAgents Desktop now projects the same
newest confirmed Gateway v8 post-image through the shared model, hydrates
existing threads with one bounded subscribe/current/unsubscribe cycle, and
renders the hierarchy above the transcript with the same schema-checked
inspect/focus intent for pointer, keyboard, and screen-reader activation. It
retains no timeline or graph poller, omits focus for historical authority, and
caps the visible hierarchy at 200 rows with the exact remainder. Only physical
iOS and Android receipts remain open. See
[`2026-07-11-cut-12-live-agent-supervision-ui-receipt.md`](./2026-07-11-cut-12-live-agent-supervision-ui-receipt.md).

---

<!-- issue:CUT-13 -->
## CUT-13 — Establish canonical project, repository, and coding-session navigation

Parents: #8566, #8574  
Depends on: CUT-04, CUT-10  
Parallel-safe lane: D2 project/session contract; shared schema and Desktop
navigation.

### Outcome

Desktop has a durable provider-neutral project/repository/session catalog and
restores the exact working context without treating a tab or local path as
authority.

### Completion criteria

- Stable project, repository, worktree, coding-session, conversation, runtime,
  and provider refs have explicit ownership and authorization semantics.
- Create/open/search/archive/recover/sort and recent-session flows use typed
  actions and survive app restart.
- Tabs/routes restore focus, editor, terminal, conversation, and agent context
  or show a typed loss/recovery state.
- Path aliases, missing worktrees, renamed repositories, duplicate opens, and
  revoked grants are covered.

Verification: schema/migration tests and built-host restart receipt.  
Non-goals: remote host movement.  
Close rule: merge shared contract/Desktop navigation and close with recovery
evidence.

### 2026-07-11 canonical catalog tranche

Khala Sync now registers provider-neutral `coding_project`,
`coding_repository`, `coding_worktree`, `coding_session`, and
`coding_navigation` shapes under `openagents.coding_catalog.v1`. Stable product
identity is structurally separate from path, host, process, vendor session,
credential, and transport identity. Owner scope, grant/availability facts,
WorkContext/thread/conversation/runtime/provider/topology/cursor refs, bounded
tabs, and typed conversation/editor/terminal/agent focus are explicit. The
shared restart resolver canonicalizes former-name and opaque checkout aliases,
deduplicates tabs, and returns named recovery for ambiguous aliases, cross-
owner selection, missing repository/worktree, archive, revocation, and
unprojected grant truth. A 64-state bounded model enforces fail-closed restore.
Only structured catalog query fields are implemented; semantic text retrieval
remains upstream. The server now validates a bounded whole-catalog change set,
refuses cross-owner/broken/private-shaped input before storage, and appends all
post-images sequentially at one dense owner-scope transaction version; a real
local-Postgres receipt proves version 1 then version 2. Confirmed client reads,
Desktop host persistence/navigation, and the built-host restart receipt remain open. See
[`2026-07-11-cut-13-canonical-coding-session-catalog-receipt.md`](./2026-07-11-cut-13-canonical-coding-session-catalog-receipt.md).

---

<!-- issue:CUT-14 -->
## CUT-14 — Bind mobile to authenticated repositories, sessions, and threads

Parents: #8566, #8597  
Depends on: CUT-01, CUT-13  
Parallel-safe lane: M1 mobile binding; mobile directory/navigation only.

### Outcome

Mobile selects and resumes canonical repositories/coding sessions/threads by
authenticated stable refs rather than mock or device-local-only state.

### Completion criteria

- Mobile lists authorized repositories and active/recent sessions with loss-
  accounted offline cache state.
- Deep links and notifications resolve stable refs after sign-in/reconnect and
  reject unauthorized/stale targets.
- Switching repositories/sessions cancels old subscriptions and cannot leak
  content across scopes.
- iOS and Android restore the selected thread after process death.

Verification: auth/revocation/deep-link tests and physical iOS/Android receipts.  
Non-goals: starting remote cloud workrooms.  
Close rule: merge and attach process-death/reconnect evidence.

---

<!-- issue:CUT-15 -->
## CUT-15 — Unify typed commands, keybindings, menus, deep links, and single-instance routing

Parents: #8566, #8574  
Depends on: CUT-13  
Parallel-safe lane: D2 commands; command registry and host routing.

### Outcome

Every global/session/workbench action is one typed command invoked consistently
from UI, palette, editable keybindings, native menus, and deep links.

### Completion criteria

- Commands declare scope, availability, authorization, arguments, result, and
  default bindings in one registry.
- User keybinding conflicts are detected and recoverable; keyboard and pointer
  actions are equivalent.
- Native menus, deep links, second-instance opens, and restored routes dispatch
  the same commands after readiness gates.
- Duplicate/deferred opens and unauthorized refs fail visibly and safely.

Verification: registry coverage oracle, conflict tests, and packaged-host deep-
link/single-instance receipt.  
Non-goals: release signing.  
Close rule: merge with command coverage report.

---

<!-- issue:CUT-16 -->
## CUT-16 — Complete the Effect Native coding composer, questions, approvals, and runtime controls

Parents: #8566, #8574, #8597  
Depends on: CUT-12, CUT-15  
Parallel-safe lane: D1-D2 interaction; composer/control contracts.

### Outcome

Codex and Claude coding turns can be composed and supervised entirely inside
OpenAgents with rich context and durable typed control outcomes.

### Completion criteria

- Composer supports text, files/images, repository/editor/diff context,
  provider/model/account selection, send/cancel, drafts, and retry.
- Provider questions, permission/tool approvals, plan/review transitions,
  interrupt, resume, retry, and cancel are first-class typed timeline items.
- Desktop and mobile render equivalent authoritative pending/resolved/expired
  states; duplicate actions remain idempotent.
- Keyboard, screen reader, mobile keyboard, reduced motion, offline, restart,
  and revoked authority paths are tested.

Verification: cross-client contract suite and named Codex/Claude live turns.  
Non-goals: remote workroom creation.  
Close rule: merge with provider and physical-device receipts.

---

<!-- issue:CUT-17 -->
## CUT-17 — Ship grant-scoped workspace tree, watch, cache, and search

Parents: #8566, #8574  
Depends on: CUT-13  
Parallel-safe lane: D3 workspace; filesystem capability service.

### Outcome

Desktop can browse and search large repositories through explicit workspace
grants without renderer ambient filesystem access.

### Completion criteria

- Recursive lazy tree, refresh/watch, ignore rules, reveal, create/rename/
  delete, and bounded content/path search use typed capability services.
- Caches declare key, invalidation, freshness, and owning scope.
- Symlink escape, path traversal, secret-shaped files, permission loss,
  watcher overflow, huge trees, and binary files fail safely.
- Project close disposes every watcher/search task exactly once.

Verification: adversarial filesystem fixtures, scale benchmark, and lifecycle
receipt.  
Non-goals: editing and Git, owned by CUT-18/CUT-19.  
Close rule: merge with boundary and disposal evidence.

---

<!-- issue:CUT-18 -->
## CUT-18 — Ship the Effect Native editor host and conflict-safe document lifecycle

Parents: #8566, #8574  
Depends on: CUT-17  
Parallel-safe lane: D3 editor; document model and foreign-view lowering.

### Outcome

Desktop provides a practical code editor while Effect Native owns document
state/actions/lifecycle and the editor widget remains a replaceable foreign
view.

### Completion criteria

- Tabs, language mode, selection, find, undo/redo, dirty state, save/save-as,
  external change, conflict resolution, and crash/restart recovery are typed.
- The editor adapter receives capabilities and emits intents; it cannot own
  project/session authority or access arbitrary filesystem paths.
- Large files, binary/unsupported files, encoding, deletion, rename, concurrent
  change, and revoked grants have explicit outcomes.
- Document/editor resources dispose exactly once on tab/project/window close.

Verification: adapter substitution tests, conflict corpus, and built-host
editing receipt.  
Non-goals: full IDE language-server breadth.  
Close rule: merge with recovery and boundary evidence.

---

<!-- issue:CUT-19 -->
## CUT-19 — Complete the typed Git review and composer-context loop

Parents: #8566, #8574  
Depends on: CUT-18  
Parallel-safe lane: D3 Git; Git service and review UI.

### Outcome

Developers can inspect repository state, review diffs, add review context, and
perform bounded safe Git actions without leaving Desktop.

### Completion criteria

- Status, staged/unstaged/untracked/conflict state, file/hunk diff, refresh,
  discard/revert confirmation, and composer-context attachment are typed.
- Secret/binary/oversized diff policy is enforced before renderer projection or
  provider attachment.
- Worktree/repository identity is verified before mutation; concurrent external
  Git changes reconcile visibly.
- Dirty, conflict, rename, submodule, detached, and no-repository fixtures are
  covered without destructive reset/checkout behavior.

Verification: real temporary-repository corpus and built-host review receipt.  
Non-goals: automated commit/push or PR publication.  
Close rule: merge with redaction and mutation-safety evidence.

---

<!-- issue:CUT-20 -->
## CUT-20 — Add workspace-bounded PTY terminals and local preview lifecycle

Parents: #8566, #8574  
Depends on: CUT-17  
Parallel-safe lane: D3 terminal-preview; process capability service.

### Outcome

Ordinary build/test/dev-server work runs in scoped Desktop terminals with
explicit local-preview lifecycle and no renderer ambient process authority.

### Completion criteria

- PTY create/input/resize/output/exit/interrupt/restart uses typed commands and
  binds cwd/environment to an authorized workspace/session.
- Terminal buffers are bounded/redacted and survive or loss-accountedly recover
  app restart; project close kills owned process trees exactly once.
- Local preview discovers an explicit bound port, shows readiness/errors, and
  stops with its owning task/session.
- Shell injection, secret environment, runaway output, orphan children,
  duplicate start, port collision, and revoked grants are tested.

Verification: adversarial PTY suite and built-host test/dev-preview receipt.  
Non-goals: remote shell/workroom terminals.  
Close rule: merge with process-tree disposal evidence.

---

<!-- issue:CUT-21 -->
## CUT-21 — Deliver provider-neutral named Codex and Claude accounts, models, and runtimes

Parents: #8566, #8574, #8640  
Depends on: CUT-06, CUT-13  
Parallel-safe lane: D4 providers; account/runtime catalog.

### Outcome

OpenAgents owns a provider-neutral catalog for named Codex and Claude accounts,
models, readiness, capabilities, and local runtime launch without depending on
developer source checkouts or default CLI homes.

### Completion criteria

- Account/model/runtime identity, readiness, capability, auth method, health,
  and loss-accounted unknown fields share one typed contract.
- Named account selection is stable per session and secrets remain in host
  vault/capability boundaries.
- Bundled/discovered runtime launch has explicit version compatibility and does
  not require the openagents source tree or mutate default Codex/Claude homes.
- Missing/revoked auth, incompatible versions, concurrent accounts, runtime
  crash/update, and offline state are tested for both providers.

Verification: clean-machine-shaped harness plus named Codex/Claude live runs.  
Non-goals: cloud provider placement.  
Close rule: merge with redacted readiness and runtime receipts.

---

<!-- issue:CUT-22 -->
## CUT-22 — Import Claude Code history into the loss-accounted local session catalog

Parents: #8566, #8574  
Depends on: CUT-21  
Parallel-safe lane: D4 continuity; read-only Claude history adapter.

### Outcome

Existing Claude Code sessions remain discoverable and inspectable beside Codex
history during cutover without claiming unsupported structure or enabling live
control of imported records.

### Completion criteria

- The adapter discovers named-account Claude history without reading unrelated
  homes and maps stable known facts into the canonical local catalog.
- Parentage, tools, timestamps, terminal status, model, and workspace fields are
  explicit value/unknown/loss-reason rather than guessed.
- Incremental refresh, malformed/truncated records, duplicates, huge history,
  account removal, and schema drift are bounded.
- Imported history is labeled read-only/historical and cannot invoke live
  runtime controls.

Verification: sanitized fixture corpus, structure-only real-history receipt,
and scale test.  
Non-goals: converting old sessions into live provider sessions.  
Close rule: merge adapter/docs and close with privacy-safe evidence.

---

<!-- issue:CUT-23 -->
## CUT-23 — Integrate MCP, skills, plugins, permissions, and settings through typed lifecycle

Parents: #8566, #8574  
Depends on: CUT-16, CUT-21  
Parallel-safe lane: D4 extensions-policy; registries and settings.

### Outcome

Codex/Claude tools, MCP servers, skills, plugins, and permissions are visible,
configurable, and auditable inside OpenAgents without ad hoc intent routing or
secret leakage.

### Completion criteria

- Provider-neutral registries expose provenance, scope, readiness, grants,
  restart requirement, and per-session use.
- Enable/disable/configure/reload actions are typed and validated; secrets use
  vault references and never renderer state/logs.
- Semantic tool/routing selection uses the central typed selector/planner rather
  than keyword matching; bounded identifiers may be parsed after route choice.
- Invalid config, crash/restart, prompt injection, revoked grants, duplicate
  names, provider disagreement, and offline state are covered.

Verification: policy/boundary tests plus one real MCP/skill workflow on each
provider.  
Non-goals: an extension marketplace.  
Close rule: merge with redacted configuration and audit receipts.

---

<!-- issue:CUT-24 -->
## CUT-24 — Finish coding-app preferences, accessibility, notifications, diagnostics, and recovery

Parents: #8566, #8574, #8597  
Depends on: CUT-12, CUT-23  
Parallel-safe lane: D4-D5 operability; settings/diagnostic surfaces.

### Outcome

The coding app is operable for sustained daily use, exposes actionable health,
and recovers visibly from failures without leaking private work.

### Completion criteria

- Theme, density, font, reduced motion, keybindings, provider defaults, privacy,
  notifications, and update preferences have typed durable schemas/migrations.
- Desktop/mobile meet keyboard, focus, screen-reader, contrast, dynamic type,
  target-size, and reduced-motion acceptance for core coding flows.
- Notifications carry stable authorized refs and never prompt/code/secrets;
  attention clears only after authoritative acknowledgement.
- Diagnostics/watchdog show provider, Runtime Gateway, Sync, workspace, PTY,
  and extension health with redacted export, restart, and recovery actions.

Verification: accessibility audit, migration tests, fault injection, and
privacy scan of notification/diagnostic artifacts.  
Non-goals: analytics expansion.  
Close rule: merge with audit and recovery receipts.

---

<!-- issue:CUT-25 -->
## CUT-25 — Make Fleet the authoritative Desktop cockpit and mobile attention surface

Parents: #8566, #8574, #8597  
Depends on: CUT-12, CUT-14, CUT-24  
Parallel-safe lane: D5 Fleet; projections and control surfaces.

### Outcome

Fleet is no longer a locally staged brief: it projects authoritative active
Codex/Claude work, attention, approvals, controls, and durable receipts on
Desktop and mobile.

### Completion criteria

- Fleet cards derive from canonical run/session/agent/attention projections and
  link to the exact conversation, graph node, repository, and receipt.
- Pause/resume/cancel/retry/approve controls are typed, authorized, idempotent,
  generation-checked, and converge across clients.
- Offline/stale/unknown/revoked state is visible and cannot issue optimistic
  authority; reconnect resolves via cursor/refetch rules.
- Named simultaneous Codex+Claude work demonstrates mobile attention,
  approval/control, Desktop acknowledgement, and durable terminal receipts.

Verification: shared projection/control suite plus built Desktop and physical
iOS/Android receipt.  
Non-goals: remote workroom placement.  
Close rule: merge and attach one public-safe end-to-end Fleet receipt.

---

<!-- issue:CUT-26 -->
## CUT-26 — Ship hardened Desktop distribution, updates, rollback, and legacy lockout

Parents: #8566, #8574  
Depends on: CUT-24  
Parallel-safe lane: D6 distribution; build/release configuration.

### Outcome

The OpenAgents Desktop coding app installs and updates on a clean supported Mac
with hardened Electron boundaries, recoverable data migration, and no path that
ships the legacy Desktop UI.

### Completion criteria

- Stable app/bundle identity, icons, versioning, production renderer artifact,
  hardened fuses/sandbox/IPC/CSP, code signing, notarization, and artifact
  provenance are automated.
- Update check/download/stage/restart/rollback preserves or loss-accountedly
  migrates sessions, vault refs, settings, and drafts.
- CI/release oracles fail if legacy UI entrypoints/assets or source-checkout
  runtime dependencies enter the artifact.
- Clean-machine install, first run, named account readiness, coding smoke,
  update, interrupted update, rollback, uninstall/reinstall, and diagnostics
  export pass.

Verification: notarized artifact metadata and public-safe clean-machine video/
transcript.  
Non-goals: Windows/Linux distribution unless separately scoped.  
Close rule: merge release automation and close only after installed-artifact
acceptance.

---

<!-- issue:CUT-27 -->
## CUT-27 — Prove and declare the Codex/Claude-to-OpenAgents coding cutover

Parents: #8566, #8574, #8597  
Depends on: CUT-19, CUT-20, CUT-21, CUT-22, CUT-23, CUT-24, CUT-25, CUT-26  
Parallel-safe lane: R7 acceptance; no feature implementation may be hidden in
this issue.

### Outcome

Real maintainers complete sustained ordinary Codex and Claude coding work
entirely in the installed OpenAgents Desktop app, with authenticated physical
mobile continuation/supervision and no required fallback to Codex or Claude
Code UI.

### Completion criteria

- On a clean installed build, complete at least one non-trivial real repository
  task with named Codex and one with named Claude: open project/session, inspect
  history/agents, compose with context, answer questions/approvals, edit files,
  run tests/preview, review Git diff, and reach a durable terminal receipt.
- During each task, physical iOS and Android reconnect, inspect the same stable
  refs/timeline/agent graph, continue a turn, handle one attention/control item,
  survive one forced network/app interruption, and converge with Desktop.
- Run the accepted #8676 physical handoff and #8677/#8678/#8640 parent receipts;
  every cutover leaf is closed and no unresolved P0 defect in these flows is
  waived.
- Publish the loss/exception register, accessibility/privacy/security results,
  artifact provenance, rollback result, and explicit later-remote-work boundary.
- Update product/runbook/roadmap docs to make OpenAgents Desktop the default
  local coding surface and mark direct Codex/Claude Code UI fallback
  unsupported for the proven scope.

Verification: public-safe evidence bundle with issue links, commit/artifact
identities, device/OS/app versions, stable refs, commands, timestamps, failures,
and recovery outcomes.  
Non-goals: remote workrooms, host movement, Daytona, elastic placement, or
voice; those retain their own open Revision 31 proof.  
Close rule: close #8574's ordinary local-coding scope only after acceptance;
do not close #8566/#8597 while their remote-first outcomes remain unproved.
