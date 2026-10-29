# Claude Agent SDK parity (Coax)

This library ports the Rust crate `claude_agent_sdk` (removed in
`0b401561a0`; read it with `git show '0b401561a0^:crates/claude_agent_sdk/PARITY.md'`)
to Coax. Like the crate, it ports the stream-json wire protocol of the
`claude` CLI, not the npm package. It tracks
`@anthropic-ai/claude-agent-sdk` **0.3.296** (`CL_UPSTREAM_SDK_VERSION` in
`claude.coax`) and is checked against Claude Code **2.1.312**.

| File | What |
| --- | --- |
| `fields.coax` | Field access/spec validation over `json.coax`; approximate cost decoding explicitly opts into `json-f64.coax` |
| `messages.coax` | Frame classification and the typed stream messages |
| `control.coax` | Control frames the SDK writes, and readers for what the CLI sends |
| `options.coax` | `QueryOptions`: CLI flags and environment |
| `mcp.coax` | In-process SDK MCP servers: initialize, ping, tools/list, tools/call |
| `query.coax` | The query as a state machine over lines; the control protocol both ways |
| `transport.coax` | The child process, through `coax/platform/process-native.c` |
| `tests.coax` | The crate's `protocol/tests.rs` fixtures |
| `query_tests.coax` | The crate's `query.rs` and `mcp.rs` control tests, scripted through `cl_feed` |
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

## Control protocol

The SDK's own requests: `initialize` (sent first; a prompt query sends its
prompt once it is answered), `interrupt`, `set_model` (`""` sends
`model: null`) and `set_permission_mode`, plus `cl_request` for any other
subtype with its members as JSON. Each returns its `sdk-N`; the answer is a
`control_result` event whose `response.sdk_number` is N.

`initialize` carries what the crate's `initialize_request` does for these
options:

- `hooks`: one matcher per `cl_add_hook`, grouped by event, events sorted by
  name and registration order within one, ids `hook_0`, `hook_1`, ... in
  that order (`HookRegistry::register`); `matcher` and `timeout` only when
  set.
- `sdkMcpServers` (names), `sdkMcpServerConfigs` (`{"timeout": ms}` for
  servers with one) and, with `sdk_mcp_manifests`, `sdkMcpServerManifests`
  (each server's `initializeResult` and `toolsListResult`). SDK servers never
  go into `--mcp-config`, and `cl_validate` refuses one whose name is also in
  `mcp_servers`.

The CLI's requests, each answered before the next line is read:

| Request | Event | Host answer | Unanswered, or no handler |
| --- | --- | --- | --- |
| `can_use_tool` | `permission`, when `options.permission_handler` (which sends `--permission-prompt-tool=stdio`) | `cl_allow(q, input, updated_permissions)`, `cl_deny(q, message, interrupt)` | "canUseTool callback is not provided" |
| `hook_callback` | `hook`, `ev.hook` is the index in `options.hooks`, `ev.arguments` the hook input | `cl_hook_reply` (a `ClHookOutput`), `cl_hook_answer` (raw `HookJSONOutput`, `{"async":true}` included), `cl_hook_fail` | an unknown id: "no hook callback found for ID: X" |
| `mcp_message` `tools/call` to a known tool | `tool_call`, `ev.server`, `ev.tool`, `ev.arguments` (`{}` when absent or null) | `cl_tool_text(q, text, is_error)`, `cl_tool_answer(q, content, is_error, structured)` | an `isError` result |
| other `mcp_message` | none: `initialize` (a supported `protocolVersion` echoed, else 2025-11-25), `ping`, `tools/list` (with `annotations` and `_meta` `anthropic/alwaysLoad` and `anthropic/searchHint`); an unknown tool -32602 "Tool X not found", an unknown method -32601; notifications, responses and null ids get `{"jsonrpc":"2.0","result":{},"id":0}` | | an unknown server: "SDK MCP server not found: X" |
| `elicitation`, `request_user_dialog`, `oauth_token_refresh`, `host_auth_token_refresh` | none | | the crate's no-handler answers: decline, silence, errors |

An allow whose `updatedPermissions` holds more than 4096 entries (updates
plus their rules or directories) goes out as a deny saying so
(`PermissionResult::within_limits`). The model and `allowed_tools` name an
SDK tool `mcp__<server>__<tool>` (`cl_put_tool_name`).

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
- `coax/bin/coax test --library coax/lib/agent/claude/query_tests.coax`:
  the crate's control tests (initialize hooks and SDK server fields,
  manifests, mcp_message initialize/notification/list/call/unknown server,
  JSON-RPC errors, hook ids and replies, unknown hook ids, permission with
  and without a handler, the oversized permission answer, set_model,
  set_permission_mode, interrupt, a failed initialize, option validation).
- `coax/lib/agent/claude/smoke.sh`: a fake CLI that forks a grandchild into
  its group is killed, and the whole group is gone; then one Haiku turn
  through the installed CLI on the owner's login with no API key, no tools,
  no settings files and no session saved. On 2026-10-28 with Claude Code
  2.1.312 it completed the handshake (13 models, 55 commands,
  `claude_code_version` 2.1.312) and a `success` result (`completed`,
  "pong", $0.0015) with no unknown messages and no invalid lines. Then two
  more Haiku queries: one called the in-process tool `mcp__calc__add` with
  `{"a":1234,"b":5678}` after a host-answered `PreToolUse` hook, and
  answered "6912"; in the other the host's permission handler denied
  `Bash` (`touch smoke-denied.txt`), the result counted one permission
  denial, and the file was not made.

- `cl_poll(q, p, timeout_ms)` is `cl_next` for a host that must not block
  (the desktop app's frame loop): it writes what is queued, then returns a
  line if one arrives within the timeout (0 takes only what is already
  there), or an `idle` event. A control request still times out after
  `control_timeout_ms`, counted across calls. `options.effort`
  (`--effort`) and `options.thinking_display` (`--thinking-display`) map
  the CLI's flags.

## Deferred

- Everything the crate lists as deferred in its own PARITY.md.
- Typed wrappers for the other outbound control subtypes (`set_max_thinking_tokens`,
  `mcp_server_status`, `rewind_files`, settings, tasks, usage, reloads, ...):
  they go through `cl_request` with their members as JSON.
- Host events for `elicitation`, `request_user_dialog` and the token
  refreshes: they get the crate's no-handler answers.
- `pending_permission_requests` carried in a `control_response` is read
  (`ClResponse`) but not dispatched again as permission events.
- A `control_cancel_request` cannot stop a host answer: the host answers
  before the next line is read, so the answer is already written.
- One callback per hook matcher; `decisionClassification` and `toolUseID`
  are not set on permission answers.
- Options not mapped yet: agents, sandbox, output format (`--json-schema`),
  thinking budgets as settings, plugins, and the `initialize` fields title, skills,
  plan-mode instructions, tool aliases, prompt suggestions, agent progress
  summaries, subagent text, dialog kinds and the stop affordance.
- Typed readers for nested arrays (slash commands, plugins, MCP server
  statuses, persisted files) stay raw JSON with `cl_count` and `cl_item`;
  `cl_mcp_status` and `cl_background_task` read the two the SDK needs.
- `ExecutableConfig` with a `.js` path run through bun or node.
- Windows.
