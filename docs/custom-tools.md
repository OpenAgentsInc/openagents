**Custom Tools in OpenAgents/Codex**

- Where to start: Tool architecture and examples live in `docs/codex/tools.md` and `docs/codex/core-openai-tools.md`.
- This doc shows practical ways to define and ship custom tools to the model without running an MCP server, plus where to plug in execution handlers inside `codex-rs`.

**Concepts**
- Tool types: `Function` (JSON schema) and `Custom/Freeform` (grammar-based).
  - Types and schemas: codex-rs/core/src/openai_tools.rs:1
  - Tools are serialized and sent with every model request: codex-rs/core/src/openai_tools.rs:287; codex-rs/core/src/client.rs:171
- Execution routing: Model emits a tool call by name; Codex matches and executes:
  - Function tools: codex-rs/core/src/codex.rs:2440
  - Custom tools: codex-rs/core/src/codex.rs:2624
- Built-ins you can emulate/extend:
  - `unified_exec` (PTY sessions): codex-rs/core/src/openai_tools.rs:161; codex-rs/core/src/codex.rs:2450
  - `apply_patch` (freeform or JSON): codex-rs/core/src/tool_apply_patch.rs:1; codex-rs/core/src/codex.rs:2515
  - `update_plan` (planning helper): codex-rs/core/src/plan_tool.rs:10

**Option 1 — Add a Function (JSON) tool**
- Define the tool schema (name/description/parameters) and include it in the tool list:
  - Add a constructor similar to `create_view_image_tool()` or `create_unified_exec_tool()`: codex-rs/core/src/openai_tools.rs:255
  - Append it from `get_openai_tools()` so it’s sent to the model: codex-rs/core/src/openai_tools.rs:479
- Handle the tool call by name in the dispatcher:
  - Add a match arm under `handle_function_call(...)` for your tool name, parse `arguments` into a struct, run logic, and return `ResponseInputItem::FunctionCallOutput`: codex-rs/core/src/codex.rs:2440
- Notes:
  - Keep `parameters` strict and small (Object with `properties`, `required`, `additionalProperties: false`).
  - Prefer snake_case tool names with <=64 chars. Avoid collisions with MCP names.

Example skeleton
- Define tool (schema):
  - Add a helper fn in openai_tools and push it from `get_openai_tools()`.
- Dispatch and execute:
  - In `handle_function_call`, add:
    - Parse `arguments` with `serde_json::from_str::<MyArgs>(&arguments)`
    - Perform work (sync/async)
    - Return `FunctionCallOutputPayload { content, success: Some(true|false) }`

**Option 2 — Add a Custom/Freeform (grammar) tool**
- For models that support “custom tools”, you can use a grammar to parse rich inputs.
  - See freeform `apply_patch`: grammar: codex-rs/core/src/tool_apply_patch.lark:1; definition: codex-rs/core/src/tool_apply_patch.rs:23
  - Add your own `OpenAiTool::Freeform(FreeformTool { name, format: { type: "grammar", syntax: "lark", definition } })`: codex-rs/core/src/openai_tools.rs:16
- Handle the call in `handle_custom_tool_call(...)` by matching your tool name and transforming the freeform `input` into execution: codex-rs/core/src/codex.rs:2624
- When possible, also provide a JSON variant for OSS models that lack custom tool support (see `create_apply_patch_json_tool()`): codex-rs/core/src/tool_apply_patch.rs:38

**How tools are sent to the model**
- Tool list assembly: `get_openai_tools()` builds the final `Vec<OpenAiTool>` including built-ins, optional view_image/web_search, and MCP tools (converted): codex-rs/core/src/openai_tools.rs:479
- Serialization for OpenAI APIs:
  - Responses API: `create_tools_json_for_responses_api()` → included under `tools` in request: codex-rs/core/src/openai_tools.rs:287, codex-rs/core/src/client.rs:171
  - Chat Completions API: `create_tools_json_for_chat_completions_api()` adapts to `{ type: function, function: { … } }`: codex-rs/core/src/openai_tools.rs:302

**Option 3 — Use MCP (no code changes here)**
- Configure servers in `~/.codex/config.toml`; Codex spawns and discovers tools automatically:
  - Manager: codex-rs/core/src/mcp_connection_manager.rs:1
  - Tool name qualification (server__tool): codex-rs/core/src/mcp_connection_manager.rs:22
- Tools are converted and appended to the list with sanitized schemas: codex-rs/core/src/openai_tools.rs:531

**Configuration switches (include/toggle tools)**
- Built-in toggles (via config → `ToolsConfig`): codex-rs/core/src/openai_tools.rs:92
  - Plan tool (`update_plan`)
  - Apply Patch tool (freeform or JSON)
  - Web search request tool
  - View image tool
  - Shell flavor (`unified_exec` vs shell/local)
- These are wired in `Config` and passed when building the client/turn context: codex-rs/core/src/config.rs:960; codex-rs/core/src/codex.rs:1218

**Minimal step-by-step (add a new built-in tool)**
1) Schema: create `create_my_tool()` returning `OpenAiTool::Function(ResponsesApiTool { name: "my_tool", description, parameters: JsonSchema::Object { ... } })` in codex-rs/core/src/openai_tools.rs:1
2) Listing: push it from `get_openai_tools()` so it’s offered to the model: codex-rs/core/src/openai_tools.rs:479
3) Handler: add a `"my_tool" => { … }` arm in `handle_function_call(...)`, parse args, do work, and return a `FunctionCallOutput`: codex-rs/core/src/codex.rs:2440
4) Optional: add a config flag to enable/disable it (thread it through `ToolsConfig` and `Config`): codex-rs/core/src/openai_tools.rs:70; codex-rs/core/src/config.rs:960
5) Test: simulate a tool call by constructing a `FunctionCall` item in a unit test and asserting the `FunctionCallOutput`.

**Tips and constraints**
- JSON Schema subset only: `JsonSchema` supports boolean/string/number/array/object with `properties`/`required`/`additionalProperties`. Avoid `anyOf`/`oneOf` unless you also sanitize to supported types: codex-rs/core/src/openai_tools.rs:332
- Keep names stable and deterministic—tool arrays are sorted for MCP tools to improve prompt caching.
- Respect approval/sandbox policies for anything that touches the filesystem or spawns processes (reuse `handle_container_exec_with_params` if applicable): codex-rs/core/src/codex.rs:2688
- Freeform tools give great UX on GPT‑5; ship a JSON fallback for OSS.

**Related docs**
- Tools overview: docs/codex/tools.md:1
- Core tool types and conversion: docs/codex/core-openai-tools.md:1
- Exec/sandboxing: docs/codex/core-exec.md:3
- Protocol and response items: docs/codex/protocol-overview.md:1; codex-rs/protocol/src/models.rs:1
