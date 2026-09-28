# The Devin route

Coder can hand a task's turn to the local Devin CLI, as it hands turns to
Claude Code and Codex. The supported coding agents for delegation are Claude
Code, Codex, Devin, and OpenCode. Devin runs on the host as `devin acp`, the
CLI's Agent Client Protocol (ACP) server, with the CLI's own login. Coder does
not use Devin's cloud API, and it needs no Devin API key.
[Issue #9916](https://github.com/OpenAgentsInc/openagents/issues/9916)
delivers it.

The pieces:

| Piece | Where |
| --- | --- |
| The ACP client, the agent process, and Devin's specifics | [`crates/acp-client`](../../../crates/acp-client/src/lib.rs), `acp_client::devin` |
| The repository turn on a Devin route | [`microcoder::repository`](../../../crates/microcoder/src/repository/devin.rs) |
| The `devin:MODEL` route, its connection probe, and the capacity book | `coder::task::autostart`, `microcoder_loop::capacity::Provider::Devin` |
| The steering row | `coder_delegate::steering::DEVIN_ACP` |
| The chat-list mirror of the owner's Devin sessions | [`coder_history::devin`](../../../crates/coder-history/src/devin.rs) |

## Turn it on

Sign in to the Devin CLI on the host once, as you would to use it yourself
(`devin auth login`). Then admit a Devin route in the host's auto-start
policy:

```sh
coder host autostart on --workspace openagents --full-access \
  --route devin:default --route codex:gpt-6-luna --probe-usage
```

`devin:default` keeps the model the Devin CLI chooses. `devin:MODEL` names an
exact model, such as `devin:swe-2-high` or `devin:claude-opus-5-5-medium`; the
host starts `devin acp --model MODEL` and refuses the turn if the session
reports a different model. The model names are the values Devin lists in its
session's `model` option (`devin models` lists them for your account).

`coder host autostart show` reports whether Devin is connected: a `devin`
binary (`DEVIN_BIN`, else `devin` on `PATH`, else `~/.local/bin/devin`) and
Devin's stored CLI login, `~/.local/share/devin/credentials.toml` (or under
`XDG_DATA_HOME`). The host checks that the login file exists and is not
empty. It never reads it.

## What a turn does

A repository run whose route is Devin does not run the Microcoder step loop.
The task owner admits the grant exactly as for any other route (the same
workspace, lease, journal, ATIF transcript, and retained artifacts), then:

1. Starts `devin acp` in the task's workspace, as the leader of its own
   process group, with the owner's login-shell environment under full access
   or the host process's environment otherwise, less every variable named
   `*_API_KEY`, `*_TOKEN`, or `*_SECRET`.
2. Sends `initialize`. It never sends `authenticate`: Devin advertises only a
   browser login, and the stored CLI login is what it uses. A Devin CLI that
   is not signed in refuses `session/new`, and the turn ends with that error.
