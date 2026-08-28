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
`stop_task`, and `get_context_usage`. `supported_models()` reads the
initialize payload (there is no `list_models` wire subtype). Remaining
control-request subtypes and hook callbacks are still open.

## Remaining work (do not expand this packet)

P1 is items 1–2 of issue #232 (message variants + no silent drop). P2 is
items 3 and 6 (initialize handshake + control timeouts). Later priorities,
still open:

- **P3 remainder** — remaining control-request subtypes, including
  `background_tasks`, `cancel_async_message`, elicitation, and
  hook callbacks (the handler is still a TODO that always continues).
- **P4 options and result fidelity** — wire the dead `QueryOptions` fields
  (`system_prompt`, `mcp_servers`, `agents`, `sandbox`, `plugins`,
  `output_format`, `fallback_model`) into `build_args()`; add `tools`,
  `thinking` / `effort`, and permission mode `auto`; carry `terminal_reason`,
  `api_error_status`, and the later `modelUsage` fields on results.

Out of scope for the whole #232 port unless a later packet says otherwise:
full hook execution, the ~50 TypeScript `Query` methods, and a complete
options audit.

Later upstream types (0.3.247+) such as `conversation_reset`, `informational`,
`control_request_progress`, `background_tasks_changed`, `worker_shutting_down`,
and `model_refusal_no_fallback` currently arrive as `SdkMessage::Unknown`.
That is the P1 contract, not a silent drop.
