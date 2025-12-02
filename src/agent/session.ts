import * as FileSystem from "@effect/platform/FileSystem";
import * as Path from "@effect/platform/Path";
import { Effect } from "effect";
import * as S from "effect/Schema";
import type { AgentConfig, AgentTurn } from "./loop.js";

const ToolCallSchema = S.Struct({
  id: S.String,
  name: S.String,
  arguments: S.String,
});

const ToolContentSchema = S.Union(
  S.Struct({ type: S.Literal("text"), text: S.String }),
  S.Struct({ type: S.Literal("image"), data: S.String, mimeType: S.String }),
);

const ToolResultSchema = S.Struct({
  toolCallId: S.String,
  name: S.String,
  result: S.Struct({ content: S.Array(ToolContentSchema) }),
  isError: S.Boolean,
});

const AgentTurnSchema = S.Struct({
  role: S.Union(S.Literal("assistant"), S.Literal("tool")),
  content: S.NullOr(S.String),
  toolCalls: S.optional(S.Array(ToolCallSchema)),
  toolResults: S.optional(S.Array(ToolResultSchema)),
});

const ChatMessageSchema = S.Struct({
  role: S.Union(S.Literal("system"), S.Literal("user"), S.Literal("assistant"), S.Literal("tool")),
  content: S.String,
  tool_call_id: S.optional(S.String),
  name: S.optional(S.String),
});

const SessionEventSchema = S.Union(
  S.Struct({
    type: S.Literal("session_start"),
    timestamp: S.String,
    sessionId: S.String,
    config: S.Struct({
      model: S.optional(S.String),
      systemPrompt: S.optional(S.String),
      maxTurns: S.optional(S.Number),
      temperature: S.optional(S.Number),
    }),
  }),
  S.Struct({
    type: S.Literal("user_message"),
    timestamp: S.String,
    content: S.String,
  }),
  S.Struct({
    type: S.Literal("turn"),
    timestamp: S.String,
    turn: AgentTurnSchema,
  }),
  S.Struct({
    type: S.Literal("message"),
    timestamp: S.String,
    message: ChatMessageSchema,
  }),
  S.Struct({
    type: S.Literal("session_end"),
    timestamp: S.String,
    totalTurns: S.Number,
    finalMessage: S.NullOr(S.String),
  }),
);

type SessionEvent = S.Schema.Type<typeof SessionEventSchema>;

export interface Session {
  id: string;
  config: AgentConfig;
  messages: Array<{ role: string; content: string; tool_call_id?: string; name?: string }>;
  turns: AgentTurn[];
  userMessage: string;
}

export class SessionError extends Error {
  readonly _tag = "SessionError";
  constructor(
    readonly reason: "not_found" | "parse_error" | "write_error",
    message: string,
  ) {
    super(message);
    this.name = "SessionError";
  }
}

const generateSessionId = () => {
  const now = new Date();
  const date = now.toISOString().slice(0, 10).replace(/-/g, "");
  const time = now.toISOString().slice(11, 19).replace(/:/g, "");
  const rand = Math.random().toString(36).slice(2, 6);
  return `session-${date}-${time}-${rand}`;
};

const timestamp = () => new Date().toISOString();

export const createSession = (
  config: AgentConfig,
  userMessage: string,
  sessionId?: string,
): Session => ({
  id: sessionId ?? generateSessionId(),
  config,
  messages: [],
  turns: [],
  userMessage,
});

export const appendEvent = (
  sessionPath: string,
  event: SessionEvent,
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const line = JSON.stringify(event) + "\n";
    yield* fs.writeFile(sessionPath, new TextEncoder().encode(line), { flag: "a" }).pipe(
      Effect.mapError((e) => new SessionError("write_error", `Failed to append event: ${e.message}`)),
    );
  });

