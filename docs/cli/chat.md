# `openagents chat`: OpenAgents from the terminal

`openagents chat` sends a message to OpenAgents, the chat router the phone
and the desktop talk to, and prints the reply as it streams. People, scripts,
and agents get the same answers, the same knowledge answers, and the same
Coder handoff. Tracking: [#10031](https://github.com/OpenAgentsInc/openagents/issues/10031).

The command adds no chat logic. Every operation is one of the shared chat
service's commands (`openagents_chat::service::Command`: create, send, read,
stop, list, run Coder) and every answer is its `Snapshot`, the types the phone
and the desktop's host already use. The chat worker's router decides every
route; the command only shows what it said.

The unit is the **thread** ([glossary](../glossary.md)): one conversation
with OpenAgents, one row of the chat list. Every run prints the thread ID it
used, so a script can continue it.

## Commands

```text
openagents chat MESSAGE                        same as `chat send MESSAGE`
openagents chat send MESSAGE [--thread ID] [--run-coder] [--timeout SECONDS]
openagents chat threads [--all] [--limit N]
openagents chat read --thread ID
openagents chat export --thread ID
openagents chat run-coder --thread ID
```

Every command also takes `--scratch`, `--local`, and `--socket PATH`, and
`--json` before or after the group.

- `send` (or a bare message) starts a new thread, or continues `--thread ID`
  with its context. `MESSAGE` may be several words; `-` reads it from stdin
  (`echo "What is the Gym?" | openagents chat -`). A message that is itself
  a command word (`threads`, `read`) needs `send`.
