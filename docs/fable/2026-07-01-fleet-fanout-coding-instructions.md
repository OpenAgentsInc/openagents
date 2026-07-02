# Fleet Fan-Out Through Khala Code — Coding Instructions

Date: 2026-07-01
Status: implementation handoff. A single set of coding instructions that, when
completed, delivers bulletproof fleet delegation steered from Khala Code: one
command spins up an arbitrary number of Codex workers, everything is
visualized and controllable in the app (pause, reboot, flags, account
reconnect, rate limits and usage ticking), every path is programmatically
testable with the rigor of
`2026-07-01-khala-code-desktop-qa-framework-design.md`, and sustained token
throughput returns to and exceeds the June 29 peak — without June 29's
failure modes. Companion analyses: the episode-245 doc (delegation state) and
the Khala Code summary in this folder. This doc flips no promise state and
broadens no public copy.
Execution: the lane/issue breakdown in §10 is consolidated into the unified
[`ROADMAP.md`](./ROADMAP.md); see also the Orca adoption plan (Priority 1:
build Lanes A/B **on** the dormant `apps/pylon/src/orchestration/` store —
one state store, not two) and the Effect integration audit (Phases 1–2 are
the spine the supervisor and cockpit should be built on).

## 0. The Mission, Stated As Outcomes

When you are done, all of the following are true and proven:

1. The owner types one message in Khala Code — e.g. *"run 15 workers against
   the open backlog until it's empty"* — approves one MCP prompt, and a
   sustained fan-out starts: workers spin up to the target concurrency, each
   claims a unique unit of work, and finished slots refill automatically.
2. The Fleet screen shows the whole thing live: every worker with state and
   controls, every account with rate limits/usage counting down and reset
   timers, aggregate tokens/minute and projected tokens/day counting up, and
   every problem (auth expired, blocker, cooldown) as an actionable flag.
3. Every UI control has an RPC method, so the identical run can be started,
   observed, and stopped programmatically, and the QA framework's scenarios
   cover all of it (fixture tier always; live tiers skip-safe).
4. A supervised overnight acceptance run produces ≥ 2B tokens/day of exact
   accounted usage with **zero duplicate PRs** and 100% verified closeouts.

## 1. Learn From June 29 Before Writing Code

The stats page (2026-07-01) tells the story: 06/27 1.9B → 06/28 1.7B →
**06/29 2.4B** (≈2B of it in the midnight–09:00 window, ~18 concurrent Codex
sessions) → 06/30 429.5M → 07/01 ~315M (projected ~500M). Model mix is 90%
Pylon-Codex; **Pylon-Claude has served 2,887 tokens total** — an entire
delegation lane sitting idle.

What made June 29 work:

- High sustained concurrency (~18 sessions) across multiple accounts with
  distinct rate budgets, refilled continuously through the overnight window.
- The refill loop never waited on a human.

What went wrong (read `docs/afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md`
and `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` before coding):

- **Duplicative PRs and wasted work.** Workers picked overlapping targets;
  nothing enforced one-worker-per-work-unit. Dedup was manual and after the
  fact.
- **Config brittleness.** Wrong env vars silently capped a 10-wide fanout at
  3; heartbeat 409s; stale capacity — the exact class the deterministic
  `khala.fleet.delegate` program has since fixed for a *single* dispatch.
- **Shell choreography.** The engine was `/tmp` scripts and operator
  knowledge, invisible to the product, unpausable, unauditable mid-run.
- **Quality unguarded.** Merges were gated by an agent watcher's judgment,
  not typed verification; "green" was not uniformly defined.
- Throughput then *collapsed* after the pivot days — because the loop lived
  in shells and context windows, not in the product. That is the whole
  argument for this work: **the throughput must be a product feature, not an
  operator ritual.**

Design consequences you must honor:

- Concurrency is a *sustained target with refill*, not a one-shot batch.
- Work selection is a *typed planner with claims*, never "each worker greps
  the backlog".
