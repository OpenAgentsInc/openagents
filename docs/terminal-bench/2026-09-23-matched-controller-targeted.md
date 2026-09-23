# Coder One's controller against the same executor on 10 TB4 tasks

2026-09-23, on the `coderos` benchmark host. This experiment answers issue
[#9567](https://github.com/OpenAgentsInc/openagents/issues/9567): with the
executor held fixed, does Coder One's controller (Jev briefing, monitoring,
checks, support, repair, escalation, second executor, and persistence) change
the pass rate or the cost on tasks other agents usually solve?

## Summary

Running. The protocol below was committed before the first trial started.

## Protocol

| Field | Value |
| --- | --- |
| Tasks and why they were chosen | The rule below, fixed before any trial ran. |
| Arms | Baseline `claude-code-opus-matched`; treatment `coder-one-matched-v8`, policy manifest SHA-256 `16ce204b…5435`. Both install the same Coder One artifact, listed under the evidence. |
| Held fixed | Claude Code 2.1.280, Opus 5.5, medium effort, the tools `Bash`, `Read`, `Edit`, `Write`, `Glob`, and `Grep`, the headless system prompt (SHA-256 `d2ccda88…9aec`), the five-minute prompt cache, a one-hour shell-command ceiling, bypass permissions in the task container, the task's own 8-hour agent timeout, its resources, and its verifier. |
| Varied | Whether Coder One's controller prepares, supervises, checks, and extends the executor's work. |
| Stopping rule | Run every scheduled attempt once; no success-based stopping. Credential, quota, and setup losses are rerun and never counted. |
| Quota budget | `--quota-usd 200`, the Claude quota set aside for this experiment. At most two of its Claude trials run at once. |

### Task selection rule

The rule uses two sources: the five highest-ranked rows of the
[TB4 leaderboard reference](tb4-leaderboard.md) (Codex on GPT-6 Astra at max,
xhigh, and high, and Claude Code on Fable 5.1 at max and xhigh; five trials
each, 25 a task), which `gym coder matrix --profile tb4 --reference-rows 5`
prints, and our graded TB4 trials in the same matrix: Claude Code on Opus 5.5
and every Coder One tunable policy except the Luna-first one. A trial
with a recorded cost under $0.05 is treated as ungraded, because those were
setup or quota failures, not attempts.

1. **Failed by us, solved by others:** the top five rows pass at least 20 of
   25 trials, and our graded trials pass fewer than half. Three tasks qualify:
   `legacy-utility-triage` (24/25; ours 0/1), `mvcc-lsm-compaction` (22/25;
   ours 0/2), and `heat-pump-warranty` (21/25; ours 0/3).
2. **Failed by us, usually solved by others:** only three tasks meet rule 1,
   so the bar drops to at least 15 of 25 with the same failure condition:
   `ks-solver-cpp` (17/25; ours 0/3) and `wal-recovery-ordering` (16/25;
   ours 0/3).
3. **Mixed:** the top five rows pass at least 20 of 25 and at least one of our
   graded trials failed while most passed: `cad-model` (25/25; ours 3/4) and
   `nextjs-performance` (23/25; ours 7/8).
4. **Passed by us:** the top five rows pass at least 20 of 25 and every one of
   at least two graded trials of ours passed; of those, the three with the
   lowest mean cost: `embedding-drift-monitor` (24/25; ours 2/2, $0.68),
   `fin-saccr-rwa` (24/25; ours 2/2, $0.91), and `sound-change-cascade`
   (25/25; ours 2/2, $1.30).

`nextjs-performance` was also one of the two
[matched pilot](2026-09-23-matched-opus-controller.md) tasks.

### Arms

**Baseline, `claude-code-opus-matched`.** The matched pilot's plain arm,
generalized from its 1,680-second pilot allowance to the task's own deadline
(`tbench.matched:MatchedPlainTask`). It reads the executor block of the
treatment's policy manifest, so the model, effort, tools, prompt cache, and
CLI version come from the same bytes, and it refuses a manifest whose system
prompt isn't the pilot's six replaced sections. It installs Coder One and runs
its read-only doctor like the treatment, then runs Claude Code once on the
task instruction until the treatment episode's deadline, 60 seconds inside
the adapter's. It never starts a Coder One episode.

**Treatment, `coder-one-matched-v8`.** The newest tunable policy, v8, with
every executor fixed to the baseline's
([`matched-opus-medium-v8.json`](../../crates/coder-one/policies/matched-opus-medium-v8.json)).
It keeps v8's deep Jev briefing with the coverage packer, monitoring,
same-model escalation on a stall, requirement and behavior checks, support
judgments, self-report checks, checked repair, a second executor after a
failed check, and up to four persistence rounds under v8's spending cap. It
drops task routing and the leaderboard family table, runs the second executor
and every persistence round on Opus instead of GPT-6 Astra or Sol, and keeps
medium effort on long tasks where v8 uses xhigh. Every TB4 task has an 8-hour
timeout, so every task counts as long for the horizon and persistence rules.

## Results

Pending.

## Evidence

- Experiment ID `matched-v8-9567`; status file
  `~/.openagents/terminal-bench/experiments/matched-v8-9567/status.json`.
- Ledger of lost and rerun attempts: `ledger.jsonl` in the same directory.
