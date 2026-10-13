# Solve an issue

Solver takes a GitHub issue to a pull request. It works in its own fresh
copy of your project, so your checkout isn't touched, and it opens the
pull request only when the checks pass.

## Before you start

On the computer that runs it you need:

- the project's Git checkout, with `git`;
- the GitHub CLI, signed in: `gh auth login`;
- Claude Code signed in (the default), or your openagents.com sign-in
  (`coder login`) so it can run on Coder's own loop.

## From the chat

Ask in plain words, naming the issue with its repository:

```text
Solve acme/web#42
```

Add "without a pull request" to stop after the checks.

Solver shows as an agent under the input. `Down` opens its
conversation, where each step shows as it happens. When it's done, its
report comes back to your chat with the outcome, the pull request, and
the cost. Stopping the chat stops the run.

## From the command line

Run it in the project's checkout, naming the GitHub repository:

```sh
coder solver 42 --github acme/web
```

It opens a full-screen view of the run. Add `--plain` to print it as text
instead. Without `--github`, Solver looks in the OpenAgents repository
(`OpenAgentsInc/openagents`).

## How it works

Four helpers take turns, each with only what it needs:

| Helper | Does |
| --- | --- |
| **Intaker** | Checks that `git`, `gh`, and the agent are ready, reads the issue, and makes the fresh copy of your project. |
| **Briefer** | Finds the files that matter and writes what the change should look like, which tests to add, and which checks prove it. |
| **Coder** | Writes the change. |
| **Tester** | Runs the checks. When they fail, it hands the work back to the Coder or the Briefer with notes, up to two more rounds. |

When the checks pass and the issue is still open, Solver commits to a
`coder/issue-N` branch, pushes it, and opens the pull request. If the
issue looks already done or no longer current, it stops and says why.
Merging stays with you.

## Options

| Option | Does |
| --- | --- |
| `--github OWNER/REPO` | The issue's repository. |
| `--repo PATH` | The checkout to work from (default: this folder). |
| `--engine NAME` | What the helpers run on: `claude`, `opencode`, `coder`, or `devin`. |
| `--rounds N` | How many times the Tester may hand work back (default 2). |
| `--coder-model NAME` | The Coder's model, such as `claude-opus-5-5`. |
| `--no-pr` | Stop after the checks; open no pull request. |
| `--no-gate` | Work the issue even if it looks already done. |
| `--plain`, `--json` | Print the run as text, or as one JSON line per event. |

`coder solver --help` lists every option. Each run keeps its record and
its copy of the project under `~/.openagents/coder/solver-runs/`.

Next: [Safety](/docs/coder-safety).
