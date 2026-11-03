# 9. Microcoder

> **Status: implemented.** The loop runs in Coder: `coder run
> "TASK" --engine micro` and the terminal UI's third agent both drive
> `coax/lib/microcoder/` through a gateway plan of Vertex Gemini Flash, the
> Gemini API and OpenRouter. Its checks cover every ending, the state's
> caps and the reply streaming out of a partial action. Gateway streams
> recorded live replay through the loop with no credentials, the same each
> time, and replayed responses reach each ending through `coder run`.

The [repository pilot](../../docs/coder/repository-pilot.md) exercises this
engine on actual edits and external acceptance checks. Gateway playback
below still executes returned commands; it is different from
[`coder replay` input verification](CONTRACT.md#what-replay-means).

Microcoder is Coder's own engine: a small loop that runs on any model we can
reach and needs no other agent installed. It is the engine for plain
questions, small jobs and machines without Claude Code or Codex.

## The loop

Each step makes one structured call. The request is the state, rendered as
one JSON object ([state.coax](../../coax/lib/microcoder/state.coax)):

- the environment facts (the operating system, the app and the working
  directory) and, in Coder, the earlier turns of the conversation;
- the task and the routing decision;
- the last 12 actions, each command with its exit status and the first and
  last 6,000 bytes of its output;
- the files the model asked to view, and the loop's own notes.

The model returns `next_action`
([action.coax](../../coax/lib/microcoder/action.coax)): a rationale for
the log, commands to run in order, files to view, `finished`, the reply and
whether it asks a question. A gateway that can't hold a strict JSON schema
answers in prose, and
[text_action.coax](../../coax/lib/microcoder/text_action.coax) reads the
action out of the text: a fenced object after prose, a leaked reasoning
block, a command written as a string or an object.

Commands run in the working directory with a timeout and capped output, and
`view` reads capped files under it
([tools.coax](../../coax/lib/microcoder/tools.coax)). Writes go through
commands, so every change shows in the transcript.

## Endings

A run ends one way, and the run reports which
([loop.coax](../../coax/lib/microcoder/loop.coax)):

| Ending | When |
| --- | --- |
| `finished` | the model set `finished` and gave a reply |
| `asked` | the model asked the person a question |
| `bad_replies` | three replies in a row failed the schema or its meaning |
| `no_capacity` | no gateway in the plan answered |
| `stopped` | the person pressed Esc or sent STOP |
| `idle` | the model gave nothing to do and no reply |

The person sets limits; the loop adds none beyond the output caps. A good
reply between bad ones resets the count, and each refusal is written into
the next step's notes so the model can correct itself.

## In Coder

[run.coax](../../coax/lib/microcoder/run.coax) makes the loop pollable:
`mc_run_start`, then `mc_run_poll` returns events (an attempt, more reply
text, a command and its output, a file viewed, the ending), and
`mc_run_stop` ends it. The reply streams out of the `reply` member of a
partial action as the bytes arrive.

Coder runs the loop in a worker process of its own,
[coder/tui/mcworker.coax](../../coder/tui/mcworker.coax), and talks to it in
JSON lines the way it talks to the Claude Code and Codex CLIs. The worker
keeps the conversation (the last six turns, each cut to 1,200 bytes, passed
as `conversation` in the facts) and owns the credentials.
[microcoder_keys.coax](../../coder/tui/microcoder_keys.coax) reads them as
the desktop Lab does: files under `~/work/.secrets` go into curl's child
environment only, and Vertex's token is minted by gcloud from a
service-account key file with a gcloud config of its own. The plan is every
gateway with a credential, Vertex first; when Vertex is skipped, the worker
says why once on stderr.

```sh
coder/tui/coder.sh run "What is 2+2? Reply with only the number." --engine micro
```

```text
› What is 2+2? Reply with only the number.
  session 20261102-065728-986
● 4
  └ finished · 6.4s
```

## Record and replay

The gateway transport ([coax/lib/gateway/transport.coax](../../coax/lib/gateway/transport.coax))
can keep what a server sent and play it back:

- `GW_RECORD=DIR` saves each response as `DIR/1.http`, `DIR/2.http` and so
  on: the status line, the headers and the body, as `curl -i` prints them.
  Nothing of the request is kept, so no key. While recording, a response
  arrives whole rather than streamed.
- `GW_REPLAY=DIR` sends nothing. Each request reads the next saved response
  instead of starting curl, in order, so no credential or server is needed.
  The plan holds one Vertex attempt, and the psionic engine skips its
  server check.

Both keep their place in `DIR/.next`. The commands a step asks for still run
for real, so a recording should ask for ones that give the same output on
any machine.

[coder/tui/fixtures/micro-replay](../../coder/tui/fixtures/micro-replay)
holds two responses recorded from Vertex Gemini Flash for "Run `echo
replay-check` and then tell me what it printed". Replayed with no
credentials, the session log matched the live one line for line, and two
replays matched each other byte for byte apart from the session's ID,
times and folder: the same request digest, the same command and result,
and the reply "It printed replay-check."

The smoke also reaches the endings through `coder run`, each from a few
replayed responses written in the check: a question (`asked`, which Coder
shows as the reply and ends as finished), three replies that aren't JSON
(`bad_replies`), a 503 on every try (`no_capacity`) and three empty steps
(`idle`). Each failed ending says what happened in plain words, such as "The
model gave nothing to do and no reply three times in a row." `stopped` is
the unit check's and the smoke's `coder stop` checks'.

## Checks

```sh
coax test coax/lib/microcoder/run.coax --library   # the loop, state, actions, tools and gateways: 83 checks
coax/lib/microcoder/tools_smoke.sh           # commands and views in a real directory
coax/lib/microcoder/live.sh                  # one live run on the plan
python3 coder/tui/smoke.py --live microcoder # two turns in the terminal UI; the second needs the first
python3 coder/tui/smoke.py                   # the Vertex recording replayed twice, with no credentials;
                                             # asked, bad_replies, no_capacity and idle through coder run
```

## Dependencies

- [Chapter 1](PLAN.md#i-the-loop): the simplest loop, which Microcoder is an
  instance of.
- [Chapter 5](PLAN.md#i-the-loop): confinement for its commands.

## Done means

- [x] `coder run --engine micro "TASK"` runs the loop in the current
  directory and prints each step and the ending.
- [x] Recorded gateway streams replay through the loop with byte-identical
  logs, apart from the session's ID, times and folder.
- [x] Each ending is reached by a scripted check
  (`loop.coax`: "asked, no_capacity, idle and stopped", "three replies in
  a row that fail the schema"), and all but `stopped` also through `coder
  run` with replayed responses.

## Sources

- `coax/lib/microcoder/` (`loop.coax`, `state.coax`, `action.coax`,
  `text_action.coax`, `tools.coax`, `run.coax`).
- `coder/tui/mcworker.coax`, `coder/tui/microcoder_keys.coax`,
  `coder/tui/microcoder.coax`.
- [Microcoder in Coax, and the inference gateways](../../docs/coder/microcoder.md).
