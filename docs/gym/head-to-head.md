# Replay traces head to head

Open Gym, press `p` from the Runs list or an open run, and choose a task and
an attempt on each side. The left side defaults to Coder One; the right
side defaults to the public Claude Code / Fable 5.1 attempts, with max
first. All versions and repetitions remain separate choices. A task with
only one available side can play on its own.

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
  cargo run -p gym --features tui --bin gym-terminal -- \
  --terminal-bench --head-to-head --no-jev
```

Replay reads files. It does not rerun commands, invoke a model, or charge
for inference. `--print` prints the selection screen without entering the
terminal. The ordinary Runs view remains available with Escape.

## Choose a comparison

| Key | Action |
| --- | --- |
| `Tab`, left/right arrows | Select the task, local attempt, or opponent column. |
| Up/down arrows, `j`/`k` | Select an item in the focused column. |
| Page Up/Down, Home/End | Move through a long list. |
| `/`, then a task name | Filter tasks; Enter finishes the search. |
| `c` | Clear the task filter. |
| `a` | Switch the left side between Coder One and all local agents. |
| `o` | Switch the opponent between public Fable attempts and local attempts. |
| `Enter` | Load the selected transcripts, initially paused. |
| `Esc` | Return to Runs. |

The picker combines the normal jobs and retained-trace catalog with the
optional benchmark-host mirror. It deduplicates by job and trial. A
current local job takes precedence; the host mirror takes precedence over
a smaller committed bundle. Outcome, model, effort or Coder version, and
attempt identity remain visible. This is a trace comparison, not a claim
that different models, budgets, or harness settings were controlled.

## Play and inspect

Both sides use one elapsed-time clock. Their calendar dates can differ;
each starts at its recorded agent start, including any earlier task or
host records. When one finishes, its transcript stays on screen while the
other continues.

| Key | Action |
| --- | --- |
| Space | Play or pause both sides. |
| `+` or `=` / `-` | Increase/decrease speed: 1×, 2×, 5×, 10×. |
| Left/right arrows or `[`/`]` | Seek backward/forward 30 seconds. |
| `n` / `b` | Seek to the next/previous event on either side. |
| `r` or Home | Restart from zero at the current speed. |
| End | Reveal the complete transcripts and pause at the end. |
| Tab | Switch the focused transcript. |
| Up/down arrows, `j`/`k`, Page Up/Down | Scroll the focused transcript independently. |
| `d` | Switch the focused side between readable conversation and complete records with metadata. |
| `g` | Scroll to the beginning of the revealed transcript. |
| `f` or `G` | Resume following the focused transcript's newest event. |
| Escape | Return to the task and attempt picker. |
| `q` | Quit. |

Message text, reasoning present in the source, system messages, tool
arguments, and tool results are kept without the ordinary transcript
view's line limits. Long output remains scrollable. Image payloads are
represented by their encoded size and digest; the complete image bytes
remain in the source JSON. Terminal control characters are escaped.

Timing labels distinguish evidence:

- **Step timestamp:** Harbor timestamps a whole ATIF step. Its tool
  results share that step's timestamp; the source does not establish a
  separate completion time for each result.
- **Message timestamp:** a native executor message has its own timestamp.
- **Host timestamp / host receipt:** Coder's append-only log records the
  event or the native stream line arriving. Native results remain separate
  events from their calls.
- **Estimated / untimed:** a native line has no timestamp or exact host
  receipt. It uses the next available receipt, the last receipt after the
  last known event, or the preceding event when no anchor exists. The
  per-side counter reports these events. This is not token streaming.

Older or interrupted runs may retain only a prefix or host summaries.
Replay cannot reconstruct text the source never recorded. A missing or
corrupt source produces a visible notice, not an empty successful run.
Public files must match the manifest's SHA-256 before loading.

If the screen is blank at `00:00:00`, check the event count. A loaded trace
can have `0 / N events` because replay starts paused before its first
recorded event. Press Space to play, `n` to jump to the first event, or End
to reveal the full transcript. The pane now explains this waiting state.

The public attempt list is committed to Git; the transcript bodies are
downloaded separately on each computer. `[not on this computer]` in the
picker means the attempt is listed but its transcript file is missing
locally. A failed side displays its cause and acquisition command inside
the pane. After downloading, press Escape and Enter to reload the pair.

## Public Fable collection

The September 23 acquisition contains all five Fable 5.1 rows on the
[public Terminal-Bench 4.0 leaderboard](https://www.tbench.ai/): 66 tasks ×
five attempts × five effort settings, or **1,650 listed attempts**.
There are **1,649 published trajectories**, totaling **6,014,416,668 bytes**.
All published steps have timestamps.

| Effort | Listed attempts | Available transcripts |
| --- | ---: | ---: |
| max | 330 | 330 |
| xhigh | 330 | 329 |
| high | 330 | 330 |
| medium | 330 | 330 |
| low | 330 | 330 |

The missing `jax-speedrun-gpu` xhigh attempt is
[`407a3728-44a6-4318-9b12-2b4bf5e89dab`](https://hub.harborframework.com/trials/407a3728-44a6-4318-9b12-2b4bf5e89dab).
It ended with `NonZeroAgentExitCodeError`. Its published archive has empty
agent logs and no trajectory. The manifest retains its identity, outcome,
missing status, and the inspected archive's digest.

Fable max is tied for rank 2 in this snapshot, at 191/330 passes; Astra max
is first at 192/330. The collection is explicitly Fable's, and is not
silently replaced when the leaderboard rank changes.

The committed [manifest](../../bench/terminal-bench/reference/fable-5.1-replays.json)
pins membership, sources, effort settings, rewards, timestamps, costs,
sizes, and SHA-256 digests. The 6 GB of transcript bodies are already
loaded on the acquisition machine at
`~/.openagents/terminal-bench/public-replays/`; they are not Git objects.
Gym opens them lazily when you choose a pair. To load the same collection
on another machine:

```sh
cd bench/terminal-bench
uv run python -m tbench.public_replays
```

On September 24, the same complete published collection was also copied
to `coderos` at the default cache path. Pulling the repository alone had
left that computer with the attempt list and no public transcript files.
All 1,649 published files passed SHA-256 verification there. A separate
terminal session on that computer loaded both sides of
`data-anonymization` and revealed the selected local attempt's 1,700 events
and Fable attempt's 38 events. Existing Gym sessions can load the files
with Escape and Enter; the clearer waiting and failure messages require
the updated binary.

The downloader resumes verified files, retries transient download
failures, and reports missing evidence. It uses Harbor's public read and
storage APIs. Add `--discover` to refresh membership from the live
leaderboard; doing so updates the manifest and requires reviewing its
changes. `--cache PATH` selects another cache; set
`GYM_PUBLIC_REPLAYS_DIR=PATH` when opening Gym with that cache.

## Benchmark-host mirror

The initial `coderos` snapshot contains **719 attempts and 5,845 files**
(1,606,317,589 bytes), including 533 episode directories and 631 native
executor streams at inventory time. It is stored at
`~/.openagents/terminal-bench/replay-jobs/`, outside Git. It includes the
host's available Coder versions, other local agents, and unfinished
attempts, with the latter limited to the captured prefix.

Refresh it through the host's SSH name or Tailscale address:

```sh
cd bench/terminal-bench
uv run python -m tbench.sync_replays HOST
```

This reads existing files only. It does not modify the execution host or
start, stop, or regrade a benchmark. It copies transcript and result
records, not workspaces or credentials; configuration files contain only
agent and task identity fields. The local directory is private. Restart
head-to-head mode after a sync to reload the inventory. The mirror is a
snapshot, not a live remote stream.

## Verification

Unit tests cover clock alignment, pause and speed, seeks and end-of-run
behavior, complete long output, separate native result timing, missing
timestamps, image references, integrity checks, selection, independent
scrolling, narrow terminals, and existing trace formats. Acquisition tests
check metadata filtering, verified resume, missing publication, and path
boundaries.

The opt-in acceptance test reads the downloaded corpus through the Rust
replay reader and renders an actual Coder/Fable pair at 10×:

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
GYM_REPLAY_AUDIT_DIR=/tmp/gym-replay-acceptance \
  cargo test -p gym --features tui --test replay_acceptance -- \
  --ignored --nocapture
```

