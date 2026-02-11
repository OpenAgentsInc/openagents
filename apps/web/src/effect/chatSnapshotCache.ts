import { Context, Effect, Layer } from "effect";

import type { ChatSnapshot } from "./chat";
import type { ChatMessage, ChatMessageFinish, ChatPart } from "./chatProtocol";

const STORAGE_KEY_PREFIX = "oa.home.chat.snapshot.v1.";
const DEFAULT_MAX_AGE_MS = 1000 * 60 * 60 * 24;
const MAX_CACHED_MESSAGES = 120;
const MAX_STORED_CHARS = 300_000;

export type CachedChatSnapshot = Readonly<{
  threadId: string;
  updatedAtMs: number;
  snapshot: ChatSnapshot;
}>;

export type ChatSnapshotCacheApi = {
  readonly readLatestForUser: (input: {
    readonly userId: string;
    readonly maxAgeMs?: number;
  }) => Effect.Effect<CachedChatSnapshot | null>;
  readonly writeLatestForUser: (input: {
    readonly userId: string;
    readonly threadId: string;
    readonly snapshot: ChatSnapshot;
  }) => Effect.Effect<void>;
  readonly clearForUser: (userId: string) => Effect.Effect<void>;
};

export class ChatSnapshotCacheService extends Context.Tag("@openagents/web/ChatSnapshotCacheService")<
  ChatSnapshotCacheService,
  ChatSnapshotCacheApi
>() {}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value != null && typeof value === "object";

const asNonEmptyString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value : null;

const asFiniteNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;

const storageKeyForUser = (userId: string): string =>
  `${STORAGE_KEY_PREFIX}${encodeURIComponent(userId)}`;

const hasBrowserStorage = (): boolean =>
  typeof window !== "undefined" &&
  "localStorage" in window &&
  window.localStorage != null;

const sanitizePart = (value: unknown): ChatPart | null => {
  if (!isRecord(value)) return null;
  const type = asNonEmptyString(value.type);
  if (!type) return null;
  return { ...value, type };
};

const sanitizeMessageFinish = (value: unknown): ChatMessageFinish | undefined => {
  if (!isRecord(value)) return undefined;

  const usageRaw = isRecord(value.usage) ? value.usage : null;
  const usage = usageRaw
    ? {
        ...(asFiniteNumber(usageRaw.inputTokens) != null ? { inputTokens: Number(usageRaw.inputTokens) } : {}),
        ...(asFiniteNumber(usageRaw.outputTokens) != null ? { outputTokens: Number(usageRaw.outputTokens) } : {}),
        ...(asFiniteNumber(usageRaw.totalTokens) != null ? { totalTokens: Number(usageRaw.totalTokens) } : {}),
        ...(asFiniteNumber(usageRaw.promptTokens) != null ? { promptTokens: Number(usageRaw.promptTokens) } : {}),
        ...(asFiniteNumber(usageRaw.completionTokens) != null
          ? { completionTokens: Number(usageRaw.completionTokens) }
          : {}),
      }
    : {};

  const finish: ChatMessageFinish = {
    ...(asNonEmptyString(value.reason) ? { reason: String(value.reason) } : {}),
    ...(Object.keys(usage).length > 0 ? { usage } : {}),
    ...(asNonEmptyString(value.modelId) ? { modelId: String(value.modelId) } : {}),
    ...(asNonEmptyString(value.provider) ? { provider: String(value.provider) } : {}),
    ...(asNonEmptyString(value.modelRoute) ? { modelRoute: String(value.modelRoute) } : {}),
    ...(asNonEmptyString(value.modelFallbackId) ? { modelFallbackId: String(value.modelFallbackId) } : {}),
    ...(asFiniteNumber(value.timeToFirstTokenMs) != null
      ? { timeToFirstTokenMs: Number(value.timeToFirstTokenMs) }
      : {}),
    ...(asFiniteNumber(value.timeToCompleteMs) != null ? { timeToCompleteMs: Number(value.timeToCompleteMs) } : {}),
  };

  return Object.keys(finish).length > 0 ? finish : undefined;
};