- `threads` lists threads newest first; `--all` includes archived ones.
- `read` prints a thread's turns, every page of them.
- `export` prints the thread as its `ATIF-v1.8` trajectory
  ([below](#threads-as-atif)). The command checks the document with
  `crates/atif` before it prints it.
- `run-coder` accepts OpenAgents' offer to run Coder for the thread.

Thread IDs are 32 lowercase hex characters, the IDs the phone and the
desktop use.

## Where threads live

1. **This computer's host.** When the host runs (the OpenAgents app, or
   `openagents host serve --control`), the command speaks the host's
   same-user control socket, the one the desktop app uses. Threads are the
   desktop's threads: one sent from the terminal shows in the app's chat
   list, and one started in the app is readable here. The host holds the
   device key and the encrypted store (`<host root>/basic-chats`).
   `--socket PATH` names another host's socket and fails if nothing answers.
2. **In process.** Without a host, or with `--local`, the chat service runs
   inside the command with its own device key, `~/.openagents/chat/device.key`
   (created on first use, mode `0600`, never printed), and its own encrypted
   store, `~/.openagents/chat/threads/`. `OPENAGENTS_CHAT_HOME` moves both.
3. **Scratch.** `--scratch` uses a throwaway key and store for one thread,
   in `$TMPDIR/openagents-chat-scratch/<thread>/`, and never touches the host
   or real history. Continue, read, or export that thread with `--scratch
   --thread ID`. It is for tests and smokes.

`openagents doctor` shows which one a run would use, the host socket, the
chat home, and the command's public identity (never the key).

The in-process and scratch modes talk to the public chat worker on
`wss://relay.openagents.com` as a NIP-CJ client signed by their own key, so
they are admitted under the worker's per-key quota like a fresh phone.
`OPENAGENTS_CHAT_RELAY` and `OPENAGENTS_CHAT_WORKER` point them at another
relay and worker (the fixture tests use a local relay). The request says
`surface: "terminal"` and `client: "openagents-cli"`; through the host it
says what the host says (`desktop`). No model key is needed.

## Coder offers

The router may offer to run Coder on a computer for a thread. The offer is
printed (on stderr, with the command that accepts it) and, under `--json`,
emitted as an `offer` event. Nothing runs until it is accepted.

`--run-coder` on `send`, or `openagents chat run-coder --thread ID`, accepts
it through the host's own handoff (`Command::RunCoder`, the path the
desktop's Run Coder uses, #10015): the host prepares the handoff prompt from
the thread, dispatches a Coder task to its project, and records the task
on the thread. The host's refusals come back verbatim, for example "Choose a
project in Settings before running Coder." A thread in the in-process or
scratch store has no Coder broker: the command prints the offer, says
explicitly that it was not accepted, and exits 1.

## Stopping

`--timeout SECONDS` (default 120, the worker's own limit) and Ctrl-C stop
receiving the reply, with the apps' words: "Stopped receiving this reply.
The hosted worker may still finish." What streamed is kept as a stopped
reply. Stopping does not cancel the remote work. The run exits 1.

## Output

Without `--json`, the reply streams to stdout and everything else goes to
stderr: the served knowledge answer, offers, suggestions, and `thread ID`.
`reply=$(openagents chat "…")` captures only the reply.

With `--json`, `send` prints NDJSON, one event per line, in this order:

| Event | Fields |
| --- | --- |
| `accepted` | `thread`, `request` (the send ID), `new`, `backend` (`host`, `in_process`, `scratch`), `at` (socket or store) |
| `partial` | `thread`, `text` (the reply so far), `delta` (what was added, or null when the preview was rewritten) |
| `route` | `thread`, `tier`, `route`, `bank`, `served_answer` (a knowledge entry `id@version`, when the reply is one), `judgment` (the router's typed judgment as it arrived), `computer` (the judgment placed it on a computer), `followups`, `cards` |
| `offer` | `thread`, `offer` (the typed offer, such as `{"offer": "run_coder"}`), `accept` (the command that accepts it, when there is one) |
| `result` | `thread`, `text`, `model` (the model the worker named), `served_answer` |
| `coder` | `thread`, `accepted`, `message`, `task` (`{host, task, project, at}` when Coder started) |
| `failure` | `thread`, `message`, `stopped`, `partial` |

`threads`, `read`, and `export` print one JSON document each. Exit codes are
the CLI's: `0` success, `1` refused, failed, stopped, or a `--run-coder` that
did not start Coder, `64` invalid usage.

```sh
$ openagents --json chat --scratch "Write a haiku about rain"
{"event":"accepted","thread":"ef9f…","request":"3a2d…","new":true,"backend":"scratch","at":"…/openagents-chat-scratch/ef9f…"}
{"event":"partial","thread":"ef9f…","text":"Here's a draft.\n\n","delta":"Here's a draft.\n\n"}
{"event":"partial","thread":"ef9f…","text":"Here's a draft.\n\nSoft drops kiss the earth,  \nPuddles","delta":"Soft drops kiss the earth,  \nPuddles"}
{"event":"route","thread":"ef9f…","tier":"opener","route":"general","bank":"chat-answers-v1@a04859020455","served_answer":null,"judgment":{…},"computer":false,"followups":[],"cards":[]}
{"event":"result","thread":"ef9f…","text":"Here's a draft.\n\nSoft drops kiss the earth, …","model":"google/gemini-3.8-flash","served_answer":null}
```

`openagents mcp serve` exposes the group as the `chat` tool: its `args` are
the words after `openagents chat`, and its result carries the events as
`{"events": [...]}`.

## Threads as ATIF

`openagents chat export --thread ID` renders the thread through
`openagents_chat::thread::trajectory`, shared code the phone and the desktop
can call too:

- `schema_version` is `ATIF-v1.8`; `session_id` and `trajectory_id` are the
  thread ID; `agent.name` is `openagents-chat`.
- One step per turn: the person's messages are `user` steps, OpenAgents'
  replies `agent` steps with the model the worker named, stamped with when
  this device saved them. Turns saved before times were kept take the time
  of the turn before them.
- The router's judgment of a reply is a decision call on its step
  (`chat_router`, `openagents.decision-call.v1`, the judgment as its
  answers and the route as its `route`).
- What the router said beside the text is the step's
  `extra["openagents.chat.router"]`; a knowledge answer names its entry as
  `extra.served_answer`.
- A Coder task the thread started is linked from the reply before it: an
  observation result whose `subagent_trajectory_ref` names the task and its
  first attempt's trajectory file (`<task>.1.atif.jsonl` on that host).

`cargo run -p atif --example read -- thread.json` reads an export back.
The document also validates against Harbor's `Trajectory` model
(`bench/terminal-bench/tbench/atif.py`). To carry it as
[NIP-ATIF](../../nips/openagents/NIP-ATIF.md), treat it as the document
form.

## Tests

- `crates/openagents-cli/tests/chat.rs` runs the command against a local
  NIP-42 relay with a scripted chat worker: a knowledge answer with an old
  citation stripped, streamed partials, a follow-up carrying context, the
  NDJSON events, `threads`, `read`, `export` read back by `crates/atif`, an
  offer that cannot be accepted without a host, text mode, and exit codes.
- `live_chat_answers_from_product_knowledge` in the same file is the live
  check against the public worker, ignored by default:
  `cargo test -p openagents-cli --test chat -- --ignored --nocapture`.
- `crates/openagents-chat/src/thread.rs` tests the ATIF mapping and the
  paged reader.

## Not supported yet

- Rename, pin, archive, restore, and retry have service commands but no
  `chat` subcommands.
- A thread in the in-process or scratch store cannot run Coder, and is not
  moved into the host when one starts later.
- Eval offers (`start_eval`, `publish_eval`) and screen offers are printed;
  accepting them needs the app.
- Suggestion ranking for a new chat (a rank job) is a phone feature and is
  not sent from the terminal.
