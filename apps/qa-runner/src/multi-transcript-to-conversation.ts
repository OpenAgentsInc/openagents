/**
 * Convert a combined multi-harness orchestration transcript (the JSONL the
 * AI SDK's seven-lane/multi-harness live smokes write: user_message,
 * assistant_message, agent.child.* lifecycle lines, and lane+model-attributed
 * `khala` event lines) into an {@link OpenAgentsConversation} so the existing
 * `/trace/{uuid}` ingest pipeline (convert -> redact -> validate -> POST)
 * can publish it unchanged.
 *
 * Projection is public-safe by construction: user and assistant text pass
 * through; lane lifecycle, handoffs, and tool activity become bounded system
 * notes; raw event payloads and text deltas are NOT copied (final assistant
 * text already carries the answer).
 */

import type { OpenAgentsConversation } from "./openagents-conversation-to-atif";

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string => (typeof value === "string" ? value : "");

interface ConversationMessage {
  readonly role: string;
  readonly content: string;
}

/** Parse the combined transcript text into an OpenAgentsConversation. */
export function multiTranscriptToConversation(
  content: string,
  options: { readonly id?: string; readonly title?: string } = {},
): OpenAgentsConversation {
  const messages: ConversationMessage[] = [];
  const system = (text: string): void => {
    messages.push({ role: "system", content: text });
  };
  for (const raw of content.split("\n")) {
    const trimmed = raw.trim();
    if (trimmed === "") continue;
    let line: JsonRecord | null;
    try {
      line = asRecord(JSON.parse(trimmed));
    } catch {
      continue;
    }
    if (line === null) continue;
    const lane = asString(line.lane);
    const model = asString(line.model);
    switch (line.type) {
      case "user_message":
        messages.push({ role: "user", content: asString(line.text) });
        break;
      case "assistant_message":
        messages.push({ role: "assistant", content: asString(line.text) });
        break;
      case "agent.child.started":
        system(`[delegate started: ${lane}${model === "" ? "" : ` (${model})`}]`);
        break;
      case "agent.child.finished":
        system(
          `[delegate finished: ${lane}${
            asString(line.finishReason) === "" ? "" : ` — ${asString(line.finishReason)}`
          }]`,
        );
        break;
      case "agent.child.interacted":
        system(
          `[handoff: ${asString(line.fromLane)} -> ${lane}${
            asString(line.note) === "" ? "" : ` — ${asString(line.note)}`
          }]`,
        );
        break;
      case "khala": {
        const event = asRecord(line.event);
        if (event === null) break;
        if (event.kind === "tool.call") {
          system(`[${lane} tool: ${asString(event.toolName) || "tool"}]`);
        } else if (event.kind === "file.change") {
          system(`[${lane} file change]`);
        } else if (event.kind === "turn.interrupted") {
          system(`[${lane} turn interrupted]`);
        }
        break;
      }
      default:
        break;
    }
  }
  return {
    id: options.id ?? "multi-harness-transcript",
    title: options.title ?? "Multi-harness orchestrated conversation",
    messages,
  } as OpenAgentsConversation;
}