const sanitizeMessage = (value: unknown): ChatMessage | null => {
  if (!isRecord(value)) return null;

  const id = asNonEmptyString(value.id);
  if (!id) return null;

  if (value.role !== "user" && value.role !== "assistant") return null;
  const role = value.role;

  const parts = Array.isArray(value.parts)
    ? value.parts.map((part) => sanitizePart(part)).filter((part): part is ChatPart => part != null)
    : [];

  const finish = sanitizeMessageFinish(value.finish);
  const runId = asNonEmptyString(value.runId);

  return {
    id,
    role,
    parts,
    ...(runId ? { runId } : {}),
    ...(finish ? { finish } : {}),
  };
};

const sanitizeSnapshot = (value: unknown): ChatSnapshot | null => {
  if (!isRecord(value)) return null;

  const status =
    value.status === "ready" ||
    value.status === "submitted" ||
    value.status === "streaming" ||
    value.status === "error"
      ? value.status
      : "ready";
  const errorText = typeof value.errorText === "string" ? value.errorText : null;
  const messages = Array.isArray(value.messages)
    ? value.messages.map((message) => sanitizeMessage(message)).filter((message): message is ChatMessage => message != null)
    : [];

  return { messages, status, errorText };
};

const trimSnapshotForStorage = (snapshot: ChatSnapshot): ChatSnapshot => {
  let messages = snapshot.messages.slice(-MAX_CACHED_MESSAGES);
  let candidate: ChatSnapshot = {
    messages,
    status: snapshot.status,
    errorText: snapshot.errorText,
  };
  let payloadSize = JSON.stringify(candidate).length;

  while (payloadSize > MAX_STORED_CHARS && messages.length > 1) {
    const nextCount = Math.max(1, Math.floor(messages.length * 0.75));
    messages = messages.slice(-nextCount);
    candidate = { ...candidate, messages };
    payloadSize = JSON.stringify(candidate).length;
  }

  return candidate;
};

const readLatestForUser = Effect.fn("ChatSnapshotCache.readLatestForUser")(function* (input: {
  readonly userId: string;
  readonly maxAgeMs?: number;
}) {
  return yield* Effect.sync(() => {
    const userId = input.userId.trim();
    if (!userId) return null;
    if (!hasBrowserStorage()) return null;

    const key = storageKeyForUser(userId);
    const raw = window.localStorage.getItem(key);
    if (!raw) return null;

    try {
      const parsed: unknown = JSON.parse(raw);
      if (!isRecord(parsed)) return null;

      const threadId = asNonEmptyString(parsed.threadId);
      const updatedAtMs = asFiniteNumber(parsed.updatedAtMs);
      const snapshot = sanitizeSnapshot(parsed.snapshot);
      if (!threadId || updatedAtMs == null || !snapshot) return null;

      const maxAgeMs = input.maxAgeMs ?? DEFAULT_MAX_AGE_MS;
      if (maxAgeMs > 0 && Date.now() - updatedAtMs > maxAgeMs) {
        window.localStorage.removeItem(key);
        return null;
      }

      return { threadId, updatedAtMs, snapshot } satisfies CachedChatSnapshot;
    } catch {
      return null;
    }
  });
});

const writeLatestForUser = Effect.fn("ChatSnapshotCache.writeLatestForUser")(function* (input: {
  readonly userId: string;
  readonly threadId: string;
  readonly snapshot: ChatSnapshot;
}) {
  yield* Effect.sync(() => {
    const userId = input.userId.trim();
    const threadId = input.threadId.trim();
    if (!userId || !threadId) return;
    if (!hasBrowserStorage()) return;

    const key = storageKeyForUser(userId);
    const trimmedSnapshot = trimSnapshotForStorage(input.snapshot);
    const payload = JSON.stringify({
      threadId,
      updatedAtMs: Date.now(),
      snapshot: trimmedSnapshot,
    });

    try {
      window.localStorage.setItem(key, payload);
    } catch {
      // Best effort cache only.
    }
  });
});

const clearForUser = Effect.fn("ChatSnapshotCache.clearForUser")(function* (userIdRaw: string) {
  yield* Effect.sync(() => {
    const userId = userIdRaw.trim();
    if (!userId) return;
    if (!hasBrowserStorage()) return;
    window.localStorage.removeItem(storageKeyForUser(userId));
  });
});

export const ChatSnapshotCacheLive = Layer.succeed(
  ChatSnapshotCacheService,
  ChatSnapshotCacheService.of({
    readLatestForUser,
    writeLatestForUser,
    clearForUser,
  }),
);
