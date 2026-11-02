# 30. mini-SWE-agent

> **Status: written.** Read at SWE-agent/mini-swe-agent@04d809ceab9df28f9adaed044884180159172930 on 2026-11-02.

Source: [`SWE-agent/mini-swe-agent`](https://github.com/SWE-agent/mini-swe-agent/tree/04d809ceab9df28f9adaed044884180159172930),
version 2, cloned shallowly into `projects/repos/mini-swe-agent` and read
only. Nothing was installed or run. Paths below are relative to that
revision unless a link says otherwise.

## Why study it

mini-SWE-agent is the smallest agent in this part of the book that people use
as a research baseline. The team behind SWE-bench and SWE-agent wrote it to
ask how much of an agent's score comes from the scaffold and how much from the
model (`README.md`, "More motivation (for research)"). It has one tool, bash,
a history that only appends, and an environment that runs every command in a
fresh process. The README states the agent class is "some 100 lines of
python"; `src/minisweagent/agents/default.py` is 190 lines with its
serialization code.

Coder needs exactly this kind of baseline. The book's rule is that an addition
to the loop stays only with a measured gain in [chapter 21](ch021-measure-the-harness.md)
against the simplest loop ([plan](PLAN.md#rules-for-every-chapter)).
mini-SWE-agent is a public, widely run version of "the simplest loop", so it
gives Coder both a design to compare with and an outside reference point for
its own baseline. Coder's own small loop, Microcoder, is close to it, and this
chapter compares the two directly.

## What it does

| Feature | File at the revision | Coder chapter |
| --- | --- | --- |
| A `run` loop that calls `step()` until the last message has role `exit` | `src/minisweagent/agents/default.py` | [1](ch001-the-simplest-loop.md), [9](ch009-microcoder.md) |
| One step: query the model, run every action, append the observations | `src/minisweagent/agents/default.py` (`step`, `query`, `execute_actions`) | [1](ch001-the-simplest-loop.md) |
| Each command in a new process; the process group is killed on timeout | `src/minisweagent/environments/local.py` (`_run`) | [5](ch005-a-sandbox-and-approvals.md) |
| The same `execute` as `docker exec` into a long-lived container | `src/minisweagent/environments/docker.py` | [5](ch005-a-sandbox-and-approvals.md) |
| Ending by a sentinel line in a command's output | `src/minisweagent/environments/local.py` (`_check_finished`) | [9](ch009-microcoder.md) |
| One `bash` function tool, or a regex over text for models without tool calls | `src/minisweagent/models/utils/actions_toolcall.py`, `src/minisweagent/models/utils/actions_text.py` | [4](ch004-tools-as-typed-capabilities.md), [9](ch009-microcoder.md) |
| A format error goes back to the model; three in a row end the run | `src/minisweagent/agents/default.py` (`max_consecutive_format_errors`) | [9](ch009-microcoder.md) |
| Step, cost and wall-time limits as configuration | `src/minisweagent/agents/default.py` (`AgentConfig`) | [21](ch021-measure-the-harness.md) |
| Long output cut to a head and a tail, with a warning to the model | `src/minisweagent/config/benchmarks/swebench.yaml` (`observation_template`) | [9](ch009-microcoder.md) |
| The whole trajectory saved after every step | `src/minisweagent/agents/default.py` (`save`, `serialize`) | [2](ch002-model-visible-means-logged.md) |
| Prompt-cache markers for Anthropic models | `src/minisweagent/models/utils/cache_control.py` | [3](ch003-keep-the-prefix-stable.md) |
| A scripted model for tests | `src/minisweagent/models/test_models.py` (`DeterministicModel`) | [21](ch021-measure-the-harness.md) |
| SWE-bench batch runs that write `preds.json` for a separate scorer | `src/minisweagent/run/benchmarks/swebench.py`, `docs/usage/swebench.md` | [21](ch021-measure-the-harness.md) |

### The loop

`DefaultAgent.run` renders two templates, a system message and an instance
message with the task, and then loops. Each pass calls `step()`, which is
`execute_actions(query())`. `query` checks the step, cost and wall-time limits,
sends the whole message list to the model, adds the model's message to the
list and returns it. `execute_actions` runs each action in the message through
`env.execute` and appends one observation message per action. The loop ends
when the last message in the list has the role `exit`
(`src/minisweagent/agents/default.py`).

Control flow outside that path uses exceptions that carry messages. A
`FormatError` appends the model's bad output and a correction to the history.
`LimitsExceeded`, `TimeExceeded` and `Submitted` append an `exit` message.
Any other exception appends an `exit` message with the traceback and is
raised again. After every pass, success or not, `save` writes the trajectory
to `output_path` (`src/minisweagent/agents/default.py`).

The history is the trajectory. The README calls this "a completely linear
history": nothing is removed, summarized or reordered, so the messages saved
to disk are the messages the model saw (`README.md`). There is no condensing,
and a long run grows its prompt until a limit stops it.

### Independent commands

`LocalEnvironment.execute` runs the command with `subprocess.Popen(...,
shell=True)` in a new session, merges stderr into stdout and waits with a
timeout. On timeout it kills the whole process group with `SIGKILL` so no
child is left running (`src/minisweagent/environments/local.py`, `_run`). The
Docker environment starts one container with `sleep` and runs each command
with `docker exec -w cwd ... bash -c COMMAND`
(`src/minisweagent/environments/docker.py`). The container's filesystem
persists between commands; the shell does not.

The FAQ explains why there is no persistent shell: it is hard to tell when a
command in a live shell has finished, a bad command can kill the shell, and an
interrupt can corrupt later output (`docs/faq.md`, "Why is not needing a
running shell session such a big deal?"). The cost is that `cd` and `export`
do not carry over. The SWE-bench instance prompt tells the model this and
asks it to prefix commands instead
(`src/minisweagent/config/benchmarks/swebench.yaml`, "Command Execution
Rules").

### Ending a run

The model ends a run from inside the environment. If a command's output
starts with the line `COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT` and the command
exits 0, the environment raises `Submitted` with the rest of the output as
the submission (`src/minisweagent/environments/local.py`,
`_check_finished`). For SWE-bench the prompt asks the model to write
`git diff` to `patch.txt`, inspect it in a separate command, and then run
`echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT && cat patch.txt`
(`src/minisweagent/config/benchmarks/swebench.yaml`, "Submission"). The patch
is the answer.

### Actions and format errors

Version 2 uses one function tool, `bash`, with a single `command` string
(`src/minisweagent/models/utils/actions_toolcall.py`, `BASH_TOOL`). A reply
with no tool call, an unknown tool or bad arguments raises `FormatError`, and
the error text goes back to the model through `format_error_template`. The
older path, kept for models without tool calls, takes exactly one action out
of the text with a regular expression
(`src/minisweagent/models/utils/actions_text.py`, `parse_regex_actions`); its
module docstring says the project now recommends tool calls. The SWE-bench
configuration sets `parallel_tool_calls: true`, so one reply can carry several
independent commands (`src/minisweagent/config/benchmarks/swebench.yaml`).

`max_consecutive_format_errors` defaults to 3. Each clean step resets the
count; at the limit the run ends with `RepeatedFormatError`
(`src/minisweagent/agents/default.py`). The format-error template also tells
the model when the reply was cut off at the output token limit, so it can
answer more briefly rather than repeat the same mistake
(`src/minisweagent/config/benchmarks/swebench.yaml`).

### Output and limits

The SWE-bench observation template shows the return code and the output. Past
10,000 characters it shows the first and last 5,000, the number of characters
left out, and a warning that tells the model to use `head`, `tail`, `sed` or
a narrower search (`src/minisweagent/config/benchmarks/swebench.yaml`,
`observation_template`). The same file sets `step_limit: 250` and
`cost_limit: 3.` and a 60-second command timeout. `AgentConfig` defaults are
no step limit, a cost limit of 3.0 and no wall-time limit
(`src/minisweagent/agents/default.py`).

### Benchmarks and tests

`run/benchmarks/swebench.py` runs instances on a thread pool, one Docker image
per instance (`get_swebench_docker_image_name`), and writes each submission to
`preds.json`. Scoring is a separate step through `sb-cli` or the SWE-bench
harness (`docs/usage/swebench.md`). The README claims more than 74% on
SWE-bench Verified; this chapter did not check that number. Tests use
`DeterministicModel`, which returns a fixed list of messages in order
(`src/minisweagent/models/test_models.py`), and recorded trajectories under
`tests/test_data/`.

## Microcoder and mini-SWE-agent side by side

Coder's own small loop is Microcoder: [docs/coder/microcoder.md](../../docs/coder/microcoder.md)
specifies it, and [`coax/lib/microcoder/`](../../coax/lib/microcoder/loop.coax)
implements it. The two loops share a shape: one model call per step, shell
commands as the only way to act, each command in its own process, and a
bounded count of bad replies. They differ in what the model sees.

| | mini-SWE-agent | Microcoder |
| --- | --- | --- |
| What one step sends | The full message list, appended to on every step | The system prompt and one user message: the state as JSON, rebuilt every step (`mc_request` in [loop.coax](../../coax/lib/microcoder/loop.coax)) |
| History kept | All of it | The last 12 actions, each with commands, exit codes and output (`MC_RECENT` in [state.coax](../../coax/lib/microcoder/state.coax)) |
| Action format | A `bash` tool call per command, or one command in text | One `next_action` JSON object: `rationale`, `commands`, `view`, `finished`, `reply`, `ask`; a text fallback reads the object out of prose ([text_action.coax](../../coax/lib/microcoder/text_action.coax)) |
| Commands | `bash -c` per command; process group killed on timeout | `/bin/sh` per command, 30 seconds each; the process group is killed past it ([tools.coax](../../coax/lib/microcoder/tools.coax)) |
| Reading files | Through commands such as `cat` and `sed` | Also a `view` list, up to 64 KB per file under the working directory |
| Long output | Past 10,000 characters: first and last 5,000 plus a warning | First and last 6,000 bytes with a line that counts what was left out (`mc_write_cut`) |
| Ending | The sentinel line in a command's output, exit 0 | `finished: true` with a `reply`, or `ask: question` |
| Bad replies | 3 format errors in a row end the run | 3 schema failures in a row end the run as `bad_replies` |
| Limits | Step, cost and wall time in configuration | None of its own beyond output caps; the person sets limits. An `idle` ending after 3 steps with nothing done |
| Trajectory on disk | Saved after every step | Not yet; the run reports steps to the TUI and the Lab ([run.coax](../../coax/lib/microcoder/run.coax)) |

The largest difference is the prompt. mini-SWE-agent's prompt only grows, so
each request shares its whole earlier prefix with the last one, and a provider
prefix cache can reuse it. That is why `cache_control.py` exists. Microcoder
rebuilds a state object each step, so only the system prompt is a stable
prefix; in exchange, its prompt has a fixed upper size and never needs
condensing. These are two answers to the question [chapter 3](ch003-keep-the-prefix-stable.md)
and [chapter 7](ch007-condense-the-history.md) ask: keep the prefix stable and
condense rarely, or keep the prompt small and pay the full prefill each step.

The second difference is how a run ends. mini-SWE-agent makes submission an
action inside the environment, so the model must run a command to finish.
Microcoder makes it a field in the reply, which also lets it end on a question
to the person (`asked`). Coder answers people in a terminal or the Lab, not
only a benchmark grader, so the field fits it better. The sentinel suits a
grader that only wants a patch.

## What Coder takes

- **A shell-only baseline that stays in the book.** Chapter 21's comparisons
  need a fixed, simple configuration. Coder takes mini-SWE-agent's discipline:
  the baseline has one action type (commands), no persistent shell and no
  condensing, and every other feature is measured as one change from it.
  Microcoder in its current form is close to that baseline already.
- **Independent commands.** Microcoder already runs each command in a new
  process and kills the process group on timeout. The FAQ's reasons (no
  end-of-command detection, no shell to corrupt) are the same reasons Coder
  keeps it.
- **Format errors go back to the model.** Both loops refuse a bad reply with
  the reason and count failures in a row. Coder keeps mini-SWE-agent's extra
  case: when a reply was cut off by the output token limit, say so, rather
  than describe it as a format error. Microcoder's `mc_bad` currently reports
  "the reply was cut off before it finished" for a stream that ends early,
  which covers the same case.
- **Save the trajectory after every step.** mini-SWE-agent writes the whole
  run to disk after each step, including failed steps. Microcoder does not yet
  persist runs. [Chapter 2](ch002-model-visible-means-logged.md)'s log is the
  place this lands, with the stricter rule that every request can be rebuilt
  from the log byte for byte.
- **A scripted model in tests.** `DeterministicModel` matches Microcoder's
  `McScript` in [loop.coax](../../coax/lib/microcoder/loop.coax): a list of
  replies fed in order, with no network. Coder keeps this as the default way
  to test loop logic.

## What Coder leaves, and why

- **The sentinel-line ending.** Coder ends on a typed field in the reply,
  because a Coder run can end with an answer or a question for a person, and
  the field cannot be produced by accident from a command's output.
- **Cost and step limits set by the agent.** mini-SWE-agent sets a cost limit
  of 3.0 by default. Coder sets no step or time limits of its own; limits the
  person sets are honoured. A benchmark run in chapter 21 sets its own budget
  per suite, so results stay comparable without a default in the loop.
- **The unbounded history as the only mode.** Coder keeps the full log on disk
  (chapter 2) but does not commit to sending all of it every step. Whether the
  growing history or Microcoder's bounded state wins is a measurement, not a
  default (see below).
- **Python, LiteLLM and Jinja templates.** Coder's loop is Coax and its
  gateways are Coax files per provider ([plan](PLAN.md#rules-for-every-chapter),
  [docs/coder/microcoder.md](../../docs/coder/microcoder.md#part-2-the-gateways)).
  Prompts are fixed strings in the loop, such as `MC_SYSTEM`, so a request is
  reproducible from the log without a template engine.

## Where a borrowed design is measured

- **The baseline itself:** [chapter 21](ch021-measure-the-harness.md) runs
  end-to-end suites one change at a time against [chapter 1](ch001-the-simplest-loop.md)'s
  loop. A mini-SWE-agent-shaped configuration (full history, bash only,
  independent commands) is a useful second reference: it lets Coder state
  where its own baseline sits relative to a public one on the same tasks.
- **Full history against bounded state:** the same suite run with Microcoder's
  12-action state and with a growing message list, on the same model,
  measured for task success, input tokens and cached tokens per step.
  [Chapter 3](ch003-keep-the-prefix-stable.md) gives the cache measurement and
  [chapter 7](ch007-condense-the-history.md) the condensing alternative.
- **Several commands per step:** mini-SWE-agent allows parallel tool calls for
  independent commands; Microcoder accepts a list of commands in one action.
  Chapter 21 can measure steps and tokens per task with a one-command limit
  and without it.
- **On a served model:** [chapter 12](ch012-a-psionic-served-model.md) runs
  Microcoder's loop on a model Psionic serves, where the prefix-cache effect of
  an append-only history can be measured from the server side.

## Sources

- [`README.md`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/README.md)
- [`src/minisweagent/agents/default.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/agents/default.py)
- [`src/minisweagent/environments/local.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/environments/local.py)
- [`src/minisweagent/environments/docker.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/environments/docker.py)
- [`src/minisweagent/models/utils/actions_toolcall.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/models/utils/actions_toolcall.py)
- [`src/minisweagent/models/utils/actions_text.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/models/utils/actions_text.py)
- [`src/minisweagent/models/utils/cache_control.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/models/utils/cache_control.py)
- [`src/minisweagent/models/test_models.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/models/test_models.py)
- [`src/minisweagent/config/benchmarks/swebench.yaml`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/config/benchmarks/swebench.yaml)
- [`src/minisweagent/run/benchmarks/swebench.py`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/src/minisweagent/run/benchmarks/swebench.py)
- [`docs/faq.md`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/docs/faq.md)
- [`docs/usage/swebench.md`](https://github.com/SWE-agent/mini-swe-agent/blob/04d809ceab9df28f9adaed044884180159172930/docs/usage/swebench.md)
- Coder: [docs/coder/microcoder.md](../../docs/coder/microcoder.md),
  [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax),
  [`state.coax`](../../coax/lib/microcoder/state.coax),
  [`run.coax`](../../coax/lib/microcoder/run.coax),
  [`tools.coax`](../../coax/lib/microcoder/tools.coax).
- [Chapter plan](PLAN.md), Part VII; the shape follows the
  [DeepSeek Harness notes](../../psionic/docs/harness.md).
