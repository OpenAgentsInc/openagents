# Targeted experiment results template

Use this template for every published comparison of two or more
Terminal-Bench arms on a chosen set of tasks. A targeted result says how
many attempts it rests on, how uncertain each pass rate is, whether the
difference survives a paired test, and that no attempt in it was lost to
credentials or quota. One attempt per task gives pass-rate intervals of
about ±15 points, wider than every accuracy difference measured so far,
so the default is three attempts per task per arm
([lesson 9](2026-09-23-what-we-have-learned.md#9-the-evidence-is-thin-for-accuracy-claims)).

Don't run a full suite until a targeted experiment shows a large,
measured improvement.

## Run the experiment

Run from `bench/terminal-bench` on the benchmark host. Check the plan
first; it prints the interleaved schedule and checks every arm's
credentials without starting anything:

```sh
uv run tbench doctor --agent claude-code-opus --agent coder-one-tunable-v7
uv run tbench experiment plan --id v7-vs-cc-0924 --profile tb4 \
  --arm claude-code-opus --arm coder-one-tunable-v7 \
  --tasks risk-scorer-replay,wal-recovery-ordering,legacy-utility-triage \
  --quota-usd 150
uv run tbench experiment run --id v7-vs-cc-0924 --profile tb4 \
  --arm claude-code-opus --arm coder-one-tunable-v7 \
  --tasks risk-scorer-replay,wal-recovery-ordering,legacy-utility-triage \
  --quota-usd 150 --detach
```

- **Arms.** List the baseline first. The report compares every other arm
  against it. `--arm NAME=PROFILE` runs an agent profile as a separately
  named arm, so one profile can run with two policies:
  `--arm coder-one-tunable-luna-v2 --arm trial=coder-one-tunable-luna-v2
  --arm-kwarg trial:policy=/path/to/policy.json`. `coder-one proposal run
  --live` starts its experiments this way.
- **Attempts.** `--attempts` defaults to 3. Use more when the expected
  difference is small; don't use fewer for a published comparison.
- **Interleaving.** The schedule runs attempt 1 of every task, then
  attempt 2, and rotates which arm goes first on each task, so no arm
  always runs on a colder cache or at a busier time.
- **Pinning.** The first start writes `experiment.json` under
  `~/.openagents/terminal-bench/experiments/<id>/`. A restart with the
  same ID resumes; a different profile, arm list, task list, attempt count,
  or arm kwarg is refused and needs a new ID. Only `--quota-usd` may
  change, so you can raise the budget to finish.
- **Credentials.** Claude trials use the long-lived token in
  `~/.openagents/claude-setup-token`. Without it, the experiment refuses
  to start. The Coder One keys come from `~/.openagents/bearer` and
  `~/.openagents/jev.json` when they aren't already set. No value is ever
  printed or recorded.
- **Quota.** `--quota-usd` budgets the Claude quota as the list-price
  value Claude Code reports in `total_cost_usd`, over graded and lost
  trials. No Claude trial starts once the budget is used, or while the
  running trials could take the total past it.
- **Losses.** A trial whose session failed to authenticate, hit a usage
  limit, or timed out during setup is set aside under
  `~/.openagents/terminal-bench/failed/`, recorded in the experiment's
  `ledger.jsonl`, and run again. A credential failure also stops every
  further trial on that provider until you fix the credential and restart.

`uv run tbench experiment status --id ID` shows progress, and
`uv run tbench experiment stop --id ID` stops the scheduler; a restart
resumes interrupted trials.

## Fill the report

Print the filled results sections:

```sh
gym terminal-bench experiment report v7-vs-cc-0924 --markdown
```

Paste its output under the results heading below, and keep its JSON
(`--json`) beside the document. The report counts only graded attempts:
finished trials with a verifier reward. Lost, ungraded, and unrun
attempts are listed but never enter a denominator.

Publish only when the report says every scheduled attempt is graded and
no lost attempt remains unreplaced. If the quota budget stopped the
schedule early, say so in the summary and report the incomplete cells as
incomplete.

---

Copy everything below this line into a new dated document, such as
`docs/terminal-bench/2026-09-24-v7-vs-claude-code.md`.

# ARM against BASELINE on N tasks

Date, host, and the question this experiment answers in one sentence.

## Summary

State the result in two or three sentences: each arm's passes over graded
attempts with the 95% Wilson interval, the paired test's p-value, and the
Claude quota used. Say plainly whether the difference is distinguishable
from chance. For example: "Coder One passed 14 of 18 (55–91%) and Claude
Code 9 of 18 (29–71%); on the 18 paired attempts, Coder One alone passed
5 and Claude Code alone 0 (exact McNemar p = 0.06), so the difference
isn't yet distinguishable from chance."

## Protocol

| Field | Value |
| --- | --- |
| Tasks and why they were chosen | Name the selection rule, fixed before any outcome. |
| Arms | Agent profile IDs, artifact digests, and policy manifest digests. |
| Held fixed | Model, effort, tools, prompt cache, timeouts, and task pins. |
| Varied | What the treatment changes. |
| Stopping rule | Run every scheduled attempt once; no success-based stopping. |
| Quota budget | The `--quota-usd` value and why. |

## Results

Paste `gym terminal-bench experiment report ID --markdown` here. It fills
the design, pass-rate, paired-comparison, per-task, and completeness
sections.

## Cost and time

Report each arm's mean cost and agent time per graded attempt, from
`gym terminal-bench compare --profile PROFILE --json` or
`gym coder matrix --json`, and name each cost's provenance as
[measurement and pricing](measurement.md) defines it.

## Analysis

Explain the per-task pattern: which tasks separate the arms, and what the
retained traces show about why. Link each claim to a trial's evidence.

## Threats to validity

- Selection: the tasks aren't a random sample of the suite.
- Multiple comparisons: with more than two arms, the p-values aren't
  corrected.
- Anything that changed during the run, such as a host restart or a
  budget change.

## Evidence

- Experiment ID and status file: `~/.openagents/terminal-bench/experiments/ID/status.json`.
- Ledger of lost and rerun attempts: `ledger.jsonl` in the same directory.
- Retained traces: `bench/terminal-bench/traces/`, per the
  [retention procedure](runbook.md#retain-the-evidence).
