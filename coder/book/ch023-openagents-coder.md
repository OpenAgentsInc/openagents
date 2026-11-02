# 23. `openagents coder`

> **Status: partly implemented.** `openagents coder` runs `run` (with
> `--detach`), `ls`, `status`, `log`, `stop`, `fork` and `route`, with
> `--json` and the spec's exit codes, and with no verb opens the terminal
> UI. Each conversation is a session file in `~/.openagents/coder/sessions`.
> Still to come: `attach`, `replay`, `bench`, the server verbs (`watch`,
> `policy`, `daemon`, `--on cloud`), and the same verbs as MCP tools.

Every chapter in this book ends in a verb. This chapter puts them into the
`coder` domain of the `openagents` binary.

## What this chapter covers

- The verbs:

  | Verb | What it does |
  | --- | --- |
  | `coder run "TASK" [--engine E] [--on mac\|cloud] [--cwd DIR]` | start a task; prints its ID and streams steps unless `--detach` |
  | `coder ls` | tasks with state, engine, Thread and age |
  | `coder status ID` | one task's state, last step and ending |
  | `coder log ID` | the task's session log as text or `--json` lines |
  | `coder attach ID` | follow a running task and send it messages |
  | `coder stop ID` | stop a task and its tools |
  | `coder fork ID [--at TURN]` | start a new task from a turn boundary |
  | `coder replay ID` | rebuild every request from the log and compare bytes |
  | `coder watch THREAD` | subscribe Coder to a Thread such as `github:owner/repo#123` |
  | `coder policy list\|add\|rm` | the trigger policies for acting without being asked |
  | `coder daemon` | attach this machine as an execution environment |
  | `coder bench SUITE` | run a harness suite and report it |

- One command table drives the parser, help, the MCP tool list and the
  reference table, like every other domain.
- Local first: `run`, `ls`, `status`, `log`, `attach`, `stop`, `fork` and
  `replay` work with no server. `watch`, `policy`, `daemon` and `--on cloud`
  need the server routes of chapters 17 and 20.
- The same verbs as MCP tools, so Claude Code or another agent can drive Coder.

## What runs

The `coder` domain is
[apps/openagents-cli/coder.coax](../../apps/openagents-cli/coder.coax). It
finds the checkout (`OPENAGENTS_SOURCE`, or the one the binary was built
in) and replaces itself with `coder/tui/coder.sh`, which rebuilds
`coder/tui/coder.coax` when it is stale. So the terminal UI gets the
terminal, and the verbs print straight to stdout. Coder owns the verbs; the
CLI only passes them on.

```sh
openagents coder run '$ echo via-cli' --engine echo
openagents coder ls
openagents coder log 20261102-0659
```

```text
› $ echo via-cli
  session 20261102-065910-982
  $ echo via-cli · exit 0
    via-cli
  └ finished · 0.0s · 1 step

20261102-065910-982  echo       finished  0s ago   $ echo via-cli
20261102-065728-986  microcoder finished  1m ago   What is 2+2? Reply with only the number.
```

### The session log

[coder/tui/session.coax](../../coder/tui/session.coax) writes one JSON
object a line, appended as things happen:

| Line | Fields |
| --- | --- |
| `session` | `id`, `version`, `engine`, `cwd`, `started_us` |
| `user` | `text` |
| `agent` | `engine`, when Shift-Tab switches |
| `tool` | `label`, as the transcript shows it |
| `result` | `text` (cut to 4,000 bytes, with `bytes` when cut), `error` |
| `diff` | `path`, `adds`, `dels`: one file an edit changed |
| `reply` | `text`: the reply since the last tool call, written before the next one and at the end |
| `end` | `ending`, `ms`, `steps`, and `text` when it failed |

An ID is the UTC start time to the millisecond, such as
`20261102-065910-982`, so the files sort by time and `status` and `log`
take any unique prefix. `/clear` in the terminal UI starts a new file.

`coder run --json` prints exactly these lines and nothing else, so a script
reads the run as it happens. Exit codes: 0 finished, 1 failed, 2 usage, 6 no
such session, 130 stopped.

## Dependencies

- Every earlier chapter, for the verb it adds.

## Done means

- [x] `run`, `ls`, `status`, `log`, `stop` and `fork` exist with `--json`
  and the spec's exit codes, and the offline smoke (`python3
  coder/tui/smoke.py`) runs them in a fresh `HOME`. `fork` and `stop` are
  chapter 14's; `route` is chapter 13's.
- [ ] Each verb exists in the command table with `--json` and the spec's exit
  codes, and a `coder-journey` check runs `run`, `log`, `fork`, `stop` and
  `replay` on a fixture repository.

## Sources

- [The openagents CLI, v1.0.0](../../docs/cli/2026-11-01-openagents-cli-v1.md).
- `apps/openagents-cli/coder.coax`, `coder/tui/coder.coax` (`tool_main` and the verbs), `coder/tui/session.coax`.
