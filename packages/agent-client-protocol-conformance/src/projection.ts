import { createHash } from "node:crypto";

import { observeSessionUpdate } from "./variants.ts";

export type ConformanceCanonicalEvent = Readonly<{
  kind:
    | "message-delta"
    | "tool-call"
    | "tool-call-update"
    | "plan"
    | "available-commands"
    | "mode-change"
    | "configuration-change"
    | "session-info"
    | "usage"
    | "turn-stop"
    | "degraded";
  source: "agent-client-protocol";
  payload: unknown;
  nativeSha256: string;
}>;

const object = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};

export const projectSessionUpdateForConformance = (update: unknown): ConformanceCanonicalEvent => {
  const observation = observeSessionUpdate(update);
  const value = object(update);
  const payload = (() => {
    switch (observation.discriminator) {
      case "user_message_chunk":
        return { role: "user", channel: "message", content: value.content };
      case "agent_message_chunk":
        return { role: "agent", channel: "message", content: value.content };
      case "agent_thought_chunk":
        return { role: "agent", channel: "thought", content: value.content };
      case "tool_call":
      case "tool_call_update":
        return {
          toolCallId: value.toolCallId,
          title: value.title,
          kind: value.kind,
          status: value.status,
          content: value.content,
          locations: value.locations,
          rawInput: value.rawInput,
          rawOutput: value.rawOutput,
        };
      case "plan":
        return { entries: value.entries };
      case "available_commands_update":
        return { availableCommands: value.availableCommands };
      case "current_mode_update":
        return { currentModeId: value.currentModeId };
      case "config_option_update":
        return { configOptions: value.configOptions };
      case "session_info_update":
        return { title: value.title, updatedAt: value.updatedAt };
      case "usage_update":
        return { used: value.used, size: value.size, cost: value.cost };
      default:
        return {
          reason: "unknown-session-update",
          discriminator: observation.discriminator,
          inspectableNativeSha256: observation.nativeSha256,
        };
    }
  })();
  const kind = (() => {
    switch (observation.discriminator) {
      case "user_message_chunk":
      case "agent_message_chunk":
      case "agent_thought_chunk":
        return "message-delta";
      case "tool_call":
        return "tool-call";
      case "tool_call_update":
        return "tool-call-update";
      case "plan":
        return "plan";
      case "available_commands_update":
        return "available-commands";
      case "current_mode_update":
        return "mode-change";
      case "config_option_update":
        return "configuration-change";
      case "session_info_update":
        return "session-info";
      case "usage_update":
        return "usage";
      default:
        return "degraded";
    }
  })() satisfies ConformanceCanonicalEvent["kind"];
  return { kind, source: "agent-client-protocol", payload, nativeSha256: observation.nativeSha256 };
};

export const projectStopReasonForConformance = (stopReason: string): ConformanceCanonicalEvent => {
  const known = ["end_turn", "max_tokens", "max_turn_requests", "refusal", "cancelled"].includes(
    stopReason,
  );
  const nativeSha256 = createHash("sha256").update(JSON.stringify({ stopReason })).digest("hex");
  return known
    ? {
        kind: "turn-stop",
        source: "agent-client-protocol",
        payload: { reason: stopReason },
        nativeSha256,
      }
    : {
        kind: "degraded",
        source: "agent-client-protocol",
        payload: { reason: "unknown-stop-reason", nativeStopReason: stopReason },
        nativeSha256,
      };
};

export type OrderedNativeUpdate = Readonly<{
  generation: number;
  sessionId: string;
  updateId: string;
  sequence: number;
  update: unknown;
}>;

export type ProjectionDisposition = Readonly<{
  outcome: "applied" | "duplicate" | "quarantined";
  reason?: "old-generation" | "out-of-order" | "tool-state-regression";
  event?: ConformanceCanonicalEvent;
}>;

const toolRank: Readonly<Record<string, number>> = {
  pending: 0,
  in_progress: 1,
  completed: 2,
  failed: 2,
};

export class ConformanceProjectionState {
  readonly #seen = new Set<string>();
  readonly #lastSequence = new Map<string, number>();
  readonly #toolRanks = new Map<string, number>();
  readonly #toolStatuses = new Map<string, string>();
  #generation = 0;

  apply(native: OrderedNativeUpdate): ProjectionDisposition {
    const identity = createHash("sha256")
      .update(`${native.generation}:${native.sessionId}:${native.updateId}`)
      .digest("hex");
    if (this.#seen.has(identity)) return { outcome: "duplicate" };
    if (native.generation < this.#generation)
      return { outcome: "quarantined", reason: "old-generation" };
    if (native.generation > this.#generation) {
      this.#generation = native.generation;
      this.#lastSequence.clear();
      this.#toolRanks.clear();
      this.#toolStatuses.clear();
    }
    const last = this.#lastSequence.get(native.sessionId) ?? -1;
    if (native.sequence <= last) return { outcome: "quarantined", reason: "out-of-order" };
    const update = object(native.update);
    if (
      update.sessionUpdate === "tool_call_update" &&
      typeof update.toolCallId === "string" &&
      typeof update.status === "string"
    ) {
      const nextRank = toolRank[update.status] ?? 0;
      const previousRank = this.#toolRanks.get(update.toolCallId) ?? -1;
      const previousStatus = this.#toolStatuses.get(update.toolCallId);
      if (
        nextRank < previousRank ||
        ((previousStatus === "completed" || previousStatus === "failed") &&
          update.status !== previousStatus)
      )
        return { outcome: "quarantined", reason: "tool-state-regression" };
      this.#toolRanks.set(update.toolCallId, nextRank);
      this.#toolStatuses.set(update.toolCallId, update.status);
    }
    this.#seen.add(identity);
    this.#lastSequence.set(native.sessionId, native.sequence);
    return { outcome: "applied", event: projectSessionUpdateForConformance(native.update) };
  }
}
