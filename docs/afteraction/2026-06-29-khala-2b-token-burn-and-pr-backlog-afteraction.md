# 2026-06-29 Khala 2B Token Burn And PR Backlog Afteraction

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Status as of `2026-06-29T13:00Z`.

This report covers the roughly 8-hour high-fanout session that pushed the
OpenAgents Khala counter by about two billion own-capacity Codex tokens, plus
the PR backlog explosion that followed. It is written for future agents who need
to keep the fleet hot without repeating the same control-plane mistakes.

## Executive Summary

The fleet did burn real tokens. D1 recorded `1,995,529,816` Pylon-Codex
own-capacity tokens on `2026-06-29` between `00:00:00Z` and `12:59:56Z`, across
`899` token rows, `645` distinct task refs, and `899` sessions. The last
8-hour cut from about `04:56Z` through `12:57Z` accounted for
`1,860,074,516` of those tokens.

The burn was not evenly productive. Early work fanned out issue implementation
too aggressively and created a huge open-PR queue. By `12:47Z`, GitHub showed
`418` open PRs; by `12:52Z`, it was `423` open PRs. At least `59` issues had
duplicate PRs. The fleet had solved or partially solved many issue prompts, but
the merge/review/close lane was too weak.

The control-plane diagnosis is:

- dispatch and execution are separate; the executor pool must stay alive;
- token accounting can fail independently from task execution and needs replay;
- killing/refilling controllers mid-turn can orphan accepted tasks and lose
  closeout unless workers are detached and reports are replayed;
- once open PRs outnumber open issues, the fleet must switch from issue
  implementation to PR consolidation;
- rate-limit UX must distinguish short 5-hour cooldowns from weekly exhaustion;
  weekly exhaustion needs explicit operator reset/recovery, while 5-hour
  cooldown should wait for `resetAt`.

## Token Accounting

### Today So Far

Query window: `2026-06-29T00:00:00.000Z` through latest observed
`2026-06-29T12:59:56.286Z`.

```text
provider: pylon-codex-own-capacity
rows: 899
distinct task refs: 645
distinct sessions: 899
total tokens: 1,995,529,816
input tokens: 1,983,879,734
output tokens: 11,648,867
reasoning tokens: 1,153,352
first observed: 2026-06-29T00:00:21.386Z
latest observed: 2026-06-29T12:59:56.286Z
```

The spend was overwhelmingly input/context tokens. That is expected for coding
agents repeatedly reading repo state, PR diffs, issue context, and logs, but it
also means wasted duplicate PR review is expensive.

### Hourly Shape

```text
2026-06-29T00:00Z  25.8M tokens   9 rows    9 tasks
2026-06-29T01:00Z  20.3M tokens   5 rows    5 tasks
2026-06-29T02:00Z  15.1M tokens   3 rows    3 tasks
2026-06-29T03:00Z  34.1M tokens   6 rows    6 tasks
2026-06-29T04:00Z  33.3M tokens   6 rows    5 tasks
2026-06-29T05:00Z  213.8M tokens  32 rows   25 tasks
2026-06-29T06:00Z  98.0M tokens   28 rows   16 tasks
2026-06-29T07:00Z  188.0M tokens  49 rows   45 tasks
2026-06-29T08:00Z  167.9M tokens  42 rows   27 tasks
2026-06-29T09:00Z  29.6M tokens   13 rows   11 tasks
2026-06-29T10:00Z  421.2M tokens  116 rows  97 tasks
2026-06-29T11:00Z  428.1M tokens  169 rows  139 tasks
2026-06-29T12:00Z  320.2M tokens  421 rows  258 tasks
```

The 10:00Z and 11:00Z hours were the strongest sustained useful burn. The
12:00Z hour had many more rows and tasks, but lower average tokens per task,
because the fleet shifted into fast PR-review/refill churn.

### Last 8 Hours Cut

Query window: `2026-06-29T04:56:00.000Z` through latest observed
`2026-06-29T12:57:03.753Z`.

```text
rows: 851
distinct task refs: 608
distinct sessions: 851
total tokens: 1,860,074,516
input tokens: 1,849,015,963
output tokens: 11,057,338
reasoning tokens: 1,072,660
```

## What The Tokens Were Spent On

Local launcher log categories as of `13:00Z`:

```text
issue top-up logs:       90 files, 78 accepted assignment refs
old PR review logs:     405 files, 280 accepted assignment refs, 264 PR numbers
PR resolver logs:        58 files, 45 accepted assignment refs, 58 PR numbers
ORCA gap logs:            7 files, 3 accepted assignment refs at sample time
```

