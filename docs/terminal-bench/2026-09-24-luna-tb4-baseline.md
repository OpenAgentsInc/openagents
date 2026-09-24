# GPT-6 Luna on 14 TB4 tasks, direct and with Jev structure

2026-09-24, on the `coderos` benchmark host. This experiment answers issue
[#9583](https://github.com/OpenAgentsInc/openagents/issues/9583), the first
step of the [Luna pivot](../coder/design/luna-pivot.md): how does GPT-6 Luna
do on Terminal-Bench 4.0 tasks, on its own in Codex and inside Coder One's
Jev structure with no escalation to Opus?

## Summary

Results pending. The protocol, the arms, the task-selection rule, and the
stopping rule below were committed before the first trial started.

## Protocol

| Field | Value |
| --- | --- |
| Tasks and why they were chosen | The rule below, fixed before any trial ran. |
| Arms | Baseline `codex-gpt-6-luna`; treatment `luna-jev`, the `coder-one-tunable-luna-pack` profile with the policy [`tunable-luna-pack-solo.json`](../../crates/coder-one/policies/tunable-luna-pack-solo.json) (SHA-256 of the file `ac33317b…9568d`) and the Coder One artifact `coder-one-9570-88fbe32ceb` (SHA-256 `b584f255…dbc9`), built from `88fbe32ceb`, which includes the end of Codex sessions that report only connection errors (#9581). |
| Held fixed | Codex CLI 0.155.1, `gpt-6-luna`, `high` reasoning effort, the ChatGPT-account sign-in from the host's `auth.json`, the task's own 8-hour agent timeout, its resources, and its verifier. |
| Varied | Whether Coder One's Jev structure prepares, supervises, and checks the Luna session: the deep Jev briefing with the coverage packer, the Jev monitor, requirement checks, and one repair session on Luna. |
| Stopping rule | Early stopping, below. Credential, quota, and setup losses are rerun and never counted. |
| Quota budget | `--quota-usd 20`. Neither arm draws on Claude, so the Claude budget is a guard, not a limit. Luna's cost is a price estimate from token counts. |

### Task selection rule

1. **The matched controller tasks.** The 10 tasks of the
   [matched controller test](2026-09-23-matched-controller-targeted.md):
   `legacy-utility-triage`, `mvcc-lsm-compaction`, `heat-pump-warranty`,
   `ks-solver-cpp`, `wal-recovery-ordering`, `cad-model`,
   `nextjs-performance`, `embedding-drift-monitor`, `fin-saccr-rwa`, and
   `sound-change-cascade`. Claude Code on Opus 5.5 has three graded
   attempts on each of them from that test.
2. **Tasks the weaker rows pass.** The weaker rows are the eight
   [TB4 leaderboard](tb4-leaderboard.md) rows under 25% accuracy (ranks 20
   to 27: Opus 4.8 max, GPT-5.6 Terra max, Grok 4.6 high, Gemini 3.8 Flash
   high, GPT-5.6 Luna max, Grok 4.5 high, Sonnet 5 max, and Gemini 3.7
   Flash high), 40 trials a task. Of the tasks not in rule 1, take the
   four with the most weaker-row passes: `wdm-design` (31 of 40),
   `shadow-relay` (26), `uefi-bootkit` (25), and `coq-block-bound` (24).
   The next tasks have 20, so the cut is clean.

### Arms

**Baseline, `codex-gpt-6-luna`.** Harbor's Codex agent, which installs
Codex CLI 0.155.1 in the task environment and runs it once on the task
instruction at `high` reasoning effort until the task's timeout.

**Treatment, `luna-jev`.** Coder One with
[`tunable-luna-pack-solo.json`](../../crates/coder-one/policies/tunable-luna-pack-solo.json),
a new manifest made from
[`tunable-luna-pack.json`](../../crates/coder-one/policies/tunable-luna-pack.json),
the Luna policy that passed 24 of 24 on the development panel
([tunable results](2026-09-23-tunable-results.md)). It differs in two
fields: `control.handoff` is removed, so nothing escalates to Claude Code
on Opus 5.5, and `control.horizon.long_effort` is `high`, the baseline's
effort, where the original runs long tasks at `medium`. Every dispatch,
including the repair, runs on Codex with GPT-6 Luna.

### Stopping rule

The schedule runs attempt 1 of every task on both arms, then attempt 2,
then attempt 3. After each round, the operator computes the paired
comparison and stops early by these rules:

1. **Decided.** Stop the comparison once the exact McNemar test on the
   paired attempts gives p < 0.05.
2. **Can't become decisive.** After round 2, stop the comparison if
   round 3 can't give p < 0.05 even when every remaining pair is
   discordant in the leading arm's favor.
3. **Dead tasks.** After round 2, a task that both arms failed on every
   attempt, for a reason classified as capability rather than
   infrastructure, gets no attempt 3.
4. **Runaway trials.** A trial that runs past 3 hours of agent time while
   the rest of its round has finished may be stopped and counted as not
   run, never as a failure.

An arm stopped early keeps its graded attempts, and the report lists the
unrun attempts as not run.

## Results

Pending.

## Cost and time

Pending. Luna's cost is a price estimate from Codex's token counts at
$0.10 per million uncached input tokens, $0.01 per million cached input
tokens, and $0.50 per million output tokens, the prices in
`crates/coder-one/src/delegate.rs`. Jev is priced as in
[measurement and pricing](measurement.md).

## Evidence

- Experiment ID and status file: `~/.openagents/terminal-bench/experiments/luna-tb4-9583/status.json`.
- Ledger of lost and rerun attempts: `ledger.jsonl` in the same directory.
