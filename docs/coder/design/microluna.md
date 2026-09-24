# Microluna: how to reach Luna on a logged-in Codex session

Status: decision, 2026-09-24. Issue #9585. This note settles how
`crates/microluna` talks to GPT-6 Luna, and what it takes from OpenAI's
Codex. The direction it serves is [the Luna pivot](luna-pivot.md).

## What Microluna needs

The Luna pivot asks five things of a transport:

- **Many short sessions.** A task is a sequence of sessions, each with a
  context that code and Jev rebuild from scratch.
- **Control of the context.** Microluna decides the instructions, every
  input item, and their order, so the stable prefix comes first and the
  provider can cache it.
- **A small tool set of our own.** Run a command, read a file region,
  apply a patch, write a file, and finish with a typed result, all as
  native function tools, and nothing else.
- **Typed events with exact usage.** Every call, output, and token count
  becomes an ATIF step, with cached tokens counted apart so cost is exact.
- **Fast startup.** A session costs milliseconds to open, not a process.

The operator's constraint: Microluna runs on the operator's **logged-in
Codex session**, the ChatGPT login in `~/.codex/auth.json`, with no
separate API key.

## The reference

Codex is open source under Apache-2.0 and cloned at `~/work/codex`.
The paths below are relative to `~/work/codex/codex-rs/`, at the commit
that ships `codex-cli 0.155.1`. Microluna reads it for design and
reimplements what it keeps. It copies no code.

## Option A: drive `codex app-server` over its protocol

`codex app-server` speaks JSON-RPC over standard input and output. The
v2 protocol is broad:

- **Threads and turns.** `thread/start`, `thread/fork`, `thread/resume`,
  `turn/start`, `turn/steer`, and `turn/interrupt`
  (`app-server-protocol/src/protocol/common.rs:551` to `:1050`).
- **Per-thread instructions and model.** `ThreadStartParams` takes
  `model`, `base_instructions`, `developer_instructions`, `cwd`,
  `sandbox`, `approval_policy`, a free-form `config` override map, and
  `ephemeral` (`app-server-protocol/src/protocol/v2/thread.rs:62`).
  `TurnStartParams` overrides `model`, the sandbox, and the approval
  policy per turn (`app-server-protocol/src/protocol/v2/turn.rs:167`).
  So `gpt-6-luna` is selectable per thread or per turn.
- **Client tools.** `ThreadStartParams.dynamic_tools` declares tools the
  client implements. The server calls them back with the
  `item/tool/call` request and waits for the client's answer
  (`common.rs:1796`, `app-server/src/dynamic_tools.rs`).
- **Approvals.** The server asks the client before a command or a file
  change: `item/commandExecution/requestApproval` and
  `item/fileChange/requestApproval` (`common.rs:1765`, `:1772`).
- **Events and usage.** `item/started`, `item/completed`, deltas, and
  `thread/tokenUsage/updated`, whose `TokenUsageBreakdown` counts input,
  cached input, and output tokens
  (`app-server-protocol/src/protocol/v2/thread.rs:1895`).

**Startup, measured on this machine** with `codex-cli 0.155.1`: the
`initialize` reply arrived 272 ms and 301 ms after spawn in two runs. The
first `thread/start` took 667 ms cold and 36 ms warm. One process can
hold many ephemeral threads, so the process cost is paid once per task.

**What it can't give Microluna:**

- **The context stays Codex's.** Codex assembles the model input itself.
  It injects its own fragments around the client's: environment context,
  `AGENTS.md` instructions, permission and sandbox instructions, plugin
  and skill notes, and more (`core/src/context/`, over 40 fragment
  kinds). Microluna can't order the input items for its cache, and
  can't see the exact request that was billed.
- **Codex's own tools stay in the loop.** Its shell, apply-patch, and
  planning tools are built in. Some have feature switches, such as
  `shell_tool` (`features/src/lib.rs:951`), but switching them off is a
  configuration fight with each release rather than a contract.
- **Compaction and history are the thread's.** Codex compacts on its own
  schedule with its own summary prompt (`core/src/compact.rs:57`,
  `:145`). A context rebuilt from Jev's judgments means a new thread per
  session, where Codex's history machinery adds nothing.
- **A large, fast-moving surface.** The v2 protocol marks many fields
  experimental (`#[experimental(...)]` in `thread.rs`), and a child
  process is one more thing to supervise, version-pin, and restart.

## Option B: call the Responses endpoint directly on the Codex login

