# Fable Unified Roadmap — Khala Code, Fleet, QA, Multi-Harness, Artanis

Date: 2026-07-01
Status: the single consolidated execution roadmap for every recommendation
and plan in `docs/fable/`. Sources: the Khala Code summary/analysis, the
episode-245 / multi-harness doc, the QA framework design, the fleet fan-out
coding instructions, the Effect integration audit, the Claude-parity and
synergies plan, the Orca adoption plan, and the Artanis fleet-administrator
audit. Each workstream below cites its source doc(s); where two docs
specified the same work, it appears once here with both citations. This doc
flips no promise state and broadens no public copy. The delivery process
(issues, PRs, worktrees, review, counters) is [`EXECUTION.md`](./EXECUTION.md).

## 0. Reading Guide

- **WS-n** = workstream. Workstreams are the parallelization unit: each can
  be delegated to a different agent/worker lane once its listed dependencies
  have landed.
- **Tn.m** = task. Tasks are the GitHub-issue unit: one issue per task, one
  PR per issue (see EXECUTION.md).
- **Deps** are hard: a task must not start until its deps are merged to
  `main`. "Soft-after" means preferred order, not a blocker.
- Delegability grades: **HIGH** = safe to hand a fleet worker with a bounded
  prompt + pinned verify command; **MED** = delegable with a tightly-written
  issue and reviewer attention on the seam; **LOW** = keep with the
  supervising agent (cross-cutting seam, high blast radius, or judgment-heavy).

## 1. Dependency Spine (What Blocks What)

```text
WS-1 Contracts spine (Effect P1)          WS-2 Orchestration store live
        |                                        |
        +----------------+-----------------------+
        |                |                       |
   WS-6 QA spine    WS-3 Fan-out engine     WS-4 Planner+claims+verify
   (G1,G2 first)         |                       |
        |                +-----------+-----------+
        |                            |
        |                     WS-5 Cockpit UI ---- WS-10 Status spine
        |                            |                    |
   WS-6 rest (monkey,           WS-15 Review loop    WS-11 Mobile companion
   live smokes, TLA+)                |
                                     |
WS-7 Effect process spine (P2) --- feeds WS-3/5/8 as it lands (soft)
WS-8 Claude chat harness (P0 gates the rest of WS-8; else parallel)
WS-9 Multi-harness routing (needs WS-4 workerKind + WS-8 P0)
WS-12 Artanis (P1/P2 parallel now; P3 needs WS-2+WS-10; P5 needs WS-9/11)
WS-13 Foldkit migration (rides behind; cockpit panel first)
WS-14 Guardrails (continuous from day 1)
WS-16 Episode 245 (owner ops; needs nothing new — rehearse now)
WS-17 Clean 2B day (needs WS-3,4,5 core + WS-6 live smokes)
```

The two foundations — **WS-1** and **WS-2** — land first and unblock nearly
everything. They are deliberately small. Everything else fans out.

## 2. Workstreams And Tasks

### WS-1 — Contracts spine (Effect audit Phase 1) — START IMMEDIATELY

Source: Effect integration audit §5 Phase 1. The highest-leverage few days
in the whole program: kills debt #1/#2-adjacent/#10/#13, gives the QA
framework its schema oracle, gives the fan-out engine typed seams.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T1.1 | Schema-first `shared/rpc.ts`: every RPC request/response as Effect Schema (derive existing TS types via `typeof X.Type`), decode on both transports (preview bridge + `main.ts` client), handler failures as tagged-error unions instead of `{ok:false,error:string}` | — | MED (big but mechanical; one worker, tight review) |
| T1.2 | Shared Pylon wire-event contract: promote `AssignmentRunLifecycleEvent` + `PylonKhalaSpawnWorkerEvent` to Effect Schema in a shared package; Pylon emits and desktop consumes through it; delete the desktop's local re-declaration and `stringField` probing | — | HIGH |
| T1.3 | `KhalaCodeConfig` service: all ~55 env keys behind one `Context.Service` with `Config.schema` + `Config.redacted`, `Layer.succeed` test profiles | — | HIGH |
| T1.4 | Notification fan-out isolation in `codex-app-server-client.ts`: one throwing subscriber must not abort delivery to the rest (small, urgent; full rewrite comes in T7.2) | — | HIGH |

