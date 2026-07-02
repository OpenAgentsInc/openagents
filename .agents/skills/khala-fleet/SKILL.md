---
name: khala-fleet
description: Operate an OpenAgents Khala coding fleet safely. Use when the user asks to connect or list fleet accounts, spawn/delegate coding work to Codex or Claude workers, start/monitor/pause/drain/stop a sustained fleet run, burn down a backlog with parallel workers, verify that fleet work actually completed, or diagnose dispatch failures like "0/1 available" or target_pylon_unavailable.
metadata:
  short-description: Operate an OpenAgents Khala coding fleet safely.
---

<!-- managed-by: khala-code; skill: khala-fleet; version: 1 -->

# Khala Fleet Management

A "fleet" is the owner's linked coding accounts (Codex and/or Claude), each
running as an ISOLATED local worker reached through Khala -> Pylon ->
assignment, coordinated from one surface (Khala Code Desktop, the `khala`
CLI, or the `khala_fleet` MCP tools). This skill encodes the operating
procedure and the non-negotiable guardrails. It is a launcher, not the law:
the canonical runbooks below win whenever they disagree with this summary.

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

## Dispatch ladder (smallest sufficient rung first)

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

2. **A parallel wave** — publish capacity first
   (`OPENAGENTS_PYLON_CODEX_CONCURRENCY=N ... $PYLON presence heartbeat`),
   then one request per work unit, each with its own claim, pinned refs,
   and verify command. Never exceed advertised availability.

3. **A sustained fleet run** — `fleet_run_start` (MCP or the Fleet panel)
   with objective, work source, target concurrency, and `workerKind`
   (`codex | claude | auto`). Monitor with `fleet_run_status`; steer with
   `fleet_run_control` (`pause | resume | drain | stop`). One supervisor
   per Pylon; refill takes the next unclaimed unit as slots free up.

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

## Hard guardrails (never violate, even under time pressure)

- **NEVER run `codex login` (or any auth flow) against the default
  `~/.codex` home, and never touch the owner's live `~/.claude`.** Login
  flows clear live credentials at flow start. Worker auth always uses
  isolated per-account homes; to inspect accounts, list them — never
  re-login to "check".
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
