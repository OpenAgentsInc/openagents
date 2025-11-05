# iOS Tool Calls — Visuals, Components, and ACP Mapping

This spec defines how the iOS app renders tool calls using ACP types, mirroring Codex TUI’s presentation (see codex-openai `docs/tool-calls-visuals.md`) while fitting our SwiftUI components and ACP contract (ADR‑0007, ADR‑0014).

## Goals

- Replace raw JSON tool call lines with typed, purpose‑built components.
- Keep visual behavior consistent with Codex TUI where appropriate (states, headers, truncation) while adapting to native UI.
- Drive everything from ACP types; no provider‑native JSON in the view layer.

## References

- Codex visuals: /Users/christopherdavid/code/codex-openai/docs/tool-calls-visuals.md
- ADR‑0007 — Agent Client Protocol as canonical contract: docs/adr/0007-agent-client-protocol.md
- ADR‑0014 — iOS ↔ Desktop WS bridge: docs/adr/0014-ios-desktop-websocket-bridge-and-pairing.md

## Inputs and Types (Swift)

- `ACPToolCall` and `ACPToolResult`
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPTool.swift
- `ACPEvent`/`ACPEventKind`
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPEvent.swift
- `ACPMessage`, `ACPContentPart`
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPMessage.swift
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPContent.swift
- `ACPPlanState`
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/ACPPlanState.swift
- Translator (temporary input path from Codex JSONL → ACP):
  - ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift

Notes
- Until the iOS desktop bridge emits typed envelopes, we translate Codex JSONL locally using `CodexAcpTranslator` and assemble timeline items (ADR‑0014 follow‑up will move to true ACP envelopes).

## Visual States (parity with Codex)

- Exec shell tool call
  - In‑progress: spinner bullet, title “Running”, command shown.
  - Completed: green bullet + title “Ran” on success; red bullet + “Ran” on failure; output snippet shown.
- MCP tool call
  - In‑progress: spinner bullet, title “Calling server.tool(args)”.
  - Completed: green/red bullet + title “Called …”; structured output snippet.
- Apply Patch (planned)
  - Approval overlay + diff summary cell similar to Codex; see Components below.
- View Image / Web Search (planned)
  - Short entry with path/query, mapped as specialized tool calls.
- Plan state
  - Already implemented: `PlanStateView` for ACP `plan_state` updates.

## Component Inventory (SwiftUI)

All components live under `ios/OpenAgents/ACP/Renderers/`.

- `ToolCallCell.swift`
  - Purpose: High‑level aggregator for a single tool call instance (call + latest result + progress state).
  - Input: `ToolCallRecord` (see View Models) containing `ACPToolCall` and optional `ACPToolResult`.
  - Behavior: Chooses a specialized renderer based on `tool_name`/arguments (Exec, MCP, Apply Patch, generic) and provides common header (bullet/title/timestamps).

- `ExecCallCell.swift`
  - Purpose: Render exec (shell) calls with command and output snippet.
  - Input: `ToolCallRecord` where `tool_name` is shell (see Mapping Rules).
  - Visuals:
    - Header: spinner/green/red bullet; title “Running”/“Ran”.
    - Command: mono, syntax‑highlight optional; wrapped with continuation styling.
    - Output: uses `ExecOutputBlock` with head/ellipsis/tail selection and wrap truncation.

- `ExecOutputBlock.swift`
  - Purpose: Render a compact stdout/stderr snippet following Codex TUI truncation strategy.
  - Input: `ExecSnippet` (see View Models) with interleaved lines and marks.
  - Limits: `TOOL_CALL_MAX_LINES = 5`; `USER_SHELL_MAX_LINES = 50` for user ‘!’ commands.

- `McpCallCell.swift`
  - Purpose: Render MCP tool calls as `server.tool(args)` with structured output.
  - Input: `ToolCallRecord` interpreted as MCP (see Mapping Rules).
  - Visuals: Cyan “server” and “tool” tokens, dim compact JSON args, wrapped invocation with indented details.

- `ApplyPatchCell.swift` (planned)
  - Purpose: Render an apply‑patch summary cell (and optionally the same content in an approval overlay).
  - Input: `ApplyPatchSummary` (see View Models), plus optional `PatchDiff` model.
  - Visuals: “Edited N files (+A -D)” header and per‑file lines. For details view, uses `DiffView`.

