# Khala CLI Spawn Stress Reliability Report

Date: 2026-06-27

Scope: installed `khala` CLI stress after the supervised spawn release. The run
used a clean `origin/main` worktree at `8458828726` and a patched local
`khala 0.1.18` build before publish.

## Summary

`khala` is reliable today for public chat volume, stored-login authenticated API
traffic, local supervised subagent spawning, natural-language spawn routing, and
local spawn inspection commands.

The remote Pylon spawn path is reliable as a bounded assignment-allocation and
status-projection surface, but not yet reliable as a completed multi-subagent
execution surface under live stress. The live test produced capacity shortfall
and target-unavailable blockers even after a fresh Pylon heartbeat.

## Live Stress Results

### Public CLI volume

- Command shape: `khala --headless --json --prompt ...`
- Volume: 48 public requests.
- Concurrency: 8.
- Result: 48 succeeded, 0 failed.
- Latency: min 6.677s, p50 8.916s, p90 10.421s, max 11.682s.
- Average response size: 89 characters.

### Authenticated API volume

Initial ambient-token batch:

- Command shape: `khala --api --headless --json --prompt ...`
- Volume: 24 requests.
- Concurrency: 6.
- Result: 0 succeeded, 24 failed.
- Failure: `unauthorized`.
- Root cause: the shell had an ambient `OPENAGENTS_AGENT_TOKEN` that the server
  rejected. This exposed token-resolution bugs fixed in `0.1.18`.

Stored-login batch after the fix:

- Command shape: `khala --api --headless --json --prompt ...`
- Volume: 24 requests.
- Concurrency: 6.
- Result: 24 succeeded, 0 failed.
- Latency: min 11.216s, p50 13.216s, p90 15.394s, max 21.626s.
- Average response size: 118 characters.

### Token counter movement

- Counter before the public/API pass: 509,450,328.
- Counter after public plus stored-login API batches: 515,956,505.
- Final counter after the whole stress window: 530,879,456.

The counter is global network-wide, so the full delta should not be treated as
exclusive attribution for this one stress run. It does prove real traffic moved
through the live Khala ledger during the window.

### Local supervised subagents

Direct local spawn:

- Command shape: `khala --json spawn --strategy local --count 5 --max-parallel 5 ...`
- Parent run: `khala_spawn.f7681fa8939c`.
- Result: completed.
- Workers: 5 accepted, 0 failed, 0 cancelled.
- Worker command count: 92 total.
- Edited files: 0.
- Inspection commands verified after closeout:
  - `khala --json workers`
  - `khala --json join khala_spawn.f7681fa8939c`
  - `khala --json worker khala_spawn.f7681fa8939c.worker.01`

Natural-language local spawn:

- Command shape: `khala --headless --json --strategy local --count 2 --prompt "Spin up 2 Khala subagents ..."`
- Parent run: `khala_spawn.498c2721cbc8`.
- Result: completed.
- Workers: 2 accepted, 0 failed.

### Remote Pylon spawn

Preflight:

- Pylon heartbeat returned registered and linked for `pylon.33afd48282a649047e3a`.
- Local Codex default-home account readiness was `ready`.

Three-child Pylon spawn:

- Command shape: `khala --json spawn --strategy pylon --count 3 --fixture --pylon-ref pylon.33afd48282a649047e3a ...`
- Parent run: `spawn.public.khala_coding.chatcmpl_0eeac960aab84f77bb0ebe954448ede0`.
- Result: running, not completed.
- Children:
  - 1 offered/running assignment:
    `assignment.public.khala_coding.chatcmpl_c80bf4783a6d47ccb4c8153273e2bb96`.
  - 2 rejected local worker projections with
    `blocker.khala_mcp.spawn.capacity_shortfall`.
- `khala --json join` preserved the same state.

Single-child follow-up:

- Command shape: `khala --json spawn --strategy pylon --count 1 --fixture --pylon-ref pylon.33afd48282a649047e3a ...`
- Parent run: `spawn.public.khala_coding.chatcmpl_ce1166b0db4441c4acb23515f9aa4eb1`.
- Result: failed.
- Worker blocker refs:
  - `blocker.khala_spawn.pylon_assignment_not_started`
  - `blocker.khala_mcp.spawn.target_pylon_unavailable`

Local cancellation was verified for the persisted Pylon-spawn records, but that
only marks the local parent/worker records. It should not be read as proof that
a remote lease was cancelled.

## Fixes Made During Stress

### Token-safe diagnostics

Problem: `khala info` printed a token-bearing trace URL and could mint a token
just to show diagnostics.

Fix:

- `khala info` now reports configured trace access with the token redacted.
- It no longer prints `token=` query URLs.
- It no longer mints a new trace token just for diagnostics.

Regression:

- `Khala CLI info diagnostics > does not print raw agent tokens or token-bearing trace URLs`.

### Stored-login API token fallback

Problem: `khala --api` told users to run `khala login`, but headless API turns
did not actually read the stored login token when no env or flag token was
provided.

Fix:

- `khala --api` now resolves the stored `khala login` token when no `--token`
  flag or `OPENAGENTS_AGENT_TOKEN` is present.
- Headless turns now use the caller-provided env instead of directly reading
  `Bun.env`, which makes token behavior testable and prevents accidental ambient
  env leakage into isolated runs.

Regression:

- `Khala CLI info diagnostics > uses the stored login token for --api when no env or flag token is present`.

### Stored-login Pylon spawn token fallback

Problem: `khala spawn --strategy pylon` advertised `khala login` as an accepted
auth source, but the remote MCP call only checked flag/env tokens.

Fix:

- `khala spawn --strategy pylon` now resolves the stored `khala login` token
  when no `--token` flag or `OPENAGENTS_AGENT_TOKEN` is present.

Regression:

- `Khala CLI info diagnostics > uses the stored login token for pylon spawn when no env or flag token is present`.

## Reliable Today

Use `khala` confidently for:

- Public prompt traffic at at least 8-way CLI concurrency.
- Authenticated API prompt traffic at at least 6-way CLI concurrency when using
  a valid stored `khala login` token or a valid explicit token.
- Local supervised subagent fanout up to the tested 5 parallel workers.
- Natural-language spawn requests such as "spin up 2 Khala subagents to ...".
- Local spawn persistence and inspection via `workers`, `join`, and `worker`.
- Local cancellation markers for local spawn records.

## Not Yet Reliable

Treat these as limited or operator-gated:

- Remote Pylon spawn completion. The CLI can allocate remote child assignment
  refs and preserve structured blockers, but live execution/acceptance depends
  on fresh Pylon availability and a worker loop actually consuming the lease.
- Multi-child Pylon fanout beyond currently advertised available capacity. The
  Worker intentionally returns `capacity_shortfall`; the live heartbeat/capacity
  projection did not admit the advertised three slots in this stress run.
- Pylon status reads. `pylon status --json` hung in this run and was manually
  interrupted. This is a Pylon dependency caveat for debugging, not a Khala CLI
  local-spawn failure.
- Stale ambient env tokens. Env/flag token still intentionally wins over the
  stored login token. If the env token is stale, API calls fail fast with
  `unauthorized`; unsetting the env token lets the stored login token work.

## Recommended Next Hardening

1. Add a Pylon spawn runner mode that consumes returned child assignments from
   the local CLI path, analogous to `pylon khala request` auto-run behavior.
2. Add live capacity reconciliation to Pylon heartbeat output so the operator
   can see the exact advertised and dispatch-admitted Codex slots.
3. Bound `pylon status --json` with a default timeout or fail-closed diagnostic
   when the local/remote status source hangs.
4. Improve `khala --api` unauthorized messaging to mention that a stale
   `OPENAGENTS_AGENT_TOKEN` can shadow a valid stored `khala login` token.
5. Add a first-class stress harness command that emits this report shape as
   structured JSON without storing prompts, raw token values, or raw logs.