- Every merge rides a *typed verification gate*, not vibes.
- The whole loop is *visible and controllable* in the Fleet screen.

## 2. Substrate Inventory (Do Not Rebuild These)

You are wiring existing engines together, not inventing them:

- **Deterministic dispatch**: `packages/khala-tools/src/fleet-delegate-program.ts`
  (`khala.fleet.delegate`: ensure_pylon → advertise_capacity →
  select_account → prepare_work → dispatch → verify_closeout, with recovery
  loops). Proven; the `0/1 available` class is dead.
- **Batch spawn**: `clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts`
  — `spawnCodexInstances`/`runDelegatedBatchSpawn`, account planning,
  `MAX_SPAWN_COUNT = 10` (a per-call bound, not the concurrency ceiling).
- **Message trigger**: the `khala_fleet` Codex MCP bridge
  (`codex-fleet-mcp-bridge.ts`, `khala-fleet-mcp-server.ts`) exposing
  `pylon_ensure`, `codex_fleet_status`, `codex_spawn` into the default Codex
  chat harness, approval mode `prompt`.
- **Runner-neutral status/control**: `apps/pylon/src/orchestration/`
  (`store.ts` task DAG + dispatch contexts, `status-control.ts` neutral
  states `idle|queued|working|blocked|failed|done|offline`,
  `PYLON_AGENT_RUNNER_CONTROL_VERBS`) — from #7808/#7809. This is your
  pause/reboot substrate.
- **Rate limits/usage**: `codex-rate-limits.ts` (resetsAt, reset
  descriptions, reset credits, `consumeCodexRateLimitResetCredit` RPC),
  `pylon accounts usage`, per-account quota probes.
- **Live lifecycle stream**: Pylon already emits
  `assignment_run_lifecycle_event.v0.1` NDJSON in real time (see
  `docs/khala-code/2026-06-30-codex-spawn-live-progress-streaming-audit.md`
  — the consumer design is written; implement it, do not redesign it).
- **Fleet/Inbox UI seeds**: `src/ui/fleet-status.ts` (delegate form, board
  graph, optimization preview), `fleet-board-projection.ts`, Unified Inbox
  routing (`approval_required`, `run_blocked`, `ready_for_review`).
- **Queue planning prior art**: epic #7590 lanes (#7595 GitHub queue planner
  with dedupe/skip reasons, #7592 cooldowns, #7593 reconciler, #7597 resume
  checks) and `khala-burndown.ts` (currently Codex-hard-coded).
- **Exact accounting**: `token_usage_events` ingest, tokens/min projection
  with `exact | pending | not_measured` honesty in `codex_fleet_status`.
- **Claude lane (~80% parity)**: `apps/pylon/src/claude-agent-executor.ts`,
  `claude_agent_task` accepted by server/CLI/MCP, per-account capacity refs,
  supervisor — missing connect UX and fleet-tool integration (see the
  episode-245 doc §3.1).
- **QA substrate**: everything in
  `2026-07-01-khala-code-desktop-qa-framework-design.md` — the four access
  modes, scenario DSL, oracles, fixture Codex app-server, qa-runner.

