# The OpenCode route

Coder can hand a task's turn to [OpenCode](https://opencode.ai), as it hands
turns to Claude Code, Codex, and Devin. The supported coding agents for
delegation are Claude Code, Codex, Devin, OpenCode, and Grok Build. OpenCode runs on the
host with its own logins, in two ways:

- **A repository turn on an `opencode` route**: `opencode acp`, OpenCode's
  Agent Client Protocol (ACP) server, takes the whole turn of an
  auto-started task, as `devin acp` does on a [Devin route](devin.md).
- **Coder One's delegate executor**: `coder-one --delegate always
  --delegate-agent opencode` hands the explored task to `opencode run
  --format json` with a briefing, beside Claude Code and Codex.

The phone shows only Coder chats
([#9920](https://github.com/OpenAgentsInc/openagents/issues/9920)): OpenCode
sessions the owner starts in OpenCode itself do not list, and the host keeps
no mirror of `opencode.db`. The OpenCode session a Coder task delegates to
is kept beside the task and reads inside that task's chat (see
[The chat list](#the-chat-list)).
[Issue #9915](https://github.com/OpenAgentsInc/openagents/issues/9915)
delivers the route.

The pieces:

| Piece | Where |
| --- | --- |
| OpenCode's ACP specifics: binary, model, permissions, logins, refusals | `acp_client::opencode` in [`crates/acp-client`](../../../crates/acp-client/src/opencode.rs) |
| The repository turn on an OpenCode route | [`microcoder::repository`](../../../crates/microcoder/src/repository/opencode.rs) |
| The `opencode:PROVIDER/MODEL` route, its connection probe, and the capacity book | `coder::task::autostart`, `microcoder_loop::capacity::Provider::OpenCode` |
| The delegate executor, `opencode run` | `coder_delegate::adapter` and `coder_delegate::stream` (`Agent::OpenCode`, `Format::OpenCode`) |
| The steering rows | `coder_delegate::steering::OPENCODE_ACP` (a route), `OPENCODE_RUN` (the delegate) |
| The delegate session's copy beside its task | `coder_history::opencode::delegate`, `coder_history::delegate` |

## Turn it on

Install OpenCode and sign in to a provider there once, as you would to use it
yourself (`opencode auth login`), or configure one in
`~/.config/opencode/opencode.json`. Then admit an OpenCode route in the host's
auto-start policy:

```sh
coder host autostart on --workspace openagents --full-access \
  --route opencode:anthropic/claude-sonnet-5 --route codex:gpt-6-luna
