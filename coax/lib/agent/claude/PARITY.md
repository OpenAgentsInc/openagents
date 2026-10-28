# Claude Agent SDK parity (Coax)

This library ports the Rust crate `claude_agent_sdk` (removed in
`0b401561a0`; read it with `git show '0b401561a0^:crates/claude_agent_sdk/PARITY.md'`)
to Coax. Like the crate, it ports the stream-json wire protocol of the
`claude` CLI, not the npm package. It tracks
`@anthropic-ai/claude-agent-sdk` **0.3.296** (`CL_UPSTREAM_SDK_VERSION` in
`claude.coax`) and is checked against Claude Code **2.1.312**.

| File | What |
| --- | --- |
| `fields.coax` | Field access and spec validation over `coax/lib/nostr/json.coax` |
| `messages.coax` | Frame classification and the typed stream messages |
| `control.coax` | Control frames the SDK writes, and readers for what the CLI sends |
| `options.coax` | `QueryOptions`: CLI flags and environment |
| `query.coax` | The query as a state machine over lines |
| `transport.coax` | The child process, through `coax/platform/process-native.c` |
| `tests.coax` | The crate's `protocol/tests.rs` fixtures |
| `smoke.coax`, `smoke.sh` | The live smoke against the installed CLI |

## How it differs from the crate, by design

Coax has no callbacks, futures, heap or dynamic arrays, so:

- **Pull, not push.** `cl_next` returns one event per stdout line. Where the
  crate calls a host closure (permission handler, hook, SDK MCP tool), the
  query returns an event and the host answers it before the next
  `cl_next`; an unanswered request gets the answer the crate gives when no
  handler is set.
- **Borrowed values.** A message's strings borrow the line buffer, or an
  arena when they hold escapes, and stay valid until the next `cl_next`.
  Nested values the crate keeps as `serde_json::Value` (assistant
  `message`, `modelUsage`, tool inputs) are raw JSON slices; `cl_at`,
  `cl_str`, `cl_int`, `cl_num`, `cl_bool` and `cl_raw` read into them.
- **Caller storage.** The host gives the query its line, arena and output
  buffers. A line over the line buffer (default `CL_LINE_MAX`, 1 MiB) or
  over 64 MiB in the shim is reported as an `invalid` event and skipped,
  where the crate has no limit. JSON nesting deeper than 16
  (`JSON_DEPTH`) makes a line invalid.
- **One process per query**, killed as a group on `cl_kill`, `cl_close`, or
  when the host exits on Linux (`PR_SET_PDEATHSIG`).

## Stream messages

Every `type` in the 0.3.296 `SDKMessage` union and every `system` and
`result` `subtype` classifies to `ClKind`, `ClSystem` and `ClResultKind`,
with a typed reader for each (`cl_result_of`, `cl_init_of`,
`cl_api_retry_of`, ...). A frame is typed only when it carries the fields
the crate's serde types require, with the right JSON types (the specs in
`messages.coax`, checked by `cl_valid`); otherwise it is `ClKind.unknown`
with its raw line and the stream goes on, as `SdkMessage::Unknown` does.
Text that isn't JSON is an `invalid` event (the crate's
`Error::UnrecognizedMessage`), and the stream goes on.

- `TerminalReason` and `AssistantMessageError` values after 0.3.296 read
  as `unknown`, so the message stays typed.
- `isSynthetic` and `isReplay` are read under their wire names, and the
  snake_case spellings too.
- Control frames (`control_request`, `control_response`,
  `control_cancel_request`) and `keep_alive` are never swallowed as
  unknown messages. A `control_request` whose subtype the crate does not
  model is `unsupported_control`; one whose modelled subtype lacks its
  required fields is an unknown message, as in the crate.
- Validation follows serde on presence, null and JSON type, `u64` as a
  non-negative integer, and the `UserMessagePriority` and
  `PermissionMode` value sets. It does not check integer widths (`u32`)
  or reject duplicate keys; the first occurrence of a key wins.

## Transport

`coax/platform/process-native.c` holds the effects, reached through
`#foreign` with integers only (Coax cannot pass pointers or views to a
foreign procedure, so strings cross one byte per call):

- The CLI is spawned with `fork`/`execve` in its own process group, with
  stdin, stdout and stderr on pipes. `claude` is looked up on `PATH`, then
  in `~/.local/bin`, `~/.claude/local`, `/usr/local/bin` and
  `/opt/homebrew/bin`.
- Flags are `--output-format stream-json --verbose --input-format
  stream-json` first, then the options in the crate's order.
- The child gets `BROWSER=true` and `NO_OPEN_BROWSER=1`, so Claude Code
  never opens a browser or a login page; the options' environment entries
  are set on top of the inherited environment and `env_remove` names are
  removed, so an `ANTHROPIC_API_KEY` lives in the child's environment only.
- stderr is drained while stdout is read and its last 64 KiB are kept
  (`cl_stderr`); the crate inherits it.
- `cl_kill` sends `SIGKILL` to the group and reaps the CLI; `cl_close` (the
  crate's `Drop`) does the same and frees the slot. The host ignores
  `SIGPIPE` once a child is spawned.
- Reads block, except while a control request the SDK sent waits for its
  answer: then a read times out after `control_timeout_ms` (60 s, the
  crate's `DEFAULT_CONTROL_TIMEOUT`) and the query fails with "control
  request timed out".

## Verification

- `coax/bin/coax test --library coax/lib/agent/claude/tests.coax`: every
  fixture in the crate's `protocol/tests.rs` that concerns messages and
  inbound control frames, plus drift cases (a known type missing a
  required field, a priority outside its set, a modelled control subtype
  without its fields) and escaped strings.
- `coax/lib/agent/claude/smoke.sh`: a fake CLI that forks a grandchild into
  its group is killed, and the whole group is gone; then one Haiku turn
  through the installed CLI on the owner's login with no API key, no tools,
  no settings files and no session saved. On 2026-10-28 with Claude Code
  2.1.312 it completed the handshake (13 models, 55 commands,
  `claude_code_version` 2.1.312) and a `success` result (`completed`,
  "pong", $0.0015) with no unknown messages and no invalid lines.

## Deferred

- Everything the crate lists as deferred in its own PARITY.md.
- Typed readers for nested arrays (slash commands, plugins, MCP server
  statuses, persisted files) stay raw JSON with `cl_count` and `cl_item`;
  `cl_mcp_status` and `cl_background_task` read the two the SDK needs.
- `ExecutableConfig` with a `.js` path run through bun or node.
- Windows.