### 1. Issue Implementation And Audit Fanout

Early in the session, the fleet pulled open issues and tried to implement the
smallest safe change for each issue. This did move code and created PRs, but it
also kept firing after issue-to-PR coverage was already high.

By the later audit:

```text
open GitHub issues: 60
open GitHub PRs: 418, then 423
issues with duplicate PRs: 59
```

Examples of duplicate PR groups:

```text
#7018: 7 PRs
#6893: 6 PRs
#7013: 6 PRs
#6892: 5 PRs
#7027: 5 PRs
#7030: 5 PRs
#7024: 5 PRs
#7020: 5 PRs
#7023: 5 PRs
#6708: 5 PRs
#6856: 5 PRs
```

Conclusion: issue implementation was useful until coverage became saturated.
After that point, every additional issue-implementation worker became likely to
increase backlog entropy.

### 2. PR Review And Merge-Readiness Work

Once the open PR count became the bottleneck, the fleet was moved toward PR
review. The first PR-review refiller used prompts like "Review open PR #NNNN for
merge readiness." That was better than starting new issue solutions, but still
too passive. It often produced review work without closing, merging, or
consolidating the queue.

The replacement PR resolver prompt now asks workers to classify and act:

- merge if safely mergeable and narrow verification passes;
- close/comment if duplicate or superseded;
- fix small blockers on the PR branch;
- report still-blocked with reason.

The old review-only parent process was stopped without killing accepted child
tasks. A new lock-aware resolver loop was started with a target of `16` active
slots so capacity remained for ORCA gap closure.

### 3. ORCA Adaptation Gap Closure

Four dedicated ORCA lanes were created from
`docs/ade/2026-06-27-orca-orchestrator-adaptation-report.md`:

- Pylon runner registry / typed coordinator spine;
- Codex rate-limit status and reset/recovery UX;
- OpenAgents desktop message/tool rendering;
- live/retained agent-status store shape.

After clarification, an extra high-priority Codex weekly reset policy task was
launched. Its required behavior:

- if a Codex fleet account only hit a temporary 5-hour limit and still has
  weekly quota, show `resetAt`/wait state and do not trigger reset;
- if a Codex fleet account exhausted weekly usage, expose a safe explicit
  operator reset/recovery action and recheck status afterward;
- do not add a fake reset button that cannot work.

This is important because weekly exhaustion is operationally different from
short cooldown. Treating both as "rate limited" hides the action the operator
actually needs.

### 4. Desktop And Stats UI Tasks

Two explicit fleet assignments were launched rather than implemented inline:

- OpenAgents desktop false empty-state fix: the Coding page said
  "No active Codex sessions" while visible session cards were active/recent and
  the badge showed `CODING: 20`.
- `/stats` layout/deploy fix: move the large Khala token counter into a compact
  top-right header box near Network Stats, widen the daily graph, and show every
  day from June 24 through present.

## What Went Wrong

### PR Creation Outran PR Resolution

The biggest product mistake was maximizing token burn by creating issue-solving
work without an equally strong PR merge/close lane. The system did not pivot
soon enough from "open issue implementation" to "consolidate PRs."

Future rule: if open PR count is above open issue count, default new work to PR
resolution, not issue implementation. If duplicate PRs exist for an issue, task
workers to select/merge/close, not produce another branch.

### Controller Replacement Risked Closeout Loss

At least one controller swap left accepted tasks terminal without normal
closeout. Some failures wrote `codex-turn-report-failures.jsonl`; some did not.
The non-spooled misses required reconstructing token reports from exact-account
Codex rollout JSONL, using the workspace hash from `statusRef`.

Future rule: do not kill a parent controller unless launched workers are
detached (`nohup`/`setsid`) and logs are redirected. The new PR helper launches
workers under `nohup` and syncs recent accepted logs into local locks.

### Token Failure Spool Is Necessary But Not Sufficient

Several report failures appeared as:

- `401 unauthorized` from stale process env;
- `500 internal_server_error`;
- `503 pylon_codex_storage_error/token_usage_ingest`.

Those were replayed through `createPylonCodexTurnReporter` using the current
Pylon agent token and archived under
`~/.pylon-fable/replayed-failures/`.

However, one batch of terminal sessions had no live failure spool. The reliable
audit is:

1. parse accepted assignment refs from local launcher logs;
2. exclude refs still active under `~/.pylon-fable/active-assignment-runs`;
3. query D1 `token_usage_events` by exact `task_ref`;
4. only reconstruct from rollout JSONL for finished refs missing D1 rows.

Never backfill active sessions early.

### The Desktop Status Model Is Still Too Shallow

