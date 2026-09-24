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

## Next steps

1. The `crates/microluna` skeleton: the transport trait, the Codex-login
   transport, a fake transport, the five tools under the boundary and the
   supervisor, sessions with the stable prefix first, and ATIF steps with
   usage and cost.
2. Refresh the login the way Codex does, with a file lock and a re-read
   before writing, only once the refusal above shows up in practice.
3. A `microluna` executor profile in `crates/coder-one` beside
   `claude-code` and `codex`.
4. The matched comparison against Luna-in-Codex on the pivot's TB4 subset.