Read first, in order: the two docs above, `docs/khala-code/*` (all),
`apps/openagents.com/INVARIANTS.md` ("Khala Coding Delegation Through
Pylons"), `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`.

## 3. Lane A — One Command, Sustained Fan-Out

**Goal**: a single chat message or one Fleet-panel action starts a *standing
run*: target concurrency N (arbitrary, not capped at 10), a work source, and
a refill policy; it runs until the backlog is empty or the owner stops it.

Build:

1. **A typed FleetRun record** (Effect Schema,
   `openagents.khala_code.fleet_run.v1`): `{ runRef, objective, workSource
   (github_backlog | issue_list | fixture), targetConcurrency, workerKind:
   "codex" | "claude" | "auto", refillPolicy { maxPerAccount, cooldownAware,
   stopCondition }, state: draft|running|paused|draining|stopped|completed,
   startedAt, counters }`. Persist under the desktop's owner-local state dir
   (same pattern as `~/.khala-code/codex-sessions.json`) and in the Pylon
   orchestration store so both survive restarts and reconcile
   (#7593/#7597 patterns). Per the Orca adoption plan's Priority 1, the
   Pylon side of this record **is** the dormant
   `apps/pylon/src/orchestration/` store (tasks = work units, dispatch
   contexts = workers, messages = lifecycle bus, VirtualHead = merge-queue
   base pinning) — wire it, do not build a second state model. Adopt Orca's
   handoff vs supervised-dispatch taxonomy: `codex_spawn` one-shots are
   handoffs (untracked), FleetRuns are supervised dispatches (DAG-tracked).
2. **A FleetRunSupervisor in the Bun host** (Effect, `Scope`d): the promoted
   watcher loop. Tick: read run state → count active assignments → if below
   target and work remains, claim next work unit (Lane B) → dispatch one
   `khala.fleet.delegate` bundle per free slot (respecting advertised
   per-account capacity and cooldowns from `codex-rate-limits`) → stream
   lifecycle events into the run's counters. `MAX_SPAWN_COUNT` stays as the
   per-tick batch bound; the supervisor achieves arbitrary N by refilling
   across ticks. One supervisor per Pylon (the one-fanout-controller
   invariant); refuse to start a second run against the same Pylon.
3. **Chat entry**: extend the `khala_fleet` MCP server with
   `fleet_run_start`, `fleet_run_status`, `fleet_run_control`
   (pause/resume/drain/stop) so the single casual message works in default
   Codex mode. Keep approval mode `prompt`. `codex_spawn` stays as the
   bounded one-shot; the new verbs own sustained mode.
4. **Fleet panel entry**: The
   Fleet panel renders a "Start fleet run" form beside the existing delegate
   runner: objective, work source, target concurrency, worker kind
   (`codex | claude | auto` accepted in the UI; `codex` wired to
   `fleetRunStart`), and a public-safe dry-run preview showing the planned
   first wave (accounts × slots × first claims) before starting.
5. **RPC parity**: `fleetRunStart`, `fleetRunStatus`, `fleetRunControl`,
   `fleetRunList` methods; everything the UI can do, the bridge can do.

Acceptance:

- Fixture tier: a fixture FleetRun with target 25 on a mocked Pylon runner
  reaches 25 simulated concurrent assignments through refill ticks, then
  drains cleanly; state survives a host restart (reconcile test).
- Live tier (skip-safe, env-armed): target 2 against the real fleet produces
  2 concurrent real assignments with accepted closeouts and exact token rows.
- The MCP verbs round-trip: a scripted app-server fixture calls
  `fleet_run_start` and status shows `running`.

## 4. Lane B — Work Planner With Claims (Kill The Duplicate-PR Class)

**Goal**: no two workers ever do the same work; no worker does dead work;
nothing merges unverified. This is the June 29 fix.

Build:

1. **A claim registry** (`openagents.khala_code.work_claim.v1`), persisted in
   the Pylon orchestration store: `{ claimRef, workUnitRef (e.g.
   issue#/PR#/task id), runRef, assignmentRef, workerAccountRef, state:
   claimed|in_progress|closeout|released|expired, ttl, claimedAt }`.
   Invariant: **at most one live claim per workUnitRef** (enforce with a
   unique key, not a scan). Claims expire on TTL or worker death (reconciler
   releases them — reuse #7593's lease-freshness pattern).
2. **A typed work planner** feeding the supervisor: source adapters
   (`github_backlog` via `gh` listing open issues/PRs, `issue_list` explicit,
   `fixture`), each emitting candidate work units with **typed skip
   reasons** (`already_claimed`, `pr_exists`, `merged`, `closed`,
   `needs_owner`, `label_excluded`) — port the #7595 planner semantics into
   the shared package rather than the retired shell loop. The planner output
   is data; log every skip with its reason (no silent drops).
3. **Prompt/pin discipline**: every real-work dispatch carries pinned `repo`,
   `commit`, `branch`, and a named `verify` command (existing
   `prepare_work` fields) plus the claimRef; the worker prompt cites the
   public issue number and the claim, and instructs the standard PR
   convention. Fixture mode never claims real units.
4. **The verification gate**: a closeout is `ready_for_review` only if the
   pinned verify command ran green in the worker's workspace (already in the
   executor contract) AND the claim is still held. A **merge policy** module
   decides what happens next, as typed policy not agent vibes:
   `manual_review` (default), `auto_merge_clean` (owner-toggled: mergeable +
   verify green + no conflicts + diff within scope). Conflicted siblings go
   to a **merge-wave resolver**: one dedicated worker per wave that
   rebases/tests/merges sequentially (the runbook's proven pattern, now a
   supervised job with its own claim).
5. **De-Codex-name the shared tooling** while you are in these files:
   `khala-burndown.ts` and the fleet tools take workerKind instead of
   hard-coding `codex_agent_task` (accept `claude_agent_task` where the
   executor parity already supports it).

Acceptance:

- Property test (fast-check): N concurrent claim attempts over M work units
  never yield two live claims on one unit, across interleavings and
  expiries.
- Fixture run: a 10-unit backlog with 6 workers completes with exactly 10
  claims, 0 duplicates, and every skip typed.
- Live tier: two workers pointed at a 2-issue public backlog produce two
  distinct PRs referencing distinct issues, each with verify-green evidence
  in the closeout.
- Regression: a synthetic "duplicate temptation" fixture (two workers, one
  juicy issue) shows the second worker skipping with `already_claimed`.

## 5. Lane C — The Fleet Screen Becomes A Cockpit

**Goal**: "get a whole sense of the thing" at a glance, and control it.

Build, on the existing `fleet-status.ts`/`fleet-board-projection.ts` — and
per the Orca adoption plan's Priority 2, make
`agent_runner_status_event.v1` the one status vocabulary the cockpit
consumes (runner → orchestration store → cockpit → Worker → mobile),
instead of extending bespoke `codexFleetStatus` shapes. The mobile
companion (Orca plan Priority 3) is a projection of exactly these cards
and flags; keep every projection public-safe with that in mind.

1. **Run header**: the active FleetRun with state, objective, target vs
   actual concurrency, backlog remaining/claimed/done, elapsed, and controls
   — **Pause** (stop claiming, keep active work), **Resume**, **Drain**
   (finish active, claim nothing), **Stop** (interrupt actives via
   `turn/interrupt`-equivalent + release claims), wired through
   `fleetRunControl` to the orchestration control verbs.
2. **Worker cards**: one card per active assignment — neutral state from
   `status-control.ts`, claimed work unit, elapsed, live lifecycle line
   (implement the streaming consumer per the 2026-06-30 audit: Effect
   `Stream` over the Pylon NDJSON channel → throttled card updates),
   tokens (exact/pending/not_measured), and per-worker controls:
   **Interrupt**, **Retry** (release claim + redispatch), **Flag** (push an
   Inbox item with a note). No fabricated progress — render only real
   lifecycle frames.
3. **Account cards**: per connected account — readiness, slots busy/free,
   **rate-limit meters counting down** (used/remaining + `resetsAt`
   countdown from `codex-rate-limits.ts`, reset credits with the existing
   consume RPC), cooldown state, and **Reconnect** when auth is expired:
   an Inbox `credentials_missing` flag plus a card action that launches the
   isolated-home device-login flow (`connectCodexAccount` — never the
   primary `~/.codex` home). Add **Pause account** (exclude from planning
   without disconnecting).
4. **Throughput gauges**: aggregate tokens/min (exact-row based, honest
   states), run-total tokens, and **projected tokens/day** at current pace —
   the in-app mirror of the stats page bar the owner watches. Reconcile
   against `GET /api/public/khala-tokens-served` deltas in the live smoke,
   never synthesize from progress frames.
5. **Flags → Inbox**: every `run_blocked`, `approval_required`,
   `credentials_missing`, `cooldown_all_accounts`, `merge_conflict_wave`,
   and `claim_expired` becomes an Inbox row with its allowed responses and a
   resume hook back into the run. The condensed sidebar Fleet entry shows
   the counts (accounts ready, workers active, slots free, flags) so the
   episode-245 italic shot works.

Acceptance:

- Fixture scenario per control (pause/resume/drain/stop/interrupt/retry/
  reconnect/pause-account) with consistency oracles: RPC state ≡ card state.
- Visual smoke extension of the part2 pattern: cockpit renders with 18
  fixture workers across 3 accounts without geometry violations, desktop +
  mobile.
- The rate-limit countdown ticks against a fixture clock (TestClock — no
  real sleeps).

## 6. Lane D — Bridge, Programmatic Steering, And QA Rigor

Apply the QA framework doc's spine to everything above (its G-gaps are
prerequisites; implement them here if not already landed):

1. **G1**: preview bridge auth (loopback bearer) + `GET /rpc/events` SSE
   carrying chat turn events, fleet lifecycle events, run counters, console
   errors. The cockpit and the QA driver consume the same stream.
2. **G2**: typed `KhalaCodeRpcClient` covering the new fleet-run methods;
   schema oracle on every response.
3. **Scenario corpus** (fixture tier, runs pre-push): fleet-run lifecycle,
   claim dedupe, every cockpit control, account reconnect flow, throughput
   gauge honesty (`pending`/`not_measured` render), Inbox flag routing.
   Every phase has expectations; commitments ride to the verifier.
4. **Seeded monkey night** over the cockpit with 18-worker fixture state:
   thousands of random control interactions; oracles = no console errors,
   no state desync, no claim invariant violation, public-safe DOM.
5. **Live tiers** (skip-safe, env-armed like `smoke:codex-parity-live`):
   `smoke:fleet-run-live` (target 2, real accounts, real closeouts, counter
   reconciliation) and `smoke:fleet-run-sustained` (target ≥ 5 for ≥ 30
   minutes, refill observed ≥ 2 times, zero duplicate claims).
6. **Formal tier**: extend the `khala.fleet.delegate` TLA+ spec (QA doc
   §9.3) with the supervisor: properties — active assignments never exceed
   advertised capacity, a paused run claims nothing, drain terminates, and
   claim uniqueness holds under concurrent supervisors racing one Pylon
   (should be *prevented*; the spec proves the guard, and the
   counterexample becomes a fixture if it ever isn't).
7. **Perf budgets**: cockpit render < 100ms with 50 worker cards; lifecycle
   event → card update p95 < 500ms; supervisor tick < 1s at target 25.

## 7. Lane E — Throughput Restoration (The Clean 2B Day)

The final acceptance is operational, run by the owner with you supervising:

1. Preflight: `khala fleet status` shows ≥ 3 ready accounts; rate-limit
   meters show fresh budgets; backlog source lists ≥ 30 claimable units;
   `smoke:fleet-run-live` green the same day.
2. Start from chat: one message, target 15–18, `auto_merge_clean` off for
   the first hour (watch the review queue), on after spot-checking.
3. Overnight window (the June 29 shape, now supervised): the supervisor
   refills through the midnight–09:00 window; cooldowns rotate accounts;
   flags land in Inbox instead of dying in a terminal.
4. Success criteria, measured from exact rows and closeouts (not the public
   counter alone):
   - tokens/day ≥ 2.0B with `usage_truth='exact'` rows covering it;
   - sustained concurrency ≥ 15 for ≥ 6 hours;
   - **duplicate-PR rate = 0** (every PR ↔ exactly one claim ↔ one work
     unit);
   - closeout coverage 100% (no orphaned assignments);
   - zero unverified merges; merge-wave resolved conflicts without human
     rebase;
   - Pylon-Claude > 0 meaningfully if Lane B's workerKind landed (even a
     10% Claude share proves the second lane).
5. Write the after-action doc either way; regressions become fixtures.

## 8. Definition Of Done (The Characteristics)

Check every box before calling this complete:

- [ ] One chat message starts a sustained N-worker run (N arbitrary, refilled).
- [ ] One Fleet-panel action does the same, with a dry-run preview.
- [ ] Pause / Resume / Drain / Stop / per-worker Interrupt / Retry all work
      from UI and RPC, and are scenario-tested.
- [ ] Account reconnect (expired auth) is a two-click Inbox→device-login
      flow that never touches `~/.codex`.
- [ ] Rate limits and usage tick down live; tokens/min and projected
      tokens/day tick up live; all with exact/pending/not_measured honesty.
- [ ] The claim registry makes duplicate work structurally impossible, with
      a property test and a live regression fixture proving it.
- [ ] Every merge is gated by typed verification; auto-merge is a visible,
      owner-toggled policy.
- [ ] Every UI capability has an RPC method, a schema, and a fixture
      scenario; the bridge has auth + an event stream.
- [ ] The monkey night and the sustained live smoke pass; the TLA+
      supervisor properties check green.
- [ ] The clean-2B-day acceptance run (§7) has happened, with evidence.
- [ ] Docs: cockpit runbook, FleetRun schema doc, and updates to the
      fleet-management spec status table. `check:deploy` and the full
      relevant suites green at every landing; every lane commits to `main`
      from a clean worktree.

## 9. Invariants (Non-Negotiable While Building)

- Isolated worker homes only; nothing ever runs `codex login` against the
  default home; reconnect flows are per-account isolated.
- One fan-out controller per Pylon; the supervisor respects advertised
  capacity refs; the dispatch gate remains the admission authority.
- Exact-only token accounting; public counters are projections; progress
  frames never move counters.
- Public-safe projections everywhere: no raw prompts, paths, tokens, or
  provider payloads in cards, flags, lifecycle lines, scenarios, or traces.
- MCP delegation keeps its approval prompt; sustained runs get *one*
  approval per run-start, not silent standing authority.
- Live test tiers are skip-safe by default and env-armed; fixture tiers
  never spend or claim real work.
- Optimizer candidates (refill/routing parameters are GEPA targets later)
  never auto-promote; owner-gated admission via the existing Gym path.
- No GitHub-hosted CI; Tier-1 pre-push + Tier-2 owned-runner patterns.

## 10. Suggested Issue Breakdown

File as one epic ("Fleet fan-out cockpit: sustained runs, claims, controls,
throughput") with lanes matching sections: A1 FleetRun schema/store, A2
supervisor + refill, A3 MCP verbs + chat entry, A4 panel entry + RPC; B1
claim registry + property tests, B2 planner + skip reasons, B3 verification
gate + merge policy + merge-wave, B4 workerKind generalization; C1 run
header + controls, C2 worker cards + lifecycle streaming, C3 account cards +
rate limits + reconnect, C4 gauges, C5 Inbox flags + condensed sidebar; D1
bridge auth+SSE, D2 RPC client + scenarios, D3 monkey night, D4 live smokes,
D5 TLA+ supervisor spec, D6 perf budgets; E1 acceptance run + after-action.
Lanes A1/B1 are the shared seams — land them first; C and D lanes fan out
cleanly against their interfaces (same coordination rule as the Codex-port
epic #7651). Every lane lands with tests green, committed and pushed to
`main` from a clean worktree, and closes its issue with proof.
