/**
 * HARN-09 (#9167): project raw Codex app-server wire notifications onto the
 * SDK harness adapter's neutral {@link CodexEvent} vocabulary
 * (`@openagentsinc/agent-harness-contract`). Used by the
 * `runCodexAppServerTurn` `onCodexEvent` tee so the live app-server turn can
 * feed `CodexAppServerTransport.runTurnStreaming` (the rc.3 streaming seam)
 * while the hand-written display path keeps emitting the rich renderer
 * events.
 *
 * Only the public-safe subset the SDK models is projected (agent message,
 * reasoning, command execution, file change, MCP tool call, web search).
 * Display-only wire notifications (plan, meter, children, output deltas,
 * notices, guardian reviews) have NO neutral origin and are deliberately not
 * projected — they stay host-owned in `codex-app-server-turn.ts`.
 */

import type { CodexEvent, CodexThreadItem } from "@openagentsinc/agent-harness-contract";

type CodexItemStatus = "in_progress" | "completed" | "failed";

const string = (value: unknown): string | null => (typeof value === "string" ? value : null);

const itemStatus = (value: unknown): CodexItemStatus => {
  if (value === "completed") return "completed";
  if (value === "failed") return "failed";
  return "in_progress";
};

const fileChangeKind = (value: unknown): "add" | "delete" | "update" => {
  const type =
    value !== null && typeof value === "object"
      ? string((value as Record<string, unknown>).type)
      : string(value);
  return type === "add" || type === "delete" ? type : "update";
};

/**
 * Normalize one app-server v2 `item/started` / `item/completed` payload item
 * (camelCase wire `item.type`) onto the adapter's {@link CodexThreadItem}.
 * Item types the SDK does not model (collabAgentToolCall, subAgentActivity,
 * plan, dynamicToolCall, hookPrompt, sleep, review modes, contextCompaction)
 * return null — they remain host-display concerns only.
 */
export const codexHarnessThreadItem = (item: Record<string, unknown>): CodexThreadItem | null => {
  const id = string(item.id) ?? `${string(item.type) ?? "item"}`;
  switch (item.type) {
    case "agentMessage":
      return { itemType: "agent_message", id, text: string(item.text) ?? "" };
    case "reasoning": {
      const parts = Array.isArray(item.summary)
        ? item.summary.filter((value): value is string => typeof value === "string")
        : [];
      return { itemType: "reasoning", id, text: parts.join("\n") };
    }
    case "commandExecution":
      return {
        itemType: "command_execution",
        id,
        commandDisplay: (string(item.command) ?? "").slice(0, 400),
        status: itemStatus(item.status),
        ...(typeof item.exitCode === "number" ? { exitCode: item.exitCode } : {}),
      };
    case "fileChange":
      return {
        itemType: "file_change",
        id,
        status: itemStatus(item.status),
        changes: (Array.isArray(item.changes) ? item.changes : []).flatMap((change) => {
          if (change === null || typeof change !== "object") return [];
          const record = change as Record<string, unknown>;
          const path = string(record.path);
          if (path === null) return [];
          return [{ path, kind: fileChangeKind(record.kind) }];
        }),
      };
    case "mcpToolCall":
      return {
        itemType: "mcp_tool_call",
        id,
        serverName: string(item.server) ?? "mcp",
        toolName: string(item.tool) ?? "tool",
        status: itemStatus(item.status),
      };
    case "webSearch":
      return { itemType: "web_search", id, status: itemStatus(item.status) };
    default:
      return null;
  }
};

/** Convenience alias so the tee call sites read as one vocabulary. */
export type { CodexEvent };
