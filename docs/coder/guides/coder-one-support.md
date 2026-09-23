# Judge requirement support with paired Jev questions

`verify.support` replaces the broad "done" judgment with two questions per
requirement: does the evidence show the requirement met, and does it show
it not met. Each requirement then gets a state that records what was
observed, what that establishes, the checker, and the candidate revision
it holds for.

The design is in
[Coder as a tunable system](../../optimization/coder-components.md#verify-and-finish).
On v3's 24 Luna trials, a 0.5 cutoff on the "done" judgment accepted all
five failures and rejected four passes. That judgment reads the executor's
report and a change summary, not the artifacts.

## What each judgment reads

For each requirement worth judging, one Jev request carries:

- The task: its title and instruction.
- The requirement: its ID, text, and kind.
- An excerpt of the artifact: the candidate's files that the requirement or
  a scenario's interface names, or every file when none is named, or its
  inline programs when it wrote no file. 6,000 characters in all.
- What `verify.checks` observed: each scenario that observes the
  requirement, with its expected relation, its verdict for this
  requirement, its observations, and its coverage limits. Scratch paths
  read as `<scratch>`, so the request, and the recorded answer's key, stay
  the same from run to run.

The request asks two Nouls: **supports** and **contradicts**. They aren't
complementary. Both low means the evidence is insufficient; both high
means it conflicts, or the requirement is compound. A requirement that a
scenario contradicted is judged first, then those a scenario observed, then
behaviors and deliverables no scenario observed, three at most per run.

No verifier output, test name, or label ever enters a request.

## Requirement states

| State | When |
| --- | --- |
| `supported` | supports ≥ 0.5 and contradicts < 0.3 |
| `contradicted` | contradicts ≥ 0.3 and supports < 0.5 |
| `unresolved` | Both at or above their cutoffs, both below, no answer (a refusal, a miss, or Jev off), or a clipped artifact |

Each state keeps both answers, the evidence they read and its digest, the
scenarios and both implementations that established it, and the
candidate's digest. `State::fresh_for(candidate)` is false for any other
revision, and `Report::stale_for` names the states a changed candidate
invalidates.

## The labeled fixtures

`coder-one support fixtures` builds 18 fixtures of (requirement, evidence,
outcome) triples, with the evidence frozen when the fixture is built:

- **Development**, 9 candidates and 16 labeled requirements: each
  mini-task's good and bad script, and the synthetic off-by-one log parser.
  The cutoffs are fitted here only.
- **Evaluation**, 9 candidates and 22 labeled requirements: the v3 Luna
  candidates `verify.checks` recovered from retained streams, with the
  verifier's reward and the trial's retained "done" answer. They're scored,
  never fitted on.

A passing candidate meets every requirement the run judges. A failing
candidate's labels name the requirement its failure breaks, from the
verifier's test outcomes or the known-bad source, and, where the verifier
said so, the requirement it still meets. Other requirements of a failing
candidate stay unlabeled. Labels are for scoring only.

## Results

Recorded Jev answers, asked live once: 58 requests, including the eight
mini-task "done" baselines, for $0.0003 in all. The fit chose supports ≥
0.5 and contradicts ≥ 0.3; a test refits it and checks that flipping every
evaluation label doesn't move it.

On the evaluation split, the recovered v3 candidates:

| Rule, per candidate | False accepts (of 5 failures) | False rejects (of 4 passes) |
| --- | --- | --- |
| "done" at 0.5 | 5 | 1 |
| "done" at its development cutoff, 0.2 | 5 | 0 |
| Support pair | 1 | 0 |
| Support pair and scenario checks | 0 | 0 |

| Rule, per labeled requirement | False accepts (of 8 unmet) | False rejects (of 14 met) |
| --- | --- | --- |
| Support pair | 5 | 0 |
| Support pair at 0.5 and 0.5 | 7 | 0 |
| Scenario checks alone | 4 | 5, 4 of them unobserved |
| Support pair and scenario checks | 3 | 1 |

On the development split, the pair makes no false accept on the 6 unmet
requirements and one false reject of 10 met: the git task's recovery,
which no scenario observes and whose artifact shows only the file's
content, so both answers are low and the state is unresolved.

What the numbers say:

- The pair reads a failed cancellation or interactive scenario as a
  contradiction (contradicts 0.62 to 0.92), and a passing one as support.
- It doesn't catch the log tasks' field-meaning failure. With the
  `data.message-severity` scenario failed, its supports answer for the
  severity requirement is 0.73 to 0.81 on all three v3 log candidates, and
  its contradicts answer is at most 0.36: two read as supported and one as
  unresolved. The known-bad mini-task candidate is unresolved too. The failed relation reads
  as a table of changed counts; the question asks about the requirement's
  words, and the counts don't name them.
- It judges the failed headless-terminal candidate's interactive-program
  requirement supported, as the passing scenario did, and its control C
  requirement supported where the scenario contradicted it. The verifier
  agrees with the pair on control C.
- Brier scores on the evaluation split: supports 0.24, one minus
  contradicts 0.20, "done" 0.28. On development: 0.15, 0.11, and 0.27.

So the pair alone doesn't replace a scenario, and a scenario alone
misreads some requirements; together they accept none of the five v3
failures and reject none of the four passes.

## Run it

```sh
coder-one support evaluate                     # recorded Jev, both splits
coder-one support evaluate --json
coder-one support evaluate --write-checks      # states beside each recovered check
coder-one component suite verify.support       # the same fixtures, recorded for the Gym
coder-one support run --input input.json --jev live
coder-one support fixtures                     # rebuild the labeled set; needs python3
```

`evaluate --jev live --save-jev` asks Jev again and records the answers in
each fixture. A mini-task episode with `--jev live` runs `verify.support`
after `verify.checks` and writes `verification/support.json`.

## See it in the Gym

```sh
coder-one checks recover --traces bench/terminal-bench/traces
coder-one support evaluate --write-checks
gym coder coverage
gym coder coverage --attempt headless-terminal__tr384w7
gym-terminal --terminal-bench
```

The coverage list adds each attempt's support counts. One attempt's report
shows, under each requirement, both judgments and the state they establish,
beside the scenario verdicts. The attempt and mini-task views in
`gym-terminal` show the same lines.
