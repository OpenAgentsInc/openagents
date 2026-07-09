---
name: khala-fleet
description: Operate an OpenAgents Khala coding fleet safely. Use when the user asks to connect or list fleet accounts, spawn/delegate coding work to Codex, Claude, or Grok workers, start/monitor/pause/drain/stop a sustained fleet run, burn down a backlog with parallel workers, verify that fleet work actually completed, or diagnose dispatch failures like "0/1 available" or target_pylon_unavailable.
metadata:
  short-description: Operate an OpenAgents Khala coding fleet safely.
---

<!-- managed-by: khala-code; skill: khala-fleet; version: 2 -->

# Khala Fleet Management

A "fleet" is the owner's linked coding accounts (**Codex, Claude, and now
Grok**), each running as an ISOLATED local worker reached through Khala ->
Pylon -> assignment, coordinated from one surface (Khala Code Desktop, the
`khala` CLI, the `khala_fleet` MCP tools, or — for Grok specifically, see
§Grok below — a direct local script). This skill encodes the operating
procedure and the non-negotiable guardrails. It is a launcher, not the law:
the canonical runbooks below win whenever they disagree with this summary.

**Harness reality check (2026-07-09) — read before promising a capability:**

| Harness | One-shot dispatch | Sustained fleet run | Exact token accounting |
|---|---|---|---|
| `codex` | ✅ full pipeline (`khala request` / `codex_spawn`) | ✅ `fleet_run_start` | ✅ exact `token_usage_events` rows |
| `claude` | ✅ full pipeline | ✅ `fleet_run_start` | ✅ near-parity (MH-2, 2026-07-08) |
| `grok` | ✅ real dispatch via `codex_spawn worker_kind=grok` or a direct local script (§Grok) | ✅ mixed-kind `fleet_run_start` (MH-5) | ❌ **not yet** — see §Grok token-accounting gap |

Do not claim Grok work "shows up in the token counter" — it doesn't yet,
by honest design (never synthesize tokens). Tell the user this plainly if
they ask.

## Canonical sources (read before non-trivial fleet work)

In the `OpenAgentsInc/openagents` repo (or https://openagents.com/AGENTS.md
when working outside the repo):

- `AGENTS.md` / `CLAUDE.md` — "Help a user connect their Codex fleet to
  Khala" and the "Khala -> Pylon -> Codex Coding Delegation Runbook"
  (the request/proof contract, SQL verification, failure signatures).
- `docs/ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md` — running
  the engine 24/7: standing pylon, supervisor, identity/token footguns,
  stall diagnosis.
- `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` — multi-account
  fan-out, refill loops, merge waves, token proof at scale.
- `docs/khala-code/2026-06-30-khala-code-fleet-management-spec.md` — the
  product capability map (Inbox, fleet board, worker cards, claims).
- `docs/fable/EXECUTION.md` — issue/PR/worktree/review discipline for work
  the fleet produces.

## The fleet model in one minute

- **Accounts are workers.** Each connected account lives in an isolated home
  (`<pylon home>/accounts/codex/<ref>`, `.claude-*` via `CLAUDE_CONFIG_DIR`).
  Distinct provider accounts have distinct rate budgets: more accounts =
  real added concurrency.
- **Pylon is the executor.** A local Pylon advertises capacity via presence
  heartbeats (`capacity.coding.codex.available=N`, busy, queued, ready);
  the server dispatch gate admits at most the advertised free slots.
- **Assignments are the work unit.** A typed request (`codex_agent_task` /
  `claude_agent_task`) produces an assignment with a lease, lifecycle
  events, a closeout checklist, and exact token rows. Fleet runs are
  supervised loops that keep N assignments in flight with refill.
- **Claims prevent duplicate work.** At most one live claim per work unit;
  claim before dispatch, release on closeout; skips are typed
  (`already_claimed`, `pr_exists`, `merged`, `closed`, `needs_owner`).

## Connect and inventory

```sh
npm install -g @openagentsinc/khala   # Node 20+ or Bun
khala fleet connect                   # paste-free device login; isolated home
khala fleet status                    # table: ref, readiness, email
```

