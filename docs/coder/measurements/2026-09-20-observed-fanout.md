# The observed six-Devin episode

On 2026-09-20, CoderBench drove `coder -p` from the task's operator sentence.
Coder selected `delegate-fan-out`, ran six real Devin sessions, and returned
six answers matching the task's independently pinned expectations. The live
driver observed an unchanged workspace and a successful exit. The run took
**48.4 seconds**, received verdict **passed**, and reported **no faults**.

## Evidence and provenance

- [ATIF log](../../../crates/coderbench/goldens/devin-fan-out-six.atif.jsonl):
  copied byte-for-byte from Coder's output, without rewriting prompts or answers.
- [Golden metadata](../../../crates/coderbench/goldens/devin-fan-out-six.meta.json):
  `observed`, orchestrator `coder`, session `20260920T074515Z-9a4668b0`.
- [Captured live grade](../../../crates/coderbench/goldens/devin-fan-out-six.grade.txt):
  the driver's actual output and preflight results.
- [Evidence summary](../../../crates/coderbench/goldens/devin-fan-out-six.evidence.json):
  decision answers, delegation results, and the observed grade.
- [Task manifest](../../../crates/coderbench/tasks/devin-fan-out-six/task.json):
  the exact questions and independently checked answers.

The measured checkout was clean at
`34df6bc026aa68947979f172614a2ae533e4ed77`. The task's base was advanced to that
commit after checking all six answers against its files. Questions, answer
expectations, required decisions/checks, write allowance, and terminal-outcome
rules were unchanged. The old staged trace was replaced, not relabeled or
kept as a second current golden. Its historical measurement sidecar remains
[separately available](2026-09-20-staged-fanout-evidence.json).

The retained ATIF bytes have SHA-256
`be816833193205cb511af43326f8aa7972d94aaf81ce6c1708566edb8d23c0db`.
The driver currently does not serialize raw before/after filesystem snapshots.
The captured grade records their live comparison; the summary is not a new
independent offline observation. `coderbench diff` therefore still returns
`unverifiable` for this trace. A successful `run` and an offline `diff` have
different evidence, and the regression tests preserve that distinction.

## What ran

Hosted Jev, reporting `jev-1.13.0`, selected `delegate-fan-out` with reported
probability 0.96 and confidence 0.93. It answered `independent` at 0.94.
The program also recorded its completion judgments, but the grade checked
answers against the manifest rather than accepting the model's judgment as
correctness evidence.

Admission recorded concurrency, isolation, and minutes as host-enforced.
No executor-declared bound was treated as verified enforcement. The approved
Devin CLI was version `3000.10.31`; each delegation ran in its own worktree
through the macOS `sandbox-exec` write boundary. The trace records that boundary
for all six calls. Adapter state and traces lived outside the measured checkout;
read-only worktrees were cleaned up before the episode was graded.

| Question | Verified answer |
| --- | --- |
| Top-level public structs in `atif/src/document.rs` | `5` |
| Partition wire labels, in order | `calibration, development, locked` |
| Estimator variants, in order | `L1, L2, L3` |
| Distinct Kev checkpoints in the model cards | `4` |
| NIP-PRG event kind | `30182` |
| Shell `ROUNDS_MAX` | `3` |

## Door choice and limits

The earlier eight-item lookup panel measured Kev-4b and Kev-0.5b; its rows
and verdicts remain in the historical sidecar. The later
[program-selection measurement](../../decision-models/2026-09-19-program-selection.md)
includes hosted Jev, a constant baseline, and real-turn error structure.
Neither a small lookup panel nor this successful episode establishes that a
local door can safely judge arbitrary task independence. The measured collision
failures behind #9414 still apply. This episode uses hosted Jev and six tasks
that are independent by inspection.

This proves the local subprocess delegation path, including the live adapter
under the host boundary. It does not prove relay transport, write-task conflict
recovery, arbitrary-backlog independence, or an improvement over a sequential
baseline. #9435 owns the remaining relay/worker proof; #9413 starts with manually
selected nonconflicting issues and independence judgments in shadow mode.
The write boundary does not restrict reads or network access.

To reproduce, build Coder, CoderBench, and capability-trust at the recorded
source revision; create a clean checkout of the pinned task base; approve the
local adapter with only its required external state directories writable;
and configure hosted Jev through the machine's existing credential mechanism.
Keep trace output outside the measured checkout and run:

```sh
coderbench run /absolute/path/to/task.json \
  --repository /absolute/path/to/clean-checkout \
  --coder /absolute/path/to/coder \
  --trace /absolute/path/to/new-run.atif.jsonl
```

A fresh trace path is required. Do not disable preflight, change the expected
answers to match a run, or treat the metadata sidecar as a substitute for a live
driver observation.

## Second observation, Linux host

The same task passed on a Linux host on 2026-09-20 with the `bwrap`
boundary backend, at source revision `85012cfd3` plus the worktree change
this note ships with. Coder answered in 38.3 seconds; CoderBench verified
six of six delegations against the task's expected answers and an unchanged
workspace, and exited `0`. Trace SHA-256:
`89151915a286dc6509b3694b4d1da5a1651a72c0ff427add3fbe6bcab81a9735`.

The first Linux attempt at `85012cfd3` failed with three faults, all Coder's
own residue rather than a delegate's: `.coder`, `.coder/worktrees`, and
`.git/coder-worktrees.lock` were created and left behind. The macOS golden
did not see them because that checkout already held them. Coder now locks
the common Git directory itself and removes the empty checkout parents when
the last checkout leaves, so the workspace comparison reads clean on a fresh
clone. Read [`delegate.md`](../delegate.md#coordinate-checkout-creation-and-cleanup).

One environment fact the preflight depends on remains: the Devin CLI's
state directory must sit inside the approval's writable grant
(`XDG_DATA_HOME`), as [`worker-executor.md`](../worker-executor.md)
describes. The `origin` check reads the checkout's configured
`remote.origin.url`, so a host whose Git configuration rewrites URLs
through a proxy no longer needs `GIT_CONFIG_GLOBAL=/dev/null` for the run.
