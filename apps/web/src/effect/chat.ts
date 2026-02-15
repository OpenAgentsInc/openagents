import { Cause, Context, Effect, Exit, Fiber, Layer, Stream, SubscriptionRef } from "effect";

import { api } from "../../convex/_generated/api";
import { AuthService } from "./auth";
import { ConvexService } from "./convex";
import { RequestContextService } from "./requestContext";
import { TelemetryService } from "./telemetry";
import {
  collectChatTelemetryEventsForSnapshot,
  collectStreamingTransitionEvents,
  createChatTelemetryState,
  hydrateChatTelemetryState,
} from "./chatTelemetry";

import type { ChatMessage, ChatMessageFinish } from "./chatProtocol";
import { applyChatWirePart } from "./chatWire";
import type { ActiveStream } from "./chatWire";

const isLocalHost = (host: string): boolean =>
  host === "localhost" || host === "127.0.0.1" || host === "::1" || host === "0.0.0.0";

const isTruthyFlag = (value: string | null): boolean => value === "1" || value === "true" || value === "yes" || value === "on";

/**
 * Debug toggle for chat streaming wire events.
 *
 * Enable via:
 * - query param: `?oa_debug_wire=1`
 * - localStorage: `localStorage.setItem("oa.debug.wire", "1")`
 *
 * Defaults to enabled on localhost to make local debugging obvious.
 */
const isChatWireDebugEnabled = (): boolean => {
  if (typeof window === "undefined") return false;
  try {
    const url = new URL(window.location.href);
    if (isTruthyFlag(url.searchParams.get("oa_debug_wire"))) return true;
    if (isTruthyFlag(window.localStorage.getItem("oa.debug.wire"))) return true;
    return isLocalHost(url.hostname);
  } catch {
    return false;
  }
};

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
  readonly telemetryState: ReturnType<typeof createChatTelemetryState>;
  readonly debug: {
    readonly seenPartKeys: Set<string>;
    readonly lastMessageFingerprintById: Map<string, string>;
  };
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

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value != null && typeof value === "object" && !Array.isArray(value);

const asString = (value: unknown): string | null =>
  typeof value === "string" && value.length > 0 ? value : null;

const asFiniteNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

type SnapshotMessageRow = {
  readonly messageId: string;
  readonly role: string;
  readonly status: string;
  readonly text: string;
  readonly runId: string | null;
};

type SnapshotPartRow = {
  readonly messageId: string;
  readonly runId: string;
  readonly seq: number;
  readonly part: unknown;
};

const parseSnapshotMessages = (snapshot: unknown): ReadonlyArray<SnapshotMessageRow> => {
  if (!isRecord(snapshot)) return [];
  if (!Array.isArray(snapshot.messages)) return [];

  const out: Array<SnapshotMessageRow> = [];
  for (const row of snapshot.messages) {
    if (!isRecord(row)) continue;
    const messageId = asString(row.messageId);
    if (!messageId) continue;
    out.push({
      messageId,
      role: typeof row.role === "string" ? row.role : "",
      status: typeof row.status === "string" ? row.status : "",
      text: typeof row.text === "string" ? row.text : "",
      runId: asString(row.runId),
    });
  }
  return out;
};

const parseSnapshotParts = (snapshot: unknown): ReadonlyArray<SnapshotPartRow> => {
  if (!isRecord(snapshot)) return [];
  if (!Array.isArray(snapshot.parts)) return [];

  const out: Array<SnapshotPartRow> = [];
  for (const row of snapshot.parts) {
    if (!isRecord(row)) continue;
    const messageId = asString(row.messageId);
    const runId = asString(row.runId);
    const seq = asFiniteNumber(row.seq);
    if (!messageId || !runId || seq == null) continue;
    out.push({
      messageId,
      runId,
      seq: Math.max(0, Math.floor(seq)),
      part: row.part,
    });
  }
  return out;
};

