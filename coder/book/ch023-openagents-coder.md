# 23. `openagents coder`

> **Status: unimplemented — design draft.** The chapter text and its checks aren't written yet. The
> domain is specified in the
> [CLI spec](../../docs/cli/2026-11-01-openagents-cli-v1.md#the-coder-domain-planned).

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

## Dependencies

- Every earlier chapter, for the verb it adds.

## Done means

- Each verb exists in the command table with `--json` and the spec's exit
  codes, and a `coder-journey` check runs `run`, `log`, `fork`, `stop` and
  `replay` on a fixture repository.

## Sources

- [The openagents CLI, v1.0.0](../../docs/cli/2026-11-01-openagents-cli-v1.md).