The output directory contains `replay-audit.json` and `replay-screen.txt`.
The test requires acquired evidence and stays separate from the ordinary
unit suite. [Issue #9580](https://github.com/OpenAgentsInc/openagents/issues/9580)
records delivery and the manual verification result.

The [retained acceptance record](measurements/2026-09-23-head-to-head.json)
reports 753 distinct local entries, 675 replayable local transcripts, and
all 1,649 published public transcripts: 236,955 events in total. The 78
local entries without a replayable transcript remain in the catalog;
they include no-op controls, setup failures, interrupted attempts, and
three malformed old Codex trajectories. A real terminal session replayed
`react-lead-form` at 10×, paused, scrolled both sides, returned to the
picker, and exited successfully.

The initial replay suite passed all 11 tests, and the acquisition suite
passed all three tests. The full manual gate
(`./scripts/verify-rust.sh --keep-going`, run
`20260924T005504Z-65d135`) passed formatting, default and feature Clippy,
dependency policy, tooling checks, and PostgreSQL acceptance. Both
workspace test phases stopped at Coder One's
`evidence_puts_the_contradicted_requirement_first_and_scrubs_scratch_paths`:
the scratch path assertion fails with this Mac's temporary directory. The
same test fails in an unchanged checkout of `40925eb882`.

The separate Gym feature suite passed 489 tests and failed eight existing
catalog, recorded-judgment, and list-display fixture tests. Their fixtures
depend on task instructions from the benchmark host that are not present
at the recorded paths on this Mac. The acceptance record lists these
failures and the baseline checks. These failures mean the full repository
gate is **not green**, despite the replay-specific checks passing. Metal
checks and the optional long relay soak were not run.

The September 24 follow-up passed all 13 replay tests. The additional
regressions reproduce a new computer with no public files, verify that the
failed pane shows the download and reload instructions, load the same pair
after acquiring the missing file, and explain the pause before the first
recorded event. Strict all-target Gym Clippy and workspace formatting also
passed after the UI changes.
The full follow-up gate, `20260924T055413Z-982924`, again passed the
non-test phases, including PostgreSQL acceptance. Both workspace test
phases stopped at the same pre-existing Coder One scratch-path assertion
described above; the full gate remains failed.
