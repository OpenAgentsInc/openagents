# Check claimed behavior with admitted scenarios

`verify.checks` observes what a candidate does rather than what its report
says. It runs scenarios, each justified by a requirement in the task's own
words, against one named candidate. Each requirement ends up observed,
contradicted, unverifiable, or unobserved. A failed scenario leaves a
diagnostic packet that repair and handoff can use.

The design is in
[Coder as a tunable system](../../optimization/coder-components.md#verify-and-finish).
No v3 Luna episode observed its own output. The five failures fall into
three families that a format check can't catch, and each family is a
scenario type here.

## The scenarios

| Scenario | Applies when | Expected relation |
| --- | --- | --- |
| `data.message-severity` | The instruction shows CSV rows that count a field's values, and an observed record marks the value as a field | Changing only records' messages, so each carries another value's word, leaves every count unchanged. |
| `data.date-boundaries` | The instruction also states a reference date, dated file names, and date periods | Each period counts exactly the files its rule includes, on both sides of each boundary. |
| `interactive.program` | The instruction asks for interactive programs through a `from MODULE import CLASS` interface | A raw-mode program started through the interface reads staged keys and writes them reversed. |
| `interactive.interrupt` | A requirement names control C | After control C interrupts a foreground command, the shell runs the next command. Control C is sent 0.05, 0.3, and 1 second after the command. |
| `cancel.signal.below`, `.at`, `.above`, `cancel.internal.above` | The instruction asks that cleanup still runs when a run is cancelled, with a concurrency limit | Once the tasks have started, an interrupt leaves every started task's cleanup finished before the call returns. No more tasks than the limit ever run at once, and the process exits. Each task runs 1.5 seconds unless cancelled, so a runner that lets started tasks finish also passes; the instruction allows either. |

Every parameter comes from the public instruction (paths, rows, periods,
and the interface), the observed input (the record format), or a
recorded host choice (a token, the limit of 3, and the delays). Protected
verifier test names, counts, and fixture timings never enter a scenario. A
test builds scenarios from every recovered v3 candidate and checks that no
verifier test name appears in them.

## How a check runs

A check runs four recorded suboperations, each an invocation under one
`verify.checks` invocation:

1. **Build** admits every scenario whose applicability conditions hold and
   that a requirement justifies. It records the scenario types that don't
   apply, and why.
2. **Select** picks among the admitted scenarios within the budget, which
   is 12 scenarios and 180 seconds of bounds by default. It takes one
   scenario per requirement first. This selector is deterministic; a Jev
   selector or a planner can compete over the same catalog.
3. **Run** executes each selected scenario in its own scratch copy of the
   candidate, so observing never changes the workspace.
4. **Record coverage** turns verdicts into each requirement's state against
   the candidate's digest and writes a packet for each failure.

Each scenario records the requirements and instruction spans that justify
it, its applicability conditions, its interface, bounds, and effects, the
candidate and input digests, the expected relation and how it was derived,
and its verdict with coverage limits. A packet names the requirement, the
candidate digest, the scenario, the expected relation, the observations,
and the hypotheses the observations leave open.

Each interactive scenario first types a plain `echo`. When even that
doesn't run, the verdict is `inconclusive`: the host may differ from the
task's environment, and the scenario says nothing about interactive
behavior. When a candidate names an absolute executable the host lacks,
such as `/bin/bash`, the check provides the host's own binary at that path
in a `bwrap` mount namespace and records that it did. When it can't, the
verdict is `unavailable`.

The requirement map comes from `task.requirements`. When the input doesn't
carry one, the check uses the rule-only map of the instruction. Scenarios
run the candidate with `python3`; without it, the verdict is `unavailable`.

## Run a check

```sh
coder-one checks synthetic                  # every known-good and known-bad pair
coder-one checks run --input input.json     # a candidate in a checks input file
coder-one component suite verify.checks     # the 16 checked-in fixtures
coder-one minitask run log-severity --script bad   # checks run before the grader
```

`checks synthetic` runs each family's known-good candidate and its
known-bad ones: a field parser against a whole-line search and an
off-by-one window, a pseudo-terminal against a terminal that runs builtins
only, and awaited cleanup against an early return. Each scenario must fail
the bad candidate it targets and pass the good one.

A mini-task episode runs `verify.checks` on its workspace after the
executor and before the grader, unless you pass `--no-checks`, and writes
the report to `verification/checks.json`.

## Check retained trials

```sh
coder-one checks recover --traces bench/terminal-bench/traces
coder-one checks recover --traces bench/terminal-bench/traces --arm all
```

`recover` rebuilds each candidate from its retained native stream: a file
written in full by a here-document or a `Write` call, an `Edit` replayed
onto such a file, or a program fed to `python3` inline. An agent that ran
directly, with no delegate stream, is rebuilt from the tool calls in its
trajectory. The observed samples come from the probe outputs in the
trajectory. A task-provided file, such as `base_terminal.py`, comes from a
`cat` of it in any trial of the task. Harbor didn't retain files the agent
left in `/app`, so a candidate the stream only names is reported as
unavailable rather than guessed at. Repair is disabled: the check only
observes.

Results go to `~/.openagents/coder-one/checks/<job>/<trial>/checks.json`,
beside the rebuilt `input.json`, with a summary of detections and false
alarms in `summary-<arm>.json`.

## See coverage in the Gym

```sh
gym coder coverage
gym coder coverage --attempt panel--coder-one-jevprobe3-luna--headless-terminal/headless-terminal__tr384w7
gym coder coverage --run latest --json
gym-terminal --terminal-bench
```

`gym coder coverage` lists each checked attempt and mini-task run with its
reward, scenario verdicts, and requirement states. `--attempt` or `--run`
shows one report: each requirement with its scenarios, verdicts, and
coverage limits, then the diagnostic packets. In `gym-terminal`, the
attempt view shows the same coverage below an attempt's counts, and the
mini-task view shows it for each run. Where `verify.support` judged the
requirements, each one also shows both Jev judgments and the state they
establish; see
[Judge requirement support with paired Jev questions](coder-one-support.md).
