# Solve an issue

Solver takes a GitHub issue to a pull request. It works in its own fresh
copy of your project, so your checkout isn't touched, and it opens the
pull request only when the checks pass.

Solver comes in the next Coder release.

## Before you start

On the computer that runs it you need:

- the project's Git checkout, with `git`;
- the GitHub CLI, signed in: `gh auth login`;
- Claude Code signed in (the default), or your openagents.com sign-in
  (`coder login`) so it can run on Coder's own loop.

## From the chat

Ask in plain words, naming the issue by its number:

```text
Solve #42
```

A bare number is an issue of the repository your chat's folder is in
(its GitHub remote, `origin` first). For an issue in another repository,
name it: `Solve acme/web#42`. If the folder has no GitHub remote, Solver
asks which repository.

Add "without a pull request" to stop after the checks.

Solver shows as an agent under the input. `Down` opens its
conversation, where each step shows as it happens. When it's done, its
report comes back to your chat with the outcome, the pull request, and
the cost. Stopping the chat stops the run.

## From the command line

Run it in the project's checkout with the issue number:

```sh
coder solver 42
```

It opens a full-screen view of the run. Add `--plain` to print it as text
instead. The issue is looked up in the checkout's GitHub repository (its
`origin` remote, or another GitHub remote). Add `--github acme/web` for an
issue in another repository, or when the checkout has no GitHub remote.

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
| `--github OWNER/REPO` | The issue's repository (default: this checkout's GitHub remote). |
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
