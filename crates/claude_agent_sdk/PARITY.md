# Claude Agent SDK Rust parity

This crate ports the wire protocol of `@anthropic-ai/claude-agent-sdk`: the
stream-json frames the `claude` CLI writes, the control protocol in both
directions, and the options that become CLI flags or `initialize` fields.
It tracks SDK **0.3.289** (`UPSTREAM_SDK_VERSION` in `src/lib.rs`), checked
against Claude Code **2.1.289**. The npm package is read as a reference;
nothing from it is vendored.

Run `scripts/check-claude-sdk-parity.sh` to compare the crate with the
tracked version, or pass `latest` to see what a newer release adds. The
script exits 1 only for gaps this file does not list as deferred.

## Stream messages

Every `type` and `system` `subtype` in the 0.3.289 `SDKMessage` union
parses to a typed `SdkMessage` variant. A valid JSON object with an
unmodelled `type` or `subtype` becomes `SdkMessage::Unknown { type_name,
raw }` and the stream continues; invalid JSON becomes
`Error::UnrecognizedMessage` and the stream continues.

Added for 0.3.289:

- Top-level types: `active_goal`, `conversation_reset`.
- `system` subtypes: `control_request_progress`,
  `model_refusal_no_fallback`, `background_tasks_changed`,
  `worker_shutting_down`, `informational`.
- `TerminalReason`: `api_error`, `malformed_tool_use_exhausted`,
  `background_requested`, `budget_exhausted`,
  `structured_output_retry_exhausted`, `tool_deferred_unavailable`,
  `turn_setup_failed`. A later value is `TerminalReason::Unknown`, so the
  result stays typed.
- `AssistantMessageError`: `account_on_hold`, `verification_required`,
  `cloud_credential_error`.
- Optional fields on existing messages: result `ResultTurnFields`
  (`queued_turn_count`, `result_index`, `user_message_uuid(s)`,
  `resume_reason`, `fast_mode_disabled_reason`, `startup_failure_reason`);
  `modelUsage` `thinkingTokens`, `canonicalModel`, `provider`,
  `costBasis`; assistant `timestamp`, `aborted`, `user_message_uuid(s)`,
  `context_usage`, `usage_report`; `system/init` `plugin_errors`,
  `effort`, `capabilities`; `tool_progress` `heartbeat`, `subagent_type`,
  `subagent_retry`; `api_retry` `no_response`; task and rate-limit fields.

No 0.3.172 message type or required field was removed in 0.3.289.

## Control protocol

Outbound (SDK to CLI): all 40 subtypes in 0.3.289 `SDKControlRequestInner`
serialize, and `Query` has a method for each one a host sends. New since
0.3.172: `list_models`, `get_hooks_listing`, `get_task_output`,
`list_permission_rules`, `mcp_read_resource`, `reload_output_styles`, and
`update_settings`. `get_settings`, `set_color`, `mcp_call`, `read_file`,
`seed_read_state`, and `register_repo_root` were in 0.3.172 but not
modelled before. `interrupt` carries `cancel_queued`, and `rewind_files`
carries `dry_run`.

`initialize` is sent first, in camelCase as the CLI expects
(`sdkMcpServers`, `jsonSchema`, `systemPrompt` as a string array, and the
other 0.3.289 fields). Earlier versions of this crate sent snake_case
names; the defaults left them empty, so no frame was affected.

Inbound (CLI to SDK), each answered on its own task so a slow callback
does not stall the stdout reader:

| Subtype | Answer |
| --- | --- |
| `can_use_tool` | The `PermissionHandler` (`can_use_tool_request` sees every 0.3.289 field). Without a handler: an error, as in the TS SDK. |
| `hook_callback` | Runs the host closure registered under that callback ID; its `HookJSONOutput` (sync or async) is the reply. An unknown ID is an error. |
| `elicitation` | The `ElicitationHandler`; `Ok(None)` sends nothing. Without a handler: `{"action":"decline"}`. |
| `request_user_dialog` | The `UserDialogHandler`; without one, no reply, as in the TS SDK. |
| `mcp_message` | Error: SDK-hosted MCP servers are not supported. |
| `oauth_token_refresh`, `host_auth_token_refresh` | Error: no token callback. |
| `remote_tool_call` and the other remote tool subtypes | No reply, as in the TS SDK. |
| Any other subtype | Error naming the subtype. |

