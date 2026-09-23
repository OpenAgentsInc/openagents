# Terminal-Bench data quality and run incidents

[Current status](README.md) · [TB4 results](tb4-results.md) · [Publication procedure](runbook.md#after-each-run)

## Current blocker: TB4 quota reconciliation

Reviewed through `4b6c619770` (2026-09-23 06:52 CDT). That commit records
21 quota-limited attempts but does not update the preceding scoreboard,
identify all affected trial IDs, or commit the proposed quota handling.
The last published totals therefore remain a historical snapshot pending
reconciliation. Do not subtract 21 from them: the overlap, rewards, and
replacement attempts are not established by the committed note.

The note reports 19 attempts moved and rerun. It does not explain the
remaining two or prove which replacements appear in the scoreboard.
The subsequent [task-win trace audit](2026-09-23-task-win-analysis.md)
retains 20 selected TB4 trial bundles from `coderos`, including all ten
highlighted wins, three failed v2 counterparts, one v5 pass, and six local
Opus baselines. Their native streams show no terminal usage-limit outcome.
Seven public comparison bundles are retained separately. This is a partial
inventory, not a reconciliation of the 21 affected attempts or a complete
population from which to regenerate the scoreboard. The extracted
[TB4 replay fixtures](../../crates/coder-one/fixtures/tb4/) cover an earlier
population too.

The audit also finds two interpretation issues: the Intrastat native
stream has three invalid JSON lines after binary document output, and
VBA's top-level CTRF reports 4/4 even when only 27/28 behavioral traces
pass and the reward is zero. The retained original bytes, separate
behavioral results, and verifier rewards are required to interpret them.

Before publishing corrected results:

1. Reconcile all 21 attempt IDs against native streams and verifier
   results on the execution host, including the two absent from the move
   count. Record each arm, task, artifact, interruption, and replacement.
2. Separate infrastructure outcomes from valid task failures. Keep
   interrupted-attempt spend in the operational ledger; do not erase it
   when excluding an attempt from a capability comparison.
3. Retain the full evidence and a versioned inclusion/exclusion manifest,
   then regenerate counts, costs, and matched-task comparisons from that
   same population. Mark unknown charges and lower bounds explicitly.
4. Update the status index with the reconciled snapshot's date and source.

The distinct usage-limited outcome, the requeue, and the host-wide Claude
concurrency cap are implemented
([#9564](https://github.com/OpenAgentsInc/openagents/issues/9564)); the
[runbook](runbook.md#schedule-a-suite) describes each layer. They apply to
trials that finish after the schedulers restart on that commit. On a
restart, the scheduler also finds throttled trials still in the jobs
directory, moves them to `failed/`, and requeues them. `tb4_scoreboard.py`
now leaves every usage-limited trial out of its cells and counts it in a
separate column, which helps with step 1 but doesn't replace the audit.

## Incident record

The following notes preserve what was reported at the time. Host paths
refer to the Linux execution host; reports of reruns are not proof of
inclusion in the current published totals.

- **21 Terminal-Bench 4.0 trials on 2026-09-23 ran into the Claude
  subscription's usage limit** (13 `claude-code-opus`, 8 Coder One) while
  several Opus 5.5 trials ran at xhigh in parallel, and were graded as if
  the agent had finished. Their sessions ended with `api_error_status`
  429 or a usage-limit message, so they measure the quota, not the agent.
  The incident note reports 19 under `failed/` as `*-usage-limit-*` and
  rerun, and proposes a distinct usage-limited outcome, a requeue, and a
  host-wide cap on concurrent Claude trials.

- **Ten Terminal-Bench 4.0 trials on 2026-09-23 died on a revoked Claude
  token** (four `claude-code-opus`, six Coder One). Refreshing the host's
  Claude login, by a refresh loop or by the operator's own Claude Code
  session on the same credentials, revokes the access token a running trial
  holds, so a trial longer than the gap between refreshes fails with
  `401 OAuth access token has been revoked`. They are under `failed/` as
  `*-revoked-token-*` and rerun. The suite launcher now prefers a
  long-lived token from `claude setup-token` in
  `~/.openagents/claude-setup-token`.
- **Two `freecad-spring-clip` trials were never graded** because the
  adapter raised on Coder One's `delegate_failed` exit code. Since commit
  `10dfdba65e`, outcome exit codes (3, 4, 5) are recorded and the verifier
  grades the environment, as it does for other agents.

- **24 `coder-one-tunable-luna-v2` trials on 2026-09-23 are invalid.** A
  runner script mapped the arm to an older artifact that predates policy
  manifests; it ignored the policy and ran Coder One's original Gemini loop,
  so the trials measured neither arm. They are under `failed/` as
  `*-wrong-artifact-*` and were rerun on artifact `753a17ed975f`.
- **The root disk filled on 2026-09-23 at about 06:39 UTC.** Terminal-Bench
  4.0's ML task images and their build cache outgrew the scheduler's 40 GB
  free-disk floor with several builds at once. Both suite schedulers
  crashed on `ENOSPC`, and four Luna-first trials ended with empty result
  files; those are under `failed/` as `*-enospc-*` and were rerun. After
  pruning, the suites restarted with a 60 GB floor, at most five and three
  concurrent trials, and smallest tasks first, and a watchdog now prunes
  dangling images and old build cache below 50 GB free.

- Twelve `coder-one-jevprobe2-opus-lean-low-5m` attempts and four
  `coder-one-jevprobe3-*` attempts ended in `AgentSetupTimeoutError` with
  eight trials installing Claude Code at once. The runner retried each once;
  the three that timed out twice were rerun three at a time. None are
  results; the attempts are under `failed/`.

- Three `extended` trials on 2026-09-22 (`claude-code-opus` on
  `cancel-async-tasks` and `sqlite-db-truncate`, and
  `coder-one-jevprobe-opus-lean-low` on `cancel-async-tasks`) ended in
  `AgentSetupTimeoutError`: sixteen trials installed their agents at once
  and passed Harbor's 360-second setup limit. They are not results; they
  were moved to `failed/` and rerun with eight trials at a time.

- Costs marked § include one generation call the door left unpriced (in
  each case the call that read cached tokens); it is estimated at that
  run's own cost per token.
- The first Jev-brief trials recorded a null total because an episode with
  no generation counted its generation cost as unknown instead of zero
  (fixed in `1f883564aa`). Their totals here are the Jev and delegate costs
  added by hand.

- Four Coder One trials on 2026-09-22 are **invalid**, not losses: `coder-one-v2`
  (prompt reordering) and `coder-one-deep` (reordering plus a Jev survey),
  each on both tasks, artifact `coder-one 0.1.0 (99e647a7a974)`, tagged
  `coder-one-speed-99e647a7a974`. Eight Coder One explorers ran at once,
  the account exceeded the `free` lane's 20 generations a minute, and each
  episode ended as `generation_failed` after 3 to 7 steps (reward 0.0).
  Coder One now waits out a rate limit (commit `9f31630ac6`). The report
  planned reruns one at a time; it did not identify replacements for these
  four attempts.

- The first `claude-code-opus` attempts on both tasks ended in about a
  second with no inference: the API refuses Opus 5.5 to Claude Code
  2.1.278 (`claude_code_version_too_old`). The arm now pins 2.1.280. The
  failed job directories are kept outside the results, under
  `~/.openagents/terminal-bench/failed/` on the Linux host.

- Three Codex `fix-git` trajectories (`smoke--codex--fix-git`, `-2`, and
  `-3`) are not valid JSON: Harbor's credential scrubber rewrote literal
  values in them. The first two were also model-rejection failures. Only
  `smoke--codex--fix-git-4` is usable.
- Devin trajectories carry one timestamp for every step, so no duration can
  be read from them.
- The early external baselines (Fable 5.1, Sonnet 4.5, Codex 0.153.3,
  and Devin) ran on an arm64 Mac, and Coder One ran on x86_64 Linux.
  Later Opus 5.5 and Codex 0.155.1 comparisons ran on Linux too. Check
  each table's host and timing source before comparing agent times.