- `DiffView.swift` (planned)
  - Purpose: Render unified diffs with gutter, colorized +/- and wrapped continuation (Codex parity).
  - Input: `PatchDiff` with hunks and changes; supports multiple files with separators.

- `GenericToolCallCell.swift`
  - Purpose: Fallback renderer for any tool call we don’t specialize.
  - Input: `ToolCallRecord`.
  - Visuals: `tool_name(args JSON)` header; result/error pretty‑printed JSON (horizontally scrollable) as a compact snippet.

- Existing reused components
  - `PlanStateView.swift` — already renders ACP plan state updates.
  - `ToolCallView.swift`/`ToolResultView.swift` — will be subsumed by the cells above; retained for internal use or removed once replaced.

## View Models and Helpers

- `ToolCallRecord`
  - Fields: `call: ACPToolCall`, `result: ACPToolResult?`, `state: ToolCallState`, `tsStart: Int64`, `tsEnd: Int64?`.
  - `ToolCallState`: `.inProgress`, `.succeeded`, `.failed`.

- `ExecSnippet`
  - Interleaved captured lines from stdout/stderr (if available), or compacted from `ACPToolResult.result` (string/array of strings) when present.
  - Selection strategy: head/ellipsis/tail (Codex parity), then wrap to available width and middle‑truncate to `TOOL_CALL_MAX_LINES` (or `USER_SHELL_MAX_LINES`).

- `JSONFormatting`
  - Helper to compact JSON (one line with spaces after `:` and `,`) for invocation args/result when needed.

- `StatusBullet`
  - Spinner (in‑progress), green dot (success), red dot (failure).

All helpers can be colocated in the same files initially; extract to `ios/OpenAgents/ACP/Helpers/` if they grow.

## Mapping Rules (ACP → Component)

Tool call categorization uses `ACPToolCall.tool_name` and `arguments`:

- Exec (shell)
  - Match when `tool_name.lowercased()` is `"shell"` or ends with `.shell`.
  - Command extraction: prefer array under `arguments["command"]` (string/number/bool → string), then compact join with quotes for whitespace args; if `bash -lc` pattern, extract the single string.
  - Component: `ExecCallCell`.

- MCP
  - Preferred patterns:
    - `tool_name` as `"server.tool"` (two segments) or `"mcp.server.tool"` (three segments).
    - Or `arguments` carries `{ server: string, tool: string }`.
  - Render invocation `server.tool(argsJSON)` with compact JSON.
  - Component: `McpCallCell`.

- Apply Patch (planned)
  - Match when `tool_name` is `"apply_patch"` (or a namespaced variant) or when `arguments` carries a patch payload (files/hunks) produced by Codex.
  - Component: `ApplyPatchCell` (summary in history) + `DiffView` for details/overlays.

- View Image / Web Search (planned)
  - `view_image` → show “Viewed image <path>”.
  - `web.search`/`search` → show “Searched: <query>”.
  - Component: `GenericToolCallCell` with tailored header strings until specialized cells are added.

- Fallback
  - Any other tool: `GenericToolCallCell` shows `tool_name(args)` and pretty JSON for result/error.

Result mapping (ACPToolResult)
- Pair on `call_id`. If no result yet → `.inProgress`.
- When present: `ok == true` → `.succeeded`; else `.failed` with `error` text shown.

## Timeline Assembly (changes required)

Current behavior
- `AcpThreadView_computeTimeline` maps tool_call/tool_result lines to `.raw` items.

Proposed behavior
- Emit typed items instead of raw:
  - On tool call: append `.toolCall(call)`.
  - On tool result: append `.toolResult(result)`.
- In the view model for the scrolling list, coalesce them into `ToolCallRecord`s:
  - Maintain a dictionary `pending[call_id] → ToolCallRecord`.
  - On `.toolCall(call)`: insert pending; render a `ToolCallCell` with `.inProgress`.
  - On `.toolResult(result)`: update the matching record and re‑render `ToolCallCell` with `.succeeded`/`.failed`.
- Optional coalescing (Exploring/Explored):
  - When a contiguous block of exec calls all parse as read/list/search ops, render a single grouped cell:
    - Header: “Exploring” (spinner) → “Explored” (complete).
    - Rows: short labels like `Read …`, `List <path>`, `Search <query> in <path>` built from parsed args.

