import { createHash } from "node:crypto";

import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";

export const CONTENT_BLOCK_FIXTURES = [
  { type: "text", text: "fixture" },
  { type: "image", data: "AA==", mimeType: "image/png" },
  { type: "audio", data: "AA==", mimeType: "audio/wav" },
  { type: "resource_link", name: "fixture", uri: "file:///workspace/a.txt" },
  { type: "resource", resource: { uri: "file:///workspace/a.txt", text: "fixture" } },
] as const;

export const STOP_REASONS = [
  "end_turn",
  "max_tokens",
  "max_turn_requests",
  "refusal",
  "cancelled",
] as const;
export const TOOL_STATUSES = ["pending", "in_progress", "completed", "failed"] as const;
export const TOOL_KINDS = [
  "read",
  "edit",
  "delete",
  "move",
  "search",
  "execute",
  "think",
  "fetch",
  "switch_mode",
  "other",
] as const;
export const TOOL_CONTENT_FIXTURES = [
  { type: "content", content: { type: "text", text: "fixture" } },
  { type: "diff", path: "/workspace/a.txt", newText: "new" },
  { type: "terminal", terminalId: "fixture-terminal-1" },
] as const;

export const SESSION_UPDATE_FIXTURES = [
  { sessionUpdate: "user_message_chunk", content: { type: "text", text: "fixture" } },
  { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "fixture" } },
  { sessionUpdate: "agent_thought_chunk", content: { type: "text", text: "fixture" } },
  {
    sessionUpdate: "tool_call",
    toolCallId: "tool-1",
    title: "fixture",
    kind: "read",
    status: "pending",
  },
  {
    sessionUpdate: "tool_call_update",
    toolCallId: "tool-1",
    status: "completed",
    content: [{ type: "content", content: { type: "text", text: "done" } }],
  },
  {
    sessionUpdate: "plan",
    entries: [{ content: "fixture", priority: "medium", status: "pending" }],
  },
  {
    sessionUpdate: "available_commands_update",
    availableCommands: [{ name: "fixture", description: "fixture command" }],
  },
  { sessionUpdate: "current_mode_update", currentModeId: "agent" },
  { sessionUpdate: "config_option_update", configOptions: [] },
  { sessionUpdate: "session_info_update", title: "Fixture session" },
  { sessionUpdate: "usage_update", used: 1, size: 10, cost: { amount: 0, currency: "USD" } },
] as const;

export type VariantObservation = Readonly<{
  classification: "known" | "unknown";
  discriminator: string;
  nativeSha256: string;
  native: unknown;
}>;

export const observeSessionUpdate = (update: unknown): VariantObservation => {
  const native = { sessionId: "fixture-session-1", update };
  const decoded = decodeStableAcpMethodPayload({
    direction: "agent-to-client",
    method: "session/update",
    phase: "params",
    payload: native,
  });
  const discriminator =
    typeof update === "object" && update !== null && "sessionUpdate" in update
      ? String((update as { sessionUpdate: unknown }).sessionUpdate)
      : "<missing>";
  return {
    classification: decoded._tag === "Decoded" ? "known" : "unknown",
    discriminator,
    nativeSha256: createHash("sha256").update(JSON.stringify(decoded.native.raw)).digest("hex"),
    native: decoded.native.raw,
  };
};
