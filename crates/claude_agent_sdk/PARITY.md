# Claude Agent SDK Rust parity

This crate ports the wire protocol of `@anthropic-ai/claude-agent-sdk`: the
stream-json frames the `claude` CLI writes, the control protocol in both
directions, and the options that become CLI flags or `initialize` fields.
It tracks SDK **0.3.296** (`UPSTREAM_SDK_VERSION` in `src/lib.rs`), checked
against Claude Code **2.1.295**, the newest CLI installed for the smoke. The npm package is read as a reference;
nothing from it is vendored.

Run `scripts/check-claude-sdk-parity.sh` to compare the crate with the
tracked version, or pass `latest` to see what a newer release adds. The
script exits 1 only for gaps this file does not list as deferred.

## Stream messages

Every `type` and `system` `subtype` in the 0.3.296 `SDKMessage` union
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

Added for 0.3.296 (0.3.290 to 0.3.296 add no message type or subtype):

- `agent_id` on assistant and user messages a subagent produces
  (0.3.292); assistant messages also carry `subagent_type` and
  `task_description`, which 0.3.289 already had.
- `run_id` on `task_started`, `task_updated`, `task_progress`, and
  `task_notification`, and `parent_task_id` on `task_started` (0.3.292).
- `background_tasks_changed` entries: `run_id`, `parent_task_id` (0.3.292)
  and `subagent_type` (0.3.293).
- `SdkRateLimitInfo::overage_enabled` (`overageEnabled`, 0.3.295). The
  `overageStatus`, `overageResetsAt`, and `overageDisabledReason` fields
  were already modelled, so `allowed_warning` events now carrying them
  parse unchanged.
- `startup_failure_reason` values `org_config_required_unavailable` and
  `org_config_refused` (0.3.296) parse; the field stays a string.
- `citations` on text blocks (fixed in 0.3.295 for streamed responses)
  pass through: `message` and stream `event` are kept as JSON, and a test
  pins both forms.
- Fixed here: an inbound `user` frame (tool results, replays) parsed to
  `SdkMessage::Unknown`, because the enum tag consumed the `type` field the
  struct required. It now parses to `SdkMessage::User` and serializes with
  one `type`, and `isSynthetic` and `isReplay` use their wire names (the
  snake_case spellings are still read). `microcoder`'s `claude_sdk` engine
  records these frames in its transcript, which it had silently skipped.

## Control protocol

Outbound (SDK to CLI): all 40 subtypes in 0.3.289 `SDKControlRequestInner`
(unchanged through 0.3.296)
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

Added for 0.3.296:

- The `initialize` response's `claude_code_version` is
  `Query::claude_code_version()`. Claude Code 2.1.295 does not send it yet,
  so the smoke reports it absent.
- A permission answer whose `updatedPermissions` holds over
  `MAX_UPDATED_PERMISSIONS` (4,096) updates, rules, and directories counted
  together is sent as a deny that names the count
  (`PermissionResult::within_limits`). Claude Code 2.1.295 and later count
  such an answer as a denial; the CLI does not say whether the limit is per
  list or combined, so the SDK applies the stricter combined count.
- `suppress_always_allow_rule` on `can_use_tool` (set for connector tools
  an organization requires approval for, 0.3.292) was already modelled.

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

Added for 0.3.296:

- Flag values ride in the flag's own argument (`--model=haiku`), as the TS
  SDK does since 0.3.295, so a value that starts with `-` is never read as
  another flag. `--mcp-config`, `--managed-settings`, `--settings`, and
  extra arguments keep `--flag value` unless the value starts with `-`,
  also as in the TS SDK. The stream-json transport flags are unchanged. The
  crate's own `--system-prompt`, `--append-system-prompt`, and `--agents`
  use the joined form too.
- `AgentDefinition::auto_compact_window` (`autoCompactWindow`, 0.3.296).
  `AgentDefinition` now derives `Default`.
- The sandbox option merges into an inline `settings.sandbox` block
  instead of replacing it (0.3.296): its values win, values it does not set
  are kept, `filesystem.denyRead`, `filesystem.denyWrite`,
  `network.deniedDomains`, `credentials.files`, and `credentials.envVars`
  from both sides are combined, `ripgrep` and `network.tlsTerminate` are
  replaced whole, `filesystem.disabled: true` is dropped when the option
  restricts the filesystem, unset proxy ports are dropped when it restricts
  domains, and `failIfUnavailable` defaults to `true` only when neither side
  sets it. `settings` given as inline JSON text merges like an object; a
  settings file path with a sandbox is still rejected.
