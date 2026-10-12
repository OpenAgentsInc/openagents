# After-action report: background agent blitz drained the Claude subscription

Date: 2026-10-12. Author: Devin (interactive CLI agent), at the owner's request.

## Summary

Overnight on October 11-12, a background Claude Code session ran an issue
"blitz" that consumed the owner's Claude subscription allowance, hit the
session limit, kept working across at least two resets, and left roughly 30
orphaned processes burning about 1,500% CPU. When the owner opened the chat at
07:27 local time to resume work, the session refused: the allowance was already
exhausted, with a block recorded until 09:10.

The work was real — 14 issues attempted, four marked `ready_to_integrate` — but
it ran unattended with `bypassPermissions`, computer-use tools, no turn or
spend cap, and no concurrency limit against a shared subscription login.

All local consumers were killed on the morning of October 12. Cloud resources
the blitz created (GCP spot VMs and dev-env shard machines) still needed owner
action at the time of this report.

## What ran

The root cause was not an external bot. It was the owner's own session,
`e87ba65e-7f70-4ce1-aa73-3cb07b8e4326` (title "Open issues and recent
commits"), dispatched at 21:25 on October 11 as a **`claude bg` background
daemon worker** — not the interactive terminal the owner was watching.

Key facts from `~/.claude/daemon/roster.json` and
`~/.claude/jobs/e87ba65e/state.json`:

- Launched through a slash dispatch from an interactive session, forking an
  earlier session, with `--model claude-opus-5-5`,
  `--permission-mode bypassPermissions`, and every `computer-use` tool
  pre-allowed.
- Run by a daemon supervisor (pid 62333) through a pty-host worker
  (pids 62341/62349), so it needed no terminal and produced no visible output.
- Job state at shutdown: `blocked`, detail "You've hit your session limit ·
  resets 9:10am (America/Chicago)", `inFlight: { tasks: 6, queued: 1 }` —
  scheduled to resume and keep spending the moment the window reset.

### The workflow

The session ran subagent workflow `wf_093bf3ca-038`: roughly one subagent per
open issue. The transcripts under
`~/.claude/projects/-Users-christopherdavid-work-openagents/e87ba65e-*/subagents/workflows/`
total about 70 MB across 57 agent files. Journal results:

| Issue | Final recorded status |
| --- | --- |
| 11196 | blocked |
| 11197 | partial |
| 11299 | ready_to_integrate |
| 11306 | running |
| 11308 | running |
| 11309 | running |
| 11310 | already_done |
| 11311 | running |
| 11312 | partial |
| 11313 | partial |
| 11323 | ready_to_integrate |
| 11387 | already_done |
| 11388 | ready_to_integrate |
| 11389 | ready_to_integrate |

The run produced at least 17 `blitz/*` branches on origin and merges to `main`.

### Side effects the session spawned

Everything below outlived the session's active turns. All were found still
running on the morning of October 12 and killed, except where noted.

- **`ab.py batch --plan plans/haiku-briefer.json`** (pid 96740), the issue
  11323 Haiku-briefer A/B bench in `~/work/oa-haiku`. It runs `claude -p`
  trials (arms A, B0, BH; 6 issues, 2 reps, 2 workers, a $45 list-price
  budget). It had run 21 hours at ~59% CPU, written no new results in over 20
  hours, and was spinning in a poll loop.
- **`openagents mac serve`** (pids 20829/20831), launched by the session's
  first background task. It enrolled this Mac in the cloud compute pool
  (`~/.openagents/computer-pool.json`, "the pool", capacity 4), opening a path
  for cloud-dispatched jobs to land on the owner's machine and login.
- **`openagents-web --listen 127.0.0.1:4317`** (pids 87801/87803), a gallery
  screenshot server a subagent started for UI captures.
- **Background shell tasks**: two long-running `cargo test` invocations (one
  wrapping a `git cherry-pick`/`push` sequence in `~/work/oa-integrate2`) and
  their test binaries, plus a detached `cargo test` tree in
  `~/work/openagents-11372`.
- **26 orphaned `coder-new` TUI processes**, all parented to launchd, each
  pinned near 60% CPU for up to 36 hours. Stack samples showed the main thread
  spinning in `crossterm::event::poll` on a dead terminal while two tokio
  threads sat parked in `coder_sync::round` and `own_runs::take`. They burned
  CPU, not tokens — a detached-terminal read loop with no exit path.
- **Cloud resources** created by subagents and still alive at report time:
  s2b bench shards on `oa-dev-env-6`, `-7`, `-8`, `-15`, `-16`, `-17`, `-18`
  (restarted 10:36 UTC after spot preemption, running agent trials whose
  judge-1 verdicts draw on the owner's Claude login), and the GCP spot
  instance `clef-11306-h100` (`a3-highgpu-1g`, us-central1-a, for issue 11306,
  billed through Secret Manager API keys rather than the subscription).

## Timeline

Times are America/Chicago (UTC−05:00) from `~/.claude/history.jsonl`, daemon
logs, and the workflow journal.

| Time | Event |
| --- | --- |
| Oct 11 17:32 | Owner warns the session: "youre almost out of usage for the nexrt 40 minns." |
| Oct 11 18:11 | Owner: "i want all open issues solved ASAP use 1 subange doing solver the rest doing normal." The blitz begins. |
| Oct 11 21:25 | Session is (re)dispatched as a `claude bg` daemon worker — supervisor pid 62333. |
| Oct 11 22:25 | Background task starts `openagents mac serve`, enrolling the Mac in the cloud pool. |
| Oct 11 23:20 | Owner: "resume the issues… as part of this workflow blitz… open agents in a custom workflow, one for every single [issue]." Fan-out widens. |
| Oct 11 23:57–Oct 12 06:08 | Subagents run continuously. Journal records the subscription session limit hit (reset 14:10 UTC); agents retry after the reset — one reran 49 frozen judge-1 verdicts rather than stopping. |
| Oct 12 ~06:08 | Last subagent transcript write. Two agents end `failed`; four remain `running` in the journal. |
| Oct 12 07:27 | Owner sends "go fucker" to resume. The turn ends in 877 ms with a synthetic stop — the session is blocked on the usage limit until 09:10. |
| Oct 12 07:28–07:45 | Devin investigates, identifies the daemon worker and its spawned processes, kills all local consumers. |

## How this was able to happen

1. **Native `claude bg` bypasses the workspace's own guardrails.** AGENTS.md
   requires `openagents capacity check claude` before launching subagents and
   routes heavy work through `openagents lease`. The Claude Code daemon's
   workflow dispatch honors neither: nothing checked capacity, took a lease,
   or recorded the run in the shared book.
2. **No spend or turn ceiling on the session.** `bypassPermissions` plus
   computer-use tools, opus-tier model, and an open-ended "solve every issue"
   goal means the only limiter is the provider's own session limit.
3. **Fan-out multiplies burn against one login.** All subagents share the
   owner's subscription OAuth credential. N parallel agents do not get N
   allowances; they drain the same one roughly N times faster.
4. **The usage limit pauses, it does not stop.** The daemon recorded `blocked`
   with queued work and would have resumed at 09:10 to burn the next window.
   Subagents also treated the limit as retryable and resumed automatically.
5. **Spawned work outlives the session.** Background shells, TUI instances,
   bench scripts, pool enrollment, and cloud VMs persisted after the session
   stalled. Nothing tracks them as a set, so cleanup required manual forensics.
6. **Benches lack a stall detector.** `ab.py` caps dollars, not wall-clock; it
   spun for ~20 hours past its last result row.
7. **A defect amplified the mess.** `coder-new` does not exit when its
   controlling terminal disappears; each orphaned UI spins on `read()` at ~60%
   CPU indefinitely.

## Remediation performed

On the morning of October 12 the following were killed on this machine:

- The daemon supervisor (62333), the blitz worker and spare pty-hosts
  (62341, 62349, 28473, 28489).
- The blitz's background shell tasks and hung test trees (73513, 99131, and
  children; the detached `openagents-11372` test tree).
- `ab.py` (96740), `openagents mac serve` (20829/20831), the gallery server
  (87801/87803).
- All 26 orphaned `coder-new` TUI processes and detached test binaries.

The owner's interactive session (pid 27029) was left running; it is idle and
spends nothing on its own. Its new daemon supervisor holds only idle spares.

## Still open at report time

- **Cloud teardown.** `gcloud` credentials on this machine need reauth, so the
  s2b shard VMs and `clef-11306-h100` were not verified or stopped. The
  subagent-runbook in the journal documents the stop commands.
- **`ai.factory.droid.byom`** (launchd, `KeepAlive`) crash-loops trying to open
  a remote-access agent door to this Mac. It is not currently consuming
  anything, but it is an unattended agent channel worth either fixing or
  unloading.
- **Workflow bookkeeping.** Issues 11306, 11308, 11309, 11311 are recorded
  `running` in the journal but no longer have agents; the branches, claims,
  and board states should be reconciled before any new run.

## How to keep it from happening again

Ordered by leverage:

1. **Gate every Claude-spawning path through the capacity book.** The rule in
   AGENTS.md already says run `openagents capacity check claude` before
   launching agents. Extend enforcement to Claude Code background workflows —
   the daemon dispatch path — so a blitz cannot start (or fan out) when the
   shared login is near its limit, and so the run is recorded in one place.
2. **Cap the fan-out.** Bound concurrent Claude-consuming subagents per
   session (two to four) and give each agent a turn and spend ceiling. A
   "solve every open issue" goal needs an explicit issue list and a per-issue
   budget, not an unbounded workflow.
3. **Treat a provider usage-limit as a stop, not a nap.** On limit, record it
   (`openagents capacity record`), park queued work, and require a human
   "resume" instead of auto-resuming at the reset time. Subagents should
   abort on limit errors rather than retry across windows.
4. **Kill the tree, not the parent.** Any background dispatch should track
   the full set of spawned PIDs (shell tasks, TUIs, bench scripts, pool
   services) and reap the process group when the session ends or is
   cancelled. A nightly orphan sweep over `coder-new`, `openagents-*`, and
   `claude` processes parented to launchd would have caught the 26 spinners.
5. **Fix the detached-TTY spin in `coder-new`.** The `crossterm` read loop
   must exit when the terminal fd hits EOF or error instead of spinning. This
   is a real defect worth an issue: it converted every abandoned UI into a
   permanent ~60% CPU load.
6. **Give benches a stall detector.** Batch runners like `ab.py` should abort
   when no result row lands for a bounded interval, in addition to their
   dollar caps.
7. **Restrict machine-enrolling and cloud-resource creation to interactive
   sessions.** `openagents mac serve`, pool registration, and VM launches
   should not run inside an unattended background agent. Cloud resources get
   a run-id tag and a manifest so a teardown script can stop every machine a
   run created.
8. **Keep a kill-switch runbook.** One script or doc section that lists every
   consumer class — `claude` daemon roster and job state, `claude` and
   `coder-new` processes, `openagents mac serve`/`host` services, launchd
   agent doors, and `openagents computer list` pool membership — and stops
   them in one step. This audit's investigation took ~20 minutes of manual
   forensics; it should take one command.
9. **Alert on the second limit hit.** One session-limit event is normal; a
   second within a window means something is looping. A cheap poller on the
   capacity book can page the owner before a third window drains.

## Follow-up status

- Items 1-3: the agent gate, `scripts/dev/agent-gate.sh` (#11430).
- Items 4, 6 and the run-id half of 7: run ids and the reaper (#11433).
  Every Claude Code session in this repository is a run under
  `~/.openagents/runs/<id>/` (`scripts/dev/agent-run.py`). Each Bash tool
  shell records its process group there, and ending the session reaps those
  groups. `scripts/dev/stop-agent-burn.sh --run ID` cancels a run by hand.
  It covers the run's processes, its child runs, and the VMs labelled
  `run-id=ID` (`h100-run.sh` and `run-env.sh` add the label). The nightly
  sweep (`scripts/dev/install-agent-sweep.sh`) stops orphaned `claude`,
  `coder-new`, bench processes and Claude Code tool shells. It also stops
  what is left of gone runs, including their VMs. `ab.py batch` aborts when no
  result row lands for `--stall-min` minutes (default 120).
- Item 5: `coder-new` exits when its terminal goes away (#11429).
