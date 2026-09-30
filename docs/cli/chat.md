# `openagents chat`: OpenAgents from the terminal

`openagents chat` sends a message to OpenAgents, the chat router the phone
and the desktop talk to, and prints the reply as it streams. People, scripts,
and agents get the same answers, the same knowledge answers, and the same
Coder handoff. When the router judges a message is coding work, Coder runs
on this computer at once and every one of its events streams here
([below](#coder-on-this-computer)). Tracking:
[#10031](https://github.com/OpenAgentsInc/openagents/issues/10031),
[#10032](https://github.com/OpenAgentsInc/openagents/issues/10032).

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
openagents chat send MESSAGE [--thread ID] [--no-run] [--timeout SECONDS]
openagents chat follow --thread ID
openagents chat stop --thread ID
openagents chat answer --thread ID TEXT
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
- `follow` replays the thread's Coder task from its first event and keeps
  streaming until it ends or asks. Ctrl-C stops following, not the task.
- `stop` stops the thread's running Coder task.
- `answer` answers the question (or approval) Coder asked, and follows the
  turn the answer starts.
- `run-coder` runs Coder for the thread's last offer, as `send` does
  without `--no-run` (useful after `--no-run`).

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
   A phone paired with this computer lists these threads beside its own,
   labelled with the computer, and can continue one: its follow-up and the
   reply show here and in the app (NIP-HOST `thread.*`,
   [#10035](https://github.com/OpenAgentsInc/openagents/issues/10035)).
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

## Coder on this computer

The router may judge that a message is work for a computer: a Coder offer
(`run_coder`) or the computer lane. The command then runs Coder right here,
with no host to attach, nothing to pair, no project to register, and no
separate accept step. `--no-run` keeps the old behavior: the offer is
printed, with `openagents chat run-coder --thread ID` to accept it later.
Nothing matches keywords; only the router's judgment decides.

Everything below is the default. The local capability settings
([settings.md](settings.md), `openagents settings`) choose the providers and
their order, whether a coding reply runs at once or only offers
(`coder.start: ask_first`), the usage threshold, which folders count as
projects, and what the run's commands may reach.

- **Project.** The Git checkout the command runs in. Coder works in its own
  detached worktree of the checkout's `HEAD`, never in the checkout, so the
  result is in the worktree the `result` event names. Uncommitted changes
  in the checkout are not carried over. Outside a checkout, or in one with
  no commit, nothing runs and the command says so and exits 1. (With a host
  running and no checkout here, the host's own handoff still runs Coder in
  the host's project, as before.)
- **Providers.** Codex, then Claude Code, each only when it is signed in on
  this computer (the same local check the host uses, reading no credential
  and asking no network). A provider with a usage or rate-limit refusal that
  still holds in the task store's capacity book is passed over, and so is
  one a fresh usage reading in the store shows near its limit. The
  `coder_started` event says which one and why, for example "Codex reached
  its usage limit until 2026-10-03 18:07 UTC; using Claude Code." During the
  run, a provider that refuses is recorded and the run switches
  (`provider_switched`).
- **The same run as the host's.** The task is started through the host
  auto-start's own start (`coder::task::autostart::Policy::launch`): the
  `microcoder repository` engine beside `openagents` (or in
  `~/.openagents/bin`; `OPENAGENTS_CODER_CONTROLLER` names another), under
  an execution grant, in the filesystem boundary, with the same failover and
  the same ATIF trajectory per turn. The shared code is
  [`coder::task::local`](../../crates/coder/src/task/local.rs).
- **Where tasks live.** `~/.openagents/tasks`, the store `coder task` and a
  host on this computer use by default, so the host's devices see these
  tasks in their history. `OPENAGENTS_TASKS` names another store; worktrees
  go in `worktrees/` beside it. A `--scratch` thread keeps its tasks in its
  own scratch directory.
- **The thread records it.** The task is bound to the thread
  (`service::Command::BindCoder`, host `local`), so `follow`, `stop`,
  `answer`, and `export` find it, and the phone and the desktop can show it.
  When this computer's host runs, the thread lives there, and the run is the
  same.

Text mode shows a compact live view on stderr (the provider and why, each
step's thinking, commands, their exit and the last lines of output, progress,
provider switches) and the final result on stdout: Coder's reply, the files
changed with lines added and removed, and the worktree. A question is printed
on stdout with the `answer` command on stderr. Ctrl-C stops following and
prints how to follow or stop the task; the task keeps running.

Exit codes after a run: `0` when the turn finished or asked, `1` when it
failed, was stopped, or could not start (not a checkout, no provider signed
in, no capacity).

### Coder events

Under `--json`, every event of the task follows the chat's own events as one
NDJSON line. The stream is `openagents_chat::coder_events`
([source](../../crates/openagents-chat/src/coder_events.rs)): the desktop
([#10033](https://github.com/OpenAgentsInc/openagents/issues/10033),
[how it draws each event](../desktop/local-coder.md#coder-on-this-computer-from-a-chat)) and the
phone ([#10035](https://github.com/OpenAgentsInc/openagents/issues/10035))
consume the same types, mapped from the same trajectories by the same
`Mapper`.

Every line carries `seq` (from 1, without gaps; a replay produces the same
numbers), `task`, `thread`, `event`, and the event's fields. Every event but
the envelope has `turn` (from 1; an answer starts the next turn).

| Event | Fields |
| --- | --- |
| `coder_started` | `project`, `checkout`, `worktree`, `base` (the commit the worktree started from), `provider` (`codex`, `claude`), `model`, `reason` (why this provider, a sentence), `fallbacks` (`provider:model`, in order), `via` (`local`) |
| `step` | `step_id` (the ATIF step in the turn's trajectory), `kind`, `source` (`user`, `agent`, `system`), `text` (at most 2 KiB). `kind` is `message` (the person's request), `thinking`, `command`, `tool_call` (an agent's tool, for Devin and OpenCode routes), `observation` (a command's exit and time), `reply` (Coder's reply as it is written, in pieces), or `note` (such as running without Jev) |
| `output` | `step_id`, `command`, `exit` (null when a signal or the deadline ended it), `timed_out`, `seconds`, `text` (at most 4 KiB), `truncated` |
| `provider_switched` | `step_id`, `from`, `to` (null when no admitted route had capacity), `reason`, `resets_at` (Unix seconds) |
| `progress` | `step`, `max_steps`, `seconds`, `done` (Jev's probability that the task is done, or null) |
| `question` | `text`, `answer` (the command that answers it) |
| `approval` | `text`, `answer` |
| `result` | `summary` (Coder's reply), `files_changed` (`path`, `status`, `added`, `removed`), `insertions`, `deletions`, `worktree`, `trajectory` (the turn's ATIF file) |
| `failure` | `message`, `ending` (such as `no_capacity`, `loop_incomplete`, `not_started`), `resets_at` |
| `stopped` | `message` |

A turn ends with exactly one of `result`, `question`, `approval`,
`failure`, or `stopped`. `follow` replays a finished task identically: what a
turn changed is computed once, when it ends, and kept in the run's record
(`<store>/local/<task>.json`).

```sh
$ cd ~/code/slugs && openagents --json chat "add a unit test for slugify that covers an empty string"
{"event":"accepted","thread":"1742…","backend":"in_process",…}
{"event":"result","thread":"1742…","text":"We'll dispatch Coder to add a unit test for slugify that covers an empty string.",…}
{"event":"coder","thread":"1742…","accepted":true,"message":"Coder started task 4d0d… in a worktree of slugs.","task":{"host":"local",…}}
{"seq":1,"task":"4d0d…","thread":"1742…","event":"coder_started","turn":1,"project":"slugs",…,"provider":"claude","model":"claude-opus-5-5","reason":"Codex reached its usage limit until 2026-09-30 17:39 UTC; using Claude Code.","fallbacks":["codex:gpt-6-luna"],"via":"local"}
{"seq":2,…,"event":"step","turn":1,"step_id":1,"kind":"message","source":"user","text":"add a unit test for slugify that covers an empty string…"}
{"seq":3,…,"event":"progress","turn":1,"step":1,"max_steps":24,"seconds":6.65,"done":0.04}
{"seq":5,…,"event":"step","turn":1,"step_id":12,"kind":"command","source":"agent","text":"git status --short | head; ls -la; …"}
{"seq":9,…,"event":"output","turn":1,"step_id":17,"command":"git status --short | head; …","exit":0,"timed_out":false,"seconds":0.1,"text":"…","truncated":false}
{"seq":36,…,"event":"result","turn":1,"summary":"I added `test_slugs.py` …","files_changed":[{"path":"test_slugs.py","status":"added","added":13,"removed":0}],"insertions":13,"deletions":0,"worktree":"…/worktrees/slugs-4d0d730cb9be","trajectory":"…/tasks/4d0d….1.atif.jsonl"}
```

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
| `coder` | `thread`, `accepted`, `message`, `task` (`{host, task, project, worktree}` when Coder started) |
| `stop` | `thread`, `task`, `requested`, `message` (from `chat stop`) |
| `failure` | `thread`, `message`, `stopped`, `partial` |

The Coder events ([above](#coder-events)) follow `result` when Coder runs.
`threads`, `read`, and `export` print one JSON document each. Exit codes are
the CLI's: `0` success, `1` refused, failed, stopped, or a coding request
whose Coder run did not finish, `64` invalid usage.

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
- When this computer's task store holds that task, each of its turns'
  trajectories travels inside the export, in `subagent_trajectories`, and
  the reference names the first by its `trajectory_id`
  (`openagents_chat::thread::trajectory_with`). The desktop and the phone
  can then render every step the task took from the thread alone.

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
- `live_chat_runs_coder_on_this_computer`, also ignored, runs a coding
  request in a scratch Python checkout with this computer's own Codex or
  Claude Code login; build the engine first (`cargo build -p microcoder`).
- `crates/microcoder/src/repository/tests.rs` (`local_run`) runs the shared
  local start with scripted provider output (a Codex refusal, Claude Code's
  steps, a question, an answer, an approval, no capacity, a stop) and checks
  every event type, the reason in `coder_started`, the worktree, and an
  identical replay. `crates/openagents-chat/src/coder_events.rs` tests the
  step mapping; `crates/coder/src/task/local.rs` tests checkout detection,
  provider choice with its reason, each local capability setting's
  effect, and that the defaults change nothing.
- `settings_change_the_local_run_and_the_defaults_change_nothing` in
  `crates/openagents-cli/tests/chat.rs` runs `openagents settings` and the
  local run each setting changes that a terminal can observe.
- `crates/openagents-chat/src/thread.rs` tests the ATIF mapping and the
  paged reader.

## Not supported yet

- Rename, pin, archive, restore, and retry have service commands but no
  `chat` subcommands.
- A thread in the in-process or scratch store is not moved into the host
  when one starts later.
- Local runs use the filesystem boundary, as the desktop's auto-start does.
  On macOS the boundary cannot load Xcode's `xcrun`, so `/usr/bin/python3`
  (the Xcode shim) fails inside it; Coder finds another interpreter, such as
  `/Library/Developer/CommandLineTools/usr/bin/python3`, or says it could
  not run the tests.
- Eval offers (`start_eval`, `publish_eval`) and screen offers are printed;
  accepting them needs the app.
- Suggestion ranking for a new chat (a rank job) is a phone feature and is
  not sent from the terminal.