3. Opens a session with `session/new`, or, on a later turn of the same task,
   reattaches the Devin session the earlier turn used with `session/load`
   (see [Follow-up turns](#follow-up-turns)). The session's `_meta` carries
   the engine mark, `{"openagents.com/engine": "openagents-coder-engine"}`,
   which Devin keeps with the session.
4. Sets the permission mode (see [Access](#access)) and checks the model.
5. Sends the turn's message with `session/prompt` and records what Devin
   streams, as it arrives, in the task's transcript: each reply segment as an
   Agent step, reasoning as a thought step, each completed tool call as a
   call step with its title, input, and bounded output (16 KiB), Devin's plan,
   and every permission request with the answer the host gave.
6. When the prompt ends, stops the process group and records the ending,
   the tokens, and the process group's cleanup.

Starting the session and the prompt are effect intents the task owner retains
before dispatch (`devin_session`, `devin_prompt`), with their observations
after. The task's result ending is `model_finished` when Devin ends the turn
(`end_turn`), `cancelled_or_host_refusal` when the task was cancelled or
reached its deadline, `no_capacity` when Devin refused for a limit and no
later route had capacity, and `engine_incomplete` otherwise; an error names
itself in a System step.

## Access

Devin runs its own tools; the task owner's command boundary does not wrap
them. The grant's `access` chooses Devin's own controls instead:

| Grant access | Devin mode | Permission requests | Other |
| --- | --- | --- | --- |
| Full (`--full-access`) | `bypass`: every tool runs without asking | Answered with Devin's `allow` option | The owner's login-shell environment |
| Boundary (default) | `accept-edits`: workspace edits run, anything else asks | Answered with Devin's `reject` option, so Devin runs no command it had to ask for | `devin --sandbox acp`, Devin's own sandbox for its exec tool |

Under the boundary, Devin can read files and edit the workspace but runs no
command that needs permission. Devin's own process reaches Devin's service
over the network with its login, and writes its session store under
`~/.local/share/devin`; the boundary's network and write limits do not apply
to the Devin process itself. [The invariant ledger](../../../INVARIANTS.md)
records this.

## Follow-up turns

A device's follow-up (`send`, a promoted `queue`, an emulated `steer`, or an
`answer`) starts the task's next turn. The turn's transcript records the Devin
session it ran in (a System step with a `devin_session` extension). The next
turn finds it in the earlier turns' retained traces and reattaches it with
`session/load`, so Devin keeps its own context; the prompt is then only the
new message. When Devin refuses the load (the session is gone), the turn opens
a new session and sends the conversation the host carries, as other engines
get it.

## Steering

`coder_delegate::steering::DEVIN_ACP`: `turn_boundary`, with cancel and
continue as its emulation and `next_turn_start` as its acknowledgment. ACP v1
takes one `session/prompt` at a time, and Devin CLI 3000.11.3 advertises no
steering extension, so a message for a running turn is not delivered into it.
The emulated steer cancels the run: the host sends `session/cancel`, waits up
to 10 seconds for Devin to end the turn as `cancelled`, and stops the process
group; the next turn reattaches the session and prompts it with the message.

## Capacity and failover

Devin is `devin` in the capacity book, `capacity.json`. The Devin CLI reports
no typed usage limit, so the host records a refusal only when Devin refuses
the prompt with a JSON-RPC error whose `data.retryable` is `true` before any
tool call or reply. It holds 30 minutes, since Devin reports no reset. The run
then fails over:

- A Devin route is its own stage. When it refuses for capacity, or the book
  already holds a refusal for Devin, the run moves to the next admitted route.
- Consecutive model routes (`codex`, `claude`) form one stage of the
  Microcoder loop, which fails over among them as before. When that stage
  ends as `no_capacity` and a Devin route follows it, the run moves to Devin.

Usage probes do not apply: Devin has no usage endpoint Coder reads, so a
probe reports it `unsupported`.

## Usage and cost

The transcript and the task's summary record Devin's tokens: the sum of the
root agent's `usage_update`s (`cognition.ai/inputTokens`,
`cognition.ai/outputTokens`), and every numeric dimension of Devin's
`_cognition.ai/turn_stats` notification (`input_tokens`, `output_tokens`,
`cached_input_tokens`, `agent_messages`, and any credit dimensions Devin
reports). Devin bills in its own credits and reports no dollar price over ACP,
so the turn's cost is recorded as unknown.

## The chat list

Devin chats the owner starts in Devin itself appear in the phone's chats as
their own source, labeled Devin. Devin keeps its local sessions in a SQLite
store, `~/.local/share/devin/cli/sessions.db` (or under `XDG_DATA_HOME`),
not in transcript files, so `coder host` mirrors it: when the store exists, a
chat invitation offers the mirror directory `~/.openagents/devin/mirror`, and
the host brings it up to date every five seconds
(`coder_history::devin::mirror`). A pass opens the store read-only and writes
one append-only `<session id>.jsonl` per session plus a `session_index.jsonl`
of titles; a session Devin hides lists as archived. The first pass over a
large store takes a while (about 35 seconds for 800 sessions and 4.6 GB on
the owner's Mac); later passes touch only sessions with new activity.

The mirror follows the conversation Devin shows: the session's newest node
back to its start, across compactions, without Devin's system prompts or the
prompts Devin writes to itself (a compaction's request, a cache keep-alive).
A turn and a compaction only append; a rewound or edited conversation starts
a new file. Subagent chains inside a session are not mirrored.

A Devin session a Coder task starts carries the engine mark in its `_meta`,
which Devin keeps as the session's `metadata.client_meta` and `session/list`
returns. The mirror never writes such a session, so it never lists as a
chat: the task's own Coder transcript is the chat.

## Tests and the live smoke

`acp-client`'s tests replay a real Devin CLI 3000.11.3 turn, recorded on the
owner's Mac and trimmed (`crates/acp-client/fixtures/devin-3000.11.3-turn.jsonl`),
through a stand-in agent (`acp_client::replay`). `microcoder`'s
`repository::devin` tests run the whole repository turn against it: full
access, the boundary's sandbox and refused permission, a follow-up that
reattaches the session, a retryable refusal recorded as `no_capacity`, and a
model other than the admitted one refused before any prompt.

`coder-history`'s `devin` tests mirror rows recorded from Devin CLI
3000.11.3's own store (`crates/coder-history/fixtures/devin/capture.rows.json`):
an owner's session that ran a command, and an engine-marked session that is
left out.

```sh
cargo test -p acp-client
cargo test -p microcoder --lib repository::devin
cargo test -p coder-history --features devin --lib devin
```

The live smoke runs the same repository turn through the installed Devin CLI
with the owner's login, under full access, in a scratch repository and task
store that the test removes. It spends Devin credits, so it is ignored by
default:

```sh
cargo test -p microcoder --lib live_devin -- --ignored
```

On 2026-09-28, on the owner's Mac with Devin CLI 3000.11.3 and `swe-2-high`,
it ended `model_finished` in about 20 seconds, wrote `result.txt`, and its
Devin session carried the engine mark, so the host's mirror left it out.
