# Tool Handling Improvements (AI SDK Review)

Date: 2026-02-06

Goal: reduce cases where the model emits an invalid tool call (unknown tool name / invalid JSON inputs) and the UI appears to stall with no user-visible text.

## What I Reviewed (in `~/code/ai`)

AI SDK docs and implementation confirm two key behaviors that matter for us:

1. Tool-call errors become `tool-error` parts (not stream errors).
   - If the UI hides tool parts, the user can see "nothing happened" unless the model follows up with normal text.
2. AI SDK supports a single-pass repair hook (`experimental_repairToolCall`) that runs *during parsing* of a tool call.
   - It can fix `NoSuchToolError` (unknown tool name) and `InvalidToolInputError` (bad input vs schema) without polluting conversation history.

References:
- Docs: `~/code/ai/content/docs/03-ai-sdk-core/15-tools-and-tool-calling.mdx`
  - Multi-step tool loops via `stopWhen`
  - `onStepFinish` callback
  - `experimental_repairToolCall` patterns (schema-based + re-ask)
- Parser: `~/code/ai/packages/ai/src/generate-text/parse-tool-call.ts`
  - Repair hook is invoked only for `NoSuchToolError` and `InvalidToolInputError`
  - If repair returns `null`, AI SDK marks the tool call `invalid` and emits a `tool-error` part
- Tool transform: `~/code/ai/packages/ai/src/generate-text/run-tools-transformation.ts`
  - `invalid` tool calls emit `tool-error` and do *not* execute tool `execute()`
- Multi-step continuation: `~/code/ai/packages/ai/src/generate-text/stream-text.ts`
  - The next step is triggered when tool calls have matching tool outputs (`tool-result` or `tool-error`)

## What Changed In OpenAgents

### 1. Add Tool-Call Repair (no side effects)

File: `apps/autopilot-worker/src/server.ts`

Added:
- `experimental_repairToolCall` on our `streamText(...)` call.
- A helper `stripToolExecution()` that clones a ToolSet but removes `execute` and input hooks.
  - This allows using `generateText(...)` to re-emit a valid tool call *without* executing it during repair.

Repair strategy:
- If inputs are invalid (`InvalidToolInputError`):
  - Force the same tool name, regenerate inputs (tool call only), then return a repaired `{ toolName, input }`.
- If the tool name is unknown (`NoSuchToolError`):
  - Restrict to `BASE_TOOLS` only (safe tools), require *some* tool call, and return that repaired call.
  - This avoids accidentally calling state-mutating Blueprint tools when the model invents tool names.

### 2. Add Step-Level Logging for Tool Activity

File: `apps/autopilot-worker/src/server.ts`

Added:
- `onStepFinish` logging when any tool calls/results occur.
  - Logs `toolName`, `toolCallId`, and whether any tool calls were marked invalid.
  - Helps diagnose tool-call failures that would otherwise be invisible in the chat UI (since tool parts are not rendered).

## How To Test

1. Start dev:
   - `cd apps/autopilot-worker && npm run dev`
   - `cd apps/web && npm run dev`
2. In chat:
   - "Use a tool: echo the word 'ping'." (should call `echo`)
   - Ask for an invalid/unknown tool name (the model sometimes invents tools):
     - e.g. "Use browser.search for MCP."
     - Expected: the model should not stall; repair should kick in and the agent should follow up with a normal text reply.
3. Watch worker logs:
   - `"[chat] repaired tool name"` / `"[chat] repaired tool inputs"` when repair triggers.
   - `"[chat] step.finish"` lines when tools are used.

