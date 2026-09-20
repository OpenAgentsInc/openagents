# CoderBench, and the first recorded episode

`crates/gym` scores a door on one item. `crates/coderbench` scores an
episode: the whole path from the operator's sentence to the final summary,
recorded as one ATIF trace.

The first task is `devin-fan-out-six`, the delegation
[`programs.md`](programs.md) specifies. Its golden is now **observed**:
Coder drove six real Devin sessions on 2026-09-20 at `34df6bc026`, and
CoderBench verified all six answers, an unchanged workspace, and a successful
exit in 48.4 seconds. Read [the observed run record](coder/measurements/2026-09-20-observed-fanout.md)
for the trace, captured grade, provenance, and limits.

The original shell-driven recording was staged. It has been replaced, not
relabeled. Historical measurements below describe those earlier runs; their
[original evidence](coder/measurements/2026-09-20-staged-fanout-evidence.json)
is retained separately from the current golden. An offline `diff` of the
observed trace remains unverifiable because it cannot observe the live exit
or workspace snapshots.

## Running one

```sh
coderbench run devin-fan-out-six
coderbench diff devin-fan-out-six runs/one.atif.jsonl
```

`run` reads the manifest, checks what the task requires, drives `coder -p`,
reads back the trace it named, judges it, and prints every fault. `diff` is
the same path with the run step removed, for a trace somebody already has.

| Flag | Effect |
| --- | --- |
| `--repository <DIR>` | The checkout to run in. Default: this directory. |
| `--coder <PATH>` | The `coder` binary. Default: `CODERBENCH_CODER`, the binary beside this one, then `PATH`. |
| `--trace <PATH>` | Where the run's trace lands. Default: a new file under `~/.openagents/coderbench/`. |
| `--timeout <SECS>` | Override the task's own timeout. |

The trace is named rather than searched for, which is what
[`--trace`](coder/headless.md) is for, and the path must not already exist:
a session never writes over another session's record. Standard output and
standard error land beside it as `<trace>.stdout` and `<trace>.stderr`, so
what the agent said is readable next to what it did. Those two files are
part of the run's record and are kept; a preflight probe's output is read
under the same 64 KiB capped capture every supervised job uses, and a
probe that overruns it is marked truncated rather than read as clean.

A run that passes its timeout takes its whole process tree with it, so an
agent stopped on one task is not still running against the repository
during the next one. Read [`coder/subprocesses.md`](coder/subprocesses.md).

| Exit code | Meaning |
| --- | --- |
| `0` | The run took the path the task expects, and the evidence shows it. |
| `1` | The run left the path. Every fault is printed. |
| `2` | The machine does not hold what the task requires. Nothing ran. |
| `3` | There is no trace to judge. |
| `4` | The evidence a judgment needs is missing. Not a pass. |
| `64` | The command line was wrong. |

A run that times out with a readable, complete trace still fails its
allowed-ending check and exits `1`. If it times out before creating any
trace, it exits `3`: there is nothing to judge. Both cases terminate the
supervised process group before returning. A complete trace does not turn
a timeout into a successful run.

