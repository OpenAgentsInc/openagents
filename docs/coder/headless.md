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
  "error": null,
  "cause": null,
  "refusal": null
}
```

The keys are always present. `reply`, `route`, and `usage` are null when
the turn did not finish; `error`, `cause`, and `refusal` are null when it
did. A script parses one thing.

## Why a turn did not finish

`error` is the sentence a person reads. `cause` and `refusal` are what a
harness reads, because several different states produce the same exit code
and telling them apart by matching on the sentence would mean depending on
its wording.

| `cause` | What happened |
| --- | --- |
| `relay_unreachable` | The relay would not take the job: the socket never opened, the NIP-42 challenge went unanswered, or the relay rejected the request event. Nothing reached a worker. |
| `worker_absent` | The relay took the job and nothing at all came back from the worker within the contact wait, 30 seconds. Either nobody is listening on that key, or somebody is and said nothing while it worked. |
| `worker_stalled` | Something came back from the worker — a judgment, a partial, a status — and then the answer never finished within the answer wait, 180 seconds. |
| `worker_declined` | A worker answered with a typed refusal. `refusal` carries the NIP-CJ code, such as `quota_exhausted`. |
| `door` | An own-key door answered with an error status, or the HTTP call failed. |
| `stream` | The door's stream broke or carried an error event. |
| `config` | The environment does not name one door: it names two, or it names one that cannot be built. The run ends before the turn. |
| `trace` | A named trace could not be opened, so the run ended before the turn. |

`worker_absent` and `worker_stalled` are both silence, and they are two
words because they are two problems. NIP-CJ's kinds are ephemeral, so a
client cannot prove a worker is missing — nothing is left on the relay to
ask about. What it can do is wait for a sign of life on a much shorter
clock than it waits for a model, and say which wait ran out.

`refusal` is non-null only for `worker_declined`, and it is read as a
field rather than searched for in the message. That is the line
`gym::eval::classify` draws: a typed refusal is an answer, a failure with
no code is the harness.

The trace says the same thing. A failed turn records a `System` step
reading `the turn did not finish (<cause>): <reason>` before the log
closes, so a reader holding only the trace does not find a session that
stops mid-turn with no reason given.

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
- [`relay-transport.md`](relay-transport.md) — the same turn over a direct
  door and over the relay, with the latency and the refusal causes
  measured.
