# 10. Drive Claude Code

> **Status: partly implemented.** Coder runs tasks through the installed
> `claude` CLI (checked with Claude Code 2.1.316) on the person's own
> login: `coder run "TASK" --engine claude` and the terminal UI's first
> agent. The reply streams, each tool call and its result show as rows,
> edits show as diffs, and every event lands in the session log. With
> `--allow`, `--ceiling` or `--root`, chapter 5's policy answers permission
> requests. Still to come: in-process MCP servers that expose Coder's tools
> to Claude Code.

Many people already pay for Claude Code. Coder can run a task through it
instead of through its own loop, and keep the same log, approvals and
replies.

## The client

[coax/lib/agent/claude/](../../coax/lib/agent/claude/) speaks the `claude`
CLI's stream-json protocol, not the npm package: one JSON object a line
each way, typed stream messages, and the control protocol in both
directions (initialize, permission requests, interrupts). The query is a
state machine over lines that the caller feeds, with no callbacks
([query.coax](../../coax/lib/agent/claude/query.coax)). The child process
runs through `coax/platform/process-native.c`.
[PARITY.md](../../coax/lib/agent/claude/PARITY.md) lists each feature of
the official SDK and where it is, or isn't, in Coax.

## In Coder

[coder.coax](../../coder/tui/coder.coax) starts the CLI on the first
message and sends later messages to the same session, so the conversation
carries over. It asks for partial messages, so `content_block_delta` text
streams into the reply as it arrives. It removes `ANTHROPIC_API_KEY` from
the child's environment, so the run always uses the person's Claude Code
login. A subagent's own traffic (a message with a `parent_tool_use_id`)
stays out of the transcript.

Tools run without asking, as with `claude --dangerously-skip-permissions`.
With `--ask`, each permission request goes to the composer's y/a/n prompt
instead; in `coder run`, which has no person to ask, a request is refused.

```sh
coder/tui/coder.sh run 'Run `ls coder/tui | wc -l` and reply with only the number.' --engine claude --json
```

```json
{"kind":"session","id":"20261102-070358-208","version":"0.1.0","engine":"claude","cwd":"/private/tmp/claude-501/cxt/mc1","started_us":1793603038208611}
{"kind":"user","text":"Run `ls coder/tui | wc -l` and reply with only the number."}
{"kind":"tool","label":"Bash(ls coder/tui | wc -l)"}
{"kind":"result","text":"      10","error":false}
{"kind":"reply","text":"10"}
{"kind":"end","ending":"finished","ms":5436,"steps":1}
```

## Approvals by policy

Pass `--allow`, `--ceiling` or `--root` to answer permission requests by
policy instead of skipping them. The policy has chapter 5's shape:

- `--ceiling EFFECT`: the strongest effect the session may ever use.
  Default `run`.
- `--allow EFFECT`: the effect that runs without asking. Default `read`.
- `--root DIR,DIR`: folders a tool may touch without asking, as well as the
  working directory.

Effects, weakest first, are `read`, `network`, `write` and `run`. Each
request gets one verdict, written as an `approval` line in the session log:

| Verdict | When |
| --- | --- |
| refuse | The effect is above the ceiling. No one is asked. |
| run | The effect is at or below `--allow`, and every path is inside a root. |
| ask | Anything else. The terminal UI asks you; a job with no one to ask refuses. |

The effect comes from the tool's exact name: `Read`, `Glob` and `Grep` read,
`WebFetch` and `WebSearch` use the network, `Edit` and `Write` write, and
`Bash` and any tool Coder doesn't know run. Paths come from the input's
`file_path`, `path`, `notebook_path` and `cwd` fields. A path with a `..`
part counts as outside. A command's room is its working directory, so the
ceiling is what bounds a command.

```text
$ coder run "…" --engine claude --allow write --ceiling write
  ⏺ Write(hi.txt)
  Allowed Write(hi.txt) · policy: write inside the roots
```

```json
{"kind":"approval","tool":"Bash(touch b.txt)","effect":"run","inside":true,"verdict":"refuse","why":"run is above the ceiling, write"}
```

Claude Code still runs read-only commands such as `ls` without asking; it
sends Coder only the requests its own rules don't settle.

## Checks

```sh
coax test coax/lib/agent/claude/tests.coax --library         # 63 checks: frames, messages, the control protocol
coax test coax/lib/agent/claude/query_tests.coax --library   # 40 checks: the query state machine
coax/lib/agent/claude/smoke.sh                     # one live query
python3 coder/tui/smoke.py --live                  # two turns in the terminal UI; the second needs the first
```

The live checks set `BROWSER=/usr/bin/true`, so a missing login fails
instead of opening a sign-in page.

## Dependencies

- [Chapter 2](PLAN.md#i-the-loop): Claude Code's events land in Coder's log.
- [Chapter 5](PLAN.md#i-the-loop): the approval policy.
- [Chapter 16](PLAN.md#iii-many-tasks): MCP.

## Done means

- [x] The client's protocol and control checks pass in Coax.
- [x] `coder run --engine claude "TASK"` runs a task through the installed
  CLI, and its events appear in `coder log`.
- [x] The live smoke runs against a named Claude Code version (2.1.316) and
  never opens a browser sign-in.
- [ ] Coder's tools reach Claude Code as an in-process MCP server.
- [x] Permission requests are answered by chapter 5's policy: `--allow`,
  `--ceiling` and `--root`, checked live (a Write allowed, `touch` refused
  above a `write` ceiling) and offline against a stand-in Claude Code.

## Sources

- `coax/lib/agent/claude/` and its [PARITY.md](../../coax/lib/agent/claude/PARITY.md).
- `coder/tui/coder.coax` (the claude engine) and `coder/tui/session.coax`.