Feature flag
- Flip the current guard in `AcpThreadView` so typed tool cells render by default; retain a debug toggle to show raw JSON in an inspector/sheet.

## Visual Details and Limits

- Fonts and colors
  - Use `OAFonts.mono` for commands/output; `OAFonts.ui` for headers.
  - Colors from `OATheme.Colors` (success/danger/textSecondary/border) for status and dimmed text.

- Truncation constants
  - `TOOL_CALL_MAX_LINES = 5`
  - `USER_SHELL_MAX_LINES = 50` (when rendering user `!` commands)

- Wrapping and continuation
  - Commands wrap with a subtle continuation indicator (indent align with header). For concise parity, we won’t draw TUI’s `│/└` characters; we’ll use padding and alignment.
  - Output uses head/ellipsis/tail selection; after wrapping, middle‑truncate extra visual rows to the max line budget and show a dim `… +N lines` row.

- JSON compaction
  - Arguments/result shown inline are compacted JSON (single line) to improve wrapping; full pretty JSON is available in a detail sheet.

## Files and Paths

- ios/OpenAgents/ACP/Renderers/ToolCallCell.swift
- ios/OpenAgents/ACP/Renderers/ExecCallCell.swift
- ios/OpenAgents/ACP/Renderers/ExecOutputBlock.swift
- ios/OpenAgents/ACP/Renderers/McpCallCell.swift
- ios/OpenAgents/ACP/Renderers/GenericToolCallCell.swift
- ios/OpenAgents/ACP/Renderers/ApplyPatchCell.swift (planned)
- ios/OpenAgents/ACP/Renderers/DiffView.swift (planned)

Integration points
- Update `ios/OpenAgents/AcpThreadView.swift`:
  - Timeline assembly: emit `.toolCall`/`.toolResult` instead of `.raw` for those events.
  - Rendering switch: replace the `{ if Features.showRawJSON { … } }` checks for tool calls/results with `ToolCallCell(record)`.
  - Add a lightweight in‑memory `pending` map to pair calls/results.

## Mapping to Codex Equivalents

- Exec
  - Codex: `ExecCommandBeginEvent`/`ExecCommandEndEvent` → ACP: `ACPToolCall` (`tool_name: shell`) / `ACPToolResult`.
  - Visuals: “Running”/“Ran”, bullet state, command, output snippet, truncation (limits above).

- MCP
  - Codex: `McpToolCallBeginEvent`/`McpToolCallEndEvent` → ACP: `ACPToolCall` (tool_name encodes `server.tool` or arguments carry `server/tool`) / `ACPToolResult`.
  - Visuals: “Calling/Called server.tool(args)”, structured result.

- Apply Patch
  - Codex: `ApplyPatchApprovalRequestEvent`, `PatchApplyBeginEvent`, `PatchApplyEndEvent` → ACP: planned mapping via `tool_name: apply_patch` + result/error.
  - Visuals: Diff summary cell + detail overlay; `DiffView` reproduces Codex gutter/wrap behavior.

## Open Questions / Assumptions

- Output capture for exec
  - On iOS we typically don’t receive interleaved stdout/stderr lines; the result may be a single string. `ExecOutputBlock` should accept both raw lines and a single string payload.

- Grouping “Exploring/Explored”
  - We will start with straightforward exec rendering; grouping will be added behind a flag once we finalize heuristics for read/list/search detection.

- Bridge envelopes
  - ADR‑0014 follow‑up will introduce typed envelopes for ACP/Tinyvex sync from the desktop bridge; when available, we will retire the local translator and drive the `ToolCallRecord` assembly from the envelopes directly.

## Rollout Plan

1) Add the new cells (Exec/MCP/Generic) and wire them in `AcpThreadView` without removing raw JSON (keep a debug inspector).
2) Switch timeline assembly to typed items for tool calls/results; add `pending` pairing.
3) Flip default to render typed tool cells; gate raw JSON behind a debug toggle.
4) Add Apply Patch summary and DiffView (if/when ACP mapping lands on the bridge/translator).

## Acceptance

- Tool calls render with typed cells, not raw JSON, in the iOS app.
- Exec/MCP calls show the correct state, invocation text, and a compact output snippet.
- The app continues to expose a raw JSON inspector for debugging, separate from the main timeline.
