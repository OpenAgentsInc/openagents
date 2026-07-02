# Fable Roadmap Execution — Artanis Fleet Process

Date: 2026-07-01
Status: operating procedure. How [`ROADMAP.md`](./ROADMAP.md) gets executed:
a supervising agent temporarily acting as **Artanis, fleet manager**, fanning
work out across the connected fleet with **Khala Code as the primary
delegation mechanism**. This run is deliberately dual-purpose: burn the
roadmap down as fast as possible AND stress-test the Khala Code fleet
management system end-to-end before it goes in front of other users — every
Khala Code bug found is fixed in-flight as first-class roadmap work. This doc
flips no promise state and broadens no public copy.

## 1. Roles

- **Artanis (supervisor)**: the session agent running this process. Files
  and sequences issues, starts and steers fleet delegations, reviews every
  PR before merge, fixes fleet-system bugs as they surface, keeps the
  after-action ledger. Does NOT bulk-implement delegable lanes itself —
  the point is to exercise the fleet.
- **Fleet workers**: isolated Codex (and, once T9.1 lands, Claude) accounts
  under `<pylon home>/accounts/…`, dispatched via Khala → Pylon →
  `codex_agent_task` / `claude_agent_task` with pinned repo/commit/verify.
- **Review subagent**: a tightly-controlled Claude subagent (or Artanis
  directly) that performs final PR review before merge. Review is never
  delegated to the worker that wrote the change.
- **Owner**: approves anything spend-bearing, copy-gated, or
  promise-flipping; runs the on-camera episode-245 segment (T16.1); flips
  `auto_merge_clean` if/when earned.

## 2. Work Item Lifecycle (Non-Negotiable)

1. **Epic + issues.** One GitHub epic for the roadmap execution; one issue
   per ROADMAP task (`Tn.m` in the title), carrying: the source fable doc +
   section, acceptance criteria copied from the roadmap row, deps by issue
   number, the pinned verify command, and the delegability grade. Issues use
   the repo's strict conventions; loose discussion goes to the Forum per
   repo policy.
2. **Claim before work.** No worker is dispatched at an issue that another
   live dispatch already covers (until T2.2's claim registry lands, Artanis
   enforces this manually via the dispatch ledger; after it lands, the
   registry enforces it structurally — dogfooding the June 29 fix).
3. **Worktrees only.** All work happens in fresh worktrees from clean
   `origin/main` on this machine (Pylon's workspace materializer already
   does this for delegated work; Artanis follows the same rule for any
   direct work). Never disturb another agent's dirty checkout.
4. **One PR per issue.** Branch → PR referencing the issue (`Closes #N`).
   The PR body carries the verify-command output. Work on a branch is
   in-progress evidence, never completion.
5. **Verify green, then review.** The pinned verify command must pass in the
   worker's workspace (executor contract) AND the full relevant suites +
   `check:deploy` must be green. Then the review subagent (or Artanis) does
   final review: correctness, invariant compliance (ROADMAP §5), public-safe
   projections, no scope creep, no copy changes.
6. **Merge → close → verify counters.** Merge to `main`, confirm the issue
   auto-closed, confirm the token accounting for the delegated run (see §4).
   Only merged-to-main work counts as done.
7. **After-action on failure.** A worker that dead-ends, duplicates, or
   produces junk gets its claim released and the failure recorded in the run
   ledger; recurring classes become fixtures or Khala Code bug issues.

## 3. Delegation Mechanics (Khala Code First)

Primary path, in order of preference:

1. **Khala Code fleet delegation** — `khala_fleet` MCP / `codex_spawn` /
   the Fleet panel delegate runner, i.e. the deterministic
   `khala.fleet.delegate` program → Pylon assignment → isolated worker home.
   Real-work dispatches always pin `--repo OpenAgentsInc/openagents
   --branch main --commit <sha> --verify "<command>"` and cite the public
   issue number only (public-safe, bounded prompts).
2. **`$PYLON khala request --workflow codex_agent_task`** (the runbook path
   in the repo CLAUDE.md) when steering from the CLI is more practical than
   the desktop.
3. **Claude subagents in-session** only for review, triage, and
   supervisor-adjacent work (per the delegability grades) — not for bulk
   lane implementation, which belongs to the fleet.

