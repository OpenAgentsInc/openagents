# Khala CLI Spawn/Subagent Audit

Status: audit and implementation specification, 2026-06-27.

Implementation progress:

- #6371 added the local Khala CLI spawn supervisor, local worker persistence,
  CLI/slash commands, cancellation, and focused CLI tests.
- #6372 extracted the Pylon-backed generic spawn runner under
  `apps/pylon/src/khala-spawn.ts`, added `pylon khala spawn`, and kept
  `pylon khala burndown` as a specialization over that runner. The Pylon slice
  plans against ready Codex accounts plus advertised availability, executes
  no-spend assignments through the existing active-run heartbeat path, and
  aggregates exact token rows, durable request ids, assignment refs,
  owner-only trace counts, raw event counts, and blockers.
- #6373 exposed spawn over the Worker and Pylon MCP surfaces. `khala.spawn`
  returns a parent `spawn.public.khala_coding.*` ref plus child assignment refs
  and durable request ids, `khala.spawnStatus` aggregates caller-owned child
  assignment state without private raw events, and child assignments encode the
  parent/worker relation as public-safe task refs.
- Remaining tracked slices are natural-language spawn routing plus Khala
  response copy (#6374).

Owner problem statement:

> `khala`
>
> `> can you spawn subprocesses of yourself`
>
> Khala answered that it works as a conceptual network but does not expose
> independent subprocess/background execution.

That answer is unacceptable for the CLI product. The `khala` command must expose
a reviewed, bounded way to start new Khala child workers so an operator can say:

```sh
khala spawn --count 5 --objective "audit X and report independent findings"
```

or, in interactive mode:

```text
> spin up 5 instances/subagents to audit X
```

and get five tracked worker runs, not a prose apology.

This document audits the current code paths and lists the work required to make
that true without weakening the existing own-capacity, public-safety, token
accounting, and no-ad-hoc-routing invariants.

## Executive Summary

Most of the executor primitives already exist, but they are not composed into a
general `khala spawn` surface.

Current facts:

- `clients/khala-cli` has no `spawn`, `subagent`, `workers`, `cancel`, or
  `join` command. It has chat, login, feedback, tokens, Artanis, and a single
  local Codex delegation command.
- `clients/khala-cli/src/codex.ts` can run one local Codex SDK thread with an
  isolated Khala/Pylon Codex home, `danger-full-access`, `approvalPolicy:
  "never"`, network enabled, and a wall-clock timeout. It is a single-turn local
  delegate, not a multi-worker supervisor.
- `apps/pylon` already has the stronger durable own-capacity lane:
  `pylon khala request`, `pylon assignment run-no-spend`, `pylon khala proof`,
  exact downstream Codex token ingest, owner-only ATIF traces, and private raw
  Codex event archives.
- `apps/pylon/src/khala-burndown.ts` already proves a partial parallel pattern:
  it plans several Codex-account slots and runs each iteration with
  `Promise.all`. It is roadmap/issue-specific and Pylon-only, not a general
  Khala CLI spawn primitive.
- `apps/openagents.com/workers/api/src/khala-mcp.ts` and
  `apps/pylon/src/khala-mcp.ts` expose `khala.request`, `khala.resume`,
  `khala.status`, and `khala.capacity`. They do not expose `khala.spawn` or a
  batch/worker-run abstraction.
- `apps/openagents.com/workers/api/src/inference/khala-identity.ts` currently
  tells Khala not to promise "background tool execution unless a separate
  reviewed surface explicitly performs it." Until `khala spawn` exists as that
  reviewed surface, the model is pushed toward the "capability we do not yet
  expose" response seen in the transcript.

Recommended target:

1. Add `khala spawn` and `/spawn` as first-class CLI surfaces.
2. Implement a local `KhalaSpawnSupervisor` that owns child worker lifecycle,
   persistent run state, logs, cancellation, and aggregation.
3. Prefer the existing Pylon-backed durable assignment path for real coding
   work because it already has own-capacity auth, exact Codex token rows,
   owner-only traces, private raw events, and proof.
4. Keep a local direct Codex fallback only for same-machine work when it can
   produce honest local run records and, if it contributes to Khala counters,
   exact SDK usage reporting.
5. Extend the typed/semantic route selector so natural language such as
   "spin up 5 agents to do X" routes to the spawn surface without keyword
   routing.
6. Update Khala's refusal/identity prompt and tests so the CLI never answers
   "we do not expose subprocesses" when the reviewed spawn surface is
   available.

## Target User Experience

### Interactive

```text
Khala CLI v0.1.x. Type /help for commands, /exit to quit.

> spin up 5 instances/subagents to audit the checkout flow
Khala: spawning 5 Khala workers for this workspace...

run: khala_spawn.4c61f8a9
workers:
  1. khala_worker.01 - starting - local codex account codex-1
  2. khala_worker.02 - starting - local codex account codex-2
  3. khala_worker.03 - queued - waiting for advertised capacity
  4. khala_worker.04 - queued - waiting for advertised capacity
  5. khala_worker.05 - queued - waiting for advertised capacity

Use /workers, /join khala_spawn.4c61f8a9, or /cancel khala_spawn.4c61f8a9.
```

Expected follow-up commands:

```text
/spawn 5 audit the auth flow and produce separate findings
/workers
/worker khala_worker.01
/join khala_spawn.4c61f8a9
/cancel khala_spawn.4c61f8a9
```

### Headless

```sh
khala spawn \
  --count 5 \
  --objective "audit the checkout flow and produce independent findings" \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$(git rev-parse origin/main)" \
  --verify "bun test apps/openagents.com/workers/api/src/checkout.test.ts" \
  --json
```

Expected JSON shape:

```json
{
  "schema": "openagents.khala.spawn_run.v0.1",
  "ok": true,
  "runRef": "khala_spawn.4c61f8a9",
  "strategy": "pylon_codex_assignments",
  "requestedCount": 5,
  "startedCount": 5,
  "maxParallel": 5,
  "workers": [
    {
      "workerRef": "khala_worker.01",
      "state": "running",
      "assignmentRef": "assignment.public.khala_coding.chatcmpl_...",
      "durableRequestId": "chatcmpl_...",
      "pylonRef": "pylon.public...",
      "accountRefHash": "account_hash..."
    }
  ]
}
```

## Current State Audit

### `clients/khala-cli`

Relevant files:

- `clients/khala-cli/src/cli.ts`
- `clients/khala-cli/src/codex.ts`
- `clients/khala-cli/README.md`

Findings:

- Command parsing recognizes `spawn`, `workers`, `worker`, `join`, and
  `cancel` beside the older chat, feedback, auth, Codex, token, login, logout,
  and utility commands.
- Interactive slash commands include `/spawn`, `/workers`, `/worker`, `/join`,
  and `/cancel` beside `/codex`, `/tokens`, `/feedback`, `/msginfo`, `/info`,
  `/changelog`, `/login`, `/logout`, `/artanis`, `/khala`, `/version`,
  `/help`, and `/exit`.
- `maybeRunLocalCodexTurn()` calls `selectKhalaRoute()` and can route one turn
  to normal chat, local Codex workspace delegation, or the local Khala spawn
  supervisor.
- `selectKhalaRoute()` asks Khala to return a schema-validated typed JSON route
  of `chat`, `local_codex`, or `spawn_khala`, including spawn intent, count,
  objective, and workspace requirement when present.
- `runKhalaCodexTask()` starts one Codex SDK thread in the current working
  directory. It tracks final text, command count, edited file count, turn count,
  and session ref. The local spawn supervisor now uses that delegate to create
  supervised child worker records, isolate per-worker git worktrees when the
  current directory is a Git checkout, aggregate worker states, and cancel
  active workers. Exact SDK token usage for Pylon/Codex ingest remains a Pylon
  assignment surface concern.
- `connectKhalaCodex()` correctly avoids the default `~/.codex` destructive
  login path by using Khala's own Codex home unless a configured account is
  present. This must be preserved for spawn.

Conclusion:

The Khala CLI now has both a working single local executor and a bounded local
spawn supervisor. The remaining work is hardening the natural-language routing,
public/browser capability copy, and the CLI strategy bridge to durable Pylon
fanout.

### Pylon Khala Requester And Burndown

Relevant files:

- `apps/pylon/src/khala-requester.ts`
- `apps/pylon/src/khala-burndown.ts`
- `apps/pylon/src/index.ts`
- `apps/pylon/src/assignment.ts`
- `apps/pylon/src/active-assignment-runs.ts`

Findings:

- `issuePylonKhalaRequest()` posts to `/v1/chat/completions` with
  `workflowClass: "codex_agent_task"` and optional target Pylon/workspace pins.
  It returns an `assignmentRef`, `durableRequestId`, durable stream URL, frames,
  and workflow projection.
- `readPylonKhalaProof()` reads `/api/pylon/codex/proof` and verifies exact
  own-capacity token rows, owner-only traces, owner-only raw events, and an ISO
  generated timestamp.
- `buildPylonKhalaBurndownPlan()` already selects ready Codex accounts and
  builds slot commands for multiple issues/accounts.
- `runPylonKhalaBurndownPlan()` executes each iteration with `Promise.all` over
  the iteration's slots, then checks proof and the public token counter.
- `runNoSpendAssignment()` handles one assignment lease per invocation, selects
  a ready Codex account, registers an active run, heartbeats load, executes
  `executeCodexAgentAssignment()`, submits progress/closeout, and clears active
  run state.
- `active-assignment-runs.ts` tracks fresh active coding runs under local Pylon
  state with a TTL, which is already the right primitive for busy/available
  capacity accounting.

Conclusion:

Pylon already has most of the durable multi-worker mechanics, but they are
wrapped around "burndown roadmap issues" rather than a general user-level
`spawn N workers for objective X` primitive. The quickest robust implementation
is to lift this pattern into a reusable batch/spawn module and call it from
`khala`.

### Pylon Codex Executor

Relevant file:

- `apps/pylon/src/codex-agent-executor.ts`

Findings:

- `CODEX_AGENT_TASK_SCHEMA` is `openagents.pylon.codex_agent_task.v0.3`.
- `runWithCodexSdk()` lazy-imports `@openai/codex-sdk` and starts one thread in
  a bounded workspace.
- The executor intentionally maps caller-owned Khala to the SDK equivalent of
  `--dangerously-bypass-approvals-and-sandbox`: sandbox mode
  `danger-full-access`, approval policy `never`, and network enabled.
- Assignment payloads cannot request that danger mode. It is local executor
  policy plus post-hoc workspace-boundary validation.
- `executeCodexAgentAssignment()` materializes the workspace, prepares
  dependencies, runs Codex, verifies with a command, emits public-safe refs,
  and posts private raw events/exact usage through reporters.

Conclusion:

The executor is suitable for spawned Khala workers as long as each worker gets a
bounded workspace, assignment ref, timeout, cancellation path, and proof.

### MCP And Worker Delegation

Relevant files:

- `apps/pylon/src/khala-mcp.ts`
- `apps/openagents.com/workers/api/src/khala-mcp.ts`
- `apps/openagents.com/workers/api/src/inference/coding-workflow-delegation.ts`

Findings:

- MCP tools currently include `khala.request`, `khala.resume`,
  `khala.capacity`, and `khala.status`.
- Remote `khala.request` creates one delegated assignment through
  `delegateCodingWorkflow()`.
- `khala.capacity` projects caller-owned linked Pylon capacity.
- There is no `khala.spawn`, no batch request schema, no parent/child run
  relation, and no aggregate status/proof endpoint.

Conclusion:

The MCP surface needs a spawn/batch tool if agent clients should use the same
capability as the CLI. The CLI can call Pylon APIs directly first, but MCP
parity should follow quickly.

### Khala Prompt/Refusal Posture

Relevant files:

- `apps/openagents.com/workers/api/src/inference/khala-identity.ts`
- `apps/openagents.com/workers/api/src/khala-chat-program.ts`
- `apps/openagents.com/workers/api/src/inference/khala-refusal-posture.test.ts`

Findings:

- The refusal posture correctly says not to promise background tool execution
  unless a reviewed surface performs it.
- Because there is currently no reviewed spawn surface, the transcript answer
  followed the "capability not exposed" path.
- Once `khala spawn` exists, the prompt must distinguish surfaces:
  public web chat still cannot claim access to the user's machine, but the
  Khala CLI can truthfully say it can spawn bounded local/Pylon workers and
  should name the command.

Conclusion:

Implementation is not complete until the model-facing tests prevent the exact
bad transcript from recurring in CLI context.

## Required Architecture

### 1. Define The Spawn Contract

Add a small shared contract, preferably under `clients/khala-cli/src/spawn.ts`
first and promoted to `packages/*` only if Pylon/Worker need to import it.

Minimum types:

```ts
type KhalaSpawnRun = {
  schema: "openagents.khala.spawn_run.v0.1"
  runRef: string
  objective: string
  requestedCount: number
  maxParallel: number
  strategy: "pylon_codex_assignments" | "local_codex_threads"
  state: "planned" | "running" | "completed" | "cancelled" | "failed"
  createdAt: string
  updatedAt: string
  workers: KhalaSpawnWorker[]
}

type KhalaSpawnWorker = {
  schema: "openagents.khala.spawn_worker.v0.1"
  workerRef: string
  runRef: string
  slotIndex: number
  objective: string
  state: "queued" | "starting" | "running" | "accepted" | "rejected" | "cancelled" | "failed"
  assignmentRef?: string
  durableRequestId?: string
  pylonRef?: string
  accountRefHash?: string
  localWorktree?: string
  sessionRef?: string
  proofRef?: string
  blockerRefs: string[]
}
```

The contract must be explicit that "spawn a new Khala" means a bounded,
user-owned child worker supervised by this CLI/Pylon, not uncontrolled
self-replication.

### 2. Add CLI Commands

Add top-level commands:

```sh
khala spawn --count <n> --objective <text> [--repo owner/repo --branch main --commit sha --verify argv]
khala workers [--json]
khala worker <workerRef> [--json]
khala join <runRef> [--json]
khala cancel <runRef|workerRef> [--json]
```

Add interactive slash commands:

```text
/spawn <count> <objective>
/workers
/worker <workerRef>
/join <runRef>
/cancel <runRef|workerRef>
```

Required flags:

- `--count <n>`: positive integer, bounded by a default max such as 10 unless a
  local config explicitly raises it.
- `--max-parallel <n>`: defaults to advertised available capacity or `count`.
- `--strategy pylon|local|auto`: default `auto`, prefer Pylon durable
  assignments when authenticated and capacity is available.
- `--repo`, `--branch`, `--commit`, `--verify`: required for repository coding
  work using Pylon `codex_agent_task`, except fixture/smoke tasks.
- `--fixture`: local/Pylon smoke mode using the existing sum-repair fixture.
- `--timeout <seconds>`: bounded per-worker timeout.
- `--json`: machine-readable output.

Do not implement natural-language spawn detection with string matching.
`selectKhalaRoute()` returns a typed union of
`chat | local_codex | spawn_khala`, with fields for `count`, `objective`,
`requiresWorkspace`, and spawn intent. That selector stays model-backed like
the current route selector, but its output is schema-validated before the CLI
acts on it.

### 3. Build The Local Supervisor

Add a local supervisor module responsible for:

- Persistent state under `~/.khala/spawn/runs/<runRef>/run.json`.
- Per-worker event logs, for example
  `~/.khala/spawn/runs/<runRef>/workers/<workerRef>.jsonl`.
- SIGINT/SIGTERM handling: first interrupt prints active workers and how to
  cancel/join; second interrupt cancels active local children if safe.
- Cancellation via an `AbortController` for SDK runs and process kill for any
  real child process.
- Cleanup policy for temporary worktrees.
- Aggregation: final summary, accepted/rejected counts, token totals when
  available, proof refs, changed paths, and blocker refs.

Prefer library calls over recursive CLI subprocesses for the first
implementation. For local direct work, call `runKhalaCodexTask()` or a
spawn-aware variant directly so auto-update, TTY handling, and CLI parser state
do not leak into workers. If a process-per-worker mode is later needed, add an
internal `khala worker run --run-ref ... --worker-ref ... --jsonl` command and
spawn that exact internal command with `Bun.spawn()`.

### 4. Use Pylon Durable Assignments As The Canonical Strategy

For coding tasks with repo pins, `khala spawn --strategy auto` should:

1. Ensure OpenAgents login/token is available.
2. Read linked Pylon capacity through `khala.capacity` or local Pylon account
   inventory.
3. Heartbeat Pylon capacity when running on the local device.
4. Create `count` Khala requests with a shared `spawnRunRef` and one
   child-specific objective per worker.
5. Run assignments up to `maxParallel`, bounded by advertised Codex availability.
6. Read `pylon khala proof` for each accepted assignment.
7. Aggregate exact token rows, traces, raw event counts, assignment refs,
   durable request ids, and public-safe closeout refs.

The current `pylon khala burndown` code should be refactored rather than copied:

- Extract reusable slot planning from `khala-burndown.ts`.
- Add a generic objective slot builder that does not assume GitHub issue
  numbers or the roadmap doc.
- Add `runNoSpendAssignmentsBatch()` or
  `pylon assignment run-no-spend --parallel --assignment-ref <a,b,c>`.
- Preserve the active-run heartbeat model in `active-assignment-runs.ts` so
  capacity refs show busy/available/queued accurately while workers run.

### 5. Local Direct Codex Fallback

Local direct spawn is useful when the user has a workspace and Codex login but
no OpenAgents/Pylon token. It should be honest about what it can prove.

Required behavior:

- Create one detached worktree or isolated working directory per worker unless
  the user explicitly asks for shared read-only analysis.
- Reuse isolated Khala/Pylon Codex homes, never destructive default login.
- Run a bounded prompt that includes the worker index, objective, scope, and
  output contract.
- Capture final text, command count, edited file count, session ref, and
  lifecycle events.
- Do not increment public Khala tokens from local direct runs until exact SDK
  usage is captured and posted through a reviewed ingest route.

If the local fallback writes files, it must avoid multiple workers editing the
same checkout concurrently unless a merge/review mode exists. The safe default
is independent detached worktrees and an aggregate report, not automatic merge.

### 6. Worker Prompting

Each worker needs a clear role:

```text
You are Khala child worker {slotIndex}/{count}.
You are part of spawn run {runRef}.
Work independently on this objective:
{objective}

Return:
1. Summary
2. Findings or changes
3. Evidence
4. Blockers
5. Suggested next step

Do not coordinate through hidden shared state. Do not claim another worker's
result. Keep private data, tokens, local auth paths, raw prompts, and raw shell
output out of public summaries.
```

For "do X in parallel" coding tasks, the parent supervisor should generate
child objectives with non-overlapping scopes when possible. For example:

- Worker 1: architecture and data flow audit.
- Worker 2: tests and regressions audit.
- Worker 3: UI/UX or CLI behavior audit.
- Worker 4: security/auth/persistence audit.
- Worker 5: docs/product promises/invariants audit.

That decomposition should also be typed/semantic, not keyword-matched.

### 7. Prompt And Response Contract Update

After `khala spawn` is implemented, update the Khala prompt/test layer:

- CLI-context response to "can you spawn subprocesses/subagents of yourself?"
  should be affirmative and bounded: "Yes, in this CLI we can spawn supervised
  Khala child workers. Use `/spawn 5 ...` or `khala spawn --count 5 ...`."
- Web/public chat response should be honest about surface: it can explain the
  CLI command and requirements, but must not claim browser chat has local
  machine execution.
- Refusal-posture tests should add the exact transcript failure as a bad fixture
  for CLI context once a CLI-context prompt or command route exists.
- The non-promise rule remains correct: no background tool execution claim
  unless the reviewed spawn surface is actually active.

### 8. Authority And Safety

Spawn must preserve existing invariants:

- Own-capacity only. A caller may spawn work only on local capacity or Pylons
  linked to the same owner scope.
- No pooled marketplace or third-party capacity for this feature until a
  separate settlement-bearing policy exists.
- No ad hoc keyword routing. Use typed CLI commands, schema-validated semantic
  selectors, or explicit parser paths after the command route has been selected.
- No raw secrets, private prompts, provider payloads, wallet material, local auth
  paths, or raw shell output in public closeouts, traces, docs, or issues.
- Bounded concurrency. Default count/max-parallel must be finite and visible.
- Cancellable. Every run and worker must have a cancel path.
- Observable. Every worker must have state, heartbeat/progress, logs, and a
  final closeout.
- Verifiable. Pylon-backed coding workers must reconcile to exact token rows,
  owner-only traces, and raw event metadata.

When code lands, update `INVARIANTS.md` only if the implementation changes the
authority contract. A likely new invariant is:

> Khala spawned child workers are owner-local or caller-owned-Pylon executions.
> Spawn requests cannot target unlinked capacity, cannot take public wire
> danger flags, and cannot publish raw private execution material.

## Implementation Slices

### Slice A: CLI Surface And Local Supervisor

Files:

- `clients/khala-cli/src/cli.ts`
- `clients/khala-cli/src/spawn.ts` (new)
- `clients/khala-cli/src/spawn.test.ts` (new)
- `clients/khala-cli/README.md`

Work:

- Add command parsing and slash commands.
- Add run/worker schemas and local store.
- Add `khala workers`, `khala worker`, `khala join`, and `khala cancel`.
- Add local direct `runKhalaCodexTask()` worker execution in isolated worktrees
  for no-Pylon fallback.

Acceptance:

- `bun test clients/khala-cli/src/spawn.test.ts clients/khala-cli/src/input.test.ts`
- `khala spawn --count 2 --fixture --json` starts two local workers and writes a
  persistent run record.
- Ctrl-C does not orphan workers without a visible recovery command.

### Slice B: Pylon Generic Batch Runner

Files:

- `apps/pylon/src/khala-spawn.ts` (new or extracted)
- `apps/pylon/src/khala-burndown.ts`
- `apps/pylon/src/assignment.ts`
- `apps/pylon/src/index.ts`
- `apps/pylon/tests/khala-spawn.test.ts` (new)

Work:

- Extract generic slot planning from burndown.
- Add a batch assignment runner with max-parallel and explicit assignment refs.
- Add `pylon khala spawn` as a lower-level command if useful.
- Keep `pylon khala burndown` as a specialization over the new generic runner.

Acceptance:

- Batch runner executes N fixture assignments across ready Codex accounts.
- It never exceeds advertised available capacity.
- It emits per-worker lifecycle JSONL and aggregate proof.

### Slice C: Khala CLI Uses Pylon Strategy By Default

Files:

- `clients/khala-cli/src/spawn.ts`
- `clients/khala-cli/src/token-store.ts`
- `apps/pylon/src/khala-requester.ts`

Work:

- From `khala spawn --strategy auto`, prefer Pylon when token and capacity are
  available.
- Fall back to local direct only when Pylon is unavailable or `--strategy local`
  is explicit.
- Aggregate `readPylonKhalaProof()` results.

Acceptance:

- `khala spawn --count 2 --fixture --strategy pylon --json` returns two
  assignment refs and exact proof rows.
- Public counter delta reconciles to exact `token_usage_events` rows for the
  assignments.

### Slice D: MCP And Worker API Parity

Files:

- `apps/openagents.com/workers/api/src/khala-mcp.ts`
- `apps/pylon/src/khala-mcp.ts`
- `apps/pylon/tests/khala-mcp-end-to-end.test.ts`
- `apps/openagents.com/workers/api/src/inference/coding-workflow-delegation.ts`

Work:

- Added `khala.spawn` tool with `count`, `objective`, optional repo pins, and
  optional target Pylon ref.
- Added `khala.spawnStatus` to read parent spawn refs.
- Encoded parent/child assignment relations as task refs until a
  table is warranted.
- Preserved durable stream resume behavior.

Acceptance:

- MCP `khala.spawn` returns parent run ref plus child assignment refs.
- Cross-owner requests fail with typed authorization errors.
- `khala.spawn` returns capacity service counts and shortfall blockers when
  fewer than requested workers start.

### Slice E: Semantic Route And Prompt Fix

Files:

- `clients/khala-cli/src/codex.ts`
- `clients/khala-cli/src/cli.ts`
- `apps/openagents.com/workers/api/src/inference/khala-identity.ts`
- `apps/openagents.com/workers/api/src/inference/khala-refusal-posture.test.ts`

Work:

- Extend route selection schema from `chat|local_codex` to include
  `spawn_khala`. Done in issue #6374.
- Add a CLI-context response fixture for "can you spawn subprocesses of
  yourself". Done in issue #6374.
- Update help text and README to show spawn commands. Help text landed with the
  local supervisor; README natural-language routing notes landed in issue
  #6374.

Acceptance:

- In interactive CLI, "spin up 5 subagents to do X" routes to spawn.
- The old answer "capability we do not yet expose" fails the CLI-context test
  once spawn is implemented.

## Acceptance Checklist

The feature is not done until all of these are true:

- `khala help` and `/help` show spawn commands.
- `khala spawn --count 5 --objective "..." --json` starts a parent run and five
  child worker records.
- Workers have stable refs, state transitions, logs, cancellation, and final
  closeouts.
- Pylon-backed workers produce exact token usage rows with
  `provider = "pylon-codex-own-capacity"`,
  `model = "openagents/pylon-codex"`,
  `usage_truth = "exact"`,
  `demand_kind = "own_capacity"`, and
  `demand_source = "khala_coding_delegation"`.
- Owner-only ATIF traces and private raw Codex event metadata exist for each
  Pylon-backed worker.
- Public counters move only from exact rows, not estimates.
- `khala join <runRef>` prints an aggregate answer that distinguishes each
  worker's result.
- `khala cancel <runRef>` stops queued/running local workers and submits safe
  interrupted closeouts for Pylon assignments where applicable.
- Cross-owner target Pylon refs are rejected.
- Natural-language spawn routing uses a typed semantic selector, not keyword
  matching.
- The transcript failure is covered by a deterministic test or fixture.

## Open Decisions

- Default count cap: recommend 10 local workers until capacity/load UX is proven.
- Default strategy: recommend `auto`, with Pylon durable assignments preferred
  when authenticated and ready.
- Local direct token accounting: either do not count it publicly, or add exact
  SDK usage capture and a reviewed ingest path before counting it.
- Merge policy for editing workers: default should be independent worktrees plus
  aggregate report. Automatic merge should be a later reviewed mode.
- Product copy: public web Khala can describe the CLI capability, but should not
  imply browser chat can spawn on the user's machine.

## Bottom Line

Khala already has enough local Codex and Pylon/Codex machinery to make "spawn 5
Khalas" real. The missing layer is a named spawn supervisor and a product
contract around it: typed CLI commands, durable child-worker records, bounded
parallelism, cancellation, capacity-aware Pylon assignment fanout, exact proof,
and prompt/tests that stop Khala from denying the capability once the reviewed
surface exists.
