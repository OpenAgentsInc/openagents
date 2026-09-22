# View Terminal-Bench runs in the Gym terminal

The Gym terminal reads the local Harbor jobs and the sanitized Terminal-Bench
evidence retained in this repository. It runs no agent, verifier, provider
call, or container. It needs no credentials.

From the repository root, run:

```sh
CARGO_TARGET_DIR=~/.cache/openagents/gym-target \
  cargo run -p gym --features tui --bin gym-terminal -- --terminal-bench
```

For a noninteractive record, add `--print`. The six views print to standard
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

## Navigate the views

| Key | Action |
| --- | --- |
| `1` to `6` | Open overview, comparison, attempt, evidence, history, or runbooks. |
| `Tab`, `h`, `l` | Move between views. |
| `j`, `k`, arrow keys, `g`, `G` | Move the selection. |
| `Enter` | Open a selected group, attempt, or its evidence. |
| `q`, `Esc` | Leave and restore the terminal. |

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
failure, while a missing reward stays unknown.

**Evidence** lists every retained file and its path. It checks SHA-256
against the bytes that exist now when a manifest supplies a digest. Missing,
unresolved, edited, and unchecked files have separate labels. The selected
file's full path appears below the list. Sanitized samples can differ from
the original manifest digest; the TUI labels those files **sanitized copy**
instead of claiming the original file still verifies.

**History** lists all attempts by start time, including failed, refused,
timed-out, and unverifiable runs. Enter opens the selected attempt.

**Runbooks** lists the harness, host, delegate, results, and episode-contract
documents beside the operating sequence and the rules for reading a number.

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
