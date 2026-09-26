# Microcoder

Microcoder is Coder's simple loop. It replaced Microluna on 2026-09-25
(issue [#9666](https://github.com/OpenAgentsInc/openagents/issues/9666)).
Each step, Jev judges the state, one call to GPT-6 Luna returns the next
commands, and the host runs them. Since 2026-09-26 the call goes through
the operator's logged-in Codex subscription, not OpenRouter:

```text
while next_action isn't finished:
    knowledge   = kb.retrieve(state)
    jev_results = jev(state + knowledge, user_prompt)
    prompt      = state + user_prompt + jev_results + knowledge
    next_action = generate(prompt)      # one structured call, not a conversation
    run next_action's commands
```

The code is `crates/microcoder`. It depends on `crates/jev`,
`crates/knowledge`, `crates/microluna` (for its Codex transport), and
`crates/openrouter` (kept for `--provider openrouter` and for embeddings).

## Run a Terminal-Bench 4 task

```sh
microcoder embedding-drift-monitor
```

The command finds the task under
`~/.openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks/`,
starts its container, runs the loop, grades the result with the task's own
tests, and prints the reward beside Fable 5.1 low's median time and cost
on the same task. Every step streams to the terminal: Jev's answers, the
knowledge-base entries kept, the model's reason, each command's first lines,
and the acceptance test results.

The model is reached through the Codex login in `~/.codex/auth.json`: run
`codex login` first. Jev needs `TYPESAFE_API_KEY` or
`~/.openagents/jev.json`. Knowledge-base embeddings use `OPENROUTER_API_KEY`
or `~/.openagents/openrouter.json` when it works; without it, search ranks
entries by words alone. No key enters the container: commands run through
`docker exec`, and the model and Jev are called from the host.

Each run writes `~/.openagents/microcoder/runs/<task>-<time>/`:
`events.jsonl` (every event as it happened), `summary.json` (the outcome,
reward, grader output, entries used, and frozen tests), and `artifacts/`
(the task's output paths, saved before grading).

Exit codes: 0 when the task's tests pass, 1 when they don't, 2 when the run
couldn't start. `--check-grading` runs the task's reference solution
instead of the loop, which checks that grading works at no model cost.

## What a step does

1. **Knowledge.** The host searches the [knowledge base](../design/knowledge-base.md)
   with a query built from the state, and Jev keeps the entries that bear on
   it. The prompt shows each kept entry's summary, and the full body of
   every entry Jev rates 0.8 or more.
2. **Jev.** Jev answers the questions in `crates/microcoder/questions.json`
   (whether the task is done, whether the last step made progress, and
   whether the steps repeat a failed approach). Its answers go into the
   prompt as evidence.
3. **Generate.** One Codex Responses request declares one strict native
   tool, `next_action`, whose parameters are `rationale`, `commands`,
   `view`, `expand`, `freeze_tests`, and `finished`; the model's call to it
   is the action. The cost shown is Luna's list price for the reported
   tokens, since the subscription doesn't bill per call.
4. **Run.** Each command is a bash script fed to the container's shell,
   in order, with a deadline, stopping at the first failure.
5. **Files in view.** The host reads the files the model lists in `view`
   after the commands run, and shows them in full in the next prompt.

## Acceptance tests first

Before changing the task's files, the model writes one bash test per
requirement under `/tmp/acceptance/` and sets `freeze_tests`. The host
keeps its own copies, runs them after every step that ran a command, and
shows the results, with each failing test's numbered script and output.

- `finished` is accepted only when every frozen test passes. Three refused
  finishes end the run.
- When the model says it's finished and tests still fail, Jev answers
  `dispute.json` for each failing test. A test Jev judges wrong at 0.7 or
  more, such as one that compares lists of different lengths, is dropped
  and recorded.
- When the model says it's finished, Jev also compares the code in view
  with each highly relevant method or edge-case entry (`conform.json`). A
  contradiction at 0.7 or more sends the finish back once per entry.
- After 3 steps in a row with every frozen test passing, the model is told
  to finish. After 6, the host ends the run and grades it.

`--no-acceptance` turns all of this off.

## Limits

| Option | Default | What it bounds |
| --- | --- | --- |
| `--max-steps N` | no limit | Steps. |
| `--max-minutes N` | 60 | Wall-clock time. |
| `--max-usd N` | 1.00 | Model, Jev, and embedding spend together. |
| `--command-seconds N` | 300 | One command. |
| `--test-seconds N` | 60 | One acceptance test. |

Three replies in a row that run nothing and ask for no new file end the
run, and so do three replies in a row that don't match the schema.

## Other options

- `--model SLUG` and `--effort low|medium|high`: the generating model,
  `gpt-6-luna` at medium effort by default.
- `--provider codex|openrouter`: how the model is reached. `codex`, the
  operator's Codex login, is the default; `openrouter` is the earlier path.
- `--kb on|off|candidates`: the knowledge base, on by default.
- `--route never|auto|always` and `--strong-model SLUG`: whether a
  stronger model (`gpt-6-sol` by default) writes the acceptance
  tests. Off by default; the code stays for later measurement.
- `--network NAME`: the container's Docker network. `bridge` by default,
  and `none` for a task whose `task.toml` sets `allow_internet = false`.
- `--prompt TEXT`, `--keep`, and `--check-grading`.

## Results

Results live in the [Terminal-Bench 4 results](../../terminal-bench/tb4-results.md)
under "Microcoder development runs." Report a run that used the knowledge
base as knowledge-assisted.