export const writeSessionStart = (
  sessionPath: string,
  session: Session,
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  appendEvent(sessionPath, {
    type: "session_start",
    timestamp: timestamp(),
    sessionId: session.id,
    config: session.config,
  });

export const writeUserMessage = (
  sessionPath: string,
  content: string,
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  appendEvent(sessionPath, {
    type: "user_message",
    timestamp: timestamp(),
    content,
  });

export const writeTurn = (
  sessionPath: string,
  turn: AgentTurn,
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  appendEvent(sessionPath, {
    type: "turn",
    timestamp: timestamp(),
    turn: turn as S.Schema.Type<typeof AgentTurnSchema>,
  });

export const writeMessage = (
  sessionPath: string,
  message: { role: string; content: string; tool_call_id?: string; name?: string },
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  appendEvent(sessionPath, {
    type: "message",
    timestamp: timestamp(),
    message: message as S.Schema.Type<typeof ChatMessageSchema>,
  });

export const writeSessionEnd = (
  sessionPath: string,
  totalTurns: number,
  finalMessage: string | null,
): Effect.Effect<void, SessionError, FileSystem.FileSystem> =>
  appendEvent(sessionPath, {
    type: "session_end",
    timestamp: timestamp(),
    totalTurns,
    finalMessage,
  });

export const loadSession = (
  sessionPath: string,
): Effect.Effect<Session, SessionError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const exists = yield* fs.exists(sessionPath).pipe(
      Effect.mapError((e) => new SessionError("not_found", `Cannot check file: ${e.message}`)),
    );
    if (!exists) {
      return yield* Effect.fail(new SessionError("not_found", `Session file not found: ${sessionPath}`));
    }

    const content = yield* fs.readFileString(sessionPath).pipe(
      Effect.mapError((e) => new SessionError("not_found", `Cannot read file: ${e.message}`)),
    );

    const lines = content.trim().split("\n").filter((l) => l.length > 0);
    if (lines.length === 0) {
      return yield* Effect.fail(new SessionError("parse_error", "Empty session file"));
    }

    let session: Session | null = null;

    for (const line of lines) {
      const parsed = yield* Effect.try({
        try: () => JSON.parse(line) as SessionEvent,
        catch: () => new SessionError("parse_error", `Invalid JSON: ${line.slice(0, 50)}...`),
      });

      const event = yield* S.decodeUnknown(SessionEventSchema)(parsed).pipe(
        Effect.mapError(() => new SessionError("parse_error", `Invalid event format: ${line.slice(0, 50)}...`)),
      );

      switch (event.type) {
        case "session_start":
          session = {
            id: event.sessionId,
            config: {
              ...(event.config.model ? { model: event.config.model } : {}),
              ...(event.config.systemPrompt ? { systemPrompt: event.config.systemPrompt } : {}),
              ...(event.config.maxTurns !== undefined ? { maxTurns: event.config.maxTurns } : {}),
              ...(event.config.temperature !== undefined ? { temperature: event.config.temperature } : {}),
            },
            messages: [],
            turns: [],
            userMessage: "",
          };
          break;

        case "user_message":
          if (session) {
            session.userMessage = event.content;
          }
          break;

        case "message":
          if (session) {
            const msg: { role: string; content: string; tool_call_id?: string; name?: string } = {
              role: event.message.role,
              content: event.message.content,
            };
            if (event.message.tool_call_id) msg.tool_call_id = event.message.tool_call_id;
            if (event.message.name) msg.name = event.message.name;
            session.messages.push(msg);
          }
          break;

        case "turn":
          if (session) {
            session.turns.push(event.turn as AgentTurn);
          }
          break;

        case "session_end":
          break;
      }
    }

    if (!session) {
      return yield* Effect.fail(new SessionError("parse_error", "No session_start event found"));
    }

    return session;
  });

export const getSessionPath = (
  sessionsDir: string,
  sessionId: string,
): Effect.Effect<string, never, Path.Path> =>
  Effect.gen(function* () {
    const path = yield* Path.Path;
    return path.join(sessionsDir, `${sessionId}.jsonl`);
  });

export const listSessions = (
  sessionsDir: string,
): Effect.Effect<string[], SessionError, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;

    const exists = yield* fs.exists(sessionsDir).pipe(
      Effect.mapError((e) => new SessionError("not_found", e.message)),
    );
    if (!exists) {
      return [];
    }

    const entries = yield* fs.readDirectory(sessionsDir).pipe(
      Effect.mapError((e) => new SessionError("not_found", e.message)),
    );

    return entries
      .filter((e) => e.endsWith(".jsonl"))
      .map((e) => e.replace(".jsonl", ""))
      .sort()
      .reverse();
  });
