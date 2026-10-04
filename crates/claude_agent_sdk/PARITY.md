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
(there is no `list_models` wire subtype). Elicitation and the remaining
auth/dialog subtypes are still open. `hook_callback` is a typed continue
stub; it does not run host hooks.

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

## Result fidelity landed (issue #232)

`ResultSuccess` models 0.3.172 `api_error_status` and `terminal_reason`.
`ResultError` models `terminal_reason`. `ModelUsage` includes
`maxOutputTokens` (optional on the wire so older fixtures still parse).
The 0.3.172 `TerminalReason` set is `blocking_limit`,
`rapid_refill_breaker`, `prompt_too_long`, `image_error`, `model_error`,
`aborted_streaming`, `aborted_tools`, `stop_hook_prevented`,
`hook_stopped`, `tool_deferred`, `max_turns`, `completed`. A later value
deserializes as `TerminalReason::Unknown` so the result stays a typed
`SdkResultMessage` instead of `SdkMessage::Unknown`. 0.3.247+ result
fields (`canonicalModel`, `queued_turn_count`, extra `modelUsage` keys)
are not modelled.

## Hook callback stub landed (issue #232)

Inbound `subtype: "hook_callback"` parses as `HookCallbackRequest`. Query
replies with typed `SyncHookJSONOutput::continue_without_running()`
(`{"continue": true}`). `HookCallbackStub` records `hook_event_name` and
`callback_id` so the path is testable; `hook_ran` is always false. This
is not hook execution: no host callback runs, and the reply has no
`hookSpecificOutput` or permission decision.

## Remaining work

#232's named first slices (P1–P4 plus this result/hook-stub packet) are
landed. Residual, out of scope unless a later packet says otherwise:

- **Full hook execution** — host callbacks, PreToolUse permission
  decisions, matchers, timeouts.
- **Elicitation and remaining auth/dialog control subtypes.**
- **The remaining TypeScript `Query` methods** (~50).
- **Later upstream types (0.3.247+)** such as `conversation_reset`,
  `informational`, `control_request_progress`,
  `background_tasks_changed`, `worker_shutting_down`, and
  `model_refusal_no_fallback` currently arrive as `SdkMessage::Unknown`.
  That is the P1 contract, not a silent drop.
