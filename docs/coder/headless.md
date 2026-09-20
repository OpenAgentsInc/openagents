# Headless mode

`coder -p "…"` runs one turn without a terminal: it reads the prompt, runs
the turn, writes the reply to standard output, and exits. No raw mode, no
alternate screen, nothing to watch.

Status: implemented in `crates/coder` (`cli.rs`, `headless.rs`, and
`turn.rs`, the turn both modes run).

## Why this exists

The binary used to enter raw mode and an alternate screen before it did
anything else, so the only way to ask Coder a question was to sit in front
of it. That made every question about the agent's behavior a question
about what a person saw, and it made an episode impossible to run: you
cannot judge a trace against a golden when producing the trace needs a
human at a keyboard.

## Running one

```sh
coder -p "count the crates"
coder --prompt-file ask.txt
coder -p --json --trace runs/one.atif.jsonl "count the crates"
```

| Flag | Effect |
| --- | --- |
| `-p`, `--print <PROMPT>` | Run one turn without a terminal. |
| `--prompt-file <FILE>` | Read the prompt from a file, newlines and all. Implies `--print`. |
| `--trace <PATH>` | Write this session's trace to `PATH`. |
| `--json` | Report the turn as one JSON object instead of as text. |
| `-h`, `--help` | Print the usage text. |

The environment picks the door exactly as it does in the terminal:
`TYPESAFE_API_KEY` turns classify on, `CODER_DOOR_KEY` with
`CODER_DOOR_URL` and `CODER_MODEL` take an own-key door, `CODER_WORKER`
with `CODER_RELAY` routes the turn through the relay, and with none of
them set the stub door answers.

## What lands where

**Standard output** is the reply and nothing else, or, with `--json`, one
object:

```json
{
  "reply": "atif, coder, coder-terminal, gym, jev, kev, lev, nostr, nostr-relay.",
  "trace": "/Users/you/.openagents/traces/20260919T142233Z-4f1a9c02.atif.jsonl",
  "outcome": "answered",
  "route": "respond",
  "usage": { "input_tokens": 812, "output_tokens": 24 },
  "error": null
}
```

The keys are always present. `reply`, `route`, and `usage` are null when
the turn did not finish; `error` is null when it did. A script parses one
thing.

**Standard error** carries where the trace went, every shell command the
turn ran, and the reason a turn failed. Reply deltas do not stream to
standard output: a command plan is a reply too, and streaming one would
put the plan's JSON in the middle of the answer.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The turn finished and the agent answered. |
| `1` | The turn did not finish. The door failed, or a named trace could not be opened. |
| `2` | The turn finished and the router declined it — Classify halted, and the answer is that there is no confident next step. |
| `64` | The command line was wrong. |

Declining is not failing. A turn that ran correctly and concluded it has no
next step is a result, and a harness that could not tell it from a dead
door would score the two the same.

## The trace

`--trace <PATH>` names the file, so a caller reads the trace back without
globbing a directory or guessing a session identifier. Naming a file is a
request to record, so it outranks `CODER_TRACE=off`. With no `--trace`,
recording follows the environment described in
[`traces.md`](traces.md), and `--json` reports where the file landed.

A named trace that cannot be opened ends the run before the turn starts,
with exit code `1`. An existing file is one such reason: a session never
writes over another session's record. The alternative — running the turn
unrecorded and exiting `0` — would hand a harness a missing file and a
success.

## One turn, in one place

Headless mode is not a second agent loop. Both modes call
`coder::turn::run`, which classifies, routes, answers, and records; the
terminal turns its events into scrollback lines and `--print` turns them
into standard error. A turn written twice is two turns that drift, and the
drift would be invisible: an episode judged from a headless run would be
judging whichever copy the harness happened to call.

## Related

- [`traces.md`](traces.md) — what a trace holds and where it goes.
- [`shell-loop.md`](shell-loop.md) — the command loop a turn may run.
