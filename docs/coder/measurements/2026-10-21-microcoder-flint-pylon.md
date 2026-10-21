# Microcoder on Flint and DeepSeek V4.1 Pylons

Date: 2026-10-21. Issue: [#12426](https://github.com/OpenAgentsInc/openagents/issues/12426).

This receipt records Microcoder runs on two providers that need no Codex or
Claude login: Flint on this Mac through `rec` (`--provider
openai-compatible`), and DeepSeek V4.1 Flash on the Pylon network
(`--provider pylon`). The [Microcoder guide](../guides/microcoder.md#run-a-task-in-a-directory-on-this-computer)
covers the commands.

## Setup

- **Tasks.** Three small Python repositories, each a fresh `git init` copy
  for every run, in the session's scratch folder, not this repository.
  Each one is checked with `python3 -m unittest`, and the run's prompt says
  not to change the tests.
  - `implement`: write `word_frequencies` (case folding, apostrophes,
    sorting, and `top`) to pass four tests.
  - `fixbug`: `parse_duration` assigns where it should add, so one test
    fails.
  - `multifile`: add percentage discounts across `shop/models.py`,
    `shop/cart.py`, and `shop/cli.py` (per-item rounding half up, bounds
    checks, and a `--discount` flag) to pass four new tests.
- **Runs.** `microcoder "PROMPT" --dir REPO --max-steps 20 --max-minutes 30`,
  local mode: commands run inside the `coder-boundary` sandbox, with no
  acceptance tests and no knowledge base. Jev judges every step through
  the hosted decision service.
- **Success** means the task's own tests pass after the run.

## Results

| Provider | Task | Result | Steps | Wall time | Answered by |
| --- | --- | --- | --- | --- | --- |
| Pylon, `deepseek-v4.1-flash` | `implement` | Tests pass | 4 | 4 min 48 s | `w16-g5b` (4) |
| Pylon, `deepseek-v4.1-flash` | `fixbug` | Tests pass | 6 | 11 min 20 s | `w16-g5b` (2), `w16-g5c` (2) |
| Pylon, `deepseek-v4.1-flash` | `multifile` | Tests pass | 6 | 7 min 25 s | `w16-g5c` (5) |
| `rec`, `flint` | `implement` | Tests pass | 4 | 33 s | This Mac |
| `rec`, `flint` | `implement`, `fixbug`, `multifile` | Not completed | - | - | `rec` stopped, see below |

The model cost is $0 on both providers (`cost_basis` `local` and
`free_network`). Jev cost $0.0002 to $0.0004 a run.

The Pylon `implement` run and the Flint run used builds from before commit
199b62c77c; the Pylon `fixbug` and `multifile` runs used 199b62c77c. The
change after those runs, waiting inside a step when every group has no
free slot, wasn't measured.

## What we saw

**Pylon DeepSeek.** All three tasks passed, with each change limited to
the files the task named. A step takes 35 to 150 seconds, and the time
grows with the prompt: about 35 seconds at step 1 (2 KB of state) and 90
seconds once the state reaches the group's cap. A short `pylon ask` takes
about 5 seconds, so a step's time is mostly prompt length. The groups
report no token counts, so `summary.json` shows 0 tokens.

Failure modes, all handled within the run:

- **Prompt cap.** The DeepSeek groups take at most 2,872 prompt tokens.
  Before the fit was added, a run that had already fixed the code failed
  at its finishing step on `limit_exceeded`. Now the provider reads the
  cap from the refusal, cuts the prompt (keeping the task, the files in
  view, and the latest steps), and asks again. This happened once in each
  of the `fixbug` and `multifile` runs, and both finished.
- **Busy or missing group.** One run saw no live group for one step, and
  others got `rate_limited: no free slot`, one `no answer within 310 s`,
  and one `unavailable: the model failed` from another group serving the
  model. A step with no live group now waits 15 seconds and asks again, and
  since 199b62c77c a step whose groups all have no free slot does the same.
  The loop ends after three unusable replies in a row.
- **First-step schema.** DeepSeek always answered with valid JSON in text
  with the short reply-format reminder.

**Flint through `rec`.** The one complete run passed `implement` in 4
steps and 33 seconds, at 3 to 11 seconds a step. `rec` accepts the strict
`json_schema` response format and ignores it: Flint's first reply put a
whole action inside `commands` as objects. `--response-format auto` moved
the run to JSON in text, and the text reader now flattens commands written
as objects.

The other Flint runs didn't complete:

- In the first batch, `rec` stopped answering during step 1 of `implement`
  (the request hung about 275 seconds, then the connection was refused),
  and the other two tasks failed at once on the closed port. We didn't
  find why `rec` went down.
- After a reboot, `rec` was started only for these runs, with the load
  average above 100 from other work. Step 1 hadn't returned after 10
  minutes (`rec` logged a 4,880-token prefill finishing after about 5
  minutes), so we stopped the runs and `rec` because of the machine load.

**Other observations.**

- The `openagents` binary installed on this Mac (d42168061a) answered
  `openagents pylon ask --model deepseek-v4.1-flash` from a 0.8B Qwen
  pylon. The current `pylon` binary routes the same request to
  `w16-g5b`, and the group refuses it unless the request carries
  `--deadline`.
- We didn't run the optional issue-sized task in a worktree of this
  repository.

## Reproduce

```sh
umask 077; rec auth token > ~/.openagents/rec-token
microcoder "Implement word_frequencies in textstats.py so that python3 -m unittest passes. Don't change the tests." \
  --provider openai-compatible --base-url http://127.0.0.1:11500/v1 \
  --token-file ~/.openagents/rec-token --model flint --max-steps 20
microcoder "python3 -m unittest fails. Find and fix the bug in durations.py; don't change the tests." \
  --provider pylon --model deepseek-v4.1-flash --max-steps 20
```
