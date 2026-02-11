import { Cause, Context, Effect, Fiber, Layer, Stream, SubscriptionRef } from "effect";

import { api } from "../../convex/_generated/api";
import { AuthService } from "./auth";
import { ConvexService } from "./convex";
import { RequestContextService } from "./requestContext";
import { TelemetryService } from "./telemetry";

import type { ChatMessage, ChatMessageFinish } from "./chatProtocol";
import { applyChatWirePart } from "./chatWire";
import type { ActiveStream } from "./chatWire";

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
  /** Resolve the current user's owned thread id (requires auth). */
  readonly getOwnedThreadId: () => Effect.Effect<string, Error, RequestContextService>;
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
    yield* AuthService;
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

      const stream = convex.subscribeQuery(api.autopilot.messages.getThreadSnapshot, {
        threadId,
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
            const part = (p as any).part as unknown;
            if (!messageId || !runId || !part || typeof part !== "object") continue;

            const active: ActiveStream =
              byMessageId.get(messageId) ?? { id: runId, messageId, parts: [] };
            byMessageId.set(messageId, applyChatWirePart(active, part));
          }

          // Last finish part per message (for assistant LLM usage/model in metadata).
          const finishByMessageId = new Map<string, ChatMessageFinish>();
          for (const p of partsSorted) {
            const messageId = String((p as any).messageId ?? "");
            const part = (p as any).part;
            if (!messageId || !part || (part as any).type !== "finish") continue;
            const raw = part as any;
            const usage = raw?.usage && typeof raw.usage === "object"
              ? {
                  ...(typeof raw.usage.inputTokens === "number" ? { inputTokens: raw.usage.inputTokens } : {}),
                  ...(typeof raw.usage.outputTokens === "number" ? { outputTokens: raw.usage.outputTokens } : {}),
                  ...(typeof raw.usage.totalTokens === "number" ? { totalTokens: raw.usage.totalTokens } : {}),
                  ...(typeof raw.usage.promptTokens === "number" ? { promptTokens: raw.usage.promptTokens } : {}),
                  ...(typeof raw.usage.completionTokens === "number" ? { completionTokens: raw.usage.completionTokens } : {}),
                }
              : undefined;
            finishByMessageId.set(messageId, {
              ...(typeof raw?.reason === "string" ? { reason: raw.reason } : {}),
              ...(usage && Object.keys(usage).length > 0 ? { usage } : {}),
              ...(typeof raw?.modelId === "string" ? { modelId: raw.modelId } : {}),
              ...(typeof raw?.provider === "string" ? { provider: raw.provider } : {}),
              ...(typeof raw?.modelRoute === "string" ? { modelRoute: raw.modelRoute } : {}),
              ...(typeof raw?.modelFallbackId === "string" ? { modelFallbackId: raw.modelFallbackId } : {}),
            });
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
              const finish = finishByMessageId.get(messageId);
              const baseMsg = {
                id: messageId,
                role: "assistant" as const,
                finish,
                ...(runId ? { runId } : {}),
              } as const;
              if (active && active.parts.length > 0) {
                messages.push({ ...baseMsg, parts: [...active.parts] });
              } else if (text.trim()) {
                messages.push({
                  ...baseMsg,
                  parts: [{ type: "text", text, state: "done" }],
                });
              } else {
                messages.push({ ...baseMsg, parts: [] });
              }

              if (status === "streaming" && runId) {
                activeRunId = runId;
              }
              continue;
            }
          }

          const wasStreaming = session.activeRunId != null;
          session.activeRunId = activeRunId;
          session.messages = messages;

          if (!wasStreaming && activeRunId) {
            Effect.runFork(
              telemetry
                .withNamespace("chat.service")
                .event("chat.streaming_started", { threadId, runId: activeRunId })
                .pipe(Effect.catchAll(() => Effect.void)),
            );
          }

          // Clear local "submitted" status once we observe streaming or ready state from Convex.
          if (session.localStatus.status === "submitted") {
            session.localStatus.status = null;
            session.localStatus.errorText = null;
          }

          updateSnapshot(session);
        }),
      )
        .pipe(
          Effect.catchAllCause((cause) =>
            Effect.gen(function* () {
              const message = Cause.pretty(cause).trim() || "Chat subscription failed."
              yield* telemetry
                .withNamespace("chat.service")
                .log("error", "chat.subscribe_failed", { threadId, message })
                .pipe(Effect.catchAll(() => Effect.void))

              // Surface to UI instead of silently stalling with an empty chat.
              yield* Effect.sync(() => {
                session.localStatus.status = "error"
                session.localStatus.errorText = message
                updateSnapshot(session)
              })
            }),
          ),
          Effect.forkDaemon,
        );

      session.dispose = () => {
        Effect.runFork(Fiber.interrupt(fiber));
      };

      yield* telemetry.withNamespace("chat.service").event("chat.open", { threadId });

      return sessionState;
    });

    const getOwnedThreadId = Effect.fn("ChatService.getOwnedThreadId")(function* () {
      const result = yield* convex
        .mutation(api.autopilot.threads.ensureOwnedThread, {})
        .pipe(
          Effect.map((r) => (r as { ok: boolean; threadId: string }).threadId),
          Effect.mapError((e) => (e instanceof Error ? e : new Error(String(e)))),
        );
      return result;
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
      yield* telemetry
        .withNamespace("chat.service")
        .event("chat.send_started", { threadId, textLength: text.length })
        .pipe(Effect.catchAll(() => Effect.void));

      yield* withSession(threadId, (session) =>
        Effect.sync(() => {
          session.localStatus.status = "submitted";
          session.localStatus.errorText = null;
          updateSnapshot(session);
        }),
      );

      const response = yield* Effect.tryPromise({
        try: () =>
          fetch("/api/autopilot/send", {
            method: "POST",
            headers: { "content-type": "application/json" },
            credentials: "include",
            cache: "no-store",
            body: JSON.stringify({ threadId, text }),
          }),
        catch: (err) => (err instanceof Error ? err : new Error(String(err))),
      });

      if (!response.ok) {
        const body = yield* Effect.tryPromise({ try: () => response.text(), catch: () => "" }).pipe(
          Effect.catchAll(() => Effect.succeed("")),
        );
        const msg = body.trim() ? body.trim() : `HTTP ${response.status}`;
        yield* telemetry
          .withNamespace("chat.service")
          .event("chat.send_failed", { threadId, error: msg })
          .pipe(Effect.catchAll(() => Effect.void));
        yield* withSession(threadId, (session) =>
          Effect.sync(() => {
            session.localStatus.status = "error";
            session.localStatus.errorText = msg;
            updateSnapshot(session);
          }),
        );
        return yield* Effect.fail(new Error(msg));
      }

      yield* telemetry
        .withNamespace("chat.service")
        .event("chat.send_complete", { threadId })
        .pipe(Effect.catchAll(() => Effect.void));

      // Best-effort clear local submitted state; subscription will set streaming.
      yield* withSession(threadId, (session) =>
        Effect.sync(() => {
          session.localStatus.status = null;
          session.localStatus.errorText = null;
        }),
      );
    });

    const stop = Effect.fn("ChatService.stop")(function* (threadId: string) {
      yield* withSession(threadId, (session) =>
        Effect.gen(function* () {
          const runId = session.activeRunId;
          if (!runId) return;

          yield* Effect.tryPromise({
            try: () =>
              fetch("/api/autopilot/cancel", {
                method: "POST",
                headers: { "content-type": "application/json" },
                credentials: "include",
                cache: "no-store",
                body: JSON.stringify({ threadId, runId }),
              }),
            catch: () => null,
          }).pipe(Effect.catchAll(() => Effect.void));

          session.localStatus.status = null;
          session.localStatus.errorText = null;
        }),
      );
    });

    const clearHistory = Effect.fn("ChatService.clearHistory")(function* (threadId: string) {
      yield* convex
        .mutation(api.autopilot.messages.clearMessages, { threadId, keepWelcome: true } as any)
        .pipe(Effect.catchAll(() => Effect.void));
    });

    return ChatService.of({ getOwnedThreadId, open, send, stop, clearHistory });
  }),
);