```

An OpenCode route's model is OpenCode's own `PROVIDER/MODEL`
(`opencode models` lists them): the provider is everything before the first
`/`, and the model may hold more `/`s, as OpenRouter's IDs do
(`opencode:openrouter/qwen/qwen3-coder`). `coder host autostart on` refuses a
model without a provider. The route takes no effort.

`coder host autostart show` reports whether OpenCode is connected: an
`opencode` binary (`OPENCODE_BIN`, else `opencode` on `PATH`, else
`~/.opencode/bin/opencode`, where OpenCode's installer puts it, else
`~/.local/bin/opencode`). The provider login is OpenCode's to find.

## What a turn does

A repository run whose route is OpenCode does not run the Microcoder step
loop. The task owner admits the grant exactly as for any other route (the same
workspace, lease, journal, ATIF transcript, and retained artifacts), then:

1. Starts `opencode acp` in the task's workspace, as the leader of its own
   process group, with the owner's login-shell environment under full access
   or the host process's environment otherwise, less every variable named
   `*_API_KEY`, `*_TOKEN`, or `*_SECRET`. The environment adds OpenCode's
   inline configuration (`OPENCODE_CONFIG_CONTENT`: the route's model, the
   permissions below, no sharing, no self-update) and the engine's own
   database (`OPENCODE_DB=openagents-coder-engine.db`, which OpenCode
   resolves in its data directory beside the owner's `opencode.db`).
2. Sends `initialize`, and opens a session with `session/new`, or, on a later
   turn of the same task, reattaches the OpenCode session the earlier turn
   used with `session/load` (see [Follow-up turns](#follow-up-turns)). The
   session's `_meta` carries the engine mark, as a Devin session's does.
3. Checks the model the session reports against the route's; a different one
   refuses the turn before any prompt.
4. Sends the turn's message with `session/prompt` and records what OpenCode
   streams, as it arrives: each reply segment as an Agent step, reasoning as a
   thought step, each completed tool call as a call step with its title,
   input, and bounded output (16 KiB), and every permission request with the
   answer the host gave.
5. When the prompt ends, stops the process group and records the ending, the
   tokens, the cost, and the process group's cleanup.

Starting the session and the prompt are effect intents the task owner retains
before dispatch (`opencode_session`, `opencode_prompt`), with their
observations after. The endings are the Devin route's: `model_finished`,
`cancelled_or_host_refusal`, `no_capacity`, `engine_stopped_after_refusal`,
`engine_cancelled`, or `engine_incomplete`.

## Access

OpenCode runs its own tools; the task owner's command boundary does not wrap
them. The grant's `access` chooses OpenCode's permissions instead:

| Grant access | OpenCode `permission` | Permission requests |
| --- | --- | --- |
| Full (`--full-access`) | `allow`: every tool runs without asking | Answered with OpenCode's allow option |
| Boundary (default) | Read, search, list, LSP, todo, and edit run; every other tool asks; `external_directory` and `question` are denied | Answered with OpenCode's `reject` option, so OpenCode runs no command and fetches nothing |

Under the boundary, OpenCode can read files and edit the workspace but runs no
shell command, web fetch, or subagent. OpenCode's own process reaches the
route's provider over the network with its login and writes its engine
database under its data directory; the boundary's network and write limits do
not apply to the OpenCode process itself. [The invariant
ledger](../../../INVARIANTS.md) records this.

## Follow-up turns

The turn's transcript records the OpenCode session it ran in (a System step
with an `opencode_session` extension). The next turn of the task finds it in
the earlier turns' retained traces and reattaches it with `session/load` from
the engine's database, so OpenCode keeps its own context; the prompt is then
only the new message. When OpenCode refuses the load, the turn opens a new
session and sends the conversation the host carries.

## Steering

`coder_delegate::steering::OPENCODE_ACP`: `turn_boundary`, with cancel and
continue as its emulation and `next_turn_start` as its acknowledgment, as for
Devin. The emulated steer sends `session/cancel`, waits up to 10 seconds, and
stops the process group; the next turn reattaches the session and prompts it
with the message.

The delegate executor's row is `OPENCODE_RUN`: `opencode run` reads its whole
message before the turn, so a steer there is refused, and its emulation stops
the process group and continues with `opencode run --session`.

## Capacity and failover

OpenCode is `opencode` in the capacity book, `capacity.json`; one refusal
holds every OpenCode route. OpenCode's ACP server reports a failed prompt with
only its error's name (`data.errorName`, such as `APIError`) and message. When
the name is `APIError` and OpenCode did no work yet, the host reads the failed
assistant message from the engine's database, read-only
(`coder_history::opencode::last_error`), and records a rate limit when its
HTTP status is 429, with the provider's `retry-after-ms` or `retry-after` as
the reset (else the 30-minute hold). The run then fails over to the next
admitted stage, exactly as a Devin route does. Any other refusal, such as a
403 for a model the account can't use, ends the turn with its error.

Usage probes do not apply: OpenCode reaches many providers and has no usage
endpoint Coder reads.

## Usage and cost

The summary records the `session/prompt` reply's tokens and OpenCode's own
cost, the newest `usage_update` `cost` in US dollars (OpenCode's list-price
figure for the provider and model). The grant's capabilities say
`provider-reported-list-price`.

## The chat list

Coder's own OpenCode sessions are saved in the engine's database,
`openagents-coder-engine.db`, never in the owner's `opencode.db`. The
delegate executor's `opencode run` uses the same database.

The task's own Coder transcript is the chat, and OpenCode's streamed reply,
reasoning, and tool calls are appended to it as they arrive. When the turn
ends, the engine also copies the whole session from the engine's database
into the task directory, beside the transcript, as
`<task>.delegate.opencode.<session>.jsonl` (`coder_history::opencode::delegate`),
and notes the copy on the transcript under `delegate_transcript`. The copy
only grows while the session only grows, so a later turn that reattaches the
session appends to it. The host's chat list names it as the task's subagent;
[`crates/coder-history/README.md`](../../../crates/coder-history/README.md#delegate-sessions)
has the shape a device reads.

## Tests and the live smoke

`acp-client`'s tests replay a real OpenCode 1.18.26 turn and a provider's 403
refusal, recorded on the owner's Mac
(`crates/acp-client/fixtures/opencode-1.18.26-*.jsonl`), through a stand-in
agent. `microcoder`'s `repository::opencode` tests run the whole repository
turn against it: full access, the boundary's permissions and refused request,
a follow-up that reattaches the session, a 429 read from the engine's database
and recorded as `no_capacity`, a 403 that ends the turn, and a model other
than the admitted one refused before any prompt. `coder-delegate`'s tests
replay OpenCode's recorded `opencode run --format json` stream through a
stand-in CLI.

```sh
cargo test -p acp-client
cargo test -p microcoder --lib repository::opencode
cargo test -p coder-delegate
```

A live repository turn through the installed OpenCode is an ignored test; it
passed on 2026-09-28 with OpenCode 1.18.26 on `google/gemini-3.6-flash`:

```sh
OPENCODE_LIVE_MODEL=google/gemini-3.6-flash \
  cargo test -p microcoder --lib a_live_opencode_turn -- --ignored
```

The end-to-end smoke from the phone is an owner step: turn on an
`opencode:PROVIDER/MODEL` route, create a task from the phone, watch it
answer, and archive it.
