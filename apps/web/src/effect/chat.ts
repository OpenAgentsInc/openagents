import { Context, Effect, Fiber, Layer, Stream, SubscriptionRef } from "effect";

import { api } from "../../convex/_generated/api";
import { AuthService } from "./auth";
import { ConvexService } from "./convex";
import { RequestContextService } from "./requestContext";
import { TelemetryService } from "./telemetry";

import type * as AiResponse from "@effect/ai/Response";
import type { ChatMessage, ChatPart } from "./chatProtocol";

export type ChatSnapshot = {
  readonly messages: ReadonlyArray<ChatMessage>;
  readonly status: ChatStatus;
  readonly errorText: string | null;
};

export type ChatStatus = "ready" | "submitted" | "streaming" | "error";

type ChatSession = {
  readonly threadId: string;
  readonly state: SubscriptionRef.SubscriptionRef<ChatSnapshot>;
  messages: Array<ChatMessage>;
  readonly localStatus: { status: ChatStatus | null; errorText: string | null };
  dispose: () => void;
  activeRunId: string | null;
};

export type ChatClient = {
  readonly open: (
    threadId: string,
  ) => Effect.Effect<SubscriptionRef.SubscriptionRef<ChatSnapshot>, never, RequestContextService>;
  readonly send: (threadId: string, text: string) => Effect.Effect<void, Error, RequestContextService>;
  readonly stop: (threadId: string) => Effect.Effect<void, never, RequestContextService>;
  readonly clearHistory: (threadId: string) => Effect.Effect<void, never, RequestContextService>;
};

export class ChatService extends Context.Tag("@openagents/web/ChatService")<
  ChatService,
  ChatClient
>() {}

const initialSnapshot = (): ChatSnapshot => ({
  messages: [],
  status: "ready",
  errorText: null,
});

const ANON_THREAD_ID_KEY = "autopilot-anon-chat-id";
const ANON_THREAD_KEY_KEY = "autopilot-anon-chat-key";

function randomId(size = 16): string {
  let out = "";
  while (out.length < size) out += Math.random().toString(36).slice(2);
  return out.slice(0, size);
}

function getOrCreateAnonThreadId(): string {
  if (typeof sessionStorage === "undefined") return `anon-${randomId(12)}`;
  let id = sessionStorage.getItem(ANON_THREAD_ID_KEY);
  if (!id) {
    id = `anon-${randomId(12)}`;
    sessionStorage.setItem(ANON_THREAD_ID_KEY, id);
  }
  return id;
}

function getOrCreateAnonKey(): string {
  if (typeof sessionStorage === "undefined") return `key-${randomId(24)}`;
  let key = sessionStorage.getItem(ANON_THREAD_KEY_KEY);
  if (!key) {
    key = `key-${randomId(24)}`;
    sessionStorage.setItem(ANON_THREAD_KEY_KEY, key);
  }
  return key;
}

function stableStringify(value: unknown): string {
  if (value == null) return String(value);
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

type ActiveStream = {
  readonly id: string;
  readonly messageId: string;
  parts: Array<ChatPart>;
};

function applyRemoteChunk(active: ActiveStream, chunkData: AiResponse.StreamPartEncoded): ActiveStream {
  switch (chunkData?.type) {
    case "text-start": {
      active.parts.push({ type: "text", text: "", state: "streaming" });
      return active;
    }
    case "text-delta": {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === "text") as any;
      if (lastTextPart && lastTextPart.type === "text") {
        lastTextPart.text += String((chunkData as any).delta ?? "");
      } else {
        active.parts.push({ type: "text", text: String((chunkData as any).delta ?? "") });
      }
      return active;
    }
    case "text-end": {
      const lastTextPart = [...active.parts].reverse().find((p) => p?.type === "text") as any;
      if (lastTextPart && "state" in lastTextPart) lastTextPart.state = "done";
      return active;
    }
    case "reasoning-start":
    case "reasoning-delta":
    case "reasoning-end": {
      // Autopilot must not render reasoning. Ignore these parts on the UI wire.
      return active;
    }
    case "tool-call": {
      const toolName = String((chunkData as any).name ?? "tool");
      active.parts.push({
        type: `tool-${toolName}`,
        toolCallId: String((chunkData as any).id ?? ""),
        toolName,
        state: "input-available",
        input: (chunkData as any).params,
      } as any);
      return active;
    }
    case "tool-result": {
      const toolCallId = String((chunkData as any).id ?? "");
      const isFailure = Boolean((chunkData as any).isFailure);
      const result = (chunkData as any).result;

      let didUpdate = false;
      active.parts = active.parts.map((p) => {
        if (
          p &&
          typeof p === "object" &&
          "toolCallId" in p &&
          String((p as any).toolCallId) === toolCallId &&
          "state" in p
        ) {
          didUpdate = true;
          return {
            ...(p as any),
            state: isFailure ? "output-error" : "output-available",
            output: result,
            ...(isFailure ? { errorText: stableStringify(result) } : {}),
          };
        }
        return p;
      });

      if (!didUpdate) {
        const toolName = String((chunkData as any).name ?? "tool");
        active.parts.push({
          type: `tool-${toolName}`,
          toolCallId,
          toolName,
          state: isFailure ? "output-error" : "output-available",
          output: result,
          ...(isFailure ? { errorText: stableStringify(result) } : {}),
        } as any);
      }

      return active;
    }
    default:
      return active;
  }
}