This is what Codex's own model client does, and it's small.

**The endpoint.** With ChatGPT login, Codex's base URL is
`https://chatgpt.com/backend-api/codex`
(`model-provider-info/src/lib.rs:77`), and a turn is
`POST {base}/responses` (`codex-api/src/endpoint/responses.rs:139`).

**The request body** (`core/src/client.rs:961` to `:1007`): `model`,
`instructions`, `input` items, `tools`, `tool_choice: "auto"`,
`parallel_tool_calls`, `reasoning`, `store: false`, `stream: true`,
`include: ["reasoning.encrypted_content"]`, and `prompt_cache_key`.

**The headers:**

- `Authorization: Bearer <access_token>` and `ChatGPT-Account-ID:
  <account_id>` (`model-provider/src/bearer_auth_provider.rs:31`).
- `originator`, which is `codex_cli_rs` for Codex itself
  (`login/src/auth/default_client.rs:42`), and a `User-Agent`.
- `session-id` and `thread-id` (`codex-api/src/requests/headers.rs:5`).

**The login** is `~/.codex/auth.json`, mode `0600`: `auth_mode`,
`tokens.id_token`, `tokens.access_token`, `tokens.refresh_token`,
`tokens.account_id`, and `last_refresh`. The access token is a JWT whose
`exp` claim says when it expires.

**Token refresh.** Codex refreshes proactively when the access token is
within a few minutes of `exp`, or when `last_refresh` is more than eight
days old (`login/src/auth/manager.rs:3004`). It posts the refresh token
to `https://auth.openai.com/oauth/token` with client ID
`app_EMoamEEZ73f0CkXaXp7hrann` (`manager.rs:212`, `:1626`, `:1717`) and
writes the new tokens back with a fresh `last_refresh` (`manager.rs:1598`).
The refresh token rotates: reusing a spent one fails with
`refresh_token_reused` (`manager.rs:1686`). Two processes that refresh
the same login independently can therefore log the operator out.

**Measured on this machine:** a direct call with our own instructions,
one native function tool, and `gpt-6-luna` returned HTTP 200 and a
`function_call` item in 1.39 s and 1.46 s end to end. The completed
event's `usage` reports `input_tokens`,
`input_tokens_details.cached_tokens`, `output_tokens`, and
`output_tokens_details.reasoning_tokens`. The backend accepted an honest
client identity, `originator: openagents_microluna`, so Microluna
doesn't present itself as Codex.

**What Microluna reimplements:** reading `auth.json`, the request
builder, the server-sent event reader for five event kinds
(`response.output_item.done`, `response.completed`, `response.failed`,
`response.incomplete`, and `error`), and the tool loop. That is a few
hundred lines.

**How fragile it is:**

- The endpoint is ChatGPT's backend for Codex, not a documented public
  API. OpenAI can change the path, a header, or the required fields.
  Codex's own client is the change log to watch.
- A model slug can disappear from the ChatGPT plan's catalog. The
  failure is a typed HTTP refusal, which Microluna reports as such.
- Refresh is the sharp edge, as above. The first slice never refreshes:
  it reads the login, and when the token is expired or within ten
  minutes of expiring, it refuses with a typed error that asks the
  operator to run any Codex command, which refreshes the login the way
  Codex does. At the time of writing the access token had about 200
  hours left.

## What to keep from Codex, and what to leave

**Keep, reimplemented:**

- **The apply-patch format and its matching.** Luna is trained on it:
  `*** Begin Patch`, `*** Add File:`, `*** Delete File:`,
  `*** Update File:` with an optional `*** Move to:`, `@@` context
  markers, `+`, `-`, and space lines, and `*** End of File`
  (`apply-patch/src/parser.rs:1` to `:22`). Hunks are located with
  decreasing strictness: exact, then ignoring trailing whitespace, then
  ignoring surrounding whitespace (`apply-patch/src/seek_sequence.rs:1`).
  Microluna ships the tool with the same format and the same fallback,
  written here.
- **The request shape.** `store: false` with encrypted reasoning carried
  forward, a `prompt_cache_key` per task so short sessions share a cache,
  and `parallel_tool_calls: false` for a small model.

**Leave out:**

- **Codex's sandbox.** `linux-sandbox` combines bubblewrap, Landlock, and
  seccomp (`linux-sandbox/src/bwrap.rs`, `landlock.rs`). This repository
  already has its boundary: `crates/coder-boundary` wraps every command in
  `bwrap` or `sandbox-exec`, and `crates/supervise` owns the process tree,
  the deadline, and the output caps. Microluna uses those.