The complete-trace timeout regression supplies the finished driver result
at the observation boundary. A separate CLI test starts a process that
never writes a trace and checks exit `3`. This keeps trace publication
from racing a one-second deadline, which caused the original combined
fixture to fail under parallel workspace tests
([#9462](https://github.com/OpenAgentsInc/openagents/issues/9462)).

## A grade answers with three values

`failed` beats `unverifiable` beats `passed`. The vocabulary and the
precedence are `crates/gym`'s `gate::Verdict`, and this crate uses that type
rather than a second word for the same idea. Each fault carries its own
verdict, and the run's verdict is the strongest one in the list.

The split is what the grade is for. These are different states, and only one
of them is a fault in the agent:

- A delegation the trace records as wrong is **failed**.
- A delegation whose answer is not the one the task owns is **failed**,
  when the task owns one — see the next section.
- Without a manifest expectation, a delegation that recorded no correctness
  either way is **unverifiable**. Nobody checked it. Six of those are not six correct answers, and a task
  that requires six correct answers does not get them from six silences.
- A trace nobody compared against the workspace is **unverifiable** about
  writes. An absent `wrote` field is a run that said nothing, not a run that
  wrote nothing.

Missing evidence never passes. That is the whole of audit finding A04
([#9418](https://github.com/OpenAgentsInc/openagents/issues/9418)): a
constructed run with six empty ungraded delegations, null decision answers,
and the required check names drew no faults at all from a task requiring six
correct delegations. `crates/coderbench/tests/negative.rs` holds that run and
the other cases that must not grade clean.

### What the grade reads

| Evidence | Where it comes from | Missing means |
| --- | --- | --- |
| Call outcome | each call's `outcome` in the trace | a failed check or delegation is a fault, not a name that was present |
| Delegation answers | the call's `arguments.prompt` and `output`, against `grade.expects` positionally | a wrong or misplaced delegation is **failed**; a self-asserted `correct` flag is a claim, not the check |
| Decision answers | the decision call's `answers` | a null answer is `unverifiable`, and `grade.answers` states the predicate the run gates on |
| Step order | the order calls appear, against `grade.path` | a step that ran before one the path puts first is a fault |
| Trace integrity | the end record and the unreadable-line count | no end record is **failed**; a torn line is `unverifiable` |
| Terminal outcome | Coder's exit code, against `grade.endings` | a timed-out or declined run cannot exit clean on a partial trace |
| Writes | the checkout read before and after the run | nobody looked is `unverifiable` |

`grade.endings` defaults to `answered`. Stating it per task is what keeps a
run that ran past its timeout from grading clean because the trace it left
holds the expected names.

## Tuning: the noise floor first

Use `coderbench tune` when you need to compare repeated episodes rather than
one episode against a golden:

```sh
coderbench tune <TASK> [--against <TRACE|DIR>]... [--trace <TRACE|DIR>]... \
  [--runs N] [--repository DIR] [--coder PATH] [--timeout SECS] [--out DIR]
```

| Flag | Effect |
| --- | --- |
| `--against <PATH>` | Adds baseline traces: a file or a directory of `.atif.jsonl` files. Repeat the flag for more. |
| `--trace <PATH>` | Adds recorded candidate traces: a file or a directory. Repeat the flag for more. Nothing runs live. |
| `--runs N` | Runs the candidate `N` times when `--trace` is absent. The default is `8`. |
| `--repository <DIR>` | The checkout for live runs. |
| `--coder <PATH>` | The `coder` binary for live runs. |
| `--timeout <SECS>` | The timeout for each live run. |
| `--out <DIR>` | The directory for live traces. |

The command reports the noise floor before it reports a comparison: the
mean, sample standard deviation, and range of the episode's fault count,
steps, seconds, and verified delegations across each series, with the
baseline first. Each metric's move is then stated as inside or outside the
baseline's own spread, using `gym`'s two-sigma detectable difference for the
two sample sizes. A move inside the spread is noise, whatever its sign. A
move outside the spread is a description, not a verdict.

A *persistent* fault appears in every run of a series. An *intermittent*
fault appears in some runs, so the report states how many. A fault seen in
one run of eight is a flake to attribute, not a regression. Delegation call
IDs change between runs, so the report keys delegation faults by their
one-based slot. A series of one run has no spread and makes every fault
persistent, so it grades `unverifiable`. With `--against`, the comparison
also names the persistent faults the candidate fixed, introduced, and left.

The verdict is the `decision-v1` gate from `crates/gym/gates/` over the pass
rate, a run counting as an item. Eight runs a side are under that gate's
ten-item floor, so an eight-run comparison is `unverifiable` by design and
the report names the criterion that decided it. The standard-error criterion
also needs at least five passes and five non-passes on each side. `passed`
means the candidate's pass rate clears the baseline's by two standard errors,
`failed` means it fell, and `coderbench tune` writes no threshold of its
own. Without `--against`, the exit code follows whether the candidate series
has any persistent fault.

A trace alone cannot say how the episode ended or whether the workspace
changed. A live series observes both and writes them beside each trace as
`<trace>.observed.json`, so a series read back with `--against` or `--trace`
is judged as it was live. A trace without that file grades as it did before:
its ending unstated and its writes unobserved. A live series refuses before
the first run, with exit code `2`, on a machine that does not hold what the
task requires, the same way `run` does.

### The first live series

The change under test was `coder` between `2ff484fbc` and `c34c1aa90`, run
from a Linux terminal host with no Devin CLI on the path, over
`wss://relay.openagents.com` to the deployed `coder-worker` at `bbd6c7e93`.
Baseline first, then the candidate against it:

```sh
export CODER_RELAY=wss://relay.openagents.com \
  CODER_WORKER=2854d7da72ded5d6b62fa6107ff464129d510235c5017980c40a27cc9134f9ef \
  CODER_DELEGATE=devin-relay
coderbench tune devin-fan-out-six --runs 10 --repository ~/bench-wt \
  --coder ~/target-old/debug/coder --out ~/tune-run/baseline-2ff484fbc
coderbench tune devin-fan-out-six --runs 10 --repository ~/bench-wt \
  --coder ./target/debug/coder --against ~/tune-run/baseline-2ff484fbc \
  --out ~/tune-run/candidate-main
```

| Series | Passed | Faults/run | Steps | Seconds | Verified |
| --- | --- | --- | --- | --- | --- |
| Baseline, 10 runs | 10 of 10 | 0.0, sd 0.00 | 15.0, sd 0.00 | 22.0, sd 2.76, 19.7–27.7 | 6.0, sd 0.00 |
| Candidate, 10 runs | 10 of 10 | 0.0, sd 0.00 | 15.0, sd 0.00 | 24.9, sd 2.66, 20.5–29.5 | 6.0, sd 0.00 |

No fault was fixed, introduced, or left. Seconds moved from 22.0 to 24.9,
outside the baseline's spread; the wall clock is the slowest of six Devin
turns on the worker, and the two series ran back to back on one worker, so
the move describes the worker's afternoon as much as the terminal. The
verdict is `unverifiable`: ten items a side meets the floor, accuracy did
not fall, and the standard-error criterion needs at least five expected
outcomes on each side, which 10 of 10 against 10 of 10 does not reach. That
is the gate's answer and the report keeps it.

The series before these found the fault the tool is for. An eight-run
candidate series against the worker at `0757355c1d` passed 7 of 8: run 8
raised `no worker answered ... in 30 seconds` while the worker's journal
showed all six jobs received and answered, the slowest in 28.8 s. One run in
eight is intermittent, and the attribution was the executor door answering
in one piece with nothing crossing the relay before it. The worker at
`bbd6c7e93` publishes one kind `27000` `status: processing` as it admits a
delegation. An eight-run series against it, with the same terminal, passed
8 of 8 and recorded one worker answer at 37.4 s that the terminal waited
for. Those two series predate `<trace>.observed.json`, so read back they
carry the two faults the sidecar exists to remove and are not compared
here.

### Observe workspace contents independently

`run` uses bounded `coder-boundary` snapshots of the repository before and
after execution. The comparison sees changes to already dirty files and
ignored files, unlike a comparison of `git status` output. Creation, deletion,
renames, retypes, and metadata changes count. `grade.writes_expected` counts
distinct changed paths; a rename includes both its old and new path. A claimed
write cannot satisfy that count without independent observation.

The whole repository directory is observed, including Git metadata and build
products. Use a dedicated checkout, keep trace and build output outside it,
and prepare host-owned directories before measuring a read-only run. The
observer cannot attribute concurrent changes to a particular process. Its
bounds are 200,000 entries and 4 GiB of file contents per snapshot; an incomplete
or unreadable snapshot makes write evidence unverifiable. No ignore rule can
make an unobserved path count as clean.

### A task can own the expected answers

`grade.expects` holds the answers the task itself checked, one `{prompt,
answer}` entry per delegation, in the order the request asks the questions.
The runtime never receives them: they exist so the grade reads what the run
recorded against something the run did not write, rather than against the
`correct` flag a trace may assert about itself.

When a task states them, a delegation verifies only when all of it lines up:

- the recorded call's `arguments.prompt` is the pinned prompt, byte for
  byte — case, spacing, and wording are the question, so a prompt that
  differs in any of them is a different question however it answered;
- the call completed;
- the recorded `output` is the pinned answer after trimming whitespace from
  the ends, with case and interior spacing intact — `L1, L2, L3` and
  `l1, l2, l3` are different answers, and a task that wants a looser
  output format states that requirement in the prompt.

The list is positional, so the run's first delegation is checked against the
first entry, the second against the second, and so on. A missing,
duplicated, reordered, or substituted delegation is a **failed** fault
rather than a match wherever it lands, and a trace that calls its own answer
wrong (`correct: false`) is the record contradicting the manifest — failed,
not proof either way. There is no unverified state on this path: the task
holds the answers, so every shortfall is measured.

The contract on the manifest is equally strict, because a malformed
expectation cannot check anything: `expects` pins one entry per delegation,
so its length is `grade.delegations`; every pinned answer must verify, so
`grade.delegations_correct` is the same count; and each prompt and answer
must be nonblank and each prompt unique — an empty answer compared to an
empty output would otherwise "verify" a delegation that said nothing.
`Task::load` refuses a manifest that breaks the contract, and `judge`
reports it as a fault on a task built by hand, so neither entry point can
silently pass what it cannot verify.

A task that states no `expects` keeps the older rule: the trace's own
`correct` flag is the only correctness evidence there is, so only a
delegation the trace itself records as checked counts, and one that recorded
nothing either way is unverifiable rather than wrong.

The 2026-09-20 verification passed 58 CoderBench tests and the 22 offline
Coder program-runtime tests under Rust 1.97.1. The runtime integration test
records delegations without expected answers or correctness flags, then
checks their outputs against independent fixture expectations. Strict Clippy
passed for CoderBench. The supervisor now gates asynchronous capture helpers
and integration tests on its `job` feature, so the blocking-only build used
by CoderBench also passes without unused-helper warnings. Both supervisor
feature configurations pass their applicable tests. #9429 still tracks the
remaining workspace verification baseline. These checks verify grading
behavior. The later observed golden is described at the top of this page.

### `diff` cannot hand back a pass

A trace does not carry the exit code, and it does not carry the checkout. So
`coderbench diff` on a clean trace answers `unverifiable` with two faults,
which is the honest answer rather than a shortcoming to route around:

```text
unverifiable: 2 faults, in the order the path takes:
   1. [unverifiable] nothing compared the workspace, so writing nothing is unobserved rather than shown
   2. [unverifiable] the trace closed without saying how the episode ended; the task allows answered
```

`coderbench run` sees both. It reads the checkout before it starts Coder and
again after, and it reads the exit code, so it is the mode that can say a run
passed.

### It refuses before it runs anything

A run at the wrong commit, or without the executor the task delegates to,
produces faults that are about the machine. Somebody then reads them as
faults in the agent and spends an afternoon on it. So `run` checks the
task's `requires` first and refuses with exit `2`, naming the requirement
that failed:

- the checkout is the repository the task names — the configured
  `remote.origin.url`, so a host-wide `insteadOf` rewrite cannot change
  what the checkout says it is — at `requires.base`, with nothing
  uncommitted, because a delegate reads the working copy, and an edited
  file is not the base commit however the commit reads;
- every capability in `requires.capabilities` is installed, resolved from
  its manifest in the `capabilities/` registry and detected the way
  [NIP-CAP](../nips/openagents/NIP-CAP.md) says to detect it — under the
  same `capability-trust` approval Coder's survey needs, so a manifest
  nobody approved is reported `unprobed` with the approval path named
  rather than run. The registry is the one `crates/coder` reads, in the
  same order and under the same `CODER_CAPABILITY_DIR`, with the
  workspace's own last so a checkout pinned to a commit from before the
  registry existed still resolves.

Every requirement is printed, met or not, because the faults underneath
mean one thing at the base commit and another thing anywhere else.

This is the harness checking its own preconditions, not the capability
probe Coder owes its own runs. When Coder grows one, the probe becomes a
step in the trace and this stays what it is: the reason the run was worth
starting.

### Faults read in path order

`grade.path` states the steps in the order a correct run takes them, and the
fault list follows it. The first fault is then the earliest thing that went
wrong rather than the first thing the checker happened to test, which
matters most when a run has no path at all: a missing probe above the
missing delegations it explains reads as a work list, and the reverse reads
as noise.

## What ran

Six read-only questions, one per file, delegated to the Devin CLI on this
computer, in parallel.

| | |
| --- | --- |
| Executor | `devin 3000.10.31`, resolved at `~/.local/bin/devin` |
| Delegations | 6, in parallel |
| Correct | **6 of 6** |
| Wall clock | 46.6 s |
| Summed agent time | 80.7 s |
| Files written | 0 |

## What the decision models were asked

**Program selection**, on kev-4b: `delegate-fan-out` at confidence 0.83.

**Independence**, on kev-4b, over three `Noul` questions:

| Question | Answer | Correct |
| --- | --- | --- |
| The six tasks can run in parallel without colliding | **0.93** | yes |
| Every task is read-only and writes no file | **0.17** | **no** |
| At least one task needs a tool restriction | 0.53 | no signal |

The door got the hard question right and the easy one wrong. This is recorded
rather than smoothed over, because a golden that showed only the flattering
half would be worth nothing.

It did not affect the run: the fan-out gated on `independent`, and admission
gated on the deterministic bounds check rather than on
`needs_tool_restriction`, which carried no signal anyway.

It does not affect the grade either, and that is deliberate. The task's
`grade.answers` names two predicates — the program choice, and `independent`
at 0.7 or above — because those are the two the run acts on. Grading
`readonly` here would measure the door rather than the path, and the door is
`crates/gym`'s subject. A task predicate the run never reads would fail a
correct path for a wrong answer nothing depended on.

### Two corrections to this call

Both from
[`decision-models/2026-09-19-restatement-and-polarity.md`](decision-models/2026-09-19-restatement-and-polarity.md),
which measured it.

**This call does not replay.** `kev-serve` is deterministic, and the program
call above reproduces exactly. Posting the `state` and `questions` this
golden records to the same door returns `readonly` at **0.76**, not 0.17, on
**72** input tokens against the 166 the golden records. The state that
produced 0.17 carried about 94 more tokens — the six delegation paths, which
the recorded `arguments` leave out. Those arguments were reconstructed by the
staging script rather than captured, and they carry no `model` field either,
which the API requires. Part of what "observed" has to mean for
[#9412](https://github.com/OpenAgentsInc/openagents/issues/9412) is capturing
the request body verbatim.

**The three recordings are one call, not three samples.** They differ only in
the one delegation path that two renames moved, and the input-token count
moves with it — 165, 165, 166. A deterministic door asked the same question
three times gives one number three times, so the 0.01 spread measures the
path string, not the door.

What survives: the door answered wrongly a question its state answered, and a
192-item panel over four doors puts that failure on the kev checkpoints
rather than on the question. Hosted Jev answers all 128 open items of that
panel correctly.

## Can a local door do the program lookup?

The question the operator asked. Eight operator sentences over four
programs, with the answer known for each.

| Door | Accuracy | Mean latency | The miss |
| --- | --- | --- | --- |
| kev-4b | **7 of 8** | 1,914 ms | confidence **0.360** |
| kev-0.5b | 6 of 8 | **212 ms** | confidence **0.980** |

**Both misses matter less than how they failed.** kev-4b's one miss was its
lowest confidence on the panel by a wide margin — every correct answer came
back at 0.97 or above, so a threshold near 0.9 would have caught it and
routed to a fallback. kev-0.5b was **confidently wrong**: it missed at 0.980,
where no threshold helps.

So a local door can do this lookup, and the smallest one cannot be trusted
to say when it could not.

**Eight items is not a measurement.** The standard error is about 0.117, so
0.875 against 0.750 is well inside the noise, and nothing here establishes
that kev-4b is better than kev-0.5b at this task. What it does establish is
that neither needs the hosted door to be tried, and that the failure modes
differ in a way worth measuring properly on a real suite.

The panel is in [`2026-09-20-staged-fanout-evidence.json`](coder/measurements/2026-09-20-staged-fanout-evidence.json).

## Two things the run found that nobody was testing for

**The capability probe cannot trust `PATH`.** The first attempt at the
delegations failed six times with `command not found: devin`. The binary is
on the operator's interactive `PATH` and not on the one a spawned subshell
inherits. [NIP-CAP](../nips/openagents/NIP-CAP.md)'s `detect` resolves a binary
rather than assuming a name resolves, and this is why. A probe that shelled
out to `devin --version` would have reported the capability absent on a
machine that has it.

**A stale checkout answers honestly and wrongly.** A warm-up delegation
asked how many crates the repository has and answered 7. That was correct
for the working copy on disk, which was 20 commits behind `main`, where the
answer is 9. The delegate read what was there. Any future task whose grade
depends on repository contents has to pin the commit, which is why the task
manifest carries `requires.repository` and why the reference
implementation's manifests carry a `base` commit.

## The first golden was deleted and re-recorded

Twice. The `nips/coder/` lane became `nips/openagents/`, and then NIP-PRO
became NIP-PRG. Each time, one of the six delegations asked about a file the
rename moved, and the recording stopped describing anything that could
happen again.

It was **deleted and the episode re-run**, not patched. A recording of a
world that no longer exists is worse than no recording, because it reads as
evidence. Editing the prompt to match today's tree would have been worse
still: the file would then say a question was asked that never was.

Each re-run pins `requires.base` to the commit it ran at. That field existed
before the first rename and was empty; the first thing to move underneath
the task is what filled it in.

**A task that reads file paths is invalidated by every rename**, which is a
property of this task rather than of goldens. It is cheap here — the episode
re-records in under a minute — and it would not be cheap for a task whose
delegates do real work. A task meant to last should ask about things that do
not move.

## What the probe learned that a present/absent check cannot say

Two findings from resolving `devin-local`, both recorded in the golden's
capability step.

**`PATH` is not enough.** The first attempt failed six times with `command
not found`. The binary is on the operator's interactive `PATH` and not on
the one a spawned subshell inherits, which is why
[NIP-CAP](../nips/openagents/NIP-CAP.md)'s `detect` resolves an absolute
path rather than assuming a name resolves.

**A present executor can still refuse.** The re-run was first attempted from
a git worktree under `/private/tmp` and was refused six times out of six:
`Refusing to run in an untrusted workspace`. The capability was installed,
detected, and unavailable for that directory.

That is a state a present-or-absent probe cannot represent, and it is not
rare — an executor that sandboxes itself will have opinions about where it
runs. The task manifest gained `requires.capabilities_refuse` for it, and a
manifest under NIP-CAP should say what its executor declines as well as what
it cannot enforce.

## The first run Coder drove

On 2026-09-19 the episode ran from a sentence for the first time.
`coderbench run devin-fan-out-six` drove `coder -p` against a checkout at
the pinned base, and the turn — not a shell script — probed the machine,
read the registry, asked which program the request wanted, and ran it. The
first run, step by step:

| Step | What it did |
| --- | --- |
| `program` | `delegate-fan-out` at 0.96, with `none` at 0.02 |
| `task_select` | 6 of 6 tasks, read from the list the request carries |
| `independence` | `independent` 0.95, clearing the 0.7 floor |
| `admit` | `minutes` kept by `devin-local`, the rest by the host |
| `fan_out` | 6 of 6 answered at a width of 6 |
| `accept` | 6 answers, one per requirement |

It ran three times: 25.6 seconds, then 52.9 after the host's execution
permit landed, then 214.0 against 932.6 seconds of summed agent time on a
machine running several agents at once. The wall clock is the slowest
delegate and the executor is not fast twice in a row. All three took the
same path and answered all six questions the same way — `5`, the three
partition names, `L1, L2, L3`, `4`, `30182`, `3`.

**Nothing on the path is a fault.** Every decision the task names was asked
and held its predicate, every check ran, the workspace is unchanged, and the
session ended `answered`.

The grade is `unverifiable`, and the reason is worth reading rather than
working around. [#9418](https://github.com/OpenAgentsInc/openagents/issues/9418)
made a delegation count only when the trace says it completed, says it was
correct, and holds an answer somebody checked. **A task read out of an
operator's sentence carries no expected answer**, so `Delegation::correct`
is `None` and six delegations record nothing either way. The six answers
were right; nothing in the run establishes that, which is exactly what the
grade said.

The question that run left open — where the expected answers come from when
the request does not carry them — is answered: the manifest owns them.
`grade.expects` pins each question's prompt and answer in order, verified
against the sources the questions name, and the grader checks the recorded
calls against them positionally (see "A task can own the expected answers").
The alternative — letting the `accept` step's decision be the evidence —
was considered and set aside for this task, because it inherits the door's
judgment rather than checking anything. At that point the staged golden
failed because its prompts differed from the manifest's. The later observed
run uses the request's exact questions and passes the live grade; its record
is linked at the top of this page.

One smaller thing changed with this run: the task's sentence now carries its
six questions. The `select` step names the `request` source, and the request
carries the list the sentence writes out — a task manifest that wants its
work found instead can name a file source. The staged golden's directive is
the older one-line version.

## What is not tested yet

The six tasks are independent **by construction** — each reads a different
file — so the independence decision had an easy instance and a known answer.
The interesting case is six tasks that are *not* independent, where the
decision has to refuse the fan-out. That is the second task this directory
should hold, and it needs tasks that write, which also brings conflict
recovery into scope.