The desktop can show a nonzero coding badge and active/recent session cards
while also rendering "No active Codex sessions." The UI needs an ORCA-like
status model:

- live vs retained sessions;
- `stateStartedAt` distinct from `updatedAt`;
- state history;
- process-liveness and latest-event status;
- empty-state derived from visible sessions, not only a strict active subset.

### Public Stats Read Path Flapped Under Load

The public stats endpoints occasionally returned transient 500s while direct D1
queries succeeded quickly. This looked like read-path pressure, not missing
token rows. Do not treat one stats-page 500 as accounting loss. Check D1 and the
failure spool first.

## What Worked

### 20-Slot Local Ceiling Was Real

With four Codex accounts (`codex-3`, `codex-4`, `codex-5`, `codex-7`) and
target 5 slots per account, the machine repeatedly reached `20` active marker
files. That was visible in the desktop and in the active assignment marker
directory. The practical ceiling was not a mysterious 5; it depended on keeping
the executor/refiller lanes alive and targeting per-account capacity.

### Replay Path Recovered Missing Accounting

The current replay method worked:

- read current token from
  `~/.pylon-fable/auth/openagents-agent-token`;
- post stored reports with `createPylonCodexTurnReporter`;
- verify exact `task_ref` rows in D1;
- archive replayed spool.

The public counter reached `5,850,136,969` at `2026-06-29T12:57:48Z` with
`composition: live_at_read`.

### Lock-Aware PR Refill Is The Right Direction

A helper was added at
`apps/pylon/scripts/codex-supervisor/pr-review-refill.sh` with tests. It:

- atomically locks per PR number;
- preserves locks while an accepted assignment is active;
- converts stale accepted/inactive locks into done markers;
- releases failed-before-accept locks;
- launches workers detached with redirected logs.

Focused tests passed:

```text
pr-review-refill.test.sh: 12 passed, 0 failed
claim-dispatch.test.sh: 13 passed, 0 failed
bash -n on helper/test: ok
```

## Current Fleet State At Handoff

At sample time after the ORCA split:

```text
ORCA gap controller: active_orca=4
PR resolver controller: target 16 active slots
live failure spools: 0
```

Additional explicit tasks launched:

- desktop empty-state fix (`codex-7`):
  `assignment.public.khala_coding.chatcmpl_50eef2af5b7c47c1bfa70d6b740f3c01`;
- `/stats` counter/graph/deploy (`codex-3`):
  `assignment.public.khala_coding.chatcmpl_c6aa92db7533443bada2392e2937e085`;
- weekly Codex rate-limit reset policy (`codex-4`):
  `assignment.public.khala_coding.chatcmpl_1d2ce7605c874f85a11a3f632a1f3173`.

Important dispatch lesson: do not force `--pylon-ref` for these ad hoc UI/fleet
tasks under high fanout. A foreground retry showed the forced-pylon path failed
with `blocker.public.pylon_dispatch.duplicate_active_assignment`. Removing
`--pylon-ref` while still pinning `--account` accepted immediately. Use
`--pylon-ref` only for a pylon-target smoke, not as the default top-up shape.

## Immediate Next Actions

1. Keep the PR resolver running, but make sure it closes or merges PRs instead
   of merely reviewing them.
2. Keep exactly a small dedicated ORCA lane active until the desktop/operator
   gaps are closed.
3. Finish the Codex weekly-reset policy work. This is not optional; the operator
   needs to recover weekly-exhausted fleet accounts while leaving 5-hour
   cooldown accounts alone.
4. Monitor `~/.pylon-fable/codex-turn-report-failures.jsonl`; replay and archive
   immediately if it appears.
5. Run the accepted-ref D1 diff periodically. If `missing_finished_candidates`
   becomes nonzero, fix accounting before claiming the counter is correct.
6. Reduce open PR count before starting new broad issue work. The live backlog
   problem is now review/consolidation throughput, not issue discovery.

## Policy Updates For Future Agents

- Token-burn goals must be coupled to a backlog-shape goal. "Max tokens" without
  "shrink PR count" creates entropy.
- When open PRs exceed open issues, route at least 75 percent of worker capacity
  to PR resolution.
- No controller should own child process lifetime by accident. Use detached
  worker launches and per-worker logs.
- Rate-limit UX must classify:
  - temporary 5-hour cooldown: wait/recheck after `resetAt`;
  - weekly usage exhaustion: explicit reset/recovery action plus recheck;
  - invalid credentials: operator reauth, never automatic login against
    default `~/.codex`.
- Desktop "active" UI must be derived from both process liveness and recent
  rollout events, and must not contradict its own visible session list.