- Each `khala fleet connect` auto-assigns the next ref (`codex`, `codex-2`,
  ...); `--account <ref>` names one; `--harness claude` connects a Claude
  account the same isolated way.
- Requires the provider CLI (`npm install -g @openai/codex`); the connect
  flow prints an install hint if missing.
- Inventory before routing work: every account you intend to use must show
  `ready`. From a Pylon checkout: `$PYLON codex accounts list --json`.
- **Grok has no `khala fleet connect` flow yet** — it uses whatever local
  `grok` CLI login already exists (`grok login`, or `XAI_API_KEY` in env).
  Check readiness directly: `grok version && grok models` (or
  `probeGrokReadiness()` from `@openagentsinc/grok-harness` — see §Grok).
  Two auth planes, never conflate them: `cli_session` (the local
  `grok login`/grok.com session — currently free for us, verify weekly, it
  is a time-limited window not a permanent economics input) vs `api_key`
  (`XAI_API_KEY`, published metered pricing). `probeGrokReadiness()` reports
  which plane is active.

## Dispatch ladder (smallest sufficient rung first)

0. **Local, no-app, ASAP rung (Codex/Claude via CLI; Grok via script) —
   use this to start fanning work out RIGHT NOW without waiting on desktop
   app / MCP wiring.** For Codex/Claude this is just rung 1 below run from
   a terminal. For Grok, see §Grok — there is no CLI dispatch yet, so the
   local path is a small script importing `@openagentsinc/grok-harness`
   directly. This rung is for a single bounded task or a hand-rolled loop
   over a short backlog; move to rung 2/3 once you need real claim-registry
   coordination across many workers.

1. **One bounded task** — `codex_spawn` (MCP) or:

   ```sh
   $PYLON khala request \
     --prompt "<public-safe bounded objective>" \
     --workflow codex_agent_task \
     --pylon-ref "<owner pylon ref>" \
     --repo <org>/<repo> --branch main --commit "<pinned sha>" \
     --verify "<pinned verification command>" \
     --json
   ```

   Use `--fixture` for a no-spend proof run before real work. If the
   response has no delegation frame (it fell through to normal model
   routing), STOP and fix preconditions; do not run spendful work.
   `$PYLON khala request --workflow` accepts `codex_agent_task |
   claude_agent_task | cloud_coding_session` today — **`grok_agent_task`
   does not exist yet** (that typed pipeline is unbuilt; use §Grok instead).

2. **A parallel wave** — publish capacity first
   (`OPENAGENTS_PYLON_CODEX_CONCURRENCY=N ... $PYLON presence heartbeat`),
   then one request per work unit, each with its own claim, pinned refs,
   and verify command. Never exceed advertised availability.

3. **A sustained fleet run** — `fleet_run_start` (MCP or the Fleet panel)
   with objective, work source, target concurrency, and `workerKind`
   (`codex | claude | grok | auto`). Monitor with `fleet_run_status`; steer
   with `fleet_run_control` (`pause | resume | drain | stop`). One
   supervisor per Pylon; refill takes the next unclaimed unit as slots free
   up. Grok participates in a mixed pool for real (MH-5, 2026-07-08); if a
   run explicitly requests `workerKind: grok` and no Grok executor path is
   reachable, it fails closed with a typed skip event — it is never
   silently downgraded to codex.

Preconditions for every rung: `pylon_ensure` (or `$PYLON provider go-online`
+ `presence heartbeat`) succeeded, heartbeat is fresh, capacity refs are
published, and `codex_fleet_status` shows ready accounts with free slots.
The June-2026 "0/1 available" dead-end class is almost always capacity that
was never advertised — heartbeat first, then dispatch.

## Work-unit hygiene

- One work unit = one claim = one issue = one PR. Search open AND
  recently-closed issues/PRs before claiming; lower issue number wins races.
- Every real-work dispatch pins repo, commit, branch, and verify command,
  and cites the issue + claim in the worker prompt.
- Worker prompts are public-safe and bounded: public issue numbers, public
  paths, public verification commands. Never include raw transcripts,
  secrets, local paths, provider payloads, or wallet material.
- Branch work is in-progress evidence, not completion. An issue closes only
  after the PR merges to the owning repo's default branch and required
  verification ran from the integrated state.