function toChatSnapshot(session: ChatSession, messages: ReadonlyArray<ChatMessage>): ChatSnapshot {
  const computedStatus: ChatStatus =
    session.activeRunId != null ? "streaming" : "ready";

  const status = session.localStatus.status ?? computedStatus;
  const errorText = session.localStatus.errorText;

  return { messages, status, errorText };
}

function updateSnapshot(session: ChatSession): void {
  // SubscriptionRef is pure (no Env). Safe to run on the default runtime.
  Effect.runFork(SubscriptionRef.set(session.state, toChatSnapshot(session, session.messages)));
}

export const ChatServiceLive = Layer.effect(
  ChatService,
  Effect.gen(function* () {
    const convex = yield* ConvexService;
    const auth = yield* AuthService;
    const telemetry = yield* TelemetryService;
    const sessions = new Map<string, ChatSession>();

    const open = Effect.fn("ChatService.open")(function* (threadId: string) {
      const existing = sessions.get(threadId);
      if (existing) return existing.state;

      // SSR safety: do not attempt realtime subscriptions during server render.
      if (typeof window === "undefined") {
        const state = yield* SubscriptionRef.make(initialSnapshot());
        sessions.set(threadId, {
          threadId,
          state,
          messages: [],
          localStatus: { status: null, errorText: null },
          dispose: () => {},
          activeRunId: null,
        });
        return state;
      }

      // Ensure thread exists for anon, then best-effort claim when authed.
      const anonThreadId = getOrCreateAnonThreadId();
      const anonKey = getOrCreateAnonKey();

      // We intentionally allow callers to pass any threadId, but for MVP we only
      // use the per-tab anon thread id as the canonical thread id.
      if (threadId !== anonThreadId) {
        // Keep the internal keying stable: if a different id is requested, still
        // create a session for it (used by tests/experiments).
      }

      yield* convex
        .mutation(api.autopilot.threads.ensureAnonThread, { threadId, anonKey } as any)
        .pipe(Effect.catchAll(() => Effect.void));

      const sessionState = yield* SubscriptionRef.make(initialSnapshot());

      const session: ChatSession = {
        threadId,
        state: sessionState,
        messages: [],
        localStatus: { status: null, errorText: null },
        dispose: () => {},
        activeRunId: null,
      };

      sessions.set(threadId, session);

      const claimIfAuthed = Effect.gen(function* () {
        const s = yield* auth.getSession().pipe(Effect.catchAll(() => Effect.succeed({ userId: null } as any)));
        if (!s.userId) return;
        yield* convex
          .mutation(api.autopilot.threads.claimAnonThread, { threadId, anonKey } as any)
          .pipe(Effect.catchAll(() => Effect.void));
      });

      yield* claimIfAuthed.pipe(Effect.catchAll(() => Effect.void));

      const stream = convex.subscribeQuery(api.autopilot.messages.getThreadSnapshot, {
        threadId,
        anonKey,
      } as any);

      const fiber = yield* Stream.runForEach(stream, (snap: any) =>
        Effect.sync(() => {
          const messagesRaw = Array.isArray(snap?.messages) ? (snap.messages as any[]) : [];
          const partsRaw = Array.isArray(snap?.parts) ? (snap.parts as any[]) : [];

          // Rebuild messages deterministically from Convex rows.
          const byMessageId = new Map<string, ActiveStream>();
          let activeRunId: string | null = null;

          const partsSorted = [...partsRaw]
            .filter((p) => p && typeof p === "object")
            .sort((a, b) => Number((a as any).seq) - Number((b as any).seq));

          for (const p of partsSorted) {
            const messageId = String((p as any).messageId ?? "");
            const runId = String((p as any).runId ?? "");
            const part = (p as any).part as AiResponse.StreamPartEncoded | undefined;
            if (!messageId || !runId || !part || typeof part !== "object") continue;

            const active = byMessageId.get(messageId) ?? { id: runId, messageId, parts: [] };
            byMessageId.set(messageId, applyRemoteChunk(active, part));
          }

          const messages: Array<ChatMessage> = [];
          for (const m of messagesRaw) {
            const messageId = String(m?.messageId ?? "");
            const role = String(m?.role ?? "");
            const status = String(m?.status ?? "");
            const text = typeof m?.text === "string" ? m.text : "";
            const runId = typeof m?.runId === "string" ? m.runId : null;

            if (!messageId) continue;
            if (role === "user") {
              messages.push({ id: messageId, role: "user", parts: [{ type: "text", text }] });
              continue;
            }
            if (role === "assistant") {
              const active = byMessageId.get(messageId);
              if (active) {
                messages.push({ id: messageId, role: "assistant", parts: [...active.parts] });
              } else if (text.trim()) {
                messages.push({ id: messageId, role: "assistant", parts: [{ type: "text", text, state: "done" }] });
              } else {
                messages.push({ id: messageId, role: "assistant", parts: [] });
              }

              if (status === "streaming" && runId) {
                activeRunId = runId;
              }
              continue;
            }
          }

          session.activeRunId = activeRunId;
          session.messages = messages;

          // Clear local "submitted" status once we observe streaming or ready state from Convex.
          if (session.localStatus.status === "submitted") {
            session.localStatus.status = null;
            session.localStatus.errorText = null;
          }

          updateSnapshot(session);
        }),
      ).pipe(Effect.forkDaemon);

      session.dispose = () => {
        Effect.runFork(Fiber.interrupt(fiber));
      };

      yield* telemetry.withNamespace("chat.service").event("chat.open", { threadId });

      return sessionState;
    });

    const withSession = <TValue, TError, R>(
      threadId: string,
      f: (session: ChatSession) => Effect.Effect<TValue, TError, R>,
    ): Effect.Effect<TValue, TError, R | RequestContextService> =>
      open(threadId).pipe(
        Effect.flatMap(() => {
          const session = sessions.get(threadId);
          if (!session) {
            return Effect.sync(() => {
              console.warn("[ChatService] Session missing after open()", { threadId });
              return undefined as unknown as TValue;
            });
          }
          return f(session);
        }),
      );

    const send = Effect.fn("ChatService.send")(function* (threadId: string, text: string) {
      const anonKey = getOrCreateAnonKey();

      yield* withSession(threadId, (session) =>
        Effect.sync(() => {
          session.localStatus.status = "submitted";
          session.localStatus.errorText = null;
          updateSnapshot(session);
        }),
      );

      // Fire the Worker endpoint; streaming updates arrive via Convex subscription.
      const response = yield* Effect.tryPromise({
        try: () =>
          fetch("/api/autopilot/send", {
            method: "POST",
            headers: { "content-type": "application/json" },
            cache: "no-store",
            body: JSON.stringify({ threadId, anonKey, text }),
          }),
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      });

      if (!response.ok) {
        const body = yield* Effect.tryPromise({ try: () => response.text(), catch: () => "" }).pipe(
          Effect.catchAll(() => Effect.succeed("")),
        );
        const msg = body.trim() ? body.trim() : `HTTP ${response.status}`;
        yield* withSession(threadId, (session) =>
          Effect.sync(() => {
            session.localStatus.status = "error";
            session.localStatus.errorText = msg;
            updateSnapshot(session);
          }),
        );
        return yield* Effect.fail(new Error(msg));
      }

      // Best-effort clear local submitted state; subscription will set streaming.
      yield* withSession(threadId, (session) =>
        Effect.sync(() => {
          session.localStatus.status = null;
          session.localStatus.errorText = null;
        }),
      );
    });

    const stop = Effect.fn("ChatService.stop")(function* (threadId: string) {
      const anonKey = getOrCreateAnonKey();

      yield* withSession(threadId, (session) =>
        Effect.gen(function* () {
          const runId = session.activeRunId;
          if (!runId) return;

          yield* Effect.tryPromise({
            try: () =>
              fetch("/api/autopilot/cancel", {
                method: "POST",
                headers: { "content-type": "application/json" },
                cache: "no-store",
                body: JSON.stringify({ threadId, anonKey, runId }),
              }),
            catch: () => null,
          }).pipe(Effect.catchAll(() => Effect.void));

          session.localStatus.status = null;
          session.localStatus.errorText = null;
        }),
      );
    });

    const clearHistory = Effect.fn("ChatService.clearHistory")(function* (threadId: string) {
      const anonKey = getOrCreateAnonKey();

      yield* convex
        .mutation(api.autopilot.messages.clearMessages, { threadId, anonKey, keepWelcome: true } as any)
        .pipe(Effect.catchAll(() => Effect.void));
    });

    return ChatService.of({ open, send, stop, clearHistory });
  }),
);