`control_cancel_request` aborts the matching in-flight answer, and a
duplicate delivery of an in-flight request is skipped. Permission and
dialog requests carried in a control response's
`pending_permission_requests` or `pending_user_dialog_requests` are
answered like any other request.

Hooks are registered as in the TS SDK: `QueryOptions::hooks` (or
`QueryOptions::hook`) maps a `HookEvent` (all 33 0.3.289 events) to
`HookMatcher`s; each callback gets the ID `hook_N`, and the matchers go to
the CLI in `initialize.hooks`.

## Options and flags

`build_args` follows the 0.3.289 argument builder. Fixed against the
CLI: `--allowedTools` and `--disallowedTools` take one comma-joined value,
`--setting-sources=` and `--betas` take joined lists,
`allow_dangerously_skip_permissions` is
`--allow-dangerously-skip-permissions`, persistence off is
`--no-session-persistence` (was the nonexistent `--no-persist-session`),
file checkpointing is the `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING`
environment variable (was the nonexistent `--enable-file-checkpointing`),
and `--permission-prompt-tool stdio` is sent only when a permission
handler is present.

Added: `task_budget`, `agent`, `debug`, `debug_file`,
`permission_prompt_tool_name`, `permission_prompts`, `strict_mcp_config`,
`include_hook_events`, `project_config_root`, `session_id`,
`resume_drops_turn`, `settings` (merged with `sandbox`, which now carries
`failIfUnavailable` when enabled), `managed_settings`,
`plugin_delivery` (`--await-initialize`), `skip_mcp_discovery`
(`--plugin-dir-no-mcp`), `verbatim_prompts` (`client_composed`), and the
`initialize` fields `supported_dialog_kinds`, `per_task_stop_affordance`,
`prompt_suggestions`, `agent_progress_summaries`,
`forward_subagent_text`, `title`, `skills`, `plan_mode_instructions`,
and `tool_aliases`. `validate` rejects a fallback model equal to the main
model, a permission handler with a permission prompt tool, and a settings
path with a sandbox.

The system prompt and agents still go as `--system-prompt`,
`--append-system-prompt`, and `--agents`; the TS SDK sends them in
`initialize`, and the CLI accepts both. Permission modes are unchanged
from 0.3.172.

## Verification

- `cargo test -p claude_agent_sdk`: fixtures for every new message,
  inbound control frame, and outbound subtype, plus fake-CLI tests for
  hook execution, elicitation, dialogs, unsupported subtypes,
  cancellation, and flag spellings.
- `cargo run -p claude_agent_sdk --example smoke`: one Haiku turn through
  the installed CLI with `--no-session-persistence`, no tools, and no
  settings files. On 2026-10-04 with Claude Code 2.1.289 it completed the
  handshake (12 models, 55 commands) and a `success` result with no
  `Unknown` messages.

## Deferred

- SDK-hosted MCP servers (`createSdkMcpServer`, `mcp_message` routing,
  `sdkMcpServerManifests`).
- Streaming input (`AsyncIterable` prompts, `streamInput`), and
  `abortController`, `stderr`, `spawnClaudeCodeProcess`, `loadTimeoutMs`,
  and `toolConfig`.
- Session stores (`sessionStore`, `sessionStoreFlush`, the
  `transcript_mirror` frame) and the session listing helpers.
- Auth token callbacks (`oauth_token_refresh`,
  `host_auth_token_refresh`), which the published declarations do not
  expose.
- Hook input and hook-specific output stay `serde_json::Value` rather than
  one Rust type per event.
