import { Effect } from "effect";

import { ChatSnapshotCacheLive, ChatSnapshotCacheService } from "../../../effect/chatSnapshotCache";

import type { ChatSnapshot } from "../../../effect/chat";
import type { AppRuntime } from "../../../effect/runtime";

export const HOME_CHAT_SNAPSHOT_CACHE_MAX_AGE_MS = 1000 * 60 * 60 * 24;

const runChatSnapshotCacheEffectSync = <A>(
  runtime: AppRuntime | undefined,
  effect: Effect.Effect<A, never, ChatSnapshotCacheService>,
): A => {
  if (runtime) return runtime.runSync(effect);
  return Effect.runSync(effect.pipe(Effect.provide(ChatSnapshotCacheLive)));
};

export const readCachedSnapshotForUser = (
  input: {
    readonly runtime: AppRuntime | undefined;
    readonly userId: string;
    readonly maxAgeMs?: number;
  },
): { readonly threadId: string; readonly snapshot: ChatSnapshot } | null => {
  if (!input.userId) return null;
  const cached = runChatSnapshotCacheEffectSync(
    input.runtime,
    Effect.gen(function* () {
      const cache = yield* ChatSnapshotCacheService;
      return yield* cache.readLatestForUser({
        userId: input.userId,
        maxAgeMs: input.maxAgeMs ?? HOME_CHAT_SNAPSHOT_CACHE_MAX_AGE_MS,
      });
    }),
  );
  if (!cached) return null;
  return { threadId: cached.threadId, snapshot: cached.snapshot };
};

export const writeCachedSnapshotForUser = (
  input: {
    readonly runtime: AppRuntime | undefined;
    readonly userId: string;
    readonly threadId: string;
    readonly snapshot: ChatSnapshot;
  },
): void => {
  if (!input.userId || !input.threadId) return;
  runChatSnapshotCacheEffectSync(
    input.runtime,
    Effect.gen(function* () {
      const cache = yield* ChatSnapshotCacheService;
      yield* cache.writeLatestForUser({
        userId: input.userId,
        threadId: input.threadId,
        snapshot: input.snapshot,
      });
    }),
  );
};

export const clearCachedSnapshotForUser = (
  input: {
    readonly runtime: AppRuntime | undefined;
    readonly userId: string;
  },
): void => {
  if (!input.userId) return;
  runChatSnapshotCacheEffectSync(
    input.runtime,
    Effect.gen(function* () {
      const cache = yield* ChatSnapshotCacheService;
      yield* cache.clearForUser(input.userId);
    }),
  );
};

export const shouldSkipHydratedPlaceholder = (input: {
  readonly skippedHydratedPlaceholder: boolean;
  readonly hasHydratedSnapshot: boolean;
  readonly hydratedSnapshotMessageCount: number;
  readonly nextSnapshotMessageCount: number;
  readonly nextSnapshotStatus: string;
  readonly nextSnapshotErrorText: string | null;
}): boolean =>
  !input.skippedHydratedPlaceholder &&
  input.hasHydratedSnapshot &&
  input.hydratedSnapshotMessageCount > 0 &&
  input.nextSnapshotMessageCount === 0 &&
  input.nextSnapshotStatus === "ready" &&
  input.nextSnapshotErrorText == null;
