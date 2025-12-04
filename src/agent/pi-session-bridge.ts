import * as S from "effect/Schema";
import type { SessionEvent } from "./session.js";
import { SessionEventSchema } from "./session.js";

// Pi-mono style session entries (JSONL)
export const PiSessionEntrySchema = S.Union(
  S.Struct({
    type: S.Literal("session"),
    id: S.String,
    timestamp: S.String,
    cwd: S.String,
    provider: S.optional(S.String),
    model: S.optional(S.String),
    thinkingLevel: S.optional(S.String),
  }),
  S.Struct({
    type: S.Literal("message"),
    timestamp: S.String,
    message: S.Unknown,
  }),
  S.Struct({
    type: S.Literal("thinking_level_change"),
    timestamp: S.String,
    thinkingLevel: S.String,
  }),
  S.Struct({
    type: S.Literal("model_change"),
    timestamp: S.String,
    provider: S.String,
    model: S.String,
  }),
);

export type PiSessionEntry = S.Schema.Type<typeof PiSessionEntrySchema>;

const TH_LEVEL_PREFIX = "PI_META:thinking_level_change:";
const MODEL_CHANGE_PREFIX = "PI_META:model_change:";
type SessionMessageEvent = Extract<SessionEvent, { type: "message" }>;

/**
 * Parse a pi-mono JSONL session file/string into typed entries.
 */
export const parsePiSession = (content: string): PiSessionEntry[] => {
  const lines = content
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);

  return lines.map((line) => {
    const parsed = JSON.parse(line);
    return S.decodeUnknownSync(PiSessionEntrySchema)(parsed);
  });
};

/**
 * Convert pi-mono session entries to OpenAgents SessionEvents (JSONL).
 * We encode pi-mono specific events into system messages so they are reversible.
 */
export const piSessionToSessionEvents = (entries: PiSessionEntry[]): SessionEvent[] => {
  return entries.map((entry): SessionEvent => {
    switch (entry.type) {
      case "session":
        return {
          type: "session_start",
          timestamp: entry.timestamp,
          sessionId: entry.id,
          config: {
            model: entry.model,
            systemPrompt: undefined,
            maxTurns: undefined,
            temperature: undefined,
          },
        };
      case "message":
        return {
          type: "message",
          timestamp: entry.timestamp,
          message: normalizeMessage(entry.message),
        };
      case "thinking_level_change":
        return {
          type: "message",
          timestamp: entry.timestamp,
          message: {
            role: "system",
            content: `${TH_LEVEL_PREFIX}${entry.thinkingLevel}`,
          },
        };
      case "model_change":
        return {
          type: "message",
          timestamp: entry.timestamp,
          message: {
            role: "system",
            content: `${MODEL_CHANGE_PREFIX}${entry.provider}:${entry.model}`,
          },
        };
    }
  });
};

/**
 * Convert OpenAgents SessionEvents back to pi-mono session entries.
 * Special PI_META messages are restored to their structured entries.
 */
export const sessionEventsToPiSession = (events: SessionEvent[]): PiSessionEntry[] => {
  return events
    .map((event): PiSessionEntry | null => {
      switch (event.type) {
        case "session_start":
          return {
            type: "session",
            id: event.sessionId,
            timestamp: event.timestamp,
            cwd: "",
            provider: undefined,
            model: event.config.model,
            thinkingLevel: undefined,
          };
        case "message": {
          const msg = event as SessionMessageEvent;
          const content = typeof msg.message.content === "string" ? msg.message.content : null;
          if (content?.startsWith(TH_LEVEL_PREFIX)) {
            return {
              type: "thinking_level_change",
              timestamp: event.timestamp,
              thinkingLevel: content.slice(TH_LEVEL_PREFIX.length),
            };
          }
          if (content?.startsWith(MODEL_CHANGE_PREFIX)) {
            const parts = content.slice(MODEL_CHANGE_PREFIX.length).split(":");
            const provider = parts[0] ?? "";
            const model = parts[1] ?? "";
            return {
              type: "model_change",
              timestamp: event.timestamp,
              provider,
              model,
            };
          }
          return {
            type: "message",
            timestamp: event.timestamp,
            message: msg.message,
          };
        }
        default:
          return null;
      }
    })
    .filter((entry): entry is PiSessionEntry => entry !== null);
};

// Ensure messages always have role/content for SessionEvent schema compatibility
const normalizeMessage = (message: unknown): SessionMessageEvent["message"] => {
  if (message && typeof message === "object" && "role" in (message as any) && "content" in (message as any)) {
    return message as SessionMessageEvent["message"];
  }
  return { role: "user", content: JSON.stringify(message) };
};

export const decodeSessionEvents = (events: unknown[]): SessionEvent[] =>
  events.map((event) => S.decodeUnknownSync(SessionEventSchema)(event));
