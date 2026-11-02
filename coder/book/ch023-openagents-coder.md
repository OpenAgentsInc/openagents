# 23. `openagents coder`

> **Status: partly implemented.** `openagents coder` runs `run` (with
> `--detach`), `ls`, `status`, `log`, `stop`, `fork`, `route`, `attach`,
> `replay`, `bench`, `watch` and `policy`, with `--json` and the spec's exit
> codes, and with no verb opens the terminal UI. `trigger` and `router` run
> chapter 17's Router. `watch` and `policy` work locally through that Router
> instead of a server. Each conversation is a session file in
> `~/.openagents/coder/sessions`. `coder mcp` serves the verbs as MCP tools
> on stdio, and Claude Code drives Coder through it. Still to come: `daemon`
> and `--on cloud`.

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

### Following and replaying a session

`coder attach ID` prints the session's log as `coder log` does and keeps
following it until the session ends. `coder attach ID "MESSAGE"` also
leaves a message for the job:

- A running job (`coder run --detach`) takes it as its next turn when the
  current turn ends, so the conversation stays on the same engine.
- If the job has already ended, `attach` takes the message back and starts
  a fork that carries the conversation, then follows the fork.

Messages wait in `sessions/ID.inbox`, one file each. Whoever renames a
message first owns it, so a message the job and `attach` both reach is taken
once. Before a job ends, it removes its pid file and then looks for messages
once more, so a message sent while it ends is still taken.

`coder replay ID` rebuilds every request the session sent an engine from the
log alone, and compares bytes. Each turn logs a `request` line with the
SHA-256 and length of exactly what the engine was sent. `replay` rebuilds
that text from the `user` line, and for a fork's first turn, from the
parent's log up to the `at` turn. It then compares digests:

```text
turn 1: same bytes (226 bytes, sha256 6307ff109763…)
1 of 1 requests rebuilt with the same bytes.
```

A changed log fails: editing the fork's `user` line makes `replay` report
`DIFFERENT` and exit 1.

### Suites

`coder bench SUITE` runs a suite from the checkout and reports whether it
passed and how long it took. Each report is kept in
`~/.openagents/coder/bench/SUITE-ID.json`:

| Suite | What it runs |
| --- | --- |
| `harness` | Chapter 21's measurement: tasks through the harness with seeds, verdicts and a receipt chain |
| `checks` | `coax test coder/tui/coder.coax` |
| `smoke` | The offline smoke: the terminal UI in a pseudo-terminal, then every verb |

```json
{"suite":"checks","passed":true,"ms":5022,"at_us":1793605778040508,"last":"233 checks, 233 passed, 0 failed"}
```

### The Router's verbs

`coder trigger`, `coder router`, `coder watch` and `coder policy` run the
Router of chapters 17 and 20 on this machine:

- `coder policy add NAME --mode M [--authors A,B] [--engine E] [--label L]
  [--channel chat|issue] [--deliver] [--text POLICY]` writes a source's
  policy to `router/sources.json`; `list` and `rm` read and remove them.
- `coder watch THREAD` makes every later event on the Thread addressed to
  the agent. For a GitHub Thread such as `github:owner/repo#123` whose
  repository has no policy yet, it adds one that admits only the account
  `gh` is logged in as.

### The verbs as MCP tools

`coder mcp` ([mcp.coax](../../coder/tui/mcp.coax)) is an MCP server on
standard input and output, one JSON-RPC message a line. It answers
`initialize`, `ping`, `tools/list` and `tools/call`, and each tool runs the
matching verb as a child process, so the tools and the verbs can't drift
apart:

| Tool | Verb |
| --- | --- |
| `coder_run` | `coder run TASK --detach [--engine E] [--cwd DIR]`: returns the ID at once |
| `coder_ls`, `coder_status`, `coder_log`, `coder_stop`, `coder_replay` | the verb of the same name |
| `coder_send` | a message for a session: a running job takes it as its next turn; an ended one continues in a fork |
| `coder_fork` | `coder fork ID TASK --detach [--at N]` |

`coder_run` always runs a background job, so a client's tool-call timeout
never cuts a task short; the client follows it with `coder_status` and
`coder_log`. To give Claude Code these tools:

```sh
claude mcp add coder -- "$PWD/coder/tui/coder.sh" mcp
```

In the live check, `claude -p` called `coder_ls` through the server and
answered with the number of sessions on the machine.

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
| `request` | `sha256`, `bytes`: what the turn sent its engine, for `replay` |
| `route` | System One's engine decision, when `run` had no `--engine` (chapter 13) |
| `hook` | `on`, `plugin`, `run`, `ok`, `text`: a plugin hook that ran (chapter 15) |
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
- [x] `attach`, `replay`, `bench`, `watch` and `policy` exist, and the smoke
  checks each: a message taken by a running job and by a fork of an ended
  one, a fork's request rebuilt byte for byte and a changed log refused, a
  suite's report kept, a watched Thread's event starting a Worker, and
  sources added, listed and removed.
- [x] The verbs are MCP tools (`coder mcp`); the smoke starts a run and
  reads its log through them, and Claude Code called one live.
- [ ] Each verb exists in the command table with `--json` and the spec's exit
  codes, and a `coder-journey` check runs `run`, `log`, `fork`, `stop` and
  `replay` on a fixture repository.

## Sources

- [The openagents CLI, v1.0.0](../../docs/cli/2026-11-01-openagents-cli-v1.md).
- `apps/openagents-cli/coder.coax`, `coder/tui/coder.coax` (`tool_main` and the verbs), `coder/tui/session.coax`.