Wave discipline comes from ROADMAP §3: seams (T1.1, T2.1, T2.2, T8.1) land
first and alone; then waves fan out wide. Target sustained concurrency rises
with account readiness — this run should push toward the June 29 shape
(15–18 workers) once Wave 1 merges, using the very FleetRun machinery it is
building as soon as T3.x lands (bootstrap: manual refill by Artanis; then
the supervisor takes over — the system takes over its own construction).

**Stress-test posture.** Every fleet-system failure (dead-end, stale
capacity, 409 thrash, dropped lifecycle events, UI desync, duplicate claim)
is itself roadmap work: file it immediately (strict-bug form), fix it in a
scoped PR (often the corresponding WS-3/4/5/6 task moved up), and add the
regression fixture. Do not route around fleet bugs silently — surfacing and
fixing them is half the mission.

## 4. Public Counter Evidence (Every Delegated Run)

Fleet-delegated coding work must increment the public counters at
`openagents.com/stats`. The chain (verified against
`apps/openagents.com/workers/api/src`):

1. Worker turn completes → local Pylon posts to
   `POST /api/pylon/codex/turns` (or `/api/pylon/claude/turns`), handled by
   `pylon-codex-turn-ingest-routes.ts` → one idempotent `token_usage_events`
   row per turn: `provider='pylon-codex-own-capacity'` (or
   `pylon-claude-own-capacity`), `model='openagents/pylon-codex'` (or
   `pylon-claude`), `usage_truth='exact'`, `demand_kind='own_capacity'`,
   `demand_source='khala_coding_delegation'`, `task_ref=<assignmentRef>`.
2. The same handler pushes a live `tokensServedDelta` onto the public sync
   feed (`inference/khala-tokens-served-sync.ts`) — the homepage/`/stats`
   counter moves in real time.
3. `GET /api/public/khala-tokens-served` re-sums the ledger live-at-read;
   `/history`, `/model-mix`, `/channel-mix` feed the `/stats` panels (the
   Pylon-Codex vs Pylon-Claude model mix is where the second lane's share
   becomes publicly visible — a stated Lane E success criterion).

Per-run verification protocol (Artanis, after each closeout):

```sh
# baseline before dispatch, delta after closeout
curl -fsS https://openagents.com/api/public/khala-tokens-served
# authority: exact rows for the assignment (never the counter alone)
$PYLON khala closeout "<assignmentRef>" --json   # closeoutChecklist.ok: true
```

Counter movement alone is never proof (other agents run concurrently);
reconcile the delta against the assignment's exact rows via the closeout
checklist. A run whose token ingest failed is not acceptable evidence —
rerun or debug until the exact rows exist.

## 5. Review Standard (Final Gate Before Merge)

Every PR review checks, in order:

1. Does it do what the issue says — fully, without scope creep?
2. Verify command + full relevant suites + `check:deploy` green (from the
   PR evidence, re-run when in doubt).
3. ROADMAP §5 invariants: no isolated-home violations, exact-only
   accounting untouched or improved, public-safe projections, approval
   prompts intact, live tiers skip-safe, no CI regression, no copy changes.
4. Contract discipline: schema/invariant changes update `INVARIANTS.md` and
   tests in the same PR; parity-pin changes update contract + gap matrix +
   doc together.
5. Cleaner-than-found: pre-existing breakage in touched areas fixed or
   explicitly flagged, never silently stepped around.

Reject → the claim releases with a typed reason and the issue gets a
comment; the worker (or a fresh one) retries against the review notes.

## 6. Run Ledger And Reporting

Artanis maintains, for the duration of the execution run:

- a dispatch ledger (issue ↔ assignmentRef ↔ worker account ↔ state) — in
  the orchestration store once T2.1 lands, scratchpad-file before that;
- periodic public-safe progress updates to the Forum as the registered
  `slug=artanis` Forum identity; the old Raynor-token workaround is retired
  debt, not normal operating practice, and `NEEDS_OWNER.md` entries for
  anything owner-gated;
- the running counter evidence (§4) per merged issue;
- an after-action section per wave: what the fleet system broke, what got
  fixed, what became a fixture — this is the stress-test deliverable that
  gates putting Khala Code in front of other users.

## 7. Stop Conditions

The run ends when ROADMAP milestones M1–M5 are green (M6 items proceed as
capacity allows), or the owner stops it. Owner-gated steps never stall the
run: write the `NEEDS_OWNER.md` entry and keep pulling non-blocked work.
