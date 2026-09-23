# View Terminal-Bench runs in the Gym terminal

The Gym terminal reads the local Harbor jobs and the sanitized Terminal-Bench
evidence retained in this repository. It runs no agent, verifier, provider
call, or container. It needs no credentials.

For text or JSON output and commands that start pinned Harbor jobs, use the
[Gym CLI](terminal-bench-cli.md).

From the repository root, run:

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
  cargo run -p gym --features tui --bin gym-terminal -- --terminal-bench
```

For a noninteractive record, add `--print`. The seven views print to standard
output. The same command works while a job is running; it reads the attempts
that the harness has already collected.

By default, the reader checks these directories in order:

1. `~/.openagents/terminal-bench/jobs/` for Harbor's versioned attempt records
   and episode manifests.
2. `bench/terminal-bench/traces/` for retained trajectories, Harbor result
   summaries, Coder One episode manifests, and component usage.
3. `bench/terminal-bench/samples/` for the checked, sanitized contract and
   resilience samples, including nested resumed trials.

The same job and trial are shown once, with the local job taking priority.
Use `--jobs-dir PATH`, `--traces-dir PATH`, or `--samples-dir PATH` to read
another location. Use `--no-jobs`, `--no-traces`, or `--no-samples` to omit
one. If a configured directory is unavailable, the overview reports that
condition; it does not silently substitute a sample.

## Read recent runs

The terminal opens on the **Runs** pane. It lists every Terminal-Bench trial
under the jobs directory and every trial retained in the checkout, newest
first, one run a line: when it started, how it came out (**passed**,
**failed**, **running**, or **not graded**), the task and what it asks, the
agent, the verifier's tests passed, the cost, and the time. Trials in
progress sort first and refresh every two seconds. A run that stopped
without a grade says why below the list: a cancelled run, a container that
failed to start, a full disk, or a provider's usage limit.

Press Enter to open a run's **summary**: a few short paragraphs built from
the run's records by fixed rules, with no model call. They say what the
task asked, what the agent did in order (Coder One's briefing, which
executor worked on it and for how long, the checks, repair, second opinion,
and persistence rounds), what the verifier found and which tests failed,
why the run likely passed or failed, and what it cost and how long it took.
Names, paths, versions, and digests stay behind `d`.

Press `t` for the **transcript**, drawn the way the Coder terminal draws a
conversation: the task, the agent's messages, each command with its output
closed until you open it, file edits, Jev's judgments as one-line decision
rows, checks and repairs as boxes, and each hand-off to an executor as a
chapter rule with its time and cost. The left margin shows the time since
the run started. Long output shows its first 60 lines and counts the rest.
It reads Coder One's episode log and each executor's native stream, Harbor's
Claude Code and Codex trajectories, and, for a trial in progress, the live
copy of the episode log or the native output.

| Key | Runs pane action |
| --- | --- |
| Arrow keys, `j`, `k` | Move. |
| `Enter` | Open a run's summary. In a transcript, open or close the selected step. |
| `t` | Open the run's transcript, or switch between summary and transcript. |
| `e` | Open or close every step of a transcript. |
| `d` | Show the details experts use in a summary. |
| `/` | Search by task, what it asks, agent, or batch. |
| `a`, `o`, `c` | Filter by agent, filter by outcome, or clear the filters. |
| `l` | Switch between newest first and most worth learning from first. |
| `x` | Mark the run, or the selected transcript step, as bad. |
| `v` | Mark the run fine: you read it and nothing is wrong. |
| `u` | Remove the mark on the run or the selected step. |
| `?` | Ask Coder One a question about the runs, with the run in view. |
| `h` | List the highlights: candidate claims worth sharing, with their evidence. |
| `Esc` | Go back. |
| `q` | Leave. |

Press `l` to order the list by what's most worth learning from, as Jev
judges each finished run. The first column shows each run's learning value
from 0 to 1, and the task column shows its strongest reasons as tags, such
as `near miss`, `stopped early`, or `contradicts: checks catch failures`.
The preview under the list names the reasons with their probabilities, and
a run's summary ends with every judgment's probability. The top rail says
how many runs Jev ranked and whether a ranking pass is running. The choice
is remembered in `~/.openagents/gym/runs-pane.json`.

In the learning order, the pane asks Jev about finished runs it hasn't
judged yet in the background, with the key in `TYPESAFE_API_KEY` or
`~/.openagents/jev.json`, and keeps the answers where `gym runs rank` keeps
them. A run is judged once and asked again only when its records change;
running trials wait until they finish. With no key, or with
`gym-terminal --terminal-bench --no-jev`, the pane asks nothing: runs
already judged still show in the learning order, and with none judged the
list stays newest first and the top rail says why.

### Mark runs

When a run, or one step of it, shows the agent doing something wrong, mark
it. In the list or a summary, `x` marks the run; in a transcript, `x` marks
the selected step. A composer opens over the pane: type a one-line note,
move through the 18 `runs-learning-v1` judgments with the arrow keys, and
press `Tab` to tag the one under the cursor, so the mark can say "this is
`unearned_success`." `Enter` saves and `Esc` cancels. Pressing `x` on a
marked run or step opens the composer with its note and tags, and saving
replaces the mark.

`v` marks the whole run fine: you read it and nothing is wrong. `u` removes
the mark on the run, or on the selected step in a transcript. The top rail
says what the last key did.

A marked run shows a flag before its task in the list, `⚑` for bad and `⚐`
for fine, and the preview under the list says who marked it, when, and
why. A run's summary has a **Marks** section, and a marked step shows its
mark under the step in the transcript.

The pane writes to the same append-only store as `gym runs mark`,
`~/.openagents/gym/marks/marks.jsonl`, so marks survive a restart, the pane
shows marks made from the command line on its next refresh, and
`gym runs agreement` measures Jev against them. Each mark carries `$USER` as
its author. See
[Mark bad runs and steps](terminal-bench-cli.md#mark-bad-runs-and-steps) and
[Measure Jev against the marks](terminal-bench-cli.md#measure-jev-against-the-marks).

### Ask Coder One about runs

Press `?` in the list, a summary, or a transcript to ask a question in
plain words, such as "why does this one rank?" or "what do the runs Jev
flagged as unearned success share?" Type it in the composer and press
`Enter`. The question goes to `coder-one ask` with what the pane shows: the
selected or open run, the step you're reading in a transcript, the active
filter, and the order. So "why does this one rank?" needs no run name.

The pane runs `coder-one ask --events` as a child process through
`supervise`, and the answer view fills in as it works: each Gym read and
Jev judgment, then the answer, drawn the way the Coder terminal draws a
reply. Each claim is marked `✓` when code checked its citations and `?`
when one didn't, with the reason. The runs the claims cite are listed
under the answer: the arrow keys choose one, and `Enter` opens it. `Esc`
from the run returns to the answer, and `Esc` from the answer returns to
the list. Press `?` and `Enter` with no question to see the last answer
again.

The pane stays a reader: `coder-one ask` only reads, inside a filesystem
boundary, and writes its record under `~/.openagents/coder-one/asks`. The
pane finds `coder-one` in `$CODER_ONE_BIN`, beside `gym-terminal`, or on
`PATH`; build it with `cargo build -p coder-one`. An ask costs about a cent
on Luna, the default, and takes under a minute; see
[Ask Coder One about runs](../coder/guides/coder-one-ask.md).

### Find highlights

Press `h` in the list for the **Highlights** view: the candidate claims
`gym runs highlights` computes, strongest first, one a line with its rule
and sample size. A claim that rests on one run shows `n=1` and draws dimmer.
Under the list, the selected claim reads in full with its key, its sample
size, how many runs it cites, and every caveat. Code computes the claims
from the runs, Jev's stored answers, the leaderboard, and the marks when
you press `h`; no model writes a number, and nothing posts anywhere.

Press `Enter` to open the cited runs: the list shows only the runs the claim
cites, and the top rail names the claim. A claim that cites one run opens
that run. `c` clears the filter. `h` or `Esc` leaves the view. See
[Find highlights worth sharing](terminal-bench-cli.md#find-highlights-worth-sharing)
for the rules, and
[Draft highlights](../coder/guides/coder-one-ask.md#draft-highlights) for
turning chosen claims into short drafts.

The expert views below stay behind their keys; `Esc` in any of them returns
to the Runs pane. `gym runs` prints the same list and summaries as text; see
the [Gym CLI](terminal-bench-cli.md#read-recent-runs) and
[Rank runs by what's worth learning from](terminal-bench-cli.md#rank-runs-by-whats-worth-learning-from).

## Navigate the views

| Key | Action |
| --- | --- |
| `1` to `9` | Open overview, comparison, attempt, evidence, history, runbooks, components, requirements, or mini-tasks. |
| `f` | Open the live view, which reads attempts in progress again every two seconds while it is open. |
| `Tab`, `h`, `l` | Move between views. |
| `j`, `k`, arrow keys, `g`, `G` | Move the selection. |
| `Enter` | Open a selected group, attempt, or its evidence. |
| `Esc` | Go back to the Runs pane. |
| `q` | Leave and restore the terminal. |

**Overview** lists every task and arm with its attempt count, verifier
rewards, terminal statuses, cost sources, and evidence health. Oracle and
`nop` controls are counted separately from agent runs. The header lists
usage coverage, reader errors, and the label of a saved
`openagents.tbench.report.v1` report when one exists.

**Comparison** shows attempts under one task, arm, and recorded pin. It keeps
each reward next to its status, agent and total time, price source, tokens,
and evidence health. The selected attempt shows its model, artifact, timing
phases, call counts, and count semantics. Other arms on that task appear below the attempts;
their different or unknown pins are named. Three or more fresh, graded,
binary attempts produce an observed pass fraction and a Wilson 95% interval.
This requires a complete commit, checksum, architecture, host, image state,
model, and artifact identity.
It is still a small development sample, not a promotion verdict. A single
trial has no pass-rate claim.

**Attempt** separates verifier reward from terminal status and shows the
task commit, checksum, architecture and image state when recorded, agent and artifact
identity, each timing phase, token coverage, total cost and provenance,
component costs, call counts, and recorded notes. A completed agent with no
verifier reward reads as unverifiable. A reward of zero reads as a task
failure, while a missing reward stays unknown. Below the counts, the
**episode timeline** lists every component invocation in start order:
setup, probes, each Jev request, the briefing, the executor session, and
the closing check, each with its duration, outcome, cost, and the spend
accumulated so far. When `coder-one checks recover` checked the attempt,
its **requirement coverage** appears above the timeline: each requirement
with its scenarios and verdicts, and the diagnostic packets. It reads
`~/.openagents/coder-one/checks/`; `--checks-dir PATH` reads elsewhere and
`--no-checks` omits it. The timeline reads the attempt's invocation log when one was
retained and derives the timeline from trajectory steps otherwise, and it
says which. An interrupted episode's timeline reads as incomplete.

**Evidence** lists every retained file and its path. It checks SHA-256
against the bytes that exist now when a manifest supplies a digest. Missing,
unresolved, edited, and unchecked files have separate labels. An absent
optional collection-failure marker reads as **none recorded**. The selected
file's full path appears below the list. Sanitized samples can differ from
the original manifest digest; the TUI labels those files **sanitized copy**
instead of claiming the original file still verifies.

**History** lists all attempts by start time, including failed, refused,
timed-out, and unverifiable runs. Enter opens the selected attempt.

**Components** (`7`) lists each Coder One component with its latest
isolated suite from `~/.openagents/coder-one/components/` beside its
invocations across episodes: Jev mode, fixtures, errors, latency, cost, the
metric summary, each fixture's output digest and metrics, and the episode
count, latency, cost, and timeline source. `--runs-dir PATH` reads runs
from elsewhere and `--no-runs` omits them. The
[component guide](../coder/guides/coder-one-components.md) records runs.

**Mini-tasks** (`9`) lists Coder One's mini-task runs from
`~/.openagents/coder-one/minitasks/`, labeled as mini-tasks rather than
Terminal-Bench attempts, with each run's task, executor, outcome, and grade.
Below the list, the selected run shows its session-control actions, its
executor events by kind, its requirement coverage when checks ran, and its
invocation timeline.
`--minitasks-dir PATH` reads runs from elsewhere and `--no-minitasks` omits
them. The [mini-task guide](../coder/guides/coder-one-minitasks.md) records
runs.

**Runbooks** lists the harness, host, delegate, results, and episode-contract
documents beside the operating sequence and the rules for reading a number.
Below them, the executor capability matrix shows which session capabilities
each adapter demonstrated: start, observe, stop, resume, and steer. It reads
`~/.openagents/coder-one/capabilities.json`, which `coder-one capabilities`
writes; `gym coder capabilities` prints the same matrix with the tests behind
each cell.

**Live** (`f`) follows Coder One attempts while they run. It lists mini-task
runs whose log has no manifest yet and Terminal-Bench trials whose host tail
is still following, then attempts that ended in the last 15 minutes. Each
in-progress attempt shows its current component, the executor's latest
events, the latest judgments, spend so far, and the time since its last
event. An attempt with no event for two minutes reads as `STALE`. A trial
also shows when the host last polled the container, so a quiet executor
reads differently from a stalled tail. The view reads again every two
seconds while it is open and says how old the current read is.

The CLI prints the same view:

```sh
gym coder live                      # one read
gym coder live --follow             # read again every 2 seconds
gym coder live --follow --json      # one JSON line per read
```

For a trial, the Harbor adapter copies the new lines of the container's
episode log to `<trial>/agent/live/episode.atif.jsonl` every
`live_interval_sec` seconds (10 by default; 0 turns it off). Each poll is
one exec that reads at most 256 KiB past the copy's offset, and the copy
keeps whole lines only and stops at 64 MiB. `live/status.json` records the
tail's state, offset, poll count, and last poll.

## Read the numbers

The TUI reads `openagents.tbench.attempt.v1`,
`openagents.tbench.episode-manifest.v1`, and
`openagents.tbench.report.v1` where available. It also reads the retained
Harbor and Coder One episode files. It does not convert coding episodes into
the Gym's `openagents.gym.eval_row.v2` decision rows, and it does not claim
that Harbor's files have the Gym result store's receipt chain.

The TUI displays `—` for an unmeasured value and `0` only for a measured
zero. Cost labels distinguish a provider report, a price estimate, and a
manual list price. The manual GPT-6 rates in
`bench/terminal-bench/profiles/manual-prices.json` reproduce the rates and
date in the [operating runbook](../terminal-bench/runbook.md); they are a
subscription reference price, not a bill. Coder One's generation, Jev, and
delegate components come from its retained `usage.json` when present.

Some older retained trajectories have no attempt record or Harbor result.
They stay visible with unknown reward, cost, timing, or pin fields. A
trajectory's first and last timestamps are not a substitute for Harbor's
phase timing. Grouping by recorded commit, task checksum, architecture, host,
image state, model, profile, and artifact keeps known pin differences apart.
When one of those identities is
unknown, the reader keeps separate jobs in separate groups and makes no
controlled interval claim. Consult the
[results page](../terminal-bench/README.md) for the written analysis and
the [harness runbook](../coder/terminal-bench.md) for running new trials.