### WS-2 — Orchestration store goes live (Orca P1 = fan-out A1/B1) — START IMMEDIATELY

Source: Orca adoption plan Priority 1; fleet fan-out §3.1/§4.1. **One state
store, not two**: FleetRun + claims are implemented ON
`apps/pylon/src/orchestration/` (tasks = work units, dispatch contexts =
workers, messages = lifecycle bus, VirtualHead = merge-queue base pinning).

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T2.1 | FleetRun record (`openagents.khala_code.fleet_run.v1`): Effect Schema, persisted in the orchestration store + desktop owner-local state dir, restart/reconcile semantics (#7593/#7597 patterns); handoff vs supervised-dispatch taxonomy in the model | — | MED |
| T2.2 | Claim registry (`openagents.khala_code.work_claim.v1`): at most one live claim per workUnitRef (unique key), TTL expiry + reconciler release, fast-check property test over concurrent claim interleavings | — | MED |
| T2.3 | Demote bash supervisors to store-driven process launchers: live desired-slot, pause, dispatch-attempt, completion, and work-claim state flows through `apps/pylon/src/orchestration/supervisor-state.ts` into the orchestration store; shell remains owner of process PID/log/cache and launchd wedge telemetry. Repo check `apps/pylon/scripts/check-supervisor-store-bypass.mjs` prevents live fleet run/claim state from returning to shell files. | T2.1 | MED |

### WS-3 — Fan-out engine (fleet fan-out Lane A)

Source: fleet fan-out §3. One message/action starts a sustained N-worker
run with refill.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T3.1 | `FleetRunSupervisor` in the Bun host: Effect + `Scope`d tick loop (count active → claim → dispatch `khala.fleet.delegate` per free slot → stream lifecycle into counters); arbitrary N via refill across ticks; one supervisor per Pylon; built on `PylonService` when T7.3 lands (start on existing seams, do not wait) | T2.1, T2.2 | MED |
| T3.2 | `khala_fleet` MCP verbs: `fleet_run_start`, `fleet_run_status`, `fleet_run_control` (pause/resume/drain/stop); approval mode stays `prompt`; `codex_spawn` remains the bounded one-shot | T3.1 | HIGH |
| T3.3 | Fleet panel "Start fleet run" form: objective, work source, target concurrency, workerKind enum (`codex\|claude\|auto` accepted now, `codex` wired), dry-run preview of the first wave | T3.1 | HIGH |
| T3.4 | RPC parity: `fleetRunStart/Status/Control/List` methods + schemas (rides T1.1's contract) | T3.1, T1.1 | HIGH |
| T3.5 | Fixture acceptance: target-25 fixture run reaches 25 simulated concurrent assignments via refill, drains cleanly, survives host restart; MCP verbs round-trip against scripted app-server fixture | T3.1–T3.4 | HIGH |

### WS-4 — Work planner, claims-in-anger, verification gate (Lane B)

Source: fleet fan-out §4. The June 29 duplicate-PR fix.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T4.1 | Typed work planner: source adapters (`github_backlog` via `gh`, `issue_list`, `fixture`) emitting candidate units with typed skip reasons (`already_claimed`, `pr_exists`, `merged`, `closed`, `needs_owner`, `label_excluded`); port #7595 semantics into the shared package | T2.2 | HIGH |
| T4.2 | Prompt/pin discipline: every real-work dispatch carries pinned repo/commit/branch/verify + claimRef; worker prompt cites issue + claim + PR convention | T2.2 | HIGH |
| T4.3 | Verification gate + merge policy: closeout `ready_for_review` requires verify-green + live claim; typed merge policy (`manual_review` default, `auto_merge_clean` owner-toggled); merge-wave resolver as a supervised job with its own claim | T4.1 | MED |
| T4.4 | workerKind generalization (= episode-245 Axis B): `workerKind` through `codex_spawn`/`khala.fleet.delegate`/fleet tools; de-Codex-name `khala-codex-fleet-tools.ts` and `khala-burndown.ts`; capacity/blocker vocabulary keyed by kind; dispatch selects `codex_agent_task` vs `claude_agent_task` | T2.1 | MED |
| T4.5 | Acceptance fixtures: 10-unit/6-worker fixture with 0 duplicates + all skips typed; "duplicate temptation" regression; live 2-worker/2-issue distinct-PR smoke (skip-safe) | T4.1–T4.4 | HIGH |

### WS-5 — The Fleet cockpit (Lane C)

Source: fleet fan-out §5, consuming the one status vocabulary from WS-10.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T5.1 | Run header + controls: Pause/Resume/Drain/Stop wired through `fleetRunControl` to orchestration control verbs | T3.1, T3.4 | HIGH |
| T5.2 | Worker cards + live lifecycle streaming: implement the NDJSON consumer per the 2026-06-30 streaming audit (Effect `Stream` → throttled card updates); per-worker Interrupt/Retry/Flag; no fabricated progress | T3.1, T1.2 | MED |
| T5.3 | Account cards: readiness, slots, rate-limit meters counting down (`resetsAt`, reset credits), cooldowns, Reconnect via isolated-home device login (never `~/.codex`), Pause-account | T3.1 | HIGH |
| T5.4 | Throughput gauges: tokens/min (exact-row based), run totals, projected tokens/day; reconcile against `GET /api/public/khala-tokens-served` deltas in live smoke only | T3.1 | HIGH |
| T5.5 | Flags → Inbox + condensed sidebar fleet counts (accounts ready / workers active / slots free / flags) — also closes the episode-245 italic-script gap §1.3.3 | T3.1 | HIGH |
| T5.6 | Cockpit visual smoke: 18 fixture workers / 3 accounts, desktop + mobile viewports, geometry oracles; TestClock-driven rate-limit countdown test | T5.1–T5.5 | HIGH |

### WS-6 — QA framework (G-gaps + scenario/explore/formal tiers)

Source: QA framework design §11 (G1–G6, P0–P4); fleet fan-out Lane D.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T6.1 | **G1** Bridge auth + events: loopback bearer (+ optional read-only mode) and `GET /rpc/events` SSE carrying chat turn events, fleet lifecycle, run counters, console errors | T1.1 soft | MED |
| T6.2 | **G2** `KhalaCodeRpcClient`: typed Effect client for all RPC methods incl. fleet-run verbs; `schema` + `consistency` oracles fall out | T1.1 | HIGH |
| T6.3 | **G4** Scenario DSL + driver service + `packages/khala-qa-harness` (extract duplicated Vite/probe helpers; four drivers: RPC/DOM/vision/headless; verifier + distiller wiring) | T6.2 | MED |
| T6.4 | **G5** Fixture Codex app-server process (JSON-RPC from recorded notification scripts incl. approvals + background terminals) — the single biggest flake-killer | — | MED |
| T6.5 | **G3** qa-runner desktop backend: boots the app headless (fixture or live), composes Chromium + RPC client + JSONL; headed variant arms the native macOS AX backend against the real Electrobun window | T6.2, T6.3 | MED |
| T6.6 | **G6** Coverage ledger: per-run mergeable JSON (RPC methods, slash commands, panels, settings keys, item variants, selectors); nightly union + frontier report; zero-for-a-week ⇒ auto-issue | T6.3 | HIGH |
| T6.7 | Mechanical seed scenario corpus: one lifecycle scenario per RPC group, per hotbar panel, per ThreadItem variant, per slash command; every phase has expectations | T6.3 | HIGH |
| T6.8 | Seeded monkey explore mode (seeded PRNG over enabled action space, seed+log replay) + fleet-cockpit monkey night with claim-invariant oracle | T6.3, T6.4 | MED |
| T6.9 | LLM explorer (qa-runner live brain) + explore→distill→regress loop; coverage-frontier steering for both explorers | T6.5, T6.6 | MED |
| T6.10 | Live smokes: `smoke:fleet-run-live` (target 2, real closeouts, counter reconciliation) and `smoke:fleet-run-sustained` (≥5 workers ≥30min, ≥2 refills, zero duplicate claims) — skip-safe, env-armed | T3.5, T4.5 | MED |
| T6.11 | Property-based tier (`fast-check`): composer editing, thread-item projector interleavings, markdown/diff renderers | T6.3 soft | HIGH |
| T6.12 | Model-based tier: Effect Schema state machines (thread, approval, delegate program, supervisor) + fast-check model-based commands against Mode P | T6.2 | MED |
| T6.13 | Formal tier (TLA+, bounded): `khala.fleet.delegate` + supervisor spec (no dead-end, termination, no oversubscription, claim uniqueness under racing supervisors, paused-claims-nothing, drain terminates); approval protocol; session/thread mapping; counterexamples → fixtures | T3.1, T2.2 | MED |
| T6.14 | Perf metrics registry + budgets: generalize `threadSwitchPerformance` into `qaMetrics()`; budgets (cockpit <100ms @50 cards, lifecycle→card p95 <500ms, tick <1s @25); nightly trends | T6.3 | HIGH |
| T6.15 | GEPA explore-policy loop (offline, Gym-admitted, never auto-promote) + scenario portfolio pruning by yield | T6.8, T6.9 | LOW |

### WS-7 — Effect process/protocol spine (Effect audit Phase 2)

Source: Effect integration audit §5 Phase 2. Feeds WS-3/5/8 as it lands.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T7.1 | `KhalaProcess` scoped subprocess service on `effect/unstable/process` (kill-on-scope-close, stdin Sink / stdout Stream, one timeout/kill policy); replace all five hand-rolled spawn implementations | T1.3 soft | MED |
| T7.2 | `CodexAppServer` as `Context.Service`: typed tagged errors, Schema-decoded responses/notifications (generate candidates from `codex app-server generate-ts`), notifications as `Stream` (subscriber isolation by construction), timeout policy that fires `turn/interrupt`, scoped supervision | T7.1 | MED |
| T7.3 | Typed `PylonService` (`request`, `runAssignment`, `lifecycle: Stream`), backed by T7.1 + T1.2 schemas; stub layer for fixtures; the supervisor (T3.1) migrates onto it | T7.1, T1.2 | MED |
| T7.4 | **Done in PR "T7.4: khala-tools substrate fixes"**: khala-tools substrate now injects runtime `Clock`/random for dispatcher durations and IDs, wraps one-shot sandbox/exec process groups in `acquireRelease`, removes `runPromise`-inside-`Effect.promise` nesting from `exec-command.ts`, and exposes a Layer-backed `KhalaToolServicesService` while keeping tool-result errors as data | — | MED |
| T7.5 | Token reporting as Effect with `Schedule.exponential` retry + Inbox flag on persistent failure; attachment temp files as scoped resources; corrupt session-state file recovery (no rethrow-brick) | T7.2 soft | HIGH |

### WS-8 — Claude chat harness (Claude-parity Phases 0–3, = Axis A)

Source: Claude-parity doc §3. `ClaudeChatRuntime` is the desktop's first
real greenfield Effect service.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T8.1 | **Phase 0** `ChatRuntime` abstraction: extend `runtimeMode`/backend kinds, `selectChatRuntime()` three-way selector replacing `useLegacyKhalaNativeRuntime()`, neutral `harnessItem`, widen headless runtime pick | T1.1 soft | LOW (this is the seam — supervising agent or best worker + close review) |
| T8.2 | **Phase 1** minimal Claude chat: `claude-app-sdk-chat-runtime.ts` (query() → Stream, Query → acquireRelease, own AbortController), `claude-thread-item-projector.ts` (SDKMessage Schema union → neutral turn events), `claude-session-store.ts`, `claude-harness-status.ts` (wrap Pylon probes), wire into index/handlers/headless | T8.1 | MED |
| T8.3 | **Phase 2** approvals (`canUseTool` → Deferred/Queue approval service, Claude-native shapes), token telemetry (exact-only, decide ingest route deliberately), `claude-fleet-mcp-bridge.ts` (inject `khala_fleet` via `options.mcpServers`), Claude settings panel | T8.2 | MED |
| T8.4 | **Phase 3** sidebar via `listSessions()`/`getSessionMessages()`, slash registry from `supportedCommands()`, `claude-parity-contract.ts` + gap matrix pinned to SDK version | T8.2 | HIGH |
| T8.5 | **SHIPPED** Harness pill UI ("Codex \| Claude \| Khala") in composer HUD + persisted setting + runtime badge rendering (= episode-245 P1 toggle, generalized) | T8.1 | HIGH |

### WS-9 — Multi-harness routing and synergies (Axis B + crossovers)

Source: episode-245 §3.2–3.3; Claude-parity §4.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T9.1 | `khala fleet connect --harness claude`: wrap `claude setup-token` into an isolated `.claude-*` home; readiness in `khala fleet status` | — | HIGH |
| T9.2 | `auto` workerKind v1: local free-slot rule in the delegate program (prefer kind with free advertised slots) | T4.4 | HIGH |
| T9.3 | `auto` v2: server classifier (`coding-workflow-classifier.ts` seam), then GEPA-optimizable routing *parameters* (never control flow), Gym-admitted | T9.2, T6.15 soft | LOW |
| T9.4 | Plan-then-fan-out: Claude plan-mode session (Fable, `permissionMode:'plan'`) emits a typed task DAG → FleetRun work units → Codex dispatch; Claude reviews returned diffs (accept / request-changes / re-plan); deterministic program stays control-flow authority | T8.2, T3.1, T4.1 | MED |
| T9.5 | Claude second-pass reviewer: structured verdict (`outputFormat: json_schema`) after verify-green, feeding merge policy as advisory signal (verify command remains authority) | T4.3, T8.2 | MED |
| T9.6 | Cross-harness session catalog: shared `SessionStore` adapter (SDK conformance suite) so mixed Codex/Claude fleet runs land in one queryable history | T8.2 | MED |
| T9.7 | Claude-lane closeout depth (deferred until Claude workers do PR delivery): PR publisher analogue, per-turn token rows, raw-event/ATIF observability, full-access posture decision, Claude burn runbook | T9.1 | MED |

### WS-10 — One status spine (Orca P2)

Source: Orca adoption plan Priority 2.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T10.1 | `agent_runner_status_event.v1` as the only status vocabulary runner→store→cockpit→Worker→mobile; adopt live-vs-retained, `stateStartedAt` vs `updatedAt`, rolling `stateHistory`, decay-to-idle | T2.1 | MED |
| T10.2 | Un-mock the `/pro` operator dashboard with live ingest from the spine | T10.1 | HIGH |
| T10.3 | Retire the bespoke `/api/operator/fleet/status` snapshot in favor of the spine projection (keep a compat window for iOS until T11.1) | T10.1 | MED |

### WS-11 — Mobile companion (Orca P3; file Port 4 now)

Source: Orca adoption plan Priority 3. Native SwiftUI, no OTA, phone =
observe/notify/approve/steer, never hosts work.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T11.1 | Pairing + transport: QR pairing offer (endpoint + per-device token + public key), keychain-held per-device bearer, NaCl-box E2EE at app layer, Durable-Object relay transport; read-only fleet status subscription replaces the bespoke poll | T10.1 | MED |
| T11.2 | Allowlisted mobile RPC surface + the enforcing test (every mobile-callable method explicitly registered) — copy Orca's discipline exactly | T11.1 | HIGH |
| T11.3 | Push notifications (APNs): finish / blocked / approval-needed | T11.1 | MED |
| T11.4 | Approve/reject + steer from the phone (Inbox typed responses; send follow-up/objective to a run or worker) | T11.1, T5.5 | MED |
| T11.5 | Bounded diff/PR review on mobile | T11.4, T15.1 | MED |

### WS-12 — Artanis: from bounded operator to fleet administrator

Source: Artanis audit §6 Priorities 1–5.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T12.1 | **P1** Authority scope first-class: typed `owner_self \| shared_fleet \| owner_operator` on every Artanis tool/gate/dispatch; dispatch gate + capacity resolver enforce scope-linked capacity only | — | LOW |
| T12.2 | **P2** Blueprint signatures 1–5 enforced (fleet-liveness, diagnosis-grounding, issue-close-safe, command-source-verified, merge-deploy) as structural gates preceding consequential actions | — | MED |
| T12.3 | **P3** Artanis on the orchestration/status spine: `get_fleet_status`/`dispatch_codex_task`/fleet-overseer tick read/write the store; `dispatch_codex_task` grows into "start/steer a FleetRun" | T2.1, T10.1, T3.2 | MED |
| T12.4 | **P4a** Consolidate the duplicate Artanis forum identity (admin re-register override; retire the Raynor-token workaround as tracked debt) | — | MED |
| T12.5 | **P4b** Autonomy ladder: raise one gate at a time, gated on signatures enforced + scope typed + clean-tick track record; treasury stays envelope-bounded | T12.1, T12.2 | LOW |
| T12.6 | **P5** AaaS productization: `owner_self`-scoped Artanis through the cockpit + mobile companion; onboarding = `khala fleet connect`; "Artanis, Fleet Commander" demo flow | T12.1, T12.3, T11.4 | MED |
| T12.7 | Verify/file the multi-user read-only Artanis chat issue the owner ordered (Jun 27) | — | HIGH |

### WS-13 — Foldkit shell migration (Effect audit Phase 3; staged, never blocking)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T13.1 | Port the autopilot-desktop Foldkit template (model/message/view/subscriptions + Runtime.run) into khala-code-desktop as the embedding skeleton; `Runtime.embed` + Schema-typed Ports | — | MED |
| T13.2 | Cockpit as the first embedded Foldkit program (new panels are Foldkit-first from here on) | T13.1, T5.1 | MED |
| T13.3 | Compose from `@openagentsinc/ui` Foldkit components; Gym pane → `arbiterGraphFigure` vdom; retire `icon-dom`/`menu-dom`/`innerHTML` graph injection | T13.1 | HIGH |
| T13.4 | Transcript/main shell migrates last, panel-by-panel; TEA model absorbs the ~18 module-level `let`s | T13.2, T13.3 | MED |

### WS-14 — Testing guardrails (Effect audit Phase 4; continuous)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T14.1 | `@effect/vitest` + TestClock for all new Effect services; port the deterministic harness (`TestEnvironmentLayer`, `withSeed`, `stubTransportLayer`) into `packages/khala-qa-harness` | T6.3 soft | HIGH |
| T14.2 | Extend the report-only architecture scan to khala-code-desktop + khala-tools (flag `JSON.parse … as`, bare `catch{}`, direct env reads, `Date.now()` in logic, stray `Effect.runPromise`, `setTimeout` kills); promote to hard-fail after WS-1/WS-7 land | — | HIGH |
| T14.3 | The Effect pattern doc with desktop-native approved examples | T7.2 | HIGH |
| T14.4 | Replace real-sleep tests with TestClock as files are touched | ongoing | HIGH |

### WS-15 — Review loop and cockpit polish (Orca P4)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T15.1 | Annotate-AI-diff-and-ship-back: comment on diff lines, return comments to the agent as steering input; desktop diff renderer first | T5.2 | MED |
| T15.2 | Source-control AI actions (commit-message / PR-body / fix-checks prompts) in the same surface | T15.1 | HIGH |

### WS-16 — Episode 245 completion + docs upkeep (owner-facing ops)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T16.1 | Run the §2.1 rehearsal checklist (fixture smokes; live fleet preconditions; THE decisive casual-prompt rehearsal through the `khala_fleet` MCP bridge); pick shots by what it proves; record | — (owner + supervisor) | LOW |
| T16.2 | Docs upkeep: keep the §1.1 naming disambiguation near the front of public copy; refresh pre-pivot framing notes (fleet spec, porting audit, ops runbook); file the six `codex.app_server.gap.*` items upstream | — | HIGH |
| T16.3 | Public promise record for "Khala Code wraps your Codex" (through `docs/promises/`, copy-gated) | — | LOW (owner-gated copy) |

### WS-17 — Throughput restoration: the clean 2B day (Lane E; final gate)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| T17.1 | Preflight: ≥3 ready accounts, fresh budgets, ≥30 claimable units, `smoke:fleet-run-live` green same-day | T6.10 | supervisor |
| T17.2 | The acceptance run: one chat message, target 15–18, overnight refill window, flags→Inbox; success = ≥2.0B exact tokens/day, ≥15 concurrency ≥6h, duplicate-PR rate 0, 100% closeout coverage, zero unverified merges, Pylon-Claude meaningfully >0 | T17.1 + WS-3/4/5 core | supervisor + owner |
| T17.3 | After-action doc either way; regressions become fixtures; update fleet-management spec status table + cockpit runbook + FleetRun schema doc | T17.2 | HIGH |

## 3. Parallelization Plan (Who Runs What, Concurrently)

Wave structure for fleet delegation (each bullet = independently delegable
lane; lanes inside a wave run in parallel):

- **Wave 0 (now, 6 lanes):** T1.1 · T1.2 · T1.3 · T1.4 · T2.1 · T2.2 —
  plus, independent of everything: T6.4 (fixture app-server), T12.1
  (authority scope), T12.2 (signatures), T12.4 (identity), T12.7, T9.1
  (claude connect), T14.2 (arch scan), T16.2 (docs upkeep). Up to ~13
  concurrent workers with zero interference.
- **Wave 1 (foundations merged):** T3.1 · T4.1 · T4.2 · T4.4 · T6.1 · T6.2 ·
  T7.1 · T8.1 · T10.1 · T2.3 · T13.1.
- **Wave 2:** T3.2 · T3.3 · T3.4 · T4.3 · T5.1–T5.5 (five lanes) · T6.3 ·
  T7.2 · T7.3 · T7.4 · T7.5 · T8.2 · T8.5 · T10.2 · T10.3 · T11.1 · T12.3 ·
  T6.11 · T6.12 · T14.1.
- **Wave 3:** T3.5 · T4.5 · T5.6 · T6.5–T6.9 · T6.13 · T6.14 · T8.3 · T8.4 ·
  T9.2 · T9.4 · T9.5 · T9.6 · T11.2 · T11.3 · T13.2 · T13.3 · T15.1 · T14.3.
- **Wave 4:** T6.10 · T9.3 · T9.7 · T11.4 · T11.5 · T12.5 · T12.6 · T13.4 ·
  T15.2 · T6.15.
- **Gate:** T17.1 → T17.2 → T17.3 (the clean 2B day), with T16.1 (episode
  recording) schedulable any time — it needs nothing from the waves.

Coordination rules (same as epic #7651's fanout): shared seams (T1.1, T2.1,
T2.2, T8.1) land first and alone; everything downstream codes against their
merged interfaces. LOW-delegability tasks stay with the supervising agent or
get the strongest worker plus mandatory supervisor review.

## 4. Milestones

- **M1 — Foundations** (WS-1 + WS-2 merged): typed contracts, live store,
  claims. Everything fans out after this.
- **M2 — Engine + spine**: sustained fixture FleetRun (T3.5), planner with
  typed skips, QA Modes P/D running the seed corpus.
- **M3 — Cockpit + second harness**: cockpit controls live, Claude Phase 1
  chat behind the pill, status spine end-to-end, `/pro` un-mocked.
- **M4 — Proof tier**: monkey night green, sustained live smoke green, TLA+
  supervisor properties checked, perf budgets enforced.
- **M5 — The clean 2B day** (T17.2) with after-action.
- **M6 — Reach**: mobile companion notify/approve/steer, plan-then-fan-out
  crossover, Artanis on the spine with authority scopes, AaaS demo flow.

## 5. Invariants (Merged, Non-Negotiable Across All Workstreams)

- Isolated worker homes for every harness; nothing ever touches `~/.codex`
  or the owner's live `~/.claude`; reconnect flows are per-account isolated.
- One fan-out controller per Pylon; the dispatch gate remains the admission
  authority; advertised capacity refs are respected.
- Exact-only token accounting; public counters are projections of
  `token_usage_events`; progress frames never move counters; counter
  movement alone is never completion evidence.
- Public-safe projections everywhere (cards, flags, lifecycle lines,
  scenarios, traces, screenshots); Rampart/tripwires run in every mode.
- MCP delegation keeps its approval prompt; sustained runs get one approval
  per run-start, never silent standing authority.
- Live test tiers are skip-safe by default and env-armed; fixture tiers
  never spend or claim real work.
- Optimizer/GEPA candidates and formal models inform; they never
  auto-promote; Gym admission + owner approval gate promotion; runtime
  policy is never weakened to make a model pass.
- Artanis risky actions require effective operator-approved gates; authority
  scope gates every action; money stays owner-enveloped.
- No GitHub-hosted CI: Tier-1 bounded pre-push + Tier-2 owned-runner.
- Orca is patterns-only: no vendored code, no name in product surfaces.
- Every landing: full relevant suites + `check:deploy` green; commits from
  clean worktrees; issues close only via merged PRs (see EXECUTION.md).
