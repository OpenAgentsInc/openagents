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
| `--json` | Report the turn as a JSON stream: one object per turn event, then the summary object. |
| `--json-deltas` | With `--json`, also stream the reply's deltas as `delta` objects. |
| `--programs <SPEC>` | Grant named program slugs, subject to the environment effect ceiling. |
| `-h`, `--help` | Print the usage text. |

The environment picks the door exactly as it does in the terminal. When
Claude Code or Codex is installed and signed in, the delegate door answers
the turn from a Jev briefing; `CODER_DELEGATE=off` turns it off and
`CODER_DELEGATE=always` refuses to start without it. See
[the delegate door](../runtime/delegate-door.md). Without a target, or
with delegation off, the fallback is the door the rest of the environment
names: `TYPESAFE_API_KEY` turns classify on, `CODER_DOOR_KEY` with
`CODER_DOOR_URL` and `CODER_MODEL` take an own-key door, `CODER_WORKER`
with `CODER_RELAY` routes the turn through the relay, and with none of
them set the stub door answers. The first line on standard error names the
door and why, such as
`answering with delegate (claude-code/claude-opus-5-5) because claude-code is installed at … and authenticated (cli_login)`.

The environment also says what a turn may do to the machine, again
exactly as in the terminal: `CODER_SHELL=off` withdraws execution, and a
headless turn then answers without running a command whatever its reply
asks for. See [the shell loop](../runtime/shell-loop.md#execution-intent).

Program execution separately requires `CODER_PROGRAMS` or `--programs`. See
[program authority](program-authority.md) for the effect ceiling and refusal behavior.

## Lanes

The default door is the Vercel AI Gateway, which serves one Open
Responses shape for every model in its catalog. A second model is
therefore configuration rather than a second client, and `CODER_MODEL`
takes either a lane's short name or a gateway model id:

| Lane | Model |
| --- | --- |
| `gemini` | `google/gemini-3.8-flash`, the default |
| `glm` | `zai/glm-5.3-flash` |

```bash
CODER_MODEL=glm cargo run -p coder -- -p "count the crates"
```

A name that is no lane is sent as a model id, so the rest of the
gateway's catalog stays reachable. `coder-worker` reads
`CODER_WORKER_MODEL` instead, and it outranks `CODER_MODEL`: the model a
service pays for is not automatically the model someone would pick at
their own terminal. A lane named for a door that does not pick its own
model — the relay, the stub — is refused rather than ignored.

Model names live in `crates/coder/src/generate.rs` and nowhere else in
the crate. A model name is door identity, and `docs/gym/regression.md`
refuses a comparison when door identity moves, so a lane switch has to be
visible in a trace: the session header names the model for an own-key
door, and each answer step names it for a relay door, where the worker
picks. `docs/coder/runtime/traces.md` covers both.

## Waits

Every wait a streaming door can keep is bounded, in three places rather
than one:

| Bound | Value | What it covers |
| --- | --- | --- |
| Connect | 10 s | Accepting the connection. |
| First word | 30 s | The response headers. A door that has not answered by then is treated like one that dropped the connection, and the request is sent again, up to three attempts, waiting one second and then two. |
| Quiet | 120 s | The longest silence between two events of a stream, measured between events rather than between bytes. |

A door that never sends headers is asked again. One that sent headers is
not, because the caller may already have seen part of the answer and a
second attempt would repeat it. The failure says which happened: read
`door_absent` and `door_stalled` in the table below.

## What lands where

**Standard output** is the reply and nothing else, or, with `--json`, a
stream of objects — one line per event the turn reports, then the summary
object that closes it:

```json
{"event":"classified","route":"respond","halt":null,"action":{"choice":"respond","confidence":0.91,"probabilities":{"respond":0.91,"clarify":0.05,"end_conversation":0.02,"none":0.02}},"note":null}
{
  "reply": "atif, coder, coder-terminal, gym, jev, kev, lev, nostr, nostr-relay.",
  "trace": "/Users/you/.openagents/traces/20260919T142233Z-4f1a9c02.atif.jsonl",
  "outcome": "answered",
  "route": "respond",
  "program": null,
  "usage": { "input_tokens": 812, "output_tokens": 24 },
  "cost_usd": null,
  "error": null,
  "cause": null,
  "refusal": null,
  "events": true
}
```

Each event object carries an `event` name and the values the terminal
draws, under their own names: `program` (the selected slug), `classified`
(the verdict's `route` and the `action` answer behind it, a halt's reason
in `halt`, or the `note` saying classify did not run), `judgment` (a
remote worker's feedback line, or a delegated turn's progress line such as
`survey ▸ 40 files judged …` or `brief ▸ 10537 characters …`), and
`shell_proposed`, `shell_outcome`,
and `shell_verdict` — the command and its reason, its status and output,
and the judge's line. A field an event does not have is `null`, not a
stand-in. `delta` objects — the reply as it streams — come only with
`--json-deltas`: the reply lands whole in the summary, and a reply's
worth of deltas is a flood a pipe should opt into.

The summary's keys are always present. `reply`, `route`, and `usage` are
null when the turn did not finish; `error`, `cause`, and `refusal` are
null when it did. `cost_usd` is what a delegated turn spent in dollars,
Jev and the executor together, and null for a door that reports tokens
only or when any part of the cost is unknown; it is never a stand-in zero.
On a delegated turn, `shell_proposed` and `shell_outcome` are the commands
the executor ran, and `delta` objects are the executor's text as it
arrives. `events` says whether event lines came before it:
`false` means the run ended before the turn started and the object is the
whole output, so a reader holding only the last line can still tell a
stream from a bare result.

`program` is the slug of the program the turn ran, and null on an ordinary
turn — which is nearly every turn. A turn that runs a program takes no
classify route, so `route` is null there and the two fields are never both
set. The reply is the run's summary: which steps ran, how many delegations
answered, and where it stopped if it stopped. Progress goes to standard
error as `running program <slug>`, printed when the program is selected rather
than when it finishes, because a fan-out takes minutes.

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
| `door_absent` | An own-key door took the request and never sent response headers, on each of three attempts 30 seconds apart. |
| `door_stalled` | An own-key door sent its headers and then went quiet for 120 seconds, or kept sending without completing for 600 seconds. The message names the wait and how much had arrived, because a door that hung before saying anything and one that answered part way and stopped are different problems. |
| `stream` | The door's stream broke, ended before `response.completed`, said the response was incomplete, carried an error event, or carried a record that was not UTF-8 or not JSON. A stream that ends short of completion is never presented as an answer; the text that arrived is named in the failure. |
| `config` | The environment does not name one door: it names two, or it names one that cannot be built. The run ends before the turn. |
| `trace` | A named trace could not be opened, so the run ended before the turn. |

`worker_absent` and `worker_stalled` are both silence, and they are two
words because they are two problems. NIP-CJ's kinds are ephemeral, so a
client cannot prove a worker is missing — nothing is left on the relay to
ask about. What it can do is wait for a sign of life on a much shorter
clock than it waits for a model, and say which wait ran out.

`door_absent` and `door_stalled` are the streaming door's two words for
the same pair of problems, named for the door because a direct door has
no worker behind it. A harness reads either pair as a field.

`refusal` is non-null only for `worker_declined`, and it is read as a
field rather than searched for in the message. That is the line
`gym::eval::classify` draws: a typed refusal is an answer, a failure with
no code is the harness.

The trace says the same thing. A failed turn records a `System` step
reading `the turn did not finish (<cause>): <reason>` before the log
closes, so a reader holding only the trace does not find a session that
stops mid-turn with no reason given.

**Standard error** carries where the trace went, which door answers and
why, every shell command the turn ran, a delegated turn's progress lines
and what it spent, and the reason a turn failed. Reply deltas do not stream to
standard output: a command plan is a reply too, and streaming one would
put the plan's JSON in the middle of the answer.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The turn finished and the agent answered. |
| `1` | The turn did not finish. The door failed, or a named trace could not be opened. |
| `2` | The turn finished without answering. `outcome` says how: `declined` — Classify halted, or a program the turn ran stopped at a step that refused; `refused` — the model's last word was a command plan the host did not run, and `reply` is the host's account of why and of what did run. Either way there is no confident next step. |
| `64` | The command line was wrong. |

Declining is not failing. A turn that ran correctly and concluded it has no
next step is a result, and a harness that could not tell it from a dead
door would score the two the same. A refused plan shares the exit code
because it is the same kind of result: the host ran the turn to its bound
and has no answer it can stand behind. `outcome` tells the two apart.

## The trace

`--trace <PATH>` names the file, so a caller reads the trace back without
globbing a directory or guessing a session identifier. Naming a file is a
request to record, so it outranks `CODER_TRACE=off`. With no `--trace`,
recording follows the environment described in
[`traces.md`](../runtime/traces.md), and `--json` reports where the file landed.

A named trace that cannot be opened ends the run before the turn starts,
with exit code `1`. An existing file is one such reason: a session never
writes over another session's record. The alternative — running the turn
unrecorded and exiting `0` — would hand a harness a missing file and a
success.

## One turn, in one place

Headless mode is not a second agent loop. Both modes call
`coder::turn::run`, which asks which program the request wants, runs it or
classifies, routes, answers, and records; the
terminal turns its events into scrollback lines and `--print` turns them
into standard error. A turn written twice is two turns that drift, and the
drift would be invisible: an episode judged from a headless run would be
judging whichever copy the harness happened to call.

## Related

- [`traces.md`](../runtime/traces.md) — what a trace holds and where it goes.
- [`shell-loop.md`](../runtime/shell-loop.md) — the command loop a turn may run.
- [`relay-transport.md`](../measurements/relay-transport.md) — the same turn over a direct
  door and over the relay, with the latency and the refusal causes
  measured.
