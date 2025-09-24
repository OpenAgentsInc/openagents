# Core: Tools and JSON Schemas

Defines model tools and adapts them for both OpenAI Responses and Chat
Completions.

File: `codex-rs/core/src/openai_tools.rs`

## Key types

- `OpenAiTool` — enum of tool flavors:
  - `Function(ResponsesApiTool)`
  - `LocalShell {}` (native tool recognized by some models)
  - `WebSearch {}`
  - `Freeform(FreeformTool)` for grammar‑based patching
- `ToolsConfig`/`ToolsConfigParams` — compute which tools to include based on
  model family + config.
- `JsonSchema` — small subset of JSON Schema used to describe function params.

## Building tool lists

- `get_openai_tools(&ToolsConfig, mcp_tools)` produces a vector of
  `OpenAiTool`s. When MCP tools are present, they are converted to function
  tools using `mcp_tool_to_openai_tool` with schema sanitation.
- `create_tools_json_for_responses_api` serializes into the Responses format.
- `create_tools_json_for_chat_completions_api` rewrites that JSON into
  `{\"type\":\"function\",\"function\":{...}}` entries for Chat.

## Apply Patch tool

- Freeform variant: `create_apply_patch_freeform_tool()` defines a grammar tool
  with the Lark grammar embedded from `tool_apply_patch.lark`.
- Function variant: `create_apply_patch_json_tool()` exposes a JSON parameter
  object with a single `input` string (entire patch envelope).
- `ToolsConfig` picks one based on model family or `include_apply_patch_tool`.

## Unified exec tool

- `create_unified_exec_tool()` exposes a single tool that can either spawn a
  new command (`input: [cmd, ...]`) or write stdin to an existing session
  (`session_id`).

## Freeform/Custom tools

- `FreeformTool` supports arbitrary tool formats (currently grammar‑based).
- Helpful for models with better freeform parsing capabilities.

## MCP tool conversion

- `sanitize_json_schema` normalizes untyped schemas from servers by inferring
  `type` for objects/arrays/strings and filling missing child fields with
  permissive defaults.