## Verification (what "done" means)

- **Closeout checklist first**: `$PYLON khala closeout "<assignmentRef>"
  --json` must report `closeoutChecklist.ok: true` — trace/proof
  projections agree, exact own-capacity token rows exist, no-spend runs
  prove `paymentMode: "no-spend"` and `payoutClaimAllowed: false`.
- **Exact token rows are the accounting truth**: one `token_usage_events`
  row per completed SDK turn (`usage_truth = 'exact'`,
  `demand_source = 'khala_coding_delegation'`, `task_ref = assignmentRef`).
  Public counters are projections of those rows.
- **Counter movement alone is NEVER completion evidence.** Reconcile the
  public counter delta against the exact rows; other agents may be running.
- A failed or missing token-ingest row is not acceptable proof; rerun or
  debug until the exact row exists. Interrupted local runs submit a typed
  stale closeout before claiming new work.

## Diagnosis: common failure signatures

- `target_pylon_not_authorized` — the token does not own/link that Pylon.
- `target_pylon_unavailable` — Pylon not active, heartbeat stale, not
  capable, or no free advertised capacity. Re-run heartbeat; check
  `capacity.coding.<kind>.available=N` vs active assignment rows.
- Provider error about unexpected `openagents` inputs — delegation did not
  happen; the request fell through to normal routing. Re-check
  `--workflow`, target freshness, caller ownership. Stop before spend.
- Second dispatch refused below advertised capacity — inspect assignment
  rows for non-expired stale leases; check whether their local process is
  alive before creating more requests.
- Long silent run — inspect lifecycle events / raw event chunks before
  assuming progress; do not stack more dispatches on a wedged worker.

## Grok as a fleet worker (real, tested 2026-07-09 — read before promising more than this)

Grok (`grok` CLI, `@openagentsinc/grok-harness`) is a real third harness
alongside Codex and Claude, per the multi-harness program
(`docs/fable/2026-07-08-multi-harness-parallelization-effect-native-analysis.md`,
MASTER_ROADMAP §MH). MH-0 (schema literals), MH-5 (mixed-kind fleet claims),
and the `codex_spawn worker_kind=grok` dispatch path are landed and real —
not mocked. This was verified live tonight: a real bounded prompt dispatched
through the exact production code path (`createGrokHeadlessWorkerExecutor`
→ real `grok` CLI subprocess) returned a real response in ~9.5s.

### Two ways to dispatch to Grok today

**A. Local, no app needed (fastest "start fanning out now" path):**

```ts
import { createGrokHeadlessWorkerExecutor, probeGrokReadiness } from "@openagentsinc/grok-harness"

const readiness = await probeGrokReadiness({})
// { ready: true, binary: "grok", version: "...", plane: "cli_session" | "api_key", models: [...] }

const executor = createGrokHeadlessWorkerExecutor({})
const closeout = await executor.runClaimedWork({
  pin: {
    claimRef: "<unique-ref>",
    workUnitRef: "<issue-or-task-ref>",
    runRef: "<your-run-name>",
    cwd: "<worktree-path>",
    repo: "<org>/<repo>",       // optional, pinned
    commit: "<sha>",            // optional, pinned
    branch: "main",             // optional, pinned
    verifyCommand: "<argv>",    // optional, pinned
  },
  prompt: "<public-safe bounded objective>",
  plane: "cli_session",         // or "api_key" if XAI_API_KEY is set
  marginalCostClass: "free",    // or "api_metered" — never hard-code "free" as a permanent fact
})
// closeout: { ok, claimRef, stopReason, text, usage: { metering: "not_measured", wallClockMs, plane, marginalCostClass }, failureClass? }
```

Run it from inside the `openagents` workspace (workspace-resolvable import)
via `bun run <script>.ts`. This is the same executor the product uses — it
is not a toy path. It shells out to
`grok --no-auto-update --no-alt-screen -p "<prompt>" --cwd <dir>
--output-format plain`, so a real `grok` login/session is required
(`probeGrokReadiness` tells you which auth plane is active).

