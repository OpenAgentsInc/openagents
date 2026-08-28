# Claude Agent SDK Rust parity

This crate ports the wire protocol of `@anthropic-ai/claude-agent-sdk`.
Parity is checked with `scripts/check-claude-sdk-parity.sh`.

## P1 landed (issue #232)

Silent JSONL skip is gone. The stdout reader:

- maps modelled 0.3.172 `type` / `subtype` values onto typed [`SdkMessage`] variants
- emits [`SdkMessage::Unknown { type_name, raw }`](src/protocol/messages.rs) for a
  valid JSON object whose `type` is not modelled, and keeps the stream going
- emits [`Error::UnrecognizedMessage { type_name, raw }`](src/error.rs) for
  invalid JSON, and keeps the stream going

Control (`control_request`, `control_response`) and `keep_alive` frames are
classified before the unknown-SDK fallback so they are not swallowed.

### 0.3.172 variants added in P1

Top-level `type` values: `prompt_suggestion`, `rate_limit_event`,
`tool_use_summary`, plus `SdkMessage::Unknown`.

`type: "system"` subtypes: `api_retry`, `model_refusal_fallback`,
`local_command_output`, `hook_started`, `hook_progress`, `hook_response`
(fields extended), `plugin_install`, `task_notification`, `task_started`,
`task_updated`, `task_progress`, `thinking_tokens`, `session_state_changed`,
`commands_changed`, `notification`, `files_persisted`, `memory_recall`,
`elicitation_complete`, `permission_denied`, `mirror_error`.

Already present before P1: `assistant`, `user`, `result`, `system` (`init`,
`compact_boundary`, `status`), `stream_event`, `tool_progress`, `auth_status`.

## P2 landed (issue #232)

`Query::new` sends `subtype: "initialize"` before the user prompt, stores
the handshake payload, and exposes it as `Query::initialization_result()`
(TS `Query.initializationResult()`). Control requests wait at most
`DEFAULT_CONTROL_TIMEOUT` (60s, overridable via
`QueryOptions::control_timeout`). A silent CLI returns
`Error::ControlTimeout` instead of hanging. A CLI `subtype: "error"` on
initialize is `Error::InitializationFailed`. The stdout reader no longer
holds the stdin lock across `recv`, so the handshake cannot deadlock.

## P3 first slice landed (issue #232)

Queryable control methods now include `apply_flag_settings`, `set_mcp_servers`,
`stop_task`, `get_context_usage`, `background_tasks`, `cancel_async_message`,
`get_session_cost`, `get_usage`, `get_binary_version`, `file_suggestions`,
`reload_plugins`, `reload_skills`, `reconnect_mcp_server`, `toggle_mcp_server`,
and `rename_session`. `supported_models()` reads the initialize payload
(there is no `list_models` wire subtype). Hook callbacks, elicitation, and
the remaining auth/dialog subtypes are still open.

## Remaining work (do not expand this packet)

P1 is items 1–2 of issue #232 (message variants + no silent drop). P2 is
items 3 and 6 (initialize handshake + control timeouts). Later priorities,
still open:

- **P3 remainder** — elicitation and hook callbacks (the handler is still a
  TODO that always continues). `background_tasks` and
  `cancel_async_message` already landed in the P3 first slice.
- **P4 remainder** — result `terminal_reason`, `api_error_status`, later
  `modelUsage` fields. Hook callbacks and elicitation stay open; this
  packet does not execute hooks.

## P4 first slice landed (issue #232)

`PermissionMode::Auto` is on the wire (`auto`). `build_args()` emits
`--fallback-model` and `--plugin-dir` from the previously ignored
`fallback_model` and `plugins` fields.

## P4 remainder landed (issue #232)

`QueryOptions::build_args()` now emits the previously ignored option
fields as CLI flags verified against `claude` 2.1.247 and
`@anthropic-ai/claude-agent-sdk` 0.3.172:

- `system_prompt`: `Custom` → `--system-prompt`; Preset `append` →
  `--append-system-prompt`. A Preset with no append uses the CLI default.
- `mcp_servers` → `--mcp-config` with a JSON string
  `{"mcpServers":{...}}` (same encoding as the TS SDK).
- `agents` → `--agents` JSON object (`disallowedTools` camelCase).
- `sandbox` → `--settings` JSON `{"sandbox":{...}}`. The main CLI rejects
  `--sandbox` (`unknown option`); the TS SDK also writes sandbox settings
  through `--settings`, not a `--sandbox` flag.
- `output_format` schema → `--json-schema`. `--output-format stream-json`
  stays the SDK transport; a response schema is not a second output
  format and must not replace stream-json.

Added because the CLI/TS SDK have real flags:

- `tools` → `--tools` (`Names` joined by comma, empty list is `--tools ""`,
  `Default` is `--tools default`).
- `thinking` → `--thinking adaptive|disabled`, `--thinking-display`, or
  `--max-thinking-tokens` for `Enabled { budget_tokens }`. Takes
  precedence over `max_thinking_tokens`.
- `effort` → `--effort` (`low|medium|high|xhigh|max`).

The fake-claude initialize fixture now builds its handshake payload with
`JSON.parse` of a JSON string so Node does not see unquoted object-literal
keys.

Out of scope for the whole #232 port unless a later packet says otherwise:
full hook execution, the ~50 TypeScript `Query` methods, and a complete
options audit.

Later upstream types (0.3.247+) such as `conversation_reset`, `informational`,
`control_request_progress`, `background_tasks_changed`, `worker_shutting_down`,
and `model_refusal_no_fallback` currently arrive as `SdkMessage::Unknown`.
That is the P1 contract, not a silent drop.