- `SandboxSettings` gains `fail_if_unavailable`, `filesystem`,
  `credentials`, and `extra`; `SandboxNetworkConfig` gains
  `allowed_domains`, `denied_domains`, `strict_allowlist`, and `extra`.
  Fields without a typed slot go in `extra` as written.

`build_args` follows the 0.3.289 argument builder, as revised above. Fixed against the
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
  `Unknown` messages. On 2026-10-09 with Claude Code 2.1.295, the owner's
  login, no API key, and the `--flag=value` arguments it completed the
  handshake (13 models, 55 commands, no `claude_code_version` yet) and a
  `success` result (`completed`) with no `Unknown` messages.
- `cargo test -p microcoder`: the `claude_sdk` engine's fake-CLI tests,
  updated for the joined flag spelling.

## SDK-hosted MCP servers

Added after 0.3.296 parity (`src/mcp.rs`): `SdkMcpServer` and `SdkMcpTool`
are the Rust `createSdkMcpServer()` and `tool()`, and
`QueryOptions::sdk_mcp_server` registers one.

- As in the TS SDK, an SDK server is not written to `--mcp-config`; its
  name goes in `initialize.sdkMcpServers`, and a `timeout_ms` goes in
  `initialize.sdkMcpServerConfigs` as `{"timeout": ms}`.
- `mcp_message` requests route to the named server and are answered with
  `{"mcp_response": <JSON-RPC reply>}`. `initialize` answers with the
  requested protocol version when supported (else 2025-11-25),
  `capabilities.tools`, `serverInfo`, and `instructions`; `tools/list`
  gives `name`, `description`, `inputSchema`, `annotations`, and `_meta`
  (`anthropic/alwaysLoad`, `anthropic/searchHint`); `tools/call` runs the
  handler; `ping` answers `{}`. An unknown tool is JSON-RPC error -32602
  and an unknown method -32601. A notification or response gets the TS
  SDK's acknowledgement `{"jsonrpc":"2.0","result":{},"id":0}`. An unknown
  server name is a control error, as in the TS SDK.
- A handler `Err` becomes an `isError` result with the error text, as the
  TS `McpServer` does for a throw. Results carry text and image content,
  `isError`, and `structuredContent`.
- `sdk_mcp_manifests(true)` sends `initialize.sdkMcpServerManifests`
  (TS `captureSdkMcpManifests`); `mcp_message` is still answered for every
  server.
- `validate` rejects an SDK server whose name is empty or also a key of
  `mcp_servers`.
- Tool names follow `mcp__<server>__<tool>` (`SdkMcpServer::tool_name`,
  `allowed_tool_names`, `mcp_tool_name`).
- The 0.3.296 description limits (4,096 characters up front and for
  server instructions, 16,384 through tool search) are applied by the CLI;
  the crate exports them as `MCP_DESCRIPTION_LIMIT` and
  `MCP_DEFERRED_DESCRIPTION_LIMIT` and does not truncate.
- Verified by unit tests for the JSON-RPC handling, fake-CLI tests for
  `tools/list` and `tools/call` over `mcp_message`, and on 2026-10-09 by
  `cargo run -p claude_agent_sdk --example sdk_mcp_tools` against Claude
  Code 2.1.295 on the owner's login: `calc` reported `connected` in
  `system/init`, Claude called `mcp__calc__add(1234, 5678)` once, and the
  result was `success` with `6912`.

## Deferred

- SDK-hosted MCP server extras: input validation against the schema
  (left to the CLI), `resources` and `prompts`, server-sent messages
  (notifications and progress from the host to the CLI), and
  `mcp_set_servers` with in-process servers.
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
- Tool inputs and results stay JSON, so the 0.3.290 to 0.3.296 tool shape
  changes (WebFetch `offset`, ListAgents `sections` and `notes`,
  SendMessage `recipient_kind`, ReadNotifications `read_at` and
  `arrived_at`, MCP `tool_use_result` caps, `tool_result_meta`) need no
  port. The same holds for settings-file fields the SDK passes through
  untyped (hook `onFailure`, `idleCompaction`).
- Message `origin` (and its 0.3.292 `runId`) is not modelled.
- `AgentDefinition` models the 0.3.172 fields plus `autoCompactWindow`;
  the other optional agent fields (`initialPrompt`, `maxTurns`,
  `background`, `omitClaudeMd`, and the rest) are not.