const parseFinishPart = (part: unknown): ChatMessageFinish | null => {
  if (!isRecord(part)) return null;
  if (part.type !== "finish") return null;

  const usageRaw = isRecord(part.usage) ? part.usage : null;
  const usage = usageRaw
    ? {
        ...(asFiniteNumber(usageRaw.inputTokens) != null
          ? { inputTokens: Number(usageRaw.inputTokens) }
          : {}),
        ...(asFiniteNumber(usageRaw.outputTokens) != null
          ? { outputTokens: Number(usageRaw.outputTokens) }
          : {}),
        ...(asFiniteNumber(usageRaw.totalTokens) != null
          ? { totalTokens: Number(usageRaw.totalTokens) }
          : {}),
        ...(asFiniteNumber(usageRaw.promptTokens) != null
          ? { promptTokens: Number(usageRaw.promptTokens) }
          : {}),
        ...(asFiniteNumber(usageRaw.completionTokens) != null
          ? { completionTokens: Number(usageRaw.completionTokens) }
          : {}),
      }
    : undefined;

  const finish: ChatMessageFinish = {
    ...(typeof part.reason === "string" ? { reason: part.reason } : {}),
    ...(usage && Object.keys(usage).length > 0 ? { usage } : {}),
    ...(typeof part.modelId === "string" ? { modelId: part.modelId } : {}),
    ...(typeof part.provider === "string" ? { provider: part.provider } : {}),
    ...(typeof part.modelRoute === "string" ? { modelRoute: part.modelRoute } : {}),
    ...(typeof part.modelFallbackId === "string" ? { modelFallbackId: part.modelFallbackId } : {}),
    ...(typeof part.timeToFirstTokenMs === "number"
      ? { timeToFirstTokenMs: part.timeToFirstTokenMs }
      : {}),
    ...(typeof part.timeToCompleteMs === "number" ? { timeToCompleteMs: part.timeToCompleteMs } : {}),
  };

  return Object.keys(finish).length > 0 ? finish : null;
};

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
          telemetryState: createChatTelemetryState(),
          debug: { seenPartKeys: new Set(), lastMessageFingerprintById: new Map() },
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
        telemetryState: createChatTelemetryState(),
        debug: { seenPartKeys: new Set(), lastMessageFingerprintById: new Map() },
      };

      sessions.set(threadId, session);

      const stream = convex.subscribeQuery(api.autopilot.messages.getThreadSnapshot, {
        threadId,
      });

      const emitChatEvent = (name: string, fields: Record<string, unknown>): void => {
        Effect.runFork(
          telemetry
            .withNamespace("chat.service")
            .event(name, { threadId, ...fields })
            .pipe(Effect.catchAll(() => Effect.void)),
        );
      };

      const fiber = yield* Stream.runForEach(stream, (snap) =>
        Effect.sync(() => {
          const messagesRaw = parseSnapshotMessages(snap);
          const partsRaw = parseSnapshotParts(snap);

          if (isChatWireDebugEnabled()) {
            // Snapshot summary (backend -> client).
            console.log("[oa:chat:snapshot]", {
              threadId,
              messages: messagesRaw.length,
              parts: partsRaw.length,
              tsMs: Date.now(),
            });

            // Log each unique part row once (LLM/tool/backend wire events).
            // Note: snapshots often include the full part history; de-dup by (runId, messageId, seq).
            // Cap memory to keep long sessions safe during debugging.
            if (session.debug.seenPartKeys.size > 25_000) session.debug.seenPartKeys.clear();
            for (const p of partsRaw) {
              const key = `${p.runId}:${p.messageId}:${p.seq}`;
              if (session.debug.seenPartKeys.has(key)) continue;
              session.debug.seenPartKeys.add(key);
              console.log("[oa:chat:wire]", {
                threadId,
                messageId: p.messageId,
                runId: p.runId,
                seq: p.seq,
                part: p.part,
              });
            }

            // Log message row transitions (status/final text updates).
            if (session.debug.lastMessageFingerprintById.size > 5000) session.debug.lastMessageFingerprintById.clear();
            for (const m of messagesRaw) {
              const fp = `${m.role}|${m.status}|${m.runId ?? ""}|${m.text.length}`;
              const prev = session.debug.lastMessageFingerprintById.get(m.messageId);
              if (prev === fp) continue;
              session.debug.lastMessageFingerprintById.set(m.messageId, fp);
              console.log("[oa:chat:message]", {
                threadId,
                messageId: m.messageId,
                role: m.role,
                status: m.status,
                runId: m.runId,
                textLength: m.text.length,
                textPreview: m.text.slice(0, 280),
              });
            }
          }

          // Rebuild messages deterministically from Convex rows.
          const byMessageId = new Map<string, ActiveStream>();
          let activeRunId: string | null = null;

          const partsSorted = [...partsRaw]
            .sort((a, b) => a.seq - b.seq);

          for (const p of partsSorted) {
            const part = p.part;
            if (!part || typeof part !== "object") continue;

            const active: ActiveStream =
              byMessageId.get(p.messageId) ?? { id: p.runId, messageId: p.messageId, parts: [] };
            byMessageId.set(p.messageId, applyChatWirePart(active, part));
          }

          // Last finish part per message (for assistant LLM usage/model in metadata).
          const finishByMessageId = new Map<string, ChatMessageFinish>();
          for (const p of partsSorted) {
            const finish = parseFinishPart(p.part);
            if (finish) finishByMessageId.set(p.messageId, finish);
          }

          const finishByRunId = new Map<string, ChatMessageFinish>();
          const messages: Array<ChatMessage> = [];
          for (const m of messagesRaw) {
            if (m.role === "user") {
              messages.push({ id: m.messageId, role: "user", parts: [{ type: "text", text: m.text }] });
              continue;
            }
            if (m.role === "assistant") {
              const active = byMessageId.get(m.messageId);
              const finish = finishByMessageId.get(m.messageId);
              const baseMsg = {
                id: m.messageId,
                role: "assistant" as const,
                ...(finish ? { finish } : {}),
                ...(m.runId ? { runId: m.runId } : {}),
              } as const;
              const activeParts = active?.parts ?? [];
              const messageText = m.text;
              const hasMessageText = messageText.trim().length > 0;
              const activeText = activeParts
                .filter((p) => p?.type === "text" && typeof (p as any).text === "string")
                .map((p) => String((p as any).text ?? ""))
                .join("");
              const hasActiveText = activeText.trim().length > 0;

              if (activeParts.length > 0) {
                // The Worker streams parts into `messageParts`, and also finalizes `messages.text` as a durable fallback.
                // If we received non-text parts (or an empty text-start) but did not receive a user-visible text delta,
                // prefer the finalized `messages.text` so the UI never renders an empty assistant bubble.
                if (hasMessageText && m.status !== "streaming") {
                  // If the streamed text doesn't match the finalized message text, replace it.
                  // This also covers the case where appendParts failed after we already computed the final text.
                  if (activeText.trim() !== messageText.trim()) {
                    const withoutText = activeParts.filter((p) => p?.type !== "text");
                    messages.push({
                      ...baseMsg,
                      parts: [...withoutText, { type: "text", text: messageText, state: "done" }],
                    });
                  } else {
                    messages.push({ ...baseMsg, parts: [...activeParts] });
                  }
                } else if (hasMessageText && !hasActiveText) {
                  messages.push({
                    ...baseMsg,
                    parts: [...activeParts, { type: "text", text: messageText, state: "done" }],
                  });
                } else {
                  messages.push({ ...baseMsg, parts: [...activeParts] });
                }
              } else if (hasMessageText) {
                messages.push({
                  ...baseMsg,
                  parts: [{ type: "text", text: messageText, state: "done" }],
                });
              } else {
                messages.push({ ...baseMsg, parts: [] });
              }

              if (m.status === "streaming" && m.runId) {
                activeRunId = m.runId;
              }
              if (m.runId && finish) finishByRunId.set(m.runId, finish);
              continue;
            }
          }

          const previousRunId = session.activeRunId;
          const hadHydratedSnapshot = session.telemetryState.hasHydratedSnapshot;

          session.activeRunId = activeRunId;
          session.messages = messages;

          if (!hadHydratedSnapshot) {
            const hydrated = hydrateChatTelemetryState(session.telemetryState, messages);
            emitChatEvent(hydrated.name, hydrated.fields);
          } else {
            const messageEvents = collectChatTelemetryEventsForSnapshot(session.telemetryState, messages);
            for (const event of messageEvents) emitChatEvent(event.name, event.fields);

            const streamEvents = collectStreamingTransitionEvents({
              previousRunId,
              nextRunId: activeRunId,
              finishByRunId,
            });
            for (const event of streamEvents) emitChatEvent(event.name, event.fields);
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
            return Effect.die(new Error(`[ChatService] Session missing after open() threadId=${threadId}`));
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
          if (!runId) {
            yield* telemetry
              .withNamespace("chat.service")
              .event("chat.stop_ignored", { threadId, reason: "no_active_run" })
              .pipe(Effect.catchAll(() => Effect.void));
            return;
          }

          yield* telemetry
            .withNamespace("chat.service")
            .event("chat.stop_requested", { threadId, runId })
            .pipe(Effect.catchAll(() => Effect.void));

          const response = yield* Effect.tryPromise({
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

          yield* telemetry
            .withNamespace("chat.service")
            .event("chat.stop_complete", {
              threadId,
              runId,
              ok: response instanceof Response ? response.ok : false,
              status: response instanceof Response ? response.status : null,
            })
            .pipe(Effect.catchAll(() => Effect.void));

          session.localStatus.status = null;
          session.localStatus.errorText = null;
        }),
      );
    });

    const clearHistory = Effect.fn("ChatService.clearHistory")(function* (threadId: string) {
      yield* telemetry
        .withNamespace("chat.service")
        .event("chat.history_clear_requested", { threadId })
        .pipe(Effect.catchAll(() => Effect.void));

      const exit = yield* convex
        .mutation(api.autopilot.messages.clearMessages, { threadId, keepWelcome: true })
        .pipe(Effect.exit);

      if (Exit.isSuccess(exit)) {
        yield* telemetry
          .withNamespace("chat.service")
          .event("chat.history_cleared", { threadId })
          .pipe(Effect.catchAll(() => Effect.void));
        return;
      }

      yield* telemetry
        .withNamespace("chat.service")
        .event("chat.history_clear_failed", {
          threadId,
          cause: Cause.pretty(exit.cause).trim() || "clear_history_failed",
        })
        .pipe(Effect.catchAll(() => Effect.void));
    });

    return ChatService.of({ getOwnedThreadId, open, send, stop, clearHistory });
  }),
);