**B. Through the product (desktop MCP `khala_fleet` tools):** `codex_spawn`
with `worker_kind: "grok"` and `fleet_run_start` with `workerKind: "grok"`
(or `"auto"`, which includes Grok in the pool) both dispatch through the
identical executor — use this when you want claim-registry coordination,
mixed-kind runs, or Sync-visible fleet state, not just a one-off local
script.

### The token-accounting gap (be honest about this, do not paper over it)

Every Grok closeout reports `usage.metering: "not_measured"` — by design,
per the metering-honesty law (never synthesize/invent tokens).

**Groundwork landed (2026-07-09):** `not_measured` is now a first-class
`usage_truth` value in the ledger schema (`@openagentsinc/sync-schema`
`TokenUsageTruth`), and the public `khala-tokens-served` counter EXCLUDES
`usage_truth = 'not_measured'` rows explicitly — both the read-side SUM /
exact-total reconcile source and the write-side daily/model/channel rollups
skip them (`workers/api/src/token-usage-ledger.ts`,
`publicTokensServedDemandWhere`). So a `not_measured`-truth Grok row can exist
as an honest zero-token accounting trail WITHOUT ever polluting the exact
public served-token total. The exact-only counter law is preserved.

**Still unbuilt — the producer:** **no code path yet posts a Grok closeout
into `token_usage_events`.** The two dispatch sites
(`clients/khala-code-desktop/src/bun/khala-fleet-tools.ts` `codex_spawn
worker_kind=grok`, and `fleet-run-supervisor-rpc-adapter.ts`) run the `grok`
CLI **locally in the desktop** and return a real `WorkerCloseout`, but do not
emit a ledger row. This means:

- Grok work does **not** move the public `khala-tokens-served` counter
  (correct — `not_measured` never would), and produces **no** row in
  `token_usage_events` yet, so there is still no accounting-side audit trail
  for Grok fleet activity (only the closeout's own `text` / `stopReason` /
  `wallClockMs`, which the caller must capture itself).
- The blocker is auth/route, not schema: the canonical ledger ingest route
  (`POST` handled by `handleTokenUsageEventsApi`) requires an **admin API
  token** the desktop does not hold. Codex/Claude fleet tokens reach the
  ledger via a Pylon **registered-agent** route (`POST /api/pylon/codex/turns`,
  `requireAgent` bearer); Grok runs outside Pylon and has no analogous
  registered-agent ingest route. Closing the trail = building that
  server-side registered-agent Grok ingest route (which inserts a
  `not_measured`-truth, zero-token row with `provider:
  "grok-cli-own-capacity"`, `demand_kind: "own_capacity"`, and
  wall-clock/plane/marginal-cost in `safeMetadata`) plus the desktop client
  that posts to it with the agent token — NOT wiring an admin POST from the
  desktop.
- Rate-limit probes (RL-1..6, part of MH-4 `#8590`, still open) are the
  next piece that turns wall-clock + failure-class data into something the
  `auto` policy (MH-8, closed) can actually rank Grok against Codex/Claude
  on cost. Until the producer route lands, treat Grok capacity as "real but
  unmetered."

Say this plainly rather than implying the counter already tracks Grok, or
that the row-posting is already built.

## Hard guardrails (never violate, even under time pressure)

- **NEVER run `codex login` (or any auth flow) against the default
  `~/.codex` home, and never touch the owner's live `~/.claude`.** Login
  flows clear live credentials at flow start. Worker auth always uses
  isolated per-account homes; to inspect accounts, list them — never
  re-login to "check". **Same rule for Grok**: never run `grok login`
  against the owner's default local session just to "check" readiness —
  use `probeGrokReadiness()` / `grok version` / `grok models`, which are
  read-only.
- Exact-only token accounting: no synthesized counters, no progress-frame
  counting, rates reported `pending`/`not_measured` rather than fabricated.
- MCP delegation keeps its approval prompt; a sustained run gets one
  approval at run-start — never silent standing authority.
- Respect advertised capacity; one fan-out supervisor per Pylon; the
  dispatch gate is the admission authority.
- Fixture/no-spend tiers never spend or claim real work; live smokes are
  env-armed and skip-safe by default.
- Never weaken a gate, test, or policy to make dispatch or closeout pass.
- Public-safe projections everywhere; raw worker events stay in owner-scoped
  private storage.