- **Compaction.** Microluna doesn't let a context grow long enough to
  compact. Each session starts from a context code and Jev rebuild, which
  is the point of the design.
- **The approval flow, plugins, skills, MCP, memories, and multi-agent
  modes.** The permit a Coder turn already carries decides what runs.
- **The WebSocket transport** (`codex-api/src/endpoint/responses_websocket.rs`).
  It saves resending input on long threads, which short sessions don't
  have.

## Decision

**Option B.** Microluna calls `https://chatgpt.com/backend-api/codex/responses`
directly from Rust with the operator's Codex login, its own instructions,
and its own five native function tools.

Why, against the pivot's needs:

- **Rebuilt contexts:** B sends exactly the input Microluna built, in the
  order it chose. A wraps it in Codex's own fragments.
- **A small tool set:** B declares five tools and the model sees five. A
  adds Codex's built-in tools unless configuration removes them.
- **Typed events and cost:** B reads usage per request, with cached
  tokens, straight from the completed event, and writes ATIF steps
  directly. A reports usage per thread through a translation layer.
- **Startup:** B opens a session with no process at all. A costs about
  300 ms per process plus a thread start.
- **Many short sessions:** both can, but in A each one is a thread that
  carries Codex's history and compaction machinery for nothing.

The cost of B is fragility: a private endpoint, and a login it must not
refresh carelessly. Microluna contains both behind a `Transport` trait, so
a change on OpenAI's side is one implementation to fix, and tests run on
a fake transport. Option A stays the fallback transport if the direct
endpoint ever closes to non-Codex clients.

## The first slice

`crates/microluna` implements the decision. `microluna TASK` runs one
session in a workspace on the Codex login, prints each step to standard
error and a JSON summary to standard output, and writes the ATIF trace to
`~/.openagents/traces/`.

Two sessions on 2026-09-24, `gpt-6-luna`:

| Session | Turns | Input (cached) | Output | Cost | Wall time |
| --- | --- | --- | --- | --- | --- |
| Change a port in a scratch `config.toml`, check it, and name the service | 4 | 2,973 (0) | 163 | $0.00038 | 7.4 s |
| Answer a question about `src/patch.rs`, with the file as evidence | 4 | 16,151 (6,656) | 315 | $0.00117 | 10.8 s |

- The first session read the file, patched one line with `apply_patch`,
  checked it with `grep`, and finished with a typed answer. Its prompts
  were under the provider's minimum cacheable length, so nothing was
  cached.
- In the second, from the third request on, the provider served most of
  the prefix from its cache: 2,816 and then 3,840 of about 4,100 input
  tokens. The boundary denied the model's `cargo test`, which tried to
  write a build directory outside the workspace; the model searched the
  source instead and said so in its answer.

## Microluna in Coder One

Coder One runs Microluna in its own process, as an executor beside
Claude Code and Codex (`crates/coder-one/src/micro.rs`). `coder-one`
depends on `microluna`, never the reverse. A policy selects it with
`executor.agent: "microluna"`. The reference manifest is
[`microluna-v1.json`](../../../crates/coder-one/policies/microluna-v1.json):
`tunable-luna-pack-solo` with Microluna as the executor and no handoff,
so nothing escalates to Opus.

`executor.microluna` sets the mode and the bounds:

- **`single`** runs one session on the briefing, as a CLI would.
- **`requirements`** runs the mini-handoff loop, and is the default:
  1. The requirement map Jev extracted is split into at most
     `max_groups` groups of consecutive requirements.
  2. Each group gets short sessions. Before each one, the host rebuilds
     the input from scratch: the task first, so every session of the task
     shares the cached prefix, then the group's requirements and only the
     evidence the coverage packer says informs them, then the current
     state. The state lists earlier sessions' reports, the workspace's
     changes, and what the checks said last.
  3. After each session, code runs `verify.checks` on the workspace and
     asks for the combined verdict (`checks::verdict`, issue #9584) over
     the session's report. A Jev Choice then picks the next move: `next`,
     `retry`, `stuck`, or `done`.
  4. Code keeps the last word. A check that contradicts the group turns a
     move past it into a retry. A verdict of `fail` keeps the loop from
     ending, by `done` or by moving past the last group. A group gets at
     most `max_attempts` sessions, and the loop stops at `max_sessions`,
     at `spend_usd`, or when the dispatch's time runs out.

In a Terminal-Bench trial the task container is the boundary, so
commands run directly (`Isolation::TaskContainer`). On this machine, as
in a mini-task, each command runs inside a `coder-boundary` boundary that
lets it write only the task's directory.

Each session is a `microluna.session` invocation in the episode log. Its
tool calls and replies are recorded as normalized executor events, the
shape the CLI adapters record, so the Gym reads a Microluna session the
way it reads a Codex one. Each move between sessions is a
`microluna.handoff` Jev decision and a `handoff` step. Microluna's own
steps, with exact usage and cost per request, go to
`artifacts/microluna-<dispatch>-<session>.atif.jsonl`, and the loop's
record, with every session and every move, goes to
`artifacts/microluna-<dispatch>.json`. The delegate call reports the
dispatch's list-price cost, so composition, `usage.json`, and the Gym's
cost views price it like any other dispatch.

## How to watch Microluna

**In Coder Terminal**, with no setup: when `~/.codex/auth.json` has more
than ten minutes left on its access token, `coder` answers every turn
through Microluna. Each session's start and requirement, its commands,
its finish, its cost, and each hand-off show as the turn runs:

```sh
coder doctor                      # door, the Codex login's hours left
coder                             # the terminal
coder -p "what does crates/gym do?"
coder -p --json "what does crates/gym do?"   # the same events as JSON lines
```

`CODER_DELEGATE_AGENT=claude-code` or `codex` picks a CLI instead. A
read-only turn runs Microluna read-only, and a follow-up turn rebuilds
each session's context rather than resuming one. The
[delegate door](../runtime/delegate-door.md) covers the rest.

**In Coder One.** Build the two binaries once, with a Cargo target directory of your own:

```sh
export CARGO_TARGET_DIR=~/.cache/openagents/target-microluna
cargo build -p coder-one -p gym --bin coder-one --bin gym
cargo build -p gym --features tui --bin gym-terminal
bin=$CARGO_TARGET_DIR/debug
```

**Run a mini-task and watch it live.** Each tool call prints as it
happens, then a line per session, per check, and per move:

```sh
$bin/coder-one minitask run log-severity --executor microluna --jev live
```

`--microluna single` runs one session instead of the loop. The run is
recorded under `~/.openagents/coder-one/minitasks/`, and the last line
names its directory.

**See a finished mini-task in the Gym**, with every session's commands and
edits, each check, and each hand-off in order:

```sh
$bin/gym coder minitasks                  # every run, newest first
$bin/gym coder minitasks --run latest     # or --run <run directory name>
```

**Run a Terminal-Bench trial on the Codex-only arm.** From
`bench/terminal-bench`, with the door and Jev keys exported as the
[delegate runbook](../../terminal-bench/coder-one-delegate-runbook.md)
shows, `CODEX_FORCE_AUTH_JSON=1`, and a Coder One artifact built by
`./scripts/build-coder-one-linux.sh`:

```sh
uv run tbench run --profile tb4 --agent coder-one-microluna-v1 \
  --auth-mode auth-json --task uefi-bootkit \
  --job-name tb4--microluna-v1--uefi-bootkit--r1 \
  --agent-kwarg artifact_path="$artifact_path" \
  --agent-kwarg artifact_sha256="$artifact_sha256"
```

**Follow the trial while it runs**, or read it after. The transcript shows
each session as a takeover, its commands, and each hand-off:

```sh
$bin/gym runs show tb4--microluna-v1--uefi-bootkit--r1 --transcript
```

**Replay it head to head** against Fable 5.1's public attempts or, with
`o`, against a local Luna-in-Codex attempt on the same task
([head to head](../../gym/head-to-head.md)):

```sh
$bin/gym-terminal --terminal-bench --head-to-head
```

**Read the raw record** of a run directory `$run` (a mini-task run, or a
trial's `agent/episode`):

```sh
jq -c '(.sessions[] | {number, focus, status, turns, cost_usd}), (.moves[] | {after_session, move, overridden})' \
  "$run"/artifacts/microluna-1.json
jq -c '.step | select(.call) | .call.name' "$run"/artifacts/microluna-1-1.atif.jsonl
```

## Next steps

1. Refresh the login the way Codex does, with a file lock and a re-read
   before writing, only once the refusal above shows up in practice.
2. Make the checks catch what the graders catch. The first mini-task
   comparison lost `log-severity` in both arms to CRLF line endings that
   no check looks for.
3. Measure the loop on more of the pivot's TB4 subset, one change at a
   time, against the #9583 baseline.
